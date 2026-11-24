//! Byte accounting of the two memo families of a p reduction (lane
//! bytebound): the interior kernel memos of the lane programs
//! ([`super::kernel::Memo`]: numerator tables, bubble ratios, carpet weights)
//! and the per-thread step memo ([`super::step_memo`]).
//!
//! Every insertion is charged at the insert, from the capacities and element
//! sizes of what it allocates (no allocator call): `capacity * size_of` per
//! allocation plus [`ALLOC_OVERHEAD`] per allocation, the heap of the
//! multi-precision integers from their significant bits, and the growth of
//! the hash tables from their bucket layout. Every removal is discharged with
//! the bytes that were charged for it, so the counters return to zero when
//! the memos are dropped. The charge is deterministic and independent of the
//! host's allocator (vakint and gammaloop hosts link RustRed with their own).
//!
//! The counters change no value and no output: without a budget nothing is
//! printed unless `RUSTRED_P_CACHE_REPORT=1` (one line at the end of each
//! forward pass).
//!
//! Families:
//! - kernel memos: entries (key, slot, value) in [`KERNEL_BYTES`]; the
//!   numerator tables are counted once, while they live, in [`TABLES_LIVE`]
//!   (a table can be held by a kernel memo, by step memo entries and by a
//!   running step at the same time); the part held by kernel memos is
//!   [`KERNEL_TABLE_BYTES`];
//! - the per-thread step memo (one-thread requests): [`STEP_BYTES`], the sum
//!   over threads; each thread's share is reset to zero with its memo state;
//! - the shared step memo of multi-thread requests: [`SHARED_STEP_BYTES`]
//!   (counted, approximate: its sharded tables' growth is not charged).
//!
//! # The budget (`RUSTRED_P_CACHE_BUDGET_GB`, decimal GB, or [`set_budget`])
//!
//! Unset (the default): today's behaviour, bit for bit (no eviction, no reset,
//! no line, memo generation 0). Set to B bytes it bounds the COVERED bytes
//! (kernel memo entries + live numerator tables + per-thread step memos)
//! DURING a request, at the scheduler's key boundaries ([`key_boundary`]) and
//! phase ends ([`phase_end`]):
//! - an insert that takes the covered total over B (and over the re-arm level
//!   of the last sweep) sets a pressure flag; the next key boundary sweeps:
//!   kernel memo slots, least recently used first (whole use ticks), until the
//!   total is at most 0.8 B; slots whose table is held elsewhere (pinned) are
//!   skipped; pressure re-arms at the total after the sweep plus
//!   max(0.05 B, 64 MB);
//! - the step memos of one-thread requests (one per thread) share B/4
//!   PROCESS-WIDE: at a key boundary where the step memos of all threads
//!   together exceed B/4, the thread drops its own memo if it holds bytes (at
//!   most once per 4096 keys of that thread);
//! - a sweep that leaves the total over B while step memos hold bytes (their
//!   own, or tables they pin) bumps a process-wide reset epoch: every
//!   one-thread scheduler drops its step memo at its next key boundary (no
//!   key window), so tables pinned by ANOTHER thread's step memo become
//!   evictable; every step-memo reset and every phase end re-arms the
//!   pressure at once (re-arm level 0, pressure = total over B);
//! - after more than 4 resets of a thread's memo in a phase, or when B/4 is
//!   under 256 MB (B = 0 included), its steps take the exact path of
//!   `RUSTRED_P_STEP_MEMO=off` for the rest of the phase; the memo is also
//!   dropped at every phase end. A reset starts a new memo generation: values
//!   and light handles of the previous one are refused, never resolved;
//! - a value of more than B/5 is not kept (nothing at B = 0).
//!
//! The bound: with N one-thread requests running at once in one process the
//! covered bytes stay below B + max(0.05 B, 64 MB) + 2 N C, where C is the
//! most one thread charges between two of its key boundaries (`max key
//! inserts` in the report line; one request: N = 1); tables held by a
//! running step or by the shared step memo of a multi-thread request are not
//! evictable and lie outside it.
//! Every evicted entry is a pure function of its key: a later miss computes the
//! same value, so the result terms and the recorded program (operation count)
//! are those of the unbounded run. What the budget does not cover (the shared
//! step memo of multi-thread requests, prepared weights, zero certificates,
//! canon / order caches, recipe and custom caches, programs, routes) is named
//! in the report line printed at each forward end under a budget; when pinned
//! or uncovered state alone exceeds B the line says so and the bound holds
//! only for what can be evicted.
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, Mutex, Weak};

use symbolica::prelude::{Integer, Rational};

use crate::algebra::{Coefficient, CoefficientPolynomial};

/// Bytes charged per heap allocation on top of its requested size: the
/// allocator's bookkeeping and size-class rounding, as an average.
pub const ALLOC_OVERHEAD: u64 = 16;

/// Heap bytes and allocations of a value (what [`Size::charge`] turns into a
/// byte count).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Size {
    pub bytes: u64,
    pub allocations: u64,
}

impl Size {
    pub const ZERO: Self = Self {
        bytes: 0,
        allocations: 0,
    };

    /// One allocation of `bytes` (none for zero bytes).
    pub fn block(bytes: usize) -> Self {
        if bytes == 0 {
            Self::ZERO
        } else {
            Self {
                bytes: bytes as u64,
                allocations: 1,
            }
        }
    }

    /// The heap block of a `Vec<T>` (or a boxed slice) of `capacity`.
    pub fn vec<T>(capacity: usize) -> Self {
        Self::block(capacity.saturating_mul(std::mem::size_of::<T>()))
    }

    /// The table of a `std::collections::HashMap<K, V>` of `capacity`
    /// (hashbrown: buckets of `(K, V)`, one control byte per bucket plus one
    /// group, data padded to the group alignment).
    pub fn map<K, V>(capacity: usize) -> Self {
        if capacity == 0 {
            return Self::ZERO;
        }
        let buckets = if capacity < 4 {
            4
        } else if capacity < 8 {
            8
        } else {
            (capacity * 8 / 7).next_power_of_two()
        };
        const GROUP: usize = 16;
        let data = (buckets * std::mem::size_of::<(K, V)>()).next_multiple_of(GROUP);
        Self::block(data + buckets + GROUP)
    }

    /// The charge in bytes.
    pub fn charge(self) -> u64 {
        self.bytes + self.allocations * ALLOC_OVERHEAD
    }
}

impl std::ops::Add for Size {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self {
            bytes: self.bytes + other.bytes,
            allocations: self.allocations + other.allocations,
        }
    }
}

impl std::ops::AddAssign for Size {
    fn add_assign(&mut self, other: Self) {
        *self = *self + other;
    }
}

impl std::ops::Sub for Size {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Self {
            bytes: self.bytes.wrapping_sub(other.bytes),
            allocations: self.allocations.wrapping_sub(other.allocations),
        }
    }
}

