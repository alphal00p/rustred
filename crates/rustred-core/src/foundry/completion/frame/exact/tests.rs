use std::sync::Arc;

use crate::algebra::{CoefficientContext, IndexedCoefficientContext};
use crate::family::{AffineDenominator, IntegralFamily};
use crate::foundry::artifact::canonical_three_loop_family;
use crate::foundry::completion::frame::modular::{
    ModularKernelLimits, ModularPhysicalFrame, ModularTargetQuery,
};
use crate::foundry::completion::frame::{
    OneSidedChartFrame, PhysicalFrameLimits, PhysicalFramePlan,
};
use crate::foundry::completion::stratum::{
    DecoratedStratum, ImmutableOwnerSnapshot, StratumRegistryError, StratumRegistryLimits,
    TargetColumnPartition,
};
use crate::identity::{CompletedIbpSourceRows, ParametricIbpGenerator};
use crate::sector::{Mask, OrderingPolicy, SectorMonotoneDomain};

use super::cleared::{
    ClearedCircuitError, ClearedCircuitLimits, ClearedSemanticGuardOrigin, try_clear_exact_circuit,
    try_compile_final_target_guard,
};
use super::{
    ExactCircuitError, ExactCircuitGuardOrigin, ExactCircuitLift, ExactCircuitLimits,
    try_lift_exact_circuit,
};

const PRIME: u64 = 1_000_000_007;

fn complete_ordinary(generator: &ParametricIbpGenerator<'_>) -> CompletedIbpSourceRows {
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    prepared.complete(rows).unwrap()
}

fn tadpole(name: &str, massive: bool) -> IntegralFamily {
    let context = CoefficientContext::new(["d"]);
    IntegralFamily::new(
        name,
        vec!["k".to_owned()],
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
        vec![AffineDenominator::new(
            if massive {
                context.integer(-1)
            } else {
                context.zero()
            },
            vec![context.one()],
        )],
        Vec::new(),
        vec![context.zero()],
    )
    .unwrap()
}

fn tadpole_frame(
    name: &str,
    massive: bool,
    degree: usize,
) -> (IndexedCoefficientContext, PhysicalFramePlan) {
    let family = tadpole(name, massive);
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let context = generator.context().clone();
    let completed = complete_ordinary(&generator);
    let plan = OneSidedChartFrame::try_new(
        &generator,
        &completed,
        Mask::try_new([true]).unwrap(),
        degree,
        PhysicalFrameLimits::default(),
    )
    .unwrap()
    .into_plan();
    (context, plan)
}

fn column(plan: &PhysicalFramePlan, shift: &[i64]) -> usize {
    plan.columns()
        .iter()
        .position(|candidate| candidate.values() == shift)
        .unwrap()
}

fn target_partition<'frame>(
    plan: &'frame PhysicalFramePlan,
    target: usize,
) -> TargetColumnPartition<'frame> {
    try_target_partition(plan, target).unwrap()
}

fn try_target_partition<'frame>(
    plan: &'frame PhysicalFramePlan,
    target: usize,
) -> Result<TargetColumnPartition<'frame>, StratumRegistryError> {
    let all_shifts = plan
        .columns()
        .iter()
        .map(|shift| shift.values())
        .collect::<Vec<_>>();
    let domain = SectorMonotoneDomain::try_maximal_for_rule(
        plan.sector().clone(),
        plan.columns()[target].values(),
        &all_shifts,
    )
    .unwrap();
    let limits = StratumRegistryLimits::default();
    let stratum = DecoratedStratum::try_guard_blind(
        plan.family_fingerprint(),
        plan.context_fingerprint(),
        domain,
        limits,
    )
    .unwrap();
    let owners = ImmutableOwnerSnapshot::try_empty(
        plan.family_fingerprint(),
        plan.context_fingerprint(),
        plan.sector().arity(),
        StratumRegistryLimits::default(),
    )
    .unwrap();
    TargetColumnPartition::try_new(
        plan,
        target,
        stratum,
        owners,
        OrderingPolicy::default(),
        limits,
    )
}

fn sample_tadpole<'frame>(
    context: &IndexedCoefficientContext,
    plan: &'frame PhysicalFramePlan,
) -> ModularPhysicalFrame<'frame> {
    plan.try_modular_sample(context, PRIME, &[37], &[2], ModularKernelLimits::default())
        .unwrap()
}

