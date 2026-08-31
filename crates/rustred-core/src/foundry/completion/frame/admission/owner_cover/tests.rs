use std::sync::Arc;

use crate::algebra::{CoefficientContext, IndexedCoefficientContext};
use crate::family::{AffineDenominator, IntegralFamily, IntegralKey};
use crate::foundry::completion::frame::exact::{
    ExactCircuitLift, ExactCircuitLimits, ExactTargetCircuit, try_lift_exact_circuit,
};
use crate::foundry::completion::frame::modular::{ModularKernelLimits, ModularTargetQuery};
use crate::foundry::completion::frame::{
    OneSidedChartFrame, PhysicalFrameLimits, PhysicalFramePlan,
};
use crate::foundry::completion::guard::{CoefficientIdealGuardAtom, CoefficientIdealGuardLimits};
use crate::foundry::completion::stratum::{
    DecoratedStratum, GuardBranch, GuardBranchIdentity, ImmutableOwnerSnapshot,
    StratumRegistryLimits, TargetColumnPartition,
};
use crate::identity::{CompletedIbpSourceRows, ParametricIbpGenerator};
use crate::sector::{InteriorBounds, Mask, OrderingPolicy, SectorMonotoneDomain};

use super::super::{ExactCircuitSemanticDag, ExactCircuitSemanticLimits};
use super::{
    ExactCircuitOuterExtensionError, ExactCircuitOuterExtensionWitness, ExactCircuitOwnerCover,
    ExactCircuitOwnerCoverError, ExactCircuitOwnerCoverLimits, ExactCircuitOwnerInput,
    ExactOwnerCoverObstructionKind, ExactOwnerCoverSelection, ExactOwnerCoverStatus,
};

const PRIME: u64 = 1_000_000_007;

fn complete_ordinary(generator: &ParametricIbpGenerator<'_>) -> CompletedIbpSourceRows {
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    prepared.complete(rows).unwrap()
}

fn tadpole_frame_at_degree(degree: usize) -> (IndexedCoefficientContext, PhysicalFramePlan) {
    let base = CoefficientContext::new(["d"]);
    let family = IntegralFamily::new(
        "owner-cover-tadpole",
        vec!["k".to_owned()],
        Vec::new(),
        base.clone(),
        base.parameter("d").unwrap(),
        vec![AffineDenominator::new(base.integer(-1), vec![base.one()])],
        Vec::new(),
        vec![base.zero()],
    )
    .unwrap();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let context = generator.context().clone();
    let completed = complete_ordinary(&generator);
    let frame = OneSidedChartFrame::try_new(
        &generator,
        &completed,
        Mask::try_new([true]).unwrap(),
        degree,
        PhysicalFrameLimits::default(),
    )
    .unwrap()
    .into_plan();
    (context, frame)
}

fn tadpole_frame() -> (IndexedCoefficientContext, PhysicalFramePlan) {
    tadpole_frame_at_degree(0)
}

fn k6_s4a_frame() -> (IndexedCoefficientContext, PhysicalFramePlan) {
    let family = crate::foundry::artifact::canonical_three_loop_family().unwrap();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let context = generator.context().clone();
    let completed = complete_ordinary(&generator);
    let frame = OneSidedChartFrame::try_new(
        &generator,
        &completed,
        Mask::try_new([false, true, true, true, true, false]).unwrap(),
        1,
        PhysicalFrameLimits::default(),
    )
    .unwrap()
    .into_plan();
    (context, frame)
}

fn target_with_shift(frame: &PhysicalFramePlan, shift: &[i64]) -> usize {
    frame
        .columns()
        .iter()
        .position(|candidate| candidate.values() == shift)
        .unwrap()
}

