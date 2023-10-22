use once_cell::sync::Lazy;
use serde::Deserialize;
use tokio::select;

use async_std::sync::Arc;
use tokio::sync::Mutex;

use crate::variables::get_global_vars;

static mut SPEAKER_INFO_GETTER: Lazy<Thread> = Lazy::new(|| Thread::default());

pub struct Thread {
    pub runtime: tokio::runtime::Runtime,
    pub stopper: Arc<Mutex<bool>>,
    pub handle: Option<tokio::task::JoinHandle<()>>,
}

impl Default for Thread {
    fn default() -> Self {
        Thread {
            runtime: tokio::runtime::Runtime::new().unwrap(),
            stopper: Arc::new(Mutex::new(false)),
            handle: None,
        }
    }
}

impl Thread {
    pub fn start(&mut self) {
        let stopper = self.stopper.clone();
        self.handle = Some(self.runtime.spawn(async move {
            loop {
                if get_global_vars().volatility.speakers_info.is_some() {
                    break;
                } else {
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
                if *stopper.lock().await {
                    break;
                }
            }
        }));
    }

    pub async fn stop(&mut self) {
        *self.stopper.lock().await = true;
        futures::join!(self.handle.take().unwrap()).0.unwrap();
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
