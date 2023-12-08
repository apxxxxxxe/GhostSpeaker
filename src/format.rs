use regex::Regex;

pub struct Dialog {
  pub text: String,
  pub scope: usize,
}

pub fn split_dialog(src: String, devide_by_lines: bool) -> Vec<Dialog> {
  let mut s = src.clone();
  s = delete_quick_section(s);

  let lines_re = Regex::new(r"(\\n(\[[^\]]+\])?)+").unwrap();
  if devide_by_lines {
    s = lines_re.replace_all(&s, "$0。").to_string();
  }

  let mut raws = split_dialog_local(s);
  for r in raws.iter_mut() {
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
        scope: r.scope,
      });
    }
  }
  result
}

pub fn split_by_punctuation(src: String) -> Vec<String> {
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
      scope,
    });
  }

  result
}

fn clear_tags(src: String) -> String {
  let sakura_script_re =
    Regex::new(r###"\\_{0,2}[a-zA-Z0-9*!&](\d|\[("([^"]|\\")+?"|([^\]]|\\\])+?)+?\])?"###).unwrap();
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
        result.push_str(&part);
        s = s[start_point.0 + start_point.1..].to_string();
        is_quicksection = true;
      } else {
        result.push_str(&s);
        break;
      }
    } else {
      if let Some(end_point) = get_end_point(&s) {
        s = s[end_point.0 + end_point.1..].to_string();
        is_quicksection = false;
      } else {
        break;
      }
    }
  }
  result
}
