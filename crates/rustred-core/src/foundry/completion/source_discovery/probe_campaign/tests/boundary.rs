//! Boundary-planner authentication at the shared probe-campaign seam.

use crate::foundry::completion::source_discovery::boundary_simplex::{
    BoundarySimplexLimits, BoundarySimplexPlan, BoundarySimplexPlanError,
    BoundarySimplexSamplingProfile, BoundarySimplexScopePartition,
    try_plan_boundary_simplex_samples,
};
use crate::foundry::completion::source_discovery::cover_delta::{
    CanonicalExactOwnerLedger, ExactOwnerCoverDeltaError, ExactOwnerCoverDeltaKind,
};
use crate::foundry::completion::source_discovery::test_fixtures::OracleDisabledK6Fixture;
use crate::foundry::completion::{LatticeBox, LatticePoint, UncoveredPartition};
use crate::sector::Mask;

use super::super::{
    ProbeCampaignAdapter, ProbeCampaignError, ProbeCampaignLimits, ProbeCampaignOutcome,
};
use super::probe;

fn adapter() -> (
    ProbeCampaignAdapter<'static, 'static, 'static>,
    ProbeCampaignLimits,
) {
    let fixture = OracleDisabledK6Fixture::shared();
    let limits = ProbeCampaignLimits::default();
    (
        ProbeCampaignAdapter::try_new(
            fixture.generator(),
            fixture.completed(),
            fixture.zero_sources(),
            limits,
        )
        .unwrap(),
        limits,
    )
}

fn rev1_ledger(fixture: &OracleDisabledK6Fixture) -> CanonicalExactOwnerLedger {
    let mut ledger = fixture.new_ledger();
    let plan = fixture.plan(&ledger, 2, 0);
    let owner = fixture.replay_owner(&plan.tasks()[0]);
    let delta = ledger.try_apply_owner(owner).unwrap();
    assert_eq!(
        delta.kind(),
        ExactOwnerCoverDeltaKind::StrictGeometricShrink
    );
    assert_eq!(ledger.revision().get(), 1);
    assert_eq!(ledger.snapshot().uncovered_box_count(), 6);
    ledger
}

fn clone_partition_reversed(partition: &UncoveredPartition) -> UncoveredPartition {
    let boxes = partition
        .boxes()
        .iter()
        .rev()
        .map(|cell| {
            LatticeBox::try_new(cell.lower().iter().copied(), cell.upper().iter().copied()).unwrap()
        })
        .collect();
    UncoveredPartition::new(boxes, partition.split_operations())
}

fn boundary_plan(
    ledger: &CanonicalExactOwnerLedger,
    sector: &Mask,
    partition: &UncoveredPartition,
    codimension: usize,
) -> BoundarySimplexPlan {
    let scope = format!(
        "{}|{}|{}|{:?}|{:?}|{}|boundary-d5-c{}",
        ledger.predecessor_snapshot().family_fingerprint(),
        ledger.predecessor_snapshot().context_fingerprint(),
        ledger.predecessor_snapshot().id().as_str(),
        sector.active_bits(),
        ledger.ordering(),
        ledger.revision().get(),
        codimension,
    );
    try_plan_boundary_simplex_samples(
        ledger.revision().get(),
        [BoundarySimplexScopePartition::new(
            &scope, sector, partition,
        )],
        5,
        codimension,
        BoundarySimplexSamplingProfile::Simplex {
            interior_margin: 2,
            polynomial_degree_ceiling: 0,
        },
        BoundarySimplexLimits::default(),
    )
    .unwrap()
}

#[test]
fn boundary_task_executes_from_reversed_exact_parent_order_without_payload_cloning() {
    let fixture = OracleDisabledK6Fixture::shared();
    let (adapter, limits) = adapter();
    let mut ledger = rev1_ledger(fixture);
    let exact = ledger.try_clone_uncovered_partition().unwrap();
    let reversed = clone_partition_reversed(&exact);
    let plan = boundary_plan(&ledger, fixture.sector(), &reversed, 1);
    let task = &plan.tasks()[1];
    assert_eq!(task.lattice_target(), &[2, 0, 2, 2, 2, 2]);
    let binding = adapter.try_bind_task(&plan, task, &ledger).unwrap();
    assert!(std::ptr::eq(binding.task(), task));
    let report = adapter
        .try_run_task(
            binding,
            &mut ledger,
            [probe(task.lattice_target().iter().copied(), limits)],
        )
        .unwrap();
    assert_eq!(report.planned_ledger_revision().get(), 1);
    let ProbeCampaignOutcome::StrictGeometricShrink(applied) = report.outcome() else {
        panic!("the authenticated boundary task must produce its exact first shrink")
    };
    assert!(applied.obstructions().is_empty());
    assert_eq!(applied.delta().baseline().revision().get(), 1);
    assert_eq!(applied.delta().baseline().uncovered_box_count(), 6);
    assert_eq!(applied.delta().updated().revision().get(), 2);
    assert_eq!(applied.delta().updated().uncovered_box_count(), 5);
}

