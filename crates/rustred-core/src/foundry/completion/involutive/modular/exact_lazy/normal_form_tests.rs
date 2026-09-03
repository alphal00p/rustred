//! Complete exact-lazy frozen normal-form differentials.

use crate::algebra::IndexedCoefficientContext;
use crate::foundry::artifact::{
    derive_one_loop_unit_mass_tadpole, derive_two_loop_unit_mass_sunset,
};
use crate::foundry::completion::CompletionGeometryLimits;
use crate::identity::{CompletedIbpSourceRows, ParametricIbpGenerator};
use crate::sector::{Mask, OrderingPolicy};

use super::super::super::{
    ForwardShift, InvolutiveLimits, OrdinaryChartLiftLimits, OreConsequence, OreOrderingAdapter,
    try_janet_normal_form, try_lift_completed_ordinary_sources, try_preprocess_initial_basis,
};
use super::import::{try_build_planned_exact_consequence, try_plan_exact_consequence_import};
use super::*;

const PRIME_A: u64 = 998_244_353;
const PRIME_B: u64 = 1_000_000_007;

fn complete_ordinary(generator: &ParametricIbpGenerator<'_>) -> CompletedIbpSourceRows {
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    prepared.complete(rows).unwrap()
}

fn ordering(
    completed: &CompletedIbpSourceRows,
    mask: impl IntoIterator<Item = bool>,
    exact: InvolutiveLimits,
) -> OreOrderingAdapter {
    OreOrderingAdapter::try_new_for_completed(
        OrderingPolicy::default(),
        Mask::try_new(mask).unwrap(),
        completed,
        exact,
    )
    .unwrap()
}

fn lift<'a>(
    completed: &'a CompletedIbpSourceRows,
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    exact: InvolutiveLimits,
) -> super::super::super::LiftedOrdinarySourceBatch {
    let mut limits = OrdinaryChartLiftLimits::default();
    limits.involutive = exact;
    try_lift_completed_ordinary_sources(completed, ordering, context, limits).unwrap()
}

fn schedule(
    session: &ExactLazySession<'_>,
    context: &IndexedCoefficientContext,
) -> ExactLazyProbeSchedule {
    let width = context.base().parameter_names().len() + context.index_count();
    ExactLazyProbeSchedule::try_new(
        session.owner(),
        session.coefficient_dag(),
        context,
        [
            ExactLazyProbeSpec::new(0, PRIME_A, vec![7; width]),
            ExactLazyProbeSpec::new(1, PRIME_B, vec![11; width]),
        ],
    )
    .unwrap()
}

fn shifted_source(
    source: &OreConsequence,
    operator_shift: ForwardShift,
    multiplier: &crate::algebra::IndexedCoefficient,
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    exact: InvolutiveLimits,
) -> OreConsequence {
    OreConsequence::try_zero(ordering, context, exact)
        .unwrap()
        .try_left_axpy(
            multiplier,
            &operator_shift,
            source,
            ordering,
            context,
            exact,
        )
        .unwrap()
}

fn assert_same_trace(
    lazy: &ExactLazyFullJanetNormalForm,
    exact: &super::super::super::JanetNormalForm,
) {
    assert_eq!(lazy.steps().len(), exact.steps().len());
    for (lazy, exact) in lazy.steps().iter().zip(exact.steps()) {
        assert_eq!(lazy.divisor_ordinal(), exact.divisor_ordinal());
        assert_eq!(lazy.target_shift(), exact.target_shift());
        assert_eq!(lazy.operator_shift(), exact.operator_shift());
    }
}

