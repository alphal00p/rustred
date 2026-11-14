//! Stop requests: SIGINT / SIGTERM / SIGHUP into one atomic COUNTER, SIGWINCH into a flag. No work in handlers.
//!
//! Semantics read by the control loop ([`StopHandle::level`]):
//! - 0 requests: running;
//! - 1: DRAIN: launch nothing new; running jobs get `terminate(grace)` (SIGTERM, grace, SIGKILL; marked interrupted);
//! - 2 or more: FORCE: running jobs get a short grace then SIGKILL now; still safe (acceptance reads only sealed
//!   tables; the ledger records the kills). The loop may treat a third request as "restore the terminal and exit at
//!   once" ([`StopHandle::count`] gives the raw number).
//!
//! The same counter is bumped by [`StopHandle::request_stop`], the entry point for the dashboard: in raw terminal mode
//! (crossterm clears ISIG) Ctrl-C arrives as a KEY event, not as SIGINT, so the key handler must call it; the effect
//! is identical. Children never see the terminal's Ctrl-C: they run in their own sessions without a controlling
//! terminal, so only the supervisor decides how they stop.
//!
//! SIGHUP (zellij pane closed, ssh dropped) counts as one stop request and also sets [`StopHandle::hangup_seen`]: the
//! terminal is gone, writes to it fail with EIO (SIGPIPE is ignored by the Rust runtime, so they return errors), and
//! the dashboard must switch to logging only. A detach/re-attach of zellij sends no signal; a resize sends SIGWINCH,
//! which only sets the flag returned once by [`StopHandle::take_winch`].
//!
//! Audit A m10 (procfix): SIGHUP never ESCALATES: it raises the counter to at least 1 (Drain), and a SIGTERM that
//! arrives within 2 s after a SIGHUP (a closing pane often sends both) does not count again; only a deliberate second
//! request (Ctrl-C, SIGTERM later, `request_stop`) gives Force. SIGTSTP / SIGTTIN / SIGTTOU are caught and ignored, so
//! Ctrl-Z outside raw mode cannot stop the supervisor while its children run unsupervised. After `StopSignals` is
//! dropped the signals stay caught and ignored (signal-hook semantics): keep it for the life of the supervisor.

use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

/// CLOCK_MONOTONIC in ns (async-signal-safe: clock_gettime is on the POSIX list).
fn mono_ns() -> u64 {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: valid out-pointer.
    unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) };
    (ts.tv_sec as u64)
        .saturating_mul(1_000_000_000)
        .saturating_add(ts.tv_nsec as u64)
}

/// A SIGTERM within this time after a SIGHUP is the same stop request.
const HUP_COALESCE_NS: u64 = 2_000_000_000;

/// What the control loop should do.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum StopLevel {
    Running,
    Drain,
    Force,
}

impl StopLevel {
    pub fn from_count(n: usize) -> Self {
        match n {
            0 => StopLevel::Running,
            1 => StopLevel::Drain,
            _ => StopLevel::Force,
        }
    }
}

/// Cheap clonable view of the stop state (shared by the control loop and the dashboard).
#[derive(Clone, Debug, Default)]
pub struct StopHandle {
    count: Arc<AtomicUsize>,
    hangup: Arc<AtomicBool>,
    winch: Arc<AtomicBool>,
    /// mono_ns() of the latest SIGHUP (0: none).
    hup_at: Arc<AtomicU64>,
}

impl StopHandle {
    /// A handle without any signal registration (tests, or a caller that installs nothing).
    pub fn new() -> Self {
        Self::default()
    }

    /// Same effect as one SIGINT: for the dashboard's Ctrl-C key in raw mode, a 'q' key, or a stop file.
    pub fn request_stop(&self) -> StopLevel {
        StopLevel::from_count(self.count.fetch_add(1, Ordering::SeqCst) + 1)
    }

    pub fn count(&self) -> usize {
        self.count.load(Ordering::SeqCst)
    }

    pub fn level(&self) -> StopLevel {
        StopLevel::from_count(self.count())
    }

