use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

use crate::coeiroink::predict::{get_speaker, predict_text};
use crate::coeiroink::speaker::get_speakers_info;
use crate::coeiroink::utils::check_connection;
use crate::player::play_wav;
use crate::variables::get_global_vars;

pub static mut QUEUE: Option<Queue> = None;

pub struct Queue {
    predict_queue: Arc<Mutex<VecDeque<PredictArgs>>>,
    predict_state: Arc<(Mutex<bool>, Condvar)>,
    predict_join_handle: Option<thread::JoinHandle<()>>,
    play_queue: Arc<Mutex<VecDeque<Vec<u8>>>>,
    play_state: Arc<(Mutex<bool>, Condvar)>,
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
            predict_state: Arc::new((Mutex::new(false), Condvar::new())),
            predict_join_handle: None,
            play_queue: Arc::new(Mutex::new(VecDeque::new())),
            play_state: Arc::new((Mutex::new(false), Condvar::new())),
            play_join_handle: None,
            thread_stopper: Arc::new(Mutex::new(false)),
        }
    }

    pub fn init(&mut self) {
        let predict_queue_cln = Arc::clone(&self.predict_queue);
        let predict_state_cln = Arc::clone(&self.predict_state);
        let thread_stopper_cln_a = self.thread_stopper.clone();
        self.predict_join_handle = Some(thread::spawn(move || {
            let (lock, cvar) = &*predict_state_cln;
            let mut update = lock.lock().unwrap();
            loop {
                if predict_queue_cln.lock().unwrap().is_empty()
                    || get_global_vars().volatility.speakers_info.is_none()
                    || !check_connection()
                {
                    update = cvar.wait_while(update, |u| !*u).unwrap();
                }

                println!("predict thread started by condvar");

                if *thread_stopper_cln_a.lock().unwrap() {
                    return;
                }

                if let Some(args) = predict_queue_cln.lock().unwrap().pop_front() {
                    if let None = get_global_vars().volatility.speakers_info {
                        if let Ok(speakers_info) = get_speakers_info() {
                            get_global_vars().volatility.speakers_info = Some(speakers_info);
                        } else {
                            println!("{}", "Failed to get speakers info when predicting");
                            continue;
                        }
                    }
                    println!("{}", format!("predicting: {}", args.text));
                    let speaker = get_speaker(args.ghost_name, args.scope);
                    predict_and_queue(args.text, speaker.spekaer_uuid, speaker.style_id);
                }

                *update = false;
            }
        }));

        let play_queue_cln = self.play_queue.clone();
        let play_state_cln = self.play_state.clone();
        let thread_stopper_cln_b = self.thread_stopper.clone();
        let thread_stopper_cln_c = self.thread_stopper.clone();
        self.play_join_handle = Some(thread::spawn(move || {
            let (lock, cvar) = &*play_state_cln;
            let mut update = lock.lock().unwrap();
            loop {
                if play_queue_cln.lock().unwrap().is_empty() {
                    update = cvar.wait_while(update, |u| !*u).unwrap();
                }

                println!("{}", "play thread start");

                if *thread_stopper_cln_b.lock().unwrap() {
                    return;
                }

                if let Some(data) = play_queue_cln.lock().unwrap().pop_front() {
                    println!("{}", format!("play: {}", data.len()));
                    play_wav(data, &thread_stopper_cln_c);
                    if *thread_stopper_cln_c.lock().unwrap() {
                        return;
                    }
                }

                *update = false;
            }
        }));
    }

    pub fn stop(&mut self) {
        *self.thread_stopper.lock().unwrap() = true;
        {
            let (lock, cvar) = &*self.predict_state;
            *lock.lock().unwrap() = true;
            cvar.notify_one();
        }
        {
            let (lock, cvar) = &*self.play_state;
            *lock.lock().unwrap() = true;
            cvar.notify_one();
        }
        if let Some(handle) = self.predict_join_handle.take() {
            handle.join().unwrap();
        }
        if let Some(handle) = self.play_join_handle.take() {
            handle.join().unwrap();
        }
    }

    pub fn push_to_prediction(&self, args: PredictArgs) {
        self.predict_queue.lock().unwrap().push_back(args);
        let (lock, cvar) = &*self.predict_state;
        *lock.lock().unwrap() = true;
        cvar.notify_one();
    }

    fn push_to_play(&self, data: Vec<u8>) {
        self.play_queue.lock().unwrap().push_back(data);
        let (lock, cvar) = &*self.play_state;
        *lock.lock().unwrap() = true;
        cvar.notify_one();
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
