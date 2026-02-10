use ghost_speaker_common::Engine;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex as StdMutex;
use sysinfo::{Pid, ProcessExt, System, SystemExt};

use winapi::um::winbase::CREATE_NO_WINDOW;

static PORT_OPENER_MUTEX: Lazy<StdMutex<Option<System>>> = Lazy::new(|| StdMutex::new(None));

pub async fn get_port_opener_path(
  port: String,
  shutting_down: &'static AtomicBool,
) -> Option<String> {
  let sd = shutting_down;
  match tokio::task::spawn_blocking(move || get_port_opener_path_sync(&port, sd)).await {
    Ok(result) => result,
    Err(e) => {
      log::error!("spawn_blocking failed in get_port_opener_path: {}", e);
      None
    }
  }
}

fn get_port_opener_path_sync(port: &str, shutting_down: &AtomicBool) -> Option<String> {
  use std::os::windows::process::CommandExt;

  // チェックポイント1: ロック取得前（高速パス）
  if shutting_down.load(Ordering::Acquire) {
    log::debug!("shutting down, skipping port check for {}", port);
    return None;
  }

  let mut guard = match PORT_OPENER_MUTEX.lock() {
    Ok(g) => g,
    Err(e) => {
      log::error!("Failed to lock PORT_OPENER_MUTEX: {}", e);
      return None;
    }
  };

  // チェックポイント2: ロック取得後、netstat実行前（待機後の再確認）
  if shutting_down.load(Ordering::Acquire) {
    log::debug!(
      "shutting down after lock acquired, skipping port check for {}",
      port
    );
    return None;
  }

  let output = match Command::new("cmd")
    .args([
      "/C",
      &format!("netstat -ano | findstr LISTENING | findstr {}", port),
    ])
    .creation_flags(CREATE_NO_WINDOW)
    .output()
  {
    Ok(output) => output,
    Err(e) => {
      log::error!("{}", e);
      return None;
    }
  };

  if output.status.success() {
    let output_str = match String::from_utf8(output.stdout) {
      Ok(s) => s,
      Err(e) => {
        log::error!("Failed to parse stdout as UTF-8: {}", e);
        return None;
      }
    };
    log::debug!(
      "netstat found listening process on port {}, querying process info",
      port
    );
    log::logger().flush();

    // チェックポイント3: refresh_processes()呼び出し直前（クラッシュサイト防御）
    if shutting_down.load(Ordering::Acquire) {
      log::debug!(
        "shutting down before refresh_processes, skipping port check for {}",
        port
      );
      return None;
    }

    // sysinfo呼び出しをcatch_unwindで保護 + Systemインスタンスをキャッシュ
    if guard.is_none() {
      log::debug!("Creating new System instance");
      match std::panic::catch_unwind(std::panic::AssertUnwindSafe(System::new)) {
        Ok(system) => *guard = Some(system),
        Err(e) => {
          log::error!("sysinfo System::new() panicked: {:?}", e);
          return None;
        }
      }
    }
    let system = guard.as_mut().unwrap();
    let refresh_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
      system.refresh_processes();
    }));
    if let Err(e) = refresh_result {
      log::error!("sysinfo refresh_processes panicked: {:?}", e);
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
              log::error!("Failed to extract parent process for pid: {}", pid);
            }
          }
          Err(e) => log::error!("failed to parse pid: {}: {}", pid_str, e),
        }
      }
    }
  } else {
    let error_str = match String::from_utf8(output.stderr) {
      Ok(s) => s,
      Err(e) => {
        log::error!("Failed to parse stderr as UTF-8: {}", e);
        "Unknown error".to_string()
      }
    };
    if error_str.is_empty() {
      log::debug!("No listening process found on port {}", port);
    } else {
      log::error!("netstat command failed for port {}: {}", port, error_str);
    }
  }
  log::logger().flush();
  None
}

pub fn cleanup_system_cache() {
  match PORT_OPENER_MUTEX.lock() {
    Ok(mut guard) => {
      *guard = None;
      log::debug!("System cache cleaned up");
    }
    Err(e) => {
      log::error!("Failed to lock PORT_OPENER_MUTEX for cleanup: {}", e);
    }
  }
}

// check the file exists on "C:\Windows\*"
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
        log::debug!("update parent: {}", r.name());
      } else {
        break;
      }
    }
    r.exe().to_str().map(|s| s.to_string())
  } else {
    None
  }
}

pub fn boot_engine(
  engine: Engine,
  engine_path: &HashMap<Engine, String>,
) -> Result<(), Box<dyn std::error::Error>> {
  let path = match engine_path.get(&engine) {
    Some(p) => p,
    None => {
      return Err(format!("No path found for engine: {}", engine.name()).into());
    }
  };

  // do nothing when already booted
  let mut system = System::new();
  system.refresh_processes();
  for process in system.processes().values() {
    if let Some(exe_path) = process.exe().to_str() {
      if exe_path == path {
        return Ok(());
      }
    }
  }

  Command::new(path).spawn()?;
  log::debug!("booted {}", engine.name());
  Ok(())
}
