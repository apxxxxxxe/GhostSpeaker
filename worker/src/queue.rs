use crate::engine::bouyomichan::predict::BouyomichanPredictor;
use crate::engine::coeiroink_v2::predict::CoeiroinkV2Predictor;
use crate::engine::voicevox_family::predict::VoicevoxFamilyPredictor;
use crate::engine::{get_speaker_getters, NoOpPredictor, Predictor};
use crate::format::{
  is_ellipsis_segment, resplit_pairs_by_raw_ellipsis, split_by_punctuation_with_raw, split_dialog,
};
use crate::player::play_wav;
use crate::system::get_port_opener_path;
use ghost_speaker_common::{
  engine_from_port, CharacterVoice, Engine, GhostVoiceInfo, SpeakerInfo, NO_VOICE_UUID,
};
use log::{debug, error, warn};
use once_cell::sync::Lazy;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex as StdMutex;
use std::sync::RwLock;
use std::time::{Duration, Instant};

// タイムアウト定数
const QUEUE_POLL_TIMEOUT: Duration = Duration::from_millis(100);

// --- グローバル状態 ---

pub static CONNECTION_DIALOGS: Lazy<StdMutex<Vec<String>>> =
  Lazy::new(|| StdMutex::new(Vec::new()));

pub static SPEAKERS_INFO: Lazy<RwLock<HashMap<Engine, Vec<SpeakerInfo>>>> =
  Lazy::new(|| RwLock::new(HashMap::new()));

pub static CURRENT_CONNECTION_STATUS: Lazy<RwLock<HashMap<Engine, bool>>> =
  Lazy::new(|| RwLock::new(HashMap::new()));

pub static ENGINE_PATH: Lazy<RwLock<HashMap<Engine, String>>> =
  Lazy::new(|| RwLock::new(HashMap::new()));

pub static ENGINE_AUTO_START: Lazy<RwLock<HashMap<Engine, bool>>> =
  Lazy::new(|| RwLock::new(HashMap::new()));

pub static VOLUME: Lazy<RwLock<f32>> = Lazy::new(|| RwLock::new(1.0));

pub static SPEAK_BY_PUNCTUATION: Lazy<RwLock<bool>> = Lazy::new(|| RwLock::new(true));

pub static GHOSTS_VOICES: Lazy<RwLock<HashMap<String, GhostVoiceInfo>>> =
  Lazy::new(|| RwLock::new(HashMap::new()));

pub static INITIAL_VOICE: Lazy<RwLock<CharacterVoice>> =
  Lazy::new(|| RwLock::new(CharacterVoice::no_voice()));

pub static SHUTTING_DOWN: AtomicBool = AtomicBool::new(false);

static PREDICT_QUEUE: Lazy<StdMutex<VecDeque<(String, String)>>> =
  Lazy::new(|| StdMutex::new(VecDeque::new()));
static PREDICT_STOPPER: AtomicBool = AtomicBool::new(false);
static PLAY_QUEUE: Lazy<StdMutex<VecDeque<Vec<u8>>>> = Lazy::new(|| StdMutex::new(VecDeque::new()));
static PLAY_STOPPER: AtomicBool = AtomicBool::new(false);
static SPEAK_QUEUE_STOPPER: AtomicBool = AtomicBool::new(false);

static SPEAK_HANDLER: Lazy<StdMutex<Option<tokio::task::JoinHandle<()>>>> =
  Lazy::new(|| StdMutex::new(None));
static PREDICT_HANDLER: Lazy<StdMutex<Option<tokio::task::JoinHandle<()>>>> =
  Lazy::new(|| StdMutex::new(None));
static PLAY_HANDLER: Lazy<StdMutex<Option<tokio::task::JoinHandle<()>>>> =
  Lazy::new(|| StdMutex::new(None));

// --- 同期再生状態 ---

pub struct SyncSegment {
  pub text: String,
  pub raw_text: String,
  pub scope: usize,
  pub predictor: Box<dyn Predictor + Send + Sync>,
}

