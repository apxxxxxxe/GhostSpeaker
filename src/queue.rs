use async_std::sync::Arc;
use std::collections::VecDeque;
use tokio::sync::{Mutex, Notify};

use crate::engine::coeiroink;
use crate::engine::voicevox;
use crate::engine::{ENGINE_COEIROINK, ENGINE_VOICEVOX};
use crate::player::free_player;
use crate::utils::check_connection;

use crate::format::split_dialog;
use crate::player::play_wav;
use crate::variables::{get_global_vars, CharacterVoice, DUMMY_VOICE_UUID};

pub static mut QUEUE: Option<Queue> = None;

pub struct Queue {
    runtime: Option<tokio::runtime::Runtime>,
    predict_queue: Arc<Mutex<VecDeque<PredictArgs>>>,
    predict_notifier: Arc<Notify>,
    predict_handler: Option<tokio::task::JoinHandle<()>>,
    play_queue: Arc<Mutex<VecDeque<Vec<u8>>>>,
    play_notifier: Arc<Notify>,
    play_handler: Option<tokio::task::JoinHandle<()>>,
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
        }
    }

    pub fn init(&mut self) {
        let predict_queue_cln = Arc::clone(&self.predict_queue);
        let predict_notifier_cln = Arc::clone(&self.predict_notifier);
        self.predict_handler = Some(self.runtime.as_mut().unwrap().spawn(async move {
            loop {
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
                    let is_coeiroink_connected = check_connection(ENGINE_COEIROINK).await;
                    let is_voicevox_connected = check_connection(ENGINE_VOICEVOX).await;
                    if !is_coeiroink_connected && !is_voicevox_connected {
                        debug!("no engine connected: skip: {}", args.text);
                        continue;
                    }

                    // エンジン側に声質が存在しない場合、または
                    // descript.txtに記述されていないキャラクターのために
                    // デフォルトの声質を用意する
                    let first_aid_voice;
                    if is_voicevox_connected {
                        first_aid_voice = CharacterVoice::default_voicevox();
                    } else {
                        first_aid_voice = CharacterVoice::default_coeiroink();
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
                    for dialog in split_dialog(args.text, devide_by_lines, speak_by_punctuation) {
                        if dialog.text.is_empty() {
                            continue;
                        }

                        let mut speaker = match get_global_vars()
                            .ghosts_voices
                            .as_ref()
                            .unwrap()
                            .get(&args.ghost_name)
                            .unwrap()
                            .voices
                            .get(dialog.scope)
                        {
                            Some(speaker) => speaker.clone(),
                            None => first_aid_voice.clone(),
                        };

                        if speaker.speaker_uuid == DUMMY_VOICE_UUID {
                            // 無効な声質ならスキップ
                            continue;
                        }
                        if let Some(speakers_by_engine) = get_global_vars()
                            .volatility
                            .speakers_info
                            .get(&speaker.engine)
                        {
                            if speakers_by_engine
                                .iter()
                                .find(|s| s.speaker_uuid == speaker.speaker_uuid)
                                .is_none()
                            {
                                speaker = first_aid_voice.clone();
                            }
                        }
                        let result;
                        match speaker.engine {
                            ENGINE_COEIROINK => {
                                result = coeiroink::predict::predict_text(
                                    dialog.text,
                                    speaker.speaker_uuid,
                                    speaker.style_id,
                                )
                                .await;
                            }
                            ENGINE_VOICEVOX => {
                                result =
                                    voicevox::predict::predict_text(dialog.text, speaker.style_id)
                                        .await;
                            }
                            _ => {
                                error!("predict failed: invalid engine");
                                continue;
                            }
                        }
                        if let Ok(res) = result {
                            debug!("pushing to play");
                            get_queue().push_to_play(res);
                        } else {
                            debug!("predict failed: {}", result.err().unwrap());
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
        self.play_handler = Some(self.runtime.as_mut().unwrap().spawn(async move {
            loop {
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
        free_player();
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
            debug!("{}", "shutdown speaker's runtime");
        }
        debug!("{}", "stopped queue");
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
