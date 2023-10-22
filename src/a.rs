use std::sync::{Arc, Mutex};
use std::thread;

use once_cell::sync::Lazy;
use serde::Deserialize;

use crate::variables::get_global_vars;

static mut SPEAKER_INFO_GETTER: Lazy<Thread> = Lazy::new(|| Thread::default());

pub struct Thread {
    pub main_thread: Option<thread::JoinHandle<()>>,
    pub handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl Default for Thread {
    fn default() -> Self {
        Thread {
            main_thread: None,
            handle: Arc::new(Mutex::new(None)),
        }
    }
}

impl Thread {
    pub fn start(&mut self) {
        let handle_clone = self.handle.clone();
        self.main_thread = Some(thread::spawn(move || {
            let handle = tokio::task::spawn(async move {
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
                        thread::sleep(std::time::Duration::from_secs(1));
                    }
                }
            });
            *handle_clone.lock().unwrap() = Some(handle);
            match futures::executor::block_on(handle_clone.lock().unwrap().as_mut().unwrap()) {
                Ok(_) => {
                    info!("Thread stopped successfully");
                }
                Err(e) => {
                    error!("Error: {}", e);
                }
            }
        }));
    }

    pub fn stop(&mut self) {
        if let Some(handle) = self.handle.lock().unwrap().take() {
            handle.abort();
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
