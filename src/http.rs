use crate::{REQUEST_CAP, SEP};

const VERSIONS: [&[u8]; 3] = [b"HTTP/1.0", b"HTTP/1.1", b"HTTP/2"];
const METHODS: [&[u8]; 8] = [
    b"GET", b"HEAD", b"POST", b"PUT", b"DELETE", b"OPTIONS", b"PATCH", b"TRACE",
];

const METHOD_CAP: usize = 7;
const PATH_CAP: usize = REQUEST_CAP;
const VERSION_CAP: usize = 8;

/// Represents a simplified HTTP request message.
pub struct RequestMessage {
    pub method: Vec<u8>,
    pub path: Vec<u8>,
    pub http: Vec<u8>,
}

impl RequestMessage {
    /// Creates a new and empty RequestMessage.
    pub fn new() -> Self {
        Self {
            method: Vec::with_capacity(METHOD_CAP),
            path: Vec::with_capacity(PATH_CAP),
            http: Vec::with_capacity(VERSION_CAP),
        }
    }

    /// Checks if the method is supported.
    pub fn is_method_valid(&self) -> bool {
        matches!(self.method.as_slice(), m if METHODS.contains(&m))
    }

    /// Checks if the HTTP version is supported.
    pub fn is_http_valid(&self) -> bool {
        matches!(self.http.as_slice(), v if VERSIONS.contains(&v))
    }
}

impl From<&Vec<u8>> for RequestMessage {
    fn from(value: &Vec<u8>) -> Self {
        let mut result = RequestMessage::new();

        let slice: &[u8] = if value.len() > REQUEST_CAP {
            &value[..REQUEST_CAP]
        } else {
            &value[..]
        };

        for (i, v) in slice.splitn(3, |i| i == &SEP[0]).enumerate() {
            match i {
                0 => result.method.extend_from_slice(v),
                1 => result.path.extend_from_slice(v),
                2 => result.http.extend_from_slice(v),
                _ => break,
            };
        }

        result
    }
}

/// Represents a simplified HTTP (response) message.
pub struct ResponseMessage<'a, 'b> {
    pub http: &'a [u8],
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
        let data = b"GET /test HTTP/1.1".to_vec();

        let result = RequestMessage::from(&data);

        assert!(result.type_id() == TypeId::of::<RequestMessage>());
        assert!(result.method.as_slice() == b"GET");
        assert!(result.path.as_slice() == b"/test");
        assert!(result.http.as_slice() == b"HTTP/1.1");
    }

    #[test]
    fn test_request_message_from_with_empty_http() {
        let data = b"GET /too-long-message".to_vec();

        let result = RequestMessage::from(&data);

        assert!(result.type_id() == TypeId::of::<RequestMessage>());
        assert!(result.method.as_slice() == b"GET");
        assert!(result.path.as_slice() == b"/too-long-message");
        assert!(result.http.is_empty());
    }

    #[test]
    fn test_request_message_from_with_empty_path() {
        let data = b"GET".to_vec();

        let result = RequestMessage::from(&data);

        assert!(result.type_id() == TypeId::of::<RequestMessage>());
        assert!(result.method.as_slice() == b"GET");
        assert!(result.path.is_empty());
        assert!(result.http.is_empty());
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
