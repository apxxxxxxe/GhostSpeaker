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
use crate::queue::{init_queues, shutdown_runtime, stop_async_tasks, RUNTIME};
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
use std::path::PathBuf;
use winapi::ctypes::c_long;
use winapi::shared::minwindef::{BOOL, FALSE, HGLOBAL, TRUE};

#[macro_use]
extern crate log;
extern crate simplelog;

#[no_mangle]
pub extern "cdecl" fn loadu(h: HGLOBAL, len: c_long) -> BOOL {
  let v = GStr::capture(h, len as usize);
  let s = match v.to_utf8_str() {
    Ok(st) => {
      // UTF-8に変換
      st.to_string()
    }
    Err(e) => {
      eprintln!("Failed to convert HGLOBAL to UTF-8: {:?}", e);
      return FALSE;
    }
  };

  match common_load_process(&s) {
    Ok(_) => {
      debug!("loadu");
      TRUE
    }
    Err(_) => {
      eprintln!("Failed to load plugin");
      FALSE
    }
  }
}

#[no_mangle]
pub extern "cdecl" fn load(h: HGLOBAL, len: c_long) -> BOOL {
  let v = GStr::capture(h, len as usize);
  let s: String;
  match v.to_utf8_str() {
    Ok(st) => {
      // UTF-8に変換
      s = st.to_string();
    }
    Err(e) => {
      eprintln!("Failed to convert HGLOBAL to UTF-8: {:?}", e);
      match v.to_ansi_str() {
        Ok(st) => {
          // ANSIに変換
          s = st.to_string_lossy().to_string();
        }
        Err(e) => {
          eprintln!("Failed to convert HGLOBAL to ANSI: {:?}", e);
          return FALSE;
        }
      }
    }
  };

  match common_load_process(&s) {
    Ok(_) => {
      debug!("load");
      TRUE
    }
    Err(_) => {
      eprintln!("Failed to load plugin");
      FALSE
    }
  }
}

fn common_load_process(dll_path: &str) -> Result<(), ()> {
  // Windows(UTF-16)を想定しPathBufでパスを作成
  let log_path = PathBuf::from(dll_path).join("ghost-speaker.log");
  println!("log_path: {:?}", log_path);
  if let Ok(log_writer) = File::create(&log_path) {
    if WriteLogger::init(LevelFilter::Debug, Config::default(), log_writer).is_err() {
      eprintln!("Failed to initialize logger");
    } else {
      let mut log_init_success = match LOG_INIT_SUCCESS.write() {
        Ok(l) => l,
        Err(_) => return Err(()),
      };
      *log_init_success = true;
    }
  };

  copy_from_raw(&RawGlobalVariables::new(dll_path));
  let mut dll_dir = match DLL_DIR.write() {
    Ok(d) => d,
    Err(_) => return Err(()),
  };
  *dll_dir = dll_path.to_string();

  panic::set_hook(Box::new(|panic_info| {
    error!("{}", panic_info);
    log::logger().flush();
  }));

  // 自動起動が設定されているエンジンを起動
  let engine_auto_start = {
    let guard = match RUNTIME.lock() {
      Ok(g) => g,
      Err(e) => {
        error!("Failed to lock RUNTIME: {}", e);
        return Err(());
      }
    };
    match guard.as_ref() {
      Some(rt) => rt.handle().block_on(async { ENGINE_AUTO_START.read().await.clone() }),
      None => {
        error!("Runtime is not initialized");
        return Err(());
      }
    }
  };
  for (engine, auto_start) in engine_auto_start.iter()
  {
    if *auto_start {
      if let Err(e) = boot_engine(*engine) {
        error!("Failed to boot {}: {}", engine.name(), e);
      }
    }
  }

  init_queues();

  Ok(())
}

#[no_mangle]
pub extern "cdecl" fn unload() -> BOOL {
  if stop_async_tasks().is_err() {
    error!("Failed to stop async tasks");
  }
  if save_variables().is_err() {
    error!("Failed to save variables");
  }
  if shutdown_runtime().is_err() {
    error!("Failed to shutdown runtime");
  }

  debug!("unload");

  TRUE
}

#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub extern "cdecl" fn request(h: HGLOBAL, len: &mut c_long) -> HGLOBAL {
  const RESPONSE_400: &str = "SHIORI/3.0 400 Bad Request\r\n\r\n";
  let v = GStr::capture(h, *len as usize);
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

  *len = response_gstr.len() as c_long;
  response_gstr.handle()
}