/// Heap of an integer: the limb allocation of a multi-precision value (GMP:
/// its capacity, which can exceed the significant limbs, e.g. for a result
/// built in a reused or over-allocated integer).
pub fn integer_heap(value: &Integer) -> Size {
    match value {
        #[cfg(feature = "native")]
        Integer::Large(large) => Size::block(large.as_raw().capacity().div_ceil(64) * 8),
        #[cfg(not(feature = "native"))]
        Integer::Large(_) => Size::block(value.significant_bits().div_ceil(64) as usize * 8),
        _ => Size::ZERO,
    }
}

/// Heap of a rational number.
pub fn rational_heap(value: &Rational) -> Size {
    integer_heap(value.numerator_ref()) + integer_heap(value.denominator_ref())
}

/// Heap of a polynomial (its context is shared and counts nothing).
pub fn polynomial_heap(polynomial: &CoefficientPolynomial) -> Size {
    let mut size = Size::vec::<Integer>(polynomial.coefficients.capacity())
        + Size::vec::<u16>(polynomial.exponents.capacity());
    for coefficient in &polynomial.coefficients {
        size += integer_heap(coefficient);
    }
    size
}

/// Heap of a coefficient of `Q(d)`.
pub fn coefficient_heap(coefficient: &Coefficient) -> Size {
    polynomial_heap(&coefficient.numerator) + polynomial_heap(&coefficient.denominator)
}

/// Kernel memo entries (keys, slots, values; tables excluded), bytes.
pub(crate) static KERNEL_BYTES: AtomicI64 = AtomicI64::new(0);
/// Kernel memo entries.
pub(crate) static KERNEL_ENTRIES: AtomicI64 = AtomicI64::new(0);
/// Bytes of the numerator tables held by kernel memos (part of [`TABLES_LIVE`]).
pub(crate) static KERNEL_TABLE_BYTES: AtomicI64 = AtomicI64::new(0);
/// Bytes of every numerator table alive in the process.
pub(crate) static TABLES_LIVE: AtomicI64 = AtomicI64::new(0);
/// Per-thread step memos, summed over threads.
pub(crate) static STEP_BYTES: AtomicI64 = AtomicI64::new(0);
/// The shared step memo of multi-thread requests (not covered by a budget).
pub(crate) static SHARED_STEP_BYTES: AtomicI64 = AtomicI64::new(0);
/// Highest covered total seen at a charge.
static PEAK_COVERED: AtomicU64 = AtomicU64::new(0);

/// Kernel memo entries + live tables + per-thread step memos: the bytes a
/// budget covers.
pub(crate) fn covered() -> u64 {
    let total = KERNEL_BYTES.load(Relaxed) + TABLES_LIVE.load(Relaxed) + STEP_BYTES.load(Relaxed);
    total.max(0) as u64
}

/// Account `delta` bytes on `counter` (negative: a discharge).
#[inline]
pub(crate) fn account(counter: &AtomicI64, delta: i64) {
    if delta == 0 {
        return;
    }
    counter.fetch_add(delta, Relaxed);
    if delta > 0 {
        let total = covered();
        PEAK_COVERED.fetch_max(total, Relaxed);
        if let Some(budget) = budget() {
            // This thread's charge since its last key boundary.
            let _ =
                KEY_CHARGE.try_with(|charge| charge.set(charge.get().saturating_add(delta as u64)));
            // Check point at the insert: over the budget and past the
            // re-arm level of the last sweep.
            if total > budget && total > REARM_AT.load(Relaxed) {
                PRESSURE.store(true, Relaxed);
            }
        }
    }
}

