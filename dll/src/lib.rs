#![windows_subsystem = "windows"]

mod common;
mod events;
mod ipc;
mod plugin;
mod variables;

use crate::ipc::{send_command, shutdown_worker, spawn_worker};
use crate::plugin::request::PluginRequest;
use crate::variables::rawvariables::copy_from_raw;
use crate::variables::rawvariables::save_variables;
use crate::variables::rawvariables::RawGlobalVariables;
use crate::variables::DLL_DIR;
use crate::variables::ENGINE_AUTO_START;
use crate::variables::ENGINE_PATH;
use crate::variables::GHOSTS_VOICES;
use crate::variables::INITIAL_VOICE;
use crate::variables::LOG_INIT_SUCCESS;
use crate::variables::SPEAK_BY_PUNCTUATION;
use crate::variables::VOLUME;
use ghost_speaker_common::{Command, WorkerConfig};
use shiori_hglobal::*;
use shiorust::message::Parser;
use simplelog::*;
use std::fs::File;
use std::panic;
use std::path::PathBuf;
use std::sync::OnceLock;
use winapi::ctypes::c_long;
use winapi::shared::minwindef::{BOOL, FALSE, HGLOBAL, TRUE};
use winapi::um::errhandlingapi::{AddVectoredExceptionHandler, RemoveVectoredExceptionHandler};
use winapi::um::winnt::{EXCEPTION_POINTERS, LONG};
use winapi::vc::excpt::EXCEPTION_CONTINUE_SEARCH;

use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU32, Ordering};

/// VEH ハンドラ用ログファイルパス
static VEH_LOG_PATH: OnceLock<PathBuf> = OnceLock::new();

/// VEH ハンドル（unload 時に RemoveVectoredExceptionHandler で削除するため保持）
static VEH_HANDLE: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());

/// VEH 例外カウンタ（ログ肥大化防止）
static VEH_EXCEPTION_COUNT: AtomicU32 = AtomicU32::new(0);
const MAX_VEH_LOG_ENTRIES: u32 = 50;

/// シャットダウン中フラグ（events ルーティングで使用）
pub(crate) static SHUTTING_DOWN: AtomicBool = AtomicBool::new(false);

unsafe extern "system" fn veh_handler(info: *mut EXCEPTION_POINTERS) -> LONG {
  let record = (*info).ExceptionRecord;
  let code = (*record).ExceptionCode;
  if code == 0x80000003 || code == 0x80000004 {
    return EXCEPTION_CONTINUE_SEARCH;
  }
  let count = VEH_EXCEPTION_COUNT.fetch_add(1, Ordering::Relaxed);
  if count < MAX_VEH_LOG_ENTRIES {
    let address = (*record).ExceptionAddress as usize;
    if let Some(log_path) = VEH_LOG_PATH.get() {
      if let Ok(mut f) = std::fs::OpenOptions::new().append(true).open(log_path) {
        use std::io::Write;
        let _ = writeln!(
          f,
          "VEH: exception 0x{:08X} at address 0x{:08X} (count: {}/{})",
          code,
          address,
          count + 1,
          MAX_VEH_LOG_ENTRIES
        );
      }
    }
  }
  EXCEPTION_CONTINUE_SEARCH
}

#[macro_use]
extern crate log;
extern crate simplelog;

#[no_mangle]
pub extern "cdecl" fn loadu(h: HGLOBAL, len: c_long) -> BOOL {
  match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    let v = GStr::capture(h, len as usize);
    let s = match v.to_utf8_str() {
      Ok(st) => st.to_string(),
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
  })) {
    Ok(v) => v,
    Err(_) => {
      eprintln!("PANIC in loadu");
      FALSE
    }
  }
}

