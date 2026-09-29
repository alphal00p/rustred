//! One scoped worker pool shared by the S2 batch adapter and S3 controller.
//! Cancellation drops queued/unmerged bytes before the caller saves; joining
//! remains scoped and may wait on a slow native AFTER that save has completed.
use std::collections::VecDeque;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex, mpsc};
use std::time::Duration;

const MAX_BATCH: usize = 4096;
type Inspect<'a> = dyn Fn(&[u8], &AtomicBool) -> Vec<u8> + Sync + 'a;

pub(in super::super) struct Work {
    pub key: u64,
    pub bytes: Vec<u8>,
}

/// Worker acceptance/return observations, not RSS attribution or job blame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super) struct Status {
    pub key: u64,
    pub started: bool,
    pub returned: bool,
}

struct Queue {
    jobs: VecDeque<(usize, Vec<u8>)>,
    status: Vec<Status>,
    shutdown: bool,
}
enum Message {
    Started(usize),
    Result(usize, Vec<u8>),
}
pub(in super::super) enum Poll {
    Started(u64),
    Result { key: u64, bytes: Vec<u8> },
    Waiting,
    Drained,
}

pub(in super::super) struct Pool<'a> {
    queue: &'a Mutex<Queue>,
    ready: &'a Condvar,
    stop: &'a AtomicBool,
    receiver: Option<mpsc::Receiver<Message>>,
    receipts: Vec<u8>,
    remaining: usize,
    cancelled: bool,
}

fn shutdown(queue: &Mutex<Queue>, ready: &Condvar, stop: &AtomicBool) -> Result<(), String> {
    stop.store(true, Ordering::Release);
    let (mut guard, poisoned) = match queue.lock() {
        Ok(guard) => (guard, false),
        Err(error) => (error.into_inner(), true),
    };
    guard.shutdown = true;
    guard.jobs.clear();
    drop(guard);
    ready.notify_all();
    if poisoned {
        Err("epoch inspector queue poisoned (C5)".into())
    } else {
        Ok(())
    }
}

impl Pool<'_> {
    pub fn submit(&mut self, jobs: Vec<Work>) -> Result<(), String> {
        if self.cancelled || self.remaining != 0 || jobs.len() > MAX_BATCH {
            return Err("epoch submit outside an empty lockstep slot (C5)".into());
        }
        for (at, job) in jobs.iter().enumerate() {
            if jobs[..at].iter().any(|earlier| earlier.key == job.key) {
                return Err("epoch duplicate submitted sequence (C5)".into());
            }
        }
        self.receipts.clear();
        self.receipts
            .try_reserve(jobs.len())
            .map_err(|_| "epoch result receipt allocation")?;
        let mut guard = self
            .queue
            .lock()
            .map_err(|_| "epoch inspector queue poisoned (C5)")?;
        if guard.shutdown || !guard.jobs.is_empty() {
            return Err("epoch submit to stopped/nonempty queue (C5)".into());
        }
        guard
            .jobs
            .try_reserve(jobs.len())
            .map_err(|_| "epoch inspector queue allocation")?;
        guard.status.clear();
        guard
            .status
            .try_reserve(jobs.len())
            .map_err(|_| "epoch inspector status allocation")?;
        self.remaining = jobs.len();
        self.receipts.resize(jobs.len(), 0);
        for (index, job) in jobs.into_iter().enumerate() {
            guard.status.push(Status {
                key: job.key,
                started: false,
                returned: false,
            });
            guard.jobs.push_back((index, job.bytes));
        }
        drop(guard);
        self.ready.notify_all();
        Ok(())
    }

    pub fn poll(&mut self, timeout: Duration) -> Result<Poll, String> {
        if self.cancelled {
            return Err("epoch poll after cancellation".into());
        }
        // A failed worker may leave idle siblings holding live senders, so
        // channel disconnection alone cannot detect a poisoned queue.
        if self.queue.is_poisoned() {
            return Err("epoch inspector queue poisoned (C5)".into());
        }
        if self.remaining == 0 {
            return Ok(Poll::Drained);
        }
        let message = match self
            .receiver
            .as_ref()
            .ok_or("epoch result receiver absent (C5)")?
            .recv_timeout(timeout)
        {
            Ok(message) => message,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if self.queue.is_poisoned() {
                    return Err("epoch inspector queue poisoned (C5)".into());
                }
                return Ok(Poll::Waiting);
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err(
                    "epoch inspector channel disconnected with work outstanding (C5)".into(),
                );
            }
        };
        let (index, bytes) = match message {
            Message::Started(index) => (index, None),
            Message::Result(index, bytes) => (index, Some(bytes)),
        };
        let receipt = self
            .receipts
            .get_mut(index)
            .ok_or("epoch result index outside batch (C5)")?;
        let key = self
            .queue
            .lock()
            .map_err(|_| "epoch inspector queue poisoned (C5)")?
            .status
            .get(index)
            .ok_or("epoch result status absent (C5)")?
            .key;
        if let Some(bytes) = bytes {
            if *receipt != 1 {
                return Err("epoch duplicate/unstarted result (C5)".into());
            }
            *receipt = 2;
            self.remaining -= 1;
            Ok(Poll::Result { key, bytes })
        } else {
            if *receipt != 0 {
                return Err("epoch duplicate worker start (C5)".into());
            }
            *receipt = 1;
            Ok(Poll::Started(key))
        }
    }

    /// Bounded by B, including queued entries explicitly marked not started.
    /// This never turns merely Reserved work into an attributed suspect.
    pub fn snapshot(&self) -> Result<Vec<Status>, String> {
        let guard = self
            .queue
            .lock()
            .map_err(|_| "epoch inspector queue poisoned (C5)")?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(guard.status.len())
            .map_err(|_| "epoch status snapshot allocation")?;
        values.extend_from_slice(&guard.status);
        Ok(values)
    }

    /// Stop admission first, release queued and channel-buffered result bytes,
    /// then let the caller save its last coherent merge state before join.
    pub fn cancel(&mut self) -> Result<(), String> {
        self.cancelled = true;
        let result = shutdown(self.queue, self.ready, self.stop);
        self.receiver.take();
        self.remaining = 0;
        result
    }
}
impl Drop for Pool<'_> {
    fn drop(&mut self) {
        let _ = self.cancel();
    }
}

