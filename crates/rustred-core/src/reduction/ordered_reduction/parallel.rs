//! Concurrent execution of Ordered phases.
//!
//! The sequential scheduler pops the greatest pending key, applies its
//! checked step and merges the weighted children into the frontier, one key
//! at a time. Concurrent execution reproduces that execution exactly; it only
//! changes which thread computes what, and when.
//!
//! A phase is processed in *windows*. A window takes the greatest
//! `window_keys` pending keys of the phase (threshold `t` = the last of them)
//! and closes them under same-phase descent above `t`: every same-phase child
//! above `t` of a window step is a window key as well. Every key the
//! sequential scheduler would pop before passing `t` is therefore a window key,
//! and every parent of a window key is a window key or was popped earlier.
//! Inside a window:
//!
//! 1. Steps are computed concurrently by the adapter's [`OrderedStepper`],
//!    exploring the closure by work stealing (a finished step spawns the steps
//!    of its new window children). Steps are pure functions of the key, so
//!    computing them early, out of order, or for keys that later cancel
//!    changes nothing; only the steps of keys the sequential scheduler expands
//!    are ever used.
//! 2. The window keys form a DAG (children strictly descend). A key settles
//!    once all of its window parents have settled: it then merges its window
//!    contributions in sequential order (parent pop order, then child order),
//!    which is exactly the sequence of merges the sequential scheduler applies
//!    to that key, and it is expanded at its turn iff its weight is then
//!    nonzero. Independent keys settle concurrently.
//! 3. Contributions to keys outside the window (lower in this phase or in
//!    later phases) and to terminals are merged per destination key (one task
//!    per destination), concurrently across keys and again in sequential order
//!    per key. A destination that receives a large share of the window is
//!    merged by exact prefix sums on several threads (`merge_prefix`), which
//!    reproduces each of its sequential states.
//! 4. A serial scan visits the expanded window keys in pop order and replays,
//!    from the recorded outcome of every merge, what the sequential scheduler
//!    does at each event: pending-frame and coalescing accounting, interning
//!    of the step's guards (first-appearance order), the adapter's step
//!    accounting ([`ConcurrentOrderedKernel::absorb`]), every progress counter
//!    including the peaks, every observation callback, and the first error.
//!
//! Hence the frontier, the terminals, the guard list, all counters and all
//! observations at every window boundary (so at every phase boundary and at
//! the end) equal those of the sequential scheduler, for every thread count
//! and window size. Exact zero erasure is decided on the same sums in the same
//! order. Weight arithmetic needs no commutativity or associativity for this,
//! except the prefix merge, which is used only for associative arithmetic with
//! a canonical representation ([`ConcurrentWeights::CANONICAL_SUMS`]).
//!
//! Products and merges run concurrently only for value arithmetic
//! ([`ConcurrentWeights`]: weights are plain values, nothing is journaled).
//! Arithmetic with a sequential journal (the modular trace recorder, whose
//! slots and coefficient interning follow the event order) keeps the
//! sequential merge loop; its steps are computed ahead by the workers
//! ([`Prefetched`]).
//!
//! Memory: a window holds at most `window_keys` popped weights and the steps
//! of its keys (children and small step coefficients). Products are formed
//! transiently by the thread that merges them, so in-flight arithmetic is at
//! most one product and one sum per worker. The frontier at every window
//! boundary is the sequential one.
use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::hash::Hash;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};

use rayon::prelude::*;
use symbolica::license::LicenseManager;
use symbolica::utils::resources::ResourceBudget;

use super::{
    Admitted, Frontier, OrderedKernel, OrderedPhaseBoundary, OrderedPosition, OrderedStep,
    OrderedWeights, Pending, RawOutput, Scheduled, StepSource, admit, check_descent, invariant,
    replace_monomials,
};
use crate::algebra::Coefficient;
use crate::family::IntegralKey;
use crate::reduction::{ReductionError, ReductionRequest, ReductionStatistics};

/// Execution options of the Ordered scheduler, independent of its weight
/// arithmetic.
///
/// Every setting returns the same terms, the same guards in the same order
/// and the same progress counters and observations; only the wall time and
/// the number of busy cores differ.
///
/// Callers that expose one core budget (Vakint's `n_cores`) pass it both here
/// and to the modular replay pool (`ModularWeightOptions::threads`); the two
/// pools are never busy at the same time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OrderedSchedulerOptions {
    /// Worker threads of the scheduler. `1` (the default) is the sequential
    /// scheduler on the calling thread: no pool and no thread is created.
    /// Larger values create one private pool per request; the calling thread
    /// coordinates and applies every sequential effect.
    pub threads: usize,
    /// Upper bound on the frontier keys taken into one concurrent window
    /// (same-phase descendants above the window's lowest key are added). It
    /// bounds the popped weights and the steps held at once.
    pub window_keys: usize,
}

impl Default for OrderedSchedulerOptions {
    fn default() -> Self {
        Self {
            threads: 1,
            window_keys: 4096,
        }
    }
}

impl OrderedSchedulerOptions {
    /// Default options with `threads` workers.
    pub fn with_threads(threads: usize) -> Self {
        Self {
            threads,
            ..Self::default()
        }
    }

    pub(crate) fn validate(&self) -> Result<(), ReductionError> {
        if self.threads == 0 {
            return Err(execution("ordered scheduler threads must be positive"));
        }
        if self.window_keys == 0 || self.window_keys > u32::MAX as usize / 2 {
            return Err(execution(
                "ordered scheduler window_keys must be positive and below 2^31",
            ));
        }
        Ok(())
    }
}

fn execution(detail: impl Into<String>) -> ReductionError {
    ReductionError::OrderedExecution {
        detail: detail.into(),
    }
}

/// `RUSTRED_TEST_ORDERED_THREADS=N` runs every Ordered unit test of a
/// concurrent adapter that uses the default options with `N` scheduler
/// threads and a small window.
#[cfg(test)]
pub(super) fn test_threads_override(scheduler: OrderedSchedulerOptions) -> OrderedSchedulerOptions {
    match std::env::var("RUSTRED_TEST_ORDERED_THREADS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
    {
        Some(threads) if scheduler == OrderedSchedulerOptions::default() && threads > 0 => {
            OrderedSchedulerOptions {
                threads,
                window_keys: 3,
            }
        }
        _ => scheduler,
    }
}

/// The weight-independent part of an Ordered adapter, shared by the worker
/// threads of one request.
///
/// `position` and `step` must be pure functions of the key and of the
/// read-only state the stepper borrows: every call for the same key returns
/// the same position, the same children and terminals with the same exact
/// coefficients, the same nonzero conditions and the same account, on any
/// thread and in any order. The scheduler computes steps ahead of time, out
/// of order and for keys that may later cancel; it applies the account of
/// exactly the keys the sequential scheduler expands, in its order.
pub(crate) trait OrderedStepper: Sync {
    type Key: Clone + Ord + Hash + Send + Sync;
    type Priority: Clone + Ord + Send + Sync;
    type Error: From<ReductionError> + Send;
    /// Work and observation records of one detached step.
    type Account: Send;

    fn position(&self, key: &Self::Key) -> Result<OrderedPosition<Self::Priority>, Self::Error>;
    /// The checked step of `key` and its account. The account is returned
    /// for failed steps as well (attempted work is retained).
    fn step(&self, key: &Self::Key)
    -> (Result<OrderedStep<Self::Key>, Self::Error>, Self::Account);
    /// `true` certifies that `child`, emitted by the step of `parent`, lies
    /// in a strictly later phase, so its position is not needed to place it
    /// outside the window. The default certifies nothing.
    fn later_phase(&self, _parent: &Self::Key, _child: &Self::Key) -> bool {
        false
    }
}

/// An adapter whose steps can be computed by worker threads.
pub(crate) trait ConcurrentOrderedKernel:
    OrderedKernel<Key: Send + Sync, Priority: Send + Sync, Error: Send>
{
    type Account: Send;
    type Stepper<'s>: OrderedStepper<
            Key = Self::Key,
            Priority = Self::Priority,
            Error = Self::Error,
            Account = Self::Account,
        >
    where
        Self: 's;

    /// The shared stepper, or `None` to keep this request sequential.
    fn stepper(&self) -> Option<Self::Stepper<'_>>;

    /// Apply the account of one detached step exactly where the sequential
    /// scheduler would have performed the step: `request` and `work` are the
    /// scheduler's, as passed to [`OrderedKernel::step`].
    fn absorb(
        &mut self,
        key: &Self::Key,
        account: Self::Account,
        request: &mut ReductionRequest,
        work: &mut ReductionStatistics,
    ) -> Result<(), Self::Error>;
}

/// Weight arithmetic whose products and merges are pure functions of their
/// operands. The merges into different keys of one window then run on
/// different threads; each key still receives its contributions in the
/// sequential order.
///
/// Value arithmetic keeps no sequential journal: `begin_step`, `end_step`,
/// `cancel` and `cancel_terminal` of [`OrderedWeights`] must be no-ops, and the
/// detached operations must equal their sequential counterparts.
pub(crate) trait ConcurrentWeights:
    OrderedWeights<Weight: Clone + Send + Sync, Contribution: Send> + Sync
{
    /// An adapter step coefficient, checked once for this arithmetic.
    type Prepared: Send + Sync;

    /// Check one step coefficient (the check of [`OrderedWeights::scale`]).
    fn prepare(&self, coefficient: Coefficient) -> Result<Self::Prepared, ReductionError>;
    fn scale_prepared(
        &self,
        weight: &Self::Weight,
        coefficient: &Self::Prepared,
    ) -> Result<Self::Contribution, ReductionError>;
    fn insert_detached(
        &self,
        contribution: Self::Contribution,
    ) -> Result<Self::Weight, ReductionError>;
    fn merge_detached(
        &self,
        existing: &mut Self::Weight,
        contribution: Self::Contribution,
    ) -> Result<bool, ReductionError>;
    fn insert_terminal_detached(
        &self,
        contribution: Self::Contribution,
    ) -> Result<Self::Weight, ReductionError>;
    fn merge_terminal_detached(
        &self,
        existing: &mut Self::Weight,
        contribution: Self::Contribution,
    ) -> Result<bool, ReductionError>;

    /// Weight sums are associative and canonical (equal values have equal
    /// representations). The sequential states of one destination can then be
    /// formed from partial sums on several threads (`merge_prefix`).
    const CANONICAL_SUMS: bool = false;
    /// `a + b`, possibly zero. Used only when `CANONICAL_SUMS`.
    fn sum_weights(
        &self,
        _a: &Self::Weight,
        _b: &Self::Weight,
    ) -> Result<Self::Weight, ReductionError> {
        Err(invariant("weight arithmetic has no canonical sums"))
    }
    /// Whether a weight formed by `sum_weights` is zero.
    fn weight_is_zero(&self, _weight: &Self::Weight) -> bool {
        false
    }
}

/// The private worker pool of one request.
pub(super) struct Workers {
    pool: rayon::ThreadPool,
    /// Resource budgets active on the calling thread, installed around every
    /// worker task.
    budget: ResourceBudget,
}

impl Workers {
    pub(super) fn new(threads: usize) -> Result<Self, ReductionError> {
        let allowed = LicenseManager::max_threads(threads);
        if allowed < threads {
            return Err(execution(format!(
                "Symbolica allows {allowed} of the {threads} requested ordered scheduler threads"
            )));
        }
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .thread_name(|ordinal| format!("rustred-ordered-{ordinal}"))
            .build()
            .map_err(|error| execution(format!("ordered scheduler pool: {error}")))?;
        // A caller-local library unlock is not inherited by new threads.
        let allowed = pool
            .broadcast(|_| LicenseManager::max_threads(threads))
            .into_iter()
            .min()
            .unwrap_or(0);
        if allowed < threads {
            return Err(execution(format!(
                "Symbolica allows {allowed} of the {threads} ordered scheduler threads on its workers"
            )));
        }
        Ok(Self {
            pool,
            budget: ResourceBudget::capture_current(),
        })
    }

    /// `f` on every item on the workers. Item costs are very uneven (merges
    /// of exact rational functions), so every item is its own task: a
    /// contiguous range never runs on one thread while the others idle.
    fn for_each<T: Send>(&self, items: &mut [T], f: impl Fn(&mut T) + Sync + Send) {
        let budget = &self.budget;
        self.pool.install(|| {
            items
                .par_iter_mut()
                .with_max_len(1)
                .for_each(|item| budget.run_captured(|| f(item)))
        });
    }
}

/// Window threshold comparison: `true` iff a same-phase key with this
/// position is popped before `threshold` (or is `threshold`).
fn at_or_above<K: Ord, P: Ord>(
    position: &OrderedPosition<P>,
    key: &K,
    threshold: &Scheduled<K, P>,
) -> bool {
    position.phase == threshold.phase.0
        && (&position.priority, key) >= (&threshold.priority, &threshold.key)
}

/// One step computed by a worker.
struct Detached<K, P, X, E, A> {
    key: K,
    position: OrderedPosition<P>,
    result: Result<X, E>,
    account: A,
}

/// Compute the steps of `roots` and of every same-phase descendant at or
/// above `threshold` on the workers. `detach` converts a step (on the worker)
/// and reports its same-phase children at or above the threshold. Roots come
/// first, in their given order; the other keys follow in no particular order.
///
/// The closure is explored by work stealing: a finished step immediately
/// spawns the steps of its newly found window children, so chains of
/// same-phase descendants (Apply phases) do not wait for synchronous rounds.
fn close_window<S, X, F>(
    workers: &Workers,
    stepper: &S,
    roots: Vec<(S::Key, OrderedPosition<S::Priority>)>,
    counters: Option<&WindowCounters>,
    detach: F,
) -> Vec<Detached<S::Key, S::Priority, X, S::Error, S::Account>>
where
    S: OrderedStepper,
    X: Send,
    F: Fn(
            &S::Key,
            &OrderedPosition<S::Priority>,
            OrderedStep<S::Key>,
            &mut Vec<(S::Key, OrderedPosition<S::Priority>)>,
        ) -> X
        + Sync
        + Send,
{
    let closure = Closure {
        stepper,
        detach: &detach,
        seen: Mutex::new(roots.iter().map(|(key, _)| key.clone()).collect()),
        discovered: Mutex::new(Vec::new()),
        counters,
        budget: &workers.budget,
    };
    let mut rooted: Vec<Option<Detached<S::Key, S::Priority, X, S::Error, S::Account>>> =
        roots.iter().map(|_| None).collect();
    {
        let closure = &closure;
        workers.pool.install(|| {
            rayon::scope(|scope| {
                for ((key, position), slot) in roots.into_iter().zip(rooted.iter_mut()) {
                    scope.spawn(move |scope| *slot = Some(closure.step(scope, key, position)));
                }
            })
        });
    }
    let discovered = closure
        .discovered
        .into_inner()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(counters) = counters {
        counters
            .discovered
            .fetch_add(discovered.len() as u64, Ordering::Relaxed);
    }
    rooted
        .into_iter()
        .map(|step| step.expect("every root step completes in the scope"))
        .chain(discovered)
        .collect()
}

/// Shared state of one window closure (`close_window`).
struct Closure<'a, S: OrderedStepper, X, F> {
    stepper: &'a S,
    detach: &'a F,
    seen: Mutex<HashSet<S::Key>>,
    discovered: Mutex<Vec<Detached<S::Key, S::Priority, X, S::Error, S::Account>>>,
    counters: Option<&'a WindowCounters>,
    budget: &'a ResourceBudget,
}

