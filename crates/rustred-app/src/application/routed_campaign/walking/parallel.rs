//! Stable bounded streaming slots. Native work never holds this scheduler lock.
#[cfg(test)]
use super::inspection::Effect;
use super::inspection::{Event, Finished};
use super::queue::{Domain, Phase};
use serde_json::{Value, json};
use std::ops::ControlFlow;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

pub(super) const CHUNK_RECORDS: usize = 64;
pub(super) const CHUNK_EVENTS: usize = 65_536;
pub(super) const CHUNK_BYTES: usize = 256 * 1024;

#[derive(Clone)]
pub(super) struct Failure {
    pub id: Option<usize>,
    pub phase: Option<Phase>,
    pub kind: &'static str,
    pub detail: String,
}
impl Failure {
    pub fn json(&self) -> Value {
        json!({"domain":self.id, "phase":self.phase.map(|p| format!("{p:?}")),
            "kind":self.kind, "detail":self.detail})
    }
}
struct Slot<const N: usize> {
    id: Option<usize>,
    phase: Option<Phase>,
    job: Option<Arc<Domain<N>>>,
    running: bool,
    chunk: Option<Vec<Event<N>>>,
    finished: Option<Finished>,
}
impl<const N: usize> Default for Slot<N> {
    fn default() -> Self {
        Self {
            id: None,
            phase: None,
            job: None,
            running: false,
            chunk: None,
            finished: None,
        }
    }
}
#[derive(Default)]
struct Totals {
    returned: usize,
    native: usize,
    rules: usize,
    predicates: usize,
    optional: usize,
    wait_seconds: f64,
    waiting: usize,
}
struct State<const N: usize> {
    slots: Vec<Slot<N>>,
    failure: Option<Failure>,
    shutdown: bool,
    totals: Totals,
}
pub(super) struct Pool<const N: usize> {
    state: Mutex<State<N>>,
    work: Condvar,
    changed: Condvar,
    pub stop: AtomicBool,
    attempted: AtomicUsize,
    buffered_events: AtomicUsize,
    buffered_bytes: AtomicUsize,
    peak_bytes: AtomicUsize,
}
pub(super) enum Poll<const N: usize> {
    Events(Vec<Event<N>>),
    Finished(Finished),
    Waiting,
}
impl<const N: usize> Pool<N> {
    fn new(workers: usize) -> Self {
        Self {
            state: Mutex::new(State {
                slots: (0..workers).map(|_| Slot::default()).collect(),
                failure: None,
                shutdown: false,
                totals: Totals::default(),
            }),
            work: Condvar::new(),
            changed: Condvar::new(),
            stop: AtomicBool::new(false),
            attempted: AtomicUsize::new(0),
            buffered_events: AtomicUsize::new(0),
            buffered_bytes: AtomicUsize::new(0),
            peak_bytes: AtomicUsize::new(0),
        }
    }
    fn lock(&self) -> std::sync::MutexGuard<'_, State<N>> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
    pub fn fail(&self, failure: Failure) {
        let mut state = self.lock();
        if state.failure.is_none() {
            state.failure = Some(failure);
        }
        self.stop.store(true, Ordering::Release);
        drop(state);
        self.work.notify_all();
        self.changed.notify_all();
    }
    pub fn failure(&self) -> Option<Failure> {
        self.lock().failure.clone()
    }
    pub fn dispatch(&self, id: usize, domain: Arc<Domain<N>>) -> bool {
        let mut state = self.lock();
        if self.stop.load(Ordering::Acquire) || state.shutdown {
            return false;
        }
        let Some(slot) = state.slots.iter_mut().find(|s| s.id.is_none()) else {
            return false;
        };
        slot.id = Some(id);
        slot.phase = Some(domain.phase);
        slot.job = Some(domain);
        drop(state);
        self.work.notify_all();
        true
    }
    fn take(&self, slot: usize) -> Option<(usize, Arc<Domain<N>>)> {
        let mut state = self.lock();
        loop {
            if state.shutdown || self.stop.load(Ordering::Acquire) {
                return None;
            }
            let current = &mut state.slots[slot];
            if let Some(job) = current.job.take() {
                current.running = true;
                return Some((current.id.expect("assigned slot"), job));
            }
            state = self.work.wait(state).unwrap_or_else(|e| e.into_inner());
        }
    }
    fn unbuffer(&self, events: &[Event<N>]) {
        self.buffered_events.fetch_sub(
            events.iter().map(|e| e.count).sum::<usize>(),
            Ordering::Relaxed,
        );
        self.buffered_bytes.fetch_sub(
            events.iter().map(Event::weight).sum::<usize>(),
            Ordering::Relaxed,
        );
    }
    fn publish(&self, slot: usize, chunk: Vec<Event<N>>) -> bool {
        let mut state = self.lock();
        let started = Instant::now();
        let waits = state.slots[slot].chunk.is_some();
        if waits {
            state.totals.waiting += 1;
        }
        while state.slots[slot].chunk.is_some()
            && !self.stop.load(Ordering::Acquire)
            && !state.shutdown
        {
            state = self.work.wait(state).unwrap_or_else(|e| e.into_inner());
        }
        if waits {
            state.totals.waiting -= 1;
            state.totals.wait_seconds += started.elapsed().as_secs_f64();
        }
        if self.stop.load(Ordering::Acquire) || state.shutdown {
            drop(state);
            self.unbuffer(&chunk);
            return false;
        }
        state.slots[slot].chunk = Some(chunk);
        drop(state);
        self.changed.notify_one();
        true
    }
    pub fn poll(&self, id: usize) -> Poll<N> {
        let mut state = self.lock();
        let Some(slot) = state.slots.iter_mut().find(|s| s.id == Some(id)) else {
            return Poll::Waiting;
        };
        if let Some(chunk) = slot.chunk.take() {
            drop(state);
            self.unbuffer(&chunk);
            self.work.notify_all();
            return Poll::Events(chunk);
        }
        if let Some(finished) = slot.finished.take() {
            slot.id = None;
            slot.phase = None;
            return Poll::Finished(finished);
        }
        Poll::Waiting
    }
    pub fn wait(&self, id: usize) {
        let guard = self.lock();
        if self.stop.load(Ordering::Acquire)
            || guard
                .slots
                .iter()
                .any(|s| s.id == Some(id) && (s.chunk.is_some() || s.finished.is_some()))
        {
            return;
        }
        let _ = self
            .changed
            .wait_timeout(guard, Duration::from_millis(100))
            .unwrap_or_else(|e| e.into_inner());
    }
    pub fn wait_drained(&self) -> bool {
        let guard = self.lock();
        if !guard.slots.iter().any(|s| s.running) {
            return true;
        }
        let (guard, _) = self
            .changed
            .wait_timeout(guard, Duration::from_millis(250))
            .unwrap_or_else(|e| e.into_inner());
        !guard.slots.iter().any(|s| s.running)
    }
    fn finish(&self, slot: usize, finished: Finished) {
        let mut state = self.lock();
        let (rules, predicates, optional) = match finished.stats {
            super::inspection::NativeStats::Apply(s) => (
                s.matching.rules,
                s.matching.predicates,
                s.optional_coefficient_refusals,
            ),
            _ => (0, 0, 0),
        };
        let next = (|| {
            Some((
                state.totals.returned.checked_add(1)?,
                state
                    .totals
                    .native
                    .checked_add(finished.native_operations())?,
                state.totals.rules.checked_add(rules)?,
                state.totals.predicates.checked_add(predicates)?,
                state.totals.optional.checked_add(optional)?,
            ))
        })();
        if let Some((returned, native, rules, predicates, optional)) = next {
            state.totals.returned = returned;
            state.totals.native = native;
            state.totals.rules = rules;
            state.totals.predicates = predicates;
            state.totals.optional = optional;
        }
        let id = state.slots[slot].id;
        let phase = state.slots[slot].phase;
        state.slots[slot].running = false;
        state.slots[slot].finished = Some(finished);
        drop(state);
        if next.is_none() {
            self.fail(Failure {
                id,
                phase,
                kind: "counter_overflow",
                detail: "attempted inspection counters overflow".into(),
            });
        }
        self.changed.notify_one();
    }
    pub fn snapshot(&self) -> Value {
        let state = self.lock();
        json!({"workers":state.slots.len(), "active_workers":state.slots.iter().filter(|s| s.running).count(),
            "dispatched_uncommitted_domains":state.slots.iter().filter(|s| s.id.is_some()).count(),
            "finished_uncommitted_domains":state.slots.iter().filter(|s| s.finished.is_some()).count(),
            "backpressured_workers":state.totals.waiting, "backpressure_seconds":state.totals.wait_seconds,
            "attempted_events":self.attempted.load(Ordering::Relaxed),
            "returned_inspections":state.totals.returned, "attempted_native_operations":state.totals.native,
            "attempted_rule_checks":state.totals.rules, "attempted_predicates":state.totals.predicates,
            "attempted_optional_coefficient_refusals":state.totals.optional,
            "native_attempt_counters_scope":"returned_inspections_including_uncommitted_and_cancelled",
            "worker_buffered_events":self.buffered_events.load(Ordering::Relaxed),
            "worker_buffered_logical_bytes":self.buffered_bytes.load(Ordering::Relaxed),
            "peak_worker_buffered_logical_bytes":self.peak_bytes.load(Ordering::Relaxed),
            "per_worker_chunk_events":CHUNK_EVENTS, "per_worker_chunk_records":CHUNK_RECORDS,
            "per_worker_chunk_logical_bytes":CHUNK_BYTES,
            "first_failure":state.failure.as_ref().map(Failure::json)})
    }
    /// Called after join. At most W summaries; no speculative event stream is
    /// retained or treated as committed. Exact returned stats remain auditable.
    pub fn uncommitted(&self) -> Vec<(usize, Finished)> {
        let mut state = self.lock();
        state
            .slots
            .iter_mut()
            .filter_map(|s| s.finished.take().map(|f| (s.id.expect("finished id"), f)))
            .collect()
    }
    fn shutdown(&self) {
        self.stop.store(true, Ordering::Release);
        self.lock().shutdown = true;
        self.work.notify_all();
        self.changed.notify_all();
    }
    fn clear_buffers(&self) {
        let mut state = self.lock();
        for slot in &mut state.slots {
            if let Some(c) = slot.chunk.take() {
                self.unbuffer(&c);
            }
        }
    }
}