#[test]
fn boundary_binding_rejects_nonmember_parent_even_when_its_target_is_uncovered() {
    let fixture = OracleDisabledK6Fixture::shared();
    let (adapter, _) = adapter();
    let ledger = rev1_ledger(fixture);
    let exact = ledger.try_clone_uncovered_partition().unwrap();
    let parent = exact
        .boxes()
        .iter()
        .find(|cell| cell.free_dimension() == 5)
        .unwrap();
    let mut lower = parent.lower().to_vec();
    let free_axis = parent.upper().iter().position(Option::is_none).unwrap();
    lower[free_axis] += 1;
    let fake_parent = (lower, parent.upper().to_vec());
    let fake_partition = UncoveredPartition::new(
        vec![
            LatticeBox::try_new(fake_parent.0.iter().copied(), fake_parent.1.iter().copied())
                .unwrap(),
        ],
        0,
    );
    let plan = boundary_plan(&ledger, fixture.sector(), &fake_partition, 1);
    let task = &plan.tasks()[0];
    assert!(
        exact
            .containing_box(&LatticePoint::try_new(task.lattice_target().iter().copied()).unwrap())
            .is_some()
    );
    assert!(!exact.boxes().iter().any(|cell| {
        cell.lower() == task.key().parent_box_lower()
            && cell.upper() == task.key().parent_box_upper()
    }));
    let baseline = ledger.snapshot();
    assert!(matches!(
        adapter.try_bind_task(&plan, task, &ledger),
        Err(ProbeCampaignError::StaleParentGeometry)
    ));
    assert_eq!(ledger.snapshot(), baseline);

    let finite_axis = parent
        .upper()
        .iter()
        .position(Option::is_some)
        .expect("a first-shrink slab must retain one finite axis");
    let mut changed_upper = parent.upper().to_vec();
    changed_upper[finite_axis] = Some(parent.lower()[finite_axis]);
    assert_ne!(changed_upper, parent.upper());
    let upper_mismatch = UncoveredPartition::new(
        vec![
            LatticeBox::try_new(
                parent.lower().iter().copied(),
                changed_upper.iter().copied(),
            )
            .unwrap(),
        ],
        0,
    );
    let upper_plan = boundary_plan(&ledger, fixture.sector(), &upper_mismatch, 1);
    let upper_task = &upper_plan.tasks()[0];
    assert!(
        exact
            .containing_box(
                &LatticePoint::try_new(upper_task.lattice_target().iter().copied()).unwrap(),
            )
            .is_some()
    );
    assert!(matches!(
        adapter.try_bind_task(&upper_plan, upper_task, &ledger),
        Err(ProbeCampaignError::StaleParentGeometry)
    ));
    assert_eq!(ledger.snapshot(), baseline);
}

#[test]
fn boundary_binding_rejects_rebuilt_epoch_and_same_arity_foreign_sector() {
    let fixture = OracleDisabledK6Fixture::shared();
    let (adapter, _) = adapter();
    let ledger = rev1_ledger(fixture);
    let partition = ledger.try_clone_uncovered_partition().unwrap();
    let first = boundary_plan(&ledger, fixture.sector(), &partition, 1);
    let rebuilt = boundary_plan(&ledger, fixture.sector(), &partition, 1);
    let baseline = ledger.snapshot();
    assert!(matches!(
        adapter.try_bind_task(&rebuilt, &first.tasks()[0], &ledger),
        Err(ProbeCampaignError::BoundaryPlan(
            BoundarySimplexPlanError::StaleGeometryEpoch {
                expected_ordinal: 1,
                actual_ordinal: 1,
            }
        ))
    ));
    assert_eq!(ledger.snapshot(), baseline);

    let mut foreign_bits = fixture.sector().active_bits().to_vec();
    foreign_bits[0] = !foreign_bits[0];
    let foreign_sector = Mask::try_new(foreign_bits).unwrap();
    assert_eq!(foreign_sector.arity(), fixture.sector().arity());
    let foreign = boundary_plan(&ledger, &foreign_sector, &partition, 1);
    assert!(matches!(
        adapter.try_bind_task(&foreign, &foreign.tasks()[0], &ledger),
        Err(ProbeCampaignError::Scope {
            detail: "planned task and canonical ledger have different sectors",
        })
    ));
    assert_eq!(ledger.snapshot(), baseline);
}

