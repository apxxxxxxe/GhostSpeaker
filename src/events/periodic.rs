use crate::events::common::*;
use crate::plugin::request::PluginRequest;
use crate::plugin::response::PluginResponse;
use crate::queue::CONNECTION_DIALOGS;
use crate::variables::LOG_INIT_SUCCESS;
use once_cell::sync::Lazy;
use std::sync::Mutex;

static LOG_INIT_CHECKED: Lazy<Mutex<bool>> = Lazy::new(|| Mutex::new(false));

pub(crate) fn on_second_change(_req: &PluginRequest) -> PluginResponse {
  let mut lines: Vec<String> = Vec::new();
  if let Some(line) = CONNECTION_DIALOGS.lock().unwrap().pop() {
    lines.push(line);
  }

  let log_init_checked;
  {
    log_init_checked = *LOG_INIT_CHECKED.lock().unwrap();
  }
  if !log_init_checked {
    *LOG_INIT_CHECKED.lock().unwrap() = true;
    if !*LOG_INIT_SUCCESS.read().unwrap() {
      lines.push("ログファイルの初期化に失敗しました".to_string());
    }
  }

  if !lines.is_empty() {
    new_response_with_nobreak(
      format!(
        "\\C\\![set,trayballoon,--text={},--title=GhostSpeaker,--icon=info,--timeout=3]",
        lines.join(" / "),
      ),
      false,
    )
  } else {
    new_response_nocontent()
  }
}
