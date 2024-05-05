use crate::engine::{engine_from_port, CharacterVoice, Engine, DUMMY_VOICE_UUID, ENGINE_LIST};
use crate::events::common::load_descript;
use crate::events::common::*;
use crate::plugin::response::PluginResponse;
use crate::queue::get_queue;
use crate::speaker::{SpeakerInfo, Style};
use crate::variables::get_global_vars;
use crate::{player::get_player, plugin::request::PluginRequest};
use once_cell::sync::Lazy;
use std::collections::HashMap;

const DEFAULT_VOICE: &str = "【不明】";
const NO_VOICE: &str = "無し";
const UNSET_VOICE: &str = "未設定";

static ACTIVATED: Lazy<String> = Lazy::new(|| greened("有効"));
static DEACTIVATED: Lazy<String> = Lazy::new(|| reded("無効"));

enum CharacterResizeMode {
  Expand,
  Shrink,
}

impl CharacterResizeMode {
  fn from_usize(n: usize) -> Self {
    match n {
      0 => Self::Expand,
      1 => Self::Shrink,
      _ => panic!("Invalid mode: {}", n),
    }
  }
}

fn colored(s: &str, r: u8, g: u8, b: u8) -> String {
  format!("\\f[color,{},{},{}]{}\\f[color,default]", r, g, b, s)
}

fn reded(s: &str) -> String {
  colored(s, 128, 0, 0)
}

fn greened(s: &str) -> String {
  colored(s, 0, 128, 0)
}

fn grayed(s: &str) -> String {
  colored(s, 128, 128, 128)
}

fn decorated(s: &str, decoration: &str) -> String {
  format!("\\f[{},1]{}\\f[{},0]", decoration, s, decoration)
}

