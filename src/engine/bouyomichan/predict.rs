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
    let volume: i16 = (100.0 * (*VOLUME.read().unwrap())) as i16;
    speak(&self.text, self.style_id as i16, volume)?;
    Ok(Vec::new())
  }
}
