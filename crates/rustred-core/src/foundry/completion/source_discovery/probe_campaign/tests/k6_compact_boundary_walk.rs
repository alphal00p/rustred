//! Compact canonical boundary walk from the authenticated K=6 positive-margin
//! cover.
//!
//! Unlike the diagnostic transcript regression, the post-bootstrap drive here
//! exercises the production window-one coordinator and retains only its scalar
//! census plus the live exact ledger. The checkpoint remains oracle-disabled:
//! every post-bootstrap owner is discovered from the declared modular probe
//! program.

use crate::foundry::completion::UncoveredPartition;
use crate::foundry::completion::frame::admission::{
    ExactOwnerCoverObstructionKind, ExactOwnerCoverStatus,
};
use crate::foundry::completion::source_discovery::cover_delta::{
    ExactOwnerCoverSnapshot, ExactOwnerLedgerCoverStatus,
};
use crate::foundry::completion::source_discovery::test_fixtures::OracleDisabledK6Fixture;

use super::super::{
    BoundaryProbeCoordinator, ProbeCampaignAdapter, ProbeCampaignLimits, ProbeCoordinatorConfig,
    ProbeCoordinatorLimits, ProbeCoordinatorNeedsRefinement, ProbeCoordinatorNeedsRefinementReason,
    ProbeCoordinatorStop, TaskRelativeModularProbe,
};
use super::k6::asserted_positive_margin_ledger;

const REPORT_LIMIT: usize = 80;
const K6_ARITY: usize = 6;

#[derive(Clone, Debug, PartialEq, Eq)]
struct CompactK6Checkpoint {
    stop: ProbeCoordinatorNeedsRefinement,
    snapshot: ExactOwnerCoverSnapshot,
    free_dimension_histogram: [usize; 7],
    dimension_five_boxes: Vec<(Vec<u64>, Vec<Option<u64>>)>,
}

fn free_dimension_histogram(partition: &UncoveredPartition) -> [usize; 7] {
    let mut histogram = [0usize; 7];
    for cell in partition.boxes() {
        histogram[cell.free_dimension()] += 1;
    }
    histogram
}

fn run_refinement_checkpoint(max_reports: usize) -> CompactK6Checkpoint {
    let fixture = OracleDisabledK6Fixture::shared();
    let campaign_limits = ProbeCampaignLimits::default();
    let adapter = ProbeCampaignAdapter::try_new(
        fixture.generator(),
        fixture.completed(),
        fixture.zero_sources(),
        campaign_limits,
    )
    .unwrap();
    let mut ledger = asserted_positive_margin_ledger();
    let coordinator_limits = ProbeCoordinatorLimits {
        max_task_reports: max_reports,
        ..ProbeCoordinatorLimits::default()
    };
    let config = ProbeCoordinatorConfig::try_new(
        [TaskRelativeModularProbe::try_new(
            1_000_000_007,
            [37],
            std::iter::repeat_n(0, fixture.sector().arity()),
            campaign_limits.replay.scheduler.campaign,
        )
        .unwrap()],
        2,
        0,
        coordinator_limits,
    )
    .unwrap();
    let mut coordinator = BoundaryProbeCoordinator::try_new(config, adapter, &ledger).unwrap();

    let terminal_stop = loop {
        match coordinator.try_run_boundary_epoch(&mut ledger) {
            ProbeCoordinatorStop::OwnerSetChanged(changed) => {
                assert_eq!(changed.after_revision(), changed.before_revision() + 1);
                assert_eq!(ledger.revision().get(), changed.after_revision());
            }
            ProbeCoordinatorStop::OperationallyBounded(stop) => {
                panic!(
                    "the exact-preimage K6 prefix unexpectedly hit an operational limit: {stop:?}"
                )
            }
            ProbeCoordinatorStop::CompilerClosed { .. } => {
                panic!("the K6 refinement checkpoint must remain exactly nonfinite")
            }
            ProbeCoordinatorStop::NeedsRefinement(stop) => break stop,
            ProbeCoordinatorStop::Failed(stop) => {
                panic!("the compact coordinator failed: {stop:?}")
            }
            ProbeCoordinatorStop::ExhaustedAtConfig { .. } => {
                panic!("the K6 refinement checkpoint must stop before exhausting its limit")
            }
        }
    };

    let snapshot = ledger.snapshot();
    let partition = ledger.try_clone_uncovered_partition().unwrap();
    CompactK6Checkpoint {
        stop: terminal_stop,
        snapshot,
        free_dimension_histogram: free_dimension_histogram(&partition),
        dimension_five_boxes: partition
            .boxes()
            .iter()
            .filter(|lattice_box| lattice_box.free_dimension() == 5)
            .map(|lattice_box| (lattice_box.lower().to_vec(), lattice_box.upper().to_vec()))
            .collect(),
    }
}

