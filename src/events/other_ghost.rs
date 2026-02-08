use crate::events::common::*;
use crate::format::scope_to_tag;
use crate::plugin::request::PluginRequest;
use crate::plugin::response::PluginResponse;
use crate::queue::{
  build_segments, cancel_sync_playback, is_sync_audio_done, pop_ready_segment,
  push_to_prediction, spawn_sync_playback, spawn_sync_prediction, sync_predict,
  SYNC_STATE,
};
use crate::variables::*;

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

  let mut segments = segments;
  let first = segments.remove(0);

  // 最初のセグメント: 合成 → 再生開始
  let wav = sync_predict(&*first.predictor).unwrap_or_default();
  spawn_sync_playback(wav);

  // 残りセグメントのバックグラウンド合成を開始
  spawn_sync_prediction(segments, ghost_name.clone());

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

  // 音声がまだ再生中 → ポーリング(200ms後にリトライ)
  if !is_sync_audio_done() {
    let script = format!(
      "\\C\\_w[200]\\![raiseplugin,{},OnSyncSpeechContinue,{}]",
      PLUGIN_UUID, ghost_name,
    );
    return new_response_with_script(script, false);
  }

  // プールから合成済みセグメントを取得
  let (segment, has_more) = pop_ready_segment(&ghost_name);

  match segment {
    Some(seg) => {
      // 再生開始
      spawn_sync_playback(seg.wav);

      let script = if has_more {
        format!(
          "\\C{}{}\\![raiseplugin,{},OnSyncSpeechContinue,{}]",
          scope_to_tag(seg.scope),
          seg.text,
          PLUGIN_UUID,
          ghost_name,
        )
      } else {
        // 最後のセグメント → チェーン終了、ステートクリア
        *SYNC_STATE.lock().unwrap() = None;
        format!("\\C{}{}", scope_to_tag(seg.scope), seg.text)
      };
      new_response_with_script(script, false)
    }
    None => {
      if has_more {
        // まだ合成中 → ポーリング継続
        let script = format!(
          "\\C\\_w[200]\\![raiseplugin,{},OnSyncSpeechContinue,{}]",
          PLUGIN_UUID, ghost_name,
        );
        new_response_with_script(script, false)
      } else {
        // 異常系: セグメントなし & 合成完了 → ステートクリアしてNoContent
        *SYNC_STATE.lock().unwrap() = None;
        new_response_nocontent()
      }
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