fn partition_at(frame: &PhysicalFramePlan, target: usize) -> TargetColumnPartition<'_> {
    let limits = StratumRegistryLimits::default();
    let shifts = frame
        .columns()
        .iter()
        .map(|shift| shift.values())
        .collect::<Vec<_>>();
    let domain = SectorMonotoneDomain::try_maximal_for_rule(
        frame.sector().clone(),
        frame.columns()[target].values(),
        &shifts,
    )
    .unwrap();
    let stratum = DecoratedStratum::try_guard_blind(
        frame.family_fingerprint(),
        frame.context_fingerprint(),
        domain,
        limits,
    )
    .unwrap();
    let owners = ImmutableOwnerSnapshot::try_empty(
        frame.family_fingerprint(),
        frame.context_fingerprint(),
        frame.sector().arity(),
        limits,
    )
    .unwrap();
    partition_at_with(frame, target, stratum, owners)
}

fn partition_at_with<'frame>(
    frame: &'frame PhysicalFramePlan,
    target: usize,
    stratum: DecoratedStratum,
    owners: ImmutableOwnerSnapshot,
) -> TargetColumnPartition<'frame> {
    let limits = StratumRegistryLimits::default();
    TargetColumnPartition::try_new(
        frame,
        target,
        stratum,
        owners,
        OrderingPolicy::default(),
        limits,
    )
    .unwrap()
}

fn partition(frame: &PhysicalFramePlan) -> TargetColumnPartition<'_> {
    partition_at(frame, target_with_shift(frame, &[1]))
}

fn partition_with(
    frame: &PhysicalFramePlan,
    domain: SectorMonotoneDomain,
    guards: impl IntoIterator<Item = GuardBranchIdentity>,
) -> TargetColumnPartition<'_> {
    let target = frame
        .columns()
        .iter()
        .position(|shift| shift.values() == [1])
        .unwrap();
    let limits = StratumRegistryLimits::default();
    let stratum = DecoratedStratum::try_new(
        frame.family_fingerprint(),
        frame.context_fingerprint(),
        domain,
        guards,
        limits,
    )
    .unwrap();
    let owners = ImmutableOwnerSnapshot::try_empty(
        frame.family_fingerprint(),
        frame.context_fingerprint(),
        frame.sector().arity(),
        limits,
    )
    .unwrap();
    TargetColumnPartition::try_new(
        frame,
        target,
        stratum,
        owners,
        OrderingPolicy::default(),
        limits,
    )
    .unwrap()
}

fn exact_circuit(
    context: &IndexedCoefficientContext,
    frame: &PhysicalFramePlan,
    partition: &TargetColumnPartition<'_>,
) -> ExactTargetCircuit {
    let sample = frame
        .try_modular_sample(context, PRIME, &[37], &[2], ModularKernelLimits::default())
        .unwrap();
    let ModularTargetQuery::Hit(hit) = sample
        .query_target(
            partition.target_column(),
            partition.forbidden_columns(),
            ModularKernelLimits::default(),
        )
        .unwrap()
    else {
        panic!("the tadpole target must have a modular hit")
    };
    let ExactCircuitLift::Replayed(circuit) =
        try_lift_exact_circuit(context, &hit, partition, ExactCircuitLimits::default()).unwrap()
    else {
        panic!("the tadpole target must replay exactly")
    };
    circuit
}

fn k6_s4a_exact_circuit(
    context: &IndexedCoefficientContext,
    frame: &PhysicalFramePlan,
    partition: &TargetColumnPartition<'_>,
) -> ExactTargetCircuit {
    let sample = frame
        .try_modular_sample(
            context,
            PRIME,
            &[2],
            &[1, 2, 3, 4, 5, 6],
            ModularKernelLimits::default(),
        )
        .unwrap();
    let ModularTargetQuery::Hit(hit) = sample
        .query_target(
            partition.target_column(),
            partition.forbidden_columns(),
            ModularKernelLimits::default(),
        )
        .unwrap()
    else {
        panic!("the canonical S4a target must have a modular hit")
    };
    let ExactCircuitLift::Replayed(circuit) =
        try_lift_exact_circuit(context, &hit, partition, ExactCircuitLimits::default()).unwrap()
    else {
        panic!("the canonical S4a modular support must replay exactly")
    };
    circuit
}

