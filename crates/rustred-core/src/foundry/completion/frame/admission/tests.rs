use std::cmp::Ordering;
use std::sync::Arc;

use crate::algebra::{CoefficientContext, IndexedCoefficientContext};
use crate::family::{AffineDenominator, IntegralFamily};
use crate::foundry::completion::frame::exact::{
    ExactCircuitLift, ExactCircuitLimits, ExactTargetCircuit, try_lift_exact_circuit,
};
use crate::foundry::completion::frame::modular::{
    ModularHit, ModularKernelLimits, ModularTargetQuery,
};
use crate::foundry::completion::frame::{
    OneSidedChartFrame, PhysicalFrameLimits, PhysicalFramePlan,
};
use crate::foundry::completion::stratum::{
    DecoratedStratum, GuardBranch, GuardBranchIdentity, ImmutableOwnerSnapshot,
    StratumRegistryError, StratumRegistryLimits, TargetColumnPartition,
};
use crate::identity::{CompletedIbpSourceRows, ParametricIbpGenerator};
use crate::sector::{Mask, OrderingPolicy, SectorMonotoneDomain};

use super::{
    ExactCircuitSemanticDag, ExactCircuitSemanticError, ExactCircuitSemanticLimits,
    ExactCircuitSemanticSelection, ExactGuardRefinementError, ExactGuardRefinementLimits,
    ExactGuardRefinementOutcome, try_refine_exact_circuit_guards,
};
use crate::foundry::completion::guard::decision::GuardDecisionEvaluationLimits;

const PRIME: u64 = 1_000_000_007;

fn complete_ordinary(generator: &ParametricIbpGenerator<'_>) -> CompletedIbpSourceRows {
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    prepared.complete(rows).unwrap()
}

fn tadpole_frame() -> (IndexedCoefficientContext, PhysicalFramePlan) {
    let base = CoefficientContext::new(["d"]);
    let family = IntegralFamily::new(
        "guard-refinement-tadpole",
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
        0,
        PhysicalFrameLimits::default(),
    )
    .unwrap()
    .into_plan();
    (context, frame)
}

fn target(frame: &PhysicalFramePlan) -> usize {
    frame
        .columns()
        .iter()
        .position(|shift| shift.values() == [1])
        .unwrap()
}

fn partition<'frame>(
    frame: &'frame PhysicalFramePlan,
    target: usize,
    guards: impl IntoIterator<Item = GuardBranchIdentity>,
) -> TargetColumnPartition<'frame> {
    try_partition(frame, target, guards).unwrap()
}

fn try_partition<'frame>(
    frame: &'frame PhysicalFramePlan,
    target: usize,
    guards: impl IntoIterator<Item = GuardBranchIdentity>,
) -> Result<TargetColumnPartition<'frame>, StratumRegistryError> {
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
    )?;
    let stratum = DecoratedStratum::try_new(
        frame.family_fingerprint(),
        frame.context_fingerprint(),
        domain,
        guards,
        limits,
    )?;
    let owners = ImmutableOwnerSnapshot::try_empty(
        frame.family_fingerprint(),
        frame.context_fingerprint(),
        frame.sector().arity(),
        limits,
    )?;
    TargetColumnPartition::try_new(
        frame,
        target,
        stratum,
        owners,
        OrderingPolicy::default(),
        limits,
    )
}

fn exact_circuit(
    context: &IndexedCoefficientContext,
    frame: &PhysicalFramePlan,
    partition: &TargetColumnPartition<'_>,
) -> ExactTargetCircuit {
    exact_circuit_at(context, frame, partition, 37, 2)
}

fn exact_circuit_at(
    context: &IndexedCoefficientContext,
    frame: &PhysicalFramePlan,
    partition: &TargetColumnPartition<'_>,
    dimension: i64,
    index: u64,
) -> ExactTargetCircuit {
    let sample = frame
        .try_modular_sample(
            context,
            PRIME,
            &[dimension],
            &[index],
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
        panic!("the tadpole target must have a modular hit")
    };
    let ExactCircuitLift::Replayed(circuit) =
        try_lift_exact_circuit(context, &hit, partition, ExactCircuitLimits::default()).unwrap()
    else {
        panic!("the tadpole modular support must lift exactly")
    };
    circuit
}

