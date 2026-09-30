//! One scoped worker pool shared by the S2 batch adapter and S3 controller.
//! Cancellation drops queued/unmerged bytes before the caller saves; joining
//! remains scoped and may wait on a slow native AFTER that save has completed.
use std::collections::VecDeque;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::time::Duration;

const MAX_BATCH: usize = 4096;
mod memory;
pub(in super::super) use memory::{EscrowDiagnostics, ResultMemory, ReturnedBytes};
type Inspect<'a> = dyn Fn(&[u8], &AtomicBool) -> Vec<u8> + Sync + 'a;

#[derive(Debug)]
pub(in super::super) enum RunError {
    Capability(String),
    /// Handle allocation/thread creation failed before the body can dispatch.
    Resource(String),
    Engine(String),
}
impl From<String> for RunError {
    fn from(error: String) -> Self {
        Self::Engine(error)
    }
}
impl From<&str> for RunError {
    fn from(error: &str) -> Self {
        Self::Engine(error.into())
    }
}
impl From<RunError> for String {
    fn from(error: RunError) -> Self {
        match error {
            RunError::Capability(message)
            | RunError::Resource(message)
            | RunError::Engine(message) => message,
        }
    }
}

pub(in super::super) struct Work {
    pub key: u64,
    pub bytes: Vec<u8>,
}

#[derive(Debug)]
pub(in super::super) enum SubmitError {
    Allocation(&'static str),
    Protocol(&'static str),
}
impl From<SubmitError> for String {
    fn from(error: SubmitError) -> Self {
        match error {
            SubmitError::Allocation(message) | SubmitError::Protocol(message) => message.into(),
        }
    }
}

/// Worker acceptance/return observations, not RSS attribution or job blame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super) struct Status {
    pub key: u64,
    pub started: bool,
    pub returned: bool,
}

/// Point-in-time callback states, not CPU utilization or mathematical progress.
/// Queued work cancelled before acceptance is counted separately.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in super::super) struct Activity {
    pub queued: usize,
    pub computing: usize,
    pub returned: usize,
    /// Completed logical reservations whose physical slots were recycled.
    pub escrow_returned: usize,
    pub cancelled_queued: usize,
    pub occupied: usize,
    pub inline: bool,
}

impl Activity {
    pub fn from_status<'a>(
        status: impl Iterator<Item = &'a Status>,
        cancelled: bool,
        inline: bool,
    ) -> Self {
        let mut value = Self {
            inline,
            ..Self::default()
        };
        for entry in status {
            value.occupied += 1;
            if entry.returned {
                value.returned += 1;
            } else if entry.started {
                value.computing += 1;
            } else if cancelled {
                value.cancelled_queued += 1;
            } else {
                value.queued += 1;
            }
        }
        value
    }
}

struct Queue {
    jobs: VecDeque<(usize, Vec<u8>)>,
    status: Vec<Status>,
    occupied: Vec<bool>,
    generations: Vec<u64>,
    next_generation: u64,
    shutdown: bool,
}
enum Message {
    Started(usize, u64, u64),
    Result(usize, u64, u64, ReturnedBytes),
}
pub(in super::super) enum Poll {
    Started(u64),
    Result { key: u64, bytes: ReturnedBytes },
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
    threads: usize,
    cancelled: bool,
    result_memory: Arc<memory::Counter>,
    recycled: Vec<Status>,
    logical_limit: usize,
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
    pub fn enable_result_escrow(&mut self, total: usize) -> Result<(), String> {
        if total == 0 || total > MAX_BATCH || self.remaining != 0 {
            return Err("invalid escrow pool initialization".into());
        }
        let mut queue = self.queue.lock().map_err(|_| "epoch queue poisoned")?;
        if queue.next_generation != 0 || !self.recycled.is_empty() {
            return Err("escrow enabled after dispatch".into());
        }
        // The stop inventory can append escrowed statuses without allocating
        // on the memory-stop path. Slot reuse itself needs no extra storage.
        queue
            .status
            .try_reserve(total)
            .map_err(|_| "escrow status allocation")?;
        queue
            .occupied
            .try_reserve(total)
            .map_err(|_| "escrow slot allocation")?;
        queue
            .generations
            .try_reserve(total)
            .map_err(|_| "escrow generation allocation")?;
        self.receipts
            .try_reserve(total)
            .map_err(|_| "escrow receipt allocation")?;
        self.recycled
            .try_reserve(total)
            .map_err(|_| "escrow stop inventory allocation")?;
        self.result_memory.enable();
        self.logical_limit = total;
        Ok(())
    }

