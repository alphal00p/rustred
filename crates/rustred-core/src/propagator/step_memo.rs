//! rr-alloc: per-thread memos of exact step coefficients (switch
//! `RUSTRED_P_STEP_MEMO`, default off).
//!
//! A p step builds its child coefficients in `Q(d)` with exact Symbolica
//! arithmetic on every application, although the coefficients are functions of
//! a few indices only and repeat across keys (the forward pass of NO3 d2_r8
//! interns about 42 K distinct coefficients for 30 M edges). The memos below
//! keep, per worker thread, the coefficients of
//!
//! - an Insertion step, keyed by the two bubble line powers and the numerator
//!   powers (box and free ISPs): the child weights `Sum_s v_s W_s` of every
//!   monomial of the numerator table, signs applied;
//! - a Route step on a routed sector, keyed by the presentation, the sector
//!   and the non-positive powers of the key: the expansion of the numerator
//!   factors relative to the layered base.
//!
//! A hit costs one hash lookup on the stack key plus one clone per child
//! coefficient; no polynomial arithmetic, and no traffic on a lock shared
//! between workers. Every value is the one the step computes without the memo
//! (the same function of the same inputs); `RUSTRED_P_STEP_MEMO=check` computes
//! both and fails the step on any difference.
//!
//! The memos are thread-local, keyed by a process-unique id of their owner (an
//! Insertion step or a p engine, never an address), and dropped whole when the
//! exact-algebra limits of the calling request change, when a memo exceeds
//! [`MEMO_ENTRIES`] entries, and at the end of every request on the calling
//! thread ([`clear`]; the scheduler's worker threads end with the request).
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use smallvec::SmallVec;

use crate::algebra::{Coefficient, CoefficientContext, ExactAlgebraLimits};

use super::error::PError;

/// Entries of one thread's memo beyond which it is cleared.
pub(crate) const MEMO_ENTRIES: usize = 1 << 20;

/// What `RUSTRED_P_STEP_MEMO` asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MemoMode {
    Off,
    On,
    /// Memo and direct computation, compared on every step.
    Check,
}

/// The memo mode of every p request of this process: ON by default (M5
/// batch 3, coordinator 2026-11-11 14:16Z: -23..-35 % one-thread reduce on
/// every r8 cell, byte-identical); `RUSTRED_P_STEP_MEMO=0|off|false`
/// disables it, `=check` compares memo and direct computation on every step.
pub(crate) fn mode() -> MemoMode {
    static MODE: OnceLock<MemoMode> = OnceLock::new();
    *MODE.get_or_init(|| match std::env::var("RUSTRED_P_STEP_MEMO").as_deref() {
        Ok("0" | "off" | "false") => MemoMode::Off,
        Ok("check") => MemoMode::Check,
        _ => MemoMode::On,
    })
}

/// A process-unique id of a memo owner.
pub(crate) fn next_uid() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// The key of a memo entry, built on the stack for every p key of the 4L
/// and 5L libraries (owner id, a few tags, at most ~20 powers); stored boxed.
pub(crate) type MemoKey = SmallVec<[i64; 32]>;

/// Child weights of an Insertion step: one per monomial of its numerator
/// table, `None` where the weight is zero, else an index into this thread's
/// coefficient table ([`coefficients`]).
pub(crate) struct InsertionWeights {
    pub table: std::sync::Arc<super::kernel::NumeratorTable>,
    pub weights: Vec<Option<u32>>,
}

/// Endpoints of a routed numerator expansion: target powers relative to the
/// layered base (`arity` entries per endpoint, `i8`), with the index of their
/// coefficient in this thread's coefficient table.
pub(crate) struct RouteExpansion {
    pub arity: usize,
    pub offsets: Vec<i8>,
    pub coefficients: Vec<u32>,
}

/// A memo value: this thread's (`Rc`) or the shared memo's (`Arc`).
pub(crate) enum Held<V> {
    Local(Rc<V>),
    /// A shared memo value and the memo whose table its indices name.
    Shared(std::sync::Arc<V>, std::sync::Arc<SharedMemo>),
}