fn k6_s4a_semantic_frame() -> (IndexedCoefficientContext, PhysicalFramePlan) {
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

fn k6_s4a_semantic_hit<'frame>(
    context: &IndexedCoefficientContext,
    frame: &'frame PhysicalFramePlan,
    partition: &TargetColumnPartition<'frame>,
) -> ModularHit<'frame> {
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
        panic!("the canonical S4a semantic target must have a modular hit")
    };
    hit
}

fn k6_s4a_circuit_omitting(
    context: &IndexedCoefficientContext,
    frame: &PhysicalFramePlan,
    partition: &TargetColumnPartition<'_>,
    hit: &ModularHit<'_>,
    omitted_row: usize,
) -> ExactTargetCircuit {
    let mut alternative = hit.clone();
    let selected = (0..frame.row_count())
        .filter(|&row| row != omitted_row)
        .collect::<Vec<_>>();
    alternative.diagnostics.augmented_rank = selected.len();
    alternative.diagnostics.forbidden_rank = selected.len() - 1;
    alternative.diagnostics.augmented_independent_source_rows = selected.into_boxed_slice();
    let ExactCircuitLift::Replayed(circuit) = try_lift_exact_circuit(
        context,
        &alternative,
        partition,
        ExactCircuitLimits::default(),
    )
    .unwrap() else {
        panic!("the independently replayed S4a support must retain its exact target")
    };
    circuit
}

fn has_branch(
    stratum: &DecoratedStratum,
    predicate: &GuardBranchIdentity,
    branch: GuardBranch,
) -> bool {
    stratum
        .guards()
        .iter()
        .any(|candidate| candidate.same_predicate(predicate) && candidate.branch() == branch)
}

#[test]
fn semantic_candidates_are_content_sorted_and_return_the_exact_replayed_arc() {
    let (context, frame) = k6_s4a_semantic_frame();
    let target = frame
        .columns()
        .iter()
        .position(|shift| shift.values() == [-1, 1, 0, 0, 1, 0])
        .unwrap();
    let target_partition = partition(&frame, target, []);
    let hit = k6_s4a_semantic_hit(&context, &frame, &target_partition);
    let first = Arc::new(k6_s4a_circuit_omitting(
        &context,
        &frame,
        &target_partition,
        &hit,
        0,
    ));
    let second = Arc::new(k6_s4a_circuit_omitting(
        &context,
        &frame,
        &target_partition,
        &hit,
        19,
    ));
    assert_ne!(first.source_combination(), second.source_combination());
    assert!(!super::semantic::exact_circuit_content_equal(
        &first, &second
    ));
    let content_order = super::semantic::compare_exact_circuit_content(&first, &second);
    assert_ne!(content_order, Ordering::Equal);
    let expected = if content_order == Ordering::Less {
        &first
    } else {
        &second
    };

    let forward = ExactCircuitSemanticDag::try_compile(
        &context,
        &target_partition,
        &[first.clone(), second.clone()],
        ExactCircuitSemanticLimits::default(),
    )
    .unwrap();
    let reverse = ExactCircuitSemanticDag::try_compile(
        &context,
        &target_partition,
        &[second.clone(), first.clone()],
        ExactCircuitSemanticLimits::default(),
    )
    .unwrap();
    assert_eq!(forward.candidates().len(), 2);
    assert_eq!(reverse.candidates().len(), 2);
    for (ordinal, (left, right)) in forward
        .candidates()
        .iter()
        .zip(reverse.candidates())
        .enumerate()
    {
        assert_eq!(left.id().ordinal(), ordinal);
        assert_eq!(right.id().ordinal(), ordinal);
        assert!(super::semantic::exact_circuit_content_equal(
            left.circuit(),
            right.circuit()
        ));
    }

    let singleton_first = ExactCircuitSemanticDag::try_compile(
        &context,
        &target_partition,
        &[first.clone()],
        ExactCircuitSemanticLimits::default(),
    )
    .unwrap();
    let singleton_second = ExactCircuitSemanticDag::try_compile(
        &context,
        &target_partition,
        &[second.clone()],
        ExactCircuitSemanticLimits::default(),
    )
    .unwrap();
    let mut common_point = None;
    'points: for n0 in -3..=0 {
        for n1 in 1..=3 {
            for n2 in 1..=3 {
                for n3 in 1..=3 {
                    for n4 in 1..=3 {
                        for n5 in -3..=0 {
                            let point = [n0, n1, n2, n3, n4, n5];
                            if matches!(
                                singleton_first.try_select_at(
                                    &context,
                                    &point,
                                    GuardDecisionEvaluationLimits::default()
                                ),
                                Ok(ExactCircuitSemanticSelection::Selected(_))
                            ) && matches!(
                                singleton_second.try_select_at(
                                    &context,
                                    &point,
                                    GuardDecisionEvaluationLimits::default()
                                ),
                                Ok(ExactCircuitSemanticSelection::Selected(_))
                            ) {
                                common_point = Some(point);
                                break 'points;
                            }
                        }
                    }
                }
            }
        }
    }
    let point = common_point.expect("the two exact S4a candidates must overlap generically");
    for dag in [&forward, &reverse] {
        let ExactCircuitSemanticSelection::Selected(selected) = dag
            .try_select_at(&context, &point, GuardDecisionEvaluationLimits::default())
            .unwrap()
        else {
            panic!("the generic S4a point must have an admitted exact circuit")
        };
        assert!(Arc::ptr_eq(selected.circuit(), expected));
        assert_eq!(selected.id().ordinal(), 0);
    }

    assert!(forward.guard_dag().stats().atoms > 0);
    let mut evaluation_limits = GuardDecisionEvaluationLimits::default();
    evaluation_limits.max_predicate_evaluations = 0;
    assert!(matches!(
        forward.try_select_at(&context, &point, evaluation_limits),
        Err(ExactCircuitSemanticError::GuardDag(_))
    ));
}

