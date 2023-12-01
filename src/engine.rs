pub mod coeiroink_v2;
pub mod voicevox_family;

use crate::speaker::SpeakerInfo;
use async_trait::async_trait;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Copy, Clone, Hash, Eq, PartialEq, Serialize)]
pub struct Engine {
  pub port: i32,
  pub name: &'static str,
}

pub const ENGINE_COEIROINKV2: Engine = Engine {
  port: 50032,
  name: "COEIROINKv2",
};
pub const ENGINE_VOICEVOX: Engine = Engine {
  port: 50021,
  name: "VOICEVOX",
};

pub static ENGINE_LIST: Lazy<Vec<Engine>> = Lazy::new(|| vec![ENGINE_COEIROINKV2, ENGINE_VOICEVOX]);

pub const DUMMY_VOICE_UUID: &str = "dummy";

pub fn engine_from_port(port: i32) -> Option<Engine> {
  ENGINE_LIST.iter().find(|e| e.port == port).cloned()
}

pub enum Predictor {
  CoeiroinkV2Predictor(String, String, i32),
  VoiceVoxFamilyPredictor(Engine, String, i32),
}

#[async_trait]
pub trait Predict {
  async fn predict(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>>;
}

#[async_trait]
impl Predict for Predictor {
  async fn predict(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    match self {
      Predictor::CoeiroinkV2Predictor(text, speaker_uuid, style_id) => {
        coeiroink_v2::predict::predict_text(text.clone(), speaker_uuid.clone(), *style_id).await
      }
      Predictor::VoiceVoxFamilyPredictor(port, text, speaker) => {
        voicevox_family::predict::predict_text(*port, text.clone(), speaker.clone()).await
      }
    }
  }
}

pub enum SpeakerGetter {
  CoeiroinkV2SpeakerGetter,
  VoiceVoxFamilySpeakerGetter(Engine),
}

pub fn get_speaker_getters() -> HashMap<Engine, SpeakerGetter> {
  let mut map = HashMap::new();
  for engine in ENGINE_LIST.iter() {
    map.insert(*engine, get_speaker_getter(*engine));
  }
  map
}

fn get_speaker_getter(engine: Engine) -> SpeakerGetter {
  match engine {
    ENGINE_COEIROINKV2 => SpeakerGetter::CoeiroinkV2SpeakerGetter,
    engine => SpeakerGetter::VoiceVoxFamilySpeakerGetter(engine),
  }
}

#[async_trait]
pub trait GetSpeakersInfo {
  async fn get_speakers_info(&self) -> Result<Vec<SpeakerInfo>, Box<dyn std::error::Error>>;
}

#[async_trait]
impl GetSpeakersInfo for SpeakerGetter {
  async fn get_speakers_info(&self) -> Result<Vec<SpeakerInfo>, Box<dyn std::error::Error>> {
    match self {
      SpeakerGetter::CoeiroinkV2SpeakerGetter => coeiroink_v2::speaker::get_speakers_info().await,
      SpeakerGetter::VoiceVoxFamilySpeakerGetter(port) => {
        voicevox_family::speaker::get_speakers_info(*port).await
      }
    }
  }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct CharacterVoice {
  pub port: i32,
  pub speaker_uuid: String,
  pub style_id: i32,
}

impl CharacterVoice {
  pub fn default_coeiroink() -> Self {
    // つくよみちゃん-れいせい
    CharacterVoice {
      port: ENGINE_COEIROINKV2.port,
      speaker_uuid: String::from("3c37646f-3881-5374-2a83-149267990abc"),
      style_id: 0,
    }
  }
  pub fn default_voicevox() -> Self {
    // ずんだもん-ノーマル
    CharacterVoice {
      port: ENGINE_VOICEVOX.port,
      speaker_uuid: String::from("388f246b-8c41-4ac1-8e2d-5d79f3ff56d9"),
      style_id: 3,
    }
  }
}

impl Default for CharacterVoice {
  fn default() -> Self {
    CharacterVoice {
      port: ENGINE_VOICEVOX.port,
      speaker_uuid: DUMMY_VOICE_UUID.to_string(),
      style_id: -1,
    }
  }
}