    pub fn result_memory(&self) -> ResultMemory {
        self.result_memory.snapshot()
    }

    /// Physical descriptor recycling is NOT publication or ledger retirement.
    pub fn recycle_result(&mut self, key: u64) -> Result<(), String> {
        if self.recycled.len() == self.recycled.capacity()
            || self.recycled.iter().any(|status| status.key == key)
        {
            return Err("escrow physical slot recycled twice or without capacity".into());
        }
        self.retire(&[key])?;
        // Survives moving result bytes into P1/P2: a stop before P3 must still
        // report this completed but logically Reserved inspection.
        self.recycled.push(Status {
            key,
            started: true,
            returned: true,
        });
        Ok(())
    }

    /// Optional profiler snapshot: the requested prefix job and aggregate pool
    /// state come from one bounded scan under one lock, without allocation.
    pub fn profiled_activity(&self, key: Option<u64>) -> Option<(Activity, Option<Status>)> {
        let guard = self.queue.lock().ok()?;
        if self.cancelled && guard.status.is_empty() {
            return None;
        }
        let mut selected = None;
        let mut activity = Activity::from_status(
            guard
                .status
                .iter()
                .zip(&guard.occupied)
                .filter_map(|(status, &live)| live.then_some(status))
                .inspect(|status| {
                    if Some(status.key) == key {
                        selected = Some(**status);
                    }
                }),
            self.cancelled,
            false,
        );
        activity.escrow_returned = self.recycled.len();
        if selected.is_none() {
            selected = self
                .recycled
                .iter()
                .find(|status| Some(status.key) == key)
                .copied();
        }
        Some((activity, selected))
    }

    /// No allocation and at most the fixed 4096 descriptor bound. Called lazily
    /// only when a rate-limited heartbeat is actually emitted.
    pub fn activity(&self) -> Option<Activity> {
        let guard = self.queue.lock().ok()?;
        if self.cancelled && guard.status.is_empty() {
            // The stop inventory has moved to the save path; late workers no
            // longer update it. Do not manufacture zero activity before join.
            return None;
        }
        let mut activity = Activity::from_status(
            guard
                .status
                .iter()
                .zip(&guard.occupied)
                .filter_map(|(status, &live)| live.then_some(status)),
            self.cancelled,
            false,
        );
        activity.escrow_returned = self.recycled.len();
        Some(activity)
    }

    pub fn submit(&mut self, jobs: Vec<Work>) -> Result<(), SubmitError> {
        if self.cancelled
            || self.remaining != 0
            || !self.recycled.is_empty()
            || jobs.len() > self.logical_limit
        {
            return Err(SubmitError::Protocol(
                "epoch submit outside an empty lockstep slot (C5)",
            ));
        }
        for (at, job) in jobs.iter().enumerate() {
            if jobs[..at].iter().any(|earlier| earlier.key == job.key) {
                return Err(SubmitError::Protocol(
                    "epoch duplicate submitted sequence (C5)",
                ));
            }
        }
        self.receipts.clear();
        self.receipts
            .try_reserve(jobs.len())
            .map_err(|_| SubmitError::Allocation("epoch result receipt allocation"))?;
        let mut guard = self
            .queue
            .lock()
            .map_err(|_| SubmitError::Protocol("epoch inspector queue poisoned (C5)"))?;
        if guard.shutdown || !guard.jobs.is_empty() {
            return Err(SubmitError::Protocol(
                "epoch submit to stopped/nonempty queue (C5)",
            ));
        }
        guard
            .jobs
            .try_reserve(jobs.len())
            .map_err(|_| SubmitError::Allocation("epoch inspector queue allocation"))?;
        guard.status.clear();
        guard.occupied.clear();
        guard.generations.clear();
        guard
            .next_generation
            .checked_add(jobs.len() as u64)
            .ok_or(SubmitError::Protocol("epoch slot generation exhausted"))?;
        guard
            .generations
            .try_reserve(jobs.len())
            .map_err(|_| SubmitError::Allocation("epoch generation allocation"))?;
        guard
            .status
            .try_reserve(jobs.len())
            .map_err(|_| SubmitError::Allocation("epoch inspector status allocation"))?;
        guard
            .occupied
            .try_reserve(jobs.len())
            .map_err(|_| SubmitError::Allocation("epoch inspector slot allocation"))?;
        self.remaining = jobs.len();
        self.receipts.resize(jobs.len(), 0);
        for (index, job) in jobs.into_iter().enumerate() {
            guard.next_generation += 1;
            let generation = guard.next_generation;
            guard.generations.push(generation);
            guard.status.push(Status {
                key: job.key,
                started: false,
                returned: false,
            });
            guard.occupied.push(true);
            guard.jobs.push_back((index, job.bytes));
        }
        drop(guard);
        self.ready.notify_all();
        Ok(())
    }

