//! Persistent exact-lazy Janet epoch tests.

use std::sync::Arc;

use crate::algebra::IndexedCoefficientContext;
use crate::foundry::artifact::{
    ClosedArtifact, derive_one_loop_unit_mass_tadpole, derive_two_loop_unit_mass_sunset,
};
use crate::foundry::completion::CompletionGeometryLimits;
use crate::identity::{CompletedIbpSourceRows, ParametricIbpGenerator};
use crate::sector::{Mask, OrderingPolicy};

use super::super::super::limits::InvolutiveWorkBudget;
use super::super::super::{
    ForwardShift, InvolutiveError, InvolutiveLimits, JanetBasisEpoch, OrdinaryChartLiftLimits,
    OreConsequence, OreOrderingAdapter, OreRow, try_lift_completed_ordinary_sources,
    try_preprocess_initial_basis,
};
use super::epoch::ExactLazyReplacementForTest;
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

fn exact_initial(
    completed: &CompletedIbpSourceRows,
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    exact: InvolutiveLimits,
) -> JanetBasisEpoch {
    let rows = lift(completed, ordering, context, exact)
        .try_into_consequences(completed, ordering, context, exact)
        .unwrap();
    try_preprocess_initial_basis(
        rows.into_vec(),
        ordering,
        context,
        exact,
        CompletionGeometryLimits::default(),
    )
    .unwrap()
    .into_parts()
    .0
}

fn consequence(
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    terms: impl IntoIterator<Item = (u64, crate::algebra::IndexedCoefficient)>,
    exact: InvolutiveLimits,
) -> OreConsequence {
    OreConsequence::try_from_source(
        0,
        OreRow::try_new(
            ordering,
            terms.into_iter().map(|(degree, coefficient)| {
                (ForwardShift::try_new([degree], exact).unwrap(), coefficient)
            }),
            context,
            exact,
        )
        .unwrap(),
        ordering,
        context,
        exact,
    )
    .unwrap()
}

fn committed_zero(
    session: &mut ExactLazySession<'_>,
    context: &IndexedCoefficientContext,
) -> Arc<ExactLazyConsequence> {
    let point = vec![7; context.base().parameter_names().len() + context.index_count()];
    let schedule = ExactLazyProbeSchedule::try_new(
        session.owner(),
        session.coefficient_dag(),
        context,
        [ExactLazyProbeSpec::new(0, PRIME, point)],
    )
    .unwrap();
    let mut support = ExactLazySupportBudget::new(session.owner());
    let transaction = session.try_begin_transaction().unwrap();
    let pending = UnclassifiedLazyOreRow::try_new(
        &transaction,
        Vec::<PendingLazyOreTerm>::new(),
        Vec::<StructuralZeroProof>::new(),
    )
    .unwrap();
    let row =
        try_classify_support(&transaction, context, &[], pending, &schedule, &mut support).unwrap();
    let zero_derivation = transaction.zero_derivation();
    let derivation =
        ImportedSourceDerivation::try_from_lineage(&transaction, zero_derivation).unwrap();
    let empty_guards = transaction.empty_guards();
    let guards = ImportedGuardLineage::try_from_lineage(&transaction, empty_guards).unwrap();
    let zero = ExactLazyConsequence::try_new(
        &transaction,
        row,
        derivation,
        guards,
        ExactLazyPayloadCensus::default(),
    )
    .unwrap();
    transaction.try_commit().unwrap();
    Arc::new(zero)
}

fn two_row_exact_epoch(
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    exact: InvolutiveLimits,
) -> JanetBasisEpoch {
    let n = context.index(0).unwrap();
    JanetBasisEpoch::try_initial(
        [
            consequence(
                ordering,
                context,
                [(0, n.clone()), (1, context.one())],
                exact,
            ),
            consequence(
                ordering,
                context,
                [
                    (0, context.add(&n, &context.one()).unwrap()),
                    (2, context.one()),
                ],
                exact,
            ),
        ],
        ordering,
        context,
        exact,
        CompletionGeometryLimits::default(),
    )
    .unwrap()
}

