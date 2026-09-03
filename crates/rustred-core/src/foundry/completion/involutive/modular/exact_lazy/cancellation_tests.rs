//! Atomic exact-lazy Janet cancellation tests.

use crate::algebra::{IndexedCoefficient, IndexedCoefficientContext};
use crate::foundry::artifact::derive_two_loop_unit_mass_sunset;
use crate::foundry::completion::CompletionGeometryLimits;
use crate::identity::{CompletedIbpSourceRows, ParametricIbpGenerator};
use crate::sector::{Mask, OrderingPolicy};

use super::super::super::{
    ForwardShift, InvolutiveError, JanetBasisEpoch, OreConsequence, OreOrderingAdapter, OreRow,
    try_janet_normal_form,
};
use super::super::{ExactMaterializationBudget, try_materialize_exact_batch};
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
        Mask::try_new([true, true, true]).unwrap(),
        completed,
        limits.exact,
    )
    .unwrap()
}

fn shift(values: [u64; 3], limits: ExactLazyLimits) -> ForwardShift {
    ForwardShift::try_new(values, limits.exact).unwrap()
}

fn consequence(
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    terms: impl IntoIterator<Item = ([u64; 3], IndexedCoefficient)>,
    limits: ExactLazyLimits,
) -> OreConsequence {
    let row = OreRow::try_new(
        ordering,
        terms
            .into_iter()
            .map(|(coordinates, coefficient)| (shift(coordinates, limits), coefficient)),
        context,
        limits.exact,
    )
    .unwrap();
    OreConsequence::try_from_source(0, row, ordering, context, limits.exact).unwrap()
}

fn divisor(
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    limits: ExactLazyLimits,
) -> OreConsequence {
    consequence(
        ordering,
        context,
        [
            ([0, 0, 0], context.index(1).unwrap()),
            ([1, 0, 0], context.one()),
        ],
        limits,
    )
}

fn subject(
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    limits: ExactLazyLimits,
) -> OreConsequence {
    let denominator = context
        .add(&context.index(0).unwrap(), &context.one())
        .unwrap();
    let rational = context.div(&context.one(), &denominator).unwrap();
    consequence(
        ordering,
        context,
        [([0, 3, 0], context.integer(3)), ([2, 0, 0], rational)],
        limits,
    )
}

fn epoch(
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    limits: ExactLazyLimits,
) -> JanetBasisEpoch {
    JanetBasisEpoch::try_initial(
        [divisor(ordering, context, limits)],
        ordering,
        context,
        limits.exact,
        CompletionGeometryLimits::default(),
    )
    .unwrap()
}

fn probe_schedule(
    session: &ExactLazySession<'_>,
    context: &IndexedCoefficientContext,
) -> ExactLazyProbeSchedule {
    let point = vec![7; context.base().parameter_names().len() + context.index_count()];
    ExactLazyProbeSchedule::try_new(
        session.owner(),
        session.coefficient_dag(),
        context,
        [ExactLazyProbeSpec::new(0, PRIME, point)],
    )
    .unwrap()
}