impl<V> std::ops::Deref for Held<V> {
    type Target = V;
    fn deref(&self) -> &V {
        match self {
            Held::Local(value) => value,
            Held::Shared(value, _) => value,
        }
    }
}

struct Memo<V> {
    entries: HashMap<Box<[i64]>, Rc<V>>,
    hits: u64,
    misses: u64,
}

impl<V> Default for Memo<V> {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
            hits: 0,
            misses: 0,
        }
    }
}

/// The distinct coefficients of this thread's memo values, each stored once
/// (on this thread's polynomial context, [`rehome`]).
#[derive(Default)]
struct Coefficients {
    index: HashMap<Coefficient, u32>,
    values: Vec<Coefficient>,
}

impl Coefficients {
    fn intern(&mut self, coefficient: &Coefficient) -> u32 {
        if let Some(&index) = self.index.get(coefficient) {
            return index;
        }
        let index = u32::try_from(self.values.len()).expect("fewer than 2^32 memo coefficients");
        let value = rehome(coefficient);
        self.values.push(value.clone());
        self.index.insert(value, index);
        index
    }
}

#[derive(Default)]
struct State {
    limits: Option<ExactAlgebraLimits>,
    insertion: Memo<InsertionWeights>,
    route: Memo<RouteExpansion>,
    coefficients: Coefficients,
}

impl State {
    /// Forget everything when the limits change or the memos are full.
    fn admit(&mut self, limits: ExactAlgebraLimits) {
        let full = self.insertion.entries.len() + self.route.entries.len() >= MEMO_ENTRIES;
        if self.limits != Some(limits) || full {
            self.insertion.entries = HashMap::new();
            self.route.entries = HashMap::new();
            self.coefficients = Coefficients::default();
            self.limits = Some(limits);
        }
    }
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
    /// Spent child power vectors of this thread, for reuse (always on: it
    /// changes no value).
    static POWERS: RefCell<Vec<Vec<i64>>> = const { RefCell::new(Vec::new()) };
}

/// Most spent power vectors kept per thread.
const POOL: usize = 1024;

/// rr-alloc: an empty power vector for a child of a step, from this thread's
/// pool of spent ones (no allocation in the steady state of a reduction).
pub(crate) fn take_powers() -> Vec<i64> {
    POWERS
        .with(|pool| pool.borrow_mut().pop())
        .unwrap_or_default()
}

/// rr-alloc: return the power vector of a consumed child to this thread's pool.
pub(crate) fn put_powers(mut powers: Vec<i64>) {
    if powers.capacity() == 0 || powers.capacity() > 256 {
        return;
    }
    powers.clear();
    POWERS.with(|pool| {
        let mut pool = pool.borrow_mut();
        if pool.len() < POOL {
            pool.push(powers);
        }
    });
}

/// `f` on the memo coefficients (the values the indices of
/// [`InsertionWeights`] and [`RouteExpansion`] name): `get(i)` is a copy of
/// coefficient `i` (on this thread's polynomial context, [`rehome`]).
#[allow(dead_code)] // memo API kept for the check mode and callers without a memo handle (rr-alloc)
pub(crate) fn coefficients<R>(f: impl FnOnce(&dyn Fn(u32) -> Coefficient) -> R) -> R {
    if shared_enabled() {
        return with_shared(|memo| f(&|index| rehome(memo.coefficient(index))));
    }
    local_coefficients(f)
}

/// [`coefficients`] for the indices of `held`: the table of the memo that
/// issued them (the mode is decided once per lookup, by [`insertion`] or
/// [`route`]).
pub(crate) fn coefficients_of<V, R>(
    held: &Held<V>,
    f: impl FnOnce(&dyn Fn(u32) -> Coefficient) -> R,
) -> R {
    match held {
        Held::Shared(_, memo) => f(&|index| rehome(memo.coefficient(index))),
        Held::Local(_) => local_coefficients(f),
    }
}

fn local_coefficients<R>(f: impl FnOnce(&dyn Fn(u32) -> Coefficient) -> R) -> R {
    STATE.with(|state| {
        let state = state.borrow();
        let values = &state.coefficients.values;
        f(&|index| values[index as usize].clone())
    })
}

