use async_std::sync::Arc;
use std::collections::VecDeque;
use tokio::sync::Mutex;
use tokio_condvar::Condvar;

use crate::coeiroink::predict::{get_speaker, predict_text};
use crate::coeiroink::utils::check_connection;

use crate::format::split_dialog;
use crate::player::play_wav;
use crate::variables::get_global_vars;

pub static mut QUEUE: Option<Queue> = None;

pub struct Queue {
    runtime: Option<tokio::runtime::Runtime>,
    predict_queue: Arc<Mutex<VecDeque<PredictArgs>>>,
    predict_handler: Option<tokio::task::JoinHandle<()>>,
    predict_state: Arc<(Mutex<bool>, Condvar)>,
    play_queue: Arc<Mutex<VecDeque<Vec<u8>>>>,
    play_handler: Option<tokio::task::JoinHandle<()>>,
    play_state: Arc<(Mutex<bool>, Condvar)>,
}

pub struct PredictArgs {
    pub text: String,
    pub ghost_name: String,
}

impl Queue {
    pub fn new() -> Self {
        Self {
            runtime: Some(tokio::runtime::Runtime::new().unwrap()),
            predict_queue: Arc::new(Mutex::new(VecDeque::new())),
            predict_handler: None,
            predict_state: Arc::new((Mutex::new(false), Condvar::new())),
            play_queue: Arc::new(Mutex::new(VecDeque::new())),
            play_handler: None,
            play_state: Arc::new((Mutex::new(false), Condvar::new())),
        }
    }

    pub fn init(&mut self) {
        let predict_queue_cln = Arc::clone(&self.predict_queue);
        self.predict_handler = Some(self.runtime.as_mut().unwrap().spawn(async move {
            let mut i = 0;
            loop {
                if predict_queue_cln.lock().await.is_empty() {
                    if i == 10 {
                        debug!("{}", "predict queue pause");
                        i = 0;
                    }
                    i += 1;
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    continue;
                }

                if let Some(args) = predict_queue_cln.lock().await.pop_front() {
                    if let None = get_global_vars().volatility.speakers_info {
                        continue;
                    }
                    if !check_connection().await {
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

        let play_queue_cln = self.play_queue.clone();
        self.play_handler = Some(self.runtime.as_mut().unwrap().spawn(async move {
            let mut i = 0;
            loop {
                if play_queue_cln.lock().await.is_empty() {
                    if i == 10 {
                        debug!("{}", "play queue pause");
                        i = 0;
                    }
                    i += 1;
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    continue;
                }

                if let Some(data) = play_queue_cln.lock().await.pop_front() {
                    debug!("{}", format!("play: {}", data.len()));
                    play_wav(data);
                }
            }
        }));
    }

    pub fn stop(&mut self) {
        debug!("{}", "stopping queue");
        if let Some(handle) = self.predict_handler.take() {
            handle.abort();
        };
        if let Some(handle) = self.play_handler.take() {
            handle.abort();
        };
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
            debug!("{}", "shutdown speaker's runtime");
        }
    }

    pub fn push_to_prediction(&self, args: PredictArgs) {
        debug!("pushing to prediction");
        futures::executor::block_on(async {
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
