//! lane bytebound (review rv1 F1): `RUSTRED_P_CACHE_BUDGET_GB` with several
//! one-thread requests at once in one process (beta's `--jobs N` shape) on an
//! assembled library whose lanes have kernel memos (SeriesParallel:
//! numerator tables and bubble ratios). Its own test binary: the budget is
//! process-wide.
//!
//! The documented bound (`cache_budget` module docs): with N one-thread
//! requests at once the covered bytes stay below B + max(0.05 B, floor) +
//! 2 N C, C the most one thread charges between two of its key boundaries.
//! Before the fix each thread capped only its own step memo at B/4 and could
//! not release tables pinned by another thread's step memo, so N requests
//! held about N B/4 of step memos on top of what the sweeps left.
use std::sync::{Arc, Barrier};

use rustred::algebra::Coefficient;
use rustred::family::IntegralKey;
use rustred::propagator::PresentationId;
use rustred::propagator::cache_budget;
use rustred::propagator::families::{P_FAMILIES_V1_JSON, PFamiliesFile};
use rustred::propagator::library::{PLibrary, PReductionRequest, PTerm, PUncoveredPolicy};
use rustred::propagator::presentation::Slot;
use rustred::reduction::{
    ModularCommon, OrderedSchedulerOptions, OrderedWeightArithmetic, ReductionLimits,
    SampledOptions,
};
use rustred_app::propagator::assemble::SplitMix;
use rustred_app::propagator::{
    AssembleOptions, AssembledLibrary, CustomOwnerOptions, CustomPayloads, assemble_library,
};

fn assemble(max_loops: u8) -> AssembledLibrary {
    let families: PFamiliesFile = serde_json::from_str(P_FAMILIES_V1_JSON).unwrap();
    assemble_library(
        &families,
        max_loops,
        CustomPayloads::Generate(CustomOwnerOptions::default()),
        &AssembleOptions {
            threads: 4,
            verify_descent: false,
        },
    )
    .unwrap()
}

/// `count` keys on the 2L tops: a random subsector of the lines with up to
/// 3 dots, and negative powers on the other slots up to a total rank of 8.
fn inputs(
    assembled: &AssembledLibrary,
    seed: u64,
    count: usize,
) -> Vec<(PresentationId, IntegralKey, Coefficient)> {
    let library = &assembled.library;
    let context = library.coefficient_context();
    let tops = assembled.tops_of(2);
    let mut rng = SplitMix::new(seed);
    (0..count)
        .map(|_| {
            let top = tops[rng.below(tops.len() as u64) as usize];
            let p = library.presentation(top);
            let lines: Vec<usize> = (0..p.slots.len())
                .filter(|&s| matches!(p.slots[s], Slot::Line(_)))
                .collect();
            let mut powers = vec![0_i64; p.slots.len()];
            let mut kept = Vec::new();
            for &slot in &lines {
                if rng.below(5) != 0 {
                    powers[slot] = 1;
                    kept.push(slot);
                }
            }
            if kept.is_empty() {
                powers[lines[0]] = 1;
                kept.push(lines[0]);
            }
            for _ in 0..rng.below(4) {
                powers[kept[rng.below(kept.len() as u64) as usize]] += 1;
            }
            let numerators: Vec<usize> = (0..powers.len()).filter(|&s| powers[s] == 0).collect();
            if !numerators.is_empty() {
                for _ in 0..4 + rng.below(5) {
                    powers[numerators[rng.below(numerators.len() as u64) as usize]] -= 1;
                }
            }
            let coefficient = context.integer(1 + rng.below(7) as i64);
            (top, IntegralKey::try_new(powers).unwrap(), coefficient)
        })
        .collect()
}

