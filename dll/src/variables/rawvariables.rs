use crate::variables::{
  ENGINE_AUTO_START, ENGINE_PATH, GHOSTS_VOICES, INITIAL_VOICE, LAST_VERSION, SPEAK_BY_PUNCTUATION,
  VAR_PATH, VOLUME,
};
use ghost_speaker_common::{CharacterVoice, Engine, GhostVoiceInfo, NO_VOICE_UUID};
use log::{debug, error};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

pub(crate) fn copy_from_raw(raw: &RawGlobalVariables) {
  if let Some(mut p) = raw.engine_path.clone() {
    // Remove corrupted paths that point to the worker itself
    p.retain(|engine, path| {
      let is_worker = std::path::Path::new(path)
        .file_name()
        .is_some_and(|name| name == "ghost_speaker_worker.exe");
      if is_worker {
        debug!(
          "Removing corrupted engine path for {}: points to worker executable",
          engine.name()
        );
      }
      !is_worker
    });
    match ENGINE_PATH.write() {
      Ok(mut engine_path) => *engine_path = p,
      Err(e) => error!("Failed to write ENGINE_PATH: {}", e),
    }
  }
  if let Some(a) = raw.engine_auto_start.clone() {
    match ENGINE_AUTO_START.write() {
      Ok(mut engine_auto_start) => *engine_auto_start = a,
      Err(e) => error!("Failed to write ENGINE_AUTO_START: {}", e),
    }
  }
  if let Some(v) = raw.volume {
    match VOLUME.write() {
      Ok(mut volume) => *volume = v,
      Err(e) => error!("Failed to write VOLUME: {}", e),
    }
  }
  if let Some(s) = raw.speak_by_punctuation {
    match SPEAK_BY_PUNCTUATION.write() {
      Ok(mut speak_by_punctuation) => *speak_by_punctuation = s,
      Err(e) => error!("Failed to write SPEAK_BY_PUNCTUATION: {}", e),
    }
  }
  if let Some(gv) = raw.ghosts_voices.clone() {
    match GHOSTS_VOICES.write() {
      Ok(mut ghosts_voices) => *ghosts_voices = gv,
      Err(e) => error!("Failed to write GHOSTS_VOICES: {}", e),
    }
  }
  match INITIAL_VOICE.write() {
    Ok(mut initial_voice) => *initial_voice = raw.initial_voice.clone(),
    Err(e) => error!("Failed to write INITIAL_VOICE: {}", e),
  }
  if let Some(lv) = raw.last_version.clone() {
    match LAST_VERSION.write() {
      Ok(mut last_version) => *last_version = lv,
      Err(e) => error!("Failed to write LAST_VERSION: {}", e),
    }
  }
}

pub(crate) fn save_variables() -> Result<(), Box<dyn std::error::Error>> {
  let engine_auto_start = ENGINE_AUTO_START
    .read()
    .map_err(|e| format!("ENGINE_AUTO_START lock poisoned: {}", e))?
    .clone();
  let raw = RawGlobalVariables {
    engine_path: Some(ENGINE_PATH.read()?.clone()),
    engine_auto_start: Some(engine_auto_start),
    volume: Some(*VOLUME.read()?),
    speak_by_punctuation: Some(*SPEAK_BY_PUNCTUATION.read()?),
    ghosts_voices: Some(GHOSTS_VOICES.read()?.clone()),
    initial_voice: INITIAL_VOICE.read()?.clone(),
    last_version: LAST_VERSION.read()?.clone().into(),
  };
  raw.save();
  Ok(())
}

#[derive(Serialize, Deserialize)]
pub(crate) struct RawGlobalVariables {
  pub engine_path: Option<HashMap<Engine, String>>,
  engine_auto_start: Option<HashMap<Engine, bool>>,
  pub volume: Option<f32>,
  pub speak_by_punctuation: Option<bool>,
  pub ghosts_voices: Option<HashMap<String, GhostVoiceInfo>>,
  #[serde(default)]
  pub initial_voice: CharacterVoice,
  pub last_version: Option<String>,
}

impl RawGlobalVariables {
  pub fn new(dll_dir: &str) -> Self {
    let mut g = Self {
      engine_path: Some(HashMap::new()),
      engine_auto_start: Some(HashMap::new()),
      volume: Some(1.0),
      speak_by_punctuation: Some(true),
      ghosts_voices: Some(HashMap::new()),
      initial_voice: CharacterVoice::no_voice(),
      last_version: None,
    };

    let path = std::path::Path::new(dll_dir).join(VAR_PATH);
    debug!("Loading variables from {}", path.display());
    let yaml_str = match std::fs::read_to_string(path) {
      Ok(s) => s,
      Err(_) => return g,
    };

    let vars: RawGlobalVariables = match serde_yaml::from_str(&yaml_str) {
      Ok(v) => v,
      Err(_) => return g,
    };

    if let Some(p) = vars.engine_path {
      g.engine_path = Some(p);
    };
    if let Some(a) = vars.engine_auto_start {
      g.engine_auto_start = Some(a);
    };
    if let Some(v) = vars.volume {
      g.volume = Some(v);
    };
    if let Some(s) = vars.speak_by_punctuation {
      g.speak_by_punctuation = Some(s);
    };
    if let Some(gv) = vars.ghosts_voices {
      g.ghosts_voices = Some(gv);
    }
    g.initial_voice = vars.initial_voice;

    let last_version = vars.last_version;
    let current_version = env!("CARGO_PKG_VERSION");
    g.last_version = Some(current_version.to_string());

    if current_version.starts_with("1.0.")
      && !last_version.clone().is_some_and(|v| v.starts_with("1.0."))
      || last_version.is_none()
    {
      g.update();
    }

    let path = PathBuf::from(dll_dir).join(VAR_PATH);
    debug!("Loaded variables from {}", path.display());

    g
  }

  pub fn save(&self) {
    let yaml_str = match serde_yaml::to_string(self) {
      Ok(s) => s,
      Err(e) => {
        error!("Failed to serialize variables. {}", e);
        return;
      }
    };
    match std::fs::write(VAR_PATH, yaml_str) {
      Ok(_) => (),
      Err(e) => {
        error!("Failed to save variables. {}", e);
        return;
      }
    };

    debug!("Saved variables");
  }

  fn update(&mut self) {
    debug!("Updating variables");
    if let Some(g) = self.ghosts_voices.as_mut() {
      for (_, v) in g.iter_mut() {
        for voice in v.voices.iter_mut() {
          if let Some(v) = voice {
            if v.speaker_uuid == NO_VOICE_UUID {
              *voice = None;
            }
          }
        }
      }
    }
  }
}
