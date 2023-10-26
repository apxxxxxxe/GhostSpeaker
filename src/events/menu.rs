use crate::events::common::load_descript;
use crate::events::common::*;
use crate::plugin::response::PluginResponse;
use crate::variables::{get_global_vars, CharacterVoice, GhostVoiceInfo};

use shiorust::message::Request;

const DEFAULT_VOICE: &str = "【削除された声質】";

pub fn on_menu_exec(req: &Request) -> PluginResponse {
    let mut characters_info = String::new();
    let mut division_setting = String::from("-");
    let mut no_engine_message = String::new();

    let refs = get_references(req);
    let ghost_name = refs.get(1).unwrap().to_string();
    let ghost_description = load_descript(refs.get(4).unwrap().to_string());
    let characters = count_characters(ghost_description);
    let path_for_arg = refs[4].to_string().replace("\\", "\\\\");

    if get_global_vars().volatility.speakers_info.is_some() {
        let speakers_info = get_global_vars().volatility.speakers_info.as_ref().unwrap();
        let mut chara_info = |name: &String, index: usize| -> String {
            let mut info = format!("\\\\{} ", index);
            if let Some(c) = characters.get(index) {
                info.push_str(&format!("{}:\\n    ", c));
            }
            let mut voice = String::from(DEFAULT_VOICE);
            if let Some(si) = get_global_vars().ghosts_voices.as_ref().unwrap().get(name) {
                let switch: String;
                if si.devide_by_lines {
                    switch = "有効".to_string();
                } else {
                    switch = "無効".to_string();
                }
                division_setting = format!(
                    "【現在 \\q[{},OnDivisionSettingChanged,{},{}]】\\n",
                    switch, ghost_name, path_for_arg
                );
                if let Some(c) = si.voices.get(index) {
                    if let Some(speaker) = speakers_info
                        .iter()
                        .find(|s| s.speaker_uuid == c.spekaer_uuid)
                    {
                        if let Some(style) = speaker
                            .styles
                            .iter()
                            .find(|s| s.style_id.unwrap() == c.style_id)
                        {
                            voice = format!(
                                "{} - {}",
                                speaker.speaker_name,
                                style.style_name.clone().unwrap(),
                            );
                        }
                    }
                }
            };
            info + &format!(
                "\\q[{},OnVoiceSelecting,{},{},{},{}]\\n",
                voice,
                ghost_name,
                characters.get(index).unwrap_or(&String::from("")),
                index,
                path_for_arg,
            )
        };

        for i in 0..characters.len() {
            characters_info.push_str(&chara_info(&ghost_name, i));
        }
    } else {
        no_engine_message = String::from("\\f[color,255,0,0]COEIROINK v2.0.0以降のエンジンの起動が必要です。\\f[color,default]\\n");
        for i in 0..characters.len() {
            characters_info.push_str(&format!(
                "{}:\\n    -\\n",
                characters.get(i).unwrap_or(&String::from("")),
            ));
        }
    }

    let unit: f32 = 0.05;
    let v = get_global_vars().volume.unwrap();

    let mut volume_changer = String::new();
    if v > unit {
        volume_changer.push_str(&format!(
            "\\q[<<,OnVolumeChange,-{},{},{}]",
            unit, refs[1], path_for_arg,
        ));
    }
    volume_changer.push_str(&format!(
        " {:.2} \
        \\q[>>,OnVolumeChange,{},{},{}]\\n\
        ",
        v, unit, refs[1], path_for_arg,
    ));

    let p = get_global_vars().speak_by_punctuation.unwrap();
    let switch: String;
    if p {
        switch = "有効".to_string();
    } else {
        switch = "無効".to_string();
    }
    let punctuation_changer = format!(
        "【現在 \\q[{},OnPunctuationSettingChanged,{},{}]】\\n",
        switch, ghost_name, path_for_arg
    );

    let m = format!(
        "\
    \\C\\c\\b[2]\\_q\
    {}\
    {}\\n\
    {}\\n\
    \\![*]音量調整(共通)\\n    {}\
    \\![*]句読点ごとにCOIROINKへ送信(共通)\\n    {}\
    \\![*]改行で一拍おく(ゴースト別)\\n    {}\
    \\n\\q[×,]\
    ",
        no_engine_message,
        ghost_name,
        characters_info,
        volume_changer,
        punctuation_changer,
        division_setting,
    );

    new_response_with_script(m.to_string(), true)
}

