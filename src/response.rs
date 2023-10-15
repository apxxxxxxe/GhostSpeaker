use shiorust::message::{parts::HeaderName, parts::*, traits::*, Response};

const CRLF: &str = "\r\n";

pub enum ResponseError {
    DecodeFailed,
}

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

impl PluginResponse {
    pub fn new() -> PluginResponse {
        let mut headers = Headers::new();
        headers.insert(
            HeaderName::Standard(StandardHeaderName::Charset),
            String::from("UTF-8"),
        );

        PluginResponse {
            response: Response {
                version: Version::V20,
                status: Status::OK,
                headers,
            },
        }
    }

    pub fn new_nocontent() -> PluginResponse {
        let mut r = PluginResponse::new();
        r.response.status = Status::NoContent;
        r
    }
}
