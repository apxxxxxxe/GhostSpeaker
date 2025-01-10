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
use crate::variables::LOG_INIT_SUCCESS;
use shiori_hglobal::*;
use shiorust::message::Parser;
use simplelog::*;
use std::fs::File;
use std::panic;
use std::path::Path;
use winapi::ctypes::c_long;
use winapi::shared::minwindef::{BOOL, FALSE, HGLOBAL, TRUE};

#[macro_use]
extern crate log;
extern crate simplelog;

#[no_mangle]
pub extern "cdecl" fn load(h: HGLOBAL, len: c_long) -> BOOL {
  let v = GStr::capture(h, len as usize);
  let s = match v.to_utf8_str() {
    Ok(s) => s,
    Err(_) => {
      eprintln!("Failed to convert HGLOBAL to UTF-8");
      return FALSE;
    }
  };

  let log_path = Path::new(&s).join("ghost-speaker.log");
  if let Ok(log_writer) = File::create(&log_path) {
    if WriteLogger::init(LevelFilter::Debug, Config::default(), log_writer).is_err() {
      eprintln!("Failed to initialize logger");
    } else {
      let mut log_init_success = match LOG_INIT_SUCCESS.write() {
        Ok(l) => l,
        Err(_) => return FALSE,
      };
      *log_init_success = true;
    }
  };

  copy_from_raw(&RawGlobalVariables::new(s));
  let mut dll_dir = match DLL_DIR.write() {
    Ok(d) => d,
    Err(_) => return FALSE,
  };
  *dll_dir = s.to_string();

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
  if save_variables().is_err() {
    error!("Failed to save variables");
  }
  if stop_queues().is_err() {
    error!("Failed to stop queues");
  }

  debug!("unload");

  TRUE
}

#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "cdecl" fn request(h: HGLOBAL, len: *mut c_long) -> HGLOBAL {
  const RESPONSE_400: &str = "SHIORI/3.0 400 Bad Request\r\n\r\n";
  let v = unsafe { GStr::capture(h, *len as usize) };
  let s = match v.to_utf8_str() {
    Ok(s) => s,
    Err(_) => {
      let response_gstr = GStr::clone_from_slice_nofree(RESPONSE_400.as_bytes());
      *len = response_gstr.len() as c_long;
      return response_gstr.handle();
    }
  };

  let pr = match PluginRequest::parse(s) {
    Ok(pr) => pr,
    Err(_) => {
      let response_gstr = GStr::clone_from_slice_nofree(RESPONSE_400.as_bytes());
      *len = response_gstr.len() as c_long;
      return response_gstr.handle();
    }
  };

  let response = events::handle_request(&pr);

  let bytes = response.to_string().into_bytes();
  let response_gstr = GStr::clone_from_slice_nofree(&bytes);

  unsafe { *len = response_gstr.len() as c_long };
  response_gstr.handle()
}