thread_local! {
    /// Under a budget: bytes this thread charged since its last key boundary.
    static KEY_CHARGE: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

// ---------------------------------------------------------------------------
// The budget (RUSTRED_P_CACHE_BUDGET_GB).
// ---------------------------------------------------------------------------

const UNINIT: u64 = u64::MAX;
const UNSET: u64 = u64::MAX - 1;
static BUDGET: AtomicU64 = AtomicU64::new(UNINIT);

/// The budget in bytes: `RUSTRED_P_CACHE_BUDGET_GB` (decimal GB = 1e9 B,
/// read once) or [`set_budget`]; `None` (unset): today's behaviour.
#[inline]
pub fn budget() -> Option<u64> {
    #[cfg(test)]
    if let Some(budget) = TEST_BUDGET.with(std::cell::Cell::get) {
        return Some(budget);
    }
    let mut budget = BUDGET.load(Relaxed);
    if budget == UNINIT {
        budget = budget_from_env();
    }
    (budget != UNSET).then_some(budget)
}

#[cold]
fn budget_from_env() -> u64 {
    let value = match std::env::var("RUSTRED_P_CACHE_BUDGET_GB") {
        Ok(text) if !text.trim().is_empty() => match text.trim().parse::<f64>() {
            Ok(gb) if gb.is_finite() && gb >= 0.0 => ((gb * 1e9) as u64).min(UNSET - 1),
            _ => {
                eprintln!(
                    "rustred cache budget: RUSTRED_P_CACHE_BUDGET_GB={text:?} is not a number of GB >= 0: \
                     no budget"
                );
                UNSET
            }
        },
        _ => UNSET,
    };
    let _ = BUDGET.compare_exchange(UNINIT, value, Relaxed, Relaxed);
    BUDGET.load(Relaxed)
}

/// Set the budget in bytes for the process (`None`: no budget), overriding
/// the environment; the API of hosts and tests.
pub fn set_budget(bytes: Option<u64>) {
    BUDGET.store(bytes.map_or(UNSET, |b| b.min(UNSET - 1)), Relaxed);
    REARM_AT.store(0, Relaxed);
    PRESSURE.store(false, Relaxed);
}

/// Whether a budget is set.
#[inline]
pub(crate) fn active() -> bool {
    #[cfg(test)]
    if TEST_ACTIVE.with(std::cell::Cell::get) {
        return true;
    }
    budget().is_some()
}

#[cfg(test)]
thread_local! {
    /// Unit tests: this thread behaves as under a budget for the stamps and
    /// the memo generations (admission and eviction stay off).
    pub(crate) static TEST_ACTIVE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// Unit tests: a budget seen by this thread only (the process budget
    /// stays unset for the other tests of the binary).
    pub(crate) static TEST_BUDGET: std::cell::Cell<Option<u64>> = const { std::cell::Cell::new(None) };
}

/// Tests: advance the use tick; returns the new tick.
#[doc(hidden)]
pub fn advance_tick() -> u64 {
    TICK.fetch_add(1, Relaxed) + 1
}

static SHARE_FLOOR: AtomicU64 = AtomicU64::new(256 << 20);
static WINDOW: AtomicU64 = AtomicU64::new(4096);

/// Tests (small reductions): the step memo's share floor (default 256 MB)
/// and the key window of the rate limit and the tick (default 4096 keys);
/// `None` restores the default.
#[doc(hidden)]
pub fn set_tuning(share_floor: Option<u64>, key_window: Option<u64>) {
    SHARE_FLOOR.store(share_floor.unwrap_or(256 << 20), Relaxed);
    WINDOW.store(key_window.unwrap_or(4096).max(1), Relaxed);
}

/// Tests (small reductions): the floor of the re-arm margin (default 64 MB);
/// `None` restores the default.
#[doc(hidden)]
pub fn set_rearm_floor(floor: Option<u64>) {
    REARM_FLOOR.store(floor.unwrap_or(64 << 20), Relaxed);
}

/// Tests: restart the covered peak and the most one thread charged between
/// two key boundaries from the current total.
#[doc(hidden)]
pub fn reset_peaks() {
    PEAK_COVERED.store(covered(), Relaxed);
    MAX_KEY_CHARGE.store(0, Relaxed);
}

/// Admission: under a budget, a value whose bytes exceed a fifth of it is
/// not kept (at a budget of 0 nothing is kept).
pub(crate) fn admits(bytes: u64) -> bool {
    match budget() {
        None => true,
        Some(budget) => {
            let ok = budget > 0 && bytes <= budget / 5;
            if !ok {
                REFUSED.fetch_add(1, Relaxed);
            }
            ok
        }
    }
}

/// The step memo's share of the budget (one thread's per-thread memo).
fn step_share(budget: u64) -> u64 {
    budget / 4
}

/// Below this share (256 MB, [`set_tuning`]) the step memo is not used under
/// a budget (the exact path of `RUSTRED_P_STEP_MEMO=off`).
fn share_floor() -> u64 {
    SHARE_FLOOR.load(Relaxed)
}
/// Most budget resets of a thread's step memo per phase before it is
/// bypassed for the rest of the phase.
const RESETS_PER_PHASE: u32 = 4;
/// Keys between two budget resets of a thread's step memo, and between two
/// advances of the use tick (4096, [`set_tuning`]).
fn key_window() -> u64 {
    WINDOW.load(Relaxed)
}
/// A sweep evicts down to this fraction of the budget (hysteresis).
const SWEEP_TO: (u64, u64) = (4, 5);
/// After a sweep, pressure re-arms only above the covered total plus
/// `max(budget / 20, REARM_FLOOR)` (64 MB, [`set_rearm_floor`]).
static REARM_FLOOR: AtomicU64 = AtomicU64::new(64 << 20);

/// The use tick: advanced at every phase end and every [`key_window`] keys of
/// a scheduler; slots carry the tick of their last use.
static TICK: AtomicU64 = AtomicU64::new(1);
/// Set at an insert over the budget; served at the next key boundary.
static PRESSURE: AtomicBool = AtomicBool::new(false);
static REARM_AT: AtomicU64 = AtomicU64::new(0);
static EVICTOR: Mutex<()> = Mutex::new(());
/// Bumped by a sweep that leaves the covered total over the budget while
/// step memos hold bytes: every one-thread scheduler drops its step memo at
/// its next key boundary.
static RESET_EPOCH: AtomicU64 = AtomicU64::new(0);
/// The most one thread charged between two of its key boundaries.
static MAX_KEY_CHARGE: AtomicU64 = AtomicU64::new(0);

/// Re-arm the pressure at once (after a step-memo reset, at a phase end and
/// at a forward end): the next insert over the budget, or the next key
/// boundary if the total is over it now, sweeps.
fn rearm(budget: u64) {
    REARM_AT.store(0, Relaxed);
    PRESSURE.store(covered() > budget, Relaxed);
}

#[inline]
pub(crate) fn tick() -> u64 {
    TICK.load(Relaxed)
}

static EVICTED_ENTRIES: AtomicU64 = AtomicU64::new(0);
static EVICTED_BYTES: AtomicU64 = AtomicU64::new(0);
static SWEEPS: AtomicU64 = AtomicU64::new(0);
static SKIPPED_PINNED: AtomicU64 = AtomicU64::new(0);
static STEP_RESETS: AtomicU64 = AtomicU64::new(0);
static EPOCH_RESETS: AtomicU64 = AtomicU64::new(0);
static PHASE_RESETS: AtomicU64 = AtomicU64::new(0);
static BYPASSES: AtomicU64 = AtomicU64::new(0);
static REFUSED: AtomicU64 = AtomicU64::new(0);
static REINSERTIONS: AtomicU64 = AtomicU64::new(0);
static BYPASSED_STEPS: AtomicU64 = AtomicU64::new(0);

/// A miss on a kernel memo that has lost slots to the budget.
pub(crate) fn note_reinsertion() {
    REINSERTIONS.fetch_add(1, Relaxed);
}

/// A kernel memo as a sweep sees it.
pub(crate) trait Sweep: Send + Sync {
    /// `out(stamp, bytes)` for every slot a sweep may evict (bytes freed:
    /// the slot's charge and a table only this memo holds); returns the
    /// slots skipped as pinned.
    fn census(&self, out: &mut dyn FnMut(u64, u64)) -> u64;
    /// Evict the evictable slots of stamp `<= threshold`; returns
    /// (entries, bytes freed).
    fn evict(&self, threshold: u64) -> (u64, u64);
}

static MEMOS: Mutex<Vec<Weak<dyn Sweep>>> = Mutex::new(Vec::new());

/// Register a kernel memo (dead ones are pruned as the registry grows).
pub(crate) fn register(memo: Weak<dyn Sweep>) {
    let mut memos = MEMOS.lock().unwrap_or_else(|e| e.into_inner());
    if memos.len() >= 1024 && memos.len().is_power_of_two() {
        memos.retain(|memo| memo.strong_count() > 0);
    }
    memos.push(memo);
}

/// Evict kernel memo slots, least recently used first (whole ticks), until
/// the covered total is at most 4/5 of `budget` or nothing evictable is
/// left; then re-arm the pressure above the result. Returns the slots skipped
/// as pinned.
fn sweep(budget: u64) -> u64 {
    let target = budget / SWEEP_TO.1 * SWEEP_TO.0;
    let total = covered();
    let mut skipped = 0;
    if total > target {
        let need = total - target;
        let memos: Vec<Arc<dyn Sweep>> = {
            let mut memos = MEMOS.lock().unwrap_or_else(|e| e.into_inner());
            memos.retain(|memo| memo.strong_count() > 0);
            memos.iter().filter_map(Weak::upgrade).collect()
        };
        let mut by_stamp: BTreeMap<u64, u64> = BTreeMap::new();
        let mut pinned = 0;
        for memo in &memos {
            pinned += memo.census(&mut |stamp, bytes| *by_stamp.entry(stamp).or_default() += bytes);
        }
        SKIPPED_PINNED.fetch_add(pinned, Relaxed);
        skipped = pinned;
        let mut freed = 0;
        let mut threshold = None;
        for (stamp, bytes) in by_stamp {
            threshold = Some(stamp);
            freed += bytes;
            if freed >= need {
                break;
            }
        }
        if let Some(threshold) = threshold {
            for memo in &memos {
                let (entries, bytes) = memo.evict(threshold);
                EVICTED_ENTRIES.fetch_add(entries, Relaxed);
                EVICTED_BYTES.fetch_add(bytes, Relaxed);
            }
        }
        SWEEPS.fetch_add(1, Relaxed);
    }
    REARM_AT.store(
        covered().saturating_add((budget / 20).max(REARM_FLOOR.load(Relaxed))),
        Relaxed,
    );
    PRESSURE.store(false, Relaxed);
    skipped
}

/// Per-thread state of the step-memo part of the budget.
struct ThreadBudget {
    keys: u64,
    /// Keys at the last budget reset of this thread's step memo.
    last_reset: Option<u64>,
    resets_in_phase: u32,
    bypass: bool,
    /// The reset epoch this thread has acted on ([`RESET_EPOCH`]).
    epoch: u64,
}

thread_local! {
    static THREAD: RefCell<ThreadBudget> = const {
        RefCell::new(ThreadBudget {
            keys: 0,
            last_reset: None,
            resets_in_phase: 0,
            bypass: false,
            epoch: 0,
        })
    };
}

/// Under a budget, whether this thread's step memo is bypassed (the exact
/// path of `RUSTRED_P_STEP_MEMO=off`): its share is below 256 MB (a budget
/// of 0 included), or it was reset for the budget more than 4 times in this
/// phase. One-thread requests only.
#[inline]
pub(crate) fn step_bypassed() -> bool {
    match budget() {
        None => false,
        Some(budget) => {
            let bypassed = super::step_memo::single_thread()
                && (step_share(budget) < share_floor()
                    || THREAD.with(|thread| thread.borrow().bypass));
            if bypassed {
                BYPASSED_STEPS.fetch_add(1, Relaxed);
            }
            bypassed
        }
    }
}

/// Reset this thread's step memo for the budget; counts towards the
/// per-phase bypass; re-arms the pressure (the tables the memo pinned can go
/// at the next sweep).
fn reset_step_memo(budget: u64, thread: &mut ThreadBudget) {
    super::step_memo::reset_for_budget();
    STEP_RESETS.fetch_add(1, Relaxed);
    thread.last_reset = Some(thread.keys);
    thread.resets_in_phase += 1;
    if thread.resets_in_phase > RESETS_PER_PHASE && !thread.bypass {
        thread.bypass = true;
        BYPASSES.fetch_add(1, Relaxed);
    }
    rearm(budget);
}

/// A scheduler's key boundary (after a key's contributions are consumed,
/// before the next pop): the check point of the budget. Returns whether this
/// thread's step memo was reset (the caller then drops its light-handle
/// cache: the handles name the memo's coefficients). Unset: one load.
#[inline]
pub fn key_boundary(one_thread: bool) -> bool {
    match budget() {
        None => false,
        Some(budget) => key_boundary_budget(budget, one_thread),
    }
}

fn key_boundary_budget(budget: u64, one_thread: bool) -> bool {
    MAX_KEY_CHARGE.fetch_max(KEY_CHARGE.with(|charge| charge.replace(0)), Relaxed);
    THREAD.with(|thread| {
        let mut thread = thread.borrow_mut();
        thread.keys += 1;
        let window = key_window();
        if thread.keys.is_multiple_of(window) {
            TICK.fetch_add(1, Relaxed);
            // Pins a sweep skipped (a running step) may be gone: re-arm
            // once per key window while over the budget (review fr L-a).
            if covered() > budget {
                rearm(budget);
            }
        }
        if thread.keys == 1 {
            // A new thread acts on the epochs bumped from now on.
            thread.epoch = RESET_EPOCH.load(Relaxed);
        }
        let keys = thread.keys;
        let usable = |thread: &ThreadBudget| {
            one_thread && !thread.bypass && step_share(budget) >= share_floor()
        };
        let mut reset = false;
        // The step memos' share is process-wide: over it, this thread drops
        // its own memo if it holds bytes (once per key window).
        if usable(&thread)
            && thread.last_reset.is_none_or(|last| keys - last >= window)
            && STEP_BYTES.load(Relaxed) > step_share(budget) as i64
            && super::step_memo::local_bytes() > 0
        {
            reset_step_memo(budget, &mut thread);
            reset = true;
        }
        if PRESSURE.load(Relaxed)
            && let Ok(_evictor) = EVICTOR.try_lock()
            && PRESSURE.load(Relaxed)
        {
            sweep(budget);
            // Still over the budget while step memos hold bytes (their own,
            // or tables they pin, which the sweep skipped): every one-thread
            // scheduler drops its step memo at its next key boundary (this
            // one below), then the next sweep can evict those tables.
            if covered() > budget && STEP_BYTES.load(Relaxed) > 0 {
                RESET_EPOCH.fetch_add(1, Relaxed);
            }
        }
        let epoch = RESET_EPOCH.load(Relaxed);
        if thread.epoch != epoch {
            thread.epoch = epoch;
            if !reset && usable(&thread) && super::step_memo::local_bytes() > 0 {
                reset_step_memo(budget, &mut thread);
                EPOCH_RESETS.fetch_add(1, Relaxed);
                reset = true;
            }
        }
        reset
    })
}

/// A scheduler's phase end: under a budget, advance the tick, end this
/// thread's per-phase bypass and (one thread) reset its step memo. Returns
/// whether the step memo was reset (the caller drops its light-handle cache).
pub fn phase_end(one_thread: bool) -> bool {
    let Some(budget) = budget() else {
        return false;
    };
    TICK.fetch_add(1, Relaxed);
    THREAD.with(|thread| {
        let mut thread = thread.borrow_mut();
        thread.last_reset = None;
        thread.resets_in_phase = 0;
        thread.bypass = false;
        // The memo goes now: the epochs bumped so far are served.
        thread.epoch = RESET_EPOCH.load(Relaxed);
    });
    if !one_thread {
        rearm(budget);
        return false;
    }
    super::step_memo::reset_for_budget();
    PHASE_RESETS.fetch_add(1, Relaxed);
    rearm(budget);
    true
}

/// The budget's own statistics (counts since the start of the process).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BudgetStats {
    pub budget: Option<u64>,
    pub sweeps: u64,
    pub evicted_entries: u64,
    pub evicted_bytes: u64,
    pub skipped_pinned: u64,
    pub step_resets: u64,
    /// Step-memo resets served from the process-wide reset epoch (part of
    /// `step_resets`).
    pub epoch_resets: u64,
    pub phase_resets: u64,
    pub bypasses: u64,
    pub refused: u64,
    pub reinsertions: u64,
    /// Steps that took the exact path of the memo off (bypassed step memo).
    pub bypassed_steps: u64,
    /// The most one thread charged between two of its key boundaries (C of
    /// the bound), bytes.
    pub max_key_charge: u64,
}