impl<'a, S, X, F> Closure<'a, S, X, F>
where
    S: OrderedStepper,
    X: Send,
    F: Fn(
            &S::Key,
            &OrderedPosition<S::Priority>,
            OrderedStep<S::Key>,
            &mut Vec<(S::Key, OrderedPosition<S::Priority>)>,
        ) -> X
        + Sync
        + Send,
{
    /// The step of `key`; spawns the steps of its new window children.
    fn step<'scope>(
        &'scope self,
        scope: &rayon::Scope<'scope>,
        key: S::Key,
        position: OrderedPosition<S::Priority>,
    ) -> Detached<S::Key, S::Priority, X, S::Error, S::Account>
    where
        'a: 'scope,
    {
        let (detached, found) = self.budget.run_captured(|| {
            let _timer = self
                .counters
                .map(|counters| CpuTimer::new(&counters.step_nanos));
            let (result, account) = self.stepper.step(&key);
            let mut found = Vec::new();
            let result = result.map(|step| (self.detach)(&key, &position, step, &mut found));
            (
                Detached {
                    key,
                    position,
                    result,
                    account,
                },
                found,
            )
        });
        for (child, position) in found {
            if lock(&self.seen).insert(child.clone()) {
                scope.spawn(move |scope| {
                    let step = self.step(scope, child, position);
                    lock(&self.discovered).push(step);
                });
            }
        }
        detached
    }
}

/// Steps computed ahead by the workers for the sequential scheduler: the
/// window of the greatest pending keys of the phase, closed under same-phase
/// descent above its lowest key. Any key whose step was not computed ahead
/// starts a new window.
pub(super) struct Prefetched<'w, K: ConcurrentOrderedKernel> {
    workers: &'w Workers,
    window_keys: usize,
    steps: HashMap<K::Key, (Result<OrderedStep<K::Key>, K::Error>, K::Account)>,
}

impl<'w, K: ConcurrentOrderedKernel> Prefetched<'w, K> {
    pub(super) fn new(workers: &'w Workers, window_keys: usize) -> Self {
        Self {
            workers,
            window_keys,
            steps: HashMap::new(),
        }
    }

    fn refill<W>(
        &mut self,
        kernel: &K,
        frontier: &Frontier<K::Key, K::Priority, W>,
        node: &Scheduled<K::Key, K::Priority>,
    ) -> Result<(), K::Error> {
        let phase = node.phase.0;
        // Every key computed ahead is either popped or dead by now.
        self.steps.clear();
        let mut roots = vec![(
            node.key.clone(),
            OrderedPosition {
                phase,
                priority: node.priority.clone(),
            },
        )];
        let mut threshold = node;
        for (scheduled, _) in frontier
            .iter()
            .take_while(|(scheduled, _)| scheduled.phase.0 == phase)
            .take(self.window_keys.saturating_sub(1))
        {
            roots.push((
                scheduled.key.clone(),
                OrderedPosition {
                    phase,
                    priority: scheduled.priority.clone(),
                },
            ));
            threshold = scheduled;
        }
        let stepper = kernel
            .stepper()
            .ok_or_else(|| invariant("concurrent ordered adapter lost its stepper"))?;
        let detached = close_window(
            self.workers,
            &stepper,
            roots,
            None,
            |parent, _, step, found| {
                for child in step.terms.keys() {
                    if stepper.later_phase(parent, child) {
                        continue;
                    }
                    if let Ok(position) = stepper.position(child)
                        && at_or_above(&position, child, threshold)
                    {
                        found.push((child.clone(), position));
                    }
                }
                step
            },
        );
        for step in detached {
            self.steps.insert(step.key, (step.result, step.account));
        }
        Ok(())
    }
}

impl<K: ConcurrentOrderedKernel> StepSource<K> for Prefetched<'_, K> {
    fn step<W>(
        &mut self,
        kernel: &mut K,
        frontier: &Frontier<K::Key, K::Priority, W>,
        node: &Scheduled<K::Key, K::Priority>,
        request: &mut ReductionRequest,
        work: &mut ReductionStatistics,
    ) -> Result<OrderedStep<K::Key>, K::Error> {
        if !self.steps.contains_key(&node.key) {
            self.refill(kernel, frontier, node)?;
        }
        let (result, account) = self
            .steps
            .remove(&node.key)
            .ok_or_else(|| invariant("a prefetched ordered step is missing"))?;
        kernel.absorb(&node.key, account, request, work)?;
        result
    }
}

/// The accounting call the sequential scheduler makes before an operation
/// that failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Accounted {
    Nothing,
    /// The contribution was counted as emitted (terminal contributions are
    /// counted only once their product is known to be nonzero).
    Emitted,
    Coalescing,
    RetainedFrame,
}

/// What one contribution did to its destination, in sequential order.
#[derive(Debug)]
enum Outcome<E> {
    Merged {
        old: usize,
        new: usize,
    },
    Cancelled {
        old: usize,
    },
    Inserted {
        new: usize,
    },
    /// A zero contribution to an absent key: nothing is stored.
    Dropped,
    Failed {
        error: E,
        accounted: Accounted,
    },
}

fn failed<E: From<ReductionError>>(error: ReductionError, accounted: Accounted) -> Outcome<E> {
    Outcome::Failed {
        error: error.into(),
        accounted,
    }
}

/// Destination of one child contribution of a detached step.
enum Target<P> {
    /// An in-window key (index into the window's nodes).
    Window(u32),
    /// A key outside the window: index of its frontier item, and the
    /// certified position when it was computed during the window's closure.
    /// `later` records the adapter's claim that the key is in a later phase;
    /// it is verified against the key's position when that is known.
    Frontier {
        item: u32,
        position: Option<OrderedPosition<P>>,
        later: bool,
    },
    /// The certified position does not descend from the parent: the
    /// sequential scheduler fails at this contribution.
    Violation(ReductionError),
}

const UNRESOLVED: u32 = u32::MAX;

enum Edge<K, P, C> {
    /// A zero step coefficient, skipped before it is emitted.
    Skip,
    Live {
        key: K,
        coefficient: Result<C, ReductionError>,
        target: Target<P>,
    },
}

struct TerminalEdge<C> {
    key: IntegralKey,
    coefficient: Result<C, ReductionError>,
    item: u32,
}

/// A step prepared for concurrent merges.
struct Expansion<K, P, C> {
    children: Vec<Edge<K, P, C>>,
    terminals: Vec<TerminalEdge<C>>,
    conditions: Vec<Coefficient>,
}

/// The state of a window key once all of its window parents have settled.
struct Settled<W> {
    /// The weight at the key's turn; `Some` iff the sequential scheduler pops
    /// the key.
    weight: Option<W>,
    /// Census of `weight`.
    monomials: Result<usize, ReductionError>,
}

/// One window key. Step failures and accounts are kept beside the nodes,
/// which are shared by the workers.
struct Node<K, P, W, C, E> {
    key: K,
    position: OrderedPosition<P>,
    rank: u32,
    initial: Mutex<Option<W>>,
    /// `None` when the step failed.
    expansion: Option<Expansion<K, P, C>>,
    /// Window parents not yet settled.
    remaining: AtomicU32,
    /// Window contributions received so far: (parent node, edge).
    incoming: Mutex<Vec<(u32, u32)>>,
    settled: OnceLock<Settled<W>>,
    /// Window contributions into the key, in sequential order: (parent node,
    /// edge, outcome).
    outcomes: Mutex<Vec<(u32, u32, Outcome<E>)>>,
}

/// One key outside the window that receives contributions from it.
struct FrontierItem<K, P, W, E> {
    key: K,
    /// Some contribution was certified by the adapter as later-phase.
    later: bool,
    position: Option<OrderedPosition<P>>,
    pending: Option<Pending<P, W>>,
    contributions: Vec<(u32, u32)>,
    outcomes: Vec<Outcome<E>>,
    inserted: bool,
    /// Merge time (window statistics only).
    seconds: f64,
}

struct TerminalItem<W, E> {
    key: IntegralKey,
    existing: Option<W>,
    contributions: Vec<(u32, u32)>,
    outcomes: Vec<Outcome<E>>,
}

type NodeOf<K, A> = Node<
    <K as OrderedKernel>::Key,
    <K as OrderedKernel>::Priority,
    <A as OrderedWeights>::Weight,
    <A as ConcurrentWeights>::Prepared,
    <K as OrderedKernel>::Error,
>;

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The settled weight of an expanded parent.
fn parent_weight<K: ConcurrentOrderedKernel, A: ConcurrentWeights>(
    nodes: &[NodeOf<K, A>],
    parent: u32,
) -> &A::Weight {
    nodes[parent as usize]
        .settled
        .get()
        .and_then(|settled| settled.weight.as_ref())
        .expect("only expanded parents contribute")
}