/// Build a deliberately narrower, but still sound, semantic router for one
/// exact circuit. Every original circuit guard is retained; each candidate
/// adds one exact target-coordinate guard `n_0 - root != 0`. This test-only
/// seam exercises partial-DAG cover behavior without fabricating an identity.
fn semantic_with_extra_roots(
    context: &IndexedCoefficientContext,
    partition: &TargetColumnPartition<'_>,
    circuit: Arc<ExactTargetCircuit>,
    roots: &[i64],
) -> Arc<ExactCircuitSemanticDag> {
    let baseline = ExactCircuitSemanticDag::try_compile(
        context,
        partition,
        std::slice::from_ref(&circuit),
        ExactCircuitSemanticLimits::default(),
    )
    .unwrap();
    assert_eq!(baseline.candidates().len(), 1);

    let mut candidates = Vec::new();
    for &root in roots {
        let guard = context
            .sub(&context.index(0).unwrap(), &context.integer(root))
            .unwrap();
        let polynomial = context
            .numerator_condition_with_limits(&guard, Default::default())
            .unwrap();
        let atom = CoefficientIdealGuardAtom::try_from_pulled_back(
            context,
            polynomial,
            CoefficientIdealGuardLimits::default(),
        )
        .unwrap();
        let mut atoms = baseline.candidates()[0].guard_atoms().to_vec();
        atoms.push(atom);
        candidates.push((circuit.clone(), atoms));
    }
    Arc::new(
        ExactCircuitSemanticDag::try_from_test_candidates(context, &baseline, candidates).unwrap(),
    )
}

fn semantic(
    context: &IndexedCoefficientContext,
    partition: &TargetColumnPartition<'_>,
    circuit: Arc<ExactTargetCircuit>,
) -> Arc<ExactCircuitSemanticDag> {
    Arc::new(
        ExactCircuitSemanticDag::try_compile(
            context,
            partition,
            &[circuit],
            ExactCircuitSemanticLimits::default(),
        )
        .unwrap(),
    )
}

#[test]
fn outer_extension_cannot_be_rebound_to_an_equal_independent_physical_plan() {
    let (context, first_frame) = tadpole_frame();
    let (_, second_frame) = tadpole_frame();
    assert_eq!(first_frame, second_frame);
    assert!(!std::ptr::eq(&first_frame, &second_frame));

    let first_partition = partition(&first_frame);
    let second_partition = partition(&second_frame);
    assert_eq!(
        first_partition.target_column(),
        second_partition.target_column()
    );
    let circuit = Arc::new(exact_circuit(&context, &first_frame, &first_partition));
    let semantic = semantic(&context, &first_partition, circuit.clone());
    assert!(matches!(
        ExactCircuitOuterExtensionWitness::try_prove(&second_partition, semantic.clone()),
        Err(ExactCircuitOuterExtensionError::WrongPhysicalPlan)
    ));
    let test_semantic = semantic_with_extra_roots(&context, &first_partition, circuit, &[0, 1]);
    assert!(matches!(
        ExactCircuitOuterExtensionWitness::try_prove(&second_partition, test_semantic),
        Err(ExactCircuitOuterExtensionError::WrongPhysicalPlan)
    ));
    let outer = ExactCircuitOuterExtensionWitness::try_prove(&first_partition, semantic).unwrap();

    assert!(matches!(
        ExactCircuitOwnerCover::try_compile(
            &context,
            [ExactCircuitOwnerInput::new(&second_partition, outer)],
            Vec::<IntegralKey>::new(),
            Default::default(),
        ),
        Err(ExactCircuitOwnerCoverError::OwnerJoin {
            owner: 0,
            detail: "outer-extension witness differs from its exact physical plan or target partition",
        })
    ));
}