pub fn budget_stats() -> BudgetStats {
    BudgetStats {
        budget: budget(),
        sweeps: SWEEPS.load(Relaxed),
        evicted_entries: EVICTED_ENTRIES.load(Relaxed),
        evicted_bytes: EVICTED_BYTES.load(Relaxed),
        skipped_pinned: SKIPPED_PINNED.load(Relaxed),
        step_resets: STEP_RESETS.load(Relaxed),
        epoch_resets: EPOCH_RESETS.load(Relaxed),
        phase_resets: PHASE_RESETS.load(Relaxed),
        bypasses: BYPASSES.load(Relaxed),
        refused: REFUSED.load(Relaxed),
        reinsertions: REINSERTIONS.load(Relaxed),
        bypassed_steps: BYPASSED_STEPS.load(Relaxed),
        max_key_charge: MAX_KEY_CHARGE.load(Relaxed),
    }
}

/// A snapshot of the counters (bytes unless named otherwise).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CacheStats {
    pub kernel_bytes: i64,
    pub kernel_entries: i64,
    pub kernel_table_bytes: i64,
    pub tables_live: i64,
    pub step_bytes: i64,
    pub shared_step_bytes: i64,
    pub covered: u64,
    pub peak_covered: u64,
}

/// The counters now.
pub fn stats() -> CacheStats {
    CacheStats {
        kernel_bytes: KERNEL_BYTES.load(Relaxed),
        kernel_entries: KERNEL_ENTRIES.load(Relaxed),
        kernel_table_bytes: KERNEL_TABLE_BYTES.load(Relaxed),
        tables_live: TABLES_LIVE.load(Relaxed),
        step_bytes: STEP_BYTES.load(Relaxed),
        shared_step_bytes: SHARED_STEP_BYTES.load(Relaxed),
        covered: covered(),
        peak_covered: PEAK_COVERED.load(Relaxed),
    }
}

