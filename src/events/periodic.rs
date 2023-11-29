use crate::engine::engine_name;
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
        lines.push(format!("{} が接続されました", engine_name(*k)));
      } else {
        lines.push(format!("{} が切断されました", engine_name(*k)));
      }
    }
  }

  vars.volatility.last_connection_status = current.clone();

  if !lines.is_empty() {
    new_response_with_script(
      format!(
        "\\C\\![set,trayballoon,--text={},--title=GhostSpeaker,--icon=info,--timeout=3]",
        lines.join(" / ")
      ),
      false,
    )
  } else {
    new_response_nocontent()
  }
}
