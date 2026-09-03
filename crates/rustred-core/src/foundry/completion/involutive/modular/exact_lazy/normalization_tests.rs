//! Guarded monic-normalization tests.

use crate::algebra::{IndexedCoefficient, IndexedCoefficientContext, IndexedPolynomial};
use crate::foundry::artifact::derive_one_loop_unit_mass_tadpole;
use crate::foundry::completion::CompletionGeometryLimits;
use crate::foundry::completion::involutive::limits::InvolutiveWorkBudget;
use crate::identity::{CompletedIbpSourceRows, ParametricIbpGenerator};
use crate::sector::{Mask, OrderingPolicy};

use super::super::super::{
    ForwardShift, InvolutiveLimits, JanetBasisEpoch, OrdinaryChartLiftLimits, OreConsequence,
    OreOrderingAdapter, OreRow, try_lift_completed_ordinary_sources,
};
use super::super::{
    ExactMaterializationBudget, ExactMaterializerLimits, try_materialize_exact_batch,
};
use super::provenance::SourceDerivationNodeView;
use super::*;

const PRIME: u64 = 998_244_353;

fn complete_ordinary(generator: &ParametricIbpGenerator<'_>) -> CompletedIbpSourceRows {
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    prepared.complete(rows).unwrap()
}

fn ordering(completed: &CompletedIbpSourceRows, limits: ExactLazyLimits) -> OreOrderingAdapter {
    OreOrderingAdapter::try_new_for_completed(
        OrderingPolicy::default(),
        Mask::try_new([true]).unwrap(),
        completed,
        limits.exact,
    )
    .unwrap()
}

fn shift(value: u64, limits: ExactLazyLimits) -> ForwardShift {
    ForwardShift::try_new([value], limits.exact).unwrap()
}

fn point(context: &IndexedCoefficientContext, index: i64) -> Vec<i64> {
    let mut point = vec![7; context.base().parameter_names().len() + context.index_count()];
    *point.last_mut().unwrap() = index;
    point
}

fn schedule(
    session: &ExactLazySession<'_>,
    context: &IndexedCoefficientContext,
    index: i64,
) -> ExactLazyProbeSchedule {
    ExactLazyProbeSchedule::try_new(
        session.owner(),
        session.coefficient_dag(),
        context,
        [ExactLazyProbeSpec::new(0, PRIME, point(context, index))],
    )
    .unwrap()
}

fn rational_parts(
    context: &IndexedCoefficientContext,
) -> (IndexedCoefficient, IndexedPolynomial, IndexedPolynomial) {
    let n = context.index(0).unwrap();
    let numerator = context.add(&n, &context.integer(-1)).unwrap();
    let denominator = context.add(&n, &context.integer(2)).unwrap();
    let rational = context.div(&numerator, &denominator).unwrap();
    let numerator_guard = context
        .numerator_condition_with_limits(
            &numerator,
            InvolutiveLimits::default().indexed_algebra.exact_algebra,
        )
        .unwrap();
    let denominator_guard = context
        .numerator_condition_with_limits(
            &denominator,
            InvolutiveLimits::default().indexed_algebra.exact_algebra,
        )
        .unwrap();
    (rational, numerator_guard, denominator_guard)
}

fn rational_consequence(
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    limits: ExactLazyLimits,
) -> OreConsequence {
    let (rational, _, denominator_guard) = rational_parts(context);
    OreConsequence::try_from_source(
        0,
        OreRow::try_new(
            ordering,
            [
                (shift(0, limits), context.integer(2)),
                (shift(1, limits), rational),
            ],
            context,
            limits.exact,
        )
        .unwrap(),
        ordering,
        context,
        limits.exact,
    )
    .unwrap()
    .try_require_nonzero_guard(denominator_guard, context, limits.exact)
    .unwrap()
    .0
}

fn sealed_scaled_consequence(
    completed: &CompletedIbpSourceRows,
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    limits: ExactLazyLimits,
) -> OreConsequence {
    let mut lift_limits = OrdinaryChartLiftLimits::default();
    lift_limits.involutive = limits.exact;
    let sources = try_lift_completed_ordinary_sources(completed, ordering, context, lift_limits)
        .unwrap()
        .try_into_consequences(completed, ordering, context, limits.exact)
        .unwrap();
    let base = JanetBasisEpoch::try_initial(
        sources.into_vec(),
        ordering,
        context,
        limits.exact,
        CompletionGeometryLimits::default(),
    )
    .unwrap();
    assert_eq!(base.elements().len(), 1);
    let (rational, _, _) = rational_parts(context);
    OreConsequence::try_zero(ordering, context, limits.exact)
        .unwrap()
        .try_left_axpy(
            &rational,
            &shift(0, limits),
            base.elements()[0].consequence(),
            ordering,
            context,
            limits.exact,
        )
        .unwrap()
}

