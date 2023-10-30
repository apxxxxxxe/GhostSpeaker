use std::time::Duration;
pub async fn check_connection(port: i32) -> bool {
    let url = format!("http://127.0.0.1:{}", port);
    reqwest::Client::new()
        .get(url)
        .timeout(Duration::from_millis(500))
        .send()
        .await
        .is_ok()
}
