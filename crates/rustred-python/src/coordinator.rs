//! Target-specific Python entry into Symbolica.
//!
//! Native callers share one stable OS thread. Single-threaded WebAssembly must
//! execute inline instead: creating a thread or waiting on a channel cannot
//! make progress there. Both paths reject further algebra after a Rust panic.

use std::any::Any;

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use native::process_coordinator;

#[cfg(any(target_arch = "wasm32", test))]
mod inline;
#[cfg(target_arch = "wasm32")]
pub(crate) use inline::process_coordinator;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CoordinatorError {
    Busy,
    Poisoned,
    #[cfg(not(target_arch = "wasm32"))]
    Forked {
        creator_pid: u32,
        current_pid: u32,
    },
    Panicked(String),
    Unavailable(String),
}

fn panic_message(payload: &(dyn Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else if let Some(message) = payload.downcast_ref::<&'static str>() {
        (*message).to_owned()
    } else {
        "non-string Rust panic payload".to_owned()
    }
}