#[test]
fn semantic_admission_rejects_duplicate_modular_discoveries_bad_joins_and_aggregate_overflow() {
    let (context, frame) = tadpole_frame();
    let target = target(&frame);
    let target_partition = partition(&frame, target, []);
    let first = Arc::new(exact_circuit_at(&context, &frame, &target_partition, 37, 2));
    let second = Arc::new(exact_circuit_at(&context, &frame, &target_partition, 41, 3));
    assert_ne!(first.sample_fingerprint(), second.sample_fingerprint());
    assert!(super::semantic::exact_circuit_content_equal(
        &first, &second
    ));
    assert_eq!(
        ExactCircuitSemanticDag::try_compile(
            &context,
            &target_partition,
            &[first.clone(), second],
            ExactCircuitSemanticLimits::default(),
        )
        .unwrap_err(),
        ExactCircuitSemanticError::DuplicateExactContent
    );

    let foreign_partition = partition(
        &frame,
        target,
        [GuardBranchIdentity::try_new(
            "semantic-foreign-parent",
            GuardBranch::NonZero,
            Default::default(),
        )
        .unwrap()],
    );
    assert!(matches!(
        ExactCircuitSemanticDag::try_compile(
            &context,
            &foreign_partition,
            &[first.clone()],
            ExactCircuitSemanticLimits::default(),
        ),
        Err(ExactCircuitSemanticError::CandidateJoin {
            candidate: 0,
            detail: "decorated stratum identity differs",
        })
    ));

    let mut limits = ExactCircuitSemanticLimits::default();
    limits.max_residual_terms = 0;
    assert!(matches!(
        ExactCircuitSemanticDag::try_compile(&context, &target_partition, &[first], limits),
        Err(ExactCircuitSemanticError::ResourceLimit {
            resource: "semantic exact-circuit residual terms",
            requested: 1,
            limit: 0,
        })
    ));

    let first = Arc::new(exact_circuit_at(&context, &frame, &target_partition, 37, 2));
    let mut limits = ExactCircuitSemanticLimits::default();
    limits.max_guard_coefficient_equations = 0;
    assert!(matches!(
        ExactCircuitSemanticDag::try_compile(&context, &target_partition, &[first], limits),
        Err(ExactCircuitSemanticError::ResourceLimit {
            resource: "semantic exact-circuit compiled guard coefficient equations",
            requested,
            limit: 0,
        }) if requested > 0
    ));

    let first = Arc::new(exact_circuit_at(&context, &frame, &target_partition, 37, 2));
    let mut limits = ExactCircuitSemanticLimits::default();
    limits.max_modular_sample_point_entries = 0;
    assert!(matches!(
        ExactCircuitSemanticDag::try_compile(&context, &target_partition, &[first], limits),
        Err(ExactCircuitSemanticError::ResourceLimit {
            resource: "semantic exact-circuit modular sample-point entries",
            requested,
            limit: 0,
        }) if requested > 0
    ));

    let first = Arc::new(exact_circuit_at(&context, &frame, &target_partition, 37, 2));
    let mut limits = ExactCircuitSemanticLimits::default();
    limits.max_modular_diagnostic_entries = 0;
    assert!(matches!(
        ExactCircuitSemanticDag::try_compile(&context, &target_partition, &[first], limits),
        Err(ExactCircuitSemanticError::ResourceLimit {
            resource: "semantic exact-circuit modular diagnostic entries",
            requested,
            limit: 0,
        }) if requested > 0
    ));
}

