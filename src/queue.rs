use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::coeiroink::predict::{get_speaker, predict_text};
use crate::coeiroink::speaker::get_speakers_info;
use crate::coeiroink::utils::check_connection;
use crate::player::play_wav;
use crate::variables::get_global_vars;

pub static mut QUEUE: Option<Queue> = None;

pub struct Queue {
    predict_queue: Arc<Mutex<VecDeque<PredictArgs>>>,
    predict_join_handle: Option<thread::JoinHandle<()>>,
    play_queue: Arc<Mutex<VecDeque<Vec<u8>>>>,
    play_join_handle: Option<thread::JoinHandle<()>>,
    thread_stopper: Arc<Mutex<bool>>,
}

pub struct PredictArgs {
    pub text: String,
    pub ghost_name: String,
    pub scope: usize,
}

impl Queue {
    pub fn new() -> Self {
        Self {
            predict_queue: Arc::new(Mutex::new(VecDeque::new())),
            predict_join_handle: None,
            play_queue: Arc::new(Mutex::new(VecDeque::new())),
            play_join_handle: None,
            thread_stopper: Arc::new(Mutex::new(false)),
        }
    }

    pub fn init(&mut self) {
        let predict_queue = self.predict_queue.clone();
        let thread_stopper_a = self.thread_stopper.clone();
        self.predict_join_handle = Some(thread::spawn(move || {
            let mut i = 0;
            loop {
                if *thread_stopper_a.lock().unwrap() {
                    break;
                }
                if !check_connection() || predict_queue.lock().unwrap().len() == 0 {
                    thread::sleep(Duration::from_secs(1));
                    continue;
                } else {
                    i += 1;
                    if i % 10 == 0 {
                        thread::sleep(Duration::from_secs(1));
                    }
                }
                let args = {
                    let mut guard = predict_queue.lock().unwrap();
                    guard.pop_front()
                };
                if let Some(args) = args {
                    if let None = get_global_vars().volatility.speakers_info {
                        // 上で接続は確認しているのでunwrapでok
                        get_global_vars().volatility.speakers_info =
                            Some(get_speakers_info().unwrap());
                    }
                    println!("{}", format!("predict_and_play: {}", args.text));
                    let speaker = get_speaker(args.ghost_name, args.scope);
                    predict_and_queue(args.text, speaker.spekaer_uuid, speaker.style_id);
                } else {
                    thread::sleep(Duration::from_millis(100));
                }
            }
        }));

        let play_queue = self.play_queue.clone();
        let thread_stopper_b = self.thread_stopper.clone();
        let thread_stopper_c = self.thread_stopper.clone();
        self.play_join_handle = Some(thread::spawn(move || loop {
            if *thread_stopper_b.lock().unwrap() {
                break;
            }
            let data = {
                let mut guard = play_queue.lock().unwrap();
                guard.pop_front()
            };
            if let Some(data) = data {
                println!("{}", format!("play: {}", data.len()));
                play_wav(data, &thread_stopper_c);
            } else {
                thread::sleep(Duration::from_millis(100));
            }
        }));
    }

    pub fn stop(&mut self) {
        *self.thread_stopper.lock().unwrap() = true;
        if let Some(handle) = self.predict_join_handle.take() {
            handle.join().unwrap();
        }
        if let Some(handle) = self.play_join_handle.take() {
            handle.join().unwrap();
        }
    }

    pub fn push_to_prediction(&self, args: PredictArgs) {
        self.predict_queue.lock().unwrap().push_back(args);
    }

    fn push_to_play(&self, data: Vec<u8>) {
        self.play_queue.lock().unwrap().push_back(data);
    }
}

fn predict_and_queue(text: String, speaker_uuid: String, style_id: i32) {
    let result = predict_text(text, speaker_uuid, style_id);
    if let Ok(result) = result {
        get_queue().push_to_play(result.data);
    } else {
        println!("predict failed: {}", result.err().unwrap());
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
