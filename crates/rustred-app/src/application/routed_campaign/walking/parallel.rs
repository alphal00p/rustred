//! Stable bounded streaming slots. Native work never holds this scheduler lock.
#[cfg(test)]
use super::inspection::Effect;
use super::inspection::{Event, Finished};
use super::queue::{Domain, Phase};
use serde_json::{Value, json};
use std::ops::{ControlFlow, Deref};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, TryLockError};
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
/// One cache line (two on CPUs that prefetch pairs) per contended word. The
/// stop flag is read on every committed record and helper lookup while the
/// inspectors update the buffer counters on every emitted event, and the
/// pool mutex is taken by all of them; sharing a line would turn each read
/// into a coherence miss. Scheduling only.
#[repr(align(128))]
#[derive(Default)]
pub(super) struct Padded<T>(T);
impl<T> Deref for Padded<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

/// A published chunk with the totals its Emitter accumulated while filling
/// it: `count` is the sum of `Event::count`, `weight` the sum of
/// `Event::weight` (which does not depend on the count, so merged runs keep
/// their first event's weight). The escrow charge taken under the pool lock
/// and the coordinator's buffer release use them instead of re-walking up to
/// CHUNK_RECORDS events.
pub(super) struct Chunk<const N: usize> {
    pub events: Vec<Event<N>>,
    pub count: usize,
    pub weight: usize,
}
impl<const N: usize> Chunk<N> {
    fn totals(events: &[Event<N>]) -> (usize, usize) {
        (
            events.iter().map(|event| event.count).sum(),
            events.iter().map(Event::weight).sum(),
        )
    }
}

/// Session synchronization counters, kept under the pool lock (no extra
/// shared atomics). Telemetry only.
#[derive(Default)]
struct SyncCounters {
    /// Targeted condvar notifications to a parked inspector: a dispatch into
    /// its slot or the poll of its published chunk.
    inspector_wakeups: u64,
    /// Such state changes whose inspector was not parked (no notification).
    inspector_wakeups_skipped: u64,
    /// Inspectors parked at those points: what the former shared `work`
    /// condvar's `notify_all` woke each time.
    shared_condvar_wakeups: u64,
    /// Stop/failure/shutdown broadcasts to every slot.
    broadcasts: u64,
    coordinator_wakeups: u64,
    coordinator_wakeups_skipped: u64,
    coordinator_lock: LockWait,
    inspector_lock: LockWait,
}
#[derive(Default)]
struct LockWait {
    acquisitions: u64,
    contended: u64,
    wait_seconds: f64,
}
impl LockWait {
    fn json(&self) -> Value {
        json!({"acquisitions":self.acquisitions, "contended":self.contended,
            "wait_seconds":self.wait_seconds})
    }
}
#[derive(Clone, Copy)]
enum Role {
    Coordinator,
    Inspector,
}

struct Slot<const N: usize> {
    id: Option<usize>,
    phase: Option<Phase>,
    job: Option<Arc<Domain<N>>>,
    running: bool,
    chunk: Option<Chunk<N>>,
    finished: Option<Finished>,
    /// Its inspector waits on this slot's condvar (in `take` or `publish`).
    parked: bool,
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
            parked: false,
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
    /// Inspectors parked on their slot condvars, and coordinator-side
    /// waiters on `changed`: a notifier signals only when someone waits.
    parked: usize,
    changed_waiters: usize,
    sync: SyncCounters,
}
impl<const N: usize> State<N> {
    fn park(&mut self, slot: usize) {
        self.slots[slot].parked = true;
        self.parked += 1;
    }
    fn unpark(&mut self, slot: usize) {
        self.slots[slot].parked = false;
        self.parked -= 1;
    }
    /// A job or chunk slot changed for `slot`'s inspector: whether it is
    /// parked and must be notified (after the lock is released).
    fn wakes(&mut self, slot: usize) -> bool {
        self.sync.shared_condvar_wakeups += self.parked as u64;
        if self.slots[slot].parked {
            self.sync.inspector_wakeups += 1;
            true
        } else {
            self.sync.inspector_wakeups_skipped += 1;
            false
        }
    }
    /// Whether a producer-side change must notify `changed`.
    fn wakes_coordinator(&mut self) -> bool {
        if self.changed_waiters > 0 {
            self.sync.coordinator_wakeups += 1;
            true
        } else {
            self.sync.coordinator_wakeups_skipped += 1;
            false
        }
    }
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
/// How much of the pool state a snapshot serializes; see the `snapshot_*`
/// methods. Lean is the historical key set.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum SnapshotTier {
    Lean,
    Detailed,
    Full,
}

