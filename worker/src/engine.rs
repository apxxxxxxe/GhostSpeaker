pub mod bouyomichan;
pub mod coeiroink_v2;
pub mod voicevox_family;

use async_trait::async_trait;
use bouyomichan::speaker::BouyomiChanSpeakerGetter;
use coeiroink_v2::speaker::CoeiroinkV2SpeakerGetter;
use ghost_speaker_common::{Engine, SpeakerInfo, ENGINE_LIST};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::Mutex as StdMutex;
use voicevox_family::speaker::VoicevoxFamilySpeakerGetter;

pub static HTTP_CLIENT: Lazy<StdMutex<Option<reqwest::Client>>> = Lazy::new(|| StdMutex::new(None));

/// HTTP_CLIENT を初期化する
pub fn init_http_client() {
  let client = reqwest::Client::builder()
    .connect_timeout(std::time::Duration::from_secs(5))
    .timeout(std::time::Duration::from_secs(30))
    .build()
    .unwrap_or_else(|e| {
      log::error!("Failed to build HTTP client with custom settings: {}", e);
      reqwest::Client::new()
    });
  if let Ok(mut guard) = HTTP_CLIENT.lock() {
    *guard = Some(client);
  }
}

/// HTTP_CLIENT からクライアントのクローンを取得する
pub fn get_http_client() -> Option<reqwest::Client> {
  HTTP_CLIENT.lock().ok().and_then(|guard| guard.clone())
}

/// HTTP_CLIENT を明示的にドロップする
pub fn shutdown_http_client() {
  if let Ok(mut guard) = HTTP_CLIENT.lock() {
    let _ = guard.take();
  }
}

#[async_trait]
pub trait Predictor {
  async fn predict(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>>;
}

pub struct NoOpPredictor;

#[async_trait]
impl Predictor for NoOpPredictor {
  async fn predict(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    Ok(Vec::new())
  }
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
  async fn get_speakers_info(
    &self,
  ) -> Result<Vec<SpeakerInfo>, Box<dyn std::error::Error + Send + Sync>>;
}
