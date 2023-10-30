use crate::engine::{engine_name, ENGINE_COEIROINK, ENGINE_VOICEVOX};
use crate::events::common::load_descript;
use crate::events::common::*;
use crate::plugin::response::PluginResponse;
use crate::variables::{get_global_vars, CharacterVoice, DUMMY_VOICE_UUID};

use shiorust::message::Request;

const DEFAULT_VOICE: &str = "【不明】";

pub fn on_menu_exec(req: &Request) -> PluginResponse {
    let mut characters_info = String::new();
    let mut division_setting = String::from("-");

    let refs = get_references(req);
    let ghost_name = refs.get(1).unwrap().to_string();
    let ghost_description = load_descript(refs.get(4).unwrap().to_string());
    let characters = count_characters(ghost_description);
    let path_for_arg = refs[4].to_string().replace("\\", "\\\\");

    let speakers_info = &mut get_global_vars().volatility.speakers_info;
    let mut chara_info = |name: &String, index: usize| -> String {
        let mut info = format!("\\\\{} ", index);
        if let Some(c) = characters.get(index) {
            info.push_str(&format!("{}:\\n    ", c));
        }
        let mut voice = String::from(DEFAULT_VOICE);
        let mut color = "";
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
                if c.speaker_uuid == DUMMY_VOICE_UUID {
                    voice = "【無し】".to_string();
                } else {
                    if let Some(speakers_by_engine) = speakers_info.get(&c.engine) {
                        if let Some(speaker) = speakers_by_engine
                            .iter()
                            .find(|s| s.speaker_uuid == c.speaker_uuid)
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
                    } else {
                        color = "\\f[color,128,128,128]";
                        voice = format!("【使用不可: {}の起動が必要】", engine_name(c.engine));
                    }
                }
            }
        };
        info + &format!(
            "{}\\q[{},OnVoiceSelecting,{},{},{},{}]\\f[color,default]\\n",
            color,
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

    let mut engine_status = String::new();
    let engines = [
        (ENGINE_VOICEVOX, "VOICEVOX"),
        (ENGINE_COEIROINK, "COEIROINK"),
    ];
    for (engine, name) in engines.iter() {
        if speakers_info.contains_key(engine) {
            engine_status += &format!("{}: \\f[color,0,128,0]起動中\\f[color,default]\\n", name);
        } else {
            engine_status += &format!(
                "{}: \\f[color,128,128,128]停止中\\f[color,default]\\n",
                name
            );
        }
    }
    if !engine_status.is_empty() {
        engine_status += "\\n";
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
        engine_status,
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
    let speakers_info = &mut get_global_vars().volatility.speakers_info;

    let mut m = format!("\\C\\c\\b[2]\\_q{}\\n{}\\n", ghost_name, character_name);
    for (engine, speakers) in speakers_info.iter() {
        for speaker in speakers.iter() {
            for style in speaker.styles.iter() {
                m.push_str(&format!(
                    "\\![*]\\q[{} | {},OnVoiceSelected,{},{},{},{},{},{}]\\n",
                    speaker.speaker_name,
                    style.style_name.as_ref().unwrap(),
                    ghost_name,
                    character_index,
                    engine,
                    speaker.speaker_uuid,
                    style.style_id.unwrap(),
                    ghost_path,
                ));
            }
        }
    }

    m.push_str(&format!("\\n\\q[×,]"));
    new_response_with_script(m.to_string(), true)
}

pub fn on_voice_selected(req: &Request) -> PluginResponse {
    let refs = get_references(req);
    let ghost_name = refs.get(0).unwrap();
    let character_index = refs.get(1).unwrap().parse::<usize>().unwrap();
    let engine = refs.get(2).unwrap();
    let speaker_uuid = refs.get(3).unwrap();
    let style_id = refs.get(4).unwrap();
    let ghost_path = refs.get(5).unwrap();

    let voice = CharacterVoice {
        engine: engine.to_string().parse::<i32>().unwrap(),
        speaker_uuid: speaker_uuid.to_string(),
        style_id: style_id.to_string().parse::<i32>().unwrap(),
    };

    if let Some(info) = get_global_vars()
        .ghosts_voices
        .as_mut()
        .unwrap()
        .get_mut(*ghost_name)
    {
        let voices = &mut info.voices;
        voices.remove(character_index);
        voices.insert(character_index, voice)
    } else {
        // OnGhostBootで設定されているはず
        panic!("Ghost {} not found", ghost_name);
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
