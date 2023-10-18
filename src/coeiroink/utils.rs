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

pub fn check_engine_status() -> EngineStatus {
    match get_global_vars().engine_path.clone() {
        Some(path) => {
            let client = Client::new();
            let res = client.get(URL).send();
            match res {
                Ok(_) => EngineStatus::Running,
                Err(_) => {
                    if let Some(_) = find_process(&path) {
                        EngineStatus::Initializing
                    } else {
                        EngineStatus::Stopped
                    }
                }
            }
        }
        None => EngineStatus::Unknown,
    }
}

pub fn check_connection() -> bool {
    let client = Client::new();
    let res = client.get(URL).send();
    match res {
        Ok(_) => true,
        Err(_) => false,
    }
}