#[test]
fn first_zero_refinement_is_deterministic_disjoint_and_keeps_only_one_admitted_child() {
    let (context, frame) = tadpole_frame();
    let target = target(&frame);
    let target_partition = partition(&frame, target, []);
    let circuit = exact_circuit(&context, &frame, &target_partition);
    assert!(!circuit.nonzero_guards().is_empty());

    let first = try_refine_exact_circuit_guards(
        &context,
        &circuit,
        &target_partition,
        ExactGuardRefinementLimits::default(),
    )
    .unwrap();
    let second = try_refine_exact_circuit_guards(
        &context,
        &circuit,
        &target_partition,
        ExactGuardRefinementLimits::default(),
    )
    .unwrap();
    assert_eq!(first, second);
    let ExactGuardRefinementOutcome::Admitted(refinement) = first else {
        panic!("a guard-blind parent must be refined, not blocked")
    };
    assert_eq!(
        refinement.parent_stratum_id(),
        target_partition.stratum_id()
    );
    assert_eq!(
        refinement.newly_split_predicate_ordinals().len(),
        refinement.required_predicates().len()
    );
    assert_eq!(
        refinement.exceptional_strata().len(),
        refinement.newly_split_predicate_ordinals().len()
    );
    for predicate in refinement.required_predicates() {
        assert!(!predicate.circuit_guard_ordinals().is_empty());
        assert!(has_branch(
            refinement.admitted_stratum(),
            predicate.nonzero_branch(),
            GuardBranch::NonZero
        ));
    }
    for (exceptional_ordinal, exceptional) in refinement.exceptional_strata().iter().enumerate() {
        let required_ordinal = exceptional.required_predicate_ordinal();
        assert_eq!(
            required_ordinal,
            refinement.newly_split_predicate_ordinals()[exceptional_ordinal]
        );
        for &earlier in &refinement.newly_split_predicate_ordinals()[..exceptional_ordinal] {
            assert!(has_branch(
                exceptional.stratum(),
                refinement.required_predicates()[earlier].nonzero_branch(),
                GuardBranch::NonZero
            ));
        }
        assert!(has_branch(
            exceptional.stratum(),
            refinement.required_predicates()[required_ordinal].nonzero_branch(),
            GuardBranch::Zero
        ));
        for &later in &refinement.newly_split_predicate_ordinals()[exceptional_ordinal + 1..] {
            assert!(!exceptional.stratum().guards().iter().any(|candidate| {
                candidate.same_predicate(refinement.required_predicates()[later].nonzero_branch())
            }));
        }
    }
}

