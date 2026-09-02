//! Boundary-planner authentication at the shared probe-campaign seam.

use crate::family::IntegralKey;
use crate::foundry::cell::SourceViewConstruction;
use crate::foundry::completion::frame::admission::ExactCircuitOuterExtensionWitness;
use crate::foundry::completion::source_discovery::boundary_simplex::{
    BoundarySimplexLimits, BoundarySimplexPlan, BoundarySimplexPlanError,
    BoundarySimplexSamplingProfile, BoundarySimplexScopePartition,
    try_plan_boundary_simplex_samples,
};
use crate::foundry::completion::source_discovery::cover_delta::{
    CanonicalExactOwnerLedger, ExactOwnerCoverDeltaError, ExactOwnerCoverDeltaKind,
};
use crate::foundry::completion::source_discovery::test_fixtures::OracleDisabledK6Fixture;
use crate::foundry::completion::source_discovery::{
    ExactExecutableOwnerCover, ExactExecutableOwnerLimits, ExactExecutableOwnerProposal,
    ExactExecutableOwnerSelection, InteriorReplayRunDisposition, try_run_interior_replay_task,
};
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
            [probe(task.base_probe_chart_origin(), limits)],
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
    // The exact one-dimensional cylinder removes measure from the cover while
    // preserving the complementary boundary faces as disjoint boxes.
    assert_eq!(applied.delta().updated().uncovered_box_count(), 15);
}

