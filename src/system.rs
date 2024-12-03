use crate::engine::Engine;
use crate::variables::*;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::Command;
use sysinfo::{Pid, Process, ProcessExt, System, SystemExt};
use winapi::um::winbase::CREATE_NO_WINDOW;

pub fn get_port_opener_path(port: String) -> Option<String> {
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
    let output_str = String::from_utf8(output.stdout).unwrap();
    for line in output_str.lines() {
      let parts: Vec<&str> = line.split_whitespace().collect();
      if let Some(pid_str) = parts.last() {
        match pid_str.parse::<usize>() {
          Ok(pid) => {
            let mut system = System::new_all();
            let proc = extract_parent_process(Pid::from(pid), &mut system).unwrap();
            return Some(proc.exe().to_str().unwrap().to_string());
          }
          Err(e) => error!("failed to parse pid: {}: {}", pid_str, e),
        }
      }
    }
  } else {
    let error_str = String::from_utf8(output.stderr).unwrap();
    eprintln!("エラー: {}", error_str);
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

pub fn boot_engine(engine: Engine) -> Result<(), Box<dyn std::error::Error>> {
  let engine_path = ENGINE_PATH.read().unwrap();
  let path = engine_path.get(&engine).unwrap();

  // do nothing when already booted
  let mut system = System::new_all();
  system.refresh_all();
  for process in system.processes().values() {
    if process.exe().to_str().unwrap() == path {
      return Ok(());
    }
  }

  Command::new(path).spawn()?;
  debug!("booted {}", engine.name());
  Ok(())
}
