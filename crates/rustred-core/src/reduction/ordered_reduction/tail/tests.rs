//! The adjoint tail returns the forward run's terms and guards (values and
//! order) in every arithmetic, scheduler and cache state, rewrites recorded
//! programs, and closes a tail from its table only when that provably
//! leaves the guards unchanged.
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use super::{TailReport, take_reported};
use crate::algebra::{Coefficient, CoefficientContext};
use crate::family::IntegralKey;
use crate::reduction::ordered_reduction::step_cache::codec::NodeStage;
use crate::reduction::ordered_reduction::step_cache::kernel::{
    ConcurrentStepCacheKernel, OrderedStepCacheKernel, run_with_step_cache_and_options,
};
use crate::reduction::ordered_reduction::step_cache::tests::Scratch;
use crate::reduction::ordered_reduction::{
    ConcurrentOrderedKernel, OrderedKernel, OrderedKeyStage, OrderedOutput, OrderedPosition,
    OrderedStep, OrderedStepper, invariant, modular,
};
use crate::reduction::{
    ModularCommon, OrderedSchedulerOptions, OrderedStepCache, OrderedStepCacheAdapter,
    OrderedStepCacheBinding, OrderedStepCacheOptions, OrderedStepShard, OrderedStepStage,
    OrderedWeightArithmetic, PolynomialOptions, ReductionError, ReductionLimits, ReductionRequest,
    ReductionStatistics, SampledOptions,
};
use crate::solver::CandidateOrderedReductionProgress;

