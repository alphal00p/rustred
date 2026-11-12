//! Global, phase-ordered reduction of exact weighted integral frontiers.
//!
//! This engine owns no completed-result cache. Artifact adapters retain the
//! existing checked mathematical operations and supply a certified descending
//! position; this module only schedules and combines their weighted results.
use std::cmp::Reverse;
use std::collections::{BTreeMap, btree_map};

pub(crate) mod arithmetic;
pub mod composition;
mod frontier;
pub(crate) mod memstats;
mod modular;
mod parallel;
pub(crate) mod parstats;
pub(crate) mod step_cache;
pub(crate) mod tail;

pub use arithmetic::{
    ModularCommon, OrderedWeightArithmetic, ParseOrderedWeightArithmeticError,
    PolynomialDenominators, PolynomialOptions, SampledOptions,
};
pub use composition::{
    ComposedTerminals, OrderedCompositionPlan, OrderedCompositionStage, OrderedTerminalComposer,
    OrderedTerminalComposerSlot,
};
use frontier::{Child, ChildAction, Frontier, FrontierLayout, FrontierMode};
pub(crate) use frontier::{CompactPowers, FrontierKey};
use modular::{Lifter, ModularRecorder};
pub(crate) use parallel::{ConcurrentOrderedKernel, NoDetached, OrderedStepper};
use parallel::{ConcurrentWeights, Pipelined, Prefetched, Workers};
pub use parallel::{OrderedProfile, OrderedSchedulerOptions};
pub use tail::{
    ORDERED_TAIL_SCHEMA, ORDERED_TAIL_SEMANTICS, OrderedTailTable, OrderedTailTableStatistics,
};
use tail::{TailRewrite, TailRewriteReport, TailRun};

use crate::algebra::{Coefficient, CoefficientContext};
use crate::family::IntegralKey;
use crate::reduction::guard_union::GuardIndex;
use crate::reduction::{ReductionError, ReductionLimits, ReductionRequest, ReductionStatistics};
use crate::solver::CandidateOrderedReductionProgress;

pub(crate) mod inspection;
use inspection::OrderedStateView;
pub(crate) use inspection::{
    OrderedFrontierEntry, OrderedKeyStage, OrderedPhaseBoundary, OrderedPhaseDescriptor,
    OrderedPhaseEvent, OrderedPhaseInspector,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OrderedPosition<P> {
    /// Zero-based phase in execution order. This total is frozen before a call.
    pub phase: usize,
    /// Larger values must be reduced first within one phase.
    pub priority: P,
}

/// One mathematically checked action. A terminal output is complete for this
/// adapter; it is never placed back on the unreduced frontier.
pub(crate) struct OrderedStep<K> {
    pub terms: BTreeMap<K, Coefficient>,
    pub terminals: BTreeMap<IntegralKey, Coefficient>,
    pub nonzero_conditions: Vec<Coefficient>,
}

impl<K> Default for OrderedStep<K> {
    fn default() -> Self {
        Self {
            terms: BTreeMap::new(),
            terminals: BTreeMap::new(),
            nonzero_conditions: vec![],
        }
    }
}

pub(crate) trait OrderedKernel {
    type Key: FrontierKey;
    /// `Hash` (rr-frontier): a packed frontier interns priorities.
    type Priority: Clone + Ord + std::hash::Hash;
    type Error: From<ReductionError>;

    fn context(&self) -> &CoefficientContext;
    fn limits(&self) -> ReductionLimits;
    fn phase_count(&self) -> usize;
    fn position(&mut self, key: &Self::Key)
    -> Result<OrderedPosition<Self::Priority>, Self::Error>;
    fn step(
        &mut self,
        key: &Self::Key,
        request: &mut ReductionRequest,
        work: &mut ReductionStatistics,
    ) -> Result<OrderedStep<Self::Key>, Self::Error>;
    fn observe(
        &mut self,
        _progress: &CandidateOrderedReductionProgress,
        _current: Option<&Self::Key>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Retain attempted work even when a checked action or arithmetic fails.
    fn record_work(&mut self, _work: ReductionStatistics) {}
    /// Public view of one scheduled identity for phase inspection.
    fn key_view(_key: &Self::Key) -> Option<(OrderedKeyStage<'_>, &IntegralKey)> {
        None
    }
    /// Called at both boundaries of every frozen phase, never inside a step.
    /// Adapters without a caller inspector keep this no-op.
    fn inspect_phase(&mut self, _event: OrderedPhaseEvent<'_>) -> Result<(), Self::Error> {
        Ok(())
    }
    /// The persistent tail table of the program (see `tail`), when one is
    /// attached (through a step cache). Called once per request.
    fn tail_table(&mut self) -> Result<Option<std::sync::Arc<OrderedTailTable>>, Self::Error> {
        Ok(None)
    }
    /// Whether the concurrent scheduler (`Exact` on more than one thread)
    /// logs the adjoint tail as the sequential journal does. With a
    /// populating tail table it then completes and tabulates the tail, which
    /// steps (and, through a step cache, records) the reached keys the
    /// forward pass erased: the run records the same steps and tail entries
    /// as the sequential scheduler and every recorded arithmetic, whatever
    /// its thread count. Off by default: closed, candidate and
    /// saved-application caches keep the concurrent scheduler unchanged.
    fn concurrent_logs_tail(&self) -> bool {
        false
    }
    /// A scheduled key from its encoded identity (the step-cache encoding),
    /// when the adapter can rebuild it.
    fn tail_key(&self, _encoded: &[u8]) -> Option<Self::Key> {
        None
    }
    /// Append the compact identity of `key` in a tail table (`false`, `out`
    /// unchanged: no compact form, nothing of the run is tabulated). The
    /// default is the step-cache encoding of the key's public view; adapters
    /// whose keys include identities without a public view (partially
    /// transported Route keys) encode those with a stage tag that no
    /// step-cache identity uses. Must be a pure function of the key.
    fn tail_encode(key: &Self::Key, out: &mut Vec<u8>) -> bool {
        Self::key_view(key)
            .is_some_and(|(stage, integral)| step_cache::codec::encode_node(stage, integral, out))
    }
    /// The caller's terminal composition for modular requests (compose
    /// before lift, `composition.rs`); `None` lifts the terminals.
    fn terminal_composer(&self) -> Option<std::sync::Arc<dyn OrderedTerminalComposer>> {
        None
    }
    /// rr-frontier diagnostics (`memstats`): allocator bytes owned by a key.
    /// Default: nothing measured.
    fn key_heap_bytes(_key: &Self::Key) -> usize {
        0
    }
    /// rr-frontier diagnostics: allocator bytes of a priority that a clone
    /// allocates again, and bytes that clones share (an `Arc`).
    fn priority_heap_bytes(_priority: &Self::Priority) -> [usize; 2] {
        [0; 2]
    }
}

pub(crate) struct OrderedOutput {
    /// Exact terminal coefficients; empty when `composed` is set.
    pub terms: BTreeMap<IntegralKey, Coefficient>,
    /// Lifted outputs of the caller's terminal composition (modular
    /// requests with a composer that planned them).
    pub composed: Option<ComposedTerminals>,
    pub nonzero_conditions: Vec<Coefficient>,
    pub work: ReductionStatistics,
    pub progress: CandidateOrderedReductionProgress,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Scheduled<K, P> {
    phase: Reverse<usize>,
    priority: P,
    key: K,
}

type FrontierOf<K, A> = Frontier<
    <K as OrderedKernel>::Key,
    <K as OrderedKernel>::Priority,
    <A as OrderedWeights>::Weight,
>;

#[allow(clippy::type_complexity)] // the lazily unpacked frontier entries
struct SchedulerState<'a, K: OrderedKernel, A: OrderedWeights> {
    frontier: &'a FrontierOf<K, A>,
    terminals: &'a BTreeMap<IntegralKey, A::Weight>,
    nonzero_conditions: &'a [Coefficient],
    /// A packed frontier holds no unpacked keys to borrow: the entries are
    /// unpacked once, on the first iteration.
    unpacked: std::cell::OnceCell<Vec<(Scheduled<K::Key, K::Priority>, &'a A::Weight)>>,
}

impl<'a, K: OrderedKernel, A: OrderedWeights> SchedulerState<'a, K, A> {
    fn new(
        frontier: &'a FrontierOf<K, A>,
        terminals: &'a BTreeMap<IntegralKey, A::Weight>,
        nonzero_conditions: &'a [Coefficient],
    ) -> Self {
        Self {
            frontier,
            terminals,
            nonzero_conditions,
            unpacked: std::cell::OnceCell::new(),
        }
    }
}

impl<K: OrderedKernel, A: OrderedWeights> OrderedStateView for SchedulerState<'_, K, A> {
    fn frontier(&self) -> Box<dyn Iterator<Item = OrderedFrontierEntry<'_>> + '_> {
        let entry = frontier_entry::<K, A>;
        match self.frontier {
            Frontier::Classic(classic) => {
                Box::new(classic.order.iter().rev().filter_map(move |scheduled| {
                    let pending = classic.pending.get(&scheduled.key)?;
                    (pending.position.phase == scheduled.phase.0
                        && pending.position.priority == scheduled.priority)
                        .then_some(())?;
                    entry(scheduled, &pending.weight)
                }))
            }
            Frontier::Packed(_) => {
                let unpacked = self.unpacked.get_or_init(|| {
                    let frontier: &FrontierOf<K, A> = self.frontier;
                    frontier
                        .iter()
                        .map(|(scheduled, weight)| (scheduled.into_owned(), weight))
                        .collect()
                });
                Box::new(
                    unpacked
                        .iter()
                        .filter_map(move |(scheduled, weight)| entry(scheduled, weight)),
                )
            }
        }
    }
    fn terminals(
        &self,
    ) -> Box<dyn ExactSizeIterator<Item = (&IntegralKey, Option<&Coefficient>)> + '_> {
        Box::new(
            self.terminals
                .iter()
                .map(|(key, weight)| (key, A::exact(weight))),
        )
    }
    fn nonzero_conditions(&self) -> &[Coefficient] {
        self.nonzero_conditions
    }
}

fn frontier_entry<'e, K: OrderedKernel, A: OrderedWeights>(
    scheduled: &'e Scheduled<K::Key, K::Priority>,
    weight: &'e A::Weight,
) -> Option<OrderedFrontierEntry<'e>> {
    let (stage, integral) = K::key_view(&scheduled.key)?;
    Some(OrderedFrontierEntry {
        phase: scheduled.phase.0,
        stage,
        integral,
        weight: A::exact(weight),
    })
}

fn inspect<K: OrderedKernel, A: OrderedWeights>(
    kernel: &mut K,
    boundary: OrderedPhaseBoundary,
    phase: usize,
    progress: &CandidateOrderedReductionProgress,
    state: SchedulerState<'_, K, A>,
) -> Result<(), K::Error> {
    kernel.inspect_phase(OrderedPhaseEvent::new(boundary, phase, *progress, &state))
}

fn invariant(detail: &'static str) -> ReductionError {
    ReductionError::ReducerInvariant { detail }
}

fn coefficient_monomials(coefficient: &Coefficient) -> Result<usize, ReductionError> {
    coefficient
        .numerator
        .nterms()
        .checked_add(coefficient.denominator.nterms())
        .ok_or(ReductionError::CacheResourceCountOverflow {
            resource: "ordered coefficient monomial census",
        })
}

/// Maintain the size of resident maps only: the popped active coefficient and
/// temporary arithmetic are excluded, just as they are from frontier_keys.
fn replace_monomials(total: &mut usize, old: usize, new: usize) -> Result<(), ReductionError> {
    *total = total
        .checked_sub(old)
        .ok_or_else(|| invariant("ordered coefficient monomial census underflow"))?
        .checked_add(new)
        .ok_or(ReductionError::CacheResourceCountOverflow {
            resource: "ordered coefficient monomial census",
        })?;
    Ok(())
}

/// A frontier weight with the certified position computed when its key
/// entered the frontier.
#[derive(Clone)]
struct Pending<P, W> {
    position: OrderedPosition<P>,
    weight: W,
}

/// Hasher of the frontier's pending map. Fx for p requests
/// (`OrderedProfile::P`), SipHash otherwise; `RUSTRED_ORDERED_FRONTIER_HASH`
/// overrides (rr-prof). The map
/// is only probed (insert, entry, get, remove, len), never iterated, so the
/// hash function changes no order, value or counter, only the time of a
/// probe (SipHash of every child key was ~4 % of a 4L run).
#[derive(Clone)]
enum FrontierHash {
    Sip(std::hash::RandomState),
    Fx,
}

