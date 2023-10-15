use shiorust::message::{
    parser::{ParseError, ParseErrorKind},
    parts::*,
    Parser, Request,
};
use std::str::FromStr;

pub struct PluginRequest {
    pub request: Request,
}

impl Parser<PluginRequest, (Method, Version)> for PluginRequest {
    fn parse(request_str: &str) -> Result<PluginRequest, ParseError> {
        let ((method, version), headers) = Self::parse_general(request_str)?;
        Ok(PluginRequest {
            request: Request {
                method,
                version,
                headers,
            },
        })
    }

    fn parse_initial_line(request_line: &str) -> Result<(Method, Version), ParseError> {
        if let Some(index) = request_line.find(" PLUGIN/") {
            let method = &request_line[..index];
            let version = &request_line[index + 8..];
            if let (Ok(method), Ok(version)) =
                (Method::from_str(method), Version::from_str(version))
            {
                return Ok((method, version));
            }
        }
        return Err(ParseError {
            kind: ParseErrorKind::RequestLine,
        });
    }
}
