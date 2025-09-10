use crate::engine::Engine;
use crate::variables::*;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::Command;
use sysinfo::{Pid, Process, ProcessExt, System, SystemExt};
use winapi::um::winbase::CREATE_NO_WINDOW;

pub(crate) fn get_port_opener_path(port: String) -> Option<String> {
  let output = match Command::new("cmd")
    .args(["/C", "netstat -ano | findstr LISTENING | findstr", &port])
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
    for line in output_str.lines() {
      let parts: Vec<&str> = line.split_whitespace().collect();
      if let Some(pid_str) = parts.last() {
        match pid_str.parse::<usize>() {
          Ok(pid) => {
            let mut system = System::new_all();
            if let Some(proc) = extract_parent_process(Pid::from(pid), &mut system) {
              if let Some(exe_path) = proc.exe().to_str() {
                return Some(exe_path.to_string());
              } else {
                error!("Failed to convert process path to string for pid: {}", pid);
              }
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
    error!("Command failed: {}", error_str);
  }
  None
}

// check the file exists on "C:\Windows\*"
// TODO: better way?
fn is_os_level_executable(path: &Path) -> bool {
  path.starts_with("C:\\Windows\\") || path.ends_with("explorer.exe") || path.ends_with("ssp.exe")
}

fn extract_parent_process(pid: Pid, system: &mut System) -> Option<&Process> {
  system.refresh_all();
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
    Some(r)
  } else {
    None
  }
}

pub(crate) fn boot_engine(engine: Engine) -> Result<(), Box<dyn std::error::Error>> {
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
  let mut system = System::new_all();
  system.refresh_all();
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