#[test]
fn finite_tail_is_not_closed_until_its_terminal_is_explicit() {
    let (context, frame) = tadpole_frame();
    let partition = partition(&frame);
    let circuit = Arc::new(exact_circuit(&context, &frame, &partition));
    let semantic = Arc::new(
        ExactCircuitSemanticDag::try_compile(
            &context,
            &partition,
            &[circuit.clone()],
            ExactCircuitSemanticLimits::default(),
        )
        .unwrap(),
    );
    assert!(!semantic.guard_dag().is_abstractly_total());

    let incomplete_extension =
        ExactCircuitOuterExtensionWitness::try_prove(&partition, semantic.clone()).unwrap();
    let incomplete = ExactCircuitOwnerCover::try_compile(
        &context,
        [ExactCircuitOwnerInput::new(
            &partition,
            incomplete_extension,
        )],
        Vec::<IntegralKey>::new(),
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        incomplete.status(),
        ExactOwnerCoverStatus::Incomplete(ExactOwnerCoverObstructionKind::FiniteTerminalOwnership)
    );
    assert_eq!(incomplete.owners().len(), 1);
    assert!(incomplete.owners()[0].is_guard_total());
    assert_eq!(incomplete.owners()[0].leading().coordinates(), [1]);
    assert_eq!(incomplete.missing_terminals().len(), 1);
    assert_eq!(incomplete.missing_terminals()[0].coordinates(), [0]);

    let closed_extension =
        ExactCircuitOuterExtensionWitness::try_prove(&partition, semantic).unwrap();
    let closed = ExactCircuitOwnerCover::try_compile(
        &context,
        [ExactCircuitOwnerInput::new(&partition, closed_extension)],
        [IntegralKey::try_new([1]).unwrap()],
        Default::default(),
    )
    .unwrap();
    assert_eq!(closed.status(), ExactOwnerCoverStatus::Closed);
    assert_eq!(closed.terminals().len(), 1);
    assert!(closed.missing_terminals().is_empty());
    assert!(closed.guard_incomplete_owners().is_empty());

    let terminal = IntegralKey::try_new([1]).unwrap();
    assert!(matches!(
        closed
            .try_select_at(&context, &terminal, Default::default())
            .unwrap(),
        ExactOwnerCoverSelection::Terminal(owner) if owner.integral() == &terminal
    ));
    let reducible = IntegralKey::try_new([2]).unwrap();
    let ExactOwnerCoverSelection::Descending { owner, candidate } = closed
        .try_select_at(&context, &reducible, Default::default())
        .unwrap()
    else {
        panic!("the first dotted tadpole must select its exact recurrence")
    };
    assert_eq!(owner.id().ordinal(), 0);
    assert!(Arc::ptr_eq(candidate.circuit(), &circuit));
}

#[test]
fn explicit_terminals_cannot_overlap_an_exactly_selected_rule() {
    let (context, frame) = tadpole_frame();
    let partition = partition(&frame);
    let circuit = Arc::new(exact_circuit(&context, &frame, &partition));
    let semantic = Arc::new(
        ExactCircuitSemanticDag::try_compile(
            &context,
            &partition,
            &[circuit],
            ExactCircuitSemanticLimits::default(),
        )
        .unwrap(),
    );
    let extension = ExactCircuitOuterExtensionWitness::try_prove(&partition, semantic).unwrap();
    assert!(matches!(
        ExactCircuitOwnerCover::try_compile(
            &context,
            [ExactCircuitOwnerInput::new(&partition, extension)],
            [IntegralKey::try_new([2]).unwrap()],
            Default::default(),
        ),
        Err(
            super::ExactCircuitOwnerCoverError::TerminalOverlapsDescendingOwner {
                terminal: 0,
                owner: 0,
            }
        )
    ));
}

