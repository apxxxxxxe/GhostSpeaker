use crate::engine::Engine;
use crate::variables::get_global_vars;
use std::os::windows::process::CommandExt;
use std::process::Command;
use sysinfo::{Pid, ProcessExt, System, SystemExt};
use winapi::um::winbase::CREATE_NO_WINDOW;

pub fn get_port_opener_path(port: String) -> Option<String> {
  let output = match Command::new("cmd")
    .args(&["/C", "netstat -ano | findstr LISTENING | findstr", &port])
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
            system.refresh_all();
            if let Some(process) = system.process(Pid::from(pid as usize)) {
              return Some(process.exe().to_str().unwrap().to_string());
            } else {
              debug!("PID {} not found", pid);
            }
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

pub fn boot_engine(engine: Engine) -> Result<(), Box<dyn std::error::Error>> {
  let vars = get_global_vars();
  let path = vars.engine_path.as_ref().unwrap().get(&engine).unwrap();

  // do nothing when already booted
  let mut system = System::new_all();
  system.refresh_all();
  for (_, process) in system.processes() {
    if process.exe().to_str().unwrap() == path {
      return Ok(());
    }
  }

  Command::new(path).spawn()?;
  debug!("booted {}", engine.name());
  Ok(())
}
