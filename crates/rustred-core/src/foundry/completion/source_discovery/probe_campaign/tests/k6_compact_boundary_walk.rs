//! Compact canonical boundary walk from the authenticated K=6 revision-nine
//! cover.
//!
//! Unlike the diagnostic transcript regression, the post-bootstrap drive here
//! exercises the production window-one coordinator and retains only its scalar
//! census plus the live exact ledger. The checkpoint remains oracle-disabled:
//! every post-bootstrap owner is discovered from the declared modular probe
//! program.

use std::num::NonZeroUsize;

use crate::foundry::completion::UncoveredPartition;
use crate::foundry::completion::frame::admission::{
    ExactOwnerCoverObstructionKind, ExactOwnerCoverStatus,
};
use crate::foundry::completion::source_discovery::OrdinarySourceIncidenceIndex;
use crate::foundry::completion::source_discovery::cover_delta::ExactOwnerLedgerCoverStatus;
use crate::foundry::completion::source_discovery::test_fixtures::OracleDisabledK6Fixture;

use super::super::{
    BoundaryProbeCoordinator, ProbeCampaignAdapter, ProbeCampaignLimits, ProbeCoordinatorConfig,
    ProbeCoordinatorLimits, ProbeCoordinatorOperationalReason, ProbeCoordinatorProbeBatch,
    ProbeCoordinatorStop,
};
use super::{k6::asserted_revision_nine_ledger, probe};

const MAX_REPORTS: usize = 80;

fn free_dimension_histogram(partition: &UncoveredPartition) -> [usize; 7] {
    let mut histogram = [0usize; 7];
    for cell in partition.boxes() {
        histogram[cell.free_dimension()] += 1;
    }
    histogram
}

#[test]
fn k6_revision_nine_compact_coordinator_reproduces_eighty_report_checkpoint() {
    let fixture = OracleDisabledK6Fixture::shared();
    let campaign_limits = ProbeCampaignLimits::default();
    let incidence = OrdinarySourceIncidenceIndex::try_new(
        fixture.zero_sources(),
        campaign_limits.replay.scheduler.source_discovery,
    )
    .unwrap();
    let adapter = ProbeCampaignAdapter::try_new(
        fixture.generator(),
        fixture.completed(),
        &incidence,
        campaign_limits,
    )
    .unwrap();
    let mut ledger = asserted_revision_nine_ledger();
    let coordinator_limits = ProbeCoordinatorLimits {
        max_task_reports: MAX_REPORTS,
        ..ProbeCoordinatorLimits::default()
    };
    let config = ProbeCoordinatorConfig::try_new(
        "unit-mass-k6-s4a-default-order-rev9-boundary-m2-d0-p1000000007-x37-v1",
        NonZeroUsize::new(1).unwrap(),
        2,
        0,
        coordinator_limits,
    )
    .unwrap();
    let batch_config = config.clone();
    let mut coordinator = BoundaryProbeCoordinator::new(config);
    let mut probes = move |task: &crate::foundry::completion::source_discovery::boundary_simplex::BoundarySimplexTask| {
        ProbeCoordinatorProbeBatch::try_new(
            [probe(task.lattice_target().iter().copied(), campaign_limits)],
            &batch_config,
        )
    };

    let terminal_stop = loop {
        match coordinator.try_run_boundary_epoch(&adapter, &mut ledger, &mut probes) {
            ProbeCoordinatorStop::OwnerSetChanged(changed) => {
                assert_eq!(changed.after_revision(), changed.before_revision() + 1);
                assert_eq!(ledger.revision().get(), changed.after_revision());
            }
            stop @ ProbeCoordinatorStop::OperationallyBounded(_) => break stop,
            ProbeCoordinatorStop::CompilerClosed { .. } => {
                panic!("the eighty-report K6 checkpoint must remain exactly nonfinite")
            }
            ProbeCoordinatorStop::NeedsRefinement(stop) => {
                panic!("the fixed probe program unexpectedly needs refinement: {stop:?}")
            }
            ProbeCoordinatorStop::Failed(stop) => {
                panic!("the compact coordinator failed: {stop:?}")
            }
            ProbeCoordinatorStop::ExhaustedAtConfig { .. } => {
                panic!("the report cap must stop this deliberately bounded prefix")
            }
        }
    };

    let ProbeCoordinatorStop::OperationallyBounded(stop) = terminal_stop else {
        unreachable!()
    };
    assert!(matches!(
        stop.reason(),
        ProbeCoordinatorOperationalReason::TaskReportLimit {
            requested: 81,
            limit: MAX_REPORTS,
        }
    ));
    let location = stop.location().unwrap();
    assert_eq!(location.ledger_revision(), 18);
    assert_eq!(location.class_ordinal(), 0);
    assert_eq!(location.effective_dimension(), 5);
    assert_eq!(location.parent_free_dimension(), 5);
    assert_eq!(location.boundary_codimension(), 0);
    assert_eq!(location.task_ordinal(), 1);

    let census = stop.census();
    assert_eq!(census.epochs_started(), 10);
    assert_eq!(census.plans_built(), 20);
    assert_eq!(census.classes_completed(), 10);
    assert_eq!(census.task_reports(), MAX_REPORTS);
    assert_eq!(census.no_proposal(), 33);
    assert_eq!(census.duplicate(), 38);
    assert_eq!(census.incomplete_proposal(), 0);
    assert_eq!(census.changed_without_geometric_shrink(), 6);
    assert_eq!(census.strict_geometric_shrink(), 3);
    assert_eq!(census.compiler_closed(), 0);
    assert_eq!(
        census.no_proposal()
            + census.duplicate()
            + census.incomplete_proposal()
            + census.changed_without_geometric_shrink()
            + census.strict_geometric_shrink()
            + census.compiler_closed(),
        MAX_REPORTS,
    );
    assert_eq!(census.invalidated_tickets(), 135);
    assert_eq!(census.declared_probes(), MAX_REPORTS);
    assert_eq!(census.scheduler_replayed(), 47);
    assert_eq!(census.scheduler_support_did_not_lift(), 33);
    assert_eq!(census.scheduler_sampled_dual(), 0);
    assert_eq!(census.scheduler_budget_stops(), 0);
    assert_eq!(census.scheduler_rejections(), 0);
    assert_eq!(census.scheduler_stalls(), 0);
    assert_eq!(census.scheduler_exact_lift_errors(), 0);
    assert_eq!(census.canonical_replayed(), 47);
    assert_eq!(census.canonical_no_modular_hit(), 0);
    assert_eq!(census.canonical_query_rejections(), 0);
    assert_eq!(census.canonical_support_did_not_lift(), 0);
    assert_eq!(census.exact_obstructions(), 0);

    let snapshot = ledger.snapshot();
    assert_eq!(snapshot.revision().get(), 18);
    assert_eq!(snapshot.owner_count(), 18);
    assert_eq!(snapshot.terminal_count(), 1);
    assert_eq!(snapshot.uncovered_box_count(), 39);
    assert!(!snapshot.uncovered_is_finite());
    assert_eq!(snapshot.missing_terminal_count(), 0);
    assert_eq!(snapshot.guard_incomplete_owner_count(), 0);
    assert_eq!(
        snapshot.status(),
        ExactOwnerLedgerCoverStatus::Compiled(ExactOwnerCoverStatus::Incomplete(
            ExactOwnerCoverObstructionKind::NonFinite,
        ))
    );
    assert_eq!(
        free_dimension_histogram(&ledger.try_clone_uncovered_partition().unwrap()),
        [0, 0, 0, 17, 20, 2, 0],
    );
}
