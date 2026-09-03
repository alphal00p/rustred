//! Atomic frozen-epoch import tests.

use crate::algebra::{CoefficientContext, IndexedCoefficientContext};
use crate::foundry::artifact::derive_one_loop_unit_mass_tadpole;
use crate::foundry::completion::CompletionGeometryLimits;
use crate::identity::{CompletedIbpSourceRows, ParametricIbpGenerator};
use crate::sector::{Mask, OrderingPolicy};

use super::super::super::{
    ForwardShift, JanetBasisEpoch, OreConsequence, OreOrderingAdapter, OreRow,
};
use super::super::ModularGuideError;
use super::*;

fn complete_ordinary(generator: &ParametricIbpGenerator<'_>) -> CompletedIbpSourceRows {
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    prepared.complete(rows).unwrap()
}

fn completed_ordering(
    completed: &CompletedIbpSourceRows,
    limits: ExactLazyLimits,
) -> OreOrderingAdapter {
    OreOrderingAdapter::try_new_for_completed(
        OrderingPolicy::default(),
        Mask::try_new([true]).unwrap(),
        completed,
        limits.exact,
    )
    .unwrap()
}

fn consequence(
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    terms: impl IntoIterator<Item = (u64, crate::algebra::IndexedCoefficient)>,
    limits: ExactLazyLimits,
) -> OreConsequence {
    let row = OreRow::try_new(
        ordering,
        terms.into_iter().map(|(degree, coefficient)| {
            (
                ForwardShift::try_new([degree], limits.exact).unwrap(),
                coefficient,
            )
        }),
        context,
        limits.exact,
    )
    .unwrap();
    OreConsequence::try_from_source(0, row, ordering, context, limits.exact).unwrap()
}

fn two_row_epoch(
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    limits: ExactLazyLimits,
) -> JanetBasisEpoch {
    let n = context.index(0).unwrap();
    let n_plus_one = context.add(&n, &context.one()).unwrap();
    let first = consequence(ordering, context, [(0, n), (1, context.one())], limits);
    let second = consequence(
        ordering,
        context,
        [(0, n_plus_one), (2, context.one())],
        limits,
    );
    JanetBasisEpoch::try_initial(
        [first, second],
        ordering,
        context,
        limits.exact,
        CompletionGeometryLimits::default(),
    )
    .unwrap()
}

#[test]
fn frozen_import_binds_exact_epoch_ordinals_leaders_and_owner() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ExactLazyLimits::default();
    let ordering = completed_ordering(&completed, limits);
    let context = generator.context();
    let epoch = two_row_epoch(&ordering, context, limits);
    let independent_epoch = two_row_epoch(&ordering, context, limits);
    let division = epoch.division();
    assert!(
        !division
            .epoch()
            .same_instance(independent_epoch.division().epoch())
    );
    let mut session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();

    let frozen =
        ExactLazyFrozenJanetEpoch::try_import(&mut session, division, &ordering, context, limits)
            .unwrap();

    assert!(std::ptr::eq(frozen.division(), division));
    assert_eq!(frozen.epoch(), division.epoch());
    assert!(frozen.owner().belongs_to(session.owner()));
    assert_eq!(frozen.len(), division.elements().len());
    assert!(!frozen.is_empty());
    for (ordinal, exact) in division.elements().iter().enumerate() {
        assert_eq!(exact.ordinal(), ordinal);
        let lazy = frozen.divisor(ordinal).unwrap();
        assert!(lazy.owner().belongs_to(frozen.owner()));
        let leader = lazy
            .row()
            .try_leading_term(&session, &ordering)
            .unwrap()
            .unwrap();
        assert_eq!(leader.shift(), exact.leading_shift());
        assert_eq!(lazy.census().provenance_terms(), 1);
    }
    assert_eq!(
        frozen.divisor(frozen.len()).unwrap_err(),
        ExactLazyError::FrozenDivisorOutOfRange {
            ordinal: frozen.len(),
            divisor_count: frozen.len(),
        }
    );
    assert_eq!(session.census().transaction_attempts(), 1);
    assert_eq!(session.census().committed_transactions(), 1);
}

#[test]
fn second_divisor_failure_rolls_back_every_arena_without_a_retained_prefix() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let fixture_limits = ExactLazyLimits::default();
    let ordering = completed_ordering(&completed, fixture_limits);
    let context = generator.context();
    let epoch = two_row_epoch(&ordering, context, fixture_limits);
    let mut limits = fixture_limits;
    limits.coefficient.max_exact_leaves = 1;
    let mut session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let floor = session.committed_floor();
    let live = session.coefficient_live_census();

    let error = ExactLazyFrozenJanetEpoch::try_import(
        &mut session,
        epoch.division(),
        &ordering,
        context,
        limits,
    )
    .unwrap_err();

    assert_eq!(
        error,
        ExactLazyError::Modular(ModularGuideError::ResourceLimit {
            resource: "modular coefficient exact leaves",
            requested: 2,
            limit: 1,
        })
    );
    assert_eq!(session.committed_floor(), floor);
    assert_eq!(session.coefficient_live_census(), live);
    assert_eq!(session.census().transaction_attempts(), 1);
    assert_eq!(session.census().committed_transactions(), 0);
    assert_eq!(session.census().imported_physical_terms(), 4);
    assert_eq!(session.census().imported_provenance_terms(), 2);
}

