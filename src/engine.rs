pub mod bouyomichan;
pub mod coeiroink_v2;
pub mod voicevox_family;

use crate::speaker::SpeakerInfo;
use async_trait::async_trait;
use bouyomichan::speaker::{BouyomiChanSpeakerGetter, BOUYOMICHAN_UUID};
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
    Engine::BouyomiChan,
  ]
});

pub const DUMMY_VOICE_UUID: &str = "dummy";

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
    engine => Box::new(VoicevoxFamilySpeakerGetter { engine }),
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

impl CharacterVoice {
  pub fn default(engine: Option<Engine>) -> Self {
    let dummy_voice = CharacterVoice {
      port: Engine::VoiceVox.port(),
      speaker_uuid: DUMMY_VOICE_UUID.to_string(),
      style_id: -1,
    };

    match engine {
      Some(engine) => match engine {
        Engine::CoeiroInkV2 => {
          // つくよみちゃん-れいせい
          CharacterVoice {
            port: Engine::CoeiroInkV2.port(),
            speaker_uuid: String::from("3c37646f-3881-5374-2a83-149267990abc"),
            style_id: 0,
          }
        }
        Engine::CoeiroInkV1 => {
          // つくよみちゃん-れいせい
          CharacterVoice {
            port: Engine::CoeiroInkV1.port(),
            speaker_uuid: String::from("3c37646f-3881-5374-2a83-149267990abc"),
            style_id: 0,
          }
        }
        Engine::VoiceVox => {
          // ずんだもん-ノーマル
          CharacterVoice {
            port: Engine::VoiceVox.port(),
            speaker_uuid: String::from("388f246b-8c41-4ac1-8e2d-5d79f3ff56d9"),
            style_id: 3,
          }
        }
        Engine::BouyomiChan => {
          // 棒読みちゃん-女性1
          CharacterVoice {
            port: Engine::BouyomiChan.port(),
            speaker_uuid: BOUYOMICHAN_UUID.to_string(),
            style_id: 1,
          }
        }
        _ => dummy_voice, // TODO: 他のエンジンのデフォルト声質
      },
      None => dummy_voice,
    }
  }
}
