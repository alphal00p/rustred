use std::mem::size_of;
use std::sync::Arc;

use crate::family::IntegralKey;
use crate::foundry::artifact::derive_one_loop_unit_mass_tadpole;
use crate::foundry::completion::frame::admission::{
    ExactOwnerCoverObstructionKind, ExactOwnerCoverStatus,
};
use crate::foundry::completion::source_discovery::cover_delta::ExactOwnerCoverDeltaKind;
use crate::foundry::completion::source_discovery::cover_delta::ExactOwnerLedgerCoverStatus;
use crate::foundry::completion::source_discovery::test_fixtures::OracleDisabledK6Fixture;
use crate::foundry::completion::source_discovery::{
    CampaignLimits, CanonicalExactOwnerLedger, ExactOwnerCoverDeltaLimits, ProbeCampaignAdapter,
    ProbeCampaignLimits,
};
use crate::foundry::completion::stratum::{ImmutableOwnerSnapshot, StratumRegistryLimits};
use crate::foundry::completion::{LatticeBox, UncoveredPartition};
use crate::identity::{IntegralShift, ParametricIbpGenerator};
use crate::sector::{Mask, OrderingPolicy};

use super::compact::{
    CompactProbeEvidence, CompactTaskAction, CompactTaskResult, operational_reason,
    search_refinement_reason, try_reserve_compact_result, try_scheduler_outcome_total,
    try_validate_canonical_join, validate_live_effect,
};
use super::run::{ProbeCoordinatorDriveStop, try_drive_partition, upgrade_drive_stop};
use super::schedule::try_build_class_schedule;
use super::{
    BoundaryProbeCoordinator, ProbeCoordinatorCensus, ProbeCoordinatorConfig,
    ProbeCoordinatorFailure, ProbeCoordinatorLimits, ProbeCoordinatorNeedsRefinementReason,
    ProbeCoordinatorOperationalReason, ProbeCoordinatorOwnerMutation, ProbeCoordinatorStop,
    TaskRelativeModularProbe,
};

fn lattice_box(lower: &[u64], upper: &[Option<u64>]) -> LatticeBox {
    LatticeBox::try_new(lower.iter().copied(), upper.iter().copied()).unwrap()
}

fn config() -> ProbeCoordinatorConfig {
    ProbeCoordinatorConfig::try_new(
        [
            TaskRelativeModularProbe::try_new(
                1_000_000_007,
                [37],
                [0, 0],
                CampaignLimits::default(),
            )
            .unwrap(),
        ],
        1,
        0,
        ProbeCoordinatorLimits::default(),
    )
    .unwrap()
}

fn no_proposal() -> CompactTaskResult {
    CompactTaskResult {
        action: CompactTaskAction::NoProposal,
        evidence: CompactProbeEvidence {
            declared_probes: 1,
            scheduler_sampled_dual: 1,
            ..CompactProbeEvidence::default()
        },
    }
}

fn single_d2_partition() -> (Mask, UncoveredPartition) {
    (
        Mask::try_new([true, true]).unwrap(),
        UncoveredPartition::new(vec![lattice_box(&[0, 0], &[None, None])], 0),
    )
}

#[test]
fn class_schedule_is_dimension_descending_and_includes_bulk_and_vertices() {
    let sector = Mask::try_new([true, true, true]).unwrap();
    let forward = UncoveredPartition::new(
        vec![
            lattice_box(&[0, 0, 0], &[None, None, None]),
            lattice_box(&[2, 0, 0], &[Some(2), Some(0), None]),
            lattice_box(&[4, 4, 4], &[Some(4), Some(4), Some(4)]),
        ],
        0,
    );
    let reverse = UncoveredPartition::new(
        forward
            .boxes()
            .iter()
            .rev()
            .map(|cell| lattice_box(cell.lower(), cell.upper()))
            .collect(),
        0,
    );
    let expected = [
        (3, 3, 0, false),
        (2, 3, 1, false),
        (1, 3, 2, false),
        (1, 1, 0, false),
        (0, 3, 3, true),
        (0, 1, 1, true),
        (0, 0, 0, true),
    ];
    let first = try_build_class_schedule(&forward, sector.arity(), &config()).unwrap();
    let second = try_build_class_schedule(&reverse, sector.arity(), &config()).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.present_parent_dimensions(), &[3, 1, 0]);
    assert_eq!(first.classes().len(), expected.len());
    for (ordinal, (class, &(effective, parent, codimension, vertex))) in
        first.classes().iter().zip(&expected).enumerate()
    {
        assert_eq!(class.canonical_ordinal(), ordinal);
        assert_eq!(class.effective_dimension(), effective);
        assert_eq!(class.parent_free_dimension(), parent);
        assert_eq!(class.boundary_codimension(), codimension);
        assert_eq!(
            matches!(
                class.profile(),
                super::super::super::boundary_simplex::BoundarySimplexSamplingProfile::Vertex
            ),
            vertex
        );
    }
}

