use crate::events::common::*;
use crate::queue::{get_queue, PredictArgs};
use crate::response::PluginResponse;
use crate::speaker::{get_speakers_info, SpeakerInfo};
use crate::variables::get_global_vars;
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
    let msg = refs[4].to_string();
    if msg.is_empty() {
        return new_response_nocontent();
    }

    let sakura_script_re =
        Regex::new(r###"\\_{0,2}[a-zA-Z0-9*!&](\d|\[("([^"]|\\")+?"|([^\]]|\\\])+?)+?\])?"###)
            .unwrap();

    let dialog = sakura_script_re.replace_all(&msg, "").to_string();
    if !dialog.is_empty() {
        let info = &get_global_vars().volatility.speakers_info;
        let speaker = info.get(0).unwrap();
        let speaker_uuid = String::from(&speaker.speaker_uuid);
        let style_id = speaker.styles.get(0).unwrap().style_id.unwrap();
        let args = PredictArgs {
            text: dialog,
            speaker_uuid,
            style_id,
        };
        get_queue().push_to_prediction(args); // TODO: 段落もしくは句点ごとに分割してpushする
    }

    new_response_nocontent()
}
