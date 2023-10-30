pub mod coeiroink;
pub mod voicevox;

pub type Engine = i32;
pub const ENGINE_COEIROINK: Engine = 50032;
pub const ENGINE_VOICEVOX: Engine = 50021;

pub fn engine_name(engine: Engine) -> String {
    match engine {
        ENGINE_COEIROINK => "COEIROINK".to_string(),
        ENGINE_VOICEVOX => "VOICEVOX".to_string(),
        _ => "UNKNOWN".to_string(),
    }
}