#[test]
fn stable_pure_drive_is_uncertified_and_visits_every_task_canonically() {
    let (sector, partition) = single_d2_partition();
    let config = config();
    let mut census = ProbeCoordinatorCensus::default();
    let mut visited = Vec::new();
    let stop = try_drive_partition(
        &config,
        "synthetic-pure-scope",
        &mut census,
        7,
        &sector,
        &partition,
        |plan, task, census, requested, invalidated| {
            visited.push((
                plan.face_dimension(),
                plan.parent_free_dimension(),
                plan.boundary_codimension(),
                task.canonical_ordinal(),
            ));
            try_reserve_compact_result(census, requested, invalidated, no_proposal())
        },
    );
    assert_eq!(
        visited,
        vec![(2, 2, 0, 0), (1, 2, 1, 0), (1, 2, 1, 1), (0, 2, 2, 0)]
    );
    let ProbeCoordinatorDriveStop::StableProgramCompleted {
        census,
        ledger_revision,
        completed_classes,
        completed_tasks,
    } = stop
    else {
        panic!("a pure driver may report only uncertified stable completion")
    };
    assert_eq!(ledger_revision, 7);
    assert_eq!(completed_classes, 3);
    assert_eq!(completed_tasks, 4);
    assert_eq!(census.epochs_started(), 1);
    assert_eq!(census.plans_built(), 3);
    assert_eq!(census.classes_completed(), 3);
    assert_eq!(census.task_reports(), 4);
    assert_eq!(census.declared_probes(), 4);
    assert_eq!(census.scheduler_sampled_dual(), 4);
}

#[test]
fn owner_mutation_invalidates_the_plan_suffix_and_next_epoch_restarts() {
    let (sector, partition) = single_d2_partition();
    for mutation in [
        ProbeCoordinatorOwnerMutation::StrictGeometricShrink,
        ProbeCoordinatorOwnerMutation::ChangedWithoutGeometricShrink,
    ] {
        let config = config();
        let mut census = ProbeCoordinatorCensus::default();
        let mut first_visits = Vec::new();
        let first = try_drive_partition(
            &config,
            "synthetic-pure-scope",
            &mut census,
            7,
            &sector,
            &partition,
            |plan, task, census, requested, invalidated| {
                first_visits.push((plan.face_dimension(), task.canonical_ordinal()));
                let compact = if first_visits.len() == 2 {
                    CompactTaskResult {
                        action: CompactTaskAction::OwnerSetChanged {
                            mutation,
                            before_revision: 7,
                            after_revision: 8,
                        },
                        evidence: CompactProbeEvidence {
                            declared_probes: 1,
                            scheduler_replayed: 1,
                            canonical_replayed: 1,
                            ..CompactProbeEvidence::default()
                        },
                    }
                } else {
                    no_proposal()
                };
                try_reserve_compact_result(census, requested, invalidated, compact)
            },
        );
        let ProbeCoordinatorDriveStop::OwnerSetChanged(changed) = first else {
            panic!("the first owner mutation must terminate its immutable epoch")
        };
        assert_eq!(first_visits, vec![(2, 0), (1, 0)]);
        assert_eq!(changed.mutation(), mutation);
        assert_eq!(changed.before_revision(), 7);
        assert_eq!(changed.after_revision(), 8);
        assert_eq!(changed.invalidated_tickets(), 1);
        assert_eq!(changed.census().invalidated_tickets(), 1);

        let mut second_first = None;
        let second = try_drive_partition(
            &config,
            "synthetic-pure-scope",
            &mut census,
            8,
            &sector,
            &partition,
            |plan, task, census, requested, invalidated| {
                second_first.get_or_insert((plan.face_dimension(), task.canonical_ordinal()));
                try_reserve_compact_result(census, requested, invalidated, no_proposal())
            },
        );
        assert_eq!(second_first, Some((2, 0)));
        assert!(matches!(
            second,
            ProbeCoordinatorDriveStop::StableProgramCompleted {
                ledger_revision: 8,
                ..
            }
        ));
    }
}