#[test]
fn frozen_cursor_rejects_an_aborted_all_preinterned_subject() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ExactLazyLimits::default();
    let ordering = ordering(&completed, [true], limits.exact);
    let context = generator.context();
    let basis_rows = lift(&completed, &ordering, context, limits.exact)
        .try_into_consequences(&completed, &ordering, context, limits.exact)
        .unwrap();
    let initial = try_preprocess_initial_basis(
        basis_rows.into_vec(),
        &ordering,
        context,
        limits.exact,
        CompletionGeometryLimits::default(),
    )
    .unwrap();
    let exact_subject = initial.epoch().elements()[0].consequence();
    let mut session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let frozen = ExactLazyFrozenJanetEpoch::try_import(
        &mut session,
        initial.epoch().division(),
        &ordering,
        context,
        limits,
    )
    .unwrap();
    try_import_exact_consequence(&mut session, exact_subject, &ordering, context, limits).unwrap();
    let plan =
        try_plan_exact_consequence_import(&session, exact_subject, &ordering, context, limits)
            .unwrap();
    let census = plan.census();
    let mut transaction = session
        .try_begin_import_batch_transaction(&[census])
        .unwrap();
    let escaped =
        try_build_planned_exact_consequence(&mut transaction, &plan, &ordering, context, limits)
            .unwrap();
    transaction.try_abort().unwrap();
    let ledger = ExactLazyCompletionLedger::try_new(&session, &ordering, context, limits).unwrap();

    assert_eq!(
        ExactLazyJanetCursor::try_new(
            &session, &frozen, escaped, None, &ordering, context, &ledger, limits,
        )
        .unwrap_err(),
        ExactLazyError::InvalidProof {
            detail: "exact-lazy consequence did not cross its transaction commit boundary",
        }
    );
}

#[test]
fn generated_one_loop_source_has_complete_exact_trajectory_and_replay_for_both_actions() {
    for active in [true, false] {
        let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
        let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
        let completed = complete_ordinary(&generator);
        assert_eq!(completed.source_row_count(), 1);
        let limits = ExactLazyLimits::default();
        let ordering = ordering(&completed, [active], limits.exact);
        let context = generator.context();

        let basis_rows = lift(&completed, &ordering, context, limits.exact)
            .try_into_consequences(&completed, &ordering, context, limits.exact)
            .unwrap();
        let initial = try_preprocess_initial_basis(
            basis_rows.into_vec(),
            &ordering,
            context,
            limits.exact,
            CompletionGeometryLimits::default(),
        )
        .unwrap();
        let source_rows = lift(&completed, &ordering, context, limits.exact);
        let denominator = context
            .add(&context.index(0).unwrap(), &context.integer(17))
            .unwrap();
        let multiplier = context.div(&context.integer(5), &denominator).unwrap();
        let exact_subject = shifted_source(
            source_rows.sources()[0].consequence(),
            ForwardShift::try_new([3], limits.exact).unwrap(),
            &multiplier,
            &ordering,
            context,
            limits.exact,
        );

        let mut session =
            ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
        let frozen = ExactLazyFrozenJanetEpoch::try_import(
            &mut session,
            initial.epoch().division(),
            &ordering,
            context,
            limits,
        )
        .unwrap();
        let lazy_subject =
            try_import_exact_consequence(&mut session, &exact_subject, &ordering, context, limits)
                .unwrap();
        let schedule = schedule(&session, context);
        let mut ledger =
            ExactLazyCompletionLedger::try_new(&session, &ordering, context, limits).unwrap();
        let exact = try_janet_normal_form(
            exact_subject,
            initial.epoch(),
            &ordering,
            context,
            limits.exact,
        )
        .unwrap();
        assert!(!exact.steps().is_empty());
        let lazy = try_exact_lazy_full_janet_normal_form(
            &mut session,
            &frozen,
            lazy_subject,
            &ordering,
            context,
            &schedule,
            &mut ledger,
            limits,
        )
        .unwrap();

        assert_same_trace(&lazy, &exact);
        assert_eq!(lazy.work_census().normal_form_steps(), exact.steps().len());
        assert_eq!(
            lazy.support_census().classification_attempts(),
            exact.steps().len()
        );
        assert_eq!(
            lazy.trace_bytes(),
            lazy.work_census().normal_form_trace_bytes()
        );
        assert!(lazy.divisor_visits() > 0);

        let lowering_limits = ExactLazyLoweringLimits::for_session(&session);
        let mut lowering = ExactLazyLoweringBudget::try_new(&session, lowering_limits).unwrap();
        let lowered = try_lower_for_exact_replay(
            &mut session,
            lazy.remainder(),
            &ordering,
            context,
            &mut lowering,
        )
        .unwrap();
        assert_eq!(lowered.consequence(), exact.remainder(), "active={active}");
    }
}

