pub mod predict;
pub mod speaker;

use crate::engine::ENGINE_BOUYOMICHAN;
use std::error::Error;
use std::io::Write;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};

pub fn connect() -> Result<TcpStream, Box<dyn Error>> {
  let address = SocketAddr::new(
    IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
    ENGINE_BOUYOMICHAN.port as u16,
  );
  let stream = TcpStream::connect(address)?;

  Ok(stream)
}

pub fn speak(text: &str, voice: i16, volume: i16) -> Result<(), Box<dyn Error>> {
  let encoded_text = text.as_bytes();
  let header = make_header(voice, volume, encoded_text.len());

  println!("header: {:?}", header);

  let mut stream = connect()?;
  stream.write_all(&header)?;
  stream.write_all(&encoded_text)?;

  Ok(())
}

fn make_header(voice: i16, volume: i16, msg_length: usize) -> Vec<u8> {
  let command: i16 = 1;
  let speed: i16 = -1;
  let tone: i16 = -1;
  let char_code: i16 = 0;

  let mut header = vec![];
  header.extend_from_slice(&command.to_le_bytes());
  header.extend_from_slice(&speed.to_le_bytes());
  header.extend_from_slice(&tone.to_le_bytes());
  header.extend_from_slice(&volume.to_le_bytes());
  header.extend_from_slice(&voice.to_le_bytes());
  header.extend_from_slice(&char_code.to_le_bytes());
  header.extend_from_slice(&msg_length.to_le_bytes());
  header
}
