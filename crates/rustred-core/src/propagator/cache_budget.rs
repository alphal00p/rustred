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
//! - a one-thread request's step memo is dropped whole when it exceeds its
//!   share B/4, or when a sweep leaves the total over B with tables pinned, at
//!   most once per 4096 keys; after more than 4 such resets in a phase, or
//!   when B/4 is under 256 MB (B = 0 included), its steps take the exact path
//!   of `RUSTRED_P_STEP_MEMO=off` for the rest of the phase; it is also
//!   dropped at every phase end. A reset starts a new memo generation: values
//!   and light handles of the previous one are refused, never resolved;
//! - a value of more than B/5 is not kept (nothing at B = 0).
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

/// Heap of an integer: the limbs of a multi-precision value.
pub fn integer_heap(value: &Integer) -> Size {
    match value {
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
        // Check point at the insert: over the budget and past the re-arm
        // level of the last sweep.
        if let Some(budget) = budget()
            && total > budget
            && total > REARM_AT.load(Relaxed)
        {
            PRESSURE.store(true, Relaxed);
        }
    }
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
/// `max(budget / 20, REARM_FLOOR)`.
const REARM_FLOOR: u64 = 64 << 20;

/// The use tick: advanced at every phase end and every [`key_window`] keys of
/// a scheduler; slots carry the tick of their last use.
static TICK: AtomicU64 = AtomicU64::new(1);
/// Set at an insert over the budget; served at the next key boundary.
static PRESSURE: AtomicBool = AtomicBool::new(false);
static REARM_AT: AtomicU64 = AtomicU64::new(0);
static EVICTOR: Mutex<()> = Mutex::new(());

#[inline]
pub(crate) fn tick() -> u64 {
    TICK.load(Relaxed)
}

static EVICTED_ENTRIES: AtomicU64 = AtomicU64::new(0);
static EVICTED_BYTES: AtomicU64 = AtomicU64::new(0);
static SWEEPS: AtomicU64 = AtomicU64::new(0);
static SKIPPED_PINNED: AtomicU64 = AtomicU64::new(0);
static STEP_RESETS: AtomicU64 = AtomicU64::new(0);
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
        covered().saturating_add((budget / 20).max(REARM_FLOOR)),
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
}

thread_local! {
    static THREAD: RefCell<ThreadBudget> = const {
        RefCell::new(ThreadBudget {
            keys: 0,
            last_reset: None,
            resets_in_phase: 0,
            bypass: false,
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

/// Reset this thread's step memo for the budget (rate limited by the
/// caller); counts towards the per-phase bypass.
fn reset_step_memo(thread: &mut ThreadBudget) {
    super::step_memo::reset_for_budget();
    STEP_RESETS.fetch_add(1, Relaxed);
    thread.last_reset = Some(thread.keys);
    thread.resets_in_phase += 1;
    if thread.resets_in_phase > RESETS_PER_PHASE && !thread.bypass {
        thread.bypass = true;
        BYPASSES.fetch_add(1, Relaxed);
    }
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
    THREAD.with(|thread| {
        let mut thread = thread.borrow_mut();
        thread.keys += 1;
        let window = key_window();
        if thread.keys.is_multiple_of(window) {
            TICK.fetch_add(1, Relaxed);
        }
        let keys = thread.keys;
        let allowed = |thread: &ThreadBudget| {
            one_thread
                && !thread.bypass
                && step_share(budget) >= share_floor()
                && thread.last_reset.is_none_or(|last| keys - last >= window)
        };
        let mut reset = false;
        if allowed(&thread) && super::step_memo::local_bytes() > step_share(budget) {
            reset_step_memo(&mut thread);
            reset = true;
        }
        if PRESSURE.load(Relaxed)
            && let Ok(_evictor) = EVICTOR.try_lock()
            && PRESSURE.load(Relaxed)
        {
            let pinned = sweep(budget);
            // Still over the budget after a sweep that skipped slots pinned by
            // step memo entries (or by a running step): the step memo goes too
            // (same rate limit), its tables become evictable at the next sweep.
            if !reset
                && pinned > 0
                && covered() > budget
                && super::step_memo::local_bytes() > 0
                && allowed(&thread)
            {
                reset_step_memo(&mut thread);
                reset = true;
                // The released tables can go at the next key boundary.
                REARM_AT.store(0, Relaxed);
                PRESSURE.store(covered() > budget, Relaxed);
            }
        }
        reset
    })
}

/// A scheduler's phase end: under a budget, advance the tick, end this
/// thread's per-phase bypass and (one thread) reset its step memo. Returns
/// whether the step memo was reset (the caller drops its light-handle cache).
pub fn phase_end(one_thread: bool) -> bool {
    if budget().is_none() {
        return false;
    }
    TICK.fetch_add(1, Relaxed);
    THREAD.with(|thread| {
        let mut thread = thread.borrow_mut();
        thread.last_reset = None;
        thread.resets_in_phase = 0;
        thread.bypass = false;
    });
    if !one_thread {
        return false;
    }
    super::step_memo::reset_for_budget();
    PHASE_RESETS.fetch_add(1, Relaxed);
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
    pub phase_resets: u64,
    pub bypasses: u64,
    pub refused: u64,
    pub reinsertions: u64,
    /// Steps that took the exact path of the memo off (bypassed step memo).
    pub bypassed_steps: u64,
}

pub fn budget_stats() -> BudgetStats {
    BudgetStats {
        budget: budget(),
        sweeps: SWEEPS.load(Relaxed),
        evicted_entries: EVICTED_ENTRIES.load(Relaxed),
        evicted_bytes: EVICTED_BYTES.load(Relaxed),
        skipped_pinned: SKIPPED_PINNED.load(Relaxed),
        step_resets: STEP_RESETS.load(Relaxed),
        phase_resets: PHASE_RESETS.load(Relaxed),
        bypasses: BYPASSES.load(Relaxed),
        refused: REFUSED.load(Relaxed),
        reinsertions: REINSERTIONS.load(Relaxed),
        bypassed_steps: BYPASSED_STEPS.load(Relaxed),
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
         (pinned skipped {}), step memo resets {} (phase ends {}), bypasses {} ({} steps), refused {}, re-insertions {} | \
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
        b.phase_resets,
        b.bypasses,
        b.bypassed_steps,
        b.refused,
        b.reinsertions,
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
        assert!(matches!(value, Integer::Large(_)));
        let bits = value.significant_bits();
        assert_eq!(
            integer_heap(&value),
            Size::block(bits.div_ceil(64) as usize * 8)
        );
        assert_eq!(integer_heap(&Integer::from(5)), Size::ZERO);
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