#[no_mangle]
pub extern "cdecl" fn load(h: HGLOBAL, len: c_long) -> BOOL {
  match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    let v = GStr::capture(h, len as usize);
    let s: String;
    match v.to_utf8_str() {
      Ok(st) => {
        s = st.to_string();
      }
      Err(e) => {
        eprintln!("Failed to convert HGLOBAL to UTF-8: {:?}", e);
        match v.to_ansi_str() {
          Ok(st) => {
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
  })) {
    Ok(v) => v,
    Err(_) => {
      eprintln!("PANIC in load");
      FALSE
    }
  }
}

fn common_load_process(dll_path: &str) -> Result<(), ()> {
  SHUTTING_DOWN.store(false, Ordering::Release);
  VEH_EXCEPTION_COUNT.store(0, Ordering::Relaxed);

  let log_path = PathBuf::from(dll_path).join("ghost-speaker.log");
  println!("log_path: {:?}", log_path);

  let _ = VEH_LOG_PATH.set(log_path.clone());

  unsafe {
    let handle = AddVectoredExceptionHandler(1, Some(veh_handler));
    VEH_HANDLE.store(handle, Ordering::Release);
  }

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

  debug!("logger initialized, loading variables");
  log::logger().flush();

  copy_from_raw(&RawGlobalVariables::new(dll_path));
  let mut dll_dir = match DLL_DIR.write() {
    Ok(d) => d,
    Err(_) => return Err(()),
  };
  *dll_dir = dll_path.to_string();
  drop(dll_dir);

  debug!("variables loaded, setting panic hook");
  log::logger().flush();

  panic::set_hook(Box::new(|panic_info| {
    error!("{}", panic_info);
    log::logger().flush();
  }));

  // ワーカープロセスを起動
  debug!("spawning worker process");
  log::logger().flush();

  if let Err(e) = spawn_worker(dll_path) {
    error!("Failed to spawn worker: {}", e);
    return Err(());
  }

  // ワーカーに Init コマンドを送信
  debug!("sending Init command to worker");
  log::logger().flush();

  let config = build_worker_config();
  match send_command(&Command::Init {
    dll_dir: dll_path.to_string(),
    config,
  }) {
    Ok(ghost_speaker_common::Response::Ok) => {
      debug!("Worker initialized successfully");
    }
    Ok(ghost_speaker_common::Response::Error { message }) => {
      error!("Worker Init failed: {}", message);
      return Err(());
    }
    Ok(other) => {
      error!("Unexpected Init response: {:?}", other);
      return Err(());
    }
    Err(e) => {
      error!("Failed to send Init command: {}", e);
      return Err(());
    }
  }

  debug!("load complete");
  log::logger().flush();

  Ok(())
}

fn build_worker_config() -> WorkerConfig {
  let volume = VOLUME.read().map(|v| *v).unwrap_or(1.0);
  let speak_by_punctuation = SPEAK_BY_PUNCTUATION.read().map(|s| *s).unwrap_or(true);
  let ghosts_voices = GHOSTS_VOICES
    .read()
    .map(|gv| gv.clone())
    .unwrap_or_default();
  let initial_voice = INITIAL_VOICE
    .read()
    .map(|iv| iv.clone())
    .unwrap_or_default();
  let engine_auto_start = ENGINE_AUTO_START
    .read()
    .map(|ea| ea.clone())
    .unwrap_or_default();
  let engine_path = ENGINE_PATH.read().map(|ep| ep.clone()).unwrap_or_default();

  WorkerConfig {
    volume,
    speak_by_punctuation,
    ghosts_voices,
    initial_voice,
    engine_auto_start,
    engine_path,
  }
}

#[no_mangle]
pub extern "cdecl" fn unload() -> BOOL {
  match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    SHUTTING_DOWN.store(true, Ordering::Release);

    let _ = panic::take_hook();

    let veh = VEH_HANDLE.swap(std::ptr::null_mut(), Ordering::AcqRel);
    if !veh.is_null() {
      unsafe {
        RemoveVectoredExceptionHandler(veh);
      }
    }

    // ワーカー停止前にエンジンステータスを同期
    match send_command(&Command::GetEngineStatus) {
      Ok(ghost_speaker_common::Response::EngineStatus {
        engine_paths,
        engine_auto_start,
        ..
      }) => {
        if let Ok(mut ep) = ENGINE_PATH.write() {
          *ep = engine_paths;
        }
        if let Ok(mut ea) = ENGINE_AUTO_START.write() {
          *ea = engine_auto_start;
        }
      }
      Ok(_) => {
        debug!("Unexpected response from GetEngineStatus");
      }
      Err(e) => {
        debug!("Failed to get engine status before shutdown: {}", e);
      }
    }

    // ワーカーを停止
    if let Err(e) = shutdown_worker() {
      error!("Failed to shutdown worker: {}", e);
    }

    // 変数保存
    if save_variables().is_err() {
      error!("Failed to save variables");
    }

    log::logger().flush();

    debug!("unload");

    TRUE
  })) {
    Ok(v) => v,
    Err(_) => {
      eprintln!("PANIC in unload, attempting emergency shutdown");
      let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = shutdown_worker();
      }));
      TRUE
    }
  }
}

#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub extern "cdecl" fn request(h: HGLOBAL, len: &mut c_long) -> HGLOBAL {
  match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
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
  })) {
    Ok(h) => h,
    Err(_) => {
      eprintln!("PANIC in request");
      const RESPONSE_500: &str = "SHIORI/3.0 500 Internal Server Error\r\n\r\n";
      let response_gstr = GStr::clone_from_slice_nofree(RESPONSE_500.as_bytes());
      *len = response_gstr.len() as c_long;
      response_gstr.handle()
    }
  }
}