#[test]
fn every_generated_k3_source_has_complete_exact_trajectory_and_replay_with_inactive_action() {
    for mask in [[true, true, true], [true, false, true]] {
        let artifact = derive_two_loop_unit_mass_sunset().unwrap();
        let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
        let completed = complete_ordinary(&generator);
        assert_eq!(completed.source_row_count(), 4);
        let limits = ExactLazyLimits::default();
        let ordering = ordering(&completed, mask, limits.exact);
        let context = generator.context();

        let basis_rows = lift(&completed, &ordering, context, limits.exact)
            .try_into_consequences(&completed, &ordering, context, limits.exact)
            .unwrap();
        let initial = try_preprocess_initial_basis(
            basis_rows.into_vec(),
            &ordering,
            context,
            limits.exact,
            CompletionGeometryLimits::default(),
        )
        .unwrap();
        let source_rows = lift(&completed, &ordering, context, limits.exact);
        let mut session =
            ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
        let frozen = ExactLazyFrozenJanetEpoch::try_import(
            &mut session,
            initial.epoch().division(),
            &ordering,
            context,
            limits,
        )
        .unwrap();
        let schedule = schedule(&session, context);
        let mut ledger =
            ExactLazyCompletionLedger::try_new(&session, &ordering, context, limits).unwrap();
        let lowering_limits = ExactLazyLoweringLimits::for_session(&session);
        let mut lowering = ExactLazyLoweringBudget::try_new(&session, lowering_limits).unwrap();

        for (ordinal, source) in source_rows.sources().iter().enumerate() {
            let steps_before = ledger.work_census().normal_form_steps();
            let classifications_before = ledger.support_census().classification_attempts();
            let multiplier = context.integer(ordinal as i64 + 2);
            let exact_subject = shifted_source(
                source.consequence(),
                ForwardShift::try_zero(3, limits.exact).unwrap(),
                &multiplier,
                &ordering,
                context,
                limits.exact,
            );
            let lazy_subject = try_import_exact_consequence(
                &mut session,
                &exact_subject,
                &ordering,
                context,
                limits,
            )
            .unwrap();
            let exact = try_janet_normal_form(
                exact_subject,
                initial.epoch(),
                &ordering,
                context,
                limits.exact,
            )
            .unwrap();
            assert!(!exact.steps().is_empty(), "mask={mask:?}, source={ordinal}");
            let lazy = try_exact_lazy_full_janet_normal_form(
                &mut session,
                &frozen,
                lazy_subject,
                &ordering,
                context,
                &schedule,
                &mut ledger,
                limits,
            )
            .unwrap();

            assert_same_trace(&lazy, &exact);
            assert_eq!(
                lazy.work_census().normal_form_steps() - steps_before,
                exact.steps().len()
            );
            assert_eq!(
                lazy.support_census().classification_attempts() - classifications_before,
                exact.steps().len()
            );
            let lowered = try_lower_for_exact_replay(
                &mut session,
                lazy.remainder(),
                &ordering,
                context,
                &mut lowering,
            )
            .unwrap_or_else(|error| {
                panic!("mask={mask:?}, source={ordinal}: cold replay failed: {error:?}")
            });
            assert_eq!(
                lowered.consequence(),
                exact.remainder(),
                "mask={mask:?}, source={ordinal}"
            );
        }
        assert_eq!(lowering.census().successful_lowerings(), 4);
    }
}

#[test]
fn cursor_cannot_be_finalized_before_complete_support_reports_irreducible() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ExactLazyLimits::default();
    let ordering = ordering(&completed, [true], limits.exact);
    let context = generator.context();
    let basis_rows = lift(&completed, &ordering, context, limits.exact)
        .try_into_consequences(&completed, &ordering, context, limits.exact)
        .unwrap();
    let initial = try_preprocess_initial_basis(
        basis_rows.into_vec(),
        &ordering,
        context,
        limits.exact,
        CompletionGeometryLimits::default(),
    )
    .unwrap();
    let source_rows = lift(&completed, &ordering, context, limits.exact);
    let exact_subject = shifted_source(
        source_rows.sources()[0].consequence(),
        ForwardShift::try_new([1], limits.exact).unwrap(),
        &context.one(),
        &ordering,
        context,
        limits.exact,
    );
    let mut session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let frozen = ExactLazyFrozenJanetEpoch::try_import(
        &mut session,
        initial.epoch().division(),
        &ordering,
        context,
        limits,
    )
    .unwrap();
    let lazy_subject =
        try_import_exact_consequence(&mut session, &exact_subject, &ordering, context, limits)
            .unwrap();
    let mut ledger =
        ExactLazyCompletionLedger::try_new(&session, &ordering, context, limits).unwrap();
    let cursor = ExactLazyJanetCursor::try_new(
        &session,
        &frozen,
        lazy_subject,
        None,
        &ordering,
        context,
        &ledger,
        limits,
    )
    .unwrap();

    assert_eq!(
        cursor
            .try_into_full_normal_form(&session, &frozen, &ordering, context, &mut ledger,)
            .unwrap_err(),
        ExactLazyError::InvalidSupport {
            detail: "an exact-lazy Janet cursor was finalized before irreducibility",
        }
    );
}

