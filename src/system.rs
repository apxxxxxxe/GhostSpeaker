use crate::engine::Engine;
use crate::queue::SHUTTING_DOWN;
use crate::variables::*;
use once_cell::sync::Lazy;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::Ordering;
use std::sync::Mutex as StdMutex;
use sysinfo::{Pid, ProcessExt, System, SystemExt};

use winapi::um::winbase::CREATE_NO_WINDOW;

static PORT_OPENER_MUTEX: Lazy<StdMutex<Option<System>>> = Lazy::new(|| StdMutex::new(None));

pub(crate) async fn get_port_opener_path(port: String) -> Option<String> {
  match tokio::task::spawn_blocking(move || get_port_opener_path_sync(&port)).await {
    Ok(result) => result,
    Err(e) => {
      error!("spawn_blocking failed in get_port_opener_path: {}", e);
      None
    }
  }
}

fn get_port_opener_path_sync(port: &str) -> Option<String> {
  use std::os::windows::process::CommandExt;

  // チェックポイント1: ロック取得前（高速パス）
  if SHUTTING_DOWN.load(Ordering::Acquire) {
    debug!("shutting down, skipping port check for {}", port);
    return None;
  }

  let mut guard = match PORT_OPENER_MUTEX.lock() {
    Ok(g) => g,
    Err(e) => {
      error!("Failed to lock PORT_OPENER_MUTEX: {}", e);
      return None;
    }
  };

  // チェックポイント2: ロック取得後、netstat実行前（待機後の再確認）
  if SHUTTING_DOWN.load(Ordering::Acquire) {
    debug!("shutting down after lock acquired, skipping port check for {}", port);
    return None;
  }

  let output = match Command::new("cmd")
    .args(["/C", &format!("netstat -ano | findstr LISTENING | findstr {}", port)])
    .creation_flags(CREATE_NO_WINDOW)
    .output()
  {
    Ok(output) => output,
    Err(e) => {
      error!("{}", e);
      return None;
    }
  };

  if output.status.success() {
    let output_str = match String::from_utf8(output.stdout) {
      Ok(s) => s,
      Err(e) => {
        error!("Failed to parse stdout as UTF-8: {}", e);
        return None;
      }
    };
    debug!("netstat found listening process on port {}, querying process info", port);
    log::logger().flush();

    // チェックポイント3: refresh_processes()呼び出し直前（クラッシュサイト防御）
    if SHUTTING_DOWN.load(Ordering::Acquire) {
      debug!("shutting down before refresh_processes, skipping port check for {}", port);
      return None;
    }

    // sysinfo呼び出しをcatch_unwindで保護 + Systemインスタンスをキャッシュ
    let system = guard.get_or_insert_with(|| {
      debug!("Creating new System instance");
      System::new()
    });
    let refresh_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
      system.refresh_processes();
    }));
    if let Err(e) = refresh_result {
      error!("sysinfo refresh_processes panicked: {:?}", e);
      // パニック後のSystemは不定状態なので破棄
      *guard = None;
      return None;
    }

    log::logger().flush();
    for line in output_str.lines() {
      let parts: Vec<&str> = line.split_whitespace().collect();
      if let Some(pid_str) = parts.last() {
        match pid_str.parse::<usize>() {
          Ok(pid) => {
            if let Some(path) = extract_parent_process_path(Pid::from(pid), system) {
              return Some(path);
            } else {
              error!("Failed to extract parent process for pid: {}", pid);
            }
          }
          Err(e) => error!("failed to parse pid: {}: {}", pid_str, e),
        }
      }
    }
  } else {
    let error_str = match String::from_utf8(output.stderr) {
      Ok(s) => s,
      Err(e) => {
        error!("Failed to parse stderr as UTF-8: {}", e);
        "Unknown error".to_string()
      }
    };
    if error_str.is_empty() {
      debug!("No listening process found on port {}", port);
    } else {
      error!("netstat command failed for port {}: {}", port, error_str);
    }
  }
  log::logger().flush();
  None
}

pub(crate) fn cleanup_system_cache() {
  match PORT_OPENER_MUTEX.lock() {
    Ok(mut guard) => {
      *guard = None;
      debug!("System cache cleaned up");
    }
    Err(e) => {
      error!("Failed to lock PORT_OPENER_MUTEX for cleanup: {}", e);
    }
  }
}

// check the file exists on "C:\Windows\*"
// TODO: better way?
fn is_os_level_executable(path: &Path) -> bool {
  path.starts_with("C:\\Windows\\") || path.ends_with("explorer.exe") || path.ends_with("ssp.exe")
}

fn extract_parent_process_path(pid: Pid, system: &mut System) -> Option<String> {
  if let Some(process) = system.process(pid) {
    let mut r = process;
    while let Some(ppid) = r.parent() {
      if let Some(parent) = system.process(ppid) {
        if is_os_level_executable(parent.exe()) {
          break;
        }
        r = parent;
        debug!("update parent: {}", r.name());
      } else {
        break;
      }
    }
    r.exe().to_str().map(|s| s.to_string())
  } else {
    None
  }
}

pub(crate) fn boot_engine(engine: Engine, system: &System) -> Result<(), Box<dyn std::error::Error>> {
  let engine_path = match ENGINE_PATH.read() {
    Ok(ep) => ep,
    Err(e) => {
      return Err(format!("Failed to read ENGINE_PATH: {}", e).into());
    }
  };

  let path = match engine_path.get(&engine) {
    Some(p) => p,
    None => {
      return Err(format!("No path found for engine: {}", engine.name()).into());
    }
  };

  // do nothing when already booted
  for process in system.processes().values() {
    if let Some(exe_path) = process.exe().to_str() {
      if exe_path == path {
        return Ok(());
      }
    }
  }

  Command::new(path).spawn()?;
  debug!("booted {}", engine.name());
  Ok(())
}
