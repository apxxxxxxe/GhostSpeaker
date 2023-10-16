use crate::events::common::*;
use crate::queue::{get_queue, PredictArgs};
use crate::response::PluginResponse;
use crate::variables::{get_global_vars, CharacterVoice};
use regex::Regex;
use shiorust::message::Request;

pub fn on_second_change(req: &Request) -> PluginResponse {
    let vars = get_global_vars();
    let total_time = vars.total_time.unwrap();
    vars.total_time = Some(total_time + 1);
    vars.volatility.ghost_up_time += 1;

    let refs = get_references(req);
    let idle_secs = refs[4].parse::<i32>().unwrap();
    vars.volatility.idle_seconds = idle_secs;

    new_response_nocontent()
}

pub fn on_other_ghost_talk(req: &Request) -> PluginResponse {
    let refs = get_references(req);
    let ghost_name = refs[0].to_string();
    let msg = refs[4].to_string();
    if msg.is_empty() {
        return new_response_nocontent();
    }

    let sakura_script_re =
        Regex::new(r###"\\_{0,2}[a-zA-Z0-9*!&](\d|\[("([^"]|\\")+?"|([^\]]|\\\])+?)+?\])?"###)
            .unwrap();

    let dialog = sakura_script_re.replace_all(&msg, "").to_string();
    if !dialog.is_empty() {
        let info = &get_global_vars().ghosts_voices.get(&ghost_name).unwrap();
        let speaker = info.get(0).unwrap(); // TODO: 話者ごとに変える
        let args = PredictArgs {
            text: dialog,
            speaker_uuid: speaker.spekaer_uuid.clone(),
            style_id: speaker.style_id,
        };
        get_queue().push_to_prediction(args); // TODO: 段落もしくは句点ごとに分割してpushする
    }

    new_response_nocontent()
}

pub fn on_ghost_boot(req: &Request) -> PluginResponse {
    let refs = get_references(req);
    let ghost_name = refs[1].to_string();
    let path = refs[4].to_string();
    let description = load_descript(path);
    let characters = count_characters(description);

    if let None = get_global_vars().ghosts_voices.get(&ghost_name) {
        let mut vec = Vec::new();
        vec.resize(characters.len(), CharacterVoice::default());
        get_global_vars().ghosts_voices.insert(ghost_name, vec);
    }

    new_response_nocontent()
}
