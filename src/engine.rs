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

#[derive(Copy, Clone, Hash, Eq, PartialEq, Serialize)]
pub struct Engine {
  pub port: i32,
  pub name: &'static str,
}

pub const ENGINE_COEIROINKV2: Engine = Engine {
  port: 50032,
  name: "COEIROINKv2",
};
pub const ENGINE_COEIROINKV1: Engine = Engine {
  port: 50031,
  name: "COEIROINKv1",
};
pub const ENGINE_VOICEVOX: Engine = Engine {
  port: 50021,
  name: "VOICEVOX",
};
pub const ENGINE_LMROID: Engine = Engine {
  port: 49973,
  name: "LMROID",
};
pub const ENGINE_SHAREVOX: Engine = Engine {
  port: 50025,
  name: "SHAREVOX",
};
pub const ENGINE_ITVOICE: Engine = Engine {
  port: 49540,
  name: "ITVOICE",
};
pub const ENGINE_BOUYOMICHAN: Engine = Engine {
  port: 50001,
  name: "棒読みちゃん",
};

pub static ENGINE_LIST: Lazy<Vec<Engine>> = Lazy::new(|| {
  vec![
    ENGINE_COEIROINKV2,
    ENGINE_COEIROINKV1,
    ENGINE_VOICEVOX,
    ENGINE_LMROID,
    ENGINE_SHAREVOX,
    ENGINE_ITVOICE,
    ENGINE_BOUYOMICHAN,
  ]
});

pub const DUMMY_VOICE_UUID: &str = "dummy";

pub fn engine_from_port(port: i32) -> Option<Engine> {
  ENGINE_LIST.iter().find(|e| e.port == port).cloned()
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
    ENGINE_COEIROINKV2 => Box::new(CoeiroinkV2SpeakerGetter),
    ENGINE_BOUYOMICHAN => Box::new(BouyomiChanSpeakerGetter),
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
      port: ENGINE_VOICEVOX.port,
      speaker_uuid: DUMMY_VOICE_UUID.to_string(),
      style_id: -1,
    };

    match engine {
      Some(engine) => match engine {
        ENGINE_COEIROINKV2 => {
          // つくよみちゃん-れいせい
          CharacterVoice {
            port: ENGINE_COEIROINKV2.port,
            speaker_uuid: String::from("3c37646f-3881-5374-2a83-149267990abc"),
            style_id: 0,
          }
        }
        ENGINE_COEIROINKV1 => {
          // つくよみちゃん-れいせい
          CharacterVoice {
            port: ENGINE_COEIROINKV1.port,
            speaker_uuid: String::from("3c37646f-3881-5374-2a83-149267990abc"),
            style_id: 0,
          }
        }
        ENGINE_VOICEVOX => {
          // ずんだもん-ノーマル
          CharacterVoice {
            port: ENGINE_VOICEVOX.port,
            speaker_uuid: String::from("388f246b-8c41-4ac1-8e2d-5d79f3ff56d9"),
            style_id: 3,
          }
        }
        ENGINE_BOUYOMICHAN => {
          // 棒読みちゃん-女性1
          CharacterVoice {
            port: ENGINE_BOUYOMICHAN.port,
            speaker_uuid: BOUYOMICHAN_UUID.to_string(),
            style_id: 1,
          }
        }
        _ => dummy_voice,
      },
      None => dummy_voice,
    }
  }
}