#[test]
fn foreign_action_is_rejected_before_accounting_or_mutation() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let foreign_completed = complete_ordinary(&generator);
    let limits = ExactLazyLimits::default();
    let ordering = completed_ordering(&completed, limits);
    let foreign_ordering = completed_ordering(&foreign_completed, limits);
    let context = generator.context();
    let foreign_epoch = two_row_epoch(&foreign_ordering, context, limits);
    let mut session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let floor = session.committed_floor();

    assert_eq!(
        ExactLazyFrozenJanetEpoch::try_import(
            &mut session,
            foreign_epoch.division(),
            &ordering,
            context,
            limits,
        )
        .unwrap_err(),
        ExactLazyError::WrongOreAction
    );
    assert_eq!(session.committed_floor(), floor);
    assert_eq!(session.census(), ExactLazyCensus::default());

    let foreign_base = CoefficientContext::new(std::iter::empty::<&str>());
    let foreign_context =
        IndexedCoefficientContext::try_new(&foreign_base, "frozen-foreign-context", 1).unwrap();
    assert_eq!(
        ExactLazyFrozenJanetEpoch::try_import(
            &mut session,
            foreign_epoch.division(),
            &ordering,
            &foreign_context,
            limits,
        )
        .unwrap_err(),
        ExactLazyError::WrongIndexedContext
    );
    let mut foreign_limits = limits;
    foreign_limits.max_frozen_epoch_divisors -= 1;
    assert_eq!(
        ExactLazyFrozenJanetEpoch::try_import(
            &mut session,
            foreign_epoch.division(),
            &ordering,
            context,
            foreign_limits,
        )
        .unwrap_err(),
        ExactLazyError::WrongLimitsContract
    );
    assert_eq!(session.committed_floor(), floor);
    assert_eq!(session.census(), ExactLazyCensus::default());
}

#[test]
fn frozen_epoch_rejects_foreign_session_owner() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ExactLazyLimits::default();
    let ordering = completed_ordering(&completed, limits);
    let context = generator.context();
    let epoch = two_row_epoch(&ordering, context, limits);
    let mut session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let foreign = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let frozen = ExactLazyFrozenJanetEpoch::try_import(
        &mut session,
        epoch.division(),
        &ordering,
        context,
        limits,
    )
    .unwrap();

    assert_eq!(
        frozen.require_owner(foreign.owner()),
        Err(ExactLazyError::WrongSessionOwner)
    );
}

#[test]
fn one_below_epoch_and_batch_caps_fail_before_opening_a_transaction() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let fixture_limits = ExactLazyLimits::default();
    let ordering = completed_ordering(&completed, fixture_limits);
    let context = generator.context();
    let epoch = two_row_epoch(&ordering, context, fixture_limits);

    let mut count_limits = fixture_limits;
    count_limits.max_frozen_epoch_divisors = 1;
    let mut count_session =
        ExactLazySession::try_new(&ordering, context, &completed, count_limits).unwrap();
    assert_eq!(
        ExactLazyFrozenJanetEpoch::try_import(
            &mut count_session,
            epoch.division(),
            &ordering,
            context,
            count_limits,
        )
        .unwrap_err(),
        ExactLazyError::ResourceLimit {
            resource: "exact-lazy frozen Janet divisors",
            requested: 2,
            limit: 1,
        }
    );
    assert_eq!(count_session.census(), ExactLazyCensus::default());

    let mut batch_limits = fixture_limits;
    batch_limits.max_total_imported_physical_terms = 3;
    let mut batch_session =
        ExactLazySession::try_new(&ordering, context, &completed, batch_limits).unwrap();
    assert_eq!(
        ExactLazyFrozenJanetEpoch::try_import(
            &mut batch_session,
            epoch.division(),
            &ordering,
            context,
            batch_limits,
        )
        .unwrap_err(),
        ExactLazyError::ResourceLimit {
            resource: "exact-lazy imported physical terms",
            requested: 4,
            limit: 3,
        }
    );
    assert_eq!(batch_session.census(), ExactLazyCensus::default());
}

#[test]
fn janet_constructor_normalizes_nonmonic_input_before_frozen_boundary() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ExactLazyLimits::default();
    let ordering = completed_ordering(&completed, limits);
    let context = generator.context();
    let input = consequence(
        &ordering,
        context,
        [(0, context.index(0).unwrap()), (1, context.integer(2))],
        limits,
    );
    let epoch = JanetBasisEpoch::try_initial(
        [input],
        &ordering,
        context,
        limits.exact,
        CompletionGeometryLimits::default(),
    )
    .unwrap();
    let exact_leader = epoch.elements()[0]
        .consequence()
        .row()
        .try_leading_term(&ordering)
        .unwrap()
        .unwrap()
        .0;
    assert_eq!(exact_leader.coefficient(), &context.one());

    let mut session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    ExactLazyFrozenJanetEpoch::try_import(
        &mut session,
        epoch.division(),
        &ordering,
        context,
        limits,
    )
    .unwrap();
}
