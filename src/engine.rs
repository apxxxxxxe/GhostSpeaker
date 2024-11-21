pub mod bouyomichan;
pub mod coeiroink_v2;
pub mod voicevox_family;

use crate::speaker::SpeakerInfo;
use async_trait::async_trait;
use bouyomichan::speaker::BouyomiChanSpeakerGetter;
use coeiroink_v2::speaker::CoeiroinkV2SpeakerGetter;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use voicevox_family::speaker::VoicevoxFamilySpeakerGetter;

#[derive(Copy, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum Engine {
  CoeiroInkV2,
  CoeiroInkV1,
  VoiceVox,
  Lmroid,
  ShareVox,
  ItVoice,
  AivisSpeech,
  BouyomiChan,
}

impl Engine {
  pub fn port(&self) -> i32 {
    match self {
      Engine::CoeiroInkV2 => 50032,
      Engine::CoeiroInkV1 => 50031,
      Engine::VoiceVox => 50021,
      Engine::Lmroid => 49973,
      Engine::ShareVox => 50025,
      Engine::ItVoice => 49540,
      Engine::AivisSpeech => 10101,
      Engine::BouyomiChan => 50001,
    }
  }

  pub fn name(&self) -> &'static str {
    match self {
      Engine::CoeiroInkV2 => "COEIROINKv2",
      Engine::CoeiroInkV1 => "COEIROINKv1",
      Engine::VoiceVox => "VOICEVOX",
      Engine::Lmroid => "LMROID",
      Engine::ShareVox => "SHAREVOX",
      Engine::ItVoice => "ITVOICE",
      Engine::AivisSpeech => "AivisSpeech",
      Engine::BouyomiChan => "棒読みちゃん",
    }
  }
}

pub static ENGINE_LIST: Lazy<Vec<Engine>> = Lazy::new(|| {
  vec![
    Engine::CoeiroInkV2,
    Engine::CoeiroInkV1,
    Engine::VoiceVox,
    Engine::Lmroid,
    Engine::ShareVox,
    Engine::ItVoice,
    Engine::AivisSpeech,
    Engine::BouyomiChan,
  ]
});

pub const NO_VOICE_UUID: &str = "dummy";

pub fn engine_from_port(port: i32) -> Option<Engine> {
  ENGINE_LIST.iter().find(|e| e.port() == port).cloned()
}

#[async_trait]
pub trait Predictor {
  async fn predict(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>>;
}

pub fn get_speaker_getters() -> HashMap<Engine, Box<dyn SpeakerGetter + Send + Sync>> {
  let mut map = HashMap::new();
  for engine in ENGINE_LIST.iter() {
    map.insert(*engine, get_speaker_getter(*engine));
  }
  map
}

fn get_speaker_getter(engine: Engine) -> Box<dyn SpeakerGetter + Send + Sync> {
  match engine {
    Engine::CoeiroInkV2 => Box::new(CoeiroinkV2SpeakerGetter),
    Engine::BouyomiChan => Box::new(BouyomiChanSpeakerGetter),
    _ => Box::new(VoicevoxFamilySpeakerGetter { engine }),
  }
}

#[async_trait]
pub trait SpeakerGetter {
  async fn get_speakers_info(&self) -> Result<Vec<SpeakerInfo>, Box<dyn std::error::Error>>;
}

#[derive(Clone, Serialize, Deserialize)]
pub struct CharacterVoice {
  pub port: i32,
  pub speaker_uuid: String,
  pub style_id: i32,
}

impl Default for CharacterVoice {
  fn default() -> Self {
    CharacterVoice::no_voice()
  }
}

impl CharacterVoice {
  pub fn no_voice() -> Self {
    Self {
      port: Engine::VoiceVox.port(),
      speaker_uuid: NO_VOICE_UUID.to_string(),
      style_id: -1,
    }
  }
}