type Key = [i64; 2];
type Edge = (Key, &'static str);

/// A head phase with several terminals, a Route-like phase (children only
/// in the next phase, constant coefficients) and an owner-like phase (same-
/// phase children, rational coefficients, guards, one master), then an empty
/// phase. Route keys `[1,3]` and `[1,4]` receive equal weights from `[0,4]`
/// and cancel each other into `[2,7]`, whose step (and guard `d+7`) the
/// forward pass never sees. `[1,10]` and `[2,12]` are reached by no request
/// but `request_b` (a table of `request_a` does not cover it).
struct Program {
    context: CoefficientContext,
    /// The steps, parsed once (a step never parses: requests that stop at
    /// a refused resource checkpoint must not trip the fixture parser).
    steps: BTreeMap<Key, OrderedStep<IntegralKey>>,
}

impl Program {
    fn new() -> Self {
        let mut steps = BTreeMap::new();
        let mut step = |key: Key, terms: &[Edge], terminals: &[Edge], guards: &[&'static str]| {
            steps.insert(key, (terms.to_vec(), terminals.to_vec(), guards.to_vec()));
        };
        step(
            [0, 4],
            &[
                ([1, 9], "2"),
                ([1, 8], "-1"),
                ([2, 11], "d"),
                ([1, 3], "3"),
                ([1, 4], "3"),
            ],
            &[([9, 1], "1/(d-3)")],
            &["d+1"],
        );
        step(
            [0, 3],
            &[([0, 1], "d+2"), ([1, 7], "1"), ([1, 6], "(d-1)/(d+4)")],
            &[([9, 2], "2")],
            &["d-3"],
        );
        step(
            [0, 2],
            &[([1, 5], "-2"), ([2, 10], "1")],
            &[([9, 1], "-1"), ([9, 2], "1"), ([9, 0], "1")],
            &["d+1", "d+2"],
        );
        step(
            [0, 1],
            &[([1, 9], "1"), ([1, 2], "4")],
            &[([9, 2], "d")],
            &[],
        );
        step([0, 0], &[], &[([9, 3], "1")], &[]);
        step(
            [1, 9],
            &[([2, 11], "1"), ([2, 9], "-3"), ([2, 5], "2")],
            &[],
            &[],
        );
        step([1, 8], &[([2, 10], "2"), ([2, 8], "1")], &[], &[]);
        step([1, 7], &[([2, 6], "-1"), ([2, 4], "5/2")], &[], &[]);
        step([1, 6], &[([2, 9], "1"), ([2, 3], "1")], &[], &[]);
        step([1, 5], &[([2, 8], "3"), ([2, 2], "-1")], &[], &[]);
        step([1, 4], &[([2, 7], "-1"), ([2, 1], "1")], &[], &[]);
        step([1, 3], &[([2, 7], "1"), ([2, 6], "1")], &[], &[]);
        step([1, 2], &[([2, 5], "1/3"), ([2, 0], "2")], &[], &[]);
        step([1, 10], &[([2, 12], "1"), ([2, 6], "1")], &[], &[]);
        // `[1,12]`/`[1,11]` cancel into `[2,16]`, whose guard `d+9` lies two
        // levels below it (`[2,15]` -> `[2,14]`).
        step([1, 12], &[([2, 16], "1"), ([2, 1], "1")], &[], &[]);
        step([1, 11], &[([2, 16], "-1"), ([2, 6], "1")], &[], &[]);
        step([2, 16], &[([2, 15], "1"), ([2, 0], "1")], &[], &[]);
        step([2, 15], &[([2, 14], "1")], &[], &[]);
        step([2, 14], &[([2, 0], "1")], &[], &["d+9"]);
        step([2, 12], &[([2, 2], "1"), ([2, 0], "1")], &[], &[]);
        step(
            [2, 11],
            &[([2, 10], "(d-2)/(d+1)"), ([2, 4], "1")],
            &[],
            &["d-1"],
        );
        step([2, 10], &[([2, 9], "d"), ([2, 3], "-1")], &[], &[]);
        step(
            [2, 9],
            &[([2, 8], "1/(2*d-1)"), ([2, 1], "3")],
            &[],
            &["2*d-1"],
        );
        step([2, 8], &[([2, 6], "2"), ([2, 5], "-1")], &[], &[]);
        step(
            [2, 7],
            &[([2, 6], "1"), ([2, 0], "(d+7)/(d-5)")],
            &[],
            &["d+7"],
        );
        step([2, 6], &[([2, 4], "d-4"), ([2, 2], "1")], &[], &[]);
        step(
            [2, 5],
            &[([2, 3], "1"), ([2, 1], "-1/(d+1)")],
            &[],
            &["d-1"],
        );
        step([2, 4], &[([2, 2], "2"), ([2, 0], "1")], &[], &[]);
        step([2, 3], &[([2, 1], "d"), ([2, 0], "-2")], &[], &[]);
        step([2, 2], &[([2, 1], "(d+1)/(d-4)")], &[], &["d+2"]);
        step([2, 1], &[([2, 0], "1/(d-6)")], &[], &[]);
        step([2, 0], &[], &[([9, 0], "1")], &["d-6"]);
        let context = CoefficientContext::try_new(["d"]).unwrap();
        let c = |text: &str| context.coefficient_fixture(text);
        let edges = |edges: &[Edge]| {
            edges
                .iter()
                .map(|(key, text)| (Self::key(*key), c(text)))
                .collect()
        };
        let steps = steps
            .into_iter()
            .map(|(key, (terms, terminals, guards))| {
                let step = OrderedStep {
                    terms: edges(&terms),
                    terminals: edges(&terminals),
                    nonzero_conditions: guards.iter().map(|text| c(text)).collect(),
                };
                (key, step)
            })
            .collect();
        Self { context, steps }
    }

    fn key(key: Key) -> IntegralKey {
        IntegralKey::try_new(key).unwrap()
    }

    fn c(&self, text: &str) -> Coefficient {
        self.context.coefficient_fixture(text)
    }

    fn step(&self, key: &IntegralKey) -> Result<OrderedStep<IntegralKey>, ReductionError> {
        let powers = key.powers();
        let step = self
            .steps
            .get(&[powers[0], powers[1]])
            .ok_or_else(|| invariant("unknown tail test key"))?;
        Ok(OrderedStep {
            terms: step.terms.clone(),
            terminals: step.terminals.clone(),
            nonzero_conditions: step.nonzero_conditions.clone(),
        })
    }

    fn inputs(&self, keys: &[(Key, &str)]) -> BTreeMap<IntegralKey, Coefficient> {
        keys.iter()
            .map(|(key, text)| (Self::key(*key), self.c(text)))
            .collect()
    }

    fn request_a(&self) -> BTreeMap<IntegralKey, Coefficient> {
        self.inputs(&[
            ([0, 4], "1"),
            ([0, 3], "d"),
            ([0, 2], "2"),
            ([0, 1], "-1"),
            ([0, 0], "1"),
            ([1, 9], "1"),
            ([2, 11], "1/d"),
        ])
    }

    /// Tail keys only, partly outside `request_a`'s tail: `[1,3]` and
    /// `[1,4]` cancel into `[2,7]`, every other reached key is expanded.
    fn request_b(&self) -> BTreeMap<IntegralKey, Coefficient> {
        self.inputs(&[([1, 10], "1"), ([1, 4], "1"), ([1, 3], "1")])
    }
}

struct TailKernel<'a> {
    program: &'a Program,
    steps: Vec<IntegralKey>,
    /// Set when the last phase finished (arms a test budget).
    finished: Option<Arc<AtomicBool>>,
}

impl OrderedKernel for TailKernel<'_> {
    type Key = IntegralKey;
    type Priority = i64;
    type Error = ReductionError;

    fn context(&self) -> &CoefficientContext {
        &self.program.context
    }
    fn limits(&self) -> ReductionLimits {
        ReductionLimits::default()
    }
    fn phase_count(&self) -> usize {
        4
    }
    fn position(&mut self, key: &IntegralKey) -> Result<OrderedPosition<i64>, ReductionError> {
        Ok(position(key))
    }
    fn step(
        &mut self,
        key: &IntegralKey,
        _: &mut ReductionRequest,
        _: &mut ReductionStatistics,
    ) -> Result<OrderedStep<IntegralKey>, ReductionError> {
        self.steps.push(key.clone());
        self.program.step(key)
    }
    fn key_view(key: &IntegralKey) -> Option<(OrderedKeyStage<'_>, &IntegralKey)> {
        Some((OrderedKeyStage::Integral, key))
    }
    fn observe(
        &mut self,
        progress: &CandidateOrderedReductionProgress,
        _: Option<&IntegralKey>,
    ) -> Result<(), ReductionError> {
        if progress.completed_phases == self.phase_count()
            && let Some(finished) = &self.finished
        {
            finished.store(true, Ordering::SeqCst);
        }
        Ok(())
    }
}

