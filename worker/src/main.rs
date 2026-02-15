mod engine;
mod format;
mod player;
mod queue;
mod system;

use ghost_speaker_common::{Command, Response, SegmentInfo, SyncState};
use log::{debug, error, info};
use simplelog::{Config, LevelFilter, WriteLogger};
use std::io::{BufRead, Write};
use std::sync::atomic::Ordering;
use std::time::Duration;

use crate::format::is_ellipsis_segment;
use crate::queue::{
  build_segments, cancel_sync_playback, is_sync_audio_done, pop_ready_segment, push_to_prediction,
  spawn_sync_playback, spawn_sync_prediction, SyncSegment, CURRENT_CONNECTION_STATUS,
  ENGINE_AUTO_START, ENGINE_PATH, GHOSTS_VOICES, INITIAL_VOICE, SHUTTING_DOWN, SPEAKERS_INFO,
  SPEAK_BY_PUNCTUATION, SYNC_STATE, VOLUME,
};

/// ワーカーの状態を保持する構造体
struct WorkerState {
  runtime_handle: tokio::runtime::Handle,
  /// 現在の同期再生で ghost_name を保持する（SyncPoll時に参照）
  sync_ghost_name: Option<String>,
}

fn main() {
  let runtime = tokio::runtime::Builder::new_multi_thread()
    .worker_threads(2)
    .max_blocking_threads(4)
    .enable_all()
    .build()
    .expect("Failed to create tokio runtime");

  let handle = runtime.handle().clone();

  // stdin/stdout を取得
  let stdin = std::io::stdin();
  let stdout = std::io::stdout();
  let reader = stdin.lock();
  let mut writer = stdout.lock();

  let mut lines = reader.lines();

  // 最初の Init コマンドを待つ
  let init_line = match lines.next() {
    Some(Ok(line)) => line,
    Some(Err(e)) => {
      eprintln!("Failed to read init command: {}", e);
      std::process::exit(1);
    }
    None => {
      eprintln!("stdin closed before init command");
      std::process::exit(1);
    }
  };

  let init_cmd: Command = match serde_json::from_str(&init_line) {
    Ok(cmd) => cmd,
    Err(e) => {
      eprintln!("Failed to parse init command: {}", e);
      std::process::exit(1);
    }
  };

  let (dll_dir, config) = match init_cmd {
    Command::Init { dll_dir, config } => (dll_dir, config),
    other => {
      // Init以外が来た場合、エラーを返す
      let resp = Response::Error {
        message: format!("Expected Init command, got: {:?}", other),
      };
      let _ = serde_json::to_writer(&mut writer, &resp);
      let _ = writeln!(writer);
      let _ = writer.flush();
      std::process::exit(1);
    }
  };

  // ロギング初期化
  let log_path = std::path::Path::new(&dll_dir).join("ghost-speaker-worker.log");
  if let Ok(file) = std::fs::File::create(&log_path) {
    let _ = WriteLogger::init(LevelFilter::Debug, Config::default(), file);
  }

  info!("Worker started, dll_dir={}", dll_dir);

  // 設定をグローバル状態に反映
  if let Ok(mut v) = VOLUME.write() {
    *v = config.volume;
  }
  if let Ok(mut sbp) = SPEAK_BY_PUNCTUATION.write() {
    *sbp = config.speak_by_punctuation;
  }
  if let Ok(mut gv) = GHOSTS_VOICES.write() {
    *gv = config.ghosts_voices;
  }
  if let Ok(mut iv) = INITIAL_VOICE.write() {
    *iv = config.initial_voice;
  }
  if let Ok(mut ea) = ENGINE_AUTO_START.write() {
    *ea = config.engine_auto_start;
  }
  if let Ok(mut ep) = ENGINE_PATH.write() {
    *ep = config.engine_path;
    // Remove corrupted paths that point to the worker itself
    if let Ok(current_exe) = std::env::current_exe() {
      ep.retain(|engine, path| {
        let is_self = std::path::Path::new(path) == current_exe.as_path();
        if is_self {
          log::warn!(
            "Removing corrupted engine path for {}: points to worker itself",
            engine.name()
          );
        }
        !is_self
      });
    }
  }

  // キューを初期化
  queue::init_queues(&handle);

  // エンジン自動起動
  {
    let auto_start = ENGINE_AUTO_START.read().map(|ea| ea.clone()).unwrap_or_default();
    let engine_path = ENGINE_PATH.read().map(|ep| ep.clone()).unwrap_or_default();

    for (engine, should_start) in &auto_start {
      if *should_start {
        match system::boot_engine(*engine, &engine_path) {
          Ok(()) => info!("Auto-started engine: {}", engine.name()),
          Err(e) => error!("Failed to auto-start engine {}: {}", engine.name(), e),
        }
      }
    }
  }

  // Init の応答を返す
  let resp = Response::Ok;
  let _ = serde_json::to_writer(&mut writer, &resp);
  let _ = writeln!(writer);
  let _ = writer.flush();

  let mut state = WorkerState {
    runtime_handle: handle.clone(),
    sync_ghost_name: None,
  };

  // メインコマンドループ
  for line_result in lines {
    let line = match line_result {
      Ok(l) => l,
      Err(e) => {
        error!("Failed to read from stdin: {}", e);
        break;
      }
    };

    if line.is_empty() {
      continue;
    }

    let cmd: Command = match serde_json::from_str(&line) {
      Ok(c) => c,
      Err(e) => {
        error!("Failed to parse command: {}", e);
        let resp = Response::Error {
          message: format!("Parse error: {}", e),
        };
        let _ = serde_json::to_writer(&mut writer, &resp);
        let _ = writeln!(writer);
        let _ = writer.flush();
        continue;
      }
    };

    let resp = handle_command(cmd, &mut state);

    let _ = serde_json::to_writer(&mut writer, &resp);
    let _ = writeln!(writer);
    let _ = writer.flush();

    // Shutdown が処理された場合は終了
    if SHUTTING_DOWN.load(Ordering::Acquire) {
      break;
    }
  }

  info!("Worker shutting down");

  if queue::GRACEFUL_SHUTDOWN.load(Ordering::Acquire) {
    info!("Waiting for playback to drain...");
    queue::wait_for_playback_drain(Duration::from_secs(60));
  }

  // ランタイムを正常にシャットダウン（dropによる暗黙のシャットダウン）
  drop(runtime);
  info!("Worker shutdown complete");
}