#[test]
fn partial_owners_are_used_pointwise_and_incomplete_points_may_be_terminals() {
    let (context, frame) = tadpole_frame_at_degree(1);
    let first_partition = partition_at(&frame, target_with_shift(&frame, &[1]));
    let second_partition = partition_at(&frame, target_with_shift(&frame, &[2]));
    let first_circuit = Arc::new(exact_circuit(&context, &frame, &first_partition));
    let second_circuit = Arc::new(exact_circuit(&context, &frame, &second_partition));
    let second_semantic = semantic(&context, &second_partition, second_circuit.clone());

    // The extra wall at I(3) leaves I(2) exactly selected by the first rule.
    // The guard-total I(n+2) recurrence certifies the infinite tail, while the
    // partial I(n+1) recurrence discharges one point of its finite complement.
    let selected_partial =
        semantic_with_extra_roots(&context, &first_partition, first_circuit.clone(), &[3]);
    let selected_cover = ExactCircuitOwnerCover::try_compile(
        &context,
        [
            ExactCircuitOwnerInput::new(
                &first_partition,
                ExactCircuitOuterExtensionWitness::try_prove(&first_partition, selected_partial)
                    .unwrap(),
            ),
            ExactCircuitOwnerInput::new(
                &second_partition,
                ExactCircuitOuterExtensionWitness::try_prove(
                    &second_partition,
                    second_semantic.clone(),
                )
                .unwrap(),
            ),
        ],
        [IntegralKey::try_new([1]).unwrap()],
        Default::default(),
    )
    .unwrap();
    assert_eq!(selected_cover.status(), ExactOwnerCoverStatus::Closed);
    assert_eq!(selected_cover.finite_point_owners().len(), 1);
    let point_owner = &selected_cover.finite_point_owners()[0];
    assert_eq!(point_owner.point().coordinates(), [1]);
    assert!(Arc::ptr_eq(point_owner.circuit(), &first_circuit));

    // On the wall at I(2), the partial DAG returns Incomplete. That exact
    // point may therefore be declared terminal even though its orthant
    // overlaps the partial owner geometrically.
    let incomplete_partial =
        semantic_with_extra_roots(&context, &first_partition, first_circuit.clone(), &[2]);
    let terminal_cover = ExactCircuitOwnerCover::try_compile(
        &context,
        [
            ExactCircuitOwnerInput::new(
                &first_partition,
                ExactCircuitOuterExtensionWitness::try_prove(&first_partition, incomplete_partial)
                    .unwrap(),
            ),
            ExactCircuitOwnerInput::new(
                &second_partition,
                ExactCircuitOuterExtensionWitness::try_prove(
                    &second_partition,
                    second_semantic.clone(),
                )
                .unwrap(),
            ),
        ],
        [
            IntegralKey::try_new([1]).unwrap(),
            IntegralKey::try_new([2]).unwrap(),
        ],
        Default::default(),
    )
    .unwrap();
    assert_eq!(terminal_cover.status(), ExactOwnerCoverStatus::Closed);
    assert!(terminal_cover.finite_point_owners().is_empty());
    let wall = IntegralKey::try_new([2]).unwrap();
    assert!(matches!(
        terminal_cover
            .try_select_at(&context, &wall, Default::default())
            .unwrap(),
        ExactOwnerCoverSelection::Terminal(owner) if owner.integral() == &wall
    ));

    let overlapping_partial =
        semantic_with_extra_roots(&context, &first_partition, first_circuit, &[2]);
    assert!(matches!(
        ExactCircuitOwnerCover::try_compile(
            &context,
            [
                ExactCircuitOwnerInput::new(
                    &first_partition,
                    ExactCircuitOuterExtensionWitness::try_prove(
                        &first_partition,
                        overlapping_partial,
                    )
                    .unwrap(),
                ),
                ExactCircuitOwnerInput::new(
                    &second_partition,
                    ExactCircuitOuterExtensionWitness::try_prove(
                        &second_partition,
                        second_semantic,
                    )
                    .unwrap(),
                ),
            ],
            [IntegralKey::try_new([3]).unwrap()],
            Default::default(),
        ),
        Err(ExactCircuitOwnerCoverError::TerminalOverlapsDescendingOwner { .. })
    ));
}

