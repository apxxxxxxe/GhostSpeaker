const URL: &str = "http://127.0.0.1:50032";

pub async fn check_connection() -> bool {
    reqwest::Client::new().get(URL).send().await.is_ok()
}
