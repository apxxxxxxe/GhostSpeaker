use crate::engine::{engine_from_port, ENGINE_LIST};
use crate::engine::{CharacterVoice, DUMMY_VOICE_UUID};
use crate::events::common::load_descript;
use crate::events::common::*;
use crate::player::get_player;
use crate::plugin::request::PluginRequest;
use crate::plugin::response::PluginResponse;
use crate::queue::get_queue;
use crate::variables::get_global_vars;
use std::collections::HashMap;

const DEFAULT_VOICE: &str = "【不明】";
const NO_VOICE: &str = "無し";

pub fn on_menu_exec(req: &PluginRequest) -> PluginResponse {
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
          voice = NO_VOICE.to_string();
        } else {
          if let Some(speakers_by_engine) = speakers_info.get(&(engine_from_port(c.port).unwrap()))
          {
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
            voice = format!(
              "【使用不可: {}の起動が必要】",
              engine_from_port(c.port).unwrap().name()
            );
          }
        }
      }
    };
    info
      + &format!(
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
  let empty = HashMap::new();
  let engine_auto_start = get_global_vars()
    .engine_auto_start
    .as_ref()
    .unwrap_or(&empty);
  for engine in ENGINE_LIST.iter() {
    if speakers_info.contains_key(engine) {
      engine_status += &format!(
        "{}: \\f[color,0,128,0]起動中\\f[color,default]",
        engine.name()
      );
    } else {
      engine_status += &format!(
        "{}: \\f[color,128,128,128]停止中\\f[color,default]",
        engine.name()
      );
    }
    let is_auto_start_string: String;
    if let Some(is_auto_start) = engine_auto_start.get(engine) {
      if *is_auto_start {
        is_auto_start_string = "\\f[color,0,128,0]有効\\f[color,default]".to_string();
      } else {
        is_auto_start_string = "\\f[color,128,0,0]無効\\f[color,default]".to_string();
      }
      engine_status += &format!(
        "\\_l[@0,]\\f[align,right]自動起動: \\__q[OnAutoStartToggled,{},{},{}]{}\\__q\\n",
        engine.port(),
        refs[1],
        path_for_arg,
        is_auto_start_string,
      );
    } else {
      engine_status += &format!(
        "\\_l[@0,]\\f[align,right]自動起動: \\f[color,128,128,128]設定未完了\\f[color,default]\\n"
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

  let player_clearer: String;
  let player_button_dialog = "再生中の音声を停止";
  if get_player().sink.empty() {
    player_clearer = format!(
      "\\![*]\\f[strike,true]{}\\f[strike,default]\\n\\n",
      player_button_dialog
    )
  } else {
    player_clearer = format!(
      "\\![*]\\q[{},OnPlayerClear,{},{}]\\n\\n",
      player_button_dialog, ghost_name, path_for_arg
    );
  }

  let m = format!(
    "\
      \\b[2]\\_q\
      \\f[align,center]\\f[size,12]{} v{}\\f[size,default]\\n\\n[half]\\f[align,left]\
      {}\
      {}\
      {}\\n\
      {}\\n\
      \\![*]音量調整(共通)\\n    {}\
      \\![*]句読点ごとにCOIROINKへ送信(共通)\\n    {}\
      \\![*]改行で一拍おく(ゴースト別)\\n    {}\
      \\n\\q[×,]\
      ",
    get_global_vars().volatility.plugin_name,
    env!("CARGO_PKG_VERSION"),
    engine_status,
    player_clearer,
    ghost_name,
    characters_info,
    volume_changer,
    punctuation_changer,
    division_setting,
  );

  new_response_with_script(m.to_string(), true)
}

pub fn on_voice_selecting(req: &PluginRequest) -> PluginResponse {
  let refs = get_references(req);
  let ghost_name = refs.get(0).unwrap();
  let character_name = refs.get(1).unwrap();
  let character_index = refs.get(2).unwrap().parse::<usize>().unwrap();
  let ghost_path = refs.get(3).unwrap();
  let speakers_info = &mut get_global_vars().volatility.speakers_info;

  let mut m = format!("\\C\\c\\b[2]\\_q{}\\n{}\\n", ghost_name, character_name);
  let def = CharacterVoice::default(None);
  m.push_str(&format!(
    "\\![*]\\q[{},OnVoiceSelected,{},{},{},{},{},{}]\\n",
    NO_VOICE, ghost_name, character_index, def.port, def.speaker_uuid, def.style_id, ghost_path,
  ));
  for (engine, speakers) in speakers_info.iter() {
    for speaker in speakers.iter() {
      for style in speaker.styles.iter() {
        m.push_str(&format!(
          "\\![*]\\q[{} | {},OnVoiceSelected,{},{},{},{},{},{}]\\n",
          speaker.speaker_name,
          style.style_name.as_ref().unwrap(),
          ghost_name,
          character_index,
          engine.port(),
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

pub fn on_voice_selected(req: &PluginRequest) -> PluginResponse {
  let refs = get_references(req);
  let ghost_name = refs.get(0).unwrap();
  let character_index = refs.get(1).unwrap().parse::<usize>().unwrap();
  let port = refs.get(2).unwrap();
  let speaker_uuid = refs.get(3).unwrap();
  let style_id = refs.get(4).unwrap();
  let ghost_path = refs.get(5).unwrap();

  let voice = CharacterVoice {
    port: port.to_string().parse::<i32>().unwrap(),
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

pub fn on_volume_change(req: &PluginRequest) -> PluginResponse {
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

pub fn on_division_setting_changed(req: &PluginRequest) -> PluginResponse {
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

pub fn on_punctuation_setting_changed(req: &PluginRequest) -> PluginResponse {
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

pub fn on_player_clear(req: &PluginRequest) -> PluginResponse {
  let refs = get_references(req);
  let ghost_name = refs[0].to_string();
  let path_for_arg = refs[1].to_string();
  get_queue().restart();
  get_player().sink.clear();

  let script = format!(
    "\\![raiseplugin,{},OnMenuExec,dummy,{},dummy,dummy,{}]",
    get_global_vars().volatility.plugin_uuid,
    ghost_name,
    path_for_arg
  );
  new_response_with_script(script, false)
}

pub fn on_auto_start_toggled(req: &PluginRequest) -> PluginResponse {
  let refs = get_references(req);
  let port = refs[0].parse::<i32>().unwrap();
  let ghost_name = refs[1].to_string();
  let path_for_arg = refs[2].to_string();

  let engine = engine_from_port(port).unwrap();
  if let Some(auto_start) = get_global_vars()
    .engine_auto_start
    .as_mut()
    .unwrap()
    .get_mut(&engine)
  {
    *auto_start = !*auto_start;
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
