use std::mem::size_of;
use std::num::NonZeroUsize;
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
    CampaignLimits, CampaignModularProbe, CanonicalExactOwnerLedger, ExactOwnerCoverDeltaLimits,
    OrdinarySourceIncidenceIndex, ProbeCampaignAdapter, ProbeCampaignLimits,
};
use crate::foundry::completion::stratum::{ImmutableOwnerSnapshot, StratumRegistryLimits};
use crate::foundry::completion::{LatticeBox, UncoveredPartition};
use crate::identity::{IntegralShift, ParametricIbpGenerator};
use crate::sector::{Mask, OrderingPolicy};

use super::compact::{
    CompactProbeEvidence, CompactTaskAction, CompactTaskResult, operational_reason,
    search_refinement_reason, try_scheduler_outcome_total, try_validate_canonical_join,
    validate_live_effect,
};
use super::run::{ProbeCoordinatorDriveStop, try_drive_partition, upgrade_drive_stop};
use super::schedule::try_build_class_schedule;
use super::{
    BoundaryProbeCoordinator, ProbeCoordinatorCensus, ProbeCoordinatorConfig,
    ProbeCoordinatorFailure, ProbeCoordinatorLimits, ProbeCoordinatorNeedsRefinementReason,
    ProbeCoordinatorOperationalReason, ProbeCoordinatorOwnerMutation, ProbeCoordinatorProbeBatch,
    ProbeCoordinatorStop,
};

fn lattice_box(lower: &[u64], upper: &[Option<u64>]) -> LatticeBox {
    LatticeBox::try_new(lower.iter().copied(), upper.iter().copied()).unwrap()
}

fn config() -> ProbeCoordinatorConfig {
    ProbeCoordinatorConfig::try_new(
        "synthetic-program-v1",
        NonZeroUsize::new(1).unwrap(),
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
    let mut coordinator = BoundaryProbeCoordinator::new(config());
    let mut visited = Vec::new();
    let stop = try_drive_partition(&mut coordinator, 7, &sector, &partition, |plan, task| {
        visited.push((
            plan.face_dimension(),
            plan.parent_free_dimension(),
            plan.boundary_codimension(),
            task.canonical_ordinal(),
        ));
        Ok(no_proposal())
    });
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
        let mut coordinator = BoundaryProbeCoordinator::new(config());
        let mut first_visits = Vec::new();
        let first = try_drive_partition(&mut coordinator, 7, &sector, &partition, |plan, task| {
            first_visits.push((plan.face_dimension(), task.canonical_ordinal()));
            if first_visits.len() == 2 {
                Ok(CompactTaskResult {
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
                })
            } else {
                Ok(no_proposal())
            }
        });
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
        let second = try_drive_partition(&mut coordinator, 8, &sector, &partition, |plan, task| {
            second_first.get_or_insert((plan.face_dimension(), task.canonical_ordinal()));
            Ok(no_proposal())
        });
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

    let mut operational = BoundaryProbeCoordinator::new(config());
    let stop = try_drive_partition(&mut operational, 0, &sector, &partition, |_, _| {
        Ok(CompactTaskResult {
            action: CompactTaskAction::NoProposal,
            evidence: CompactProbeEvidence {
                declared_probes: 1,
                scheduler_budget_stops: 1,
                ..CompactProbeEvidence::default()
            },
        })
    });
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

    let mut refinement = BoundaryProbeCoordinator::new(config());
    let stop = try_drive_partition(&mut refinement, 0, &sector, &partition, |_, _| {
        Ok(CompactTaskResult {
            action: CompactTaskAction::NoProposal,
            evidence: CompactProbeEvidence {
                declared_probes: 1,
                scheduler_sampled_dual: 1,
                canonical_query_rejections: 1,
                ..CompactProbeEvidence::default()
            },
        })
    });
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

    let mut failed_coordinator = BoundaryProbeCoordinator::new(config());
    let stop = try_drive_partition(&mut failed_coordinator, 0, &sector, &partition, |_, _| {
        Err(ProbeCoordinatorFailure::Invariant {
            detail: "synthetic failure",
        })
    });
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
fn typed_probe_batch_is_nonempty_exact_counted_and_bounded() {
    let two_probe_config = ProbeCoordinatorConfig::try_new(
        "two-probe-program",
        NonZeroUsize::new(2).unwrap(),
        1,
        0,
        ProbeCoordinatorLimits::default(),
    )
    .unwrap();
    let probe = || {
        CampaignModularProbe::try_new(1_000_000_007, [37], [2], CampaignLimits::default()).unwrap()
    };
    let batch = ProbeCoordinatorProbeBatch::try_new([probe(), probe()], &two_probe_config).unwrap();
    assert_eq!(batch.declared_count(), 2);
    assert_eq!(batch.into_probes().count(), 2);
    assert!(matches!(
        ProbeCoordinatorProbeBatch::try_new([probe()], &two_probe_config),
        Err(ProbeCoordinatorFailure::ProbeCountMismatch {
            expected: 2,
            actual: 1
        })
    ));
    assert!(matches!(
        ProbeCoordinatorProbeBatch::try_new(std::iter::repeat_with(probe), &two_probe_config),
        Err(ProbeCoordinatorFailure::ProbeCountMismatch {
            expected: 2,
            actual: 3
        })
    ));
    assert!(matches!(
        ProbeCoordinatorProbeBatch::try_new([], &two_probe_config),
        Err(ProbeCoordinatorFailure::EmptyProbeBatch)
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
    let incidence = OrdinarySourceIncidenceIndex::try_new(
        &zero_sources,
        campaign_limits.replay.scheduler.source_discovery,
    )
    .unwrap();
    let adapter =
        ProbeCampaignAdapter::try_new(&generator, &completed, &incidence, campaign_limits).unwrap();
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
    let coordinator_config = ProbeCoordinatorConfig::try_new(
        "one-loop-two-probe-program",
        NonZeroUsize::new(2).unwrap(),
        1,
        0,
        ProbeCoordinatorLimits::default(),
    )
    .unwrap();
    let batch_config = coordinator_config.clone();
    let mut coordinator = BoundaryProbeCoordinator::new(coordinator_config);
    let mut probes = move |_: &super::super::super::boundary_simplex::BoundarySimplexTask| {
        let build = |coordinate| {
            CampaignModularProbe::try_new(
                1_000_000_007,
                [37],
                [coordinate],
                campaign_limits.replay.scheduler.campaign,
            )
            .unwrap()
        };
        ProbeCoordinatorProbeBatch::try_new([build(2), build(3)], &batch_config)
    };
    let stop = coordinator.try_run_boundary_epoch(&adapter, &mut ledger, &mut probes);
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
