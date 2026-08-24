use std::sync::Arc;

use rustred::{
    AffineDenominator, CoefficientContext, CoordinateEqualityLeafStatus,
    GENERATED_SECTOR_LIVE_LEAF_QUEUE_V2_SCHEMA, GeneratedPartialReeliminationCompilation,
    GeneratedSectorDiscoveryCompiler, GeneratedSectorDiscoveryLimits,
    GeneratedSectorIndexBoundaryInterruption, GeneratedSectorLiveLeafOutcome,
    GeneratedSectorLiveLeafQueueCompiler, GeneratedSectorLiveLeafQueueError,
    GeneratedSectorLiveLeafQueueLimits, GeneratedSectorQueuedSourceDisposition,
    GeneratedSymbolicRowSpanCompiler, IntegralFamily, IntegralOrderingPolicy,
    ParametricCoefficientContext, ParametricIbpGenerator, ParametricSectorLeafDisposition,
    SectorMask,
};

fn tadpole_family(name: &str) -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    IntegralFamily::new(
        name,
        vec!["k".into()],
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![AffineDenominator::new(
            coefficients.parse("-m2").unwrap(),
            vec![coefficients.one()],
        )],
        Vec::new(),
        vec![coefficients.zero()],
    )
    .unwrap()
}

fn power_shifted_tadpole_family(name: &str) -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "m2", "nu"]);
    IntegralFamily::new(
        name,
        vec!["k".into()],
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![AffineDenominator::new(
            coefficients.parse("-m2").unwrap(),
            vec![coefficients.one()],
        )],
        Vec::new(),
        vec![coefficients.parameter("nu").unwrap()],
    )
    .unwrap()
}

fn max_guard_root_tadpole_family(name: &str) -> IntegralFamily {
    // Keep one inert base-field symbol because Symbolica's current zero-variable
    // polynomial formatter does not support this generated-relation path.
    let coefficients = CoefficientContext::new(["unused"]);
    IntegralFamily::new(
        name,
        vec!["k".into()],
        Vec::new(),
        coefficients.clone(),
        coefficients.parse("18446744073709551614").unwrap(),
        vec![AffineDenominator::new(
            coefficients.zero(),
            vec![coefficients.one()],
        )],
        Vec::new(),
        vec![coefficients.zero()],
    )
    .unwrap()
}

fn context(family: &IntegralFamily) -> ParametricCoefficientContext {
    ParametricIbpGenerator::try_new(family)
        .unwrap()
        .context()
        .clone()
}