#[test]
fn exact_lift_replays_a_frame_bound_nonempty_partition_deterministically() {
    let (context, plan) = tadpole_frame("exact-circuit-massive", true, 1);
    assert_eq!(
        plan.columns()
            .iter()
            .map(|shift| shift.values())
            .collect::<Vec<_>>(),
        vec![&[0][..], &[1][..], &[2][..]]
    );
    let target = column(&plan, &[1]);
    let lower = column(&plan, &[0]);
    let harder = column(&plan, &[2]);
    let partition = target_partition(&plan, target);
    assert_eq!(partition.forbidden_columns(), &[harder]);
    assert_eq!(
        partition
            .allowed_columns()
            .iter()
            .map(|descriptor| descriptor.column())
            .collect::<Vec<_>>(),
        vec![lower]
    );

    let sampled = sample_tadpole(&context, &plan);
    let query = sampled
        .query_target(
            target,
            partition.forbidden_columns(),
            ModularKernelLimits::default(),
        )
        .unwrap();
    let ModularTargetQuery::Hit(hit) = query else {
        panic!("the exact tadpole target must have a modular rank hit")
    };
    assert!(Arc::ptr_eq(
        sampled.sample_fingerprint(),
        hit.sample_fingerprint()
    ));
    assert_eq!(hit.sample_fingerprint().modulus(), PRIME);

    let first =
        try_lift_exact_circuit(&context, &hit, &partition, ExactCircuitLimits::default()).unwrap();
    let second =
        try_lift_exact_circuit(&context, &hit, &partition, ExactCircuitLimits::default()).unwrap();
    assert_eq!(first, second);
    let ExactCircuitLift::Replayed(circuit) = first else {
        panic!("the nonvanishing modular support must lift exactly")
    };
    assert!(Arc::ptr_eq(
        hit.sample_fingerprint(),
        circuit.sample_fingerprint()
    ));
    assert_eq!(circuit.target_column(), target);
    assert_eq!(circuit.modular_diagnostics(), hit.diagnostics());
    assert_eq!(circuit.target_shift().values(), [1]);
    assert_eq!(circuit.stratum_id(), partition.stratum_id());
    assert_eq!(circuit.owner_snapshot_id(), partition.snapshot_id());
    assert_eq!(circuit.residual_terms().len(), 1);
    let residual = &circuit.residual_terms()[0];
    assert_eq!(residual.physical_column(), lower);
    assert_eq!(residual.shift().values(), [0]);
    assert!(residual.descent().verify());
    assert!(residual.proper_subsector_owners().is_empty());
    assert!(!residual.coefficient().is_zero());
    assert!(!circuit.source_combination().is_empty());
    assert!(circuit.replay().source_contributions() > 0);
    assert!(circuit.replay().source_terms() > 0);
    assert_eq!(circuit.replay().physical_columns(), plan.columns().len());
    assert!(circuit.replay().exact_operations() > 0);
    assert!(!circuit.pivot_guards().is_empty());
    assert!(
        circuit
            .nonzero_guards()
            .iter()
            .all(|guard| !guard.polynomial().is_zero())
    );
    assert!(circuit.nonzero_guards().iter().any(|guard| {
        guard.origins().iter().any(|origin| {
            matches!(
                origin,
                ExactCircuitGuardOrigin::ReducerPivotNumerator { .. }
                    | ExactCircuitGuardOrigin::SourceMultiplierDenominator { .. }
            )
        })
    }));

    let cleared =
        try_clear_exact_circuit(&context, &plan, &circuit, ClearedCircuitLimits::default())
            .expect("the tadpole circuit must admit a fraction-free source replay");
    assert_eq!(cleared.target_column(), target);
    assert!(!cleared.target_coefficient().is_zero());
    assert_eq!(
        cleared.source_cofactors().len(),
        circuit.source_combination().len()
    );
    assert!(cleared.source_cofactors().iter().all(|source| {
        plan.source_instances().get(source.frame_row_ordinal()) == Some(source.source_instance())
            && !source.row_denominator().is_zero()
            && !source.cofactor().is_zero()
    }));
    assert!(cleared.physical_terms().iter().any(|term| {
        term.physical_column() == target && term.coefficient() == cleared.target_coefficient()
    }));
    assert!(cleared.semantic_guards().iter().all(|guard| {
        !guard.polynomial().is_zero()
            && guard.origins().iter().all(|origin| {
                matches!(
                    origin,
                    ClearedSemanticGuardOrigin::SourceOrFamily(_)
                        | ClearedSemanticGuardOrigin::FinalTargetCoefficient
                )
            })
    }));
    let telemetry = cleared.guard_telemetry();
    assert_eq!(telemetry.before_unique(), circuit.nonzero_guards().len());
    assert!(telemetry.before_intermediate_only() + telemetry.before_mixed() > 0);
    assert_eq!(
        telemetry.before_unique(),
        telemetry.before_source_or_family_only()
            + telemetry.before_intermediate_only()
            + telemetry.before_mixed()
    );
    assert_eq!(telemetry.after_unique(), cleared.semantic_guards().len());
    assert!(telemetry.after_source_or_family() <= telemetry.after_unique());
    assert_eq!(
        telemetry.final_target_guard_retained(),
        !cleared.target_coefficient().is_nonzero_constant()
    );
    assert!(cleared.exact_operations() > 0);
    assert!(cleared.retained_polynomial_terms() > 0);
    assert_eq!(
        (
            telemetry.before_unique(),
            telemetry.before_source_or_family_only(),
            telemetry.before_intermediate_only(),
            telemetry.before_mixed(),
            telemetry.after_unique(),
            telemetry.after_source_or_family(),
            telemetry.final_target_guard_retained(),
        ),
        (2, 0, 2, 0, 1, 0, true)
    );
    assert_eq!(
        (
            cleared.source_cofactors().len(),
            cleared.physical_terms().len(),
            cleared.target_coefficient().raw().nterms(),
            cleared
                .physical_terms()
                .iter()
                .map(|term| term.coefficient().raw().nterms())
                .sum::<usize>(),
            cleared.exact_operations(),
            cleared.gcd_term_pairs(),
            cleared.retained_polynomial_terms(),
        ),
        (1, 2, 1, 3, 31, 1, 15)
    );

    let zero_operation_limits = ClearedCircuitLimits::default().with_max_polynomial_operations(0);
    assert_eq!(
        try_clear_exact_circuit(&context, &plan, &circuit, zero_operation_limits).unwrap_err(),
        ClearedCircuitError::ResourceLimit {
            resource: "cleared-circuit polynomial operations",
            requested: 1,
            limit: 0,
        }
    );
}

