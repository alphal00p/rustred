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

mod escrow;
mod owner_retention;
use escrow::Escrow;
pub(super) use escrow::Limits as EscrowLimits;

// The observed first owner emits >700k logical callbacks. Even with homogeneous
// successor compression, productive rules usually retain a Count boundary and
// at least one successor run; 64 physical records therefore forced premature
// head-of-line backpressure. These independent bounds permit useful lookahead,
// not an unbounded whole-domain reservoir or a promise of parallel speedup.
// At 50 workers, published+private chunks charge at most 800 MiB logical storage;
// coordinator-held chunk, incoming descriptors, Vec spare capacity, caches,
// allocator overhead, immutable programs and native scratch are additional.
pub(super) const CHUNK_RECORDS: usize = 16_384;
pub(super) const CHUNK_EVENTS: usize = 1_048_576;
pub(super) const CHUNK_BYTES: usize = 8 * 1024 * 1024;

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
    /// Telemetry only: wall time of the current stream, whether the worker is
    /// parked in publish backpressure, events published by the current
    /// stream, and cumulative busy/backpressure/idle seconds of this slot.
    started: Option<Instant>,
    blocked: bool,
    stream_events: usize,
    busy_seconds: f64,
    backpressure_seconds: f64,
    idle_seconds: f64,
    idle_since: Option<Instant>,
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
            started: None,
            blocked: false,
            stream_events: 0,
            busy_seconds: 0.0,
            backpressure_seconds: 0.0,
            idle_seconds: 0.0,
            idle_since: Some(Instant::now()),
        }
    }
}
impl<const N: usize> Slot<N> {
    fn stream_started(&mut self) {
        let now = Instant::now();
        if let Some(idle) = self.idle_since.take() {
            self.idle_seconds += now.duration_since(idle).as_secs_f64();
        }
        self.started = Some(now);
        self.stream_events = 0;
    }
    fn stream_ended(&mut self) {
        let now = Instant::now();
        if let Some(started) = self.started.take() {
            self.busy_seconds += now.duration_since(started).as_secs_f64();
        }
        self.blocked = false;
        self.idle_since = Some(now);
    }
    /// Cumulative busy time including the stream in progress.
    fn busy_now(&self) -> f64 {
        self.busy_seconds
            + self
                .started
                .map_or(0.0, |started| started.elapsed().as_secs_f64())
    }
    fn idle_now(&self) -> f64 {
        self.idle_seconds
            + self
                .idle_since
                .map_or(0.0, |idle| idle.elapsed().as_secs_f64())
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
    escrow: Escrow<N>,
    failure: Option<Failure>,
    /// Chronological first failure stays intact; a later genuine fault must
    /// still disqualify checkpoint pause after cancellation-triggered drain.
    non_cancellation_failure: Option<Failure>,
    shutdown: bool,
    totals: Totals,
}
impl<const N: usize> State<N> {
    fn next_reclaimable(&self, publisher: usize) -> Option<usize> {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(i, s)| {
                s.id.filter(|&id| id > publisher)
                    .filter(|_| Escrow::eligible(s))
                    .map(|id| (i, id))
            })
            .min_by_key(|&(_, id)| id)
            .map(|(slot, _)| slot)
    }
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
    #[cfg(test)]
    fn new(workers: usize) -> Self {
        Self::with_limits(workers, EscrowLimits::default())
    }
    fn with_limits(workers: usize, limits: EscrowLimits) -> Self {
        Self {
            state: Mutex::new(State {
                slots: (0..workers).map(|_| Slot::default()).collect(),
                escrow: Escrow::new(limits),
                failure: None,
                non_cancellation_failure: None,
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
        // StoppedByConsumer is the expected native return when our emitter
        // observes an already-recorded pool stop, not an independent fault.
        let derivative_stop = failure.kind == "consumer_stop" && state.failure.is_some();
        if failure.kind != "cancelled" && !derivative_stop {
            state
                .non_cancellation_failure
                .get_or_insert_with(|| failure.clone());
        }
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
    /// Detach only later successful terminals, never publish their effects.
    /// Native totals were charged by finish(); buffer ownership moves intact.
    pub fn reclaim_finished(&self, publisher: usize) {
        let mut state = self.lock();
        if self.stop.load(Ordering::Acquire) || state.shutdown {
            return;
        }
        while let Some(slot) = state.next_reclaimable(publisher) {
            let Some(charge) = Escrow::charge(&state.slots[slot]) else {
                break; // Accounted-size overflow is an optional-index miss.
            };
            if !state.escrow.reserve(charge) {
                break;
            }
            let id = state.slots[slot].id.expect("eligible escrow slot");
            let State { slots, escrow, .. } = &mut *state;
            escrow.insert(id, &mut slots[slot], charge);
        }
    }
    /// Ready policy: every successful, fully flushed, non-running slot is
    /// detached into the bounded escrow regardless of ID order, so finished
    /// streams stop occupying workers until the coordinator polls them.
    /// Allocation-free; a full store or an oversized charge skips that slot.
    /// Returns the number of slots freed by this call.
    pub fn reclaim_all_finished(&self) -> usize {
        let mut state = self.lock();
        if self.stop.load(Ordering::Acquire) || state.shutdown {
            return 0;
        }
        let mut reclaimed = 0;
        for index in 0..state.slots.len() {
            let Some(id) = state.slots[index].id else {
                continue;
            };
            let Some(charge) = Escrow::charge(&state.slots[index]) else {
                continue;
            };
            if !state.escrow.reserve(charge) {
                continue;
            }
            let State { slots, escrow, .. } = &mut *state;
            escrow.insert(id, &mut slots[index], charge);
            reclaimed += 1;
        }
        reclaimed
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
                current.stream_started();
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
        let events: usize = chunk.iter().map(|event| event.count).sum();
        let mut state = self.lock();
        let started = Instant::now();
        let waits = state.slots[slot].chunk.is_some();
        if waits {
            state.totals.waiting += 1;
            state.slots[slot].blocked = true;
        }
        while state.slots[slot].chunk.is_some()
            && !self.stop.load(Ordering::Acquire)
            && !state.shutdown
        {
            state = self.work.wait(state).unwrap_or_else(|e| e.into_inner());
        }
        if waits {
            state.totals.waiting -= 1;
            let waited = started.elapsed().as_secs_f64();
            state.totals.wait_seconds += waited;
            state.slots[slot].blocked = false;
            state.slots[slot].backpressure_seconds += waited;
        }
        if self.stop.load(Ordering::Acquire) || state.shutdown {
            drop(state);
            self.unbuffer(&chunk);
            return false;
        }
        state.slots[slot].stream_events = state.slots[slot].stream_events.saturating_add(events);
        state.slots[slot].chunk = Some(chunk);
        drop(state);
        self.changed.notify_one();
        true
    }
    pub fn poll(&self, id: usize) -> Poll<N> {
        let mut state = self.lock();
        if let Some(chunk) = state.escrow.take_chunk(id) {
            drop(state);
            self.unbuffer(&chunk);
            return Poll::Events(chunk);
        }
        if let Some(finished) = state.escrow.take_finished(id) {
            return Poll::Finished(finished);
        }
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
            || guard.escrow.contains(id)
            || guard
                .next_reclaimable(id)
                .and_then(|slot| Escrow::charge(&guard.slots[slot]))
                .is_some_and(|charge| guard.escrow.fits(charge))
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
    /// Ready-stream publication waits only when none of its active tickets
    /// has data. Readiness and waiting use the same mutex, avoiding a lost
    /// notification between the coordinator's nonblocking scan and this wait.
    /// Unrelated tickets/notifications cannot make the coordinator busy-spin.
    pub fn wait_for_any_stream(&self, ids: &[usize]) -> bool {
        let guard = self.lock();
        if ids.is_empty() {
            return false;
        }
        let ready = |state: &State<N>| {
            state.slots.iter().any(|slot| {
                slot.id.is_some_and(|id| ids.contains(&id))
                    && (slot.chunk.is_some() || slot.finished.is_some())
            })
        };
        let waiting = |state: &mut State<N>| {
            !self.stop.load(Ordering::Acquire) && !state.shutdown && !ready(state)
        };
        let (guard, _) = self
            .changed
            .wait_timeout_while(guard, Duration::from_millis(100), waiting)
            .unwrap_or_else(|error| error.into_inner());
        !self.stop.load(Ordering::Acquire) && !guard.shutdown && ready(&guard)
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
            super::inspection::NativeStats::Apply(s)
            | super::inspection::NativeStats::ApplyPartial(s, _) => (
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
        state.slots[slot].stream_ended();
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
        self.snapshot_with_slots(true)
    }
    /// Without the per-slot timing arrays, for per-domain progress events.
    pub fn snapshot_lean(&self) -> Value {
        self.snapshot_with_slots(false)
    }
    fn snapshot_with_slots(&self, slots: bool) -> Value {
        let state = self.lock();
        let running = state.slots.iter().filter(|s| s.running).count();
        let blocked = state
            .slots
            .iter()
            .filter(|s| s.running && s.blocked)
            .count();
        let heaviest = state
            .slots
            .iter()
            .filter(|s| s.running)
            .filter_map(|s| Some((s.id?, s.started?.elapsed().as_secs_f64(), s.stream_events)))
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(id, seconds, events)| {
                json!({"id":id, "seconds":seconds, "attempted_events":events})
            });
        let mut snapshot = json!({"workers":state.slots.len(), "active_workers":running,
            "computing_workers":running - blocked,
            "finished_awaiting_poll":state.slots.iter().filter(|s| s.finished.is_some()).count(),
            "heaviest_active_stream":heaviest,
            "stream_stall_share":if running == 0 { 0.0 } else { blocked as f64 / running as f64 },
            "occupied_native_slots":state.slots.iter().filter(|s| s.id.is_some()).count(),
            "dispatched_uncommitted_domains":state.slots.iter().filter(|s| s.id.is_some()).count() + state.escrow.len(),
            "finished_uncommitted_domains":state.slots.iter().filter(|s| s.finished.is_some()).count() + state.escrow.len(),
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
            "first_failure":state.failure.as_ref().map(Failure::json),
            "non_cancellation_failure":state.non_cancellation_failure.as_ref().map(Failure::json)});
        if slots {
            snapshot["slot_busy_seconds"] =
                json!(state.slots.iter().map(Slot::busy_now).collect::<Vec<_>>());
            snapshot["slot_backpressure_seconds"] = json!(
                state
                    .slots
                    .iter()
                    .map(|s| s.backpressure_seconds)
                    .collect::<Vec<_>>()
            );
            snapshot["slot_idle_seconds"] =
                json!(state.slots.iter().map(Slot::idle_now).collect::<Vec<_>>());
            snapshot["slot_timing_scope"] = json!(
                "cumulative_wall_seconds_per_physical_slot_this_process; busy_includes_backpressure; idle_is_time_without_a_stream"
            );
        }
        let escrow = json!({
            "worker_buffer_accounting_scope":"all_pool_owned_chunks_including_completed_escrow; excludes_coordinator_chunk",
            "completed_escrow_entries":state.escrow.len(),
            "completed_escrow_events":state.escrow.events,
            "completed_escrow_accounted_bytes":state.escrow.bytes,
            "completed_escrow_peak_entries":state.escrow.peak_entries,
            "completed_escrow_peak_accounted_bytes":state.escrow.peak_bytes,
            "completed_slots_reclaimed":state.escrow.reclaimed,
            "completed_escrow_max_entries":state.escrow.limits.entries,
            "completed_escrow_max_accounted_bytes":state.escrow.limits.bytes,
            "completed_escrow_reserve_fallback":state.escrow.reserve_failed});
        if let Value::Object(fields) = escrow {
            snapshot
                .as_object_mut()
                .expect("snapshot object")
                .extend(fields);
        }
        snapshot
    }
    /// Called after join. At most W + escrow-entry-limit summaries; speculative
    /// streams are released, never committed. Returned stats are not recounted.
    pub fn uncommitted(&self) -> Vec<(usize, Finished)> {
        let mut state = self.lock();
        let mut finished: Vec<_> = state
            .slots
            .iter_mut()
            .filter_map(|s| s.finished.take().map(|f| (s.id.expect("finished id"), f)))
            .collect();
        for (id, entry) in state.escrow.drain() {
            if let Some(chunk) = entry.chunk {
                self.unbuffer(&chunk);
            }
            finished.push((id, entry.finished));
        }
        finished
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
        state.escrow.clear_chunks(|chunk| self.unbuffer(chunk));
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

/// The pool's IDs are physical handles. The ordered coordinator supplies a
/// typed parent/part mapping; legacy users continue to use one handle per job.
/// The completed-result store bound is explicit: Ordered keeps the default,
/// Ready sizes it by its inspector count.
pub(super) fn with_ticket_pool_escrow<const N: usize, R>(
    workers: usize,
    limits: EscrowLimits,
    inspect: impl Fn(
        usize,
        &Domain<N>,
        &AtomicBool,
        &mut dyn FnMut(Event<N>) -> ControlFlow<()>,
    ) -> Finished
    + Sync,
    coordinate: impl FnOnce(&Pool<N>) -> R,
) -> (R, Value, Vec<(usize, Finished)>) {
    with_ticket_pool_limits(workers, None, limits, inspect, coordinate)
}
fn with_pool_inner<const N: usize, R>(
    workers: usize,
    fail_spawn_at: Option<usize>,
    inspect: impl Fn(&Domain<N>, &AtomicBool, &mut dyn FnMut(Event<N>) -> ControlFlow<()>) -> Finished
    + Sync,
    coordinate: impl FnOnce(&Pool<N>) -> R,
) -> (R, Value, Vec<(usize, Finished)>) {
    with_pool_limits(
        workers,
        fail_spawn_at,
        EscrowLimits::default(),
        inspect,
        coordinate,
    )
}
fn with_pool_limits<const N: usize, R>(
    workers: usize,
    fail_spawn_at: Option<usize>,
    limits: EscrowLimits,
    inspect: impl Fn(&Domain<N>, &AtomicBool, &mut dyn FnMut(Event<N>) -> ControlFlow<()>) -> Finished
    + Sync,
    coordinate: impl FnOnce(&Pool<N>) -> R,
) -> (R, Value, Vec<(usize, Finished)>) {
    with_ticket_pool_limits(
        workers,
        fail_spawn_at,
        limits,
        |_, domain, stop, emit| inspect(domain, stop, emit),
        coordinate,
    )
}

fn with_ticket_pool_limits<const N: usize, R>(
    workers: usize,
    fail_spawn_at: Option<usize>,
    limits: EscrowLimits,
    inspect: impl Fn(
        usize,
        &Domain<N>,
        &AtomicBool,
        &mut dyn FnMut(Event<N>) -> ControlFlow<()>,
    ) -> Finished
    + Sync,
    coordinate: impl FnOnce(&Pool<N>) -> R,
) -> (R, Value, Vec<(usize, Finished)>) {
    let pool = Pool::with_limits(workers, limits);
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
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| inspect(id, &domain, &pool.stop, &mut |event| emitter.emit(event))));
                    match result {
                        Ok(finished) => {
                            if let Some(error) = &finished.error {
                                pool.fail(Failure { id: Some(id), phase: Some(domain.phase), kind: finished.error_kind, detail: error.clone() });
                            } else { emitter.flush(); }
                            pool.finish(slot, finished);
                        }
                        Err(_) => {
                            pool.fail(Failure { id: Some(id), phase: Some(domain.phase), kind: "worker_panic", detail: "symbolic inspection worker panicked; partial native stats unavailable".into() });
                            let mut state = pool.lock();
                            state.slots[slot].running = false;
                            state.slots[slot].stream_ended();
                        }
                    }
                }
                }));
                if lifecycle.is_err() {
                    let (id, phase) = { let mut state = pool.lock();
                        let s = &mut state.slots[slot]; s.running = false; s.stream_ended(); (s.id, s.phase) };
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

#[cfg(test)]
mod ready_stream_tests;
