use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct SpeakerInfo {
    #[serde(rename = "speakerName")]
    pub speaker_name: String,

    #[serde(rename = "speakerUuid")]
    pub speaker_uuid: String,

    #[serde(rename = "styles")]
    pub styles: Vec<Style>,

    #[serde(rename = "version")]
    pub version: String,

    #[serde(rename = "base64Portrait")]
    pub base64_portrait: String,
}

#[derive(Debug, Deserialize)]
pub struct Style {
    #[serde(rename = "styleName")]
    pub style_name: Option<String>,

    #[serde(rename = "styleId")]
    pub style_id: Option<i32>,

    #[serde(rename = "base64Icon")]
    pub base64_icon: Option<String>,

    #[serde(rename = "base64Portrait")]
    pub base64_portrait: Option<String>,
}

pub fn get_speakers_info() -> Result<Vec<SpeakerInfo>, serde_json::Error> {
    const URL: &str = "http://localhost:50032/v1/speakers";

    let body = reqwest::blocking::Client::new()
        .get(URL)
        .header("Content-Type", "application/json")
        .send()
        .unwrap()
        .text()
        .unwrap();

    serde_json::from_str(&body)
}
