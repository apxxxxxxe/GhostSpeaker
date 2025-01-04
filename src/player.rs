use crate::variables::*;
use once_cell::sync::Lazy;
use rodio::{Decoder, OutputStream, Sink};
use std::io::BufReader;
use std::io::Cursor;
use std::sync::Mutex;

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
  while !sink.empty() {
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
