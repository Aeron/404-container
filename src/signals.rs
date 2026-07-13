//! Handles OS termination signals so the process can shut down cleanly as PID 1.

#[cfg(unix)]
mod unix {
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
        // SAFETY: `handle_signal` is `extern "C" fn(c_int)`, matching the libc
        // `signal` handler signature, and only stores to an atomic, so it is
        // safe to run from a signal handler context.
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
mod fallback {
    pub fn install() {}

    pub fn should_quit() -> bool {
        false
    }
}

#[cfg(unix)]
pub use unix::{install, should_quit};

#[cfg(not(unix))]
pub use fallback::{install, should_quit};