/// One one-thread request (ModularSampled on one thread, one scheduler
/// thread: the production arithmetic of beta's jobs).
fn reduce(
    library: &PLibrary,
    inputs: Vec<(PresentationId, IntegralKey, Coefficient)>,
) -> Vec<PTerm> {
    let request = PReductionRequest {
        inputs,
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
    library.reduce(request).unwrap().terms
}

/// `N` requests at once, each on its own std thread.
fn concurrently(
    library: &Arc<PLibrary>,
    sets: &[Vec<(PresentationId, IntegralKey, Coefficient)>],
) -> Vec<Vec<PTerm>> {
    let start = Barrier::new(sets.len());
    std::thread::scope(|scope| {
        let handles: Vec<_> = sets
            .iter()
            .map(|set| {
                let (library, start) = (Arc::clone(library), &start);
                scope.spawn(move || {
                    start.wait();
                    reduce(&library, set.clone())
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    })
}

/// What an arm measured.
struct Arm {
    peak: u64,
    bound: u64,
    key: u64,
    unbounded_peak: u64,
    evicted: u64,
    resets: u64,
}

/// `n` one-thread requests of `keys` keys each at once under `budget`, then
/// unbounded (at once and one at a time): identical terms; the covered peak
/// against the documented bound.
fn arm(assembled: &AssembledLibrary, n: usize, budget: u64, keys: usize, seed: u64) -> Arm {
    let library = &assembled.library;
    let sets: Vec<_> = (0..n as u64)
        .map(|i| inputs(assembled, seed + i, keys))
        .collect();
    // Under the budget first: the step memo is never bypassed for its share
    // (floor 0), the key window is production's (4096 keys: a thread's own
    // share resets are rare) and the re-arm margin is 0.05 B (floor 0), so
    // the bound is tight enough to see n B/4.
    cache_budget::set_tuning(Some(0), None);
    cache_budget::set_rearm_floor(Some(0));
    cache_budget::set_budget(Some(budget));
    cache_budget::reset_peaks();
    let before = cache_budget::budget_stats();
    let bounded = concurrently(library, &sets);
    let peak = cache_budget::stats().peak_covered;
    let after = cache_budget::budget_stats();
    cache_budget::set_budget(None);
    cache_budget::set_rearm_floor(None);
    cache_budget::set_tuning(None, None);
    let key = after.max_key_charge;
    let bound = budget + budget / 20 + 2 * n as u64 * key;
    cache_budget::reset_peaks();
    let unbounded = concurrently(library, &sets);
    let unbounded_peak = cache_budget::stats().peak_covered;
    let alone: Vec<Vec<PTerm>> = sets
        .iter()
        .map(|set| reduce(library, set.clone()))
        .collect();
    println!(
        "N {n} keys {keys} budget {budget} B: covered peak {peak} B, bound {bound} B (one key {key} B), \
         unbounded peak {unbounded_peak} B; sweeps {} evicted {} step resets {} (epoch {}) bypasses {} \
         pinned skipped {}",
        after.sweeps - before.sweeps,
        after.evicted_entries - before.evicted_entries,
        after.step_resets - before.step_resets,
        after.epoch_resets - before.epoch_resets,
        after.bypasses - before.bypasses,
        after.skipped_pinned - before.skipped_pinned,
    );
    for (i, terms) in bounded.iter().enumerate() {
        assert!(!terms.is_empty());
        assert_eq!(
            *terms, unbounded[i],
            "request {i}: bounded vs unbounded at once"
        );
        assert_eq!(*terms, alone[i], "request {i}: bounded vs alone");
    }
    assert!(
        unbounded_peak > 2 * budget,
        "the requests need more than the budget: {unbounded_peak} B"
    );
    Arm {
        peak,
        bound,
        key,
        unbounded_peak,
        evicted: after.evicted_entries - before.evicted_entries,
        resets: after.step_resets - before.step_resets,
    }
}

/// Twelve requests at 6 MB (the step memos of all threads share B/4: about
/// 12 B/4 before the fix), and six at 0.6 MB (the kernel memos are swept,
/// tables pinned by other threads' step memos released by the epoch).
#[test]
fn concurrent_one_thread_requests_stay_within_the_documented_bound() {
    let assembled = assemble(2);
    let mut arms = vec![
        (12, 6_000_000, 1500, 0x5eed_0000),
        (6, 600_000, 400, 0x5eed_1000),
    ];
    // Calibration: P_CACHE_BUDGET_TEST_ARM=n,budget,keys replaces the arms.
    if let Ok(arm) = std::env::var("P_CACHE_BUDGET_TEST_ARM") {
        let v: Vec<u64> = arm.split(',').map(|x| x.parse().unwrap()).collect();
        arms = vec![(v[0] as usize, v[1], v[2] as usize, 0x5eed_2000)];
    }
    for (n, budget, keys, seed) in arms {
        let arm = arm(&assembled, n, budget, keys, seed);
        assert!(arm.resets > 0);
        if budget < 1_000_000 {
            assert!(arm.evicted > 0, "the kernel memos were swept");
        }
        assert!(
            arm.peak <= arm.bound,
            "N {n}: covered peak {} B over the bound {} B (budget {budget}, one key {}, unbounded {})",
            arm.peak,
            arm.bound,
            arm.key,
            arm.unbounded_peak
        );
    }
}