#[test]
fn total_owner_dominance_removes_partial_obligations_and_input_order_is_stable() {
    let (context, frame) = tadpole_frame_at_degree(1);
    let first_partition = partition_at(&frame, target_with_shift(&frame, &[1]));
    let second_partition = partition_at(&frame, target_with_shift(&frame, &[2]));
    let first_circuit = Arc::new(exact_circuit(&context, &frame, &first_partition));
    let second_circuit = Arc::new(exact_circuit(&context, &frame, &second_partition));
    let total = semantic(&context, &first_partition, first_circuit);
    let dominated_partial =
        semantic_with_extra_roots(&context, &second_partition, second_circuit, &[3]);

    let compile = |reverse: bool| {
        let first = ExactCircuitOwnerInput::new(
            &first_partition,
            ExactCircuitOuterExtensionWitness::try_prove(&first_partition, total.clone()).unwrap(),
        );
        let second = ExactCircuitOwnerInput::new(
            &second_partition,
            ExactCircuitOuterExtensionWitness::try_prove(
                &second_partition,
                dominated_partial.clone(),
            )
            .unwrap(),
        );
        ExactCircuitOwnerCover::try_compile(
            &context,
            if reverse {
                vec![second, first]
            } else {
                vec![first, second]
            },
            [IntegralKey::try_new([1]).unwrap()],
            Default::default(),
        )
        .unwrap()
    };
    let forward = compile(false);
    let reverse = compile(true);
    for cover in [&forward, &reverse] {
        assert_eq!(cover.status(), ExactOwnerCoverStatus::Closed);
        assert!(cover.guard_incomplete_owners().is_empty());
        assert_eq!(cover.owners().len(), 2);
        assert!(cover.owners()[0].is_guard_total());
        assert!(!cover.owners()[1].is_guard_total());
    }
    assert_eq!(forward.status(), reverse.status());
    assert_eq!(forward.uncovered_partition(), reverse.uncovered_partition());
    for (left, right) in forward.owners().iter().zip(reverse.owners()) {
        assert_eq!(left.id(), right.id());
        assert_eq!(left.leading(), right.leading());
        assert_eq!(left.is_guard_total(), right.is_guard_total());
        for (left, right) in left
            .semantic
            .candidates()
            .iter()
            .zip(right.semantic.candidates())
        {
            assert_eq!(
                super::super::semantic::compare_exact_circuit_content(
                    left.circuit(),
                    right.circuit()
                ),
                std::cmp::Ordering::Equal
            );
        }
    }
}

#[test]
fn jointly_exhaustive_partial_candidates_remain_a_typed_guard_obstruction() {
    let (context, frame) = tadpole_frame();
    let partition = partition(&frame);
    let circuit = Arc::new(exact_circuit(&context, &frame, &partition));
    let semantic = semantic_with_extra_roots(&context, &partition, circuit, &[2, 3]);
    assert!(!semantic.guard_dag().is_abstractly_total());
    let cover = ExactCircuitOwnerCover::try_compile(
        &context,
        [ExactCircuitOwnerInput::new(
            &partition,
            ExactCircuitOuterExtensionWitness::try_prove(&partition, semantic).unwrap(),
        )],
        Vec::<IntegralKey>::new(),
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        cover.status(),
        ExactOwnerCoverStatus::Incomplete(ExactOwnerCoverObstructionKind::GuardIncomplete)
    );
    assert_eq!(cover.guard_incomplete_owners(), [cover.owners()[0].id()]);

    // The candidates are in fact jointly exhaustive on these exact points,
    // but bounded observations and this finite sample confer no closure.
    for power in 2..=8 {
        assert!(matches!(
            cover
                .try_select_at(
                    &context,
                    &IntegralKey::try_new([power]).unwrap(),
                    Default::default(),
                )
                .unwrap(),
            ExactOwnerCoverSelection::Descending { .. }
        ));
    }
}