pub fn on_voice_selecting(req: &Request) -> PluginResponse {
    let refs = get_references(req);
    let ghost_name = refs.get(0).unwrap();
    let character_name = refs.get(1).unwrap();
    let character_index = refs.get(2).unwrap().parse::<usize>().unwrap();
    let ghost_path = refs.get(3).unwrap();

    let mut m = format!("\\C\\c\\b[2]\\_q{}\\n{}\\n", ghost_name, character_name);
    for speaker in get_global_vars()
        .volatility
        .speakers_info
        .as_ref()
        .unwrap()
        .iter()
    {
        for style in speaker.styles.iter() {
            m.push_str(&format!(
                "\\![*]\\q[{} | {},OnVoiceSelected,{},{},{},{},{}]\\n",
                speaker.speaker_name,
                style.style_name.as_ref().unwrap(),
                ghost_name,
                character_index,
                speaker.speaker_uuid,
                style.style_id.unwrap(),
                ghost_path,
            ));
        }
    }

    m.push_str(&format!("\\n\\q[×,]"));
    new_response_with_script(m.to_string(), true)
}

pub fn on_voice_selected(req: &Request) -> PluginResponse {
    let refs = get_references(req);
    let ghost_name = refs.get(0).unwrap();
    let character_index = refs.get(1).unwrap().parse::<usize>().unwrap();
    let speaker_uuid = refs.get(2).unwrap();
    let style_id = refs.get(3).unwrap();
    let ghost_path = refs.get(4).unwrap();

    let voice = CharacterVoice {
        spekaer_uuid: speaker_uuid.to_string(),
        style_id: style_id.to_string().parse::<i32>().unwrap(),
    };

    if let Some(info) = get_global_vars()
        .ghosts_voices
        .as_mut()
        .unwrap()
        .get_mut(*ghost_name)
    {
        let voices = &mut info.voices;
        if voices.len() - 1 < character_index {
            voices.resize(character_index + 1, CharacterVoice::default());
        }
        voices.remove(character_index);
        voices.insert(character_index, voice)
    } else {
        let mut g = GhostVoiceInfo::default();
        g.voices.resize(character_index, CharacterVoice::default());
        g.voices.insert(character_index, voice);
        get_global_vars()
            .ghosts_voices
            .as_mut()
            .unwrap()
            .insert(ghost_name.to_string(), g);
    }
    let script = format!(
        "\\![raiseplugin,{},OnMenuExec,dummy,{},dummy,dummy,{}]",
        get_global_vars().volatility.plugin_uuid,
        ghost_name,
        ghost_path.replace("\\", "\\\\")
    );
    new_response_with_script(script, false)
}

pub fn on_volume_change(req: &Request) -> PluginResponse {
    let refs = get_references(req);
    let volume: f32 = refs.get(0).unwrap().parse().unwrap();
    let vars = get_global_vars();
    let v = vars.volume.unwrap_or(1.0);
    vars.volume = Some(v + volume);
    let script = format!(
        "\\![raiseplugin,{},OnMenuExec,dummy,{},dummy,dummy,{}]",
        vars.volatility.plugin_uuid, refs[1], refs[2]
    );
    new_response_with_script(script, false)
}

pub fn on_division_setting_changed(req: &Request) -> PluginResponse {
    let refs = get_references(req);
    let ghost_name = refs[0].to_string();
    let path_for_arg = refs[1].to_string();
    if let Some(info) = get_global_vars()
        .ghosts_voices
        .as_mut()
        .unwrap()
        .get_mut(&ghost_name)
    {
        info.devide_by_lines = !info.devide_by_lines
    }

    let script = format!(
        "\\![raiseplugin,{},OnMenuExec,dummy,{},dummy,dummy,{}]",
        get_global_vars().volatility.plugin_uuid,
        ghost_name,
        path_for_arg
    );
    new_response_with_script(script, false)
}

pub fn on_punctuation_setting_changed(req: &Request) -> PluginResponse {
    let refs = get_references(req);
    let ghost_name = refs[0].to_string();
    let path_for_arg = refs[1].to_string();
    if let Some(s) = get_global_vars().speak_by_punctuation {
        get_global_vars().speak_by_punctuation = Some(!s);
    }

    let script = format!(
        "\\![raiseplugin,{},OnMenuExec,dummy,{},dummy,dummy,{}]",
        get_global_vars().volatility.plugin_uuid,
        ghost_name,
        path_for_arg
    );
    new_response_with_script(script, false)
}

#[cfg(test)]
mod test {
    use crate::events::common::load_descript;
    #[test]
    fn test_parse_descript() {
        let map = load_descript("E:\\Ukagaka\\Ukagaka-Ghost\\お気に入り\\DSLGS".to_string());
        for (k, v) in map.iter() {
            println!("{}: {}", k, v);
        }
    }
}
