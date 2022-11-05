use std::net::{Ipv4Addr, SocketAddrV4};

use async_signals::Signals;
use async_std::io::ErrorKind;
use async_std::net::TcpListener;
use async_std::prelude::*;
use async_std::task;

const MSG404: &str = "HTTP/1.1 404 Not Found\r\n\r\n";

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
            if let Some(e) = stream.write_all(MSG404.as_bytes()).await.err() {
                if e.kind() != ErrorKind::WouldBlock {
                    return;
                }
            }

            stream.flush().await.ok();
        });
    }
}
