use reqwest::blocking::Client;

const URL: &str = "http://127.0.0.1:50032";

pub fn check_connection() -> bool {
    Client::new().get(URL).send().is_ok()
}
