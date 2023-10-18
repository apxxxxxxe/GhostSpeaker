use crate::process::find_process;
use crate::variables::get_global_vars;
use reqwest::blocking::Client;

#[derive(Debug, PartialEq, Eq)]
pub enum EngineStatus {
    Initializing,
    Running,
    Stopped,
    Unknown, // not connected but it is not sure that it is running or not
}

const URL: &str = "http://127.0.0.1:50032";

pub fn check_engine_status() -> (bool, bool, bool) {
    let (mut is_connected, mut is_running, mut is_traced) = (false, false, false);

    let res = Client::new().get(URL).send();
    if let Ok(_) = res {
        is_connected = true;
        is_running = true;
    }

    if let Some(path) = get_global_vars().engine_path.clone() {
        is_traced = true;
        if let Some(_) = find_process(&path) {
            is_running = true;
        }
    }

    (is_connected, is_running, is_traced)
}

pub fn check_connection() -> bool {
    Client::new().get(URL).send().is_ok()
}
