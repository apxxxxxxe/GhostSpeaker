use crate::events::common::*;
use crate::plugin::request::PluginRequest;
use crate::plugin::response::PluginResponse;
use crate::queue::CONNECTION_DIALOGS;
use crate::variables::{LOG_INIT_SUCCESS, PLUGIN_NAME};
use once_cell::sync::Lazy;
use std::sync::Mutex;

static UPDATE_CHECKED: Lazy<Mutex<bool>> = Lazy::new(|| Mutex::new(false));
static LOG_INIT_CHECKED: Lazy<Mutex<bool>> = Lazy::new(|| Mutex::new(false));

pub(crate) fn on_second_change(_req: &PluginRequest) -> PluginResponse {
  let mut lines: Vec<String> = Vec::new();
  if let Ok(mut dialogs) = CONNECTION_DIALOGS.lock() {
    if let Some(line) = dialogs.pop() {
      lines.push(line);
    }
  }

  let update: String;
  let update_checked;
  {
    update_checked = *UPDATE_CHECKED.lock().unwrap();
  }
  if !update_checked {
    *UPDATE_CHECKED.lock().unwrap() = true;
    update = format!("\\C\\![updateother,--plugin={}]", PLUGIN_NAME);
  } else {
    update = String::new();
  }

  let log_init_checked;
  {
    log_init_checked = match LOG_INIT_CHECKED.lock() {
      Ok(lic) => *lic,
      Err(e) => {
        error!("Failed to lock LOG_INIT_CHECKED: {}", e);
        true // フォールバック値
      }
    };
  }
  if !log_init_checked {
    match LOG_INIT_CHECKED.lock() {
      Ok(mut lic) => *lic = true,
      Err(e) => error!("Failed to lock LOG_INIT_CHECKED for write: {}", e),
    }
    let log_init_success = match LOG_INIT_SUCCESS.read() {
      Ok(lis) => *lis,
      Err(e) => {
        error!("Failed to read LOG_INIT_SUCCESS: {}", e);
        false // フォールバック値
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
