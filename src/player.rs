use crate::variables::VOLUME;
use log::error;
use rodio::{Decoder, OutputStream, Sink};
use std::io::BufReader;
use std::io::Cursor;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

// 音声再生の最大時間（60秒）
const MAX_AUDIO_PLAY_TIME: Duration = Duration::from_secs(60);

pub(crate) static FORCE_STOP_SINK: AtomicBool = AtomicBool::new(false);

pub(crate) fn play_wav(wav: Vec<u8>) -> Result<(), Box<dyn std::error::Error>> {
  let (_stream, handle) = OutputStream::try_default()?;
  let sink = Sink::try_new(&handle)?;
  let volume = match VOLUME.read() {
    Ok(v) => *v,
    Err(e) => {
      error!("Failed to read volume: {}", e);
      1.0 // デフォルト音量
    }
  };
  sink.set_volume(volume);
  let file = BufReader::new(Cursor::new(wav));
  match Decoder::new(file) {
    Ok(source) => {
      sink.append(source);
    }
    Err(e) => return Err(Box::new(e)),
  };
  let start_time = Instant::now();
  while !sink.empty() {
    // タイムアウトチェック
    if start_time.elapsed() >= MAX_AUDIO_PLAY_TIME {
      error!("Audio playback timeout exceeded, stopping playback");
      sink.pause();
      sink.stop();
      return Ok(());
    }

    std::thread::sleep(std::time::Duration::from_millis(50));
    // シャットダウンチェック
    if crate::queue::SHUTTING_DOWN.load(std::sync::atomic::Ordering::Acquire) {
      sink.pause();
      sink.stop();
      return Ok(());
    }
    if FORCE_STOP_SINK.load(Ordering::Acquire) {
      sink.pause();
      sink.stop();
      FORCE_STOP_SINK.store(false, Ordering::Release);
      return Ok(());
    }
  }
  Ok(())
}