fn assert_same_geometry(artifact: ClosedArtifact, mask: Vec<bool>) {
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ExactLazyLimits::default();
    let ordering = ordering(&completed, mask, limits.exact);
    let context = generator.context();
    let exact = exact_initial(&completed, &ordering, context, limits.exact);
    let mut session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let frozen = ExactLazyFrozenJanetEpoch::try_import(
        &mut session,
        exact.division(),
        &ordering,
        context,
        limits,
    )
    .unwrap();
    let mut ledger =
        ExactLazyCompletionLedger::try_new(&session, &ordering, context, limits).unwrap();
    let lazy = ExactLazyJanetEpoch::try_initial_from_frozen(
        &session,
        frozen,
        &ordering,
        context,
        limits,
        CompletionGeometryLimits::default(),
        &mut ledger,
    )
    .unwrap();

    assert_eq!(lazy.elements().len(), exact.elements().len());
    for (lazy, exact) in lazy.elements().iter().zip(exact.elements()) {
        assert_eq!(lazy.ordinal(), exact.ordinal());
        assert_eq!(lazy.leading_shift(), exact.leading_shift());
        assert_eq!(lazy.leading_key(), exact.leading_key());
        assert_eq!(lazy.multiplicative(), exact.multiplicative());
        assert!(lazy.consequence().owner().belongs_to(session.owner()));
    }
    assert_eq!(lazy.leading_ideal(), exact.leading_ideal());
    assert_eq!(lazy.uncovered_partition(), exact.uncovered_partition());
    assert_eq!(lazy.pure_power_coverage(), exact.pure_power_coverage());
    assert_eq!(lazy.prolongations().len(), exact.prolongations().len());
    for (lazy, exact) in lazy.prolongations().iter().zip(exact.prolongations()) {
        assert_eq!(lazy.basis_ordinal(), exact.basis_ordinal());
        assert_eq!(lazy.variable(), exact.variable());
        assert_eq!(lazy.target_leading_shift(), exact.target_leading_shift());
        assert_eq!(lazy.target_key(), exact.target_key());
    }

    let mut exact_scratch = exact.try_divisor_scratch(limits.exact).unwrap();
    let mut lazy_scratch = lazy
        .division()
        .try_divisor_scratch(&ordering, limits)
        .unwrap();
    let mut exact_queries = InvolutiveWorkBudget::default();
    let mut lazy_queries = InvolutiveWorkBudget::default();
    for element in exact.elements() {
        let target = element.leading_shift();
        let expected = exact
            .try_janet_divisor_with_scratch(
                target,
                None,
                &mut exact_scratch,
                limits.exact,
                &mut exact_queries,
            )
            .unwrap();
        let actual = lazy
            .division()
            .try_janet_divisor_with_scratch(
                target,
                None,
                &mut lazy_scratch,
                &ordering,
                limits,
                &mut lazy_queries,
            )
            .unwrap();
        assert_eq!(actual, expected);
    }
    assert_eq!(lazy_queries.census(), exact_queries.census());
}

#[test]
fn shared_geometry_matches_exact_epochs_for_one_loop_and_all_four_k3_sources() {
    assert_same_geometry(derive_one_loop_unit_mass_tadpole().unwrap(), vec![true]);
    assert_same_geometry(
        derive_two_loop_unit_mass_sunset().unwrap(),
        vec![true, true, true],
    );
    assert_same_geometry(
        derive_two_loop_unit_mass_sunset().unwrap(),
        vec![true, false, true],
    );
}