    /// Add bounded work without waiting for unrelated running inspections.
    /// By default physical slots remain occupied until publication. Opt-in
    /// recycling moves completed status to a distinct logical inventory;
    /// both inventories count against the same declared total reservation cap.
    pub fn submit_rolling(&mut self, jobs: Vec<Work>) -> Result<(), SubmitError> {
        let wake = jobs.len().min(self.threads);
        let mut guard = self
            .queue
            .lock()
            .map_err(|_| SubmitError::Protocol("epoch inspector queue poisoned (C5)"))?;
        let occupied = guard.occupied.iter().filter(|&&live| live).count();
        if self.cancelled
            || guard.shutdown
            || occupied
                .saturating_add(self.recycled.len())
                .saturating_add(jobs.len())
                > self.logical_limit
        {
            return Err(SubmitError::Protocol(
                "epoch rolling in-flight bound or stopped pool",
            ));
        }
        for (at, job) in jobs.iter().enumerate() {
            if jobs[..at].iter().any(|other| other.key == job.key)
                || self.recycled.iter().any(|status| status.key == job.key)
                || guard
                    .status
                    .iter()
                    .zip(&guard.occupied)
                    .any(|(status, &live)| live && status.key == job.key)
            {
                return Err(SubmitError::Protocol(
                    "epoch duplicate submitted sequence (C5)",
                ));
            }
        }
        // Reserve every collection before accepting any descriptor.
        guard
            .next_generation
            .checked_add(jobs.len() as u64)
            .ok_or(SubmitError::Protocol("epoch slot generation exhausted"))?;
        guard
            .generations
            .try_reserve(jobs.len())
            .map_err(|_| SubmitError::Allocation("epoch generation allocation"))?;
        guard
            .jobs
            .try_reserve(jobs.len())
            .map_err(|_| SubmitError::Allocation("epoch inspector queue allocation"))?;
        guard
            .status
            .try_reserve(jobs.len())
            .map_err(|_| SubmitError::Allocation("epoch inspector status allocation"))?;
        guard
            .occupied
            .try_reserve(jobs.len())
            .map_err(|_| SubmitError::Allocation("epoch inspector slot allocation"))?;
        self.receipts
            .try_reserve(jobs.len())
            .map_err(|_| SubmitError::Allocation("epoch result receipt allocation"))?;
        self.remaining += jobs.len();
        for job in jobs {
            guard.next_generation += 1;
            let generation = guard.next_generation;
            let status = Status {
                key: job.key,
                started: false,
                returned: false,
            };
            let index = if let Some(index) = guard.occupied.iter().position(|&live| !live) {
                guard.occupied[index] = true;
                guard.status[index] = status;
                guard.generations[index] = generation;
                self.receipts[index] = 0;
                index
            } else {
                let index = guard.status.len();
                guard.status.push(status);
                guard.generations.push(generation);
                guard.occupied.push(true);
                self.receipts.push(0);
                index
            };
            guard.jobs.push_back((index, job.bytes));
        }
        drop(guard);
        // Active workers keep pulling from the shared queue. Wake only enough
        // sleepers for the new descriptors, rather than every wide-pool worker
        // after a small cut. A later waiter checks the queue before sleeping.
        for _ in 0..wake {
            self.ready.notify_one();
        }
        Ok(())
    }

