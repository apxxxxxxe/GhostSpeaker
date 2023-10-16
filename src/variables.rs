use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::speaker::{get_speakers_info, SpeakerInfo};

const VAR_PATH: &str = "vars.json";
static mut GLOBALVARS: Option<GlobalVariables> = None;

#[derive(Serialize, Deserialize)]
pub struct GlobalVariables {
    // 変数を追加した場合はloadの中身も変更することを忘れないように

    // 読み上げ音量
    pub volume: Option<f32>,

    // ゴーストごとの声の情報
    pub ghosts_voices: Option<HashMap<String, Vec<CharacterVoice>>>,

    // 起動ごとにリセットされる変数
    #[serde(skip)]
    pub volatility: VolatilityVariables,
}

impl GlobalVariables {
    pub fn new() -> Self {
        Self {
            volume: Some(1.0),
            ghosts_voices: Some(HashMap::new()),
            volatility: VolatilityVariables::default(),
        }
    }

    pub fn load(&mut self) {
        let json_str = match std::fs::read_to_string(VAR_PATH) {
            Ok(s) => s,
            Err(_) => {
                error!("Failed to load variables.");
                return;
            }
        };

        let vars: GlobalVariables = match serde_json::from_str(&json_str) {
            Ok(v) => v,
            Err(_) => {
                error!("Failed to parse variables.");
                return;
            }
        };

        // TODO: 変数追加時はここに追加することを忘れない
        if let Some(v) = vars.volume {
            self.volume = Some(v);
        };
        if let Some(g) = vars.ghosts_voices {
            self.ghosts_voices = Some(g);
        }
    }

    pub fn save(&self) {
        let json_str = match serde_json::to_string(self) {
            Ok(s) => s,
            Err(_) => {
                error!("Failed to serialize variables");
                return;
            }
        };
        match std::fs::write(VAR_PATH, json_str) {
            Ok(_) => (),
            Err(_) => {
                error!("Failed to save variables");
                return;
            }
        };
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
pub struct CharacterVoice {
    pub spekaer_uuid: String,
    pub style_id: i32,
}

impl Default for CharacterVoice {
    fn default() -> Self {
        // つくよみちゃん-れいせい
        CharacterVoice {
            spekaer_uuid: String::from("3c37646f-3881-5374-2a83-149267990abc"),
            style_id: 0,
        }
    }
}

// ゴーストのグローバル変数のうち、揮発性(起動毎にリセットされる)なもの
pub struct VolatilityVariables {
    pub plugin_uuid: String,

    pub speakers_info: Vec<SpeakerInfo>,
}

impl Default for VolatilityVariables {
    fn default() -> Self {
        Self {
            plugin_uuid: "1e1e0813-f16f-409e-b870-2c36b9084732".to_string(),
            speakers_info: get_speakers_info().unwrap(), // TODO: ちゃんとエラー処理
        }
    }
}
