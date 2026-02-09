use crate::engine::bouyomichan::predict::BouyomichanPredictor;
use crate::engine::coeiroink_v2::predict::CoeiroinkV2Predictor;
use crate::engine::voicevox_family::predict::VoicevoxFamilyPredictor;
use crate::engine::{engine_from_port, get_speaker_getters, Engine, NoOpPredictor, Predictor, NO_VOICE_UUID};
use crate::format::{is_ellipsis_segment, split_by_punctuation_with_raw, split_dialog};
use crate::player::play_wav;
use crate::system::get_port_opener_path;
use crate::variables::GHOSTS_VOICES;
use crate::variables::*;
use once_cell::sync::Lazy;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

// タイムアウト定数
const GRACEFUL_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(8);
const RUNTIME_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(3);
const QUEUE_POLL_TIMEOUT: Duration = Duration::from_millis(100);

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex as StdMutex;

pub(crate) static CONNECTION_DIALOGS: Lazy<StdMutex<Vec<String>>> =
  Lazy::new(|| StdMutex::new(Vec::new()));

pub(crate) static RUNTIME: Lazy<StdMutex<Option<tokio::runtime::Runtime>>> =
  Lazy::new(|| StdMutex::new(None));

/// 必要に応じてランタイムを作成する（DLLリロード後の再生成に対応）
fn ensure_runtime() {
  let mut guard = match RUNTIME.lock() {
    Ok(g) => g,
    Err(e) => {
      error!("Failed to lock RUNTIME in ensure_runtime: {}", e);
      return;
    }
  };
  if guard.is_none() {
    match tokio::runtime::Builder::new_multi_thread()
      .worker_threads(1)
      .max_blocking_threads(4)
      .enable_all()
      .build()
    {
      Ok(runtime) => *guard = Some(runtime),
      Err(e) => error!("Failed to create tokio runtime: {}", e),
    }
  }
}

/// RUNTIMEからtokioハンドルを安全に取得する
pub(crate) fn get_runtime_handle() -> Option<tokio::runtime::Handle> {
  match RUNTIME.lock() {
    Ok(guard) => guard.as_ref().map(|rt| rt.handle().clone()),
    Err(e) => {
      error!("Failed to lock runtime: {}", e);
      None
    }
  }
}

pub(crate) static SPEAK_HANDLERS: Lazy<StdMutex<Option<tokio::task::JoinHandle<()>>>> =
  Lazy::new(|| StdMutex::new(None));
pub(crate) static PREDICT_HANDLER: Lazy<StdMutex<Option<tokio::task::JoinHandle<()>>>> =
  Lazy::new(|| StdMutex::new(None));
pub(crate) static PREDICT_QUEUE: Lazy<StdMutex<VecDeque<(String, String)>>> =
  Lazy::new(|| StdMutex::new(VecDeque::new()));
pub(crate) static PREDICT_STOPPER: AtomicBool = AtomicBool::new(false);
pub(crate) static PLAY_HANDLER: Lazy<StdMutex<Option<tokio::task::JoinHandle<()>>>> =
  Lazy::new(|| StdMutex::new(None));
pub(crate) static PLAY_QUEUE: Lazy<StdMutex<VecDeque<Vec<u8>>>> =
  Lazy::new(|| StdMutex::new(VecDeque::new()));
pub(crate) static PLAY_STOPPER: AtomicBool = AtomicBool::new(false);
pub(crate) static SPEAK_QUEUE_STOPPER: AtomicBool = AtomicBool::new(false);

pub(crate) static SHUTTING_DOWN: AtomicBool = AtomicBool::new(false);

