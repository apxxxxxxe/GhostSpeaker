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
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use tokio::sync::Mutex;

pub(crate) static CONNECTION_DIALOGS: Lazy<StdMutex<Vec<String>>> =
  Lazy::new(|| StdMutex::new(Vec::new()));

pub(crate) static RUNTIME: Lazy<StdMutex<Option<tokio::runtime::Runtime>>> =
  Lazy::new(|| StdMutex::new(Some(tokio::runtime::Runtime::new().unwrap())));
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

fn init_speak_queue() {
  let mut runtime = RUNTIME.lock().unwrap();
  let mut speak_handlers = Vec::new();
  for (engine, getter) in get_speaker_getters() {
    let handler = runtime.as_mut().unwrap().spawn(async move {
      loop {
        if let Some(port_opener_path) = get_port_opener_path(format!("{}", engine.port())) {
          match getter.get_speakers_info().await {
            Ok(speakers_info) => {
              let mut connection_status = CURRENT_CONNECTION_STATUS.write().await;
              if connection_status.get(&engine).is_none()
                || connection_status.get(&engine).is_some_and(|v| !*v)
              {
                {
                  CONNECTION_DIALOGS
                    .lock()
                    .unwrap()
                    .push(format!("{} が接続されました", engine.name()));
                }
                // 接続時、ポートを開いているプロセスのパスを記録
                ENGINE_PATH
                  .write()
                  .unwrap()
                  .insert(engine, port_opener_path);
                let mut auto_start = ENGINE_AUTO_START.write().await;
                if auto_start.get(&engine).is_none() {
                  auto_start.insert(engine, false);
                }
              }
              connection_status.insert(engine, true);
              SPEAKERS_INFO.write().await.insert(engine, speakers_info);
            }
            Err(e) => {
              error!("Error: {}", e);
              let mut connection_status = CURRENT_CONNECTION_STATUS.write().await;
              if connection_status.get(&engine).is_some_and(|v| *v) {
                CONNECTION_DIALOGS
                  .lock()
                  .unwrap()
                  .push(format!("{} が切断されました", engine.name()));
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
  let handler = RUNTIME.lock().unwrap().as_mut().unwrap().spawn(async move {
    loop {
      {
        if predict_queue_cln.lock().await.is_empty() {
          if *predict_stopper_cln.lock().await {
            break;
          }
          tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
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
        Some(parg) => match args_to_predictors(parg) {
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
        },
      }
    }
  });
  futures::executor::block_on(async {
    *PREDICT_HANDLER.lock().await = Some(handler);
  });
}

pub(crate) fn init_play_queue() {
  let play_queue_cln = PLAY_QUEUE.clone();
  let play_stopper_cln = PLAY_STOPPER.clone();
  let handler = RUNTIME.lock().unwrap().as_mut().unwrap().spawn(async move {
    loop {
      {
        if play_queue_cln.lock().await.is_empty() {
          if *play_stopper_cln.lock().await {
            break;
          }
          tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
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
          debug!("{}", format!("play: {}", data.len()));
          if let Err(e) = play_wav(data) {
            error!("play_wav failed: {}", e);
          };
        }
      }
    }
  });
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
  {
    // stop signals
    futures::executor::block_on(async {
      // speak_handler の停止
      for handler in SPEAK_HANDLERS.lock().await.iter() {
        handler.abort();
      }
      debug!("{}", "stopped speak");
      // predict_handler の停止
      {
        *PREDICT_STOPPER.lock().await = true;
      }
      loop {
        {
          let predict_stopped = if let Some(handler) = &*PREDICT_HANDLER.lock().await {
            handler.is_finished()
          } else {
            true
          };
          if predict_stopped {
            break;
          }
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
        debug!("{}", "stopping predict");
      }
      debug!("{}", "stopped predict");
      // play_handler の停止
      {
        *PLAY_STOPPER.lock().await = true;
      }
      loop {
        {
          let play_stopped = if let Some(handler) = &*PLAY_HANDLER.lock().await {
            handler.is_finished()
          } else {
            true
          };
          if play_stopped {
            break;
          }
          debug!("{}", "stopping play");
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
      }
      debug!("{}", "stopped play");
    });
  }
  debug!("{}", "stopped queue");
  if let Some(runtime) = RUNTIME.lock()?.take() {
    runtime.shutdown_background();
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
  let mut predictors: VecDeque<Box<dyn Predictor + Send + Sync>> = VecDeque::new();
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
  if connected_engines.clone().is_empty() {
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
  let speak_quicksection = GHOSTS_VOICES
    .read()
    .unwrap()
    .get(&ghost_name)
    .unwrap()
    .speak_quicksection;

  let speak_by_punctuation = SPEAK_BY_PUNCTUATION.read().unwrap();

  let ghosts_voices = GHOSTS_VOICES.write().unwrap();
  let speakers = &ghosts_voices.get(&ghost_name).unwrap().voices;
  for dialog in split_dialog(text, devide_by_lines, speak_quicksection) {
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
          // エンジン側に声質が存在しないならスキップ
          voice_not_found = true;
        }
      }
    });
    if voice_not_found {
      continue;
    }
    let engine = engine_from_port(speaker.port).unwrap();
    let texts = if *speak_by_punctuation && engine != Engine::BouyomiChan {
      split_by_punctuation(dialog.text)
    } else {
      /* 棒読みちゃんは細切れの恩恵が少ない&
      読み上げ順がばらばらになることがあるので常にまとめて読み上げる */
      vec![dialog.text]
    };
    for text in texts {
      match engine {
        Engine::CoeiroInkV2 => {
          predictors.push_back(Box::new(CoeiroinkV2Predictor::new(
            text,
            speaker.speaker_uuid.clone(),
            speaker.style_id,
          )));
        }
        Engine::BouyomiChan => {
          predictors.push_back(Box::new(BouyomichanPredictor::new(text, speaker.style_id)));
        }
        Engine::CoeiroInkV1
        | Engine::VoiceVox
        | Engine::Lmroid
        | Engine::ShareVox
        | Engine::ItVoice
        | Engine::AivisSpeech => {
          predictors.push_back(Box::new(VoicevoxFamilyPredictor::new(
            engine,
            text,
            speaker.style_id,
          )));
        }
      }
    }
  }
  Some(predictors)
}
