use std::sync::Arc;

use crate::algebra::IndexedCoefficientContext;
use crate::foundry::artifact::derive_one_loop_unit_mass_tadpole;
use crate::identity::{CompletedIbpSourceRows, ParametricIbpGenerator};
use crate::sector::{Mask, OrderingPolicy};

use super::super::super::{ForwardShift, OreOrderingAdapter};
use super::*;

const PRIME: u64 = 998_244_353;

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

fn point(context: &IndexedCoefficientContext, index: i64) -> Vec<i64> {
    let mut point = vec![7; context.base().parameter_names().len() + context.index_count()];
    *point.last_mut().unwrap() = index;
    point
}

fn shift(value: u64, limits: ExactLazyLimits) -> ForwardShift {
    ForwardShift::try_new([value], limits.exact).unwrap()
}

#[test]
fn complete_support_uses_modular_nonzero_and_one_exact_batch_for_sampled_zeros() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ExactLazyLimits::default();
    let ordering = completed_ordering(&completed, limits);
    let context = generator.context();
    let mut session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let mut transaction = session.try_begin_transaction().unwrap();
    let exact_n = context.index(0).unwrap();
    let n = transaction
        .try_exact_leaf(context, Arc::new(exact_n.clone()))
        .unwrap();
    let n_plus_one = transaction.try_add(&n, &transaction.one()).unwrap();
    let exact_neg_n = context.mul(&context.integer(-1), &exact_n).unwrap();
    let neg_n = transaction
        .try_exact_leaf(context, Arc::new(exact_neg_n))
        .unwrap();
    let nonsyntactic_zero = transaction.try_add(&n, &neg_n).unwrap();
    assert!(
        !transaction
            .try_is_structural_zero(&nonsyntactic_zero)
            .unwrap()
    );
    let row = UnclassifiedLazyOreRow::try_new(
        &transaction,
        [
            PendingLazyOreTerm::from_changed(shift(0, limits), n_plus_one),
            PendingLazyOreTerm::from_changed(shift(1, limits), n),
            PendingLazyOreTerm::from_changed(shift(2, limits), nonsyntactic_zero),
        ],
        [],
    )
    .unwrap();
    let schedule = ExactLazyProbeSchedule::try_new(
        transaction.owner(),
        transaction.coefficient_dag(),
        context,
        [ExactLazyProbeSpec::new(11, PRIME, point(context, 0))],
    )
    .unwrap();
    let mut budget = ExactLazySupportBudget::new(transaction.owner());
    let classified =
        try_classify_support(&transaction, context, &[], row, &schedule, &mut budget).unwrap();

    let terms = classified.try_terms_in_transaction(&transaction).unwrap();
    let zero_elisions = classified
        .try_exact_zero_elisions_in_transaction(&transaction)
        .unwrap();
    assert_eq!(terms.len(), 2);
    assert_eq!(zero_elisions.len(), 1);
    assert!(matches!(
        terms[0].nonzero_proof(),
        ExactNonzeroProof::Modular(_)
    ));
    assert!(matches!(
        terms[1].nonzero_proof(),
        ExactNonzeroProof::ExactFallback(_)
    ));
    assert!(matches!(
        &zero_elisions[0],
        ExactZeroProof::ExactFallback(_)
    ));
    assert_eq!(budget.census().classification_attempts(), 1);
    assert_eq!(budget.census().classification_roots(), 3);
    assert_eq!(budget.census().scheduled_probes(), 1);
    assert_eq!(budget.census().successful_probes(), 1);
    assert_eq!(budget.census().rejected_probes(), 0);
    assert_eq!(budget.census().probe().queries(), 3);
    assert_eq!(budget.census().exact_fallback_batches(), 1);
    assert_eq!(budget.census().exact_fallback_roots(), 2);
    assert_eq!(budget.exact_fallback_attempts(), 1);
    assert_eq!(budget.exact_fallback_census().output_values(), 2);
    transaction.try_commit().unwrap();
}

