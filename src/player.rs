use std::io::BufReader;
use std::io::Cursor;

use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};

use crate::variables::get_global_vars;

static mut PLAYER: Option<Player> = None;

pub struct Wave {
    pub data: Vec<u8>,
    pub duration_ms: u64,
}

pub struct Player {
    // 直接アクセスされることはない
    // ただし、drop時にストリームが閉じられるため、変数として保持しておく必要がある
    _stream: OutputStream,
    _stream_handle: OutputStreamHandle,

    sink: Sink,
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
}

pub fn free_player() {
    debug!("free_player");
    get_player().sink.pause();
    get_player().sink.stop();
    unsafe {
        PLAYER = None;
    }
    debug!("free_player done");
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
    // Get a output stream handle to the default physical sound device
    let player = get_player();
    let sink = &mut player.sink;
    sink.set_volume(get_global_vars().volume.unwrap_or(1.0));
    let file = BufReader::new(Cursor::new(wav));
    // Decode that sound file into a source
    let source = Decoder::new(file).unwrap();
    // Play the sound directly on the device
    sink.append(source);
    sink.sleep_until_end();
}
