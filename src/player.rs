use std::io::BufReader;
use std::io::Cursor;

use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use once_cell::sync::Lazy;

use crate::variables::get_global_vars;

static mut STREAM_HANDLE: Lazy<OutputStreamHandle> = Lazy::new(|| {
    let (_stream, stream_handle) = OutputStream::try_default().unwrap();
    stream_handle
});

pub struct Wave {
    pub data: Vec<u8>,
    pub duration_ms: u64,
}

pub async fn play_wav(wav: Vec<u8>) {
    let stream_handle = unsafe { &*STREAM_HANDLE };
    // Get a output stream handle to the default physical sound device
    let sink = Sink::try_new(&stream_handle).unwrap();
    sink.set_volume(get_global_vars().volume.unwrap_or(1.0));
    let file = BufReader::new(Cursor::new(wav));
    // Decode that sound file into a source
    let source = Decoder::new(file).unwrap();
    // Play the sound directly on the device
    sink.append(source);
    sink.sleep_until_end();
}