fn materialize(
    dag: &super::super::ModularCoefficientDag,
    context: &IndexedCoefficientContext,
    coefficients: &[LazyCoeff],
) -> Vec<IndexedCoefficient> {
    let roots: Vec<_> = coefficients
        .iter()
        .map(|coefficient| coefficient.root().clone())
        .collect();
    let mut budget = ExactMaterializationBudget::new(ExactMaterializerLimits::default());
    try_materialize_exact_batch(dag, context, &roots, &mut budget)
        .unwrap()
        .into_materializations()
        .into_vec()
        .into_iter()
        .map(|value| value.value().clone())
        .collect()
}

#[test]
fn rational_leader_normalization_scales_row_and_derivation_and_keeps_both_guards() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ExactLazyLimits::default();
    let ordering = ordering(&completed, limits);
    let context = generator.context();
    let exact = rational_consequence(&ordering, context, limits);
    let mut session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let imported =
        try_import_exact_consequence(&mut session, &exact, &ordering, context, limits).unwrap();
    let original_derivation = imported.derivation().root().clone();
    let original_census = imported.census();
    let probe_schedule = schedule(&session, context, 5);
    let mut ledger =
        ExactLazyCompletionLedger::try_new(&session, &ordering, context, limits).unwrap();

    let normalized = try_normalize_monic_test_local(
        &mut session,
        &imported,
        &ordering,
        context,
        &probe_schedule,
        &mut ledger,
        limits,
    )
    .unwrap()
    .expect("the rational leader is nonmonic");

    assert_eq!(
        normalized.census().physical_terms(),
        original_census.physical_terms()
    );
    assert_eq!(
        normalized.census().provenance_terms(),
        original_census.provenance_terms()
    );
    assert_eq!(
        normalized.census().guard_descriptors(),
        original_census.guard_descriptors() + 1
    );
    let terms = normalized.row().try_terms_live(&session).unwrap();
    let leader = normalized
        .row()
        .try_leading_term(&session, &ordering)
        .unwrap()
        .unwrap();
    assert_eq!(leader.shift().values(), &[1]);
    assert_eq!(leader.coefficient(), &session.one());
    assert!(matches!(
        leader.nonzero_proof(),
        ExactNonzeroProof::GuardedStructuralOne(_)
    ));

    let exact_coefficients = materialize(
        session.coefficient_dag(),
        context,
        &terms
            .iter()
            .map(|term| term.coefficient().clone())
            .collect::<Vec<_>>(),
    );
    let (rational, numerator_guard, denominator_guard) = rational_parts(context);
    let expected_inverse = context.div(&context.one(), &rational).unwrap();
    assert_eq!(exact_coefficients[1], context.one());
    assert_eq!(
        exact_coefficients[0],
        context.mul(&context.integer(2), &expected_inverse).unwrap()
    );

    let mut inspection = session.try_begin_transaction().unwrap();
    match inspection
        .try_derivation_node_view(normalized.derivation().root())
        .unwrap()
    {
        SourceDerivationNodeView::Axpy {
            target,
            multiplier,
            source,
        } => {
            assert_eq!(source, original_derivation);
            assert!(matches!(
                inspection.try_derivation_node_view(&target).unwrap(),
                SourceDerivationNodeView::Zero
            ));
            assert_eq!(
                materialize(inspection.coefficient_dag(), context, &[multiplier]),
                vec![expected_inverse]
            );
        }
        other => panic!("expected one scalar AXPY derivation, got {other:?}"),
    }
    let requirements = inspection
        .try_collect_guard_probe_requirements(normalized.guards().root(), &ordering)
        .unwrap();
    let guard_coefficients: Vec<_> = requirements
        .iter()
        .map(|requirement| match requirement {
            GuardProbeRequirement::Nonzero(root) | GuardProbeRequirement::Defined(root) => {
                root.clone()
            }
        })
        .collect();
    let exact_guards = materialize(inspection.coefficient_dag(), context, &guard_coefficients)
        .into_iter()
        .map(|coefficient| {
            context
                .numerator_condition_with_limits(
                    &coefficient,
                    limits.exact.indexed_algebra.exact_algebra,
                )
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert!(exact_guards.contains(&denominator_guard));
    assert!(exact_guards.contains(&numerator_guard));
    inspection.try_abort().unwrap();
}

#[test]
fn numerator_exceptional_fibre_is_rejected_and_retained_for_exact_fallback() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ExactLazyLimits::default();
    let ordering = ordering(&completed, limits);
    let context = generator.context();
    let exact = rational_consequence(&ordering, context, limits);
    let mut session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let imported =
        try_import_exact_consequence(&mut session, &exact, &ordering, context, limits).unwrap();
    let exceptional = schedule(&session, context, 1);
    let mut ledger =
        ExactLazyCompletionLedger::try_new(&session, &ordering, context, limits).unwrap();

    let normalized = try_normalize_monic_test_local(
        &mut session,
        &imported,
        &ordering,
        context,
        &exceptional,
        &mut ledger,
        limits,
    )
    .unwrap()
    .unwrap();

    assert_eq!(ledger.support_census().scheduled_probes(), 1);
    assert_eq!(ledger.support_census().successful_probes(), 0);
    assert_eq!(ledger.support_census().rejected_probes(), 1);
    assert!(ledger.support_census().exact_fallback_roots() > 0);
    assert_eq!(
        normalized.guards().descriptor_count(),
        imported.guards().descriptor_count() + 1
    );
}

