use std::env;
use std::net::{Ipv4Addr, SocketAddrV4};

use async_signals::Signals;
use async_std::io::ErrorKind;
use async_std::net::{TcpListener, TcpStream};
use async_std::prelude::*;
use async_std::task;

use std::ops::{Index, IndexMut};

const CRLF: [u8; 2] = [13, 10];
const SEP: u8 = 32;

const BUFFER_LENGTH: usize = 16;
const REQUEST_CAP: usize = BUFFER_LENGTH * 4096; // 65536 should be enough

const VERSIONS: [BytesSlice; 3] = [b"HTTP/1.0", b"HTTP/1.1", b"HTTP/2"];
const METHODS: [BytesSlice; 8] = [
    b"GET", b"HEAD", b"POST", b"PUT", b"DELETE", b"OPTIONS", b"PATCH", b"TRACE",
];

/// The slice of bytes type.
///
/// It is a covenient alias for `&[u8]`.
type BytesSlice<'a> = &'a [u8];

/// The vector of bytes type.
///
/// It is a covenient alias for `Vec<u8>`.
type BytesVec = Vec<u8>;

/// Represents a simplified HTTP (request) message.
struct HTTPRequestMessage {
    method: BytesVec,
    path: BytesVec,
    http: BytesVec,
}

impl HTTPRequestMessage {
    /// Creates a new and empty HTTPMessage.
    fn new() -> Self {
        let meth_cap = max_len(METHODS.to_vec());
        let vers_cap = max_len(VERSIONS.to_vec());
        let path_cap = REQUEST_CAP - meth_cap - vers_cap;

        Self {
            method: Vec::with_capacity(meth_cap),
            path: Vec::with_capacity(path_cap),
            http: Vec::with_capacity(vers_cap),
        }
    }

    /// Checks if the method is supported.
    fn is_method_valid(&self) -> bool {
        match self.method.as_slice() {
            m if METHODS.contains(&m) => true,
            _ => false,
        }
    }

    /// Checks if the HTTP version is supported.
    fn is_http_valid(&self) -> bool {
        match self.http.as_slice() {
            v if VERSIONS.contains(&v) => true,
            _ => false,
        }
    }
}

impl Index<usize> for HTTPRequestMessage {
    type Output = BytesVec;

    fn index(&self, index: usize) -> &BytesVec {
        match index {
            0 => &self.method,
            1 => &self.path,
            2 => &self.http,
            _ => panic!("invalid index {index}"),
        }
    }
}

impl IndexMut<usize> for HTTPRequestMessage {
    fn index_mut(&mut self, index: usize) -> &mut BytesVec {
        match index {
            0 => &mut self.method,
            1 => &mut self.path,
            2 => &mut self.http,
            _ => panic!("invalid index {index}"),
        }
    }
}

impl From<&BytesVec> for HTTPRequestMessage {
    fn from(data: &BytesVec) -> Self {
        let mut result = HTTPRequestMessage::new();

        for (i, v) in data.splitn(3, |i| i == &SEP).enumerate() {
            result[i].extend_from_slice(v);
        }

        result
    }
}

struct HTTPResponseMessage<'a, 'b> {
    http: BytesSlice<'a>,
    code: u16,
    desc: BytesSlice<'b>,
}

impl<'a, 'b> HTTPResponseMessage<'a, 'b> {
    fn new() -> Self {
        Self {
            http: VERSIONS[1],
            code: 404,
            desc: b"Not Found",
        }
    }

    fn status(&mut self, code: u16, desc: BytesSlice<'b>) {
        self.code = code;
        self.desc = desc;
    }

    fn to_vec(&self) -> BytesVec {
        [
            self.http,
            &[SEP],
            self.code.to_string().as_bytes(),
            &[SEP],
            self.desc,
            &CRLF,
            &CRLF,
        ]
        .concat()
    }
}

// HACK: because constant unwrap methods are unstable yet
/// Returns the maximum length of bytes in a given sequence.
fn max_len(arr: Vec<BytesSlice>) -> usize {
    arr.iter().map(|i| i.len()).max().unwrap_or_default()
}

