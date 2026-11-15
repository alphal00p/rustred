//! Bounded-time I/O for the control thread (audit A M5).
//!
//! A per-pid /proc read can block in the kernel for minutes (a read of `environ` takes the target's mmap lock), and a
//! write to /shared can stall for seconds (ZFS dmu_tx_wait), both measured on this host (ops/guarded_run.py). The
//! control thread therefore never performs such I/O directly: it runs it on a helper thread and waits at most a given
//! time. On timeout the caller gets [`Bounded::TimedOut`] and the helper thread finishes on its own later (its result
//! is dropped, or handed to an `on_abandon` callback that runs ON THE HELPER THREAD, e.g. to undo a ledger claim).
//!
//! At most [`MAX_STUCK`] helper threads may be outstanding at once: beyond that, new calls time out at once instead of
//! piling up threads behind a kernel that does not answer.

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// Outstanding helper threads beyond which calls fail fast.
pub const MAX_STUCK: usize = 32;

static OUTSTANDING: AtomicUsize = AtomicUsize::new(0);

/// Result of a bounded call.
#[derive(Debug)]
pub enum Bounded<T> {
    Done(T),
    /// Not finished within the bound; the work may still complete in the background.
    TimedOut,
    /// The helper thread could not be started (gate3 M3): a RESOURCE failure of this process (EAGAIN from the thread
    /// limit, ENOMEM), or [`MAX_STUCK`] helpers already outstanding (reported as EAGAIN). Never a timeout: callers on
    /// the launch path turn it into the supervisor's resource back-off (exit 71), not into a job attempt.
    SpawnFailed(std::io::Error),
}

impl<T> Bounded<T> {
    pub fn done(self) -> Option<T> {
        match self {
            Bounded::Done(t) => Some(t),
            Bounded::TimedOut | Bounded::SpawnFailed(_) => None,
        }
    }
}

/// Helper threads currently outstanding (diagnostics; tests).
pub fn outstanding_helpers() -> usize {
    OUTSTANDING.load(Ordering::SeqCst)
}

struct Outstanding;
impl Drop for Outstanding {
    fn drop(&mut self) {
        OUTSTANDING.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Run `work` on a helper thread; wait at most `timeout`. If the caller gave up, `on_abandon(result)` runs on the helper
/// thread once `work` returns.
pub fn run_bounded_with<T, F, A>(timeout: Duration, work: F, on_abandon: A) -> Bounded<T>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
    A: FnOnce(T) + Send + 'static,
{
    if OUTSTANDING.fetch_add(1, Ordering::SeqCst) >= MAX_STUCK {
        OUTSTANDING.fetch_sub(1, Ordering::SeqCst);
        return Bounded::SpawnFailed(std::io::Error::from_raw_os_error(libc::EAGAIN));
    }
    struct Slot<T> {
        abandoned: bool,
        result: Option<T>,
    }
    let shared = Arc::new((
        Mutex::new(Slot {
            abandoned: false,
            result: None,
        }),
        Condvar::new(),
    ));
    let theirs = Arc::clone(&shared);
    let spawned = std::thread::Builder::new()
        .name("rr-bounded-io".into())
        .spawn(move || {
            let _guard = Outstanding;
            let r = work();
            let (m, cv) = &*theirs;
            let mut slot = m.lock().unwrap_or_else(|p| p.into_inner());
            if slot.abandoned {
                drop(slot);
                on_abandon(r);
            } else {
                slot.result = Some(r);
                cv.notify_all();
            }
        });
    if let Err(e) = spawned {
        OUTSTANDING.fetch_sub(1, Ordering::SeqCst);
        // std reports a failed clone() with its errno (EAGAIN at the thread / process limit, ENOMEM); keep it typed.
        let e = match e.raw_os_error() {
            Some(_) => e,
            None => std::io::Error::from_raw_os_error(libc::EAGAIN),
        };
        return Bounded::SpawnFailed(e);
    }
    // Hand-off under one mutex: either the caller takes the result, or it marks the slot abandoned and the helper then
    // runs on_abandon; a result can never be lost in between.
    let (m, cv) = &*shared;
    let deadline = Instant::now() + timeout;
    let mut slot = m.lock().unwrap_or_else(|p| p.into_inner());
    loop {
        if let Some(r) = slot.result.take() {
            return Bounded::Done(r);
        }
        let now = Instant::now();
        if now >= deadline {
            slot.abandoned = true;
            return Bounded::TimedOut;
        }
        slot = match cv.wait_timeout(slot, deadline - now) {
            Ok((g, _)) => g,
            Err(p) => p.into_inner().0,
        };
    }
}

/// [`run_bounded_with`] without an abandon action.
pub fn run_bounded<T, F>(timeout: Duration, work: F) -> Bounded<T>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    run_bounded_with(timeout, work, |_| {})
}

/// Outcome of a bounded file read.
#[derive(Debug)]
pub enum ReadOutcome {
    Data(Vec<u8>),
    /// ENOENT / ESRCH: the file (process) does not exist.
    Gone,
    Error(std::io::Error),
    TimedOut,
}

/// `fs::read(path)` within `timeout`.
pub fn read_bounded(path: &Path, timeout: Duration) -> ReadOutcome {
    let p = path.to_path_buf();
    match run_bounded(timeout, move || std::fs::read(&p)) {
        Bounded::Done(Ok(b)) => ReadOutcome::Data(b),
        Bounded::Done(Err(e))
            if matches!(e.raw_os_error(), Some(libc::ENOENT) | Some(libc::ESRCH)) =>
        {
            ReadOutcome::Gone
        }
        Bounded::Done(Err(e)) => ReadOutcome::Error(e),
        Bounded::SpawnFailed(e) => ReadOutcome::Error(e),
        Bounded::TimedOut => ReadOutcome::TimedOut,
    }
}

#[allow(clippy::let_underscore_must_use)] // test cleanup (outside the reviewed scope, procfix7)
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn fast_work_returns_and_slow_work_times_out_with_abandon() {
        assert_eq!(run_bounded(Duration::from_secs(10), || 7).done(), Some(7));
        let (tx, rx) = mpsc::channel();
        let t0 = Instant::now();
        let r = run_bounded_with(
            Duration::from_millis(50),
            || {
                std::thread::sleep(Duration::from_millis(400));
                5
            },
            move |v| tx.send(v).unwrap(),
        );
        assert!(matches!(r, Bounded::TimedOut));
        assert!(t0.elapsed() < Duration::from_millis(350));
        assert_eq!(rx.recv_timeout(Duration::from_secs(30)).unwrap(), 5);
    }
}
