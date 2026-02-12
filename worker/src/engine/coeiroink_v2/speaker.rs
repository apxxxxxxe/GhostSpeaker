use crate::engine::SpeakerGetter;
use async_trait::async_trait;
use ghost_speaker_common::{SpeakerInfo, Style};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct SpeakerResponse {
  #[serde(rename = "speakerName")]
  pub speaker_name: String,

  #[serde(rename = "speakerUuid")]
  pub speaker_uuid: String,

  #[serde(rename = "styles")]
  pub styles: Vec<StyleResponse>,

  #[serde(rename = "version")]
  pub _version: String,

  #[serde(rename = "base64Portrait")]
  pub _base64_portrait: String,
}

impl SpeakerResponse {
  pub fn to_speaker_info(&self) -> SpeakerInfo {
    SpeakerInfo {
      speaker_name: self.speaker_name.clone(),
      speaker_uuid: self.speaker_uuid.clone(),
      styles: self.styles.iter().map(|style| style.to_style()).collect(),
    }
  }
}

#[derive(Debug, Deserialize)]
pub struct StyleResponse {
  #[serde(rename = "styleName")]
  pub style_name: Option<String>,

  #[serde(rename = "styleId")]
  pub style_id: Option<i32>,

  #[serde(rename = "base64Icon")]
  pub _base64_icon: Option<String>,

  #[serde(rename = "base64Portrait")]
  pub _base64_portrait: Option<String>,
}

impl StyleResponse {
  pub fn to_style(&self) -> Style {
    Style {
      style_name: self.style_name.clone(),
      style_id: self.style_id,
    }
  }
}

pub struct CoeiroinkV2SpeakerGetter;

#[async_trait]
impl SpeakerGetter for CoeiroinkV2SpeakerGetter {
  async fn get_speakers_info(
    &self,
  ) -> Result<Vec<SpeakerInfo>, Box<dyn std::error::Error + Send + Sync>> {
    const URL: &str = "http://localhost:50032/v1/speakers";

    log::debug!("getting speakers info");
    let client =
      crate::engine::get_http_client().ok_or_else(|| "HTTP client not initialized".to_string())?;
    let body: String = match client.get(URL).send().await {
      Ok(res) => {
        log::debug!("get_speakers_info success");
        res.text().await?
      }
      Err(e) => {
        log::error!("Failed to get speakers info: {}", e);
        return Err(Box::new(e));
      }
    };
    let speakers_responses: Vec<SpeakerResponse> = serde_json::from_str(&body)?;

    let mut speakers_info: Vec<SpeakerInfo> = Vec::new();
    for speaker_response in speakers_responses {
      speakers_info.push(speaker_response.to_speaker_info());
    }

    Ok(speakers_info)
  }
}