#[test]
fn normalized_derivation_cold_replays_the_exact_monic_source() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ExactLazyLimits::default();
    let ordering = ordering(&completed, limits);
    let context = generator.context();
    let exact = sealed_scaled_consequence(&completed, &ordering, context, limits);
    let mut exact_work = InvolutiveWorkBudget::default();
    let expected = exact
        .try_monic_copy_sealed(&ordering, context, limits.exact, &mut exact_work)
        .unwrap()
        .expect("a nontrivial scalar multiple must need normalization");
    let mut session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let imported =
        try_import_exact_consequence(&mut session, &exact, &ordering, context, limits).unwrap();
    let probe_schedule = schedule(&session, context, 5);
    let mut ledger =
        ExactLazyCompletionLedger::try_new(&session, &ordering, context, limits).unwrap();
    let normalized = try_normalize_monic_test_local(
        &mut session,
        &imported,
        &ordering,
        context,
        &probe_schedule,
        &mut ledger,
        limits,
    )
    .unwrap()
    .unwrap();
    let lowering_limits = ExactLazyLoweringLimits::for_session(&session);
    let mut lowering = ExactLazyLoweringBudget::try_new(&session, lowering_limits).unwrap();
    let lowered =
        try_lower_for_exact_replay(&mut session, &normalized, &ordering, context, &mut lowering)
            .unwrap();

    assert_eq!(lowered.consequence(), &expected);
    assert_eq!(lowering.census().successful_lowerings(), 1);
}

#[test]
fn leader_inverse_seal_rejects_foreign_stale_and_different_row_use() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ExactLazyLimits::default();
    let ordering = ordering(&completed, limits);
    let context = generator.context();
    let first_exact = rational_consequence(&ordering, context, limits);
    let second_exact = OreConsequence::try_from_source(
        0,
        OreRow::try_new(
            &ordering,
            [
                (shift(0, limits), context.integer(3)),
                (shift(2, limits), context.index(0).unwrap()),
            ],
            context,
            limits.exact,
        )
        .unwrap(),
        &ordering,
        context,
        limits.exact,
    )
    .unwrap();
    let mut first = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let first_row =
        try_import_exact_consequence(&mut first, &first_exact, &ordering, context, limits).unwrap();
    let second_row =
        try_import_exact_consequence(&mut first, &second_exact, &ordering, context, limits)
            .unwrap();
    let mut foreign = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();

    let mut transaction = first.try_begin_transaction().unwrap();
    let seal = transaction
        .try_actual_leader_inverse(first_row.row(), &ordering)
        .unwrap();
    assert_eq!(seal.leader_shift().values(), &[1]);
    assert_ne!(
        seal.leader(),
        first_row
            .row()
            .try_terms_in_transaction(&transaction)
            .unwrap()[0]
            .coefficient()
    );
    assert!(matches!(
        transaction.try_guarded_structural_one(&seal, second_row.row(), &ordering),
        Err(ExactLazyError::InvalidProof { .. })
    ));

    let foreign_transaction = foreign.try_begin_transaction().unwrap();
    assert_eq!(
        foreign_transaction
            .try_guarded_structural_one(&seal, first_row.row(), &ordering)
            .unwrap_err(),
        ExactLazyError::WrongSessionOwner
    );
    foreign_transaction.try_abort().unwrap();
    transaction.try_abort().unwrap();

    let replacement = first.try_begin_transaction().unwrap();
    assert!(matches!(
        replacement.try_guarded_structural_one(&seal, first_row.row(), &ordering),
        Err(ExactLazyError::Modular(
            super::super::ModularGuideError::StaleDagReference { .. }
        ))
    ));
    replacement.try_abort().unwrap();
}

