//! Cold exact-lowering and sealed-source replay tests.

use crate::algebra::IndexedCoefficientContext;
use crate::foundry::artifact::{
    derive_one_loop_unit_mass_tadpole, derive_two_loop_unit_mass_sunset,
};
use crate::foundry::completion::CompletionGeometryLimits;
use crate::identity::{CompletedIbpSourceRows, ParametricIbpGenerator};
use crate::sector::{Mask, OrderingPolicy};

use super::super::super::{
    ForwardShift, InvolutiveLimits, JanetBasisEpoch, OrdinaryChartLiftLimits, OreConsequence,
    OreOrderingAdapter, OreRow, try_janet_normal_form, try_lift_completed_ordinary_sources,
    try_preprocess_initial_basis,
};
use super::import::{try_build_planned_exact_consequence, try_plan_exact_consequence_import};
use super::*;

const PRIME: u64 = 998_244_353;

fn complete_ordinary(generator: &ParametricIbpGenerator<'_>) -> CompletedIbpSourceRows {
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    prepared.complete(rows).unwrap()
}

fn ordering(
    completed: &CompletedIbpSourceRows,
    active: bool,
    exact: InvolutiveLimits,
) -> OreOrderingAdapter {
    OreOrderingAdapter::try_new_for_completed(
        OrderingPolicy::default(),
        Mask::try_new([active]).unwrap(),
        completed,
        exact,
    )
    .unwrap()
}

fn ordering_with_mask(
    completed: &CompletedIbpSourceRows,
    active: impl IntoIterator<Item = bool>,
    exact: InvolutiveLimits,
) -> OreOrderingAdapter {
    OreOrderingAdapter::try_new_for_completed(
        OrderingPolicy::default(),
        Mask::try_new(active).unwrap(),
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

fn shifted_source(
    source: &OreConsequence,
    shift: u64,
    multiplier: &crate::algebra::IndexedCoefficient,
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    exact: InvolutiveLimits,
) -> OreConsequence {
    OreConsequence::try_zero(ordering, context, exact)
        .unwrap()
        .try_left_axpy(
            multiplier,
            &ForwardShift::try_new([shift], exact).unwrap(),
            source,
            ordering,
            context,
            exact,
        )
        .unwrap()
}

#[test]
fn cold_lowering_rejects_an_aborted_all_preinterned_consequence_before_accounting() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ExactLazyLimits::default();
    let ordering = ordering(&completed, true, limits.exact);
    let context = generator.context();
    let lifted = lift(&completed, &ordering, context, limits.exact);
    let exact = lifted.sources()[0].consequence();
    let mut session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    try_import_exact_consequence(&mut session, exact, &ordering, context, limits).unwrap();

    let plan =
        try_plan_exact_consequence_import(&session, exact, &ordering, context, limits).unwrap();
    let census = plan.census();
    let mut transaction = session
        .try_begin_import_batch_transaction(&[census])
        .unwrap();
    let escaped =
        try_build_planned_exact_consequence(&mut transaction, &plan, &ordering, context, limits)
            .unwrap();
    transaction.try_abort().unwrap();

    let lowering_limits = ExactLazyLoweringLimits::for_session(&session);
    let mut budget = ExactLazyLoweringBudget::try_new(&session, lowering_limits).unwrap();
    assert_eq!(
        try_lower_for_exact_replay(&mut session, &escaped, &ordering, context, &mut budget)
            .unwrap_err(),
        ExactLazyError::InvalidProof {
            detail: "exact-lazy consequence did not cross its transaction commit boundary",
        }
    );
    assert_eq!(budget.census().attempts(), 0);
}

#[test]
fn cold_lowering_replays_the_minimally_lifted_one_loop_source_exactly() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ExactLazyLimits::default();
    let ordering = ordering(&completed, true, limits.exact);
    let context = generator.context();
    let lifted = lift(&completed, &ordering, context, limits.exact);
    let exact = lifted.sources()[0].consequence();

    let mut session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let lazy =
        try_import_exact_consequence(&mut session, exact, &ordering, context, limits).unwrap();
    let floor = session.committed_floor();
    let transactions = session.census();
    let lowering_limits = ExactLazyLoweringLimits::for_session(&session);
    let mut budget = ExactLazyLoweringBudget::try_new(&session, lowering_limits).unwrap();
    let lowered =
        try_lower_for_exact_replay(&mut session, &lazy, &ordering, context, &mut budget).unwrap();

    assert!(lowered.belongs_to(session.owner()));
    assert_eq!(lowered.consequence(), exact);
    assert_eq!(session.committed_floor(), floor);
    assert_eq!(
        session.census().committed_transactions(),
        transactions.committed_transactions()
    );
    assert_eq!(
        session.census().transaction_attempts(),
        transactions.transaction_attempts() + 1
    );
    assert_eq!(budget.census().attempts(), 1);
    assert_eq!(budget.census().successful_lowerings(), 1);
    assert!(budget.census().derivation_visits() > 0);
    assert!(budget.materialization_census().output_values() > 0);
}