/// Extracts the first line of a message if anything is there.
async fn extract(mut stream: &TcpStream) -> BytesVec {
    let mut request: BytesVec = Vec::with_capacity(REQUEST_CAP);
    let mut buf = [0 as u8; BUFFER_LENGTH];

    loop {
        match stream.read(&mut buf).await {
            Ok(mut size) if size > 0 => {
                if let Some(pos) = buf.iter().position(|i| i == &CRLF[0]) {
                    size = pos;
                }

                if request.len() + size > REQUEST_CAP {
                    size = REQUEST_CAP - request.len();
                }

                request.extend(&buf[0..size]);

                if size < BUFFER_LENGTH {
                    break;
                }
            }
            Ok(_) => break,
            Err(_) => break,
        }
    }

    request
}

#[async_std::main]
async fn main() {
    task::spawn(async {
        // NOTE: SIGHUP = 1, SIGINT = 2, SIGTERM = 15
        let mut signals = Signals::new([1, 2, 15]).unwrap();

        while let Some(_) = signals.next().await {
            println!("Quitting");
            std::process::exit(0);
        }
    });

    let port: u16 = match env::var("PORT") {
        Ok(value) => match value.parse::<u16>() {
            Ok(port) => port,
            Err(_) => {
                eprintln!("Invalid port; Quitting");
                std::process::exit(1);
            }
        },
        Err(_) => 8080,
    };

    let addr = SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), port);

    let listener = match TcpListener::bind(addr).await {
        Ok(listener) => {
            println!("Listening on {}", addr);
            listener
        }
        Err(ref e) => {
            println!("Cannot listen on {}: {}", addr, e);
            return;
        }
    };

    let mut incoming = listener.incoming();

    while let Some(stream) = incoming.next().await {
        let mut stream = match stream {
            Ok(stream) => stream,
            Err(_) => continue,
        };
        stream.set_nodelay(true).ok(); // we do not really care if it clicks or not

        task::spawn(async move {
            let mut response = HTTPResponseMessage::new();

            let data = extract(&stream).await;

            if !data.is_empty() && data.is_ascii() {
                let message = HTTPRequestMessage::from(&data);

                if !message.is_method_valid() {
                    response.status(405, b"Method Not Allowed");
                } else if message.path.is_empty() || message.http.is_empty() {
                    response.status(414, b"URI Too Long");
                } else if !message.is_http_valid() {
                    response.status(505, b"HTTP Version Not Supported");
                } else if message.path == b"/healthz" {
                    response.status(200, b"OK"); // I would prefer 204 though
                }
            } else {
                response.status(400, b"Bad Request");
            }

            if let Some(e) = stream.write_all(response.to_vec().as_slice()).await.err() {
                if e.kind() != ErrorKind::WouldBlock {
                    return;
                }
            }

            stream.flush().await.ok();
        });
    }
}

#[cfg(test)]
mod tests {
    use std::any::{Any, TypeId};

    use super::*;

    #[test]
    fn test_http_message_from() {
        let data = b"GET /test HTTP/1.1".to_vec();

        let result = HTTPRequestMessage::from(&data);

        assert!(result.type_id() == TypeId::of::<HTTPRequestMessage>());
        assert!(result.method.as_slice() == b"GET");
        assert!(result.path.as_slice() == b"/test");
        assert!(result.http.as_slice() == b"HTTP/1.1");
    }

    #[test]
    fn test_http_message_from_with_empty_http() {
        let data = b"GET /too-long-message".to_vec();

        let result = HTTPRequestMessage::from(&data);

        assert!(result.type_id() == TypeId::of::<HTTPRequestMessage>());
        assert!(result.method.as_slice() == b"GET");
        assert!(result.path.as_slice() == b"/too-long-message");
        assert!(result.http.is_empty());
    }

    #[test]
    fn test_http_message_from_with_empty_path() {
        let data = b"GET".to_vec();

        let result = HTTPRequestMessage::from(&data);

        assert!(result.type_id() == TypeId::of::<HTTPRequestMessage>());
        assert!(result.method.as_slice() == b"GET");
        assert!(result.path.is_empty());
        assert!(result.http.is_empty());
    }
}