fn edge_of<K: ConcurrentOrderedKernel, A: ConcurrentWeights>(
    nodes: &[NodeOf<K, A>],
    parent: u32,
    edge: u32,
) -> (
    &Result<A::Prepared, ReductionError>,
    &OrderedPosition<K::Priority>,
) {
    let node = &nodes[parent as usize];
    let Some(expansion) = &node.expansion else {
        unreachable!("only expanded parents contribute")
    };
    let Edge::Live { coefficient, .. } = &expansion.children[edge as usize] else {
        unreachable!("skipped edges carry no contribution")
    };
    (coefficient, &node.position)
}

/// Apply one contribution to a frontier key exactly as the sequential
/// scheduler does (`run_inner`), recording what happened.
#[allow(clippy::too_many_arguments)]
fn merge_child<S: OrderedStepper, A: ConcurrentWeights>(
    stepper: &S,
    weights: &A,
    state: &mut Option<Pending<S::Priority, A::Weight>>,
    key: &S::Key,
    position: &mut Option<OrderedPosition<S::Priority>>,
    parent_weight: &A::Weight,
    parent_priority: &S::Priority,
    coefficient: &Result<A::Prepared, ReductionError>,
    phase: usize,
    total_phases: usize,
) -> Outcome<S::Error> {
    match state {
        Some(pending) => {
            if let Err(error) =
                check_descent(&pending.position, phase, parent_priority, total_phases)
            {
                return failed(error, Accounted::Nothing);
            }
            let contribution = match coefficient
                .clone_error()
                .and_then(|coefficient| weights.scale_prepared(parent_weight, coefficient))
            {
                Ok(contribution) => contribution,
                Err(error) => return failed(error, Accounted::Nothing),
            };
            let merged = weights.monomials(&pending.weight).and_then(|old| {
                let zero = weights.merge_detached(&mut pending.weight, contribution)?;
                let new = if zero {
                    0
                } else {
                    weights.monomials(&pending.weight)?
                };
                Ok((old, zero, new))
            });
            match merged {
                Ok((old, true, _)) => {
                    *position = state.take().map(|pending| pending.position);
                    Outcome::Cancelled { old }
                }
                Ok((old, false, new)) => Outcome::Merged { old, new },
                Err(error) => failed(error, Accounted::Coalescing),
            }
        }
        None => {
            let certified = match position.take() {
                Some(certified) => certified,
                None => match stepper.position(key) {
                    Ok(certified) => certified,
                    Err(error) => {
                        return Outcome::Failed {
                            error,
                            accounted: Accounted::Nothing,
                        };
                    }
                },
            };
            if let Err(error) = check_descent(&certified, phase, parent_priority, total_phases) {
                *position = Some(certified);
                return failed(error, Accounted::Nothing);
            }
            let contribution = match coefficient
                .clone_error()
                .and_then(|coefficient| weights.scale_prepared(parent_weight, coefficient))
            {
                Ok(contribution) => contribution,
                Err(error) => {
                    *position = Some(certified);
                    return failed(error, Accounted::Nothing);
                }
            };
            if weights.contribution_is_zero(&contribution) {
                *position = Some(certified);
                return Outcome::Dropped;
            }
            let inserted = weights.insert_detached(contribution).and_then(|weight| {
                let new = weights.monomials(&weight)?;
                Ok((weight, new))
            });
            match inserted {
                Ok((weight, new)) => {
                    *state = Some(Pending {
                        position: certified,
                        weight,
                    });
                    Outcome::Inserted { new }
                }
                Err(error) => {
                    *position = Some(certified);
                    failed(error, Accounted::RetainedFrame)
                }
            }
        }
    }
}

/// Contributions per block of `merge_prefix`.
const PREFIX_BLOCK: usize = 16;

/// Merge the leading contributions of one heavily targeted frontier key on
/// several threads, reproducing its sequential states exactly.
///
/// The sequential states are the prefix sums `P_j = E + x_1 + ... + x_j` of
/// the existing weight `E` and the products `x_i`, merged in sequential
/// order. With associative, canonical sums (`CANONICAL_SUMS`) every `P_j` can
/// be formed as `O_b + L_{b,j}`, where `L_{b,j}` are running sums inside block
/// `b` and `O_b = E + (sums of the earlier blocks)`: the products, the block
/// running sums and the final `P_j` are computed concurrently, the block
/// offsets serially. Each `P_j` equals the sequential state as a value and as
/// a representation, so its zero test and census, hence every outcome, are
/// those of the sequential merge. Chunks of `PREFIX_BLOCK * threads`
/// contributions bound the partial sums held at once.
///
/// Returns the number of contributions merged; the caller merges the rest
/// sequentially (the first contribution that would fail, and everything
/// after an arithmetic error here, is left to the sequential merge, which then
/// reproduces the sequential error).
fn merge_prefix<K, A, S>(
    stepper: &S,
    weights: &A,
    budget: &ResourceBudget,
    nodes: &[NodeOf<K, A>],
    item: &mut FrontierItem<K::Key, K::Priority, A::Weight, K::Error>,
    phase: usize,
    total_phases: usize,
) -> usize
where
    K: ConcurrentOrderedKernel,
    A: ConcurrentWeights,
    S: OrderedStepper<Key = K::Key, Priority = K::Priority, Error = K::Error>,
{
    // The key's certified position (stored, cached, or computed once).
    let position = match (&item.pending, &item.position) {
        (Some(pending), _) => pending.position.clone(),
        (None, Some(position)) => position.clone(),
        (None, None) => match stepper.position(&item.key) {
            Ok(position) => position,
            Err(_) => return 0,
        },
    };
    // Contributions that pass the checks made before any arithmetic.
    let clean = item
        .contributions
        .iter()
        .take_while(|&&(parent, edge)| {
            let (coefficient, parent_position) = edge_of::<K, A>(nodes, parent, edge);
            coefficient.is_ok()
                && check_descent(&position, phase, &parent_position.priority, total_phases).is_ok()
        })
        .count();
    let chunk = PREFIX_BLOCK * rayon::current_num_threads().max(2);
    let mut merged = 0;
    while clean - merged >= 2 * PREFIX_BLOCK {
        let end = clean.min(merged + chunk);
        let Some(states) = prefix_states::<K, A>(
            weights,
            budget,
            nodes,
            &item.contributions[merged..end],
            item.pending.as_ref().map(|pending| &pending.weight),
        ) else {
            break;
        };
        // Outcomes from the zero tests and censuses of the states.
        let mut present = item
            .pending
            .as_ref()
            .map(|pending| weights.monomials(&pending.weight));
        let mut outcomes = Vec::with_capacity(states.census.len());
        for &(zero, monomials) in &states.census {
            outcomes.push(match present {
                Some(Ok(old)) if zero => {
                    present = None;
                    Outcome::Cancelled { old }
                }
                Some(Ok(old)) => {
                    present = Some(Ok(monomials));
                    Outcome::Merged {
                        old,
                        new: monomials,
                    }
                }
                None if !zero => {
                    present = Some(Ok(monomials));
                    Outcome::Inserted { new: monomials }
                }
                // A zero product into an absent key, or a census error:
                // leave this chunk to the sequential merge.
                _ => break,
            });
        }
        if outcomes.len() != states.census.len() {
            break;
        }
        item.inserted |= outcomes
            .iter()
            .any(|outcome| matches!(outcome, Outcome::Inserted { .. }));
        item.outcomes.extend(outcomes);
        item.pending = match present {
            Some(_) => Some(Pending {
                position: position.clone(),
                weight: states.last,
            }),
            None => None,
        };
        item.position = Some(position.clone());
        merged = end;
    }
    #[cfg(test)]
    PREFIX_MERGED.fetch_add(merged, Ordering::Relaxed);
    merged
}

/// Contributions merged by `merge_prefix` (tests only).
#[cfg(test)]
static PREFIX_MERGED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// The sequential states of one destination after each contribution.
struct PrefixStates<W> {
    /// Zero test and census of each state.
    census: Vec<(bool, usize)>,
    /// The last state.
    last: W,
}

/// `E + x_1 + ... + x_j` for every contribution `j` of `contributions`, by
/// block running sums (see `merge_prefix`). `None` on any arithmetic error
/// or zero product.
fn prefix_states<K, A>(
    weights: &A,
    budget: &ResourceBudget,
    nodes: &[NodeOf<K, A>],
    contributions: &[(u32, u32)],
    existing: Option<&A::Weight>,
) -> Option<PrefixStates<A::Weight>>
where
    K: ConcurrentOrderedKernel,
    A: ConcurrentWeights,
{
    // Block running sums of the products, concurrently per block.
    let blocks: Vec<Option<Vec<A::Weight>>> = contributions
        .par_chunks(PREFIX_BLOCK)
        .with_max_len(1)
        .map(|block| {
            budget.run_captured(|| {
                let mut running: Vec<A::Weight> = Vec::with_capacity(block.len());
                for &(parent, edge) in block {
                    let (coefficient, _) = edge_of::<K, A>(nodes, parent, edge);
                    let coefficient = coefficient.as_ref().ok()?;
                    let product = weights
                        .scale_prepared(parent_weight::<K, A>(nodes, parent), coefficient)
                        .ok()?;
                    if weights.contribution_is_zero(&product) {
                        return None;
                    }
                    let product = weights.insert_detached(product).ok()?;
                    let sum = match running.last() {
                        Some(previous) => weights.sum_weights(previous, &product).ok()?,
                        None => product,
                    };
                    running.push(sum);
                }
                Some(running)
            })
        })
        .collect();
    let blocks: Vec<Vec<A::Weight>> = blocks.into_iter().collect::<Option<_>>()?;
    // Block offsets, serially: O_0 = E, O_{b+1} = O_b + (block b total).
    let mut offsets: Vec<Option<A::Weight>> = Vec::with_capacity(blocks.len() + 1);
    offsets.push(existing.cloned());
    for block in &blocks {
        let total = block.last()?;
        let next = match offsets.last()? {
            Some(offset) => budget
                .run_captured(|| weights.sum_weights(offset, total))
                .ok()?,
            None => total.clone(),
        };
        offsets.push(Some(next));
    }
    // Every state P = O_b + L_{b,i}, concurrently; a block's last state is
    // the next offset.
    let census: Vec<Option<Vec<(bool, usize)>>> = blocks
        .par_iter()
        .with_max_len(1)
        .enumerate()
        .map(|(b, block)| {
            budget.run_captured(|| {
                let census = |state: &A::Weight| -> Option<(bool, usize)> {
                    if weights.weight_is_zero(state) {
                        Some((true, 0))
                    } else {
                        Some((false, weights.monomials(state).ok()?))
                    }
                };
                let mut out = Vec::with_capacity(block.len());
                for running in &block[..block.len() - 1] {
                    let state = match &offsets[b] {
                        Some(offset) => weights.sum_weights(offset, running).ok()?,
                        None => running.clone(),
                    };
                    out.push(census(&state)?);
                }
                out.push(census(offsets[b + 1].as_ref()?)?);
                Some(out)
            })
        })
        .collect();
    let census = census
        .into_iter()
        .collect::<Option<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect();
    Some(PrefixStates {
        census,
        last: offsets.pop()??,
    })
}

fn merge_terminal<A: ConcurrentWeights, E: From<ReductionError>>(
    weights: &A,
    state: &mut Option<A::Weight>,
    parent_weight: &A::Weight,
    coefficient: &Result<A::Prepared, ReductionError>,
) -> Outcome<E> {
    let contribution = match coefficient
        .clone_error()
        .and_then(|coefficient| weights.scale_prepared(parent_weight, coefficient))
    {
        Ok(contribution) => contribution,
        Err(error) => return failed(error, Accounted::Nothing),
    };
    if weights.contribution_is_zero(&contribution) {
        return Outcome::Dropped;
    }
    match state {
        Some(existing) => {
            let merged = weights.monomials(existing).and_then(|old| {
                let zero = weights.merge_terminal_detached(existing, contribution)?;
                let new = if zero {
                    0
                } else {
                    weights.monomials(existing)?
                };
                Ok((old, zero, new))
            });
            match merged {
                Ok((old, true, _)) => {
                    *state = None;
                    Outcome::Cancelled { old }
                }
                Ok((old, false, new)) => Outcome::Merged { old, new },
                Err(error) => failed(error, Accounted::Coalescing),
            }
        }
        None => {
            let inserted = weights
                .insert_terminal_detached(contribution)
                .and_then(|weight| {
                    let new = weights.monomials(&weight)?;
                    Ok((weight, new))
                });
            match inserted {
                Ok((weight, new)) => {
                    *state = Some(weight);
                    Outcome::Inserted { new }
                }
                Err(error) => failed(error, Accounted::Emitted),
            }
        }
    }
}

