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
mod layered;
pub(crate) mod memstats;
mod modular;
mod parallel;
pub(crate) mod parstats;
pub(crate) mod resume;
pub(crate) mod step_cache;
pub(crate) mod tail;

pub use arithmetic::{
    ModularCommon, OrderedWeightArithmetic, ParseOrderedWeightArithmeticError,
    PolynomialDenominators, PolynomialOptions, SampledOptions, TadicOptions,
};
pub use composition::{
    ComposedTerminals, OrderedCompositionPlan, OrderedCompositionStage, OrderedTerminalComposer,
    OrderedTerminalComposerSlot,
};
use frontier::{Child, ChildAction, Frontier, FrontierLayout, FrontierMode};
pub(crate) use frontier::{CompactPowers, FrontierKey, StoredKey};
pub(crate) use modular::tadic::coefficient_valuation;
pub use modular::tadic::{
    TADIC_PRECISION_FAILURE, TadicImage, TadicSeries, TadicTerminals, TadicVariants, add_series,
    expand_coefficient, is_precision_failure, scale_series,
};
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
    /// Called once when the forward pass of a modular request is over,
    /// before the recorded trace is replayed (no step runs after it):
    /// adapters may release memory that only the steps use.
    fn forward_finished(&mut self) {}
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
    /// 12d2 resume binding: everything outside the request (keys,
    /// coefficients, conditions) that can change a step of this program, e.g.
    /// the library and its step identity. A v3 resume state stores its
    /// BLAKE3 and is refused by a program whose identity differs.
    fn resume_identity(&self, _out: &mut Vec<u8>) {}
    /// 12d2: the part of [`Self::resume_identity`] that makes a run partial
    /// by design (a target selection). A non-empty selection also enters the
    /// request directory, so a projected and a full run never share one.
    fn resume_selection(&self, _out: &mut Vec<u8>) {}
    /// eng-stage: why no state may be saved inside `phase` (its frontier
    /// can hold keys without a compact form), or `None` (the default). A
    /// refused in-phase save writes nothing: the last saved state stays.
    fn inphase_save_refusal(&self, _phase: usize) -> Option<String> {
        None
    }
    /// stale memory: drop the step caches of this thread (the step memo with
    /// its coefficient table, prepared weights, zero certificates, canon /
    /// order caches): value-neutral, rebuilt on use. The caller drops the
    /// arithmetic's light-handle cache at the same time (the memo issued the
    /// handles). Returns the memo coefficients dropped.
    fn release_step_caches(&mut self) -> usize {
        0
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
    /// eng-mem: [`Self::tail_encode`] of a pending key as the frontier
    /// stores it (the streaming state save). It must append exactly what
    /// `tail_encode` appends for the key; the default unpacks a packed key.
    fn tail_encode_stored(key: StoredKey<'_, Self::Key>, out: &mut Vec<u8>) -> bool {
        match key {
            StoredKey::Key(key) => Self::tail_encode(key, out),
            StoredKey::Packed(packed) => {
                Self::tail_encode(&<Self::Key as FrontierKey>::unpack(packed), out)
            }
        }
    }
    /// The caller's terminal composition for modular requests (compose
    /// before lift, `composition.rs`); `None` lifts the terminals.
    fn terminal_composer(&self) -> Option<std::sync::Arc<dyn OrderedTerminalComposer>> {
        None
    }
    /// helper-vh2: whether this adapter consumes the series outputs of the
    /// t-adic arithmetic (`OrderedOutput::tadic`). Every other adapter reads
    /// exact terms, so a t-adic request fails closed on it.
    fn accepts_tadic(&self) -> bool {
        false
    }
    /// helper-vh2: the absolute precision each live terminal (in key order)
    /// must reach in a t-adic request so that the adapter's final outputs
    /// reach `options.order` (an adapter that multiplies a terminal by exact
    /// coefficients with poles at `d0` raises its target by their order).
    fn tadic_targets(
        &self,
        terminals: &[IntegralKey],
        options: &TadicOptions,
    ) -> Result<Vec<i32>, Self::Error> {
        Ok(vec![options.order; terminals.len()])
    }
    /// helper-vh2 B3: the seeds and weight vectors one recording of a t-adic
    /// request is replayed for (`None`: the request's seed and weights).
    fn tadic_variants(&self) -> Option<TadicVariants> {
        None
    }
    /// rr-frontier diagnostics (`memstats`): allocator bytes owned by a key.
    /// Default: nothing measured.
    /// fwd-opt light steps on the calling thread (see
    /// `OrderedStepper::set_light`); default: never.
    fn set_light(&mut self, _on: bool) {}

    /// fwd-opt: the light children of the step computed last on this thread.
    fn take_light(&mut self) -> Vec<(Self::Key, u64)> {
        Vec::new()
    }

    /// fwd-opt: the coefficient a light handle names (on the step's thread).
    fn light_value(&self, _handle: u64) -> Option<Coefficient> {
        None
    }

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
    /// helper-vh2: the series of the live terminals of a t-adic request
    /// (`terms` is then empty).
    pub tadic: Option<TadicTerminals>,
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
    /// rr-par2: `scale_hashed` with the coefficient's id in the shared intern
    /// table (`modular::SharedIntern`) when a worker interned it.
    fn scale_interned(
        &mut self,
        weight: &Self::Weight,
        coefficient: &Coefficient,
        hash: Option<u64>,
        _id: Option<(u32, bool)>,
    ) -> Result<Self::Contribution, ReductionError> {
        self.scale_hashed(weight, coefficient, hash)
    }
    fn contribution_is_zero(&self, contribution: &Self::Contribution) -> bool;
    /// vk5l-physrun resume state (`resume`): bind the request directory
    /// (`resume`: the op count of a saved state to continue from and its
    /// trace digest, `None` for a state of format v1). `false`: this
    /// arithmetic does not resume.
    fn resume_bind(
        &mut self,
        _dir: &std::path::Path,
        _resume: Option<(u64, Option<[u8; 32]>)>,
    ) -> Result<bool, ReductionError> {
        Ok(false)
    }
    /// The arithmetic's section of the state, streamed (eng-mem: call
    /// [`resume::StateWriter::drain`] between items).
    fn resume_save(&mut self, _w: &mut resume::StateWriter) -> Result<(), ReductionError> {
        Err(invariant("this arithmetic keeps no resume state"))
    }
    /// `digest`: the state is of format v3 (a 32-byte trace digest follows
    /// the op count). eng-mem: the section is read item by item
    /// ([`resume::StateReader::item`]).
    fn resume_load(
        &mut self,
        _r: &mut resume::StateReader,
        _digest: bool,
    ) -> Result<(), ReductionError> {
        Err(invariant("this arithmetic keeps no resume state"))
    }
    fn resume_put_weight(&self, _weight: &Self::Weight, _w: &mut step_cache::codec::Writer) {}
    fn resume_get_weight(
        &self,
        _r: &mut step_cache::codec::Reader<'_>,
    ) -> Result<Self::Weight, ReductionError> {
        Err(invariant("this arithmetic keeps no resume state"))
    }
    /// fwd-opt: the contribution `weight * value(handle)` of a light child
    /// (see `OrderedKernel::take_light`); an arithmetic may remember the
    /// coefficient of a handle for the request. `None` from `value`: the
    /// handle names nothing (an invariant error).
    fn scale_handle(
        &mut self,
        weight: &Self::Weight,
        _handle: u64,
        value: &mut dyn FnMut() -> Option<Coefficient>,
    ) -> Result<Self::Contribution, ReductionError> {
        let coefficient =
            value().ok_or_else(|| invariant("a light handle names no coefficient"))?;
        self.scale(weight, &coefficient)
    }
    /// fwd-opt: whether this arithmetic takes light children (one thread).
    fn takes_light(&self) -> bool {
        false
    }
    /// stale memory: forget the light-handle -> coefficient cache (with
    /// [`OrderedKernel::release_step_caches`]); returns the entries dropped.
    fn release_handles(&mut self) -> usize {
        0
    }
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
    /// rr-par2: workers prepare children and intern step coefficients
    /// (pipelined sources only).
    prestage: bool,
    /// rr-par2: the scheduler reads step coefficients through their ids only.
    hollow: bool,
    /// fwd-opt: TFORM-style layered forward pass (`layered`).
    layered: bool,
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
            prestage: tuning.prestage && tuning.pipeline && threads > 1,
            hollow: tuning.hollow,
            layered: tuning.layered,
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
        | OrderedWeightArithmetic::ModularPolynomial(..)
        | OrderedWeightArithmetic::Tadic(..) => run_modular(
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
    // fwd-opt: the layered mode runs the same algorithm at every thread count.
    if layered::eligible(kernel, &arithmetic, &tail) {
        return layered::run(
            kernel,
            request,
            initial,
            nonzero_conditions,
            arithmetic,
            scheduler.threads,
            tail,
        );
    }
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
        | OrderedWeightArithmetic::ModularPolynomial(..)
        | OrderedWeightArithmetic::Tadic(..) => {
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
                            &mut Pipelined::new(
                                scope,
                                workers,
                                stepper,
                                scheduler.window_keys,
                                tail.prestage,
                            ),
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
    refuse_unaccepted_tadic(&*kernel, &arithmetic)?;
    let mut recorder = ModularRecorder::try_new(kernel.context(), kernel.limits(), common, lifter)?;
    recorder.set_modular_round(tail.modular_round);
    recorder.set_par_replay(tail.par_replay);
    recorder.set_prestage(tail.prestage);
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
        // vk5l-physrun channel replay: record the (mixed) inputs once, replay
        // the trace per channel with its own inputs.
        if let Some((channels, lambdas)) = modular::channels::take::<K::Key>() {
            let mut weights = recorder;
            weights.set_multi();
            let order: Vec<K::Key> = initial
                .iter()
                .filter(|(_, coefficient)| !coefficient.is_zero())
                .map(|(key, _)| key.clone())
                .collect();
            for channel in &channels {
                if channel
                    .iter()
                    .any(|(key, value)| !value.is_zero() && order.binary_search(key).is_err())
                {
                    kernel.set_light(false);
                    return Err(modular::error("a channel input vanished in the mix").into());
                }
            }
            if !weights.set_multi_probes(&order, &channels, &lambdas)? {
                kernel.set_light(false);
                return Err(modular::error("a mixed input vanishes at every probe point").into());
            }
            let output = run_inner(
                kernel,
                &mut weights,
                request,
                initial,
                nonzero_conditions,
                work,
                source,
                tail,
            );
            kernel.set_light(false);
            let output = output?;
            kernel.forward_finished();
            let zero = kernel.context().zero();
            let patches = channels
                .iter()
                .map(|channel| {
                    order
                        .iter()
                        .map(|key| channel.get(key).cloned().unwrap_or_else(|| zero.clone()))
                        .collect()
                })
                .collect();
            let finished = weights.finish_channels(output.terms, patches)?;
            let mut per_channel = Vec::with_capacity(finished.len());
            for (finished, statistics) in finished {
                modular::report_statistics(&statistics);
                let modular::Finished::Terminals(terms) = finished else {
                    return Err(modular::error("a channel replay composed its terminals").into());
                };
                per_channel.push(terms);
            }
            let first = per_channel.first().cloned().unwrap_or_default();
            modular::channels::put_out(per_channel);
            return Ok(OrderedOutput {
                terms: first,
                composed: None,
                nonzero_conditions: output.nonzero_conditions,
                work: output.work,
                progress: output.progress,
                tadic: None,
            });
        }
        let mut weights = recorder;
        if tail.prestage
            && let Some(table) = weights.shared_intern()
        {
            source.attach_intern(table, !tail.enabled && tail.hollow);
        }
        let output = run_inner(
            kernel,
            &mut weights,
            request,
            initial,
            nonzero_conditions,
            work,
            source,
            tail,
        );
        // fwd-opt: light steps end with the request, whatever its outcome.
        kernel.set_light(false);
        output.and_then(|output| {
            kernel.forward_finished();
            let targets = tadic_targets(&*kernel, &arithmetic, &output)?;
            weights.set_tadic_variants(kernel.tadic_variants());
            modular_output(weights, output, composer.as_deref(), targets).map_err(Into::into)
        })
    }
}

/// helper-vh2: a t-adic request on an adapter that reads exact terms only is
/// refused before any work (its exact terms would be empty: a silent zero).
fn refuse_unaccepted_tadic<K: OrderedKernel>(
    kernel: &K,
    arithmetic: &OrderedWeightArithmetic,
) -> Result<(), K::Error> {
    if matches!(arithmetic, OrderedWeightArithmetic::Tadic(..)) && !kernel.accepts_tadic() {
        return Err(modular::error(
            "the t-adic arithmetic returns truncated series, not exact terms; this adapter \
             reads exact terms (use the propagator library's reduce_tadic)",
        )
        .into());
    }
    Ok(())
}

/// helper-vh2: the per-terminal target precisions of a t-adic request (the
/// adapter's, in the key order of the live terminals); `None` otherwise.
fn tadic_targets<K: OrderedKernel, W>(
    kernel: &K,
    arithmetic: &OrderedWeightArithmetic,
    output: &RawOutput<W>,
) -> Result<Option<Vec<i32>>, K::Error> {
    let OrderedWeightArithmetic::Tadic(_, options) = arithmetic else {
        return Ok(None);
    };
    let keys: Vec<IntegralKey> = output.terms.keys().cloned().collect();
    kernel.tadic_targets(&keys, options).map(Some)
}

fn exact_output(output: RawOutput<Coefficient>) -> OrderedOutput {
    OrderedOutput {
        terms: output.terms,
        composed: None,
        tadic: None,
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
    let (reconstructed, statistics) = weights.modular.finish(modular, None, None)?;
    modular::report_statistics(&statistics);
    let tadic = match reconstructed {
        modular::Finished::Terminals(reconstructed) => {
            if reconstructed != exact {
                return Err(modular::error(
                    "modular reconstruction differs from the exact terminal map",
                ));
            }
            None
        }
        // helper-vh2: every series must be the Laurent expansion of the
        // exact terminal through its precision.
        modular::Finished::Tadic(tadic) => {
            cross_check_tadic(&exact, &tadic)?;
            Some(tadic)
        }
        modular::Finished::Composed(_) => {
            return Err(modular::error(
                "a cross-checked request composed its terminals",
            ));
        }
    };
    Ok(OrderedOutput {
        // A t-adic request returns its series (and, cross-checked, the exact
        // terms they were compared with).
        terms: exact,
        composed: None,
        tadic,
        nonzero_conditions: output.nonzero_conditions,
        work: output.work,
        progress: output.progress,
    })
}

/// helper-vh2: compare every t-adic series with the expansion of the exact
/// terminal coefficient (`cross_check_exact`).
fn cross_check_tadic(
    exact: &BTreeMap<IntegralKey, Coefficient>,
    tadic: &TadicTerminals,
) -> Result<(), ReductionError> {
    if tadic.terminals.len() != exact.len()
        || tadic
            .terminals
            .iter()
            .zip(exact.keys())
            .any(|(a, b)| a != b)
    {
        return Err(modular::error(
            "the t-adic terminals differ from the exact terminal map",
        ));
    }
    for image in &tadic.images {
        for (series, coefficient) in image.terminals.iter().zip(exact.values()) {
            let expected =
                expand_coefficient(coefficient, tadic.d0, image.prime, series.precision())?
                    .ok_or_else(|| {
                        modular::error("a t-adic prime is bad for an exact terminal coefficient")
                    })?;
            if expected != *series {
                return Err(modular::error(
                    "a t-adic series differs from the expansion of the exact terminal",
                ));
            }
        }
    }
    Ok(())
}

/// Replay the recorded trace and reconstruct the exact terminals, or only
/// their composition when `composer` plans one, or (t-adic) their series.
fn modular_output(
    weights: ModularRecorder,
    output: RawOutput<modular::ModularWeight>,
    composer: Option<&dyn OrderedTerminalComposer>,
    tadic_targets: Option<Vec<i32>>,
) -> Result<OrderedOutput, ReductionError> {
    crate::prof!(FinOutput);
    let resume_dir = weights.resume_dir();
    let (finished, statistics) =
        weights.finish(output.terms, composer, tadic_targets.as_deref())?;
    // 12d2: the result exists. Its state and trace go now, or (caller commit
    // mode) once the caller has durably written the result
    // ([`commit_resume`]).
    if let Some(dir) = resume_dir {
        resume::result_ready(dir);
    }
    modular::report_statistics(&statistics);
    let (terms, composed, tadic) = match finished {
        modular::Finished::Terminals(terms) => (terms, None, None),
        modular::Finished::Composed(composed) => (BTreeMap::new(), Some(composed), None),
        modular::Finished::Tadic(tadic) => (BTreeMap::new(), None, Some(tadic)),
    };
    Ok(OrderedOutput {
        terms,
        composed,
        tadic,
        nonzero_conditions: output.nonzero_conditions,
        work: output.work,
        progress: output.progress,
    })
}

/// 12d2: in caller-commit mode, a resumable request whose result exists
/// keeps its final state and trace until the caller calls
/// [`commit_resume`] after durably writing that result (a re-launch before
/// that replays only). Off by default (state and trace removed when the
/// result exists); `RUSTRED_RESUME_COMMIT=caller|engine` overrides.
pub fn set_resume_caller_commit(on: bool) {
    resume::set_caller_commit(on);
}

/// 12d2: remove the kept state and trace of every request of this process
/// whose result exists (caller-commit mode); returns how many.
pub fn commit_resume() -> usize {
    resume::commit()
}

/// R-B2: the storage stop (low-disk pause, failed checkpoint, trace or
/// dictionary spill write / sync, refused resume) behind the last failed
/// request of this process, cleared. A batch caller that gets `Some` must not
/// record the request's error as its output: the request stays retryable.
pub fn take_resume_storage_stop() -> Option<String> {
    resume::take_storage_stop()
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

    /// rr-par2: what the workers prepared for the step just returned (children
    /// packed, hashed and positioned; coefficient ids of `attach_intern`'s table).
    fn take_prestaged(&mut self) -> Option<PrestagedFor<K>> {
        None
    }

    /// rr-par2: whether the frontier probes keys packed and hashed by the
    /// workers (`Frontier::takes_prepacked`); called once, before any step.
    fn configure_prestage(&mut self, _prepacked: bool) {}

    /// rr-par2: drop spent prepared children (on another thread).
    fn dispose_prestaged(&mut self, spent: Vec<PrestagedFor<K>>) {
        drop(spent);
    }

    /// rr-par2: the recorder's shared coefficient table; a source that can
    /// intern step coefficients on its workers reports their ids. `hollow`:
    /// the scheduler reads step coefficients only through the ids (no tail
    /// journal), so the workers may free them after interning.
    fn attach_intern(&mut self, _table: std::sync::Arc<modular::SharedIntern>, _hollow: bool) {}

    /// fwd-opt: the steps are computed on the scheduler thread by the kernel
    /// (light children can be taken right after each step).
    fn on_this_thread(&self) -> bool {
        false
    }
}

/// rr-par2 (`RUSTRED_ORDERED_PRESTAGE=on|off` overrides the profile; on for
/// p requests since M5 batch 4; effective with the pipeline, i.e. more than
/// one thread): the workers that compute a step also prepare what the
/// sequential merge needs per child, so that the scheduler thread only probes
/// and records.
pub(crate) fn prestage_environment() -> Option<bool> {
    static ON: std::sync::OnceLock<Option<bool>> = std::sync::OnceLock::new();
    *ON.get_or_init(|| switch_environment("RUSTRED_ORDERED_PRESTAGE"))
}

/// rr-par2 (`RUSTRED_ORDERED_HOLLOW=on|off` overrides the profile; on for p
/// requests since M5 batch 4; effective with prestage and without the adjoint
/// tail): the worker that interned a step's coefficients frees them (where
/// they were allocated) and hands the scheduler placeholders; the scheduler
/// uses the ids and zero tests.
pub(crate) fn hollow_environment() -> Option<bool> {
    static ON: std::sync::OnceLock<Option<bool>> = std::sync::OnceLock::new();
    *ON.get_or_init(|| switch_environment("RUSTRED_ORDERED_HOLLOW"))
}

/// rr-par2: one term child of a step, packed and hashed with the Fx frontier
/// hash, and its certified position when it could be computed (the position
/// is a pure function of the key: `None` leaves it, and its error, to the
/// scheduler).
pub(crate) struct PreChild<K, C, P> {
    /// The key itself, moved out of the step (`Prestaged::lean` only).
    pub(crate) key: Option<K>,
    pub(crate) packed: C,
    pub(crate) hash: u64,
    pub(crate) position: Option<OrderedPosition<P>>,
}

/// rr-par2: the prepared children (terms in order; empty when not prepared)
/// and coefficient ids with their zero tests (terms in order, then
/// terminals; empty when no table). With `hollow`, the step's coefficients
/// were freed by the worker after interning and replaced by placeholders:
/// only the ids and zero tests carry them.
pub(crate) struct Prestaged<K, C, P> {
    pub(crate) children: Vec<PreChild<K, C, P>>,
    pub(crate) ids: Vec<(u32, bool)>,
    /// The step's terms were moved into `children` (keys) and `ids`
    /// (coefficients): its term map is empty, its terminals hollow.
    pub(crate) lean: bool,
}

type PrestagedFor<K> = Prestaged<
    <K as OrderedKernel>::Key,
    <<K as OrderedKernel>::Key as FrontierKey>::Packed,
    <K as OrderedKernel>::Priority,
>;

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

    fn on_this_thread(&self) -> bool {
        true
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
    // vk5l-physrun resume state (`resume`): runs without a tail, sequential
    // or pipelined (both record the sequential trace, so a state saved at one
    // thread count continues at any other).
    let resume_dir = if tail.enabled {
        None
    } else {
        resume::dir().and_then(|root| {
            let mut selection = Vec::new();
            kernel.resume_selection(&mut selection);
            resume::request_dir::<K>(&root, &initial, &nonzero_conditions, &selection)
        })
    };
    let mut start_phase = 0;
    let mut restored = None;
    let mut resume_dir = match resume_dir {
        Some(dir) if dir.join("state.bin").exists() => {
            match resume_restore(kernel, weights, request, &dir, tail.frontier) {
                Ok((state, next)) => {
                    eprintln!(
                        "rustred resume: continuing {} at phase {next}",
                        dir.display()
                    );
                    start_phase = next;
                    restored = Some(state);
                    Some(dir)
                }
                // A state that cannot be continued is a hard error, never a
                // silent fresh start or a deletion: the directory (state +
                // trace) is kept for inspection; remove it to start afresh.
                Err(error) => {
                    let message = format!(
                        "resume refused for {}: {error} (state and trace kept; remove the directory to start this \
                         request afresh)",
                        dir.display()
                    );
                    eprintln!("rustred resume: {message}");
                    resume::storage_stop(&message);
                    return Err(modular::error(message).into());
                }
            }
        }
        // R-B2: binding the request directory without a state (a trace that
        // cannot be set aside, an unwritable directory) is a storage stop too.
        Some(dir) => weights
            .resume_bind(&dir, None)
            .map_err(|error| {
                resume::storage_stop(&format!("resume refused for {}: {error}", dir.display()));
                error
            })?
            .then_some(dir),
        None => None,
    };
    let mut last_save = std::time::Instant::now();
    // 12d2: the phase of the last saved state (a final state is saved after
    // the last phase unless that phase was just saved).
    let mut saved_phase: Option<usize> = start_phase.checked_sub(1);
    // eng-stage: the last phase whose in-phase saves the kernel refused.
    let mut refused_phase: Option<usize> = None;
    let mut state = match restored {
        Some(state) => state,
        None => crate::prof_wrap!(
            Admit,
            admit(
                kernel,
                weights,
                request,
                initial,
                nonzero_conditions,
                tail.frontier,
            )
        )?,
    };
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
    steps.configure_prestage(state.frontier.takes_prepacked());
    let mut spent_prestaged: Vec<PrestagedFor<K>> = Vec::new();
    // A placeholder for coefficients carried by ids only (`Prestaged::lean`).
    let placeholder = context.zero();
    // fwd-opt light steps: the kernel's singly met memo children come as
    // (key, handle) and the arithmetic maps a handle to its coefficient once
    // (one thread, recorded arithmetics). Switched off again by the caller
    // (`run_modular`) whatever the outcome.
    let light = steps.on_this_thread() && weights.takes_light() && tail.is_none();
    if light {
        kernel.set_light(true);
    }
    for phase in start_phase..state.progress.total_phases {
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
        while let Some((node, weight)) = crate::prof_wrap!(Pop, frontier.pop_in_phase(phase)) {
            crate::prof!(Loop);
            request.release_pending_frame()?;
            replace_monomials(
                &mut progress.frontier_coefficient_monomials,
                weights.monomials(&weight)?,
                0,
            )?;
            progress.frontier_keys = frontier.len();
            kernel.observe(progress, Some(&node.key))?;
            let source_started = par_clock.now();
            let checked =
                crate::prof_wrap!(Source, steps.step(kernel, frontier, &node, request, work))?;
            let light_children = if light {
                kernel.take_light()
            } else {
                Vec::new()
            };
            let hashes = steps.take_hashes();
            let term_count = checked.terms.len();
            // rr-par2: children packed/hashed/positioned and coefficient ids
            // from the workers (same values the code below would compute).
            let mut prestaged = steps.take_prestaged();
            let lean = prestaged.as_ref().is_some_and(|p| p.lean);
            let (mut prepared, ids) = match prestaged.as_mut() {
                Some(p) if !lean => (
                    (frontier.takes_prepacked() && p.children.len() == term_count)
                        .then(|| std::mem::take(&mut p.children).into_iter()),
                    std::mem::take(&mut p.ids),
                ),
                _ => (None, Vec::new()),
            };
            let term_count = match &prestaged {
                Some(p) if lean => p.children.len(),
                _ => term_count,
            };
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
            // rr-par2: the merge of one term child (shared by the loop over
            // the step's terms and the lean loop over prepared children).
            macro_rules! decide {
                ($coefficient:expr, $hash:expr, $id:expr, $key:ident => $position:expr) => {
                    |found: Child<'_, K::Key, K::Priority, A::Weight>| {
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
                                let contribution = crate::prof_wrap_fine!(
                                    Intern,
                                    weights.scale_interned(&weight, $coefficient, $hash, $id)
                                )?;
                                work.record_coalescing_additions(1);
                                request.record_coalescing_additions(
                                    1,
                                    limits.diagnostic_limit(limits.max_coalescing_additions),
                                )?;
                                let old = weights.monomials(existing)?;
                                let zero = crate::prof_wrap_fine!(
                                    Record,
                                    weights.merge(existing, contribution)
                                )?;
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
                            Child::Vacant($key) => {
                                let position = crate::prof_wrap!(Position, $position);
                                check_descent(
                                    &position,
                                    phase,
                                    &node.priority,
                                    progress.total_phases,
                                )?;
                                let contribution = crate::prof_wrap_fine!(
                                    Intern,
                                    weights.scale_interned(&weight, $coefficient, $hash, $id)
                                )?;
                                if weights.contribution_is_zero(&contribution) {
                                    ChildAction::Keep
                                } else {
                                    request.retain_pending_frame(
                                        limits.diagnostic_limit(limits.max_pending_frames),
                                    )?;
                                    let inserted = crate::prof_wrap_fine!(
                                        Record,
                                        weights.insert(contribution)
                                    )?;
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
                    }
                };
            }
            if lean && let Some(p) = &prestaged {
                for (index, child) in p.children.iter().enumerate() {
                    let (id, zero) = p.ids[index];
                    if zero {
                        continue;
                    }
                    let key = child
                        .key
                        .as_ref()
                        .ok_or_else(|| invariant("a lean step child lost its key"))?;
                    progress.emitted_terms = progress.emitted_terms.saturating_add(1);
                    let decide = decide!(&placeholder, None, Some((id, zero)), vacant => match &child.position {
                        Some(position) => position.clone(),
                        None => kernel.position(vacant)?,
                    });
                    let removed = crate::prof_wrap_fine!(
                        Child,
                        frontier.child_packed_ref(key, &child.packed, child.hash, decide)
                    )?;
                    if let Some(removed) = removed {
                        crate::prof_wrap_fine!(Record, weights.cancel(removed))?;
                        request.release_pending_frame()?;
                        progress.cancelled_terms = progress.cancelled_terms.saturating_add(1);
                    }
                    progress.peak_frontier_keys = progress.peak_frontier_keys.max(frontier.len());
                    progress.peak_frontier_coefficient_monomials = progress
                        .peak_frontier_coefficient_monomials
                        .max(progress.frontier_coefficient_monomials);
                }
            }
            let ids = match &prestaged {
                Some(p) if lean => &p.ids,
                _ => &ids,
            };
            for (index, (child, coefficient)) in checked.terms.into_iter().enumerate() {
                let pre = prepared.as_mut().and_then(Iterator::next);
                let id = ids.get(index).copied();
                if id.map_or_else(|| coefficient.is_zero(), |(_, zero)| zero) {
                    continue;
                }
                let hash = hashes.as_ref().map(|hashes| hashes[index]);
                let (packed, mut preposition) = match pre {
                    Some(PreChild {
                        packed,
                        hash,
                        position,
                        ..
                    }) => (Some((packed, hash)), position),
                    None => (None, None),
                };
                progress.emitted_terms = progress.emitted_terms.saturating_add(1);
                let decide = decide!(&coefficient, hash, id, key => match preposition.take() {
                    Some(position) => position,
                    None => kernel.position(key)?,
                });
                let removed = crate::prof_wrap_fine!(
                    Child,
                    match packed {
                        Some((packed, hash)) => frontier.child_packed(child, packed, hash, decide),
                        None => frontier.child(child, decide),
                    }
                )?;
                if disposing {
                    spent.push(coefficient);
                }
                if let Some(removed) = removed {
                    crate::prof_wrap_fine!(Record, weights.cancel(removed))?;
                    request.release_pending_frame()?;
                    progress.cancelled_terms = progress.cancelled_terms.saturating_add(1);
                }
                progress.peak_frontier_keys = progress.peak_frontier_keys.max(frontier.len());
                progress.peak_frontier_coefficient_monomials = progress
                    .peak_frontier_coefficient_monomials
                    .max(progress.frontier_coefficient_monomials);
            }
            for (child, handle) in light_children {
                let contribution =
                    weights.scale_handle(&weight, handle, &mut || kernel.light_value(handle))?;
                if weights.contribution_is_zero(&contribution) {
                    // A zero coefficient is no child (as a zero term).
                    continue;
                }
                progress.emitted_terms = progress.emitted_terms.saturating_add(1);
                let mut contribution = Some(contribution);
                let removed =
                    frontier.child(child, |found: Child<'_, K::Key, K::Priority, A::Weight>| {
                        let contribution = contribution
                            .take()
                            .ok_or_else(|| invariant("a light contribution was used twice"))?;
                        Ok::<_, K::Error>(match found {
                            Child::Occupied {
                                phase: entered,
                                priority,
                                weight: existing,
                            } => {
                                check_descent_parts(
                                    entered,
                                    priority,
                                    phase,
                                    &node.priority,
                                    progress.total_phases,
                                )?;
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
                            Child::Vacant(vacant) => {
                                let position = kernel.position(vacant)?;
                                check_descent(
                                    &position,
                                    phase,
                                    &node.priority,
                                    progress.total_phases,
                                )?;
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
                        })
                    })?;
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
                crate::prof!(Terminal);
                let hash = hashes.as_ref().map(|hashes| hashes[term_count + index]);
                let id = ids.get(term_count + index).copied();
                let contribution = weights.scale_interned(&weight, &coefficient, hash, id)?;
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
            // lane bytebound (RUSTRED_P_CACHE_BUDGET_GB; unset: one load): the
            // cache budget's check point at the key boundary. Its light
            // children are consumed; a reset of the step memo drops the
            // light-handle cache with it (as RUSTRED_PHASE_REFRESH).
            if crate::propagator::cache_budget::key_boundary(steps.threads() <= 1) {
                weights.release_handles();
            }
            if spent.len() >= 4096 {
                let capacity = if lean || prepared.is_some() { 4096 } else { 0 };
                steps.dispose(std::mem::replace(&mut spent, Vec::with_capacity(capacity)));
            }
            #[allow(clippy::collapsible_if)]
            // rr-par2: the prestaged step is dropped here unless disposing
            if let Some(p) = prestaged {
                if disposing {
                    spent_prestaged.push(p);
                    if spent_prestaged.len() >= 256 {
                        steps.dispose_prestaged(std::mem::replace(
                            &mut spent_prestaged,
                            Vec::with_capacity(256),
                        ));
                    }
                }
            }
            progress.processed_keys = progress.processed_keys.saturating_add(1);
            // A resumable save INSIDE a long phase (one thread, at a key
            // boundary): written as the state after phase - 1, so a restore
            // continues this phase from the remaining frontier (the pop order
            // depends only on the frontier: the same sequence as without a stop).
            // Low disk: save at this key boundary and PAUSE (stop), so a
            // restart continues here instead of the spill failing mid-write.
            let low_disk = resume_dir
                .as_ref()
                .is_some_and(|dir| progress.processed_keys % 4096 == 0 && resume::disk_low(dir));
            if phase > 0
                && let Some(dir) = &resume_dir
                && steps.threads() <= 1
                && (low_disk || resume::inphase_due(progress.processed_keys, &last_save))
            {
                // eng-stage: a phase whose frontier the state cannot hold
                // (once per phase on stderr): nothing is written; low disk
                // stops without a new state (the last one stays).
                if let Some(reason) = kernel.inphase_save_refusal(phase) {
                    if refused_phase != Some(phase) {
                        eprintln!("rustred resume: no state inside phase {phase} ({reason})");
                        refused_phase = Some(phase);
                    }
                    if low_disk {
                        let message = format!(
                            "paused: low disk under {} (no state inside phase {phase}: {reason})",
                            dir.display()
                        );
                        resume::storage_stop(&message);
                        return Err(modular::error(message).into());
                    }
                } else {
                    match resume_save_parts(
                        kernel,
                        weights,
                        nonzero_conditions,
                        frontier,
                        output,
                        phase - 1,
                        &dir.join("state.bin"),
                    )
                    .map_err(|failure| save_failed(dir, phase, failure))?
                    {
                        Ok(size) => {
                            eprintln!(
                                "rustred resume: state saved INSIDE phase {phase} ({size} bytes, {} keys processed): continues this phase",
                                progress.processed_keys
                            );
                            last_save = std::time::Instant::now();
                            if low_disk {
                                let message = format!(
                                    "paused: low disk under {} (state saved inside phase {phase}; restart to continue)",
                                    dir.display()
                                );
                                resume::storage_stop(&message);
                                return Err(modular::error(message).into());
                            }
                            if resume::stop_inphase() {
                                let message = "stopped after a save inside a phase (RUSTRED_RESUME_STOP_INPHASE)";
                                resume::storage_stop(message);
                                return Err(invariant(message).into());
                            }
                        }
                        Err(reason) => {
                            eprintln!("rustred resume: no state inside phase {phase} ({reason})")
                        }
                    }
                }
            }
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
        #[cfg(feature = "fwd-profile")]
        crate::prof::phase_mark(phase);
        if let Some(tail) = &mut tail {
            tail.phase_finished(phase);
        }
        state.finish_phase(kernel, phase)?;
        // stale memory (RUSTRED_PHASE_REFRESH=1, one thread): at a phase end
        // drop the step caches and the light-handle cache together: exactly
        // the state a process restarted from this phase's saved state starts
        // with (T241: -5.7 GiB at phase 19, saved state byte-identical).
        if phase_refresh() && steps.threads() <= 1 {
            kernel.release_step_caches();
            weights.release_handles();
        }
        // lane bytebound: under a cache budget the step memo (one thread) and
        // the light-handle cache are dropped at every phase end.
        if crate::propagator::cache_budget::phase_end(steps.threads() <= 1) {
            weights.release_handles();
        }
        if let Some(dir) = &resume_dir {
            let stop = resume::stop_after_phase() == Some(phase);
            if stop || last_save.elapsed().as_secs_f64() >= resume::interval() {
                match resume_save(kernel, weights, &state, phase, &dir.join("state.bin"))
                    .map_err(|failure| save_failed(dir, phase, failure))?
                {
                    Ok(size) => {
                        eprintln!("rustred resume: state saved after phase {phase} ({size} bytes)");
                        last_save = std::time::Instant::now();
                        saved_phase = Some(phase);
                    }
                    Err(reason) => {
                        eprintln!("rustred resume: no state after phase {phase} ({reason})");
                        resume_dir = None;
                    }
                }
                if stop {
                    let message = "stopped after a saved phase (RUSTRED_RESUME_STOP_AFTER_PHASE)";
                    resume::storage_stop(message);
                    return Err(invariant(message).into());
                }
            }
        }
    }
    // 12d2: the record pass is complete. Its state is KEPT (and saved now if
    // the last phase was not just saved) until the replay has produced the
    // result (`modular_output` removes state and trace then): a replay that
    // stops (memory bound, OOM, signal) restarts as a replay only.
    if let Some(dir) = &resume_dir
        && let Some(last) = state.progress.total_phases.checked_sub(1)
        && saved_phase != Some(last)
    {
        match resume_save(kernel, weights, &state, last, &dir.join("state.bin"))
            .map_err(|failure| save_failed(dir, last, failure))?
        {
            Ok(size) => {
                eprintln!(
                    "rustred resume: final state saved after phase {last} ({size} bytes): kept until the replay ends"
                );
            }
            // not a correctness issue: only the replay is then not resumable
            Err(reason) => eprintln!("rustred resume: no final state ({reason})"),
        }
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

/// vk5l-physrun channel replay (see `PLibrary::reduce_channels`).
pub(crate) fn modular_channels_set<K: FrontierKey>(
    channels: Vec<BTreeMap<K, Coefficient>>,
    lambdas: Vec<u64>,
) {
    modular::channels::set(channels, lambdas);
}

/// The per-channel terminals the last channel replay left on this thread.
pub(crate) fn modular_channels_take_out() -> Option<Vec<BTreeMap<IntegralKey, Coefficient>>> {
    modular::channels::take_out()
}

/// stale memory: `RUSTRED_PHASE_REFRESH=1` (see the phase loop).
fn phase_refresh() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("RUSTRED_PHASE_REFRESH").as_deref() == Ok("1"))
}

/// R-B2: a checkpoint (trace prefix or state) could not be written or
/// synced: a hard error naming the request directory, raised before another
/// key is dispatched or a pause is claimed. The previous `state.bin` and the
/// trace prefix it covers are untouched (a longer `ops.bin` is cut back to
/// the saved count on resume).
fn checkpoint_failed(dir: &std::path::Path, phase: usize, detail: &str) -> ReductionError {
    let message = format!(
        "resume checkpoint in phase {phase} failed under {}: {detail} (the previous state and its trace prefix are \
         kept; restart to continue from them)",
        dir.display()
    );
    eprintln!("rustred resume: {message}");
    resume::storage_stop(&message);
    modular::error(message)
}

/// eng-mem: a streamed state save that failed: the error of the checkpoint
/// (the previous state and its trace prefix are kept), with the messages of
/// whole-state save.
fn save_failed(
    dir: &std::path::Path,
    phase: usize,
    failure: resume::StateFailure<ReductionError>,
) -> ReductionError {
    match failure {
        resume::StateFailure::Content(error) => checkpoint_failed(dir, phase, &error.to_string()),
        resume::StateFailure::Write(error) => {
            checkpoint_failed(dir, phase, &format!("state write: {error}"))
        }
    }
}

/// vk5l-physrun: the saved scheduler state after `phase` (see `resume`),
/// written to `path`.
fn resume_save<K: OrderedKernel, A: OrderedWeights>(
    kernel: &K,
    weights: &mut A,
    state: &Admitted<K, A>,
    phase: usize,
    path: &std::path::Path,
) -> Result<Result<u64, String>, resume::StateFailure<ReductionError>> {
    resume_save_parts(
        kernel,
        weights,
        &state.nonzero_conditions,
        &state.frontier,
        &state.output,
        phase,
        path,
    )
}

/// The saved state from its parts (`phase`: the last COMPLETED phase; a
/// save inside phase `p` is written as after `p - 1`, see the phase loop),
/// streamed durably to `path` (eng-mem, `resume::write_state`: the same
/// bytes as the whole-state buffer of before, never a partial file at
/// `path`). R-B2: `Ok(Err(reason))`: this state cannot be saved (no state,
/// the run continues); `Err`: writing or syncing the trace prefix it covers
/// or the state itself failed (a hard error: the previous state and its
/// prefix stay usable). `Ok(Ok(size))`: the state's size.
fn resume_save_parts<K: OrderedKernel, A: OrderedWeights>(
    kernel: &K,
    weights: &mut A,
    nonzero_conditions: &[Coefficient],
    frontier: &FrontierOf<K, A>,
    output: &BTreeMap<IntegralKey, A::Weight>,
    phase: usize,
    path: &std::path::Path,
) -> Result<Result<u64, String>, resume::StateFailure<ReductionError>> {
    resume::write_state(path, |w| {
        w.raw(resume::STATE_FORMAT.as_bytes());
        w.varint(phase as u64);
        weights.resume_save(w)?;
        w.len(nonzero_conditions.len());
        for condition in nonzero_conditions {
            resume::put_coefficient(w, condition);
            w.drain();
        }
        if let Err(reason) = save_pending(kernel, &*weights, frontier, w) {
            return Ok(Err(reason));
        }
        w.len(output.len());
        for (key, weight) in output {
            w.len(key.powers().len());
            for &power in key.powers() {
                w.zigzag(power);
            }
            weights.resume_put_weight(weight, w);
            w.drain();
        }
        // 12d2: the program binding (trailer, so that the header read by
        // ops/resume_check.py stays magic, phase, op count); the state's own
        // digest over every preceding byte follows (`resume::write_state`).
        w.raw(resume_identity_digest(kernel).as_bytes());
        Ok(Ok(()))
    })
}

/// eng-mem: leading bytes of a compact form that make its sort cell.
const SAVE_CELL: usize = 11;
/// eng-mem: bytes of a compact form after its cell kept in a sort record.
const SAVE_PREFIX: usize = 20;

/// eng-mem: the leading bytes (at most [`SAVE_CELL`]) of a compact form,
/// ordered as byte strings.
#[derive(Clone, Copy, PartialEq, Eq)]
struct SaveCell {
    bytes: [u8; SAVE_CELL],
    len: u8,
}

impl SaveCell {
    fn of(encoded: &[u8]) -> Self {
        let len = encoded.len().min(SAVE_CELL);
        let mut bytes = [0; SAVE_CELL];
        bytes[..len].copy_from_slice(&encoded[..len]);
        Self {
            bytes,
            len: len as u8,
        }
    }

    fn as_slice(&self) -> &[u8] {
        &self.bytes[..usize::from(self.len)]
    }
}

impl PartialOrd for SaveCell {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SaveCell {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_slice().cmp(other.as_slice())
    }
}

/// eng-mem: one pending key in the sort of a cell: the [`SAVE_PREFIX`]
/// bytes of its compact form after the cell (zero padded) and its handle.
/// Records order as their compact forms when their prefixes differ; equal
/// prefixes are ordered by the whole forms.
#[derive(Clone, Copy)]
struct SaveRecord {
    prefix: [u8; SAVE_PREFIX],
    handle: u32,
}

impl SaveRecord {
    fn new(encoded: &[u8], cell: usize, handle: u32) -> Self {
        let rest = &encoded[cell..];
        let n = rest.len().min(SAVE_PREFIX);
        let mut prefix = [0; SAVE_PREFIX];
        prefix[..n].copy_from_slice(&rest[..n]);
        Self { prefix, handle }
    }
}

/// eng-mem: keys per sort batch of the streaming save (cells of one batch
/// are collected by one scan of the frontier; a larger cell is a batch of
/// its own). `RUSTRED_RESUME_SAVE_BATCH=<keys>` (tests) overrides.
fn save_batch_keys() -> usize {
    static KEYS: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *KEYS.get_or_init(|| {
        std::env::var("RUSTRED_RESUME_SAVE_BATCH")
            .ok()
            .and_then(|v| v.parse().ok())
            .filter(|&n| n > 0)
            .unwrap_or(1 << 26)
    })
}

/// eng-mem: the pending section of a state (count, then every pending key's
/// compact form and weight in the order of the compact forms, 4A), written
/// without a buffer of the section and without unpacking a key: one pass
/// encodes (and decodes, as a restore will) every key once and counts the
/// keys of every cell (the first [`SAVE_CELL`] bytes of the form); then,
/// batch by batch of consecutive cells, one scan collects a 24-byte record
/// per key of the batch, every cell is sorted and written. `Err(reason)`:
/// a key has no decodable compact form (no state).
fn save_pending<K: OrderedKernel, A: OrderedWeights>(
    kernel: &K,
    weights: &A,
    frontier: &FrontierOf<K, A>,
    w: &mut resume::StateWriter,
) -> Result<(), String> {
    let table = frontier
        .pending_table()
        .ok_or_else(|| "more pending keys than a classic frontier state holds".to_owned())?;
    write_pending(
        &table,
        frontier.len(),
        K::tail_encode_stored,
        |encoded| kernel.tail_key(encoded).is_some(),
        |weight, w| weights.resume_put_weight(weight, w),
        save_batch_keys(),
        w,
    )
}

/// eng-mem: [`save_pending`] for any key type: `encode` writes a stored
/// key's compact form (`false`: none), `decodable` tells whether a restore
/// can rebuild a key from it, `put_weight` writes a weight; `expected`
/// pending keys (the frontier's count), `budget` keys per batch.
fn write_pending<'f, Q: FrontierKey, P: Clone + Ord + std::hash::Hash, W>(
    table: &frontier::PendingTable<'f, Q, P, W>,
    expected: usize,
    encode: impl Fn(StoredKey<'f, Q>, &mut Vec<u8>) -> bool,
    decodable: impl Fn(&[u8]) -> bool,
    put_weight: impl Fn(&W, &mut step_cache::codec::Writer),
    budget: usize,
    w: &mut resume::StateWriter,
) -> Result<(), String> {
    const UNDECODABLE: &str = "a pending key has no decodable compact form";
    let mut scratch = Vec::new();
    let mut cells: BTreeMap<SaveCell, usize> = BTreeMap::new();
    let mut total = 0usize;
    let mut all_decodable = true;
    table.for_each(|_, key, _| {
        if !all_decodable {
            return;
        }
        scratch.clear();
        if !encode(key, &mut scratch) || !decodable(&scratch) {
            all_decodable = false;
            return;
        }
        *cells.entry(SaveCell::of(&scratch)).or_insert(0) += 1;
        total += 1;
    });
    if !all_decodable {
        return Err(UNDECODABLE.to_owned());
    }
    if total != expected {
        return Err(format!(
            "{total} pending entries found for {expected} pending keys"
        ));
    }
    w.len(total);
    let cells: Vec<(SaveCell, usize)> = cells.into_iter().collect();
    let (mut a, mut b) = (Vec::new(), Vec::new());
    let mut start = 0;
    while start < cells.len() {
        let mut end = start + 1;
        let mut size = cells[start].1;
        while end < cells.len() && size + cells[end].1 <= budget {
            size += cells[end].1;
            end += 1;
        }
        let batch = &cells[start..end];
        let (low, high) = (batch[0].0, batch[batch.len() - 1].0);
        let mut records: Vec<Vec<SaveRecord>> =
            batch.iter().map(|&(_, n)| Vec::with_capacity(n)).collect();
        table.for_each(|handle, key, _| {
            scratch.clear();
            // encodable: checked by the first pass (a pure function)
            encode(key, &mut scratch);
            let cell = SaveCell::of(&scratch);
            if cell < low || cell > high {
                return;
            }
            let at = batch
                .binary_search_by(|(c, _)| c.cmp(&cell))
                .expect("every cell was counted by the first pass");
            records[at].push(SaveRecord::new(&scratch, usize::from(cell.len), handle));
        });
        for cell in &mut records {
            cell.sort_unstable_by(|x, y| {
                x.prefix.cmp(&y.prefix).then_with(|| {
                    a.clear();
                    b.clear();
                    encode(table.get(x.handle).0, &mut a);
                    encode(table.get(y.handle).0, &mut b);
                    a.cmp(&b)
                })
            });
            for record in cell.iter() {
                let (key, weight) = table.get(record.handle);
                scratch.clear();
                encode(key, &mut scratch);
                w.len(scratch.len());
                w.raw(&scratch);
                put_weight(weight, &mut **w);
                w.drain();
            }
            *cell = Vec::new();
        }
        start = end;
    }
    Ok(())
}

/// 12d2: BLAKE3 of the kernel's resume identity (the state trailer).
fn resume_identity_digest<K: OrderedKernel>(kernel: &K) -> blake3::Hash {
    let mut identity = Vec::new();
    kernel.resume_identity(&mut identity);
    blake3::hash(&identity)
}

/// eng-mem: the BLAKE3 of the first `content` bytes of `file`, read in
/// chunks (no whole-file buffer).
fn digest_prefix(file: &mut std::fs::File, content: u64) -> std::io::Result<blake3::Hash> {
    use std::io::{Read, Seek};
    file.seek(std::io::SeekFrom::Start(0))?;
    let mut hasher = blake3::Hasher::new();
    let mut chunk = vec![0u8; 8 << 20];
    let mut left = content;
    while left > 0 {
        let n = (chunk.len() as u64).min(left) as usize;
        file.read_exact(&mut chunk[..n])?;
        hasher.update(&chunk[..n]);
        left -= n as u64;
    }
    Ok(hasher.finalize())
}

/// vk5l-physrun: the scheduler state saved by [`resume_save`] and the phase
/// to continue with. eng-mem: read as it is used (no whole-file buffer):
/// one pass checks the state's own digest and the program identity (before
/// anything of the file or the trace is used, as before), a second parses
/// it item by item ([`resume::StateReader`]).
fn resume_restore<K: OrderedKernel, A: OrderedWeights>(
    kernel: &mut K,
    weights: &mut A,
    request: &mut ReductionRequest,
    dir: &std::path::Path,
    layout: FrontierLayout,
) -> Result<(Admitted<K, A>, usize), String> {
    use std::os::unix::fs::FileExt;
    let path = dir.join("state.bin");
    let io = |e: std::io::Error| format!("{}: {e}", path.display());
    let mut file = std::fs::File::open(&path).map_err(io)?;
    let size = file.metadata().map_err(io)?.len();
    let mut head = vec![0u8; (resume::STATE_FORMAT.len() as u64).min(size) as usize];
    file.read_exact_at(&mut head, 0).map_err(io)?;
    // 12d2 (format v3): the file ends with its own BLAKE3 and, before it, the
    // BLAKE3 of the program identity that saved it; both are checked before
    // anything of the file (or the trace) is used.
    let current = head.starts_with(resume::STATE_FORMAT.as_bytes());
    if !current && !head.starts_with(resume::RESUME_VERSION.as_bytes()) {
        return Err(format!(
            "{} is not a resume state of this format ({} expected)",
            path.display(),
            resume::STATE_FORMAT
        ));
    }
    let body: u64 = if current {
        let content = size
            .checked_sub(32)
            .filter(|&n| n >= 32 + resume::STATE_FORMAT.len() as u64)
            .ok_or_else(|| format!("{} is truncated", path.display()))?;
        let own = digest_prefix(&mut file, content).map_err(io)?;
        let mut recorded = [0u8; 32];
        file.read_exact_at(&mut recorded, content).map_err(io)?;
        if recorded != *own.as_bytes() {
            return Err(format!(
                "the state file {} does not match its own digest ({} recorded, {} computed): it is damaged \
                 and nothing of it is used",
                path.display(),
                blake3::Hash::from_bytes(recorded).to_hex(),
                own.to_hex()
            ));
        }
        let body = content - 32;
        let mut saved = [0u8; 32];
        file.read_exact_at(&mut saved, body).map_err(io)?;
        let expected = resume_identity_digest(kernel);
        if saved != *expected.as_bytes() {
            return Err(format!(
                "the saved state was made by another program, library or target selection \
                 (identity {} saved, {} now)",
                blake3::Hash::from_bytes(saved).to_hex(),
                expected.to_hex()
            ));
        }
        body
    } else {
        // A v1 state carries neither a trace digest nor a program identity:
        // it may come from a projected (target-selected) run of the same keys
        // or another package revision. Refused unless the user vouches for it.
        if std::env::var("RUSTRED_RESUME_ACCEPT_V1").as_deref() != Ok("1") {
            return Err("a format v1 state (no trace digest, no program identity): it may come from a \
                 projected run or another package; set RUSTRED_RESUME_ACCEPT_V1=1 to continue it after \
                 checking that a full run of this package saved it and that its trace is intact \
                 (ops/RESUME_CHECK.md)"
                .to_owned());
        }
        eprintln!(
            "rustred resume: WARNING: RUSTRED_RESUME_ACCEPT_V1=1: continuing a format v1 state in {}: \
             it is NOT checked that this library without a target selection saved it",
            dir.display()
        );
        size
    };
    {
        use std::io::Seek;
        file.seek(std::io::SeekFrom::Start(0)).map_err(io)?;
    }
    let mut r = resume::StateReader::new(file, body);
    // format v3 (trace digest) or v1 (none: the trace is length-checked only)
    let magic = r.item(|r| r.take(resume::STATE_FORMAT.len()).map(<[u8]>::to_vec))?;
    let v2 = if magic == resume::STATE_FORMAT.as_bytes() {
        true
    } else if magic == resume::RESUME_VERSION.as_bytes() {
        false
    } else {
        return Err("another resume state version".to_owned());
    };
    let phase = r.item(|r| r.varint())? as usize;
    // The op count of the state (the trace must hold at least that many ops;
    // it is cut back to them) and, in v3, the digest of that trace prefix.
    let (ops, digest) = r.peek(|r| {
        let ops = r.varint()?;
        let digest = if v2 {
            let d: [u8; 32] = r
                .take(32)?
                .try_into()
                .map_err(|_| "a short trace digest".to_owned())?;
            Some(d)
        } else {
            None
        };
        Ok((ops, digest))
    })?;
    weights
        .resume_bind(dir, Some((ops, digest)))
        .map_err(|e| e.to_string())?;
    weights.resume_load(&mut r, v2).map_err(|e| e.to_string())?;
    let template = kernel.context().zero();
    let mut guards = GuardIndex::new();
    let mut nonzero_conditions = Vec::new();
    let n = r.item(|r| r.len(usize::MAX))?;
    for _ in 0..n {
        let condition = r.item(|r| resume::get_coefficient(r, &template))?;
        guards
            .intern(&mut nonzero_conditions, &condition, |_| {
                Ok::<(), ReductionError>(())
            })
            .map_err(|e| e.to_string())?;
    }
    let mut frontier = Frontier::new(layout);
    // As admit: the census of the resident coefficient size of the frontier
    // (summed as the keys are read; the same sum as over the frontier).
    let mut monomials = 0usize;
    let n = r.item(|r| r.len(usize::MAX))?;
    // (every entry takes at least two bytes: a count read from an unchecked
    // v1 state reserves no more than its body can hold)
    frontier.reserve(n.min(usize::try_from(body / 2).unwrap_or(usize::MAX)));
    for _ in 0..n {
        let (key, weight) = {
            let (kernel, weights) = (&*kernel, &*weights);
            r.item(|r| {
                let len = r.len(usize::MAX)?;
                let encoded = r.take(len)?;
                let key = kernel
                    .tail_key(encoded)
                    .ok_or("an undecodable pending key")?;
                let weight = weights.resume_get_weight(r).map_err(|e| e.to_string())?;
                Ok((key, weight))
            })?
        };
        let position = kernel
            .position(&key)
            .map_err(|_| "a pending key without a position".to_owned())?;
        // As admit: every pending entry holds one frame of the request.
        request
            .retain_pending_frame(
                kernel
                    .limits()
                    .diagnostic_limit(kernel.limits().max_pending_frames),
            )
            .map_err(|e| e.to_string())?;
        monomials =
            monomials.saturating_add(weights.monomials(&weight).map_err(|e| e.to_string())?);
        frontier.insert(key, Pending { position, weight });
    }
    let mut output = BTreeMap::new();
    let mut terminal_monomials = 0usize;
    let n = r.item(|r| r.len(usize::MAX))?;
    for _ in 0..n {
        let (key, weight) = {
            let weights = &*weights;
            r.item(|r| {
                let len = r.len(usize::MAX)?;
                let powers = (0..len)
                    .map(|_| r.zigzag())
                    .collect::<Result<Vec<_>, _>>()?;
                let key = IntegralKey::try_new(powers).map_err(|e| e.to_string())?;
                let weight = weights.resume_get_weight(r).map_err(|e| e.to_string())?;
                Ok((key, weight))
            })?
        };
        terminal_monomials = terminal_monomials
            .saturating_add(weights.monomials(&weight).map_err(|e| e.to_string())?);
        output.insert(key, weight);
    }
    let progress = CandidateOrderedReductionProgress {
        terminal_coefficient_monomials: terminal_monomials,
        peak_terminal_coefficient_monomials: terminal_monomials,
        total_phases: kernel.phase_count(),
        completed_phases: phase + 1,
        frontier_keys: frontier.len(),
        frontier_coefficient_monomials: monomials,
        peak_frontier_keys: frontier.len(),
        peak_frontier_coefficient_monomials: monomials,
        ..Default::default()
    };
    Ok((
        Admitted {
            guards,
            nonzero_conditions,
            frontier,
            output,
            progress,
        },
        phase + 1,
    ))
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
        // Discovery grows from half a batch until the interpolations converge,
        // then every prime replays exactly the needed points, and the
        // verification prime its checks; nothing is computed ahead of need.
        let needed = statistics.points_per_prime;
        let first = statistics.points_first_prime;
        // Discovery starts with half a batch and grows by a quarter of the
        // points drawn (at least an eighth of a batch) until convergence.
        assert!(
            first >= needed && first <= (needed * 5 / 4 + 16).max(64),
            "{first} {needed}"
        );
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
            OrderedWeightArithmetic::Exact | OrderedWeightArithmetic::Tadic(..) => {
                unreachable!("lifted modular arithmetics only")
            }
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

    /// eng-mem: the pending section written by the streaming save
    /// (`write_pending`: cells, batches, 24-byte records, ties by the whole
    /// form) is byte for byte the section of the buffered save it replaces
    /// (every pending key unpacked and encoded into its own buffer, all
    /// sorted by their compact forms), for both layouts, with tombstones,
    /// reused slots, wide keys, ties beyond the record prefix and every
    /// batch size; a key without a compact form leaves no state.
    #[test]
    fn the_streamed_pending_section_is_the_buffered_one() {
        use rand::{Rng, SeedableRng, rngs::StdRng};
        use step_cache::codec::{Writer, encode_node};
        let encode_key =
            |key: &IntegralKey, out: &mut Vec<u8>| encode_node(OrderedKeyStage::Integral, key, out);
        // the packed form encoded without unpacking (as the p library does)
        let encode = |key: StoredKey<'_, IntegralKey>, out: &mut Vec<u8>| match key {
            StoredKey::Key(key) => encode_key(key, out),
            StoredKey::Packed(packed) => {
                let start = out.len();
                out.push(0);
                out.extend_from_slice(&0u64.to_le_bytes());
                packed.encode_i8_into(out) || {
                    out.truncate(start);
                    false
                }
            }
        };
        let dir = std::env::temp_dir().join(format!("rustred-pending-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("state.bin");
        for (seed, mode) in [
            (11, FrontierMode::Phased),
            (12, FrontierMode::Compact),
            (13, FrontierMode::Classic),
        ] {
            let mut rng = StdRng::seed_from_u64(seed);
            let mut frontier: Frontier<IntegralKey, i64, i64> = Frontier::new(mode);
            let mut keys = Vec::new();
            for _ in 0..3000 {
                let len = match rng.random_range(0..10) {
                    0 => rng.random_range(1..4),
                    1 => rng.random_range(39..44),
                    _ => 36,
                };
                // mostly 0 / 1: many forms share the first 31 bytes
                let powers: Vec<i64> = (0..len)
                    .map(|_| match rng.random_range(0..12) {
                        0 => rng.random_range(-3..4),
                        1 => -128,
                        2 => 127,
                        _ => rng.random_range(0..2),
                    })
                    .collect();
                let key = IntegralKey::try_new(powers).unwrap();
                let phase = (key.powers().iter().sum::<i64>().unsigned_abs() % 4) as usize;
                let priority = key.powers()[0];
                if frontier.remove(&key).is_some() {
                    continue;
                }
                let weight = rng.random_range(1..1000);
                frontier.insert(
                    key.clone(),
                    Pending {
                        position: OrderedPosition { phase, priority },
                        weight,
                    },
                );
                keys.push(key);
            }
            // pops free slots, the inserts after them reuse ids; removals
            // leave tombstones
            for _ in 0..200 {
                frontier.pop_in_phase(0);
            }
            for key in keys.iter().step_by(7) {
                frontier.remove(key);
            }
            for _ in 0..300 {
                let powers: Vec<i64> = (0..36).map(|_| rng.random_range(0..3)).collect();
                let key = IntegralKey::try_new(powers).unwrap();
                if frontier.remove(&key).is_none() {
                    let position = OrderedPosition {
                        phase: 1 + rng.random_range(0..3),
                        priority: 0,
                    };
                    frontier.insert(
                        key,
                        Pending {
                            position,
                            weight: 5,
                        },
                    );
                }
            }
            let mut entries: Vec<(Vec<u8>, i64)> = frontier
                .iter()
                .map(|(scheduled, weight)| {
                    let mut bytes = Vec::new();
                    assert!(encode_key(&scheduled.key, &mut bytes));
                    (bytes, *weight)
                })
                .collect();
            entries.sort_unstable_by(|a, b| a.0.cmp(&b.0));
            let mut reference = Writer::default();
            reference.len(entries.len());
            for (bytes, weight) in &entries {
                reference.len(bytes.len());
                reference.raw(bytes);
                reference.zigzag(*weight);
            }
            let table = frontier.pending_table().unwrap();
            for budget in [1, 3, 100, 1000, usize::MAX] {
                let written = resume::write_state::<()>(&path, |w| {
                    Ok(write_pending(
                        &table,
                        frontier.len(),
                        encode,
                        |_| true,
                        |weight, w| w.zigzag(*weight),
                        budget,
                        w,
                    ))
                });
                assert!(matches!(written, Ok(Ok(_))), "{mode:?} budget {budget}");
                let file = std::fs::read(&path).unwrap();
                assert_eq!(
                    file[..file.len() - 32],
                    reference.bytes[..],
                    "{mode:?} budget {budget}"
                );
            }
            // a key that has no compact form: no state, the previous kept
            let before = std::fs::read(&path).unwrap();
            let refused = resume::write_state::<()>(&path, |w| {
                Ok(write_pending(
                    &table,
                    frontier.len(),
                    encode,
                    |bytes| !bytes.ends_with(&[1, 1, 1]),
                    |weight, w| w.zigzag(*weight),
                    usize::MAX,
                    w,
                ))
            });
            assert!(
                matches!(&refused, Ok(Err(reason)) if reason.contains("no decodable compact form")),
                "{mode:?}"
            );
            assert_eq!(std::fs::read(&path).unwrap(), before);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