/// Bytes of the calling thread's step memo.
pub fn thread_step_bytes() -> u64 {
    super::step_memo::local_bytes()
}

/// `RUSTRED_P_CACHE_REPORT=1`: one report line per forward pass.
pub(crate) fn report_requested() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| {
        matches!(
            std::env::var("RUSTRED_P_CACHE_REPORT").as_deref(),
            Ok("1" | "on" | "true")
        )
    })
}

fn gb(bytes: i64) -> String {
    format!("{:.3}", bytes as f64 / 1e9)
}

/// The report line (GB = 1e9 B).
pub fn report_line() -> String {
    let s = stats();
    let b = budget_stats();
    let pinned = s.tables_live - s.kernel_table_bytes;
    let over = match b.budget {
        Some(budget) if s.covered > budget => format!(
            " | OVER BUDGET by {} GB: pinned tables and step memos of other threads are not evictable",
            gb((s.covered - budget) as i64)
        ),
        _ => String::new(),
    };
    format!(
        "rustred cache budget: budget {} | covered {} GB (peak {}) = kernel memos {} GB ({} entries) + tables {} GB \
         (in kernel memos {}, pinned elsewhere {}) + step memo {} GB | sweeps {} evicted {} entries / {} GB \
         (pinned skipped {}), step memo resets {} (epoch {}, phase ends {}), bypasses {} ({} steps), refused {}, re-insertions {}, \
         max key inserts {} GB | \
         not covered: shared step memo {} GB (counted), prepared weights {} entries; zero certificates, canon / \
         order caches, recipe and custom caches not counted{}",
        b.budget.map_or("unset".to_owned(), |budget| format!(
            "{} GB",
            gb(budget as i64)
        )),
        gb(s.covered as i64),
        gb(s.peak_covered as i64),
        gb(s.kernel_bytes),
        s.kernel_entries,
        gb(s.tables_live),
        gb(s.kernel_table_bytes),
        gb(pinned),
        gb(s.step_bytes),
        b.sweeps,
        b.evicted_entries,
        gb(b.evicted_bytes as i64),
        b.skipped_pinned,
        b.step_resets,
        b.epoch_resets,
        b.phase_resets,
        b.bypasses,
        b.bypassed_steps,
        b.refused,
        b.reinsertions,
        gb(b.max_key_charge as i64),
        gb(s.shared_step_bytes),
        super::insertion::prepared_entries(),
        over,
    )
}

/// The end of a forward pass: the report line under a budget or when asked
/// for (`RUSTRED_P_CACHE_REPORT=1`).
pub(crate) fn forward_finished() {
    if budget().is_some() || report_requested() {
        eprintln!("{}", report_line());
    }
    // The re-arm level of this request's last sweep is not the next one's.
    if let Some(budget) = budget() {
        rearm(budget);
    }
}

/// Tests: a global allocator that counts, per thread, the net bytes and the
/// net number of allocations, so the charges can be compared with what was
/// really allocated.
#[cfg(test)]
pub(crate) mod alloc_count {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    pub(crate) struct Counting;

    thread_local! {
        static NET: Cell<(i64, i64)> = const { Cell::new((0, 0)) };
    }