fn init_speak_queue() {
  let Some(handle) = get_runtime_handle() else {
    error!("Runtime is not available for speak queue");
    return;
  };

  let speaker_getters = get_speaker_getters();
  let handler = handle.spawn(async move {
    // エンジンごとの連続失敗カウンタ
    let mut consecutive_failures: HashMap<Engine, u32> = HashMap::new();
    const MAX_CONSECUTIVE_FAILURES: u32 = 10;
    const BACKOFF_BASE: u64 = 2;

    loop {
      // 停止チェック
      if SPEAK_QUEUE_STOPPER.load(Ordering::Acquire) {
        debug!("Speak queue stopping");
        break;
      }

      // 全エンジンを順番にチェック
      for (engine, getter) in &speaker_getters {
        let engine = *engine;
        if SPEAK_QUEUE_STOPPER.load(Ordering::Acquire) {
          break;
        }
        if let Some(port_opener_path) = get_port_opener_path(format!("{}", engine.port())).await {
          match getter.get_speakers_info().await {
            Ok(speakers_info) => {
              consecutive_failures.insert(engine, 0);
              let was_disconnected = {
                let cs = CURRENT_CONNECTION_STATUS.read().unwrap_or_else(|e| e.into_inner());
                cs.get(&engine).is_none() || cs.get(&engine).is_some_and(|v| !*v)
              };
              if was_disconnected {
                if let Ok(mut dialogs) = CONNECTION_DIALOGS.lock() {
                  dialogs.push(format!("{} が接続されました", engine.name()));
                } else {
                  error!("Failed to lock CONNECTION_DIALOGS for connection message");
                }
                if let Ok(mut engine_path) = ENGINE_PATH.write() {
                  engine_path.insert(engine, port_opener_path);
                } else {
                  error!("Failed to lock ENGINE_PATH for engine: {}", engine.name());
                }
                if let Ok(mut auto_start) = ENGINE_AUTO_START.write() {
                  if auto_start.get(&engine).is_none() {
                    auto_start.insert(engine, false);
                  }
                } else {
                  error!("Failed to lock ENGINE_AUTO_START for engine: {}", engine.name());
                }
              }
              if let Ok(mut cs) = CURRENT_CONNECTION_STATUS.write() {
                cs.insert(engine, true);
              } else {
                error!("Failed to lock CURRENT_CONNECTION_STATUS for engine: {}", engine.name());
              }
              if let Ok(mut si) = SPEAKERS_INFO.write() {
                si.insert(engine, speakers_info);
              } else {
                error!("Failed to lock SPEAKERS_INFO for engine: {}", engine.name());
              }
            }
            Err(e) => {
              let failures = consecutive_failures.entry(engine).or_insert(0);
              *failures += 1;
              error!(
                "Error: {} (consecutive failures: {})",
                e, *failures
              );

              if *failures >= MAX_CONSECUTIVE_FAILURES {
                error!(
                  "Too many consecutive failures for engine: {}, backing off",
                  engine.name()
                );
                let backoff_time = std::cmp::min(BACKOFF_BASE.pow(*failures / 5), 60);
                tokio::time::sleep(Duration::from_secs(backoff_time)).await;
              }

              {
                let was_connected = {
                  let cs = CURRENT_CONNECTION_STATUS.read().unwrap_or_else(|e| e.into_inner());
                  cs.get(&engine).is_some_and(|v| *v)
                };
                if was_connected {
                  if let Ok(mut dialogs) = CONNECTION_DIALOGS.lock() {
                    dialogs.push(format!("{} が切断されました", engine.name()));
                  } else {
                    error!("Failed to lock CONNECTION_DIALOGS for disconnection message");
                  }
                }
              }
              if let Ok(mut cs) = CURRENT_CONNECTION_STATUS.write() {
                cs.insert(engine, false);
              } else {
                error!("Failed to lock CURRENT_CONNECTION_STATUS for disconnect");
              }
              if let Ok(mut si) = SPEAKERS_INFO.write() {
                si.remove(&engine);
              } else {
                error!("Failed to lock SPEAKERS_INFO for disconnect");
              }
            }
          }
        }
      }
      // 1秒のスリープを100ms x 10に分割して応答性を向上
      for _ in 0..10 {
        if SPEAK_QUEUE_STOPPER.load(Ordering::Acquire) {
          break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
      }
    }
  });
  match SPEAK_HANDLERS.lock() {
    Ok(mut guard) => *guard = Some(handler),
    Err(e) => error!("Failed to lock SPEAK_HANDLERS: {}", e),
  }
}

fn init_predict_queue() {
  let Some(handle) = get_runtime_handle() else {
    error!("Runtime is not available for predict queue");
    return;
  };

  let handler = handle.spawn(async move {
    let mut last_activity = Instant::now();
    const MAX_IDLE_TIME: Duration = Duration::from_secs(300); // 5分間のアイドル時間

    loop {
      {
        let is_empty = PREDICT_QUEUE
          .lock()
          .unwrap_or_else(|e| e.into_inner())
          .is_empty();
        if is_empty {
          if PREDICT_STOPPER.load(Ordering::Acquire) {
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

      let parg = PREDICT_QUEUE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .pop_front();

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
                    PLAY_QUEUE
                      .lock()
                      .unwrap_or_else(|e| e.into_inner())
                      .push_back(res);
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
  match PREDICT_HANDLER.lock() {
    Ok(mut guard) => *guard = Some(handler),
    Err(e) => error!("Failed to lock PREDICT_HANDLER: {}", e),
  }
}

pub(crate) fn init_play_queue() {
  let Some(handle) = get_runtime_handle() else {
    error!("Runtime is not available for play queue");
    return;
  };

  let handler = handle.spawn(async move {
    let mut last_activity = Instant::now();
    const MAX_IDLE_TIME: Duration = Duration::from_secs(300); // 5分間のアイドル時間

    loop {
      {
        let is_empty = PLAY_QUEUE
          .lock()
          .unwrap_or_else(|e| e.into_inner())
          .is_empty();
        if is_empty {
          if PLAY_STOPPER.load(Ordering::Acquire) {
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

      let wav = PLAY_QUEUE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .pop_front();
      if let Some(data) = wav {
        if !data.is_empty() {
          last_activity = Instant::now(); // アクティビティ更新
          debug!("{}", format!("play: {}", data.len()));
          match tokio::task::spawn_blocking(move || {
            play_wav(data).map_err(|e| e.to_string())
          })
          .await
          {
            Ok(Ok(())) => {}
            Ok(Err(e)) => error!("play_wav failed: {}", e),
            Err(e) => error!("play_wav spawn_blocking failed: {}", e),
          }
        }
      }
    }
  });
  match PLAY_HANDLER.lock() {
    Ok(mut guard) => *guard = Some(handler),
    Err(e) => error!("Failed to lock PLAY_HANDLER: {}", e),
  }
}

pub(crate) fn init_queues() {
  SHUTTING_DOWN.store(false, Ordering::Release);
  PREDICT_STOPPER.store(false, Ordering::Release);
  PLAY_STOPPER.store(false, Ordering::Release);
  SPEAK_QUEUE_STOPPER.store(false, Ordering::Release);
  crate::player::FORCE_STOP_SINK.store(false, Ordering::Release);

  // 同期世代カウンタをリセット（DLLがメモリに残ったままunload→loadされた場合の不整合防止）
  SYNC_AUDIO_GENERATION.store(0, Ordering::Release);
  SYNC_AUDIO_DONE_GEN.store(0, Ordering::Release);
  // 同期予測ハンドラをクリア
  if let Ok(mut h) = SYNC_PREDICTION_HANDLER.lock() {
    *h = None;
  }

  // ランタイムを必要に応じて作成（DLLリロード後の再生成対応）
  ensure_runtime();

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
  SHUTTING_DOWN.store(true, Ordering::Release);

  // 同期再生ステートをクリア
  match SYNC_STATE.lock() {
    Ok(mut s) => *s = None,
    Err(e) => error!("Failed to lock SYNC_STATE during shutdown: {}", e),
  }
  
  // 音声再生を即座に強制停止
  crate::player::FORCE_STOP_SINK.store(true, Ordering::Release);
  debug!("{}", "set force stop sink flag");
  
  // ランタイムの存在を確認
  let runtime_available = get_runtime_handle().is_some();

  if runtime_available {
    // 全停止フラグを設定（協調的停止の開始）
    PLAY_STOPPER.store(true, Ordering::Release);
    PREDICT_STOPPER.store(true, Ordering::Release);
    SPEAK_QUEUE_STOPPER.store(true, Ordering::Release);
    debug!("{}", "set all stop flags");
    
    // 同期予測ハンドラもabort
    if let Ok(mut h) = SYNC_PREDICTION_HANDLER.lock() {
      if let Some(handle) = h.take() {
        if !handle.is_finished() {
          handle.abort();
        }
      }
    }

    // 協調的停止を待機
    let start_time = Instant::now();
    while start_time.elapsed() < GRACEFUL_SHUTDOWN_TIMEOUT {
      // 全タスクの完了状況を確認
      let play_stopped = PLAY_HANDLER
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .is_none_or(|h| h.is_finished());
      let predict_stopped = PREDICT_HANDLER
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .is_none_or(|h| h.is_finished());
      let speak_stopped = SPEAK_HANDLERS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .is_none_or(|h| h.is_finished());

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
        debug!("{}", "waiting for speak handler to finish");
      }

      std::thread::sleep(Duration::from_millis(200));
    }

    // タイムアウト時: abort + take で確実にタスクを消滅させる
    if start_time.elapsed() >= GRACEFUL_SHUTDOWN_TIMEOUT {
      warn!("Some tasks did not stop within timeout, aborting remaining tasks");
      if let Ok(mut h) = PLAY_HANDLER.lock() {
        if let Some(handle) = h.take() {
          if !handle.is_finished() {
            handle.abort();
            warn!("Aborted play handler");
          }
        }
      }
      if let Ok(mut h) = PREDICT_HANDLER.lock() {
        if let Some(handle) = h.take() {
          if !handle.is_finished() {
            handle.abort();
            warn!("Aborted predict handler");
          }
        }
      }
      if let Ok(mut h) = SPEAK_HANDLERS.lock() {
        if let Some(handle) = h.take() {
          if !handle.is_finished() {
            handle.abort();
            warn!("Aborted speak handler");
          }
        }
      }
    }
  } else {
    warn!("Runtime was not initialized, skipping graceful shutdown");
  }

  crate::system::cleanup_system_cache();

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
  // StdMutexなのでランタイムハンドル不要
  PREDICT_QUEUE
    .lock()
    .unwrap_or_else(|e| e.into_inner())
    .push_back((text, ghost_name));
}

async fn args_to_predictors(
  args: (String, String),
) -> Option<VecDeque<Box<dyn Predictor + Send + Sync>>> {
  let (text, ghost_name) = args;
  build_segments_async(text, ghost_name, false).await.map(|segments| {
    segments
      .into_iter()
      .filter(|seg| !is_ellipsis_segment(&seg.text))
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
    match CURRENT_CONNECTION_STATUS.read() {
      Ok(cs) => cs
        .iter()
        .filter(|(_, v)| **v)
        .map(|(k, _)| *k)
        .collect::<Vec<_>>(),
      Err(e) => {
        error!("Failed to read CURRENT_CONNECTION_STATUS: {}", e);
        return None;
      }
    }
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
      match SPEAKERS_INFO.read() {
        Ok(si) => {
          if let Some(speakers_by_engine) = si.get(&engine) {
            !speakers_by_engine
              .iter()
              .any(|s| s.speaker_uuid == speaker.speaker_uuid)
          } else {
            false
          }
        }
        Err(e) => {
          error!("Failed to read SPEAKERS_INFO: {}", e);
          false
        }
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
      if is_ellipsis_segment(&t) {
        segments.push(SyncSegment {
          text: t,
          raw_text: rt,
          scope: dialog.scope,
          predictor: Box::new(NoOpPredictor),
        });
        continue;
      }
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
  let handle = get_runtime_handle()?;
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

// 世代カウンタ方式: Mutexを排除しロックフリーに
// SYNC_AUDIO_GENERATION: 再生開始ごとにインクリメント
// SYNC_AUDIO_DONE_GEN: スレッド完了時にfetch_maxで更新
static SYNC_AUDIO_GENERATION: std::sync::atomic::AtomicU64 =
  std::sync::atomic::AtomicU64::new(0);
static SYNC_AUDIO_DONE_GEN: std::sync::atomic::AtomicU64 =
  std::sync::atomic::AtomicU64::new(0);

pub(crate) static SYNC_PREDICTION_HANDLER: Lazy<StdMutex<Option<tokio::task::JoinHandle<()>>>> =
  Lazy::new(|| StdMutex::new(None));


pub(crate) fn spawn_sync_playback(wav: Vec<u8>) {
  let gen = SYNC_AUDIO_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
  // cancel_sync_playback で設定された FORCE_STOP_SINK をリセット
  crate::player::FORCE_STOP_SINK.store(false, Ordering::Release);
  let Some(handle) = get_runtime_handle() else {
    error!("Runtime is not available for sync playback");
    SYNC_AUDIO_DONE_GEN.fetch_max(gen, Ordering::SeqCst);
    return;
  };
  handle.spawn(async move {
    if !wav.is_empty() {
      match tokio::task::spawn_blocking(move || {
        play_wav(wav).map_err(|e| e.to_string())
      })
      .await
      {
        Ok(Ok(())) => {}
        Ok(Err(e)) => error!("sync play_wav failed: {}", e),
        Err(e) => error!("sync play_wav spawn_blocking failed: {}", e),
      }
    }
    SYNC_AUDIO_DONE_GEN.fetch_max(gen, Ordering::SeqCst);
  });
}

pub(crate) fn is_sync_audio_done() -> bool {
  SYNC_AUDIO_DONE_GEN.load(Ordering::SeqCst) >= SYNC_AUDIO_GENERATION.load(Ordering::SeqCst)
}

/// 同期再生をキャンセル
pub(crate) fn cancel_sync_playback() {
  match SYNC_STATE.lock() {
    Ok(mut s) => *s = None,
    Err(e) => error!("Failed to lock SYNC_STATE in cancel: {}", e),
  }
  // 進行中の予測タスクを即座に停止
  if let Ok(mut h) = SYNC_PREDICTION_HANDLER.lock() {
    if let Some(handle) = h.take() {
      if !handle.is_finished() {
        handle.abort();
      }
    }
  }
  crate::player::FORCE_STOP_SINK.store(true, Ordering::Release);
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

  let Some(handle) = get_runtime_handle() else {
    error!("Runtime is not available for sync prediction");
    return;
  };

  let task_handle = handle.spawn(async move {
    for segment in segments {
      // シャットダウンチェック
      if SHUTTING_DOWN.load(Ordering::Acquire) {
        return;
      }
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

      let wav = if is_ellipsis_segment(&segment.text) {
        Vec::new()
      } else {
        // predictは元々async関数なので直接awaitで呼び出す
        let wav_result: Result<Vec<u8>, String> =
          tokio::time::timeout(Duration::from_secs(30), segment.predictor.predict())
            .await
            .map_err(|_| "predict timed out".to_string())
            .and_then(|r| r.map_err(|e| e.to_string()));
        match wav_result {
          Ok(data) => data,
          Err(e) => {
            debug!("sync predict failed: {}", e);
            Vec::new()
          }
        }
      };

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

  match SYNC_PREDICTION_HANDLER.lock() {
    Ok(mut h) => *h = Some(task_handle),
    Err(e) => error!("Failed to lock SYNC_PREDICTION_HANDLER: {}", e),
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
