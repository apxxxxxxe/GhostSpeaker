use crate::engine::{SpeakerGetter, ENGINE_BOUYOMICHAN};
use crate::speaker::{SpeakerInfo, Style};
use async_trait::async_trait;
use sysinfo::{ProcessExt, System, SystemExt};

pub const BOUYOMICHAN_UUID: &str = "bouyomichan";

pub struct BouyomiChanSpeakerGetter;

#[async_trait]
impl SpeakerGetter for BouyomiChanSpeakerGetter {
  async fn get_speakers_info(&self) -> Result<Vec<SpeakerInfo>, Box<dyn std::error::Error>> {
    if !is_process_running("BouyomiChan.exe") {
      return Err("BouyomiChan.exe is not running".into());
    }

    let speakers_info = vec![SpeakerInfo {
      speaker_name: ENGINE_BOUYOMICHAN.name.to_string(),
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

fn is_process_running(process_name: &str) -> bool {
  let system = System::new_all();
  for process in system.processes_by_name(process_name) {
    if process.name() == process_name {
      return true;
    }
  }
  false
}
