mod coeiroink;
mod events;
mod format;
mod player;
mod process;
mod queue;
mod request;
mod response;
mod variables;

use crate::coeiroink::utils::{check_engine_status, EngineStatus};
use crate::process::{exec_process, kill_process};
use crate::queue::get_queue;
use crate::request::PluginRequest;
use crate::variables::get_global_vars;

use std::fs::File;
use std::path::Path;

use shiori_hglobal::*;
use shiorust::message::Parser;

use winapi::ctypes::c_long;
use winapi::shared::minwindef::{BOOL, HGLOBAL, TRUE};

#[macro_use]
extern crate log;
extern crate simplelog;

use simplelog::*;

pub static mut DLL_PATH: String = String::new();

#[derive(Debug)]
pub enum ResponseError {
    DecodeFailed,
}

#[no_mangle]
pub extern "cdecl" fn load(h: HGLOBAL, len: c_long) -> BOOL {
    let v = GStr::capture(h, len as usize);
    let s = v.to_utf8_str().unwrap();

    get_global_vars().volatility.dll_dir = s.to_string();
    get_global_vars().load();

    let log_path = Path::new(&get_global_vars().volatility.dll_dir)
        .parent()
        .unwrap()
        .join("voice-caller.log");
    WriteLogger::init(
        LevelFilter::Debug,
        Config::default(),
        File::create(log_path).unwrap(),
    )
    .unwrap();

    debug!("load");

    match check_engine_status() {
        EngineStatus::Initializing => {
            debug!("Engine is initializing");
        }
        EngineStatus::Running => {
            debug!("Engine is running");
        }
        EngineStatus::Stopped => {
            debug!("Engine is stopped");
            let path = get_global_vars().engine_path.clone().unwrap();
            if let Err(e) = exec_process(&path) {
                error!("Failed to start engine process. {}", e);
            }
        }
        EngineStatus::Unknown => {
            debug!("Engine status is unknown: engine path is not set");
        }
    }

    return TRUE;
}

#[no_mangle]
pub extern "cdecl" fn unload() -> BOOL {
    debug!("unload");

    get_global_vars().save();
    get_queue().stop();

    if get_global_vars().volatility.is_booted_with_engine {
        if let Some(path) = get_global_vars().engine_path.clone() {
            if let Err(e) = kill_process(&path) {
                error!("Failed to kill engine process. {}", e);
            }
        }
    }

    return TRUE;
}

#[no_mangle]
pub extern "cdecl" fn request(h: HGLOBAL, len: *mut c_long) -> HGLOBAL {
    // リクエストの取得
    let v = unsafe { GStr::capture(h, *len as usize) };

    let s = v.to_utf8_str().unwrap();

    let pr = PluginRequest::parse(&s).unwrap();
    let r = pr.request;

    let response = events::handle_request(&r);

    let bytes = response.to_string().into_bytes();
    let response_gstr = GStr::clone_from_slice_nofree(&bytes);

    unsafe { *len = response_gstr.len() as c_long };
    response_gstr.handle()
}

#[cfg(test)]
mod test {
    use crate::coeiroink::speaker::{get_speakers_info, SpeakerInfo};
    use crate::queue::{get_queue, PredictArgs};
    use std::time::Duration;

    #[test]
    fn test_main() {
        let info: Vec<SpeakerInfo> = get_speakers_info().unwrap();
        for i in info.iter() {
            println!("{:?}", i.speaker_name);
        }
        let speaker = info.get(0).unwrap();
        let speaker_uuid = String::from(&speaker.speaker_uuid);
        let style_id = speaker.styles.get(0).unwrap().style_id.unwrap();
        for i in 1..4 {
            let args = PredictArgs {
                text: format!("こんにちは{}", i),
                speaker_uuid: String::from(&speaker_uuid),
                style_id,
            };
            get_queue().push_to_prediction(args);
        }
        for i in 0..20 {
            println!("{}", i);
            std::thread::sleep(Duration::from_secs(1));
        }
    }
}