/// Borrow the value of a prepared coefficient, or clone its (cheap) error.
trait CloneError<C> {
    fn clone_error(&self) -> Result<&C, ReductionError>;
}

impl<C> CloneError<C> for Result<C, ReductionError> {
    fn clone_error(&self) -> Result<&C, ReductionError> {
        self.as_ref().map_err(Clone::clone)
    }
}

/// Settle one window key: merge its window contributions in sequential
/// order, then release its window children.
fn settle<'scope, K, A, S>(
    scope: &rayon::Scope<'scope>,
    context: &'scope WindowContext<'scope, K, A, S>,
    index: u32,
) where
    K: ConcurrentOrderedKernel,
    A: ConcurrentWeights,
    S: OrderedStepper<Key = K::Key, Priority = K::Priority, Error = K::Error>,
{
    let _timer = context
        .counters
        .map(|counters| CpuTimer::new(&counters.settle_nanos));
    let nodes = context.nodes;
    let node = &nodes[index as usize];
    let mut incoming = std::mem::take(&mut *lock(&node.incoming));
    incoming.sort_unstable_by_key(|&(parent, edge)| (nodes[parent as usize].rank, edge));
    let mut state = lock(&node.initial).take().map(|weight| Pending {
        position: node.position.clone(),
        weight,
    });
    let mut failure = false;
    let mut outcomes = Vec::with_capacity(incoming.len());
    for (parent, edge) in incoming {
        let (coefficient, parent_position) = edge_of::<K, A>(nodes, parent, edge);
        let mut position = Some(node.position.clone());
        let outcome = context.budget.run_captured(|| {
            merge_child(
                context.stepper,
                context.weights,
                &mut state,
                &node.key,
                &mut position,
                parent_weight::<K, A>(nodes, parent),
                &parent_position.priority,
                coefficient,
                context.phase,
                context.total_phases,
            )
        });
        failure = matches!(outcome, Outcome::Failed { .. });
        outcomes.push((parent, edge, outcome));
        if failure {
            break;
        }
    }
    let weight = if failure {
        None
    } else {
        state.map(|pending| pending.weight)
    };
    let monomials = weight
        .as_ref()
        .map_or(Ok(0), |weight| context.weights.monomials(weight));
    let expands = weight.is_some() && node.expansion.is_some();
    *lock(&node.outcomes) = outcomes;
    if node.settled.set(Settled { weight, monomials }).is_err() {
        unreachable!("an ordered window key settles exactly once");
    }
    let Some(expansion) = &node.expansion else {
        return;
    };
    for (edge, child) in expansion.children.iter().enumerate() {
        let Edge::Live {
            target: Target::Window(child),
            ..
        } = child
        else {
            continue;
        };
        let child_node = &nodes[*child as usize];
        if expands {
            lock(&child_node.incoming).push((index, edge as u32));
        }
        if child_node.remaining.fetch_sub(1, Ordering::AcqRel) == 1 {
            let child = *child;
            scope.spawn(move |scope| settle(scope, context, child));
        }
    }
}

/// Shared read-only state of one window's dataflow.
struct WindowContext<'a, K: ConcurrentOrderedKernel, A: ConcurrentWeights, S> {
    nodes: &'a [NodeOf<K, A>],
    stepper: &'a S,
    weights: &'a A,
    budget: &'a ResourceBudget,
    phase: usize,
    total_phases: usize,
    counters: Option<&'a WindowCounters>,
}

/// `RUSTRED_ORDERED_WINDOW_STATS=<file>` appends one JSON line per concurrent
/// window (stage wall times, sizes, the heaviest destination). Diagnostic
/// instrumentation only; no effect on results.
fn window_stats_path() -> Option<&'static std::path::Path> {
    static PATH: OnceLock<Option<std::path::PathBuf>> = OnceLock::new();
    PATH.get_or_init(|| std::env::var_os("RUSTRED_ORDERED_WINDOW_STATS").map(Into::into))
        .as_deref()
}

struct WindowClock {
    last: Option<std::time::Instant>,
    laps: Vec<(&'static str, f64)>,
    counters: WindowCounters,
}

/// Worker-side totals of one window (statistics only).
#[derive(Default)]
struct WindowCounters {
    discovered: std::sync::atomic::AtomicU64,
    step_nanos: std::sync::atomic::AtomicU64,
    settle_nanos: std::sync::atomic::AtomicU64,
}

/// Adds the elapsed nanoseconds to a counter when dropped.
struct CpuTimer<'a>(std::time::Instant, &'a std::sync::atomic::AtomicU64);

impl<'a> CpuTimer<'a> {
    fn new(counter: &'a std::sync::atomic::AtomicU64) -> Self {
        Self(std::time::Instant::now(), counter)
    }
}

impl Drop for CpuTimer<'_> {
    fn drop(&mut self) {
        let nanos = u64::try_from(self.0.elapsed().as_nanos()).unwrap_or(u64::MAX);
        self.1.fetch_add(nanos, Ordering::Relaxed);
    }
}

impl WindowClock {
    fn start() -> Self {
        Self {
            last: window_stats_path().map(|_| std::time::Instant::now()),
            laps: Vec::new(),
            counters: WindowCounters::default(),
        }
    }
    fn counters(&self) -> Option<&WindowCounters> {
        self.enabled().then_some(&self.counters)
    }
    fn enabled(&self) -> bool {
        self.last.is_some()
    }
    fn lap(&mut self, stage: &'static str) {
        if let Some(last) = &mut self.last {
            let now = std::time::Instant::now();
            self.laps.push((stage, (now - *last).as_secs_f64()));
            *last = now;
        }
    }
    fn report<K, P, W, C, E>(
        &self,
        phase: usize,
        nodes: &[Node<K, P, W, C, E>],
        items: &[(usize, f64)],
        contributions: usize,
        terminal_items: usize,
    ) {
        use std::io::Write;
        let Some(path) = window_stats_path() else {
            return;
        };
        let expanded = nodes
            .iter()
            .filter(|node| node.settled.get().is_some_and(|s| s.weight.is_some()))
            .count();
        let heaviest = items
            .iter()
            .copied()
            .fold((0, 0.0), |a, b| if b.1 > a.1 { b } else { a });
        let merge_cpu: f64 = items.iter().map(|item| item.1).sum();
        let max_fan_in = items.iter().map(|item| item.0).max().unwrap_or(0);
        let mut laps: Vec<String> = self
            .laps
            .iter()
            .map(|(stage, seconds)| format!("\"{stage}\":{seconds:.6}"))
            .collect();
        let seconds =
            |counter: &std::sync::atomic::AtomicU64| counter.load(Ordering::Relaxed) as f64 * 1e-9;
        laps.push(format!(
            "\"discovered\":{},\"step_cpu\":{:.6},\"settle_cpu\":{:.6}",
            self.counters.discovered.load(Ordering::Relaxed),
            seconds(&self.counters.step_nanos),
            seconds(&self.counters.settle_nanos)
        ));
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            let _ = writeln!(
                file,
                "{{\"phase\":{phase},\"nodes\":{},\"expanded\":{expanded},\"items\":{},\
                 \"contributions\":{contributions},\"terminal_items\":{terminal_items},\
                 \"max_fan_in\":{max_fan_in},\"merge_cpu\":{merge_cpu:.6},\
                 \"heaviest_fan_in\":{},\"heaviest_seconds\":{:.6},{}}}",
                nodes.len(),
                items.len(),
                heaviest.0,
                heaviest.1,
                laps.join(",")
            );
        }
    }
}

/// The concurrent scheduler for value arithmetic. See the module docs.
#[allow(clippy::too_many_arguments)]
pub(super) fn run_concurrent<K: ConcurrentOrderedKernel, A: ConcurrentWeights>(
    kernel: &mut K,
    weights: &mut A,
    request: &mut ReductionRequest,
    initial: BTreeMap<K::Key, Coefficient>,
    nonzero_conditions: Vec<Coefficient>,
    work: &mut ReductionStatistics,
    workers: &Workers,
    window_keys: usize,
) -> Result<RawOutput<A::Weight>, K::Error> {
    let mut state = admit(kernel, weights, request, initial, nonzero_conditions)?;
    for phase in 0..state.progress.total_phases {
        state.inspect(kernel, OrderedPhaseBoundary::Started, phase)?;
        while run_window(
            kernel,
            weights,
            request,
            work,
            workers,
            window_keys,
            &mut state,
            phase,
        )? {}
        state.finish_phase(kernel, phase)?;
    }
    state.finish(work)
}

