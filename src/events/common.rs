use crate::plugin::request::PluginRequest;
use crate::plugin::response::PluginResponse;
use encoding_rs::{SHIFT_JIS, UTF_8};
use shiorust::message::{parts::HeaderName, parts::*, traits::*};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub fn new_response() -> PluginResponse {
  let mut headers = Headers::new();
  headers.insert(
    HeaderName::Standard(StandardHeaderName::Charset),
    String::from("UTF-8"),
  );
  PluginResponse {
    version: Version::V20,
    status: Status::OK,
    headers,
  }
}

pub fn new_response_nocontent() -> PluginResponse {
  let mut r = new_response();
  r.status = Status::NoContent;
  r
}

pub fn new_response_with_script(script: String, _use_translate: bool) -> PluginResponse {
  let mut r = new_response();
  r.headers.insert(HeaderName::from("Script"), script);
  r
}

pub fn new_response_with_nobreak(script: String, use_translate: bool) -> PluginResponse {
  let mut r = new_response_with_script(script, use_translate);
  r.headers
    .insert(HeaderName::from("ScriptOption"), "nobreak".to_string());
  r
}

pub fn get_references(req: &PluginRequest) -> Vec<&str> {
  let mut references: Vec<&str> = Vec::new();
  let mut i = 0;
  while let Some(value) = req
    .headers
    .get(&HeaderName::from(&format!("Reference{}", i)))
  {
    references.push(value);
    i += 1;
  }
  references
}

pub fn load_descript(file_path: String) -> HashMap<String, String> {
  let mut descript = HashMap::new();
  let path = Path::new(&file_path)
    .join("ghost")
    .join("master")
    .join("descript.txt");
  let buffer = fs::read(path).unwrap();
  let mut result = SHIFT_JIS.decode(&buffer).0;

  // TODO: more smart way to detect charset
  if result
    .clone()
    .into_owned()
    .as_str()
    .contains("charset,UTF-8")
  {
    result = UTF_8.decode(&buffer).0;
  }

  let input_text = result.into_owned();
  for line in input_text.lines() {
    if line.match_indices(',').count() != 1 {
      continue;
    }
    let mut iter = line.split(',');
    let key = iter.next().unwrap().to_string();
    let mut value = iter.next().unwrap().to_string();
    for v in iter {
      value.push_str(v);
    }
    descript.insert(key, value);
  }
  descript
}

pub fn count_characters(ghost_description: HashMap<String, String>) -> Vec<String> {
  let mut characters = Vec::new();
  if let Some(sakura) = ghost_description.get("sakura.name") {
    characters.push(sakura.to_string());
  }
  if let Some(kero) = ghost_description.get("kero.name") {
    characters.push(kero.to_string());
  }
  let mut i = 2;
  while let Some(c) = ghost_description.get(&format!("char{}.name", i)) {
    characters.push(c.to_string());
    i += 1;
  }
  characters
}
