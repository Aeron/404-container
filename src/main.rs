mod http;
mod utils;

use std::env;
use std::net::{Ipv4Addr, SocketAddrV4};

use async_signals::Signals;
use async_std::io::ErrorKind;
use async_std::net::TcpListener;
use async_std::prelude::*;
use async_std::task;

use http::{RequestMessage, ResponseMessage};
use smallvec::ToSmallVec;
use utils::extract;

const CRLF: [u8; 2] = [13, 10];

const RESP_200: ResponseMessage = ResponseMessage::with_status(200, b"OK");
const RESP_400: ResponseMessage = ResponseMessage::with_status(400, b"Bad Request");
const RESP_404: ResponseMessage = ResponseMessage::new();
const RESP_405: ResponseMessage = ResponseMessage::with_status(405, b"Method Not Allowed");
const RESP_414: ResponseMessage = ResponseMessage::with_status(414, b"URI Too Long");
const RESP_505: ResponseMessage = ResponseMessage::with_status(505, b"HTTP Version Not Supported");

#[async_std::main]
async fn main() {
    task::spawn(async {
        // NOTE: SIGHUP = 1, SIGINT = 2, SIGTERM = 15
        let mut signals = Signals::new([1, 2, 15]).unwrap();

        while (signals.next().await).is_some() {
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
        let mut stream = match stream {
            Ok(stream) => stream,
            Err(_) => continue,
        };
        stream.set_nodelay(true).ok(); // we do not really care if it clicks or not

        task::spawn(async move {
            let mut response = &RESP_404;

            let data = extract(&stream).await;

            if !data.is_empty() && data.is_ascii() {
                let message = RequestMessage::from(&data);

                if !message.is_method_valid() {
                    response = &RESP_405;
                } else if message.path.is_empty() || message.http.is_empty() {
                    response = &RESP_414;
                } else if !message.is_http_valid() {
                    response = &RESP_505;
                } else if message.path.as_slice() == b"/healthz" {
                    response = &RESP_200; // I would prefer 204 though
                }
            } else {
                response = &RESP_400;
            }

            if let Some(e) = stream
                .write_all(response.to_smallvec().as_slice())
                .await
                .err()
            {
                if e.kind() != ErrorKind::WouldBlock {
                    return;
                }
            }

            stream.flush().await.ok();
        });
    }
}
