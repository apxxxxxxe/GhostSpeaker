use std::io::Cursor;

use rodio::{Decoder, OutputStream, Sink};
use std::io::BufReader;


pub struct Wave {
    pub data: Vec<u8>,
    pub duration_ms: u64,
}

pub fn play_wav(wav: Vec<u8>) {
    // Get a output stream handle to the default physical sound device
    let (_stream, stream_handle) = OutputStream::try_default().unwrap();
    let sink = Sink::try_new(&stream_handle).unwrap();
    let file = BufReader::new(Cursor::new(wav));
    // Decode that sound file into a source
    let source = Decoder::new(file).unwrap();
    // Play the sound directly on the device
    sink.append(source);

    // Wait until the sound has finished playing or has been stopped manually
    sink.sleep_until_end();
}
