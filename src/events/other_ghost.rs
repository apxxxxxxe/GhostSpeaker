use crate::events::common::*;
use crate::plugin::request::PluginRequest;
use crate::plugin::response::PluginResponse;
use crate::queue::get_queue;
use crate::variables::{get_global_vars, GhostVoiceInfo};

pub fn on_other_ghost_talk(req: &PluginRequest) -> PluginResponse {
  let refs = get_references(req);
  let ghost_name = refs[0].to_string();
  let msg = refs[4].to_string();

  if !msg.is_empty() {
    get_queue().push_to_prediction(msg.clone(), ghost_name.clone());
  }

  new_response_nocontent()
}

pub fn on_ghost_boot(req: &PluginRequest) -> PluginResponse {
  let refs = get_references(req);
  let ghost_name = refs[1].to_string();
  let path = refs[4].to_string();
  let description = load_descript(path);
  let characters = count_characters(description);

  if get_global_vars()
    .ghosts_voices
    .as_ref()
    .unwrap()
    .get(&ghost_name)
    .is_none()
  {
    get_global_vars()
      .ghosts_voices
      .as_mut()
      .unwrap()
      .insert(ghost_name, GhostVoiceInfo::new(characters.len()));
  }

  new_response_nocontent()
}