fn position(key: &IntegralKey) -> OrderedPosition<i64> {
    OrderedPosition {
        phase: key.powers()[0] as usize,
        priority: key.powers()[1],
    }
}

struct TailStepper<'a> {
    program: &'a Program,
}

impl OrderedStepper for TailStepper<'_> {
    type Key = IntegralKey;
    type Priority = i64;
    type Error = ReductionError;
    type Account = IntegralKey;

    fn position(&self, key: &IntegralKey) -> Result<OrderedPosition<i64>, ReductionError> {
        Ok(position(key))
    }
    fn step(
        &self,
        key: &IntegralKey,
    ) -> (
        Result<OrderedStep<IntegralKey>, ReductionError>,
        IntegralKey,
    ) {
        (self.program.step(key), key.clone())
    }
}

impl ConcurrentOrderedKernel for TailKernel<'_> {
    type Account = IntegralKey;
    type Stepper<'s>
        = TailStepper<'s>
    where
        Self: 's;

    fn stepper(&self) -> Option<TailStepper<'_>> {
        Some(TailStepper {
            program: self.program,
        })
    }
    fn absorb(
        &mut self,
        key: &IntegralKey,
        _: IntegralKey,
        _: &mut ReductionRequest,
        _: &mut ReductionStatistics,
    ) -> Result<(), ReductionError> {
        self.steps.push(key.clone());
        Ok(())
    }
}

fn shard(key: &IntegralKey) -> Option<OrderedStepShard> {
    Some(OrderedStepShard {
        stage: OrderedStepStage::Integral,
        support: key.powers()[0] as u16,
        class: 0,
    })
}

fn rebuild(stage: NodeStage, integral: IntegralKey) -> Result<IntegralKey, ReductionError> {
    match stage {
        NodeStage::Integral => Ok(integral),
        _ => Err(invariant("a recorded tail test key is not an integral")),
    }
}

impl OrderedStepCacheKernel for TailKernel<'_> {
    fn step_cache_shard(&mut self, key: &IntegralKey) -> Option<OrderedStepShard> {
        shard(key)
    }
    fn step_cache_key(
        &self,
        stage: NodeStage,
        integral: IntegralKey,
    ) -> Result<IntegralKey, ReductionError> {
        rebuild(stage, integral)
    }
    fn step_cache_hit(&mut self, key: &IntegralKey) -> Result<(), ReductionError> {
        self.steps.push(key.clone());
        Ok(())
    }
}

impl ConcurrentStepCacheKernel for TailKernel<'_> {
    fn stepper_shard<'s>(_: &TailStepper<'s>, key: &IntegralKey) -> Option<OrderedStepShard>
    where
        Self: 's,
    {
        shard(key)
    }
    fn stepper_key<'s>(
        _: &TailStepper<'s>,
        stage: NodeStage,
        integral: IntegralKey,
    ) -> Result<IntegralKey, ReductionError>
    where
        Self: 's,
    {
        rebuild(stage, integral)
    }
}

struct Run {
    output: OrderedOutput,
    /// Keys whose steps the run applied, computed or cached (sequential
    /// order).
    steps: Vec<IntegralKey>,
    tail: Vec<TailReport>,
    ops: Option<usize>,
}

fn run(
    program: &Program,
    initial: &BTreeMap<IntegralKey, Coefficient>,
    conditions: &[&str],
    arithmetic: OrderedWeightArithmetic,
    scheduler: OrderedSchedulerOptions,
    cache: Option<&OrderedStepCache>,
) -> Run {
    let mut kernel = TailKernel {
        program,
        steps: Vec::new(),
        finished: None,
    };
    take_reported();
    modular::take_reported_statistics();
    let output = run_with_step_cache_and_options(
        &mut kernel,
        &mut ReductionRequest::default(),
        initial.clone(),
        conditions.iter().map(|text| program.c(text)).collect(),
        arithmetic,
        scheduler,
        cache,
    )
    .unwrap();
    Run {
        output,
        steps: kernel.steps,
        tail: take_reported(),
        ops: modular::take_reported_statistics()
            .last()
            .map(|statistics| statistics.ops),
    }
}

/// The arithmetics under test. `Exact` is left out under the
/// `RUSTRED_TEST_ORDERED_MODULAR` override, which turns it into a
/// cross-checked modular arithmetic.
fn arithmetics() -> Vec<OrderedWeightArithmetic> {
    let mut all = every_arithmetic();
    if std::env::var_os("RUSTRED_TEST_ORDERED_MODULAR").is_some() {
        all.retain(|arithmetic| *arithmetic != OrderedWeightArithmetic::Exact);
    }
    all
}