/// Whether memo values move to a polynomial context of their thread
/// (`RUSTRED_P_STEP_MEMO_REHOME=0` keeps the shared one; default: move).
fn rehome_enabled() -> bool {
    static REHOME: OnceLock<bool> = OnceLock::new();
    *REHOME.get_or_init(|| std::env::var("RUSTRED_P_STEP_MEMO_REHOME").as_deref() != Ok("0"))
}

thread_local! {
    /// Zero polynomials owning this thread's polynomial contexts, one per
    /// variable map met recently (rr-par2: several families' maps alternate;
    /// one slot was rebuilt on every alternation).
    static HOME: RefCell<Vec<crate::algebra::CoefficientPolynomial>> = const { RefCell::new(Vec::new()) };
}

/// rr-alloc, the 16-thread lever: `coefficient` with both polynomials on
/// this thread's own polynomial context (same ring, same variable map, so
/// equal, hashing and comparing as before). Every Symbolica polynomial holds
/// an `Arc` of its context and every clone and drop updates its count: memo
/// values cloned once per edge on a worker would otherwise all update the one
/// count of the library's context, from every worker. Symbolica merges equal
/// contexts on arithmetic (`unify_variables`), so nothing else changes.
pub(crate) fn rehome(coefficient: &Coefficient) -> Coefficient {
    if !rehome_enabled() {
        return coefficient.clone();
    }
    HOME.with(|home| {
        let mut home = home.borrow_mut();
        let variables = coefficient.numerator.variables();
        let at = match home.iter().position(|zero| {
            std::sync::Arc::ptr_eq(zero.variables(), variables)
                && zero.ring() == coefficient.numerator.ring()
        }) {
            Some(at) => at,
            None => {
                if home.len() >= HOME_SLOTS {
                    home.remove(0);
                }
                home.push(crate::algebra::CoefficientPolynomial::new(
                    coefficient.numerator.ring(),
                    None,
                    std::sync::Arc::clone(variables),
                ));
                home.len() - 1
            }
        };
        let zero = &home[at];
        let moved = |polynomial: &crate::algebra::CoefficientPolynomial| {
            if !std::sync::Arc::ptr_eq(polynomial.variables(), variables) {
                return polynomial.clone();
            }
            let mut copy = zero.clone();
            copy.coefficients.clone_from(&polynomial.coefficients);
            copy.exponents.clone_from(&polynomial.exponents);
            copy
        };
        Coefficient {
            numerator: moved(&coefficient.numerator),
            denominator: moved(&coefficient.denominator),
        }
    })
}

/// `RUSTRED_P_HOME_CONTEXT` (rr-par2; on by default since M5 batch 4,
/// `0`/`off`/`false` disables; multi-thread requests only): Insertion
/// weights are computed on a copy of the coefficient context whose template
/// lives on this thread's polynomial context ([`with_home_context`]).
fn home_context_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| {
        !matches!(
            std::env::var("RUSTRED_P_HOME_CONTEXT").as_deref(),
            Ok("0" | "off" | "false")
        )
    })
}

thread_local! {
    /// This thread's copies of the coefficient contexts seen recently by
    /// [`with_home_context`] (same names and variable map, template rehomed).
    static HOME_CONTEXT: RefCell<Vec<CoefficientContext>> = const { RefCell::new(Vec::new()) };
}

/// Variable maps (and contexts) remembered per thread.
const HOME_SLOTS: usize = 16;

/// rr-par2: `f` on `context`, or (switch on) on its copy whose constants
/// (`zero`, `one`, `integer`, ...) are made on this thread's polynomial
/// context. Every polynomial derived from the library context clones and
/// drops the one `Arc` of that context, from every worker; derived from the
/// copy they touch only this thread's. Same ring, same variable map (the same
/// `Arc`), so every value, comparison and hash is the one of `context`.
pub(crate) fn with_home_context<R>(
    context: &CoefficientContext,
    f: impl FnOnce(&CoefficientContext) -> R,
) -> R {
    // One-thread requests (marked by `begin`) have no reference counts to
    // share: measured +12..+37 % at one thread with the copy.
    if !home_context_enabled() || SINGLE.with(std::cell::Cell::get) {
        return f(context);
    }
    HOME_CONTEXT.with(|home| {
        let find = |home: &[CoefficientContext]| home.iter().position(|c| c.shares_maps(context));
        if find(&home.borrow()).is_none()
            && let Ok(mut slots) = home.try_borrow_mut()
        {
            if slots.len() >= HOME_SLOTS {
                slots.remove(0);
            }
            slots.push(context.with_template(rehome(context.template())));
        }
        match home.try_borrow() {
            Ok(guard) => match find(&guard) {
                Some(at) => f(&guard[at]),
                None => f(context),
            },
            Err(_) => f(context),
        }
    })
}

