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

/// 指定ポートを使用しているエンジンの実行ファイルパスを探す。
/// HTTPで通信するエンジンの接続判定には使わず、ENGINE_PATH の補完にのみ使う
/// （OSの接続テーブルは信頼しきれないため）。
pub async fn get_port_opener_path(port: u16, shutting_down: &'static AtomicBool) -> Option<String> {
  let sd = shutting_down;
  match tokio::task::spawn_blocking(move || get_port_opener_path_sync(port, sd)).await {
    Ok(result) => result,
    Err(e) => {
      log::error!("spawn_blocking failed in get_port_opener_path: {}", e);
      None
    }
  }
}

/// netstat -ano の出力から、指定ローカルポートを持つ行のPIDを優先度順に返す。
/// 待ち受け行（相手ポートが0）を優先し、無ければ接続中の行で代用する。
/// 状態の文字列には依存しない（待ち受け行が正しく報告されない環境への対策）。
fn parse_port_owner_pids(netstat_output: &str, port: u16) -> Vec<usize> {
  let mut listeners = Vec::new();
  let mut others = Vec::new();
  for line in netstat_output.lines() {
    let parts: Vec<&str> = line.split_whitespace().collect();
    let [proto, local, foreign, _state, pid] = parts.as_slice() else {
      continue;
    };
    if !proto.eq_ignore_ascii_case("TCP") {
      continue;
    }
    let local_port = local.rsplit(':').next().and_then(|p| p.parse::<u16>().ok());
    if local_port != Some(port) {
      continue;
    }
    // PID 0 は TIME_WAIT など、所有プロセスのない行
    let Some(pid) = pid.parse::<usize>().ok().filter(|p| *p != 0) else {
      continue;
    };
    let target = if foreign.ends_with(":0") {
      &mut listeners
    } else {
      &mut others
    };
    if !target.contains(&pid) {
      target.push(pid);
    }
  }
  for pid in others {
    if !listeners.contains(&pid) {
      listeners.push(pid);
    }
  }
  listeners
}

/// netstat -ano を実行して出力を返す
fn run_netstat() -> Option<String> {
  use std::os::windows::process::CommandExt;

  let output = match Command::new("netstat")
    .arg("-ano")
    .creation_flags(CREATE_NO_WINDOW)
    .output()
  {
    Ok(output) => output,
    Err(e) => {
      log::error!("Failed to run netstat: {}", e);
      return None;
    }
  };
  if !output.status.success() {
    log::error!("netstat failed: {}", output.status);
    return None;
  }
  // ヘッダ行はOSの言語に依存した文字コードになるため lossy で読む
  Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// 指定ポートを使っているプロセスが netstat に現れるかを返す
pub fn is_port_in_use(port: u16) -> bool {
  run_netstat().is_some_and(|output| !parse_port_owner_pids(&output, port).is_empty())
}

fn get_port_opener_path_sync(port: u16, shutting_down: &AtomicBool) -> Option<String> {
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

  let pids = parse_port_owner_pids(&run_netstat()?, port);
  if pids.is_empty() {
    log::warn!("netstat reported no process for port {}", port);
    return None;
  }

  // チェックポイント3: refresh_processes()呼び出し直前（クラッシュサイト防御）
  if shutting_down.load(Ordering::Acquire) {
    log::debug!(
      "shutting down before refresh_processes, skipping port check for {}",
      port
    );
    return None;
  }

  let system = refreshed_system(&mut guard)?;
  for pid in pids {
    if let Some(path) = extract_parent_process_path(Pid::from(pid), system) {
      return Some(path);
    }
    log::error!("Failed to extract parent process for pid: {}", pid);
  }
  None
}

/// キャッシュ済みの System を最新のプロセス一覧に更新して返す。
/// sysinfo 呼び出しは catch_unwind で保護する。
fn refreshed_system(cache: &mut Option<System>) -> Option<&mut System> {
  if cache.is_none() {
    log::debug!("Creating new System instance");
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(System::new)) {
      Ok(system) => *cache = Some(system),
      Err(e) => {
        log::error!("sysinfo System::new() panicked: {:?}", e);
        return None;
      }
    }
  }
  let refresh_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    if let Some(system) = cache.as_mut() {
      system.refresh_processes();
    }
  }));
  if let Err(e) = refresh_result {
    log::error!("sysinfo refresh_processes panicked: {:?}", e);
    // パニック後のSystemは不定状態なので破棄
    *cache = None;
    return None;
  }
  cache.as_mut()
}