#[test]
fn residual_chart_shift_replay_handles_active_and_inactive_ore_actions() {
    for active in [true, false] {
        let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
        let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
        let completed = complete_ordinary(&generator);
        let limits = ExactLazyLimits::default();
        let ordering = ordering(&completed, active, limits.exact);
        let context = generator.context();
        let lifted = lift(&completed, &ordering, context, limits.exact);
        let n = context.index(0).unwrap();
        let denominator = context.add(&n, &context.integer(3)).unwrap();
        let multiplier = context.div(&context.one(), &denominator).unwrap();
        let exact = shifted_source(
            lifted.sources()[0].consequence(),
            2,
            &multiplier,
            &ordering,
            context,
            limits.exact,
        );

        let mut session =
            ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
        let lazy =
            try_import_exact_consequence(&mut session, &exact, &ordering, context, limits).unwrap();
        let lowering_limits = ExactLazyLoweringLimits::for_session(&session);
        let mut budget = ExactLazyLoweringBudget::try_new(&session, lowering_limits).unwrap();
        let lowered =
            try_lower_for_exact_replay(&mut session, &lazy, &ordering, context, &mut budget)
                .unwrap()
                .into_consequence();

        assert_eq!(lowered, exact, "active={active}");
    }
}

#[test]
fn lowering_cap_rolls_back_every_temporary_root_but_keeps_work_charged() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ExactLazyLimits::default();
    let ordering = ordering(&completed, true, limits.exact);
    let context = generator.context();
    let lifted = lift(&completed, &ordering, context, limits.exact);
    let mut session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let lazy = try_import_exact_consequence(
        &mut session,
        lifted.sources()[0].consequence(),
        &ordering,
        context,
        limits,
    )
    .unwrap();
    let floor = session.committed_floor();
    let live = session.coefficient_live_census();
    let mut lowering_limits = ExactLazyLoweringLimits::for_session(&session);
    lowering_limits.max_derivation_visits = 0;
    let mut budget = ExactLazyLoweringBudget::try_new(&session, lowering_limits).unwrap();

    assert_eq!(
        try_lower_for_exact_replay(&mut session, &lazy, &ordering, context, &mut budget)
            .unwrap_err(),
        ExactLazyError::ResourceLimit {
            resource: "exact-lazy lowering derivation visits",
            requested: 1,
            limit: 0,
        }
    );
    assert_eq!(session.committed_floor(), floor);
    assert_eq!(session.coefficient_live_census(), live);
    assert_eq!(budget.census().attempts(), 1);
    assert_eq!(budget.census().derivation_visits(), 1);
    assert_eq!(budget.census().successful_lowerings(), 0);
}

#[test]
fn initial_derivation_frame_cap_is_charged_before_traversal_or_materialization() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ExactLazyLimits::default();
    let ordering = ordering(&completed, true, limits.exact);
    let context = generator.context();
    let lifted = lift(&completed, &ordering, context, limits.exact);
    let mut session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let lazy = try_import_exact_consequence(
        &mut session,
        lifted.sources()[0].consequence(),
        &ordering,
        context,
        limits,
    )
    .unwrap();
    let mut lowering_limits = ExactLazyLoweringLimits::for_session(&session);
    lowering_limits.max_derivation_frame_pushes = 0;
    let mut budget = ExactLazyLoweringBudget::try_new(&session, lowering_limits).unwrap();
    let floor = session.committed_floor();

    assert_eq!(
        try_lower_for_exact_replay(&mut session, &lazy, &ordering, context, &mut budget)
            .unwrap_err(),
        ExactLazyError::ResourceLimit {
            resource: "exact-lazy lowering derivation frame pushes",
            requested: 1,
            limit: 0,
        }
    );
    assert_eq!(session.committed_floor(), floor);
    assert_eq!(budget.census().derivation_frame_pushes(), 1);
    assert_eq!(budget.census().derivation_visits(), 0);
    assert_eq!(budget.materialization_census(), Default::default());
}