fn handle_command(cmd: Command, state: &mut WorkerState) -> Response {
  match cmd {
    Command::Init { .. } => {
      // 二重初期化は不可
      Response::Error {
        message: "Already initialized".to_string(),
      }
    }

    Command::Shutdown => {
      debug!("Shutdown command received");
      queue::stop_queues();
      Response::Ok
    }

    Command::SpeakAsync { text, ghost_name } => {
      push_to_prediction(text, ghost_name);
      Response::Ok
    }

    Command::SyncStart { text, ghost_name } => handle_sync_start(text, ghost_name, state),

    Command::SyncPoll => handle_sync_poll(state),

    Command::SyncCancel => {
      cancel_sync_playback();
      state.sync_ghost_name = None;
      Response::Ok
    }

    Command::PopDialog => {
      let message = match queue::CONNECTION_DIALOGS.lock() {
        Ok(mut dialogs) => {
          if dialogs.is_empty() {
            None
          } else {
            Some(dialogs.remove(0))
          }
        }
        Err(e) => {
          error!("Failed to lock CONNECTION_DIALOGS: {}", e);
          None
        }
      };
      Response::Dialog { message }
    }

    Command::GetEngineStatus => {
      let speakers_info = SPEAKERS_INFO
        .read()
        .map(|si| si.clone())
        .unwrap_or_default();
      let connection_status = CURRENT_CONNECTION_STATUS
        .read()
        .map(|cs| cs.clone())
        .unwrap_or_default();
      let engine_paths = ENGINE_PATH.read().map(|ep| ep.clone()).unwrap_or_default();
      let engine_auto_start = ENGINE_AUTO_START
        .read()
        .map(|ea| ea.clone())
        .unwrap_or_default();
      Response::EngineStatus {
        speakers_info,
        connection_status,
        engine_paths,
        engine_auto_start,
      }
    }

    Command::UpdateVolume { volume } => {
      if let Ok(mut v) = VOLUME.write() {
        *v = volume;
      }
      Response::Ok
    }

    Command::UpdateGhostVoices { ghost_name, info } => {
      if let Ok(mut gv) = GHOSTS_VOICES.write() {
        gv.insert(ghost_name, info);
      }
      Response::Ok
    }

    Command::UpdateInitialVoice { voice } => {
      if let Ok(mut iv) = INITIAL_VOICE.write() {
        *iv = voice;
      }
      Response::Ok
    }

    Command::UpdateSpeakByPunctuation { enabled } => {
      if let Ok(mut sbp) = SPEAK_BY_PUNCTUATION.write() {
        *sbp = enabled;
      }
      Response::Ok
    }

    Command::UpdateEngineAutoStart { engine, auto_start } => {
      if let Ok(mut ea) = ENGINE_AUTO_START.write() {
        ea.insert(engine, auto_start);
      }
      Response::Ok
    }

    Command::BootEngine { engine } => {
      let engine_path = ENGINE_PATH.read().map(|ep| ep.clone()).unwrap_or_default();
      match system::boot_engine(engine, &engine_path) {
        Ok(()) => Response::Ok,
        Err(e) => Response::Error {
          message: format!("Failed to boot engine: {}", e),
        },
      }
    }

    Command::ForceStopPlayback => {
      player::FORCE_STOP_SINK.store(true, Ordering::Release);
      cancel_sync_playback();
      state.sync_ghost_name = None;
      Response::Ok
    }

    Command::GracefulShutdown => {
      debug!("GracefulShutdown command received");
      queue::graceful_stop_queues();
      Response::Ok
    }
  }
}

