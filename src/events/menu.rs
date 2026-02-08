use crate::engine::{engine_from_port, CharacterVoice, Engine, ENGINE_LIST, NO_VOICE_UUID};
use crate::events::common::load_descript;
use crate::events::common::*;
use crate::plugin::request::PluginRequest;
use crate::plugin::response::PluginResponse;
use crate::speaker::{SpeakerInfo, Style};
use crate::variables::*;
use crate::variables::{PLUGIN_NAME, PLUGIN_UUID};
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
  fn from_usize(n: usize) -> Result<Self, String> {
    match n {
      0 => Ok(Self::Expand),
      1 => Ok(Self::Shrink),
      _ => Err(format!("Invalid mode: {}", n)),
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

pub(crate) fn on_menu_exec(req: &PluginRequest) -> PluginResponse {
  let mut characters_info = String::new();
  let mut division_setting = String::from("-");

  let refs = get_references(req);
  let ghost_name = match refs.get(1) {
    Some(name) => name.to_string(),
    None => {
      error!("Missing ghost name in references");
      return new_response_with_script(String::new(), false);
    }
  };
  let ghost_description = match refs.get(4) {
    Some(path) => load_descript(path.to_string()),
    None => {
      error!("Missing ghost path in references");
      HashMap::new()
    }
  };
  let characters = count_characters(ghost_description);
  let path_for_arg = refs[4].to_string().replace('\\', "\\\\");
  debug!("getting ghosts_voices");
  let ghosts_voices = match GHOSTS_VOICES.read() {
    Ok(gv) => gv,
    Err(e) => {
      error!("Failed to read GHOSTS_VOICES: {}", e);
      return new_response_with_script(String::new(), false);
    }
  };
  let character_voices = match ghosts_voices.get(&ghost_name) {
    Some(info) => &info.voices,
    None => {
      error!("Ghost not found in GHOSTS_VOICES: {}", ghost_name);
      return new_response_with_script(String::new(), false);
    }
  };
  debug!("success to get ghosts_voices");

  debug!("getting speakers_info");
  let speakers_info =
    &mut futures::executor::block_on(async { SPEAKERS_INFO.read().await.clone() });
  debug!("success to get speakers_info");

  if let Some(si) = ghosts_voices.get(&ghost_name) {
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

  let mut sync_balloon_setting = String::from("-");
  if let Some(si) = ghosts_voices.get(&ghost_name) {
    let switch = if si.sync_speech_to_balloon {
      ACTIVATED.to_string()
    } else {
      DEACTIVATED.to_string()
    };
    sync_balloon_setting = format!(
      "【現在 \\__q[OnSyncBalloonSettingChanged,{},{}]{}\\__q】\\n",
      ghost_name,
      path_for_arg,
      decorated(&switch, "bold"),
    );
  }

  for i in 0..character_voices.len() {
    characters_info.push_str(&chara_info(
      &characters,
      &ghost_name,
      i,
      &path_for_arg,
      &ghosts_voices,
    ));
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
  let engine_auto_start = futures::executor::block_on(async { ENGINE_AUTO_START.read().await });
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
  let v = match VOLUME.read() {
    Ok(volume) => *volume,
    Err(e) => {
      error!("Failed to read VOLUME: {}", e);
      1.0 // デフォルト音量
    }
  };

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

  let p = match SPEAK_BY_PUNCTUATION.read() {
    Ok(sbp) => *sbp,
    Err(e) => {
      error!("Failed to read SPEAK_BY_PUNCTUATION: {}", e);
      true // デフォルト値
    }
  };
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

  let default_voice_info = format!(
    "【現在 \\__q[OnDefaultVoiceSelecting,{},{}]{}\\__q】\\n",
    ghost_name,
    path_for_arg,
    match INITIAL_VOICE.read() {
      Ok(iv) => get_voice(&Some(iv.clone())),
      Err(e) => {
        error!("Failed to read INITIAL_VOICE: {}", e);
        get_voice(&None) // フォールバック
      }
    },
  );

  let m = format!(
    "\
      \\b[2]\\_q\
      \\f[align,center]\\f[size,12]{} v{}\\f[size,default]\\n\\n[half]\\f[align,left]\
      {}\
      {}\\n\
      {}\\n\
      \\![*]音量調整(共通)\\n    {}\
      \\![*]句読点ごとに読み上げ(共通)\\n    {}\
      \\![*]改行で一拍おく(ゴースト別)\\n    {}\
      \\![*]セリフ表示を読み上げ音声に合わせる(ゴースト別)\\n    {}\
      \\![*]デフォルト声質(共通)\\n    {}\
      \\n\\q[×,]\
      ",
    PLUGIN_NAME,
    env!("CARGO_PKG_VERSION"),
    engine_status,
    ghost_name,
    characters_info,
    volume_changer,
    punctuation_changer,
    division_setting,
    sync_balloon_setting,
    default_voice_info,
  );

  new_response_with_script(m.to_string(), true)
}

fn chara_info(
  characters: &[String],
  ghost_name: &String,
  index: usize,
  ghost_path: &String,
  ghosts_voices: &std::sync::RwLockReadGuard<HashMap<String, GhostVoiceInfo>>,
) -> String {
  let voice = get_voice_from_ghost(ghost_name, index, ghosts_voices);
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

fn get_voice_from_ghost(
  ghost_name: &String,
  index: usize,
  ghosts_voices: &std::sync::RwLockReadGuard<HashMap<String, GhostVoiceInfo>>,
) -> String {
  if let Some(si) = ghosts_voices.get(ghost_name) {
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
  let speakers_info = futures::executor::block_on(async { SPEAKERS_INFO.read().await.clone() });
  if c.speaker_uuid == NO_VOICE_UUID {
    voice = NO_VOICE.to_string();
  } else if let Some(engine) = engine_from_port(c.port) {
    if let Some(speakers_by_engine) = speakers_info.get(&engine) {
    if let Some(speaker) = speakers_by_engine
      .iter()
      .find(|s| s.speaker_uuid == c.speaker_uuid)
    {
      if let Some(style) = speaker
        .styles
        .iter()
        .find(|s| s.style_id.unwrap_or(-1) == c.style_id)
      {
        voice = format!(
          "{} - {}",
          speaker.speaker_name,
          style.style_name.clone().unwrap_or_else(|| "不明なスタイル".to_string()),
        );
      }
    }
    } else {
      voice = grayed(&format!(
        "【使用不可: {}の起動が必要】",
        engine_from_port(c.port).map_or("不明なエンジン", |e| e.name())
      ));
    }
  }
  voice
}

type ListCallback = Box<dyn Fn(&Engine, &SpeakerInfo, &Style) -> String>;
type DummyCallback = Box<dyn Fn(String, &CharacterVoice) -> String>;

fn list_available_voices(callbacks: (ListCallback, DummyCallback)) -> String {
  let def = CharacterVoice::no_voice();
  let mut m = "\\b[2]".to_string();
  m.push_str(callbacks.1(NO_VOICE.to_string(), &def).as_str());
  let speakers_info = futures::executor::block_on(async { SPEAKERS_INFO.read().await.clone() });
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
        style.style_name.as_ref().unwrap_or(&"不明なスタイル".to_string()),
        ghost_name,
        character_index,
        engine.port(),
        speaker.speaker_uuid,
        style.style_id.unwrap_or(-1),
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

pub(crate) fn on_voice_selecting(req: &PluginRequest) -> PluginResponse {
  let refs = get_references(req);
  let ghost_name = match refs.first() {
    Some(name) => name,
    None => {
      error!("Missing ghost_name parameter");
      return new_response_with_script(String::new(), false);
    }
  };
  let character_name = match refs.get(1) {
    Some(name) => name,
    None => {
      error!("Missing character_name parameter");
      return new_response_with_script(String::new(), false);
    }
  };
  let character_index = match refs.get(2) {
    Some(index_str) => match index_str.parse::<usize>() {
      Ok(index) => index,
      Err(e) => {
        error!("Failed to parse character_index: {}", e);
        return new_response_with_script(String::new(), false);
      }
    },
    None => {
      error!("Missing character_index parameter");
      return new_response_with_script(String::new(), false);
    }
  };
  let ghost_path = match refs.get(3) {
    Some(path) => path,
    None => {
      error!("Missing ghost_path parameter");
      return new_response_with_script(String::new(), false);
    }
  };

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

pub(crate) fn on_voice_selected(req: &PluginRequest) -> PluginResponse {
  let refs = get_references(req);
  let ghost_name = match refs.first() {
    Some(name) => name,
    None => {
      error!("Missing ghost_name parameter");
      return new_response_with_script(String::new(), false);
    }
  };
  let character_index = match refs.get(1) {
    Some(index_str) => match index_str.parse::<usize>() {
      Ok(index) => index,
      Err(e) => {
        error!("Failed to parse character_index: {}", e);
        return new_response_with_script(String::new(), false);
      }
    },
    None => {
      error!("Missing character_index parameter");
      return new_response_with_script(String::new(), false);
    }
  };
  let port = match refs.get(2) {
    Some(port) => port,
    None => {
      error!("Missing port parameter");
      return new_response_with_script(String::new(), false);
    }
  };
  let speaker_uuid = match refs.get(3) {
    Some(uuid) => uuid,
    None => {
      error!("Missing speaker_uuid parameter");
      return new_response_with_script(String::new(), false);
    }
  };
  let style_id = match refs.get(4) {
    Some(id) => id,
    None => {
      error!("Missing style_id parameter");
      return new_response_with_script(String::new(), false);
    }
  };
  let ghost_path = match refs.get(5) {
    Some(path) => path,
    None => {
      error!("Missing ghost_path parameter");
      return new_response_with_script(String::new(), false);
    }
  };

  let voice = CharacterVoice {
    port: match port.to_string().parse::<i32>() {
      Ok(p) => p,
      Err(e) => {
        error!("Failed to parse port in on_voice_selected: {}", e);
        return new_response_with_script(String::new(), false);
      }
    },
    speaker_uuid: speaker_uuid.to_string(),
    style_id: match style_id.to_string().parse::<i32>() {
      Ok(id) => id,
      Err(e) => {
        error!("Failed to parse style_id in on_voice_selected: {}", e);
        return new_response_with_script(String::new(), false);
      }
    },
  };

  let mut ghosts_voices = match GHOSTS_VOICES.write() {
    Ok(gv) => gv,
    Err(e) => {
      error!("Failed to write GHOSTS_VOICES: {}", e);
      return new_response_with_script(String::new(), false);
    }
  };
  if let Some(info) = ghosts_voices.get_mut(*ghost_name) {
    let voices = &mut info.voices;
    voices.remove(character_index);
    voices.insert(character_index, Some(voice))
  } else {
    // OnGhostBootで設定されているはず
    error!("Ghost {} not found", ghost_name);
    return new_response_with_script(String::new(), false);
  }
  let script = format!(
    "\\![raiseplugin,{},OnMenuExec,dummy,{},dummy,dummy,{}]",
    PLUGIN_UUID,
    ghost_name,
    ghost_path.replace('\\', "\\\\")
  );
  new_response_with_script(script, false)
}

pub(crate) fn on_volume_change(req: &PluginRequest) -> PluginResponse {
  let refs = get_references(req);
  let volume: f32 = match refs.first() {
    Some(vol_str) => match vol_str.parse() {
      Ok(v) => v,
      Err(e) => {
        error!("Failed to parse volume: {}", e);
        return new_response_with_script(String::new(), false);
      }
    },
    None => {
      error!("Missing volume parameter");
      return new_response_with_script(String::new(), false);
    }
  };
  match VOLUME.write() {
    Ok(mut v) => *v += volume,
    Err(e) => error!("Failed to write VOLUME: {}", e),
  }
  let script = format!(
    "\\![raiseplugin,{},OnMenuExec,dummy,{},dummy,dummy,{}]",
    PLUGIN_UUID, refs[1], refs[2]
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
        style.style_name.as_ref().unwrap_or(&"不明なスタイル".to_string()),
        engine.port(),
        speaker.speaker_uuid,
        style.style_id.unwrap_or(-1),
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

pub(crate) fn on_default_voice_selecting(req: &PluginRequest) -> PluginResponse {
  let refs = get_references(req);
  let ghost_name = match refs.first() {
    Some(name) => name,
    None => {
      error!("Missing ghost_name parameter");
      return new_response_with_script(String::new(), false);
    }
  };
  let ghost_path = match refs.get(1) {
    Some(path) => path,
    None => {
      error!("Missing ghost_path parameter");
      return new_response_with_script(String::new(), false);
    }
  };
  let callback = list_callback_for_defaultvoices(ghost_name.to_string(), ghost_path.to_string());
  let mut m = "\\_qデフォルトボイスの設定\\n".to_string();
  m.push_str(list_available_voices(callback).as_str());
  m.push_str("\\n\\q[×,]");
  new_response_with_script(m.to_string(), true)
}

pub(crate) fn on_default_voice_selected(req: &PluginRequest) -> PluginResponse {
  let refs = get_references(req);
  let port = match refs.first() {
    Some(port) => port,
    None => {
      error!("Missing port parameter");
      return new_response_with_script(String::new(), false);
    }
  };
  let speaker_uuid = match refs.get(1) {
    Some(uuid) => uuid,
    None => {
      error!("Missing speaker_uuid parameter");
      return new_response_with_script(String::new(), false);
    }
  };
  let style_id = match refs.get(2) {
    Some(id) => id,
    None => {
      error!("Missing style_id parameter");
      return new_response_with_script(String::new(), false);
    }
  };
  let ghost_name = match refs.get(3) {
    Some(name) => name,
    None => {
      error!("Missing ghost_name parameter");
      return new_response_with_script(String::new(), false);
    }
  };
  let ghost_path = match refs.get(4) {
    Some(path) => path,
    None => {
      error!("Missing ghost_path parameter");
      return new_response_with_script(String::new(), false);
    }
  };
  let path_for_arg = ghost_path.replace('\\', "\\\\");

  let voice = CharacterVoice {
    port: match port.to_string().parse::<i32>() {
      Ok(p) => p,
      Err(e) => {
        error!("Failed to parse port in on_default_voice_selected: {}", e);
        return new_response_with_script(String::new(), false);
      }
    },
    speaker_uuid: speaker_uuid.to_string(),
    style_id: match style_id.to_string().parse::<i32>() {
      Ok(id) => id,
      Err(e) => {
        error!("Failed to parse style_id in on_default_voice_selected: {}", e);
        return new_response_with_script(String::new(), false);
      }
    },
  };

  match INITIAL_VOICE.write() {
    Ok(mut iv) => *iv = voice,
    Err(e) => error!("Failed to write INITIAL_VOICE: {}", e),
  }
  let script = format!(
    "\\![raiseplugin,{},OnMenuExec,dummy,{},dummy,dummy,{}]",
    PLUGIN_UUID, ghost_name, path_for_arg
  );
  new_response_with_script(script, false)
}

pub(crate) fn on_division_setting_changed(req: &PluginRequest) -> PluginResponse {
  let refs = get_references(req);
  let ghost_name = refs[0].to_string();
  let path_for_arg = refs[1].to_string();
  let mut ghosts_voices = match GHOSTS_VOICES.write() {
    Ok(gv) => gv,
    Err(e) => {
      error!("Failed to write GHOSTS_VOICES: {}", e);
      return new_response_with_script(String::new(), false);
    }
  };
  if let Some(info) = ghosts_voices.get_mut(&ghost_name) {
    info.devide_by_lines = !info.devide_by_lines
  }

  let script = format!(
    "\\![raiseplugin,{},OnMenuExec,dummy,{},dummy,dummy,{}]",
    PLUGIN_UUID, ghost_name, path_for_arg
  );
  new_response_with_script(script, false)
}

pub(crate) fn on_sync_balloon_setting_changed(req: &PluginRequest) -> PluginResponse {
  let refs = get_references(req);
  let ghost_name = refs[0].to_string();
  let path_for_arg = refs[1].to_string();
  let mut ghosts_voices = match GHOSTS_VOICES.write() {
    Ok(gv) => gv,
    Err(e) => {
      error!("Failed to write GHOSTS_VOICES: {}", e);
      return new_response_with_script(String::new(), false);
    }
  };
  if let Some(info) = ghosts_voices.get_mut(&ghost_name) {
    info.sync_speech_to_balloon = !info.sync_speech_to_balloon;
  }

  let script = format!(
    "\\![raiseplugin,{},OnMenuExec,dummy,{},dummy,dummy,{}]",
    PLUGIN_UUID, ghost_name, path_for_arg
  );
  new_response_with_script(script, false)
}

pub(crate) fn on_punctuation_setting_changed(req: &PluginRequest) -> PluginResponse {
  let refs = get_references(req);
  let ghost_name = refs[0].to_string();
  let path_for_arg = refs[1].to_string();
  let mut sbp = match SPEAK_BY_PUNCTUATION.write() {
    Ok(s) => s,
    Err(e) => {
      error!("Failed to write SPEAK_BY_PUNCTUATION: {}", e);
      return new_response_with_script(String::new(), false);
    }
  };
  *sbp = !*sbp;

  let script = format!(
    "\\![raiseplugin,{},OnMenuExec,dummy,{},dummy,dummy,{}]",
    PLUGIN_UUID, ghost_name, path_for_arg
  );
  new_response_with_script(script, false)
}

pub(crate) fn on_auto_start_toggled(req: &PluginRequest) -> PluginResponse {
  let refs = get_references(req);
  let port = match refs[0].parse::<i32>() {
    Ok(p) => p,
    Err(e) => {
      error!("Failed to parse port: {}", e);
      return new_response_with_script(String::new(), false);
    }
  };
  let ghost_name = refs[1].to_string();
  let path_for_arg = refs[2].to_string();

  let engine = match engine_from_port(port) {
    Some(e) => e,
    None => {
      error!("Unknown engine for port: {}", port);
      return new_response_with_script(String::new(), false);
    }
  };
  if let Some(auto_start) =
    futures::executor::block_on(async { ENGINE_AUTO_START.write().await }).get_mut(&engine)
  {
    *auto_start = !*auto_start;
  }

  let script = format!(
    "\\![raiseplugin,{},OnMenuExec,dummy,{},dummy,dummy,{}]",
    PLUGIN_UUID, ghost_name, path_for_arg
  );
  new_response_with_script(script, false)
}

pub(crate) fn on_character_resized(req: &PluginRequest) -> PluginResponse {
  let refs = get_references(req);
  let ghost_name = refs[0].to_string();
  let ghost_path = refs[1].to_string();
  let mode: usize = match refs[2].parse() {
    Ok(m) => m,
    Err(e) => {
      error!("Failed to parse mode: {}", e);
      return new_response_with_script(String::new(), false);
    }
  };
  let description_characters = count_characters(load_descript(ghost_path.clone()));
  let characters = match GHOSTS_VOICES.write() {
    Ok(gv) => match gv.get(&ghost_name) {
      Some(info) => info.voices.clone(),
      None => {
        error!("Ghost {} not found", ghost_name);
        return new_response_with_script(String::new(), false);
      }
    },
    Err(e) => {
      error!("Failed to read GHOSTS_VOICES: {}", e);
      return new_response_with_script(String::new(), false);
    }
  };
  let mut new_characters = Vec::new();
  match CharacterResizeMode::from_usize(mode) {
    Ok(CharacterResizeMode::Expand) => {
      for c in characters.iter() {
        new_characters.push(c.clone());
      }
      new_characters.push(None);
    }
    Ok(CharacterResizeMode::Shrink) => {
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
    Err(e) => {
      error!("Invalid character resize mode: {}", e);
      return new_response_with_script(String::new(), false);
    }
  }
  match GHOSTS_VOICES.write() {
    Ok(mut gv) => match gv.get_mut(&ghost_name) {
      Some(info) => info.voices = new_characters,
      None => {
        error!("Ghost {} not found for character resize", ghost_name);
        return new_response_with_script(String::new(), false);
      }
    },
    Err(e) => {
      error!("Failed to write GHOSTS_VOICES for character resize: {}", e);
      return new_response_with_script(String::new(), false);
    }
  }

  let script = format!(
    "\\![raiseplugin,{},OnMenuExec,dummy,{},dummy,dummy,{}]",
    PLUGIN_UUID, ghost_name, ghost_path
  );
  new_response_with_script(script, false)
}
