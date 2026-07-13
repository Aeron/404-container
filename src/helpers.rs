//! Server plumbing: request processing, listener setup, and the worker pool.

use std::env;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, Ipv6Addr, Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::http::RequestMessage;

const CRLF: &[u8; 2] = b"\r\n";
const MIN_WORKERS: usize = 4;
const READ_BUFFER: usize = 1024;
const READ_TIMEOUT: Duration = Duration::from_secs(5);
pub(crate) const SEP: &[u8; 1] = b" ";
const WORKERS_PER_CPU: usize = 4;
const WRITE_TIMEOUT: Duration = Duration::from_secs(5);

/// Processes TCP stream bytes as an HTTP request message, and responds accordingly.
pub fn process(mut stream: TcpStream) -> io::Result<()> {
    stream.set_read_timeout(Some(READ_TIMEOUT))?;
    stream.set_write_timeout(Some(WRITE_TIMEOUT))?;

    let mut buffer = Vec::with_capacity(READ_BUFFER);

    {
        let mut reader = BufReader::with_capacity(READ_BUFFER, &mut stream);
        reader
            .by_ref()
            .take(RequestMessage::LIMIT as u64)
            .read_until(CRLF[0], &mut buffer)?;
    }

    if buffer.ends_with(&[CRLF[0]]) {
        buffer.pop();
    }

    let request = RequestMessage::from(buffer.as_slice());
    let response = request.response();

    stream.write_all(
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
    )?;
    stream.flush()?;
    stream.shutdown(Shutdown::Both)?;

    Ok(())
}

pub fn port() -> u16 {
    match env::var("PORT") {
        Ok(value) => match value.parse::<u16>() {
            Ok(port) => port,
            Err(_) => {
                eprintln!("Invalid port; Quitting");
                std::process::exit(1);
            }
        },
        Err(_) => 8080,
    }
}

pub fn bind(port: u16) -> io::Result<Vec<TcpListener>> {
    let mut listeners = Vec::new();
    let mut error = None;

    match TcpListener::bind(SocketAddr::from((Ipv6Addr::UNSPECIFIED, port))) {
        Ok(listener) => listeners.push(listener),
        Err(err) => error = Some(err),
    }

    match TcpListener::bind(SocketAddr::from((Ipv4Addr::UNSPECIFIED, port))) {
        Ok(listener) => listeners.push(listener),
        Err(err) if listeners.is_empty() => error = Some(err),
        Err(_) => {}
    }

    if listeners.is_empty() {
        Err(error.expect("listener bind error"))
    } else {
        Ok(listeners)
    }
}

pub fn worker_count() -> usize {
    thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1)
        .saturating_mul(WORKERS_PER_CPU)
        .max(MIN_WORKERS)
}

pub fn spawn_workers(receiver: Receiver<TcpStream>, workers: usize) {
    let receiver = Arc::new(Mutex::new(receiver));

    for _ in 0..workers {
        let receiver = Arc::clone(&receiver);

        thread::spawn(move || {
            loop {
                let stream = {
                    let receiver = receiver.lock().expect("worker queue poisoned");
                    receiver.recv()
                };

                let Ok(stream) = stream else {
                    break;
                };

                #[cfg(debug_assertions)]
                if let Err(err) = process(stream) {
                    eprintln!("Processing error: {err}");
                }

                #[cfg(not(debug_assertions))]
                let _ = process(stream);
            }
        });
    }
}