fn handle_sync_start(text: String, ghost_name: String, state: &mut WorkerState) -> Response {
  // 既存の同期再生をキャンセル
  cancel_sync_playback();

  let handle = &state.runtime_handle;

  // セグメントを構築
  let segments = match build_segments(text.clone(), ghost_name.clone(), true, handle) {
    Some(s) if s.len() >= 2 => s,
    _ => {
      // セグメント1つ以下 → 同期不要、通常モードにフォールバック
      debug!("SyncStart: segments < 2, falling back to async mode");
      // cancel_sync_playback()で設定されたFORCE_STOP_SINKをリセット
      // (非同期再生パスが影響を受けないようにする)
      player::FORCE_STOP_SINK.store(false, Ordering::Release);
      push_to_prediction(text, ghost_name);
      return Response::SyncStarted {
        first_segment: None,
        has_more: false,
      };
    }
  };

  // デバッグ: 全セグメントの内容をログ出力
  debug!(
    "SyncStart: {} segments for ghost={}",
    segments.len(),
    ghost_name
  );
  for (i, seg) in segments.iter().enumerate() {
    debug!(
      "  seg[{}]: text={:?}, raw_text={:?}, scope={}",
      i, seg.text, seg.raw_text, seg.scope
    );
  }

  // 最初のセグメントを分離
  let mut segments_iter = segments.into_iter();
  let first = segments_iter.next().unwrap();
  let remaining: Vec<SyncSegment> = segments_iter.collect();
  let has_more = !remaining.is_empty();

  // 最初のセグメント情報
  let first_info = SegmentInfo {
    text: first.text.clone(),
    raw_text: first.raw_text.clone(),
    scope: first.scope,
    is_ellipsis: is_ellipsis_segment(&first.text),
  };

  if !first_info.is_ellipsis && !first.text.is_empty() {
    let wav_result: Result<Vec<u8>, String> = handle.block_on(async {
      tokio::time::timeout(
        std::time::Duration::from_secs(30),
        first.predictor.predict(),
      )
      .await
      .map_err(|_| "predict timed out".to_string())
      .and_then(|r| r.map_err(|e| e.to_string()))
    });

    match wav_result {
      Ok(wav) => {
        // 最初のセグメントを再生開始
        spawn_sync_playback(wav, first.volume, handle);
      }
      Err(e) => {
        error!("First segment predict failed: {}", e);
        // 空のwavで再生開始（すぐ完了する）
        spawn_sync_playback(Vec::new(), first.volume, handle);
      }
    }
  } else {
    // 省略記号/空テキストセグメント: 空のwavで再生開始
    spawn_sync_playback(Vec::new(), first.volume, handle);
  }

  // 残りのセグメントをバックグラウンドで合成
  if has_more {
    spawn_sync_prediction(remaining, ghost_name.clone(), handle);
  }

  state.sync_ghost_name = Some(ghost_name);

  Response::SyncStarted {
    first_segment: Some(first_info),
    has_more,
  }
}

