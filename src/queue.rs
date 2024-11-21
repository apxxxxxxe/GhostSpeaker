use crate::engine::bouyomichan::predict::BouyomichanPredictor;
use crate::engine::coeiroink_v2::predict::CoeiroinkV2Predictor;
use crate::engine::voicevox_family::predict::VoicevoxFamilyPredictor;
use crate::engine::{engine_from_port, get_speaker_getters, Engine, Predictor, NO_VOICE_UUID};
use crate::format::{split_by_punctuation, split_dialog};
use crate::player::{cooperative_free_player, force_free_player, play_wav};
use crate::variables::get_global_vars;
use async_std::sync::Arc;
use std::collections::VecDeque;
use tokio::sync::{Mutex, Notify};

pub static mut QUEUE: Option<Queue> = None;

pub struct Queue {
  runtime: Option<tokio::runtime::Runtime>,
  predict_queue: Arc<Mutex<VecDeque<(String, String)>>>,
  predict_notifier: Arc<Notify>,
  play_queue: Arc<Mutex<VecDeque<Vec<u8>>>>,
  play_notifier: Arc<Notify>,
}

impl Queue {
  pub fn new() -> Self {
    Self {
      runtime: Some(tokio::runtime::Runtime::new().unwrap()),
      predict_queue: Arc::new(Mutex::new(VecDeque::new())),
      predict_notifier: Arc::new(Notify::new()),
      play_queue: Arc::new(Mutex::new(VecDeque::new())),
      play_notifier: Arc::new(Notify::new()),
    }
  }

  pub fn init(&mut self) {
    for (engine, getter) in get_speaker_getters() {
      self.runtime.as_mut().unwrap().spawn(async move {
        loop {
          let sinfo = &mut get_global_vars().volatility.speakers_info;
          let connection_status = &mut get_global_vars().volatility.current_connection_status;
          match getter.get_speakers_info().await {
            Ok(speakers_info) => {
              connection_status.insert(engine, true);
              sinfo.insert(engine, speakers_info);
            }
            Err(e) => {
              error!("Error: {}", e);
              connection_status.insert(engine, false);
              sinfo.remove(&engine);
            }
          }
          tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
      });
    }

    let predict_queue_cln = Arc::clone(&self.predict_queue);
    let predict_notifier_cln = Arc::clone(&self.predict_notifier);
    self.runtime.as_mut().unwrap().spawn(async move {
      loop {
        if predict_queue_cln.lock().await.is_empty() {
          predict_notifier_cln.notified().await;
          continue;
        }

        let parg;
        {
          let mut guard = predict_queue_cln.lock().await;
          parg = guard.pop_front();
        }

        match parg {
          None => continue,
          Some(parg) => match args_to_predictors(parg).await {
            None => continue,
            Some(predictors) => {
              for predictor in predictors {
                match predictor.predict().await {
                  Ok(res) => {
                    debug!("pushing to play");
                    get_queue().push_to_play(res);
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

    let play_queue_cln = self.play_queue.clone();
    let play_notifier_cln = self.play_notifier.clone();
    self.runtime.as_mut().unwrap().spawn(async move {
      loop {
        if play_queue_cln.lock().await.is_empty() {
          play_notifier_cln.notified().await;
          continue;
        }

        let wav;
        {
          let mut guard = play_queue_cln.lock().await;
          wav = guard.pop_front();
        }
        if let Some(data) = wav {
          if !data.is_empty() {
            debug!("{}", format!("play: {}", data.len()));
            play_wav(data);
          }
        }
      }
    });
  }

  pub fn stop(&mut self) {
    debug!("{}", "stopping queue");
    if get_global_vars().wait_for_speech.unwrap() {
      cooperative_free_player();
    } else {
      force_free_player();
    }
    if let Some(runtime) = self.runtime.take() {
      runtime.shutdown_background();
      debug!("{}", "shutdown speaker's runtime");
    }
    debug!("{}", "stopped queue");
  }

  // remove all queue and stop running threads
  pub fn restart(&mut self) {
    debug!("{}", "restarting queue");
    self.predict_queue = Arc::new(Mutex::new(VecDeque::new()));
    self.play_queue = Arc::new(Mutex::new(VecDeque::new()));
    self.runtime = Some(tokio::runtime::Runtime::new().unwrap());
    self.init();
    debug!("{}", "restarted queue");
  }

  pub fn push_to_prediction(&self, text: String, ghost_name: String) {
    debug!("pushing to prediction");
    futures::executor::block_on(async {
      // 処理が重いので、別スレッドに投げてそっちでPredictorを作る
      self
        .predict_queue
        .lock()
        .await
        .push_back((text, ghost_name));
    });
    self.predict_notifier.notify_one();
    debug!("pushed and notified to prediction");
  }

  fn push_to_play(&self, data: Vec<u8>) {
    debug!("pushing to play");
    futures::executor::block_on(async {
      self.play_queue.lock().await.push_back(data);
    });
    self.play_notifier.notify_one();
    debug!("pushed and notified to play");
  }
}

async fn args_to_predictors(
  args: (String, String),
) -> Option<VecDeque<Box<dyn Predictor + Send + Sync>>> {
  let (text, ghost_name) = args;
  let mut predictors: VecDeque<Box<dyn Predictor + Send + Sync>> = VecDeque::new();
  let connected_engines = get_global_vars()
    .volatility
    .current_connection_status
    .iter()
    .filter(|(_, v)| **v)
    .map(|(k, _)| *k)
    .collect::<Vec<_>>();
  if connected_engines.clone().is_empty() {
    debug!("no engine connected: skip: {}", text);
    return None;
  }

  debug!("{}", format!("predicting: {}", text));
  let devide_by_lines = get_global_vars()
    .ghosts_voices
    .as_ref()
    .unwrap()
    .get(&ghost_name)
    .unwrap()
    .devide_by_lines;

  let speakers = &get_global_vars()
    .ghosts_voices
    .as_ref()
    .unwrap()
    .get(&ghost_name)
    .unwrap()
    .voices;

  let speak_by_punctuation = get_global_vars().speak_by_punctuation.unwrap();

  for dialog in split_dialog(text, devide_by_lines) {
    if dialog.text.is_empty() {
      continue;
    }

    let initial_speaker = &get_global_vars().initial_voice;
    debug!("selecting speaker: {}", dialog.scope);
    let speaker = match speakers.get(dialog.scope) {
      Some(speaker) => {
        if let Some(speaker) = speaker {
          speaker.clone()
        } else {
          initial_speaker.clone()
        }
      }
      None => initial_speaker.clone(),
    };

    if speaker.speaker_uuid == NO_VOICE_UUID {
      continue;
    }
    if let Some(speakers_by_engine) = get_global_vars()
      .volatility
      .speakers_info
      .get(&(engine_from_port(speaker.port).unwrap()))
    {
      if !speakers_by_engine
        .iter()
        .any(|s| s.speaker_uuid == speaker.speaker_uuid)
      {
        // エンジン側に声質が存在しないならスキップ
        continue;
      }
    }
    let engine = engine_from_port(speaker.port).unwrap();
    let texts = if speak_by_punctuation && engine != Engine::BouyomiChan {
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

// for singleton
pub fn get_queue() -> &'static mut Queue {
  unsafe {
    if QUEUE.is_none() {
      QUEUE = Some(Queue::new());
      QUEUE.as_mut().unwrap().init();
    }
    QUEUE.as_mut().unwrap()
  }
}