#[test]
fn operational_refinement_failure_and_stable_stops_are_disjoint() {
    let (sector, partition) = single_d2_partition();

    let operational_config = config();
    let mut operational_census = ProbeCoordinatorCensus::default();
    let stop = try_drive_partition(
        &operational_config,
        "synthetic-pure-scope",
        &mut operational_census,
        0,
        &sector,
        &partition,
        |_, _, census, requested, invalidated| {
            try_reserve_compact_result(
                census,
                requested,
                invalidated,
                CompactTaskResult {
                    action: CompactTaskAction::NoProposal,
                    evidence: CompactProbeEvidence {
                        declared_probes: 1,
                        scheduler_budget_stops: 1,
                        ..CompactProbeEvidence::default()
                    },
                },
            )
        },
    );
    assert!(matches!(
        stop,
        ProbeCoordinatorDriveStop::OperationallyBounded(stop)
            if matches!(
                stop.reason(),
                ProbeCoordinatorOperationalReason::IncompleteProbeExecution {
                    scheduler_budget_stops: 1,
                    ..
                }
            )
    ));

    let refinement_config = config();
    let mut refinement_census = ProbeCoordinatorCensus::default();
    let stop = try_drive_partition(
        &refinement_config,
        "synthetic-pure-scope",
        &mut refinement_census,
        0,
        &sector,
        &partition,
        |_, _, census, requested, invalidated| {
            try_reserve_compact_result(
                census,
                requested,
                invalidated,
                CompactTaskResult {
                    action: CompactTaskAction::NoProposal,
                    evidence: CompactProbeEvidence {
                        declared_probes: 1,
                        scheduler_sampled_dual: 1,
                        canonical_query_rejections: 1,
                        ..CompactProbeEvidence::default()
                    },
                },
            )
        },
    );
    assert!(matches!(
        stop,
        ProbeCoordinatorDriveStop::NeedsRefinement(stop)
            if matches!(
                stop.reason(),
                ProbeCoordinatorNeedsRefinementReason::CanonicalQueryRejected {
                    canonical_query_rejections: 1
                }
            )
    ));

    let failed_config = config();
    let mut failed_census = ProbeCoordinatorCensus::default();
    let stop = try_drive_partition(
        &failed_config,
        "synthetic-pure-scope",
        &mut failed_census,
        0,
        &sector,
        &partition,
        |_, _, _, _, _| {
            Err(ProbeCoordinatorFailure::Invariant {
                detail: "synthetic failure",
            })
        },
    );
    assert!(matches!(stop, ProbeCoordinatorDriveStop::Failed(_)));
}

