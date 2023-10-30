use async_std::sync::Arc;
use std::collections::VecDeque;
use tokio::sync::{Mutex, Notify};

use crate::coeiroink::predict::{get_speaker, predict_text};
use crate::coeiroink::speaker::get_speaker_getter;
use crate::coeiroink::utils::check_connection;
use crate::player::free_player;

use crate::format::split_dialog;
use crate::player::play_wav;
use crate::variables::{get_global_vars, CharacterVoice};

pub static mut QUEUE: Option<Queue> = None;

pub struct Queue {
    runtime: Option<tokio::runtime::Runtime>,
    predict_queue: Arc<Mutex<VecDeque<PredictArgs>>>,
    predict_notifier: Arc<Notify>,
    predict_handler: Option<tokio::task::JoinHandle<()>>,
    play_queue: Arc<Mutex<VecDeque<Vec<u8>>>>,
    play_notifier: Arc<Notify>,
    play_handler: Option<tokio::task::JoinHandle<()>>,
    stopper: Arc<Mutex<bool>>,
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
            predict_notifier: Arc::new(Notify::new()),
            predict_handler: None,
            play_queue: Arc::new(Mutex::new(VecDeque::new())),
            play_notifier: Arc::new(Notify::new()),
            play_handler: None,
            stopper: Arc::new(Mutex::new(false)),
        }
    }

    pub fn init(&mut self) {
        let predict_queue_cln = Arc::clone(&self.predict_queue);
        let predict_notifier_cln = Arc::clone(&self.predict_notifier);
        let predict_stopper_cln = Arc::clone(&self.stopper);
        self.predict_handler = Some(self.runtime.as_mut().unwrap().spawn(async move {
            loop {
                if *predict_stopper_cln.lock().await {
                    debug!("predict handler break");
                    break;
                }

                if predict_queue_cln.lock().await.is_empty() {
                    predict_notifier_cln.notified().await;
                    continue;
                }

                let parg;
                {
                    let mut guard = predict_queue_cln.lock().await;
                    parg = guard.pop_front();
                }
                if let Some(args) = parg {
                    if let Some(speakers) = get_global_vars().volatility.speakers_info.as_mut() {
                        if !check_connection().await {
                            get_speaker_getter().need_update();
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
                        let speak_by_punctuation = get_global_vars().speak_by_punctuation.unwrap();
                        for dialog in split_dialog(args.text, devide_by_lines, speak_by_punctuation)
                        {
                            if dialog.text.is_empty() {
                                continue;
                            }
                            let mut speaker = get_speaker(args.ghost_name.clone(), dialog.scope);
                            if speakers
                                .iter()
                                .find(|s| s.speaker_uuid == speaker.spekaer_uuid)
                                .is_none()
                            {
                                speaker = CharacterVoice::default();
                            }
                            let result =
                                predict_text(dialog.text, speaker.spekaer_uuid, speaker.style_id)
                                    .await;
                            if let Ok(res) = result {
                                debug!("pushing to play");
                                get_queue().push_to_play(res);
                            } else {
                                debug!("predict failed: {}", result.err().unwrap());
                            }
                        }
                    }
                }
                debug!(
                    "remaining predict queue: {}",
                    predict_queue_cln.lock().await.len()
                );
            }
        }));

        let play_queue_cln = self.play_queue.clone();
        let play_notifier_cln = self.play_notifier.clone();
        let play_stopper_cln = self.stopper.clone();
        self.play_handler = Some(self.runtime.as_mut().unwrap().spawn(async move {
            loop {
                if *play_stopper_cln.lock().await {
                    debug!("play handler break");
                    break;
                }

                if play_queue_cln.lock().await.is_empty() {
                    play_notifier_cln.notified().await;
                    continue;
                }

                let wav;
                {
                    let mut guard = play_queue_cln.lock().await;
                    wav = guard.pop_front();
                }
                if let Some(data) = wav {
                    debug!("{}", format!("play: {}", data.len()));
                    play_wav(data);
                }
                debug!(
                    "remaining play queue: {}",
                    play_queue_cln.lock().await.len()
                );
            }
        }));
    }

    pub fn stop(&mut self) {
        debug!("{}", "stopping queue");
        futures::executor::block_on(async {
            *self.stopper.lock().await = true;
            free_player();
            self.predict_notifier.notify_one();
            self.play_notifier.notify_one();
            let (predict_result, play_result) = futures::join!(
                self.predict_handler.take().unwrap(),
                self.play_handler.take().unwrap()
            );
            if let Err(e) = predict_result {
                error!("error while stopping predict queue: {}", e);
            }
            if let Err(e) = play_result {
                error!("error while stopping play queue: {}", e);
            }
        });
        debug!("{}", "stopped queue");
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
        self.predict_notifier.notify_one();
        debug!("pushed and notified to prediction");
    }

    fn push_to_play(&self, data: Vec<u8>) {
        debug!("pushing to play");
        futures::executor::block_on(async {
            self.play_queue.lock().await.push_back(data);
        });
        self.play_notifier.notify_one();
        debug!("pushed and notified to play");
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