#[test]
fn exhausted_success_cap_does_not_count_or_start_a_lowering() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ExactLazyLimits::default();
    let ordering = ordering(&completed, true, limits.exact);
    let context = generator.context();
    let lifted = lift(&completed, &ordering, context, limits.exact);
    let mut session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let lazy = try_import_exact_consequence(
        &mut session,
        lifted.sources()[0].consequence(),
        &ordering,
        context,
        limits,
    )
    .unwrap();
    let mut lowering_limits = ExactLazyLoweringLimits::for_session(&session);
    lowering_limits.max_successful_lowerings = 0;
    let mut budget = ExactLazyLoweringBudget::try_new(&session, lowering_limits).unwrap();
    let before = session.census();

    assert_eq!(
        try_lower_for_exact_replay(&mut session, &lazy, &ordering, context, &mut budget)
            .unwrap_err(),
        ExactLazyError::ResourceLimit {
            resource: "exact-lazy successful cold lowerings",
            requested: 1,
            limit: 0,
        }
    );
    assert_eq!(session.census(), before);
    assert_eq!(budget.census().attempts(), 1);
    assert_eq!(budget.census().successful_lowerings(), 0);
    assert_eq!(budget.materialization_census(), Default::default());
}

#[test]
fn foreign_lowering_budget_is_rejected_before_opening_a_transaction() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ExactLazyLimits::default();
    let ordering = ordering(&completed, true, limits.exact);
    let context = generator.context();
    let lifted = lift(&completed, &ordering, context, limits.exact);
    let mut first = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let second = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let lazy = try_import_exact_consequence(
        &mut first,
        lifted.sources()[0].consequence(),
        &ordering,
        context,
        limits,
    )
    .unwrap();
    let mut budget =
        ExactLazyLoweringBudget::try_new(&second, ExactLazyLoweringLimits::for_session(&second))
            .unwrap();
    let before = first.census();

    assert_eq!(
        try_lower_for_exact_replay(&mut first, &lazy, &ordering, context, &mut budget).unwrap_err(),
        ExactLazyError::WrongLimitsContract
    );
    assert_eq!(first.census(), before);
    assert_eq!(budget.census(), ExactLazyLoweringCensus::default());
}

#[test]
fn lowering_policy_cannot_change_the_session_exact_contract() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ExactLazyLimits::default();
    let ordering = ordering(&completed, true, limits.exact);
    let context = generator.context();
    let session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let mut lowering = ExactLazyLoweringLimits::for_session(&session);
    lowering.chart_lift.involutive.max_row_terms += 1;
    assert_eq!(
        ExactLazyLoweringBudget::try_new(&session, lowering).unwrap_err(),
        ExactLazyError::WrongLimitsContract
    );
}

#[test]
fn materialization_root_cap_is_preflighted_before_derivation_expansion() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ExactLazyLimits::default();
    let ordering = ordering(&completed, true, limits.exact);
    let context = generator.context();
    let lifted = lift(&completed, &ordering, context, limits.exact);
    let mut session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let lazy = try_import_exact_consequence(
        &mut session,
        lifted.sources()[0].consequence(),
        &ordering,
        context,
        limits,
    )
    .unwrap();
    let envelope = lazy.census().physical_terms()
        + lazy.derivation().source_term_count()
        + lazy.guards().descriptor_count();
    assert!(envelope > 0);
    let mut lowering_limits = ExactLazyLoweringLimits::for_session(&session);
    lowering_limits.max_materialization_roots = envelope - 1;
    let mut budget = ExactLazyLoweringBudget::try_new(&session, lowering_limits).unwrap();
    let floor = session.committed_floor();

    assert_eq!(
        try_lower_for_exact_replay(&mut session, &lazy, &ordering, context, &mut budget)
            .unwrap_err(),
        ExactLazyError::ResourceLimit {
            resource: "exact-lazy lowering materialization roots",
            requested: envelope,
            limit: envelope - 1,
        }
    );
    assert_eq!(session.committed_floor(), floor);
    assert_eq!(budget.census().attempts(), 1);
    assert_eq!(budget.census().derivation_visits(), 0);
    assert_eq!(budget.census().materialization_roots(), 0);
    assert_eq!(budget.materialization_census(), Default::default());
}

