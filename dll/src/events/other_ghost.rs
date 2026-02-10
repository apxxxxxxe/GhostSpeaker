use crate::events::common::*;
use crate::ipc::send_command_logged;
use crate::plugin::request::PluginRequest;
use crate::plugin::response::PluginResponse;
use crate::variables::*;
use ghost_speaker_common::{Command, GhostVoiceInfo, Response, SyncState};

pub(crate) fn on_other_ghost_talk(req: &PluginRequest) -> PluginResponse {
  let refs = get_references(req);
  let ghost_name = refs[0].to_string();
  let flags = refs[2].to_string();
  let msg = refs[4].to_string();

  if msg.is_empty() || flags.contains("plugin-script") {
    return new_response_nocontent();
  }

  let sync_enabled = match GHOSTS_VOICES.read() {
    Ok(gv) => gv
      .get(&ghost_name)
      .map(|info| info.sync_speech_to_balloon)
      .unwrap_or(false),
    Err(e) => {
      error!("Failed to read GHOSTS_VOICES: {}", e);
      false
    }
  };

  if !sync_enabled {
    debug!("pushing to prediction via IPC");
    send_command_logged(&Command::SpeakAsync {
      text: msg,
      ghost_name,
    });
    debug!("pushed to prediction");
    return new_response_nocontent();
  }

  // 同期モード: SyncStart を送信
  let resp = match send_command_logged(&Command::SyncStart {
    text: msg,
    ghost_name: ghost_name.clone(),
  }) {
    Some(r) => r,
    None => return new_response_nocontent(),
  };

  match resp {
    Response::SyncStarted {
      first_segment: Some(seg),
      has_more,
    } => {
      let tag = scope_to_tag(seg.scope);
      let script = if has_more {
        format!(
          "{}{}\\![raiseplugin,{},OnSyncSpeechContinue,{}]",
          tag, seg.raw_text, PLUGIN_UUID, ghost_name,
        )
      } else {
        format!("{}{}", tag, seg.raw_text)
      };
      new_response_with_script(script, false)
    }
    Response::SyncStarted {
      first_segment: None,
      ..
    } => new_response_nocontent(),
    _ => {
      error!("Unexpected SyncStart response: {:?}", resp);
      new_response_nocontent()
    }
  }
}

pub(crate) fn on_sync_speech_continue(req: &PluginRequest) -> PluginResponse {
  let refs = get_references(req);
  let ghost_name = refs[0].to_string();

  let resp = match send_command_logged(&Command::SyncPoll) {
    Some(r) => r,
    None => return new_response_nocontent(),
  };

  match resp {
    Response::SyncStatus { state } => match state {
      SyncState::Playing | SyncState::Waiting => {
        // まだ再生中 or 合成待ち → 200ms後にリトライ
        let script = format!(
          "\\C\\_w[200]\\![raiseplugin,{},OnSyncSpeechContinue,{}]",
          PLUGIN_UUID, ghost_name,
        );
        new_response_with_script(script, false)
      }
      SyncState::Ready {
        segment: seg,
        has_more,
      } => {
        let tag = scope_to_tag(seg.scope);
        let script = if has_more {
          format!(
            "\\C{}{}\\![raiseplugin,{},OnSyncSpeechContinue,{}]",
            tag, seg.raw_text, PLUGIN_UUID, ghost_name,
          )
        } else {
          format!("\\C{}{}", tag, seg.raw_text)
        };
        new_response_with_script(script, false)
      }
      SyncState::Complete => new_response_nocontent(),
    },
    _ => {
      error!("Unexpected SyncPoll response: {:?}", resp);
      new_response_nocontent()
    }
  }
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