pub struct SyncReadySegment {
  pub text: String,
  pub raw_text: String,
  pub scope: usize,
  pub wav: Vec<u8>,
}

pub struct SyncPlaybackState {
  pub ready_queue: VecDeque<SyncReadySegment>,
  pub ghost_name: String,
  pub all_predicted: bool,
}

pub static SYNC_STATE: Lazy<StdMutex<Option<SyncPlaybackState>>> =
  Lazy::new(|| StdMutex::new(None));

// 世代カウンタ方式: Mutexを排除しロックフリーに
static SYNC_AUDIO_GENERATION: AtomicU64 = AtomicU64::new(0);
static SYNC_AUDIO_DONE_GEN: AtomicU64 = AtomicU64::new(0);

static SYNC_PREDICTION_HANDLER: Lazy<StdMutex<Option<tokio::task::JoinHandle<()>>>> =
  Lazy::new(|| StdMutex::new(None));

static SYNC_PLAYBACK_HANDLER: Lazy<StdMutex<Option<tokio::task::JoinHandle<()>>>> =
  Lazy::new(|| StdMutex::new(None));

// --- キュー初期化 ---

fn init_speak_queue(handle: &tokio::runtime::Handle) {
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
        if let Some(port_opener_path) =
          get_port_opener_path(format!("{}", engine.port()), &SHUTTING_DOWN).await
        {
          match getter.get_speakers_info().await {
            Ok(speakers_info) => {
              consecutive_failures.insert(engine, 0);
              let was_disconnected = {
                let cs = CURRENT_CONNECTION_STATUS
                  .read()
                  .unwrap_or_else(|e| e.into_inner());
                cs.get(&engine).is_none() || cs.get(&engine).is_some_and(|v| !*v)
              };
              if was_disconnected {
                if let Ok(mut dialogs) = CONNECTION_DIALOGS.lock() {
                  dialogs.push(format!("{} が接続されました", engine.name()));
                } else {
                  error!("Failed to lock CONNECTION_DIALOGS for connection message");
                }
                if let Ok(mut ep) = ENGINE_PATH.write() {
                  ep.insert(engine, port_opener_path);
                } else {
                  error!("Failed to lock ENGINE_PATH for engine: {}", engine.name());
                }
                if let Ok(mut auto_start) = ENGINE_AUTO_START.write() {
                  if auto_start.get(&engine).is_none() {
                    auto_start.insert(engine, false);
                  }
                } else {
                  error!(
                    "Failed to lock ENGINE_AUTO_START for engine: {}",
                    engine.name()
                  );
                }
              }
              if let Ok(mut cs) = CURRENT_CONNECTION_STATUS.write() {
                cs.insert(engine, true);
              } else {
                error!(
                  "Failed to lock CURRENT_CONNECTION_STATUS for engine: {}",
                  engine.name()
                );
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
              error!("Error: {} (consecutive failures: {})", e, *failures);

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
                  let cs = CURRENT_CONNECTION_STATUS
                    .read()
                    .unwrap_or_else(|e| e.into_inner());
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
  match SPEAK_HANDLER.lock() {
    Ok(mut guard) => *guard = Some(handler),
    Err(e) => error!("Failed to lock SPEAK_HANDLER: {}", e),
  }
}

fn init_predict_queue(handle: &tokio::runtime::Handle) {
  let handler = handle.spawn(async move {
    let mut last_activity = Instant::now();
    const MAX_IDLE_TIME: Duration = Duration::from_secs(300);

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
          last_activity = Instant::now();
          match args_to_predictors(parg).await {
            None => continue,
            Some(predictors) => {
              for predictor in predictors {
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

fn init_play_queue(handle: &tokio::runtime::Handle) {
  let handler = handle.spawn(async move {
    let mut last_activity = Instant::now();
    const MAX_IDLE_TIME: Duration = Duration::from_secs(300);

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
          last_activity = Instant::now();
          debug!("{}", format!("play: {}", data.len()));
          let volume = VOLUME.read().map(|v| *v).unwrap_or(1.0);
          match tokio::task::spawn_blocking(move || {
            play_wav(data, volume, &SHUTTING_DOWN).map_err(|e| e.to_string())
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

pub fn init_queues(handle: &tokio::runtime::Handle) {
  SHUTTING_DOWN.store(false, Ordering::Release);
  PREDICT_STOPPER.store(false, Ordering::Release);
  PLAY_STOPPER.store(false, Ordering::Release);
  SPEAK_QUEUE_STOPPER.store(false, Ordering::Release);
  crate::player::FORCE_STOP_SINK.store(false, Ordering::Release);

  // 残留データをクリア
  PREDICT_QUEUE
    .lock()
    .unwrap_or_else(|e| e.into_inner())
    .clear();
  PLAY_QUEUE.lock().unwrap_or_else(|e| e.into_inner()).clear();
  CONNECTION_DIALOGS
    .lock()
    .unwrap_or_else(|e| e.into_inner())
    .clear();

  // 同期世代カウンタをリセット
  SYNC_AUDIO_GENERATION.store(0, Ordering::Release);
  SYNC_AUDIO_DONE_GEN.store(0, Ordering::Release);
  if let Ok(mut h) = SYNC_PREDICTION_HANDLER.lock() {
    *h = None;
  }
  if let Ok(mut h) = SYNC_PLAYBACK_HANDLER.lock() {
    *h = None;
  }

  // HTTPクライアントを初期化
  crate::engine::init_http_client();

  init_speak_queue(handle);
  init_predict_queue(handle);
  init_play_queue(handle);
}

pub fn stop_queues() {
  debug!("stopping queues");
  SHUTTING_DOWN.store(true, Ordering::Release);

  // 同期再生ステートをクリア
  match SYNC_STATE.lock() {
    Ok(mut s) => *s = None,
    Err(e) => error!("Failed to lock SYNC_STATE during shutdown: {}", e),
  }

  // 音声再生を即座に強制停止
  crate::player::FORCE_STOP_SINK.store(true, Ordering::Release);
  debug!("set force stop sink flag");

  // 全停止フラグを設定
  PLAY_STOPPER.store(true, Ordering::Release);
  PREDICT_STOPPER.store(true, Ordering::Release);
  SPEAK_QUEUE_STOPPER.store(true, Ordering::Release);
  debug!("set all stop flags");

  // 同期ハンドラをabort
  for (name, handler_mutex) in [
    ("sync_prediction", &*SYNC_PREDICTION_HANDLER),
    ("sync_playback", &*SYNC_PLAYBACK_HANDLER),
  ] {
    if let Ok(mut h) = handler_mutex.lock() {
      if let Some(handle) = h.take() {
        if !handle.is_finished() {
          handle.abort();
          warn!("Aborted {} handler", name);
        }
      }
    }
  }

  // 協調的停止を待機
  let graceful_timeout = Duration::from_secs(8);
  let start_time = Instant::now();
  while start_time.elapsed() < graceful_timeout {
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
    let speak_stopped = SPEAK_HANDLER
      .lock()
      .unwrap_or_else(|e| e.into_inner())
      .as_ref()
      .is_none_or(|h| h.is_finished());

    if play_stopped && predict_stopped && speak_stopped {
      debug!("all tasks stopped gracefully");
      break;
    }

    std::thread::sleep(Duration::from_millis(200));
  }

  // ハンドル回収
  for (name, handler_mutex) in [
    ("play", &*PLAY_HANDLER),
    ("predict", &*PREDICT_HANDLER),
    ("speak", &*SPEAK_HANDLER),
  ] {
    if let Ok(mut h) = handler_mutex.lock() {
      if let Some(handle) = h.take() {
        if !handle.is_finished() {
          handle.abort();
          warn!("Aborted {} handler", name);
        }
      }
    }
  }

  // キューに残留したデータをクリア
  PREDICT_QUEUE
    .lock()
    .unwrap_or_else(|e| e.into_inner())
    .clear();
  PLAY_QUEUE.lock().unwrap_or_else(|e| e.into_inner()).clear();
  CONNECTION_DIALOGS
    .lock()
    .unwrap_or_else(|e| e.into_inner())
    .clear();

  crate::system::cleanup_system_cache();

  // HTTPクライアントを明示的にドロップ
  crate::engine::shutdown_http_client();

  debug!("stopped queues");
}

// --- 非同期読み上げ ---

pub fn push_to_prediction(text: String, ghost_name: String) {
  if SHUTTING_DOWN.load(Ordering::Acquire) {
    return;
  }
  PREDICT_QUEUE
    .lock()
    .unwrap_or_else(|e| e.into_inner())
    .push_back((text, ghost_name));
}

async fn args_to_predictors(
  args: (String, String),
) -> Option<VecDeque<Box<dyn Predictor + Send + Sync>>> {
  let (text, ghost_name) = args;
  build_segments_async(text, ghost_name, false)
    .await
    .map(|segments| {
      segments
        .into_iter()
        .filter(|seg| !is_ellipsis_segment(&seg.text))
        .map(|seg| seg.predictor)
        .collect()
    })
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
  let (devide_by_lines, speak_by_punctuation_val, speakers, initial_voice, volume) = {
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
    let volume = match VOLUME.read() {
      Ok(v) => *v,
      Err(e) => {
        error!("Failed to read VOLUME: {}", e);
        1.0
      }
    };
    (
      devide_by_lines,
      speak_by_punctuation_val,
      speakers,
      initial_voice,
      volume,
    )
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
      let p = split_by_punctuation_with_raw(dialog.text.clone(), dialog.raw_text.clone());
      // 同期モード: \_q内の省略記号をraw_textベースで再分割
      if sync_mode {
        resplit_pairs_by_raw_ellipsis(p)
      } else {
        p
      }
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
        Engine::BouyomiChan => Box::new(BouyomichanPredictor::new(
          t.clone(),
          speaker.style_id,
          volume,
        )),
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

/// sync ラッパー
pub fn build_segments(
  text: String,
  ghost_name: String,
  sync_mode: bool,
  handle: &tokio::runtime::Handle,
) -> Option<Vec<SyncSegment>> {
  handle.block_on(build_segments_async(text, ghost_name, sync_mode))
}

// --- 同期再生 ---

pub fn spawn_sync_playback(wav: Vec<u8>, handle: &tokio::runtime::Handle) {
  if SHUTTING_DOWN.load(Ordering::Acquire) {
    return;
  }
  let gen = SYNC_AUDIO_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
  // cancel_sync_playback で設定された FORCE_STOP_SINK をリセット
  crate::player::FORCE_STOP_SINK.store(false, Ordering::Release);
  let volume = VOLUME.read().map(|v| *v).unwrap_or(1.0);
  let task_handle = handle.spawn(async move {
    if !wav.is_empty() {
      match tokio::task::spawn_blocking(move || {
        play_wav(wav, volume, &SHUTTING_DOWN).map_err(|e| e.to_string())
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
  // 同期再生タスクを追跡
  if let Ok(mut h) = SYNC_PLAYBACK_HANDLER.lock() {
    *h = Some(task_handle);
  }
}

pub fn is_sync_audio_done() -> bool {
  SYNC_AUDIO_DONE_GEN.load(Ordering::SeqCst) >= SYNC_AUDIO_GENERATION.load(Ordering::SeqCst)
}

/// 同期再生をキャンセル
pub fn cancel_sync_playback() {
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
pub fn spawn_sync_prediction(
  segments: Vec<SyncSegment>,
  ghost_name: String,
  handle: &tokio::runtime::Handle,
) {
  if SHUTTING_DOWN.load(Ordering::Acquire) {
    return;
  }

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
pub fn pop_ready_segment(ghost_name: &str) -> (Option<SyncReadySegment>, bool) {
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