fn every_arithmetic() -> Vec<OrderedWeightArithmetic> {
    let common = ModularCommon {
        threads: 2,
        ..Default::default()
    };
    let checked = ModularCommon {
        cross_check_exact: true,
        ..common
    };
    let sampled = SampledOptions {
        batch_points: 4,
        ..Default::default()
    };
    let polynomial = PolynomialOptions {
        lanes: 2,
        ..Default::default()
    };
    vec![
        OrderedWeightArithmetic::Exact,
        OrderedWeightArithmetic::ModularSampled(common, sampled),
        OrderedWeightArithmetic::ModularSampled(checked, sampled),
        OrderedWeightArithmetic::ModularPolynomial(common, polynomial),
        OrderedWeightArithmetic::ModularPolynomial(checked, polynomial),
    ]
}

fn schedulers(adjoint_tail: bool) -> Vec<OrderedSchedulerOptions> {
    [(1, 1), (2, 1), (3, 2), (4, 4096)]
        .into_iter()
        .map(|(threads, window_keys)| OrderedSchedulerOptions {
            threads,
            window_keys,
            adjoint_tail,
        })
        .collect()
}

fn binding(program: &Program) -> OrderedStepCacheBinding {
    OrderedStepCacheBinding::new(
        OrderedStepCacheAdapter::CandidateProgram,
        "ordered tail test",
        &program.context,
        None,
    )
}

fn is_tail(key: &IntegralKey) -> bool {
    matches!(key.powers()[0], 1 | 2)
}

fn same_result(left: &OrderedOutput, right: &OrderedOutput) {
    assert_eq!(left.terms, right.terms);
    assert_eq!(left.nonzero_conditions, right.nonzero_conditions);
}

const RETAINED: [&str; 6] = ["d-2", "d-1", "2*d-1", "d+7", "d+2", "d-6"];
/// `[2,7]`'s guard retained: its erasure no longer matters for guards.
const SAFE: [&str; 2] = ["d-2", "d+7"];
/// `[2,7]` is erased by the forward pass and carries the new guard `d+7`.
const SENSITIVE: [&str; 1] = ["d-2"];

fn recorded(arithmetic: OrderedWeightArithmetic) -> bool {
    arithmetic != OrderedWeightArithmetic::Exact
}

fn probes(arithmetic: OrderedWeightArithmetic) -> bool {
    match arithmetic {
        OrderedWeightArithmetic::Exact => false,
        OrderedWeightArithmetic::ModularSampled(common, _)
        | OrderedWeightArithmetic::ModularPolynomial(common, _) => !common.cross_check_exact,
    }
}

/// Without a table: the recorded tail program is replaced, everything the
/// scheduler does and returns is the forward run's; an erased key carrying
/// a guard new to the tail keeps the forward program.
#[test]
fn adjoint_tail_rewrites_recorded_programs_and_changes_no_result() {
    if super::forced_by_environment() {
        // The forward reference would be an adjoint run as well.
        return;
    }
    let program = Program::new();
    let initial = program.request_a();
    for arithmetic in arithmetics() {
        for conditions in [&SAFE[..], &SENSITIVE[..]] {
            let forward = run(
                &program,
                &initial,
                conditions,
                arithmetic,
                schedulers(false)[0],
                None,
            );
            assert!(forward.tail.is_empty());
            for scheduler in schedulers(true) {
                let adjoint = run(&program, &initial, conditions, arithmetic, scheduler, None);
                same_result(&adjoint.output, &forward.output);
                assert_eq!(adjoint.output.progress, forward.output.progress);
                if !recorded(arithmetic) {
                    // Nothing to rewrite and no table: no tail run at all.
                    assert!(adjoint.tail.is_empty());
                    assert_eq!(adjoint.steps, forward.steps);
                    continue;
                }
                let report = adjoint.tail.last().expect("a tail report");
                assert_eq!(report.start_phase, Some(1));
                // `[2,7]` (erased) is stepped after the run to complete the
                // tail.
                assert_eq!(report.closure_steps, 1, "{report:?}");
                let mut forward_steps = forward.steps.clone();
                forward_steps.push(Program::key([2, 7]));
                assert_eq!(adjoint.steps, forward_steps);
                if conditions == SENSITIVE {
                    assert_eq!(report.mode, "forward", "{arithmetic} {report:?}");
                    assert_eq!(report.sensitive_keys, 1, "{report:?}");
                    assert_eq!(adjoint.ops, forward.ops);
                } else {
                    assert_eq!(report.mode, "adjoint", "{arithmetic} {report:?}");
                    assert_ne!(adjoint.ops, forward.ops);
                }
            }
        }
    }
}

