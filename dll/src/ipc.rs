use ghost_speaker_common::{Command, Response};
use log::{debug, error};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::process::{Child, Stdio};
use std::sync::Mutex;

struct WorkerConnection {
  child: Child,
  writer: BufWriter<std::process::ChildStdin>,
  reader: BufReader<std::process::ChildStdout>,
}

static WORKER: Mutex<Option<WorkerConnection>> = Mutex::new(None);

pub(crate) fn spawn_worker(dll_dir: &str) -> Result<(), String> {
  let worker_path = std::path::Path::new(dll_dir).join("ghost_speaker_worker.exe");
  if !worker_path.exists() {
    return Err(format!(
      "Worker executable not found: {}",
      worker_path.display()
    ));
  }

  #[cfg(windows)]
  let child = {
    use std::os::windows::process::CommandExt;
    std::process::Command::new(&worker_path)
      .stdin(Stdio::piped())
      .stdout(Stdio::piped())
      .stderr(Stdio::null())
      .creation_flags(winapi::um::winbase::CREATE_NO_WINDOW)
      .spawn()
      .map_err(|e| format!("Failed to spawn worker: {}", e))?
  };

  #[cfg(not(windows))]
  let child = {
    std::process::Command::new(&worker_path)
      .stdin(Stdio::piped())
      .stdout(Stdio::piped())
      .stderr(Stdio::null())
      .spawn()
      .map_err(|e| format!("Failed to spawn worker: {}", e))?
  };

  let mut child = child;
  let stdin = child.stdin.take().ok_or("Failed to take worker stdin")?;
  let stdout = child.stdout.take().ok_or("Failed to take worker stdout")?;

  let writer = BufWriter::new(stdin);
  let reader = BufReader::new(stdout);

  let mut guard = WORKER
    .lock()
    .map_err(|e| format!("Failed to lock WORKER: {}", e))?;
  *guard = Some(WorkerConnection {
    child,
    writer,
    reader,
  });

  debug!("Worker spawned successfully");
  Ok(())
}

pub(crate) fn send_command(cmd: &Command) -> Result<Response, String> {
  let mut guard = WORKER
    .lock()
    .map_err(|e| format!("Failed to lock WORKER: {}", e))?;

  let conn = guard
    .as_mut()
    .ok_or_else(|| "Worker not running".to_string())?;

  // Check if worker is still alive
  match conn.child.try_wait() {
    Ok(Some(status)) => {
      error!("Worker process exited with status: {}", status);
      *guard = None;
      return Err(format!("Worker process exited: {}", status));
    }
    Ok(None) => {} // still running
    Err(e) => {
      error!("Failed to check worker status: {}", e);
    }
  }

  let json = serde_json::to_string(cmd).map_err(|e| format!("Serialize error: {}", e))?;

  conn
    .writer
    .write_all(json.as_bytes())
    .map_err(|e| format!("Write error: {}", e))?;
  conn
    .writer
    .write_all(b"\n")
    .map_err(|e| format!("Write newline error: {}", e))?;
  conn
    .writer
    .flush()
    .map_err(|e| format!("Flush error: {}", e))?;

  let mut line = String::new();
  conn
    .reader
    .read_line(&mut line)
    .map_err(|e| format!("Read error: {}", e))?;

  if line.is_empty() {
    *guard = None;
    return Err("Worker closed connection".to_string());
  }

  serde_json::from_str(line.trim()).map_err(|e| format!("Deserialize error: {}", e))
}

/// Send a command without caring about errors (fire-and-forget style logging)
pub(crate) fn send_command_logged(cmd: &Command) -> Option<Response> {
  match send_command(cmd) {
    Ok(resp) => Some(resp),
    Err(e) => {
      error!("IPC command failed: {}", e);
      // Try to respawn worker once
      try_respawn_worker();
      None
    }
  }
}

fn try_respawn_worker() {
  let dll_dir = match crate::variables::DLL_DIR.read() {
    Ok(d) => d.clone(),
    Err(_) => return,
  };
  if dll_dir.is_empty() {
    return;
  }
  debug!("Attempting to respawn worker...");
  if let Err(e) = spawn_worker(&dll_dir) {
    error!("Worker respawn failed: {}", e);
  }
}

pub(crate) fn shutdown_worker() -> Result<(), String> {
  // GracefulShutdown コマンドを送信
  let _ = send_command(&Command::GracefulShutdown);

  // ワーカー接続をドロップ（パイプを閉じる）
  // ワーカーの終了を待たない → detach
  let mut guard = WORKER
    .lock()
    .map_err(|e| format!("Failed to lock WORKER: {}", e))?;
  if let Some(conn) = guard.take() {
    drop(conn); // パイプクローズ、プロセスハンドル解放
    debug!("Worker detached for graceful shutdown");
  }

  Ok(())
}
