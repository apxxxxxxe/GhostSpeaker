use crate::engine::bouyomichan::predict::BouyomichanPredictor;
use crate::engine::coeiroink_v2::predict::CoeiroinkV2Predictor;
use crate::engine::voicevox_family::predict::VoicevoxFamilyPredictor;
use crate::engine::{engine_from_port, get_speaker_getters, Engine, Predictor, NO_VOICE_UUID};
use crate::format::{split_by_punctuation, split_dialog};
use crate::player::play_wav;
use crate::system::get_port_opener_path;
use crate::variables::GHOSTS_VOICES;
use crate::variables::*;
use once_cell::sync::Lazy;
use std::time::{Duration, Instant};

// タイムアウト定数
const GRACEFUL_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(8);
const RUNTIME_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(3);
const QUEUE_POLL_TIMEOUT: Duration = Duration::from_millis(100);

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use tokio::sync::Mutex;

pub(crate) static CONNECTION_DIALOGS: Lazy<StdMutex<Vec<String>>> =
  Lazy::new(|| StdMutex::new(Vec::new()));

pub(crate) static RUNTIME: Lazy<StdMutex<Option<tokio::runtime::Runtime>>> =
  Lazy::new(|| match tokio::runtime::Runtime::new() {
    Ok(runtime) => StdMutex::new(Some(runtime)),
    Err(e) => {
      error!("Failed to create tokio runtime: {}", e);
      StdMutex::new(None)
    }
  });
pub(crate) static SPEAK_HANDLERS: Lazy<Mutex<Vec<tokio::task::JoinHandle<()>>>> =
  Lazy::new(|| Mutex::new(Vec::new()));
pub(crate) static PREDICT_HANDLER: Lazy<Mutex<Option<tokio::task::JoinHandle<()>>>> =
  Lazy::new(|| Mutex::new(None));
pub(crate) static PREDICT_QUEUE: Lazy<Arc<Mutex<VecDeque<(String, String)>>>> =
  Lazy::new(|| Arc::new(Mutex::new(VecDeque::new())));
pub(crate) static PREDICT_STOPPER: Lazy<Arc<Mutex<bool>>> =
  Lazy::new(|| Arc::new(Mutex::new(false)));
pub(crate) static PLAY_HANDLER: Lazy<Mutex<Option<tokio::task::JoinHandle<()>>>> =
  Lazy::new(|| Mutex::new(None));
pub(crate) static PLAY_QUEUE: Lazy<Arc<Mutex<VecDeque<Vec<u8>>>>> =
  Lazy::new(|| Arc::new(Mutex::new(VecDeque::new())));
pub(crate) static PLAY_STOPPER: Lazy<Arc<Mutex<bool>>> = Lazy::new(|| Arc::new(Mutex::new(false)));
pub(crate) static SPEAK_QUEUE_STOPPER: Lazy<Arc<Mutex<bool>>> =
  Lazy::new(|| Arc::new(Mutex::new(false)));