/// Process one window of `phase`; `false` when the phase has drained.
#[allow(clippy::too_many_arguments)]
fn run_window<K: ConcurrentOrderedKernel, A: ConcurrentWeights>(
    kernel: &mut K,
    weights: &A,
    request: &mut ReductionRequest,
    work: &mut ReductionStatistics,
    workers: &Workers,
    window_keys: usize,
    state: &mut Admitted<K, A>,
    phase: usize,
) -> Result<bool, K::Error> {
    let total_phases = state.progress.total_phases;
    let mut clock = WindowClock::start();
    let frontier_keys_before = state.frontier.len();
    let mut taken = Vec::new();
    while taken.len() < window_keys {
        let Some(popped) = state.frontier.pop_in_phase(phase) else {
            break;
        };
        taken.push(popped);
    }
    let Some((threshold, _)) = taken.last() else {
        return Ok(false);
    };
    let threshold = threshold.clone();
    let roots = taken
        .iter()
        .map(|(scheduled, _)| {
            (
                scheduled.key.clone(),
                OrderedPosition {
                    phase,
                    priority: scheduled.priority.clone(),
                },
            )
        })
        .collect();
    // 1. Steps of the window and of its same-phase descendants above the
    //    threshold, prepared for the arithmetic and classified per child.
    let mut accounts = Vec::new();
    let mut failures = Vec::new();
    let mut nodes: Vec<NodeOf<K, A>> = {
        let stepper = kernel
            .stepper()
            .ok_or_else(|| invariant("concurrent ordered adapter lost its stepper"))?;
        let threshold = &threshold;
        let shared: &A = weights;
        let detached = close_window(
            workers,
            &stepper,
            roots,
            clock.counters(),
            |parent, position, step, found| {
                let children = step
                    .terms
                    .into_iter()
                    .map(|(child, coefficient)| {
                        if coefficient.is_zero() {
                            return Edge::Skip;
                        }
                        let coefficient = shared.prepare(coefficient);
                        let target = if stepper.later_phase(parent, &child) {
                            Target::Frontier {
                                item: UNRESOLVED,
                                position: None,
                                later: true,
                            }
                        } else {
                            match stepper.position(&child) {
                                // Only a vacant key asks for its position; the
                                // sequential error is reproduced there.
                                Err(_) => Target::Frontier {
                                    item: UNRESOLVED,
                                    position: None,
                                    later: false,
                                },
                                Ok(certified) => match check_descent(
                                    &certified,
                                    phase,
                                    &position.priority,
                                    total_phases,
                                ) {
                                    Err(error) => Target::Violation(error),
                                    Ok(()) if at_or_above(&certified, &child, threshold) => {
                                        found.push((child.clone(), certified));
                                        Target::Window(UNRESOLVED)
                                    }
                                    Ok(()) => Target::Frontier {
                                        item: UNRESOLVED,
                                        position: Some(certified),
                                        later: false,
                                    },
                                },
                            }
                        };
                        Edge::Live {
                            key: child,
                            coefficient,
                            target,
                        }
                    })
                    .collect();
                let terminals = step
                    .terminals
                    .into_iter()
                    .map(|(key, coefficient)| TerminalEdge {
                        key,
                        coefficient: shared.prepare(coefficient),
                        item: UNRESOLVED,
                    })
                    .collect();
                Expansion {
                    children,
                    terminals,
                    conditions: step.nonzero_conditions,
                }
            },
        );
        let mut initial = taken.into_iter().map(|(_, weight)| weight);
        detached
            .into_iter()
            .map(|step| {
                let (expansion, failure) = match step.result {
                    Ok(expansion) => (Some(expansion), None),
                    Err(error) => (None, Some(error)),
                };
                accounts.push(Some(step.account));
                failures.push(failure);
                Node {
                    key: step.key,
                    position: step.position,
                    rank: 0,
                    initial: Mutex::new(initial.next()),
                    expansion,
                    remaining: AtomicU32::new(0),
                    incoming: Mutex::new(Vec::new()),
                    settled: OnceLock::new(),
                    outcomes: Mutex::new(Vec::new()),
                }
            })
            .collect()
    };
    if nodes.len() >= UNRESOLVED as usize {
        return Err(execution("ordered window exceeds 2^32 keys").into());
    }
    // Pop order of the window keys: all of them are in `phase`.
    let mut order: Vec<u32> = (0..nodes.len() as u32).collect();
    order.sort_unstable_by(|&a, &b| {
        let (a, b) = (&nodes[a as usize], &nodes[b as usize]);
        (&b.position.priority, &b.key).cmp(&(&a.position.priority, &a.key))
    });
    for (rank, &index) in order.iter().enumerate() {
        nodes[index as usize].rank = rank as u32;
    }
    // Resolve window edges and count window parents.
    {
        let index: HashMap<&K::Key, u32> = nodes
            .iter()
            .enumerate()
            .map(|(i, node)| (&node.key, i as u32))
            .collect();
        let mut resolved = Vec::new();
        for (parent, node) in nodes.iter().enumerate() {
            let Some(expansion) = &node.expansion else {
                continue;
            };
            for (edge, child) in expansion.children.iter().enumerate() {
                if let Edge::Live {
                    key,
                    target: Target::Window(_),
                    ..
                } = child
                {
                    let child = *index
                        .get(key)
                        .ok_or_else(|| invariant("ordered window child was not closed"))?;
                    resolved.push((parent, edge, child));
                }
            }
        }
        for (parent, edge, child) in resolved {
            if let Some(expansion) = &mut nodes[parent].expansion
                && let Edge::Live { target, .. } = &mut expansion.children[edge]
            {
                *target = Target::Window(child);
            }
            *nodes[child as usize].remaining.get_mut() += 1;
        }
    }
    clock.lap("steps");
    // 2. Settle the window keys as their window parents settle.
    {
        let stepper = kernel
            .stepper()
            .ok_or_else(|| invariant("concurrent ordered adapter lost its stepper"))?;
        let context = WindowContext::<K, A, _> {
            nodes: &nodes,
            stepper: &stepper,
            weights,
            budget: &workers.budget,
            phase,
            total_phases,
            counters: clock.counters(),
        };
        let context = &context;
        // Roots are collected before any key settles: settling releases
        // children concurrently, and each key must settle exactly once.
        let roots: Vec<u32> = order
            .iter()
            .copied()
            .filter(|&index| {
                context.nodes[index as usize]
                    .remaining
                    .load(Ordering::Acquire)
                    == 0
            })
            .collect();
        workers.pool.install(|| {
            rayon::scope(|scope| {
                for index in roots {
                    scope.spawn(move |scope| settle(scope, context, index));
                }
            })
        });
    }
    if nodes.iter().any(|node| node.settled.get().is_none()) {
        return Err(invariant("ordered window dataflow left a key unsettled").into());
    }
    clock.lap("dataflow");
    // 3. Contributions leaving the window, grouped per destination in
    //    sequential order, merged concurrently across destinations.
    let mut items: Vec<FrontierItem<K::Key, K::Priority, A::Weight, K::Error>> = Vec::new();
    let mut terminal_items: Vec<TerminalItem<A::Weight, K::Error>> = Vec::new();
    {
        let mut item_index: HashMap<K::Key, u32> = HashMap::new();
        let mut terminal_index: HashMap<IntegralKey, u32> = HashMap::new();
        for &parent in &order {
            let node = &mut nodes[parent as usize];
            let expands = node
                .settled
                .get()
                .is_some_and(|settled| settled.weight.is_some());
            let Some(expansion) = &mut node.expansion else {
                continue;
            };
            if !expands {
                continue;
            }
            for (edge, child) in expansion.children.iter_mut().enumerate() {
                let Edge::Live {
                    key,
                    target:
                        Target::Frontier {
                            item,
                            position,
                            later,
                        },
                    ..
                } = child
                else {
                    continue;
                };
                let index = match item_index.entry(key.clone()) {
                    Entry::Occupied(entry) => *entry.get(),
                    Entry::Vacant(entry) => {
                        let index = items.len() as u32;
                        items.push(FrontierItem {
                            key: entry.key().clone(),
                            later: false,
                            position: None,
                            pending: None,
                            contributions: Vec::new(),
                            outcomes: Vec::new(),
                            inserted: false,
                            seconds: 0.0,
                        });
                        entry.insert(index);
                        index
                    }
                };
                *item = index;
                let destination = &mut items[index as usize];
                destination.later |= *later;
                if destination.position.is_none() {
                    destination.position = position.clone();
                }
                destination.contributions.push((parent, edge as u32));
            }
            for (edge, terminal) in expansion.terminals.iter_mut().enumerate() {
                let index = match terminal_index.entry(terminal.key.clone()) {
                    Entry::Occupied(entry) => *entry.get(),
                    Entry::Vacant(entry) => {
                        let index = terminal_items.len() as u32;
                        terminal_items.push(TerminalItem {
                            key: entry.key().clone(),
                            existing: None,
                            contributions: Vec::new(),
                            outcomes: Vec::new(),
                        });
                        entry.insert(index);
                        index
                    }
                };
                terminal.item = index;
                terminal_items[index as usize]
                    .contributions
                    .push((parent, edge as u32));
            }
        }
    }
    let terminal_keys_before = state.output.len();
    for item in &mut items {
        item.pending = state.frontier.pending.remove(&item.key);
    }
    for item in &mut terminal_items {
        item.existing = state.output.remove(&item.key);
    }
    {
        let stepper = kernel
            .stepper()
            .ok_or_else(|| invariant("concurrent ordered adapter lost its stepper"))?;
        let nodes = &nodes;
        let stepper = &stepper;
        clock.lap("group");
        let threads = workers.pool.current_num_threads();
        let contributions_total: usize = items.iter().map(|item| item.contributions.len()).sum();
        workers.for_each(&mut items, |item| {
            let started = clock.enabled().then(std::time::Instant::now);
            'merge: {
                if item.later {
                    // The adapter placed this key outside the window without its
                    // position. A same-phase key would break the window's
                    // closure, so the claim is checked when the position is known
                    // (a pending key's stored one, or the one certified at its
                    // insertion below).
                    let stored = item.pending.as_ref().map(|pending| &pending.position);
                    if stored.is_some_and(|position| position.phase <= phase) {
                        item.outcomes.push(failed(
                            invariant("ordered adapter misplaced a same-phase child"),
                            Accounted::Nothing,
                        ));
                        break 'merge;
                    }
                }
                let heavy = A::CANONICAL_SUMS
                    && item.contributions.len() >= 2 * PREFIX_BLOCK
                    && item.contributions.len() * threads > contributions_total;
                let start = if heavy {
                    merge_prefix::<K, A, _>(
                        stepper,
                        weights,
                        &workers.budget,
                        nodes,
                        item,
                        phase,
                        total_phases,
                    )
                } else {
                    0
                };
                for &(parent, edge) in &item.contributions[start..] {
                    let (coefficient, parent_position) = edge_of::<K, A>(nodes, parent, edge);
                    let outcome = merge_child(
                        stepper,
                        weights,
                        &mut item.pending,
                        &item.key,
                        &mut item.position,
                        parent_weight::<K, A>(nodes, parent),
                        &parent_position.priority,
                        coefficient,
                        phase,
                        total_phases,
                    );
                    let misplaced = item.later
                        && matches!(outcome, Outcome::Inserted { .. })
                        && item
                            .pending
                            .as_ref()
                            .is_some_and(|pending| pending.position.phase <= phase);
                    if misplaced {
                        item.outcomes.push(failed(
                            invariant("ordered adapter misplaced a same-phase child"),
                            Accounted::Nothing,
                        ));
                        break;
                    }
                    item.inserted |= matches!(outcome, Outcome::Inserted { .. });
                    let failure = matches!(outcome, Outcome::Failed { .. });
                    item.outcomes.push(outcome);
                    if failure {
                        break;
                    }
                }
            }
            if let Some(started) = started {
                item.seconds += started.elapsed().as_secs_f64();
            }
        });
        workers.for_each(&mut terminal_items, |item| {
            for &(parent, edge) in &item.contributions {
                let node = &nodes[parent as usize];
                let Some(expansion) = &node.expansion else {
                    unreachable!("only expanded parents contribute")
                };
                let outcome = merge_terminal::<A, K::Error>(
                    weights,
                    &mut item.existing,
                    parent_weight::<K, A>(nodes, parent),
                    &expansion.terminals[edge as usize].coefficient,
                );
                let failure = matches!(outcome, Outcome::Failed { .. });
                item.outcomes.push(outcome);
                if failure {
                    break;
                }
            }
        });
    }
    clock.lap("merge");
    // 4. Serial scan in pop order.
    let scanned = scan(
        kernel,
        request,
        work,
        state,
        &mut nodes,
        &mut accounts,
        &mut failures,
        &order,
        &mut items,
        &mut terminal_items,
        frontier_keys_before,
        terminal_keys_before,
    );
    // Restore every destination (the request fails anyway on an error).
    let contributions: usize = items.iter().map(|item| item.contributions.len()).sum();
    let terminal_items_count = terminal_items.len();
    let items_seconds: Vec<(usize, f64)> = if clock.enabled() {
        items
            .iter()
            .map(|item| (item.contributions.len(), item.seconds))
            .collect()
    } else {
        Vec::new()
    };
    for item in items {
        let FrontierItem {
            key,
            position,
            pending,
            inserted,
            ..
        } = item;
        if inserted {
            let position = pending
                .as_ref()
                .map(|pending| &pending.position)
                .or(position.as_ref())
                .ok_or_else(|| invariant("an inserted ordered key has no position"))?;
            state.frontier.order.insert(Scheduled {
                phase: std::cmp::Reverse(position.phase),
                priority: position.priority.clone(),
                key: key.clone(),
            });
        }
        if let Some(pending) = pending {
            state.frontier.pending.insert(key, pending);
        }
    }
    for item in terminal_items {
        if let Some(existing) = item.existing {
            state.output.insert(item.key, existing);
        }
    }
    clock.lap("scan");
    clock.report(
        phase,
        &nodes,
        &items_seconds,
        contributions,
        terminal_items_count,
    );
    let (frontier_after, terminals_after) = scanned?;
    if frontier_after != state.frontier.len() || terminals_after != state.output.len() {
        return Err(
            invariant("concurrent ordered window diverged from its sequential census").into(),
        );
    }
    Ok(true)
}

