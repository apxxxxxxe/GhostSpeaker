use crate::coeiroink::speaker::get_speakers_info;
use crate::coeiroink::utils::check_connection;
use crate::events::common::*;
use crate::format::*;
use crate::queue::{get_queue, PredictArgs};
use crate::response::PluginResponse;
use crate::variables::{get_global_vars, CharacterVoice};
use shiorust::message::Request;

pub fn on_second_change(_req: &Request) -> PluginResponse {
    let vars = get_global_vars();
    if vars.volatility.speakers_info.is_none() {
        if check_connection() {
            if let Ok(speakers_info) = get_speakers_info() {
                vars.volatility.speakers_info = Some(speakers_info);
            }
        }
    }
    new_response_nocontent()
}

pub fn on_other_ghost_talk(req: &Request) -> PluginResponse {
    if check_connection() == false {
        return new_response_nocontent();
    }

    let refs = get_references(req);
    let ghost_name = refs[0].to_string();
    let msg = refs[4].to_string();
    if msg.is_empty() {
        return new_response_nocontent();
    }

    for dialog in split_dialog(msg) {
        if dialog.text.is_empty() {
            continue;
        }
        get_queue().push_to_prediction(PredictArgs {
            text: dialog.text,
            ghost_name: ghost_name.clone(),
            scope: dialog.scope,
        });
    }

    new_response_nocontent()
}

pub fn on_ghost_boot(req: &Request) -> PluginResponse {
    let refs = get_references(req);
    let ghost_name = refs[1].to_string();
    let path = refs[4].to_string();
    let description = load_descript(path);
    let characters = count_characters(description);

    if let None = get_global_vars()
        .ghosts_voices
        .as_ref()
        .unwrap()
        .get(&ghost_name)
    {
        let mut vec = Vec::new();
        vec.resize(characters.len(), CharacterVoice::default());
        get_global_vars()
            .ghosts_voices
            .as_mut()
            .unwrap()
            .insert(ghost_name, vec);
    }

    new_response_nocontent()
}
