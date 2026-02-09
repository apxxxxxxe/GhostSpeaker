use crate::engine::bouyomichan::predict::BouyomichanPredictor;
use crate::engine::coeiroink_v2::predict::CoeiroinkV2Predictor;
use crate::engine::voicevox_family::predict::VoicevoxFamilyPredictor;
use crate::engine::{engine_from_port, get_speaker_getters, Engine, Predictor, NO_VOICE_UUID};
use crate::format::{split_by_punctuation_with_raw, split_dialog};
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
  Lazy::new(|| {
    match tokio::runtime::Builder::new_multi_thread()
      .worker_threads(4)
      .max_blocking_threads(4)
      .enable_all()
      .build()
    {
      Ok(runtime) => StdMutex::new(Some(runtime)),
      Err(e) => {
        error!("Failed to create tokio runtime: {}", e);
        StdMutex::new(None)
      }
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
  let handle = {
    let guard = match RUNTIME.lock() {
      Ok(g) => g,
      Err(e) => {
        error!("Failed to lock runtime: {}", e);
        return;
      }
    };
    match guard.as_ref() {
      Some(rt) => rt.handle().clone(),
      None => {
        error!("Runtime is not initialized");
        return;
      }
    }
  };

  let mut speak_handlers = Vec::new();
  for (engine, getter) in get_speaker_getters() {
    let handler = handle.spawn(async move {
      let mut consecutive_failures = 0;
      const MAX_CONSECUTIVE_FAILURES: u32 = 10; // 最大連続失敗回数
      const BACKOFF_BASE: u64 = 2; // バックオフ基数（秒）

      loop {
        // 停止チェック
        if *SPEAK_QUEUE_STOPPER.lock().await {
          debug!("Speak queue stopping for engine: {}", engine.name());
          break;
        }
        if let Some(port_opener_path) = get_port_opener_path(format!("{}", engine.port())).await {
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
        // 1秒のスリープを100ms x 10に分割して応答性を向上
        for _ in 0..10 {
          if *SPEAK_QUEUE_STOPPER.lock().await {
            break;
          }
          tokio::time::sleep(Duration::from_millis(100)).await;
        }
      }
    });
    speak_handlers.push(handler);
  }
  handle.block_on(async {
    *SPEAK_HANDLERS.lock().await = speak_handlers;
  });
}

fn init_predict_queue() {
  let predict_queue_cln = PREDICT_QUEUE.clone();
  let predict_stopper_cln = PREDICT_STOPPER.clone();

  let handle = {
    let guard = match RUNTIME.lock() {
      Ok(g) => g,
      Err(e) => {
        error!("Failed to lock runtime for predict queue: {}", e);
        return;
      }
    };
    match guard.as_ref() {
      Some(rt) => rt.handle().clone(),
      None => {
        error!("Runtime is not initialized for predict queue");
        return;
      }
    }
  };

  let handler = handle.spawn(async move {
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
          match args_to_predictors(parg).await {
            None => continue,
            Some(predictors) => {
              for predictor in predictors {
                // predict結果をOk/Errで分けてからawaitを行う
                // Box<dyn Error>はSendではないので、awaitをまたがないようにする
                let wav_result: Result<Vec<u8>, String> =
                  predictor.predict().await.map_err(|e| e.to_string());
                match wav_result {
                  Ok(res) => {
                    debug!("pushing to play");
                    PLAY_QUEUE.lock().await.push_back(res);
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
  });
  handle.block_on(async {
    *PREDICT_HANDLER.lock().await = Some(handler);
  });
}

pub(crate) fn init_play_queue() {
  let play_queue_cln = PLAY_QUEUE.clone();
  let play_stopper_cln = PLAY_STOPPER.clone();

  let handle = {
    let guard = match RUNTIME.lock() {
      Ok(g) => g,
      Err(e) => {
        error!("Failed to lock runtime for play queue: {}", e);
        return;
      }
    };
    match guard.as_ref() {
      Some(rt) => rt.handle().clone(),
      None => {
        error!("Runtime is not initialized for play queue");
        return;
      }
    }
  };

  let handler = handle.spawn(async move {
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
          match tokio::task::spawn_blocking(move || {
            play_wav(data).map_err(|e| e.to_string())
          }).await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => error!("play_wav failed: {}", e),
            Err(e) => error!("play_wav spawn_blocking failed: {}", e),
          }
        }
      }
    }
  });
  handle.block_on(async {
    *PLAY_HANDLER.lock().await = Some(handler);
  });
}

pub(crate) fn init_queues() {
  init_speak_queue();
  init_predict_queue();
  init_play_queue();
}

pub(crate) fn stop_async_tasks() -> Result<
  (),
  std::sync::PoisonError<
    std::sync::MutexGuard<'static, std::option::Option<tokio::runtime::Runtime>>,
  >,
> {
  debug!("{}", "stopping async tasks");

  // 同期再生ステートをクリア
  match SYNC_STATE.lock() {
    Ok(mut s) => *s = None,
    Err(e) => error!("Failed to lock SYNC_STATE during shutdown: {}", e),
  }
  
  // 音声再生を即座に強制停止
  if let Ok(mut force_stop) = crate::player::FORCE_STOP_SINK.lock() {
    *force_stop = true;
    debug!("{}", "set force stop sink flag");
  } else {
    error!("Failed to set FORCE_STOP_SINK flag");
  }
  
  // ランタイムハンドルを取得（take()前に）
  let handle = {
    let guard = RUNTIME.lock()?;
    guard.as_ref().map(|rt| rt.handle().clone())
  };

  if let Some(handle) = &handle {
    // 全停止フラグを設定（協調的停止の開始）
    handle.block_on(async {
      *PLAY_STOPPER.lock().await = true;
      *PREDICT_STOPPER.lock().await = true;
      *SPEAK_QUEUE_STOPPER.lock().await = true;
      debug!("{}", "set all stop flags");
    });
    
    // 協調的停止を待機（abort()は使用しない）
    let start_time = Instant::now();
    while start_time.elapsed() < GRACEFUL_SHUTDOWN_TIMEOUT {
      // 全タスクの完了状況を確認
      let (play_stopped, predict_stopped, speak_stopped) = handle.block_on(async {
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
      let (play_stopped, predict_stopped, speak_stopped) = handle.block_on(async {
        let play_stopped = PLAY_HANDLER.lock().await.as_ref().map_or(true, |h| h.is_finished());
        let predict_stopped = PREDICT_HANDLER.lock().await.as_ref().map_or(true, |h| h.is_finished());
        let speak_stopped = SPEAK_HANDLERS.lock().await.iter().all(|h| h.is_finished());
        (play_stopped, predict_stopped, speak_stopped)
      });
      warn!("Task status - play: {}, predict: {}, speak: {}", play_stopped, predict_stopped, speak_stopped);
    }
  } else {
    warn!("Runtime was not initialized, skipping graceful shutdown");
  }
  
  // 同期スレッドの JoinHandle を回収して join する
  match SYNC_THREAD_HANDLES.lock() {
    Ok(mut handles) => {
      let handles_to_join: Vec<_> = handles.drain(..).collect();
      drop(handles); // ロック解放してから join
      let join_start = Instant::now();
      for h in handles_to_join {
        let remaining = GRACEFUL_SHUTDOWN_TIMEOUT.saturating_sub(join_start.elapsed());
        if remaining.is_zero() {
          warn!("Timeout waiting for sync threads to finish");
          break;
        }
        let poll_start = Instant::now();
        while !h.is_finished() && poll_start.elapsed() < remaining {
          std::thread::sleep(Duration::from_millis(50));
        }
        if h.is_finished() {
          let _ = h.join();
        } else {
          warn!("Sync thread did not finish within timeout");
        }
      }
    }
    Err(e) => error!("Failed to lock SYNC_THREAD_HANDLES during shutdown: {}", e),
  }

  debug!("{}", "stopped async tasks");
  Ok(())
}

pub(crate) fn shutdown_runtime() -> Result<
  (),
  std::sync::PoisonError<
    std::sync::MutexGuard<'static, std::option::Option<tokio::runtime::Runtime>>,
  >,
> {
  debug!("{}", "shutting down runtime");
  if let Some(runtime) = RUNTIME.lock()?.take() {
    runtime.shutdown_timeout(RUNTIME_SHUTDOWN_TIMEOUT);
  }
  Ok(())
}

pub(crate) fn push_to_prediction(text: String, ghost_name: String) {
  let handle = {
    let guard = match RUNTIME.lock() {
      Ok(g) => g,
      Err(e) => {
        error!("Failed to lock RUNTIME in push_to_prediction: {}", e);
        return;
      }
    };
    match guard.as_ref() {
      Some(rt) => rt.handle().clone(),
      None => {
        error!("Runtime is not initialized in push_to_prediction");
        return;
      }
    }
  };
  handle.block_on(async {
    // 処理が重いので、別スレッドに投げてそっちでPredictorを作る
    PREDICT_QUEUE.lock().await.push_back((text, ghost_name));
  });
}

async fn args_to_predictors(
  args: (String, String),
) -> Option<VecDeque<Box<dyn Predictor + Send + Sync>>> {
  let (text, ghost_name) = args;
  build_segments_async(text, ghost_name, false).await.map(|segments| {
    segments
      .into_iter()
      .map(|seg| seg.predictor)
      .collect()
  })
}

pub(crate) struct SyncSegment {
  pub text: String,
  pub raw_text: String,
  pub scope: usize,
  pub predictor: Box<dyn Predictor + Send + Sync>,
}

async fn build_segments_async(
  text: String,
  ghost_name: String,
  sync_mode: bool,
) -> Option<Vec<SyncSegment>> {
  let mut segments: Vec<SyncSegment> = Vec::new();
  let connected_engines = {
    CURRENT_CONNECTION_STATUS
      .read()
      .await
      .clone()
      .iter()
      .filter(|(_, v)| **v)
      .map(|(k, _)| *k)
      .collect::<Vec<_>>()
  };
  if connected_engines.is_empty() {
    debug!("no engine connected: skip: {}", text);
    return None;
  }

  debug!("{}", format!("predicting: {}", text));

  // GHOSTS_VOICES から必要なデータをクローンしてからガードをドロップ
  let (devide_by_lines, speak_by_punctuation_val, speakers, initial_voice) = {
    let ghosts_voices = match GHOSTS_VOICES.read() {
      Ok(gv) => gv,
      Err(e) => {
        error!("Failed to read GHOSTS_VOICES: {}", e);
        return None;
      }
    };
    let ghost_info = match ghosts_voices.get(&ghost_name) {
      Some(info) => info,
      None => {
        error!("Ghost not found in GHOSTS_VOICES: {}", ghost_name);
        return None;
      }
    };
    let devide_by_lines = ghost_info.devide_by_lines;
    let speakers = ghost_info.voices.clone();
    let speak_by_punctuation_val = match SPEAK_BY_PUNCTUATION.read() {
      Ok(sbp) => *sbp,
      Err(e) => {
        error!("Failed to read SPEAK_BY_PUNCTUATION: {}", e);
        true
      }
    };
    let initial_voice = match INITIAL_VOICE.read() {
      Ok(iv) => iv.clone(),
      Err(e) => {
        error!("Failed to read INITIAL_VOICE: {}", e);
        return None;
      }
    };
    (devide_by_lines, speak_by_punctuation_val, speakers, initial_voice)
  };
  // ここではすべてのstd::sync::RwLockガードがドロップ済み

  for dialog in split_dialog(text, devide_by_lines) {
    if dialog.text.is_empty() {
      continue;
    }

    debug!("selecting speaker: {}", dialog.scope);
    let speaker = match speakers.get(dialog.scope) {
      Some(speaker) => {
        if let Some(sp) = speaker {
          sp.clone()
        } else {
          initial_voice.clone()
        }
      }
      None => initial_voice.clone(),
    };

    if speaker.speaker_uuid == NO_VOICE_UUID {
      continue;
    }
    let voice_not_found = {
      let engine = match engine_from_port(speaker.port) {
        Some(e) => e,
        None => continue,
      };
      if let Some(speakers_by_engine) = &SPEAKERS_INFO
        .read()
        .await
        .get(&engine)
      {
        !speakers_by_engine
          .iter()
          .any(|s| s.speaker_uuid == speaker.speaker_uuid)
      } else {
        false
      }
    };
    if voice_not_found {
      continue;
    }
    let engine = match engine_from_port(speaker.port) {
      Some(e) => e,
      None => continue,
    };
    let pairs = if (speak_by_punctuation_val || sync_mode) && engine != Engine::BouyomiChan {
      split_by_punctuation_with_raw(dialog.text.clone(), dialog.raw_text.clone())
    } else {
      /* 棒読みちゃんは細切れの恩恵が少ない&
      読み上げ順がばらばらになることがあるので常にまとめて読み上げる */
      vec![(dialog.text.clone(), dialog.raw_text.clone())]
    };
    for (t, rt) in pairs {
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
        raw_text: rt,
        scope: dialog.scope,
        predictor,
      });
    }
  }
  Some(segments)
}

/// sync ラッパー（on_other_ghost_talk 用）
pub(crate) fn build_segments(
  text: String,
  ghost_name: String,
  sync_mode: bool,
) -> Option<Vec<SyncSegment>> {
  let handle = {
    let guard = match RUNTIME.lock() {
      Ok(g) => g,
      Err(e) => {
        error!("Failed to lock RUNTIME: {}", e);
        return None;
      }
    };
    guard.as_ref()?.handle().clone()
  };
  handle.block_on(build_segments_async(text, ghost_name, sync_mode))
}

#[allow(dead_code)]
pub(crate) struct SyncReadySegment {
  pub text: String,
  pub raw_text: String,
  pub scope: usize,
  pub wav: Vec<u8>,
}

pub(crate) struct SyncPlaybackState {
  pub ready_queue: VecDeque<SyncReadySegment>,
  pub ghost_name: String,
  pub all_predicted: bool,
}

pub(crate) static SYNC_STATE: Lazy<StdMutex<Option<SyncPlaybackState>>> =
  Lazy::new(|| StdMutex::new(None));

static SYNC_AUDIO_DONE: Lazy<StdMutex<Arc<AtomicBool>>> =
  Lazy::new(|| StdMutex::new(Arc::new(AtomicBool::new(true))));

static SYNC_THREAD_HANDLES: Lazy<StdMutex<Vec<std::thread::JoinHandle<()>>>> =
  Lazy::new(|| StdMutex::new(Vec::new()));

/// 同期モード用: Predictor.predict() を同期的に呼び出す
pub(crate) fn sync_predict(
  predictor: &dyn Predictor,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
  let handle = {
    let guard = RUNTIME
      .lock()
      .map_err(|e| format!("RUNTIME lock poisoned: {}", e))?;
    guard
      .as_ref()
      .ok_or("Runtime not initialized")?
      .handle()
      .clone()
  };
  handle.block_on(async {
    tokio::time::timeout(Duration::from_secs(30), predictor.predict())
      .await
      .map_err(|_| -> Box<dyn std::error::Error> { "predict timed out".into() })?
  })
}

/// 同期モード用: WAV を別スレッドで再生し、完了時にフラグ設定
pub(crate) fn spawn_sync_playback(wav: Vec<u8>) {
  let done = Arc::new(AtomicBool::new(false));
  match SYNC_AUDIO_DONE.lock() {
    Ok(mut guard) => *guard = done.clone(),
    Err(e) => {
      error!("Failed to lock SYNC_AUDIO_DONE: {}", e);
      return;
    }
  }
  // cancel_sync_playback で設定された FORCE_STOP_SINK をリセット
  if let Ok(mut force_stop) = crate::player::FORCE_STOP_SINK.lock() {
    *force_stop = false;
  }
  let handle = std::thread::spawn(move || {
    if !wav.is_empty() {
      let _ = play_wav(wav);
    }
    done.store(true, Ordering::SeqCst);
  });
  match SYNC_THREAD_HANDLES.lock() {
    Ok(mut handles) => {
      handles.retain(|h| !h.is_finished());
      handles.push(handle);
    }
    Err(e) => {
      error!("Failed to lock SYNC_THREAD_HANDLES: {}", e);
      let _ = handle.join();
    }
  }
}

/// 同期再生の音声が完了したか確認（非ブロッキング）
pub(crate) fn is_sync_audio_done() -> bool {
  match SYNC_AUDIO_DONE.lock() {
    Ok(guard) => guard.load(Ordering::SeqCst),
    Err(e) => {
      error!("Failed to lock SYNC_AUDIO_DONE: {}", e);
      true // poison時は完了として扱う
    }
  }
}

/// 同期再生をキャンセル
pub(crate) fn cancel_sync_playback() {
  match SYNC_STATE.lock() {
    Ok(mut s) => *s = None,
    Err(e) => error!("Failed to lock SYNC_STATE in cancel: {}", e),
  }
  if let Ok(mut force_stop) = crate::player::FORCE_STOP_SINK.lock() {
    *force_stop = true;
  }
}

/// 同期モード用: 全セグメントをバックグラウンドで順次合成し、プールに蓄積する
pub(crate) fn spawn_sync_prediction(segments: Vec<SyncSegment>, ghost_name: String) {
  // SYNC_STATE を初期化（空の ready_queue）
  match SYNC_STATE.lock() {
    Ok(mut s) => {
      *s = Some(SyncPlaybackState {
        ready_queue: VecDeque::new(),
        ghost_name,
        all_predicted: false,
      });
    }
    Err(e) => {
      error!("Failed to lock SYNC_STATE for initialization: {}", e);
      return;
    }
  }

  let handle = std::thread::spawn(move || {
    for segment in segments {
      // キャンセルチェック: SYNC_STATE が None ならば中断
      {
        match SYNC_STATE.lock() {
          Ok(state) => {
            if state.is_none() {
              return;
            }
          }
          Err(e) => {
            error!("Failed to lock SYNC_STATE for cancel check: {}", e);
            return;
          }
        }
      }

      let wav = sync_predict(&*segment.predictor).unwrap_or_default();

      // 合成結果をプールに追加
      {
        match SYNC_STATE.lock() {
          Ok(mut state) => {
            if let Some(s) = state.as_mut() {
              s.ready_queue.push_back(SyncReadySegment {
                text: segment.text,
                raw_text: segment.raw_text,
                scope: segment.scope,
                wav,
              });
            } else {
              return; // キャンセルされた
            }
          }
          Err(e) => {
            error!("Failed to lock SYNC_STATE for push: {}", e);
            return;
          }
        }
      }
    }

    // 全合成完了フラグをセット
    match SYNC_STATE.lock() {
      Ok(mut state) => {
        if let Some(s) = state.as_mut() {
          s.all_predicted = true;
        }
      }
      Err(e) => error!("Failed to lock SYNC_STATE for completion flag: {}", e),
    }
  });
  match SYNC_THREAD_HANDLES.lock() {
    Ok(mut handles) => {
      handles.retain(|h| !h.is_finished());
      handles.push(handle);
    }
    Err(e) => {
      error!("Failed to lock SYNC_THREAD_HANDLES: {}", e);
      let _ = handle.join();
    }
  }
}

/// 同期モード用: プールから合成済みセグメントを取得し、残りがあるかも返す
pub(crate) fn pop_ready_segment(
  ghost_name: &str,
) -> (Option<SyncReadySegment>, bool) {
  match SYNC_STATE.lock() {
    Ok(mut state) => match state.as_mut() {
      Some(s) if s.ghost_name == ghost_name => {
        let segment = s.ready_queue.pop_front();
        let has_more = !s.ready_queue.is_empty() || !s.all_predicted;
        (segment, has_more)
      }
      _ => (None, false),
    },
    Err(e) => {
      error!("Failed to lock SYNC_STATE in pop_ready_segment: {}", e);
      (None, false)
    }
  }
}
