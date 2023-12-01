use crate::engine::Engine;
use http::StatusCode;

pub async fn predict_text(
  engine: Engine,
  text: String,
  speaker: i32,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
  let domain: String = format!("http://localhost:{}/", engine.port);

  let synthesis_req: Vec<u8>;
  match reqwest::Client::new()
    .post(&format!("{}{}", domain, "audio_query"))
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
        return Err(res.status().to_string().into());
      }
    },
    Err(e) => {
      println!("Error: {:?}", e);
      return Err(e.to_string().into());
    }
  }

  let wav: Vec<u8>;
  match reqwest::Client::new()
    .post(&format!("{}{}", domain, "synthesis"))
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
        return Err(res.status().to_string().into());
      }
    },
    Err(e) => {
      println!("Error: {:?}", e);
      return Err(e.to_string().into());
    }
  }

  Ok(wav)
}