#[test]
fn greatest_reducible_nonleader_cancels_strictly_and_matches_exact_normal_form() {
    let artifact = derive_two_loop_unit_mass_sunset().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ExactLazyLimits::default();
    let ordering = ordering(&completed, limits);
    let context = generator.context();
    let epoch = epoch(&ordering, context, limits);

    let exact_subject = subject(&ordering, context, limits);
    let exact_leader = exact_subject
        .row()
        .try_leading_term(&ordering)
        .unwrap()
        .unwrap()
        .0;
    assert_eq!(exact_leader.shift().values(), &[0, 3, 0]);
    let exact =
        try_janet_normal_form(exact_subject, &epoch, &ordering, context, limits.exact).unwrap();
    assert_eq!(exact.steps().len(), 2);

    let mut session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let frozen = ExactLazyFrozenJanetEpoch::try_import(
        &mut session,
        epoch.division(),
        &ordering,
        context,
        limits,
    )
    .unwrap();
    let lazy_subject = try_import_exact_consequence(
        &mut session,
        &subject(&ordering, context, limits),
        &ordering,
        context,
        limits,
    )
    .unwrap();
    let mut ledger =
        ExactLazyCompletionLedger::try_new(&session, &ordering, context, limits).unwrap();
    let mut cursor = ExactLazyJanetCursor::try_new(
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
    let schedule = probe_schedule(&session, context);
    assert_eq!(
        cursor
            .try_cancel_once(
                &mut session,
                &frozen,
                &ordering,
                context,
                &schedule,
                &mut ledger,
            )
            .unwrap(),
        ExactLazyCancellationOutcome::Reduced
    );
    assert_eq!(cursor.trace()[0].target_shift().values(), &[2, 0, 0]);
    assert!(
        cursor
            .subject()
            .row()
            .try_exact_zero_elisions_live(&session)
            .unwrap()
            .iter()
            .any(|proof| proof.shift().values() == [2, 0, 0])
    );

    assert_eq!(
        cursor
            .try_cancel_once(
                &mut session,
                &frozen,
                &ordering,
                context,
                &schedule,
                &mut ledger,
            )
            .unwrap(),
        ExactLazyCancellationOutcome::Reduced
    );
    assert_eq!(cursor.trace()[1].target_shift().values(), &[1, 0, 0]);
    assert!(
        ordering.try_key(cursor.trace()[1].target_shift()).unwrap()
            < ordering.try_key(cursor.trace()[0].target_shift()).unwrap()
    );
    assert!(
        cursor
            .subject()
            .row()
            .try_exact_zero_elisions_live(&session)
            .unwrap()
            .iter()
            .any(|proof| proof.shift().values() == [1, 0, 0])
    );
    assert_eq!(
        cursor
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
    assert_eq!(cursor.trace().len(), 2);
    assert_eq!(cursor.work_census(&ledger).unwrap().normal_form_steps(), 2);

    let lazy_terms = cursor.subject().row().try_terms_live(&session).unwrap();
    let roots = lazy_terms
        .iter()
        .map(|term| term.coefficient().root().clone())
        .collect::<Vec<_>>();
    let mut materialization = ExactMaterializationBudget::new(limits.support.exact_fallback);
    let materialized = try_materialize_exact_batch(
        session.coefficient_dag(),
        context,
        &roots,
        &mut materialization,
    )
    .unwrap();
    let exact_terms = exact.remainder().row().terms();
    assert_eq!(lazy_terms.len(), exact_terms.len());
    for ((lazy, materialized), exact) in lazy_terms
        .iter()
        .zip(materialized.materializations())
        .zip(exact_terms)
    {
        assert_eq!(lazy.shift(), exact.shift());
        assert_eq!(materialized.value(), exact.coefficient());
    }
}

#[test]
fn classification_cap_rolls_back_subject_and_floor_but_keeps_attempted_work_charged() {
    let artifact = derive_two_loop_unit_mass_sunset().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let mut limits = ExactLazyLimits::default();
    limits.support.max_classification_attempts = 0;
    let ordering = ordering(&completed, limits);
    let context = generator.context();
    let epoch = epoch(&ordering, context, limits);
    let mut session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let frozen = ExactLazyFrozenJanetEpoch::try_import(
        &mut session,
        epoch.division(),
        &ordering,
        context,
        limits,
    )
    .unwrap();
    let lazy_subject = try_import_exact_consequence(
        &mut session,
        &subject(&ordering, context, limits),
        &ordering,
        context,
        limits,
    )
    .unwrap();
    let mut ledger =
        ExactLazyCompletionLedger::try_new(&session, &ordering, context, limits).unwrap();
    let mut cursor = ExactLazyJanetCursor::try_new(
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
    let before_terms = cursor
        .subject()
        .row()
        .try_terms_live(&session)
        .unwrap()
        .iter()
        .map(|term| (term.shift().clone(), term.coefficient().clone()))
        .collect::<Vec<_>>();
    let floor = session.committed_floor();
    let committed = session.census().committed_transactions();
    let attempted = session.census().transaction_attempts();
    let schedule = probe_schedule(&session, context);
    assert_eq!(
        cursor
            .try_cancel_once(
                &mut session,
                &frozen,
                &ordering,
                context,
                &schedule,
                &mut ledger,
            )
            .unwrap_err(),
        ExactLazyError::ResourceLimit {
            resource: "exact-lazy support-classification attempts",
            requested: 1,
            limit: 0,
        }
    );
    assert_eq!(session.committed_floor(), floor);
    assert_eq!(session.census().committed_transactions(), committed);
    assert_eq!(session.census().transaction_attempts(), attempted + 1);
    assert_eq!(
        cursor
            .support_census(&ledger)
            .unwrap()
            .classification_attempts(),
        1
    );
    assert_eq!(cursor.work_census(&ledger).unwrap().normal_form_steps(), 1);
    assert!(
        cursor
            .work_census(&ledger)
            .unwrap()
            .normal_form_trace_bytes()
            > 0
    );
    assert!(cursor.trace().is_empty());
    let after_terms = cursor
        .subject()
        .row()
        .try_terms_live(&session)
        .unwrap()
        .iter()
        .map(|term| (term.shift().clone(), term.coefficient().clone()))
        .collect::<Vec<_>>();
    assert_eq!(after_terms, before_terms);

    // The same cursor owns both ledgers. A retry therefore observes the next
    // cumulative attempt instead of receiving fresh caller-created budgets.
    assert_eq!(
        cursor
            .try_cancel_once(
                &mut session,
                &frozen,
                &ordering,
                context,
                &schedule,
                &mut ledger,
            )
            .unwrap_err(),
        ExactLazyError::ResourceLimit {
            resource: "exact-lazy support-classification attempts",
            requested: 2,
            limit: 0,
        }
    );
    assert_eq!(session.committed_floor(), floor);
    assert_eq!(session.census().committed_transactions(), committed);
    assert_eq!(session.census().transaction_attempts(), attempted + 2);
    assert_eq!(
        cursor
            .support_census(&ledger)
            .unwrap()
            .classification_attempts(),
        2
    );
    assert_eq!(cursor.work_census(&ledger).unwrap().normal_form_steps(), 2);
    assert!(cursor.trace().is_empty());
}

#[test]
fn foreign_session_and_foreign_epoch_are_rejected_without_cursor_mutation() {
    let artifact = derive_two_loop_unit_mass_sunset().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ExactLazyLimits::default();
    let ordering = ordering(&completed, limits);
    let context = generator.context();
    let first_epoch = epoch(&ordering, context, limits);
    let second_epoch = epoch(&ordering, context, limits);
    let mut session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let first_frozen = ExactLazyFrozenJanetEpoch::try_import(
        &mut session,
        first_epoch.division(),
        &ordering,
        context,
        limits,
    )
    .unwrap();
    let second_frozen = ExactLazyFrozenJanetEpoch::try_import(
        &mut session,
        second_epoch.division(),
        &ordering,
        context,
        limits,
    )
    .unwrap();
    let lazy_subject = try_import_exact_consequence(
        &mut session,
        &subject(&ordering, context, limits),
        &ordering,
        context,
        limits,
    )
    .unwrap();
    let mut ledger =
        ExactLazyCompletionLedger::try_new(&session, &ordering, context, limits).unwrap();
    let mut cursor = ExactLazyJanetCursor::try_new(
        &session,
        &first_frozen,
        lazy_subject,
        None,
        &ordering,
        context,
        &ledger,
        limits,
    )
    .unwrap();
    let schedule = probe_schedule(&session, context);
    let stale = cursor
        .try_cancel_once(
            &mut session,
            &second_frozen,
            &ordering,
            context,
            &schedule,
            &mut ledger,
        )
        .unwrap_err();
    assert_eq!(
        stale,
        ExactLazyError::Involutive(InvolutiveError::StaleEpoch {
            expected: second_frozen.epoch().clone(),
            actual: first_frozen.epoch().clone(),
        })
    );
    assert!(cursor.trace().is_empty());

    let mut foreign = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let foreign_schedule = probe_schedule(&foreign, context);
    assert_eq!(
        cursor
            .try_cancel_once(
                &mut foreign,
                &first_frozen,
                &ordering,
                context,
                &foreign_schedule,
                &mut ledger,
            )
            .unwrap_err(),
        ExactLazyError::WrongSessionOwner
    );
    assert!(cursor.trace().is_empty());
}

#[test]
fn one_below_trace_cap_stops_before_transaction_after_charging_step_attempt() {
    let artifact = derive_two_loop_unit_mass_sunset().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let mut limits = ExactLazyLimits::default();
    let step_bytes = std::mem::size_of::<ExactLazyReductionStep>() + 6 * std::mem::size_of::<u64>();
    limits.exact.max_normal_form_trace_bytes = step_bytes - 1;
    let ordering = ordering(&completed, limits);
    let context = generator.context();
    let epoch = epoch(&ordering, context, limits);
    let mut session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let frozen = ExactLazyFrozenJanetEpoch::try_import(
        &mut session,
        epoch.division(),
        &ordering,
        context,
        limits,
    )
    .unwrap();
    let lazy_subject = try_import_exact_consequence(
        &mut session,
        &subject(&ordering, context, limits),
        &ordering,
        context,
        limits,
    )
    .unwrap();
    let mut ledger =
        ExactLazyCompletionLedger::try_new(&session, &ordering, context, limits).unwrap();
    let mut cursor = ExactLazyJanetCursor::try_new(
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
    let schedule = probe_schedule(&session, context);
    let floor = session.committed_floor();
    let attempts = session.census().transaction_attempts();

    assert_eq!(
        cursor
            .try_cancel_once(
                &mut session,
                &frozen,
                &ordering,
                context,
                &schedule,
                &mut ledger,
            )
            .unwrap_err(),
        ExactLazyError::Involutive(InvolutiveError::ResourceLimit {
            resource: "Janet normal-form trace bytes",
            requested: step_bytes,
            limit: step_bytes - 1,
        })
    );
    assert_eq!(cursor.work_census(&ledger).unwrap().normal_form_steps(), 1);
    assert_eq!(session.census().transaction_attempts(), attempts);
    assert_eq!(session.committed_floor(), floor);
    assert!(cursor.trace().is_empty());
}

#[test]
fn cursor_and_final_authority_reject_sibling_ledger_and_sibling_epoch() {
    let artifact = derive_two_loop_unit_mass_sunset().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ExactLazyLimits::default();
    let ordering = ordering(&completed, limits);
    let context = generator.context();
    let first_epoch = epoch(&ordering, context, limits);
    let sibling_epoch = epoch(&ordering, context, limits);
    let mut session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let first_frozen = ExactLazyFrozenJanetEpoch::try_import(
        &mut session,
        first_epoch.division(),
        &ordering,
        context,
        limits,
    )
    .unwrap();
    let sibling_frozen = ExactLazyFrozenJanetEpoch::try_import(
        &mut session,
        sibling_epoch.division(),
        &ordering,
        context,
        limits,
    )
    .unwrap();
    let lazy_subject = try_import_exact_consequence(
        &mut session,
        &subject(&ordering, context, limits),
        &ordering,
        context,
        limits,
    )
    .unwrap();
    let mut ledger =
        ExactLazyCompletionLedger::try_new(&session, &ordering, context, limits).unwrap();
    let mut sibling_ledger =
        ExactLazyCompletionLedger::try_new(&session, &ordering, context, limits).unwrap();
    let mut cursor = ExactLazyJanetCursor::try_new(
        &session,
        &first_frozen,
        lazy_subject,
        None,
        &ordering,
        context,
        &ledger,
        limits,
    )
    .unwrap();
    let schedule = probe_schedule(&session, context);
    let floor = session.committed_floor();

    assert_eq!(
        cursor
            .try_cancel_once(
                &mut session,
                &first_frozen,
                &ordering,
                context,
                &schedule,
                &mut sibling_ledger,
            )
            .unwrap_err(),
        ExactLazyError::WrongCompletionLedger
    );
    assert_eq!(session.committed_floor(), floor);
    assert!(cursor.trace().is_empty());
    assert_eq!(ledger.support_census(), ExactLazySupportCensus::default());
    assert_eq!(
        sibling_ledger.support_census(),
        ExactLazySupportCensus::default()
    );

    cursor
        .try_reduce_to_irreducible(
            &mut session,
            &first_frozen,
            &ordering,
            context,
            &schedule,
            &mut ledger,
        )
        .unwrap();
    let normal_form = cursor
        .try_into_full_normal_form(&session, &first_frozen, &ordering, context, &mut ledger)
        .unwrap();
    normal_form
        .require_binding(
            &session,
            first_frozen.epoch(),
            &ordering,
            context,
            &ledger,
            limits,
        )
        .unwrap();
    assert_eq!(
        normal_form
            .require_binding(
                &session,
                first_frozen.epoch(),
                &ordering,
                context,
                &sibling_ledger,
                limits,
            )
            .unwrap_err(),
        ExactLazyError::WrongCompletionLedger
    );
    assert_eq!(
        normal_form
            .require_binding(
                &session,
                sibling_frozen.epoch(),
                &ordering,
                context,
                &ledger,
                limits,
            )
            .unwrap_err(),
        ExactLazyError::Involutive(InvolutiveError::StaleEpoch {
            expected: sibling_frozen.epoch().clone(),
            actual: first_frozen.epoch().clone(),
        })
    );
}

#[test]
fn support_budget_exhaustion_is_cumulative_across_two_subjects() {
    let artifact = derive_two_loop_unit_mass_sunset().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let mut limits = ExactLazyLimits::default();
    limits.support.max_classification_attempts = 2;
    let ordering = ordering(&completed, limits);
    let context = generator.context();
    let exact_epoch = epoch(&ordering, context, limits);
    let mut session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let frozen = ExactLazyFrozenJanetEpoch::try_import(
        &mut session,
        exact_epoch.division(),
        &ordering,
        context,
        limits,
    )
    .unwrap();
    let schedule = probe_schedule(&session, context);
    let mut ledger =
        ExactLazyCompletionLedger::try_new(&session, &ordering, context, limits).unwrap();

    let first = try_import_exact_consequence(
        &mut session,
        &subject(&ordering, context, limits),
        &ordering,
        context,
        limits,
    )
    .unwrap();
    let first = try_exact_lazy_full_janet_normal_form(
        &mut session,
        &frozen,
        first,
        &ordering,
        context,
        &schedule,
        &mut ledger,
        limits,
    )
    .unwrap();
    assert_eq!(first.steps().len(), 2);
    assert_eq!(ledger.support_census().classification_attempts(), 2);

    let second = try_import_exact_consequence(
        &mut session,
        &subject(&ordering, context, limits),
        &ordering,
        context,
        limits,
    )
    .unwrap();
    let mut second = ExactLazyJanetCursor::try_new(
        &session, &frozen, second, None, &ordering, context, &ledger, limits,
    )
    .unwrap();
    let floor = session.committed_floor();
    let committed = session.census().committed_transactions();

    assert_eq!(
        second
            .try_cancel_once(
                &mut session,
                &frozen,
                &ordering,
                context,
                &schedule,
                &mut ledger,
            )
            .unwrap_err(),
        ExactLazyError::ResourceLimit {
            resource: "exact-lazy support-classification attempts",
            requested: 3,
            limit: 2,
        }
    );
    assert_eq!(ledger.support_census().classification_attempts(), 3);
    assert_eq!(ledger.work_census().normal_form_steps(), 3);
    assert_eq!(session.committed_floor(), floor);
    assert_eq!(session.census().committed_transactions(), committed);
    assert!(second.trace().is_empty());
}

#[test]
fn involutive_work_exhaustion_is_cumulative_across_two_subjects() {
    let artifact = derive_two_loop_unit_mass_sunset().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let mut limits = ExactLazyLimits::default();
    limits.exact.max_normal_form_steps = 2;
    let ordering = ordering(&completed, limits);
    let context = generator.context();
    let exact_epoch = epoch(&ordering, context, limits);
    let mut session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let frozen = ExactLazyFrozenJanetEpoch::try_import(
        &mut session,
        exact_epoch.division(),
        &ordering,
        context,
        limits,
    )
    .unwrap();
    let schedule = probe_schedule(&session, context);
    let mut ledger =
        ExactLazyCompletionLedger::try_new(&session, &ordering, context, limits).unwrap();

    let first = try_import_exact_consequence(
        &mut session,
        &subject(&ordering, context, limits),
        &ordering,
        context,
        limits,
    )
    .unwrap();
    let first = try_exact_lazy_full_janet_normal_form(
        &mut session,
        &frozen,
        first,
        &ordering,
        context,
        &schedule,
        &mut ledger,
        limits,
    )
    .unwrap();
    assert_eq!(first.steps().len(), 2);
    assert_eq!(ledger.work_census().normal_form_steps(), 2);

    let second = try_import_exact_consequence(
        &mut session,
        &subject(&ordering, context, limits),
        &ordering,
        context,
        limits,
    )
    .unwrap();
    let mut second = ExactLazyJanetCursor::try_new(
        &session, &frozen, second, None, &ordering, context, &ledger, limits,
    )
    .unwrap();
    let floor = session.committed_floor();
    let attempts = session.census().transaction_attempts();
    let support_before = ledger.support_census();

    assert_eq!(
        second
            .try_cancel_once(
                &mut session,
                &frozen,
                &ordering,
                context,
                &schedule,
                &mut ledger,
            )
            .unwrap_err(),
        ExactLazyError::Involutive(InvolutiveError::ResourceLimit {
            resource: "Janet normal-form steps",
            requested: 3,
            limit: 2,
        })
    );
    assert_eq!(ledger.work_census().normal_form_steps(), 2);
    assert_eq!(ledger.support_census(), support_before);
    assert_eq!(session.committed_floor(), floor);
    assert_eq!(session.census().transaction_attempts(), attempts);
    assert!(second.trace().is_empty());
}
