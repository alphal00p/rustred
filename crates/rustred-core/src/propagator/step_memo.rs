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
//! exact-algebra limits of the calling request change or a memo exceeds
//! [`MEMO_ENTRIES`] entries.
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use smallvec::SmallVec;

use crate::algebra::{Coefficient, ExactAlgebraLimits};

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

pub(crate) fn mode() -> MemoMode {
    static MODE: OnceLock<MemoMode> = OnceLock::new();
    *MODE.get_or_init(|| match std::env::var("RUSTRED_P_STEP_MEMO").as_deref() {
        Ok("1" | "on") => MemoMode::On,
        Ok("check") => MemoMode::Check,
        _ => MemoMode::Off,
    })
}

/// A process-unique id of a memo owner.
pub(crate) fn next_uid() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// The key of a memo entry, on the stack for every p key of the 4L and 5L
/// libraries (owner id, a few tags, at most ~20 powers).
pub(crate) type MemoKey = SmallVec<[i64; 32]>;

/// Child weights of an Insertion step: one per monomial of its numerator
/// table, `None` where the weight is zero.
pub(crate) struct InsertionWeights {
    pub table: std::sync::Arc<super::kernel::NumeratorTable>,
    pub weights: Vec<Option<Coefficient>>,
}

/// Endpoints of a routed numerator expansion: target powers relative to the
/// layered base, with their coefficients.
pub(crate) type RouteExpansion = Vec<(SmallVec<[i64; 24]>, Coefficient)>;

struct Memo<V> {
    limits: Option<ExactAlgebraLimits>,
    entries: HashMap<MemoKey, Rc<V>>,
    hits: u64,
    misses: u64,
}

impl<V> Default for Memo<V> {
    fn default() -> Self {
        Self {
            limits: None,
            entries: HashMap::new(),
            hits: 0,
            misses: 0,
        }
    }
}

thread_local! {
    static INSERTION: RefCell<Memo<InsertionWeights>> = RefCell::new(Memo::default());
    static ROUTE: RefCell<Memo<RouteExpansion>> = RefCell::new(Memo::default());
}

fn lookup<V>(
    memo: &RefCell<Memo<V>>,
    key: &MemoKey,
    limits: ExactAlgebraLimits,
    compute: impl FnOnce() -> Result<V, PError>,
) -> Result<Rc<V>, PError> {
    {
        let mut memo = memo.borrow_mut();
        if memo.limits != Some(limits) {
            memo.entries.clear();
            memo.limits = Some(limits);
        }
        if let Some(value) = memo.entries.get(key) {
            let value = Rc::clone(value);
            memo.hits += 1;
            return Ok(value);
        }
        memo.misses += 1;
    }
    // Computed outside the borrow: the computation never re-enters this
    // memo, but a panic in it must not poison the cell.
    let value = Rc::new(compute()?);
    let mut memo = memo.borrow_mut();
    if memo.entries.len() >= MEMO_ENTRIES {
        memo.entries.clear();
    }
    memo.entries.insert(key.clone(), Rc::clone(&value));
    Ok(value)
}

/// The Insertion weights of `key`, from this thread's memo or `compute`.
pub(crate) fn insertion(
    key: &MemoKey,
    limits: ExactAlgebraLimits,
    compute: impl FnOnce() -> Result<InsertionWeights, PError>,
) -> Result<Rc<InsertionWeights>, PError> {
    INSERTION.with(|memo| lookup(memo, key, limits, compute))
}

/// The routed expansion of `key`, from this thread's memo or `compute`.
pub(crate) fn route(
    key: &MemoKey,
    limits: ExactAlgebraLimits,
    compute: impl FnOnce() -> Result<RouteExpansion, PError>,
) -> Result<Rc<RouteExpansion>, PError> {
    ROUTE.with(|memo| lookup(memo, key, limits, compute))
}

/// `(insertion hits, misses, entries, route hits, misses, entries)` of this
/// thread's memos (diagnostics and tests).
pub(crate) fn counters() -> [u64; 6] {
    let insertion = INSERTION.with(|memo| {
        let memo = memo.borrow();
        [memo.hits, memo.misses, memo.entries.len() as u64]
    });
    let route = ROUTE.with(|memo| {
        let memo = memo.borrow();
        [memo.hits, memo.misses, memo.entries.len() as u64]
    });
    [
        insertion[0],
        insertion[1],
        insertion[2],
        route[0],
        route[1],
        route[2],
    ]
}

/// The error of a `check` step whose memo value differs from the direct one.
pub(crate) fn mismatch(what: &str) -> PError {
    PError::Contract(format!(
        "RUSTRED_P_STEP_MEMO=check: the memoised {what} differs from the direct computation"
    ))
}
