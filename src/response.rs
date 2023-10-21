use shiorust::message::Response;

const CRLF: &str = "\r\n";

#[derive(Debug)]
pub struct PluginResponse {
    pub response: Response,
}

impl std::fmt::Display for PluginResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "PLUGIN/{} {}{}{}{}",
            self.response.version, self.response.status, CRLF, self.response.headers, CRLF
        )
    }
}