#[test]
fn persistent_addition_rebuilds_geometry_and_shares_every_predecessor_arc() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ExactLazyLimits::default();
    let ordering = ordering(&completed, [true], limits.exact);
    let context = generator.context();
    let exact = two_row_exact_epoch(&ordering, context, limits.exact);
    let mut session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let frozen = ExactLazyFrozenJanetEpoch::try_import(
        &mut session,
        exact.division(),
        &ordering,
        context,
        limits,
    )
    .unwrap();
    let mut ledger =
        ExactLazyCompletionLedger::try_new(&session, &ordering, context, limits).unwrap();
    let initial = ExactLazyJanetEpoch::try_initial_from_frozen(
        &session,
        frozen,
        &ordering,
        context,
        limits,
        CompletionGeometryLimits::default(),
        &mut ledger,
    )
    .unwrap();
    let predecessor_handles: Vec<_> = initial
        .elements()
        .iter()
        .map(|element| Arc::clone(element.consequence_handle()))
        .collect();
    let added = Arc::new(
        try_import_exact_consequence(
            &mut session,
            &consequence(
                &ordering,
                context,
                [(0, context.integer(7)), (3, context.one())],
                limits.exact,
            ),
            &ordering,
            context,
            limits,
        )
        .unwrap(),
    );
    let successor = initial
        .try_addition_successor_for_test(
            &session,
            vec![Arc::clone(&added)],
            &ordering,
            context,
            limits,
            CompletionGeometryLimits::default(),
            &mut ledger,
        )
        .unwrap();

    assert!(successor.epoch().same_instance(initial.epoch()));
    assert_eq!(successor.epoch().revision(), initial.epoch().revision() + 1);
    assert_eq!(successor.predecessor(), Some(initial.epoch()));
    for predecessor in predecessor_handles {
        assert!(
            successor
                .elements()
                .iter()
                .any(|element| { Arc::ptr_eq(element.consequence_handle(), &predecessor) })
        );
    }
    assert!(
        successor
            .elements()
            .iter()
            .any(|element| { Arc::ptr_eq(element.consequence_handle(), &added) })
    );
    assert_eq!(initial.elements().len(), 2);
    assert_eq!(successor.elements().len(), 3);
}

#[test]
fn persistent_replacement_shares_retained_rows_and_admits_only_the_new_payload() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ExactLazyLimits::default();
    let ordering = ordering(&completed, [true], limits.exact);
    let context = generator.context();
    let exact = two_row_exact_epoch(&ordering, context, limits.exact);
    let mut session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let frozen = ExactLazyFrozenJanetEpoch::try_import(
        &mut session,
        exact.division(),
        &ordering,
        context,
        limits,
    )
    .unwrap();
    let mut ledger =
        ExactLazyCompletionLedger::try_new(&session, &ordering, context, limits).unwrap();
    let initial = ExactLazyJanetEpoch::try_initial_from_frozen(
        &session,
        frozen,
        &ordering,
        context,
        limits,
        CompletionGeometryLimits::default(),
        &mut ledger,
    )
    .unwrap();
    let retained = Arc::clone(initial.elements()[0].consequence_handle());
    let replacement = Arc::new(
        try_import_exact_consequence(
            &mut session,
            &consequence(
                &ordering,
                context,
                [(0, context.integer(11)), (3, context.one())],
                limits.exact,
            ),
            &ordering,
            context,
            limits,
        )
        .unwrap(),
    );

    let successor = initial
        .try_replacement_successor_for_test(
            &session,
            vec![
                ExactLazyReplacementForTest::Shared(0),
                ExactLazyReplacementForTest::New(Arc::clone(&replacement)),
            ],
            &ordering,
            context,
            limits,
            CompletionGeometryLimits::default(),
            &mut ledger,
        )
        .unwrap();

    assert_eq!(successor.predecessor(), Some(initial.epoch()));
    assert!(Arc::ptr_eq(
        successor.elements()[0].consequence_handle(),
        &retained,
    ));
    assert!(
        successor
            .elements()
            .iter()
            .any(|element| { Arc::ptr_eq(element.consequence_handle(), &replacement) })
    );
    assert_eq!(initial.elements().len(), 2);
    assert_eq!(successor.elements().len(), 2);
}