#[test]
fn independently_reconstructed_equal_row_spans_replay_through_public_nested_apis() {
    let family = tadpole_family("live-leaf-queue-independent-row-span-replay");
    let context = context(&family);
    let discovery_limits = GeneratedSectorDiscoveryLimits::default();
    let when_bad = discovery_limits.coverage.generated_when_bad;
    let first = Arc::new(
        GeneratedSymbolicRowSpanCompiler::compile(
            &family,
            &context,
            when_bad.ibp,
            when_bad.row_span,
        )
        .unwrap(),
    );
    let second = Arc::new(
        GeneratedSymbolicRowSpanCompiler::compile(
            &family,
            &context,
            when_bad.ibp,
            when_bad.row_span,
        )
        .unwrap(),
    );
    assert!(!Arc::ptr_eq(&first, &second));

    let discovery = GeneratedSectorDiscoveryCompiler::compile_with_row_span(
        &family,
        &context,
        SectorMask::try_new([true]).unwrap(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        first.clone(),
        discovery_limits,
    )
    .unwrap();
    assert!(Arc::ptr_eq(discovery.row_span_arc(), &first));
    discovery
        .coverage()
        .replay_with_row_span(&family, &context, second.clone())
        .unwrap();
    discovery
        .replay_with_row_span(&family, &context, second.clone())
        .unwrap();

    let queue = GeneratedSectorLiveLeafQueueCompiler::compile_with_row_span(
        &family,
        &context,
        &discovery,
        second.clone(),
        GeneratedSectorLiveLeafQueueLimits::default(),
    )
    .unwrap();
    queue
        .replay_with_row_span(&family, &context, second)
        .unwrap();
}

#[test]
fn one_loop_n_one_exception_is_queued_and_conditionally_reeliminated() {
    let family = tadpole_family("live-leaf-queue-one-loop");
    let context = context(&family);
    let discovery = GeneratedSectorDiscoveryCompiler::compile(
        &family,
        &context,
        SectorMask::try_new([true]).unwrap(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        GeneratedSectorDiscoveryLimits::default(),
    )
    .unwrap();
    let n_one = discovery
        .coverage()
        .classification_for_indices(&context, &[1])
        .unwrap()
        .unwrap();
    assert!(matches!(
        n_one.disposition(),
        ParametricSectorLeafDisposition::Uncovered
            | ParametricSectorLeafDisposition::Unsupported { .. }
    ));

    let mut limits = GeneratedSectorLiveLeafQueueLimits::default();
    limits.translation_radius = 1;
    let queue =
        GeneratedSectorLiveLeafQueueCompiler::compile(&family, &context, &discovery, limits)
            .unwrap();

    assert_eq!(queue.schema(), GENERATED_SECTOR_LIVE_LEAF_QUEUE_V2_SCHEMA);
    assert_eq!(
        queue
            .translations()
            .iter()
            .map(|shift| shift.values())
            .collect::<Vec<_>>(),
        vec![&[-1][..], &[0][..], &[1][..]]
    );
    assert_eq!(
        queue.stats().queued_leaves(),
        discovery.stats().uncovered_leaves() + discovery.stats().unsupported_leaves()
    );
    assert_eq!(
        queue.stats().queued_leaves()
            + queue.stats().descending_leaves_skipped()
            + queue.stats().structurally_empty_leaves_skipped(),
        queue.stats().global_leaves()
    );
    let item = queue
        .work_items()
        .iter()
        .find(|item| item.source_case() == n_one.case())
        .unwrap();
    assert_eq!(item.extraction().assignment().entries(), &[(0, 1)]);
    assert_eq!(
        item.extraction().status(),
        &CoordinateEqualityLeafStatus::NotProvedEmpty
    );
    assert!(matches!(
        item.outcome(),
        GeneratedSectorLiveLeafOutcome::PartialReelimination { .. }
    ));
    let Some(GeneratedPartialReeliminationCompilation::Certified(conditional)) =
        item.outcome().partial_reelimination()
    else {
        panic!("the n=1 equality locus must retain a certified conditional elimination");
    };
    assert!(conditional.elimination_stats().rank() > 0);
    assert!(!conditional.centered_pivot_loci().is_empty());
    queue.replay(&family, &context).unwrap();
}

#[test]
fn unsupported_root_without_an_equality_is_preserved_not_promoted() {
    let family = tadpole_family("live-leaf-queue-unsupported-root");
    let context = context(&family);
    let mut discovery_limits = GeneratedSectorDiscoveryLimits::default();
    discovery_limits.adaptive.max_search_depth = 0;
    let discovery = GeneratedSectorDiscoveryCompiler::compile(
        &family,
        &context,
        SectorMask::try_new([false]).unwrap(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        discovery_limits,
    )
    .unwrap();
    let mut limits = GeneratedSectorLiveLeafQueueLimits::default();
    limits.translation_radius = 0;
    let queue =
        GeneratedSectorLiveLeafQueueCompiler::compile(&family, &context, &discovery, limits)
            .unwrap();

    assert_eq!(queue.work_items().len(), 1);
    let item = &queue.work_items()[0];
    assert!(matches!(
        item.source_disposition(),
        GeneratedSectorQueuedSourceDisposition::Unsupported {
            candidate_ordinals,
        } if !candidate_ordinals.is_empty()
    ));
    assert!(item.extraction().assignment().is_empty());
    assert!(matches!(
        item.outcome(),
        GeneratedSectorLiveLeafOutcome::PreservedWithoutEqualityAssignment
    ));
    assert_eq!(queue.stats().partial_reelimination_attempts(), 0);
    assert_eq!(queue.stats().preserved_without_assignment_leaves(), 1);
    queue.replay(&family, &context).unwrap();
}

#[test]
fn every_residual_predicate_is_retained_when_the_generated_family_has_one() {
    let family = power_shifted_tadpole_family("live-leaf-queue-unresolved-power-shift");
    let context = context(&family);
    let discovery = GeneratedSectorDiscoveryCompiler::compile(
        &family,
        &context,
        SectorMask::try_new([true]).unwrap(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        GeneratedSectorDiscoveryLimits::default(),
    )
    .unwrap();
    let mut limits = GeneratedSectorLiveLeafQueueLimits::default();
    limits.translation_radius = 0;
    let queue =
        GeneratedSectorLiveLeafQueueCompiler::compile(&family, &context, &discovery, limits)
            .unwrap();

    let residual_items = queue
        .work_items()
        .iter()
        .filter(|item| !item.extraction().unresolved_predicates().is_empty())
        .collect::<Vec<_>>();
    assert!(
        !residual_items.is_empty(),
        "the fixture must actually exercise residual predicate retention"
    );
    for item in residual_items {
        let retained = item.extraction().unresolved_predicates().len();
        match item.outcome() {
            GeneratedSectorLiveLeafOutcome::PartialReelimination {
                residual_unresolved_predicates,
                ..
            } => assert_eq!(*residual_unresolved_predicates, retained),
            GeneratedSectorLiveLeafOutcome::PreservedWithoutEqualityAssignment => {
                assert!(item.extraction().assignment().is_empty())
            }
            GeneratedSectorLiveLeafOutcome::PreservedIndexBoundary {
                residual_unresolved_predicates,
                ..
            } => assert_eq!(*residual_unresolved_predicates, retained),
            GeneratedSectorLiveLeafOutcome::CoordinateLeafProvedEmpty => {
                assert!(item.extraction().is_proved_empty())
            }
        }
    }
    assert_eq!(
        queue.stats().coordinate_unresolved_predicates(),
        queue
            .work_items()
            .iter()
            .map(|item| item.extraction().unresolved_predicates().len())
            .sum::<usize>()
    );
    queue.replay(&family, &context).unwrap();
}

#[test]
fn generated_max_guard_root_is_preserved_by_the_actual_queue_and_replays() {
    let family = max_guard_root_tadpole_family("live-leaf-queue-generated-max-boundary");
    let context = context(&family);
    let discovery = GeneratedSectorDiscoveryCompiler::compile(
        &family,
        &context,
        SectorMask::try_new([true]).unwrap(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        GeneratedSectorDiscoveryLimits::default(),
    )
    .unwrap();
    let mut limits = GeneratedSectorLiveLeafQueueLimits::default();
    limits.translation_radius = 1;
    let queue =
        GeneratedSectorLiveLeafQueueCompiler::compile(&family, &context, &discovery, limits)
            .unwrap();

    let boundary = queue
        .work_items()
        .iter()
        .find(|item| {
            matches!(
                item.outcome(),
                GeneratedSectorLiveLeafOutcome::PreservedIndexBoundary { .. }
            )
        })
        .expect("the generated n=i64::MAX guard root must reach the checked queue boundary");
    assert_eq!(
        boundary.extraction().assignment().entries(),
        [(0, i64::MAX)]
    );
    let GeneratedSectorLiveLeafOutcome::PreservedIndexBoundary {
        residual_unresolved_predicates,
        witness,
    } = boundary.outcome()
    else {
        unreachable!("the work-item search already selected the preserved boundary outcome")
    };
    assert_eq!(*residual_unresolved_predicates, 0);
    assert_eq!(witness.ordering().anchor(), [i64::MAX]);
    assert_eq!(
        witness.interruption(),
        GeneratedSectorIndexBoundaryInterruption::EliminationIndexOverflow { position: 0 }
    );
    assert_eq!(queue.stats().preserved_index_boundary_leaves(), 1);
    queue.replay(&family, &context).unwrap();
}

#[test]
fn aggregate_coordinate_budget_fails_before_partial_reelimination() {
    let family = tadpole_family("live-leaf-queue-coordinate-preflight");
    let context = context(&family);
    let discovery = GeneratedSectorDiscoveryCompiler::compile(
        &family,
        &context,
        SectorMask::try_new([true]).unwrap(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        GeneratedSectorDiscoveryLimits::default(),
    )
    .unwrap();
    let mut limits = GeneratedSectorLiveLeafQueueLimits::default();
    limits.translation_radius = 1;
    limits.max_total_coordinate_predicates = 0;
    // If the aggregate extraction preflight were delayed until after partial
    // re-elimination, this stricter inner budget would win instead.
    limits.partial_reelimination.max_canonical_rows = 0;

    assert!(matches!(
        GeneratedSectorLiveLeafQueueCompiler::compile(&family, &context, &discovery, limits),
        Err(GeneratedSectorLiveLeafQueueError::ResourceLimit {
            resource: "aggregate coordinate predicates",
            limit: 0,
            ..
        })
    ));
}

#[test]
fn aggregate_expanded_row_budget_fails_before_generated_compilation() {
    let family = tadpole_family("live-leaf-queue-expanded-row-preflight");
    let context = context(&family);
    let discovery = GeneratedSectorDiscoveryCompiler::compile(
        &family,
        &context,
        SectorMask::try_new([true]).unwrap(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        GeneratedSectorDiscoveryLimits::default(),
    )
    .unwrap();
    let mut limits = GeneratedSectorLiveLeafQueueLimits::default();
    limits.translation_radius = 1;
    limits.max_total_conditional_expanded_rows = 0;
    // A delayed aggregate check would enter the inner compiler and report
    // this independent budget first.
    limits.partial_reelimination.max_canonical_rows = 0;

    assert!(matches!(
        GeneratedSectorLiveLeafQueueCompiler::compile(&family, &context, &discovery, limits),
        Err(GeneratedSectorLiveLeafQueueError::ResourceLimit {
            resource: "aggregate conditional expanded rows",
            limit: 0,
            ..
        })
    ));
}

#[test]
fn arbitrary_partial_reelimination_failure_is_not_boundary_preserved() {
    let family = tadpole_family("live-leaf-queue-non-boundary-failure");
    let context = context(&family);
    let discovery = GeneratedSectorDiscoveryCompiler::compile(
        &family,
        &context,
        SectorMask::try_new([true]).unwrap(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        GeneratedSectorDiscoveryLimits::default(),
    )
    .unwrap();
    let mut limits = GeneratedSectorLiveLeafQueueLimits::default();
    limits.translation_radius = 0;
    limits.partial_reelimination.max_canonical_rows = 0;

    assert!(matches!(
        GeneratedSectorLiveLeafQueueCompiler::compile(&family, &context, &discovery, limits),
        Err(GeneratedSectorLiveLeafQueueError::PartialReelimination(
            rustred::GeneratedPartialReeliminationError::ResourceLimit {
                resource: "canonical generated rows",
                limit: 0,
                ..
            }
        ))
    ));
}