fn init_speak_queue() {
  let mut runtime_guard = match RUNTIME.lock() {
    Ok(guard) => guard,
    Err(e) => {
      error!("Failed to lock runtime: {}", e);
      return;
    }
  };

  let runtime = match runtime_guard.as_mut() {
    Some(rt) => rt,
    None => {
      error!("Runtime is not initialized");
      return;
    }
  };

  let mut speak_handlers = Vec::new();
  for (engine, getter) in get_speaker_getters() {
    let handler = runtime.spawn(async move {
      let mut consecutive_failures = 0;
      const MAX_CONSECUTIVE_FAILURES: u32 = 10; // 最大連続失敗回数
      const BACKOFF_BASE: u64 = 2; // バックオフ基数（秒）

      loop {
        // 停止チェック
        if *SPEAK_QUEUE_STOPPER.lock().await {
          debug!("Speak queue stopping for engine: {}", engine.name());
          break;
        }
        if let Some(port_opener_path) = get_port_opener_path(format!("{}", engine.port())) {
          match getter.get_speakers_info().await {
            Ok(speakers_info) => {
              consecutive_failures = 0; // 成功時はリセット
              let mut connection_status = CURRENT_CONNECTION_STATUS.write().await;
              if connection_status.get(&engine).is_none()
                || connection_status.get(&engine).is_some_and(|v| !*v)
              {
                {
                  if let Ok(mut dialogs) = CONNECTION_DIALOGS.lock() {
                    dialogs.push(format!("{} が接続されました", engine.name()));
                  } else {
                    error!("Failed to lock CONNECTION_DIALOGS for connection message");
                  }
                }
                // 接続時、ポートを開いているプロセスのパスを記録
                if let Ok(mut engine_path) = ENGINE_PATH.write() {
                  engine_path.insert(engine, port_opener_path);
                } else {
                  error!("Failed to lock ENGINE_PATH for engine: {}", engine.name());
                }
                let mut auto_start = ENGINE_AUTO_START.write().await;
                if auto_start.get(&engine).is_none() {
                  auto_start.insert(engine, false);
                }
              }
              connection_status.insert(engine, true);
              SPEAKERS_INFO.write().await.insert(engine, speakers_info);
            }
            Err(e) => {
              consecutive_failures += 1;
              error!(
                "Error: {} (consecutive failures: {})",
                e, consecutive_failures
              );

              // 最大失敗回数に達した場合の処理
              if consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                error!(
                  "Too many consecutive failures for engine: {}, backing off",
                  engine.name()
                );
                let backoff_time = std::cmp::min(BACKOFF_BASE.pow(consecutive_failures / 5), 60); // 最大60秒
                tokio::time::sleep(Duration::from_secs(backoff_time)).await;
              }

              let mut connection_status = CURRENT_CONNECTION_STATUS.write().await;
              if connection_status.get(&engine).is_some_and(|v| *v) {
                if let Ok(mut dialogs) = CONNECTION_DIALOGS.lock() {
                  dialogs.push(format!("{} が切断されました", engine.name()));
                } else {
                  error!("Failed to lock CONNECTION_DIALOGS for disconnection message");
                }
              }
              connection_status.insert(engine, false);
              SPEAKERS_INFO.write().await.remove(&engine);
            }
          }
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
      }
    });
    speak_handlers.push(handler);
  }
  futures::executor::block_on(async {
    *SPEAK_HANDLERS.lock().await = speak_handlers;
  });
}

fn init_predict_queue() {
  let predict_queue_cln = PREDICT_QUEUE.clone();
  let predict_stopper_cln = PREDICT_STOPPER.clone();

  let handler = {
    let mut runtime_guard = match RUNTIME.lock() {
      Ok(guard) => guard,
      Err(e) => {
        error!("Failed to lock runtime for predict queue: {}", e);
        return;
      }
    };

    let runtime = match runtime_guard.as_mut() {
      Some(rt) => rt,
      None => {
        error!("Runtime is not initialized for predict queue");
        return;
      }
    };

    runtime.spawn(async move {
      let mut last_activity = Instant::now();
      const MAX_IDLE_TIME: Duration = Duration::from_secs(300); // 5分間のアイドル時間

      loop {
        {
          if predict_queue_cln.lock().await.is_empty() {
            if *predict_stopper_cln.lock().await {
              break;
            }

            // アイドル時間チェック
            if last_activity.elapsed() > MAX_IDLE_TIME {
              debug!("Predict queue idle for too long, continuing...");
              last_activity = Instant::now();
            }

            tokio::time::sleep(QUEUE_POLL_TIMEOUT).await;
            continue;
          }
        }

        let parg;
        {
          let mut guard = predict_queue_cln.lock().await;
          parg = guard.pop_front();
        }

        match parg {
          None => continue,
          Some(parg) => {
            last_activity = Instant::now(); // アクティビティ更新
            match args_to_predictors(parg) {
              None => continue,
              Some(predictors) => {
                for predictor in predictors {
                  match predictor.predict().await {
                    Ok(res) => {
                      debug!("pushing to play");
                      futures::executor::block_on(async {
                        PLAY_QUEUE.lock().await.push_back(res);
                      });
                      debug!("pushed to play");
                    }
                    Err(e) => {
                      debug!("predict failed: {}", e);
                    }
                  }
                }
              }
            }
          }
        }
      }
    })
  };
  futures::executor::block_on(async {
    *PREDICT_HANDLER.lock().await = Some(handler);
  });
}

