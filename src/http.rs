use crate::{REQUEST_CAP, SEP};

type Version<'v> = &'v [u8];
type Method<'m> = &'m [u8];
type Path<'p> = &'p [u8];

const VERSIONS: [Version; 3] = [b"HTTP/1.0", b"HTTP/1.1", b"HTTP/2"];
const METHODS: [Method; 8] = [
    b"GET", b"HEAD", b"POST", b"PUT", b"DELETE", b"OPTIONS", b"PATCH", b"TRACE",
];

const VERSION_CAP: usize = 8;
const METHOD_CAP: usize = 7;
const PATH_CAP: usize = REQUEST_CAP - VERSION_CAP - METHOD_CAP - 2;

/// Represents a simplified HTTP request message.
pub struct RequestMessage<'a> {
    pub method: Method<'a>,
    pub path: Path<'a>,
    pub http: Version<'a>,
}

impl<'a> RequestMessage<'a> {
    /// Creates a new and empty RequestMessage.
    pub fn new() -> Self {
        Self {
            method: b"",
            path: b"",
            http: b"",
        }
    }

    /// Checks if the method is supported.
    pub fn is_method_valid(&self) -> bool {
        METHODS.contains(&self.method)
    }

    /// Checks if the HTTP version is supported.
    pub fn is_http_valid(&self) -> bool {
        VERSIONS.contains(&self.http)
    }
}

impl<'a> From<&'a [u8]> for RequestMessage<'a> {
    fn from(value: &'a [u8]) -> Self {
        let mut result = RequestMessage::new();

        let slice: &[u8] = if value.len() > REQUEST_CAP {
            &value[..REQUEST_CAP]
        } else {
            value
        };

        for (num, src) in slice.splitn(3, |char| char == &SEP[0]).enumerate() {
            match num {
                0 if src.len() > METHOD_CAP => result.method = &src[..METHOD_CAP],
                1 if src.len() > PATH_CAP => result.path = &src[..PATH_CAP],
                2 if src.len() > VERSION_CAP => result.http = &src[..VERSION_CAP],
                0 => result.method = src,
                1 => result.path = src,
                2 => result.http = src,
                _ => unreachable!(),
            };
        }

        result
    }
}

/// Represents a simplified HTTP (response) message.
pub struct ResponseMessage<'a, 'b> {
    pub http: Version<'a>,
    pub code: u16,
    pub desc: &'b [u8],
    pub headers: [&'b [u8]; 1],
}

impl<'a, 'b> ResponseMessage<'a, 'b> {
    /// Creates a new ResponseMessage with a given status code and description.
    pub const fn with_status(code: u16, desc: &'b [u8]) -> ResponseMessage<'a, 'b> {
        ResponseMessage {
            http: VERSIONS[1],
            code,
            desc,
            headers: [b"Connection: close"],
        }
    }
}

#[cfg(test)]
mod tests {
    use std::any::{Any, TypeId};

    use super::*;

    #[test]
    fn test_request_message_from() {
        let data = b"GET /test HTTP/1.1";

        let result = RequestMessage::from(data.as_slice());

        assert!(result.type_id() == TypeId::of::<RequestMessage>());
        assert!(result.method == b"GET");
        assert!(result.path == b"/test");
        assert!(result.http == b"HTTP/1.1");
    }

    #[test]
    fn test_request_message_from_with_empty_http() {
        let data = b"GET /too-long-path";

        let result = RequestMessage::from(data.as_slice());

        assert!(result.type_id() == TypeId::of::<RequestMessage>());
        assert!(result.method == b"GET");
        assert!(result.path == b"/too-long-path");
        assert!(result.http.is_empty());
    }

    #[test]
    fn test_request_message_from_with_empty_path() {
        let data = b"GET";

        let result = RequestMessage::from(data.as_slice());

        assert!(result.type_id() == TypeId::of::<RequestMessage>());
        assert!(result.method == b"GET");
        assert!(result.path.is_empty());
        assert!(result.http.is_empty());
    }

    #[test]
    fn test_request_message_from_with_longer_method() {
        let data = b"OPTIONSBUTLONGER /test HTTP/1.1";

        let result = RequestMessage::from(data.as_slice());

        assert!(result.type_id() == TypeId::of::<RequestMessage>());
        assert!(result.method == b"OPTIONS");
        assert!(result.path == b"/test");
        assert!(result.http == b"HTTP/1.1");
    }

    #[test]
    fn test_response_message_with_status() {
        let result = ResponseMessage::with_status(204, b"No Content");

        assert!(result.type_id() == TypeId::of::<ResponseMessage>());
        assert!(result.http == b"HTTP/1.1");
        assert!(result.code == 204);
        assert!(result.desc == b"No Content");
        assert!(result.headers[0] == b"Connection: close");
    }
}