pub fn on_menu_exec(req: &PluginRequest) -> PluginResponse {
  let mut characters_info = String::new();
  let mut division_setting = String::from("-");

  let refs = get_references(req);
  let ghost_name = refs.get(1).unwrap().to_string();
  let ghost_description = load_descript(refs.get(4).unwrap().to_string());
  let characters = count_characters(ghost_description);
  let path_for_arg = refs[4].to_string().replace('\\', "\\\\");
  let character_voices = &get_global_vars()
    .ghosts_voices
    .as_ref()
    .unwrap()
    .get(&ghost_name)
    .unwrap()
    .voices;

  let speakers_info = &mut get_global_vars().volatility.speakers_info;

  if let Some(si) = get_global_vars()
    .ghosts_voices
    .as_ref()
    .unwrap()
    .get(&ghost_name)
  {
    let switch = if si.devide_by_lines {
      ACTIVATED.to_string()
    } else {
      DEACTIVATED.to_string()
    };
    division_setting = format!(
      "【現在 \\__q[OnDivisionSettingChanged,{},{}]{}\\__q】\\n",
      ghost_name,
      path_for_arg,
      decorated(&switch, "bold"),
    );
  }

  for i in 0..character_voices.len() {
    characters_info.push_str(&chara_info(&characters, &ghost_name, i, &path_for_arg));
  }
  let mut character_resize_buttons = String::new();
  if character_voices.len() > characters.len() {
    character_resize_buttons.push_str(&format!(
      "\\__q[OnCharacterResized,{},{},{}]{}\\__q ",
      ghost_name,
      path_for_arg,
      CharacterResizeMode::Shrink as usize,
      decorated("-", "bold"),
    ));
  }
  character_resize_buttons.push_str(&format!(
    "\\__q[OnCharacterResized,{},{},{}]{}\\__q",
    ghost_name,
    path_for_arg,
    CharacterResizeMode::Expand as usize,
    decorated("+", "bold"),
  ));
  characters_info.push_str(&format!("【{}】\\n", character_resize_buttons));

  let mut engine_status = String::new();
  let empty = HashMap::new();
  let engine_auto_start = get_global_vars()
    .engine_auto_start
    .as_ref()
    .unwrap_or(&empty);
  for engine in ENGINE_LIST.iter() {
    if speakers_info.contains_key(engine) {
      engine_status += &format!("{}: {}", engine.name(), greened("起動中"),);
    } else {
      engine_status += &format!("{}: {}", engine.name(), grayed("停止中"),);
    }
    let is_auto_start_string: String;
    if let Some(is_auto_start) = engine_auto_start.get(engine) {
      if *is_auto_start {
        is_auto_start_string = decorated(&ACTIVATED, "bold");
      } else {
        is_auto_start_string = decorated(&DEACTIVATED, "bold");
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
        "\\_l[@0,]\\f[align,right]自動起動: {}\\n",
        grayed("設定未完了")
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
      "\\__q[OnVolumeChange,-{},{},{}]{}\\__q",
      unit,
      refs[1],
      path_for_arg,
      decorated("<<", "bold"),
    ));
  }
  volume_changer.push_str(&format!(
    " {:.2} \
    \\__q[OnVolumeChange,{},{},{}]{}\\__q\\n\
    ",
    v,
    unit,
    refs[1],
    path_for_arg,
    decorated(">>", "bold"),
  ));

  let p = get_global_vars().speak_by_punctuation.unwrap();
  let switch = if p {
    ACTIVATED.to_string()
  } else {
    DEACTIVATED.to_string()
  };
  let punctuation_changer = format!(
    "【現在 \\__q[OnPunctuationSettingChanged,{},{}]{}\\__q】\\n",
    ghost_name,
    path_for_arg,
    decorated(&switch, "bold"),
  );

  let player_button_dialog = "再生中の音声を停止";
  let player_clearer = if get_player().sink.empty() {
    format!("\\![*]{}\\n\\n", grayed(player_button_dialog))
  } else {
    format!(
      "\\![*]\\__q[OnPlayerClear,{},{}]{}\\__q\\n\\n",
      ghost_name,
      path_for_arg,
      decorated(player_button_dialog, "bold"),
    )
  };

  let wait_setting = if get_global_vars().wait_for_speech.unwrap() {
    ACTIVATED.to_string()
  } else {
    DEACTIVATED.to_string()
  };
  let wait_for_speech_changer = format!(
    "【現在 \\__q[OnPlayerSettingToggled,{},{}]{}\\__q】\\n",
    ghost_name,
    path_for_arg,
    decorated(&wait_setting, "bold"),
  );

  let default_voice_info = format!(
    "【現在 \\__q[OnDefaultVoiceSelecting,{},{}]{}\\__q】\\n",
    ghost_name,
    path_for_arg,
    get_voice(&Some(get_global_vars().initial_voice.clone()))
  );

  let m = format!(
    "\
      \\b[2]\\_q\
      \\f[align,center]\\f[size,12]{} v{}\\f[size,default]\\n\\n[half]\\f[align,left]\
      {}\
      {}\
      {}\\n\
      {}\\n\
      \\![*]音量調整(共通)\\n    {}\
      \\![*]句読点ごとに読み上げ(共通)\\n    {}\
      \\![*]改行で一拍おく(ゴースト別)\\n    {}\
      \\![*]終了時に読み上げが終わるのを待つ(共通)\\n    {}\
      \\![*]デフォルト声質(共通)\\n    {}\
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
    wait_for_speech_changer,
    default_voice_info,
  );

  new_response_with_script(m.to_string(), true)
}

fn chara_info(
  characters: &[String],
  ghost_name: &String,
  index: usize,
  ghost_path: &String,
) -> String {
  let voice = get_voice_from_ghost(ghost_name, index);
  let index_tag = if index < 2 {
    format!("\\\\{}", index)
  } else {
    format!("\\\\p[{}]", index)
  };

  let character_name = if let Some(c) = characters.get(index) {
    format!(" {}:", c)
  } else {
    "".to_string()
  };

  format!(
    "{}{}\\n    \\__q[OnVoiceSelecting,{},{},{},{}]{}\\__q\\n",
    index_tag,
    character_name,
    ghost_name,
    characters.get(index).unwrap_or(&String::from("")),
    index,
    ghost_path,
    decorated(&voice, "bold"),
  )
}

fn get_voice_from_ghost(ghost_name: &String, index: usize) -> String {
  if let Some(si) = get_global_vars()
    .ghosts_voices
    .as_ref()
    .unwrap()
    .get(ghost_name)
  {
    if let Some(c) = si.voices.get(index) {
      return get_voice(c);
    }
  };
  UNSET_VOICE.to_string()
}

