mod events;
mod format;
mod player;
mod predict;
mod queue;
mod request;
mod response;
mod speaker;
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
use winapi::um::libloaderapi::GetModuleFileNameW;
use winapi::um::winbase::GlobalFree;
use winapi::um::winnt::{
    DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH, DLL_THREAD_ATTACH, DLL_THREAD_DETACH,
};

#[macro_use]
extern crate log;
extern crate simplelog;

use simplelog::*;

static mut DLL_PATH: String = String::new();

#[derive(Debug)]
pub enum ResponseError {
    DecodeFailed,
}

#[no_mangle]
pub extern "system" fn DllMain(
    h_module: HINSTANCE,
    ul_reason_for_call: DWORD,
    _l_reserved: LPVOID,
) -> BOOL {
    match ul_reason_for_call {
        DLL_PROCESS_ATTACH => {
            register_dll_path(h_module);
            let path;
            unsafe {
                path = Path::new(&DLL_PATH.clone())
                    .parent()
                    .unwrap()
                    .join("voice-caller.log");
            };
            WriteLogger::init(
                LevelFilter::Debug,
                Config::default(),
                File::create(path).unwrap(),
            )
            .unwrap();
            debug!("DLL_PROCESS_ATTACH");
        }
        DLL_PROCESS_DETACH => {
            debug!("DLL_PROCESS_DETACH");
        }
        DLL_THREAD_ATTACH => {}
        DLL_THREAD_DETACH => {
            debug!("DLL_THREAD_DETACH");
        }
        _ => {}
    }
    return TRUE;
}

fn register_dll_path(h_module: HINSTANCE) {
    let mut buf: [u16; MAX_PATH + 1] = [0; MAX_PATH + 1];
    unsafe {
        GetModuleFileNameW(h_module, buf.as_mut_ptr(), MAX_PATH as u32);
    }

    let p = buf.partition_point(|v| *v != 0);

    unsafe {
        DLL_PATH = String::from_utf16_lossy(&buf[..p]);
    }
}

#[no_mangle]
pub extern "cdecl" fn load(h: HGLOBAL, len: c_long) -> BOOL {
    let v = GStr::capture(h, len as usize);
    let s = v.to_utf8_str().unwrap();
    unsafe { GlobalFree(h) };

    debug!("load");

    get_global_vars().volatility.dll_dir = s.to_string();
    get_global_vars().load();

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
    use crate::queue::{get_queue, PredictArgs};
    use crate::speaker::{get_speakers_info, SpeakerInfo};
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