/// Written by every inspector on every emitted event.
#[derive(Default)]
struct Counters {
    attempted: AtomicUsize,
    buffered_events: AtomicUsize,
    buffered_bytes: AtomicUsize,
    peak_bytes: AtomicUsize,
}

pub(super) struct Pool<const N: usize> {
    state: Padded<Mutex<State<N>>>,
    /// One condvar per slot. Its inspector parks there for a job (`take`) or
    /// for its published chunk to be polled (`publish`), so a dispatch or a
    /// poll wakes exactly that inspector, and only when it is parked; a stop
    /// broadcasts to all of them.
    work: Box<[Padded<Condvar>]>,
    changed: Padded<Condvar>,
    pub stop: Padded<AtomicBool>,
    /// Set under the lock together with the first failure, so `failure()`
    /// answers without the lock until one exists.
    failed: Padded<AtomicBool>,
    counters: Padded<Counters>,
}
pub(super) enum Poll<const N: usize> {
    Events(Vec<Event<N>>),
    Finished(Finished),
    Waiting,
}
impl<const N: usize> Pool<N> {
    #[cfg(test)]
    pub(super) fn new(workers: usize) -> Self {
        Self::with_limits(workers, EscrowLimits::default())
    }
    fn with_limits(workers: usize, limits: EscrowLimits) -> Self {
        Self {
            state: Padded(Mutex::new(State {
                slots: (0..workers).map(|_| Slot::default()).collect(),
                escrow: Escrow::new(limits),
                failure: None,
                non_cancellation_failure: None,
                shutdown: false,
                totals: Totals::default(),
                parked: 0,
                changed_waiters: 0,
                sync: SyncCounters::default(),
            })),
            work: (0..workers).map(|_| Padded::default()).collect(),
            changed: Padded::default(),
            stop: Padded::default(),
            failed: Padded::default(),
            counters: Padded::default(),
        }
    }
    /// Coordinator-side acquisition (also tests and the rare shared paths).
    fn lock(&self) -> MutexGuard<'_, State<N>> {
        self.lock_as(Role::Coordinator)
    }
    /// Uncontended acquisitions cost one extra CAS; only a contended one
    /// reads the clock.
    fn lock_as(&self, role: Role) -> MutexGuard<'_, State<N>> {
        let (mut state, waited) = match self.state.try_lock() {
            Ok(state) => (state, None),
            Err(TryLockError::Poisoned(error)) => (error.into_inner(), None),
            Err(TryLockError::WouldBlock) => {
                let started = Instant::now();
                let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
                (state, Some(started.elapsed().as_secs_f64()))
            }
        };
        let wait = match role {
            Role::Coordinator => &mut state.sync.coordinator_lock,
            Role::Inspector => &mut state.sync.inspector_lock,
        };
        wait.acquisitions += 1;
        if let Some(seconds) = waited {
            wait.contended += 1;
            wait.wait_seconds += seconds;
        }
        state
    }
    /// Wake every parked thread: stop, failure and shutdown only.
    fn broadcast(&self, mut state: MutexGuard<'_, State<N>>) {
        state.sync.broadcasts += 1;
        drop(state);
        for work in &self.work {
            work.notify_all();
        }
        self.changed.notify_all();
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
            self.failed.store(true, Ordering::Release);
        }
        self.stop.store(true, Ordering::Release);
        self.broadcast(state);
    }
    /// Lock-free while no failure has been recorded: the coordinator asks on
    /// every loop iteration and dispatched ID.
    pub fn failure(&self) -> Option<Failure> {
        if !self.failed.load(Ordering::Acquire) {
            return None;
        }
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
    /// Allocation-free. Once the store has no room (entry cap or a failed
    /// reserve) no slot is even charged; a byte-cap miss skips only that
    /// slot so a smaller result can still fit. Returns the slots freed.
    pub fn reclaim_all_finished(&self) -> usize {
        let mut state = self.lock();
        if self.stop.load(Ordering::Acquire) || state.shutdown || !state.escrow.has_room() {
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
                if !state.escrow.has_room() {
                    break;
                }
                continue;
            }
            let State { slots, escrow, .. } = &mut *state;
            escrow.insert(id, &mut slots[index], charge);
            reclaimed += 1;
            if !state.escrow.has_room() {
                break;
            }
        }
        reclaimed
    }
    pub fn dispatch(&self, id: usize, domain: Arc<Domain<N>>) -> bool {
        let mut state = self.lock();
        if self.stop.load(Ordering::Acquire) || state.shutdown {
            return false;
        }
        let Some(index) = state.slots.iter().position(|s| s.id.is_none()) else {
            return false;
        };
        let slot = &mut state.slots[index];
        slot.id = Some(id);
        slot.phase = Some(domain.phase);
        slot.job = Some(domain);
        let wake = state.wakes(index);
        drop(state);
        if wake {
            self.work[index].notify_one();
        }
        true
    }
    fn take(&self, slot: usize) -> Option<(usize, Arc<Domain<N>>)> {
        let mut state = self.lock_as(Role::Inspector);
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
            state.park(slot);
            state = self.work[slot]
                .wait(state)
                .unwrap_or_else(|e| e.into_inner());
            state.unpark(slot);
        }
    }
    fn unbuffer(&self, events: usize, bytes: usize) {
        self.counters
            .buffered_events
            .fetch_sub(events, Ordering::Relaxed);
        self.counters
            .buffered_bytes
            .fetch_sub(bytes, Ordering::Relaxed);
    }
    /// Rare paths (drain, shutdown) that hold a bare chunk.
    fn unbuffer_events(&self, events: &[Event<N>]) {
        let (count, weight) = Chunk::totals(events);
        self.unbuffer(count, weight);
    }
    fn publish(&self, slot: usize, chunk: Chunk<N>) -> bool {
        #[cfg(test)]
        assert_eq!(
            Chunk::totals(&chunk.events),
            (chunk.count, chunk.weight),
            "emitter totals"
        );
        let mut state = self.lock_as(Role::Inspector);
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
            state.park(slot);
            state = self.work[slot]
                .wait(state)
                .unwrap_or_else(|e| e.into_inner());
            state.unpark(slot);
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
            self.unbuffer(chunk.count, chunk.weight);
            return false;
        }
        state.slots[slot].stream_events = state.slots[slot]
            .stream_events
            .saturating_add(chunk.count);
        state.slots[slot].chunk = Some(chunk);
        let wake = state.wakes_coordinator();
        drop(state);
        if wake {
            self.changed.notify_one();
        }
        true
    }
    pub fn poll(&self, id: usize) -> Poll<N> {
        let mut state = self.lock();
        if let Some(chunk) = state.escrow.take_chunk(id) {
            drop(state);
            self.unbuffer(chunk.count, chunk.weight);
            return Poll::Events(chunk.events);
        }
        if let Some(finished) = state.escrow.take_finished(id) {
            return Poll::Finished(finished);
        }
        let Some(index) = state.slots.iter().position(|s| s.id == Some(id)) else {
            return Poll::Waiting;
        };
        if let Some(chunk) = state.slots[index].chunk.take() {
            let wake = state.wakes(index);
            drop(state);
            self.unbuffer(chunk.count, chunk.weight);
            if wake {
                self.work[index].notify_one();
            }
            return Poll::Events(chunk.events);
        }
        let slot = &mut state.slots[index];
        if let Some(finished) = slot.finished.take() {
            slot.id = None;
            slot.phase = None;
            return Poll::Finished(finished);
        }
        Poll::Waiting
    }
    pub fn wait(&self, id: usize) {
        let mut guard = self.lock();
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
        guard.changed_waiters += 1;
        let (mut guard, _) = self
            .changed
            .wait_timeout(guard, Duration::from_millis(100))
            .unwrap_or_else(|e| e.into_inner());
        guard.changed_waiters -= 1;
    }
    /// Ready-stream publication waits only when none of its active tickets
    /// has data. Readiness and waiting use the same mutex, avoiding a lost
    /// notification between the coordinator's nonblocking scan and this wait.
    /// Unrelated tickets/notifications cannot make the coordinator busy-spin.
    pub fn wait_for_any_stream(&self, ids: &[usize]) -> bool {
        let mut guard = self.lock();
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
        guard.changed_waiters += 1;
        let (mut guard, _) = self
            .changed
            .wait_timeout_while(guard, Duration::from_millis(100), waiting)
            .unwrap_or_else(|error| error.into_inner());
        guard.changed_waiters -= 1;
        !self.stop.load(Ordering::Acquire) && !guard.shutdown && ready(&guard)
    }

    pub fn wait_drained(&self) -> bool {
        let mut guard = self.lock();
        if !guard.slots.iter().any(|s| s.running) {
            return true;
        }
        guard.changed_waiters += 1;
        let (mut guard, _) = self
            .changed
            .wait_timeout(guard, Duration::from_millis(250))
            .unwrap_or_else(|e| e.into_inner());
        guard.changed_waiters -= 1;
        !guard.slots.iter().any(|s| s.running)
    }
    fn finish(&self, slot: usize, finished: Finished) {
        let mut state = self.lock_as(Role::Inspector);
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
        let wake = state.wakes_coordinator();
        drop(state);
        if next.is_none() {
            self.fail(Failure {
                id,
                phase,
                kind: "counter_overflow",
                detail: "attempted inspection counters overflow".into(),
            });
        }
        if wake {
            self.changed.notify_one();
        }
    }
    /// Everything, including the per-slot timing arrays (3 x W numbers): the
    /// drain events and the final report only.
    pub fn snapshot(&self) -> Value {
        self.snapshot_tier(SnapshotTier::Full)
    }
    /// Scalar activity aggregates on top of the historical keys, without the
    /// per-slot arrays: the heartbeat tier.
    pub fn snapshot_detailed(&self) -> Value {
        self.snapshot_tier(SnapshotTier::Detailed)
    }
    /// The historical key set only, for per-domain progress events: no
    /// activity breakdown and no per-slot timing arrays.
    pub fn snapshot_lean(&self) -> Value {
        self.snapshot_tier(SnapshotTier::Lean)
    }
    fn snapshot_tier(&self, tier: SnapshotTier) -> Value {
        let state = self.lock();
        let running = state.slots.iter().filter(|s| s.running).count();
        let mut snapshot = json!({"workers":state.slots.len(), "active_workers":running,
            "occupied_native_slots":state.slots.iter().filter(|s| s.id.is_some()).count(),
            "dispatched_uncommitted_domains":state.slots.iter().filter(|s| s.id.is_some()).count() + state.escrow.len(),
            "finished_uncommitted_domains":state.slots.iter().filter(|s| s.finished.is_some()).count() + state.escrow.len(),
            "backpressured_workers":state.totals.waiting, "backpressure_seconds":state.totals.wait_seconds,
            "attempted_events":self.counters.attempted.load(Ordering::Relaxed),
            "returned_inspections":state.totals.returned, "attempted_native_operations":state.totals.native,
            "attempted_rule_checks":state.totals.rules, "attempted_predicates":state.totals.predicates,
            "attempted_optional_coefficient_refusals":state.totals.optional,
            "native_attempt_counters_scope":"returned_inspections_including_uncommitted_and_cancelled",
            "worker_buffered_events":self.counters.buffered_events.load(Ordering::Relaxed),
            "worker_buffered_logical_bytes":self.counters.buffered_bytes.load(Ordering::Relaxed),
            "peak_worker_buffered_logical_bytes":self.counters.peak_bytes.load(Ordering::Relaxed),
            "per_worker_chunk_events":CHUNK_EVENTS, "per_worker_chunk_records":CHUNK_RECORDS,
            "per_worker_chunk_logical_bytes":CHUNK_BYTES,
            "first_failure":state.failure.as_ref().map(Failure::json),
            "non_cancellation_failure":state.non_cancellation_failure.as_ref().map(Failure::json)});
        if tier >= SnapshotTier::Detailed {
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
            snapshot["computing_workers"] = json!(running - blocked);
            snapshot["finished_awaiting_poll"] =
                json!(state.slots.iter().filter(|s| s.finished.is_some()).count());
            snapshot["heaviest_active_stream"] = json!(heaviest);
            snapshot["stream_stall_share"] = json!(if running == 0 {
                0.0
            } else {
                blocked as f64 / running as f64
            });
            // Nested: counts and seconds, never a flat heartbeat key.
            let sync = &state.sync;
            snapshot["pool_sync"] = json!({
                "scope":"this_pool_session; inspectors_park_on_their_slot_condvar",
                "inspector_wakeups":sync.inspector_wakeups,
                "inspector_wakeups_skipped_not_parked":sync.inspector_wakeups_skipped,
                "shared_condvar_equivalent_wakeups":sync.shared_condvar_wakeups,
                "broadcasts":sync.broadcasts,
                "coordinator_wakeups":sync.coordinator_wakeups,
                "coordinator_wakeups_skipped_not_waiting":sync.coordinator_wakeups_skipped,
                "coordinator_lock":sync.coordinator_lock.json(),
                "inspector_lock":sync.inspector_lock.json()});
        }
        if tier >= SnapshotTier::Full {
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
                self.unbuffer_events(&chunk);
            }
            finished.push((id, entry.finished));
        }
        finished
    }
    fn shutdown(&self) {
        self.stop.store(true, Ordering::Release);
        let mut state = self.lock();
        state.shutdown = true;
        self.broadcast(state);
    }
    fn clear_buffers(&self) {
        let mut state = self.lock();
        for slot in &mut state.slots {
            if let Some(chunk) = slot.chunk.take() {
                self.unbuffer(chunk.count, chunk.weight);
            }
        }
        state
            .escrow
            .clear_chunks(|chunk| self.unbuffer_events(chunk));
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
        let chunk = Chunk {
            events: std::mem::take(&mut self.chunk),
            count: std::mem::take(&mut self.events),
            weight: std::mem::take(&mut self.bytes),
        };
        self.pool.publish(self.slot, chunk)
    }
    fn emit(&mut self, event: Event<N>) -> ControlFlow<()> {
        if self
            .pool
            .counters
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
            .counters
            .buffered_events
            .fetch_add(event.count, Ordering::Relaxed);
        self.events += event.count;
        if let Some(last) = self.chunk.last_mut()
            && last.mergeable(&event)
        {
            last.count += event.count;
        } else {
            let counters = &self.pool.counters;
            counters.buffered_bytes.fetch_add(weight, Ordering::Relaxed);
            self.bytes += weight;
            self.chunk.push(event);
            counters.peak_bytes.fetch_max(
                counters.buffered_bytes.load(Ordering::Relaxed),
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
    /// The unflushed tail's totals are the Emitter's own.
    fn drop(&mut self) {
        self.pool.unbuffer(self.events, self.bytes);
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