#[test]
fn sibling_successors_reject_each_others_scratch_and_prolongations() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ExactLazyLimits::default();
    let ordering = ordering(&completed, [true], limits.exact);
    let context = generator.context();
    let exact = two_row_exact_epoch(&ordering, context, limits.exact);
    let mut session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let frozen = ExactLazyFrozenJanetEpoch::try_import(
        &mut session,
        exact.division(),
        &ordering,
        context,
        limits,
    )
    .unwrap();
    let mut ledger =
        ExactLazyCompletionLedger::try_new(&session, &ordering, context, limits).unwrap();
    let initial = ExactLazyJanetEpoch::try_initial_from_frozen(
        &session,
        frozen,
        &ordering,
        context,
        limits,
        CompletionGeometryLimits::default(),
        &mut ledger,
    )
    .unwrap();
    let shared = || {
        vec![
            ExactLazyReplacementForTest::Shared(0),
            ExactLazyReplacementForTest::Shared(1),
        ]
    };
    let left = initial
        .try_replacement_division_successor_for_test(
            &session,
            shared(),
            &ordering,
            context,
            limits,
            &mut ledger,
        )
        .unwrap();
    let right = initial
        .try_replacement_division_successor_for_test(
            &session,
            shared(),
            &ordering,
            context,
            limits,
            &mut ledger,
        )
        .unwrap();
    assert!(left.epoch().same_instance(right.epoch()));
    assert_eq!(left.epoch().revision(), right.epoch().revision());
    assert_ne!(left.epoch(), right.epoch());
    for ordinal in 0..initial.elements().len() {
        assert!(Arc::ptr_eq(
            initial.elements()[ordinal].consequence_handle(),
            left.elements()[ordinal].consequence_handle(),
        ));
        assert!(Arc::ptr_eq(
            initial.elements()[ordinal].consequence_handle(),
            right.elements()[ordinal].consequence_handle(),
        ));
    }

    let mut stale_scratch = left.try_divisor_scratch(&ordering, limits).unwrap();
    let mut query_work = InvolutiveWorkBudget::default();
    assert!(matches!(
        right.try_janet_divisor_with_scratch(
            right.elements()[0].leading_shift(),
            None,
            &mut stale_scratch,
            &ordering,
            limits,
            &mut query_work,
        ),
        Err(ExactLazyError::Involutive(
            InvolutiveError::StaleEpoch { .. }
        ))
    ));

    let left = left
        .try_seal(
            &session,
            &ordering,
            context,
            limits,
            CompletionGeometryLimits::default(),
            &ledger,
        )
        .unwrap();
    let right = right
        .try_seal(
            &session,
            &ordering,
            context,
            limits,
            CompletionGeometryLimits::default(),
            &ledger,
        )
        .unwrap();
    assert!(!left.prolongations().is_empty());
    assert!(matches!(
        right.require_current(left.prolongations().first().unwrap(), &ordering, limits),
        Err(ExactLazyError::Involutive(
            InvolutiveError::StaleEpoch { .. }
        ))
    ));
}

#[test]
fn division_only_revisions_keep_the_last_complete_predecessor_until_sealed() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ExactLazyLimits::default();
    let ordering = ordering(&completed, [true], limits.exact);
    let context = generator.context();
    let exact = two_row_exact_epoch(&ordering, context, limits.exact);
    let mut session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let frozen = ExactLazyFrozenJanetEpoch::try_import(
        &mut session,
        exact.division(),
        &ordering,
        context,
        limits,
    )
    .unwrap();
    let mut ledger =
        ExactLazyCompletionLedger::try_new(&session, &ordering, context, limits).unwrap();
    let initial = ExactLazyJanetEpoch::try_initial_from_frozen(
        &session,
        frozen,
        &ordering,
        context,
        limits,
        CompletionGeometryLimits::default(),
        &mut ledger,
    )
    .unwrap();
    let first = initial
        .try_replacement_division_successor_for_test(
            &session,
            vec![
                ExactLazyReplacementForTest::Shared(0),
                ExactLazyReplacementForTest::Shared(1),
            ],
            &ordering,
            context,
            limits,
            &mut ledger,
        )
        .unwrap();
    let second = first
        .try_replacement_successor_for_test(
            &session,
            vec![
                ExactLazyReplacementForTest::Shared(0),
                ExactLazyReplacementForTest::Shared(1),
            ],
            &ordering,
            context,
            limits,
            &mut ledger,
        )
        .unwrap();
    assert_eq!(second.epoch().revision(), initial.epoch().revision() + 2);
    assert_eq!(second.predecessor(), Some(initial.epoch()));
    let sealed = second
        .try_seal(
            &session,
            &ordering,
            context,
            limits,
            CompletionGeometryLimits::default(),
            &ledger,
        )
        .unwrap();
    assert_eq!(sealed.predecessor(), Some(initial.epoch()));
}