fn get_voice(c: &Option<CharacterVoice>) -> String {
  let c = match c {
    Some(c) => c,
    None => return UNSET_VOICE.to_string(),
  };
  let mut voice = String::from(DEFAULT_VOICE);
  if c.speaker_uuid == DUMMY_VOICE_UUID {
    voice = NO_VOICE.to_string();
  } else if let Some(speakers_by_engine) = get_global_vars()
    .volatility
    .speakers_info
    .get(&(engine_from_port(c.port).unwrap()))
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
    voice = grayed(&format!(
      "【使用不可: {}の起動が必要】",
      engine_from_port(c.port).unwrap().name()
    ));
  }
  voice
}

type ListCallback = Box<dyn Fn(&Engine, &SpeakerInfo, &Style) -> String>;
type DummyCallback = Box<dyn Fn(String, &CharacterVoice) -> String>;

fn list_available_voices(callbacks: (ListCallback, DummyCallback)) -> String {
  let def = CharacterVoice::dummy();
  let mut m = "\\b[2]".to_string();
  m.push_str(callbacks.1(NO_VOICE.to_string(), &def).as_str());
  let speakers_info = &get_global_vars().volatility.speakers_info;
  for (engine, speakers) in speakers_info.iter() {
    for speaker in speakers.iter() {
      for style in speaker.styles.iter() {
        m.push_str(callbacks.0(engine, speaker, style).as_str());
      }
    }
  }
  m
}

fn list_callback_for_characters(
  ghost_name: String,
  character_index: usize,
  ghost_path: String,
) -> (ListCallback, DummyCallback) {
  let gn = ghost_name.clone();
  let gp = ghost_path.clone();
  let list_callback = Box::new(
    move |engine: &Engine, speaker: &SpeakerInfo, style: &Style| {
      format!(
        "\\![*]\\q[{} | {},OnVoiceSelected,{},{},{},{},{},{}]\\n",
        speaker.speaker_name,
        style.style_name.as_ref().unwrap(),
        ghost_name,
        character_index,
        engine.port(),
        speaker.speaker_uuid,
        style.style_id.unwrap(),
        ghost_path,
      )
    },
  );
  let dummy_callback = Box::new(move |voice: String, c: &CharacterVoice| {
    format!(
      "\\![*]\\q[{},OnVoiceSelected,{},{},{},{},{},{}]\\n",
      voice, gn, character_index, c.port, c.speaker_uuid, c.style_id, gp,
    )
  });
  (list_callback, dummy_callback)
}

pub fn on_voice_selecting(req: &PluginRequest) -> PluginResponse {
  let refs = get_references(req);
  let ghost_name = refs.first().unwrap();
  let character_name = refs.get(1).unwrap();
  let character_index = refs.get(2).unwrap().parse::<usize>().unwrap();
  let ghost_path = refs.get(3).unwrap();

  let callback = list_callback_for_characters(
    ghost_name.to_string(),
    character_index,
    ghost_path.to_string(),
  );
  let mut m = format!("\\C\\c\\b[2]\\_q{}\\n{}\\n", ghost_name, character_name);
  m.push_str(list_available_voices(callback).as_str());
  m.push_str("\\n\\q[×,]");
  new_response_with_script(m.to_string(), true)
}

pub fn on_voice_selected(req: &PluginRequest) -> PluginResponse {
  let refs = get_references(req);
  let ghost_name = refs.first().unwrap();
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
    voices.insert(character_index, Some(voice))
  } else {
    // OnGhostBootで設定されているはず
    panic!("Ghost {} not found", ghost_name);
  }
  let script = format!(
    "\\![raiseplugin,{},OnMenuExec,dummy,{},dummy,dummy,{}]",
    get_global_vars().volatility.plugin_uuid,
    ghost_name,
    ghost_path.replace('\\', "\\\\")
  );
  new_response_with_script(script, false)
}

pub fn on_volume_change(req: &PluginRequest) -> PluginResponse {
  let refs = get_references(req);
  let volume: f32 = refs.first().unwrap().parse().unwrap();
  let vars = get_global_vars();
  let v = vars.volume.unwrap_or(1.0);
  vars.volume = Some(v + volume);
  let script = format!(
    "\\![raiseplugin,{},OnMenuExec,dummy,{},dummy,dummy,{}]",
    vars.volatility.plugin_uuid, refs[1], refs[2]
  );
  new_response_with_script(script, false)
}

