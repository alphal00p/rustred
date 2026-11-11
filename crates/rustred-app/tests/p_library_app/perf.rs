//! Step cache and cancellation of the p path (owner: M3-T19 rr-perf).
//!
//! Two benchmark cells (vakint `tests/fixtures/benchmark_p_integrals.json`
//! L2_d1_r6 and L2_d1_r8) on the assembled scope 1-2 program: an Exact
//! request on several threads records the steps every arithmetic replays
//! (R1: the adjoint tail's completion steps included), and a refused resource
//! checkpoint (rustred #20's cancellation) stops `PLibrary::reduce` with the
//! refusal in the error's cause chain.
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use rustred::algebra::Coefficient;
use rustred::family::IntegralKey;
use rustred::propagator::PresentationId;
use rustred::propagator::error::PError;
use rustred::propagator::library::{PLibrary, PReduction, PReductionRequest, PUncoveredPolicy};
use rustred::reduction::{
    ModularCommon, OrderedProfile, OrderedSchedulerOptions, OrderedStepCache,
    OrderedStepCacheOptions, OrderedWeightArithmetic, PolynomialOptions, ReductionLimits,
    SampledOptions,
};
use symbolica::domains::rational::Rational;
use symbolica::utils::resources::{ResourceBudget, ResourceLimitError, ResourceLimits};

use super::package::{Scratch, scope_two};

type Inputs = Vec<(PresentationId, IntegralKey, Coefficient)>;

/// Scheduler and modular threads of every request (vakint's `n_cores`).
const THREADS: usize = 4;
/// The tail table file of a step-cache directory.
const TAIL_FILE: &str = "ordered-tail.rrtail";
const CANCELLED: &str = "request cancelled";

/// `P2L(2,1,1,1,1)` times `(k1.p)^2 (k2.p)^2 (k1.k2)^k1k2`, lowered as vakint
/// lowers it: every scalar product is the presentation's affine form in its
/// slots, and a slot factor lowers that slot's power.
fn benchmark_cell(library: &PLibrary, k1k2: usize) -> Inputs {
    let top = library.top_presentation("P2L").unwrap();
    let presentation = library.presentation(top);
    let arity = presentation.slots.len();
    let (k1, k2, q) = ([1, 0, 0], [0, 1, 0], [0, 0, 1]);
    let mut dots = vec![(k1, q), (k1, q), (k2, q), (k2, q)];
    dots.extend(std::iter::repeat_n((k1, k2), k1k2));
    let mut polynomial = BTreeMap::from([(vec![0_i64; arity], Rational::one())]);
    for (a, b) in dots {
        let form = presentation.express_scalar_product(&a, &b).unwrap();
        let mut product = BTreeMap::<Vec<i64>, Rational>::new();
        for (monomial, value) in &polynomial {
            let mut add = |monomial: Vec<i64>, factor: &Rational| {
                *product.entry(monomial).or_insert_with(Rational::zero) += &(value * factor);
            };
            add(monomial.clone(), &form.constant);
            for (slot, factor) in &form.terms {
                let mut shifted = monomial.clone();
                shifted[usize::from(*slot)] += 1;
                add(shifted, factor);
            }
        }
        polynomial = product;
    }
    let context = library.coefficient_context();
    let limits = ReductionLimits::default().exact_algebra;
    let integer = |value: &symbolica::domains::integer::Integer| {
        context.integer(i64::try_from(value.clone()).unwrap())
    };
    let powers = [2, 1, 1, 1, 1];
    (polynomial.into_iter())
        .filter(|(_, value)| !value.is_zero())
        .map(|(monomial, value)| {
            let key =
                (0..arity).map(|slot| powers.get(slot).copied().unwrap_or(0) - monomial[slot]);
            let coefficient = context
                .try_div(
                    &integer(value.numerator_ref()),
                    &integer(value.denominator_ref()),
                    limits,
                )
                .unwrap();
            (top, IntegralKey::try_new(key).unwrap(), coefficient)
        })
        .collect()
}

fn arithmetics() -> [OrderedWeightArithmetic; 3] {
    let common = ModularCommon {
        threads: THREADS,
        ..ModularCommon::default()
    };
    [
        OrderedWeightArithmetic::Exact,
        OrderedWeightArithmetic::ModularPolynomial(common, PolynomialOptions::default()),
        OrderedWeightArithmetic::ModularSampled(common, SampledOptions::default()),
    ]
}

fn reduce(
    library: &PLibrary,
    inputs: &Inputs,
    arithmetic: OrderedWeightArithmetic,
    step_cache: Option<Arc<OrderedStepCache>>,
) -> Result<PReduction, PError> {
    reduce_with(
        library,
        inputs,
        arithmetic,
        step_cache,
        OrderedProfile::Auto,
    )
}

/// [`reduce`] under `profile` (`Generic` keeps the adjoint tail and its
/// table, which the p profile turns off since M5 batch 2).
fn reduce_with(
    library: &PLibrary,
    inputs: &Inputs,
    arithmetic: OrderedWeightArithmetic,
    step_cache: Option<Arc<OrderedStepCache>>,
    profile: OrderedProfile,
) -> Result<PReduction, PError> {
    library.reduce(PReductionRequest {
        inputs: inputs.clone(),
        arithmetic,
        scheduler: OrderedSchedulerOptions {
            profile,
            ..OrderedSchedulerOptions::with_threads(THREADS)
        },
        step_cache,
        uncovered: PUncoveredPolicy::Reject,
        onto_masters: true,
        limits: ReductionLimits::default(),
        inspector: None,
    })
}