pub(in super::super) fn with_polling_pool<R>(
    threads: usize,
    job: &Inspect<'_>,
    body: impl FnOnce(&mut Pool<'_>) -> R,
) -> Result<R, String> {
    if threads == 0 {
        return Err("responsive epoch pool requires an inspector plus its coordinator".into());
    }
    let queue = Mutex::new(Queue {
        jobs: VecDeque::new(),
        status: Vec::new(),
        shutdown: false,
    });
    let ready = Condvar::new();
    let stop = AtomicBool::new(false);
    let (sender, receiver) = mpsc::channel();
    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        handles
            .try_reserve_exact(threads)
            .map_err(|_| "epoch worker handle allocation")?;
        for slot in 0..threads {
            let (queue, ready, stop, sender) = (&queue, &ready, &stop, sender.clone());
            match std::thread::Builder::new()
                .name(format!("epoch-inspector-{slot}"))
                .spawn_scoped(scope, move || {
                    loop {
                        let next = {
                            let Ok(mut guard) = queue.lock() else { return };
                            loop {
                                // Crucially precedes pop: stop never drains queued work.
                                if guard.shutdown {
                                    break None;
                                }
                                if let Some((index, bytes)) = guard.jobs.pop_front() {
                                    guard.status[index].started = true;
                                    break Some((index, bytes));
                                }
                                guard = match ready.wait(guard) {
                                    Ok(guard) => guard,
                                    Err(_) => return,
                                };
                            }
                        };
                        let Some((index, bytes)) = next else { return };
                        if sender.send(Message::Started(index)).is_err() {
                            return;
                        }
                        let result = catch_unwind(AssertUnwindSafe(|| job(&bytes, stop)))
                            .unwrap_or_default();
                        let Ok(mut guard) = queue.lock() else { return };
                        guard.status[index].returned = true;
                        drop(guard);
                        if sender.send(Message::Result(index, result)).is_err() {
                            return;
                        }
                    }
                }) {
                Ok(handle) => handles.push(handle),
                Err(error) => {
                    let _ = shutdown(&queue, &ready, &stop);
                    return Err(format!("epoch inspector spawn: {error}"));
                }
            }
        }
        drop(sender);
        let mut pool = Pool {
            queue: &queue,
            ready: &ready,
            stop: &stop,
            receiver: Some(receiver),
            receipts: Vec::new(),
            remaining: 0,
            cancelled: false,
        };
        let result = catch_unwind(AssertUnwindSafe(|| body(&mut pool)));
        let cancelled = pool.cancel();
        drop(pool);
        let mut failed_join = false;
        for handle in handles {
            failed_join |= handle.join().is_err();
        }
        match result {
            Err(panic) => std::panic::resume_unwind(panic),
            Ok(value) => {
                cancelled?;
                if failed_join {
                    Err("epoch inspector worker failed outside job frame (C5)".into())
                } else {
                    Ok(value)
                }
            }
        }
    })
}

#[cfg(test)]
mod tests;
