use crate::variables::VOLUME;
use log::error;
use once_cell::sync::Lazy;
use rodio::{Decoder, OutputStream, Sink};
use std::io::BufReader;
use std::io::Cursor;
use std::sync::Mutex;
use std::time::{Duration, Instant};

// 音声再生の最大時間（60秒）
const MAX_AUDIO_PLAY_TIME: Duration = Duration::from_secs(60);

pub(crate) static FORCE_STOP_SINK: Lazy<Mutex<bool>> = Lazy::new(|| Mutex::new(false));

pub(crate) fn play_wav(wav: Vec<u8>) -> Result<(), Box<dyn std::error::Error>> {
  let (_stream, handle) = OutputStream::try_default()?;
  let sink = Sink::try_new(&handle)?;
  sink.set_volume(*VOLUME.read().unwrap());
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

    std::thread::sleep(std::time::Duration::from_millis(100));
    {
      let mut force_stop = FORCE_STOP_SINK.lock().unwrap();
      if *force_stop {
        sink.pause();
        sink.stop();
        *force_stop = false;
        return Ok(());
      }
    }
  }
  Ok(())
}
