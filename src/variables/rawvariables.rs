use crate::engine::{CharacterVoice, Engine, NO_VOICE_UUID};
use crate::variables::{
  GhostVoiceInfo, ENGINE_AUTO_START, ENGINE_PATH, GHOSTS_VOICES, INITIAL_VOICE, LAST_VERSION,
  SPEAK_BY_PUNCTUATION, VAR_PATH, VOLUME,
};
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub(crate) fn copy_from_raw(raw: &RawGlobalVariables) {
  // *self.engine_path.write().unwrap() = raw.engine_path.clone().unwrap_or_default();
  // *self.engine_auto_start.write().unwrap() = raw.engine_auto_start.clone().unwrap_or_default();
  // *self.volume.write().unwrap() = raw.volume.unwrap_or(DEFAULT_VOLUME);
  // *self.speak_by_punctuation.write().unwrap() = raw
  //   .speak_by_punctuation
  //   .unwrap_or(DEFAULT_SPEAK_BY_PUNCTUATION);
  // *self.ghosts_voices.write().unwrap() = raw.ghosts_voices.clone().unwrap_or_default();
  // *self.wait_for_speech.write().unwrap() = raw.wait_for_speech.unwrap_or(DEFAULT_WAIT_FOR_SPEECH);
  // *self.initial_voice.write().unwrap() = raw.initial_voice.clone();
  // *self.last_version.write().unwrap() = raw.last_version.clone();
  if let Some(p) = raw.engine_path.clone() {
    *ENGINE_PATH.write().unwrap() = p;
  }
  if let Some(a) = raw.engine_auto_start.clone() {
    futures::executor::block_on(async {
      *ENGINE_AUTO_START.write().await = a;
    });
  }
  if let Some(v) = raw.volume {
    *VOLUME.write().unwrap() = v;
  }
  if let Some(s) = raw.speak_by_punctuation {
    *SPEAK_BY_PUNCTUATION.write().unwrap() = s;
  }
  if let Some(gv) = raw.ghosts_voices.clone() {
    *GHOSTS_VOICES.write().unwrap() = gv;
  }
  *INITIAL_VOICE.write().unwrap() = raw.initial_voice.clone();
  if let Some(lv) = raw.last_version.clone() {
    *LAST_VERSION.write().unwrap() = lv;
  }
}

pub(crate) fn save_variables() -> Result<(), Box<dyn std::error::Error>> {
  // RawGlobalVariablesに変換
  let engine_auto_start =
    futures::executor::block_on(async { ENGINE_AUTO_START.read().await.clone() });
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

// yamlファイルからパースする際の構造体
#[derive(Serialize, Deserialize)]
pub(crate) struct RawGlobalVariables {
  // 変数を追加した場合はloadの中身も変更することを忘れないように

  // エンジンのパス
  pub engine_path: Option<HashMap<Engine, String>>,

  // 各エンジンを起動時に起動するかどうか
  engine_auto_start: Option<HashMap<Engine, bool>>,

  // 読み上げ音量
  pub volume: Option<f32>,

  pub speak_by_punctuation: Option<bool>,

  // ゴーストごとの声の情報
  pub ghosts_voices: Option<HashMap<String, GhostVoiceInfo>>,

  // 初期声質設定
  #[serde(default)]
  pub initial_voice: CharacterVoice,

  // 最終起動時のバージョン
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

    // TODO: 変数追加時はここに追加することを忘れない
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

  // 互換性のための処理: dummy voiceをNoneに変換する
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