fn list_callback_for_defaultvoices(
  ghost_name: String,
  ghost_path: String,
) -> (ListCallback, DummyCallback) {
  let gn = ghost_name.clone();
  let gp = ghost_path.clone();
  let list_callback = Box::new(
    move |engine: &Engine, speaker: &SpeakerInfo, style: &Style| {
      format!(
        "\\![*]\\q[{} | {},OnDefaultVoiceSelected,{},{},{},{},{}]\\n",
        speaker.speaker_name,
        style.style_name.as_ref().unwrap(),
        engine.port(),
        speaker.speaker_uuid,
        style.style_id.unwrap(),
        ghost_name,
        ghost_path,
      )
    },
  );
  let dummy_callback = Box::new(move |voice: String, c: &CharacterVoice| {
    format!(
      "\\![*]\\q[{},OnDefaultVoiceSelected,{},{},{},{},{}]\\n",
      voice, c.port, c.speaker_uuid, c.style_id, gn, gp,
    )
  });
  (list_callback, dummy_callback)
}

pub fn on_default_voice_selecting(req: &PluginRequest) -> PluginResponse {
  let refs = get_references(req);
  let ghost_name = refs.first().unwrap();
  let ghost_path = refs.get(1).unwrap();
  let callback = list_callback_for_defaultvoices(ghost_name.to_string(), ghost_path.to_string());
  let mut m = "\\_qデフォルトボイスの設定\\n".to_string();
  m.push_str(list_available_voices(callback).as_str());
  m.push_str("\\n\\q[×,]");
  new_response_with_script(m.to_string(), true)
}

pub fn on_default_voice_selected(req: &PluginRequest) -> PluginResponse {
  let refs = get_references(req);
  let port = refs.first().unwrap();
  let speaker_uuid = refs.get(1).unwrap();
  let style_id = refs.get(2).unwrap();
  let ghost_name = refs.get(3).unwrap();
  let ghost_path = refs.get(4).unwrap();
  let path_for_arg = ghost_path.replace('\\', "\\\\");

  let voice = CharacterVoice {
    port: port.to_string().parse::<i32>().unwrap(),
    speaker_uuid: speaker_uuid.to_string(),
    style_id: style_id.to_string().parse::<i32>().unwrap(),
  };

  get_global_vars().initial_voice = voice;
  let script = format!(
    "\\![raiseplugin,{},OnMenuExec,dummy,{},dummy,dummy,{}]",
    get_global_vars().volatility.plugin_uuid,
    ghost_name,
    path_for_arg
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

pub fn on_character_resized(req: &PluginRequest) -> PluginResponse {
  let refs = get_references(req);
  let ghost_name = refs[0].to_string();
  let ghost_path = refs[1].to_string();
  let mode: usize = refs[2].parse().unwrap();
  let description_characters = count_characters(load_descript(ghost_path.clone()));
  let characters = get_global_vars()
    .ghosts_voices
    .as_mut()
    .unwrap()
    .get(&ghost_name)
    .unwrap()
    .voices
    .clone();
  let mut new_characters = Vec::new();
  match CharacterResizeMode::from_usize(mode) {
    CharacterResizeMode::Expand => {
      for c in characters.iter() {
        new_characters.push(c.clone());
      }
      new_characters.push(None);
    }
    CharacterResizeMode::Shrink => {
      if characters.len() > description_characters.len() {
        for c in characters.iter().take(characters.len() - 1) {
          new_characters.push(c.clone());
        }
      } else {
        for c in characters.iter() {
          new_characters.push(c.clone());
        }
      }
    }
  }
  get_global_vars()
    .ghosts_voices
    .as_mut()
    .unwrap()
    .get_mut(&ghost_name)
    .unwrap()
    .voices = new_characters;

  let script = format!(
    "\\![raiseplugin,{},OnMenuExec,dummy,{},dummy,dummy,{}]",
    get_global_vars().volatility.plugin_uuid,
    ghost_name,
    ghost_path
  );
  new_response_with_script(script, false)
}

pub fn on_player_setting_toggled(req: &PluginRequest) -> PluginResponse {
  let refs = get_references(req);
  let ghost_name = refs[0].to_string();
  let path_for_arg = refs[1].to_string();

  get_global_vars().wait_for_speech = Some(!get_global_vars().wait_for_speech.unwrap());

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
