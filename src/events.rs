mod common;
mod menu;
mod periodic;

use crate::events::common::*;
use crate::events::menu::*;
use crate::events::periodic::*;
use crate::response::PluginResponse;

use shiorust::message::{parts::*, traits::*, Request};

pub fn handle_request(req: &Request) -> PluginResponse {
    match req.method {
        Method::GET => (),
        _ => return new_response_nocontent(),
    };

    let event_id;
    match req.headers.get("ID") {
        Some(id) => {
            event_id = id;
        }
        None => return new_response_nocontent(),
    };

    debug!("event: {}", event_id);

    let event = match get_event(event_id.as_str()) {
        Some(e) => e,
        None => {
            let base_id = match req.headers.get("BaseID") {
                Some(id) => id,
                None => return new_response_nocontent(),
            };
            match get_event(base_id.as_str()) {
                Some(e) => e,
                None => return new_response_nocontent(),
            }
        }
    };

    let res = event(req);
    debug!("response: {:?}", res);
    res
}

pub fn version(_req: &Request) -> PluginResponse {
    new_response_with_script(String::from(env!("CARGO_PKG_VERSION")), false)
}

fn get_event(id: &str) -> Option<fn(&Request) -> PluginResponse> {
    match id {
        "version" => Some(version),
        "OnSecondChange" => Some(on_second_change),
        "OnMenuExec" => Some(on_menu_exec),
        "OnOtherGhostTalk" => Some(on_other_ghost_talk),
        _ => None,
    }
}
