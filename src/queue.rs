use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::player::play_wav;
use crate::predict::predict_text;

// なんだかこんがらがっている
// playとpredictを分ける必要はないのでは？
// predict_and_playをasync fnとして実装すればいい
// asyncである必要すらないかも

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
    pub speaker_uuid: String,
    pub style_id: i32,
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
        self.predict_join_handle = Some(thread::spawn(move || loop {
            if *thread_stopper_a.lock().unwrap() {
                break;
            }
            let mut guard = predict_queue.lock().unwrap();
            let args = guard.pop_front();
            drop(guard);
            if let Some(args) = args {
                println!("{}", format!("predict_and_play: {}", args.text));
                predict_and_queue(args);
            } else {
                thread::sleep(Duration::from_millis(100));
            }
        }));

        let play_queue = self.play_queue.clone();
        let thread_stopper_b = self.thread_stopper.clone();
        self.play_join_handle = Some(thread::spawn(move || loop {
            if *thread_stopper_b.lock().unwrap() {
                break;
            }
            let mut guard = play_queue.lock().unwrap();
            let data = guard.pop_front();
            drop(guard);
            if let Some(data) = data {
                println!("{}", format!("play: {}", data.len()));
                play_wav(data);
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

fn predict_and_queue(args: PredictArgs) {
    let PredictArgs {
        text,
        speaker_uuid,
        style_id,
    } = args;
    let result = predict_text(String::from(&text), String::from(&speaker_uuid), style_id);
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
