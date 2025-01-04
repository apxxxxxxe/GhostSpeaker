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
use crate::queue::{init_queues, stop_queues};
use crate::system::boot_engine;
use crate::variables::rawvariables::copy_from_raw;
use crate::variables::rawvariables::save_variables;
use crate::variables::rawvariables::RawGlobalVariables;
use crate::variables::DLL_DIR;
use crate::variables::ENGINE_AUTO_START;
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

  copy_from_raw(&RawGlobalVariables::new(s));
  *DLL_DIR.write().unwrap() = s.to_string();

  panic::set_hook(Box::new(|panic_info| {
    debug!("{}", panic_info);
  }));

  // 自動起動が設定されているエンジンを起動
  for (engine, auto_start) in
    futures::executor::block_on(async { ENGINE_AUTO_START.read().await }).iter()
  {
    if *auto_start {
      if let Err(e) = boot_engine(*engine) {
        error!("Failed to boot {}: {}", engine.name(), e);
      }
    }
  }

  init_queues();

  debug!("load");

  TRUE
}

#[no_mangle]
pub extern "cdecl" fn unload() -> BOOL {
  save_variables();
  stop_queues();

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
