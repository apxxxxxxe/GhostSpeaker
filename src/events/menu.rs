use crate::events::common::*;
use crate::response::PluginResponse;
use shiorust::message::Request;

pub fn on_menu_exec(_req: &Request) -> PluginResponse {
    let m = "\
    \\_l[0,4em]\
    \\![*]\\q[なにか話して,OnAiTalk]\
    \\_l[0,12em]\\q[×,]\
    ";
    new_response_with_script(m.to_string(), true)
}
