#![windows_subsystem = "windows"]

mod common;
mod engine;
mod events;
mod format;
mod player;
mod plugin;
mod queue;
mod speaker;
mod system;
mod variables;

use crate::plugin::request::PluginRequest;
use crate::queue::QUEUE;
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

  let log_path = Path::new(&s).join("ghost-speaker.log");
  WriteLogger::init(
    LevelFilter::Debug,
    Config::default(),
    File::create(log_path).unwrap(),
  )
  .unwrap();

  get_global_vars().volatility.dll_dir = s.to_string();
  get_global_vars().load();

  panic::set_hook(Box::new(|panic_info| {
    debug!("{}", panic_info);
  }));

  // autostart enabled engines
  for (engine, auto_start) in get_global_vars().engine_auto_start.as_ref().unwrap() {
    if *auto_start {
      if let Err(e) = system::boot_engine(*engine) {
        error!("Failed to boot {}: {}", engine.name(), e);
      }
    }
  }

  debug!("load");

  TRUE
}

#[no_mangle]
pub extern "cdecl" fn unload() -> BOOL {
  get_global_vars().save();
  let mut queue = QUEUE.lock().unwrap();
  queue.stop();

  debug!("unload");

  TRUE
}

#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "cdecl" fn request(h: HGLOBAL, len: *mut c_long) -> HGLOBAL {
  // リクエストの取得
  let v = unsafe { GStr::capture(h, *len as usize) };

  let s = v.to_utf8_str().unwrap();

  let pr = PluginRequest::parse(s).unwrap();

  let response = events::handle_request(&pr);

  let bytes = response.to_string().into_bytes();
  let response_gstr = GStr::clone_from_slice_nofree(&bytes);

  unsafe { *len = response_gstr.len() as c_long };
  response_gstr.handle()
}
