use once_cell::sync::Lazy;
use regex::Regex;

static LINES_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(\\n(\[[^\]]+\])?)+").unwrap());
static DELIMS_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"[！!?？。]").unwrap());
static ELLIPSIS_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"[…]+|・{2,}|\.{2,}").unwrap());
static ELLIPSIS_FULL_RE: Lazy<Regex> =
  Lazy::new(|| Regex::new(r"^(?:[…]+|・{2,}|\.{2,})$").unwrap());
static CHANGE_SCOPE_RE: Lazy<Regex> =
  Lazy::new(|| Regex::new(r"\\([0h1u])|\\p\[([0-9]+)\]").unwrap());
static SAKURA_SCRIPT_RE: Lazy<Regex> = Lazy::new(|| {
  Regex::new(r###"\\_{0,2}(w[1-9]|[a-zA-Z0-9*!&\-+](\[("([^"]|\\")+?"|([^\]]|\\\])+?)+?\])?)"###)
    .unwrap()
});
static QUICK_SECTION_TAG_RE: Lazy<Regex> =
  Lazy::new(|| Regex::new(r"(\\_q|\\!\[quicksection,(0|1|true|false)\])").unwrap());

pub struct Dialog {
  pub text: String,
  pub raw_text: String,
  pub scope: usize,
}

pub fn split_dialog(src: String, devide_by_lines: bool) -> Vec<Dialog> {
  let lines_re = &*LINES_RE;

  // raw_text 用: \_qタグだけ除去しテキスト内容は保持、。挿入前
  let raw_dialogs = split_dialog_local(strip_quick_section_tags_only(src.clone()));

  let mut s = delete_quick_section(src);

  if devide_by_lines {
    // \0（null文字）を行区切りマーカーとして使用する。
    // 以前は「。」を挿入していたが、raw_textには「。」が存在しないため、
    // split_by_punctuation_with_rawの文字アライメントが破綻していた。
    // \0はsplit_by_punctuationで自然に分割ポイントとして扱われ、
    // raw_textとのアライメントに影響しない。
    s = lines_re.replace_all(&s, "$0\u{0}").to_string();
  }

  let mut raws = split_dialog_local(s);
  for (i, r) in raws.iter_mut().enumerate() {
    // raw_text には。挿入前のテキストを使用
    if i < raw_dialogs.len() {
      r.raw_text = raw_dialogs[i].raw_text.clone();
    }
    r.text = clear_tags(r.text.clone());
  }

  let mut result = Vec::new();
  let mut accumulated_prefix = String::new();
  for r in raws {
    if r.text.is_empty() {
      // scopeタグ + raw_textを累積して次の非空Dialogに引き継ぐ
      accumulated_prefix.push_str(&scope_to_tag(r.scope));
      accumulated_prefix.push_str(&r.raw_text);
      continue;
    }
    let raw_text = if accumulated_prefix.is_empty() {
      r.raw_text.clone()
    } else {
      let mut full = std::mem::take(&mut accumulated_prefix);
      full.push_str(&scope_to_tag(r.scope));
      full.push_str(&r.raw_text);
      full
    };
    // \0はsplit_by_punctuationで分割されるため、ここでは分割しない。
    // 以前は\0で分割していたが、各サブDialogが同じraw_textを共有してしまい、
    // split_by_punctuation_with_rawのアライメントが不正になっていた。
    if !r.text.is_empty() {
      result.push(Dialog {
        text: r.text.clone(),
        raw_text,
        scope: r.scope,
      });
    }
  }
  result
}

/// テキストを正規表現で分割する。
/// マッチ部分が先頭にある場合は独立セグメントとし、
/// 前にテキストがある場合は前テキストと結合する。
/// 例: split_keeping_delimiters("あ……い", /[…]+/) => ["あ……", "い"]
/// 例: split_keeping_delimiters("……い", /[…]+/) => ["……", "い"]
fn split_keeping_delimiters(text: &str, re: &Regex) -> Vec<String> {
  let mut result = Vec::new();
  let mut last_end = 0;
  for m in re.find_iter(text) {
    if m.start() > last_end {
      // 前テキストがある場合: 前テキスト+マッチを結合
      result.push(text[last_end..m.end()].to_string());
    } else {
      // 先頭の省略記号: 独立セグメント
      result.push(m.as_str().to_string());
    }
    last_end = m.end();
  }
  if last_end < text.len() {
    result.push(text[last_end..].to_string());
  }
  result
}

/// テキストが省略記号のみで構成されているか判定する
pub fn is_ellipsis_segment(text: &str) -> bool {
  if text.is_empty() {
    return false;
  }
  ELLIPSIS_FULL_RE.is_match(text)
}

/// 同期モード用: \_q内の省略記号をraw_textベースで再分割する。
/// clean textに省略記号がないがraw_textのクリーンテキストに省略記号がある場合、
/// raw_textベースで再分割する。
/// 再分割後、各セグメントのtextフィールドには元のclean text(t)に含まれる
/// 文字だけを使用し、quicksection由来の文字はTTSに送らない。
pub fn resplit_pairs_by_raw_ellipsis(pairs: Vec<(String, String)>) -> Vec<(String, String)> {
  let mut result = Vec::new();
  for (t, rt) in pairs {
    let rt_clean = clear_tags(rt.clone());
    if !t.is_empty() && !ELLIPSIS_RE.is_match(&t) && ELLIPSIS_RE.is_match(&rt_clean) {
      // \_q内の省略記号: raw-cleanベースで再分割
      let new_pairs = split_by_punctuation_with_raw(rt_clean.clone(), rt);

      // rt_cleanの各文字が元のtに含まれるかをマッピング
      // tはrt_cleanの部分列（quicksection内容除去後）なので、
      // 貪欲マッチングで対応関係を特定できる
      let rt_clean_chars: Vec<char> = rt_clean.chars().collect();
      let t_chars: Vec<char> = t.chars().collect();
      let mut is_original = vec![false; rt_clean_chars.len()];
      let mut t_idx = 0;
      for (i, &c) in rt_clean_chars.iter().enumerate() {
        if t_idx < t_chars.len() && c == t_chars[t_idx] {
          is_original[i] = true;
          t_idx += 1;
        }
      }

      // 各新ペアについて、元のtに対応する文字だけをtext fieldに使う
      let mut rt_clean_pos = 0;
      for (seg_text, seg_raw) in new_pairs {
        let seg_char_count = seg_text.chars().count();
        if is_ellipsis_segment(&seg_text) {
          rt_clean_pos += seg_char_count;
          result.push((seg_text, seg_raw));
        } else {
          let mut clean_text = String::new();
          for i in 0..seg_char_count {
            let idx = rt_clean_pos + i;
            if idx < is_original.len() && is_original[idx] {
              clean_text.push(rt_clean_chars[idx]);
            }
          }
          rt_clean_pos += seg_char_count;
          result.push((clean_text, seg_raw));
        }
      }
    } else {
      result.push((t, rt));
    }
  }
  result
}

pub fn split_by_punctuation(src: String) -> Vec<String> {
  let t = DELIMS_RE.replace_all(&src, "$0\u{0}").to_string();
  let mut result = Vec::new();
  for text in t.split('\u{0}') {
    if text.is_empty() {
      continue;
    }
    // 省略記号で追加分割（省略記号自体を独立セグメントとして保持）
    for s in split_keeping_delimiters(text, &ELLIPSIS_RE) {
      if !s.is_empty() {
        result.push(s);
      }
    }
  }
  result
}

pub fn split_by_punctuation_with_raw(clean: String, raw: String) -> Vec<(String, String)> {
  let clean_segments = split_by_punctuation(clean);
  if clean_segments.len() <= 1 {
    return vec![(clean_segments.into_iter().next().unwrap_or_default(), raw)];
  }

  let tag_re = sakura_script_regex();
  let tag_ranges: Vec<(usize, usize)> = tag_re
    .find_iter(&raw)
    .map(|m| (m.start(), m.end()))
    .collect();

  let raw_bytes = raw.as_bytes();
  let mut raw_pos: usize = 0;
  let mut tag_idx: usize = 0;
  let mut result: Vec<(String, String)> = Vec::new();

  for clean_seg in &clean_segments {
    let raw_start = raw_pos;
    for c in clean_seg.chars() {
      // タグをスキップ
      while tag_idx < tag_ranges.len() && tag_ranges[tag_idx].0 == raw_pos {
        raw_pos = tag_ranges[tag_idx].1;
        tag_idx += 1;
      }
      // テキスト文字を消費 - rawの文字と一致する場合のみ
      if raw_pos < raw_bytes.len() {
        let ch_len = utf8_char_len(raw_bytes[raw_pos]);
        let end = (raw_pos + ch_len).min(raw_bytes.len());
        let raw_char = std::str::from_utf8(&raw_bytes[raw_pos..end])
          .ok()
          .and_then(|s| s.chars().next());
        if raw_char == Some(c) {
          raw_pos += ch_len;
        } else {
          // raw に余分な文字がある可能性 -> 前方スキャンでマッチを探す
          let mut scan_pos = raw_pos;
          let mut scan_tag_idx = tag_idx;
          let mut found = false;
          while scan_pos < raw_bytes.len() {
            // タグをスキップ
            while scan_tag_idx < tag_ranges.len() && tag_ranges[scan_tag_idx].0 == scan_pos {
              scan_pos = tag_ranges[scan_tag_idx].1;
              scan_tag_idx += 1;
            }
            if scan_pos >= raw_bytes.len() {
              break;
            }
            let scan_ch_len = utf8_char_len(raw_bytes[scan_pos]);
            let scan_end = (scan_pos + scan_ch_len).min(raw_bytes.len());
            let scan_char = std::str::from_utf8(&raw_bytes[scan_pos..scan_end])
              .ok()
              .and_then(|s| s.chars().next());
            if scan_char == Some(c) {
              // マッチ発見: raw_pos を進めて文字を消費
              raw_pos = scan_pos + scan_ch_len;
              tag_idx = scan_tag_idx;
              found = true;
              break;
            }
            scan_pos += scan_ch_len;
          }
          if !found {
            // 見つからない場合: c は clean にのみ存在する人工文字
          }
        }
      }
    }
    // 次のテキスト文字の直前までのタグも含める
    while tag_idx < tag_ranges.len() && tag_ranges[tag_idx].0 == raw_pos {
      raw_pos = tag_ranges[tag_idx].1;
      tag_idx += 1;
    }
    result.push((clean_seg.clone(), raw[raw_start..raw_pos].to_string()));
  }

  // 末尾の残り（末尾タグ等）を最後のセグメントに追加
  if raw_pos < raw.len() {
    if let Some(last) = result.last_mut() {
      last.1.push_str(&raw[raw_pos..]);
    }
  }

  result
}

fn utf8_char_len(first_byte: u8) -> usize {
  if first_byte & 0x80 == 0 {
    1
  } else if first_byte & 0xE0 == 0xC0 {
    2
  } else if first_byte & 0xF0 == 0xE0 {
    3
  } else {
    4
  }
}

pub fn scope_to_tag(scope: usize) -> String {
  match scope {
    0 => "\\0".to_string(),
    1 => "\\1".to_string(),
    n => format!("\\p[{}]", n),
  }
}

fn split_dialog_local(src: String) -> Vec<Dialog> {
  let change_scope_re = &*CHANGE_SCOPE_RE;
  let mut result = Vec::new();

  if src.is_empty() {
    return result;
  }
  let s = format!("\\0{}", src);

  let sep = change_scope_re.split(&s).collect::<Vec<&str>>();
  let submatch_iter = change_scope_re.captures_iter(&s);
  for (i, cap) in submatch_iter.enumerate() {
    let sakura_kero = cap.get(1).map(|m| m.as_str());
    let char = cap.get(2).map(|m| m.as_str());
    let scope: usize;
    if let Some(sakura_kero) = sakura_kero {
      scope = match sakura_kero {
        "0" => 0,
        "h" => 0,
        "1" => 1,
        "u" => 1,
        _ => unreachable!(),
      };
    } else if let Some(char) = char {
      scope = char.parse().unwrap_or(0);
    } else {
      unreachable!();
    }

    result.push(Dialog {
      text: sep[i + 1].to_string(),
      raw_text: sep[i + 1].to_string(),
      scope,
    });
  }

  result
}

fn sakura_script_regex() -> &'static Regex {
  &SAKURA_SCRIPT_RE
}

