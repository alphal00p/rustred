//! Invocation-local restore progress and bounded immutable validation work.
//! The observer stays on the caller thread; workers only borrow proof inputs.
use std::io;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug)]
pub(super) struct Update {
    pub stage: &'static str,
    pub completed: usize,
    pub total: usize,
    pub worker_limit: usize,
}

pub(super) struct Control<'a> {
    pub workers: usize,
    pub cancelled: &'a dyn Fn() -> bool,
    pub observer: &'a dyn Fn(Update),
}

#[derive(Debug)]
struct RestoreCancelled;
impl std::fmt::Display for RestoreCancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("epoch restore cancelled before session adoption")
    }
}
impl std::error::Error for RestoreCancelled {}

/// Only this marker proves that the operation stopped before session adoption.
/// An unrelated Interrupted I/O error can also arise from the adoption itself.
pub(super) fn is_cancelled(error: &io::Error) -> bool {
    error
        .get_ref()
        .is_some_and(|error| error.is::<RestoreCancelled>())
}

#[derive(Debug)]
struct WorkerFailure(String);
impl std::fmt::Display for WorkerFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for WorkerFailure {}

struct WorkerPanic<'a>(&'a AtomicBool);
impl Drop for WorkerPanic<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.0.store(true, Ordering::Release);
        }
    }
}

/// Operational stops cannot be repaired by decoding the previous generation.
pub(super) fn operational(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::Interrupted
        || error
            .get_ref()
            .is_some_and(|error| error.is::<WorkerFailure>())
}

impl Control<'_> {
    pub fn serial() -> Self {
        Self {
            workers: 1,
            cancelled: &|| false,
            observer: &|_| {},
        }
    }

    pub fn check(&self) -> io::Result<()> {
        if (self.cancelled)() {
            Err(io::Error::new(io::ErrorKind::Interrupted, RestoreCancelled))
        } else {
            Ok(())
        }
    }

    pub fn stage(&self, stage: &'static str, completed: usize, total: usize) -> io::Result<()> {
        self.check()?;
        (self.observer)(Update {
            stage,
            completed,
            total,
            worker_limit: if stage == "anchor_coverage" {
                self.workers.saturating_sub(1).max(1).min(total.max(1))
            } else {
                1
            },
        });
        self.check()
    }

    /// At most W-1 workers plus the caller when W > 1. One-worker requests
    /// validate inline. There is no global/nested pool or campaign-sized output.
    pub fn parallel(
        &self,
        stage: &'static str,
        total: usize,
        validate: &(impl Fn(usize) -> io::Result<()> + Sync),
    ) -> io::Result<()> {
        self.stage(stage, 0, total)?;
        if self.workers <= 1 || total <= 1 {
            let mut last = Instant::now();
            for index in 0..total {
                self.check()?;
                validate(index)?;
                if last.elapsed() >= Duration::from_secs(1) {
                    self.stage(stage, index + 1, total)?;
                    last = Instant::now();
                }
            }
            return self.stage(stage, total, total);
        }
        let workers = (self.workers - 1).min(total);
        let next = AtomicUsize::new(0);
        let completed = AtomicUsize::new(0);
        let cancelled = AtomicBool::new(false);
        let first_failure = AtomicUsize::new(total);
        let failure = Mutex::new(None::<(usize, io::Error)>);
        std::thread::scope(|scope| {
            let mut handles = Vec::with_capacity(workers);
            for _ in 0..workers {
                let handle = std::thread::Builder::new()
                    .name("epoch-restore".into())
                    .spawn_scoped(scope, || {
                        let _panic = WorkerPanic(&cancelled);
                        loop {
                            // Small dynamic chunks balance differing exact cover costs.
                            let start = next.fetch_add(32, Ordering::Relaxed);
                            let end = start.saturating_add(32).min(total);
                            if start >= end {
                                break;
                            }
                            for index in start..end {
                                if cancelled.load(Ordering::Acquire)
                                    || index >= first_failure.load(Ordering::Acquire)
                                {
                                    return;
                                }
                                let result = validate(index);
                                completed.fetch_add(1, Ordering::Relaxed);
                                if let Err(error) = result {
                                    let mut failure = failure.lock().unwrap();
                                    if failure.as_ref().is_none_or(|(first, _)| index < *first) {
                                        *failure = Some((index, error));
                                        first_failure.store(index, Ordering::Release);
                                    }
                                    return;
                                }
                            }
                        }
                    });
                match handle {
                    Ok(handle) => handles.push(handle),
                    Err(error) => {
                        cancelled.store(true, Ordering::Release);
                        for handle in handles {
                            let _ = handle.join();
                        }
                        return Err(io::Error::other(WorkerFailure(format!(
                            "epoch restore worker spawn failed: {error}"
                        ))));
                    }
                }
            }
            let mut last = Instant::now();
            let mut interrupted = None;
            while handles.iter().any(|handle| !handle.is_finished()) {
                let result = if last.elapsed() >= Duration::from_secs(1) {
                    last = Instant::now();
                    self.stage(stage, completed.load(Ordering::Relaxed), total)
                } else {
                    self.check()
                };
                if let Err(error) = result {
                    cancelled.store(true, Ordering::Release);
                    interrupted = Some(error);
                    break;
                }
                std::thread::park_timeout(Duration::from_millis(100));
            }
            let mut panicked = false;
            for handle in handles {
                panicked |= handle.join().is_err();
            }
            if let Some(error) = interrupted {
                return Err(error);
            }
            self.check()?;
            if panicked {
                return Err(io::Error::other(WorkerFailure(
                    "epoch restore worker panicked".into(),
                )));
            }
            if let Some((_, error)) = failure.lock().unwrap().take() {
                return Err(error);
            }
            self.stage(stage, total, total)
        })
    }
}

#[cfg(test)]
mod tests;