#[test]
fn defined_zero_guard_is_admissible_while_nonzero_guard_rejects_and_is_accounted() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ExactLazyLimits::default();
    let ordering = completed_ordering(&completed, limits);
    let context = generator.context();
    let mut session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let mut transaction = session.try_begin_transaction().unwrap();
    let n = transaction
        .try_exact_leaf(context, Arc::new(context.index(0).unwrap()))
        .unwrap();
    let n_plus_one = transaction.try_add(&n, &transaction.one()).unwrap();
    let schedule = ExactLazyProbeSchedule::try_new(
        transaction.owner(),
        transaction.coefficient_dag(),
        context,
        [ExactLazyProbeSpec::new(0, PRIME, point(context, 0))],
    )
    .unwrap();

    let defined_row = UnclassifiedLazyOreRow::try_new(
        &transaction,
        [PendingLazyOreTerm::from_changed(
            shift(0, limits),
            n_plus_one.clone(),
        )],
        [],
    )
    .unwrap();
    let mut defined_budget = ExactLazySupportBudget::new(transaction.owner());
    let defined = try_classify_support(
        &transaction,
        context,
        &[GuardProbeRequirement::Defined(n.clone())],
        defined_row,
        &schedule,
        &mut defined_budget,
    )
    .unwrap();
    assert!(matches!(
        defined.try_terms_in_transaction(&transaction).unwrap()[0].nonzero_proof(),
        ExactNonzeroProof::Modular(_)
    ));
    assert_eq!(defined_budget.census().successful_probes(), 1);
    assert_eq!(defined_budget.census().probe().queries(), 2);
    assert_eq!(defined_budget.census().exact_fallback_roots(), 0);

    let nonzero_row = UnclassifiedLazyOreRow::try_new(
        &transaction,
        [PendingLazyOreTerm::from_changed(
            shift(0, limits),
            n_plus_one,
        )],
        [],
    )
    .unwrap();
    let mut nonzero_budget = ExactLazySupportBudget::new(transaction.owner());
    let nonzero = try_classify_support(
        &transaction,
        context,
        &[
            GuardProbeRequirement::Defined(n.clone()),
            GuardProbeRequirement::Nonzero(n),
        ],
        nonzero_row,
        &schedule,
        &mut nonzero_budget,
    )
    .unwrap();
    assert!(matches!(
        nonzero.try_terms_in_transaction(&transaction).unwrap()[0].nonzero_proof(),
        ExactNonzeroProof::ExactFallback(_)
    ));
    assert_eq!(nonzero_budget.census().successful_probes(), 0);
    assert_eq!(nonzero_budget.census().rejected_probes(), 1);
    assert_eq!(nonzero_budget.census().probe().queries(), 1);
    assert_eq!(nonzero_budget.census().exact_fallback_roots(), 1);
}

#[test]
fn deterministic_schedule_uses_lowest_valid_ordinal_and_rejects_bad_schedules() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ExactLazyLimits::default();
    let ordering = completed_ordering(&completed, limits);
    let context = generator.context();
    let mut session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let mut transaction = session.try_begin_transaction().unwrap();
    let n = transaction
        .try_exact_leaf(context, Arc::new(context.index(0).unwrap()))
        .unwrap();

    assert_eq!(
        ExactLazyProbeSchedule::try_new(
            transaction.owner(),
            transaction.coefficient_dag(),
            context,
            [
                ExactLazyProbeSpec::new(2, PRIME, point(context, 0)),
                ExactLazyProbeSpec::new(1, PRIME, point(context, 1)),
            ],
        )
        .unwrap_err(),
        ExactLazyError::InvalidSupport {
            detail: "probe schedule ordinals are not strictly increasing"
        }
    );
    let mut equivalent = point(context, 0);
    *equivalent.last_mut().unwrap() = PRIME as i64;
    assert_eq!(
        ExactLazyProbeSchedule::try_new(
            transaction.owner(),
            transaction.coefficient_dag(),
            context,
            [
                ExactLazyProbeSpec::new(0, PRIME, point(context, 0)),
                ExactLazyProbeSpec::new(1, PRIME, equivalent),
            ],
        )
        .unwrap_err(),
        ExactLazyError::InvalidSupport {
            detail: "probe schedule contains residue-equivalent points"
        }
    );

    let schedule = ExactLazyProbeSchedule::try_new(
        transaction.owner(),
        transaction.coefficient_dag(),
        context,
        [
            ExactLazyProbeSpec::new(11, PRIME, point(context, 0)),
            ExactLazyProbeSpec::new(12, PRIME, point(context, 1)),
            ExactLazyProbeSpec::new(13, PRIME, point(context, 2)),
        ],
    )
    .unwrap();
    assert_eq!(schedule.len(), 3);
    assert!(!schedule.is_empty());
    let row = UnclassifiedLazyOreRow::try_new(
        &transaction,
        [PendingLazyOreTerm::from_changed(shift(0, limits), n)],
        [],
    )
    .unwrap();
    let mut budget = ExactLazySupportBudget::new(transaction.owner());
    let classified =
        try_classify_support(&transaction, context, &[], row, &schedule, &mut budget).unwrap();
    let ExactNonzeroProof::Modular(proof) =
        classified.try_terms_in_transaction(&transaction).unwrap()[0].nonzero_proof()
    else {
        panic!("the second scheduled point must certify n")
    };
    assert_eq!(proof.certificate().probe().ordinal(), 12);
    assert_eq!(budget.census().scheduled_probes(), 2);
    assert_eq!(budget.census().successful_probes(), 2);
    assert_eq!(budget.census().exact_fallback_roots(), 0);
}

