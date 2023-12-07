use crate::engine::Predictor;
use async_trait::async_trait;
use http::StatusCode;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct PredictRequest {
  #[serde(rename = "speakerUuid")]
  pub speaker_uuid: String,

  #[serde(rename = "styleId")]
  pub style_id: i32,

  #[serde(rename = "text")]
  pub text: String,

  #[serde(rename = "prosodyDetail")]
  pub prosody_detail: Option<Vec<Vec<ProsodyDetail>>>,

  #[serde(rename = "speedScale")]
  pub speed_scale: f32,
}

#[derive(Debug, Serialize)]
pub struct ProsodyDetail {
  #[serde(rename = "phoneme")]
  pub phoneme: String,

  #[serde(rename = "hira")]
  pub hira: String,

  #[serde(rename = "accent")]
  pub accent: i32,
}

pub struct CoeiroinkV2Predictor {
  pub text: String,
  pub speaker_uuid: String,
  pub style_id: i32,
}

impl CoeiroinkV2Predictor {
  pub fn new(text: String, speaker_uuid: String, style_id: i32) -> Self {
    Self {
      text,
      speaker_uuid,
      style_id,
    }
  }
}

#[async_trait]
impl Predictor for CoeiroinkV2Predictor {
  async fn predict(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    const URL: &str = "http://localhost:50032/v1/predict";

    let req = PredictRequest {
      speaker_uuid: self.speaker_uuid.clone(),
      style_id: self.style_id,
      text: self.text.clone(),
      prosody_detail: None,
      speed_scale: 1.0,
    };
    let b = serde_json::to_string(&req).unwrap();

    let wav: Vec<u8>;
    match reqwest::Client::new()
      .post(URL)
      .header("Content-Type", "application/json")
      .header("Accept", "audio/wav")
      .body(b)
      .send()
      .await
    {
      Ok(res) => match res.status() {
        StatusCode::OK => {
          wav = res.bytes().await.unwrap().to_vec();
        }
        _ => {
          println!("Error: {:?}", res);
          return Err(res.error_for_status().unwrap_err().into());
        }
      },
      Err(e) => {
        println!("Error: {:?}", e);
        return Err(e.into());
      }
    }

    Ok(wav)
  }
}
