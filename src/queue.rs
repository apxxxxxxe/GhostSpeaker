use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::thread;

use crate::player::play_wav;
use crate::predict::predict_text;

// なんだかこんがらがっている
// playとpredictを分ける必要はないのでは？
// predict_and_playをasync fnとして実装すればいい
// asyncである必要すらないかも

pub static mut QUEUE: Option<Queue> = None;

pub struct Queue {
    queue: Arc<Mutex<VecDeque<PredictArgs>>>,
    join_handle: Option<thread::JoinHandle<()>>,
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
            queue: Arc::new(Mutex::new(VecDeque::new())),
            join_handle: None,
            thread_stopper: Arc::new(Mutex::new(false)),
        }
    }

    pub fn init(&mut self) {
        let queue = self.queue.clone();
        let thread_stopper = self.thread_stopper.clone();
        self.join_handle = Some(thread::spawn(move || loop {
            if *thread_stopper.lock().unwrap() {
                break;
            }
            if let Some(args) = queue.lock().unwrap().pop_front() {
                println!("{}", format!("predict_and_play: {}", args.text));
                predict_and_play(args);
            }
        }));
    }

    pub fn stop(&mut self) {
        *self.thread_stopper.lock().unwrap() = true;
        if let Some(handle) = self.join_handle.take() {
            handle.join().unwrap();
        }
    }

    pub fn push(&self, args: PredictArgs) {
        self.queue.lock().unwrap().push_back(args);
    }
}

fn predict_and_play(args: PredictArgs) {
    let PredictArgs {
        text,
        speaker_uuid,
        style_id,
    } = args;
    let result = predict_text(String::from(&text), String::from(&speaker_uuid), style_id);
    if let Ok(result) = result {
        play_wav(result.data);
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
