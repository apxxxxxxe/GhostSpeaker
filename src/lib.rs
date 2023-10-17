mod coeiroink;
mod events;
mod format;
mod player;
mod queue;
mod request;
mod response;
mod variables;

use crate::queue::get_queue;
use crate::request::PluginRequest;
use crate::variables::get_global_vars;

use std::fs::File;
use std::path::Path;

use shiori_hglobal::*;
use shiorust::message::Parser;

use winapi::ctypes::c_long;
use winapi::shared::minwindef::{BOOL, DWORD, HGLOBAL, HINSTANCE, LPVOID, MAX_PATH, TRUE};
use winapi::um::winnt::{
    DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH, DLL_THREAD_ATTACH, DLL_THREAD_DETACH,
};

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

    debug!("load");

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

    return TRUE;
}

#[no_mangle]
pub extern "cdecl" fn unload() -> BOOL {
    debug!("unload");

    get_global_vars().save();
    get_queue().stop();

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
