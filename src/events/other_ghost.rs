use crate::events::common::*;
use crate::format::scope_to_tag;
use crate::plugin::request::PluginRequest;
use crate::plugin::response::PluginResponse;
use crate::queue::{
  build_segments, cancel_sync_playback, is_sync_audio_done, push_to_prediction,
  spawn_sync_playback, sync_predict, SyncPlaybackState, SYNC_STATE,
};
use crate::variables::*;
use std::collections::VecDeque;

pub(crate) fn on_other_ghost_talk(req: &PluginRequest) -> PluginResponse {
  let refs = get_references(req);
  let ghost_name = refs[0].to_string();
  let flags = refs[2].to_string();
  let msg = refs[4].to_string();

  if msg.is_empty() || flags.contains("plugin-script") {
    return new_response_nocontent();
  }

  // 同期設定チェック
  let sync_enabled = GHOSTS_VOICES
    .read()
    .unwrap()
    .get(&ghost_name)
    .map(|info| info.sync_speech_to_balloon)
    .unwrap_or(false);

  if !sync_enabled {
    debug!("pushing to prediction");
    push_to_prediction(msg, ghost_name);
    debug!("pushed to prediction");
    return new_response_nocontent();
  }

  // 進行中の同期再生をキャンセル
  cancel_sync_playback();

  // セグメント生成
  let segments = match build_segments(msg.clone(), ghost_name.clone(), true) {
    Some(s) if s.len() >= 2 => s,
    _ => {
      // セグメント1つ以下 → 同期不要、通常モード
      push_to_prediction(msg, ghost_name);
      return new_response_nocontent();
    }
  };

  let mut seg_deque: VecDeque<_> = segments.into();
  let first = seg_deque.pop_front().unwrap();

  // 最初のセグメント: 合成 → 再生開始
  let wav = sync_predict(&*first.predictor).unwrap_or_default();
  spawn_sync_playback(wav);

  // 残りをステートに保存
  *SYNC_STATE.lock().unwrap() = Some(SyncPlaybackState {
    segments: seg_deque,
    ghost_name: ghost_name.clone(),
  });

  // スクリプト返却: scopeタグ + テキスト + raiseplugin
  let script = format!(
    "{}{}\\![raiseplugin,{},OnSyncSpeechContinue,{}]",
    scope_to_tag(first.scope),
    first.text,
    PLUGIN_UUID,
    ghost_name,
  );
  new_response_with_script(script, false)
}

pub(crate) fn on_sync_speech_continue(req: &PluginRequest) -> PluginResponse {
  let refs = get_references(req);
  let ghost_name = refs[0].to_string();

  // 音声がまだ再生中 → ポーリング(450ms後にリトライ)
  if !is_sync_audio_done() {
    let script = format!(
      "\\w9\\![raiseplugin,{},OnSyncSpeechContinue,{}]",
      PLUGIN_UUID, ghost_name,
    );
    return new_response_with_script(script, false);
  }

  // 次のセグメントを取得
  let next = {
    let mut state = SYNC_STATE.lock().unwrap();
    match state.as_mut() {
      Some(s) if s.ghost_name == ghost_name => s.segments.pop_front(),
      _ => None,
    }
  };

  match next {
    Some(segment) => {
      // 合成 → 再生開始
      let wav = sync_predict(&*segment.predictor).unwrap_or_default();
      spawn_sync_playback(wav);

      // 残りセグメントがあるか確認
      let has_more = SYNC_STATE
        .lock()
        .unwrap()
        .as_ref()
        .map(|s| !s.segments.is_empty())
        .unwrap_or(false);

      let script = if has_more {
        format!(
          "\\C{}{}\\![raiseplugin,{},OnSyncSpeechContinue,{}]",
          scope_to_tag(segment.scope),
          segment.text,
          PLUGIN_UUID,
          ghost_name,
        )
      } else {
        // 最後のセグメント → チェーン終了、ステートクリア
        *SYNC_STATE.lock().unwrap() = None;
        format!("\\C{}{}", scope_to_tag(segment.scope), segment.text)
      };
      new_response_with_script(script, false)
    }
    None => {
      // 異常系: セグメントなし → ステートクリアしてNoContent
      *SYNC_STATE.lock().unwrap() = None;
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
