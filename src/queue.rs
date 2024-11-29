use crate::engine::bouyomichan::predict::BouyomichanPredictor;
use crate::engine::coeiroink_v2::predict::CoeiroinkV2Predictor;
use crate::engine::voicevox_family::predict::VoicevoxFamilyPredictor;
use crate::engine::{engine_from_port, get_speaker_getters, Engine, Predictor, NO_VOICE_UUID};
use crate::format::{split_by_punctuation, split_dialog};
// use crate::player::{cooperative_free_player, force_free_player, play_wav};
use crate::system::get_port_opener_path;
use crate::variables::get_global_vars;
use futures::executor::block_on;
use std::collections::VecDeque;
use std::sync::Condvar;
use std::sync::{Arc, Mutex};
use std::thread::{spawn, JoinHandle};
use tokio::runtime::Runtime;

pub static mut QUEUE: Option<Queue> = None;

#[derive(Debug, Eq, PartialEq)]
enum FlowType {
  Pause,
  Continue,
  Break,
}

pub struct Queue {
  speaker_handler: Option<JoinHandle<()>>,
  speaker_pauser: Arc<Mutex<FlowType>>,
  predict_queue: Arc<Mutex<VecDeque<(String, String)>>>,
  predict_handler: Option<JoinHandle<()>>,
  predict_pauser: Arc<(Mutex<FlowType>, Condvar)>,
  play_queue: Arc<Mutex<VecDeque<Vec<u8>>>>,
  play_handler: Option<JoinHandle<()>>,
  play_pauser: Arc<(Mutex<FlowType>, Condvar)>,
}

impl Queue {
  pub fn new() -> Self {
    Self {
      speaker_handler: None,
      speaker_pauser: Arc::new(Mutex::new(FlowType::Continue)),
      predict_queue: Arc::new(Mutex::new(VecDeque::new())),
      predict_handler: None,
      predict_pauser: Arc::new((Mutex::new(FlowType::Pause), Condvar::new())),
      play_queue: Arc::new(Mutex::new(VecDeque::new())),
      play_handler: None,
      play_pauser: Arc::new((Mutex::new(FlowType::Pause), Condvar::new())),
    }
  }

  pub fn init(&mut self) {
    let speaker_pauser_cln = Arc::clone(&self.speaker_pauser);
    self.speaker_handler = Some(spawn(move || {
      let connection_status = &mut get_global_vars().volatility.current_connection_status;
      // let runtime = Runtime::new().unwrap();
      loop {
        // for (engine, getter) in get_speaker_getters() {
        //   let sinfo = &mut get_global_vars().volatility.speakers_info;
        //   // // ポートが開いていないならスキップ
        //   // if get_port_opener_path(engine.port()).is_none() {
        //   //   connection_status.insert(engine, false);
        //   //   sinfo.remove(&engine);
        //   //   continue;
        //   // }
        //   // match runtime.block_on(getter.get_speakers_info()) {
        //   //   Ok(speakers_info) => {
        //   //     connection_status.insert(engine, true);
        //   //     sinfo.insert(engine, speakers_info);
        //   //   }
        //   //   Err(e) => {
        //   //     error!("Error: {}", e);
        //   //     connection_status.insert(engine, false);
        //   //     sinfo.remove(&engine);
        //   //   }
        //   // }
        {
          let guard = speaker_pauser_cln.lock().unwrap();
          if *guard == FlowType::Break {
              panic!("speaker thread is stopped");
            break;
        }
        }
        // }
        std::thread::sleep(std::time::Duration::from_secs(1));
      }
    }));

    let predict_queue_cln = Arc::clone(&self.predict_queue);
    let predict_pauser_cln = Arc::clone(&self.predict_pauser);
    self.predict_handler = Some(spawn(move || loop {
      if predict_queue_cln.lock().unwrap().is_empty() {
        let (lock, cvar) = &*predict_pauser_cln;
        let mut guard = lock.lock().unwrap();
        while *guard == FlowType::Pause {
          guard = cvar.wait(guard).unwrap();
        }
        if *guard == FlowType::Break {
          break;
        }
        *guard = FlowType::Pause;
      }

      let parg;
      {
        let mut guard = predict_queue_cln.lock().unwrap();
        parg = guard.pop_front();
      }

      match parg {
        None => continue,
        Some(parg) => match block_on(args_to_predictors(parg)) {
          None => continue,
          Some(predictors) => {
            for predictor in predictors {
              match block_on(predictor.predict()) {
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
    }));

    let play_queue_cln = self.play_queue.clone();
    self.play_handler = Some(spawn(move || loop {
      if play_queue_cln.lock().unwrap().is_empty() {
        let (lock, cvar) = &*get_queue().play_pauser;
        let mut guard = lock.lock().unwrap();
        while *guard == FlowType::Pause {
          guard = cvar.wait(guard).unwrap();
        }
        if *guard == FlowType::Break {
          break;
        }
        *guard = FlowType::Pause;
      }

      let wav;
      {
        let mut guard = play_queue_cln.lock().unwrap();
        wav = guard.pop_front();
      }
      if let Some(data) = wav {
        if !data.is_empty() {
          debug!("{}", format!("play: {}", data.len()));
          // play_wav(data);
        }
      }
    }));
  }

  pub fn stop(&mut self) {
    debug!("{}", "stopping queue");
    if get_global_vars().wait_for_speech.unwrap() {
      // cooperative_free_player();
    } else {
      // force_free_player();
    }
    // stop all threads
    let lock = &*self.speaker_pauser;
    {
      let mut guard = lock.lock().unwrap();
      *guard = FlowType::Break;
    }
    debug!("{}", "stopping speaker");
    let (lock, cvar) = &*self.predict_pauser;
    let mut guard = lock.lock().unwrap();
    *guard = FlowType::Break;
    cvar.notify_one();
    debug!("{}", "stopping predict");
    let (lock, cvar) = &*self.play_pauser;
    let mut guard = lock.lock().unwrap();
    *guard = FlowType::Break;
    cvar.notify_one();
    debug!("{}", "stopping play");
    let mut is_speaker_stoppedd = false;
    let mut is_predict_stopped = false;
    let mut is_play_stopped = false;
    while !is_speaker_stoppedd || !is_predict_stopped || !is_play_stopped {
      is_speaker_stoppedd = self.speaker_handler.take().is_none();
      is_predict_stopped = self.predict_handler.take().is_none();
      is_play_stopped = self.play_handler.take().is_none();
      std::thread::sleep(std::time::Duration::from_millis(100));
    }
    debug!("{}", "stopped speaker");
    debug!("{}", "stopped queue");
  }

  // remove all queue and stop running threads
  pub fn restart(&mut self) {
    debug!("{}", "restarting queue");
    self.predict_queue = Arc::new(Mutex::new(VecDeque::new()));
    self.play_queue = Arc::new(Mutex::new(VecDeque::new()));
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
        .unwrap()
        .push_back((text, ghost_name));
    });
    // predictスレッドに通知
    let (lock, cvar) = &*self.predict_pauser;
    let mut guard = lock.lock().unwrap();
    *guard = FlowType::Continue;
    cvar.notify_one();
    debug!("pushed and notified to prediction");
  }

  fn push_to_play(&self, data: Vec<u8>) {
    debug!("pushing to play");
    futures::executor::block_on(async {
      self.play_queue.lock().unwrap().push_back(data);
    });
    // playスレッドに通知
    let (lock, cvar) = &*self.play_pauser;
    let mut guard = lock.lock().unwrap();
    *guard = FlowType::Continue;
    cvar.notify_one();
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