/// The Insertion weights of `key`, from this thread's memo or `compute`
/// (the numerator table and the weight of each of its monomials).
#[allow(clippy::type_complexity)]
pub(crate) fn insertion(
    key: &[i64],
    limits: ExactAlgebraLimits,
    compute: impl FnOnce() -> Result<
        (
            std::sync::Arc<super::kernel::NumeratorTable>,
            Vec<Option<Coefficient>>,
        ),
        PError,
    >,
) -> Result<Option<Held<InsertionWeights>>, PError> {
    if shared_enabled() {
        return with_shared(|memo| {
            if let Some(value) = memo.get(key, true, limits, |shard| &shard.insertion) {
                return Ok(Some(Held::Shared(value, std::sync::Arc::clone(memo))));
            }
            let (table, weights) = compute()?;
            let Some(weights) = weights
                .iter()
                .map(|weight| match weight {
                    None => Some(None),
                    Some(w) => memo.intern(w).map(Some),
                })
                .collect::<Option<Vec<_>>>()
            else {
                return Ok(None);
            };
            let value = std::sync::Arc::new(InsertionWeights { table, weights });
            memo.put(key, limits, |shard| &mut shard.insertion, &value);
            Ok(Some(Held::Shared(value, std::sync::Arc::clone(memo))))
        });
    }
    STATE.with(|state| {
        {
            let mut state = state.borrow_mut();
            state.admit(limits);
            if let Some(value) = state.insertion.entries.get(key).cloned() {
                state.insertion.hits += 1;
                return Ok(Some(Held::Local(value)));
            }
            state.insertion.misses += 1;
        }
        // Computed outside the borrow (a panic in it must not poison the cell).
        let (table, weights) = compute()?;
        let mut state = state.borrow_mut();
        let weights = weights
            .iter()
            .map(|weight| weight.as_ref().map(|w| state.coefficients.intern(w)))
            .collect();
        let value = Rc::new(InsertionWeights { table, weights });
        state
            .insertion
            .entries
            .insert(key.into(), Rc::clone(&value));
        Ok(Some(Held::Local(value)))
    })
}

/// The routed expansion of `key`, from this thread's memo or `compute`
/// (target arity, relative endpoint powers and coefficients); `None` when an
/// offset does not fit `i8` (the caller then transports directly).
#[allow(clippy::type_complexity)]
pub(crate) fn route(
    key: &[i64],
    limits: ExactAlgebraLimits,
    compute: impl FnOnce() -> Result<(usize, Vec<i64>, Vec<Coefficient>), PError>,
) -> Result<Option<Held<RouteExpansion>>, PError> {
    if shared_enabled() {
        return with_shared(|memo| {
            if let Some(value) = memo.get(key, false, limits, |shard| &shard.route) {
                return Ok(Some(Held::Shared(value, std::sync::Arc::clone(memo))));
            }
            let (arity, offsets, coefficients) = compute()?;
            let Ok(offsets) = offsets
                .iter()
                .map(|&o| i8::try_from(o))
                .collect::<Result<Vec<i8>, _>>()
            else {
                return Ok(None);
            };
            let Some(coefficients) = coefficients
                .iter()
                .map(|c| memo.intern(c))
                .collect::<Option<Vec<_>>>()
            else {
                return Ok(None);
            };
            let value = std::sync::Arc::new(RouteExpansion {
                arity,
                offsets,
                coefficients,
            });
            memo.put(key, limits, |shard| &mut shard.route, &value);
            Ok(Some(Held::Shared(value, std::sync::Arc::clone(memo))))
        });
    }
    STATE.with(|state| {
        {
            let mut state = state.borrow_mut();
            state.admit(limits);
            if let Some(value) = state.route.entries.get(key).cloned() {
                state.route.hits += 1;
                return Ok(Some(Held::Local(value)));
            }
            state.route.misses += 1;
        }
        let (arity, offsets, coefficients) = compute()?;
        let Ok(offsets) = offsets
            .iter()
            .map(|&o| i8::try_from(o))
            .collect::<Result<Vec<i8>, _>>()
        else {
            return Ok(None);
        };
        let mut state = state.borrow_mut();
        let coefficients = coefficients
            .iter()
            .map(|c| state.coefficients.intern(c))
            .collect();
        let value = Rc::new(RouteExpansion {
            arity,
            offsets,
            coefficients,
        });
        state.route.entries.insert(key.into(), Rc::clone(&value));
        Ok(Some(Held::Local(value)))
    })
}