/// The cache in `directory`, read-write (`populate`) or read-only.
fn open(library: &PLibrary, directory: &std::path::Path, populate: bool) -> Arc<OrderedStepCache> {
    let options = OrderedStepCacheOptions {
        populate,
        ..OrderedStepCacheOptions::default()
    };
    OrderedStepCache::open(directory, library.step_cache_binding().unwrap(), options).unwrap()
}

/// R1 (M2-V6: 9 misses): an Exact request on several threads used to skip the
/// adjoint tail, whose completion steps the modular replays then missed. The
/// exact-prepared cache now replays read-only without a miss under the three
/// arithmetics, closing the tail from its table; without the table every
/// record is a step a modular replay takes. The tail runs under the Generic
/// profile (the p profile has no tail since M5 batch 2).
#[test]
fn an_exact_prepared_step_cache_replays_without_a_miss_under_every_arithmetic() {
    let library = &scope_two().library;
    let scratch = Scratch::new("perf-r1");
    for (cell, k1k2) in [("L2_d1_r6", 1), ("L2_d1_r8", 2)] {
        let inputs = benchmark_cell(library, k1k2);
        let off = reduce(library, &inputs, OrderedWeightArithmetic::Exact, None).unwrap();
        let directory = scratch.join(cell);
        let cache = open(library, &directory, true);
        let prepared = reduce_with(
            library,
            &inputs,
            OrderedWeightArithmetic::Exact,
            Some(cache.clone()),
            OrderedProfile::Generic,
        );
        assert_eq!(prepared.unwrap().terms, off.terms, "{cell}");
        cache.save().unwrap();
        let records = cache.statistics().records;
        for table in [true, false] {
            if !table {
                std::fs::remove_file(directory.join(TAIL_FILE)).unwrap();
            }
            for arithmetic in arithmetics() {
                let cache = open(library, &directory, false);
                let replay = reduce_with(
                    library,
                    &inputs,
                    arithmetic,
                    Some(cache),
                    OrderedProfile::Generic,
                )
                .unwrap();
                let progress = replay.statistics.progress;
                let label = format!("{cell} {arithmetic} tail table {table}");
                assert_eq!(replay.terms, off.terms, "{label}");
                assert_eq!(replay.nonzero_conditions, off.nonzero_conditions, "{label}");
                assert_eq!(progress.step_cache_misses, 0, "{label}");
                if !table && arithmetic != OrderedWeightArithmetic::Exact {
                    assert_eq!(progress.step_cache_hits, records, "{label}");
                }
            }
        }
    }
}

/// A budget whose memory monitor counts checkpoints and refuses from the
/// `stop_at`-th on, as a cancelled request does.
fn stopping(stop_at: usize, seen: Arc<AtomicUsize>) -> ResourceBudget {
    let limits = ResourceLimits {
        max_process_rss_bytes: Some(u64::MAX),
        max_elapsed: None,
    };
    ResourceBudget::with_memory_monitor(limits, move || {
        if seen.fetch_add(1, Ordering::SeqCst) + 1 >= stop_at {
            Err(CANCELLED.to_owned())
        } else {
            Ok(0)
        }
    })
}

fn is_cancellation(error: &PError) -> bool {
    let mut cause: Option<&(dyn std::error::Error + 'static)> = Some(error);
    while let Some(error) = cause {
        if let Some(ResourceLimitError::MonitorUnavailable { message, .. }) = error.downcast_ref() {
            return message == CANCELLED;
        }
        cause = error.source();
    }
    false
}

/// rustred #20: under every arithmetic, a refused checkpoint at the first
/// step or halfway through the request stops `PLibrary::reduce` with the
/// refusal in the cause chain; a budget that never refuses changes nothing.
#[test]
fn a_refused_checkpoint_stops_a_p_reduction_under_every_arithmetic() {
    let library = &scope_two().library;
    let inputs = benchmark_cell(library, 2);
    let off = reduce(library, &inputs, OrderedWeightArithmetic::Exact, None).unwrap();
    for arithmetic in arithmetics() {
        let seen = Arc::new(AtomicUsize::new(0));
        let full = stopping(usize::MAX, Arc::clone(&seen))
            .run(|| reduce(library, &inputs, arithmetic, None))
            .unwrap();
        assert_eq!(full.terms, off.terms, "{arithmetic}");
        let checkpoints = seen.load(Ordering::SeqCst);
        assert!(
            checkpoints > 1000,
            "{arithmetic}: {checkpoints} checkpoints"
        );
        for stop_at in [1, checkpoints / 2] {
            let seen = Arc::new(AtomicUsize::new(0));
            let error = stopping(stop_at, seen)
                .run(|| reduce(library, &inputs, arithmetic, None))
                .unwrap_err();
            assert!(
                is_cancellation(&error),
                "{arithmetic} at {stop_at}: {error}"
            );
        }
    }
}