#[test]
fn full_and_self_excluding_finalization_authorities_are_not_interchangeable() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ExactLazyLimits::default();
    let ordering = ordering(&completed, [true], limits.exact);
    let context = generator.context();
    let basis_rows = lift(&completed, &ordering, context, limits.exact)
        .try_into_consequences(&completed, &ordering, context, limits.exact)
        .unwrap();
    let initial = try_preprocess_initial_basis(
        basis_rows.into_vec(),
        &ordering,
        context,
        limits.exact,
        CompletionGeometryLimits::default(),
    )
    .unwrap();
    let exact_subject = initial.epoch().elements()[0].consequence();
    let mut session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let frozen = ExactLazyFrozenJanetEpoch::try_import(
        &mut session,
        initial.epoch().division(),
        &ordering,
        context,
        limits,
    )
    .unwrap();
    let schedule = schedule(&session, context);
    let mut ledger =
        ExactLazyCompletionLedger::try_new(&session, &ordering, context, limits).unwrap();

    let self_subject =
        try_import_exact_consequence(&mut session, exact_subject, &ordering, context, limits)
            .unwrap();
    let mut self_cursor = ExactLazyJanetCursor::try_new(
        &session,
        &frozen,
        self_subject,
        Some(0),
        &ordering,
        context,
        &ledger,
        limits,
    )
    .unwrap();
    assert_eq!(
        self_cursor
            .try_cancel_once(
                &mut session,
                &frozen,
                &ordering,
                context,
                &schedule,
                &mut ledger,
            )
            .unwrap(),
        ExactLazyCancellationOutcome::Irreducible
    );
    assert_eq!(
        self_cursor
            .try_into_full_normal_form(&session, &frozen, &ordering, context, &mut ledger,)
            .unwrap_err(),
        ExactLazyError::WrongNormalFormMode {
            expected: "full",
            actual: "self-excluding",
        }
    );

    let full_subject =
        try_import_exact_consequence(&mut session, exact_subject, &ordering, context, limits)
            .unwrap();
    let mut full_cursor = ExactLazyJanetCursor::try_new(
        &session,
        &frozen,
        full_subject,
        None,
        &ordering,
        context,
        &ledger,
        limits,
    )
    .unwrap();
    full_cursor
        .try_reduce_to_irreducible(
            &mut session,
            &frozen,
            &ordering,
            context,
            &schedule,
            &mut ledger,
        )
        .unwrap();
    assert_eq!(
        full_cursor
            .try_into_self_excluded_normal_form(&session, &frozen, &ordering, context, &mut ledger,)
            .unwrap_err(),
        ExactLazyError::WrongNormalFormMode {
            expected: "self-excluding",
            actual: "full",
        }
    );

    let self_subject =
        try_import_exact_consequence(&mut session, exact_subject, &ordering, context, limits)
            .unwrap();
    let self_normal_form = try_exact_lazy_self_excluded_janet_normal_form(
        &mut session,
        &frozen,
        self_subject,
        0,
        &ordering,
        context,
        &schedule,
        &mut ledger,
        limits,
    )
    .unwrap();
    assert_eq!(self_normal_form.excluded_divisor(), 0);
    self_normal_form
        .require_binding(
            &session,
            frozen.epoch(),
            &ordering,
            context,
            &ledger,
            limits,
        )
        .unwrap();
}