#[test]
fn nested_materializer_batch_cap_is_preflighted_before_derivation_expansion() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ExactLazyLimits::default();
    let ordering = ordering(&completed, true, limits.exact);
    let context = generator.context();
    let lifted = lift(&completed, &ordering, context, limits.exact);
    let mut session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let lazy = try_import_exact_consequence(
        &mut session,
        lifted.sources()[0].consequence(),
        &ordering,
        context,
        limits,
    )
    .unwrap();
    let physical_roots = lazy.census().physical_terms();
    assert!(physical_roots > 0);
    let mut lowering_limits = ExactLazyLoweringLimits::for_session(&session);
    lowering_limits.materialization.max_batch_roots = physical_roots - 1;
    let mut budget = ExactLazyLoweringBudget::try_new(&session, lowering_limits).unwrap();
    let floor = session.committed_floor();

    assert_eq!(
        try_lower_for_exact_replay(&mut session, &lazy, &ordering, context, &mut budget)
            .unwrap_err(),
        ExactLazyError::ResourceLimit {
            resource: "exact-lazy lowering nested materializer batch roots",
            requested: physical_roots,
            limit: physical_roots - 1,
        }
    );
    assert_eq!(session.committed_floor(), floor);
    assert_eq!(budget.census().attempts(), 1);
    assert_eq!(budget.census().derivation_visits(), 0);
    assert_eq!(budget.census().materialization_roots(), 0);
    assert_eq!(budget.materialization_census(), Default::default());
}

#[test]
fn structurally_valid_but_false_source_provenance_is_rejected_by_replay() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ExactLazyLimits::default();
    let ordering = ordering(&completed, true, limits.exact);
    let context = generator.context();
    let lifted = lift(&completed, &ordering, context, limits.exact);
    let false_row = OreRow::try_new(
        &ordering,
        [(
            ForwardShift::try_zero(1, limits.exact).unwrap(),
            context.integer(17),
        )],
        context,
        limits.exact,
    )
    .unwrap();
    let false_consequence = OreConsequence::try_from_left_shifted_source(
        0,
        lifted.sources()[0].left_shift().clone(),
        false_row,
        &ordering,
        context,
        limits.exact,
    )
    .unwrap();
    let mut session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let lazy =
        try_import_exact_consequence(&mut session, &false_consequence, &ordering, context, limits)
            .unwrap();
    let floor = session.committed_floor();
    let lowering_limits = ExactLazyLoweringLimits::for_session(&session);
    let mut budget = ExactLazyLoweringBudget::try_new(&session, lowering_limits).unwrap();

    assert_eq!(
        try_lower_for_exact_replay(&mut session, &lazy, &ordering, context, &mut budget)
            .unwrap_err(),
        ExactLazyError::InvalidProof {
            detail: "materialized exact-lazy row disagrees with complete sealed-source replay",
        }
    );
    assert_eq!(session.committed_floor(), floor);
    assert_eq!(budget.census().successful_lowerings(), 0);
}

#[test]
fn every_generated_k3_ordinary_source_round_trips_through_cold_replay() {
    let artifact = derive_two_loop_unit_mass_sunset().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    assert_eq!(completed.source_row_count(), 4);
    let limits = ExactLazyLimits::default();
    let ordering = ordering_with_mask(&completed, [true, true, true], limits.exact);
    let context = generator.context();
    let lifted = lift(&completed, &ordering, context, limits.exact);
    assert_eq!(lifted.len(), 4);
    let mut session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let lowering_limits = ExactLazyLoweringLimits::for_session(&session);
    let mut budget = ExactLazyLoweringBudget::try_new(&session, lowering_limits).unwrap();

    for source in lifted.sources() {
        let lazy = try_import_exact_consequence(
            &mut session,
            source.consequence(),
            &ordering,
            context,
            limits,
        )
        .unwrap();
        let lowered =
            try_lower_for_exact_replay(&mut session, &lazy, &ordering, context, &mut budget)
                .unwrap();
        assert_eq!(lowered.consequence(), source.consequence());
    }
    assert_eq!(budget.census().successful_lowerings(), 4);
}

