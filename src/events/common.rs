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
        response: Response{
        version: Version::V30,
        status: Status::OK,
        headers,
        }
    }
}

pub fn new_response_nocontent() -> PluginResponse {
    let mut r = new_response();
    r.response.status = Status::NoContent;
    r
}

pub fn new_response_with_script(script: String, use_translate: bool) -> PluginResponse {
    let mut r = new_response();
    r.response.headers.insert(HeaderName::from("Script"), script);
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
