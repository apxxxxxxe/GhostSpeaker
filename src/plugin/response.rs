use crate::common::CRLF;
use shiorust::message::parts::*;

#[derive(Debug)]
pub struct PluginResponse {
  pub version: Version,
  pub status: Status,
  pub headers: Headers,
}

impl std::fmt::Display for PluginResponse {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(
      f,
      "PLUGIN/{} {}{}{}{}",
      self.version, self.status, CRLF, self.headers, CRLF
    )
  }
}