/// Replay the sequential scheduler's per-key effects for the expanded window
/// keys, in pop order. Returns the frontier and terminal key counts after the
/// window, as the sequential scheduler would observe them.
#[allow(clippy::too_many_arguments)]
fn scan<K: ConcurrentOrderedKernel, A: ConcurrentWeights>(
    kernel: &mut K,
    request: &mut ReductionRequest,
    work: &mut ReductionStatistics,
    state: &mut Admitted<K, A>,
    nodes: &mut [NodeOf<K, A>],
    accounts: &mut [Option<K::Account>],
    failures: &mut [Option<K::Error>],
    order: &[u32],
    items: &mut [FrontierItem<K::Key, K::Priority, A::Weight, K::Error>],
    terminal_items: &mut [TerminalItem<A::Weight, K::Error>],
    mut frontier_keys: usize,
    mut terminal_keys: usize,
) -> Result<(usize, usize), K::Error> {
    let limits = kernel.limits();
    let coalescing_limit = limits.diagnostic_limit(limits.max_coalescing_additions);
    let frame_limit = limits.diagnostic_limit(limits.max_pending_frames);
    let mut window_cursor = vec![0usize; nodes.len()];
    let mut item_cursor = vec![0usize; items.len()];
    let mut terminal_cursor = vec![0usize; terminal_items.len()];
    // Window outcomes are consumed by value; take them out of the cells.
    let mut window_outcomes: Vec<Vec<(u32, u32, Outcome<K::Error>)>> = nodes
        .iter_mut()
        .map(|node| {
            std::mem::take(
                node.outcomes
                    .get_mut()
                    .unwrap_or_else(std::sync::PoisonError::into_inner),
            )
        })
        .collect();
    let mut taken_outcomes: Vec<std::vec::IntoIter<Outcome<K::Error>>> = items
        .iter_mut()
        .map(|item| std::mem::take(&mut item.outcomes).into_iter())
        .collect();
    let mut terminal_outcomes: Vec<std::vec::IntoIter<Outcome<K::Error>>> = terminal_items
        .iter_mut()
        .map(|item| std::mem::take(&mut item.outcomes).into_iter())
        .collect();
    let progress = &mut state.progress;
    for &index in order {
        let node = &mut nodes[index as usize];
        let Some(settled) = node.settled.get_mut() else {
            return Err(invariant("ordered window key was not settled").into());
        };
        if settled.weight.is_none() {
            continue;
        }
        let monomials = settled.monomials.clone();
        // Pop.
        request.release_pending_frame()?;
        replace_monomials(&mut progress.frontier_coefficient_monomials, monomials?, 0)?;
        frontier_keys = frontier_keys
            .checked_sub(1)
            .ok_or_else(|| invariant("ordered window frontier census underflow"))?;
        progress.frontier_keys = frontier_keys;
        kernel.observe(progress, Some(&node.key))?;
        let account = accounts[index as usize]
            .take()
            .ok_or_else(|| invariant("ordered window step account was consumed twice"))?;
        kernel.absorb(&node.key, account, request, work)?;
        let Some(expansion) = node.expansion.take() else {
            return Err(failures[index as usize]
                .take()
                .unwrap_or_else(|| invariant("ordered window step was consumed twice").into()));
        };
        for condition in &expansion.conditions {
            state
                .guards
                .intern(&mut state.nonzero_conditions, condition, |_| Ok(()))?;
        }
        for (edge, child) in expansion.children.into_iter().enumerate() {
            let target = match child {
                Edge::Skip => continue,
                Edge::Live { target, .. } => target,
            };
            progress.emitted_terms = progress.emitted_terms.saturating_add(1);
            let outcome = match target {
                Target::Violation(error) => failed(error, Accounted::Nothing),
                Target::Window(child) => {
                    let cursor = &mut window_cursor[child as usize];
                    let entries = &mut window_outcomes[child as usize];
                    let entry = entries.get_mut(*cursor).ok_or_else(|| {
                        invariant("ordered window contribution has no recorded outcome")
                    })?;
                    if (entry.0, entry.1) != (index, edge as u32) {
                        return Err(invariant(
                            "ordered window contributions were merged out of order",
                        )
                        .into());
                    }
                    *cursor += 1;
                    std::mem::replace(&mut entry.2, Outcome::Dropped)
                }
                Target::Frontier { item, .. } => {
                    let cursor = &mut item_cursor[item as usize];
                    if items[item as usize].contributions.get(*cursor)
                        != Some(&(index, edge as u32))
                    {
                        return Err(invariant(
                            "ordered frontier contributions were merged out of order",
                        )
                        .into());
                    }
                    *cursor += 1;
                    taken_outcomes[item as usize].next().ok_or_else(|| {
                        invariant("ordered frontier contribution has no recorded outcome")
                    })?
                }
            };
            match outcome {
                Outcome::Merged { old, new } => {
                    work.record_coalescing_additions(1);
                    request.record_coalescing_additions(1, coalescing_limit)?;
                    replace_monomials(&mut progress.frontier_coefficient_monomials, old, new)?;
                    progress.merged_terms = progress.merged_terms.saturating_add(1);
                }
                Outcome::Cancelled { old } => {
                    work.record_coalescing_additions(1);
                    request.record_coalescing_additions(1, coalescing_limit)?;
                    replace_monomials(&mut progress.frontier_coefficient_monomials, old, 0)?;
                    request.release_pending_frame()?;
                    frontier_keys = frontier_keys
                        .checked_sub(1)
                        .ok_or_else(|| invariant("ordered window frontier census underflow"))?;
                    progress.cancelled_terms = progress.cancelled_terms.saturating_add(1);
                }
                Outcome::Inserted { new } => {
                    request.retain_pending_frame(frame_limit)?;
                    replace_monomials(&mut progress.frontier_coefficient_monomials, 0, new)?;
                    frontier_keys += 1;
                    progress.peak_frontier_keys = progress.peak_frontier_keys.max(frontier_keys);
                }
                Outcome::Dropped => {}
                Outcome::Failed { error, accounted } => {
                    match accounted {
                        Accounted::Nothing | Accounted::Emitted => {}
                        Accounted::Coalescing => {
                            work.record_coalescing_additions(1);
                            request.record_coalescing_additions(1, coalescing_limit)?;
                        }
                        Accounted::RetainedFrame => request.retain_pending_frame(frame_limit)?,
                    }
                    return Err(error);
                }
            }
            progress.peak_frontier_coefficient_monomials = progress
                .peak_frontier_coefficient_monomials
                .max(progress.frontier_coefficient_monomials);
        }
        for (edge, terminal) in expansion.terminals.into_iter().enumerate() {
            let item = terminal.item as usize;
            let cursor = &mut terminal_cursor[item];
            if terminal_items[item].contributions.get(*cursor) != Some(&(index, edge as u32)) {
                return Err(
                    invariant("ordered terminal contributions were merged out of order").into(),
                );
            }
            *cursor += 1;
            let outcome = terminal_outcomes[item].next().ok_or_else(|| {
                invariant("ordered terminal contribution has no recorded outcome")
            })?;
            if matches!(outcome, Outcome::Dropped) {
                continue;
            }
            if let Outcome::Failed {
                error,
                accounted: Accounted::Nothing,
            } = outcome
            {
                return Err(error);
            }
            progress.emitted_terms = progress.emitted_terms.saturating_add(1);
            match outcome {
                Outcome::Merged { old, new } => {
                    work.record_coalescing_additions(1);
                    request.record_coalescing_additions(1, coalescing_limit)?;
                    replace_monomials(&mut progress.terminal_coefficient_monomials, old, new)?;
                    progress.terminal_merges = progress.terminal_merges.saturating_add(1);
                }
                Outcome::Cancelled { old } => {
                    work.record_coalescing_additions(1);
                    request.record_coalescing_additions(1, coalescing_limit)?;
                    replace_monomials(&mut progress.terminal_coefficient_monomials, old, 0)?;
                    terminal_keys = terminal_keys
                        .checked_sub(1)
                        .ok_or_else(|| invariant("ordered window terminal census underflow"))?;
                    progress.terminal_cancellations =
                        progress.terminal_cancellations.saturating_add(1);
                }
                Outcome::Inserted { new } => {
                    replace_monomials(&mut progress.terminal_coefficient_monomials, 0, new)?;
                    terminal_keys += 1;
                }
                Outcome::Failed { error, accounted } => {
                    if accounted == Accounted::Coalescing {
                        work.record_coalescing_additions(1);
                        request.record_coalescing_additions(1, coalescing_limit)?;
                    }
                    return Err(error);
                }
                Outcome::Dropped => unreachable!("handled above"),
            }
            progress.peak_terminal_coefficient_monomials = progress
                .peak_terminal_coefficient_monomials
                .max(progress.terminal_coefficient_monomials);
        }
        progress.processed_keys = progress.processed_keys.saturating_add(1);
        progress.frontier_keys = frontier_keys;
        progress.peak_frontier_keys = progress.peak_frontier_keys.max(frontier_keys);
        progress.terminal_keys = terminal_keys;
        kernel.observe(progress, Some(&node.key))?;
    }
    Ok((frontier_keys, terminal_keys))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::AtomicUsize;
    use std::thread::ThreadId;

    use super::*;
    use crate::algebra::CoefficientContext;
    use crate::reduction::ordered_reduction::step_cache::codec::NodeStage;
    use crate::reduction::ordered_reduction::step_cache::kernel::{
        ConcurrentStepCacheKernel, OrderedStepCacheKernel, run_with_step_cache_and_options,
    };
    use crate::reduction::ordered_reduction::step_cache::tests::Scratch;
    use crate::reduction::ordered_reduction::{OrderedKeyStage, OrderedOutput, OrderedPhaseEvent};
    use crate::reduction::{
        ModularWeightOptions, OrderedStepCache, OrderedStepCacheAdapter, OrderedStepCacheBinding,
        OrderedStepCacheStatistics, OrderedStepShard, OrderedStepStage, OrderedWeightArithmetic,
        ReductionLimits,
    };
    use crate::solver::CandidateOrderedReductionProgress;

    const PHASES: i64 = 5;

    fn mix(mut value: u64) -> u64 {
        value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    /// A pseudo-random weighted DAG over keys `[phase, id]`, larger ids
    /// first. Even phases are chains (same-phase descent), odd phases are
    /// antichains. Small integer coefficients make exact cancellations of
    /// frontier keys and terminals frequent; some parents scale everything
    /// they emit by a rational function of `d`.
    struct Lattice {
        context: CoefficientContext,
        size: i64,
        seed: u64,
        /// Keys whose step fails.
        poisoned: BTreeSet<[i64; 2]>,
        /// Explicit steps overriding the generated ones.
        explicit: BTreeMap<[i64; 2], Vec<([i64; 2], i64)>>,
    }

    impl Lattice {
        fn new(size: i64, seed: u64) -> Self {
            Self {
                context: CoefficientContext::try_new(["d"]).unwrap(),
                size,
                seed,
                poisoned: BTreeSet::new(),
                explicit: BTreeMap::new(),
            }
        }

        fn key(phase: i64, id: i64) -> IntegralKey {
            IntegralKey::try_new([phase, id]).unwrap()
        }

        fn position(&self, key: &IntegralKey) -> OrderedPosition<i64> {
            OrderedPosition {
                phase: key.powers()[0] as usize,
                priority: key.powers()[1],
            }
        }

        fn small(&self, value: u64) -> i64 {
            [-2, -1, 1, 2, 1, -1][(value % 6) as usize]
        }

        fn step(&self, key: &IntegralKey) -> Result<OrderedStep<IntegralKey>, ReductionError> {
            let (phase, id) = (key.powers()[0], key.powers()[1]);
            if self.poisoned.contains(&[phase, id]) {
                return Err(ReductionError::ReducerInvariant {
                    detail: "poisoned lattice key",
                });
            }
            let c = |text: String| self.context.coefficient_fixture(&text);
            if let Some(children) = self.explicit.get(&[phase, id]) {
                let mut step = OrderedStep::default();
                for (child, coefficient) in children {
                    if child[0] < 0 {
                        step.terminals
                            .insert(Self::key(9, child[1]), self.context.integer(*coefficient));
                    } else {
                        step.terms.insert(
                            Self::key(child[0], child[1]),
                            self.context.integer(*coefficient),
                        );
                    }
                }
                return Ok(step);
            }
            let hash = mix(self.seed ^ ((phase as u64) << 32) ^ id as u64);
            let factor = match hash % 13 {
                0 => "(d+1)/(d-2)",
                1 => "d",
                _ => "1",
            };
            let conditions = ["d+1", "d-2", "2*d+3", "d+5"];
            let first = (hash >> 40) as usize % conditions.len();
            let mut step = OrderedStep {
                nonzero_conditions: vec![
                    c(conditions[first].into()),
                    c(
                        conditions[(first + 1 + (hash >> 44) as usize % 2) % conditions.len()]
                            .into(),
                    ),
                ],
                ..Default::default()
            };
            if id < 4 || (phase == PHASES - 1 && hash % 3 != 0) {
                step.terminals.insert(
                    Self::key(9, (hash >> 8) as i64 % 3),
                    c(format!("{}*{factor}", self.small(hash >> 12))),
                );
                if hash % 5 == 0 {
                    step.terminals
                        .insert(Self::key(9, 7), c(format!("(d-{id})/(d+{phase}+1)")));
                }
                return Ok(step);
            }
            if phase % 2 == 0 {
                for k in 0..1 + (hash >> 4) % 3 {
                    let h = mix(hash + k);
                    let child = id - 1 - (h % 5) as i64;
                    if child >= 0 {
                        step.terms.insert(
                            Self::key(phase, child),
                            c(format!("{}*{factor}", self.small(h >> 8))),
                        );
                    }
                }
            }
            if phase + 1 < PHASES {
                for k in 0..1 + (hash >> 6) % 3 {
                    let h = mix(hash ^ (k << 20));
                    let later = phase + 1 + (h % (PHASES - phase - 1) as u64) as i64;
                    let child = (h >> 16) as i64 % (self.size / 3);
                    step.terms.insert(
                        Self::key(later, child),
                        c(format!("{}*{factor}", self.small(h >> 8))),
                    );
                }
            }
            if (hash >> 16) % 5 == 0 {
                step.terms.insert(
                    Self::key(PHASES - 1, (hash >> 24) as i64 % 4),
                    c("0".into()),
                );
            }
            if (hash >> 20) % 4 == 0 {
                step.terminals.insert(
                    Self::key(9, (hash >> 24) as i64 % 3),
                    c(format!("{}", self.small(hash >> 28))),
                );
            }
            Ok(step)
        }

        fn initial(&self) -> BTreeMap<IntegralKey, Coefficient> {
            (0..self.size)
                .filter(|id| mix(self.seed ^ 0x1717 ^ *id as u64) % 3 == 0)
                .map(|id| {
                    let phase = (mix(self.seed ^ id as u64) % 3) as i64;
                    (
                        Self::key(phase, id),
                        self.context
                            .integer(self.small(mix(self.seed ^ 0x55 ^ id as u64))),
                    )
                })
                .collect()
        }
    }

    #[derive(Debug, PartialEq)]
    struct Record {
        output: Result<(OrderedOutput, String), String>,
        observations: Vec<(CandidateOrderedReductionProgress, Option<IntegralKey>)>,
        steps: Vec<IntegralKey>,
        inspections: Vec<String>,
        /// Keys whose step was replayed from a step cache, in hit order.
        hits: Vec<IntegralKey>,
    }

    impl PartialEq for OrderedOutput {
        fn eq(&self, other: &Self) -> bool {
            (
                &self.terms,
                &self.nonzero_conditions,
                &self.work,
                &self.progress,
            ) == (
                &other.terms,
                &other.nonzero_conditions,
                &other.work,
                &other.progress,
            )
        }
    }

    impl std::fmt::Debug for OrderedOutput {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("OrderedOutput")
                .field("terms", &self.terms)
                .field("nonzero_conditions", &self.nonzero_conditions)
                .field("progress", &self.progress)
                .finish_non_exhaustive()
        }
    }

    struct LatticeKernel<'a> {
        lattice: &'a Lattice,
        record: Record,
        threads: &'a Mutex<HashSet<ThreadId>>,
        stepped: &'a AtomicUsize,
    }

    struct LatticeStepper<'a> {
        lattice: &'a Lattice,
        threads: &'a Mutex<HashSet<ThreadId>>,
        /// Steps computed by the stepper (window keys, including keys that
        /// cancel before their turn).
        stepped: &'a AtomicUsize,
    }

    fn work_of_one_step() -> ReductionStatistics {
        let mut work = ReductionStatistics::default();
        work.record_rule_application();
        work
    }

    impl OrderedKernel for LatticeKernel<'_> {
        type Key = IntegralKey;
        type Priority = i64;
        type Error = ReductionError;

        fn context(&self) -> &CoefficientContext {
            &self.lattice.context
        }
        fn limits(&self) -> ReductionLimits {
            ReductionLimits::default()
        }
        fn phase_count(&self) -> usize {
            PHASES as usize
        }
        fn position(&mut self, key: &IntegralKey) -> Result<OrderedPosition<i64>, ReductionError> {
            lock(self.threads).insert(std::thread::current().id());
            Ok(self.lattice.position(key))
        }
        fn step(
            &mut self,
            key: &IntegralKey,
            _: &mut ReductionRequest,
            work: &mut ReductionStatistics,
        ) -> Result<OrderedStep<IntegralKey>, ReductionError> {
            lock(self.threads).insert(std::thread::current().id());
            self.record.steps.push(key.clone());
            work.merge_work(work_of_one_step());
            self.lattice.step(key)
        }
        fn observe(
            &mut self,
            progress: &CandidateOrderedReductionProgress,
            current: Option<&IntegralKey>,
        ) -> Result<(), ReductionError> {
            self.record.observations.push((*progress, current.cloned()));
            Ok(())
        }
        fn key_view(key: &IntegralKey) -> Option<(OrderedKeyStage<'_>, &IntegralKey)> {
            Some((OrderedKeyStage::Integral, key))
        }
        fn inspect_phase(&mut self, event: OrderedPhaseEvent<'_>) -> Result<(), ReductionError> {
            let frontier: Vec<String> = event
                .frontier()
                .map(|entry| {
                    format!(
                        "{}:{:?}={}",
                        entry.phase,
                        entry.integral.powers(),
                        entry.weight.map_or("-".into(), |w| w.to_string())
                    )
                })
                .collect();
            let terminals: Vec<String> = event
                .terminals()
                .map(|(key, weight)| {
                    format!(
                        "{:?}={}",
                        key.powers(),
                        weight.map_or("-".into(), |w| w.to_string())
                    )
                })
                .collect();
            let conditions: Vec<String> = event
                .nonzero_conditions()
                .iter()
                .map(ToString::to_string)
                .collect();
            self.record.inspections.push(format!(
                "{:?} {} {:?} | {frontier:?} | {terminals:?} | {conditions:?}",
                event.boundary, event.phase, event.progress
            ));
            Ok(())
        }
    }

    impl OrderedStepper for LatticeStepper<'_> {
        type Key = IntegralKey;
        type Priority = i64;
        type Error = ReductionError;
        type Account = ReductionStatistics;

        fn position(&self, key: &IntegralKey) -> Result<OrderedPosition<i64>, ReductionError> {
            lock(self.threads).insert(std::thread::current().id());
            Ok(self.lattice.position(key))
        }
        fn step(
            &self,
            key: &IntegralKey,
        ) -> (
            Result<OrderedStep<IntegralKey>, ReductionError>,
            ReductionStatistics,
        ) {
            lock(self.threads).insert(std::thread::current().id());
            self.stepped.fetch_add(1, Ordering::Relaxed);
            (self.lattice.step(key), work_of_one_step())
        }
        fn later_phase(&self, parent: &IntegralKey, child: &IntegralKey) -> bool {
            // Certify only part of the later-phase children, so that both
            // classifications are exercised.
            child.powers()[0] > parent.powers()[0] + 1
        }
    }

    impl ConcurrentOrderedKernel for LatticeKernel<'_> {
        type Account = ReductionStatistics;
        type Stepper<'s>
            = LatticeStepper<'s>
        where
            Self: 's;

        fn stepper(&self) -> Option<LatticeStepper<'_>> {
            Some(LatticeStepper {
                lattice: self.lattice,
                threads: self.threads,
                stepped: self.stepped,
            })
        }
        fn absorb(
            &mut self,
            key: &IntegralKey,
            account: ReductionStatistics,
            _: &mut ReductionRequest,
            work: &mut ReductionStatistics,
        ) -> Result<(), ReductionError> {
            self.record.steps.push(key.clone());
            work.merge_work(account);
            Ok(())
        }
    }

    /// Lattice records are sharded by phase; phase 3 is not recorded.
    fn lattice_shard(key: &IntegralKey) -> Option<OrderedStepShard> {
        let phase = key.powers()[0];
        (phase != 3).then_some(OrderedStepShard {
            stage: OrderedStepStage::Integral,
            support: phase as u16,
            class: 0,
        })
    }

    fn lattice_key(stage: NodeStage, integral: IntegralKey) -> Result<IntegralKey, ReductionError> {
        match stage {
            NodeStage::Integral => Ok(integral),
            _ => Err(invariant("a recorded lattice key is not an integral")),
        }
    }

    impl OrderedStepCacheKernel for LatticeKernel<'_> {
        fn step_cache_shard(&mut self, key: &IntegralKey) -> Option<OrderedStepShard> {
            lattice_shard(key)
        }
        fn step_cache_key(
            &self,
            stage: NodeStage,
            integral: IntegralKey,
        ) -> Result<IntegralKey, ReductionError> {
            lattice_key(stage, integral)
        }
        fn step_cache_hit(&mut self, key: &IntegralKey) -> Result<(), ReductionError> {
            self.record.hits.push(key.clone());
            Ok(())
        }
    }

    impl ConcurrentStepCacheKernel for LatticeKernel<'_> {
        fn stepper_shard<'s>(_: &LatticeStepper<'s>, key: &IntegralKey) -> Option<OrderedStepShard>
        where
            Self: 's,
        {
            lattice_shard(key)
        }
        fn stepper_key<'s>(
            _: &LatticeStepper<'s>,
            stage: NodeStage,
            integral: IntegralKey,
        ) -> Result<IntegralKey, ReductionError>
        where
            Self: 's,
        {
            lattice_key(stage, integral)
        }
    }

    fn run(
        lattice: &Lattice,
        initial: &BTreeMap<IntegralKey, Coefficient>,
        arithmetic: OrderedWeightArithmetic,
        scheduler: OrderedSchedulerOptions,
    ) -> (Record, HashSet<ThreadId>) {
        let (record, threads, _) = run_cached(lattice, initial, arithmetic, scheduler, None);
        (record, threads)
    }

    /// One request through the step-cache entry point (`None`: uncached,
    /// i.e. `run_with_options`). Also returns the stepper's step count.
    fn run_cached(
        lattice: &Lattice,
        initial: &BTreeMap<IntegralKey, Coefficient>,
        arithmetic: OrderedWeightArithmetic,
        scheduler: OrderedSchedulerOptions,
        cache: Option<&OrderedStepCache>,
    ) -> (Record, HashSet<ThreadId>, usize) {
        let threads = Mutex::new(HashSet::new());
        let stepped = AtomicUsize::new(0);
        let mut kernel = LatticeKernel {
            lattice,
            record: Record {
                output: Err(String::new()),
                observations: Vec::new(),
                steps: Vec::new(),
                inspections: Vec::new(),
                hits: Vec::new(),
            },
            threads: &threads,
            stepped: &stepped,
        };
        let mut request = ReductionRequest::default();
        let result = run_with_step_cache_and_options(
            &mut kernel,
            &mut request,
            initial.clone(),
            vec![lattice.context.coefficient_fixture("d-2")],
            arithmetic,
            scheduler,
            cache,
        );
        kernel.record.output = result
            .map(|output| (output, format!("{}", request.pending_frame_count())))
            .map_err(|error| format!("{error:?}"));
        (
            kernel.record,
            threads.into_inner().unwrap(),
            stepped.into_inner(),
        )
    }

    /// The sequential scheduler, explicitly (immune to the test override).
    fn sequential() -> OrderedSchedulerOptions {
        OrderedSchedulerOptions {
            threads: 1,
            window_keys: 1,
        }
    }

    fn arithmetics() -> [OrderedWeightArithmetic; 3] {
        let modular = ModularWeightOptions {
            batch_points: 4,
            threads: 2,
            ..Default::default()
        };
        [
            OrderedWeightArithmetic::Exact,
            OrderedWeightArithmetic::Modular(modular),
            OrderedWeightArithmetic::Modular(ModularWeightOptions {
                cross_check_exact: true,
                ..modular
            }),
        ]
    }

    #[test]
    fn concurrent_phases_reproduce_the_sequential_execution() {
        let mut cancellations = 0;
        for seed in 0..6 {
            let lattice = Lattice::new(40, seed);
            let initial = lattice.initial();
            for arithmetic in arithmetics() {
                let (sequential, used) = run(&lattice, &initial, arithmetic, sequential());
                assert_eq!(used.len(), 1, "one thread runs the sequential scheduler");
                let output = &sequential.output.as_ref().unwrap().0;
                assert!(!output.terms.is_empty());
                cancellations +=
                    output.progress.cancelled_terms + output.progress.terminal_cancellations;
                for threads in [2, 3, 8] {
                    for window_keys in [1, 2, 5, 64, 4096] {
                        let (concurrent, _) = run(
                            &lattice,
                            &initial,
                            arithmetic,
                            OrderedSchedulerOptions {
                                threads,
                                window_keys,
                            },
                        );
                        assert_eq!(
                            concurrent, sequential,
                            "seed {seed}, {arithmetic:?}, {threads} threads, window {window_keys}"
                        );
                    }
                }
            }
        }
        assert!(
            cancellations > 20,
            "the fixture must exercise exact erasure ({cancellations})"
        );
    }

    #[test]
    fn one_thread_creates_no_worker_and_more_threads_use_them() {
        let lattice = Lattice::new(60, 11);
        let initial = lattice.initial();
        let caller = std::thread::current().id();
        let (_, used) = run(
            &lattice,
            &initial,
            OrderedWeightArithmetic::Exact,
            sequential(),
        );
        assert_eq!(used, HashSet::from([caller]));
        for arithmetic in arithmetics() {
            let (_, used) = run(
                &lattice,
                &initial,
                arithmetic,
                OrderedSchedulerOptions {
                    threads: 4,
                    window_keys: 64,
                },
            );
            assert!(used.iter().any(|thread| *thread != caller));
        }
        assert!(OrderedSchedulerOptions::with_threads(0).validate().is_err());
    }

    /// Many parents of one window merge into the same keys: alternating
    /// signs cancel and re-insert one destination at every other contribution,
    /// another accumulates onto an existing weight. The concurrent prefix
    /// merge of such heavy destinations must reproduce every sequential state
    /// (outcomes, census, peaks, observations).
    #[test]
    fn heavy_destinations_reproduce_every_sequential_state() {
        let mut lattice = Lattice::new(20, 5);
        let mut initial = BTreeMap::new();
        for id in 1000..1240 {
            let sign = if id % 2 == 0 { 1 } else { -1 };
            lattice.explicit.insert(
                [0, id],
                vec![
                    ([1, 5], sign),
                    ([1, 6], 1 + id % 3),
                    ([1, 7], id % 5 - 2),
                    ([-1, 0], 1),
                ],
            );
            initial.insert(
                Lattice::key(0, id),
                lattice
                    .context
                    .coefficient_fixture(if id % 7 == 0 { "(d+1)/(d-2)" } else { "1" }),
            );
        }
        // A zero product never reaches a destination: `[1, 7]` gets id % 5 == 2
        // as a zero coefficient (skipped), the others are nonzero.
        initial.insert(Lattice::key(1, 6), lattice.context.integer(7));
        for id in [5, 6, 7] {
            lattice.explicit.insert([1, id], vec![([-1, id], 1)]);
        }
        let before = PREFIX_MERGED.load(Ordering::Relaxed);
        for arithmetic in arithmetics() {
            let (sequential, _) = run(&lattice, &initial, arithmetic, sequential());
            let progress = sequential.output.as_ref().unwrap().0.progress;
            assert!(progress.cancelled_terms > 50, "{progress:?}");
            for threads in [2, 3, 8] {
                for window_keys in [64, 4096] {
                    let (concurrent, _) = run(
                        &lattice,
                        &initial,
                        arithmetic,
                        OrderedSchedulerOptions {
                            threads,
                            window_keys,
                        },
                    );
                    assert_eq!(
                        concurrent, sequential,
                        "{arithmetic:?}, {threads} threads, window {window_keys}"
                    );
                }
            }
        }
        // Exact weights took the concurrent prefix merge (the journaled
        // modular arithmetics keep the sequential merge loop), unless the
        // test override runs exact requests as cross-checked modular ones.
        if std::env::var_os("RUSTRED_TEST_ORDERED_MODULAR").is_none_or(|value| value != "1") {
            assert!(PREFIX_MERGED.load(Ordering::Relaxed) - before > 1000);
        }
    }

    /// Two equal weights emit opposite contributions to a key whose step
    /// fails: the key cancels before its turn, so its (speculatively
    /// computed) failure must not surface. An expanded failing key, and a
    /// child above its parent, fail exactly as in the sequential scheduler.
    #[test]
    fn concurrent_failures_are_those_of_the_sequential_scheduler() {
        let mut lattice = Lattice::new(20, 3);
        lattice.explicit = BTreeMap::from([
            ([0, 10], vec![([0, 5], 1), ([0, 4], 1)]),
            ([0, 9], vec![([0, 5], -1), ([0, 4], 2)]),
            ([0, 4], vec![([-1, 0], 1), ([1, 3], 1)]),
            ([0, 1], vec![([-1, 1], 1)]),
            ([1, 3], vec![([-1, 0], -3)]),
        ]);
        lattice.poisoned.insert([0, 5]);
        let initial = BTreeMap::from([
            (Lattice::key(0, 10), lattice.context.one()),
            (Lattice::key(0, 9), lattice.context.one()),
            (Lattice::key(0, 1), lattice.context.one()),
        ]);
        let failing_steps = |lattice: &Lattice| {
            for arithmetic in arithmetics() {
                let (sequential, _) = run(lattice, &initial, arithmetic, sequential());
                for window_keys in [1, 2, 3, 16] {
                    let (concurrent, _) = run(
                        lattice,
                        &initial,
                        arithmetic,
                        OrderedSchedulerOptions {
                            threads: 3,
                            window_keys,
                        },
                    );
                    assert_eq!(concurrent, sequential, "window {window_keys}");
                }
                assert!(sequential.output.is_ok() || sequential.observations.len() > 1);
            }
        };
        failing_steps(&lattice);
        let (ok, _) = run(
            &lattice,
            &initial,
            OrderedWeightArithmetic::Exact,
            sequential(),
        );
        assert!(
            ok.output.is_ok(),
            "the cancelled poisoned key is never expanded"
        );
        // An expanded failing key.
        lattice.poisoned.insert([0, 4]);
        failing_steps(&lattice);
        // A child above its parent.
        lattice.poisoned.clear();
        lattice.explicit.insert([0, 4], vec![([0, 6], 1)]);
        failing_steps(&lattice);
        let (failed, _) = run(
            &lattice,
            &initial,
            OrderedWeightArithmetic::Exact,
            sequential(),
        );
        assert!(
            failed
                .output
                .unwrap_err()
                .contains("does not strictly decrease")
        );
    }

    /// Every file under `root`, by relative path.
    fn listing(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
        let mut files = Vec::new();
        let mut pending = vec![root.to_path_buf()];
        while let Some(directory) = pending.pop() {
            for entry in std::fs::read_dir(&directory).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    pending.push(path);
                } else {
                    let bytes = std::fs::read(&path).unwrap();
                    files.push((path.strip_prefix(root).unwrap().to_path_buf(), bytes));
                }
            }
        }
        files.sort();
        files
    }

    /// The counted lookups and inserts of every shard. Shard loading (state,
    /// loaded records, resident bytes, load time) may differ: a worker can
    /// load a shard for a key that then cancels.
    fn counted(statistics: OrderedStepCacheStatistics) -> Vec<(OrderedStepShard, [u64; 5])> {
        statistics
            .shards
            .iter()
            .map(|shard| {
                let counters = [
                    shard.hits,
                    shard.misses,
                    shard.inserts,
                    shard.skipped_inserts,
                    shard.opaque_hits,
                ];
                (shard.shard, counters)
            })
            .filter(|(_, counters)| counters.iter().any(|&count| count > 0))
            .collect()
    }

    /// A cached request on the scheduler's workers does what the sequential
    /// cached request does: same results, observations (with the request's
    /// cache hits and misses), adapter accounts and hit accounting in pop
    /// order, and it leaves the same shard counters and byte-identical saved
    /// files, starting from an empty, a partial and a complete cache.
    /// Its workers also step keys that cancel before their turn; those steps
    /// are neither counted nor recorded.
    #[test]
    fn cached_concurrent_runs_replay_and_record_exactly_the_sequential_steps() {
        let binding = |lattice: &Lattice| {
            OrderedStepCacheBinding::new(
                OrderedStepCacheAdapter::CandidateProgram,
                [7; 32],
                "ordered parallel lattice",
                &lattice.context,
                None,
            )
        };
        let copy = |from: &Path, label: &str| {
            let target = Scratch::new(label);
            for (path, bytes) in listing(from) {
                let path = target.0.join(path);
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(path, bytes).unwrap();
            }
            target
        };
        let caller = std::thread::current().id();
        let (mut speculative, mut replayed) = (0, 0);
        for (seed, poisoned) in [(0, false), (1, false), (2, false), (3, true)] {
            let mut lattice = Lattice::new(40, seed);
            let initial = lattice.initial();
            let (uncached, _) = run(
                &lattice,
                &initial,
                OrderedWeightArithmetic::Exact,
                sequential(),
            );
            if poisoned {
                // The request fails at an expanded key half-way through.
                let middle = &uncached.steps[uncached.steps.len() / 2];
                lattice
                    .poisoned
                    .insert([middle.powers()[0], middle.powers()[1]]);
            }
            for arithmetic in arithmetics() {
                let (uncached, _) = run(&lattice, &initial, arithmetic, sequential());
                assert_eq!(uncached.output.is_err(), poisoned);
                // Saved caches: empty, trained on part of the inputs, and
                // complete (trained on the whole request).
                let empty = Scratch::new("lattice-empty");
                let partial = Scratch::new("lattice-partial");
                let trained =
                    OrderedStepCache::open(&partial.0, binding(&lattice), Default::default())
                        .unwrap();
                let half: BTreeMap<_, _> = initial
                    .iter()
                    .step_by(2)
                    .map(|(key, weight)| (key.clone(), weight.clone()))
                    .collect();
                run_cached(&lattice, &half, arithmetic, sequential(), Some(&trained));
                trained.save().unwrap();
                let complete = Scratch::new("lattice-complete");
                let full =
                    OrderedStepCache::open(&complete.0, binding(&lattice), Default::default())
                        .unwrap();
                run_cached(&lattice, &initial, arithmetic, sequential(), Some(&full));
                full.save().unwrap();
                for start in [&empty, &partial, &complete] {
                    let cached_run = |scheduler: OrderedSchedulerOptions| {
                        let directory = copy(&start.0, "lattice-run");
                        let cache = OrderedStepCache::open(
                            &directory.0,
                            binding(&lattice),
                            Default::default(),
                        )
                        .unwrap();
                        let (record, threads, stepped) =
                            run_cached(&lattice, &initial, arithmetic, scheduler, Some(&cache));
                        let statistics = counted(cache.statistics());
                        cache.save().unwrap();
                        (record, threads, stepped, statistics, listing(&directory.0))
                    };
                    let (sequential, _, _, statistics, files) = cached_run(sequential());
                    // A cached request reproduces the uncached one.
                    match (&sequential.output, &uncached.output) {
                        (Ok((cached, _)), Ok((plain, _))) => {
                            assert_eq!(cached.terms, plain.terms);
                            assert_eq!(cached.nonzero_conditions, plain.nonzero_conditions);
                        }
                        (Err(cached), Err(plain)) => assert_eq!(cached, plain),
                        _ => panic!("cached and uncached requests disagree"),
                    }
                    for (threads, window_keys) in [(2, 1), (3, 3), (8, 64)] {
                        let (concurrent, used, stepped, concurrent_statistics, concurrent_files) =
                            cached_run(OrderedSchedulerOptions {
                                threads,
                                window_keys,
                            });
                        let context =
                            format!("seed {seed} {arithmetic:?} t{threads} w{window_keys}");
                        assert_eq!(concurrent, sequential, "{context}");
                        assert_eq!(concurrent_statistics, statistics, "{context}");
                        assert_eq!(concurrent_files, files, "{context}");
                        assert!(used.iter().any(|thread| *thread != caller), "{context}");
                        // Steps computed on the workers beyond the expanded
                        // keys that missed: keys that cancelled.
                        let computed = concurrent.steps.len();
                        speculative += stepped.saturating_sub(computed);
                        replayed += concurrent.hits.len();
                    }
                }
            }
        }
        assert!(
            speculative > 0,
            "no key was stepped ahead and then cancelled"
        );
        assert!(replayed > 0, "no step was replayed");
    }
}
