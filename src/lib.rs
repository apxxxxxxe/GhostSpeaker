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
use crate::queue::{init_queues, shutdown_runtime, stop_async_tasks};
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
use std::sync::OnceLock;
use winapi::ctypes::c_long;
use winapi::shared::minwindef::{BOOL, FALSE, HGLOBAL, TRUE};
use winapi::um::errhandlingapi::{AddVectoredExceptionHandler, RemoveVectoredExceptionHandler};
use winapi::um::winnt::{EXCEPTION_POINTERS, LONG};
use winapi::vc::excpt::EXCEPTION_CONTINUE_SEARCH;

use std::sync::atomic::{AtomicPtr, AtomicU32, Ordering};

/// VEH ハンドラ用ログファイルパス
static VEH_LOG_PATH: OnceLock<PathBuf> = OnceLock::new();

/// VEH ハンドル（unload 時に RemoveVectoredExceptionHandler で削除するため保持）
static VEH_HANDLE: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());

/// VEH 例外カウンタ（ログ肥大化防止）
static VEH_EXCEPTION_COUNT: AtomicU32 = AtomicU32::new(0);
const MAX_VEH_LOG_ENTRIES: u32 = 50;

unsafe extern "system" fn veh_handler(info: *mut EXCEPTION_POINTERS) -> LONG {
  let record = (*info).ExceptionRecord;
  let code = (*record).ExceptionCode;
  // 非致命的例外は無視（EXCEPTION_BREAKPOINT, STATUS_SINGLE_STEP）
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
  })) {
    Ok(v) => v,
    Err(_) => {
      eprintln!("PANIC in load");
      FALSE
    }
  }
}

fn common_load_process(dll_path: &str) -> Result<(), ()> {
  VEH_EXCEPTION_COUNT.store(0, Ordering::Relaxed);

  // Windows(UTF-16)を想定しPathBufでパスを作成
  let log_path = PathBuf::from(dll_path).join("ghost-speaker.log");
  println!("log_path: {:?}", log_path);

  // VEH 用ログパスを設定（ロガー初期化前でも書き込み可能にする）
  let _ = VEH_LOG_PATH.set(log_path.clone());

  // VEH (Vectored Exception Handler) を登録
  // ACCESS_VIOLATION 等の SEH 例外をログに記録するため
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

  debug!("variables loaded, setting panic hook");
  log::logger().flush();

  panic::set_hook(Box::new(|panic_info| {
    error!("{}", panic_info);
    log::logger().flush();
  }));

  // 自動起動が設定されているエンジンを起動
  debug!("checking engine auto-start");
  log::logger().flush();
  let engine_auto_start = match ENGINE_AUTO_START.read() {
    Ok(eas) => eas.clone(),
    Err(e) => {
      error!("Failed to read ENGINE_AUTO_START: {}", e);
      return Err(());
    }
  };
  {
    use sysinfo::{System, SystemExt};
    let system_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
      let mut system = System::new();
      system.refresh_processes();
      system
    }));
    match system_result {
      Ok(system) => {
        for (engine, auto_start) in engine_auto_start.iter() {
          if *auto_start {
            if let Err(e) = boot_engine(*engine, &system) {
              error!("Failed to boot {}: {}", engine.name(), e);
            }
          }
        }
      }
      Err(e) => {
        error!("sysinfo panicked during engine auto-start: {:?}", e);
      }
    }
  }

  debug!("engine boot complete, initializing queues");
  log::logger().flush();

  init_queues();

  debug!("queues initialized, load complete");
  log::logger().flush();

  Ok(())
}

#[no_mangle]
pub extern "cdecl" fn unload() -> BOOL {
  match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    // 1. panic hook を除去（FreeLibrary 後の dangling 防止）
    let _ = panic::take_hook();

    // 2. VEH ハンドラを削除（DLLアンロード前に必須）
    let veh = VEH_HANDLE.swap(std::ptr::null_mut(), Ordering::AcqRel);
    if !veh.is_null() {
      unsafe {
        RemoveVectoredExceptionHandler(veh);
      }
    }

    // 3. 非同期タスク停止
    if stop_async_tasks().is_err() {
      error!("Failed to stop async tasks");
    }

    // 4. 変数保存
    if save_variables().is_err() {
      error!("Failed to save variables");
    }

    // 5. ログフラッシュ（ランタイム停止前に確実にフラッシュ）
    log::logger().flush();

    // 6. ランタイム停止（全スレッド join）
    if shutdown_runtime().is_err() {
      error!("Failed to shutdown runtime");
    }

    debug!("unload");

    TRUE
  })) {
    Ok(v) => v,
    Err(_) => {
      eprintln!("PANIC in unload, attempting emergency shutdown");
      let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = shutdown_runtime();
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