#[test]
fn n_times_target_zero_mutant_retains_the_final_target_guard() {
    let (context, _) = tadpole_frame("cleared-circuit-n-target-mutant", true, 0);
    let n = context.index(0).unwrap();
    let n = context
        .numerator_condition_with_limits(&n, Default::default())
        .unwrap();
    let guards = try_compile_final_target_guard(&context, &n, ClearedCircuitLimits::default())
        .expect("the mutant target polynomial must compile as a mandatory guard");
    assert_eq!(guards.len(), 1);
    assert_eq!(guards[0].polynomial(), &n);
    assert_eq!(
        guards[0].origins(),
        &[ClearedSemanticGuardOrigin::FinalTargetCoefficient]
    );
    let at_zero = context
        .specialize_polynomial(guards[0].polynomial(), &[0], Default::default())
        .unwrap();
    assert!(at_zero.is_zero(), "n*I_t=0 must not own the n=0 branch");
}

#[test]
fn an_inconsistent_selected_support_returns_typed_inconclusive_evidence() {
    let (context, plan) = tadpole_frame("exact-circuit-support-miss", true, 1);
    let discovered_target = column(&plan, &[1]);
    let discovered_partition = target_partition(&plan, discovered_target);
    let sampled = sample_tadpole(&context, &plan);
    let ModularTargetQuery::Hit(mut hit) = sampled
        .query_target(
            discovered_target,
            discovered_partition.forbidden_columns(),
            ModularKernelLimits::default(),
        )
        .unwrap()
    else {
        panic!("the tadpole target must produce a hit")
    };

    // Model a corrupted/unlucky discovery support with a rank-shaped row that
    // does not contain the declared exact target. Exact replay must not turn
    // this into an authoritative no-relation result.
    let absent_target = column(&plan, &[2]);
    let partition = target_partition(&plan, absent_target);
    hit.diagnostics.target_column = absent_target;
    hit.diagnostics.forbidden_columns = Box::new([]);
    hit.diagnostics.forbidden_rank = 0;
    hit.diagnostics.augmented_rank = 1;
    hit.diagnostics.augmented_independent_source_rows = Box::new([0]);
    let ExactCircuitLift::ModularSupportDidNotLift(miss) =
        try_lift_exact_circuit(&context, &hit, &partition, ExactCircuitLimits::default()).unwrap()
    else {
        panic!("an absent exact target pivot must stay typed and inconclusive")
    };
    assert!(Arc::ptr_eq(
        miss.sample_fingerprint(),
        hit.sample_fingerprint()
    ));
    assert_eq!(miss.modular_diagnostics(), hit.diagnostics());
    assert_eq!(miss.selected_source_instances().len(), 1);
    assert_eq!(miss.exact_forbidden_rank(), 0);
    assert_eq!(miss.exact_augmented_rank(), 0);
}

