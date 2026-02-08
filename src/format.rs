use regex::Regex;

pub(crate) struct Dialog {
  pub text: String,
  pub raw_text: String,
  pub scope: usize,
}

pub(crate) fn split_dialog(src: String, devide_by_lines: bool) -> Vec<Dialog> {
  let mut s = src.clone();
  s = delete_quick_section(s);

  let lines_re = Regex::new(r"(\\n(\[[^\]]+\])?)+").unwrap();
  if devide_by_lines {
    s = lines_re.replace_all(&s, "$0。").to_string();
  }

  let mut raws = split_dialog_local(s);
  for r in raws.iter_mut() {
    // raw_text は split_dialog_local で既に text と同値に設定済み
    r.text = clear_tags(r.text.clone());
  }

  let mut result = Vec::new();
  for r in raws {
    if r.text.is_empty() {
      continue;
    }
    for text in r.text.split('\u{0}') {
      if text.is_empty() {
        continue;
      }
      result.push(Dialog {
        text: text.to_string(),
        raw_text: r.raw_text.clone(),
        scope: r.scope,
      });
    }
  }
  result
}

pub(crate) fn split_by_punctuation(src: String) -> Vec<String> {
  let delims_re = Regex::new(r"[！!?？。]").unwrap();

  let t = delims_re.replace_all(&src, "$0\u{0}").to_string();
  let mut result = Vec::new();
  for text in t.split('\u{0}') {
    if text.is_empty() {
      continue;
    }
    result.push(text.to_string());
  }
  result
}

pub(crate) fn split_by_punctuation_with_raw(
  clean: String,
  raw: String,
) -> Vec<(String, String)> {
  let clean_segments = split_by_punctuation(clean);
  if clean_segments.len() <= 1 {
    return vec![(
      clean_segments.into_iter().next().unwrap_or_default(),
      raw,
    )];
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
    for _ in clean_seg.chars() {
      // タグをスキップ
      while tag_idx < tag_ranges.len() && tag_ranges[tag_idx].0 == raw_pos {
        raw_pos = tag_ranges[tag_idx].1;
        tag_idx += 1;
      }
      // テキスト文字を消費
      if raw_pos < raw_bytes.len() {
        let ch_len = utf8_char_len(raw_bytes[raw_pos]);
        raw_pos += ch_len;
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

pub(crate) fn scope_to_tag(scope: usize) -> String {
  match scope {
    0 => "\\0".to_string(),
    1 => "\\1".to_string(),
    n => format!("\\p[{}]", n),
  }
}

fn split_dialog_local(src: String) -> Vec<Dialog> {
  let change_scope_re = Regex::new(r"\\([0h1u])|\\p\[([0-9]+)\]").unwrap();
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
      scope = char.parse().unwrap();
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

fn sakura_script_regex() -> Regex {
  Regex::new(r###"\\_{0,2}(w[1-9]|[a-zA-Z0-9*!&\-+](\[("([^"]|\\")+?"|([^\]]|\\\])+?)+?\])?)"###)
    .unwrap()
}

fn clear_tags(src: String) -> String {
  let sakura_script_re = sakura_script_regex();
  sakura_script_re.replace_all(&src.clone(), "").to_string()
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
        println!("add: {}", part);
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
