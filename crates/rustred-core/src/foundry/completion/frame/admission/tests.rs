use crate::algebra::{CoefficientContext, IndexedCoefficientContext};
use crate::family::{AffineDenominator, IntegralFamily};
use crate::foundry::completion::frame::exact::{
    ExactCircuitLift, ExactCircuitLimits, ExactTargetCircuit, try_lift_exact_circuit,
};
use crate::foundry::completion::frame::modular::{ModularKernelLimits, ModularTargetQuery};
use crate::foundry::completion::frame::{PhysicalFrameLimits, PhysicalFramePlan};
use crate::foundry::completion::stratum::{
    DecoratedStratum, GuardBranch, GuardBranchIdentity, ImmutableOwnerSnapshot,
    StratumRegistryLimits, TargetColumnPartition,
};
use crate::identity::{CompletedIbpSourceRows, ParametricIbpGenerator};
use crate::sector::{Mask, OrderingPolicy, SectorMonotoneDomain};

use super::{
    ExactGuardRefinementError, ExactGuardRefinementLimits, ExactGuardRefinementOutcome,
    try_refine_exact_circuit_guards,
};

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
    let frame = PhysicalFramePlan::try_new(
        &generator,
        &completed,
        Mask::try_new([true]).unwrap(),
        0,
        PhysicalFrameLimits::default(),
    )
    .unwrap();
    (context, frame)
}

fn target(frame: &PhysicalFramePlan) -> usize {
    frame
        .columns()
        .iter()
        .position(|shift| shift.values() == [1])
        .unwrap()
}

fn maximal_domain(frame: &PhysicalFramePlan, target: usize) -> SectorMonotoneDomain {
    let shifts = frame
        .columns()
        .iter()
        .map(|shift| shift.values())
        .collect::<Vec<_>>();
    SectorMonotoneDomain::try_maximal_for_rule(
        frame.sector().clone(),
        frame.columns()[target].values(),
        &shifts,
    )
    .unwrap()
}

fn partition<'frame>(
    frame: &'frame PhysicalFramePlan,
    target: usize,
    guards: impl IntoIterator<Item = GuardBranchIdentity>,
) -> TargetColumnPartition<'frame> {
    let limits = StratumRegistryLimits::default();
    let stratum = DecoratedStratum::try_new(
        frame.family_fingerprint(),
        frame.context_fingerprint(),
        maximal_domain(frame, target),
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
        panic!("the tadpole modular support must lift exactly")
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
