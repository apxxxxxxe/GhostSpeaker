use base64::{engine::general_purpose, Engine as _};
use http::StatusCode;
use serde::{Deserialize, Serialize};

use crate::player::Wave;
use crate::variables::{get_global_vars, CharacterVoice, GhostVoiceInfo};

#[derive(Debug, Deserialize)]
struct PredictResponse {
    #[serde(rename = "wavBase64")]
    wav_base64: String,

    #[serde(rename = "moraDurations")]
    mora_durations: Vec<MoraDuration>,
}

impl PredictResponse {
    fn to_wav(&self) -> Wave {
        let d = (self.mora_durations.last().unwrap().wav_range.end) as u64;
        Wave {
            data: general_purpose::STANDARD.decode(&self.wav_base64).unwrap(),
            duration_ms: d,
        }
    }
}

#[derive(Debug, Deserialize)]
struct MoraDuration {
    #[serde(rename = "mora")]
    _mora: String,

    #[serde(rename = "hira")]
    _hira: String,

    #[serde(rename = "phonemePitches")]
    _phoneme_pitches: Vec<PhonemePitch>,

    #[serde(rename = "wavRange")]
    wav_range: WavRange,
}

#[derive(Debug, Deserialize)]
struct PhonemePitch {
    #[serde(rename = "phoneme")]
    _phoneme: String,

    #[serde(rename = "wavRange")]
    _wav_range: WavRange,
}

#[derive(Debug, Deserialize)]
struct WavRange {
    #[serde(rename = "start")]
    _start: u32,

    #[serde(rename = "end")]
    end: u32,
}

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

pub fn get_speaker(ghost_name: String, scope: usize) -> CharacterVoice {
    let info: &GhostVoiceInfo;
    let g = GhostVoiceInfo::default();
    match &get_global_vars()
        .ghosts_voices
        .as_ref()
        .unwrap()
        .get(&ghost_name)
    {
        Some(i) => info = i,
        None => info = &g,
    }

    match info.voices.get(scope) {
        Some(voice) => voice.clone(),
        None => CharacterVoice::default(), // descript.txtにないキャラの場合
    }
}

pub async fn predict_text(
    text: String,
    speaker_uuid: String,
    style_id: i32,
) -> Result<Wave, reqwest::Error> {
    const URL: &str = "http://localhost:50032/v1/predict_with_duration";

    let req = PredictRequest {
        speaker_uuid,
        style_id,
        text,
        prosody_detail: None,
        speed_scale: 1.0,
    };
    let b = serde_json::to_string(&req).unwrap();

    let predict_res: PredictResponse;
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
                predict_res = serde_json::from_str(&res.text().await.unwrap()).unwrap();
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

    Ok(predict_res.to_wav())
}