impl FrontierHash {
    fn new(fx: bool) -> Self {
        if fx {
            Self::Fx
        } else {
            Self::Sip(std::hash::RandomState::new())
        }
    }
}

/// Whether p requests (`OrderedProfile::P`) run the adjoint tail in this
/// process: off unless `RUSTRED_ORDERED_ADJOINT_TAIL=on` (M5 batch 2). The p
/// library's step identity records it, so caches with and without a tail
/// table never mix.
pub(crate) fn p_requests_run_the_adjoint_tail() -> bool {
    tail::environment().unwrap_or(false)
}

/// `RUSTRED_ORDERED_FRONTIER_HASH=fx|sip` overrides the profile's frontier
/// hash; unset or another value: `None`.
pub(crate) fn frontier_hash_environment() -> Option<bool> {
    static FX: std::sync::OnceLock<Option<bool>> = std::sync::OnceLock::new();
    *FX.get_or_init(
        || match std::env::var("RUSTRED_ORDERED_FRONTIER_HASH").as_deref() {
            Ok("fx") => Some(true),
            Ok("sip") => Some(false),
            _ => None,
        },
    )
}

/// An on/off switch from the environment: `1`/`on`/`true` is `Some(true)`,
/// `0`/`off`/`false` is `Some(false)`, unset or another value `None`.
fn switch_environment(name: &str) -> Option<bool> {
    match std::env::var(name).as_deref() {
        Ok("1" | "on" | "true") => Some(true),
        Ok("0" | "off" | "false") => Some(false),
        _ => None,
    }
}

/// `RUSTRED_ORDERED_PIPELINE=on|off` overrides the profile's pipelined step
/// source (rr-par; on for p requests since M5 batch 3).
pub(crate) fn pipeline_environment() -> Option<bool> {
    static ON: std::sync::OnceLock<Option<bool>> = std::sync::OnceLock::new();
    *ON.get_or_init(|| switch_environment("RUSTRED_ORDERED_PIPELINE"))
}

/// `RUSTRED_MODULAR_PAR_REPLAY=on|off` overrides the profile's parallel
/// modular replay (rr-par; on for p requests since M5 batch 3).
pub(crate) fn par_replay_environment() -> Option<bool> {
    static ON: std::sync::OnceLock<Option<bool>> = std::sync::OnceLock::new();
    *ON.get_or_init(|| switch_environment("RUSTRED_MODULAR_PAR_REPLAY"))
}

/// `RUSTRED_MODULAR_POLY_ROUND=n` overrides the profile's first modular
/// polynomial round (`Some(Some(n))`; `0`/`off`: `Some(None)`, one prime per
/// thread); unset or unparsable: `None`.
pub(crate) fn modular_round_environment() -> Option<Option<usize>> {
    static ROUND: std::sync::OnceLock<Option<Option<usize>>> = std::sync::OnceLock::new();
    *ROUND.get_or_init(|| {
        let value = std::env::var("RUSTRED_MODULAR_POLY_ROUND").ok()?;
        if value == "off" {
            return Some(None);
        }
        value
            .parse::<usize>()
            .ok()
            .map(|round| (round > 0).then_some(round))
    })
}

enum FrontierHasher {
    Sip(std::hash::DefaultHasher),
    Fx(u64),
}

impl FrontierHasher {
    #[inline]
    fn fx(state: &mut u64, word: u64) {
        *state = (state.rotate_left(5) ^ word).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
}

impl std::hash::Hasher for FrontierHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        match self {
            Self::Sip(hasher) => hasher.write(bytes),
            Self::Fx(state) => {
                let (chunks, rest) = bytes.as_chunks::<8>();
                for chunk in chunks {
                    Self::fx(state, u64::from_le_bytes(*chunk));
                }
                if !rest.is_empty() {
                    let mut word = [0u8; 8];
                    word[..rest.len()].copy_from_slice(rest);
                    Self::fx(
                        state,
                        u64::from_le_bytes(word) ^ ((rest.len() as u64) << 56),
                    );
                }
            }
        }
    }
    #[inline]
    fn write_u8(&mut self, value: u8) {
        self.write_u64(u64::from(value));
    }
    #[inline]
    fn write_u16(&mut self, value: u16) {
        self.write_u64(u64::from(value));
    }
    #[inline]
    fn write_u32(&mut self, value: u32) {
        self.write_u64(u64::from(value));
    }
    #[inline]
    fn write_u64(&mut self, value: u64) {
        match self {
            Self::Sip(hasher) => hasher.write_u64(value),
            Self::Fx(state) => Self::fx(state, value),
        }
    }
    #[inline]
    fn write_usize(&mut self, value: usize) {
        self.write_u64(value as u64);
    }
    #[inline]
    fn finish(&self) -> u64 {
        match self {
            Self::Sip(hasher) => hasher.finish(),
            // Fold the high bits down: hashbrown takes its control byte from
            // the top 7 bits and its bucket from the low ones.
            Self::Fx(state) => *state ^ (*state >> 29),
        }
    }
}

impl std::hash::BuildHasher for FrontierHash {
    type Hasher = FrontierHasher;
    #[inline]
    fn build_hasher(&self) -> FrontierHasher {
        match self {
            Self::Sip(state) => FrontierHasher::Sip(state.build_hasher()),
            Self::Fx => FrontierHasher::Fx(0),
        }
    }
}

/// Children must enter a later phase, or the current phase strictly below
/// their parent.
fn check_descent<P: Ord>(
    position: &OrderedPosition<P>,
    phase: usize,
    parent: &P,
    total_phases: usize,
) -> Result<(), ReductionError> {
    check_descent_parts(
        position.phase,
        &position.priority,
        phase,
        parent,
        total_phases,
    )
}

fn check_descent_parts<P: Ord>(
    entered: usize,
    priority: &P,
    phase: usize,
    parent: &P,
    total_phases: usize,
) -> Result<(), ReductionError> {
    if entered >= total_phases || entered < phase || (entered == phase && *priority >= *parent) {
        return Err(invariant(
            "ordered checked action does not strictly decrease its certified position",
        ));
    }
    Ok(())
}

/// Weight arithmetic of one Ordered run.
///
/// The scheduler's control flow depends on weights only through the zero
/// reports of `merge` and `merge_terminal` (exact-zero erasure). Every other
/// decision (popped keys, steps, children, positions, guards) is made from
/// integer keys and the adapters' exact step coefficients.
///
/// `admit` and `scale` receive adapter coefficients that the scheduler has
/// not validated; every implementation validates them against the request's
/// context and exact-algebra limits before using them (once per occurrence
/// or once per distinct value).
pub(crate) trait OrderedWeights {
    type Weight: Clone;
    /// The product of the active weight and one exact step coefficient.
    type Contribution;
    /// The image of a weight on which recorded arithmetics decide erasures
    /// (see `tail_probe`); `()` for value arithmetics.
    type Probe: Copy;

    /// Admit one nonzero input weight.
    fn admit(&mut self, coefficient: Coefficient) -> Result<Self::Weight, ReductionError>;
    /// The popped key's weight becomes the active weight.
    fn begin_step(&mut self, weight: &Self::Weight) -> Result<(), ReductionError>;
    /// The popped key's children and terminals have all been merged.
    fn end_step(&mut self, weight: Self::Weight);
    fn scale(
        &mut self,
        weight: &Self::Weight,
        coefficient: &Coefficient,
    ) -> Result<Self::Contribution, ReductionError>;
    /// `scale` with the coefficient's interning hash computed ahead
    /// (`modular::coefficient_hash`, rr-par; `None`: not computed).
    fn scale_hashed(
        &mut self,
        weight: &Self::Weight,
        coefficient: &Coefficient,
        _hash: Option<u64>,
    ) -> Result<Self::Contribution, ReductionError> {
        self.scale(weight, coefficient)
    }
    fn contribution_is_zero(&self, contribution: &Self::Contribution) -> bool;
    /// Store a new frontier entry from a nonzero contribution.
    fn insert(&mut self, contribution: Self::Contribution) -> Result<Self::Weight, ReductionError>;
    /// Add into an existing frontier entry. `true` reports an exactly zero
    /// sum; the scheduler then erases the entry and calls `cancel`.
    fn merge(
        &mut self,
        existing: &mut Self::Weight,
        contribution: Self::Contribution,
    ) -> Result<bool, ReductionError>;
    fn cancel(&mut self, removed: Self::Weight) -> Result<(), ReductionError>;
    fn insert_terminal(
        &mut self,
        contribution: Self::Contribution,
    ) -> Result<Self::Weight, ReductionError>;
    fn merge_terminal(
        &mut self,
        existing: &mut Self::Weight,
        contribution: Self::Contribution,
    ) -> Result<bool, ReductionError>;
    fn cancel_terminal(&mut self, removed: Self::Weight) -> Result<(), ReductionError>;
    /// Census of resident coefficient size (monomials for exact weights).
    fn monomials(&self, weight: &Self::Weight) -> Result<usize, ReductionError>;
    /// The exact value of a weight for phase inspection, when this arithmetic
    /// carries one.
    fn exact(weight: &Self::Weight) -> Option<&Coefficient>;
    /// The sequential scheduler starts `phase` (diagnostics only: recorded
    /// arithmetics time their replay per phase).
    fn phase_started(&mut self, _phase: usize) {}
    /// Recorded arithmetics only: the length of the recorded program, the
    /// point a later `rewrite_tail` truncates to. `None` for value
    /// arithmetics (nothing to rewrite).
    fn tail_checkpoint(&self) -> Option<usize> {
        None
    }
    /// Recorded arithmetics only: the recorded slot of a weight.
    fn tail_slot(_weight: &Self::Weight) -> Option<u32> {
        None
    }
    /// Recorded arithmetics only: replace the recorded program after
    /// `rewrite.ops` by `T += sum w(slot) * u` over the entries (see `tail`).
    /// `terminal` is the output weight of `T`: on return it is the weight of
    /// the rewritten accumulator when `rewrite.keep` (a new weight carries
    /// `probe`), else `None` (the accumulator is recorded as erased, which
    /// every replay verifies).
    fn rewrite_tail(
        &mut self,
        _rewrite: TailRewrite<'_>,
        _terminal: &mut Option<Self::Weight>,
        _probe: Option<Self::Probe>,
    ) -> Result<TailRewriteReport, ReductionError> {
        Err(invariant(
            "this weight arithmetic records no program to rewrite",
        ))
    }
    /// Arithmetics whose every erasure decision is "the probe of the merged
    /// weight is zero" (recorded arithmetics without exact cross-check):
    /// the probe of a weight. `None`: decisions are exact.
    fn tail_probe(&self, _weight: &Self::Weight) -> Option<Self::Probe> {
        None
    }
    /// The probe of an exact step coefficient, as `scale` uses it.
    fn tail_probe_coefficient(
        &self,
        _coefficient: &Coefficient,
    ) -> Result<Self::Probe, ReductionError> {
        Err(invariant("this weight arithmetic has no probes"))
    }
    /// `accumulator += a * b` on probes (`None`: nothing accumulated yet).
    fn tail_probe_add_product(
        &self,
        _accumulator: &mut Option<Self::Probe>,
        _a: &Self::Probe,
        _b: &Self::Probe,
    ) {
    }
    /// The erasure decision on a merged probe.
    fn tail_probe_is_zero(&self, _probe: &Self::Probe) -> bool {
        true
    }
    /// rr-frontier diagnostics: allocator bytes a clone of `weight`
    /// allocates (0: nothing on the heap, or not measured).
    fn weight_heap_bytes(_weight: &Self::Weight) -> usize {
        0
    }
    /// rr-frontier diagnostics: the weights' own resident tables.
    fn memstats(&self) -> serde_json::Value {
        serde_json::Value::Null
    }
}

/// Exact normalized rational-function weights (historical arithmetic).
///
/// Every input weight and every adapter-supplied step coefficient is
/// validated once on entry. All frontier and terminal weights are then
/// validated values or checked results of this context, so merges and
/// products never rescan their operands.
pub(crate) struct ExactWeights {
    context: CoefficientContext,
    limits: crate::algebra::ExactAlgebraLimits,
}

impl ExactWeights {
    pub(crate) fn new(context: &CoefficientContext, limits: ReductionLimits) -> Self {
        Self {
            context: context.clone(),
            limits: limits.exact_algebra,
        }
    }

    fn validate(&self, coefficient: &Coefficient) -> Result<(), ReductionError> {
        self.context
            .validate_with_limits(coefficient, self.limits)
            .map_err(ReductionError::from)
    }

