use async_std::sync::Arc;

use once_cell::sync::Lazy;
use serde::Deserialize;

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
        let need_update = self.need_update.clone();
        self.handler = Some(self.runtime.as_mut().unwrap().spawn(async move {
            loop {
                if get_global_vars().volatility.speakers_info.is_some() {
                    need_update.notified().await;
                    get_global_vars().volatility.speakers_info = None;
                }
                match get_speakers_info().await {
                    Ok(speakers_info) => {
                        get_global_vars().volatility.speakers_info = Some(speakers_info);
                    }
                    Err(e) => {
                        error!("Error: {}", e);
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
        futures::executor::block_on(async {
            if let Some(handler) = self.handler.take() {
                handler.abort();
                debug!("{}", "abort speaker's handler");
            }
        });
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
    let speakers_info: Vec<SpeakerInfo> = serde_json::from_str(&body).unwrap();

    Ok(speakers_info)
}