/// 指定した名前のプロセスが動いているかを返す
pub fn is_process_running(process_name: &str) -> bool {
  let mut guard = match PORT_OPENER_MUTEX.lock() {
    Ok(g) => g,
    Err(e) => {
      log::error!("Failed to lock PORT_OPENER_MUTEX: {}", e);
      return false;
    }
  };
  let Some(system) = refreshed_system(&mut guard) else {
    return false;
  };
  let running = system
    .processes_by_name(process_name)
    .any(|process| process.name() == process_name);
  running
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
  if let Ok(current_exe) = std::env::current_exe() {
    if path == current_exe.as_path() {
      return true;
    }
  }
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

  // prevent launching the worker itself
  if let Ok(current_exe) = std::env::current_exe() {
    if Path::new(path) == current_exe.as_path() {
      return Err(
        format!(
          "engine path for {} points to the worker itself",
          engine.name()
        )
        .into(),
      );
    }
  }

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

  {
    use std::os::windows::process::CommandExt;
    Command::new(path)
      .stdin(std::process::Stdio::null())
      .stdout(std::process::Stdio::null())
      .stderr(std::process::Stdio::null())
      .creation_flags(CREATE_NO_WINDOW)
      .spawn()?;
  }
  log::debug!("booted {}", engine.name());
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  const SAMPLE: &str = "
Active Connections

  Proto  Local Address          Foreign Address        State           PID
  TCP    0.0.0.0:135            0.0.0.0:0              LISTENING       50021
  TCP    127.0.0.1:49552        127.0.0.1:50021        ESTABLISHED     33808
  TCP    127.0.0.1:50021        0.0.0.0:0              LISTENING       35504
  TCP    127.0.0.1:50021        127.0.0.1:60681        ESTABLISHED     35504
  TCP    127.0.0.1:60688        127.0.0.1:50021        TIME_WAIT       0
  UDP    0.0.0.0:50021          *:*                                    4444
";

  #[test]
  fn finds_listener_by_exact_local_port() {
    assert_eq!(parse_port_owner_pids(SAMPLE, 50021), vec![35504]);
  }

  #[test]
  fn does_not_match_port_number_inside_pid() {
    // PID 50021 のプロセスはポート135を使っているだけなので対象外
    assert!(!parse_port_owner_pids(SAMPLE, 50021).contains(&50021));
    assert_eq!(parse_port_owner_pids(SAMPLE, 135), vec![50021]);
  }

  #[test]
  fn ignores_client_side_rows_and_udp() {
    let pids = parse_port_owner_pids(SAMPLE, 50021);
    assert!(!pids.contains(&33808));
    assert!(!pids.contains(&4444));
  }

  #[test]
  fn falls_back_to_established_row_without_listener() {
    let output = "  TCP    127.0.0.1:50021        127.0.0.1:60681        ESTABLISHED     35504\n";
    assert_eq!(parse_port_owner_pids(output, 50021), vec![35504]);
  }

  #[test]
  fn listener_is_detected_even_with_wrong_state_label() {
    let output = "  TCP    127.0.0.1:50021        0.0.0.0:0              CLOSED          35504
  TCP    127.0.0.1:50021        127.0.0.1:60681        ESTABLISHED     777
";
    assert_eq!(parse_port_owner_pids(output, 50021), vec![35504, 777]);
  }

  #[test]
  fn supports_ipv6_rows() {
    let output = "  TCP    [::1]:50021            [::]:0                 LISTENING       35504\n";
    assert_eq!(parse_port_owner_pids(output, 50021), vec![35504]);
  }

  #[test]
  fn returns_empty_when_no_row_matches() {
    assert!(parse_port_owner_pids("", 50021).is_empty());
    assert!(parse_port_owner_pids(SAMPLE, 10101).is_empty());
  }
}
