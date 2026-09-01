//! Oracle-disabled first-face execution after the proven K=6 orthant owner.

use std::collections::BTreeSet;

use crate::foundry::completion::LatticePoint;
use crate::foundry::completion::frame::admission::{
    ExactOwnerCoverObstructionKind, ExactOwnerCoverStatus,
};
use crate::foundry::completion::source_discovery::cover_delta::{
    CanonicalExactOwnerLedger, ExactOwnerCoverDeltaError, ExactOwnerCoverDeltaKind,
    ExactOwnerLedgerCoverStatus,
};
use crate::foundry::completion::source_discovery::test_fixtures::OracleDisabledK6Fixture;
use crate::foundry::completion::source_discovery::{
    CampaignModularProbe, OrdinarySourceIncidenceIndex,
};

use super::super::{
    ProbeCampaignAdapter, ProbeCampaignError, ProbeCampaignLimits, ProbeCampaignNoProposal,
    ProbeCampaignOutcome, ProbeCampaignOwnerEffect,
};

const PRIME: u64 = 1_000_000_007;

fn expected_slab(pivot: usize) -> (Vec<u64>, Vec<Option<u64>>) {
    let mut lower = vec![0; 6];
    lower[..pivot].fill(2);
    let mut upper = vec![None; 6];
    upper[pivot] = Some(1);
    (lower, upper)
}