#[test]
fn successors_and_sealing_reject_an_equal_environment_sibling_ledger() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ExactLazyLimits::default();
    let ordering = ordering(&completed, [true], limits.exact);
    let context = generator.context();
    let exact = two_row_exact_epoch(&ordering, context, limits.exact);
    let mut session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let frozen = ExactLazyFrozenJanetEpoch::try_import(
        &mut session,
        exact.division(),
        &ordering,
        context,
        limits,
    )
    .unwrap();
    let mut ledger =
        ExactLazyCompletionLedger::try_new(&session, &ordering, context, limits).unwrap();
    let initial = ExactLazyJanetEpoch::try_initial_from_frozen(
        &session,
        frozen,
        &ordering,
        context,
        limits,
        CompletionGeometryLimits::default(),
        &mut ledger,
    )
    .unwrap();
    let shared = || {
        vec![
            ExactLazyReplacementForTest::Shared(0),
            ExactLazyReplacementForTest::Shared(1),
        ]
    };
    let mut sibling =
        ExactLazyCompletionLedger::try_new(&session, &ordering, context, limits).unwrap();
    let primary_before = ledger.work_census();
    let sibling_before = sibling.work_census();
    assert_eq!(
        initial
            .try_replacement_division_successor_for_test(
                &session,
                shared(),
                &ordering,
                context,
                limits,
                &mut sibling,
            )
            .unwrap_err(),
        ExactLazyError::WrongCompletionLedger
    );
    assert_eq!(ledger.work_census(), primary_before);
    assert_eq!(sibling.work_census(), sibling_before);

    let division = initial
        .try_replacement_division_successor_for_test(
            &session,
            shared(),
            &ordering,
            context,
            limits,
            &mut ledger,
        )
        .unwrap();
    assert_eq!(
        division
            .try_seal(
                &session,
                &ordering,
                context,
                limits,
                CompletionGeometryLimits::default(),
                &sibling,
            )
            .unwrap_err(),
        ExactLazyError::WrongCompletionLedger
    );
}

#[test]
fn epoch_rejects_zero_nonmonic_duplicate_foreign_and_aborted_rows_before_index_work() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ExactLazyLimits::default();
    let ordering = ordering(&completed, [true], limits.exact);
    let context = generator.context();
    let mut session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();

    let zero = committed_zero(&mut session, context);
    let mut ledger =
        ExactLazyCompletionLedger::try_new(&session, &ordering, context, limits).unwrap();
    assert_eq!(
        ExactLazyJanetEpoch::try_initial_for_test(
            &session,
            vec![zero],
            &ordering,
            context,
            limits,
            CompletionGeometryLimits::default(),
            &mut ledger,
        )
        .unwrap_err(),
        ExactLazyError::Involutive(InvolutiveError::ZeroBasisRow)
    );
    assert_eq!(ledger.work_census(), Default::default());

    let nonmonic = Arc::new(
        try_import_exact_consequence(
            &mut session,
            &consequence(
                &ordering,
                context,
                [(0, context.one()), (1, context.integer(2))],
                limits.exact,
            ),
            &ordering,
            context,
            limits,
        )
        .unwrap(),
    );
    assert_eq!(
        ExactLazyJanetEpoch::try_initial_for_test(
            &session,
            vec![nonmonic],
            &ordering,
            context,
            limits,
            CompletionGeometryLimits::default(),
            &mut ledger,
        )
        .unwrap_err(),
        ExactLazyError::InvalidSupport {
            detail: "an exact-lazy Janet row is not monic structural one",
        }
    );
    assert_eq!(ledger.work_census(), Default::default());

    let first_exact = consequence(
        &ordering,
        context,
        [(0, context.integer(3)), (2, context.one())],
        limits.exact,
    );
    let second_exact = consequence(
        &ordering,
        context,
        [(0, context.integer(5)), (2, context.one())],
        limits.exact,
    );
    let first = Arc::new(
        try_import_exact_consequence(&mut session, &first_exact, &ordering, context, limits)
            .unwrap(),
    );
    let second = Arc::new(
        try_import_exact_consequence(&mut session, &second_exact, &ordering, context, limits)
            .unwrap(),
    );
    assert!(matches!(
        ExactLazyJanetEpoch::try_initial_for_test(
            &session,
            vec![first, second],
            &ordering,
            context,
            limits,
            CompletionGeometryLimits::default(),
            &mut ledger,
        ),
        Err(ExactLazyError::Involutive(
            InvolutiveError::DuplicateLeadingShift
        ))
    ));
    assert_eq!(ledger.work_census(), Default::default());

    let committed =
        try_import_exact_consequence(&mut session, &first_exact, &ordering, context, limits)
            .unwrap();
    let plan =
        try_plan_exact_consequence_import(&session, &first_exact, &ordering, context, limits)
            .unwrap();
    let census = plan.census();
    let mut transaction = session
        .try_begin_import_batch_transaction(&[census])
        .unwrap();
    let escaped =
        try_build_planned_exact_consequence(&mut transaction, &plan, &ordering, context, limits)
            .unwrap();
    transaction.try_abort().unwrap();
    assert!(matches!(
        ExactLazyJanetEpoch::try_initial_for_test(
            &session,
            vec![Arc::new(escaped)],
            &ordering,
            context,
            limits,
            CompletionGeometryLimits::default(),
            &mut ledger,
        ),
        Err(ExactLazyError::InvalidProof { .. })
    ));
    committed.try_validate_live(&session).unwrap();

    let mut foreign = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let foreign_row = Arc::new(
        try_import_exact_consequence(&mut foreign, &first_exact, &ordering, context, limits)
            .unwrap(),
    );
    assert_eq!(
        ExactLazyJanetEpoch::try_initial_for_test(
            &session,
            vec![foreign_row],
            &ordering,
            context,
            limits,
            CompletionGeometryLimits::default(),
            &mut ledger,
        )
        .unwrap_err(),
        ExactLazyError::WrongSessionOwner
    );
    assert_eq!(ledger.work_census(), Default::default());
}