#[test]
fn compact_probe_census_joins_replay_exactly_and_classifies_every_scheduler_bucket() {
    assert!(try_validate_canonical_join(0, 0, None).is_ok());
    assert!(try_validate_canonical_join(1, 1, Some((1, 1))).is_ok());
    assert!(try_validate_canonical_join(1, 0, Some((0, 1))).is_err());
    assert!(try_validate_canonical_join(1, 1, Some((0, 1))).is_err());
    assert!(try_validate_canonical_join(1, 1, Some((1, 0))).is_err());
    assert!(try_validate_canonical_join(1, 1, None).is_err());

    let scheduler_buckets = [
        CompactProbeEvidence {
            scheduler_replayed: 1,
            ..CompactProbeEvidence::default()
        },
        CompactProbeEvidence {
            scheduler_support_did_not_lift: 1,
            ..CompactProbeEvidence::default()
        },
        CompactProbeEvidence {
            scheduler_exact_lift_errors: 1,
            ..CompactProbeEvidence::default()
        },
        CompactProbeEvidence {
            scheduler_sampled_dual: 1,
            ..CompactProbeEvidence::default()
        },
        CompactProbeEvidence {
            scheduler_budget_stops: 1,
            ..CompactProbeEvidence::default()
        },
        CompactProbeEvidence {
            scheduler_rejections: 1,
            ..CompactProbeEvidence::default()
        },
        CompactProbeEvidence {
            scheduler_stalls: 1,
            ..CompactProbeEvidence::default()
        },
    ];
    for evidence in scheduler_buckets {
        assert_eq!(try_scheduler_outcome_total(evidence).unwrap(), 1);
    }
    let every_bucket = CompactProbeEvidence {
        scheduler_replayed: 1,
        scheduler_support_did_not_lift: 1,
        scheduler_exact_lift_errors: 1,
        scheduler_sampled_dual: 1,
        scheduler_budget_stops: 1,
        scheduler_rejections: 1,
        scheduler_stalls: 1,
        ..CompactProbeEvidence::default()
    };
    assert_eq!(try_scheduler_outcome_total(every_bucket).unwrap(), 7);
    assert!(
        try_scheduler_outcome_total(CompactProbeEvidence {
            scheduler_replayed: usize::MAX,
            scheduler_support_did_not_lift: 1,
            ..CompactProbeEvidence::default()
        })
        .is_err()
    );

    let as_result = |evidence| CompactTaskResult {
        action: CompactTaskAction::NoProposal,
        evidence,
    };
    for evidence in [
        CompactProbeEvidence {
            scheduler_budget_stops: 1,
            ..CompactProbeEvidence::default()
        },
        CompactProbeEvidence {
            scheduler_rejections: 1,
            ..CompactProbeEvidence::default()
        },
        CompactProbeEvidence {
            scheduler_exact_lift_errors: 1,
            ..CompactProbeEvidence::default()
        },
    ] {
        assert!(operational_reason(as_result(evidence)).is_some());
    }
    for evidence in [
        CompactProbeEvidence {
            scheduler_stalls: 1,
            ..CompactProbeEvidence::default()
        },
        CompactProbeEvidence {
            canonical_query_rejections: 1,
            ..CompactProbeEvidence::default()
        },
    ] {
        assert!(search_refinement_reason(as_result(evidence)).is_some());
    }
    for evidence in [
        CompactProbeEvidence {
            scheduler_replayed: 1,
            ..CompactProbeEvidence::default()
        },
        CompactProbeEvidence {
            scheduler_sampled_dual: 1,
            ..CompactProbeEvidence::default()
        },
        // Both support-did-not-lift buckets are exact finite-program misses.
        // They remain counted but do not require a different search config.
        CompactProbeEvidence {
            scheduler_support_did_not_lift: 1,
            ..CompactProbeEvidence::default()
        },
        CompactProbeEvidence {
            canonical_support_did_not_lift: 1,
            ..CompactProbeEvidence::default()
        },
    ] {
        assert!(operational_reason(as_result(evidence)).is_none());
        assert!(search_refinement_reason(as_result(evidence)).is_none());
    }
}

#[test]
fn task_relative_probe_program_is_nonempty_exact_counted_and_bounded() {
    let probe = |offset| {
        TaskRelativeModularProbe::try_new(1_000_000_007, [37], [offset], CampaignLimits::default())
            .unwrap()
    };
    let two_probe_config = ProbeCoordinatorConfig::try_new(
        [probe(1), probe(2)],
        1,
        0,
        ProbeCoordinatorLimits::default(),
    )
    .unwrap();
    assert_eq!(two_probe_config.probes_per_task(), 2);
    assert_eq!(two_probe_config.probes()[0].chart_offsets(), &[1]);
    assert_eq!(two_probe_config.probes()[1].chart_offsets(), &[2]);

    assert!(matches!(
        ProbeCoordinatorConfig::try_new([], 1, 0, ProbeCoordinatorLimits::default(),),
        Err(ProbeCoordinatorFailure::EmptyProbeProgram)
    ));
    let bounded = ProbeCoordinatorLimits {
        max_probes_per_task: 1,
        ..ProbeCoordinatorLimits::default()
    };
    assert!(matches!(
        ProbeCoordinatorConfig::try_new([probe(1), probe(2)], 1, 0, bounded),
        Err(ProbeCoordinatorFailure::ResourceLimit {
            resource: "fixed task-relative probes",
            requested: 2,
            limit: 1,
        })
    ));
    assert!(matches!(
        ProbeCoordinatorConfig::try_new(std::iter::repeat_with(|| probe(1)), 1, 0, bounded,),
        Err(ProbeCoordinatorFailure::ResourceLimit {
            resource: "fixed task-relative probes",
            requested: 2,
            limit: 1,
        })
    ));
}

