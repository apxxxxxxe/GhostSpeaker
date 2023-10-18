use create_process_w::Command;
use std::io::{Error, ErrorKind};
use sysinfo::{Pid, ProcessExt, System, SystemExt};

pub fn find_process(path: &str) -> Option<Pid> {
    let mut sys = System::new_all();
    sys.refresh_processes();
    for (pid, process) in sys.processes() {
        if process.exe().to_str().unwrap() == path {
            return Some(*pid);
        }
    }
    None
}

pub fn exec_process(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut sys = System::new_all();
    sys.refresh_processes();
    if let Some(pid) = find_process(path) {
        println!(
            "Process already started: {:?}",
            sys.process(pid).unwrap().exe()
        );
        return Ok(());
    }

    if let Err(e) = Command::new(&path).inherit_handles(false).spawn() {
        println!("Process start failed: {:?}", e);
        return Err(Box::new(Error::new(
            ErrorKind::Other,
            "Process start failed",
        )));
    }

    let mut sys = System::new_all();
    sys.refresh_processes();
    if let Some(pid) = find_process(path.clone()) {
        println!("Process started: {:?}", sys.process(pid).unwrap().exe());
        return Ok(());
    }

    Err(Box::new(Error::new(
        ErrorKind::Other,
        "Process start failed",
    )))
}

pub fn kill_process(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut sys = System::new_all();
    sys.refresh_processes();
    if let Some(pid) = find_process(path) {
        let process = sys.process(pid).unwrap();
        if process.kill() {
            println!("Process killed: {:?}", process.exe());
            return Ok(());
        } else {
            return Err(Box::new(Error::new(
                ErrorKind::Other,
                "Process kill failed",
            )));
        }
    }
    println!("Process not found: {:?}", path);
    Ok(())
}

#[test]
fn test() {
    const proc: &str = "D:\\tools_large\\COEIROINK_WIN_GPU_v.2.1.1\\engine\\engine.exe";
    println!("{}", find_process(proc).is_some());
    println!("{}", exec_process(proc).is_ok());
    println!("{}", find_process(proc).is_some());
    println!("{}", kill_process(proc).is_ok());
}