    fn note(bytes: i64, allocations: i64) {
        let _ = NET.try_with(|net| {
            let (b, a) = net.get();
            net.set((b + bytes, a + allocations));
        });
    }

    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            note(layout.size() as i64, 1);
            unsafe { System.alloc(layout) }
        }
        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            note(layout.size() as i64, 1);
            unsafe { System.alloc_zeroed(layout) }
        }
        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            note(-(layout.size() as i64), -1);
            unsafe { System.dealloc(ptr, layout) }
        }
        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            note(new_size as i64 - layout.size() as i64, 0);
            unsafe { System.realloc(ptr, layout, new_size) }
        }
    }

    #[global_allocator]
    static COUNTING: Counting = Counting;

    /// Net (bytes, allocations) of this thread so far.
    pub(crate) fn net() -> (i64, i64) {
        NET.with(Cell::get)
    }

    /// The charge of a net (bytes, allocations) difference.
    pub(crate) fn charge(before: (i64, i64), after: (i64, i64)) -> i64 {
        (after.0 - before.0) + (after.1 - before.1) * super::ALLOC_OVERHEAD as i64
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::alloc_count::{charge, net};
    use super::*;
    use crate::algebra::{CoefficientContext, ExactAlgebraLimits};
    use crate::propagator::kernel::{Memo, NumeratorTable, read_memo, write_memo};

    /// A small deterministic generator (xorshift).
    pub(crate) struct Rng(pub u64);
    impl Rng {
        pub(crate) fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
    }

    /// A coefficient of `Q(d)` with `terms` terms and integers of about
    /// `limbs` extra words (multi-precision from 2 on).
    pub(crate) fn coefficient(
        context: &CoefficientContext,
        rng: &mut Rng,
        terms: u64,
        limbs: u32,
    ) -> Coefficient {
        let limits = ExactAlgebraLimits::default();
        let d = context.parameter("d").unwrap();
        let mut numerator = context.integer(1);
        for term in 0..terms {
            let mut c = context.integer((rng.next() % 1000) as i64 + 1);
            for _ in 0..limbs {
                c = context
                    .try_mul(
                        &c,
                        &context.integer(i64::MAX - (rng.next() % 1000) as i64),
                        limits,
                    )
                    .unwrap();
            }
            let mut monomial = c;
            for _ in 0..term {
                monomial = context.try_mul(&monomial, &d, limits).unwrap();
            }
            numerator = context.try_add(&numerator, &monomial, limits).unwrap();
        }
        let denominator = context
            .try_add(&d, &context.integer((rng.next() % 7) as i64 + 3), limits)
            .unwrap();
        context.try_div(&numerator, &denominator, limits).unwrap()
    }

    fn memo_charged<K, V: crate::propagator::kernel::MemoValue>(memo: &Memo<K, V>) -> i64 {
        memo.charged() as i64
    }

    /// The charge of a kernel memo equals what its insertions allocated, after
    /// random insertions, replacements (racing builds) and growth; dropping it
    /// frees exactly what was charged.
    #[test]
    fn kernel_memo_charges_match_the_allocator() {
        let context = CoefficientContext::try_new(["d"]).unwrap();
        let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
        let values: Vec<Coefficient> = (0..64)
            .map(|i| coefficient(&context, &mut rng, 1 + i % 6, (i % 2) as u32))
            .collect();
        let memo: Memo<(i64, i64, i64, i64, i64, i64), Coefficient> = Memo::default();
        let start = net();
        for round in 0..400_i64 {
            let key = ((rng.next() % 300) as i64, round % 3, 0, 0, 0, 0);
            let value = &values[(rng.next() % 64) as usize];
            write_memo(&memo, key, value, usize::MAX).unwrap();
            assert_eq!(
                charge(start, net()),
                memo_charged(&memo),
                "after insertion {round}"
            );
            if round % 37 == 0 {
                assert!(read_memo(&memo, &key).unwrap().is_some());
            }
        }
        let charged = memo_charged(&memo);
        assert!(charged > 0);
        let charged = memo_charged(&memo);
        let before = net();
        drop(memo);
        assert_eq!(
            charge(before, net()),
            -charged,
            "the memo frees what it charged"
        );
    }

    /// Table memos: keys are boxed; the table bytes are counted once, in
    /// TABLES_LIVE, while the table lives, whoever holds it.
    #[test]
    fn table_bytes_match_the_allocator() {
        let context = CoefficientContext::try_new(["d"]).unwrap();
        let mut rng = Rng(7);
        let _ = coefficient(&context, &mut rng, 1, 0);
        for rows in [0usize, 1, 3, 17, 200] {
            let start = net();
            let table = Arc::new(NumeratorTable::for_test(
                (0..rows)
                    .map(|row| {
                        let orders: Vec<(u32, Rational)> = (0..(row % 5) as u32)
                            .map(|order| {
                                let mut value = Rational::from(rng.next() as i64 / 3 + 1);
                                for _ in 0..row % 2 {
                                    value = &value * &Rational::from(i64::MAX - 5);
                                }
                                (order, value)
                            })
                            .collect();
                        (vec![row as u32; row % 9], orders)
                    })
                    .collect(),
            ));
            // The temporaries of the test rows are freed: what is left is the table.
            assert_eq!(charge(start, net()), table.bytes() as i64, "{rows} rows");
            let memo: Memo<Box<[u32]>, Arc<NumeratorTable>> = Memo::default();
            let before = net();
            write_memo(
                &memo,
                vec![1, 2, rows as u32].into_boxed_slice(),
                &table,
                usize::MAX,
            )
            .unwrap();
            assert_eq!(charge(before, net()), memo_charged(&memo));
            let charged = memo_charged(&memo);
            let before = net();
            drop(memo);
            assert_eq!(charge(before, net()), -charged);
            let bytes = table.bytes() as i64;
            let before = net();
            drop(table);
            assert_eq!(charge(before, net()), -bytes);
        }
    }

    /// Tests that advance the global tick run one at a time.
    fn tick_serial() -> std::sync::MutexGuard<'static, ()> {
        static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
        SERIAL
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Random insertions, reads, replacements and evictions (random
    /// thresholds): after every operation the charge equals the allocator's
    /// count and a deep-size walk; the memo frees what it charged.
    #[test]
    fn kernel_memo_charges_survive_evictions() {
        let _serial = tick_serial();
        TEST_ACTIVE.with(|active| active.set(true));
        let context = CoefficientContext::try_new(["d"]).unwrap();
        let mut rng = Rng(0x1234_5678_9abc_def1);
        let values: Vec<Coefficient> = (0..48)
            .map(|i| coefficient(&context, &mut rng, 1 + i % 5, (i % 2) as u32))
            .collect();
        let memo: Memo<(i64, i64, i64, i64, i64, i64), Coefficient> = Memo::default();
        let start = net();
        let mut evicted = 0;
        for round in 0..3000_i64 {
            let key = ((rng.next() % 500) as i64, 1, 2, 3, 4, 5);
            let op = rng.next() % 10;
            let len = memo.len();
            match op {
                0..=5 => {
                    write_memo(&memo, key, &values[(rng.next() % 48) as usize], usize::MAX).unwrap()
                }
                6..=7 => {
                    let _ = read_memo(&memo, &key).unwrap();
                }
                8 => {
                    advance_tick();
                }
                _ => {
                    let threshold = tick().saturating_sub(rng.next() % 4);
                    evicted += memo.evict_for_test(threshold).0;
                }
            }
            assert_eq!(
                charge(start, net()),
                memo_charged(&memo),
                "round {round} op {op} len {len} -> {}",
                memo.len()
            );
            assert_eq!(memo.walk(), memo.charged(), "round {round}");
        }
        assert!(evicted > 100, "evictions happened ({evicted})");
        let charged = memo_charged(&memo);
        let before = net();
        drop(memo);
        assert_eq!(charge(before, net()), -charged);
        TEST_ACTIVE.with(|active| active.set(false));
    }

    /// A table held elsewhere (a step memo entry, a running step) is pinned:
    /// a sweep skips its slot; once released, the next sweep evicts it and
    /// frees the table.
    #[test]
    fn pinned_tables_are_skipped_then_evicted() {
        let table = |rows: u32| {
            Arc::new(NumeratorTable::for_test(
                (0..rows)
                    .map(|r| (vec![r; 4], vec![(0, Rational::from(i64::from(r) + 3))]))
                    .collect(),
            ))
        };
        let memo: Memo<Box<[u32]>, Arc<NumeratorTable>> = Memo::default();
        let start = net();
        let kept = table(9);
        write_memo(&memo, vec![1].into_boxed_slice(), &kept, usize::MAX).unwrap();
        write_memo(&memo, vec![2].into_boxed_slice(), &table(5), usize::MAX).unwrap();
        let (entries, bytes) = memo.evict_for_test(u64::MAX);
        assert_eq!(entries, 1, "the unpinned slot only");
        assert!(bytes > 0);
        assert_eq!(memo.len(), 1);
        drop(kept);
        let (entries, _) = memo.evict_for_test(u64::MAX);
        assert_eq!(entries, 1, "released: evicted");
        assert_eq!(memo.len(), 0);
        drop(memo);
        assert_eq!(charge(start, net()), 0, "tables freed");
    }

    /// The eviction order is the order of last use (whole ticks), the same
    /// for the same history.
    #[test]
    fn eviction_follows_the_last_use_and_is_deterministic() {
        let _serial = tick_serial();
        TEST_ACTIVE.with(|active| active.set(true));
        let context = CoefficientContext::try_new(["d"]).unwrap();
        let run = || {
            let memo: Memo<(i64, i64, i64, i64, i64, i64), Coefficient> = Memo::default();
            let base = advance_tick();
            for k in 0..64_i64 {
                write_memo(
                    &memo,
                    (k, 0, 0, 0, 0, 0),
                    &context.integer(k + 1),
                    usize::MAX,
                )
                .unwrap();
                if k % 8 == 7 {
                    advance_tick();
                }
            }
            // Keys 0, 9, 18, ... are used again, at the last tick.
            advance_tick();
            for k in (0..64_i64).step_by(9) {
                assert!(read_memo(&memo, &(k, 0, 0, 0, 0, 0)).unwrap().is_some());
            }
            // Evict the first four ticks of insertions.
            let (entries, _) = memo.evict_for_test(base + 3);
            let kept: Vec<i64> = (0..64_i64)
                .filter(|k| read_memo(&memo, &(*k, 0, 0, 0, 0, 0)).unwrap().is_some())
                .collect();
            (entries, kept)
        };
        let (entries, kept) = run();
        let expected: Vec<i64> = (0..64_i64).filter(|k| *k >= 32 || k % 9 == 0).collect();
        assert_eq!(kept, expected);
        assert_eq!(entries, 64 - expected.len() as u64);
        assert_eq!(run(), (entries, kept));
        TEST_ACTIVE.with(|active| active.set(false));
    }

    /// Two threads build the same keys (replacements) and one evicts while
    /// the other reads: the charge stays equal to a deep-size walk.
    #[test]
    fn concurrent_builds_and_evictions_keep_the_charge() {
        let context = CoefficientContext::try_new(["d"]).unwrap();
        let mut rng = Rng(99);
        let values: Vec<Coefficient> = (0..16)
            .map(|i| coefficient(&context, &mut rng, 1 + i % 4, 1))
            .collect();
        let memo: Memo<(i64, i64, i64, i64, i64, i64), Coefficient> = Memo::default();
        std::thread::scope(|scope| {
            for t in 0..4_u64 {
                let (memo, values) = (&memo, &values);
                scope.spawn(move || {
                    let mut rng = Rng(t + 1);
                    for _ in 0..4000 {
                        let key = ((rng.next() % 64) as i64, 0, 0, 0, 0, 0);
                        if t == 3 && rng.next() % 50 == 0 {
                            memo.evict_for_test(u64::MAX);
                        } else if read_memo(memo, &key).unwrap().is_none() {
                            write_memo(memo, key, &values[key.0 as usize % 16], usize::MAX)
                                .unwrap();
                        }
                    }
                });
            }
        });
        assert_eq!(memo.walk(), memo.charged());
    }

    /// Multi-precision integers are charged their limbs (GMP allocates them
    /// outside the Rust allocator, so the allocator tests above use integers
    /// of at most two words).
    #[test]
    fn large_integers_are_charged_their_limbs() {
        let mut value = Integer::from(i64::MAX);
        for _ in 0..5 {
            value = &value * &Integer::from(i64::MAX);
        }
        let Integer::Large(large) = &value else {
            panic!("a multi-precision value");
        };
        let bits = value.significant_bits();
        let limbs = large.as_raw().capacity().div_ceil(64);
        assert!(limbs as u64 >= bits.div_ceil(64));
        assert_eq!(integer_heap(&value), Size::block(limbs * 8));
        // An over-allocated value (a result built in a larger integer) is
        // charged its allocation, not its significant limbs (rv1 L3).
        let mut raw = large.as_raw().clone();
        raw.reserve(64 * 64);
        let spacious = Integer::Large(raw.into());
        assert_eq!(spacious, value);
        assert!(integer_heap(&spacious).bytes >= (limbs as u64 + 64) * 8);
        assert_eq!(integer_heap(&Integer::from(5)), Size::ZERO);
    }

    /// Restores the process tuning and this thread's test budget on drop.
    struct TestBudget;
    impl TestBudget {
        fn set(budget: u64) -> Self {
            TEST_BUDGET.with(|b| b.set(Some(budget)));
            TEST_ACTIVE.with(|active| active.set(true));
            Self
        }
    }
    impl Drop for TestBudget {
        fn drop(&mut self) {
            TEST_BUDGET.with(|b| b.set(None));
            TEST_ACTIVE.with(|active| active.set(false));
        }
    }

    /// The policy itself (review rv1 L1): a thread under a (thread-local)
    /// budget inserts into a kernel memo and passes key boundaries; the
    /// boundaries sweep, the memo loses slots and its bytes stay within the
    /// budget plus the re-arm margin plus one key's insert. Other tests of
    /// the binary only add to the process total, which makes sweeps earlier,
    /// so the bound on this memo's own bytes holds whatever runs beside it.
    #[test]
    fn key_boundaries_sweep_and_bound_a_kernel_memo() {
        let _serial = tick_serial();
        const B: u64 = 400_000;
        set_rearm_floor(Some(0));
        REARM_AT.store(0, Relaxed);
        let _budget = TestBudget::set(B);
        let context = CoefficientContext::try_new(["d"]).unwrap();
        let mut rng = Rng(0x51ed_270b_a5f0_1c2d);
        let values: Vec<Coefficient> = (0..32)
            .map(|i| coefficient(&context, &mut rng, 4 + i % 6, 3))
            .collect();
        let memo: Memo<(i64, i64, i64, i64, i64, i64), Coefficient> = Memo::default();
        let (sweeps, evicted) = (SWEEPS.load(Relaxed), EVICTED_ENTRIES.load(Relaxed));
        let mut most_per_key = 0;
        let mut most = 0;
        for key in 0..3000_i64 {
            let before = memo.charged();
            write_memo(
                &memo,
                (key, 0, 0, 0, 0, 0),
                &values[(rng.next() % 32) as usize],
                usize::MAX,
            )
            .unwrap();
            most_per_key = most_per_key.max(memo.charged().saturating_sub(before));
            most = most.max(memo.charged());
            assert!(!key_boundary(true), "no step memo on this thread");
        }
        let margin = B / 20;
        println!(
            "memo peak {most} B, budget {B}, margin {margin}, one key {most_per_key}, entries {}",
            memo.len()
        );
        assert!(
            most <= B + margin + most_per_key,
            "{most} > {B} + {margin} + {most_per_key}"
        );
        assert!(SWEEPS.load(Relaxed) > sweeps);
        assert!(EVICTED_ENTRIES.load(Relaxed) > evicted);
        assert!(memo.len() < 3000, "the memo lost slots");
        set_rearm_floor(None);
    }

    /// rv1 F1: a table pinned by ANOTHER thread's step memo. The sweeping
    /// thread cannot evict it and bumps the reset epoch; the pinning thread
    /// drops its step memo at its next key boundary (its own share is far
    /// below B/4: only the epoch resets it); the next sweep evicts the table.
    #[test]
    fn a_table_pinned_by_another_threads_step_memo_goes_after_the_epoch() {
        use std::sync::mpsc::channel;

        use crate::propagator::step_memo;
        let _serial = tick_serial();
        // Eight tables of about 1.3 MB pinned: over the 8 MB budget alone.
        const B: u64 = 8_000_000;
        set_rearm_floor(Some(0));
        set_tuning(Some(0), Some(1));
        let (to_pinner, pinner_rx) = channel::<Option<Vec<Arc<NumeratorTable>>>>();
        let (to_sweeper, sweeper_rx) = channel::<u64>();
        let pinner = std::thread::spawn(move || {
            let _budget = TestBudget::set(B);
            let counted = step_memo::begin(1);
            // The first boundary of a thread syncs its epoch.
            assert!(!key_boundary(true));
            to_sweeper.send(0).unwrap();
            let tables = pinner_rx.recv().unwrap().unwrap();
            let limits = crate::algebra::ExactAlgebraLimits::default();
            let owner = step_memo::next_uid() as i64;
            for (t, table) in tables.into_iter().enumerate() {
                let held =
                    step_memo::insertion(&[owner, t as i64], limits, || Ok((table, vec![None; 4])))
                        .unwrap()
                        .unwrap();
                drop(held);
            }
            to_sweeper.send(step_memo::local_bytes()).unwrap();
            // Wait for the sweep, then pass a key boundary.
            pinner_rx.recv().unwrap();
            let resets = EPOCH_RESETS.load(Relaxed);
            let reset = key_boundary(true);
            // The epoch, unless step memos of other tests took the process
            // over B/4 meanwhile (then the share reset came first).
            println!(
                "pinner reset: epoch {}, step bytes {}",
                EPOCH_RESETS.load(Relaxed) - resets,
                STEP_BYTES.load(Relaxed)
            );
            to_sweeper.send(step_memo::local_bytes()).unwrap();
            assert!(reset, "the epoch reset this thread's step memo");
            step_memo::end(counted);
        });
        assert_eq!(sweeper_rx.recv().unwrap(), 0);
        let _budget = TestBudget::set(B);
        REARM_AT.store(0, Relaxed);
        let memo: Memo<Box<[u32]>, Arc<NumeratorTable>> = Memo::default();
        let tables: Vec<Arc<NumeratorTable>> = (0..8_u32)
            .map(|t| {
                Arc::new(NumeratorTable::for_test(
                    (0..7_500_u32)
                        .map(|r| {
                            (
                                vec![r, t, 1, 2],
                                vec![(0, Rational::from(i64::from(r) + 3))],
                            )
                        })
                        .collect(),
                ))
            })
            .collect();
        assert!(tables[0].bytes() < B / 5, "admitted: {}", tables[0].bytes());
        for (t, table) in tables.iter().enumerate() {
            write_memo(&memo, vec![t as u32].into_boxed_slice(), table, usize::MAX).unwrap();
        }
        assert_eq!(memo.len(), 8);
        to_pinner.send(Some(tables.clone())).unwrap();
        drop(tables);
        let pinned = sweeper_rx.recv().unwrap();
        assert!(
            pinned > 0 && pinned < B / 4,
            "the pinner's own share: {pinned} B"
        );
        // The tables now live in the memo and in the pinner's step memo only.
        let epoch = RESET_EPOCH.load(Relaxed);
        assert!(
            PRESSURE.load(Relaxed),
            "the inserts over B set the pressure"
        );
        assert!(!key_boundary(true));
        assert_eq!(memo.len(), 8, "pinned: skipped");
        assert!(
            RESET_EPOCH.load(Relaxed) > epoch,
            "the sweep bumped the reset epoch"
        );
        to_pinner.send(None).unwrap();
        assert_eq!(
            sweeper_rx.recv().unwrap(),
            0,
            "the pinner dropped its step memo"
        );
        pinner.join().unwrap();
        // The reset re-armed the pressure (rv1 F2): the next boundary evicts
        // them although the last sweep's re-arm level was above the total.
        assert!(PRESSURE.load(Relaxed), "re-armed by the reset");
        assert!(!key_boundary(true));
        assert_eq!(memo.len(), 0, "released: evicted");
        set_rearm_floor(None);
        set_tuning(None, None);
    }

    /// Review fr L-a (F2 with pins that are not a step memo): a running step
    /// pins every table of a kernel memo, so a sweep leaves the total over B
    /// and re-arms above it; then the pins go. Within one key window the key
    /// boundaries re-arm the pressure and the next sweep evicts the released
    /// slots, without waiting for a phase end, a step-memo reset or the
    /// forward end. (Without the key-window re-arm all 5000 boundaries stay
    /// over B with nothing pinned.)
    #[test]
    fn fr_stale_rearm_after_non_step_memo_pins_go() {
        let _serial = tick_serial();
        const B: u64 = 8_000_000;
        set_rearm_floor(Some(0));
        REARM_AT.store(0, Relaxed);
        PRESSURE.store(false, Relaxed);
        let _budget = TestBudget::set(B);
        let memo: Memo<Box<[u32]>, Arc<NumeratorTable>> = Memo::default();
        let tables: Vec<Arc<NumeratorTable>> = (0..8_u32)
            .map(|t| {
                Arc::new(NumeratorTable::for_test(
                    (0..7_500_u32)
                        .map(|r| (vec![r, t, 1, 2], vec![(0, Rational::from(i64::from(r) + 3))]))
                        .collect(),
                ))
            })
            .collect();
        for (t, table) in tables.iter().enumerate() {
            write_memo(&memo, vec![t as u32].into_boxed_slice(), table, usize::MAX).unwrap();
        }
        println!("covered {} B, B {B}, step bytes {}", covered(), STEP_BYTES.load(Relaxed));
        assert!(PRESSURE.load(Relaxed));
        let sweeps = SWEEPS.load(Relaxed);
        assert!(!key_boundary(true));
        assert!(SWEEPS.load(Relaxed) > sweeps, "swept");
        assert_eq!(memo.len(), 8, "all pinned by the test (a running step)");
        // The pins go.
        drop(tables);
        let mut stale_boundaries = 0;
        for _ in 0..5000 {
            key_boundary(true);
            if memo.len() == 8 && covered() > B {
                stale_boundaries += 1;
            }
        }
        println!(
            "pins released: {stale_boundaries} of 5000 key boundaries over B with nothing pinned; {} entries, covered {}",
            memo.len(),
            covered()
        );
        set_rearm_floor(None);
        assert!(stale_boundaries < 5000, "re-armed within one key window");
        assert!(memo.len() < 8, "the released slots were evicted");
    }

    /// `Size::map` follows the table layout of `std::collections::HashMap`.
    #[test]
    fn map_sizes_match_the_allocator() {
        let mut map: std::collections::HashMap<u64, [u8; 24]> = std::collections::HashMap::new();
        let start = net();
        for i in 0..5000_u64 {
            map.insert(i, [0; 24]);
            assert_eq!(
                charge(start, net()),
                Size::map::<u64, [u8; 24]>(map.capacity()).charge() as i64,
                "at {i} entries"
            );
        }
    }
}