#[test]
fn translated_k3_source_accepts_factorized_lazy_domain_against_reducible_replay_guard() {
    let artifact = derive_two_loop_unit_mass_sunset().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    assert_eq!(completed.source_row_count(), 4);
    let limits = ExactLazyLimits::default();
    let ordering = ordering_with_mask(&completed, [true, true, true], limits.exact);
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
    let source = source_rows
        .sources()
        .iter()
        .find(|source| source.source_ordinal() == 1)
        .unwrap();
    let outer_shift = ForwardShift::try_new([1, 1, 0], limits.exact).unwrap();
    let exact_subject = OreConsequence::try_zero(&ordering, context, limits.exact)
        .unwrap()
        .try_left_axpy(
            &context.integer(3),
            &outer_shift,
            source.consequence(),
            &ordering,
            context,
            limits.exact,
        )
        .unwrap();

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
    let point = vec![7; context.base().parameter_names().len() + context.index_count()];
    let schedule = ExactLazyProbeSchedule::try_new(
        session.owner(),
        session.coefficient_dag(),
        context,
        [ExactLazyProbeSpec::new(0, PRIME, point)],
    )
    .unwrap();
    let mut ledger =
        ExactLazyCompletionLedger::try_new(&session, &ordering, context, limits).unwrap();
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
    let exact = try_janet_normal_form(
        exact_subject,
        initial.epoch(),
        &ordering,
        context,
        limits.exact,
    )
    .unwrap();
    // The cancellation DAG conservatively retains 24 historic guard
    // occurrences. DAG collection removes shared (node, translation) paths,
    // leaving 20 distinct typed leaves; exact materialization canonicalizes
    // those to the three factor guards n0+1, n1+1, n1+2. The independent cold
    // sealed-source replay additionally reconstructs their reducible product,
    // which the principal-open proof must accept without retaining it.
    assert_eq!(lazy.remainder().guards().descriptor_count(), 24);
    let lazy_guard_leaf_count = {
        let mut transaction = session.try_begin_transaction().unwrap();
        let count = transaction
            .try_collect_guard_probe_requirements(lazy.remainder().guards().root(), &ordering)
            .unwrap()
            .len();
        transaction.try_abort().unwrap();
        count
    };
    assert_eq!(lazy_guard_leaf_count, 20);
    assert_eq!(exact.remainder().required_nonzero_guards().len(), 3);

    let n0_plus_one = context
        .add(&context.index(0).unwrap(), &context.one())
        .unwrap();
    let n1_plus_one = context
        .add(&context.index(1).unwrap(), &context.one())
        .unwrap();
    let n1_plus_two = context
        .add(&context.index(1).unwrap(), &context.integer(2))
        .unwrap();
    for expected in [&n0_plus_one, &n1_plus_one, &n1_plus_two] {
        let expected = context
            .numerator_condition_with_limits(expected, limits.exact.indexed_algebra.exact_algebra)
            .unwrap();
        assert!(
            exact
                .remainder()
                .required_nonzero_guards()
                .iter()
                .any(|guard| guard.as_ref() == &expected)
        );
    }

    let floor = session.committed_floor();
    let lowering_limits = ExactLazyLoweringLimits::for_session(&session);
    let mut budget = ExactLazyLoweringBudget::try_new(&session, lowering_limits).unwrap();
    let lowered = try_lower_for_exact_replay(
        &mut session,
        lazy.remainder(),
        &ordering,
        context,
        &mut budget,
    )
    .unwrap()
    .into_consequence();

    assert_eq!(lowered.row(), exact.remainder().row());
    assert_eq!(lowered.provenance(), exact.remainder().provenance());
    assert_eq!(lowered.required_nonzero_guards().len(), 3);
    assert_eq!(
        lowered.required_nonzero_guards(),
        exact.remainder().required_nonzero_guards(),
        "cold replay must retain the authenticated canonical lazy witness"
    );
    assert_eq!(budget.localization_domain_census().attempts(), 1);
    assert_eq!(budget.localization_domain_census().output_signatures(), 2);
    assert_eq!(session.committed_floor(), floor);
}

