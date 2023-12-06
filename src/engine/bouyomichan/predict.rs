use async_trait::async_trait;

use crate::engine::bouyomichan::speak;
use crate::engine::Predictor;
use crate::variables::get_global_vars;

#[derive(Debug)]
pub struct BouyomichanPredictor {
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
    let volume: i16 = (100.0 * get_global_vars().volume.unwrap_or(1.0)) as i16;
    speak(&self.text, self.style_id as i16, volume)?;
    Ok(Vec::new())
  }
}
