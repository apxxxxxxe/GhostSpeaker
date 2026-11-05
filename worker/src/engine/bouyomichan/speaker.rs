use crate::engine::{EngineNotRunning, SpeakerGetter};
use async_trait::async_trait;
use ghost_speaker_common::{Engine, SpeakerInfo, Style};

pub const BOUYOMICHAN_UUID: &str = "bouyomichan";
const PROCESS_NAME: &str = "BouyomiChan.exe";

pub struct BouyomiChanSpeakerGetter;

#[async_trait]
impl SpeakerGetter for BouyomiChanSpeakerGetter {
  async fn get_speakers_info(
    &self,
  ) -> Result<Vec<SpeakerInfo>, Box<dyn std::error::Error + Send + Sync>> {
    // HTTPを話さないポートなので接続は試さず、ポートの使用状況とプロセスの両方で判定する
    let port = Engine::BouyomiChan.port() as u16;
    let running = tokio::task::spawn_blocking(move || {
      crate::system::is_port_in_use(port) && crate::system::is_process_running(PROCESS_NAME)
    })
    .await
    .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
      format!("spawn_blocking failed: {}", e).into()
    })?;
    if !running {
      return Err(Box::new(EngineNotRunning(PROCESS_NAME)));
    }

    let speakers_info = vec![SpeakerInfo {
      speaker_name: Engine::BouyomiChan.name().to_string(),
      speaker_uuid: BOUYOMICHAN_UUID.to_string(),
      styles: vec![
        Style {
          style_name: Some("女性1".to_string()),
          style_id: Some(1),
        },
        Style {
          style_name: Some("女性2".to_string()),
          style_id: Some(2),
        },
        Style {
          style_name: Some("男性1".to_string()),
          style_id: Some(3),
        },
        Style {
          style_name: Some("男性2".to_string()),
          style_id: Some(4),
        },
        Style {
          style_name: Some("中性".to_string()),
          style_id: Some(5),
        },
        Style {
          style_name: Some("ロボット".to_string()),
          style_id: Some(6),
        },
        Style {
          style_name: Some("機械1".to_string()),
          style_id: Some(7),
        },
        Style {
          style_name: Some("機械2".to_string()),
          style_id: Some(8),
        },
      ],
    }];

    Ok(speakers_info)
  }
}
