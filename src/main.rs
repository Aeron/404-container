use std::net::{Ipv4Addr, SocketAddrV4};

use async_signals::Signals;
use async_std::io::{Error, ErrorKind, Result};
use async_std::net::{TcpListener, TcpStream};
use async_std::prelude::*;
use async_std::task;

const CRLF: [u8; 2] = [13, 10]; // simply \r\n
const BUFFER_LENGTH: usize = 16;
const REQUEST_CAP: usize = BUFFER_LENGTH * 4096; // 65536 should be enough
const RESPONSE_CAP: usize = 30 + CRLF.len() * 2;

/// Represents a simplified HTTP (request) message.
struct HTTPMessage {
    method: Vec<u8>,
    path: Vec<u8>,
    http: Vec<u8>,
}

impl HTTPMessage {
    /// Checks if the method is supported.
    fn is_method_valid(&self) -> bool {
        match self.method.as_slice() {
            b"GET" | b"HEAD" | b"POST" | b"PUT" | b"DELETE" | b"OPTIONS" | b"PATCH" | b"TRACE" => {
                true
            }
            _ => false,
        }
    }

    /// Checks if the HTTP version is supported.
    fn is_http_valid(&self) -> bool {
        match self.http.as_slice() {
            b"HTTP/1.0" | b"HTTP/1.1" | b"HTTP/2" => true,
            _ => false,
        }
    }
}

/// Extracts the first line of a message if anything is there.
async fn extract(mut stream: &TcpStream) -> Result<Vec<u8>> {
    let mut request: Vec<u8> = Vec::with_capacity(REQUEST_CAP);
    let mut buf = [0 as u8; BUFFER_LENGTH];

    loop {
        let mut size = stream.read(&mut buf).await?;

        if size > 0 {
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
        } else {
            break;
        }
    }

    if !request.is_empty() {
        if request.is_ascii() {
            return Ok(request);
        }
        return Err(Error::new(ErrorKind::InvalidData, "Non-ASCII HTTP message"));
    }

    Err(Error::new(ErrorKind::InvalidData, "Empty HTTP message"))
}

// TODO: a From trait implementation maybe?
/// Parses a given data into an HTTP message instance.
async fn parse(data: &Vec<u8>) -> HTTPMessage {
    let mut result: Vec<&[u8]> = Vec::with_capacity(3);

    for value in data.splitn(3, |i| i == &b' ').into_iter() {
        result.push(value);
    }

    HTTPMessage {
        method: result[0].to_vec(),
        path: result[1].to_vec(),
        http: result[2].to_vec(),
    }
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

    let addr = SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 8080);

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
            let mut response: Vec<u8> = Vec::with_capacity(RESPONSE_CAP);

            response.extend("HTTP/1.1".as_bytes());
            response.extend(b" ");

            // TODO: do we want to handle errors here or remove them altogether?
            if let Some(data) = extract(&stream).await.ok() {
                let message = parse(&data).await;

                if !message.is_method_valid() {
                    response.extend("405 Method Not Allowed".as_bytes());
                } else if message.path.is_empty() || message.http.is_empty() {
                    response.extend("414 URI Too Long".as_bytes());
                } else if !message.is_http_valid() {
                    response.extend("505 HTTP Version Not Supported".as_bytes());
                } else if message.path == b"/healthz" {
                    // TODO: do we care about the method here?
                    response.extend("200 OK".as_bytes()); // I would prefer 204 though
                } else {
                    response.extend("404 Not Found".as_bytes());
                }
            }

            // TODO: not very elegant but will do for now
            if response.len() == 9 {
                response.extend("400 Bad Request".as_bytes());
            }

            response.extend(CRLF);
            response.extend(CRLF);

            if let Some(e) = stream.write_all(response.as_slice()).await.err() {
                if e.kind() != ErrorKind::WouldBlock {
                    return;
                }
            }

            stream.flush().await.ok();
        });
    }
}
