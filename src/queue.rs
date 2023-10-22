use async_std::sync::Arc;
use std::collections::VecDeque;
use tokio::sync::Mutex;

use crate::coeiroink::predict::{get_speaker, predict_text};

use crate::format::split_dialog;
use crate::player::play_wav;
use crate::variables::get_global_vars;

pub static mut QUEUE: Option<Queue> = None;

pub struct Queue {
    runtime: tokio::runtime::Runtime,
    stopper: Arc<Mutex<bool>>,
    predict_queue: Arc<Mutex<VecDeque<PredictArgs>>>,
    predict_join_handle: Option<tokio::task::JoinHandle<()>>,
    play_queue: Arc<Mutex<VecDeque<Vec<u8>>>>,
    play_join_handle: Option<tokio::task::JoinHandle<()>>,
}

pub struct PredictArgs {
    pub text: String,
    pub ghost_name: String,
}

impl Queue {
    pub fn new() -> Self {
        Self {
            runtime: tokio::runtime::Runtime::new().unwrap(),
            stopper: Arc::new(Mutex::new(false)),
            predict_queue: Arc::new(Mutex::new(VecDeque::new())),
            predict_join_handle: None,
            play_queue: Arc::new(Mutex::new(VecDeque::new())),
            play_join_handle: None,
        }
    }

    pub fn init(&mut self) {
        let predict_queue = self.predict_queue.clone();
        let stopper = self.stopper.clone();
        self.predict_join_handle = Some(self.runtime.spawn(async move {
            loop {
                if *stopper.lock().await {
                    break;
                }

                if predict_queue.lock().await.is_empty() {
                    debug!("{}", "predict queue pause");
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    continue;
                }

                if let Some(args) = predict_queue.lock().await.pop_front() {
                    if let None = get_global_vars().volatility.speakers_info {
                        continue;
                    }
                    debug!("{}", format!("predicting: {}", args.text));
                    let devide_by_lines = get_global_vars()
                        .ghosts_voices
                        .as_ref()
                        .unwrap()
                        .get(&args.ghost_name)
                        .unwrap()
                        .devide_by_lines;
                    for dialog in split_dialog(args.text, devide_by_lines) {
                        if dialog.text.is_empty() {
                            continue;
                        }
                        let speaker = get_speaker(args.ghost_name.clone(), dialog.scope);
                        predict_and_queue(dialog.text, speaker.spekaer_uuid, speaker.style_id)
                            .await;
                    }
                }
            }
        }));

        let play_queue = self.play_queue.clone();
        let stopper = self.stopper.clone();
        self.play_join_handle = Some(self.runtime.spawn(async move {
            loop {
                if *stopper.lock().await {
                    break;
                }

                if play_queue.lock().await.is_empty() {
                    debug!("{}", "play queue pause");
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    continue;
                }

                if let Some(data) = play_queue.lock().await.pop_front() {
                    debug!("{}", format!("play: {}", data.len()));
                    play_wav(data).await;
                }
            }
        }));
    }

    pub async fn stop(&mut self) {
        *self.stopper.lock().await = true;
        futures::future::join_all(vec![
            self.predict_join_handle.take().unwrap(),
            self.play_join_handle.take().unwrap(),
        ])
        .await;
    }

    pub fn push_to_prediction(&self, args: PredictArgs) {
        debug!("pushing to prediction");
        futures::executor::block_on(async {
            debug!("pushing to prediction");
            self.predict_queue.lock().await.push_back(args);
        });
    }

    fn push_to_play(&self, data: Vec<u8>) {
        debug!("pushing to play");
        futures::executor::block_on(async {
            self.play_queue.lock().await.push_back(data);
        });
    }
}

async fn predict_and_queue(text: String, speaker_uuid: String, style_id: i32) {
    let result = predict_text(text, speaker_uuid, style_id).await;
    if let Ok(res) = result {
        get_queue().push_to_play(res.data);
    } else {
        debug!("predict failed: {}", result.err().unwrap());
    }
}

// for singleton
pub fn get_queue() -> &'static mut Queue {
    unsafe {
        if QUEUE.is_none() {
            QUEUE = Some(Queue::new());
            QUEUE.as_mut().unwrap().init();
        }
        QUEUE.as_mut().unwrap()
    }
}
