//! lane bytebound: `RUSTRED_P_CACHE_BUDGET_GB` on whole p reductions. Its own
//! test binary: the budget is process-wide, so the tests here run one at a time
//! and no other test of the binary can see a budget.
//!
//! Under any budget the result terms and the recorded program (operation
//! count, peak slots, polynomials, constants, trace digest) are those of the
//! unbounded run; tiny budgets evict, reset and bypass (the counts are
//! checked to be nonzero), at different points for different budgets.
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use rustred::algebra::{Coefficient, CoefficientContext, ExactAlgebraLimits};
use rustred::family::IntegralKey;
use rustred::propagator::PresentationId;
use rustred::propagator::cache_budget::{self, BudgetStats};
use rustred::propagator::library::{PLibrary, PReductionRequest, PTerm, PUncoveredPolicy};
use rustred::propagator::testing::{
    REFERENCE_FLIPPED, REFERENCE_SHIFTED, REFERENCE_SWAPPED, REFERENCE_TOP,
    one_loop_reference_library,
};
use rustred::reduction::{
    ModularCommon, OrderedSchedulerOptions, OrderedWeightArithmetic, ReductionLimits,
    SampledOptions,
};

fn serial() -> MutexGuard<'static, ()> {
    static SERIAL: Mutex<()> = Mutex::new(());
    SERIAL.lock().unwrap_or_else(PoisonError::into_inner)
}

fn key(powers: &[i64]) -> IntegralKey {
    IntegralKey::try_new(powers.iter().copied()).unwrap()
}

fn fraction(context: &CoefficientContext, numerator: i64, denominator: i64) -> Coefficient {
    let limits = ExactAlgebraLimits::default();
    context
        .try_div(
            &context.integer(numerator),
            &context.integer(denominator),
            limits,
        )
        .unwrap()
}

fn binomial(n: i64, k: i64) -> i64 {
    (1..=k).fold(1, |value, j| value * (n + 1 - j) / j)
}

const ROUTINGS: [(PresentationId, usize, usize, i64); 4] = [
    (REFERENCE_TOP, 0, 1, 1),
    (REFERENCE_SWAPPED, 1, 0, 1),
    (REFERENCE_FLIPPED, 0, 1, -1),
    (REFERENCE_SHIFTED, 1, 0, -1),
];

/// `(k.q)_M^r / D_k^a D_s^b` over the slots (as `tests/p_library/plib.rs`).
fn lowered(
    context: &CoefficientContext,
    (presentation, k_slot, s_slot, sign): (PresentationId, usize, usize, i64),
    a: i64,
    b: i64,
    r: i64,
) -> Vec<(PresentationId, IntegralKey, Coefficient)> {
    let mut inputs = Vec::new();
    for i in 0..=r {
        for j in 0..=r - i {
            let l = r - i - j;
            let multinomial = binomial(r, i) * binomial(r - i, j);
            let parity = if (j + l) % 2 == 0 { 1 } else { -1 };
            let orientation = if r % 2 == 0 { 1 } else { sign };
            let coefficient = fraction(context, orientation * parity * multinomial, 1 << r);
            let mut powers = [0; 2];
            powers[k_slot] = a - i;
            powers[s_slot] = b - j;
            inputs.push((presentation, key(&powers), coefficient));
        }
    }
    inputs
}

fn inputs(library: &PLibrary, large: bool) -> Vec<(PresentationId, IntegralKey, Coefficient)> {
    let context = library.coefficient_context();
    let mut inputs = Vec::new();
    for (n, routing) in ROUTINGS.into_iter().enumerate() {
        if large {
            for a in 1..=4 {
                for b in 1..=4 {
                    inputs.extend(lowered(context, routing, a, b, 6 + (a + b + n as i64) % 3));
                }
            }
        } else {
            for (a, b, r) in [(2, 3, 5), (4, 2, 6), (3, 3, 7)] {
                inputs.extend(lowered(context, routing, a + n as i64 % 2, b, r));
            }
        }
    }
    inputs
}