#[test]
fn owner_cover_resource_limits_fail_before_admission() {
    let (context, frame) = tadpole_frame();
    let partition = partition(&frame);
    let circuit = Arc::new(exact_circuit(&context, &frame, &partition));
    let semantic = semantic(&context, &partition, circuit);
    let extension = ExactCircuitOuterExtensionWitness::try_prove(&partition, semantic).unwrap();
    let limits = ExactCircuitOwnerCoverLimits {
        max_owner_inputs: 0,
        ..Default::default()
    };
    assert!(matches!(
        ExactCircuitOwnerCover::try_compile(
            &context,
            [ExactCircuitOwnerInput::new(&partition, extension)],
            Vec::<IntegralKey>::new(),
            limits,
        ),
        Err(ExactCircuitOwnerCoverError::ResourceLimit {
            resource: "exact owner-cover semantic inputs",
            requested: 1,
            limit: 0,
        })
    ));
}

#[test]
fn mixed_immutable_owner_snapshots_are_rejected() {
    let artifact = crate::foundry::artifact::derive_one_loop_unit_mass_tadpole().unwrap();
    let family = artifact.family();
    let generator = ParametricIbpGenerator::try_new(family).unwrap();
    let context = generator.context().clone();
    let completed = complete_ordinary(&generator);
    let frame = OneSidedChartFrame::try_new(
        &generator,
        &completed,
        Mask::try_new([true]).unwrap(),
        0,
        PhysicalFrameLimits::default(),
    )
    .unwrap()
    .into_plan();
    let target = target_with_shift(&frame, &[1]);
    let shifts = frame
        .columns()
        .iter()
        .map(|shift| shift.values())
        .collect::<Vec<_>>();
    let domain = SectorMonotoneDomain::try_maximal_for_rule(
        frame.sector().clone(),
        frame.columns()[target].values(),
        &shifts,
    )
    .unwrap();
    let limits = StratumRegistryLimits::default();
    let empty_partition = partition_at_with(
        &frame,
        target,
        DecoratedStratum::try_guard_blind(
            frame.family_fingerprint(),
            frame.context_fingerprint(),
            domain.clone(),
            limits,
        )
        .unwrap(),
        ImmutableOwnerSnapshot::try_empty(
            frame.family_fingerprint(),
            frame.context_fingerprint(),
            frame.sector().arity(),
            limits,
        )
        .unwrap(),
    );
    let installed_partition = partition_at_with(
        &frame,
        target,
        DecoratedStratum::try_guard_blind(
            frame.family_fingerprint(),
            frame.context_fingerprint(),
            domain,
            limits,
        )
        .unwrap(),
        ImmutableOwnerSnapshot::try_from_closed_artifact(&artifact, limits).unwrap(),
    );
    let empty_semantic = semantic(
        &context,
        &empty_partition,
        Arc::new(exact_circuit(&context, &frame, &empty_partition)),
    );
    let installed_semantic = semantic(
        &context,
        &installed_partition,
        Arc::new(exact_circuit(&context, &frame, &installed_partition)),
    );
    assert!(matches!(
        ExactCircuitOwnerCover::try_compile(
            &context,
            [
                ExactCircuitOwnerInput::new(
                    &empty_partition,
                    ExactCircuitOuterExtensionWitness::try_prove(&empty_partition, empty_semantic,)
                        .unwrap(),
                ),
                ExactCircuitOwnerInput::new(
                    &installed_partition,
                    ExactCircuitOuterExtensionWitness::try_prove(
                        &installed_partition,
                        installed_semantic,
                    )
                    .unwrap(),
                ),
            ],
            Vec::<IntegralKey>::new(),
            Default::default(),
        ),
        Err(ExactCircuitOwnerCoverError::MixedOwnerScope {
            owner: 1,
            detail: "immutable lower-sector owner snapshot differs",
        })
    ));
}

