use once_cell::sync::Lazy;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::player::{play_wav, Wave};
use crate::predict::predict_text;

static mut PREDICT_QUEUE: Lazy<PredictQueue> = Lazy::new(|| PredictQueue::new());
static mut PLAY_QUEUE: Lazy<PlayQueue> = Lazy::new(|| PlayQueue::new());

struct PlayQueue {
    queue: Arc<Mutex<VecDeque<Vec<u8>>>>,
    join_handle: Arc<Mutex<Option<task::JoinHandle<()>>>>,
    thread_stopper: Arc<Mutex<bool>>,
}

impl PlayQueue {
    pub fn new() -> Self {
        Self {
            queue: Arc::new(Mutex::new(VecDeque::new())),
            join_handle: Arc::new(Mutex::new(None)),
            thread_stopper: Arc::new(Mutex::new(false)),
        }
    }
}

pub fn push_to_play_queue(data: Vec<u8>) {
    unsafe {
        PLAY_QUEUE.queue.lock().unwrap().push_back(data);
    }
}

struct PredictQueue {
    queue: Arc<Mutex<VecDeque<task::JoinHandle<()>>>>,
    thread_stopper: Arc<Mutex<bool>>,
}

impl PredictQueue {
    pub fn new() -> Self {
        Self {
            queue: Arc::new(Mutex::new(VecDeque::new())),
            thread_stopper: Arc::new(Mutex::new(false)),
        }
    }
}

pub struct PredictArgs {
    pub text: String,
    pub speaker_uuid: String,
    pub style_id: i32,
}

pub fn push_to_predict_queue(data: PredictArgs) {
    let predict_queue = unsafe { PREDICT_QUEUE.queue.clone() };
    let play_queue = unsafe { PLAY_QUEUE.queue.clone() };

    let predict_handle = task::spawn(async move {
        play_queue.lock().unwrap().push_back(
            predict_text(data.text, data.speaker_uuid, data.style_id)
                .unwrap()
                .data,
        );
    });
    predict_queue.lock().unwrap().push_back(predict_handle);
}

pub fn init_queues() {
    unsafe {
        let play_queue_b = PLAY_QUEUE.queue.clone();
        let play_join_handle = PLAY_QUEUE.join_handle.clone();
        let play_handle = task::spawn(async move {
            loop {
                if play_queue_b.lock().unwrap().len() > 0 {
                    // play_wav
                    play_wav(play_queue_b.lock().unwrap().pop_front().unwrap());
                } else {
                    task::sleep(Duration::from_millis(100)).await;
                }
            }
        });
        *play_join_handle.lock().unwrap() = Some(play_handle);
    }
}
