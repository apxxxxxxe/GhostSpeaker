use crate::engine::Predictor;
use async_trait::async_trait;
use ghost_speaker_common::{Engine, VoiceQuality};
use http::StatusCode;
use serde_json::json;

pub struct VoicevoxFamilyPredictor {
  pub engine: Engine,
  pub text: String,
  pub speaker: i32,
  pub voice_quality: VoiceQuality,
}

impl VoicevoxFamilyPredictor {
  pub fn new(engine: Engine, text: String, speaker: i32, voice_quality: VoiceQuality) -> Self {
    Self {
      engine,
      text,
      speaker,
      voice_quality,
    }
  }
}

#[async_trait]
impl Predictor for VoicevoxFamilyPredictor {
  async fn predict(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let domain: String = format!("http://localhost:{}/", self.engine.port());

    let client =
      crate::engine::get_http_client().ok_or_else(|| "HTTP client not initialized".to_string())?;

    let synthesis_req: Vec<u8>;
    match client
      .post(&format!("{}{}", domain, "audio_query"))
      .query(&[
        ("speaker", self.speaker.to_string()),
        ("text", self.text.clone()),
      ])
      .send()
      .await
    {
      Ok(res) => match res.status() {
        StatusCode::OK => {
          synthesis_req = match res.bytes().await {
            Ok(bytes) => bytes.to_vec(),
            Err(e) => {
              log::error!("Failed to read audio_query response bytes: {}", e);
              return Err(Box::new(e));
            }
          };
        }
        _ => {
          log::error!("Error: {:?}", res);
          return Err(res.status().to_string().into());
        }
      },
      Err(e) => {
        log::error!("Error: {:?}", e);
        return Err(e.to_string().into());
      }
    }

    let synthesis_body = {
      let mut query: serde_json::Value = serde_json::from_slice(&synthesis_req)
        .map_err(|e| format!("Failed to parse audio_query JSON: {}", e))?;
      if let Some(obj) = query.as_object_mut() {
        obj.insert("speedScale".into(), json!(self.voice_quality.speed_scale));
        obj.insert("pitchScale".into(), json!(self.voice_quality.pitch_scale));
        obj.insert(
          "intonationScale".into(),
          json!(self.voice_quality.intonation_scale),
        );
      }
      serde_json::to_vec(&query)
        .map_err(|e| format!("Failed to serialize modified audio_query: {}", e))?
    };

    let wav: Vec<u8>;
    match client
      .post(&format!("{}{}", domain, "synthesis"))
      .header("Content-Type", "application/json")
      .header("Accept", "audio/wav")
      .query(&[("speaker", self.speaker.to_string())])
      .body(synthesis_body)
      .send()
      .await
    {
      Ok(res) => match res.status() {
        StatusCode::OK => {
          wav = match res.bytes().await {
            Ok(bytes) => bytes.to_vec(),
            Err(e) => {
              log::error!("Failed to read synthesis response bytes: {}", e);
              return Err(Box::new(e));
            }
          };
        }
        _ => {
          log::error!("Error: {:?}", res);
          return Err(res.status().to_string().into());
        }
      },
      Err(e) => {
        log::error!("Error: {:?}", e);
        return Err(e.to_string().into());
      }
    }

    Ok(wav)
  }
}
