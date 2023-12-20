mod http;

use std::env;
use std::net::{Ipv4Addr, Shutdown, SocketAddrV4};

use async_signals::Signals;
use async_std::io::{ErrorKind, ReadExt, WriteExt};
use async_std::net::{TcpListener, TcpStream};
use async_std::prelude::*;
use async_std::task;

use crate::http::{RequestMessage, ResponseMessage};

const CRLF: &[u8; 2] = b"\r\n";
const SEP: &[u8; 1] = b" ";

const REQUEST_CAP: usize = 65536;
const BUFFER_LEN: usize = 64;

const RESP_200: ResponseMessage = ResponseMessage::with_status(200, b"OK");
const RESP_400: ResponseMessage = ResponseMessage::with_status(400, b"Bad Request");
const RESP_404: ResponseMessage = ResponseMessage::with_status(404, b"Not Found");
const RESP_405: ResponseMessage = ResponseMessage::with_status(405, b"Method Not Allowed");
const RESP_414: ResponseMessage = ResponseMessage::with_status(414, b"URI Too Long");
const RESP_505: ResponseMessage = ResponseMessage::with_status(505, b"HTTP Version Not Supported");

async fn process(mut stream: TcpStream) {
    let mut request: Vec<u8> = Vec::with_capacity(REQUEST_CAP);
    let mut buf = [0_u8; BUFFER_LEN];

    loop {
        match stream.read(&mut buf).await {
            Ok(mut size) if size > 0 => {
                if let Some(pos) = buf.iter().position(|i| i == &CRLF[0]) {
                    size = pos;
                }

                if request.len() + size > REQUEST_CAP {
                    size = REQUEST_CAP - request.len();
                }

                request.extend_from_slice(&buf[..size]);

                if size < BUFFER_LEN {
                    break;
                }
            }
            _ => break,
        }
    }

    let response = if !request.is_empty() && request.is_ascii() {
        let message = RequestMessage::from(request.as_slice());

        if !message.is_method_valid() {
            &RESP_405
        } else if message.path.is_empty() || message.http.is_empty() {
            &RESP_414
        } else if !message.is_http_valid() {
            &RESP_505
        } else if message.path == b"/healthz" {
            &RESP_200 // I would prefer 204 though
        } else {
            &RESP_404
        }
    } else {
        &RESP_400
    };

    for part in [
        response.http,
        SEP,
        response.code.to_string().as_bytes(),
        SEP,
        response.desc,
        CRLF,
        response.headers.join(&CRLF[..]).as_slice(),
        CRLF,
        CRLF,
    ] {
        if let Some(e) = stream.write_all(part).await.err() {
            if e.kind() != ErrorKind::WouldBlock {
                return;
            }
        }
    }

    stream.flush().await.ok();
    stream.shutdown(Shutdown::Both).ok();
}

#[async_std::main]
async fn main() {
    task::spawn(async {
        // NOTE: SIGHUP = 1, SIGINT = 2, SIGTERM = 15
        let mut signals = Signals::new([1, 2, 15]).unwrap();

        if (signals.next().await).is_some() {
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
            println!("Listening on {addr}");
            listener
        }
        Err(ref e) => {
            eprintln!("Cannot listen on {addr}: {e}");
            return;
        }
    };

    let mut incoming = listener.incoming();

    while let Some(stream) = incoming.next().await {
        let stream = match stream {
            Ok(stream) => stream,
            Err(_) => continue,
        };
        stream.set_nodelay(true).ok(); // we do not really care if it clicks or not

        task::spawn(process(stream));
    }
}