#[test]
fn already_monic_row_is_an_allocation_free_none_path() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ExactLazyLimits::default();
    let ordering = ordering(&completed, limits);
    let context = generator.context();
    let exact = OreConsequence::try_from_source(
        0,
        OreRow::try_new(
            &ordering,
            [
                (shift(0, limits), context.integer(2)),
                (shift(1, limits), context.one()),
            ],
            context,
            limits.exact,
        )
        .unwrap(),
        &ordering,
        context,
        limits.exact,
    )
    .unwrap();
    let mut session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let imported =
        try_import_exact_consequence(&mut session, &exact, &ordering, context, limits).unwrap();
    let probe_schedule = schedule(&session, context, 5);
    let mut ledger =
        ExactLazyCompletionLedger::try_new(&session, &ordering, context, limits).unwrap();
    let floor = session.committed_floor();
    let census = session.census();

    assert!(
        try_normalize_monic_test_local(
            &mut session,
            &imported,
            &ordering,
            context,
            &probe_schedule,
            &mut ledger,
            limits,
        )
        .unwrap()
        .is_none()
    );
    assert_eq!(session.committed_floor(), floor);
    assert_eq!(session.census(), census);
    assert_eq!(ledger.support_census(), ExactLazySupportCensus::default());
}

#[test]
fn one_below_guard_arena_cap_rolls_back_all_live_arenas() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let mut limits = ExactLazyLimits::default();
    // Empty + imported polynomial + new NumeratorOf fit; their required
    // union is exactly one node beyond this cap.
    limits.max_guard_lineage_nodes = 3;
    let ordering = ordering(&completed, limits);
    let context = generator.context();
    let exact = rational_consequence(&ordering, context, limits);
    let mut session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let imported =
        try_import_exact_consequence(&mut session, &exact, &ordering, context, limits).unwrap();
    let probe_schedule = schedule(&session, context, 5);
    let mut ledger =
        ExactLazyCompletionLedger::try_new(&session, &ordering, context, limits).unwrap();
    let floor = session.committed_floor();
    let live_coefficients = session.coefficient_live_census();
    let before_coefficients = session.coefficient_cumulative_census();
    let before_lineage = session.lineage_cumulative_census();

    assert_eq!(
        try_normalize_monic_test_local(
            &mut session,
            &imported,
            &ordering,
            context,
            &probe_schedule,
            &mut ledger,
            limits,
        )
        .unwrap_err(),
        ExactLazyError::ResourceLimit {
            resource: "exact-lazy guard-lineage nodes",
            requested: 4,
            limit: 3,
        }
    );
    assert_eq!(session.committed_floor(), floor);
    assert_eq!(session.coefficient_live_census(), live_coefficients);
    assert!(session.coefficient_cumulative_census().0 > before_coefficients.0);
    let after_lineage = session.lineage_cumulative_census();
    assert!(after_lineage.0.0 > before_lineage.0.0);
    assert!(after_lineage.1.0 > before_lineage.1.0);
    assert_eq!(ledger.support_census(), ExactLazySupportCensus::default());
}

