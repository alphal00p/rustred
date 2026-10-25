use super::*;
use crate::{
    CandidateOwnerBundle, FamilyCandidatesRequest, family_candidates,
    load_generated_candidate_owners,
};
use rustred::algebra::CoefficientContext;
use rustred::family::AffineDenominator;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

pub(super) struct Scratch(pub(super) PathBuf);
impl Scratch {
    pub(super) fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "rustred-application-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn qualified(family: &IntegralFamily, power: i64) -> SavedApplicationIntegral {
    SavedApplicationIntegral {
        family_fingerprint: family.fingerprint().to_owned(),
        integral: IntegralKey::try_new([power]).unwrap(),
    }
}

fn maps_fixture() -> TerminalMaps {
    let context = CoefficientContext::try_new(["d"]).unwrap();
    let family = Arc::new(
        IntegralFamily::new(
            "native-application-codec",
            vec!["q".into()],
            vec![],
            context.clone(),
            context.parameter("d").unwrap(),
            vec![AffineDenominator::new(
                context.integer(-1),
                vec![context.one()],
            )],
            vec![],
            vec![context.zero()],
        )
        .unwrap(),
    );
    let guard = context
        .try_sub(
            &context.parameter("d").unwrap(),
            &context.integer(4),
            Default::default(),
        )
        .unwrap();
    let coefficient = context
        .try_div(&context.parameter("d").unwrap(), &guard, Default::default())
        .unwrap();
    TerminalMaps {
        families: BTreeMap::from([(family.fingerprint().to_owned(), family.clone())]),
        rows: BTreeMap::from([
            (
                qualified(&family, 1),
                TerminalRow {
                    terms: BTreeMap::from([(qualified(&family, 1), context.one())]),
                    conditions: Arc::new(vec![guard.clone()]),
                },
            ),
            (
                qualified(&family, 2),
                TerminalRow {
                    terms: BTreeMap::from([(qualified(&family, 1), coefficient)]),
                    conditions: Arc::new(vec![guard]),
                },
            ),
        ]),
        remaining: BTreeSet::from([qualified(&family, 1)]),
        layer: None,
    }
}

