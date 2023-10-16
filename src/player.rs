use std::io::BufReader;
use std::io::Cursor;

use rodio::{Decoder, OutputStream, Sink};

use crate::variables::get_global_vars;

pub struct Wave {
    pub data: Vec<u8>,
    pub duration_ms: u64,
}

pub fn play_wav(wav: Vec<u8>) {
    // Get a output stream handle to the default physical sound device
    let (_stream, stream_handle) = OutputStream::try_default().unwrap();
    let sink = Sink::try_new(&stream_handle).unwrap();
    sink.set_volume(get_global_vars().volume.unwrap_or(1.0));
    let file = BufReader::new(Cursor::new(wav));
    // Decode that sound file into a source
    let source = Decoder::new(file).unwrap();
    // Play the sound directly on the device
    sink.append(source);

    // Wait until the sound has finished playing or has been stopped manually
    sink.sleep_until_end();
}