fn assert_refinement_checkpoint(checkpoint: &CompactK6Checkpoint, max_reports: usize) {
    let stop = checkpoint.stop;
    let ProbeCoordinatorNeedsRefinementReason::IncompleteProposal { exact_obstructions } =
        stop.reason()
    else {
        panic!("the K6 refinement stop must retain the exact obstruction census")
    };
    assert!(exact_obstructions > 0);
    let location = stop.location().unwrap();
    let census = stop.census();
    assert!(census.epochs_started() > 0);
    assert!(census.plans_built() >= census.epochs_started());
    assert!(census.classes_completed() <= census.plans_built());
    assert!(census.task_reports() > 0);
    assert!(census.task_reports() < max_reports);
    assert_eq!(
        census.no_proposal()
            + census.duplicate()
            + census.incomplete_proposal()
            + census.changed_without_geometric_shrink()
            + census.strict_geometric_shrink()
            + census.compiler_closed(),
        census.task_reports(),
    );
    assert_eq!(census.compiler_closed(), 0);
    assert_eq!(census.declared_probes(), census.task_reports());
    assert_eq!(
        census.scheduler_replayed() + census.scheduler_support_did_not_lift(),
        census.task_reports()
    );
    assert_eq!(census.scheduler_sampled_dual(), 0);
    assert_eq!(census.scheduler_budget_stops(), 0);
    assert_eq!(census.scheduler_rejections(), 0);
    assert_eq!(census.scheduler_stalls(), 0);
    assert_eq!(census.scheduler_exact_lift_errors(), 0);
    assert_eq!(census.canonical_replayed(), census.scheduler_replayed());
    assert_eq!(census.canonical_no_modular_hit(), 0);
    assert_eq!(census.canonical_query_rejections(), 0);
    assert_eq!(census.canonical_support_did_not_lift(), 0);
    assert_eq!(census.exact_obstructions(), exact_obstructions);

    let snapshot = checkpoint.snapshot;
    let mutation_count = census
        .changed_without_geometric_shrink()
        .checked_add(census.strict_geometric_shrink())
        .unwrap();
    assert_eq!(
        snapshot.revision().get(),
        7_u64
            .checked_add(u64::try_from(mutation_count).unwrap())
            .unwrap()
    );
    assert_eq!(location.ledger_revision(), snapshot.revision().get());
    assert!(snapshot.owner_count() >= 7);
    assert!(u64::try_from(snapshot.owner_count()).unwrap() <= snapshot.revision().get());
    assert_eq!(snapshot.terminal_count(), 1);
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
        checkpoint.free_dimension_histogram.iter().sum::<usize>(),
        snapshot.uncovered_box_count()
    );
    assert_eq!(checkpoint.free_dimension_histogram[6], 0);
    assert_eq!(
        checkpoint.dimension_five_boxes.len(),
        checkpoint.free_dimension_histogram[5]
    );
    assert!(
        checkpoint
            .dimension_five_boxes
            .iter()
            .all(|(lower, upper)| { lower.len() == K6_ARITY && upper.len() == K6_ARITY })
    );
}

#[test]
fn k6_positive_margin_compact_coordinator_reports_exact_refinement_stop() {
    let checkpoint = run_refinement_checkpoint(REPORT_LIMIT);
    assert_refinement_checkpoint(&checkpoint, REPORT_LIMIT);
}