    fn add(
        &self,
        existing: &mut Coefficient,
        contribution: Coefficient,
    ) -> Result<bool, ReductionError> {
        let sum = self
            .context
            .try_add_validated(existing, &contribution, self.limits)
            .map_err(ReductionError::from)?;
        let zero = sum.is_zero();
        if !zero {
            *existing = sum;
        }
        Ok(zero)
    }
}

fn is_one(coefficient: &Coefficient) -> bool {
    coefficient.numerator.is_one() && coefficient.denominator.is_one()
}

impl OrderedWeights for ExactWeights {
    type Weight = Coefficient;
    type Contribution = Coefficient;
    type Probe = ();

    fn admit(&mut self, coefficient: Coefficient) -> Result<Coefficient, ReductionError> {
        self.validate(&coefficient)?;
        Ok(coefficient)
    }
    fn begin_step(&mut self, _: &Coefficient) -> Result<(), ReductionError> {
        Ok(())
    }
    fn end_step(&mut self, _: Coefficient) {}
    /// `weight * coefficient`; an exact unit coefficient needs no arithmetic.
    fn scale(
        &mut self,
        weight: &Coefficient,
        coefficient: &Coefficient,
    ) -> Result<Coefficient, ReductionError> {
        self.validate(coefficient)?;
        if is_one(coefficient) {
            return Ok(weight.clone());
        }
        self.context
            .try_mul_validated(weight, coefficient, self.limits)
            .map_err(ReductionError::from)
    }
    fn contribution_is_zero(&self, contribution: &Coefficient) -> bool {
        contribution.is_zero()
    }
    fn insert(&mut self, contribution: Coefficient) -> Result<Coefficient, ReductionError> {
        Ok(contribution)
    }
    fn merge(
        &mut self,
        existing: &mut Coefficient,
        contribution: Coefficient,
    ) -> Result<bool, ReductionError> {
        self.add(existing, contribution)
    }
    fn cancel(&mut self, _: Coefficient) -> Result<(), ReductionError> {
        Ok(())
    }
    fn insert_terminal(
        &mut self,
        contribution: Coefficient,
    ) -> Result<Coefficient, ReductionError> {
        Ok(contribution)
    }
    fn merge_terminal(
        &mut self,
        existing: &mut Coefficient,
        contribution: Coefficient,
    ) -> Result<bool, ReductionError> {
        self.add(existing, contribution)
    }
    fn cancel_terminal(&mut self, _: Coefficient) -> Result<(), ReductionError> {
        Ok(())
    }
    fn monomials(&self, weight: &Coefficient) -> Result<usize, ReductionError> {
        coefficient_monomials(weight)
    }
    fn exact(weight: &Coefficient) -> Option<&Coefficient> {
        Some(weight)
    }
    fn weight_heap_bytes(weight: &Coefficient) -> usize {
        coefficient_heap_bytes(weight)
    }
}

/// rr-frontier diagnostics: the allocator bytes a clone of `coefficient`
/// allocates (term vectors; big-integer limbs are not counted).
pub(crate) fn coefficient_heap_bytes(coefficient: &Coefficient) -> usize {
    [&coefficient.numerator, &coefficient.denominator]
        .into_iter()
        .map(|p| {
            memstats::block(p.coefficients.as_ptr(), p.coefficients.capacity())
                + memstats::block(p.exponents.as_ptr(), p.exponents.capacity())
        })
        .sum()
}

/// Exact weights are values: products and sums are pure functions of their
/// operands and nothing is journaled.
impl ConcurrentWeights for ExactWeights {
    type Prepared = Coefficient;

    fn prepare(&self, coefficient: Coefficient) -> Result<Coefficient, ReductionError> {
        self.validate(&coefficient)?;
        Ok(coefficient)
    }
    fn prepared_coefficient(prepared: &Coefficient) -> &Coefficient {
        prepared
    }
    /// The product of [`ExactWeights::scale`] for an already validated
    /// coefficient.
    fn scale_prepared(
        &self,
        weight: &Coefficient,
        coefficient: &Coefficient,
    ) -> Result<Coefficient, ReductionError> {
        if is_one(coefficient) {
            return Ok(weight.clone());
        }
        self.context
            .try_mul_validated(weight, coefficient, self.limits)
            .map_err(ReductionError::from)
    }
    fn insert_detached(&self, contribution: Coefficient) -> Result<Coefficient, ReductionError> {
        Ok(contribution)
    }
    fn merge_detached(
        &self,
        existing: &mut Coefficient,
        contribution: Coefficient,
    ) -> Result<bool, ReductionError> {
        self.add(existing, contribution)
    }
    fn insert_terminal_detached(
        &self,
        contribution: Coefficient,
    ) -> Result<Coefficient, ReductionError> {
        Ok(contribution)
    }
    fn merge_terminal_detached(
        &self,
        existing: &mut Coefficient,
        contribution: Coefficient,
    ) -> Result<bool, ReductionError> {
        self.add(existing, contribution)
    }

    /// Normalized rational functions: sums are associative and the normal
    /// form is canonical.
    const CANONICAL_SUMS: bool = true;
    fn sum_weights(&self, a: &Coefficient, b: &Coefficient) -> Result<Coefficient, ReductionError> {
        self.context
            .try_add_validated(a, b, self.limits)
            .map_err(ReductionError::from)
    }
    fn weight_is_zero(&self, weight: &Coefficient) -> bool {
        weight.is_zero()
    }
}

/// Exact and modular arithmetic in one pass: the exact zero decision is
/// authoritative and every modular decision must agree with it.
struct CrossChecked {
    exact: ExactWeights,
    modular: ModularRecorder,
}

fn disagreement() -> ReductionError {
    modular::error("modular zero decision differs from the exact decision")
}

impl OrderedWeights for CrossChecked {
    type Weight = (Coefficient, modular::ModularWeight);
    type Contribution = (Coefficient, modular::ModularContribution);
    /// Erasures are decided exactly.
    type Probe = ();

    fn admit(&mut self, coefficient: Coefficient) -> Result<Self::Weight, ReductionError> {
        let exact = self.exact.admit(coefficient.clone())?;
        Ok((exact, self.modular.admit(coefficient)?))
    }
    fn begin_step(&mut self, weight: &Self::Weight) -> Result<(), ReductionError> {
        self.modular.begin_step(&weight.1)
    }
    fn end_step(&mut self, weight: Self::Weight) {
        self.modular.end_step(weight.1);
    }
    fn scale(
        &mut self,
        weight: &Self::Weight,
        coefficient: &Coefficient,
    ) -> Result<Self::Contribution, ReductionError> {
        Ok((
            self.exact.scale(&weight.0, coefficient)?,
            self.modular.scale(&weight.1, coefficient)?,
        ))
    }
    fn contribution_is_zero(&self, contribution: &Self::Contribution) -> bool {
        contribution.0.is_zero()
    }
    fn insert(&mut self, contribution: Self::Contribution) -> Result<Self::Weight, ReductionError> {
        Ok((contribution.0, self.modular.insert(contribution.1)?))
    }
    fn merge(
        &mut self,
        existing: &mut Self::Weight,
        contribution: Self::Contribution,
    ) -> Result<bool, ReductionError> {
        let exact = self.exact.merge(&mut existing.0, contribution.0)?;
        if self.modular.merge(&mut existing.1, contribution.1)? != exact {
            return Err(disagreement());
        }
        Ok(exact)
    }
    fn cancel(&mut self, removed: Self::Weight) -> Result<(), ReductionError> {
        self.modular.cancel(removed.1)
    }
    fn insert_terminal(
        &mut self,
        contribution: Self::Contribution,
    ) -> Result<Self::Weight, ReductionError> {
        Ok((
            contribution.0,
            self.modular.insert_terminal(contribution.1)?,
        ))
    }
    fn merge_terminal(
        &mut self,
        existing: &mut Self::Weight,
        contribution: Self::Contribution,
    ) -> Result<bool, ReductionError> {
        let exact = self.exact.merge_terminal(&mut existing.0, contribution.0)?;
        if self
            .modular
            .merge_terminal(&mut existing.1, contribution.1)?
            != exact
        {
            return Err(disagreement());
        }
        Ok(exact)
    }
    fn cancel_terminal(&mut self, removed: Self::Weight) -> Result<(), ReductionError> {
        self.modular.cancel_terminal(removed.1)
    }
    fn monomials(&self, weight: &Self::Weight) -> Result<usize, ReductionError> {
        self.exact.monomials(&weight.0)
    }
    fn exact(weight: &Self::Weight) -> Option<&Coefficient> {
        Some(&weight.0)
    }
    fn weight_heap_bytes(weight: &Self::Weight) -> usize {
        coefficient_heap_bytes(&weight.0)
    }
    fn memstats(&self) -> serde_json::Value {
        self.modular.memstats()
    }
    fn phase_started(&mut self, phase: usize) {
        self.modular.phase_started(phase);
    }
    /// The modular trace is rewritten; the exact weights stay those of the
    /// forward pass, so the final cross-check compares the adjoint tail with
    /// the forward one.
    fn tail_checkpoint(&self) -> Option<usize> {
        self.modular.tail_checkpoint()
    }
    fn tail_slot(weight: &Self::Weight) -> Option<u32> {
        ModularRecorder::tail_slot(&weight.1)
    }
    fn rewrite_tail(
        &mut self,
        rewrite: TailRewrite<'_>,
        terminal: &mut Option<Self::Weight>,
        _probe: Option<Self::Probe>,
    ) -> Result<TailRewriteReport, ReductionError> {
        // Only the forward decision is rewritten here (the exact weight is
        // the forward one).
        let mut modular = terminal.as_ref().map(|weight| weight.1);
        let report = self.modular.rewrite_tail(rewrite, &mut modular, None)?;
        if let (Some(weight), Some(modular)) = (terminal.as_mut(), modular) {
            weight.1 = modular;
        }
        Ok(report)
    }
}

#[allow(dead_code)]
/// Run one request with exact weights. Adapters must validate every original
/// input, including zero weights and canceled duplicates, before constructing
/// `initial`.
pub(crate) fn run<K: OrderedKernel>(
    kernel: &mut K,
    request: &mut ReductionRequest,
    initial: BTreeMap<K::Key, Coefficient>,
    nonzero_conditions: Vec<Coefficient>,
) -> Result<OrderedOutput, K::Error> {
    run_with_arithmetic(
        kernel,
        request,
        initial,
        nonzero_conditions,
        OrderedWeightArithmetic::Exact,
    )
}

/// Run one request with the selected weight arithmetic. Every mode expands
/// the same keys and returns the same exact terms and conditions.
///
/// This is the sequential scheduler: every step, product and merge runs on
/// the calling thread, and no worker pool or thread is created.
pub(crate) fn run_with_arithmetic<K: OrderedKernel>(
    kernel: &mut K,
    request: &mut ReductionRequest,
    initial: BTreeMap<K::Key, Coefficient>,
    nonzero_conditions: Vec<Coefficient>,
    arithmetic: OrderedWeightArithmetic,
) -> Result<OrderedOutput, K::Error> {
    run_sequential(
        kernel,
        request,
        initial,
        nonzero_conditions,
        arithmetic,
        TailOptions::sequential(false),
    )
}

/// How a request evaluates a single-terminal tail (see `tail`).
#[derive(Clone, Copy, Debug)]
struct TailOptions {
    enabled: bool,
    /// Threads for evaluating independent tail values (1: inline).
    threads: usize,
    /// The frontier of the sequential journal (M5 batch 2 profile).
    frontier: FrontierLayout,
    /// The first round of a modular polynomial lift (`None`: one prime
    /// per thread).
    modular_round: Option<usize>,
    /// rr-par: compute the next window of steps while the scheduler merges
    /// and records the current one (recorded arithmetics, several threads).
    pipeline: bool,
    /// rr-par: one replay job on the whole modular pool.
    par_replay: bool,
}

impl TailOptions {
    fn sequential(enabled: bool) -> Self {
        let scheduler = OrderedSchedulerOptions {
            adjoint_tail: enabled,
            ..OrderedSchedulerOptions::default()
        }
        .resolve_profile(OrderedProfile::Generic);
        Self::of(&scheduler, 1)
    }

    fn of(scheduler: &OrderedSchedulerOptions, threads: usize) -> Self {
        let tuning = scheduler.tuning();
        Self {
            enabled: tuning.adjoint_tail,
            threads,
            frontier: FrontierLayout {
                mode: tuning.frontier,
                fx_hash: tuning.fx_hash,
            },
            modular_round: tuning.modular_round,
            pipeline: tuning.pipeline && threads > 1,
            par_replay: tuning.par_replay,
        }
    }
}