#[test]
fn exact_preexisting_nonzero_is_reused_and_known_zero_blocks_without_an_owner() {
    let (context, frame) = tadpole_frame();
    let target = target(&frame);
    let blind = partition(&frame, target, []);
    let blind_circuit = exact_circuit(&context, &frame, &blind);
    let first_guard = GuardBranchIdentity::try_from_indexed_polynomial(
        &context,
        blind_circuit.nonzero_guards()[0].polynomial(),
        GuardBranch::NonZero,
        Default::default(),
        Default::default(),
    )
    .unwrap();

    let nonzero_partition = partition(&frame, target, [first_guard.clone()]);
    let nonzero_circuit = exact_circuit(&context, &frame, &nonzero_partition);
    let ExactGuardRefinementOutcome::Admitted(nonzero) = try_refine_exact_circuit_guards(
        &context,
        &nonzero_circuit,
        &nonzero_partition,
        Default::default(),
    )
    .unwrap() else {
        panic!("a proved nonzero guard must remain applicable")
    };
    assert!(has_branch(
        nonzero.admitted_stratum(),
        &first_guard,
        GuardBranch::NonZero
    ));
    assert!(
        nonzero
            .exceptional_strata()
            .iter()
            .all(|child| { !has_branch(child.stratum(), &first_guard, GuardBranch::Zero) })
    );

    let zero_guard = first_guard.with_branch(GuardBranch::Zero);
    let zero_partition = partition(&frame, target, [zero_guard.clone()]);
    let zero_circuit = exact_circuit(&context, &frame, &zero_partition);
    let blocked = try_refine_exact_circuit_guards(
        &context,
        &zero_circuit,
        &zero_partition,
        Default::default(),
    )
    .unwrap();
    assert!(matches!(
        blocked,
        ExactGuardRefinementOutcome::BlockedByKnownZero {
            required_predicate_ordinal: 0,
            first_circuit_guard_ordinal: 0,
            zero_branch,
        } if zero_branch == zero_guard
    ));
}

#[test]
fn refinement_resource_limits_and_cross_partition_joins_fail_closed() {
    let (context, frame) = tadpole_frame();
    let target = target(&frame);
    let target_partition = partition(&frame, target, []);
    let circuit = exact_circuit(&context, &frame, &target_partition);

    let mut limits = ExactGuardRefinementLimits::default();
    limits.max_circuit_guard_identity_bytes = 0;
    assert!(matches!(
        try_refine_exact_circuit_guards(&context, &circuit, &target_partition, limits),
        Err(ExactGuardRefinementError::ResourceLimit {
            resource: "exact guard refinement circuit guard identity bytes",
            requested,
            limit: 0,
        }) if requested > 0
    ));

    let mut limits = ExactGuardRefinementLimits::default();
    limits.max_exceptional_strata = 0;
    assert!(matches!(
        try_refine_exact_circuit_guards(&context, &circuit, &target_partition, limits),
        Err(ExactGuardRefinementError::ResourceLimit {
            resource: "exact guard refinement exceptional strata",
            requested,
            limit: 0,
        }) if requested > 0
    ));

    let mut limits = ExactGuardRefinementLimits::default();
    limits.max_result_stratum_identity_bytes = 0;
    assert!(matches!(
        try_refine_exact_circuit_guards(&context, &circuit, &target_partition, limits),
        Err(ExactGuardRefinementError::ResourceLimit {
            resource: "exact guard refinement result stratum identity bytes",
            requested,
            limit: 0,
        }) if requested > 0
    ));

    let foreign_parent = partition(
        &frame,
        target,
        [GuardBranchIdentity::try_new(
            "foreign-parent-branch",
            GuardBranch::NonZero,
            Default::default(),
        )
        .unwrap()],
    );
    assert_eq!(
        try_refine_exact_circuit_guards(&context, &circuit, &foreign_parent, Default::default(),)
            .unwrap_err(),
        ExactGuardRefinementError::CircuitStratumMismatch
    );
}