#[test]
fn exact_lift_accepts_a_genuine_target_only_zero_circuit() {
    let (context, plan) = tadpole_frame("exact-circuit-massless", false, 0);
    assert_eq!(plan.columns().len(), 1);
    let partition = target_partition(&plan, 0);
    assert!(partition.forbidden_columns().is_empty());
    assert!(partition.allowed_columns().is_empty());
    let sampled = sample_tadpole(&context, &plan);
    let ModularTargetQuery::Hit(hit) = sampled
        .query_target(0, &[], ModularKernelLimits::default())
        .unwrap()
    else {
        panic!("the nonzero massless tadpole coefficient must produce a hit")
    };
    let ExactCircuitLift::Replayed(circuit) =
        try_lift_exact_circuit(&context, &hit, &partition, ExactCircuitLimits::default()).unwrap()
    else {
        panic!("the target-only zero circuit must lift exactly")
    };
    assert!(circuit.residual_terms().is_empty());
    assert_eq!(circuit.target_shift().values(), [0]);
    assert_eq!(circuit.replay().physical_columns(), 1);
}

#[test]
fn a_modular_hit_cannot_be_reused_with_an_equal_foreign_frame() {
    let (first_context, first_plan) = tadpole_frame("exact-circuit-foreign", true, 1);
    let (second_context, second_plan) = tadpole_frame("exact-circuit-foreign", true, 1);
    assert_eq!(first_plan, second_plan);
    assert!(!std::ptr::eq(&first_plan, &second_plan));
    let target = column(&first_plan, &[1]);
    let first_partition = target_partition(&first_plan, target);
    let second_partition = target_partition(&second_plan, target);
    let sampled = sample_tadpole(&first_context, &first_plan);
    let ModularTargetQuery::Hit(hit) = sampled
        .query_target(
            target,
            first_partition.forbidden_columns(),
            ModularKernelLimits::default(),
        )
        .unwrap()
    else {
        panic!("the tadpole target must produce a hit")
    };
    assert_eq!(
        try_lift_exact_circuit(
            &second_context,
            &hit,
            &second_partition,
            ExactCircuitLimits::default(),
        )
        .unwrap_err(),
        ExactCircuitError::ForeignFrameHit
    );
}

#[test]
fn native_decomposition_is_preflighted_before_exact_reduction() {
    let (context, plan) = tadpole_frame("exact-circuit-native-bound", true, 1);
    let target = column(&plan, &[1]);
    let partition = target_partition(&plan, target);
    let sampled = sample_tadpole(&context, &plan);
    let ModularTargetQuery::Hit(hit) = sampled
        .query_target(
            target,
            partition.forbidden_columns(),
            ModularKernelLimits::default(),
        )
        .unwrap()
    else {
        panic!("the tadpole target must produce a hit")
    };
    let rows = hit.diagnostics().augmented_independent_source_rows.len();
    let projected = partition.forbidden_columns().len() + 1;
    let requested = rows * (projected + 2 * rows);
    assert!(requested > 0);
    let mut limits = ExactCircuitLimits::default();
    limits.max_native_decomposition_nonzero_entries = requested - 1;
    assert_eq!(
        try_lift_exact_circuit(&context, &hit, &partition, limits).unwrap_err(),
        ExactCircuitError::ResourceLimit {
            resource: "exact-circuit native U/L nonzero entries",
            requested,
            limit: requested - 1,
        }
    );
}

