use http::StatusCode;

pub async fn predict_text(text: String, speaker: i32) -> Result<Vec<u8>, reqwest::Error> {
    const DOMAIN: &str = "http://localhost:50021/";

    let synthesis_req: Vec<u8>;
    match reqwest::Client::new()
        .post(&format!("{}{}", DOMAIN, "audio_query"))
        .query(&[("speaker", speaker.to_string()), ("text", text)])
        .send()
        .await
    {
        Ok(res) => match res.status() {
            StatusCode::OK => {
                synthesis_req = res.bytes().await.unwrap().to_vec();
            }
            _ => {
                println!("Error: {:?}", res);
                return Err(res.error_for_status().unwrap_err());
            }
        },
        Err(e) => {
            println!("Error: {:?}", e);
            return Err(e);
        }
    }

    let wav: Vec<u8>;
    match reqwest::Client::new()
        .post(&format!("{}{}", DOMAIN, "synthesis"))
        .header("Content-Type", "application/json")
        .header("Accept", "audio/wav")
        .query(&[("speaker", speaker.to_string())])
        .body(synthesis_req)
        .send()
        .await
    {
        Ok(res) => match res.status() {
            StatusCode::OK => {
                wav = res.bytes().await.unwrap().to_vec();
            }
            _ => {
                println!("Error: {:?}", res);
                return Err(res.error_for_status().unwrap_err());
            }
        },
        Err(e) => {
            println!("Error: {:?}", e);
            return Err(e);
        }
    }

    Ok(wav)
}