    pub fn retire(&mut self, keys: &[u64]) -> Result<(), String> {
        let mut guard = self
            .queue
            .lock()
            .map_err(|_| "epoch inspector queue poisoned (C5)")?;
        for &key in keys {
            if let Some(index) = self.recycled.iter().position(|status| status.key == key) {
                self.recycled.swap_remove(index);
                continue;
            }
            let index = guard
                .status
                .iter()
                .zip(&guard.occupied)
                .position(|(status, &live)| live && status.key == key)
                .ok_or("epoch retirement sequence absent (C5)")?;
            if self.receipts.get(index) != Some(&2) {
                return Err("epoch retirement before returned receipt (C5)".into());
            }
            guard.occupied[index] = false;
        }
        Ok(())
    }

    pub fn retire_all_returned(&mut self) -> Result<(), String> {
        let mut guard = self
            .queue
            .lock()
            .map_err(|_| "epoch inspector queue poisoned (C5)")?;
        if self.remaining != 0
            || !self.recycled.is_empty()
            || guard
                .occupied
                .iter()
                .enumerate()
                .any(|(index, &live)| live && self.receipts.get(index) != Some(&2))
        {
            return Err("epoch batch retirement before returned receipts (C5)".into());
        }
        guard.occupied.fill(false);
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
        let (index, message_key, generation, bytes) = match message {
            Message::Started(index, key, generation) => (index, key, generation, None),
            Message::Result(index, key, generation, bytes) => (index, key, generation, Some(bytes)),
        };
        let receipt = self
            .receipts
            .get_mut(index)
            .ok_or("epoch result index outside batch (C5)")?;
        let guard = self
            .queue
            .lock()
            .map_err(|_| "epoch inspector queue poisoned (C5)")?;
        let key = guard
            .status
            .get(index)
            .ok_or("epoch result status absent (C5)")?
            .key;
        if key != message_key
            || guard.occupied.get(index) != Some(&true)
            || guard.generations.get(index) != Some(&generation)
        {
            return Err("epoch stale result slot generation (C5)".into());
        }
        drop(guard);
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
            .try_reserve_exact(guard.status.len() + self.recycled.len())
            .map_err(|_| "epoch status snapshot allocation")?;
        values.extend(
            guard
                .status
                .iter()
                .zip(&guard.occupied)
                .filter_map(|(status, &live)| live.then_some(*status)),
        );
        values.extend_from_slice(&self.recycled);
        Ok(values)
    }

