use crate::engine::Predictor;
use async_trait::async_trait;
use ghost_speaker_common::Engine;
use http::StatusCode;

pub struct VoicevoxFamilyPredictor {
  pub engine: Engine,
  pub text: String,
  pub speaker: i32,
}

impl VoicevoxFamilyPredictor {
  pub fn new(engine: Engine, text: String, speaker: i32) -> Self {
    Self {
      engine,
      text,
      speaker,
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

    let wav: Vec<u8>;
    match client
      .post(&format!("{}{}", domain, "synthesis"))
      .header("Content-Type", "application/json")
      .header("Accept", "audio/wav")
      .query(&[("speaker", self.speaker.to_string())])
      .body(synthesis_req)
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
