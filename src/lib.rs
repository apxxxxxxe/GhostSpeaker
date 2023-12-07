mod engine;
mod events;
mod format;
mod player;
mod plugin;
mod queue;
mod speaker;
mod variables;

use crate::plugin::request::PluginRequest;
use crate::queue::get_queue;
use crate::variables::get_global_vars;
use shiori_hglobal::*;
use shiorust::message::Parser;
use simplelog::*;
use std::fs::File;
use std::panic;
use std::path::Path;
use winapi::ctypes::c_long;
use winapi::shared::minwindef::{BOOL, HGLOBAL, TRUE};

#[macro_use]
extern crate log;
extern crate simplelog;

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
  get_queue(); // init

  let log_path = Path::new(&get_global_vars().volatility.dll_dir).join("ghost-speaker.log");
  WriteLogger::init(
    LevelFilter::Debug,
    Config::default(),
    File::create(log_path).unwrap(),
  )
  .unwrap();

  panic::set_hook(Box::new(|panic_info| {
    debug!("{}", panic_info);
  }));

  debug!("load");

  return TRUE;
}

#[no_mangle]
pub extern "cdecl" fn unload() -> BOOL {
  get_global_vars().save();
  get_queue().stop();

  debug!("unload");

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