pub(crate) fn init_play_queue() {
  let play_queue_cln = PLAY_QUEUE.clone();
  let play_stopper_cln = PLAY_STOPPER.clone();

  let handler = {
    let mut runtime_guard = match RUNTIME.lock() {
      Ok(guard) => guard,
      Err(e) => {
        error!("Failed to lock runtime for play queue: {}", e);
        return;
      }
    };

    let runtime = match runtime_guard.as_mut() {
      Some(rt) => rt,
      None => {
        error!("Runtime is not initialized for play queue");
        return;
      }
    };

    runtime.spawn(async move {
      let mut last_activity = Instant::now();
      const MAX_IDLE_TIME: Duration = Duration::from_secs(300); // 5分間のアイドル時間

      loop {
        {
          if play_queue_cln.lock().await.is_empty() {
            if *play_stopper_cln.lock().await {
              break;
            }

            // アイドル時間チェック
            if last_activity.elapsed() > MAX_IDLE_TIME {
              debug!("Play queue idle for too long, continuing...");
              last_activity = Instant::now();
            }

            tokio::time::sleep(QUEUE_POLL_TIMEOUT).await;
            continue;
          }
        }

        let wav;
        {
          let mut guard = play_queue_cln.lock().await;
          wav = guard.pop_front();
        }
        if let Some(data) = wav {
          if !data.is_empty() {
            last_activity = Instant::now(); // アクティビティ更新
            debug!("{}", format!("play: {}", data.len()));
            if let Err(e) = play_wav(data) {
              error!("play_wav failed: {}", e);
            };
          }
        }
      }
    })
  };
  futures::executor::block_on(async {
    *PLAY_HANDLER.lock().await = Some(handler);
  });
}

pub(crate) fn init_queues() {
  init_speak_queue();
  init_predict_queue();
  init_play_queue();
}

pub(crate) fn stop_queues() -> Result<
  (),
  std::sync::PoisonError<
    std::sync::MutexGuard<'static, std::option::Option<tokio::runtime::Runtime>>,
  >,
