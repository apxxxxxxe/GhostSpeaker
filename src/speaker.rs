#[derive(Debug, Clone)]
pub struct SpeakerInfo {
  pub speaker_name: String,

  pub speaker_uuid: String,

  pub styles: Vec<Style>,
}

#[derive(Debug, Clone)]
pub struct Style {
  pub style_name: Option<String>,

  pub style_id: Option<i32>,
}
