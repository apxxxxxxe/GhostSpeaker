use crate::engine::Engine;
use crate::engine::Predictor;
use async_trait::async_trait;
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

    let synthesis_req: Vec<u8>;
    match reqwest::Client::new()
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
          synthesis_req = res.bytes().await.unwrap().to_vec();
        }
        _ => {
          println!("Error: {:?}", res);
          return Err(res.status().to_string().into());
        }
      },
      Err(e) => {
        println!("Error: {:?}", e);
        return Err(e.to_string().into());
      }
    }

    let wav: Vec<u8>;
    match reqwest::Client::new()
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
          wav = res.bytes().await.unwrap().to_vec();
        }
        _ => {
          println!("Error: {:?}", res);
          return Err(res.status().to_string().into());
        }
      },
      Err(e) => {
        println!("Error: {:?}", e);
        return Err(e.to_string().into());
      }
    }

    Ok(wav)
  }
}
