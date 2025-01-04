pub(crate) mod rawvariables;

use crate::engine::{CharacterVoice, Engine};
use crate::speaker::SpeakerInfo;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;
use tokio::sync::RwLock as TokioRwLock;

pub(crate) const PLUGIN_NAME: &str = "GhostSpeaker";
pub(crate) const PLUGIN_UUID: &str = "1e1e0813-f16f-409e-b870-2c36b9084732";
const VAR_PATH: &str = "vars.yaml";

pub(crate) static ENGINE_PATH: Lazy<RwLock<HashMap<Engine, String>>> =
  Lazy::new(|| RwLock::new(HashMap::new()));
pub(crate) static ENGINE_AUTO_START: Lazy<TokioRwLock<HashMap<Engine, bool>>> =
  Lazy::new(|| TokioRwLock::new(HashMap::new()));
pub(crate) static VOLUME: Lazy<RwLock<f32>> = Lazy::new(|| RwLock::new(1.0));
pub(crate) static SPEAK_BY_PUNCTUATION: Lazy<RwLock<bool>> = Lazy::new(|| RwLock::new(true));
pub(crate) static GHOSTS_VOICES: Lazy<RwLock<HashMap<String, GhostVoiceInfo>>> =
  Lazy::new(|| RwLock::new(HashMap::new()));
pub(crate) static INITIAL_VOICE: Lazy<RwLock<CharacterVoice>> =
  Lazy::new(|| RwLock::new(CharacterVoice::no_voice()));
pub(crate) static LAST_VERSION: Lazy<RwLock<String>> = Lazy::new(|| RwLock::new(String::new()));
pub(crate) static DLL_DIR: Lazy<RwLock<String>> = Lazy::new(|| RwLock::new(String::new()));
pub(crate) static SPEAKERS_INFO: Lazy<TokioRwLock<HashMap<Engine, Vec<SpeakerInfo>>>> =
  Lazy::new(|| TokioRwLock::new(HashMap::new()));
pub(crate) static CURRENT_CONNECTION_STATUS: Lazy<TokioRwLock<HashMap<Engine, bool>>> =
  Lazy::new(|| TokioRwLock::new(HashMap::new()));
pub(crate) static LOG_INIT_SUCCESS: Lazy<RwLock<bool>> = Lazy::new(|| RwLock::new(false));

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct GhostVoiceInfo {
  pub devide_by_lines: bool,
  pub voices: Vec<Option<CharacterVoice>>,
}

impl Default for GhostVoiceInfo {
  fn default() -> Self {
    let mut v = Vec::new();
    v.resize(10, None);
    GhostVoiceInfo {
      devide_by_lines: false,
      voices: v,
    }
  }
}

impl GhostVoiceInfo {
  pub fn new(character_count: usize) -> Self {
    let mut v = Vec::new();
    v.resize(character_count, None);
    GhostVoiceInfo {
      devide_by_lines: false,
      voices: v,
    }
  }
}
