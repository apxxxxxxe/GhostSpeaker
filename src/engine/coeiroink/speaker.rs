use once_cell::sync::Lazy;
use serde::Deserialize;

use crate::engine::ENGINE_COEIROINK;
use crate::speaker::{SpeakerInfo, Style};
use crate::variables::get_global_vars;

static mut SPEAKER_INFO_GETTER: Lazy<Thread> = Lazy::new(|| Thread::default());

pub struct Thread {
    pub runtime: Option<tokio::runtime::Runtime>,
    pub handler: Option<tokio::task::JoinHandle<()>>,
}

impl Default for Thread {
    fn default() -> Self {
        Thread {
            runtime: Some(tokio::runtime::Runtime::new().unwrap()),
            handler: None,
        }
    }
}

impl Thread {
    pub fn start(&mut self) {
        self.handler = Some(self.runtime.as_mut().unwrap().spawn(async move {
            loop {
                let sinfo = &mut get_global_vars().volatility.speakers_info;
                match get_speakers_info().await {
                    Ok(speakers_info) => {
                        sinfo.insert(ENGINE_COEIROINK, speakers_info);
                    }
                    Err(e) => {
                        error!("Error: {}", e);
                        sinfo.remove(&ENGINE_COEIROINK);
                    }
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
        }));
    }

    pub fn stop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
            debug!("{}", "shutdown speaker's runtime");
        }
    }
}

pub fn get_speaker_getter() -> &'static mut Thread {
    unsafe { &mut SPEAKER_INFO_GETTER }
}

#[derive(Debug, Deserialize)]
pub struct SpeakerResponse {
    #[serde(rename = "speakerName")]
    pub speaker_name: String,

    #[serde(rename = "speakerUuid")]
    pub speaker_uuid: String,

    #[serde(rename = "styles")]
    pub styles: Vec<StyleResponse>,

    #[serde(rename = "version")]
    pub version: String,

    #[serde(rename = "base64Portrait")]
    pub base64_portrait: String,
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
    pub base64_icon: Option<String>,

    #[serde(rename = "base64Portrait")]
    pub base64_portrait: Option<String>,
}

impl StyleResponse {
    pub fn to_style(&self) -> Style {
        Style {
            style_name: self.style_name.clone(),
            style_id: self.style_id.clone(),
        }
    }
}

pub async fn get_speakers_info() -> Result<Vec<SpeakerInfo>, reqwest::Error> {
    const URL: &str = "http://localhost:50032/v1/speakers";
    println!("Requesting speakers info from {}", URL);

    debug!("getting speakers info");
    let body;
    match reqwest::Client::new().get(URL).send().await {
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