#[test]
fn epoch_and_shape_limits_reject_before_publishing_a_successor() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let mut limits = ExactLazyLimits::default();
    limits.exact.max_epoch = 0;
    let ordering = ordering(&completed, [true], limits.exact);
    let context = generator.context();
    let exact = two_row_exact_epoch(&ordering, context, limits.exact);
    let mut session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let frozen = ExactLazyFrozenJanetEpoch::try_import(
        &mut session,
        exact.division(),
        &ordering,
        context,
        limits,
    )
    .unwrap();
    let mut ledger =
        ExactLazyCompletionLedger::try_new(&session, &ordering, context, limits).unwrap();
    let initial = ExactLazyJanetEpoch::try_initial_from_frozen(
        &session,
        frozen,
        &ordering,
        context,
        limits,
        CompletionGeometryLimits::default(),
        &mut ledger,
    )
    .unwrap();
    let before = ledger.work_census();
    assert_eq!(
        initial
            .try_replacement_division_successor_for_test(
                &session,
                vec![
                    ExactLazyReplacementForTest::Shared(0),
                    ExactLazyReplacementForTest::Shared(1),
                ],
                &ordering,
                context,
                limits,
                &mut ledger,
            )
            .unwrap_err(),
        ExactLazyError::Involutive(InvolutiveError::EpochLimit {
            requested: 1,
            limit: 0,
        })
    );
    assert_eq!(ledger.work_census(), before);

    let widened = ExactLazyLimits {
        exact: InvolutiveLimits {
            max_epoch: 1,
            ..limits.exact
        },
        ..limits
    };
    assert_eq!(
        initial
            .division()
            .try_divisor_scratch(&ordering, widened)
            .unwrap_err(),
        ExactLazyError::WrongLimitsContract
    );

    let foreign_ordering = OreOrderingAdapter::try_new_for_completed(
        OrderingPolicy::default(),
        Mask::try_new([true]).unwrap(),
        &completed,
        limits.exact,
    )
    .unwrap();
    assert_eq!(
        initial
            .division()
            .try_divisor_scratch(&foreign_ordering, limits)
            .unwrap_err(),
        ExactLazyError::WrongOreAction
    );
}