struct Emitter<'a, const N: usize> {
    pool: &'a Pool<N>,
    slot: usize,
    id: usize,
    phase: Phase,
    chunk: Vec<Event<N>>,
    bytes: usize,
    events: usize,
}
impl<const N: usize> Emitter<'_, N> {
    fn flush(&mut self) -> bool {
        if self.chunk.is_empty() {
            return true;
        }
        self.bytes = 0;
        self.events = 0;
        self.pool
            .publish(self.slot, std::mem::take(&mut self.chunk))
    }
    fn emit(&mut self, event: Event<N>) -> ControlFlow<()> {
        if self
            .pool
            .attempted
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
                v.checked_add(event.count)
            })
            .is_err()
        {
            self.pool.fail(Failure {
                id: Some(self.id),
                phase: Some(self.phase),
                kind: "counter_overflow",
                detail: "attempted events overflow".into(),
            });
            return ControlFlow::Break(());
        }
        if self.pool.stop.load(Ordering::Acquire) {
            return ControlFlow::Break(());
        }
        let weight = event.weight();
        if weight > CHUNK_BYTES || event.count == 0 || event.count > CHUNK_EVENTS {
            self.pool.fail(Failure {
                id: Some(self.id),
                phase: Some(self.phase),
                kind: "buffer_limit",
                detail:
                    "single symbolic descriptor exceeds worker chunk allowance or has invalid count"
                        .into(),
            });
            return ControlFlow::Break(());
        }
        let merges = self.chunk.last().is_some_and(|last| last.mergeable(&event));
        if (self.events + event.count > CHUNK_EVENTS
            || (!merges && self.chunk.len() >= CHUNK_RECORDS)
            || (!merges && self.bytes + weight > CHUNK_BYTES))
            && !self.flush()
        {
            return ControlFlow::Break(());
        }
        self.pool
            .buffered_events
            .fetch_add(event.count, Ordering::Relaxed);
        self.events += event.count;
        if let Some(last) = self.chunk.last_mut()
            && last.mergeable(&event)
        {
            last.count += event.count;
        } else {
            self.pool
                .buffered_bytes
                .fetch_add(weight, Ordering::Relaxed);
            self.bytes += weight;
            self.chunk.push(event);
            self.pool.peak_bytes.fetch_max(
                self.pool.buffered_bytes.load(Ordering::Relaxed),
                Ordering::Relaxed,
            );
        }
        if self.events >= CHUNK_EVENTS && !self.flush() {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    }
}
impl<const N: usize> Drop for Emitter<'_, N> {
    fn drop(&mut self) {
        self.pool.unbuffer(&self.chunk);
    }
}

