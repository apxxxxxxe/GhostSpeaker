use crate::engine::bouyomichan::speak;
use crate::variables::get_global_vars;

pub async fn predict_text(
  text: String,
  style_id: i32,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
  let volume: i16 = (100.0 * get_global_vars().volume.unwrap_or(1.0)) as i16;
  speak(&text, style_id as i16, volume)?;
  Ok(Vec::new())
}
