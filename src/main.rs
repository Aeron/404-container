mod helpers;
mod http;
mod signals;

use std::io;
use std::sync::mpsc::{self, TrySendError};
use std::thread;
use std::time::Duration;

const ACCEPT_SLEEP: Duration = Duration::from_millis(100);
const QUEUE_PER_WORKER: usize = 32;

fn main() {
    signals::install();

    let port = helpers::port();
    let listeners = match helpers::bind(port) {
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

    let workers = helpers::worker_count();
    let (sender, receiver) = mpsc::sync_channel(workers * QUEUE_PER_WORKER);
    helpers::spawn_workers(receiver, workers);

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