fn stats_file() -> PathBuf {
    std::env::temp_dir().join(format!(
        "rustred-p-cache-budget-{}.jsonl",
        std::process::id()
    ))
}

/// What must not change: the terms and the recorded program.
#[derive(Debug, PartialEq)]
struct Outcome {
    terms: Vec<PTerm>,
    recorded: Vec<(String, String)>,
}

fn field(line: &str, name: &str) -> String {
    let at = line
        .find(&format!("\"{name}\":"))
        .unwrap_or_else(|| panic!("{name} in {line}"));
    line[at + name.len() + 3..]
        .split([',', '}'])
        .next()
        .unwrap()
        .trim_matches('"')
        .to_owned()
}

/// One reduction on one thread with the sampled modular arithmetic (the
/// recorded program of production) under `budget`; the budget's statistics
/// of this run.
fn run(
    library: &Arc<PLibrary>,
    budget: Option<u64>,
    tuning: (Option<u64>, Option<u64>),
    large: bool,
) -> (Outcome, BudgetStats) {
    let path = stats_file();
    let _ = std::fs::remove_file(&path);
    // SAFETY: the tests of this binary run one at a time (`serial`) and no
    // other thread reads the environment meanwhile.
    unsafe { std::env::set_var("RUSTRED_ORDERED_MODULAR_STATS", &path) };
    let before = cache_budget::budget_stats();
    cache_budget::set_tuning(tuning.0, tuning.1);
    cache_budget::set_budget(budget);
    let request = PReductionRequest {
        inputs: inputs(library, large),
        arithmetic: OrderedWeightArithmetic::ModularSampled(
            ModularCommon {
                threads: 1,
                ..Default::default()
            },
            SampledOptions::default(),
        ),
        scheduler: OrderedSchedulerOptions::default(),
        step_cache: None,
        uncovered: PUncoveredPolicy::Reject,
        onto_masters: false,
        limits: ReductionLimits::default(),
        inspector: None,
    };
    let reduced = library.reduce(request);
    cache_budget::set_budget(None);
    cache_budget::set_tuning(None, None);
    let after = cache_budget::budget_stats();
    let reduced = reduced.unwrap();
    let stats = std::fs::read_to_string(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    let line = stats
        .lines()
        .find(|line| line.contains("\"stage\":\"recorded\""))
        .expect("a recorded-stage line");
    let recorded = [
        "ops",
        "peak_slots",
        "polynomials",
        "constants",
        "trace_digest",
    ]
    .into_iter()
    .map(|name| (name.to_owned(), field(line, name)))
    .collect();
    let delta = BudgetStats {
        budget,
        sweeps: after.sweeps - before.sweeps,
        evicted_entries: after.evicted_entries - before.evicted_entries,
        evicted_bytes: after.evicted_bytes - before.evicted_bytes,
        skipped_pinned: after.skipped_pinned - before.skipped_pinned,
        step_resets: after.step_resets - before.step_resets,
        epoch_resets: after.epoch_resets - before.epoch_resets,
        phase_resets: after.phase_resets - before.phase_resets,
        bypasses: after.bypasses - before.bypasses,
        refused: after.refused - before.refused,
        reinsertions: after.reinsertions - before.reinsertions,
        bypassed_steps: after.bypassed_steps - before.bypassed_steps,
        max_key_charge: if after.max_key_charge > before.max_key_charge {
            after.max_key_charge
        } else {
            0
        },
    };
    (
        Outcome {
            terms: reduced.terms,
            recorded,
        },
        delta,
    )
}

/// Unset, a generous budget and tiny budgets: the same terms and the same
/// recorded program; the tiny budgets refuse or evict.
#[test]
fn results_and_operation_counts_are_identical_under_any_budget() {
    let _serial = serial();
    let library = one_loop_reference_library().unwrap();
    let (reference, unset) = run(&library, None, (None, None), false);
    assert_eq!(
        unset,
        BudgetStats::default(),
        "unset: the budget does nothing"
    );
    assert!(!reference.terms.is_empty());
    for budget in [1 << 40, 0, 1, 700, 3_000, 20_000, 150_000] {
        let (outcome, stats) = run(&library, Some(budget), (None, None), false);
        assert_eq!(outcome, reference, "budget {budget} B");
        if budget < 1 << 30 {
            assert!(
                stats.refused + stats.evicted_entries + stats.bypassed_steps > 0,
                "budget {budget} B acts: {stats:?}"
            );
        }
        println!("budget {budget} B: {stats:?}");
    }
}

/// The step memo under tiny shares: reset at key boundaries (rate limit of
/// 1, 2 or 3 keys), bypassed after its fifth reset in a phase, so light
/// (memoised) and direct steps mix within phases; same terms and program.
#[test]
fn step_memo_resets_and_bypasses_keep_the_program() {
    let _serial = serial();
    let library = one_loop_reference_library().unwrap();
    let (reference, _) = run(&library, None, (None, None), true);
    let (mut bypassed, mut resets) = (0, 0);
    for (budget, window) in [
        (1_500, 1),
        (3_000, 1),
        (6_000, 2),
        (12_000, 1),
        (40_000, 3),
        (100_000, 1),
    ] {
        let (outcome, stats) = run(&library, Some(budget), (Some(0), Some(window)), true);
        assert_eq!(outcome, reference, "budget {budget} B, window {window}");
        assert!(stats.phase_resets > 0);
        resets += stats.step_resets;
        bypassed += stats.bypasses;
        println!("budget {budget} B window {window}: {stats:?}");
    }
    assert!(resets > 0, "the step memo was reset at key boundaries");
    assert!(bypassed > 0, "some phase ran bypassed after its resets");
}

/// The terms of one reduction of the large inputs on `threads` scheduler
/// threads (and as many arithmetic threads), without modular statistics.
fn terms(library: &Arc<PLibrary>, threads: usize) -> Vec<PTerm> {
    let request = PReductionRequest {
        inputs: inputs(library, true),
        arithmetic: OrderedWeightArithmetic::ModularSampled(
            ModularCommon {
                threads,
                ..Default::default()
            },
            SampledOptions::default(),
        ),
        scheduler: OrderedSchedulerOptions {
            threads,
            ..Default::default()
        },
        step_cache: None,
        uncovered: PUncoveredPolicy::Reject,
        onto_masters: false,
        limits: ReductionLimits::default(),
        inspector: None,
    };
    library.reduce(request).unwrap().terms
}

/// Concurrency under a tiny budget: two one-thread requests and one request
/// on 8 threads on the same library at the same time, step-memo resets at
/// every key (each thread its own memo generation): the terms of each are
/// the unbounded ones.
#[test]
fn concurrent_requests_under_a_tiny_budget_keep_their_terms() {
    let _serial = serial();
    // SAFETY: as in `run`.
    unsafe { std::env::remove_var("RUSTRED_ORDERED_MODULAR_STATS") };
    let library = one_loop_reference_library().unwrap();
    let one = terms(&library, 1);
    let eight = terms(&library, 8);
    assert_eq!(one, eight);
    cache_budget::set_tuning(Some(0), Some(1));
    cache_budget::set_budget(Some(3_000));
    let before = cache_budget::budget_stats();
    let results: Vec<Vec<PTerm>> = std::thread::scope(|scope| {
        let handles: Vec<_> = [1, 1, 8]
            .into_iter()
            .map(|threads| {
                let library = Arc::clone(&library);
                scope.spawn(move || terms(&library, threads))
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let after = cache_budget::budget_stats();
    cache_budget::set_budget(None);
    cache_budget::set_tuning(None, None);
    for result in &results {
        assert_eq!(*result, one);
    }
    assert!(after.step_resets > before.step_resets, "{after:?}");
}
