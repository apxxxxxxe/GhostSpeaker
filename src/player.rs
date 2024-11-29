use crate::variables::get_global_vars;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use std::io::BufReader;
use std::io::Cursor;

static mut PLAYER: Option<Player> = None;

pub struct Player {
  // 直接アクセスされることはない
  // ただし、drop時にストリームが閉じられるため、変数として保持しておく必要がある
  _stream: OutputStream,
  _stream_handle: OutputStreamHandle,

  pub sink: Sink,
}

impl Player {
  pub fn new() -> Self {
    let (stream, stream_handle) = OutputStream::try_default().unwrap();
    let sink = Sink::try_new(&stream_handle).unwrap();
    Player {
      _stream: stream,
      _stream_handle: stream_handle,
      sink,
    }
  }

  fn reset_device(&mut self) {
    let (stream, stream_handle) = OutputStream::try_default().unwrap();
    self._stream = stream;
    self._stream_handle = stream_handle;
    self.sink = Sink::try_new(&self._stream_handle).unwrap();
  }
}

pub fn force_free_player() {
  debug!("free_player");
  get_player().sink.pause();
  get_player().sink.stop();
  unsafe {
    PLAYER = None;
  }
  debug!("force_free_player done");
}

pub fn cooperative_free_player() {
  debug!("sleep until end");
  while !get_player().sink.empty() {
    std::thread::sleep(std::time::Duration::from_millis(100));
  }
  get_player().sink.pause();
  get_player().sink.stop();
  unsafe {
    PLAYER = None;
  }
  debug!("cooperative_free_player done");
}

pub fn get_player() -> &'static mut Player {
  if unsafe { PLAYER.is_none() } {
    unsafe {
      PLAYER = Some(Player::new());
    }
  }
  unsafe { PLAYER.as_mut().unwrap() }
}

pub fn play_wav(wav: Vec<u8>) {
  let player = get_player();
  if player.sink.empty() {
    // 再生する前に、一度デバイスをリセットする
    // 再生デバイスが変更されていた場合に対応するため
    player.reset_device();
  }
  player
    .sink
    .set_volume(get_global_vars().volume.unwrap_or(1.0));
  let file = BufReader::new(Cursor::new(wav));
  match Decoder::new(file) {
    Ok(source) => {
      player.sink.append(source);
    }
    Err(e) => {
      error!("Error: {}", e);
    }
  }
}
*/