fn run_sequential<K: OrderedKernel>(
    kernel: &mut K,
    request: &mut ReductionRequest,
    initial: BTreeMap<K::Key, Coefficient>,
    nonzero_conditions: Vec<Coefficient>,
    arithmetic: OrderedWeightArithmetic,
    tail: TailOptions,
) -> Result<OrderedOutput, K::Error> {
    #[cfg(test)]
    let arithmetic = test_override(arithmetic, kernel.context());
    let mut work = ReductionStatistics::default();
    let result = match arithmetic {
        OrderedWeightArithmetic::Exact => {
            let mut weights = ExactWeights::new(kernel.context(), kernel.limits());
            run_inner(
                kernel,
                &mut weights,
                request,
                initial,
                nonzero_conditions,
                &mut work,
                &mut Inline,
                tail,
            )
            .map(exact_output)
        }
        OrderedWeightArithmetic::ModularSampled(..)
        | OrderedWeightArithmetic::ModularPolynomial(..) => run_modular(
            kernel,
            request,
            initial,
            nonzero_conditions,
            arithmetic,
            &mut work,
            &mut Inline,
            tail,
        ),
    };
    kernel.record_work(work);
    result
}

/// Run one request with the selected weight arithmetic and scheduler
/// options. Every thread count expands the same keys in the same sequential
/// order and returns the same terms, guards (in the same order), counters
/// and observations; see `parallel` for the concurrent execution.
///
/// `scheduler.threads == 1` is the sequential scheduler of
/// [`run_with_arithmetic`]: no pool and no thread is created. Adapters
/// without a concurrent stepper and requests with diagnostic limits also run
/// sequentially, because diagnostic limits are checked at the exact point of
/// the sequential execution where they are crossed.
pub(crate) fn run_with_options<K: ConcurrentOrderedKernel>(
    kernel: &mut K,
    request: &mut ReductionRequest,
    initial: BTreeMap<K::Key, Coefficient>,
    nonzero_conditions: Vec<Coefficient>,
    arithmetic: OrderedWeightArithmetic,
    scheduler: OrderedSchedulerOptions,
) -> Result<OrderedOutput, K::Error> {
    #[cfg(test)]
    let (arithmetic, scheduler) = (
        test_override(arithmetic, kernel.context()),
        parallel::test_threads_override(scheduler),
    );
    scheduler.validate()?;
    let scheduler = scheduler.resolve_profile(OrderedProfile::Generic);
    let tail = TailOptions::of(&scheduler, scheduler.threads);
    if scheduler.threads == 1 || kernel.limits().diagnostics_enabled || kernel.stepper().is_none() {
        return run_sequential(
            kernel,
            request,
            initial,
            nonzero_conditions,
            arithmetic,
            TailOptions { threads: 1, ..tail },
        );
    }
    let workers = Workers::new(scheduler.threads)?;
    let mut work = ReductionStatistics::default();
    let result = match arithmetic {
        OrderedWeightArithmetic::Exact => {
            let mut weights = ExactWeights::new(kernel.context(), kernel.limits());
            parallel::run_concurrent(
                kernel,
                &mut weights,
                request,
                initial,
                nonzero_conditions,
                &mut work,
                &workers,
                scheduler.window_keys,
                tail.enabled,
                tail.frontier.fx_hash,
            )
            .map(exact_output)
        }
        // Recorded (sampled or polynomial) and cross-checked weights keep
        // their sequential journal; their steps are computed ahead of time by
        // the workers.
        OrderedWeightArithmetic::ModularSampled(..)
        | OrderedWeightArithmetic::ModularPolynomial(..) => {
            match tail.pipeline.then(|| kernel.detached_stepper()).flatten() {
                // rr-par: the next window is computed while this one is
                // merged and recorded.
                Some(stepper) => {
                    let (stepper, workers) = (&stepper, &workers);
                    workers.in_place_scope(|scope| {
                        run_modular(
                            kernel,
                            request,
                            initial,
                            nonzero_conditions,
                            arithmetic,
                            &mut work,
                            &mut Pipelined::new(scope, workers, stepper, scheduler.window_keys),
                            tail,
                        )
                    })
                }
                None => run_modular(
                    kernel,
                    request,
                    initial,
                    nonzero_conditions,
                    arithmetic,
                    &mut work,
                    &mut Prefetched::new(&workers, scheduler.window_keys),
                    tail,
                ),
            }
        }
    };
    kernel.record_work(work);
    result
}

/// Record the request once (steps from `source`), then evaluate and lift the
/// terminals modulo primes. With `cross_check_exact` the exact arithmetic
/// runs in the same pass: every zero decision and the final terminal map
/// must agree.
#[allow(clippy::too_many_arguments)]
fn run_modular<K: OrderedKernel, S: StepSource<K>>(
    kernel: &mut K,
    request: &mut ReductionRequest,
    initial: BTreeMap<K::Key, Coefficient>,
    nonzero_conditions: Vec<Coefficient>,
    arithmetic: OrderedWeightArithmetic,
    work: &mut ReductionStatistics,
    source: &mut S,
    tail: TailOptions,
) -> Result<OrderedOutput, K::Error> {
    let (common, lifter) = Lifter::of(arithmetic).expect("modular arithmetic");
    let mut recorder = ModularRecorder::try_new(kernel.context(), kernel.limits(), common, lifter)?;
    recorder.set_modular_round(tail.modular_round);
    recorder.set_par_replay(tail.par_replay);
    // A cross-checked request compares the exact terminal map itself, so it
    // never composes.
    let composer = (!common.cross_check_exact)
        .then(|| kernel.terminal_composer())
        .flatten();
    if common.cross_check_exact {
        let mut weights = CrossChecked {
            exact: ExactWeights::new(kernel.context(), kernel.limits()),
            modular: recorder,
        };
        run_inner(
            kernel,
            &mut weights,
            request,
            initial,
            nonzero_conditions,
            work,
            source,
            tail,
        )
        .and_then(|output| cross_checked_output(weights, output).map_err(Into::into))
    } else {
        let mut weights = recorder;
        run_inner(
            kernel,
            &mut weights,
            request,
            initial,
            nonzero_conditions,
            work,
            source,
            tail,
        )
        .and_then(|output| modular_output(weights, output, composer.as_deref()).map_err(Into::into))
    }
}

fn exact_output(output: RawOutput<Coefficient>) -> OrderedOutput {
    OrderedOutput {
        terms: output.terms,
        composed: None,
        nonzero_conditions: output.nonzero_conditions,
        work: output.work,
        progress: output.progress,
    }
}

/// Reconstruct the modular terminals and require them to equal the exact ones.
fn cross_checked_output(
    weights: CrossChecked,
    output: RawOutput<(Coefficient, modular::ModularWeight)>,
) -> Result<OrderedOutput, ReductionError> {
    let (exact, modular): (BTreeMap<_, _>, BTreeMap<_, _>) = output
        .terms
        .into_iter()
        .map(|(key, (e, m))| ((key.clone(), e), (key, m)))
        .unzip();
    let (reconstructed, statistics) = weights.modular.finish(modular, None)?;
    modular::report_statistics(&statistics);
    let modular::Finished::Terminals(reconstructed) = reconstructed else {
        return Err(modular::error(
            "a cross-checked request composed its terminals",
        ));
    };
    if reconstructed != exact {
        return Err(modular::error(
            "modular reconstruction differs from the exact terminal map",
        ));
    }
    Ok(OrderedOutput {
        terms: exact,
        composed: None,
        nonzero_conditions: output.nonzero_conditions,
        work: output.work,
        progress: output.progress,
    })
}

/// Replay the recorded trace and reconstruct the exact terminals, or only
/// their composition when `composer` plans one.
fn modular_output(
    weights: ModularRecorder,
    output: RawOutput<modular::ModularWeight>,
    composer: Option<&dyn OrderedTerminalComposer>,
) -> Result<OrderedOutput, ReductionError> {
    let (finished, statistics) = weights.finish(output.terms, composer)?;
    modular::report_statistics(&statistics);
    let (terms, composed) = match finished {
        modular::Finished::Terminals(terms) => (terms, None),
        modular::Finished::Composed(composed) => (BTreeMap::new(), Some(composed)),
    };
    Ok(OrderedOutput {
        terms,
        composed,
        nonzero_conditions: output.nonzero_conditions,
        work: output.work,
        progress: output.progress,
    })
}

/// `RUSTRED_TEST_ORDERED_MODULAR=1` (or `sampled`) runs every exact Ordered
/// unit test with cross-checked `ModularSampled` weights, and `polynomial`
/// with cross-checked `ModularPolynomial` weights (univariate contexts only).
#[cfg(test)]
fn test_override(
    arithmetic: OrderedWeightArithmetic,
    context: &CoefficientContext,
) -> OrderedWeightArithmetic {
    if arithmetic != OrderedWeightArithmetic::Exact || context.variables().len() > 1 {
        return arithmetic;
    }
    let common = ModularCommon {
        threads: 2,
        cross_check_exact: true,
        ..Default::default()
    };
    match std::env::var("RUSTRED_TEST_ORDERED_MODULAR").as_deref() {
        Ok("1" | "sampled") => OrderedWeightArithmetic::ModularSampled(
            common,
            SampledOptions {
                batch_points: 8,
                ..Default::default()
            },
        ),
        Ok("polynomial") => OrderedWeightArithmetic::ModularPolynomial(
            common,
            PolynomialOptions {
                lanes: 2,
                ..Default::default()
            },
        ),
        _ => arithmetic,
    }
}

struct RawOutput<W> {
    terms: BTreeMap<IntegralKey, W>,
    nonzero_conditions: Vec<Coefficient>,
    work: ReductionStatistics,
    progress: CandidateOrderedReductionProgress,
}

/// Where the scheduler obtains the checked step of a popped key.
trait StepSource<K: OrderedKernel> {
    /// The step of `node`, the greatest pending key of `phase`, which has
    /// just been popped from `frontier`.
    fn step<W>(
        &mut self,
        kernel: &mut K,
        frontier: &Frontier<K::Key, K::Priority, W>,
        node: &Scheduled<K::Key, K::Priority>,
        request: &mut ReductionRequest,
        work: &mut ReductionStatistics,
    ) -> Result<OrderedStep<K::Key>, K::Error>;

    /// Worker threads behind this source (diagnostics).
    fn threads(&self) -> usize {
        1
    }

    /// rr-par: the interning hashes of the coefficients of the step just
    /// returned (terms in order, then terminals), when computed ahead.
    fn take_hashes(&mut self) -> Option<Vec<u64>> {
        None
    }

    /// rr-par: whether spent step coefficients go to [`StepSource::dispose`]
    /// instead of being dropped by the scheduler.
    fn disposes(&self) -> bool {
        false
    }

    /// Drop spent step coefficients (on another thread).
    fn dispose(&mut self, spent: Vec<Coefficient>) {
        drop(spent);
    }
}

/// The adapter's own step, computed when the key is popped.
struct Inline;

impl<K: OrderedKernel> StepSource<K> for Inline {
    fn step<W>(
        &mut self,
        kernel: &mut K,
        _: &Frontier<K::Key, K::Priority, W>,
        node: &Scheduled<K::Key, K::Priority>,
        request: &mut ReductionRequest,
        work: &mut ReductionStatistics,
    ) -> Result<OrderedStep<K::Key>, K::Error> {
        kernel.step(&node.key, request, work)
    }
}

/// Scheduler state after admission of the request's inputs.
struct Admitted<K: OrderedKernel, A: OrderedWeights> {
    guards: GuardIndex,
    nonzero_conditions: Vec<Coefficient>,
    frontier: FrontierOf<K, A>,
    output: BTreeMap<IntegralKey, A::Weight>,
    progress: CandidateOrderedReductionProgress,
}

/// Intern the caller's conditions and admit every nonzero input weight.
fn admit<K: OrderedKernel, A: OrderedWeights>(
    kernel: &mut K,
    weights: &mut A,
    request: &mut ReductionRequest,
    initial: BTreeMap<K::Key, Coefficient>,
    mut nonzero_conditions: Vec<Coefficient>,
    layout: FrontierLayout,
) -> Result<Admitted<K, A>, K::Error> {
    let limits = kernel.limits();
    let mut guards = GuardIndex::new();
    // Use the same hash/equality guard interning as the depth-first engine.
    let initial_conditions = std::mem::take(&mut nonzero_conditions);
    for condition in &initial_conditions {
        guards.intern(&mut nonzero_conditions, condition, |_| Ok(()))?;
    }
    let mut frontier = Frontier::new(layout);
    let mut progress = CandidateOrderedReductionProgress {
        total_phases: kernel.phase_count(),
        input_keys: initial.len(),
        ..Default::default()
    };
    for (key, coefficient) in initial {
        if coefficient.is_zero() {
            continue;
        }
        let position = kernel.position(&key)?;
        if position.phase >= progress.total_phases {
            return Err(invariant("ordered entry phase is outside the frozen phase plan").into());
        }
        request.retain_pending_frame(limits.diagnostic_limit(limits.max_pending_frames))?;
        let weight = weights.admit(coefficient)?;
        replace_monomials(
            &mut progress.frontier_coefficient_monomials,
            0,
            weights.monomials(&weight)?,
        )?;
        frontier.insert(key, Pending { position, weight });
    }
    progress.frontier_keys = frontier.len();
    progress.peak_frontier_keys = frontier.len();
    progress.peak_frontier_coefficient_monomials = progress.frontier_coefficient_monomials;
    kernel.observe(&progress, None)?;
    Ok(Admitted {
        guards,
        nonzero_conditions,
        frontier,
        output: BTreeMap::new(),
        progress,
    })
}