#[test]
fn ordered_saved_application_load_option_and_strategy_switch_preserve_exact_results() {
    use rustred::reduction::ReductionStrategy;
    let scratch = Scratch::new();
    let original = prepared_fixture(&scratch.0.join("source"));
    let runtime = scratch.0.join("runtime");
    original.export_runtime(&runtime).unwrap();
    let mut application = load_saved_application::<1>(
        &runtime,
        SavedApplicationOptions {
            reduction_strategy: ReductionStrategy::Ordered,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(application.reduction_strategy(), ReductionStrategy::Ordered);
    let context = application.family().coefficient_context();
    let inputs = vec![
        (IntegralKey::try_new([4]).unwrap(), context.integer(3)),
        (IntegralKey::try_new([2]).unwrap(), context.integer(-2)),
    ];
    let ordered = application
        .reduce_linear_combination(inputs.clone())
        .unwrap();
    assert!(ordered.unresolved().is_empty());
    assert_eq!(ordered.statistics().work.cache_hits(), 0);
    assert!(ordered.statistics().work.rule_applications() > 0);
    assert_eq!(application.cache_statistics().cached_integrals(), 0);
    let again = application
        .reduce_linear_combination(inputs.clone())
        .unwrap();
    assert_eq!(ordered.terms(), again.terms());
    assert_eq!(
        ordered.statistics().work.rule_applications(),
        again.statistics().work.rule_applications()
    );
    application.set_reduction_strategy(ReductionStrategy::DepthFirst);
    let dfs = application
        .reduce_linear_combination(inputs.clone())
        .unwrap();
    assert_eq!(ordered.terms(), dfs.terms());
    assert_eq!(ordered.nonzero_conditions(), dfs.nonzero_conditions());
    let cached = application.cache_statistics().cached_integrals();
    assert!(cached > 0);
    application.set_reduction_strategy(ReductionStrategy::Ordered);
    let isolated = application.reduce_linear_combination(inputs).unwrap();
    assert_eq!(ordered.terms(), isolated.terms());
    assert_eq!(isolated.statistics().work.cache_hits(), 0);
    assert_eq!(application.cache_statistics().cached_integrals(), cached);
}

#[test]
fn ordered_step_cache_replays_routed_steps_across_processes_without_owner_preparation() {
    use rustred::reduction::{OrderedKeyStage, OrderedSchedulerOptions, ReductionStrategy};
    let scratch = Scratch::new();
    let original = prepared_fixture(&scratch.0.join("source"));
    let runtime = scratch.0.join("runtime");
    original.export_runtime(&runtime).unwrap();
    // Forward-pass properties: with the adjoint tail (on by default) the warm
    // session's tail table closes the single-terminal tail at phase 0, so its
    // phase snapshots differ from the forward run (see
    // `rustred::reduction::ordered_reduction::tail`).
    let load = || {
        load_saved_application::<1>(
            &runtime,
            SavedApplicationOptions {
                reduction_strategy: ReductionStrategy::Ordered,
                ordered_scheduler: OrderedSchedulerOptions {
                    adjoint_tail: false,
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .unwrap()
    };
    let context = original.family().coefficient_context().clone();
    let inputs = vec![
        (IntegralKey::try_new([4]).unwrap(), context.integer(3)),
        (IntegralKey::try_new([2]).unwrap(), context.integer(-2)),
    ];
    // Every pending identity and exact weight, terminals and guards, at
    // every phase boundary.
    let run = |application: &mut PreparedSavedApplication<1>| {
        let mut snapshots = Vec::new();
        let reduced = application
            .reduce_routed_linear_combination_with_ordered_inspector(inputs.clone(), &mut |event| {
                let mut lines = vec![format!(
                    "{:?} {} {}",
                    event.boundary, event.phase, event.progress.processed_keys
                )];
                for entry in event.frontier() {
                    let stage = match entry.stage {
                        OrderedKeyStage::Apply { owner_sector } => format!("{owner_sector:?}"),
                        other => format!("{other:?}"),
                    };
                    lines.push(format!(
                        "{stage} {:?} {}",
                        entry.integral.powers(),
                        canonical(entry.weight.unwrap())
                    ));
                }
                for (key, weight) in event.terminals() {
                    lines.push(format!("{:?} {}", key.powers(), canonical(weight.unwrap())));
                }
                lines.extend(event.nonzero_conditions().iter().map(canonical));
                snapshots.push(lines);
                Ok(())
            })
            .unwrap();
        (reduced, snapshots)
    };
    let mut application = load();
    let (expected, expected_snapshots) = run(&mut application);
    let directory = scratch.0.join("ordered-steps");
    let cache = application
        .open_ordered_step_cache(&directory, Default::default())
        .unwrap();
    application
        .set_ordered_step_cache(Some(cache.clone()))
        .unwrap();
    let (cold, cold_snapshots) = run(&mut application);
    assert_eq!(cold.terms(), expected.terms());
    assert_eq!(cold.nonzero_conditions(), expected.nonzero_conditions());
    assert_eq!(cold_snapshots, expected_snapshots);
    let recorded = cache.statistics();
    assert!(recorded.records > 0);
    assert_eq!(recorded.hits, 0);
    let saved = cache.save().unwrap();
    assert_eq!(saved.records, recorded.records);
    // The manifest's shard lines (payload digests) are independent of the
    // stamp; pin them to the step semantics version. The stamp names only
    // versions and identity fields, never the engine build.
    let manifest = std::fs::read_to_string(directory.join("ordered-step-cache.txt")).unwrap();
    assert!(manifest.starts_with(&format!(
        "schema {}\nsemantics {}\nadapter saved-application\n",
        rustred::reduction::ORDERED_STEP_CACHE_SCHEMA,
        rustred::reduction::ORDERED_STEP_SEMANTICS
    )));
    let shards: String = manifest
        .lines()
        .filter(|line| line.starts_with("shard "))
        .map(|line| format!("{line}\n"))
        .collect();
    assert_eq!(
        (
            rustred::reduction::ORDERED_STEP_SEMANTICS,
            blake3::hash(shards.as_bytes()).to_hex().as_str()
        ),
        (
            "rustred-ordered-steps-2026-10-31",
            "64a36123af5c9c1c836fcaf88b35fbe96371db90d77dfd8e86126dd4d0cec02e"
        ),
        "the recorded Route/Apply steps of the saved fixture changed: bump \
         ORDERED_STEP_SEMANTICS and update the golden digest"
    );
    // Another session loads the shards lazily and replays every step; the
    // owners' formulas are never prepared.
    let mut fresh = load();
    let warm_cache = fresh
        .open_ordered_step_cache(&directory, Default::default())
        .unwrap();
    fresh
        .set_ordered_step_cache(Some(warm_cache.clone()))
        .unwrap();
    let (warm, warm_snapshots) = run(&mut fresh);
    assert_eq!(warm.terms(), expected.terms());
    assert_eq!(warm.nonzero_conditions(), expected.nonzero_conditions());
    assert_eq!(warm_snapshots, expected_snapshots);
    let replayed = warm_cache.statistics();
    assert_eq!(replayed.hits as usize, recorded.records);
    assert_eq!((replayed.misses, replayed.inserts), (0, 0));
    assert_eq!(fresh.owner_preparation_statistics().prepared_owners, 0);
    // A cache of another residual policy is refused.
    let mut preserving = load_saved_application::<1>(
        &runtime,
        SavedApplicationOptions {
            reduction_strategy: ReductionStrategy::Ordered,
            unresolved_policy: rustred::solver::UnresolvedPolicy::Preserve,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(preserving.set_ordered_step_cache(Some(warm_cache)).is_err());
    assert!(
        preserving
            .open_ordered_step_cache(&directory, Default::default())
            .is_err()
    );

    fn canonical(coefficient: &Coefficient) -> String {
        use symbolica::atom::AtomCore;
        coefficient.to_expression().to_canonical_string()
    }
}

#[test]
fn ordered_phase_inspection_preserves_routed_results_and_stops_cleanly() {
    use rustred::reduction::{OrderedKeyStage, OrderedPhaseBoundary, ReductionStrategy};
    use rustred::solver::CandidateRoutedError;
    let scratch = Scratch::new();
    let mut application = prepared_fixture(&scratch.0.join("source"));
    application.set_reduction_strategy(ReductionStrategy::Ordered);
    let context = application.family().coefficient_context().clone();
    let inputs = vec![
        (IntegralKey::try_new([4]).unwrap(), context.integer(3)),
        (IntegralKey::try_new([2]).unwrap(), context.integer(-2)),
    ];
    let expected = application
        .reduce_routed_linear_combination(inputs.clone())
        .unwrap();
    let mut boundaries = Vec::new();
    let mut first_frontier = BTreeSet::new();
    let actual = application
        .reduce_routed_linear_combination_with_ordered_inspector(inputs.clone(), &mut |event| {
            if boundaries.is_empty() {
                first_frontier = event
                    .frontier()
                    .map(|entry| {
                        assert_eq!(entry.stage, OrderedKeyStage::Route);
                        assert_eq!(entry.phase, 0);
                        (entry.integral.clone(), canonical(entry.weight.unwrap()))
                    })
                    .collect();
            }
            boundaries.push((event.boundary, event.phase, event.descriptor.support));
            Ok(())
        })
        .unwrap();
    assert_eq!(actual.terms(), expected.terms());
    assert_eq!(actual.nonzero_conditions(), expected.nonzero_conditions());
    assert_eq!(
        first_frontier,
        inputs
            .iter()
            .map(|(key, weight)| (key.clone(), canonical(weight)))
            .collect()
    );
    assert!(!boundaries.is_empty());
    for (phase, pair) in boundaries.chunks(2).enumerate() {
        assert_eq!(pair[0].0, OrderedPhaseBoundary::Started);
        assert_eq!(pair[1].0, OrderedPhaseBoundary::Finished);
        assert_eq!((pair[0].1, pair[1].1), (phase, phase));
        assert!(pair[0].2.is_some());
    }
    let stopped = application.reduce_routed_linear_combination_with_ordered_inspector(
        inputs.clone(),
        &mut |event| match event.boundary {
            OrderedPhaseBoundary::Finished => Err(CandidateRoutedError::Cancelled),
            OrderedPhaseBoundary::Started => Ok(()),
        },
    );
    assert!(stopped.is_err());
    application.set_reduction_strategy(ReductionStrategy::DepthFirst);
    assert!(
        application
            .reduce_routed_linear_combination_with_ordered_inspector(inputs, &mut |_| Ok(()))
            .is_err()
    );

    fn canonical(coefficient: &Coefficient) -> String {
        use symbolica::atom::AtomCore;
        coefficient.to_expression().to_canonical_string()
    }
}

/// Cached requests run on the scheduler's workers too: for every thread
/// count, cold (populating) and warm (replaying) requests give the sequential
/// cached request's results, phase states, request statistics (the hit
/// accounting charges one operational node per replayed step), shard hit,
/// miss and insert counters and byte-identical saved shards.
#[test]
fn ordered_step_cache_with_scheduler_threads_matches_the_sequential_cached_run() {
    use rustred::reduction::{
        ModularCommon, OrderedSchedulerOptions, OrderedStepCacheOptions,
        OrderedStepCacheStatistics, OrderedWeightArithmetic, PolynomialOptions, ReductionStrategy,
        SampledOptions,
    };
    use symbolica::atom::AtomCore;
    let scratch = Scratch::new();
    let original = prepared_fixture(&scratch.0.join("source"));
    let runtime = scratch.0.join("runtime");
    original.export_runtime(&runtime).unwrap();
    let context = original.family().coefficient_context().clone();
    // The fixture admits roots [2] and [4]; the repeated root coalesces.
    let inputs = vec![
        (IntegralKey::try_new([4]).unwrap(), context.integer(3)),
        (IntegralKey::try_new([2]).unwrap(), context.integer(-2)),
        (IntegralKey::try_new([4]).unwrap(), context.integer(7)),
    ];
    let listing = |root: &std::path::Path| {
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
    };
    // Counted lookups and inserts per shard (shard loading may differ: a
    // worker can load a shard for a key that then cancels).
    let counted = |statistics: OrderedStepCacheStatistics| {
        statistics
            .shards
            .iter()
            .map(|shard| {
                (
                    shard.shard,
                    [
                        shard.hits,
                        shard.misses,
                        shard.inserts,
                        shard.skipped_inserts,
                        shard.opaque_hits,
                    ],
                )
            })
            .filter(|(_, counters)| counters.iter().any(|&count| count > 0))
            .collect::<Vec<_>>()
    };
    let common = ModularCommon {
        threads: 2,
        ..Default::default()
    };
    let modular = OrderedWeightArithmetic::ModularSampled(
        common,
        SampledOptions {
            batch_points: 4,
            ..Default::default()
        },
    );
    let polynomial =
        OrderedWeightArithmetic::ModularPolynomial(common, PolynomialOptions::default());
    for (index, arithmetic) in [OrderedWeightArithmetic::Exact, modular, polynomial]
        .into_iter()
        .enumerate()
    {
        // One cached request in a fresh session; `populate: false` replays
        // a saved cache read-only.
        let run =
            |scheduler: OrderedSchedulerOptions, directory: &std::path::Path, populate: bool| {
                let mut application = load_saved_application::<1>(
                    &runtime,
                    SavedApplicationOptions {
                        reduction_strategy: ReductionStrategy::Ordered,
                        ordered_scheduler: scheduler,
                        ..Default::default()
                    },
                )
                .unwrap();
                application.set_ordered_weight_arithmetic(arithmetic);
                let cache = application
                    .open_ordered_step_cache(
                        directory,
                        OrderedStepCacheOptions {
                            populate,
                            ..Default::default()
                        },
                    )
                    .unwrap();
                application
                    .set_ordered_step_cache(Some(cache.clone()))
                    .unwrap();
                let mut states = Vec::new();
                let reduced = application
                    .reduce_routed_linear_combination_with_ordered_inspector(
                        inputs.clone(),
                        &mut |event| {
                            let frontier: Vec<_> = event
                                .frontier()
                                .map(|entry| {
                                    (
                                        entry.phase,
                                        entry.integral.clone(),
                                        entry.weight.map(|weight| {
                                            weight.to_expression().to_canonical_string()
                                        }),
                                    )
                                })
                                .collect();
                            states.push((
                                event.boundary,
                                event.phase,
                                event.progress,
                                frontier,
                                event.nonzero_conditions().to_vec(),
                            ));
                            Ok(())
                        },
                    )
                    .unwrap();
                let statistics = cache.statistics();
                if populate {
                    cache.save().unwrap();
                }
                (
                    (
                        reduced.terms().clone(),
                        reduced.nonzero_conditions().to_vec(),
                        *reduced.statistics(),
                        states,
                    ),
                    (
                        counted(statistics.clone()),
                        [
                            statistics.records as u64,
                            statistics.hits,
                            statistics.misses,
                            statistics.inserts,
                        ],
                    ),
                    application.owner_preparation_statistics().prepared_owners,
                )
            };
        let mut cold = Vec::new();
        let mut warm = Vec::new();
        for (threads, window_keys) in [(1, 4096), (2, 1), (3, 2), (4, 4096)] {
            // Forward-pass properties (phase states, operational nodes): a
            // warm run's tail table would close the adjoint tail (on by
            // default) at phase 0.
            let scheduler = OrderedSchedulerOptions {
                threads,
                window_keys,
                adjoint_tail: false,
                ..Default::default()
            };
            let directory = scratch
                .0
                .join(format!("cold-{index}-{threads}-{window_keys}"));
            let (reduced, statistics, _) = run(scheduler, &directory, true);
            cold.push((reduced, statistics, listing(&directory)));
            // Replays the sequential run's records.
            let saved = scratch.0.join(format!("cold-{index}-1-4096"));
            let (reduced, statistics, prepared) = run(scheduler, &saved, false);
            if threads == 1 {
                // Workers may step (and prepare owners for) keys that cancel.
                assert_eq!(prepared, 0, "a sequential warm request prepares no owner");
            }
            warm.push((reduced, statistics));
        }
        let (sequential_cold, (_, [records, hits, _, _]), files) = &cold[0];
        assert!(*records > 0 && *hits == 0);
        assert!(!files.is_empty());
        let (sequential_warm, (_, [_, hits, misses, inserts])) = &warm[0];
        assert_eq!(hits, records);
        assert_eq!((*misses, *inserts), (0, 0));
        assert_eq!(sequential_warm.0, sequential_cold.0);
        assert_eq!(sequential_warm.1, sequential_cold.1);
        assert_eq!(
            sequential_warm.2.operational_nodes,
            sequential_cold.2.operational_nodes
        );
        for run in &cold[1..] {
            assert_eq!(run, &cold[0]);
        }
        for run in &warm[1..] {
            assert_eq!(run, &warm[0]);
        }
    }
}

#[test]
fn ordered_saved_application_scheduler_threads_preserve_results_and_phase_states() {
    use rustred::reduction::{
        ModularCommon, OrderedSchedulerOptions, OrderedWeightArithmetic, PolynomialOptions,
        ReductionStrategy, SampledOptions,
    };
    use symbolica::atom::AtomCore;
    let scratch = Scratch::new();
    let original = prepared_fixture(&scratch.0.join("source"));
    let runtime = scratch.0.join("runtime");
    original.export_runtime(&runtime).unwrap();
    let mut application = load_saved_application::<1>(
        &runtime,
        SavedApplicationOptions {
            reduction_strategy: ReductionStrategy::Ordered,
            ordered_scheduler: OrderedSchedulerOptions::with_threads(3),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(application.ordered_scheduler_options().threads, 3);
    // Route layering is on by default (`FewestTerms`), from the options
    // through the loaded handle.
    assert_eq!(
        SavedApplicationOptions::default().ordered_route_layering,
        rustred::solver::OrderedRouteLayering::FewestTerms
    );
    assert_eq!(
        application.ordered_route_layering(),
        rustred::solver::OrderedRouteLayering::FewestTerms
    );
    let context = application.family().coefficient_context().clone();
    // The fixture admits roots [2] and [4]; the repeated root coalesces.
    let inputs = vec![
        (IntegralKey::try_new([4]).unwrap(), context.integer(3)),
        (IntegralKey::try_new([2]).unwrap(), context.integer(-2)),
        (IntegralKey::try_new([4]).unwrap(), context.integer(7)),
    ];
    let common = ModularCommon {
        threads: 2,
        ..Default::default()
    };
    let modular = OrderedWeightArithmetic::ModularSampled(
        common,
        SampledOptions {
            batch_points: 4,
            ..Default::default()
        },
    );
    let polynomial =
        OrderedWeightArithmetic::ModularPolynomial(common, PolynomialOptions::default());
    for arithmetic in [OrderedWeightArithmetic::Exact, modular, polynomial] {
        application.set_ordered_weight_arithmetic(arithmetic);
        let mut runs = Vec::new();
        for (threads, window_keys) in [(1, 4096), (2, 1), (3, 2), (4, 4096)] {
            application.set_ordered_scheduler_options(OrderedSchedulerOptions {
                threads,
                window_keys,
                ..Default::default()
            });
            let mut states = Vec::new();
            let reduced = application
                .reduce_routed_linear_combination_with_ordered_inspector(
                    inputs.clone(),
                    &mut |event| {
                        let frontier: Vec<_> = event
                            .frontier()
                            .map(|entry| {
                                (
                                    entry.phase,
                                    entry.integral.clone(),
                                    entry
                                        .weight
                                        .map(|weight| weight.to_expression().to_canonical_string()),
                                )
                            })
                            .collect();
                        states.push((
                            event.boundary,
                            event.phase,
                            event.progress,
                            frontier,
                            event.nonzero_conditions().to_vec(),
                        ));
                        Ok(())
                    },
                )
                .unwrap();
            runs.push((
                reduced.terms().clone(),
                reduced.nonzero_conditions().to_vec(),
                states,
            ));
        }
        assert!(!runs[0].0.is_empty());
        for run in &runs[1..] {
            assert_eq!(run, &runs[0]);
        }
    }
}

#[test]
fn unmemoized_saved_application_load_option_and_switch_keep_dfs_results_reusable() {
    use rustred::reduction::ReductionStrategy;
    let scratch = Scratch::new();
    let original = prepared_fixture(&scratch.0.join("source"));
    let runtime = scratch.0.join("runtime");
    original.export_runtime(&runtime).unwrap();
    let mut application = load_saved_application::<1>(
        &runtime,
        SavedApplicationOptions {
            reduction_strategy: ReductionStrategy::DepthFirstWithoutMemoization,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        application.reduction_strategy(),
        ReductionStrategy::DepthFirstWithoutMemoization
    );
    let context = application.family().coefficient_context();
    let inputs = vec![
        (IntegralKey::try_new([4]).unwrap(), context.integer(3)),
        (IntegralKey::try_new([2]).unwrap(), context.integer(-2)),
    ];
    let expected = application
        .reduce_linear_combination(inputs.clone())
        .unwrap();
    assert!(expected.unresolved().is_empty());
    assert_eq!(expected.statistics().work.cache_hits(), 0);
    assert!(expected.statistics().work.rule_applications() > 0);
    assert_eq!(application.cache_statistics().cached_integrals(), 0);
    let repeated = application
        .reduce_linear_combination(inputs.clone())
        .unwrap();
    assert_eq!(repeated.terms(), expected.terms());
    assert_eq!(repeated.nonzero_conditions(), expected.nonzero_conditions());
    assert_eq!(
        repeated.statistics().work.rule_applications(),
        expected.statistics().work.rule_applications()
    );

    // Defaults retain a real completed-result cache. Switching execution mode
    // must neither consume those records nor silently replace it with a zero
    // cache budget that would change ordinary DFS behavior.
    application.set_reduction_strategy(ReductionStrategy::DepthFirst);
    let dfs = application
        .reduce_linear_combination(inputs.clone())
        .unwrap();
    assert_eq!(dfs.terms(), expected.terms());
    assert_eq!(dfs.nonzero_conditions(), expected.nonzero_conditions());
    assert!(
        expected.statistics().work.rule_applications() > dfs.statistics().work.rule_applications()
    );
    let retained = application.cache_statistics().cached_integrals();
    assert!(retained > 0);
    application.set_reduction_strategy(ReductionStrategy::DepthFirstWithoutMemoization);
    let isolated = application
        .reduce_linear_combination(inputs.clone())
        .unwrap();
    assert_eq!(isolated.terms(), expected.terms());
    assert_eq!(isolated.nonzero_conditions(), expected.nonzero_conditions());
    assert_eq!(isolated.statistics().work.cache_hits(), 0);
    assert_eq!(
        isolated.statistics().work.rule_applications(),
        expected.statistics().work.rule_applications()
    );
    assert_eq!(application.cache_statistics().cached_integrals(), retained);
    application.set_reduction_strategy(ReductionStrategy::DepthFirst);
    let warm = application.reduce_linear_combination(inputs).unwrap();
    assert_eq!(warm.terms(), expected.terms());
    assert_eq!(warm.nonzero_conditions(), expected.nonzero_conditions());
    assert_eq!(warm.statistics().work.rule_applications(), 0);
    assert!(warm.statistics().work.cache_hits() > 0);
}

#[test]
fn partial_saved_application_loads_policy_without_changing_recorded_coverage() {
    let scratch = Scratch::new();
    let original = prepared_fixture(&scratch.0.join("source"));
    let runtime = scratch.0.join("runtime");
    original.export_runtime(&runtime).unwrap();
    let mut strict = load_saved_application::<1>(&runtime, Default::default()).unwrap();
    let mut partial = load_saved_application::<1>(
        &runtime,
        SavedApplicationOptions {
            unresolved_policy: UnresolvedPolicy::Preserve,
            ..Default::default()
        },
    )
    .unwrap();
    let key = IntegralKey::try_new([3]).unwrap();
    let one = partial.family().coefficient_context().one();
    assert!(!partial.routed_entry_is_admitted(&key).unwrap());
    assert!(
        strict
            .reduce_routed_linear_combination([(key.clone(), one.clone())])
            .is_err()
    );
    let actual = partial
        .reduce_routed_linear_combination([(key.clone(), one.clone())])
        .unwrap();
    assert!(actual.unresolved().is_empty());
    assert_eq!(actual.terms().len(), 1);
    assert!(!partial.routed_entry_is_admitted(&key).unwrap());
    let warm = partial.reduce_linear_combination([(key, one)]).unwrap();
    assert_eq!(actual.terms(), warm.terms());
    assert_eq!(actual.nonzero_conditions(), warm.nonzero_conditions());
}

#[test]
fn partial_saved_substitution_preserves_unpublished_leaves_and_cancellation() {
    use rustred::sector::OrderingPolicy;
    use rustred::solver::{
        CandidateOwnerContext, CandidateOwnerInput, CandidateOwnerPrograms, CandidateOwnerScope,
        Integral, IntegralOrder, RoutedCandidateReducer, SectorSolution,
    };
    let scratch = Scratch::new();
    let mut application = prepared_fixture(&scratch.0.join("source"));
    let family = application
        .output_family_shared(application.family().fingerprint())
        .unwrap();
    // A deliberately incomplete owner: only I1 is declared, and no rule
    // applies to I2. This fixture tests composition, not IBP provenance.
    let context = Arc::new(
        CandidateOwnerContext::try_new(
            family.clone(),
            CandidateOwnerScope {
                max_numerator_rank: None,
                finite_case_policy: Default::default(),
            },
            vec![],
            Default::default(),
        )
        .unwrap(),
    );
    let owner = CandidateOwnerInput {
        sector: [true],
        saved_root: [true],
        ordering: OrderingPolicy::SpiredUncutV1,
        solution: SectorSolution {
            order: IntegralOrder::new([true], [false]),
            max_numerator_rank: None,
            finite_case_policy: Default::default(),
            rules: vec![],
            finite_residuals: vec![Integral::numeric([1]).unwrap()],
            stats: Default::default(),
        },
    };
    let programs = Arc::new(CandidateOwnerPrograms::try_new(context, [owner]).unwrap());
    application.application = RoutedCandidateReducer::try_new(programs, [], Default::default())
        .unwrap()
        .application();
    application
        .application
        .set_unresolved_policy(UnresolvedPolicy::Preserve)
        .unwrap();
    application.options.unresolved_policy = UnresolvedPolicy::Preserve;
    let key = IntegralKey::try_new([2]).unwrap();
    let c = family.coefficient_context();
    let result = application
        .reduce_linear_combination([(key.clone(), c.integer(7))])
        .unwrap();
    assert_eq!(
        result.terms(),
        &BTreeMap::from([(qualified(&family, 2), c.integer(7))])
    );
    assert_eq!(
        result.unresolved(),
        &BTreeSet::from([qualified(&family, 2)])
    );
    assert_eq!(
        application.remaining_terminals(),
        &BTreeSet::from([qualified(&family, 1)])
    );
    let zero = application
        .reduce_linear_combination([(key.clone(), c.one()), (key, c.integer(-1))])
        .unwrap();
    assert!(zero.terms().is_empty());
    assert!(zero.unresolved().is_empty());
}

#[test]
fn native_compact_maps_preserve_every_coefficient_context_and_guard() {
    let maps = maps_fixture();
    let options = SavedApplicationOptions::default();
    let bytes = codec::encode(&maps, options).unwrap();
    codec::verify_roundtrip(&maps, &bytes, options).unwrap();
    let restored = codec::decode(&bytes, options).unwrap();
    assert_eq!(maps.rows, restored.rows);
    assert_eq!(maps.remaining, restored.remaining);
    let mut restored_rows = restored.rows.values();
    let first_guards = &restored_rows.next().unwrap().conditions;
    assert!(restored_rows.all(|row| Arc::ptr_eq(first_guards, &row.conditions)));
    let family = maps.families.values().next().unwrap();
    let restored_family = restored.families.values().next().unwrap();
    assert!(
        family
            .coefficient_context()
            .has_same_variable_map(restored_family.coefficient_context())
    );
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(codec::decode(&trailing, options).is_err());
    let mut short_limit = options;
    short_limit.binary_limits.max_program_bytes = bytes.len() - 1;
    assert!(codec::decode(&bytes, short_limit).is_err());
}

#[test]
fn application_progress_preserves_output_and_orders_completion_after_substitution() {
    let scratch = Scratch::new();
    let mut application = prepared_fixture(&scratch.0.join("source"));
    let input = [(
        IntegralKey::try_new([4]).unwrap(),
        application.family().coefficient_context().one(),
    )];
    let expected = application
        .reduce_linear_combination(input.clone())
        .unwrap();
    application.clear_cache().unwrap();
    let mut events = Vec::new();
    let actual = application
        .reduce_linear_combination_with_progress(input, Duration::ZERO, |event| events.push(event))
        .unwrap();
    assert_eq!(actual.terms(), expected.terms());
    assert_eq!(actual.nonzero_conditions(), expected.nonzero_conditions());
    assert_eq!(actual.statistics(), expected.statistics());
    assert_eq!(
        actual.terminal_substitution_additions(),
        expected.terminal_substitution_additions()
    );
    let core_done = events
        .iter()
        .position(|event| {
            event["phase"] == "routed_reduction" && event["core"]["status"] == "succeeded"
        })
        .unwrap();
    let substitution = events
        .iter()
        .position(|event| event["phase"] == "terminal_substitution")
        .unwrap();
    assert!(core_done < substitution);
    assert!(
        events[..events.len() - 1]
            .iter()
            .all(|event| event["request_finished"] == false)
    );
    let last = events.last().unwrap();
    assert_eq!(last["request_finished"], true);
    assert_eq!(last["status"], "succeeded");
    assert_eq!(last["completed_terminal_rows"], last["terminal_rows"]);
    assert_eq!(
        last["terminal_substitution_additions"],
        actual.terminal_substitution_additions()
    );
    assert!(last["core"]["cached"]["coefficient_bytes"].is_u64());
    assert_eq!(last["schema"], "rustred.saved-application-progress.v1");
}

#[test]
fn application_progress_reports_admission_and_terminal_substitution_failures() {
    let scratch = Scratch::new();
    let mut application = prepared_fixture(&scratch.0.join("source"));
    let one = application.family().coefficient_context().one();
    let mut events = Vec::new();
    assert!(
        application
            .reduce_linear_combination_with_progress(
                [(IntegralKey::try_new([1, 2]).unwrap(), one.clone())],
                Duration::ZERO,
                |event| events.push(event),
            )
            .is_err()
    );
    assert_eq!(events.len(), 2);
    assert_eq!(events[1]["phase"], "entry_admission");
    assert_eq!(events[1]["status"], "failed");
    assert!(events[1]["core"].is_null());
    events.clear();
    application.terminals.rows.clear();
    assert!(
        application
            .reduce_linear_combination_with_progress(
                [(IntegralKey::try_new([4]).unwrap(), one)],
                Duration::ZERO,
                |event| events.push(event),
            )
            .is_err()
    );
    let last = events.last().unwrap();
    assert_eq!(last["phase"], "terminal_substitution");
    assert_eq!(last["status"], "failed");
    assert_eq!(last["core"]["status"], "succeeded");
    assert_eq!(last["request_finished"], true);
}

#[test]
#[ignore = "bounded release timing diagnostic; reports observations, no timing assertion"]
fn application_progress_cost_probe() {
    let scratch = Scratch::new();
    let mut application = prepared_fixture(&scratch.0.join("source"));
    let input = [(
        IntegralKey::try_new([4]).unwrap(),
        application.family().coefficient_context().one(),
    )];
    let expected = application
        .reduce_linear_combination(input.clone())
        .unwrap();
    // Alternate order to reduce warming/drift bias. This small symbolic fixture
    // is deliberately a high callback-frequency test, not a five-loop forecast.
    for pair in 0..6 {
        for observed in if pair % 2 == 0 {
            [false, true]
        } else {
            [true, false]
        } {
            let started = Instant::now();
            let mut callbacks = 0usize;
            for _ in 0..1000 {
                application.clear_cache().unwrap();
                let actual = if observed {
                    application
                        .reduce_linear_combination_with_progress(
                            input.clone(),
                            Duration::from_secs(1),
                            |event| {
                                std::hint::black_box(event);
                                callbacks += 1;
                            },
                        )
                        .unwrap()
                } else {
                    application
                        .reduce_linear_combination(input.clone())
                        .unwrap()
                };
                assert_eq!(actual.terms(), expected.terms());
                assert_eq!(actual.nonzero_conditions(), expected.nonzero_conditions());
            }
            println!(
                "{}",
                json!({"event":"application_progress_cost", "pair":pair,
                "observed":observed,"requests":1000,"callbacks":callbacks,
                "seconds":started.elapsed().as_secs_f64()})
            );
        }
    }
}

#[test]
fn compact_boundary_rejects_unknown_terminals_and_impossible_guards() {
    let mut maps = maps_fixture();
    let options = SavedApplicationOptions::default();
    maps.remaining.clear();
    assert!(maps.validate(options).is_err());
    let mut maps = maps_fixture();
    let context = maps
        .families
        .values()
        .next()
        .unwrap()
        .coefficient_context()
        .clone();
    Arc::make_mut(&mut maps.rows.values_mut().next().unwrap().conditions).push(context.zero());
    assert!(codec::encode(&maps, options).is_err());
}

#[test]
fn thousands_of_overlapping_guards_keep_first_seen_order_on_warm_application() {
    let scratch = Scratch::new();
    let mut application = prepared_fixture(&scratch.0.join("source"));
    let c = application.family().coefficient_context().clone();
    let input = [(IntegralKey::try_new([2]).unwrap(), c.one())];
    let baseline = application
        .reduce_linear_combination(input.clone())
        .unwrap();
    let d = c.parameter("d").unwrap();
    let guards = (100..6244)
        .map(|n| c.try_sub(&d, &c.integer(n), Default::default()).unwrap())
        .collect::<Vec<_>>();
    let conditions = Arc::make_mut(
        &mut application
            .terminals
            .rows
            .values_mut()
            .next()
            .unwrap()
            .conditions,
    );
    conditions.extend_from_slice(&guards[2048..4096]);
    conditions.extend_from_slice(&guards[..3072]);
    conditions.extend_from_slice(&guards[1024..]);
    let mut expected = baseline.nonzero_conditions().to_vec();
    expected.extend_from_slice(&guards[2048..4096]);
    expected.extend_from_slice(&guards[..2048]);
    expected.extend_from_slice(&guards[4096..]);
    for _ in 0..2 {
        let result = application
            .reduce_linear_combination(input.clone())
            .unwrap();
        assert_eq!(result.nonzero_conditions(), expected);
        assert_eq!(result.terms(), baseline.terms());
        assert_eq!(result.statistics().work.rule_applications(), 0);
        assert!(result.statistics().work.cache_hits() > 0);
    }
}

pub(super) fn prepared_fixture(source: &Path) -> PreparedSavedApplication<1> {
    std::fs::create_dir_all(source.join("objects")).unwrap();
    let source_text = r#"
schema="rustred.project.toml.v1"
[family]
name="saved_application_portable_test"
loop_momenta=["q"]
external_momenta=[]
dimension="d"
[[family.denominators]]
id="P"
expression="q^2-1"
[target]
powers=[1]
"#;
    let mut request = FamilyCandidatesRequest::new(source_text);
    request.numerical_depth = 0;
    request.max_numerator_rank = Some(2);
    let bundle = family_candidates(request).unwrap();
    let bytes = bundle.bundle();
    let (family, _) = load_generated_candidate_owners::<1>(
        &[CandidateOwnerBundle {
            bytes,
            owner_sector: &rustred::sector::Mask::try_new([true]).unwrap(),
        }],
        Default::default(),
        Default::default(),
    )
    .unwrap();
    std::fs::write(source.join("objects/owner.rrbin"), bytes).unwrap();
    let selection = json!({"family_fingerprint":family.fingerprint(),
        "owners":[{"path":"objects/owner.rrbin","bytes":bytes.len(),"mask":"1"}],
        "initial_frontier_routes":[]})
    .to_string();
    // A hole between admitted powers 2 and 4 catches rectangular-envelope bugs.
    let queries = json!({"schema":"rustred.owner-domain-queries.json.v2", "queries":[
        {"id":"two", "owner":"1", "lower":[1], "upper":[1], "max_numerator_rank":0},
        {"id":"four", "owner":"1", "lower":[3], "upper":[3], "max_numerator_rank":0}
    ]})
    .to_string();
    let context = family.coefficient_context();
    let guard = context
        .try_sub(
            &context.parameter("d").unwrap(),
            &context.integer(4),
            Default::default(),
        )
        .unwrap();
    let terminals = TerminalMaps {
        families: BTreeMap::from([(family.fingerprint().to_owned(), family.clone())]),
        rows: BTreeMap::from([(
            qualified(&family, 1),
            TerminalRow {
                terms: BTreeMap::from([(qualified(&family, 1), context.one())]),
                conditions: Arc::new(vec![guard]),
            },
        )]),
        remaining: BTreeSet::from([qualified(&family, 1)]),
        layer: None,
    };
    let payloads =
        serde_json::from_value(json!([{"path":"objects/owner.rrbin","bytes":bytes.len(),
        "blake3":blake3::hash(bytes).to_hex().to_string()}]))
        .unwrap();
    let metadata = json!({"family_fingerprint":family.fingerprint(),"raw_terminals":1,"remaining_terminals":1,
        "source_keys_checked":1,"publication_assurance":"trusted_saved_scope","minimality_claim":false});
    PreparedSavedApplication::from_parts(
        package::ApplicationInputs {
            selection,
            queries,
            owner_base: source.to_path_buf(),
            payloads,
        },
        terminals,
        metadata,
        Default::default(),
    )
    .unwrap()
}

#[test]
fn runtime_is_portable_reuses_cache_and_keeps_exact_scope_holes() {
    let scratch = Scratch::new();
    let source = scratch.0.join("source");
    let mut original = prepared_fixture(&source);
    let context = original.family().coefficient_context().clone();
    let input = vec![
        (IntegralKey::try_new([4]).unwrap(), context.integer(3)),
        (IntegralKey::try_new([2]).unwrap(), context.integer(-2)),
    ];
    let expected = original.reduce_linear_combination(input.clone()).unwrap();
    assert!(!expected.terms().is_empty());
    assert!(!expected.nonzero_conditions().is_empty());
    let destination = scratch.0.join("runtime");
    let report = original.export_runtime(&destination).unwrap();
    assert_eq!(report["schema"], SAVED_APPLICATION_SCHEMA);
    assert!(!destination.join("state-0000000000000001.rrbin").exists());
    assert!(!destination.join("latest.json").exists());
    std::fs::rename(&source, scratch.0.join("source-away")).unwrap();
    let mut restored = load_saved_application::<1>(&destination, Default::default()).unwrap();
    let actual = restored.reduce_linear_combination(input.clone()).unwrap();
    assert_eq!(actual.terms(), expected.terms());
    assert_eq!(actual.nonzero_conditions(), expected.nonzero_conditions());
    let warm = restored.reduce_linear_combination(input.clone()).unwrap();
    assert!(warm.statistics().work.cache_hits() > 0);
    assert_eq!(warm.statistics().work.rule_applications(), 0);
    // Power 3 was necessarily cached as a descendant of power 4; neither a
    // cached answer nor a zero caller coefficient can widen the entry union.
    assert!(
        restored
            .reduce_linear_combination([(IntegralKey::try_new([3]).unwrap(), context.one())])
            .is_err()
    );
    assert!(
        restored
            .reduce_linear_combination([(IntegralKey::try_new([3]).unwrap(), context.zero())])
            .is_err()
    );
    restored.clear_cache().unwrap();
    let cold_again = restored.reduce_linear_combination(input).unwrap();
    assert!(cold_again.statistics().work.rule_applications() > 0);
    assert_eq!(cold_again.terms(), expected.terms());
    let domains = saved_application_domains::<1>(&destination).unwrap();
    assert_eq!(
        domains.iter().map(|d| d.id.as_str()).collect::<Vec<_>>(),
        ["two", "four"]
    );
    // Integrity is checked before any changed owner coefficient is imported.
    let chunk = report["payloads"][0]["chunks"][0]["path"].as_str().unwrap();
    std::fs::write(destination.join(chunk), b"changed").unwrap();
    assert!(load_saved_application::<1>(&destination, Default::default()).is_err());
}

#[test]
fn runtime_options_select_native_cache_representation_and_report_preparation() {
    use rustred::solver::CandidateCacheRepresentation;
    let scratch = Scratch::new();
    let original = prepared_fixture(&scratch.0.join("source"));
    let runtime = scratch.0.join("runtime");
    original.export_runtime(&runtime).unwrap();
    let mut expected = None;
    for representation in [
        CandidateCacheRepresentation::Sparse,
        CandidateCacheRepresentation::Factorized,
    ] {
        let options = SavedApplicationOptions {
            cache_representation: representation,
            ..Default::default()
        };
        let mut application = load_saved_application::<1>(&runtime, options).unwrap();
        assert_eq!(
            application.application.cache_representation(),
            representation
        );
        assert_eq!(application.cache_statistics().cached_integrals(), 0);
        assert_eq!(
            application.owner_preparation_statistics().prepared_owners,
            0
        );
        let timing = application.preparation_statistics();
        assert_eq!(
            timing.total,
            timing.metadata
                + timing.terminal_maps
                + timing.owner_payloads
                + timing.owner_preparation
                + timing.route_preparation
                + timing.other
        );
        let input = [(
            IntegralKey::try_new([4]).unwrap(),
            application.family().coefficient_context().one(),
        )];
        application.route_entry_to_owner(&input[0].0).unwrap();
        assert_eq!(
            application
                .owner_preparation_statistics()
                .preparation_attempts,
            0
        );
        let first = application
            .reduce_linear_combination(input.clone())
            .unwrap();
        assert_eq!(
            application.owner_preparation_statistics().prepared_owners,
            1
        );
        assert_eq!(
            application
                .owner_preparation_statistics()
                .preparation_attempts,
            1
        );
        if let Some((terms, conditions)) = &expected {
            assert_eq!(first.terms(), terms);
            assert_eq!(first.nonzero_conditions(), conditions);
        } else {
            expected = Some((first.terms().clone(), first.nonzero_conditions().to_vec()));
        }
        assert!(first.statistics().work.rule_applications() > 0);
        let warm = application
            .reduce_linear_combination(input.clone())
            .unwrap();
        assert_eq!(warm.terms(), first.terms());
        assert_eq!(warm.nonzero_conditions(), first.nonzero_conditions());
        assert_eq!(warm.statistics().work.rule_applications(), 0);
        assert!(warm.statistics().work.cache_hits() > 0);
        assert!(application.cache_statistics().cached_coefficient_bytes() > 0);
        application.clear_cache().unwrap();
        assert_eq!(
            application.owner_preparation_statistics().prepared_owners,
            1
        );
        assert_eq!(application.cache_statistics().cached_integrals(), 0);
        assert_eq!(application.cache_statistics().cached_coefficient_terms(), 0);
        assert_eq!(application.cache_statistics().cached_coefficient_bytes(), 0);
        assert_eq!(
            application.application.cache_representation(),
            representation
        );
        let after_clear = application.reduce_linear_combination(input).unwrap();
        assert_eq!(after_clear.terms(), first.terms());
        assert!(after_clear.statistics().work.rule_applications() > 0);
        assert_eq!(
            application
                .owner_preparation_statistics()
                .preparation_attempts,
            1
        );
    }
}

#[test]
fn compressed_terminal_export_and_legacy_raw_load_have_identical_maps_and_metadata() {
    let scratch = Scratch::new();
    let original = prepared_fixture(&scratch.0.join("source"));
    let runtime = scratch.0.join("runtime");
    let mut report = original.export_runtime(&runtime).unwrap();
    let descriptor: package::Payload =
        serde_json::from_value(report["terminal_maps"].clone()).unwrap();
    assert_eq!(descriptor.encoding.as_deref(), Some("gzip-chunks-v1"));
    assert!(!runtime.join(&descriptor.path).exists());
    assert!(report["selection"].get("encoding").is_none());
    assert!(report["queries"].get("encoding").is_none());
    let decoded = transport::decode(&runtime, &descriptor).unwrap();
    let native = codec::encode(&original.terminals, original.options).unwrap();
    assert_eq!(decoded, native);
    assert_eq!(descriptor.blake3, blake3::hash(&native).to_hex().as_str());
    let compressed = load_saved_application::<1>(&runtime, Default::default()).unwrap();

    // Simulate a runtime from before terminal-map compression; no native codec
    // version, guard, family definition or mathematical metadata is rewritten.
    std::fs::write(runtime.join(&descriptor.path), &native).unwrap();
    report["terminal_maps"]
        .as_object_mut()
        .unwrap()
        .remove("encoding");
    report["terminal_maps"]
        .as_object_mut()
        .unwrap()
        .remove("chunks");
    std::fs::write(
        runtime.join("application.json"),
        serde_json::to_vec(&report).unwrap(),
    )
    .unwrap();
    let raw = load_saved_application::<1>(&runtime, Default::default()).unwrap();
    // Runtime loading prepares owners lazily for both transports, unlike the
    // eager source fixture. All other mathematical and operational metadata
    // must remain identical.
    assert_eq!(original.metadata["owner_preparation"], "eager");
    let mut runtime_metadata = original.metadata.clone();
    runtime_metadata["owner_preparation"] = json!("on_first_use");
    assert_eq!(compressed.metadata, raw.metadata);
    for restored in [&compressed, &raw] {
        assert_eq!(restored.terminals.rows, original.terminals.rows);
        assert_eq!(restored.terminals.remaining, original.terminals.remaining);
        assert_eq!(restored.metadata, runtime_metadata);
        assert_eq!(restored.inputs.selection, original.inputs.selection);
        assert_eq!(restored.inputs.queries, original.inputs.queries);
    }
}

#[test]
fn compact_saved_maps_use_caller_byte_grants_and_optional_count_diagnostics() {
    let scratch = Scratch::new();
    let mut original = prepared_fixture(&scratch.0.join("source"));
    let runtime = scratch.0.join("runtime");
    original.export_runtime(&runtime).unwrap();
    let input = [(
        IntegralKey::try_new([4]).unwrap(),
        original.family().coefficient_context().one(),
    )];
    let expected = original.reduce_linear_combination(input.clone()).unwrap();
    let mut options = SavedApplicationOptions::default();
    options.binary_limits.max_program_bytes = 1024 * 1024 * 1024 + 1;
    options.binary_limits.max_collection_entries = 0;
    let mut restored = load_saved_application::<1>(&runtime, options).unwrap();
    let actual = restored.reduce_linear_combination(input).unwrap();
    assert_eq!(actual.terms(), expected.terms());
    assert_eq!(actual.nonzero_conditions(), expected.nonzero_conditions());
    options.binary_limits.diagnostics_enabled = true;
    assert!(load_saved_application::<1>(&runtime, options).is_err());
    options.binary_limits.diagnostics_enabled = false;
    options.binary_limits.max_program_bytes = 1;
    assert!(load_saved_application::<1>(&runtime, options).is_err());
}

#[test]
fn saved_runtime_eviction_is_default_and_preserves_exact_refined_outputs() {
    use rustred::solver::{CandidateCacheRepresentation, CandidateRoutedCachePolicy};
    let scratch = Scratch::new();
    let mut original = prepared_fixture(&scratch.0.join("source"));
    let runtime = scratch.0.join("runtime");
    original.export_runtime(&runtime).unwrap();
    let input = [(
        IntegralKey::try_new([4]).unwrap(),
        original.family().coefficient_context().one(),
    )];
    let expected = original.reduce_linear_combination(input.clone()).unwrap();
    for representation in [
        CandidateCacheRepresentation::Sparse,
        CandidateCacheRepresentation::Factorized,
    ] {
        let mut options = SavedApplicationOptions {
            cache_representation: representation,
            ..Default::default()
        };
        options.reduction_limits.diagnostics_enabled = true;
        options.reduction_limits.max_cached_integrals = 2;
        assert_eq!(
            options.cache_policy,
            CandidateRoutedCachePolicy::EvictLeastRecentlyUsed
        );
        let mut application = load_saved_application::<1>(&runtime, options).unwrap();
        let result = application
            .reduce_linear_combination(input.clone())
            .unwrap();
        assert_eq!(result.terms(), expected.terms());
        assert_eq!(result.nonzero_conditions(), expected.nonzero_conditions());
        assert!(result.statistics().cache.evicted_entries > 0);
        assert!(application.cache_statistics().cached_integrals() <= 2);
        assert_eq!(
            application.cache_work_statistics(),
            result.statistics().cache
        );
        let warm = application
            .reduce_linear_combination(input.clone())
            .unwrap();
        assert_eq!(warm.terms(), result.terms());
        assert_eq!(warm.nonzero_conditions(), result.nonzero_conditions());
        assert_eq!(warm.statistics().work.rule_applications(), 0);
        let work = application.cache_work_statistics();
        application.clear_cache().unwrap();
        assert_eq!(application.cache_work_statistics(), work);
        let again = application
            .reduce_linear_combination(input.clone())
            .unwrap();
        assert_eq!(again.terms(), result.terms());
        assert_eq!(again.nonzero_conditions(), result.nonzero_conditions());
        assert!(application.cache_work_statistics().evicted_entries > work.evicted_entries);
        options.cache_policy = CandidateRoutedCachePolicy::FailOnLimit;
        let mut fail_on_limit = load_saved_application::<1>(&runtime, options).unwrap();
        let error = fail_on_limit
            .reduce_linear_combination(input.clone())
            .unwrap_err();
        assert_eq!(error.kind(), crate::AppErrorKind::Execution);
        assert!(
            error.message().contains(
                &rustred::reduction::ReductionError::CacheLimit {
                    requested: 3,
                    limit: 2,
                }
                .to_string()
            )
        );
    }
}

#[test]
fn deferred_runtime_owns_authenticated_bytes_before_first_formula_use() {
    let scratch = Scratch::new();
    let mut original = prepared_fixture(&scratch.0.join("source"));
    let key = IntegralKey::try_new([2]).unwrap();
    let one = original.family().coefficient_context().one();
    let expected = original
        .reduce_linear_combination([(key.clone(), one.clone())])
        .unwrap();
    let runtime = scratch.0.join("runtime");
    let report = original.export_runtime(&runtime).unwrap();
    let mut lazy = load_saved_application::<1>(&runtime, Default::default()).unwrap();
    assert_eq!(lazy.owner_preparation_statistics().prepared_owners, 0);
    let chunk = report["payloads"][0]["chunks"][0]["path"].as_str().unwrap();
    std::fs::write(runtime.join(chunk), b"changed after boundary validation").unwrap();
    let actual = lazy.reduce_linear_combination([(key, one)]).unwrap();
    assert_eq!(actual.terms(), expected.terms());
    assert_eq!(actual.nonzero_conditions(), expected.nonzero_conditions());
    assert_eq!(lazy.owner_preparation_statistics().prepared_owners, 1);
    assert!(load_saved_application::<1>(&runtime, Default::default()).is_err());
    let invalid_workers = SavedApplicationOptions {
        preparation_workers: 0,
        ..Default::default()
    };
    assert!(
        load_saved_application::<1>(&runtime, invalid_workers)
            .err()
            .unwrap()
            .to_string()
            .contains("preparation_workers")
    );
}

/// The entry-bridge fixture (arity 3): one owner `011` of a two-loop family
/// and two selected transport routes (sources `110` and `101`) to it.
/// Returns the application's parts (inputs, terminal maps, metadata).
fn entry_bridge_parts(
    scratch: &Scratch,
) -> (package::ApplicationInputs, TerminalMaps, serde_json::Value) {
    let objects = scratch.0.join("objects");
    if !objects.is_dir() {
        std::fs::create_dir(&objects).unwrap();
    }
    let source = r#"
schema="rustred.project.toml.v1"
[family]
name="saved_application_entry_bridge"
loop_momenta=["q1","q2"]
external_momenta=[]
dimension="d"
[[family.denominators]]
id="P1"
expression="q1^2-1"
[[family.denominators]]
id="P2"
expression="q2^2-1"
[[family.denominators]]
id="P3"
expression="(q1-q2)^2-1"
[target]
powers=[0,1,1]
"#;
    let mut request = FamilyCandidatesRequest::new(source);
    request.nonpositive_indices = vec![0];
    request.numerical_depth = 0;
    request.max_numerator_rank = Some(2);
    let bundle = family_candidates(request).unwrap();
    let bytes = bundle.bundle();
    let (family, programs) = load_generated_candidate_owners::<3>(
        &[CandidateOwnerBundle {
            bytes,
            owner_sector: &rustred::sector::Mask::try_new([false, true, true]).unwrap(),
        }],
        Default::default(),
        Default::default(),
    )
    .unwrap();
    let canonical = IntegralKey::try_new([0, 2, 2]).unwrap();
    let c = family.coefficient_context().clone();
    let direct = rustred::solver::RoutedCandidateReducer::try_new(
        Arc::new(programs),
        [],
        Default::default(),
    )
    .unwrap();
    let trace = direct.trace_targets([canonical]).unwrap();
    assert!(trace.frontier().is_empty());
    let mut rows = BTreeMap::new();
    let mut remaining = BTreeSet::new();
    for terminal in trace.declared_terminals() {
        let terminal = SavedApplicationIntegral {
            family_fingerprint: family.fingerprint().to_owned(),
            integral: terminal.clone(),
        };
        rows.insert(
            terminal.clone(),
            TerminalRow {
                terms: BTreeMap::from([(terminal.clone(), c.one())]),
                conditions: Arc::new(vec![]),
            },
        );
        remaining.insert(terminal);
    }
    assert!(!remaining.is_empty());
    std::fs::write(objects.join("owner.rrbin"), bytes).unwrap();
    let selection = json!({"family_fingerprint":family.fingerprint(),
        "owners":[{"path":"objects/owner.rrbin","bytes":bytes.len(),"mask":"011"}],
        "initial_frontier_routes":[{"source_mask":"110","owner_mask":"011","requires_transport":true,
            "source_to_representative":[["1","0"],["0","1"]],
            "owner_to_representative":[["1","-1"],["1","0"]]},
            {"source_mask":"101","owner_mask":"011","requires_transport":true,
            "source_to_representative":[["1","0"],["1","-1"]],
            "owner_to_representative":[["1","-1"],["1","0"]]}]}).to_string();
    let queries = json!({"schema":"rustred.owner-domain-queries.json.v2", "queries":[
        {"id":"single-owner-image", "owner":"011", "lower":[0,1,1], "upper":[0,1,1], "max_numerator_rank":0}
    ]}).to_string();
    let payloads =
        serde_json::from_value(json!([{"path":"objects/owner.rrbin","bytes":bytes.len(),
        "blake3":blake3::hash(bytes).to_hex().to_string()}]))
        .unwrap();
    let metadata = json!({"family_fingerprint":family.fingerprint(),"raw_terminals":rows.len(),
        "remaining_terminals":remaining.len(),"source_keys_checked":rows.len()});
    (
        package::ApplicationInputs {
            selection,
            queries,
            owner_base: scratch.0.clone(),
            payloads,
        },
        TerminalMaps {
            families: BTreeMap::from([(family.fingerprint().to_owned(), family)]),
            rows,
            remaining,
            layer: None,
        },
        metadata,
    )
}

/// The entry-bridge fixture as an (eagerly prepared) application.
fn entry_bridge_application(
    scratch: &Scratch,
    options: SavedApplicationOptions,
) -> PreparedSavedApplication<3> {
    let (inputs, terminals, metadata) = entry_bridge_parts(scratch);
    PreparedSavedApplication::<3>::from_parts(inputs, terminals, metadata, options).unwrap()
}

#[test]
fn entry_routing_uses_only_admitted_images_and_preserves_exact_batch_coefficients() {
    let scratch = Scratch::new();
    let key = |powers| IntegralKey::try_new(powers).unwrap();
    let canonical = key([0, 2, 2]);
    let original = key([2, 2, 0]);
    let mut options = SavedApplicationOptions::default();
    options.trace_limits.diagnostics_enabled = true;
    options.trace_limits.max_transport_calls = 1;
    let mut application = entry_bridge_application(&scratch, options);
    let c = application.family().coefficient_context().clone();
    assert!(application.validate_entry(&original).is_err());
    assert!(application.routed_entry_is_admitted(&original).unwrap());
    let routed = application.route_entry_to_owner(&original).unwrap();
    assert_eq!(
        routed.terms(),
        &BTreeMap::from([(canonical.clone(), c.one())])
    );
    assert_eq!(routed.statistics().work.rule_applications(), 0);
    assert_eq!(application.cache_statistics().cached_integrals(), 0);
    let expected = application
        .reduce_linear_combination([(canonical.clone(), c.integer(3))])
        .unwrap();
    let actual = application
        .reduce_routed_linear_combination([(original.clone(), c.integer(3))])
        .unwrap();
    assert_eq!(actual.terms(), expected.terms());
    assert_eq!(actual.nonzero_conditions(), expected.nonzero_conditions());
    assert_eq!(actual.statistics().transport_calls, 1);
    assert!(actual.statistics().work.cache_hits() > 0);
    // Admission of the original strict API stays unchanged even after a
    // successful equivalent-entry request and warmed descendant caches.
    assert!(
        application
            .reduce_linear_combination([(original.clone(), c.one())])
            .is_err()
    );
    assert!(application.route_entry_to_owner(&key([0, 1, 1])).is_err());
    assert!(
        !application
            .routed_entry_is_admitted(&key([0, 1, 1]))
            .unwrap()
    );
    assert!(
        !application
            .routed_entry_is_admitted(&key([1, 1, 1]))
            .unwrap()
    );
    // A rejected geometrical witness must not abort a search that can reach an
    // admitted alternative. This does not turn an absent route into authority.
    let mut accepted = None;
    for candidate in [key([1, 1, 1]), original.clone()] {
        if application.routed_entry_is_admitted(&candidate).unwrap() {
            accepted = Some(candidate);
            break;
        }
    }
    assert_eq!(accepted, Some(original.clone()));
    assert!(
        application
            .routed_entry_is_admitted(&IntegralKey::try_new([1]).unwrap())
            .is_err()
    );
    assert!(application.route_entry_to_owner(&key([1, 1, 1])).is_err());
    assert!(
        application
            .reduce_routed_linear_combination([(key([1, 1, 1]), c.one())])
            .is_err()
    );
    assert!(
        application
            .reduce_routed_linear_combination([(key([2, 2, -1]), c.zero())])
            .is_err()
    );
    // Two individually valid transports cannot each reset the one-call budget,
    // even when their final root coefficients would cancel exactly.
    assert!(
        application
            .reduce_routed_linear_combination([
                (original.clone(), c.one()),
                (original, c.integer(-1)),
            ])
            .is_err()
    );
    // Product preparation is optional presentation over the unchanged flat
    // terminal basis; it neither evaluates lower masters nor clears work.
    let flat = application.remaining_terminals().clone();
    let cache = application.cache_statistics();
    let statistics = application
        .prepare_terminal_products(Default::default())
        .unwrap();
    assert!(statistics.work.factored_terminals > 0);
    assert_eq!(application.remaining_terminals(), &flat);
    assert_eq!(application.cache_statistics(), cache);
    assert_eq!(
        application
            .prepare_terminal_products(Default::default())
            .unwrap(),
        statistics
    );
    assert_eq!(application.terminal_product_statistics(), Some(&statistics));
    for terminal in actual.terms().keys() {
        let product = application.terminal_product(terminal).unwrap();
        assert_eq!(product.coefficient(), &c.one());
        assert_eq!(product.factors().len(), 2);
        assert!(product.factors().iter().all(|factor| {
            application
                .output_family(factor.family_fingerprint())
                .unwrap()
                .loop_count()
                == 1
        }));
    }
    assert!(
        application
            .prepare_terminal_products(
                rustred::reduction::terminal_normalization::VacuumProductLimits {
                    max_terminals: 0,
                    ..Default::default()
                }
            )
            .is_err()
    );
    assert_eq!(application.terminal_product_statistics(), Some(&statistics));
    let after_products = application
        .reduce_linear_combination([(canonical, c.integer(3))])
        .unwrap();
    assert_eq!(after_products.terms(), actual.terms());
    let runtime = scratch.0.join("runtime");
    application.export_runtime(&runtime).unwrap();
    let mut serial = load_saved_application::<3>(&runtime, options).unwrap();
    let mut parallel = load_saved_application::<3>(
        &runtime,
        SavedApplicationOptions {
            preparation_workers: 2,
            ..options
        },
    )
    .unwrap();
    assert_eq!(serial.owner_preparation_statistics().prepared_owners, 0);
    assert_eq!(parallel.owner_preparation_statistics().prepared_owners, 0);
    assert!(parallel.routed_entry_is_admitted(&key([2, 2, 0])).unwrap());
    assert!(!parallel.routed_entry_is_admitted(&key([0, 1, 1])).unwrap());
    assert_eq!(
        parallel.owner_preparation_statistics().preparation_attempts,
        0
    );
    for target in [key([2, 2, 0]), key([2, 0, 2])] {
        let serial_route = serial.route_entry_to_owner(&target).unwrap();
        let parallel_route = parallel.route_entry_to_owner(&target).unwrap();
        assert_eq!(serial_route.terms(), parallel_route.terms());
        assert_eq!(
            serial_route.nonzero_conditions(),
            parallel_route.nonzero_conditions()
        );
        let expected = serial
            .reduce_routed_linear_combination([(target.clone(), c.one())])
            .unwrap();
        let actual = parallel
            .reduce_routed_linear_combination([(target, c.one())])
            .unwrap();
        assert_eq!(actual.terms(), expected.terms());
        assert_eq!(actual.nonzero_conditions(), expected.nonzero_conditions());
    }
    assert_eq!(
        parallel.owner_preparation_statistics().preparation_attempts,
        1
    );
}

#[test]
fn saved_application_persistent_file_preserves_terminal_substitutions_and_scope() {
    use rustred::persistence::application_cache::{ApplicationCacheKey, ApplicationCacheRecord};
    use rustred::reduction::{GuardedCacheBackingError, GuardedIntermediateCapture};
    struct Capture(BTreeMap<ApplicationCacheKey, ApplicationCacheRecord>);
    impl GuardedIntermediateCapture for Capture {
        fn wants(
            &mut self,
            _: &ApplicationCacheKey,
            _: &CoefficientContext,
        ) -> Result<bool, GuardedCacheBackingError> {
            Ok(true)
        }
        fn store_completed(
            &mut self,
            record: &ApplicationCacheRecord,
            _: &CoefficientContext,
        ) -> Result<(), GuardedCacheBackingError> {
            self.0.insert(record.key.clone(), record.clone());
            Ok(())
        }
    }
    let scratch = Scratch::new();
    let mut prepared = prepared_fixture(&scratch.0.join("source"));
    let context = prepared.family().coefficient_context().clone();
    let binding = prepared.cache_binding([3; 32]);
    let mut capture = Capture(Default::default());
    let key = IntegralKey::try_new([4]).unwrap();
    let cold = prepared
        .reduce_linear_combination_with_cache(
            [(key.clone(), context.one())],
            &binding,
            None,
            Some(&mut capture),
        )
        .unwrap();
    let root = prepared
        .completed_cache_record(&binding, &ApplicationCacheKey::route(&key))
        .unwrap()
        .unwrap();
    assert_eq!(root, capture.0[&root.key]);
    for record in capture.0.values() {
        prepared
            .validate_completed_cache_record(&binding, record)
            .unwrap();
    }
    let records = capture.0.into_values().collect::<Vec<_>>();
    let (bytes, identity) = SavedApplicationCacheFile::encode_generated(
        &binding,
        &records,
        &context,
        Default::default(),
    )
    .unwrap();
    let mut file = SavedApplicationCacheFile::decode_generated(
        &bytes,
        &binding,
        &identity,
        Default::default(),
    )
    .unwrap();
    prepared.clear_cache().unwrap();
    let warm = prepared
        .reduce_routed_linear_combination_with_cache(
            [(key, context.one())],
            &binding,
            Some(&mut file),
            None,
        )
        .unwrap();
    assert_eq!(cold.terms(), warm.terms());
    assert_eq!(cold.nonzero_conditions(), warm.nonzero_conditions());
    assert_eq!(warm.statistics().persistent_cache.requested_root_hits, 1);
    assert_eq!(warm.statistics().work.rule_applications(), 0);
    assert!(
        prepared
            .reduce_linear_combination_with_cache(
                [(IntegralKey::try_new([3]).unwrap(), context.one())],
                &binding,
                Some(&mut file),
                None
            )
            .is_err()
    );
}

#[test]
fn advisory_saved_coverage_is_independent_of_strict_complete_application() {
    let scratch = Scratch::new();
    let mut prepared = prepared_fixture(&scratch.0.join("source"));
    let context = prepared.family().coefficient_context().clone();
    let outside = IntegralKey::try_new([3]).unwrap();
    assert!(!prepared.entry_is_admitted(&outside).unwrap());
    assert!(prepared.validate_entry(&outside).is_err());
    assert!(prepared.validate_entry_for_reduction(&outside).is_err());
    assert!(prepared.route_entry_to_owner(&outside).is_err());
    assert!(
        prepared
            .reduce_linear_combination([(outside.clone(), context.one())])
            .is_err()
    );
    prepared.options.entry_coverage_policy = SavedApplicationEntryCoveragePolicy::Advisory;
    prepared.validate_entry_for_reduction(&outside).unwrap();
    assert!(!prepared.entry_is_admitted(&outside).unwrap());
    assert!(prepared.validate_entry(&outside).is_err());
    assert!(prepared.route_entry_to_owner(&outside).is_ok());
    let result = prepared
        .reduce_linear_combination([(outside.clone(), context.one())])
        .unwrap();
    assert!(result.unresolved().is_empty());
    assert_eq!(prepared.options.unresolved_policy, UnresolvedPolicy::Reject);
    prepared.clear_cache().unwrap();
    prepared.options.entry_coverage_policy =
        SavedApplicationEntryCoveragePolicy::FollowUnresolvedPolicy;
    prepared.options.unresolved_policy = UnresolvedPolicy::Preserve;
    prepared
        .application
        .set_unresolved_policy(UnresolvedPolicy::Preserve)
        .unwrap();
    assert!(prepared.route_entry_to_owner(&outside).is_err());
    let legacy = prepared
        .reduce_linear_combination([(outside, context.one())])
        .unwrap();
    assert_eq!(legacy.terms(), result.terms());
    assert_eq!(legacy.nonzero_conditions(), result.nonzero_conditions());
}

#[test]
fn public_saved_application_keeps_resource_causes_and_can_retry() {
    use symbolica::utils::resources::{ResourceBudget, ResourceLimitError, ResourceLimits};
    let scratch = Scratch::new();
    let mut prepared = prepared_fixture(&scratch.0.join("resource-native-source"));
    let context = prepared.family().coefficient_context().clone();
    let key = IntegralKey::try_new([4]).unwrap();
    let expired = ResourceBudget::new(ResourceLimits {
        max_process_rss_bytes: None,
        max_elapsed: Some(Duration::ZERO),
    });
    let error = expired
        .run(|| prepared.reduce_linear_combination([(key.clone(), context.one())]))
        .unwrap_err();
    assert_eq!(error.kind(), crate::AppErrorKind::Limit);
    assert!(matches!(
        error.resource_limit_error(),
        Some(ResourceLimitError::Elapsed { .. })
    ));
    let result = prepared
        .reduce_linear_combination([(key, context.one())])
        .unwrap();
    assert!(!result.terms.is_empty());
    assert!(result.unresolved.is_empty());
    assert!(
        !result.conditions.is_empty(),
        "the retry retains native and terminal guards"
    );
}

#[test]
fn public_saved_input_count_template_is_inactive_until_diagnostics_are_enabled() {
    let scratch = Scratch::new();
    let mut prepared = prepared_fixture(&scratch.0.join("count-template-native-source"));
    let context = prepared.family().coefficient_context().clone();
    let key = IntegralKey::try_new([4]).unwrap();
    prepared.options.trace_limits.max_input_targets = 0;
    assert!(!prepared.options.trace_limits.diagnostics_enabled);
    let reference = prepared
        .reduce_linear_combination([(key.clone(), context.one())])
        .unwrap();
    assert!(!reference.terms().is_empty());
    prepared.options.trace_limits.diagnostics_enabled = true;
    let error = prepared
        .reduce_linear_combination([(key.clone(), context.one())])
        .unwrap_err();
    assert!(error.message().contains("input term limit"));
    prepared.options.trace_limits.diagnostics_enabled = false;
    let repeated = prepared
        .reduce_linear_combination([(key, context.one())])
        .unwrap();
    assert_eq!(repeated.terms(), reference.terms());
    assert_eq!(
        repeated.nonzero_conditions(),
        reference.nonzero_conditions()
    );
}

/// Copy a (small) runtime directory.
fn copy_tree(source: &Path, destination: &Path) {
    std::fs::create_dir(destination).unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let path = entry.unwrap().path();
        let target = destination.join(path.file_name().unwrap());
        if path.is_dir() {
            copy_tree(&path, &target);
        } else {
            std::fs::copy(&path, &target).unwrap();
        }
    }
}

/// Review R7SR-F1: the route preparation add-on end to end on a fixture with
/// two transport routes. Writing captures the verified maps; `Auto` restores
/// them (routes and results identical to a verifying load). Every rejection
/// (corrupt bytes, another selection, a missing file, a directory or a
/// dangling symlink in its place, saved routes that do not match the
/// selection at restore time, an eager preparation) is announced once, when
/// it is detected, and the load falls back to verifying all routes.
#[test]
fn route_addon_restores_verified_routes_and_rejections_fall_back_to_verification() {
    let scratch = Scratch::new();
    let options = SavedApplicationOptions::default();
    let runtime = scratch.0.join("runtime");
    entry_bridge_application(&scratch, options)
        .export_runtime(&runtime)
        .unwrap();
    let key = |powers| IntegralKey::try_new(powers).unwrap();
    route_addon::take_announced();
    let mut verified =
        load_saved_application_with_route_addon::<3>(&runtime, options, SavedRouteAddon::Disabled)
            .unwrap();
    assert_eq!(
        verified.route_preparation(),
        &SavedRoutePreparation::default()
    );
    let c = verified.family().coefficient_context().clone();
    // Results of every routed entry, the reference for each load below.
    let results = |application: &mut PreparedSavedApplication<3>| {
        [key([2, 2, 0]), key([2, 0, 2])]
            .into_iter()
            .map(|target| {
                let routed = application.route_entry_to_owner(&target).unwrap();
                let reduced = application
                    .reduce_routed_linear_combination([(target, c.integer(3))])
                    .unwrap();
                (
                    routed.terms().clone(),
                    routed.nonzero_conditions().to_vec(),
                    reduced.terms().clone(),
                    reduced.nonzero_conditions().to_vec(),
                )
            })
            .collect::<Vec<_>>()
    };
    let expected = results(&mut verified);
    // Without an add-on next to `application.json`, `Auto` verifies silently.
    let plain = load_saved_application::<3>(&runtime, options).unwrap();
    assert_eq!(plain.route_preparation(), &SavedRoutePreparation::default());
    assert!(route_addon::take_announced().is_empty());

    let addon = runtime.join(SAVED_ROUTE_PREPARATION_FILE);
    let report = write_saved_application_route_addon::<3>(&runtime, &addon, options).unwrap();
    assert_eq!(report["transport_routes"], 2);
    assert_eq!(report["schema"], SAVED_ROUTE_PREPARATION_SCHEMA);
    let bytes = std::fs::read(&addon).unwrap();
    // An existing add-on is never replaced.
    assert!(write_saved_application_route_addon::<3>(&runtime, &addon, options).is_err());
    let checked = check_saved_application_route_addon::<3>(&runtime, &addon, options).unwrap();
    assert_eq!(
        (&checked["routes"], &checked["identical"]),
        (&json!(2), &json!(true))
    );
    route_addon::take_announced();

    // `Auto` restores the routes from the sibling file.
    let mut restored = load_saved_application::<3>(&runtime, options).unwrap();
    assert_eq!(
        restored.route_preparation(),
        &SavedRoutePreparation {
            source: SavedRoutePreparationSource::Addon,
            addon: Some(addon.clone()),
            rejected: None,
        }
    );
    assert!(route_addon::take_announced().is_empty());
    assert_eq!(route_addon::same_routes(&verified, &restored).unwrap(), 2);
    assert_eq!(results(&mut restored), expected);
    // So does an explicit file, and `Disabled` ignores it.
    let explicit = load_saved_application_with_route_addon::<3>(
        &runtime,
        options,
        SavedRouteAddon::File(&addon),
    )
    .unwrap();
    assert_eq!(
        explicit.route_preparation().source,
        SavedRoutePreparationSource::Addon
    );
    let disabled =
        load_saved_application_with_route_addon::<3>(&runtime, options, SavedRouteAddon::Disabled)
            .unwrap();
    assert_eq!(
        disabled.route_preparation(),
        &SavedRoutePreparation::default()
    );

    let identity =
        route_addon::AddonIdentity::of_application(&package::application_report(&runtime).unwrap())
            .unwrap();
    let routes = route_addon::decode(&bytes, &identity).unwrap();
    assert_eq!(
        routes
            .routes
            .iter()
            .map(|(ordinal, _)| *ordinal)
            .collect::<Vec<_>>(),
        [0, 1]
    );
    // A rejected add-on: loud (announced once, at detection), recorded, and
    // the load verifies every route.
    let rejected = |label: &str, choice: SavedRouteAddon<'_>, path: &Path, reason: &str| {
        let mut loaded =
            load_saved_application_with_route_addon::<3>(&runtime, options, choice).unwrap();
        let preparation = loaded.route_preparation().clone();
        assert_eq!(
            preparation.source,
            SavedRoutePreparationSource::Verified,
            "{label}"
        );
        assert_eq!(preparation.addon.as_deref(), Some(path), "{label}");
        let recorded = preparation.rejected.unwrap();
        assert!(recorded.contains(reason), "{label}: {recorded}");
        let announced = route_addon::take_announced();
        assert_eq!(announced.len(), 1, "{label}: {announced:?}");
        assert!(announced[0].contains(&recorded), "{label}: {announced:?}");
        assert_eq!(route_addon::same_routes(&verified, &loaded).unwrap(), 2);
        assert_eq!(results(&mut loaded), expected, "{label}");
    };
    let mut corrupt = bytes.clone();
    let middle = corrupt.len() / 2;
    corrupt[middle] ^= 1;
    std::fs::remove_file(&addon).unwrap();
    std::fs::write(&addon, &corrupt).unwrap();
    rejected("corrupt", SavedRouteAddon::Auto, &addon, "checksum");
    let mut other = package::application_report(&runtime).unwrap();
    other["selection"]["blake3"] = json!("0".repeat(64));
    let other = route_addon::AddonIdentity::of_application(&other).unwrap();
    std::fs::remove_file(&addon).unwrap();
    std::fs::write(&addon, route_addon::encode(&other, &routes).unwrap()).unwrap();
    rejected(
        "selection",
        SavedRouteAddon::Auto,
        &addon,
        "written for selection",
    );
    let missing = scratch.0.join("missing.rrbin");
    rejected(
        "missing",
        SavedRouteAddon::File(&missing),
        &missing,
        "No such file",
    );
    // Restore-time rejection: valid file and identity, but the recorded
    // ordinals are not the selection's transport routes.
    let mut swapped = routes.clone();
    swapped.routes.swap(0, 1);
    std::fs::remove_file(&addon).unwrap();
    std::fs::write(&addon, route_addon::encode(&identity, &swapped).unwrap()).unwrap();
    rejected(
        "ordinals",
        SavedRouteAddon::Auto,
        &addon,
        "saved routes differ from the selection's transport routes",
    );
    let mut fewer = routes.clone();
    fewer.routes.pop();
    std::fs::remove_file(&addon).unwrap();
    std::fs::write(&addon, route_addon::encode(&identity, &fewer).unwrap()).unwrap();
    rejected(
        "count",
        SavedRouteAddon::Auto,
        &addon,
        "saved routes differ from the selection's transport routes",
    );
    // Something that is not a regular file at the add-on's name is not
    // silently ignored (review R7SR-F3).
    std::fs::remove_file(&addon).unwrap();
    std::fs::create_dir(&addon).unwrap();
    rejected("directory", SavedRouteAddon::Auto, &addon, "directory");
    std::fs::remove_dir(&addon).unwrap();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(scratch.0.join("absent.rrbin"), &addon).unwrap();
        rejected(
            "dangling symlink",
            SavedRouteAddon::Auto,
            &addon,
            "No such file",
        );
        std::fs::remove_file(&addon).unwrap();
    }
    // An eager preparation verifies every route and says so.
    let eager_scratch = Scratch::new();
    let (inputs, terminals, metadata) = entry_bridge_parts(&eager_scratch);
    let eager = PreparedSavedApplication::<3>::from_parts_with_mode(
        inputs,
        terminals,
        metadata,
        options,
        false,
        None,
        Some((addon.clone(), routes)),
    )
    .unwrap();
    assert_eq!(
        eager.route_preparation().rejected.as_deref(),
        Some("not used by an eager preparation")
    );
    assert_eq!(route_addon::take_announced().len(), 1);
}

/// Review R7SR-F3: a rejection is announced when it is detected, so its
/// reason is visible even when the verifying load that replaces the add-on
/// fails afterwards (here: a manifest family checked after the routes).
#[test]
fn route_addon_rejection_is_announced_before_a_failing_verifying_load() {
    let scratch = Scratch::new();
    let options = SavedApplicationOptions::default();
    let runtime = scratch.0.join("runtime");
    entry_bridge_application(&scratch, options)
        .export_runtime(&runtime)
        .unwrap();
    let addon = runtime.join(SAVED_ROUTE_PREPARATION_FILE);
    write_saved_application_route_addon::<3>(&runtime, &addon, options).unwrap();
    let identity =
        route_addon::AddonIdentity::of_application(&package::application_report(&runtime).unwrap())
            .unwrap();
    let routes = route_addon::decode(&std::fs::read(&addon).unwrap(), &identity).unwrap();
    // A copy whose manifest names another family: the add-on (written for
    // the original) is rejected when it is read, the load fails later.
    let broken = scratch.0.join("broken");
    copy_tree(&runtime, &broken);
    let manifest = broken.join("application.json");
    let mut report: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&manifest).unwrap()).unwrap();
    let family = report["metadata"]["family_fingerprint"]
        .as_str()
        .unwrap()
        .to_owned();
    report["metadata"]["family_fingerprint"] = json!(format!("{family}X"));
    std::fs::write(&manifest, serde_json::to_vec(&report).unwrap()).unwrap();
    route_addon::take_announced();
    let error = load_saved_application::<3>(&broken, options)
        .err()
        .expect("the manifest family is checked after the routes");
    assert!(error.to_string().contains("manifest family"), "{error}");
    let announced = route_addon::take_announced();
    assert_eq!(announced.len(), 1, "{announced:?}");
    assert!(
        announced[0].contains("another integral family"),
        "{announced:?}"
    );
    // The same with a restore-time rejection: an add-on of the broken
    // identity whose route ordinals do not match the selection.
    let broken_identity = route_addon::AddonIdentity::of_application(&report).unwrap();
    let mut swapped = routes;
    swapped.routes.swap(0, 1);
    let broken_addon = broken.join(SAVED_ROUTE_PREPARATION_FILE);
    std::fs::remove_file(&broken_addon).unwrap();
    std::fs::write(
        &broken_addon,
        route_addon::encode(&broken_identity, &swapped).unwrap(),
    )
    .unwrap();
    assert!(load_saved_application::<3>(&broken, options).is_err());
    let announced = route_addon::take_announced();
    assert_eq!(announced.len(), 1, "{announced:?}");
    assert!(
        announced[0].contains("saved routes differ from the selection's transport routes"),
        "{announced:?}"
    );
}

/// Review R7ML-F1: the `ordered_route_layering` load option reaches the
/// loaded handle (with and without a route preparation add-on), the setter
/// round-trips, and every layering gives the same routed Ordered results.
#[test]
fn ordered_route_layering_load_option_reaches_the_loaded_application() {
    use rustred::reduction::ReductionStrategy;
    use rustred::solver::OrderedRouteLayering;
    let scratch = Scratch::new();
    let runtime = scratch.0.join("runtime");
    entry_bridge_application(&scratch, Default::default())
        .export_runtime(&runtime)
        .unwrap();
    let addon = runtime.join(SAVED_ROUTE_PREPARATION_FILE);
    write_saved_application_route_addon::<3>(&runtime, &addon, Default::default()).unwrap();
    let key = |powers| IntegralKey::try_new(powers).unwrap();
    let mut expected = None;
    for layering in [
        OrderedRouteLayering::Off,
        OrderedRouteLayering::Index,
        OrderedRouteLayering::FewestTerms,
    ] {
        let options = SavedApplicationOptions {
            reduction_strategy: ReductionStrategy::Ordered,
            ordered_route_layering: layering,
            ..Default::default()
        };
        let mut loads = vec![
            load_saved_application::<3>(&runtime, options).unwrap(),
            load_saved_application_with_route_addon::<3>(
                &runtime,
                options,
                SavedRouteAddon::Disabled,
            )
            .unwrap(),
        ];
        assert_eq!(
            loads[0].route_preparation().source,
            SavedRoutePreparationSource::Addon
        );
        for application in &mut loads {
            assert_eq!(application.ordered_route_layering(), layering);
            assert_eq!(application.reduction_strategy(), ReductionStrategy::Ordered);
            let c = application.family().coefficient_context().clone();
            let reduced = application
                .reduce_routed_linear_combination([
                    (key([2, 2, 0]), c.integer(3)),
                    (key([2, 0, 2]), c.integer(-2)),
                ])
                .unwrap();
            let result = (
                reduced.terms().clone(),
                reduced.nonzero_conditions().to_vec(),
            );
            assert!(!result.0.is_empty());
            assert_eq!(expected.get_or_insert_with(|| result.clone()), &result);
        }
        // The setter round-trips on a loaded handle.
        let other = if layering == OrderedRouteLayering::Off {
            OrderedRouteLayering::FewestTerms
        } else {
            OrderedRouteLayering::Off
        };
        loads[0].set_ordered_route_layering(other);
        assert_eq!(loads[0].ordered_route_layering(), other);
        loads[0].set_ordered_route_layering(layering);
        assert_eq!(loads[0].ordered_route_layering(), layering);
    }
}

/// Review R7ML-F2: the default configuration (adjoint tail on) with a
/// directory-backed step cache: a cold populating session writes the tail
/// table next to the steps, a new session reads it back (no rejection) and
/// closes the tail from it, and both give the uncached results.
#[test]
fn ordered_step_cache_with_the_default_adjoint_tail_saves_and_reads_its_tail_table() {
    use rustred::reduction::{
        ModularCommon, OrderedStepCacheOptions, OrderedWeightArithmetic, PolynomialOptions,
        ReductionStrategy, SampledOptions,
    };
    let scratch = Scratch::new();
    let original = prepared_fixture(&scratch.0.join("source"));
    let runtime = scratch.0.join("runtime");
    original.export_runtime(&runtime).unwrap();
    let options = SavedApplicationOptions {
        reduction_strategy: ReductionStrategy::Ordered,
        ..Default::default()
    };
    assert!(options.ordered_scheduler.adjoint_tail);
    let context = original.family().coefficient_context().clone();
    let inputs = vec![
        (IntegralKey::try_new([4]).unwrap(), context.integer(3)),
        (IntegralKey::try_new([2]).unwrap(), context.integer(-2)),
    ];
    let common = ModularCommon {
        threads: 2,
        ..Default::default()
    };
    for (index, arithmetic) in [
        OrderedWeightArithmetic::ModularSampled(
            common,
            SampledOptions {
                batch_points: 4,
                ..Default::default()
            },
        ),
        OrderedWeightArithmetic::ModularPolynomial(common, PolynomialOptions::default()),
    ]
    .into_iter()
    .enumerate()
    {
        let run = |cache: Option<&Path>, populate: bool| {
            let mut application = load_saved_application::<1>(&runtime, options).unwrap();
            application.set_ordered_weight_arithmetic(arithmetic);
            let cache = cache.map(|directory| {
                let cache = application
                    .open_ordered_step_cache(
                        directory,
                        OrderedStepCacheOptions {
                            populate,
                            ..Default::default()
                        },
                    )
                    .unwrap();
                application
                    .set_ordered_step_cache(Some(cache.clone()))
                    .unwrap();
                cache
            });
            let reduced = application
                .reduce_routed_linear_combination(inputs.clone())
                .unwrap();
            (
                (
                    reduced.terms().clone(),
                    reduced.nonzero_conditions().to_vec(),
                ),
                cache,
            )
        };
        let (expected, _) = run(None, false);
        assert!(!expected.0.is_empty());
        let directory = scratch.0.join(format!("steps-{index}"));
        let (cold, cache) = run(Some(&directory), true);
        assert_eq!(cold, expected);
        let saved = cache.unwrap().save().unwrap();
        assert!(saved.records > 0);
        assert!(saved.tail_entries > 0, "{arithmetic:?}: {saved:?}");
        assert!(directory.join("ordered-tail.rrtail").is_file());
        let (warm, cache) = run(Some(&directory), false);
        assert_eq!(warm, expected);
        let tail = cache.unwrap().tail_statistics().unwrap();
        assert_eq!(tail.rejected, None);
        assert_eq!(tail.loaded_entries, saved.tail_entries);
    }
}