/// Drop this thread's memo entries (end of a request); the shared memo is
/// dropped when no request uses it any more ([`begin`]).
pub(crate) fn clear() {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let (insertion, route) = (
            (state.insertion.hits, state.insertion.misses),
            (state.route.hits, state.route.misses),
        );
        *state = State::default();
        (state.insertion.hits, state.insertion.misses) = insertion;
        (state.route.hits, state.route.misses) = route;
    });
}

/// `(insertion hits, misses, entries, route hits, misses, entries,
/// distinct coefficients)` of this thread's memos (diagnostics and tests).
pub(crate) fn counters() -> [u64; 7] {
    if shared_requested()
        && SHARED_MEMO
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_some()
    {
        return with_shared(|memo| memo.counters());
    }
    STATE.with(|state| {
        let state = state.borrow();
        [
            state.insertion.hits,
            state.insertion.misses,
            state.insertion.entries.len() as u64,
            state.route.hits,
            state.route.misses,
            state.route.entries.len() as u64,
            state.coefficients.values.len() as u64,
        ]
    })
}

/// `RUSTRED_P_STEP_MEMO_SHARED` (rr-par; ON by default since M5 batch 3,
/// `0`/`off`/`false` disables it; only requests on several threads): with the
/// memo on, ONE memo shared by every thread of the process instead of one
/// per thread (16 copies at 16 threads). Entries are sharded under
/// read-write locks; the distinct coefficients are stored once in an
/// append-only table read without a lock, and copied onto the reading
/// thread's polynomial context ([`rehome`]). The memo is bounded
/// ([`MEMO_ENTRIES`] entries, [`SHARED_COEFFICIENTS`] coefficients): beyond
/// that a step computes directly. It lives while requests use it (a count
/// of active requests, [`begin`] / [`clear`]) and is dropped after the last
/// one. Values are those of the per-thread memo (same keys, same
/// computations).
pub(crate) fn shared_enabled() -> bool {
    // Only for requests on several threads: one thread has nothing to share
    // and keeps the cheaper per-thread memo (a one-thread request runs all
    // its steps on its calling thread, marked by `begin`).
    shared_requested()
        && !SINGLE.with(std::cell::Cell::get)
        && SHARED_ACTIVE.load(Ordering::Relaxed) > 0
}