#[test]
fn canonical_k6_s4a_owner_reports_nonfinite_without_claiming_closure() {
    let (context, frame) = k6_s4a_frame();
    let target = target_with_shift(&frame, &[-1, 1, 0, 0, 1, 0]);
    let partition = partition_at(&frame, target);
    let circuit = Arc::new(k6_s4a_exact_circuit(&context, &frame, &partition));
    let semantic = semantic(&context, &partition, circuit);

    let compile = || {
        ExactCircuitOwnerCover::try_compile(
            &context,
            [ExactCircuitOwnerInput::new(
                &partition,
                ExactCircuitOuterExtensionWitness::try_prove(&partition, semantic.clone()).unwrap(),
            )],
            Vec::<IntegralKey>::new(),
            Default::default(),
        )
        .unwrap()
    };
    let first = compile();
    let second = compile();
    for cover in [&first, &second] {
        assert_eq!(
            cover.status(),
            ExactOwnerCoverStatus::Incomplete(ExactOwnerCoverObstructionKind::NonFinite)
        );
        assert_eq!(cover.owners().len(), 1);
        assert!(cover.guard_incomplete_owners().is_empty());
        assert!(!cover.uncovered_partition().is_finite());
    }
    assert_eq!(first.status(), second.status());
    assert_eq!(first.owners()[0].leading(), second.owners()[0].leading());
    assert_eq!(
        first.owners()[0].is_guard_total(),
        second.owners()[0].is_guard_total()
    );
    assert_eq!(first.uncovered_partition(), second.uncovered_partition());
}

#[test]
fn bounded_and_predecorated_strata_cannot_authorize_infinite_rays() {
    let (context, frame) = tadpole_frame();
    let target = frame
        .columns()
        .iter()
        .position(|shift| shift.values() == [1])
        .unwrap();
    let shifts = frame
        .columns()
        .iter()
        .map(|shift| shift.values())
        .collect::<Vec<_>>();
    let bounded_domain = SectorMonotoneDomain::try_new_for_rule(
        frame.sector().clone(),
        [InteriorBounds::new(1, 10)],
        frame.columns()[target].values(),
        &shifts,
    )
    .unwrap();
    let bounded = partition_with(&frame, bounded_domain, []);
    let bounded_circuit = Arc::new(exact_circuit(&context, &frame, &bounded));
    let bounded_semantic = Arc::new(
        ExactCircuitSemanticDag::try_compile(
            &context,
            &bounded,
            &[bounded_circuit],
            Default::default(),
        )
        .unwrap(),
    );
    assert_eq!(
        ExactCircuitOuterExtensionWitness::try_prove(&bounded, bounded_semantic).unwrap_err(),
        ExactCircuitOuterExtensionError::TightenedCarrierDomain
    );

    let maximal = SectorMonotoneDomain::try_maximal_for_rule(
        frame.sector().clone(),
        frame.columns()[target].values(),
        &shifts,
    )
    .unwrap();
    let decorated = partition_with(
        &frame,
        maximal,
        [GuardBranchIdentity::try_new(
            "unhandled-outer-predicate",
            GuardBranch::NonZero,
            Default::default(),
        )
        .unwrap()],
    );
    let decorated_circuit = Arc::new(exact_circuit(&context, &frame, &decorated));
    let decorated_semantic = Arc::new(
        ExactCircuitSemanticDag::try_compile(
            &context,
            &decorated,
            &[decorated_circuit],
            Default::default(),
        )
        .unwrap(),
    );
    assert_eq!(
        ExactCircuitOuterExtensionWitness::try_prove(&decorated, decorated_semantic).unwrap_err(),
        ExactCircuitOuterExtensionError::PreexistingStratumPredicates { count: 1 }
    );
}
