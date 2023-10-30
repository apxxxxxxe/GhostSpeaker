use http::StatusCode;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct PredictRequest {
    #[serde(rename = "speakerUuid")]
    pub speaker_uuid: String,

    #[serde(rename = "styleId")]
    pub style_id: i32,

    #[serde(rename = "text")]
    pub text: String,

    #[serde(rename = "prosodyDetail")]
    pub prosody_detail: Option<Vec<Vec<ProsodyDetail>>>,

    #[serde(rename = "speedScale")]
    pub speed_scale: f32,
}

#[derive(Debug, Serialize)]
pub struct ProsodyDetail {
    #[serde(rename = "phoneme")]
    pub phoneme: String,

    #[serde(rename = "hira")]
    pub hira: String,

    #[serde(rename = "accent")]
    pub accent: i32,
}

pub async fn predict_text(
    text: String,
    speaker_uuid: String,
    style_id: i32,
) -> Result<Vec<u8>, reqwest::Error> {
    const URL: &str = "http://localhost:50032/v1/predict";

    let req = PredictRequest {
        speaker_uuid,
        style_id,
        text,
        prosody_detail: None,
        speed_scale: 1.0,
    };
    let b = serde_json::to_string(&req).unwrap();

    let wav: Vec<u8>;
    match reqwest::Client::new()
        .post(URL)
        .header("Content-Type", "application/json")
        .header("Accept", "audio/wav")
        .body(b)
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