/// Observer/coordinator panic is caught solely to stop and join producers
/// before resuming unwinding. No detached workers or blocking channel sends.
pub(super) fn with_pool<const N: usize, R>(
    workers: usize,
    inspect: impl Fn(&Domain<N>, &AtomicBool, &mut dyn FnMut(Event<N>) -> ControlFlow<()>) -> Finished
    + Sync,
    coordinate: impl FnOnce(&Pool<N>) -> R,
) -> (R, Value, Vec<(usize, Finished)>) {
    with_pool_inner(workers, None, inspect, coordinate)
}
fn with_pool_inner<const N: usize, R>(
    workers: usize,
    fail_spawn_at: Option<usize>,
    inspect: impl Fn(&Domain<N>, &AtomicBool, &mut dyn FnMut(Event<N>) -> ControlFlow<()>) -> Finished
    + Sync,
    coordinate: impl FnOnce(&Pool<N>) -> R,
) -> (R, Value, Vec<(usize, Finished)>) {
    let pool = Pool::new(workers);
    let result = std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for slot in 0..workers {
            if fail_spawn_at == Some(slot) {
                pool.fail(Failure {
                    id: None,
                    phase: None,
                    kind: "worker_spawn",
                    detail: "injected worker spawn failure".into(),
                });
                break;
            }
            let pool = &pool;
            let inspect = &inspect;
            match std::thread::Builder::new().name(format!("owner-domain-{slot}")).spawn_scoped(scope, move || {
                let lifecycle = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                if !symbolica::license::LicenseManager::is_licensed() {
                    pool.fail(Failure { id: None, phase: None, kind: "worker_license", detail: "parallel Symbolica worker requires a license".into() });
                    return;
                }
                while let Some((id, domain)) = pool.take(slot) {
                    let mut emitter = Emitter { pool, slot, id, phase: domain.phase, chunk: Vec::new(), bytes: 0, events: 0 };
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| inspect(&domain, &pool.stop, &mut |event| emitter.emit(event))));
                    match result {
                        Ok(finished) => {
                            if let Some(error) = &finished.error {
                                pool.fail(Failure { id: Some(id), phase: Some(domain.phase), kind: finished.error_kind, detail: error.clone() });
                            } else { emitter.flush(); }
                            pool.finish(slot, finished);
                        }
                        Err(_) => {
                            pool.fail(Failure { id: Some(id), phase: Some(domain.phase), kind: "worker_panic", detail: "symbolic inspection worker panicked; partial native stats unavailable".into() });
                            pool.lock().slots[slot].running = false;
                        }
                    }
                }
                }));
                if lifecycle.is_err() {
                    let (id, phase) = { let mut state = pool.lock();
                        let s = &mut state.slots[slot]; s.running = false; (s.id, s.phase) };
                    pool.fail(Failure { id, phase, kind: "worker_panic", detail: "symbolic worker lifecycle panicked; partial stats unavailable".into() });
                }
            }) {
                Ok(h) => handles.push(h),
                Err(e) => { pool.fail(Failure { id: None, phase: None, kind: "worker_spawn", detail: e.to_string() }); break; }
            }
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| coordinate(&pool)));
        pool.shutdown();
        for handle in handles {
            if handle.join().is_err() {
                pool.fail(Failure {
                    id: None,
                    phase: None,
                    kind: "worker_panic",
                    detail: "symbolic worker lifecycle panicked".into(),
                });
            }
        }
        pool.clear_buffers();
        result
    });
    let result = match result {
        Ok(r) => r,
        Err(panic) => std::panic::resume_unwind(panic),
    };
    (result, pool.snapshot(), pool.uncommitted())
}

#[cfg(test)]
mod tests;
