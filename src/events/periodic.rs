use crate::coeiroink::speaker::{get_speakers_info};
use crate::coeiroink::utils::{check_engine_status, EngineStatus};
use crate::events::common::*;
use crate::format::*;
use crate::queue::{get_queue, PredictArgs};
use crate::response::PluginResponse;
use crate::variables::{get_global_vars, CharacterVoice};
use shiorust::message::Request;

pub fn on_second_change(_req: &Request) -> PluginResponse {
    new_response_nocontent()
}

pub fn on_other_ghost_talk(req: &Request) -> PluginResponse {
    if check_engine_status() != EngineStatus::Running {
        return new_response_nocontent();
    }

    if let None = get_global_vars().volatility.speakers_info {
        get_global_vars().volatility.speakers_info = Some(get_speakers_info().unwrap());
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
        let info = &get_global_vars()
            .ghosts_voices
            .as_ref()
            .unwrap()
            .get(&ghost_name)
            .unwrap();
        let speaker = info.get(dialog.scope as usize).unwrap();
        let args = PredictArgs {
            text: dialog.text,
            speaker_uuid: speaker.spekaer_uuid.clone(),
            style_id: speaker.style_id,
        };
        get_queue().push_to_prediction(args);
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
