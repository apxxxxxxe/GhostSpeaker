use crate::engine::{CharacterVoice, Engine, DUMMY_VOICE_UUID};
use crate::speaker::SpeakerInfo;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const VAR_PATH: &str = "vars.yaml";
static mut GLOBALVARS: Option<GlobalVariables> = None;

#[derive(Serialize, Deserialize)]
pub struct GlobalVariables {
  // 変数を追加した場合はloadの中身も変更することを忘れないように

  // エンジンのパス
  pub engine_path: Option<HashMap<Engine, String>>,

  // 各エンジンを起動時に起動するかどうか
  pub engine_auto_start: Option<HashMap<Engine, bool>>,

  // 読み上げ音量
  pub volume: Option<f32>,

  pub speak_by_punctuation: Option<bool>,

  // ゴーストごとの声の情報
  pub ghosts_voices: Option<HashMap<String, GhostVoiceInfo>>,

  // unload時に音声再生の完了を待つかどうか
  pub wait_for_speech: Option<bool>,

  // 初期声質設定
  pub initial_voice: CharacterVoice,

  // 最終起動時のバージョン
  pub last_version: Option<String>,

  // 起動ごとにリセットされる変数
  #[serde(skip)]
  pub volatility: VolatilityVariables,
}

impl GlobalVariables {
  pub fn new() -> Self {
    Self {
      engine_path: Some(HashMap::new()),
      engine_auto_start: Some(HashMap::new()),
      volume: Some(1.0),
      speak_by_punctuation: Some(true),
      ghosts_voices: Some(HashMap::new()),
      wait_for_speech: Some(true),
      initial_voice: CharacterVoice::dummy(),
      volatility: VolatilityVariables::default(),
      last_version: None,
    }
  }

  pub fn load(&mut self) {
    let path = std::path::Path::new(get_global_vars().volatility.dll_dir.as_str()).join(VAR_PATH);
    debug!("Loading variables from {}", path.display());
    let yaml_str = match std::fs::read_to_string(path) {
      Ok(s) => s,
      Err(e) => {
        error!("Failed to load variables. {}", e);
        return;
      }
    };

    let mut vars: GlobalVariables = match serde_yaml::from_str(&yaml_str) {
      Ok(v) => v,
      Err(e) => {
        error!("Failed to parse variables. {}", e);
        return;
      }
    };

    // TODO: 変数追加時はここに追加することを忘れない
    if let Some(p) = vars.engine_path {
      self.engine_path = Some(p);
    };
    if let Some(a) = vars.engine_auto_start {
      self.engine_auto_start = Some(a);
    };
    if let Some(v) = vars.volume {
      self.volume = Some(v);
    };
    if let Some(s) = vars.speak_by_punctuation {
      self.speak_by_punctuation = Some(s);
    };
    if let Some(g) = vars.ghosts_voices {
      self.ghosts_voices = Some(g);
    }
    if let Some(w) = vars.wait_for_speech {
      self.wait_for_speech = Some(w);
    }
    self.initial_voice = vars.initial_voice;

    let mut is_updated = false;
    match vars.last_version {
      Some(v) => {
        if v != env!("CARGO_PKG_VERSION") {
          info!(
            "GhostSpeaker has been updated from {} to {}",
            v,
            env!("CARGO_PKG_VERSION")
          );
          is_updated = true;
          vars.last_version = Some(env!("CARGO_PKG_VERSION").to_string());
        }
      }
      None => {
        info!(
          "GhostSpeaker has been updated to {}",
          env!("CARGO_PKG_VERSION")
        );
        vars.last_version = Some(env!("CARGO_PKG_VERSION").to_string());
        is_updated = true;
      }
    }

    if is_updated {
      get_global_vars().update();
    }

    let path = std::path::Path::new(get_global_vars().volatility.dll_dir.as_str()).join(VAR_PATH);
    debug!("Loaded variables from {}", path.display());
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
    if let Some(ref mut g) = self.ghosts_voices {
      for (_, v) in g.iter_mut() {
        for i in 0..v.voices.len() {
          let vec = &v.voices;
          if let Some(voice) = &vec[i] {
            if voice.speaker_uuid == DUMMY_VOICE_UUID {
              (vec.to_owned())[i] = None;
            }
          }
        }
      }
    }
  }
}

pub fn get_global_vars() -> &'static mut GlobalVariables {
  unsafe {
    if GLOBALVARS.is_none() {
      GLOBALVARS = Some(GlobalVariables::new());
    }
    GLOBALVARS.as_mut().unwrap()
  }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct GhostVoiceInfo {
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

// ゴーストのグローバル変数のうち、揮発性(起動毎にリセットされる)のもの
pub struct VolatilityVariables {
  pub plugin_name: String,

  pub plugin_uuid: String,

  // プラグインのディレクトリ
  pub dll_dir: String,

  pub is_update_checked: bool,

  pub speakers_info: HashMap<Engine, Vec<SpeakerInfo>>,

  pub current_connection_status: HashMap<Engine, bool>,

  pub last_connection_status: HashMap<Engine, bool>,
}

impl Default for VolatilityVariables {
  fn default() -> Self {
    Self {
      plugin_name: "GhostSpeaker".to_string(),
      plugin_uuid: "1e1e0813-f16f-409e-b870-2c36b9084732".to_string(),
      dll_dir: "".to_string(),
      is_update_checked: false,
      speakers_info: HashMap::new(),
      current_connection_status: HashMap::new(),
      last_connection_status: HashMap::new(),
    }
  }
}