#[test]
fn nested_k3_left_axpy_cancellation_replays_row_provenance_and_denominator_guards() {
    let artifact = derive_two_loop_unit_mass_sunset().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ExactLazyLimits::default();
    let ordering = ordering_with_mask(&completed, [true, true, true], limits.exact);
    let context = generator.context();
    let lifted = lift(&completed, &ordering, context, limits.exact);
    let basis_source = lifted.sources()[0].consequence();
    let epoch = JanetBasisEpoch::try_initial(
        [OreConsequence::try_zero(&ordering, context, limits.exact)
            .unwrap()
            .try_left_axpy(
                &context.one(),
                &ForwardShift::try_zero(3, limits.exact).unwrap(),
                basis_source,
                &ordering,
                context,
                limits.exact,
            )
            .unwrap()],
        &ordering,
        context,
        limits.exact,
        CompletionGeometryLimits::default(),
    )
    .unwrap();
    let n0 = context.index(0).unwrap();
    let numerator = context.add(&n0, &context.integer(137)).unwrap();
    let denominator = context.add(&n0, &context.integer(139)).unwrap();
    let rational = context.div(&numerator, &denominator).unwrap();
    let operator_shift = ForwardShift::try_new([1, 0, 0], limits.exact).unwrap();
    let exact_subject = OreConsequence::try_zero(&ordering, context, limits.exact)
        .unwrap()
        .try_left_axpy(
            &rational,
            &operator_shift,
            epoch.elements()[0].consequence(),
            &ordering,
            context,
            limits.exact,
        )
        .unwrap();
    let exact_normal_form = try_janet_normal_form(
        OreConsequence::try_zero(&ordering, context, limits.exact)
            .unwrap()
            .try_left_axpy(
                &context.one(),
                &ForwardShift::try_zero(3, limits.exact).unwrap(),
                &exact_subject,
                &ordering,
                context,
                limits.exact,
            )
            .unwrap(),
        &epoch,
        &ordering,
        context,
        limits.exact,
    )
    .unwrap();
    assert!(exact_normal_form.is_zero());
    assert_eq!(exact_normal_form.steps().len(), 1);

    let mut session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let frozen = ExactLazyFrozenJanetEpoch::try_import(
        &mut session,
        epoch.division(),
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
    let point = vec![7; context.base().parameter_names().len() + context.index_count()];
    let schedule = ExactLazyProbeSchedule::try_new(
        session.owner(),
        session.coefficient_dag(),
        context,
        [ExactLazyProbeSpec::new(0, PRIME, point)],
    )
    .unwrap();
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
    assert!(cursor.subject().row().physical_term_count() == 0);

    let lowering_limits = ExactLazyLoweringLimits::for_session(&session);
    let mut budget = ExactLazyLoweringBudget::try_new(&session, lowering_limits).unwrap();
    let lowered = try_lower_for_exact_replay(
        &mut session,
        cursor.subject(),
        &ordering,
        context,
        &mut budget,
    )
    .unwrap()
    .into_consequence();
    assert!(lowered.is_zero());
    let expected_denominator_guard = context
        .numerator_condition_with_limits(&denominator, limits.exact.indexed_algebra.exact_algebra)
        .unwrap();
    let forbidden_numerator_guard = context
        .numerator_condition_with_limits(&numerator, limits.exact.indexed_algebra.exact_algebra)
        .unwrap();
    assert!(
        lowered
            .required_nonzero_guards()
            .iter()
            .any(|guard| guard.as_ref() == &expected_denominator_guard)
    );
    assert!(
        !lowered
            .required_nonzero_guards()
            .iter()
            .any(|guard| guard.as_ref() == &forbidden_numerator_guard),
        "a Defined rational guard must lower to its denominator, not its numerator"
    );
    // The exact eager and exact-lazy paths must retain the same complete
    // source-module identity even though every final provenance term cancels.
    assert_eq!(
        lowered.provenance(),
        exact_normal_form.remainder().provenance()
    );
}