#[test]
fn compact_census_is_copyable_and_contains_no_dynamic_payload() {
    fn assert_copy<T: Copy>() {}
    assert_copy::<ProbeCoordinatorCensus>();
    assert!(size_of::<ProbeCoordinatorCensus>() <= 32 * size_of::<usize>());
}

#[test]
fn exact_nonfinite_upgrade_requires_current_identity_and_preserves_nonmutating_outcomes() {
    let fixture = OracleDisabledK6Fixture::shared();
    let mut ledger = fixture.new_ledger();
    let plan = fixture.plan(&ledger, 2, 0);
    let owner = fixture.replay_owner(&plan.tasks()[0]);
    let stale_owner_free_identity = ledger.snapshot_identity();
    let owner_free = ledger.snapshot();

    let first = ledger.try_apply_owner(Arc::clone(&owner)).unwrap();
    assert_eq!(
        first.kind(),
        ExactOwnerCoverDeltaKind::StrictGeometricShrink
    );
    let nonfinite = ledger.snapshot();
    assert_eq!(
        nonfinite.status(),
        ExactOwnerLedgerCoverStatus::Compiled(ExactOwnerCoverStatus::Incomplete(
            ExactOwnerCoverObstructionKind::NonFinite,
        ))
    );
    assert!(!nonfinite.uncovered_is_finite());
    assert_eq!(nonfinite.missing_terminal_count(), 0);
    assert_eq!(nonfinite.guard_incomplete_owner_count(), 0);

    let stale_upgrade = ProbeCoordinatorDriveStop::StableProgramCompleted {
        census: ProbeCoordinatorCensus::default(),
        ledger_revision: nonfinite.revision().get(),
        completed_classes: 3,
        completed_tasks: 4,
    };
    assert!(matches!(
        upgrade_drive_stop(stale_upgrade, &ledger, &stale_owner_free_identity),
        ProbeCoordinatorStop::Failed(_)
    ));

    assert!(validate_live_effect(owner_free, nonfinite, CompactTaskAction::NoProposal).is_err());
    let before_duplicate = ledger.snapshot();
    let duplicate = ledger.try_apply_owner(owner).unwrap();
    assert_eq!(duplicate.kind(), ExactOwnerCoverDeltaKind::Duplicate);
    let after_duplicate = ledger.snapshot();
    assert_eq!(before_duplicate, after_duplicate);
    validate_live_effect(
        before_duplicate,
        after_duplicate,
        CompactTaskAction::Duplicate,
    )
    .unwrap();
    validate_live_effect(
        before_duplicate,
        after_duplicate,
        CompactTaskAction::NoProposal,
    )
    .unwrap();

    let current_identity = ledger.snapshot_identity();
    let stable = ProbeCoordinatorDriveStop::StableProgramCompleted {
        census: ProbeCoordinatorCensus::default(),
        ledger_revision: after_duplicate.revision().get(),
        completed_classes: 3,
        completed_tasks: 4,
    };
    let ProbeCoordinatorStop::ExhaustedAtConfig {
        ledger_snapshot,
        exact,
        completed_classes,
        completed_tasks,
        ..
    } = upgrade_drive_stop(stable, &ledger, &current_identity)
    else {
        panic!("an unchanged exact nonfinite zero-gap ledger must permit config exhaustion")
    };
    ledger
        .try_require_current_snapshot(&ledger_snapshot)
        .unwrap();
    assert_eq!(exact, after_duplicate);
    assert_eq!(completed_classes, 3);
    assert_eq!(completed_tasks, 4);
    assert!(!exact.status().is_compiler_closed());
}