fn handle_sync_poll(state: &mut WorkerState) -> Response {
  let ghost_name = match &state.sync_ghost_name {
    Some(name) => name.clone(),
    None => {
      return Response::SyncStatus {
        state: SyncState::Complete,
      };
    }
  };

  let handle = &state.runtime_handle;

  // 1. 現在のオーディオがまだ再生中か？
  if !is_sync_audio_done() {
    return Response::SyncStatus {
      state: SyncState::Playing,
    };
  }

  // 2. 次のセグメントがプールに準備できているか？
  let (ready_seg, has_more) = pop_ready_segment(&ghost_name);

  match ready_seg {
    Some(seg) => {
      let segment_info = SegmentInfo {
        text: seg.text.clone(),
        raw_text: seg.raw_text.clone(),
        scope: seg.scope,
        is_ellipsis: is_ellipsis_segment(&seg.text),
      };

      debug!(
        "SyncPoll: Ready seg text={:?}, raw_text={:?}, is_ellipsis={}, has_more={}",
        seg.text, seg.raw_text, segment_info.is_ellipsis, has_more
      );

      // 省略記号セグメントと空テキストセグメント（quicksection由来）は音声再生なし
      if !segment_info.is_ellipsis && !seg.text.is_empty() {
        spawn_sync_playback(seg.wav, seg.volume, handle);
      }

      // 最後のセグメント → ステートクリア
      if !has_more {
        match SYNC_STATE.lock() {
          Ok(mut s) => *s = None,
          Err(e) => error!("Failed to lock SYNC_STATE for cleanup: {}", e),
        }
      }

      Response::SyncStatus {
        state: SyncState::Ready {
          segment: segment_info,
          has_more,
        },
      }
    }
    None => {
      // セグメントが無い
      // SYNC_STATE をチェック: all_predicted で ready_queue が空 なら Complete
      let is_complete = match SYNC_STATE.lock() {
        Ok(sync_state) => match sync_state.as_ref() {
          Some(s) if s.ghost_name == ghost_name => s.all_predicted && s.ready_queue.is_empty(),
          Some(_) => true, // 別のゴーストの状態: 完了扱い
          None => true,    // 状態なし: 完了扱い
        },
        Err(_) => true,
      };

      if is_complete {
        state.sync_ghost_name = None;
        match SYNC_STATE.lock() {
          Ok(mut s) => *s = None,
          Err(e) => error!("Failed to lock SYNC_STATE for complete cleanup: {}", e),
        }
        Response::SyncStatus {
          state: SyncState::Complete,
        }
      } else {
        // まだ合成中だがセグメントが未完了
        Response::SyncStatus {
          state: SyncState::Waiting,
        }
      }
    }
  }
}
