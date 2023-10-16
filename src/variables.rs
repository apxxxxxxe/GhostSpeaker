use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::speaker::{get_speakers_info, SpeakerInfo};

const VAR_PATH: &str = "vars.json";
static mut GLOBALVARS: Option<GlobalVariables> = None;

#[derive(Serialize, Deserialize)]
pub struct GlobalVariables {
    // ゴーストの累計起動時間(秒数)
    pub total_time: Option<u64>,

    // ランダムトークの間隔(秒数)
    pub random_talk_interval: Option<u64>,

    // ユーザ名
    pub user_name: Option<String>,

    // ゴーストごとの声の情報
    pub ghosts_voices: HashMap<String, Vec<CharacterVoice>>,

    // 起動ごとにリセットされる変数
    #[serde(skip)]
    pub volatility: VolatilityVariables,
}

impl GlobalVariables {
    pub fn new() -> Self {
        let mut s = Self {
            total_time: Some(0),
            random_talk_interval: Some(180),
            user_name: Some("ユーザ".to_string()),
            ghosts_voices: HashMap::new(),
            volatility: VolatilityVariables::default(),
        };

        s
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
        if let Some(t) = vars.total_time {
            self.total_time = Some(t);
        }
        if let Some(t) = vars.random_talk_interval {
            self.random_talk_interval = Some(t);
        }
        if let Some(t) = vars.user_name {
            self.user_name = Some(t);
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

#[derive(PartialEq)]
pub enum Direction {
    Up,
    Down,
}

impl Direction {
    pub fn to_str(&self) -> &str {
        match self {
            Direction::Up => "up",
            Direction::Down => "down",
        }
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

    // ゴーストが起動してからの秒数
    pub ghost_up_time: u64,

    // ゴーストの起動日時
    pub ghost_boot_time: SystemTime,

    pub nade_counter: i32,

    pub last_nade_count_unixtime: SystemTime,

    pub last_nade_part: String,

    pub wheel_direction: Direction,

    pub wheel_counter: i32,

    pub last_wheel_count_unixtime: SystemTime,

    pub last_wheel_part: String,

    pub first_sexial_touch: bool,

    pub touch_count: i32,

    pub last_touch_info: String,

    pub idle_seconds: i32,

    pub idle_threshold: i32,

    pub speakers_info: Vec<SpeakerInfo>,
}

impl Default for VolatilityVariables {
    fn default() -> Self {
        Self {
            plugin_uuid: "1e1e0813-f16f-409e-b870-2c36b9084732".to_string(),
            ghost_up_time: 0,
            ghost_boot_time: SystemTime::now(),
            nade_counter: 0,
            last_nade_count_unixtime: UNIX_EPOCH,
            last_nade_part: "".to_string(),
            wheel_direction: Direction::Up,
            wheel_counter: 0,
            last_wheel_count_unixtime: UNIX_EPOCH,
            last_wheel_part: "".to_string(),
            first_sexial_touch: false,
            touch_count: 0,
            last_touch_info: "".to_string(),
            idle_seconds: 0,
            idle_threshold: 60 * 5,
            speakers_info: get_speakers_info().unwrap(), // TODO: ちゃんとエラー処理
        }
    }
}
