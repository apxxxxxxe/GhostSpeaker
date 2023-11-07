use serde::{Deserialize, Serialize};

use crate::speaker::{SpeakerInfo, Style};

#[derive(Debug, Serialize)]
struct SpeakersRequest {
  pub core_version: String,
}

#[derive(Debug, Deserialize)]
struct SpeakerResponse {
  #[serde(rename = "supported_features")]
  pub _supported_features: SupportedFeatures,
  pub name: String,
  pub speaker_uuid: String,
  pub styles: Vec<StyleResponse>,
  #[serde(rename = "version")]
  pub _version: String,
}

impl SpeakerResponse {
  pub fn to_speaker_info(&self) -> SpeakerInfo {
    SpeakerInfo {
      speaker_name: self.name.clone(),
      speaker_uuid: self.speaker_uuid.clone(),
      styles: self.styles.iter().map(|style| style.to_style()).collect(),
    }
  }
}

#[derive(Debug, Deserialize)]
struct SupportedFeatures {
  #[serde(rename = "permitted_synthesis_morphing")]
  _permitted_synthesis_morphing: String,
}

#[derive(Debug, Deserialize)]
struct StyleResponse {
  pub name: Option<String>,
  pub id: Option<i32>,
}

impl StyleResponse {
  pub fn to_style(&self) -> Style {
    Style {
      style_name: self.name.clone(),
      style_id: self.id.clone(),
    }
  }
}

pub async fn get_speakers_info() -> Result<Vec<SpeakerInfo>, reqwest::Error> {
  const DOMAIN: &str = "http://localhost:50021/";
  println!("Requesting speakers info from {}", DOMAIN);

  debug!("getting speakers info");
  let body;
  match reqwest::Client::new()
    .get(format!("{}{}", DOMAIN, "speakers").as_str())
    .header("Content-Type", "application/json")
    .send()
    .await
  {
    Ok(res) => {
      debug!("get_speakers_info success");
      body = res.text().await?;
    }
    Err(e) => {
      println!("Failed to get speakers info: {}", e);
      return Err(e);
    }
  }
  let speakers_responses: Vec<SpeakerResponse> = serde_json::from_str(&body).unwrap();

  let mut speakers_info: Vec<SpeakerInfo> = Vec::new();
  for speaker_response in speakers_responses {
    speakers_info.push(speaker_response.to_speaker_info());
  }

  Ok(speakers_info)
}