#[test]
fn full_normal_form_normalization_cannot_swap_or_reset_its_campaign_ledger() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let mut limits = ExactLazyLimits::default();
    limits.support.max_classification_attempts = 1;
    let ordering = ordering(&completed, limits);
    let context = generator.context();
    let exact_divisor = OreConsequence::try_from_source(
        0,
        OreRow::try_new(
            &ordering,
            [
                (shift(0, limits), context.integer(2)),
                (shift(1, limits), context.one()),
            ],
            context,
            limits.exact,
        )
        .unwrap(),
        &ordering,
        context,
        limits.exact,
    )
    .unwrap();
    let exact_epoch = JanetBasisEpoch::try_initial(
        [exact_divisor],
        &ordering,
        context,
        limits.exact,
        CompletionGeometryLimits::default(),
    )
    .unwrap();
    let mut session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let frozen = ExactLazyFrozenJanetEpoch::try_import(
        &mut session,
        exact_epoch.division(),
        &ordering,
        context,
        limits,
    )
    .unwrap();
    let subject = try_import_exact_consequence(
        &mut session,
        &rational_consequence(&ordering, context, limits),
        &ordering,
        context,
        limits,
    )
    .unwrap();
    let probe_schedule = schedule(&session, context, 5);
    let mut ledger =
        ExactLazyCompletionLedger::try_new(&session, &ordering, context, limits).unwrap();
    let mut normal_form = try_exact_lazy_full_janet_normal_form(
        &mut session,
        &frozen,
        subject,
        &ordering,
        context,
        &probe_schedule,
        &mut ledger,
        limits,
    )
    .unwrap();
    assert_eq!(normal_form.steps().len(), 1);
    assert_eq!(ledger.support_census().classification_attempts(), 1);

    let mut sibling_ledger =
        ExactLazyCompletionLedger::try_new(&session, &ordering, context, limits).unwrap();
    let floor = session.committed_floor();
    assert_eq!(
        try_normalize_full_normal_form_monic(
            &mut session,
            &frozen,
            &mut normal_form,
            &ordering,
            context,
            &probe_schedule,
            &mut sibling_ledger,
            limits,
        )
        .unwrap_err(),
        ExactLazyError::WrongCompletionLedger
    );
    assert_eq!(session.committed_floor(), floor);
    assert_eq!(
        sibling_ledger.support_census(),
        ExactLazySupportCensus::default()
    );

    assert_eq!(
        try_normalize_full_normal_form_monic(
            &mut session,
            &frozen,
            &mut normal_form,
            &ordering,
            context,
            &probe_schedule,
            &mut ledger,
            limits,
        )
        .unwrap_err(),
        ExactLazyError::ResourceLimit {
            resource: "exact-lazy support-classification attempts",
            requested: 2,
            limit: 1,
        }
    );
    assert_eq!(session.committed_floor(), floor);
    assert_eq!(ledger.support_census().classification_attempts(), 2);
    normal_form
        .require_binding(
            &session,
            frozen.epoch(),
            &ordering,
            context,
            &ledger,
            limits,
        )
        .unwrap();

    assert_eq!(
        try_normalize_full_normal_form_monic(
            &mut session,
            &frozen,
            &mut normal_form,
            &ordering,
            context,
            &probe_schedule,
            &mut ledger,
            limits,
        )
        .unwrap_err(),
        ExactLazyError::ResourceLimit {
            resource: "exact-lazy support-classification attempts",
            requested: 3,
            limit: 1,
        }
    );
    assert_eq!(ledger.support_census().classification_attempts(), 3);
}

#[test]
fn guarded_normalization_preserves_full_normal_form_authority() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ExactLazyLimits::default();
    let ordering = ordering(&completed, limits);
    let context = generator.context();
    let exact_divisor = OreConsequence::try_from_source(
        0,
        OreRow::try_new(
            &ordering,
            [
                (shift(0, limits), context.integer(2)),
                (shift(2, limits), context.one()),
            ],
            context,
            limits.exact,
        )
        .unwrap(),
        &ordering,
        context,
        limits.exact,
    )
    .unwrap();
    let exact_epoch = JanetBasisEpoch::try_initial(
        [exact_divisor],
        &ordering,
        context,
        limits.exact,
        CompletionGeometryLimits::default(),
    )
    .unwrap();
    let mut session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let frozen = ExactLazyFrozenJanetEpoch::try_import(
        &mut session,
        exact_epoch.division(),
        &ordering,
        context,
        limits,
    )
    .unwrap();
    let subject = try_import_exact_consequence(
        &mut session,
        &rational_consequence(&ordering, context, limits),
        &ordering,
        context,
        limits,
    )
    .unwrap();
    let probe_schedule = schedule(&session, context, 5);
    let mut ledger =
        ExactLazyCompletionLedger::try_new(&session, &ordering, context, limits).unwrap();
    let mut normal_form = try_exact_lazy_full_janet_normal_form(
        &mut session,
        &frozen,
        subject,
        &ordering,
        context,
        &probe_schedule,
        &mut ledger,
        limits,
    )
    .unwrap();
    assert!(normal_form.steps().is_empty());

    assert!(
        try_normalize_full_normal_form_monic(
            &mut session,
            &frozen,
            &mut normal_form,
            &ordering,
            context,
            &probe_schedule,
            &mut ledger,
            limits,
        )
        .unwrap()
    );
    normal_form
        .require_binding(
            &session,
            frozen.epoch(),
            &ordering,
            context,
            &ledger,
            limits,
        )
        .unwrap();
    let leader = normal_form
        .remainder()
        .row()
        .try_leading_term(&session, &ordering)
        .unwrap()
        .unwrap();
    assert_eq!(leader.shift().values(), &[1]);
    assert_eq!(leader.coefficient(), &session.one());
    assert_eq!(ledger.support_census().classification_attempts(), 1);
    assert_eq!(
        normal_form.support_census().classification_attempts(),
        ledger.support_census().classification_attempts()
    );
}
