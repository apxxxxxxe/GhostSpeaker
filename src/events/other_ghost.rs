use crate::events::common::*;
use crate::plugin::request::PluginRequest;
use crate::plugin::response::PluginResponse;
use crate::queue::push_to_prediction;
use crate::variables::*;

pub(crate) fn on_other_ghost_talk(req: &PluginRequest) -> PluginResponse {
  let refs = get_references(req);
  let ghost_name = refs[0].to_string();
  let flags = refs[2].to_string();
  let msg = refs[4].to_string();

  if !msg.is_empty() && !flags.contains("plugin-script") {
    debug!("pushing to prediction");
    push_to_prediction(msg.clone(), ghost_name.clone());
    debug!("pushed to prediction");
  }

  new_response_nocontent()
}

pub(crate) fn on_ghost_boot(req: &PluginRequest) -> PluginResponse {
  let refs = get_references(req);
  let ghost_name = refs[1].to_string();
  let path = refs[4].to_string();
  let description = load_descript(path);
  let characters = count_characters(description);

  let mut ghosts_voices = match GHOSTS_VOICES.write() {
    Ok(gv) => gv,
    Err(e) => {
      error!("Failed to write GHOSTS_VOICES: {}", e);
      return new_response_nocontent();
    }
  };
  if ghosts_voices.get(&ghost_name).is_none() {
    ghosts_voices.insert(ghost_name, GhostVoiceInfo::new(characters.len()));
  }

  new_response_nocontent()
}