/// A request of `program` from `initial` (conditions `SAFE`) under a test
/// budget armed when its last phase finished: its result and the
/// checkpoints the budget saw (it refuses from the `stop_at`-th on).
fn stopped_after_the_last_phase(
    program: &Program,
    initial: &BTreeMap<IntegralKey, Coefficient>,
    arithmetic: OrderedWeightArithmetic,
    scheduler: OrderedSchedulerOptions,
    stop_at: Option<usize>,
    cache: Option<&OrderedStepCache>,
) -> (Result<OrderedOutput, ReductionError>, usize) {
    use crate::reduction::stopping::Stopping;
    let stopping = Stopping::disarmed(stop_at);
    let mut kernel = TailKernel {
        program,
        steps: Vec::new(),
        finished: Some(stopping.trigger()),
    };
    let result = stopping.run(|| {
        run_with_step_cache_and_options(
            &mut kernel,
            &mut ReductionRequest::default(),
            initial.clone(),
            SAFE.iter().map(|text| program.c(text)).collect(),
            arithmetic,
            scheduler,
            cache,
        )
    });
    take_reported();
    modular::take_reported_statistics();
    (result, stopping.seen())
}

/// A cancellation (a refused resource budget) at any checkpoint after the
/// last phase - the adjoint tail's exact values, the end of the recording,
/// the modular replays and the lift - stops the request with that typed
/// error, in every modular arithmetic and scheduler; the next request on the
/// same program returns the same result as before.
#[test]
fn a_request_stopped_after_its_last_phase_returns_the_refusal_and_keeps_no_state() {
    use crate::reduction::stopping::is_stop;
    let program = Program::new();
    let initial = program.request_a();
    let stopped = |arithmetic, scheduler, stop_at| {
        stopped_after_the_last_phase(&program, &initial, arithmetic, scheduler, stop_at, None)
    };
    for arithmetic in every_arithmetic().into_iter().filter(|a| recorded(*a)) {
        for scheduler in [
            schedulers(true)[0],
            schedulers(true)[3],
            schedulers(false)[2],
        ] {
            let reference = run(&program, &initial, &SAFE, arithmetic, scheduler, None);
            let (clean, checkpoints) = stopped(arithmetic, scheduler, None);
            same_result(&clean.unwrap(), &reference.output);
            assert!(checkpoints > 0, "{arithmetic} {scheduler:?}");
            for stop_at in 1..=checkpoints {
                let (result, _) = stopped(arithmetic, scheduler, Some(stop_at));
                let Err(error) = result else {
                    panic!(
                        "{arithmetic} {scheduler:?}: checkpoint {stop_at} did not stop the request"
                    );
                };
                assert!(
                    is_stop(&error),
                    "{arithmetic} {scheduler:?} {stop_at}: {error}"
                );
            }
            let again = run(&program, &initial, &SAFE, arithmetic, scheduler, None);
            same_result(&again.output, &reference.output);
        }
    }
}

/// The same stops with a populating table (a fresh one per request): the
/// request returns the refusal and leaves either an empty table (stopped in
/// the tail's exact values) or exactly the table of a complete request
/// (stopped later): entries are tabulated in one update once every value is
/// known, so they depend on the program alone. The next request on either
/// kind of table returns the same result.
#[test]
fn a_request_stopped_after_its_last_phase_leaves_only_complete_tail_entries() {
    use crate::reduction::stopping::is_stop;
    /// Entries, edges, values, coefficients, guards, guard sets, terminals.
    fn shape(cache: &OrderedStepCache) -> [usize; 7] {
        cache.tail_statistics().map_or([0; 7], |s| {
            [
                s.entries,
                s.edges,
                s.values,
                s.coefficients,
                s.guards,
                s.guard_sets,
                s.terminals,
            ]
        })
    }
    let program = Program::new();
    let initial = program.request_a();
    let stopped = |arithmetic, scheduler, stop_at| {
        let cache = OrderedStepCache::in_memory(binding(&program), Default::default());
        let (result, seen) = stopped_after_the_last_phase(
            &program,
            &initial,
            arithmetic,
            scheduler,
            stop_at,
            Some(&cache),
        );
        (result, seen, cache)
    };
    for arithmetic in every_arithmetic().into_iter().filter(|a| recorded(*a)) {
        for scheduler in [schedulers(true)[0], schedulers(true)[3]] {
            let reference = run(&program, &initial, &SAFE, arithmetic, scheduler, None);
            let (clean, checkpoints, cache) = stopped(arithmetic, scheduler, None);
            same_result(&clean.unwrap(), &reference.output);
            let complete = shape(&cache);
            assert!(
                complete[0] >= 20,
                "{arithmetic} {scheduler:?}: {complete:?}"
            );
            let (mut empty, mut full) = (0, 0);
            for stop_at in 1..=checkpoints {
                let (result, _, cache) = stopped(arithmetic, scheduler, Some(stop_at));
                let Err(error) = result else {
                    panic!(
                        "{arithmetic} {scheduler:?}: checkpoint {stop_at} did not stop the request"
                    );
                };
                assert!(
                    is_stop(&error),
                    "{arithmetic} {scheduler:?} {stop_at}: {error}"
                );
                let left = shape(&cache);
                let first = if left == [0; 7] {
                    empty += 1;
                    empty == 1
                } else {
                    assert_eq!(left, complete, "{arithmetic} {scheduler:?} {stop_at}");
                    full += 1;
                    full == 1
                };
                // Tables of one kind are alike: one next request each.
                if first {
                    let again = run(
                        &program,
                        &initial,
                        &SAFE,
                        arithmetic,
                        scheduler,
                        Some(&cache),
                    );
                    same_result(&again.output, &reference.output);
                }
            }
            // Stops on both sides of the tabulation.
            assert!(
                empty > 0 && full > 0,
                "{arithmetic} {scheduler:?}: {empty} {full}"
            );
        }
    }
}

