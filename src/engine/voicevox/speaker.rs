use async_std::sync::Arc;

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

use crate::engine::ENGINE_VOICEVOX;
use crate::speaker::{SpeakerInfo, Style};
use crate::variables::get_global_vars;

static mut SPEAKER_INFO_GETTER: Lazy<Thread> = Lazy::new(|| Thread::default());

pub struct Thread {
    pub runtime: Option<tokio::runtime::Runtime>,
    pub need_update: Arc<tokio::sync::Notify>,
    pub handler: Option<tokio::task::JoinHandle<()>>,
}

impl Default for Thread {
    fn default() -> Self {
        Thread {
            runtime: Some(tokio::runtime::Runtime::new().unwrap()),
            need_update: Arc::new(tokio::sync::Notify::new()),
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
                        sinfo.insert(ENGINE_VOICEVOX, speakers_info);
                    }
                    Err(e) => {
                        error!("Error: {}", e);
                        sinfo.remove(&ENGINE_VOICEVOX);
                    }
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
        }));
    }

    pub fn need_update(&self) {
        self.need_update.notify_one();
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