> {
  debug!("{}", "stopping queue");

  // 同期再生ステートをクリア
  *SYNC_STATE.lock().unwrap() = None;
  
  // 音声再生を即座に強制停止
  if let Ok(mut force_stop) = crate::player::FORCE_STOP_SINK.lock() {
    *force_stop = true;
    debug!("{}", "set force stop sink flag");
  } else {
    error!("Failed to set FORCE_STOP_SINK flag");
  }
  
  // 全停止フラグを設定（協調的停止の開始）
  futures::executor::block_on(async {
    *PLAY_STOPPER.lock().await = true;
    *PREDICT_STOPPER.lock().await = true;
    *SPEAK_QUEUE_STOPPER.lock().await = true;
    debug!("{}", "set all stop flags");
  });
  
  // 協調的停止を待機（abort()は使用しない）
  let start_time = Instant::now();
  while start_time.elapsed() < GRACEFUL_SHUTDOWN_TIMEOUT {
    // 全タスクの完了状況を確認
    let (play_stopped, predict_stopped, speak_stopped) = futures::executor::block_on(async {
      let play_stopped = PLAY_HANDLER.lock().await.as_ref().map_or(true, |h| h.is_finished());
      let predict_stopped = PREDICT_HANDLER.lock().await.as_ref().map_or(true, |h| h.is_finished());
      let speak_stopped = SPEAK_HANDLERS.lock().await.iter().all(|h| h.is_finished());
      (play_stopped, predict_stopped, speak_stopped)
    });
    
    if play_stopped && predict_stopped && speak_stopped {
      debug!("{}", "all tasks stopped gracefully");
      break;
    }
    
    // 進行状況をログ出力
    if !play_stopped {
      debug!("{}", "waiting for play handler to finish");
    }
    if !predict_stopped {
      debug!("{}", "waiting for predict handler to finish");
    }
    if !speak_stopped {
      debug!("{}", "waiting for speak handlers to finish");
    }
    
    std::thread::sleep(Duration::from_millis(200));
  }
  
  // タイムアウト時もabortはせず、警告のみ
  if start_time.elapsed() >= GRACEFUL_SHUTDOWN_TIMEOUT {
    warn!("Some tasks did not stop within timeout, proceeding with shutdown");
    let (play_stopped, predict_stopped, speak_stopped) = futures::executor::block_on(async {
      let play_stopped = PLAY_HANDLER.lock().await.as_ref().map_or(true, |h| h.is_finished());
      let predict_stopped = PREDICT_HANDLER.lock().await.as_ref().map_or(true, |h| h.is_finished());
      let speak_stopped = SPEAK_HANDLERS.lock().await.iter().all(|h| h.is_finished());
      (play_stopped, predict_stopped, speak_stopped)
    });
    warn!("Task status - play: {}, predict: {}, speak: {}", play_stopped, predict_stopped, speak_stopped);
  }
  
  debug!("{}", "stopped queue");
  if let Some(runtime) = RUNTIME.lock()?.take() {
    runtime.shutdown_timeout(RUNTIME_SHUTDOWN_TIMEOUT);
  }
  Ok(())
}

pub(crate) fn push_to_prediction(text: String, ghost_name: String) {
  futures::executor::block_on(async {
    // 処理が重いので、別スレッドに投げてそっちでPredictorを作る
    PREDICT_QUEUE.lock().await.push_back((text, ghost_name));
  });
}

fn args_to_predictors(
  args: (String, String),
) -> Option<VecDeque<Box<dyn Predictor + Send + Sync>>> {
  let (text, ghost_name) = args;
  build_segments(text, ghost_name, false).map(|segments| {
    segments
      .into_iter()
      .map(|seg| seg.predictor)
      .collect()
  })
}

pub(crate) struct SyncSegment {
  pub text: String,
  pub scope: usize,
  pub predictor: Box<dyn Predictor + Send + Sync>,
}