/// With a table: a cold run populates it; a warm run closes the tail when
/// every tabulated guard is retained (any arithmetic) or when probe
/// arithmetics can repeat the forward decisions, never when an erased key's
/// guards would matter, and returns the forward run's terms and guards
/// either way.
#[test]
fn tail_table_closes_the_tail_only_when_guards_are_unchanged() {
    let program = Program::new();
    let initial = program.request_a();
    let only_tail = program.inputs(&[([1, 9], "1"), ([2, 8], "d"), ([1, 3], "-2")]);
    let uncached = |mut progress: crate::solver::CandidateOrderedReductionProgress| {
        (progress.step_cache_hits, progress.step_cache_misses) = (0, 0);
        progress
    };
    for arithmetic in arithmetics() {
        let forward = |initial, conditions: &[&str]| {
            run(
                &program,
                initial,
                conditions,
                arithmetic,
                schedulers(false)[0],
                None,
            )
            .output
        };
        for scheduler in schedulers(true) {
            let cache = OrderedStepCache::in_memory(binding(&program), Default::default());
            let cold = run(
                &program,
                &initial,
                &SAFE,
                arithmetic,
                scheduler,
                Some(&cache),
            );
            same_result(&cold.output, &forward(&initial, &SAFE));
            let report = cold.tail.last().unwrap();
            assert_eq!(report.closed_phase, None, "{report:?}");
            if !recorded(arithmetic) && scheduler.threads > 1 {
                // The concurrent exact scheduler only reads tables:
                // populate it sequentially.
                assert_eq!(report.added_entries, 0, "{report:?}");
                let cold = run(
                    &program,
                    &initial,
                    &SAFE,
                    arithmetic,
                    schedulers(true)[0],
                    Some(&cache),
                );
                same_result(&cold.output, &forward(&initial, &SAFE));
            } else {
                assert!(report.added_entries >= 20, "{report:?}");
            }
            assert!(cache.tail_statistics().unwrap().entries >= 20);

            // New guards in the tail: probe arithmetics repeat the forward
            // decisions over the table; exact decisions expand the tail.
            let warm = run(
                &program,
                &initial,
                &SAFE,
                arithmetic,
                scheduler,
                Some(&cache),
            );
            let reference = forward(&initial, &SAFE);
            same_result(&warm.output, &reference);
            let report = warm.tail.last().unwrap();
            assert!(report.guard_misses > 0, "{report:?}");
            if probes(arithmetic) {
                assert_eq!(report.mode, "closed-probes", "{report:?}");
                assert_eq!(report.closed_phase, Some(1), "{report:?}");
                assert!(report.carriers > 0, "{report:?}");
                assert!(!warm.steps.iter().any(is_tail), "{:?}", warm.steps);
                assert_eq!(
                    warm.output.progress.processed_keys,
                    reference.progress.processed_keys
                );
                assert_eq!(
                    warm.output.progress.emitted_terms,
                    reference.progress.emitted_terms
                );
            } else {
                assert_eq!(report.closed_phase, None, "{report:?}");
                assert!(warm.steps.iter().any(is_tail));
                assert_eq!(
                    uncached(warm.output.progress),
                    uncached(cold.output.progress)
                );
            }

            // `[2,7]` is erased with a new guard in phase 1: no closing
            // before the forward pass made (and recorded) that erasure;
            // probe arithmetics then close at phase 2, which never reaches
            // `[2,7]` again.
            let warm = run(
                &program,
                &initial,
                &SENSITIVE,
                arithmetic,
                scheduler,
                Some(&cache),
            );
            same_result(&warm.output, &forward(&initial, &SENSITIVE));
            let report = warm.tail.last().unwrap();
            if probes(arithmetic) {
                assert_eq!(report.sensitive_keys, 1, "{report:?}");
                assert_eq!(report.closed_phase, Some(2), "{report:?}");
                assert!(warm.steps.iter().any(|key| key.powers()[0] == 1));
                assert!(!warm.steps.iter().any(|key| key.powers()[0] == 2));
            } else {
                assert_eq!(report.closed_phase, None, "{report:?}");
                assert!(warm.steps.iter().any(is_tail));
            }

            // Every tail guard retained by the caller: the tail closes at
            // its start without expanding any tail key.
            let warm = run(
                &program,
                &initial,
                &RETAINED,
                arithmetic,
                scheduler,
                Some(&cache),
            );
            same_result(&warm.output, &forward(&initial, &RETAINED));
            let report = warm.tail.last().unwrap();
            assert_eq!(report.mode, "closed", "{report:?}");
            assert_eq!(report.closed_phase, Some(1), "{report:?}");
            assert!(!warm.steps.iter().any(is_tail), "{:?}", warm.steps);

            // Inputs inside the tail only: closes before the first phase.
            let warm = run(
                &program,
                &only_tail,
                &RETAINED,
                arithmetic,
                scheduler,
                Some(&cache),
            );
            same_result(&warm.output, &forward(&only_tail, &RETAINED));
            assert_eq!(warm.tail.last().unwrap().closed_phase, Some(0));
            assert!(warm.steps.is_empty());
        }
    }
}