#[test]
fn k6_boundary_adapter_replays_the_exact_mixed_dot_ray_quotient_and_owner_region() {
    let fixture = OracleDisabledK6Fixture::shared();
    let (adapter, limits) = adapter();
    let sector = Mask::try_new([false, true, true, true, true, false]).unwrap();
    let parent = LatticeBox::try_new(
        [0, 0, 1, 1, 0, 0],
        [Some(0), Some(0), Some(1), Some(1), None, Some(0)],
    )
    .unwrap();
    let partition = UncoveredPartition::new(vec![parent], 0);
    let plan = try_plan_boundary_simplex_samples(
        0,
        [BoundarySimplexScopePartition::new(
            "k6-mixed-dot-ray",
            &sector,
            &partition,
        )],
        1,
        0,
        BoundarySimplexSamplingProfile::Simplex {
            interior_margin: 2,
            polynomial_degree_ceiling: 0,
        },
        BoundarySimplexLimits::default(),
    )
    .unwrap();
    assert_eq!(plan.tasks().len(), 1);
    let task = &plan.tasks()[0];
    assert_eq!(task.key().remaining_axes(), &[4]);
    assert_eq!(task.lattice_target(), &[0, 0, 1, 1, 2, 0]);
    assert_eq!(task.target_shift().values(), &[0, 0, 1, 1, 2, 0]);
    assert_eq!(
        task.base_probe_chart_origin().collect::<Vec<_>>(),
        [0, 0, 0, 0, 1, 0]
    );

    let (_, anchor) = adapter.try_build_anchor_for_test(task).unwrap();
    let expected_fixed = [(0, 0), (1, 1), (2, 1), (3, 1), (5, 0)];
    assert_eq!(
        anchor
            .initial()
            .domain()
            .bounds()
            .iter()
            .enumerate()
            .filter_map(|(position, bounds)| {
                (bounds.lower() == bounds.upper()).then_some((position, bounds.lower()))
            })
            .collect::<Vec<_>>(),
        expected_fixed
    );

    let replay = try_run_interior_replay_task(
        fixture.generator(),
        fixture.completed(),
        task.target_shift().clone(),
        anchor,
        fixture.predecessor().clone(),
        fixture.new_ledger().ordering(),
        [probe(task.base_probe_chart_origin(), limits)],
        limits.replay,
    )
    .unwrap();
    let InteriorReplayRunDisposition::OwnerProposal {
        proposal:
            ExactExecutableOwnerProposal::Compiled {
                owner,
                obstructions,
            },
        ..
    } = replay.disposition()
    else {
        panic!("the automatic mixed-dot boundary replay must compile an exact owner")
    };
    assert!(obstructions.is_empty());
    assert!(!owner.executable_candidates().is_empty());
    for candidate in owner.executable_candidates() {
        assert_eq!(candidate.circuit().fixed_indices(), expected_fixed);
        let SourceViewConstruction::FixedIndexSpecialization(evidence) =
            candidate.cell().sources().construction()
        else {
            panic!("the mixed-dot boundary rule must retain fixed-index specialization evidence")
        };
        assert_eq!(
            evidence
                .fixed_restrictions()
                .iter()
                .map(|restriction| (restriction.position(), restriction.value()))
                .collect::<Vec<_>>(),
            expected_fixed
        );
    }
    let fixed_cells = owner
        .executable_candidates()
        .iter()
        .map(|candidate| candidate.cell_owner().clone())
        .collect::<Vec<_>>();
    crate::foundry::artifact::authenticate_k6_rule_cell_sources_for_test(
        fixture.generator(),
        fixture.completed(),
        &fixed_cells,
    )
    .unwrap();

    let epoch = owner.executable_candidates()[0].epoch();
    let target_partition = epoch
        .try_partition(limits.replay.scheduler.campaign.stratum)
        .unwrap();
    let extension =
        ExactCircuitOuterExtensionWitness::try_prove(&target_partition, owner.semantic().clone())
            .unwrap();
    assert_eq!(extension.region().lower(), [0, 0, 1, 1, 2, 0]);
    assert_eq!(
        extension.region().upper(),
        [
            Some(0),
            Some(0),
            Some(1),
            Some(1),
            Some(i64::MAX as u64 - 3),
            Some(0)
        ]
    );

    let cover = ExactExecutableOwnerCover::try_compile(
        fixture.generator().context(),
        vec![owner.clone()],
        Vec::new(),
        ExactExecutableOwnerLimits::default(),
    )
    .unwrap();
    let maximal_owned = IntegralKey::try_new([0, 1, 2, 2, i64::MAX - 2, 0]).unwrap();
    assert!(matches!(
        cover
            .try_select_at(
                fixture.generator().context(),
                &maximal_owned,
                Default::default(),
            )
            .unwrap(),
        ExactExecutableOwnerSelection::Descending { cell, .. }
            if cell.assignment_for_target(&maximal_owned).unwrap().is_some()
    ));
    let beyond_executable = IntegralKey::try_new([0, 1, 2, 2, i64::MAX - 1, 0]).unwrap();
    assert!(matches!(
        cover
            .try_select_at(
                fixture.generator().context(),
                &beyond_executable,
                Default::default(),
            )
            .unwrap(),
        ExactExecutableOwnerSelection::Incomplete
    ));
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
            [probe(source_task.base_probe_chart_origin(), limits)],
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
    let strict_mutating_task = &strict_interior.tasks()[0];
    let strict_mutating_binding = adapter
        .try_bind_task(&strict_interior, strict_mutating_task, &strict)
        .unwrap();
    let strict_mutating_report = adapter
        .try_run_task(
            strict_mutating_binding,
            &mut strict,
            [probe(
                strict_mutating_task.lattice_target().iter().copied(),
                limits,
            )],
        )
        .unwrap();
    let ProbeCampaignOutcome::StrictGeometricShrink(strict_applied) =
        strict_mutating_report.outcome()
    else {
        panic!("the canonical first boundary task must strictly shrink geometry")
    };
    assert_eq!(
        strict_applied.delta().kind(),
        ExactOwnerCoverDeltaKind::StrictGeometricShrink
    );
    let strict_baseline = strict.snapshot();
    assert!(matches!(
        adapter.try_run_task(
            strict_binding,
            &mut strict,
            [probe(strict_task.base_probe_chart_origin(), limits)],
        ),
        Err(ProbeCampaignError::CoverDelta(
            ExactOwnerCoverDeltaError::StaleLedgerSnapshotIdentity { expected, actual },
        )) if expected.get() == 2 && actual.get() == 1
    ));
    assert_eq!(strict.snapshot(), strict_baseline);

    // Exact product-domain preimages make the former ordinal-seven redundant
    // owner a genuine geometric shrink. The stale-binding invariant is about
    // the owner-set revision identity, not the historical delta shape, so
    // authenticate it on this distinct mutation as well.
    let mut second_change = rev1_ledger(fixture);
    let second_partition = second_change.try_clone_uncovered_partition().unwrap();
    let second_boundary = boundary_plan(&second_change, fixture.sector(), &second_partition, 1);
    let second_task = &second_boundary.tasks()[0];
    let second_binding = adapter
        .try_bind_task(&second_boundary, second_task, &second_change)
        .unwrap();
    let second_interior = fixture.plan(&second_change, 2, 0);
    let mutating_task = &second_interior.tasks()[7];
    let mutating_binding = adapter
        .try_bind_task(&second_interior, mutating_task, &second_change)
        .unwrap();
    let mutating_report = adapter
        .try_run_task(
            mutating_binding,
            &mut second_change,
            [probe(
                mutating_task.lattice_target().iter().copied(),
                limits,
            )],
        )
        .unwrap();
    let ProbeCampaignOutcome::StrictGeometricShrink(mutating_applied) = mutating_report.outcome()
    else {
        panic!("the exact-preimage ordinal-seven task must strictly shrink geometry")
    };
    assert_eq!(
        mutating_applied.delta().kind(),
        ExactOwnerCoverDeltaKind::StrictGeometricShrink
    );
    let second_baseline = second_change.snapshot();
    assert!(matches!(
        adapter.try_run_task(
            second_binding,
            &mut second_change,
            [probe(second_task.base_probe_chart_origin(), limits)],
        ),
        Err(ProbeCampaignError::CoverDelta(
            ExactOwnerCoverDeltaError::StaleLedgerSnapshotIdentity { expected, actual },
        )) if expected.get() == 2 && actual.get() == 1
    ));
    assert_eq!(second_change.snapshot(), second_baseline);
}