thread_local! {
    /// This thread runs a one-thread request (per-thread memo).
    static SINGLE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn shared_requested() -> bool {
    static SHARED: OnceLock<bool> = OnceLock::new();
    *SHARED.get_or_init(|| {
        mode() != MemoMode::Off
            && !matches!(
                std::env::var("RUSTRED_P_STEP_MEMO_SHARED").as_deref(),
                Ok("0" | "off" | "false")
            )
    })
}

/// Distinct coefficients of the shared memo.
pub(crate) const SHARED_COEFFICIENTS: usize = 1 << 20;
const CHUNK: usize = 1 << 12;
const SHARDS: usize = 64;

#[derive(Default)]
struct Shard {
    insertion: HashMap<Box<[i64]>, std::sync::Arc<InsertionWeights>>,
    route: HashMap<Box<[i64]>, std::sync::Arc<RouteExpansion>>,
}

pub(crate) struct SharedMemo {
    /// The limits of the first request (rr-par2: set once, read lock-free).
    limits: OnceLock<ExactAlgebraLimits>,
    shards: Box<[std::sync::RwLock<Shard>]>,
    entries: AtomicU64,
    #[allow(clippy::type_complexity)] // chunked append-only coefficient table (rr-par)
    chunks: Box<[OnceLock<Box<[OnceLock<Coefficient>]>>]>,
    /// rr-par2: the coefficient index, sharded by value (Fx hash), and the
    /// next free index. Index numbering follows the arrival order of the
    /// threads (as with one lock); the values the indices name do not.
    index: Box<[std::sync::Mutex<HashMap<Coefficient, u32, FxLiteBuild>>]>,
    next: std::sync::atomic::AtomicUsize,
    stats: [AtomicU64; 4],
}

#[derive(Clone, Copy, Default)]
struct FxLiteBuild;

impl std::hash::BuildHasher for FxLiteBuild {
    type Hasher = FxLite;
    fn build_hasher(&self) -> FxLite {
        FxLite(0)
    }
}

impl SharedMemo {
    fn new() -> Self {
        Self {
            limits: OnceLock::new(),
            shards: (0..SHARDS).map(|_| Default::default()).collect(),
            entries: AtomicU64::new(0),
            chunks: (0..SHARED_COEFFICIENTS / CHUNK)
                .map(|_| OnceLock::new())
                .collect(),
            index: (0..SHARDS).map(|_| Default::default()).collect(),
            next: std::sync::atomic::AtomicUsize::new(0),
            stats: Default::default(),
        }
    }

    fn shard(&self, key: &[i64]) -> &std::sync::RwLock<Shard> {
        use std::hash::{Hash, Hasher};
        let mut h = FxLite(0);
        key.hash(&mut h);
        &self.shards[(h.finish() as usize) % SHARDS]
    }

    /// The memo applies to requests with the limits of its first request.
    fn admits(&self, limits: ExactAlgebraLimits) -> bool {
        *self.limits.get_or_init(|| limits) == limits
    }

    fn get<V>(
        &self,
        key: &[i64],
        insertion: bool,
        limits: ExactAlgebraLimits,
        map: impl Fn(&Shard) -> &HashMap<Box<[i64]>, std::sync::Arc<V>>,
    ) -> Option<std::sync::Arc<V>> {
        let stat = if insertion { 0 } else { 2 };
        if !self.admits(limits) {
            return None;
        }
        let shard = self.shard(key).read().unwrap_or_else(|e| e.into_inner());
        let found = map(&shard).get(key).cloned();
        self.stats[stat + usize::from(found.is_none())].fetch_add(1, Ordering::Relaxed);
        found
    }

    fn put<V>(
        &self,
        key: &[i64],
        limits: ExactAlgebraLimits,
        map: impl Fn(&mut Shard) -> &mut HashMap<Box<[i64]>, std::sync::Arc<V>>,
        value: &std::sync::Arc<V>,
    ) {
        if !self.admits(limits) || self.entries.load(Ordering::Relaxed) >= MEMO_ENTRIES as u64 {
            return;
        }
        let mut shard = self.shard(key).write().unwrap_or_else(|e| e.into_inner());
        if map(&mut shard)
            .insert(key.into(), std::sync::Arc::clone(value))
            .is_none()
        {
            self.entries.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// The index of `coefficient` in the table; `None` when it is full.
    fn intern(&self, coefficient: &Coefficient) -> Option<u32> {
        use std::hash::BuildHasher;
        let shard = (FxLiteBuild.hash_one(coefficient) >> 32) as usize % SHARDS;
        let mut index = self.index[shard].lock().unwrap_or_else(|e| e.into_inner());
        if let Some(&found) = index.get(coefficient) {
            return Some(found);
        }
        if self.next.load(Ordering::Relaxed) >= SHARED_COEFFICIENTS {
            return None;
        }
        let next = self.next.fetch_add(1, Ordering::Relaxed);
        if next >= SHARED_COEFFICIENTS {
            return None;
        }
        let value = rehome(coefficient);
        let chunk =
            self.chunks[next / CHUNK].get_or_init(|| (0..CHUNK).map(|_| OnceLock::new()).collect());
        let _ = chunk[next % CHUNK].set(value.clone());
        index.insert(value, next as u32);
        Some(next as u32)
    }

    fn coefficient(&self, index: u32) -> &Coefficient {
        let index = index as usize;
        self.chunks[index / CHUNK]
            .get()
            .and_then(|chunk| chunk[index % CHUNK].get())
            .expect("an interned shared memo coefficient")
    }

    fn counters(&self) -> [u64; 7] {
        let (mut insertion, mut route) = (0u64, 0u64);
        for shard in self.shards.iter() {
            let shard = shard.read().unwrap_or_else(|e| e.into_inner());
            insertion += shard.insertion.len() as u64;
            route += shard.route.len() as u64;
        }
        let stat = |i: usize| self.stats[i].load(Ordering::Relaxed);
        let distinct = (self.next.load(Ordering::Relaxed) as u64).min(SHARED_COEFFICIENTS as u64);
        [
            stat(0),
            stat(1),
            insertion,
            stat(2),
            stat(3),
            route,
            distinct,
        ]
    }
}

/// A small fixed hasher for shard selection.
struct FxLite(u64);

impl std::hash::Hasher for FxLite {
    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.write_u64(u64::from(byte));
        }
    }
    fn write_u64(&mut self, value: u64) {
        self.0 = (self.0.rotate_left(5) ^ value).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
    fn write_i64(&mut self, value: i64) {
        self.write_u64(value as u64);
    }
    fn write_usize(&mut self, value: usize) {
        self.write_u64(value as u64);
    }
    fn finish(&self) -> u64 {
        self.0
    }
}

/// The process-wide shared memo, its generation and its active requests.
static SHARED_MEMO: std::sync::Mutex<Option<std::sync::Arc<SharedMemo>>> =
    std::sync::Mutex::new(None);
static SHARED_GENERATION: AtomicU64 = AtomicU64::new(1);
static SHARED_ACTIVE: AtomicU64 = AtomicU64::new(0);

thread_local! {
    /// This thread's handle on the shared memo and its generation.
    static SHARED_LOCAL: RefCell<Option<(u64, std::sync::Arc<SharedMemo>)>> = const { RefCell::new(None) };
}

/// `f` on the current shared memo (created on first use).
fn with_shared<R>(f: impl FnOnce(&std::sync::Arc<SharedMemo>) -> R) -> R {
    let generation = SHARED_GENERATION.load(Ordering::Acquire);
    // The thread's handle, without touching the shared count of the memo on
    // every call (one refresh per generation).
    let fresh = SHARED_LOCAL.with(|local| {
        local
            .try_borrow()
            .is_ok_and(|local| matches!(&*local, Some((held, _)) if *held == generation))
    });
    if !fresh {
        let memo = std::sync::Arc::clone(
            SHARED_MEMO
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get_or_insert_with(|| std::sync::Arc::new(SharedMemo::new())),
        );
        let installed = SHARED_LOCAL.with(|local| match local.try_borrow_mut() {
            Ok(mut local) => {
                *local = Some((generation, std::sync::Arc::clone(&memo)));
                true
            }
            Err(_) => false,
        });
        if !installed {
            // A nested call while the handle is borrowed: use this one.
            return f(&memo);
        }
    }
    SHARED_LOCAL.with(|local| {
        let local = local.borrow();
        f(&local.as_ref().expect("installed above").1)
    })
}

/// A request on `threads` threads starts using the memos: the shared memo
/// (when requested) serves requests on several threads and lives while any
/// of them runs. Returns whether this request counts as one ([`end`]).
pub(crate) fn begin(threads: usize) -> bool {
    SINGLE.with(|single| single.set(threads <= 1));
    let counted = shared_requested() && threads > 1;
    if counted {
        SHARED_ACTIVE.fetch_add(1, Ordering::AcqRel);
    }
    counted
}

/// The request of [`begin`] ends: drop the calling thread's memo entries,
/// and the shared memo after the last request that used it.
pub(crate) fn end(counted: bool) {
    if counted {
        end_shared();
    }
    SINGLE.with(|single| single.set(false));
    clear();
}

fn end_shared() {
    let before = SHARED_ACTIVE
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
            Some(n.saturating_sub(1))
        })
        .unwrap_or(0);
    if before <= 1 {
        let mut memo = SHARED_MEMO.lock().unwrap_or_else(|e| e.into_inner());
        SHARED_GENERATION.fetch_add(1, Ordering::AcqRel);
        *memo = None;
        SHARED_LOCAL.with(|local| *local.borrow_mut() = None);
    }
}