fn clear_tags(src: String) -> String {
  let sakura_script_re = sakura_script_regex();
  sakura_script_re.replace_all(&src, "").to_string()
}

fn delete_quick_section(src: String) -> String {
  const NOT_FOUND_INDEX: usize = 10000;

  // 最も早く見つかったクイックセクションの開始位置を取得
  // (index, tag_length)
  let get_start_point = |src: &str| -> Option<(usize, usize)> {
    let tags = vec!["\\![quicksection,1]", "\\![quicksection,true]", "\\_q"];
    let mut indexes: Vec<(usize, usize)> = vec![];
    for tag in tags {
      if let Some(index) = src.find(tag) {
        indexes.push((index, tag.len()));
      };
    }
    let mut min_index = (NOT_FOUND_INDEX, 0);
    for idx in indexes {
      if idx.0 < min_index.0 {
        min_index = idx;
      }
    }
    if min_index.0 == NOT_FOUND_INDEX {
      None
    } else {
      Some(min_index)
    }
  };

  // 最も早く見つかったクイックセクションの終了位置を取得
  let get_end_point = |src: &str| -> Option<(usize, usize)> {
    let tags = vec!["\\![quicksection,0]", "\\![quicksection,false]", "\\_q"];
    let mut indexes: Vec<(usize, usize)> = vec![];
    for tag in tags {
      if let Some(index) = src.find(tag) {
        indexes.push((index, tag.len()));
      };
    }
    let mut min_index = (NOT_FOUND_INDEX, 0);
    for idx in indexes {
      if idx.0 < min_index.0 {
        min_index = idx;
      }
    }
    if min_index.0 == NOT_FOUND_INDEX {
      None
    } else {
      Some(min_index)
    }
  };

  let mut s = src.clone();
  let mut result = String::new();
  let mut is_quicksection = false;
  loop {
    if !is_quicksection {
      if let Some(start_point) = get_start_point(&s) {
        let part = &s[..start_point.0];
        result.push_str(part);
        s = s[start_point.0 + start_point.1..].to_string();
        is_quicksection = true;
      } else {
        result.push_str(&s);
        break;
      }
    } else if let Some(end_point) = get_end_point(&s) {
      s = s[end_point.0 + end_point.1..].to_string();
      is_quicksection = false;
    } else {
      break;
    }
  }
  result
}

/// クイックセクションのタグだけを除去し、タグ間のテキスト内容は保持する。
/// 入力: "Hello\_q...\_qWorld" -> 出力: "Hello...World"
/// (delete_quick_section はタグもテキストも両方削除する)
fn strip_quick_section_tags_only(src: String) -> String {
  QUICK_SECTION_TAG_RE.replace_all(&src, "").to_string()
}