#[test]
fn cumulative_probe_and_fallback_caps_charge_before_error_escape() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let mut limits = ExactLazyLimits::default();
    limits.support.max_total_probe_queries = 0;
    let ordering = completed_ordering(&completed, limits);
    let context = generator.context();
    let mut session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let mut transaction = session.try_begin_transaction().unwrap();
    let n = transaction
        .try_exact_leaf(context, Arc::new(context.index(0).unwrap()))
        .unwrap();
    let n_plus_one = transaction.try_add(&n, &transaction.one()).unwrap();
    let schedule = ExactLazyProbeSchedule::try_new(
        transaction.owner(),
        transaction.coefficient_dag(),
        context,
        [ExactLazyProbeSpec::new(0, PRIME, point(context, 1))],
    )
    .unwrap();
    let row = UnclassifiedLazyOreRow::try_new(
        &transaction,
        [PendingLazyOreTerm::from_changed(
            shift(0, limits),
            n_plus_one,
        )],
        [],
    )
    .unwrap();
    let mut budget = ExactLazySupportBudget::new(transaction.owner());
    assert_eq!(
        try_classify_support(&transaction, context, &[], row, &schedule, &mut budget).unwrap_err(),
        ExactLazyError::ResourceLimit {
            resource: "exact-lazy cumulative probe queries",
            requested: 1,
            limit: 0,
        }
    );
    assert_eq!(budget.census().scheduled_probes(), 1);
    assert_eq!(budget.census().successful_probes(), 1);
    assert_eq!(budget.census().probe().queries(), 1);

    transaction.try_abort().unwrap();

    let mut fallback_limits = ExactLazyLimits::default();
    fallback_limits.support.max_exact_fallback_roots_per_batch = 0;
    let fallback_ordering = completed_ordering(&completed, fallback_limits);
    let mut fallback_session =
        ExactLazySession::try_new(&fallback_ordering, context, &completed, fallback_limits)
            .unwrap();
    let mut fallback_transaction = fallback_session.try_begin_transaction().unwrap();
    let n = fallback_transaction
        .try_exact_leaf(context, Arc::new(context.index(0).unwrap()))
        .unwrap();
    let empty_schedule = ExactLazyProbeSchedule::try_new(
        fallback_transaction.owner(),
        fallback_transaction.coefficient_dag(),
        context,
        [],
    )
    .unwrap();
    let row = UnclassifiedLazyOreRow::try_new(
        &fallback_transaction,
        [PendingLazyOreTerm::from_changed(
            shift(0, fallback_limits),
            n,
        )],
        [],
    )
    .unwrap();
    let mut fallback_budget = ExactLazySupportBudget::new(fallback_transaction.owner());
    assert_eq!(
        try_classify_support(
            &fallback_transaction,
            context,
            &[],
            row,
            &empty_schedule,
            &mut fallback_budget,
        )
        .unwrap_err(),
        ExactLazyError::ResourceLimit {
            resource: "exact-lazy exact-support fallback roots",
            requested: 1,
            limit: 0,
        }
    );
    assert_eq!(fallback_budget.census().exact_fallback_batches(), 1);
    assert_eq!(fallback_budget.census().exact_fallback_roots(), 1);
    assert_eq!(fallback_budget.exact_fallback_attempts(), 0);
}

#[test]
fn classified_row_escaping_abort_rechecks_shared_modular_batch_liveness() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ExactLazyLimits::default();
    let ordering = completed_ordering(&completed, limits);
    let context = generator.context();
    let mut session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();

    let mut stable_transaction = session.try_begin_transaction().unwrap();
    let exact_n = context.index(0).unwrap();
    let n = stable_transaction
        .try_exact_leaf(context, Arc::new(exact_n.clone()))
        .unwrap();
    stable_transaction.try_commit().unwrap();

    let mut transient_transaction = session.try_begin_transaction().unwrap();
    let neg_n = transient_transaction
        .try_exact_leaf(
            context,
            Arc::new(context.mul(&context.integer(-1), &exact_n).unwrap()),
        )
        .unwrap();
    let transient_zero = transient_transaction.try_add(&n, &neg_n).unwrap();
    assert!(
        !transient_transaction
            .try_is_structural_zero(&transient_zero)
            .unwrap()
    );
    let row = UnclassifiedLazyOreRow::try_new(
        &transient_transaction,
        [
            PendingLazyOreTerm::from_changed(shift(0, limits), n.clone()),
            PendingLazyOreTerm::from_changed(shift(1, limits), transient_zero),
        ],
        [],
    )
    .unwrap();
    let schedule = ExactLazyProbeSchedule::try_new(
        transient_transaction.owner(),
        transient_transaction.coefficient_dag(),
        context,
        [ExactLazyProbeSpec::new(0, PRIME, point(context, 2))],
    )
    .unwrap();
    let mut budget = ExactLazySupportBudget::new(transient_transaction.owner());
    let escaped = try_classify_support(
        &transient_transaction,
        context,
        &[],
        row,
        &schedule,
        &mut budget,
    )
    .unwrap();
    assert_eq!(
        escaped
            .try_terms_in_transaction(&transient_transaction)
            .unwrap()
            .len(),
        1
    );
    transient_transaction.try_abort().unwrap();

    // The retained n root predates the transaction and remains live, while
    // its shared certificate seal also names the rolled-back zero root.
    session.require_lazy_coefficient(&n).unwrap();
    assert_eq!(
        escaped.try_terms_live(&session).unwrap_err(),
        ExactLazyError::InvalidProof {
            detail: "classified Ore term proof is no longer live"
        }
    );
    assert_eq!(
        escaped.try_leading_term(&session, &ordering).unwrap_err(),
        ExactLazyError::InvalidProof {
            detail: "classified Ore term proof is no longer live"
        }
    );
}