    pub fn hangup_seen(&self) -> bool {
        self.hangup.load(Ordering::SeqCst)
    }

    /// True once per burst of SIGWINCH (the dashboard re-reads the terminal size).
    pub fn take_winch(&self) -> bool {
        self.winch.swap(false, Ordering::SeqCst)
    }
}

/// Installed handlers; dropping unregisters them (signal-hook then keeps catching the signals and ignores them).
pub struct StopSignals {
    handle: StopHandle,
    ids: Vec<signal_hook::SigId>,
}

impl StopSignals {
    /// Register SIGINT, SIGTERM, SIGHUP (counter; SIGHUP also the hang-up flag) and SIGWINCH (flag).
    pub fn install() -> io::Result<Self> {
        Self::install_into(StopHandle::new())
    }

    /// Register into an existing handle (so that a dashboard created earlier shares the counter).
    pub fn install_into(handle: StopHandle) -> io::Result<Self> {
        use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM, SIGTSTP, SIGTTIN, SIGTTOU, SIGWINCH};
        let mut ids = Vec::new();
        let register = |ids: &mut Vec<signal_hook::SigId>| -> io::Result<()> {
            {
                let c = Arc::clone(&handle.count);
                // SAFETY: the closure runs in the handler and only does atomic operations (async-signal-safe).
                ids.push(unsafe {
                    signal_hook::low_level::register(SIGINT, move || {
                        c.fetch_add(1, Ordering::SeqCst);
                    })
                }?);
            }
            {
                let c = Arc::clone(&handle.count);
                let hup = Arc::clone(&handle.hup_at);
                // SAFETY: atomics and clock_gettime only (async-signal-safe).
                ids.push(unsafe {
                    signal_hook::low_level::register(SIGTERM, move || {
                        let h = hup.load(Ordering::SeqCst);
                        if h != 0 && mono_ns().saturating_sub(h) < HUP_COALESCE_NS {
                            let _ = c.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
                                (n == 0).then_some(1)
                            });
                        } else {
                            c.fetch_add(1, Ordering::SeqCst);
                        }
                    })
                }?);
            }
            {
                let c = Arc::clone(&handle.count);
                let hup = Arc::clone(&handle.hup_at);
                // SAFETY: atomics and clock_gettime only (async-signal-safe).
                ids.push(unsafe {
                    signal_hook::low_level::register(SIGHUP, move || {
                        hup.store(mono_ns().max(1), Ordering::SeqCst);
                        let _ = c.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
                            (n == 0).then_some(1)
                        });
                    })
                }?);
            }
            for sig in [SIGTSTP, SIGTTIN, SIGTTOU] {
                // SAFETY: an empty handler (async-signal-safe): replaces the default "stop" action.
                ids.push(unsafe { signal_hook::low_level::register(sig, || {}) }?);
            }
            ids.push(signal_hook::flag::register(
                SIGHUP,
                Arc::clone(&handle.hangup),
            )?);
            ids.push(signal_hook::flag::register(
                SIGWINCH,
                Arc::clone(&handle.winch),
            )?);
            Ok(())
        };
        if let Err(e) = register(&mut ids) {
            for id in ids {
                signal_hook::low_level::unregister(id);
            }
            return Err(e);
        }
        Ok(Self { handle, ids })
    }

    pub fn handle(&self) -> StopHandle {
        self.handle.clone()
    }
}

impl Drop for StopSignals {
    fn drop(&mut self) {
        for id in self.ids.drain(..) {
            signal_hook::low_level::unregister(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counter_semantics_without_signals() {
        let h = StopHandle::new();
        assert_eq!(h.level(), StopLevel::Running);
        let view = h.clone();
        assert_eq!(view.request_stop(), StopLevel::Drain);
        assert_eq!(h.level(), StopLevel::Drain);
        assert_eq!(h.request_stop(), StopLevel::Force);
        assert_eq!(h.request_stop(), StopLevel::Force);
        assert_eq!(h.count(), 3);
        assert!(!h.hangup_seen());
        assert!(!h.take_winch());
    }
}