impl<K: OrderedKernel, A: OrderedWeights> Admitted<K, A> {
    fn inspect(
        &self,
        kernel: &mut K,
        boundary: OrderedPhaseBoundary,
        phase: usize,
    ) -> Result<(), K::Error> {
        inspect::<K, A>(
            kernel,
            boundary,
            phase,
            &self.progress,
            SchedulerState::new(&self.frontier, &self.output, &self.nonzero_conditions),
        )
    }

    /// Check that `phase` has drained, then report and inspect its end.
    fn finish_phase(&mut self, kernel: &mut K, phase: usize) -> Result<(), K::Error> {
        if self.frontier.earlier_than(phase) {
            return Err(invariant("ordered phase was regenerated after completion").into());
        }
        self.progress.completed_phases = phase + 1;
        kernel.observe(&self.progress, None)?;
        self.inspect(kernel, OrderedPhaseBoundary::Finished, phase)
    }

    fn finish(self, work: &ReductionStatistics) -> Result<RawOutput<A::Weight>, K::Error> {
        if !self.frontier.is_empty() {
            return Err(invariant("ordered frontier remains after the last phase").into());
        }
        Ok(RawOutput {
            terms: self.output,
            nonzero_conditions: self.nonzero_conditions,
            work: *work,
            progress: self.progress,
        })
    }
}

/// rr-frontier diagnostics: one snapshot of the resident scheduler state
/// (see `memstats`). Reads and clones only.
fn memstats_snapshot<K: OrderedKernel, A: OrderedWeights>(
    weights: &A,
    frontier: &FrontierOf<K, A>,
    output: &BTreeMap<IntegralKey, A::Weight>,
    progress: &CandidateOrderedReductionProgress,
    phase: usize,
) -> serde_json::Value {
    use std::mem::size_of;
    let started = std::time::Instant::now();
    let rss = memstats::rss_bytes();
    let in_use = memstats::in_use();
    let (arena, free) = memstats::arena();
    let [mut p_key, mut p_prio, mut p_shared, mut p_weight] = [0usize; 4];
    let [mut o_key, mut o_prio] = [0usize; 2];
    let (mut pending_clone, mut order_clone, mut capacity, mut order_len) = (0, 0, 0, 0);
    let mut packed = serde_json::Value::Null;
    match frontier {
        Frontier::Classic(classic) => {
            for (key, pending) in &classic.pending {
                let [u, s] = K::priority_heap_bytes(&pending.position.priority);
                p_key += K::key_heap_bytes(key);
                p_prio += u;
                p_shared += s;
                p_weight += A::weight_heap_bytes(&pending.weight);
            }
            for scheduled in &classic.order {
                o_key += K::key_heap_bytes(&scheduled.key);
                o_prio += K::priority_heap_bytes(&scheduled.priority)[0];
            }
            pending_clone = memstats::measure_clone(|| classic.pending.clone());
            order_clone = memstats::measure_clone(|| classic.order.clone());
            capacity = classic.pending.capacity();
            order_len = classic.order.len();
        }
        Frontier::Packed(p) => {
            for (priority, weight) in p.live() {
                let [u, s] = K::priority_heap_bytes(priority);
                p_prio += u;
                p_shared += s;
                p_weight += A::weight_heap_bytes(weight);
            }
            packed = p.memstats();
        }
    }
    let terminals_clone = memstats::measure_clone(|| output.clone());
    let buckets = if capacity < 8 {
        capacity + 1
    } else {
        (capacity * 8 / 7).next_power_of_two()
    };
    let bucket = size_of::<(K::Key, Pending<K::Priority, A::Weight>)>();
    serde_json::json!({
        "phase": phase,
        "pending_keys": frontier.len(),
        "order_entries": order_len,
        "packed": packed,
        "processed_keys": progress.processed_keys,
        "emitted_terms": progress.emitted_terms,
        "terminal_keys": output.len(),
        "rss_bytes": rss,
        "malloc_in_use_bytes": in_use,
        "malloc_arena_bytes": arena,
        "malloc_free_bytes": free,
        "sizes": {
            "key": size_of::<K::Key>(),
            "priority": size_of::<K::Priority>(),
            "weight": size_of::<A::Weight>(),
            "position": size_of::<OrderedPosition<K::Priority>>(),
            "pending": size_of::<Pending<K::Priority, A::Weight>>(),
            "scheduled": size_of::<Scheduled<K::Key, K::Priority>>(),
            "bucket": bucket,
        },
        "pending_map": {
            "capacity": capacity,
            "buckets": buckets,
            "table_bytes_formula": buckets * (bucket + 1) + 16 + 8,
            "clone_bytes": pending_clone,
            "key_heap_bytes": p_key,
            "priority_heap_bytes": p_prio,
            "priority_shared_heap_bytes": p_shared,
            "weight_heap_bytes": p_weight,
        },
        "order_set": {
            "clone_bytes": order_clone,
            "key_heap_bytes": o_key,
            "priority_heap_bytes": o_prio,
            "node_bytes": order_clone.saturating_sub(o_key + o_prio),
        },
        "terminals": { "len": output.len(), "clone_bytes": terminals_clone },
        "weights": weights.memstats(),
        "snapshot_seconds": started.elapsed().as_secs_f64(),
    })
}

/// The sequential scheduler: pop the greatest pending key, apply its checked
/// step and merge the weighted children and terminals, one key at a time.
fn run_inner<K: OrderedKernel, A: OrderedWeights, S: StepSource<K>>(
    kernel: &mut K,
    weights: &mut A,
    request: &mut ReductionRequest,
    initial: BTreeMap<K::Key, Coefficient>,
    nonzero_conditions: Vec<Coefficient>,
    work: &mut ReductionStatistics,
    steps: &mut S,
    tail: TailOptions,
) -> Result<RawOutput<A::Weight>, K::Error> {
    let limits = kernel.limits();
    let mut state = admit(
        kernel,
        weights,
        request,
        initial,
        nonzero_conditions,
        tail.frontier,
    )?;
    let mut tail = if tail.enabled {
        TailRun::new(kernel, weights, tail.threads, true)?
    } else {
        None
    };
    let context = kernel.context().clone();
    let mut mem_probe = memstats::Probe::new();
    let mut par_clock = parstats::ForwardClock::new();
    let disposing = steps.disposes();
    let mut spent: Vec<Coefficient> = Vec::new();
    for phase in 0..state.progress.total_phases {
        let phase_started = par_clock.now();
        let (mut par_pops, mut par_source) = (0u64, 0.0f64);
        // Phase marker first: a tail closed at this phase records its
        // products after the marker, so the replay attributes them here.
        weights.phase_started(phase);
        if let Some(tail) = &mut tail {
            tail.phase_started(kernel, weights, request, work, &mut state, phase)?;
        }
        state.inspect(kernel, OrderedPhaseBoundary::Started, phase)?;
        let Admitted {
            guards,
            nonzero_conditions,
            frontier,
            output,
            progress,
        } = &mut state;
        while let Some((node, weight)) = frontier.pop_in_phase(phase) {
            request.release_pending_frame()?;
            replace_monomials(
                &mut progress.frontier_coefficient_monomials,
                weights.monomials(&weight)?,
                0,
            )?;
            progress.frontier_keys = frontier.len();
            kernel.observe(progress, Some(&node.key))?;
            let source_started = par_clock.now();
            let checked = steps.step(kernel, frontier, &node, request, work)?;
            let hashes = steps.take_hashes();
            let term_count = checked.terms.len();
            if let Some(started) = source_started {
                par_pops += 1;
                par_source += started.elapsed().as_secs_f64();
            }
            if let Some(tail) = &mut tail {
                tail.observe_step(&node, &checked, &context, limits.exact_algebra)?;
            }
            for condition in &checked.nonzero_conditions {
                guards.intern(nonzero_conditions, condition, |_| Ok(()))?;
            }
            weights.begin_step(&weight)?;
            for (index, (child, coefficient)) in checked.terms.into_iter().enumerate() {
                if coefficient.is_zero() {
                    continue;
                }
                let hash = hashes.as_ref().map(|hashes| hashes[index]);
                progress.emitted_terms = progress.emitted_terms.saturating_add(1);
                let removed = frontier.child(child, |found| {
                    Ok::<_, K::Error>(match found {
                        Child::Occupied {
                            phase: entered,
                            priority,
                            weight: existing,
                        } => {
                            // A pending key keeps the position certified at entry.
                            check_descent_parts(
                                entered,
                                priority,
                                phase,
                                &node.priority,
                                progress.total_phases,
                            )?;
                            let contribution = weights.scale_hashed(&weight, &coefficient, hash)?;
                            work.record_coalescing_additions(1);
                            request.record_coalescing_additions(
                                1,
                                limits.diagnostic_limit(limits.max_coalescing_additions),
                            )?;
                            let old = weights.monomials(existing)?;
                            let zero = weights.merge(existing, contribution)?;
                            let new = if zero {
                                0
                            } else {
                                weights.monomials(existing)?
                            };
                            replace_monomials(
                                &mut progress.frontier_coefficient_monomials,
                                old,
                                new,
                            )?;
                            if zero {
                                ChildAction::Remove
                            } else {
                                progress.merged_terms = progress.merged_terms.saturating_add(1);
                                ChildAction::Keep
                            }
                        }
                        Child::Vacant(key) => {
                            let position = kernel.position(key)?;
                            check_descent(&position, phase, &node.priority, progress.total_phases)?;
                            let contribution = weights.scale_hashed(&weight, &coefficient, hash)?;
                            if weights.contribution_is_zero(&contribution) {
                                ChildAction::Keep
                            } else {
                                request.retain_pending_frame(
                                    limits.diagnostic_limit(limits.max_pending_frames),
                                )?;
                                let inserted = weights.insert(contribution)?;
                                replace_monomials(
                                    &mut progress.frontier_coefficient_monomials,
                                    0,
                                    weights.monomials(&inserted)?,
                                )?;
                                ChildAction::Insert(Pending {
                                    position,
                                    weight: inserted,
                                })
                            }
                        }
                    })
                })?;
                if disposing {
                    spent.push(coefficient);
                }
                if let Some(removed) = removed {
                    weights.cancel(removed)?;
                    request.release_pending_frame()?;
                    progress.cancelled_terms = progress.cancelled_terms.saturating_add(1);
                }
                progress.peak_frontier_keys = progress.peak_frontier_keys.max(frontier.len());
                progress.peak_frontier_coefficient_monomials = progress
                    .peak_frontier_coefficient_monomials
                    .max(progress.frontier_coefficient_monomials);
            }
            for (index, (terminal, coefficient)) in checked.terminals.into_iter().enumerate() {
                let hash = hashes.as_ref().map(|hashes| hashes[term_count + index]);
                let contribution = weights.scale_hashed(&weight, &coefficient, hash)?;
                if weights.contribution_is_zero(&contribution) {
                    continue;
                }
                progress.emitted_terms = progress.emitted_terms.saturating_add(1);
                merge_terminal_contribution(
                    weights,
                    output,
                    progress,
                    work,
                    request,
                    limits,
                    terminal,
                    contribution,
                )?;
            }
            weights.end_step(weight);
            if spent.len() >= 4096 {
                steps.dispose(std::mem::take(&mut spent));
            }
            progress.processed_keys = progress.processed_keys.saturating_add(1);
            progress.frontier_keys = frontier.len();
            progress.peak_frontier_keys = progress.peak_frontier_keys.max(frontier.len());
            progress.terminal_keys = output.len();
            kernel.observe(&progress, Some(&node.key))?;
            if let Some(probe) = &mut mem_probe
                && probe.due(frontier.len())
            {
                let snapshot =
                    memstats_snapshot::<K, A>(weights, frontier, output, progress, phase);
                probe.taken(frontier.len(), snapshot);
            }
        }
        if let Some(started) = phase_started {
            let wall = started.elapsed().as_secs_f64();
            par_clock.phase(phase, par_pops, wall, par_source, wall - par_source);
        }
        if let Some(tail) = &mut tail {
            tail.phase_finished(phase);
        }
        state.finish_phase(kernel, phase)?;
    }
    if let Some(tail) = tail {
        tail.finish(kernel, weights, request, work, &mut state)?;
    }
    par_clock.report(steps.threads());
    if let Some(probe) = &mem_probe {
        probe.write(serde_json::json!({
            "rss_bytes": memstats::rss_bytes(),
            "malloc_in_use_bytes": memstats::in_use(),
            "peak_frontier_keys": state.progress.peak_frontier_keys,
            "terminals": { "len": state.output.len(),
                "clone_bytes": memstats::measure_clone(|| state.output.clone()) },
            "weights": weights.memstats(),
        }));
    }
    state.finish(work)
}

