use crate::engine::bouyomichan::speak;
use crate::engine::Predictor;
use async_trait::async_trait;

#[derive(Debug)]
pub struct BouyomichanPredictor {
  pub text: String,
  pub style_id: i32,
  pub volume: f32,
}

impl BouyomichanPredictor {
  pub fn new(text: String, style_id: i32, volume: f32) -> Self {
    Self {
      text,
      style_id,
      volume,
    }
  }
}

#[async_trait]
impl Predictor for BouyomichanPredictor {
  async fn predict(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let volume: i16 = (100.0 * self.volume) as i16;
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
