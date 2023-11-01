pub mod coeiroink;
pub mod voicevox;

use async_trait::async_trait;

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

pub enum Predictor {
    CoeiroinkPredictor(String, String, i32),
    VoiceVoxPredictor(String, i32),
}

#[async_trait]
pub trait Predict {
    async fn predict(&self) -> Result<Vec<u8>, reqwest::Error>;
}

#[async_trait]
impl Predict for Predictor {
    async fn predict(&self) -> Result<Vec<u8>, reqwest::Error> {
        match self {
            Predictor::CoeiroinkPredictor(text, speaker_uuid, style_id) => {
                coeiroink::predict::predict_text(text.clone(), speaker_uuid.clone(), *style_id)
                    .await
            }
            Predictor::VoiceVoxPredictor(text, speaker) => {
                voicevox::predict::predict_text(text.clone(), speaker.clone()).await
            }
        }
    }
}