#[test]
fn production_stop_rejoins_live_identity_and_never_upgrades_owner_free_state() {
    let artifact = Arc::new(derive_one_loop_unit_mass_tadpole().unwrap());
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    let completed = prepared.complete(rows).unwrap();
    let campaign_limits = ProbeCampaignLimits::default();
    let zero_sources = generator
        .translate_completed_source_rows(
            &completed,
            [IntegralShift::try_new([0]).unwrap()],
            campaign_limits
                .replay
                .scheduler
                .source_discovery
                .translation,
        )
        .unwrap();
    let make_adapter = || {
        ProbeCampaignAdapter::try_new(&generator, &completed, &zero_sources, campaign_limits)
            .unwrap()
    };
    let predecessor = ImmutableOwnerSnapshot::try_from_closed_artifact(
        Arc::clone(&artifact),
        StratumRegistryLimits::default(),
    )
    .unwrap();
    let sector = Mask::try_new([true]).unwrap();
    let mut ledger = CanonicalExactOwnerLedger::try_new(
        generator.context(),
        predecessor,
        sector.clone(),
        OrderingPolicy::default(),
        [IntegralKey::try_new(sector.corner_indices()).unwrap()],
        ExactOwnerCoverDeltaLimits::default(),
    )
    .unwrap();
    let owner_free_identity = ledger.snapshot_identity();
    let owner_free = ledger.snapshot();
    assert_eq!(owner_free.status(), ExactOwnerLedgerCoverStatus::OwnerFree);
    let synthetic_stable = ProbeCoordinatorDriveStop::StableProgramCompleted {
        census: ProbeCoordinatorCensus::default(),
        ledger_revision: owner_free.revision().get(),
        completed_classes: 0,
        completed_tasks: 0,
    };
    assert!(matches!(
        upgrade_drive_stop(synthetic_stable, &ledger, &owner_free_identity),
        ProbeCoordinatorStop::NeedsRefinement(stop)
            if matches!(
                stop.reason(),
                ProbeCoordinatorNeedsRefinementReason::ExactCompilerState {
                    status: ExactOwnerLedgerCoverStatus::OwnerFree,
                    ..
                }
            )
    ));

    let wrong_base = ProbeCoordinatorConfig::try_new(
        [TaskRelativeModularProbe::try_new(
            1_000_000_007,
            [],
            [0],
            campaign_limits.replay.scheduler.campaign,
        )
        .unwrap()],
        1,
        0,
        ProbeCoordinatorLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        BoundaryProbeCoordinator::try_new(wrong_base, make_adapter(), &ledger),
        Err(ProbeCoordinatorFailure::WrongProbeBaseParameterArity {
            probe_ordinal: 0,
            expected: 1,
            actual: 0,
        })
    ));
    let wrong_offsets = ProbeCoordinatorConfig::try_new(
        [TaskRelativeModularProbe::try_new(
            1_000_000_007,
            [37],
            [0, 0],
            campaign_limits.replay.scheduler.campaign,
        )
        .unwrap()],
        1,
        0,
        ProbeCoordinatorLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        BoundaryProbeCoordinator::try_new(wrong_offsets, make_adapter(), &ledger),
        Err(ProbeCoordinatorFailure::WrongProbeChartOffsetArity {
            probe_ordinal: 0,
            expected: 1,
            actual: 2,
        })
    ));

    let coordinator_config = ProbeCoordinatorConfig::try_new(
        [
            TaskRelativeModularProbe::try_new(
                1_000_000_007,
                [37],
                [1],
                campaign_limits.replay.scheduler.campaign,
            )
            .unwrap(),
            TaskRelativeModularProbe::try_new(
                1_000_000_007,
                [37],
                [2],
                campaign_limits.replay.scheduler.campaign,
            )
            .unwrap(),
        ],
        1,
        0,
        ProbeCoordinatorLimits::default(),
    )
    .unwrap();

    let mut peer = CanonicalExactOwnerLedger::try_new(
        generator.context(),
        ledger.predecessor_snapshot().clone(),
        sector.clone(),
        OrderingPolicy::default(),
        [IntegralKey::try_new(sector.corner_indices()).unwrap()],
        ExactOwnerCoverDeltaLimits::default(),
    )
    .unwrap();
    let mut foreign_coordinator =
        BoundaryProbeCoordinator::try_new(coordinator_config.clone(), make_adapter(), &ledger)
            .unwrap();
    assert!(matches!(
        foreign_coordinator.try_run_boundary_epoch(&mut peer),
        ProbeCoordinatorStop::Failed(ref stop)
            if matches!(
                stop.failure(),
                ProbeCoordinatorFailure::Cover(
                    crate::foundry::completion::source_discovery::cover_delta::ExactOwnerCoverDeltaError::ForeignLedgerSnapshotIdentity
                )
            )
    ));

    let overflow_program = ProbeCoordinatorConfig::try_new(
        [TaskRelativeModularProbe::try_new(
            1_000_000_007,
            [37],
            [u64::MAX],
            campaign_limits.replay.scheduler.campaign,
        )
        .unwrap()],
        1,
        0,
        ProbeCoordinatorLimits::default(),
    )
    .unwrap();
    let mut coordinate_overflow =
        BoundaryProbeCoordinator::try_new(overflow_program, make_adapter(), &ledger).unwrap();
    assert!(matches!(
        coordinate_overflow.try_run_boundary_epoch(&mut ledger),
        ProbeCoordinatorStop::Failed(ref stop)
            if matches!(
                stop.failure(),
                ProbeCoordinatorFailure::ProbeChartCoordinateOverflow {
                    probe_ordinal: 0,
                    coordinate: 0,
                }
            )
    ));
    assert_eq!(ledger.snapshot(), owner_free);

    // A compiled owner may have any exact cover effect. Force one possible
    // action counter to overflow and require the reservation to fail before
    // serial application, leaving the opaque exact ledger untouched.
    let before_overflow = ledger.snapshot();
    let before_overflow_identity = ledger.snapshot_identity();
    let mut overflow_coordinator =
        BoundaryProbeCoordinator::try_new(coordinator_config.clone(), make_adapter(), &ledger)
            .unwrap();
    overflow_coordinator.census.duplicate = usize::MAX;
    let overflow = overflow_coordinator.try_run_boundary_epoch(&mut ledger);
    assert!(matches!(
        overflow,
        ProbeCoordinatorStop::Failed(ref stop)
            if matches!(
                stop.failure(),
                ProbeCoordinatorFailure::ResourceCountOverflow { resource: "scalar census" }
            )
    ));
    assert_eq!(ledger.snapshot(), before_overflow);
    assert!(
        ledger
            .snapshot_identity()
            .same_snapshot_as(&before_overflow_identity)
    );

    let mut alternate_action_overflow_coordinator =
        BoundaryProbeCoordinator::try_new(coordinator_config.clone(), make_adapter(), &ledger)
            .unwrap();
    alternate_action_overflow_coordinator
        .census
        .strict_geometric_shrink = usize::MAX;
    let alternate_action_overflow =
        alternate_action_overflow_coordinator.try_run_boundary_epoch(&mut ledger);
    assert!(matches!(
        alternate_action_overflow,
        ProbeCoordinatorStop::Failed(ref stop)
            if matches!(
                stop.failure(),
                ProbeCoordinatorFailure::ResourceCountOverflow { resource: "scalar census" }
            )
    ));
    assert_eq!(ledger.snapshot(), before_overflow);
    assert!(
        ledger
            .snapshot_identity()
            .same_snapshot_as(&before_overflow_identity)
    );

    let mut coordinator =
        BoundaryProbeCoordinator::try_new(coordinator_config, make_adapter(), &ledger).unwrap();
    let stop = coordinator.try_run_boundary_epoch(&mut ledger);
    let ProbeCoordinatorStop::CompilerClosed {
        ledger_snapshot,
        exact,
        ..
    } = stop
    else {
        panic!("the exact one-loop compiler must be the sole closure source")
    };
    ledger
        .try_require_current_snapshot(&ledger_snapshot)
        .unwrap();
    assert_eq!(ledger.snapshot(), exact);
    assert!(exact.status().is_compiler_closed());
}
