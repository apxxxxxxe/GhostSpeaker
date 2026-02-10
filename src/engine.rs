pub(crate) mod bouyomichan;
pub(crate) mod coeiroink_v2;
pub(crate) mod voicevox_family;

use crate::speaker::SpeakerInfo;
use async_trait::async_trait;
use bouyomichan::speaker::BouyomiChanSpeakerGetter;
use coeiroink_v2::speaker::CoeiroinkV2SpeakerGetter;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex as StdMutex;
use voicevox_family::speaker::VoicevoxFamilySpeakerGetter;

pub(crate) static HTTP_CLIENT: Lazy<StdMutex<Option<reqwest::Client>>> =
  Lazy::new(|| StdMutex::new(None));

/// HTTP_CLIENT を初期化する
pub(crate) fn init_http_client() {
  let client = reqwest::Client::builder()
    .connect_timeout(std::time::Duration::from_secs(5))
    .timeout(std::time::Duration::from_secs(30))
    .build()
    .unwrap_or_else(|e| {
      error!("Failed to build HTTP client with custom settings: {}", e);
      reqwest::Client::new()
    });
  if let Ok(mut guard) = HTTP_CLIENT.lock() {
    *guard = Some(client);
  }
}

/// HTTP_CLIENT からクライアントのクローンを取得する
pub(crate) fn get_http_client() -> Option<reqwest::Client> {
  HTTP_CLIENT.lock().ok().and_then(|guard| guard.clone())
}

/// HTTP_CLIENT を明示的にドロップする（DLLアンロード時に呼ぶ）
pub(crate) fn shutdown_http_client() {
  if let Ok(mut guard) = HTTP_CLIENT.lock() {
    let _ = guard.take();
  }
}

#[derive(Copy, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) enum Engine {
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

pub(crate) static ENGINE_LIST: Lazy<Vec<Engine>> = Lazy::new(|| {
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

pub(crate) const NO_VOICE_UUID: &str = "dummy";

pub(crate) fn engine_from_port(port: i32) -> Option<Engine> {
  ENGINE_LIST.iter().find(|e| e.port() == port).cloned()
}

#[async_trait]
pub(crate) trait Predictor {
  async fn predict(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>>;
}

pub(crate) struct NoOpPredictor;

#[async_trait]
impl Predictor for NoOpPredictor {
  async fn predict(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    Ok(Vec::new())
  }
}

pub(crate) fn get_speaker_getters() -> HashMap<Engine, Box<dyn SpeakerGetter + Send + Sync>> {
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
pub(crate) trait SpeakerGetter {
  async fn get_speakers_info(
    &self,
  ) -> Result<Vec<SpeakerInfo>, Box<dyn std::error::Error + Send + Sync>>;
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct CharacterVoice {
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
