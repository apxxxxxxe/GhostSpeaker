use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::engine::{Engine, ENGINE_COEIROINK, ENGINE_VOICEVOX};
use crate::speaker::SpeakerInfo;

const VAR_PATH: &str = "vars.json";
static mut GLOBALVARS: Option<GlobalVariables> = None;

#[derive(Serialize, Deserialize)]
pub struct GlobalVariables {
    // 変数を追加した場合はloadの中身も変更することを忘れないように

    // エンジンのパス
    pub engine_path: Option<String>,

    // 読み上げ音量
    pub volume: Option<f32>,

    pub speak_by_punctuation: Option<bool>,

    // ゴーストごとの声の情報
    pub ghosts_voices: Option<HashMap<String, GhostVoiceInfo>>,

    // 起動ごとにリセットされる変数
    #[serde(skip)]
    pub volatility: VolatilityVariables,
}

impl GlobalVariables {
    pub fn new() -> Self {
        Self {
            engine_path: None,
            volume: Some(1.0),
            speak_by_punctuation: Some(false),
            ghosts_voices: Some(HashMap::new()),
            volatility: VolatilityVariables::default(),
        }
    }

    pub fn load(&mut self) {
        let path =
            std::path::Path::new(get_global_vars().volatility.dll_dir.as_str()).join(VAR_PATH);
        debug!("Loading variables from {}", path.display());
        let json_str = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to load variables. {}", e);
                return;
            }
        };

        let vars: GlobalVariables = match serde_json::from_str(&json_str) {
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
        if let Some(v) = vars.volume {
            self.volume = Some(v);
        };
        if let Some(s) = vars.speak_by_punctuation {
            self.speak_by_punctuation = Some(s);
        };
        if let Some(g) = vars.ghosts_voices {
            self.ghosts_voices = Some(g);
        }

        let path =
            std::path::Path::new(get_global_vars().volatility.dll_dir.as_str()).join(VAR_PATH);
        debug!("Loaded variables from {}", path.display());
    }

    pub fn save(&self) {
        let json_str = match serde_json::to_string_pretty(self) {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to serialize variables. {}", e);
                return;
            }
        };
        match std::fs::write(VAR_PATH, json_str) {
            Ok(_) => (),
            Err(e) => {
                error!("Failed to save variables. {}", e);
                return;
            }
        };

        debug!("Saved variables");
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
    pub voices: Vec<CharacterVoice>,
}

impl Default for GhostVoiceInfo {
    fn default() -> Self {
        let mut v = Vec::new();
        v.resize(10, CharacterVoice::default_voicevox());
        GhostVoiceInfo {
            devide_by_lines: false,
            voices: v,
        }
    }
}

impl GhostVoiceInfo {
    pub fn new(character_count: usize) -> Self {
        let mut v = Vec::new();
        v.resize(character_count, CharacterVoice::default_voicevox());
        GhostVoiceInfo {
            devide_by_lines: false,
            voices: v,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct CharacterVoice {
    pub engine: Engine,
    pub speaker_uuid: String,
    pub style_id: i32,
}

impl CharacterVoice {
    pub fn default_coeiroink() -> Self {
        // つくよみちゃん-れいせい
        CharacterVoice {
            engine: ENGINE_COEIROINK,
            speaker_uuid: String::from("3c37646f-3881-5374-2a83-149267990abc"),
            style_id: 0,
        }
    }
    pub fn default_voicevox() -> Self {
        // ずんだもん-ノーマル
        CharacterVoice {
            engine: ENGINE_VOICEVOX,
            speaker_uuid: String::from("388f246b-8c41-4ac1-8e2d-5d79f3ff56d9"),
            style_id: 3,
        }
    }
}

// ゴーストのグローバル変数のうち、揮発性(起動毎にリセットされる)のもの
pub struct VolatilityVariables {
    pub plugin_uuid: String,

    // プラグインのディレクトリ
    pub dll_dir: String,

    pub speakers_info: HashMap<Engine, Vec<SpeakerInfo>>,
}

impl Default for VolatilityVariables {
    fn default() -> Self {
        Self {
            plugin_uuid: "1e1e0813-f16f-409e-b870-2c36b9084732".to_string(),
            dll_dir: "".to_string(),
            speakers_info: HashMap::new(),
        }
    }
}

pub fn get_character_voice(ghost_name: String, scope: usize) -> CharacterVoice {
    let info: &GhostVoiceInfo;
    let g = GhostVoiceInfo::default();
    match &get_global_vars()
        .ghosts_voices
        .as_ref()
        .unwrap()
        .get(&ghost_name)
    {
        Some(i) => info = i,
        None => info = &g,
    }

    let default_voice: CharacterVoice;
    if get_global_vars()
        .volatility
        .speakers_info
        .contains_key(&ENGINE_VOICEVOX)
    {
        default_voice = CharacterVoice::default_voicevox();
    } else {
        default_voice = CharacterVoice::default_coeiroink();
    }

    match info.voices.get(scope) {
        Some(voice) => voice.clone(),
        None => default_voice, // descript.txtにないキャラの場合
    }
}