/// Add one nonzero terminal contribution to `output`, with the counters and
/// limits of a terminal emission.
#[allow(clippy::too_many_arguments)]
fn merge_terminal_contribution<A: OrderedWeights>(
    weights: &mut A,
    output: &mut BTreeMap<IntegralKey, A::Weight>,
    progress: &mut CandidateOrderedReductionProgress,
    work: &mut ReductionStatistics,
    request: &mut ReductionRequest,
    limits: ReductionLimits,
    terminal: IntegralKey,
    contribution: A::Contribution,
) -> Result<(), ReductionError> {
    match output.entry(terminal) {
        btree_map::Entry::Occupied(mut entry) => {
            work.record_coalescing_additions(1);
            request.record_coalescing_additions(
                1,
                limits.diagnostic_limit(limits.max_coalescing_additions),
            )?;
            let existing = entry.get_mut();
            let old = weights.monomials(existing)?;
            let zero = weights.merge_terminal(existing, contribution)?;
            let new = if zero {
                0
            } else {
                weights.monomials(existing)?
            };
            replace_monomials(&mut progress.terminal_coefficient_monomials, old, new)?;
            if zero {
                weights.cancel_terminal(entry.remove())?;
                progress.terminal_cancellations = progress.terminal_cancellations.saturating_add(1);
            } else {
                progress.terminal_merges = progress.terminal_merges.saturating_add(1);
            }
        }
        btree_map::Entry::Vacant(entry) => {
            let inserted = weights.insert_terminal(contribution)?;
            replace_monomials(
                &mut progress.terminal_coefficient_monomials,
                0,
                weights.monomials(&inserted)?,
            )?;
            entry.insert(inserted);
        }
    }
    progress.peak_terminal_coefficient_monomials = progress
        .peak_terminal_coefficient_monomials
        .max(progress.terminal_coefficient_monomials);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct PolynomialKernel {
        context: CoefficientContext,
        observations: Vec<CandidateOrderedReductionProgress>,
        expanded: Vec<usize>,
    }

    impl OrderedKernel for PolynomialKernel {
        type Key = usize;
        type Priority = usize;
        type Error = ReductionError;

        fn context(&self) -> &CoefficientContext {
            &self.context
        }
        fn limits(&self) -> ReductionLimits {
            ReductionLimits::default()
        }
        fn phase_count(&self) -> usize {
            1
        }
        fn position(&mut self, key: &usize) -> Result<OrderedPosition<usize>, ReductionError> {
            Ok(OrderedPosition {
                phase: 0,
                priority: *key,
            })
        }
        fn step(
            &mut self,
            key: &usize,
            _: &mut ReductionRequest,
            _: &mut ReductionStatistics,
        ) -> Result<OrderedStep<usize>, ReductionError> {
            self.expanded.push(*key);
            let terminal = IntegralKey::try_new([1]).unwrap();
            Ok(match key {
                5 | 4 => {
                    let (polynomial, constant) = if *key == 5 { ("d+1", 7) } else { ("1-d", -7) };
                    OrderedStep {
                        terms: BTreeMap::from([
                            (0, self.context.integer(constant)),
                            (1, self.context.coefficient_fixture(polynomial)),
                        ]),
                        terminals: BTreeMap::from([(
                            terminal,
                            self.context.coefficient_fixture(polynomial),
                        )]),
                        nonzero_conditions: vec![],
                    }
                }
                1 => OrderedStep {
                    terminals: BTreeMap::from([(terminal, self.context.integer(-1))]),
                    ..Default::default()
                },
                _ => panic!("the canceled integral must not be expanded"),
            })
        }
        fn observe(
            &mut self,
            progress: &CandidateOrderedReductionProgress,
            _: Option<&usize>,
        ) -> Result<(), ReductionError> {
            self.observations.push(*progress);
            Ok(())
        }
    }

    #[test]
    fn coefficient_census_tracks_polynomial_shrink_and_zero_erasure_incrementally() {
        let mut kernel = PolynomialKernel {
            context: CoefficientContext::try_new(["d"]).unwrap(),
            observations: vec![],
            expanded: vec![],
        };
        let initial = BTreeMap::from([(5, kernel.context.one()), (4, kernel.context.one())]);
        let result = run(
            &mut kernel,
            &mut ReductionRequest::default(),
            initial,
            vec![],
        )
        .unwrap();
        assert_eq!(kernel.expanded, [5, 4, 1]);
        assert!(result.terms.is_empty());
        let after_first = kernel
            .observations
            .iter()
            .find(|p| p.processed_keys == 1)
            .unwrap();
        assert_eq!(
            (
                after_first.frontier_keys,
                after_first.frontier_coefficient_monomials,
                after_first.terminal_coefficient_monomials
            ),
            (3, 7, 3)
        );
        let after_second = kernel
            .observations
            .iter()
            .find(|p| p.processed_keys == 2)
            .unwrap();
        // d+1 + (1-d) leaves the same surviving key with a smaller constant
        // coefficient; the other key cancels exactly and must lose its size.
        assert_eq!(
            (
                after_second.frontier_keys,
                after_second.frontier_coefficient_monomials,
                after_second.terminal_coefficient_monomials
            ),
            (1, 2, 2)
        );
        assert_eq!(
            (
                result.progress.cancelled_terms,
                result.progress.merged_terms,
                result.progress.terminal_cancellations
            ),
            (1, 1, 1)
        );
        assert_eq!(
            (
                result.progress.frontier_coefficient_monomials,
                result.progress.terminal_coefficient_monomials
            ),
            (0, 0)
        );
        assert_eq!(
            (
                result.progress.peak_frontier_coefficient_monomials,
                result.progress.peak_terminal_coefficient_monomials
            ),
            (7, 3)
        );
    }

    fn modular(cross_check_exact: bool) -> OrderedWeightArithmetic {
        OrderedWeightArithmetic::ModularSampled(
            ModularCommon {
                threads: 3,
                cross_check_exact,
                ..Default::default()
            },
            SampledOptions {
                batch_points: 4,
                ..Default::default()
            },
        )
    }

    #[test]
    fn modular_weights_erase_the_same_canceled_keys() {
        for cross_check in [false, true] {
            let mut kernel = PolynomialKernel {
                context: CoefficientContext::try_new(["d"]).unwrap(),
                observations: vec![],
                expanded: vec![],
            };
            let initial = BTreeMap::from([(5, kernel.context.one()), (4, kernel.context.one())]);
            let result = run_with_arithmetic(
                &mut kernel,
                &mut ReductionRequest::default(),
                initial,
                vec![],
                modular(cross_check),
            )
            .unwrap();
            assert_eq!(kernel.expanded, [5, 4, 1]);
            assert!(result.terms.is_empty());
            assert_eq!(
                (
                    result.progress.cancelled_terms,
                    result.progress.merged_terms,
                    result.progress.terminal_cancellations
                ),
                (1, 1, 1)
            );
        }
    }

    /// A ladder with large integers, poles and repeated merges.
    struct LadderKernel {
        context: CoefficientContext,
        length: usize,
    }

    impl OrderedKernel for LadderKernel {
        type Key = usize;
        type Priority = usize;
        type Error = ReductionError;

        fn context(&self) -> &CoefficientContext {
            &self.context
        }
        fn limits(&self) -> ReductionLimits {
            ReductionLimits::default()
        }
        fn phase_count(&self) -> usize {
            2
        }
        fn position(&mut self, key: &usize) -> Result<OrderedPosition<usize>, ReductionError> {
            Ok(OrderedPosition {
                phase: usize::from(*key < self.length / 2),
                priority: *key,
            })
        }
        fn step(
            &mut self,
            key: &usize,
            _: &mut ReductionRequest,
            _: &mut ReductionStatistics,
        ) -> Result<OrderedStep<usize>, ReductionError> {
            let k = *key as i64;
            let c = |text: String| self.context.coefficient_fixture(&text);
            let terminal = |i: i64| IntegralKey::try_new([i]).unwrap();
            if *key < 3 {
                return Ok(OrderedStep {
                    terminals: BTreeMap::from([
                        (terminal(k), c(format!("(d-{k})/(2*d+{k}+1)"))),
                        (terminal(7), c(format!("{}", 3 - k))),
                    ]),
                    nonzero_conditions: vec![c(format!("2*d+{k}+1"))],
                    ..Default::default()
                });
            }
            Ok(OrderedStep {
                terms: BTreeMap::from([
                    (
                        key - 1,
                        c(format!("(123456789012345678901*d^2-{k}*d+5)/(3*d+{k})")),
                    ),
                    (key - 2, c(format!("(d+{k})/(7*d-{})", 2 * k + 1))),
                    (key - 3, c(format!("-{k}/98765432109876543211"))),
                ]),
                terminals: BTreeMap::from([(terminal(9), c(format!("d^{}", k % 3)))]),
                nonzero_conditions: vec![c(format!("3*d+{k}"))],
            })
        }
    }

    #[test]
    fn modular_weights_reconstruct_large_exact_ladder_results() {
        let context = CoefficientContext::try_new(["d"]).unwrap();
        let initial = BTreeMap::from([
            (13, context.coefficient_fixture("(d^3-2)/(d-11)")),
            (11, context.coefficient_fixture("1/3")),
        ]);
        let exact = run(
            &mut LadderKernel {
                context: context.clone(),
                length: 13,
            },
            &mut ReductionRequest::default(),
            initial.clone(),
            vec![context.coefficient_fixture("d-11")],
        )
        .unwrap();
        for cross_check in [false, true] {
            let result = run_with_arithmetic(
                &mut LadderKernel {
                    context: context.clone(),
                    length: 13,
                },
                &mut ReductionRequest::default(),
                initial.clone(),
                vec![context.coefficient_fixture("d-11")],
                modular(cross_check),
            )
            .unwrap();
            assert_eq!(result.terms, exact.terms);
            assert_eq!(result.nonzero_conditions, exact.nonzero_conditions);
            assert_eq!(
                result.progress.processed_keys,
                exact.progress.processed_keys
            );
        }
        assert_eq!(exact.terms.len(), 5);
    }

    fn ladder() -> (LadderKernel, BTreeMap<usize, Coefficient>, Vec<Coefficient>) {
        let context = CoefficientContext::try_new(["d"]).unwrap();
        let initial = BTreeMap::from([
            (13, context.coefficient_fixture("(d^3-2)/(d-11)")),
            (11, context.coefficient_fixture("1/3")),
        ]);
        let conditions = vec![context.coefficient_fixture("d-11")];
        (
            LadderKernel {
                context,
                length: 13,
            },
            initial,
            conditions,
        )
    }

    fn sampled(threads: usize, batch_points: usize) -> OrderedWeightArithmetic {
        OrderedWeightArithmetic::ModularSampled(
            ModularCommon {
                threads,
                ..Default::default()
            },
            SampledOptions {
                batch_points,
                ..Default::default()
            },
        )
    }

    fn run_reported(
        arithmetic: OrderedWeightArithmetic,
    ) -> (OrderedOutput, modular::ModularStatistics) {
        let (mut kernel, initial, conditions) = ladder();
        modular::take_reported_statistics();
        let result = run_with_arithmetic(
            &mut kernel,
            &mut ReductionRequest::default(),
            initial,
            conditions,
            arithmetic,
        )
        .unwrap();
        let mut reported = modular::take_reported_statistics();
        assert_eq!(reported.len(), 1);
        (result, reported.pop().unwrap())
    }

    #[test]
    fn modular_sampled_results_do_not_depend_on_the_thread_count() {
        let (mut kernel, initial, conditions) = ladder();
        let exact = run(
            &mut kernel,
            &mut ReductionRequest::default(),
            initial,
            conditions,
        )
        .unwrap();
        let (_, reference) = run_reported(sampled(1, 8));
        for threads in [1, 2, 4, 7] {
            let (result, statistics) = run_reported(sampled(threads, 8));
            assert_eq!(result.terms, exact.terms);
            assert_eq!(result.nonzero_conditions, exact.nonzero_conditions);
            assert_eq!(
                result.progress.processed_keys,
                exact.progress.processed_keys
            );
            // Images are consumed in prime order whatever ran ahead.
            let consumed = |s: &modular::ModularStatistics| {
                (
                    s.primes,
                    s.verification_primes,
                    s.unusable_primes,
                    s.unlucky_images,
                    s.points_first_prime,
                    s.points_per_prime,
                    s.max_numerator_degree,
                    s.max_denominator_degree,
                    s.max_coefficient_bits,
                )
            };
            assert_eq!(consumed(&statistics), consumed(&reference), "{threads}");
            assert_eq!(
                statistics.pool_threads,
                if threads == 1 { 0 } else { threads }
            );
        }
    }

    #[test]
    fn one_modular_thread_runs_inline_without_a_pool() {
        let pools = modular::pools_built_on_this_thread();
        let (_, statistics) = run_reported(sampled(1, 8));
        assert_eq!(modular::pools_built_on_this_thread(), pools);
        assert_eq!(
            (
                statistics.pool_threads,
                statistics.job_threads,
                statistics.jobs_on_caller_only
            ),
            (0, 1, true)
        );
        let (_, statistics) = run_reported(sampled(3, 8));
        assert_eq!(modular::pools_built_on_this_thread(), pools + 1);
        assert_eq!(statistics.pool_threads, 3);
        assert!(!statistics.jobs_on_caller_only);
    }

    #[test]
    fn modular_sampled_replays_only_the_points_it_needs() {
        let (_, statistics) = run_reported(sampled(1, 16));
        // Discovery doubles from 8 points until the interpolations converge,
        // then every prime replays exactly the needed points, and the
        // verification prime its checks; nothing is computed ahead of need.
        let needed = statistics.points_per_prime;
        let first = statistics.points_first_prime;
        assert!(first.is_power_of_two() && first >= needed && first < 2 * needed.max(8));
        assert_eq!(
            statistics.points_replayed,
            first
                + (statistics.primes - 1) * needed
                + statistics.verification_primes * SampledOptions::default().verification_points,
            "{}",
            statistics.to_json()
        );
        assert_eq!(statistics.verification_primes, 1);
        assert_eq!(statistics.discarded_images, 0);
        assert_eq!(statistics.rounds, statistics.primes);
        // With many threads at most one round of images runs ahead.
        let (_, wide) = run_reported(sampled(16, 16));
        assert_eq!(wide.primes, statistics.primes);
        assert!(wide.discarded_images < 16, "{}", wide.discarded_images);
    }

    fn polynomial(
        threads: usize,
        lanes: usize,
        denominators: PolynomialDenominators,
        cross_check_exact: bool,
    ) -> OrderedWeightArithmetic {
        OrderedWeightArithmetic::ModularPolynomial(
            ModularCommon {
                threads,
                cross_check_exact,
                ..Default::default()
            },
            PolynomialOptions {
                lanes,
                denominators,
                ..Default::default()
            },
        )
    }

    #[test]
    fn modular_polynomial_reconstructs_the_exact_ladder_for_every_thread_count() {
        let (mut kernel, initial, conditions) = ladder();
        let exact = run(
            &mut kernel,
            &mut ReductionRequest::default(),
            initial,
            conditions,
        )
        .unwrap();
        let (_, reference) =
            run_reported(polynomial(1, 4, PolynomialDenominators::Factorized, false));
        assert_eq!(reference.arithmetic, "modular_polynomial");
        // The ladder's denominators: 2d+k+1, 3d+k, 7d-2k-1, d-11 (all linear).
        assert!(reference.basis_factors > 0);
        assert_eq!(reference.basis_factors, reference.basis_linear_factors);
        for (threads, lanes) in [(1, 1), (1, 4), (2, 1), (4, 3), (7, 2)] {
            for denominators in [
                PolynomialDenominators::Factorized,
                PolynomialDenominators::Dense,
            ] {
                for cross_check in [false, true] {
                    let (result, statistics) =
                        run_reported(polynomial(threads, lanes, denominators, cross_check));
                    assert_eq!(result.terms, exact.terms);
                    assert_eq!(result.nonzero_conditions, exact.nonzero_conditions);
                    assert_eq!(
                        result.progress.processed_keys,
                        exact.progress.processed_keys
                    );
                    // Images are consumed in prime order whatever ran ahead.
                    let consumed = |s: &modular::ModularStatistics| {
                        (
                            s.primes,
                            s.verification_primes,
                            s.unusable_primes,
                            s.unlucky_images,
                            s.max_numerator_degree,
                            s.max_denominator_degree,
                            s.max_coefficient_bits,
                        )
                    };
                    assert_eq!(
                        consumed(&statistics),
                        consumed(&reference),
                        "{threads} {lanes}"
                    );
                    assert_eq!(
                        statistics.pool_threads,
                        if threads == 1 { 0 } else { threads }
                    );
                }
            }
        }
    }

    #[test]
    fn modular_polynomial_erases_the_same_cancelled_keys() {
        for threads in [1, 3] {
            let mut kernel = PolynomialKernel {
                context: CoefficientContext::try_new(["d"]).unwrap(),
                observations: vec![],
                expanded: vec![],
            };
            let initial = BTreeMap::from([(5, kernel.context.one()), (4, kernel.context.one())]);
            let result = run_with_arithmetic(
                &mut kernel,
                &mut ReductionRequest::default(),
                initial,
                vec![],
                polynomial(threads, 2, PolynomialDenominators::Factorized, true),
            )
            .unwrap();
            assert_eq!(kernel.expanded, [5, 4, 1]);
            assert!(result.terms.is_empty());
            assert_eq!(
                (
                    result.progress.cancelled_terms,
                    result.progress.merged_terms,
                    result.progress.terminal_cancellations
                ),
                (1, 1, 1)
            );
        }
    }

    #[test]
    fn one_modular_polynomial_thread_runs_inline_without_a_pool() {
        let pools = modular::pools_built_on_this_thread();
        let (_, statistics) =
            run_reported(polynomial(1, 4, PolynomialDenominators::Factorized, false));
        assert_eq!(modular::pools_built_on_this_thread(), pools);
        assert_eq!(
            (
                statistics.pool_threads,
                statistics.job_threads,
                statistics.jobs_on_caller_only
            ),
            (0, 1, true)
        );
        // The first round fills the threads with one prime each.
        let (_, statistics) =
            run_reported(polynomial(3, 4, PolynomialDenominators::Factorized, false));
        assert_eq!(modular::pools_built_on_this_thread(), pools + 1);
        assert_eq!(statistics.pool_threads, 3);
        assert!(statistics.lanes_replayed >= 3);
    }

    #[test]
    fn modular_weights_support_constant_contexts_and_reject_multivariate_ones() {
        let constant = CoefficientContext::try_new(Vec::<String>::new()).unwrap();
        struct Constant(CoefficientContext);
        impl OrderedKernel for Constant {
            type Key = usize;
            type Priority = usize;
            type Error = ReductionError;
            fn context(&self) -> &CoefficientContext {
                &self.0
            }
            fn limits(&self) -> ReductionLimits {
                ReductionLimits::default()
            }
            fn phase_count(&self) -> usize {
                1
            }
            fn position(&mut self, key: &usize) -> Result<OrderedPosition<usize>, ReductionError> {
                Ok(OrderedPosition {
                    phase: 0,
                    priority: *key,
                })
            }
            fn step(
                &mut self,
                key: &usize,
                _: &mut ReductionRequest,
                _: &mut ReductionStatistics,
            ) -> Result<OrderedStep<usize>, ReductionError> {
                let terminal = IntegralKey::try_new([0]).unwrap();
                Ok(if *key == 0 {
                    OrderedStep {
                        terminals: BTreeMap::from([(terminal, self.0.integer(1))]),
                        ..Default::default()
                    }
                } else {
                    OrderedStep {
                        terms: BTreeMap::from([(key - 1, self.0.coefficient_fixture("-7/3"))]),
                        ..Default::default()
                    }
                })
            }
        }
        let initial = BTreeMap::from([(30, constant.coefficient_fixture("5/11"))]);
        let exact = run(
            &mut Constant(constant.clone()),
            &mut ReductionRequest::default(),
            initial.clone(),
            vec![],
        )
        .unwrap();
        let multivariate = CoefficientContext::try_new(["d", "m"]).unwrap();
        for arithmetic in [
            modular(false),
            polynomial(2, 2, PolynomialDenominators::Factorized, false),
            polynomial(1, 1, PolynomialDenominators::Dense, true),
        ] {
            let modular_result = run_with_arithmetic(
                &mut Constant(constant.clone()),
                &mut ReductionRequest::default(),
                initial.clone(),
                vec![],
                arithmetic,
            )
            .unwrap();
            assert_eq!(modular_result.terms, exact.terms);
            assert!(matches!(
                run_with_arithmetic(
                    &mut Constant(multivariate.clone()),
                    &mut ReductionRequest::default(),
                    BTreeMap::from([(1, multivariate.one())]),
                    vec![],
                    arithmetic,
                ),
                Err(ReductionError::ModularWeights { .. })
            ));
        }
    }

    /// A kernel with an installed terminal composer (compose before lift).
    struct Composing<K> {
        inner: K,
        composer: Option<std::sync::Arc<dyn OrderedTerminalComposer>>,
    }

    impl<K: OrderedKernel> OrderedKernel for Composing<K> {
        type Key = K::Key;
        type Priority = K::Priority;
        type Error = K::Error;

        fn context(&self) -> &CoefficientContext {
            self.inner.context()
        }
        fn limits(&self) -> ReductionLimits {
            self.inner.limits()
        }
        fn phase_count(&self) -> usize {
            self.inner.phase_count()
        }
        fn position(
            &mut self,
            key: &Self::Key,
        ) -> Result<OrderedPosition<Self::Priority>, Self::Error> {
            self.inner.position(key)
        }
        fn step(
            &mut self,
            key: &Self::Key,
            request: &mut ReductionRequest,
            work: &mut ReductionStatistics,
        ) -> Result<OrderedStep<Self::Key>, Self::Error> {
            self.inner.step(key, request, work)
        }
        fn terminal_composer(&self) -> Option<std::sync::Arc<dyn OrderedTerminalComposer>> {
            self.composer.clone()
        }
    }

    /// Two-stage rows over the ladder's terminals [0], [1], [2], [7], [9]:
    /// stage 1 maps every terminal onto "middle" outputs, stage 2 maps the
    /// middles onto final outputs; output 2 receives T7 - T7 (exactly zero).
    /// `missing` drops the row of one terminal (the plan is then refused).
    struct LadderRows {
        context: CoefficientContext,
        first: BTreeMap<i64, Vec<(u32, Coefficient)>>,
        second: Vec<Vec<(u32, Coefficient)>>,
        outputs: usize,
        missing: Option<i64>,
    }

    impl LadderRows {
        fn new(missing: Option<i64>) -> Self {
            let context = CoefficientContext::try_new(["d"]).unwrap();
            let c = |text: &str| context.coefficient_fixture(text);
            let first = BTreeMap::from([
                (0, vec![(0, c("(d+1)/(d-3)")), (1, c("2"))]),
                (1, vec![(0, c("-(5*d+7)/(d^2+1)")), (3, c("1"))]),
                (
                    2,
                    vec![
                        (4, c("123456789012345678901234567890/(d+5)")),
                        (1, c("-1/3")),
                    ],
                ),
                (7, vec![(2, c("1")), (5, c("-1"))]),
                (9, vec![(1, c("d^2/(5*d-7)")), (3, c("(d-2)^3/11"))]),
            ]);
            let second = vec![
                vec![(0, c("1"))],
                vec![(1, c("(2*d-1)/(d+9)")), (0, c("7"))],
                vec![(2, c("1"))],
                vec![(3, c("1"))],
                vec![(3, c("d/(d-1)"))],
                vec![(2, c("1"))],
            ];
            Self {
                context,
                first,
                second,
                outputs: 4,
                missing,
            }
        }

        /// Add `(terminal, middle, coefficient)` entries to the first stage.
        fn with_entries(mut self, entries: &[(i64, u32, String)]) -> Self {
            for (terminal, middle, coefficient) in entries {
                let coefficient = self.context.coefficient_fixture(coefficient);
                self.first
                    .get_mut(terminal)
                    .expect("a ladder terminal")
                    .push((*middle, coefficient));
            }
            self
        }

        /// The exact composition of exact terminal coefficients.
        fn compose(&self, terms: &BTreeMap<IntegralKey, Coefficient>) -> Vec<Coefficient> {
            let mut middle = vec![self.context.zero(); self.second.len()];
            for (key, value) in terms {
                for (m, a) in &self.first[&key.powers()[0]] {
                    middle[*m as usize] = &middle[*m as usize] + &(value * a);
                }
            }
            let mut outputs = vec![self.context.zero(); self.outputs];
            for (m, row) in self.second.iter().enumerate() {
                for (o, b) in row.iter().map(|(o, b)| (*o as usize, b)) {
                    outputs[o] = &outputs[o] + &(&middle[m] * b);
                }
            }
            outputs
        }
    }

    impl OrderedTerminalComposer for LadderRows {
        fn plan<'a>(
            &'a self,
            terminals: &[IntegralKey],
        ) -> Result<Option<OrderedCompositionPlan<'a>>, ReductionError> {
            let mut first = vec![];
            for key in terminals {
                let label = key.powers()[0];
                if Some(label) == self.missing {
                    return Ok(None);
                }
                first.push(self.first[&label].iter().map(|(m, c)| (*m, c)).collect());
            }
            Ok(Some(OrderedCompositionPlan {
                stages: vec![
                    OrderedCompositionStage {
                        rows: first,
                        width: self.second.len(),
                    },
                    OrderedCompositionStage {
                        rows: self
                            .second
                            .iter()
                            .map(|row| row.iter().map(|(o, c)| (*o, c)).collect())
                            .collect(),
                        width: self.outputs,
                    },
                ],
                labels: std::sync::Arc::new(terminals.to_vec()),
            }))
        }
    }

    fn run_composed(
        arithmetic: OrderedWeightArithmetic,
        rows: std::sync::Arc<LadderRows>,
    ) -> (OrderedOutput, modular::ModularStatistics) {
        let (inner, initial, conditions) = ladder();
        modular::take_reported_statistics();
        let result = run_with_arithmetic(
            &mut Composing {
                inner,
                composer: Some(rows),
            },
            &mut ReductionRequest::default(),
            initial,
            conditions,
            arithmetic,
        )
        .unwrap();
        let mut reported = modular::take_reported_statistics();
        assert_eq!(reported.len(), 1);
        (result, reported.pop().unwrap())
    }

    #[test]
    fn composed_modular_requests_lift_exactly_the_composition_of_the_exact_terminals() {
        let (mut kernel, initial, conditions) = ladder();
        let exact = run(
            &mut kernel,
            &mut ReductionRequest::default(),
            initial,
            conditions,
        )
        .unwrap();
        let rows = std::sync::Arc::new(LadderRows::new(None));
        let expected = rows.compose(&exact.terms);
        assert!(expected[2].is_zero());
        assert!(expected.iter().filter(|c| !c.is_zero()).count() == 3);
        let mut arithmetics = vec![];
        for threads in [1, 3] {
            arithmetics.push(sampled(threads, 8));
        }
        for (threads, lanes) in [(1, 1), (1, 4), (2, 2), (4, 3)] {
            for denominators in [
                PolynomialDenominators::Factorized,
                PolynomialDenominators::Dense,
            ] {
                arithmetics.push(polynomial(threads, lanes, denominators, false));
            }
        }
        for arithmetic in arithmetics {
            let (result, statistics) = run_composed(arithmetic, rows.clone());
            let composed = result.composed.as_ref().expect("composed request");
            assert!(result.terms.is_empty());
            assert_eq!(
                composed.terminals,
                exact.terms.keys().cloned().collect::<Vec<_>>()
            );
            assert_eq!(composed.coefficients, expected, "{arithmetic}");
            assert_eq!(
                composed.labels.downcast_ref::<Vec<IntegralKey>>(),
                Some(&composed.terminals)
            );
            assert_eq!(result.nonzero_conditions, exact.nonzero_conditions);
            assert_eq!(
                result.progress.processed_keys,
                exact.progress.processed_keys
            );
            assert!(statistics.composed);
            assert_eq!(statistics.composed_outputs, 4);
            assert_eq!(statistics.composition_coefficients, 12);
            assert_eq!(statistics.output_terminals, exact.terms.len());
        }
        // Exact and cross-checked requests never compose.
        for arithmetic in [OrderedWeightArithmetic::Exact, modular(true)] {
            let (inner, initial, conditions) = ladder();
            let result = run_with_arithmetic(
                &mut Composing {
                    inner,
                    composer: Some(rows.clone()),
                },
                &mut ReductionRequest::default(),
                initial,
                conditions,
                arithmetic,
            )
            .unwrap();
            assert!(result.composed.is_none());
            assert_eq!(result.terms, exact.terms);
        }
    }

    #[test]
    fn a_refused_plan_lifts_the_raw_terminals() {
        let (mut kernel, initial, conditions) = ladder();
        let exact = run(
            &mut kernel,
            &mut ReductionRequest::default(),
            initial,
            conditions,
        )
        .unwrap();
        let rows = std::sync::Arc::new(LadderRows::new(Some(7)));
        for arithmetic in [
            sampled(2, 8),
            polynomial(2, 2, PolynomialDenominators::Factorized, false),
        ] {
            let (result, statistics) = run_composed(arithmetic, rows.clone());
            assert!(result.composed.is_none());
            assert_eq!(result.terms, exact.terms);
            assert!(!statistics.composed);
        }
    }

    /// A composition whose output is much simpler than the terminals (here
    /// T7 - T7, exactly zero) needs fewer sample points and primes than
    /// lifting the terminals.
    #[test]
    fn composing_to_a_simple_output_samples_fewer_points_and_primes() {
        let (_, raw) = run_reported(sampled(1, 16));
        struct Collapse(Coefficient, Coefficient);
        impl OrderedTerminalComposer for Collapse {
            fn plan<'a>(
                &'a self,
                terminals: &[IntegralKey],
            ) -> Result<Option<OrderedCompositionPlan<'a>>, ReductionError> {
                // Only terminal [7] reaches the output, with 1 - 1.
                Ok(Some(OrderedCompositionPlan {
                    stages: vec![OrderedCompositionStage {
                        rows: terminals
                            .iter()
                            .map(|key| {
                                if key.powers()[0] == 7 {
                                    vec![(0, &self.0), (0, &self.1)]
                                } else {
                                    vec![]
                                }
                            })
                            .collect(),
                        width: 1,
                    }],
                    labels: std::sync::Arc::new(()),
                }))
            }
        }
        let context = CoefficientContext::try_new(["d"]).unwrap();
        let (one, minus_one) = (context.one(), context.integer(-1));
        let (inner, initial, conditions) = ladder();
        modular::take_reported_statistics();
        let result = run_with_arithmetic(
            &mut Composing {
                inner,
                composer: Some(std::sync::Arc::new(Collapse(one, minus_one))),
            },
            &mut ReductionRequest::default(),
            initial,
            conditions,
            sampled(1, 16),
        )
        .unwrap();
        let composed = modular::take_reported_statistics().pop().unwrap();
        assert_eq!(result.composed.unwrap().coefficients, vec![context.zero()]);
        assert!(composed.points_per_prime < raw.points_per_prime);
        assert!(composed.points_replayed < raw.points_replayed);
        assert!(composed.primes <= raw.primes);
    }

    /// Unusable primes and bad points inside a full composed lift. A plan
    /// coefficient whose denominator content is the first and the third
    /// reconstruction prime and the first verification prime makes those
    /// primes unusable (`ModularSampled`: discovery, a later round and, with
    /// two verification primes, the owner-indexed verification;
    /// `ModularPolynomial`: the rounds and the verification images);
    /// coefficients with a pole at the first sample point (`ModularSampled`)
    /// and at the first composition point (`ModularPolynomial`) of the second
    /// reconstruction prime give bad points there. The lifted outputs are
    /// still exactly the composition of the exact terminals.
    #[test]
    fn composed_lifts_skip_unusable_primes_and_bad_points() {
        let (mut kernel, initial, conditions) = ladder();
        let exact = run(
            &mut kernel,
            &mut ReductionRequest::default(),
            initial,
            conditions,
        )
        .unwrap();
        let common = ModularCommon {
            verification_primes: 2,
            ..Default::default()
        };
        let lift = modular::test_hooks::lift_primes(common.seed, 3);
        let verification = modular::test_hooks::verification_primes(common.seed, 1);
        let sample_pole = modular::test_hooks::sample_points(common.seed, lift[1], 1)[0];
        let compose_pole = modular::test_hooks::composition_points(common.seed, lift[1], 1)[0];
        let rows = std::sync::Arc::new(LadderRows::new(None).with_entries(&[
            (
                0,
                3,
                format!("(d+2)/({}*{}*{})", lift[0], lift[2], verification[0]),
            ),
            (1, 4, format!("1/(d-{sample_pole})")),
            (9, 0, format!("(3*d-1)/(d-{compose_pole})")),
        ]));
        let expected = rows.compose(&exact.terms);
        let mut arithmetics = vec![];
        for threads in [1, 3] {
            arithmetics.push(OrderedWeightArithmetic::ModularSampled(
                ModularCommon { threads, ..common },
                SampledOptions {
                    batch_points: 8,
                    ..Default::default()
                },
            ));
        }
        for (threads, lanes) in [(1, 1), (2, 2), (4, 3)] {
            arithmetics.push(OrderedWeightArithmetic::ModularPolynomial(
                ModularCommon { threads, ..common },
                PolynomialOptions {
                    lanes,
                    ..Default::default()
                },
            ));
        }
        for arithmetic in arithmetics {
            let (result, statistics) = run_composed(arithmetic, rows.clone());
            let composed = result.composed.as_ref().expect("composed request");
            assert_eq!(composed.coefficients, expected, "{arithmetic}");
            assert_eq!(result.nonzero_conditions, exact.nonzero_conditions);
            assert!(statistics.composed && !statistics.composition_fallback);
            assert!(
                statistics.unusable_primes >= 3,
                "{arithmetic}: {} unusable primes",
                statistics.unusable_primes
            );
            assert!(
                statistics.bad_points >= 1,
                "{arithmetic}: {} bad points",
                statistics.bad_points
            );
        }
    }

    /// A composed lift that fails closed falls back to the raw lift once.
    /// Here the outputs need far more primes than the budget (a plan
    /// coefficient of about 2,000 bits) while the raw terminals fit in it:
    /// the request returns the exact terminal map (which the caller then
    /// substitutes exactly), with the fallback reported.
    #[test]
    fn a_composed_lift_that_fails_closed_falls_back_to_the_raw_lift() {
        let (mut kernel, initial, conditions) = ladder();
        let exact = run(
            &mut kernel,
            &mut ReductionRequest::default(),
            initial,
            conditions,
        )
        .unwrap();
        let rows = std::sync::Arc::new(LadderRows::new(None).with_entries(&[(
            2,
            4,
            "(d+1)/(7^700*(d+5))".to_owned(),
        )]));
        let with_budget = |arithmetic: OrderedWeightArithmetic, max_primes: usize| match arithmetic
        {
            OrderedWeightArithmetic::ModularSampled(common, options) => {
                OrderedWeightArithmetic::ModularSampled(
                    ModularCommon {
                        max_primes,
                        ..common
                    },
                    options,
                )
            }
            OrderedWeightArithmetic::ModularPolynomial(common, options) => {
                OrderedWeightArithmetic::ModularPolynomial(
                    ModularCommon {
                        max_primes,
                        ..common
                    },
                    options,
                )
            }
            OrderedWeightArithmetic::Exact => unreachable!("modular arithmetics only"),
        };
        for arithmetic in [
            sampled(2, 8),
            polynomial(2, 2, PolynomialDenominators::Factorized, false),
        ] {
            let (raw, raw_statistics) = run_reported(arithmetic);
            assert_eq!(raw.terms, exact.terms);
            let budget = raw_statistics.primes
                + raw_statistics.unusable_primes
                + raw_statistics.verification_primes
                + 2;
            let (result, statistics) = run_composed(with_budget(arithmetic, budget), rows.clone());
            assert!(result.composed.is_none(), "{arithmetic}");
            assert_eq!(result.terms, exact.terms, "{arithmetic}");
            assert_eq!(result.nonzero_conditions, exact.nonzero_conditions);
            assert!(statistics.composition_fallback && !statistics.composed);
            assert!(statistics.composition_fallback_seconds > 0.0);
            assert_eq!(statistics.primes, raw_statistics.primes);
        }
    }
}