fn probe(target: &[u64], limits: ProbeCampaignLimits) -> CampaignModularProbe {
    CampaignModularProbe::try_new(
        PRIME,
        [37],
        target.iter().copied(),
        limits.replay.scheduler.campaign,
    )
    .unwrap()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RecordedOutcome {
    NoReplayedNominations,
    NoRebasedCircuits,
    IncompleteProposal,
    Duplicate,
    ChangedWithoutGeometricShrink,
    StrictGeometricShrink,
    Closed(ProbeCampaignOwnerEffect),
}

#[derive(Debug, PartialEq, Eq)]
struct TaskRecord {
    canonical_ordinal: usize,
    target: Vec<u64>,
    finite_assignment_ordinal: usize,
    before_revision: u64,
    after_revision: u64,
    uncovered_boxes: usize,
    outcome: RecordedOutcome,
}

pub(super) fn classify_outcome(outcome: ProbeCampaignOutcome<'_>) -> RecordedOutcome {
    match outcome {
        ProbeCampaignOutcome::NoProposal(ProbeCampaignNoProposal::NoReplayedNominations) => {
            RecordedOutcome::NoReplayedNominations
        }
        ProbeCampaignOutcome::NoProposal(ProbeCampaignNoProposal::NoRebasedCircuits { .. }) => {
            RecordedOutcome::NoRebasedCircuits
        }
        ProbeCampaignOutcome::IncompleteProposal(_) => RecordedOutcome::IncompleteProposal,
        ProbeCampaignOutcome::Duplicate(_) => RecordedOutcome::Duplicate,
        ProbeCampaignOutcome::ChangedWithoutGeometricShrink(applied) => {
            assert_eq!(
                applied.delta().kind(),
                ExactOwnerCoverDeltaKind::ChangedWithoutGeometricShrink
            );
            RecordedOutcome::ChangedWithoutGeometricShrink
        }
        ProbeCampaignOutcome::StrictGeometricShrink(applied) => {
            assert_eq!(
                applied.delta().kind(),
                ExactOwnerCoverDeltaKind::StrictGeometricShrink
            );
            RecordedOutcome::StrictGeometricShrink
        }
        ProbeCampaignOutcome::Closed { effect, .. } => RecordedOutcome::Closed(effect),
    }
}

#[test]
fn k6_degree_zero_margin_two_first_face_campaign_is_revision_safe() {
    let fixture = OracleDisabledK6Fixture::shared();
    let mut ledger = fixture.new_ledger();
    let first_plan = fixture.plan(&ledger, 2, 0);
    let first_owner = fixture.replay_owner(&first_plan.tasks()[0]);
    let first_delta = ledger.try_apply_owner(first_owner).unwrap();
    assert_eq!(
        first_delta.kind(),
        ExactOwnerCoverDeltaKind::StrictGeometricShrink
    );
    assert_eq!(first_delta.updated().revision().get(), 1);
    assert_eq!(
        first_delta.updated().status(),
        ExactOwnerLedgerCoverStatus::Compiled(ExactOwnerCoverStatus::Incomplete(
            ExactOwnerCoverObstructionKind::NonFinite,
        ))
    );

    let partition = ledger.try_clone_uncovered_partition().unwrap();
    assert_eq!(partition.boxes().len(), 6);
    assert!(
        partition
            .boxes()
            .iter()
            .all(|cell| cell.free_dimension() == 5)
    );
    let mut actual_slabs = partition
        .boxes()
        .iter()
        .map(|cell| (cell.lower().to_vec(), cell.upper().to_vec()))
        .collect::<Vec<_>>();
    let mut expected_slabs = (0..6).map(expected_slab).collect::<Vec<_>>();
    actual_slabs.sort_unstable();
    expected_slabs.sort_unstable();
    assert_eq!(actual_slabs, expected_slabs);

    let face_plan = fixture.plan(&ledger, 2, 0);
    assert_eq!(face_plan.selected_box_count(), 6);
    assert_eq!(face_plan.finite_assignment_count(), 12);
    assert_eq!(face_plan.maximal_free_dimension(), 5);
    assert_eq!(face_plan.simplex_sample_count(), 1);
    assert_eq!(face_plan.tasks().len(), 12);
    for (ordinal, task) in face_plan.tasks().iter().enumerate() {
        assert_eq!(task.canonical_ordinal(), ordinal);
        assert_eq!(task.key().simplex_offset(), &[0; 5]);
        let pivot = task
            .key()
            .box_upper()
            .iter()
            .position(Option::is_some)
            .expect("each first residual slab has one finite axis");
        assert_eq!(
            (
                task.key().box_lower().to_vec(),
                task.key().box_upper().to_vec()
            ),
            expected_slab(pivot)
        );
        let assignment = task.key().finite_assignment_ordinal();
        assert!(assignment < 2);
        assert_eq!(task.lattice_target()[pivot], assignment as u64);
        let mut expected_target = vec![2; 6];
        expected_target[..pivot].fill(4);
        expected_target[pivot] = assignment as u64;
        assert_eq!(task.lattice_target(), expected_target);
        assert_eq!(assignment, usize::from(ordinal >= 6));
    }

    let limits = ProbeCampaignLimits::default();
    let incidence = OrdinarySourceIncidenceIndex::try_new(
        fixture.zero_sources(),
        limits.replay.scheduler.source_discovery,
    )
    .unwrap();
    let adapter =
        ProbeCampaignAdapter::try_new(fixture.generator(), fixture.completed(), &incidence, limits)
            .unwrap();
    let initial_targets = face_plan
        .tasks()
        .iter()
        .map(|task| task.lattice_target().to_vec())
        .collect::<Vec<_>>();
    let mut records = Vec::new();
    let first_owner = ledger.owners()[0].clone();
    for canonical_ordinal in 0..initial_targets.len() {
        let mut branch = fixture.new_ledger();
        let baseline = branch.try_apply_owner(first_owner.clone()).unwrap();
        assert_eq!(baseline.updated().revision().get(), 1);
        assert_eq!(baseline.updated().uncovered_box_count(), 6);
        let plan = fixture.plan(&branch, 2, 0);
        assert_eq!(plan.epoch_ordinal(), branch.revision().get());
        let task = &plan.tasks()[canonical_ordinal];
        assert_eq!(task.lattice_target(), initial_targets[canonical_ordinal]);
        let target = task.lattice_target().to_vec();
        let before_revision = branch.revision().get();
        let binding = adapter.try_bind_task(&plan, task, &branch).unwrap();
        let report = adapter
            .try_run_task(binding, &mut branch, [probe(&target, limits)])
            .unwrap();
        let outcome = classify_outcome(report.outcome());
        let after_revision = branch.revision().get();
        records.push(TaskRecord {
            canonical_ordinal,
            target,
            finite_assignment_ordinal: task.key().finite_assignment_ordinal(),
            before_revision,
            after_revision,
            uncovered_boxes: branch.snapshot().uncovered_box_count(),
            outcome,
        });

        // Any retained-owner mutation invalidates the frozen plan even if its
        // exact uncovered union did not shrink. Replan before another task can
        // be bound; the compiler remains the sole closure authority.
        if after_revision != before_revision {
            let replanned = fixture.plan(&branch, 2, 0);
            assert_eq!(replanned.epoch_ordinal(), after_revision);
        }
        assert!(!branch.snapshot().status().is_compiler_closed());
    }

    assert_eq!(records.len(), 12);
    for (ordinal, record) in records.iter().enumerate() {
        assert_eq!(record.canonical_ordinal, ordinal);
        assert_eq!(record.target, initial_targets[ordinal]);
        assert_eq!(record.finite_assignment_ordinal, usize::from(ordinal >= 6));
        assert_eq!(record.before_revision, 1);
        let expected = match ordinal {
            3 => RecordedOutcome::NoReplayedNominations,
            8 => RecordedOutcome::ChangedWithoutGeometricShrink,
            _ => RecordedOutcome::StrictGeometricShrink,
        };
        assert_eq!(record.outcome, expected);
        assert_eq!(record.after_revision, if ordinal == 3 { 1 } else { 2 });
        assert!(record.uncovered_boxes > 0);
    }
}

/// Construct and fully re-authenticate the deterministic revision-nine K=6
/// ledger used by subsequent proposal experiments. Keeping the complete
/// nineteen-report assertion here prevents downstream tests from treating a
/// hand-written owner set or serialized partition as discovery authority.
pub(super) fn asserted_revision_nine_ledger() -> CanonicalExactOwnerLedger {
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
    let mut ledger = fixture.new_ledger();
    let orthant_plan = fixture.plan(&ledger, 2, 0);
    let orthant_task = &orthant_plan.tasks()[0];
    let orthant_binding = adapter
        .try_bind_task(&orthant_plan, orthant_task, &ledger)
        .unwrap();
    let orthant_report = adapter
        .try_run_task(
            orthant_binding,
            &mut ledger,
            [probe(orthant_task.lattice_target(), limits)],
        )
        .unwrap();
    let ProbeCampaignOutcome::StrictGeometricShrink(orthant_applied) = orthant_report.outcome()
    else {
        panic!("the canonical orthant task must reproduce the proven first owner")
    };
    assert!(orthant_applied.obstructions().is_empty());
    assert_eq!(orthant_report.census().scheduler_outcomes().replayed(), 1);
    assert_eq!(
        orthant_report.census().scheduler_outcomes().budget_stop(),
        0
    );
    assert_eq!(orthant_report.census().scheduler_outcomes().rejected(), 0);
    assert_eq!(orthant_report.census().scheduler_outcomes().stalled(), 0);
    assert_eq!(
        orthant_report
            .census()
            .scheduler_outcomes()
            .exact_lift_error(),
        0
    );
    let first_face = fixture.plan(&ledger, 2, 0);
    let initial_targets = first_face
        .tasks()
        .iter()
        .map(|task| task.lattice_target().to_vec())
        .collect::<BTreeSet<_>>();
    assert_eq!(initial_targets.len(), 12);

    let mut unresolved = initial_targets.clone();
    let mut records = Vec::new();

    while !unresolved.is_empty() {
        let plan = fixture.plan(&ledger, 2, 0);
        assert_eq!(plan.epoch_ordinal(), ledger.revision().get());
        let mut mutated = false;
        for task_ordinal in 0..plan.tasks().len() {
            assert!(
                records.len() < 19,
                "the cumulative first-face regression exceeded its exact report cap"
            );
            let task = &plan.tasks()[task_ordinal];
            let delayed = plan.tasks().get(task_ordinal + 1).map(|next| {
                (
                    adapter.try_bind_task(&plan, next, &ledger).unwrap(),
                    next.lattice_target().to_vec(),
                )
            });
            let target = task.lattice_target().to_vec();
            if initial_targets.contains(&target) {
                unresolved.remove(&target);
            }
            let before_revision = ledger.revision().get();
            let binding = adapter.try_bind_task(&plan, task, &ledger).unwrap();
            let report = adapter
                .try_run_task(binding, &mut ledger, [probe(&target, limits)])
                .unwrap();
            let outcome = classify_outcome(report.outcome());
            let after_revision = ledger.revision().get();
            records.push(TaskRecord {
                canonical_ordinal: task.canonical_ordinal(),
                target,
                finite_assignment_ordinal: task.key().finite_assignment_ordinal(),
                before_revision,
                after_revision,
                uncovered_boxes: ledger.snapshot().uncovered_box_count(),
                outcome,
            });

            let partition = ledger.try_clone_uncovered_partition().unwrap();
            unresolved.retain(|target| {
                partition
                    .containing_box(&LatticePoint::try_new(target.iter().copied()).unwrap())
                    .is_some()
            });
            if after_revision != before_revision {
                if let Some((delayed, delayed_target)) = delayed {
                    assert!(matches!(
                        adapter.try_run_task(
                            delayed,
                            &mut ledger,
                            [probe(&delayed_target, limits)],
                        ),
                        Err(ProbeCampaignError::CoverDelta(
                            ExactOwnerCoverDeltaError::StaleLedgerSnapshotIdentity {
                                expected,
                                actual,
                            }
                        )) if expected.get() == after_revision && actual.get() == before_revision
                    ));
                }
                let replanned = fixture.plan(&ledger, 2, 0);
                assert_eq!(replanned.epoch_ordinal(), after_revision);
                mutated = true;
                break;
            }
        }
        if !mutated {
            break;
        }
    }

    assert_eq!(records.len(), 19);
    let expected_records = [
        (
            [0, 2, 2, 2, 2, 2],
            0,
            1,
            2,
            5,
            RecordedOutcome::StrictGeometricShrink,
        ),
        (
            [2, 0, 2, 2, 2, 2],
            0,
            2,
            3,
            9,
            RecordedOutcome::StrictGeometricShrink,
        ),
        (
            [2, 4, 0, 2, 2, 2],
            0,
            3,
            4,
            14,
            RecordedOutcome::StrictGeometricShrink,
        ),
        (
            [2, 4, 4, 0, 2, 2],
            0,
            4,
            4,
            14,
            RecordedOutcome::NoReplayedNominations,
        ),
        (
            [2, 4, 4, 4, 0, 2],
            0,
            4,
            5,
            19,
            RecordedOutcome::StrictGeometricShrink,
        ),
        (
            [2, 4, 4, 0, 2, 2],
            0,
            5,
            5,
            19,
            RecordedOutcome::NoReplayedNominations,
        ),
        (
            [2, 4, 4, 4, 4, 0],
            0,
            5,
            6,
            23,
            RecordedOutcome::StrictGeometricShrink,
        ),
        (
            [2, 4, 4, 0, 2, 2],
            0,
            6,
            6,
            23,
            RecordedOutcome::NoReplayedNominations,
        ),
        (
            [4, 6, 0, 2, 2, 2],
            0,
            6,
            7,
            23,
            RecordedOutcome::ChangedWithoutGeometricShrink,
        ),
        (
            [2, 4, 4, 0, 2, 2],
            0,
            7,
            7,
            23,
            RecordedOutcome::NoReplayedNominations,
        ),
        ([4, 6, 0, 2, 2, 2], 0, 7, 7, 23, RecordedOutcome::Duplicate),
        (
            [4, 6, 6, 6, 0, 2],
            0,
            7,
            8,
            23,
            RecordedOutcome::ChangedWithoutGeometricShrink,
        ),
        (
            [2, 4, 4, 0, 2, 2],
            0,
            8,
            8,
            23,
            RecordedOutcome::NoReplayedNominations,
        ),
        ([4, 6, 0, 2, 2, 2], 0, 8, 8, 23, RecordedOutcome::Duplicate),
        ([4, 6, 6, 6, 0, 2], 0, 8, 8, 23, RecordedOutcome::Duplicate),
        (
            [2, 4, 4, 1, 2, 2],
            1,
            8,
            9,
            28,
            RecordedOutcome::StrictGeometricShrink,
        ),
        ([4, 6, 0, 2, 2, 2], 0, 9, 9, 28, RecordedOutcome::Duplicate),
        (
            [4, 6, 6, 0, 2, 2],
            0,
            9,
            9,
            28,
            RecordedOutcome::NoReplayedNominations,
        ),
        ([4, 6, 6, 6, 0, 2], 0, 9, 9, 28, RecordedOutcome::Duplicate),
    ];
    for (record, (target, assignment, before, after, boxes, outcome)) in
        records.iter().zip(expected_records)
    {
        assert_eq!(record.target, target);
        assert_eq!(record.finite_assignment_ordinal, assignment);
        assert_eq!(record.before_revision, before);
        assert_eq!(record.after_revision, after);
        assert_eq!(record.uncovered_boxes, boxes);
        assert_eq!(record.outcome, outcome);
    }
    assert_eq!(
        records
            .iter()
            .filter(|record| record.outcome == RecordedOutcome::StrictGeometricShrink)
            .count(),
        6
    );
    assert_eq!(
        records
            .iter()
            .filter(|record| { record.outcome == RecordedOutcome::ChangedWithoutGeometricShrink })
            .count(),
        2
    );
    assert_eq!(
        records
            .iter()
            .filter(|record| record.outcome == RecordedOutcome::NoReplayedNominations)
            .count(),
        6
    );
    assert_eq!(
        records
            .iter()
            .filter(|record| record.outcome == RecordedOutcome::Duplicate)
            .count(),
        5
    );
    let snapshot = ledger.snapshot();
    assert_eq!(snapshot.revision().get(), 9);
    assert_eq!(snapshot.owner_count(), 9);
    assert_eq!(snapshot.terminal_count(), 1);
    assert_eq!(snapshot.uncovered_box_count(), 28);
    assert!(!snapshot.uncovered_is_finite());
    assert_eq!(snapshot.missing_terminal_count(), 0);
    assert_eq!(snapshot.guard_incomplete_owner_count(), 0);
    assert_eq!(
        snapshot.status(),
        ExactOwnerLedgerCoverStatus::Compiled(ExactOwnerCoverStatus::Incomplete(
            ExactOwnerCoverObstructionKind::NonFinite,
        ))
    );
    let final_partition = ledger.try_clone_uncovered_partition().unwrap();
    let still_uncovered = initial_targets
        .iter()
        .filter(|target| {
            final_partition
                .containing_box(&LatticePoint::try_new(target.iter().copied()).unwrap())
                .is_some()
        })
        .cloned()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        still_uncovered,
        BTreeSet::from([
            vec![4, 4, 0, 2, 2, 2],
            vec![4, 4, 4, 0, 2, 2],
            vec![4, 4, 4, 4, 0, 2],
        ])
    );
    let mut free_dimension_histogram = [0usize; 7];
    for cell in final_partition.boxes() {
        free_dimension_histogram[cell.free_dimension()] += 1;
    }
    assert_eq!(free_dimension_histogram, [0, 0, 0, 0, 25, 3, 0]);
    let global_maximum = final_partition
        .boxes()
        .iter()
        .map(|cell| cell.free_dimension())
        .max()
        .unwrap();
    assert_eq!(global_maximum, 5);
    let unresolved_cells = [
        (
            [4, 4, 0, 2, 2, 2],
            [2, 4, 0, 0, 0, 0],
            [None, None, Some(0), None, None, None],
            [4, 6, 0, 2, 2, 2],
        ),
        (
            [4, 4, 4, 0, 2, 2],
            [2, 4, 4, 0, 0, 0],
            [None, None, None, Some(0), None, None],
            [4, 6, 6, 0, 2, 2],
        ),
        (
            [4, 4, 4, 4, 0, 2],
            [2, 4, 4, 4, 0, 0],
            [None, None, None, None, Some(0), None],
            [4, 6, 6, 6, 0, 2],
        ),
    ];
    let replanned = fixture.plan(&ledger, 2, 0);
    assert_eq!(replanned.selected_free_dimension(), global_maximum);
    assert_eq!(replanned.tasks().len(), 3);
    for (point, lower, upper, representative) in unresolved_cells {
        assert!(still_uncovered.contains(point.as_slice()));
        let containing = final_partition
            .containing_box(&LatticePoint::try_new(point).unwrap())
            .expect("the bounded-inconclusive target must remain exactly uncovered");
        assert_eq!(containing.lower(), lower);
        assert_eq!(containing.upper(), upper);
        assert_eq!(containing.free_dimension(), global_maximum);
        assert!(
            replanned
                .tasks()
                .iter()
                .any(|task| task.lattice_target() == representative)
        );
        assert!(
            replanned
                .tasks()
                .iter()
                .all(|task| task.lattice_target() != point)
        );
    }
    ledger
}

#[test]
fn k6_cumulative_jit_replan_is_bounded_inconclusive_at_positive_margin() {
    let ledger = asserted_revision_nine_ledger();
    assert_eq!(ledger.revision().get(), 9);
}