fn s4a_degree_one() -> (IndexedCoefficientContext, PhysicalFramePlan) {
    let family = canonical_three_loop_family().unwrap();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let context = generator.context().clone();
    let completed = complete_ordinary(&generator);
    let plan = OneSidedChartFrame::try_new(
        &generator,
        &completed,
        Mask::try_new([false, true, true, true, true, false]).unwrap(),
        1,
        PhysicalFrameLimits::default(),
    )
    .unwrap()
    .into_plan();
    (context, plan)
}

#[test]
fn canonical_k6_s4a_has_a_nonempty_partition_exact_circuit() {
    let (context, plan) = s4a_degree_one();
    let sampled = plan
        .try_modular_sample(
            &context,
            PRIME,
            &[37],
            &[1, 2, 3, 4, 5, 6],
            ModularKernelLimits::default(),
        )
        .unwrap();

    let mut exact_hit = None;
    for target in 0..plan.columns().len() {
        let Ok(partition) = try_target_partition(&plan, target) else {
            continue;
        };
        if partition.forbidden_columns().is_empty() || partition.allowed_columns().is_empty() {
            continue;
        }
        let ModularTargetQuery::Hit(hit) = sampled
            .query_target(
                target,
                partition.forbidden_columns(),
                ModularKernelLimits::default(),
            )
            .unwrap()
        else {
            continue;
        };
        if let ExactCircuitLift::Replayed(circuit) =
            try_lift_exact_circuit(&context, &hit, &partition, ExactCircuitLimits::default())
                .unwrap()
        {
            exact_hit = Some((partition, circuit));
            break;
        }
    }

    let (partition, circuit) = exact_hit
        .expect("canonical K6 S4a degree one must expose a genuine nonempty-partition circuit");
    assert!(!partition.forbidden_columns().is_empty());
    assert!(!partition.allowed_columns().is_empty());
    assert_eq!(circuit.stratum_id(), partition.stratum_id());
    assert_eq!(circuit.owner_snapshot_id(), partition.snapshot_id());
    assert!(circuit.source_combination().iter().all(|source| {
        plan.source_instances().get(source.frame_row_ordinal()) == Some(source.source_instance())
    }));
    assert!(
        circuit
            .residual_terms()
            .iter()
            .all(|term| term.descent().verify())
    );

    let cleared =
        try_clear_exact_circuit(&context, &plan, &circuit, ClearedCircuitLimits::default())
            .expect("the K6 S4a circuit must admit a fraction-free source replay");
    let repeated =
        try_clear_exact_circuit(&context, &plan, &circuit, ClearedCircuitLimits::default())
            .expect("the same K6 S4a circuit must replay a second time");
    assert_eq!(cleared, repeated);
    let telemetry = cleared.guard_telemetry();
    assert_eq!(telemetry.before_unique(), circuit.nonzero_guards().len());
    assert_eq!(telemetry.after_unique(), cleared.semantic_guards().len());
    assert!(
        telemetry.after_unique() <= telemetry.before_unique().saturating_add(1),
        "clearing should replace elimination guards by at most one final-target predicate"
    );
    assert!(cleared.exact_operations() > 0);
    assert!(cleared.gcd_term_pairs() > 0);
    assert_eq!(
        (
            telemetry.before_unique(),
            telemetry.before_source_or_family_only(),
            telemetry.before_intermediate_only(),
            telemetry.before_mixed(),
            telemetry.after_unique(),
            telemetry.after_source_or_family(),
            telemetry.final_target_guard_retained(),
        ),
        (10, 0, 10, 0, 1, 0, true)
    );
    assert_eq!(
        (
            cleared.source_cofactors().len(),
            cleared.physical_terms().len(),
            cleared.target_coefficient().raw().nterms(),
            cleared
                .physical_terms()
                .iter()
                .map(|term| term.coefficient().raw().nterms())
                .sum::<usize>(),
            cleared.exact_operations(),
            cleared.gcd_term_pairs(),
            cleared.retained_polynomial_terms(),
        ),
        (6, 14, 1, 26, 422, 10, 221)
    );
}
