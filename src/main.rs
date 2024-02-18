mod http;

use std::env;
use std::net::{Ipv4Addr, Shutdown, SocketAddrV4};

use async_signals::Signals;
use async_std::io::{ReadExt, WriteExt};
use async_std::net::{TcpListener, TcpStream};
use async_std::prelude::*;
use async_std::task;

use crate::http::RequestMessage;

const CRLF: &[u8; 2] = b"\r\n";
const SEP: &[u8; 1] = b" ";

/// Processes TCP stream bytes as an HTTP request message, and responds accordingly.
async fn process(mut stream: TcpStream) -> Result<(), std::io::Error> {
    let mut buffer: Vec<u8> = Vec::with_capacity(RequestMessage::LIMIT);

    stream
        .by_ref()
        .bytes()
        .map(|result| result.unwrap_or_default())
        .take(RequestMessage::LIMIT)
        .take_while(|byte| byte != &CRLF[0])
        .enumerate()
        .for_each(|(index, element)| buffer.insert(index, element))
        .await;

    let request = RequestMessage::from(buffer.as_slice());
    let response = request.response();

    stream
        .write_all(
            &[
                response.http,
                SEP,
                response.code.to_string().as_bytes(),
                SEP,
                response.desc,
                CRLF,
                response.headers.join(&CRLF[..]).as_slice(),
                CRLF,
                CRLF,
            ]
            .concat(),
        )
        .await?;
    stream.flush().await?;
    stream.shutdown(Shutdown::Both)?;

    Ok(())
}

#[async_std::main]
async fn main() {
    task::spawn(async {
        // NOTE: SIGHUP = 1, SIGINT = 2, SIGTERM = 15
        let mut signals = Signals::new([1, 2, 15]).unwrap();

        if signals.next().await.is_some() {
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

    let addr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port);

    let listener = match TcpListener::bind(addr).await {
        Ok(listener) => {
            println!("Listening on {addr}");
            listener
        }
        Err(ref err) => {
            eprintln!("Cannot listen on {addr}: {err}");
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

        // NOTE: processing errors are not very helpful when running a release binary
        #[cfg(debug_assertions)]
        task::spawn(async {
            process(stream)
                .await
                .map_err(|ref err| eprintln!("Processing error: {err}"))
        });
        #[cfg(not(debug_assertions))]
        task::spawn(process(stream));
    }
}
