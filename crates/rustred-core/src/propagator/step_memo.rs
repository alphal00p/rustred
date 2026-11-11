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

/// `f` on this thread's memo coefficients (the values the indices of
/// [`InsertionWeights`] and [`RouteExpansion`] name).
pub(crate) fn coefficients<R>(f: impl FnOnce(&[Coefficient]) -> R) -> R {
    STATE.with(|state| f(&state.borrow().coefficients.values))
}

/// Whether memo values move to a polynomial context of their thread
/// (`RUSTRED_P_STEP_MEMO_REHOME=0` keeps the shared one; default: move).
fn rehome_enabled() -> bool {
    static REHOME: OnceLock<bool> = OnceLock::new();
    *REHOME.get_or_init(|| std::env::var("RUSTRED_P_STEP_MEMO_REHOME").as_deref() != Ok("0"))
}

thread_local! {
    /// A zero polynomial owning this thread's polynomial context, and the
    /// variable map it was made for.
    static HOME: RefCell<Option<crate::algebra::CoefficientPolynomial>> = const { RefCell::new(None) };
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
        let fits = home.as_ref().is_some_and(|zero| {
            std::sync::Arc::ptr_eq(zero.variables(), variables)
                && zero.ring() == coefficient.numerator.ring()
        });
        if !fits {
            *home = Some(crate::algebra::CoefficientPolynomial::new(
                coefficient.numerator.ring(),
                None,
                std::sync::Arc::clone(variables),
            ));
        }
        let zero = home.as_ref().expect("set above");
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
) -> Result<Rc<InsertionWeights>, PError> {
    STATE.with(|state| {
        {
            let mut state = state.borrow_mut();
            state.admit(limits);
            if let Some(value) = state.insertion.entries.get(key).cloned() {
                state.insertion.hits += 1;
                return Ok(value);
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
        Ok(value)
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
) -> Result<Option<Rc<RouteExpansion>>, PError> {
    STATE.with(|state| {
        {
            let mut state = state.borrow_mut();
            state.admit(limits);
            if let Some(value) = state.route.entries.get(key).cloned() {
                state.route.hits += 1;
                return Ok(Some(value));
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
        Ok(Some(value))
    })
}

/// Drop this thread's memo entries (end of a request).
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
            coefficients(|values| assert_eq!(values[got.coefficients[0] as usize], value));
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
