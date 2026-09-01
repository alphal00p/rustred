//! Canonical oracle-disabled boundary walk from the authenticated K=6
//! revision-nine cover.

use crate::foundry::completion::UncoveredPartition;
use crate::foundry::completion::frame::admission::{
    ExactOwnerCoverObstructionKind, ExactOwnerCoverStatus,
};
use crate::foundry::completion::source_discovery::OrdinarySourceIncidenceIndex;
use crate::foundry::completion::source_discovery::boundary_simplex::{
    BoundarySimplexLimits, BoundarySimplexPlan, BoundarySimplexSamplingProfile,
    BoundarySimplexScopePartition, try_plan_boundary_simplex_samples,
};
use crate::foundry::completion::source_discovery::cover_delta::{
    CanonicalExactOwnerLedger, ExactOwnerCoverDeltaError, ExactOwnerLedgerCoverStatus,
};
use crate::foundry::completion::source_discovery::test_fixtures::OracleDisabledK6Fixture;

use super::super::{
    ProbeCampaignAdapter, ProbeCampaignCensus, ProbeCampaignError, ProbeCampaignLimits,
};
use super::k6::{RecordedOutcome, asserted_revision_nine_ledger, classify_outcome};
use super::probe;

const MAX_REPORTS: usize = 80;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct OutcomeHistogram {
    no_replayed_nominations: usize,
    no_rebased_circuits: usize,
    incomplete_proposal: usize,
    duplicate: usize,
    changed_without_geometric_shrink: usize,
    strict_geometric_shrink: usize,
    closed: usize,
}

impl OutcomeHistogram {
    fn increment(&mut self, outcome: RecordedOutcome) {
        match outcome {
            RecordedOutcome::NoReplayedNominations => self.no_replayed_nominations += 1,
            RecordedOutcome::NoRebasedCircuits => self.no_rebased_circuits += 1,
            RecordedOutcome::IncompleteProposal => self.incomplete_proposal += 1,
            RecordedOutcome::Duplicate => self.duplicate += 1,
            RecordedOutcome::ChangedWithoutGeometricShrink => {
                self.changed_without_geometric_shrink += 1;
            }
            RecordedOutcome::StrictGeometricShrink => self.strict_geometric_shrink += 1,
            RecordedOutcome::Closed(_) => self.closed += 1,
        }
    }