/// A table that does not cover the request (populated by another request,
/// or a run whose closing failed) holds back no rewrite: a tabulated key
/// the forward pass erased is checked over the table's steps (below it only
/// keys the forward pass did not expand), exactly as without the table.
#[test]
fn tail_table_of_another_request_rewrites_like_no_table() {
    if super::forced_by_environment() {
        return;
    }
    let program = Program::new();
    let (a, b) = (program.request_a(), program.request_b());
    for arithmetic in arithmetics()
        .into_iter()
        .filter(|&arithmetic| recorded(arithmetic))
    {
        for scheduler in schedulers(true) {
            // A fresh table holding `request_a`'s tail only (runs populate
            // it).
            let table_of_a = || {
                let cache = OrderedStepCache::in_memory(binding(&program), Default::default());
                let cold = run(&program, &a, &SAFE, arithmetic, scheduler, Some(&cache));
                assert!(cold.tail.last().unwrap().added_entries >= 20);
                (cache, cold)
            };
            for conditions in [&SAFE[..], &SENSITIVE[..]] {
                let alone = run(&program, &b, conditions, arithmetic, scheduler, None);
                let (cache, _) = table_of_a();
                let foreign = run(
                    &program,
                    &b,
                    conditions,
                    arithmetic,
                    scheduler,
                    Some(&cache),
                );
                same_result(&foreign.output, &alone.output);
                let (expected, report) = (
                    alone.tail.last().expect("a tail report"),
                    foreign.tail.last().expect("a tail report"),
                );
                // `[1,10]` and `[2,12]` are not tabulated: no closing; the
                // erased `[2,7]` is, with `d+2` and `d-6` (new to the tail)
                // in its closure set, but only below expanded keys.
                assert_eq!(report.closed_phase, None, "{report:?}");
                assert!(report.table_values > 0, "{report:?}");
                assert_eq!(expected.start_phase, Some(0), "{expected:?}");
                let mode = if conditions == SAFE {
                    "adjoint"
                } else {
                    "forward"
                };
                assert_eq!(expected.mode, mode, "{arithmetic} {expected:?}");
                assert_eq!(report.mode, mode, "{arithmetic} {report:?}");
                assert_eq!(report.sensitive_keys, expected.sensitive_keys, "{report:?}");
                assert_eq!(foreign.ops, alone.ops, "{arithmetic} {report:?}");
            }
            // The table's own request, warm, when probes cannot close it
            // (cross-checked arithmetics): rewritten like the cold run.
            if !probes(arithmetic) {
                let (cache, cold) = table_of_a();
                let warm = run(&program, &a, &SAFE, arithmetic, scheduler, Some(&cache));
                let report = warm.tail.last().unwrap();
                assert_eq!(report.closed_phase, None, "{report:?}");
                assert_eq!(report.mode, "adjoint", "{arithmetic} {report:?}");
                assert_eq!(warm.ops, cold.ops, "{arithmetic} {report:?}");
            }
        }
    }
}