    /// Move the already allocated B-sized receipt inventory after cancellation.
    /// Late workers cannot mutate this captured stop observation; no allocation
    /// is required on a memory-pressure save path.
    pub fn take_cancelled_status(&mut self) -> Result<Vec<Status>, String> {
        if !self.cancelled {
            return Err("epoch status take before cancellation (C5)".into());
        }
        let mut guard = self
            .queue
            .lock()
            .map_err(|_| "epoch inspector queue poisoned (C5)")?;
        if !guard.shutdown {
            return Err("epoch cancelled status with live queue (C5)".into());
        }
        let mut status = std::mem::take(&mut guard.status);
        let occupied = std::mem::take(&mut guard.occupied);
        let mut index = 0;
        status.retain(|_| {
            let keep = occupied[index];
            index += 1;
            keep
        });
        if status.capacity().saturating_sub(status.len()) < self.recycled.len() {
            return Err("escrow stop inventory exceeds preallocated capacity".into());
        }
        if !self.recycled.is_empty() {
            status.append(&mut self.recycled);
            status.sort_unstable_by_key(|entry| entry.key);
        }
        Ok(status)
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
    with_authorized_pool(threads, &|| Ok(()), job, body).map_err(String::from)
}

/// Every actual worker proves its operational capability before body can
/// submit even one descriptor. A refusal never becomes a malformed job result.
pub(in super::super) fn with_authorized_pool<R>(
    threads: usize,
    authorize: &(dyn Fn() -> Result<(), String> + Sync),
    job: &Inspect<'_>,
    body: impl FnOnce(&mut Pool<'_>) -> R,
) -> Result<R, RunError> {
    if threads == 0 {
        return Err("responsive epoch pool requires an inspector plus its coordinator".into());
    }
    let queue = Mutex::new(Queue {
        jobs: VecDeque::new(),
        status: Vec::new(),
        occupied: Vec::new(),
        generations: Vec::new(),
        next_generation: 0,
        shutdown: false,
    });
    let ready = Condvar::new();
    let stop = AtomicBool::new(false);
    let (sender, receiver) = mpsc::channel();
    let (authorized, authorization) = mpsc::channel();
    let result_memory = Arc::new(memory::Counter::default());
    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        handles
            .try_reserve_exact(threads)
            .map_err(|_| RunError::Resource("epoch worker handle allocation".into()))?;
        for slot in 0..threads {
            let (queue, ready, stop, sender, authorized, result_memory) = (
                &queue,
                &ready,
                &stop,
                sender.clone(),
                authorized.clone(),
                &result_memory,
            );
            match std::thread::Builder::new()
                .name(format!("epoch-inspector-{slot}"))
                .spawn_scoped(scope, move || {
                    let capability = catch_unwind(AssertUnwindSafe(authorize))
                        .unwrap_or_else(|_| Err("epoch worker capability check panicked".into()));
                    let accepted = capability.is_ok();
                    if authorized.send(capability).is_err() || !accepted {
                        return;
                    }
                    drop(authorized);
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
                                    break Some((
                                        index,
                                        guard.status[index].key,
                                        guard.generations[index],
                                        bytes,
                                    ));
                                }
                                guard = match ready.wait(guard) {
                                    Ok(guard) => guard,
                                    Err(_) => return,
                                };
                            }
                        };
                        let Some((index, key, generation, bytes)) = next else {
                            return;
                        };
                        if sender
                            .send(Message::Started(index, key, generation))
                            .is_err()
                        {
                            return;
                        }
                        let result = catch_unwind(AssertUnwindSafe(|| job(&bytes, stop)))
                            .unwrap_or_default();
                        let result = result_memory.wrap(result);
                        let Ok(mut guard) = queue.lock() else { return };
                        if !guard.shutdown {
                            assert_eq!(
                                guard.generations.get(index),
                                Some(&generation),
                                "epoch worker generation changed"
                            );
                        }
                        if let Some(status) = guard.status.get_mut(index) {
                            assert_eq!(status.key, key, "epoch worker slot reused before return");
                            status.returned = true;
                        } else {
                            // Cancellation may have moved the stop inventory to
                            // the save path; never recreate it for a late result.
                            assert!(guard.shutdown, "missing live epoch worker status");
                        }
                        drop(guard);
                        if sender
                            .send(Message::Result(index, key, generation, result))
                            .is_err()
                        {
                            return;
                        }
                    }
                }) {
                Ok(handle) => handles.push(handle),
                Err(error) => {
                    let _ = shutdown(&queue, &ready, &stop);
                    return Err(RunError::Resource(format!(
                        "epoch inspector spawn: {error}"
                    )));
                }
            }
        }
        drop(sender);
        drop(authorized);
        for _ in 0..threads {
            match authorization.recv() {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    let _ = shutdown(&queue, &ready, &stop);
                    return Err(RunError::Capability(error));
                }
                Err(_) => {
                    let _ = shutdown(&queue, &ready, &stop);
                    return Err(RunError::Engine(
                        "epoch worker capability channel disconnected".into(),
                    ));
                }
            }
        }
        let mut pool = Pool {
            queue: &queue,
            ready: &ready,
            stop: &stop,
            receiver: Some(receiver),
            receipts: Vec::new(),
            remaining: 0,
            threads,
            cancelled: false,
            result_memory: Arc::clone(&result_memory),
            recycled: Vec::new(),
            logical_limit: MAX_BATCH,
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