    fn total(self) -> usize {
        self.no_replayed_nominations
            + self.no_rebased_circuits
            + self.incomplete_proposal
            + self.duplicate
            + self.changed_without_geometric_shrink
            + self.strict_geometric_shrink
            + self.closed
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PlanRecord {
    revision: u64,
    effective_dimension: usize,
    parent_free_dimension: usize,
    boundary_codimension: usize,
    selected_parent_boxes: usize,
    boundary_faces: usize,
    finite_assignments: usize,
    simplex_samples: usize,
    tasks: usize,
    completed_tasks: usize,
    scheduler_workspace_entries: usize,
    scheduler_visits: usize,
    subset_unrank_work_upper_bound: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TaskRecord {
    before_revision: u64,
    after_revision: u64,
    effective_dimension: usize,
    parent_free_dimension: usize,
    boundary_codimension: usize,
    canonical_ordinal: usize,
    lattice_target: Vec<u64>,
    before_owner_count: usize,
    after_owner_count: usize,
    before_box_count: usize,
    after_box_count: usize,
    outcome: RecordedOutcome,
    census: ProbeCampaignCensus,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SnapshotRecord {
    revision: u64,
    owner_count: usize,
    uncovered_box_count: usize,
    present_parent_dimensions: Vec<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StopReason {
    ExactCompilerClosed,
    StableScheduledRadiusZeroSweep,
    StableSweepNeedsRefinement,
    ReportCap,
}

#[derive(Debug)]
struct WalkResult {
    ledger: CanonicalExactOwnerLedger,
    snapshots: Vec<SnapshotRecord>,
    plans: Vec<PlanRecord>,
    tasks: Vec<TaskRecord>,
    outcomes: OutcomeHistogram,
    stale_siblings_rejected: usize,
    stop_reason: StopReason,
}

fn boundary_plan(
    ledger: &CanonicalExactOwnerLedger,
    fixture: &OracleDisabledK6Fixture,
    partition: &UncoveredPartition,
    effective_dimension: usize,
    parent_free_dimension: usize,
) -> Option<BoundarySimplexPlan> {
    if !partition
        .boxes()
        .iter()
        .any(|cell| cell.free_dimension() == parent_free_dimension)
    {
        return None;
    }
    let boundary_codimension = parent_free_dimension - effective_dimension;
    let scope = format!(
        "{}|{}|{}|{:?}|{:?}|{}|boundary-d{}-c{}-r{}",
        ledger.predecessor_snapshot().family_fingerprint(),
        ledger.predecessor_snapshot().context_fingerprint(),
        ledger.predecessor_snapshot().id().as_str(),
        fixture.sector().active_bits(),
        ledger.ordering(),
        ledger.revision().get(),
        parent_free_dimension,
        boundary_codimension,
        effective_dimension,
    );
    let profile = if effective_dimension == 0 {
        BoundarySimplexSamplingProfile::Vertex
    } else {
        BoundarySimplexSamplingProfile::Simplex {
            interior_margin: 2,
            polynomial_degree_ceiling: 0,
        }
    };
    Some(
        try_plan_boundary_simplex_samples(
            ledger.revision().get(),
            [BoundarySimplexScopePartition::new(
                &scope,
                fixture.sector(),
                partition,
            )],
            parent_free_dimension,
            boundary_codimension,
            profile,
            BoundarySimplexLimits::default(),
        )
        .unwrap(),
    )
}

fn assert_clean_completed_probe(census: ProbeCampaignCensus) {
    let outcomes = census.scheduler_outcomes();
    assert_eq!(outcomes.budget_stop(), 0);
    assert_eq!(outcomes.rejected(), 0);
    assert_eq!(outcomes.stalled(), 0);
    assert_eq!(outcomes.exact_lift_error(), 0);
}

fn run_walk(report_cap: usize) -> WalkResult {
    assert!(report_cap > 0);
    let fixture = OracleDisabledK6Fixture::shared();
    let limits = ProbeCampaignLimits::default();
    let incidence = OrdinarySourceIncidenceIndex::try_new(
        fixture.zero_sources(),
        limits.replay.scheduler.source_discovery,
    )
    .unwrap();
    let adapter =
        ProbeCampaignAdapter::try_new(fixture.generator(), fixture.completed(), &incidence, limits)
            .unwrap();
    let mut ledger = asserted_revision_nine_ledger();
    let mut snapshots = Vec::new();
    let mut plans = Vec::new();
    let mut tasks = Vec::new();
    let mut outcomes = OutcomeHistogram::default();
    let mut stale_siblings_rejected = 0usize;

    'snapshots: loop {
        if ledger.snapshot().status().is_compiler_closed() {
            return WalkResult {
                ledger,
                snapshots,
                plans,
                tasks,
                outcomes,
                stale_siblings_rejected,
                stop_reason: StopReason::ExactCompilerClosed,
            };
        }
        let snapshot_revision = ledger.revision().get();
        let partition = ledger.try_clone_uncovered_partition().unwrap();
        let snapshot_identity = ledger.snapshot_identity();
        let mut scheduled_any_class = false;
        let mut stable_incomplete_proposal = false;
        let mut present_parent_dimensions = partition
            .boxes()
            .iter()
            .map(|cell| cell.free_dimension())
            .collect::<Vec<_>>();
        present_parent_dimensions.sort_unstable();
        present_parent_dimensions.dedup();
        present_parent_dimensions.reverse();
        snapshots.push(SnapshotRecord {
            revision: snapshot_revision,
            owner_count: ledger.snapshot().owner_count(),
            uncovered_box_count: partition.boxes().len(),
            present_parent_dimensions: present_parent_dimensions.clone(),
        });
        let maximal_effective_dimension = *present_parent_dimensions
            .first()
            .expect("an incomplete nonfinite cover must retain an uncovered box");

        for effective_dimension in (0..=maximal_effective_dimension).rev() {
            for &parent_free_dimension in &present_parent_dimensions {
                if parent_free_dimension < effective_dimension {
                    continue;
                }
                let Some(plan) = boundary_plan(
                    &ledger,
                    fixture,
                    &partition,
                    effective_dimension,
                    parent_free_dimension,
                ) else {
                    continue;
                };
                scheduled_any_class = true;
                assert_eq!(plan.epoch_ordinal(), snapshot_revision);
                assert_eq!(plan.parent_free_dimension(), parent_free_dimension);
                assert_eq!(plan.face_dimension(), effective_dimension);
                let boundary_codimension = parent_free_dimension - effective_dimension;
                assert_eq!(plan.boundary_codimension(), boundary_codimension);
                let plan_record_index = plans.len();
                plans.push(PlanRecord {
                    revision: snapshot_revision,
                    effective_dimension,
                    parent_free_dimension,
                    boundary_codimension,
                    selected_parent_boxes: plan.selected_parent_box_count(),
                    boundary_faces: plan.boundary_face_count(),
                    finite_assignments: plan.face_finite_assignment_count(),
                    simplex_samples: plan.simplex_sample_count(),
                    tasks: plan.tasks().len(),
                    completed_tasks: 0,
                    scheduler_workspace_entries: plan.scheduler_workspace_entries(),
                    scheduler_visits: plan.scheduler_visit_count(),
                    subset_unrank_work_upper_bound: plan.subset_unrank_work_upper_bound(),
                });

                for (task_ordinal, task) in plan.tasks().iter().enumerate() {
                    if tasks.len() == report_cap {
                        return WalkResult {
                            ledger,
                            snapshots,
                            plans,
                            tasks,
                            outcomes,
                            stale_siblings_rejected,
                            stop_reason: StopReason::ReportCap,
                        };
                    }
                    assert_eq!(task.canonical_ordinal(), task_ordinal);
                    let sibling = plan.tasks().get(task_ordinal + 1).map(|next| {
                        (
                            adapter.try_bind_task(&plan, next, &ledger).unwrap(),
                            next.lattice_target().to_vec(),
                        )
                    });
                    let binding = adapter.try_bind_task(&plan, task, &ledger).unwrap();
                    let before = ledger.snapshot();
                    let before_identity = ledger.snapshot_identity();
                    assert!(before_identity.same_snapshot_as(&snapshot_identity));
                    let report = adapter
                        .try_run_task(
                            binding,
                            &mut ledger,
                            [probe(task.lattice_target().iter().copied(), limits)],
                        )
                        .unwrap();
                    let census = report.census();
                    let outcome = classify_outcome(report.outcome());
                    assert_clean_completed_probe(census);
                    stable_incomplete_proposal |= outcome == RecordedOutcome::IncompleteProposal;
                    outcomes.increment(outcome);
                    plans[plan_record_index].completed_tasks += 1;
                    let after = ledger.snapshot();
                    tasks.push(TaskRecord {
                        before_revision: before.revision().get(),
                        after_revision: after.revision().get(),
                        effective_dimension,
                        parent_free_dimension,
                        boundary_codimension,
                        canonical_ordinal: task_ordinal,
                        lattice_target: task.lattice_target().to_vec(),
                        before_owner_count: before.owner_count(),
                        after_owner_count: after.owner_count(),
                        before_box_count: before.uncovered_box_count(),
                        after_box_count: after.uncovered_box_count(),
                        outcome,
                        census,
                    });

                    let mutated = after.revision() != before.revision();
                    if !mutated {
                        assert_eq!(after, before);
                        assert!(
                            ledger
                                .snapshot_identity()
                                .same_snapshot_as(&before_identity)
                        );
                        continue;
                    }

                    assert_eq!(after.revision().get(), before.revision().get() + 1);
                    assert_eq!(after.owner_count(), before.owner_count() + 1);
                    assert!(matches!(
                        outcome,
                        RecordedOutcome::ChangedWithoutGeometricShrink
                            | RecordedOutcome::StrictGeometricShrink
                            | RecordedOutcome::Closed(_)
                    ));
                    if let Some((sibling, sibling_target)) = sibling {
                        let stale_baseline = ledger.snapshot();
                        assert!(matches!(
                            adapter.try_run_task(
                                sibling,
                                &mut ledger,
                                [probe(sibling_target, limits)],
                            ),
                            Err(ProbeCampaignError::CoverDelta(
                                ExactOwnerCoverDeltaError::StaleLedgerSnapshotIdentity {
                                    expected,
                                    actual,
                                }
                            )) if expected.get() == after.revision().get()
                                && actual.get() == before.revision().get()
                        ));
                        assert_eq!(ledger.snapshot(), stale_baseline);
                        stale_siblings_rejected += 1;
                    }
                    if after.status().is_compiler_closed() {
                        return WalkResult {
                            ledger,
                            snapshots,
                            plans,
                            tasks,
                            outcomes,
                            stale_siblings_rejected,
                            stop_reason: StopReason::ExactCompilerClosed,
                        };
                    }
                    continue 'snapshots;
                }
            }
        }

        assert!(scheduled_any_class);
        assert_eq!(ledger.revision().get(), snapshot_revision);
        assert!(
            ledger
                .snapshot_identity()
                .same_snapshot_as(&snapshot_identity)
        );
        return WalkResult {
            ledger,
            snapshots,
            plans,
            tasks,
            outcomes,
            stale_siblings_rejected,
            stop_reason: if stable_incomplete_proposal {
                StopReason::StableSweepNeedsRefinement
            } else {
                StopReason::StableScheduledRadiusZeroSweep
            },
        };
    }
}

fn assert_canonical_schedule_prefix(result: &WalkResult) {
    for snapshot in &result.snapshots {
        assert_eq!(snapshot.owner_count, snapshot.revision as usize);
        let maximal_dimension = *snapshot.present_parent_dimensions.first().unwrap();
        let mut expected_classes = Vec::new();
        for effective_dimension in (0..=maximal_dimension).rev() {
            for &parent_free_dimension in &snapshot.present_parent_dimensions {
                if parent_free_dimension >= effective_dimension {
                    expected_classes.push((effective_dimension, parent_free_dimension));
                }
            }
        }
        let actual_classes = result
            .plans
            .iter()
            .filter(|plan| plan.revision == snapshot.revision)
            .map(|plan| (plan.effective_dimension, plan.parent_free_dimension))
            .collect::<Vec<_>>();
        assert_eq!(
            actual_classes,
            expected_classes[..actual_classes.len()],
            "every immutable snapshot must service a canonical class prefix"
        );
    }

    for plan in &result.plans {
        assert_eq!(
            plan.boundary_codimension,
            plan.parent_free_dimension - plan.effective_dimension
        );
        assert!(plan.completed_tasks <= plan.tasks);
        let task_ordinals = result
            .tasks
            .iter()
            .filter(|task| {
                task.before_revision == plan.revision
                    && task.effective_dimension == plan.effective_dimension
                    && task.parent_free_dimension == plan.parent_free_dimension
            })
            .map(|task| task.canonical_ordinal)
            .collect::<Vec<_>>();
        assert_eq!(task_ordinals, (0..plan.completed_tasks).collect::<Vec<_>>());
    }
}

fn free_dimension_histogram(partition: &UncoveredPartition) -> [usize; 7] {
    let mut histogram = [0usize; 7];
    for cell in partition.boxes() {
        histogram[cell.free_dimension()] += 1;
    }
    histogram
}

fn assert_exact_eighty_report_checkpoint(result: &WalkResult) {
    assert_eq!(result.stop_reason, StopReason::ReportCap);
    assert_eq!(result.tasks.len(), MAX_REPORTS);
    assert_eq!(result.outcomes.total(), MAX_REPORTS);
    assert_eq!(result.plans.len(), 20);
    assert_eq!(result.snapshots.len(), 10);
    assert_eq!(result.stale_siblings_rejected, 9);
    assert_eq!(
        result.outcomes,
        OutcomeHistogram {
            no_replayed_nominations: 33,
            no_rebased_circuits: 0,
            incomplete_proposal: 0,
            duplicate: 38,
            changed_without_geometric_shrink: 6,
            strict_geometric_shrink: 3,
            closed: 0,
        }
    );
    assert_eq!(
        result.snapshots.first(),
        Some(&SnapshotRecord {
            revision: 9,
            owner_count: 9,
            uncovered_box_count: 28,
            present_parent_dimensions: vec![5, 4],
        })
    );
    assert_eq!(
        result.snapshots.last(),
        Some(&SnapshotRecord {
            revision: 18,
            owner_count: 18,
            uncovered_box_count: 39,
            present_parent_dimensions: vec![5, 4, 3],
        })
    );
    assert_eq!(
        result
            .tasks
            .iter()
            .filter(|record| record.parent_free_dimension == 4)
            .count(),
        2
    );
    assert_eq!(
        result
            .tasks
            .iter()
            .filter(|record| record.parent_free_dimension == 3)
            .count(),
        0
    );
    assert!(
        result
            .tasks
            .iter()
            .all(|record| record.census.exact_obstructions() == 0)
    );

    let snapshot = result.ledger.snapshot();
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
    let partition = result.ledger.try_clone_uncovered_partition().unwrap();
    assert_eq!(
        free_dimension_histogram(&partition),
        [0, 0, 0, 17, 20, 2, 0]
    );
}

#[test]
fn k6_revision_nine_canonical_boundary_walk_is_revision_safe_and_bounded() {
    let first = run_walk(MAX_REPORTS);
    assert_canonical_schedule_prefix(&first);
    assert_exact_eighty_report_checkpoint(&first);
    let first_partition = first.ledger.try_clone_uncovered_partition().unwrap();

    let second = run_walk(MAX_REPORTS);
    assert_canonical_schedule_prefix(&second);
    assert_exact_eighty_report_checkpoint(&second);
    let second_partition = second.ledger.try_clone_uncovered_partition().unwrap();

    assert_eq!(second.snapshots, first.snapshots);
    assert_eq!(second.plans, first.plans);
    assert_eq!(second.tasks, first.tasks);
    assert_eq!(second.outcomes, first.outcomes);
    assert_eq!(
        second.stale_siblings_rejected,
        first.stale_siblings_rejected
    );
    assert_eq!(second.stop_reason, first.stop_reason);
    assert_eq!(second.ledger.snapshot(), first.ledger.snapshot());
    let first_owner_keys = first
        .ledger
        .owners()
        .iter()
        .map(|owner| owner.content_order_key())
        .collect::<Vec<_>>();
    let second_owner_keys = second
        .ledger
        .owners()
        .iter()
        .map(|owner| owner.content_order_key())
        .collect::<Vec<_>>();
    assert_eq!(second_owner_keys, first_owner_keys);
    assert_eq!(second_partition, first_partition);
}
