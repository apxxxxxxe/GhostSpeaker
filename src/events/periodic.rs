use crate::events::common::*;
use crate::plugin::response::PluginResponse;
use crate::variables::get_global_vars;

use shiorust::message::Request;

pub fn on_second_change(_req: &Request) -> PluginResponse {
  let vars = get_global_vars();

  let last = &vars.volatility.last_connection_status;
  let current = &vars.volatility.current_connection_status;
  let mut lines: Vec<String> = Vec::new();

  for (k, v) in current.iter() {
    if last.get(k).unwrap_or(&false) != v {
      if *v {
        lines.push(format!("{} が接続されました", k.name));
      } else {
        lines.push(format!("{} が切断されました", k.name));
      }
    }
  }

  vars.volatility.last_connection_status = current.clone();

  let update: String;
  if !vars.volatility.is_update_checked {
    update = format!(
      "\\C\\![updateother,--plugin={}]",
      vars.volatility.plugin_name
    );
    vars.volatility.is_update_checked = true;
  } else {
    update = String::new();
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