pub(crate) fn build_segments(
  text: String,
  ghost_name: String,
  sync_mode: bool,
) -> Option<Vec<SyncSegment>> {
  let mut segments: Vec<SyncSegment> = Vec::new();
  let connected_engines = futures::executor::block_on(async {
    CURRENT_CONNECTION_STATUS
      .read()
      .await
      .clone()
      .iter()
      .filter(|(_, v)| **v)
      .map(|(k, _)| *k)
      .collect::<Vec<_>>()
  });
  if connected_engines.is_empty() {
    debug!("no engine connected: skip: {}", text);
    return None;
  }

  debug!("{}", format!("predicting: {}", text));
  let devide_by_lines = GHOSTS_VOICES
    .read()
    .unwrap()
    .get(&ghost_name)
    .unwrap()
    .devide_by_lines;

  let speak_by_punctuation = SPEAK_BY_PUNCTUATION.read().unwrap();

  let ghosts_voices = GHOSTS_VOICES.write().unwrap();
  let speakers = &ghosts_voices.get(&ghost_name).unwrap().voices;
  for dialog in split_dialog(text, devide_by_lines) {
    if dialog.text.is_empty() {
      continue;
    }

    let initial_speaker = &INITIAL_VOICE.read().unwrap();
    debug!("selecting speaker: {}", dialog.scope);
    let speaker = match speakers.get(dialog.scope) {
      Some(speaker) => {
        if let Some(sp) = speaker {
          sp.clone()
        } else {
          (*initial_speaker).clone()
        }
      }
      None => (*initial_speaker).clone(),
    };

    if speaker.speaker_uuid == NO_VOICE_UUID {
      continue;
    }
    let mut voice_not_found = false;
    futures::executor::block_on(async {
      if let Some(speakers_by_engine) = &SPEAKERS_INFO
        .read()
        .await
        .get(&(engine_from_port(speaker.port).unwrap()))
      {
        if !speakers_by_engine
          .iter()
          .any(|s| s.speaker_uuid == speaker.speaker_uuid)
        {
          voice_not_found = true;
        }
      }
    });
    if voice_not_found {
      continue;
    }
    let engine = engine_from_port(speaker.port).unwrap();
    let texts = if (*speak_by_punctuation || sync_mode) && engine != Engine::BouyomiChan {
      split_by_punctuation(dialog.text.clone())
    } else {
      /* 棒読みちゃんは細切れの恩恵が少ない&
      読み上げ順がばらばらになることがあるので常にまとめて読み上げる */
      vec![dialog.text.clone()]
    };
    for t in texts {
      let predictor: Box<dyn Predictor + Send + Sync> = match engine {
        Engine::CoeiroInkV2 => Box::new(CoeiroinkV2Predictor::new(
          t.clone(),
          speaker.speaker_uuid.clone(),
          speaker.style_id,
        )),
        Engine::BouyomiChan => {
          Box::new(BouyomichanPredictor::new(t.clone(), speaker.style_id))
        }
        Engine::CoeiroInkV1
        | Engine::VoiceVox
        | Engine::Lmroid
        | Engine::ShareVox
        | Engine::ItVoice
        | Engine::AivisSpeech => Box::new(VoicevoxFamilyPredictor::new(
          engine,
          t.clone(),
          speaker.style_id,
        )),
      };
      segments.push(SyncSegment {
        text: t,
        scope: dialog.scope,
        predictor,
      });
    }
  }
  Some(segments)
}

pub(crate) struct SyncPlaybackState {
  pub segments: VecDeque<SyncSegment>,
  pub ghost_name: String,
}

pub(crate) static SYNC_STATE: Lazy<StdMutex<Option<SyncPlaybackState>>> =
  Lazy::new(|| StdMutex::new(None));

static SYNC_AUDIO_DONE: Lazy<StdMutex<Arc<AtomicBool>>> =
  Lazy::new(|| StdMutex::new(Arc::new(AtomicBool::new(true))));

/// 同期モード用: Predictor.predict() を同期的に呼び出す
pub(crate) fn sync_predict(
  predictor: &dyn Predictor,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
  let handle = RUNTIME
    .lock()
    .unwrap()
    .as_ref()
    .unwrap()
    .handle()
    .clone();
  handle.block_on(predictor.predict())
}

/// 同期モード用: WAV を別スレッドで再生し、完了時にフラグ設定
pub(crate) fn spawn_sync_playback(wav: Vec<u8>) {
  let done = Arc::new(AtomicBool::new(false));
  *SYNC_AUDIO_DONE.lock().unwrap() = done.clone();
  // cancel_sync_playback で設定された FORCE_STOP_SINK をリセット
  if let Ok(mut force_stop) = crate::player::FORCE_STOP_SINK.lock() {
    *force_stop = false;
  }
  std::thread::spawn(move || {
    if !wav.is_empty() {
      let _ = play_wav(wav);
    }
    done.store(true, Ordering::SeqCst);
  });
}

/// 同期再生の音声が完了したか確認（非ブロッキング）
pub(crate) fn is_sync_audio_done() -> bool {
  SYNC_AUDIO_DONE.lock().unwrap().load(Ordering::SeqCst)
}

/// 同期再生をキャンセル
pub(crate) fn cancel_sync_playback() {
  *SYNC_STATE.lock().unwrap() = None;
  if let Ok(mut force_stop) = crate::player::FORCE_STOP_SINK.lock() {
    *force_stop = true;
  }
}
