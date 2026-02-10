use crate::engine::SpeakerGetter;
use async_trait::async_trait;
use ghost_speaker_common::{Engine, SpeakerInfo, Style};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct SpeakerResponse {
  #[serde(rename = "supported_features")]
  pub _supported_features: Option<SupportedFeatures>,
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
      style_id: self.id,
    }
  }
}

pub struct VoicevoxFamilySpeakerGetter {
  pub engine: Engine,
}

#[async_trait]
impl SpeakerGetter for VoicevoxFamilySpeakerGetter {
  async fn get_speakers_info(
    &self,
  ) -> Result<Vec<SpeakerInfo>, Box<dyn std::error::Error + Send + Sync>> {
    let domain: String = format!("http://localhost:{}/", self.engine.port());

    log::debug!("getting speakers info");
    let client =
      crate::engine::get_http_client().ok_or_else(|| "HTTP client not initialized".to_string())?;
    let body = client
      .get(format!("{}{}", domain, "speakers").as_str())
      .header("Content-Type", "application/json")
      .send()
      .await?
      .text()
      .await?;
    let speakers_responses: Vec<SpeakerResponse> = serde_json::from_str(&body)?;

    let mut speakers_info: Vec<SpeakerInfo> = Vec::new();
    for speaker_response in speakers_responses {
      speakers_info.push(speaker_response.to_speaker_info());
    }

    Ok(speakers_info)
  }
}