/// The error of a `check` step whose memo value differs from the direct one.
pub(crate) fn mismatch(what: &str) -> PError {
    PError::Contract(format!(
        "RUSTRED_P_STEP_MEMO=check: the memoised {what} differs from the direct computation"
    ))
}

#[cfg(test)]
mod tests {
    use std::hash::BuildHasher;

    use super::*;
    use crate::algebra::CoefficientContext;

    fn sample() -> (CoefficientContext, Coefficient) {
        let context = CoefficientContext::try_new(["d"]).unwrap();
        let limits = ExactAlgebraLimits::default();
        let d = context.parameter("d").unwrap();
        let numerator = context.try_sub(&d, &context.integer(4), limits).unwrap();
        let denominator = context.try_add(&d, &context.integer(3), limits).unwrap();
        let value = context.try_div(&numerator, &denominator, limits).unwrap();
        (context, value)
    }

    /// A rehomed coefficient is the same value on another polynomial context:
    /// equal, same hash, accepted by the context, and arithmetic with it
    /// gives the same results.
    #[test]
    fn a_rehomed_coefficient_is_the_same_value() {
        let (context, value) = sample();
        let moved = std::thread::spawn({
            let value = value.clone();
            move || rehome(&value)
        })
        .join()
        .unwrap();
        assert_eq!(moved, value);
        let hasher = std::collections::hash_map::RandomState::new();
        assert_eq!(hasher.hash_one(&moved), hasher.hash_one(&value));
        assert!(context.contains(&moved));
        let limits = ExactAlgebraLimits::default();
        assert_eq!(
            context.try_add(&moved, &value, limits).unwrap(),
            context.try_add(&value, &value, limits).unwrap()
        );
        assert_eq!(
            context.try_mul(&moved, &moved, limits).unwrap(),
            context.try_mul(&value, &value, limits).unwrap()
        );
    }

