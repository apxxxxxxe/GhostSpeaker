use crate::engine::bouyomichan::speak;
use crate::engine::Predictor;
use crate::variables::*;
use async_trait::async_trait;

#[derive(Debug)]
pub(crate) struct BouyomichanPredictor {
  pub text: String,
  pub style_id: i32,
}

impl BouyomichanPredictor {
  pub fn new(text: String, style_id: i32) -> Self {
    Self { text, style_id }
  }
}

#[async_trait]
impl Predictor for BouyomichanPredictor {
  async fn predict(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let volume: i16 = match VOLUME.read() {
      Ok(v) => (100.0 * *v) as i16,
      Err(e) => {
        error!("Failed to read VOLUME, using default: {}", e);
        100 // デフォルト音量
      }
    };
    let text = self.text.clone();
    let style_id = self.style_id as i16;
    let result = tokio::task::spawn_blocking(move || {
      speak(&text, style_id, volume).map_err(|e| e.to_string())
    })
    .await;
    match result {
      Ok(Ok(())) => {}
      Ok(Err(e)) => return Err(e.into()),
      Err(e) => return Err(format!("spawn_blocking failed: {}", e).into()),
    }
    Ok(Vec::new())
  }
}
