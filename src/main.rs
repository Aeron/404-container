mod http;

use std::env;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, Ipv6Addr, Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::mpsc::{self, Receiver, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::http::RequestMessage;

const ACCEPT_SLEEP: Duration = Duration::from_millis(100);
const CRLF: &[u8; 2] = b"\r\n";
const MIN_WORKERS: usize = 4;
const QUEUE_PER_WORKER: usize = 32;
const READ_BUFFER: usize = 1024;
const READ_TIMEOUT: Duration = Duration::from_secs(5);
const SEP: &[u8; 1] = b" ";
const WORKERS_PER_CPU: usize = 4;
const WRITE_TIMEOUT: Duration = Duration::from_secs(5);

#[cfg(unix)]
mod signals {
    use std::os::raw::c_int;
    use std::sync::atomic::{AtomicBool, Ordering};

    const SIGHUP: c_int = 1;
    const SIGINT: c_int = 2;
    const SIGTERM: c_int = 15;

    static SHOULD_QUIT: AtomicBool = AtomicBool::new(false);

    unsafe extern "C" {
        fn signal(signum: c_int, handler: extern "C" fn(c_int)) -> extern "C" fn(c_int);
    }

    extern "C" fn handle_signal(_signal: c_int) {
        SHOULD_QUIT.store(true, Ordering::Relaxed);
    }

    pub fn install() {
        unsafe {
            signal(SIGHUP, handle_signal);
            signal(SIGINT, handle_signal);
            signal(SIGTERM, handle_signal);
        }
    }

    pub fn should_quit() -> bool {
        SHOULD_QUIT.load(Ordering::Relaxed)
    }
}

#[cfg(not(unix))]
mod signals {
    pub fn install() {}

    pub fn should_quit() -> bool {
        false
    }
}

/// Processes TCP stream bytes as an HTTP request message, and responds accordingly.
fn process(mut stream: TcpStream) -> io::Result<()> {
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

fn port() -> u16 {
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

fn bind(port: u16) -> io::Result<Vec<TcpListener>> {
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

fn worker_count() -> usize {
    thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1)
        .saturating_mul(WORKERS_PER_CPU)
        .max(MIN_WORKERS)
}

fn spawn_workers(receiver: Receiver<TcpStream>, workers: usize) {
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

fn main() {
    signals::install();

    let port = port();
    let listeners = match bind(port) {
        Ok(listeners) => listeners,
        Err(ref err) => {
            eprintln!("Cannot listen on port {port}: {err}");
            return;
        }
    };

    for listener in &listeners {
        if let Err(err) = listener.set_nonblocking(true) {
            eprintln!("Cannot set listener to non-blocking mode: {err}");
            return;
        }
    }

    let workers = worker_count();
    let (sender, receiver) = mpsc::sync_channel(workers * QUEUE_PER_WORKER);
    spawn_workers(receiver, workers);

    let addrs = listeners
        .iter()
        .filter_map(|listener| listener.local_addr().ok())
        .map(|addr| addr.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    println!("Listening on {addrs} with {workers} workers");

    while !signals::should_quit() {
        let mut accepted = false;

        for listener in &listeners {
            loop {
                match listener.accept() {
                    Ok((stream, _addr)) => {
                        accepted = true;
                        stream.set_nodelay(true).ok();

                        match sender.try_send(stream) {
                            Ok(()) => {}
                            Err(TrySendError::Full(_stream)) => {}
                            Err(TrySendError::Disconnected(_stream)) => return,
                        }
                    }
                    Err(ref err) if err.kind() == io::ErrorKind::WouldBlock => break,
                    Err(ref err) if err.kind() == io::ErrorKind::Interrupted => continue,
                    Err(_) => break,
                }
            }
        }

        if !accepted {
            thread::sleep(ACCEPT_SLEEP);
        }
    }

    println!("Quitting");
}