    /// The memo returns what `compute` gave for a key, computes once per key
    /// and limits, stores equal coefficients once, and forgets everything on
    /// `clear`.
    #[test]
    fn the_route_memo_computes_once_per_key() {
        let (_, value) = sample();
        let limits = ExactAlgebraLimits::default();
        let key = [next_uid() as i64, 1, -2];
        let mut calls = 0;
        for _ in 0..3 {
            let got = route(&key, limits, || {
                calls += 1;
                Ok((2, vec![0, -1, 1, 0], vec![value.clone(), value.clone()]))
            })
            .unwrap()
            .unwrap();
            assert_eq!(got.offsets, [0, -1, 1, 0]);
            assert_eq!(got.coefficients[0], got.coefficients[1]);
            coefficients(|values| assert_eq!(values(got.coefficients[0]), value));
        }
        assert_eq!(calls, 1);
        // An offset outside i8 is not memoised.
        let wide = [next_uid() as i64];
        assert!(
            route(&wide, limits, || Ok((1, vec![300], vec![value.clone()])))
                .unwrap()
                .is_none()
        );
        clear();
        route(&key, limits, || {
            calls += 1;
            Ok((0, vec![], vec![]))
        })
        .unwrap();
        assert_eq!(calls, 2);
    }

    #[test]
    fn spent_powers_are_reused() {
        let mut powers = take_powers();
        powers.extend([1, 2, 3]);
        let address = powers.as_ptr();
        put_powers(powers);
        let again = take_powers();
        assert!(again.is_empty());
        assert_eq!(again.as_ptr(), address);
    }
}
