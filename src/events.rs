mod common;
mod menu;
mod other_ghost;
mod periodic;

use crate::events::common::*;
use crate::events::menu::*;
use crate::events::other_ghost::*;
use crate::events::periodic::*;
use crate::plugin::request::PluginRequest;
use crate::plugin::response::PluginResponse;
use shiorust::message::{parts::*, traits::*};

pub fn handle_request(req: &PluginRequest) -> PluginResponse {
  match req.method {
    Method::GET | Method::NOTIFY => (),
    _ => return new_response_nocontent(),
  };

  let event_id = match req.headers.get("ID") {
    Some(id) => id,
    None => return new_response_nocontent(),
  };

  debug!("event: {}", event_id);

  let event = match get_event(event_id.as_str()) {
    Some(e) => e,
    None => {
      let base_id = match req.headers.get("BaseID") {
        Some(id) => id,
        None => return new_response_nocontent(),
      };
      match get_event(base_id.as_str()) {
        Some(e) => e,
        None => return new_response_nocontent(),
      }
    }
  };

  let res = event(req);
  debug!("response: {:?}", res);
  res
}

pub fn version(_req: &PluginRequest) -> PluginResponse {
  new_response_with_script(String::from(env!("CARGO_PKG_VERSION")), false)
}

fn get_event(id: &str) -> Option<fn(&PluginRequest) -> PluginResponse> {
  match id {
    "version" => Some(version),
    "OnOtherGhostTalk" => Some(on_other_ghost_talk),
    "OnMenuExec" => Some(on_menu_exec),
    "OnVoiceSelecting" => Some(on_voice_selecting),
    "OnVoiceSelected" => Some(on_voice_selected),
    "OnVolumeChange" => Some(on_volume_change),
    "OnDefaultVoiceSelecting" => Some(on_default_voice_selecting),
    "OnDefaultVoiceSelected" => Some(on_default_voice_selected),
    "OnDivisionSettingChanged" => Some(on_division_setting_changed),
    "OnPunctuationSettingChanged" => Some(on_punctuation_setting_changed),
    "OnSecondChange" => Some(on_second_change),
    "OnPlayerClear" => Some(on_player_clear),
    "OnAutoStartToggled" => Some(on_auto_start_toggled),
    "OnCharacterResized" => Some(on_character_resized),
    "OnPlayerSettingToggled" => Some(on_player_setting_toggled),
    "OnGhostBoot" => Some(on_ghost_boot),
    _ => None,
  }
}