#[test]
fn boundary_binding_rejects_foreign_ledger_and_every_owner_set_revision_change() {
    let fixture = OracleDisabledK6Fixture::shared();
    let (adapter, limits) = adapter();

    let source = rev1_ledger(fixture);
    let source_partition = source.try_clone_uncovered_partition().unwrap();
    let source_plan = boundary_plan(&source, fixture.sector(), &source_partition, 1);
    let source_task = &source_plan.tasks()[0];
    let foreign_binding = adapter
        .try_bind_task(&source_plan, source_task, &source)
        .unwrap();
    let mut foreign = rev1_ledger(fixture);
    let foreign_baseline = foreign.snapshot();
    assert!(matches!(
        adapter.try_run_task(
            foreign_binding,
            &mut foreign,
            [probe(source_task.lattice_target().iter().copied(), limits)],
        ),
        Err(ProbeCampaignError::CoverDelta(
            ExactOwnerCoverDeltaError::ForeignLedgerSnapshotIdentity,
        ))
    ));
    assert_eq!(foreign.snapshot(), foreign_baseline);

    let mut strict = rev1_ledger(fixture);
    let strict_partition = strict.try_clone_uncovered_partition().unwrap();
    let strict_boundary = boundary_plan(&strict, fixture.sector(), &strict_partition, 1);
    let strict_task = &strict_boundary.tasks()[0];
    let strict_binding = adapter
        .try_bind_task(&strict_boundary, strict_task, &strict)
        .unwrap();
    let strict_interior = fixture.plan(&strict, 2, 0);
    let strict_owner = fixture.replay_owner(&strict_interior.tasks()[0]);
    let strict_delta = strict.try_apply_owner(strict_owner).unwrap();
    assert_eq!(
        strict_delta.kind(),
        ExactOwnerCoverDeltaKind::StrictGeometricShrink
    );
    let strict_baseline = strict.snapshot();
    assert!(matches!(
        adapter.try_run_task(
            strict_binding,
            &mut strict,
            [probe(strict_task.lattice_target().iter().copied(), limits)],
        ),
        Err(ProbeCampaignError::CoverDelta(
            ExactOwnerCoverDeltaError::StaleLedgerSnapshotIdentity { expected, actual },
        )) if expected.get() == 2 && actual.get() == 1
    ));
    assert_eq!(strict.snapshot(), strict_baseline);

    let mut unchanged_cover = rev1_ledger(fixture);
    let unchanged_partition = unchanged_cover.try_clone_uncovered_partition().unwrap();
    let unchanged_boundary =
        boundary_plan(&unchanged_cover, fixture.sector(), &unchanged_partition, 1);
    let unchanged_task = &unchanged_boundary.tasks()[0];
    let unchanged_binding = adapter
        .try_bind_task(&unchanged_boundary, unchanged_task, &unchanged_cover)
        .unwrap();
    let unchanged_interior = fixture.plan(&unchanged_cover, 2, 0);
    let unchanged_owner = fixture.replay_owner(&unchanged_interior.tasks()[8]);
    let unchanged_delta = unchanged_cover.try_apply_owner(unchanged_owner).unwrap();
    assert_eq!(
        unchanged_delta.kind(),
        ExactOwnerCoverDeltaKind::ChangedWithoutGeometricShrink
    );
    let unchanged_baseline = unchanged_cover.snapshot();
    assert!(matches!(
        adapter.try_run_task(
            unchanged_binding,
            &mut unchanged_cover,
            [probe(
                unchanged_task.lattice_target().iter().copied(),
                limits,
            )],
        ),
        Err(ProbeCampaignError::CoverDelta(
            ExactOwnerCoverDeltaError::StaleLedgerSnapshotIdentity { expected, actual },
        )) if expected.get() == 2 && actual.get() == 1
    ));
    assert_eq!(unchanged_cover.snapshot(), unchanged_baseline);
}