/// The safety half of the walk over the table's steps: a tabulated erased key
/// whose guard new to the tail lies below it, only through keys the forward
/// pass did not expand (absent from the run's log), still aborts the rewrite
/// exactly like the run without the table; with that guard retained by the
/// caller, the table of another request holds back no rewrite (only `d-6` of
/// the expanded `[2,0]` is new to the tail in the closure set).
#[test]
fn tail_table_walk_aborts_for_a_new_guard_below_unexpanded_keys() {
    if super::forced_by_environment() {
        return;
    }
    let program = Program::new();
    let c = program.inputs(&[([1, 12], "1"), ([1, 11], "1")]);
    let d = program.inputs(&[([2, 16], "1")]);
    let with_d9: [&str; 3] = ["d-2", "d+7", "d+9"];
    for arithmetic in arithmetics()
        .into_iter()
        .filter(|&arithmetic| recorded(arithmetic))
    {
        for scheduler in schedulers(true) {
            let table_of_d = || {
                let cache = OrderedStepCache::in_memory(binding(&program), Default::default());
                let cold = run(&program, &d, &SAFE, arithmetic, scheduler, Some(&cache));
                assert!(
                    cold.tail.last().unwrap().added_entries >= 4,
                    "{:?}",
                    cold.tail
                );
                cache
            };
            for (conditions, mode) in [(&SAFE[..], "forward"), (&with_d9[..], "adjoint")] {
                let alone = run(&program, &c, conditions, arithmetic, scheduler, None);
                let cache = table_of_d();
                let foreign = run(
                    &program,
                    &c,
                    conditions,
                    arithmetic,
                    scheduler,
                    Some(&cache),
                );
                same_result(&foreign.output, &alone.output);
                let (expected, report) = (
                    alone.tail.last().expect("a tail report"),
                    foreign.tail.last().expect("a tail report"),
                );
                assert_eq!(report.closed_phase, None, "{report:?}");
                assert!(report.table_values > 0, "{report:?}");
                assert_eq!(expected.mode, mode, "{arithmetic} {expected:?}");
                assert_eq!(report.mode, mode, "{arithmetic} {report:?}");
                assert_eq!(report.sensitive_keys, expected.sensitive_keys, "{report:?}");
                assert_eq!(foreign.ops, alone.ops, "{arithmetic} {report:?}");
            }
        }
    }
}

/// The table file: written by `save`, reused by a reopened cache, bound by
/// versioning only (a changed program digest does not invalidate it; a
/// changed identity field or a corrupt file is rejected loudly and the run
/// starts cold), never populated by read-only caches.
#[test]
fn tail_table_files_are_bound_by_version_and_identity() {
    let program = Program::new();
    let initial = program.request_a();
    let arithmetic = every_arithmetic()[3];
    let scheduler = schedulers(true)[0];
    let scratch = Scratch::new("tail-table");
    let directory = scratch.0.join("cache");
    let open = |directory: &Path, binding: OrderedStepCacheBinding, populate: bool| {
        OrderedStepCache::open(
            directory,
            binding,
            OrderedStepCacheOptions {
                populate,
                ..Default::default()
            },
        )
        .unwrap()
    };
    let cache = open(&directory, binding(&program), true);
    let cold = run(
        &program,
        &initial,
        &SAFE,
        arithmetic,
        scheduler,
        Some(&cache),
    );
    let report = cache.save().unwrap();
    assert!(
        report.tail_entries >= 20 && report.tail_bytes > 0,
        "{report:?}"
    );
    let file = directory.join(super::table::TAIL_FILE);
    let bytes = std::fs::read(&file).unwrap();

    // Reopened read-only: the tail closes from the file, nothing is added.
    let cache = open(&directory, binding(&program), false);
    let warm = run(
        &program,
        &initial,
        &SAFE,
        arithmetic,
        scheduler,
        Some(&cache),
    );
    same_result(&warm.output, &cold.output);
    assert_eq!(warm.tail.last().unwrap().mode, "closed-probes");
    let statistics = cache.tail_statistics().unwrap();
    assert_eq!(statistics.loaded_entries, report.tail_entries);
    assert_eq!(statistics.rejected, None);
    // Saving the same entries again writes nothing.
    let cache = open(&directory, binding(&program), true);
    run(
        &program,
        &initial,
        &SAFE,
        arithmetic,
        scheduler,
        Some(&cache),
    );
    assert_eq!(cache.save().unwrap().tail_entries, 0);
    assert_eq!(std::fs::read(&file).unwrap(), bytes);

    // Copies of the file in fresh cache directories.
    let reopen = |label: &str, binding: OrderedStepCacheBinding, contents: &[u8]| {
        let other = scratch.0.join(label);
        std::fs::create_dir_all(&other).unwrap();
        std::fs::write(other.join(super::table::TAIL_FILE), contents).unwrap();
        let cache = open(&other, binding, true);
        let warm = run(
            &program,
            &initial,
            &SAFE,
            arithmetic,
            scheduler,
            Some(&cache),
        );
        same_result(&warm.output, &cold.output);
        (warm, cache.tail_statistics().unwrap())
    };
    // The same version and identity in another directory: reused.
    let (warm, statistics) = reopen("copy", binding(&program), &bytes);
    assert_eq!(warm.tail.last().unwrap().mode, "closed-probes");
    assert_eq!(statistics.rejected, None);
    // Another identity field: rejected, cold.
    let mut family = binding(&program);
    family.family_fingerprint = "another family".into();
    let (warm, statistics) = reopen("family", family, &bytes);
    assert_eq!(warm.tail.last().unwrap().mode, "adjoint");
    assert!(statistics.rejected.unwrap().contains("identity"));
    assert_eq!(statistics.loaded_entries, 0);
    // A corrupt payload: rejected, cold.
    let mut corrupt = bytes.clone();
    let last = corrupt.len() - 1;
    corrupt[last] ^= 1;
    let (warm, statistics) = reopen("corrupt", binding(&program), &corrupt);
    assert_eq!(warm.tail.last().unwrap().mode, "adjoint");
    assert!(statistics.rejected.unwrap().contains("digest"));
}
