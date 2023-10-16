use std::collections::HashMap;
use std::fs;
use std::path::Path;

use encoding_rs::{SHIFT_JIS, UTF_8};
use rand::Rng;
use shiorust::message::{parts::HeaderName, parts::*, traits::*, Request, Response};

use crate::response::PluginResponse;

pub fn new_response() -> PluginResponse {
    let mut headers = Headers::new();
    headers.insert(
        HeaderName::Standard(StandardHeaderName::Charset),
        String::from("UTF-8"),
    );
    PluginResponse {
        response: Response {
            version: Version::V30,
            status: Status::OK,
            headers,
        },
    }
}

pub fn new_response_nocontent() -> PluginResponse {
    let mut r = new_response();
    r.response.status = Status::NoContent;
    r
}

pub fn new_response_with_script(script: String, use_translate: bool) -> PluginResponse {
    let mut r = new_response();
    r.response
        .headers
        .insert(HeaderName::from("Script"), script);
    r
}

pub fn choose_one(values: &Vec<String>, update_weight: bool) -> Option<String> {
    if values.len() == 0 {
        return None;
    }
    let u = rand::thread_rng().gen_range(0..values.len());
    Some(values.get(u).unwrap().to_owned())
}

// return all combinations of values
// e.g. [a, b], [c, d], [e, f] => "ace", "acf", "ade", "adf", "bce", "bcf", "bde", "bdf"
pub fn all_combo(values: &Vec<Vec<String>>) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = Vec::new();
    all_combo_inner(values, &mut result, &mut current, 0);
    result.iter().map(|v| v.join("")).collect()
}

fn all_combo_inner(
    values: &Vec<Vec<String>>,
    result: &mut Vec<Vec<String>>,
    current: &mut Vec<String>,
    index: usize,
) {
    if index == values.len() {
        result.push(current.clone());
        return;
    }
    for v in values[index].iter() {
        current.push(v.to_string());
        all_combo_inner(values, result, current, index + 1);
        current.pop();
    }
}

pub fn get_references(req: &Request) -> Vec<&str> {
    let mut references: Vec<&str> = Vec::new();
    let mut i = 0;
    loop {
        match req
            .headers
            .get(&HeaderName::from(&format!("Reference{}", i)))
        {
            Some(value) => {
                references.push(value);
                i += 1;
            }
            None => break,
        }
    }
    references
}

pub fn user_talk(dialog: &str, text: &str, text_first: bool) -> String {
    let mut d = String::new();
    if dialog != "" {
        d = format!("『{}』", dialog);
    }
    let mut t = String::new();
    if text != "" {
        t = format!("{}", text);
    }

    let mut v: Vec<String>;
    if text_first {
        v = vec![t, d];
    } else {
        v = vec![d, t];
    }
    v = v
        .iter()
        .filter(|s| s != &&"")
        .map(|s| s.to_string())
        .collect();

    format!("\\1{}\\n", v.join("\\n"))
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
        .find("charset,UTF-8")
        .is_some()
    {
        result = UTF_8.decode(&buffer).0;
    }

    let input_text = result.into_owned();
    for line in input_text.lines() {
        if line.match_indices(",").count() != 1 {
            continue;
        }
        let mut iter = line.split(",");
        let key = iter.next().unwrap().to_string();
        let value = iter.next().unwrap().to_string();
        descript.insert(key, value);
    }
    descript
}

pub fn count_characters(ghost_description: HashMap<String,String>) -> Vec<String> {
    let mut characters = Vec::new();
    if let Some(sakura) = ghost_description.get("sakura.name") {
        characters.push(sakura.to_string());
    }
    if let Some(kero) = ghost_description.get("kero.name") {
        characters.push(kero.to_string());
    }
    let mut i = 2;
    loop {
        if let Some(c) = ghost_description.get(&format!("char{}.name", i)) {
            characters.push(c.to_string());
        } else {
            break;
        }
        i += 1;
    }
    characters
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_combo() {
        let values = vec![
            vec!["a".to_string(), "b".to_string()],
            vec!["c".to_string(), "d".to_string()],
            vec!["e".to_string(), "f".to_string()],
        ];
        let result = all_combo(&values);
        println!("{:?}", result);
    }
}
