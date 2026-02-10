use crate::events::common::*;
use crate::ipc::send_command_logged;
use crate::plugin::request::PluginRequest;
use crate::plugin::response::PluginResponse;
use crate::variables::{LOG_INIT_SUCCESS, PLUGIN_NAME};
use ghost_speaker_common::{Command, Response};
use std::sync::atomic::{AtomicBool, Ordering};

pub(crate) fn on_second_change(_req: &PluginRequest) -> PluginResponse {
  static UPDATE_CHECKED: AtomicBool = AtomicBool::new(false);
  static LOG_INIT_CHECKED: AtomicBool = AtomicBool::new(false);

  let mut lines: Vec<String> = Vec::new();

  // ワーカーから接続ダイアログをポーリング
  if let Some(Response::Dialog {
    message: Some(msg), ..
  }) = send_command_logged(&Command::PopDialog)
  {
    lines.push(msg);
  }

  let update = if !UPDATE_CHECKED.swap(true, Ordering::Relaxed) {
    format!("\\C\\![updateother,--plugin={}]", PLUGIN_NAME)
  } else {
    String::new()
  };

  if !LOG_INIT_CHECKED.swap(true, Ordering::Relaxed) {
    let log_init_success = match LOG_INIT_SUCCESS.read() {
      Ok(lis) => *lis,
      Err(e) => {
        error!("Failed to read LOG_INIT_SUCCESS: {}", e);
        false
      }
    };
    if !log_init_success {
      lines.push("ログファイルの初期化に失敗しました".to_string());
    }
  }

  if !lines.is_empty() {
    new_response_with_nobreak(
      format!(
        "\\C\\![set,trayballoon,--text={},--title=GhostSpeaker,--icon=info,--timeout=3]{}",
        lines.join(" / "),
        update
      ),
      false,
    )
  } else if !update.is_empty() {
    new_response_with_script(update, false)
  } else {
    new_response_nocontent()
  }
}
