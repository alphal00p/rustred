use crate::algebra::{
    CoefficientContext, ExactAlgebraError, IndexedAlgebraError, IndexedCoefficient,
    IndexedCoefficientContext, IndexedPolynomial,
};
use crate::foundry::artifact::{
    derive_one_loop_unit_mass_tadpole, derive_two_loop_unit_mass_sunset,
};
use crate::identity::{CompletedIbpSourceRows, ParametricIbpGenerator};
use crate::sector::{Mask, OrderingPolicy};

use super::super::{
    ForwardShift, OrdinaryChartLiftLimits, OreConsequence, OreOrderingAdapter, OreRow,
    try_lift_completed_ordinary_sources,
};
use super::arithmetic::polynomial_as_coefficient;
use super::error::ProjectiveError;
use super::limits::{ProjectiveLimits, ProjectiveNormalizationPolicy, ProjectiveWorkBudget};
use super::model::{
    PrimitiveOreConsequence, ProjectiveNormalizationState, ValidatedProjectiveConsequence,
};
use super::polynomial::PolynomialWork;
use super::replay::ProjectiveReplayCursor;

fn context(scope: &str, arity: usize) -> IndexedCoefficientContext {
    IndexedCoefficientContext::try_new(
        &CoefficientContext::new(std::iter::empty::<&str>()),
        scope,
        arity,
    )
    .unwrap()
}

fn ordering(mask: &[bool], context_limits: ProjectiveLimits) -> OreOrderingAdapter {
    OreOrderingAdapter::try_new(
        OrderingPolicy::default(),
        Mask::try_new(mask.iter().copied()).unwrap(),
        context_limits.involutive,
    )
    .unwrap()
}

fn shift(values: &[u64], limits: ProjectiveLimits) -> ForwardShift {
    ForwardShift::try_new(values.iter().copied(), limits.involutive).unwrap()
}

fn polynomial(
    context: &IndexedCoefficientContext,
    coefficient: &IndexedCoefficient,
) -> IndexedPolynomial {
    context
        .numerator_condition_with_limits(coefficient, Default::default())
        .unwrap()
}

fn source(
    source_ordinal: usize,
    terms: impl IntoIterator<Item = (ForwardShift, IndexedCoefficient)>,
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    limits: ProjectiveLimits,
) -> OreConsequence {
    OreConsequence::try_from_source(
        source_ordinal,
        OreRow::try_new(ordering, terms, context, limits.involutive).unwrap(),
        ordering,
        context,
        limits.involutive,
    )
    .unwrap()
}

fn assert_projectively_equal(
    projective: &PrimitiveOreConsequence,
    rational: &OreConsequence,
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    limits: ProjectiveLimits,
) {
    projective.try_validate(ordering, context, limits).unwrap();
    rational
        .try_validate(ordering, context, limits.involutive)
        .unwrap();
    assert_eq!(projective.is_zero(), rational.is_zero());
    assert_eq!(projective.row().len(), rational.row().terms().len());
    assert_eq!(
        projective.provenance().len(),
        rational.provenance().terms().len()
    );

    let (projective_scale, rational_scale) =
        if let Some(projective_leader) = projective.try_leading_term(ordering).unwrap() {
            let (rational_leader, _) = rational.row().try_leading_term(ordering).unwrap().unwrap();
            assert_eq!(projective_leader.shift(), rational_leader.shift());
            (
                polynomial_as_coefficient(projective_leader.coefficient(), context).unwrap(),
                rational_leader.coefficient().clone(),
            )
        } else if let (Some(projective_source), Some(rational_source)) = (
            projective.provenance().first(),
            rational.provenance().terms().first(),
        ) {
            assert_eq!(
                (
                    projective_source.source_ordinal(),
                    projective_source.left_shift()
                ),
                (
                    rational_source.source_ordinal(),
                    rational_source.left_shift()
                )
            );
            (
                polynomial_as_coefficient(projective_source.left_coefficient(), context).unwrap(),
                rational_source.left_coefficient().clone(),
            )
        } else {
            return;
        };

    for (projective_term, rational_term) in projective.row().iter().zip(rational.row().terms()) {
        assert_eq!(projective_term.shift(), rational_term.shift());
        let projective_coefficient =
            polynomial_as_coefficient(projective_term.coefficient(), context).unwrap();
        assert_eq!(
            context
                .mul(&projective_coefficient, &rational_scale)
                .unwrap(),
            context
                .mul(rational_term.coefficient(), &projective_scale)
                .unwrap(),
        );
    }
    for (projective_term, rational_term) in projective
        .provenance()
        .iter()
        .zip(rational.provenance().terms())
    {
        assert_eq!(
            (
                projective_term.source_ordinal(),
                projective_term.left_shift()
            ),
            (rational_term.source_ordinal(), rational_term.left_shift())
        );
        let projective_coefficient =
            polynomial_as_coefficient(projective_term.left_coefficient(), context).unwrap();
        assert_eq!(
            context
                .mul(&projective_coefficient, &rational_scale)
                .unwrap(),
            context
                .mul(rational_term.left_coefficient(), &projective_scale)
                .unwrap(),
        );
    }
}

fn assert_projective_source_replay(
    projective: &PrimitiveOreConsequence,
    sources: &[(&OreConsequence, &ForwardShift)],
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    limits: ProjectiveLimits,
) {
    let mut replayed = OreConsequence::try_zero(ordering, context, limits.involutive).unwrap();
    for term in projective.provenance() {
        let (source, source_left_shift) = sources
            .get(term.source_ordinal())
            .expect("projective provenance source ordinal must exist");
        let relative_shift = term
            .left_shift()
            .try_checked_sub(source_left_shift, limits.involutive)
            .expect("derived source shift must contain its chart-lift shift");
        replayed = replayed
            .try_left_axpy(
                &polynomial_as_coefficient(term.left_coefficient(), context).unwrap(),
                &relative_shift,
                source,
                ordering,
                context,
                limits.involutive,
            )
            .unwrap();
    }
    assert_eq!(projective.row().len(), replayed.row().terms().len());
    for (projective_term, replayed_term) in projective.row().iter().zip(replayed.row().terms()) {
        assert_eq!(projective_term.shift(), replayed_term.shift());
        assert_eq!(
            polynomial_as_coefficient(projective_term.coefficient(), context).unwrap(),
            replayed_term.coefficient().clone(),
        );
    }
}

#[test]
fn rational_ingress_clears_the_complete_augmented_vector_and_retains_denominators() {
    let limits = ProjectiveLimits::default();
    let context = context("projective-rational-ingress", 1);
    let ordering = ordering(&[true], limits);
    let zero = shift(&[0], limits);
    let unit = shift(&[1], limits);
    let n = context.index(0).unwrap();
    let n_plus_one = context.add(&n, &context.one()).unwrap();
    let n_plus_two = context.add(&n, &context.integer(2)).unwrap();
    let source = source(
        0,
        [
            (zero, context.div(&context.one(), &n_plus_one).unwrap()),
            (unit, context.div(&context.one(), &n_plus_two).unwrap()),
        ],
        &ordering,
        &context,
        limits,
    );

    let mut budget = ProjectiveWorkBudget::new(limits);
    let projective = PrimitiveOreConsequence::try_from_rational(
        &source,
        &ordering,
        &context,
        &mut budget,
        limits,
    )
    .unwrap();
    assert_projectively_equal(&projective, &source, &ordering, &context, limits);
    assert_eq!(projective.required_nonzero_guards().len(), 2);
    assert!(
        projective
            .required_nonzero_guards()
            .iter()
            .any(|guard| guard.as_ref() == &polynomial(&context, &n_plus_one))
    );
    assert!(
        projective
            .required_nonzero_guards()
            .iter()
            .any(|guard| guard.as_ref() == &polynomial(&context, &n_plus_two))
    );
}

#[test]
fn rational_ingress_skips_unit_denominators_after_a_nontrivial_lcm() {
    let limits = ProjectiveLimits::default();
    let context = context("projective-unit-denominator-lcm", 1);
    let ordering = ordering(&[true], limits);
    let zero = shift(&[0], limits);
    let unit = shift(&[1], limits);
    let squared = shift(&[2], limits);
    let n = context.index(0).unwrap();
    let denominator = context.add(&n, &context.one()).unwrap();
    let source = source(
        0,
        [
            (zero, context.div(&context.one(), &denominator).unwrap()),
            (unit, context.integer(2)),
            (squared, context.integer(3)),
        ],
        &ordering,
        &context,
        limits,
    );

    let mut budget = ProjectiveWorkBudget::new(limits);
    let projective = PrimitiveOreConsequence::try_from_rational(
        &source,
        &ordering,
        &context,
        &mut budget,
        limits,
    )
    .unwrap();

    assert_projectively_equal(&projective, &source, &ordering, &context, limits);
    assert_eq!(
        budget.census().lcm_steps(),
        1,
        "only the nonunit denominator may enter the common-LCM fold",
    );
}

#[test]
fn content_normalization_is_augmented_and_never_row_only() {
    let limits = ProjectiveLimits::default();
    let context = context("projective-augmented-content", 1);
    let ordering = ordering(&[true], limits);
    let zero = shift(&[0], limits);
    let unit = shift(&[1], limits);
    let n = context.index(0).unwrap();
    let common = context.add(&n, &context.one()).unwrap();
    let row_content = source(
        0,
        [
            (zero.clone(), common.clone()),
            (unit, context.mul(&context.integer(2), &common).unwrap()),
        ],
        &ordering,
        &context,
        limits,
    );
    let mut budget = ProjectiveWorkBudget::new(limits);
    let retained = PrimitiveOreConsequence::try_from_rational(
        &row_content,
        &ordering,
        &context,
        &mut budget,
        limits,
    )
    .unwrap();
    assert_eq!(
        retained.provenance()[0].left_coefficient(),
        &polynomial(&context, &context.one()),
        "the provenance unit prevents illegal row-only content removal",
    );
    assert_eq!(
        retained.coefficient(&zero),
        Some(&polynomial(&context, &common))
    );

    let scaled = OreConsequence::try_zero(&ordering, &context, limits.involutive)
        .unwrap()
        .try_left_axpy(
            &common,
            &zero,
            &row_content,
            &ordering,
            &context,
            limits.involutive,
        )
        .unwrap();
    let normalized = PrimitiveOreConsequence::try_from_rational(
        &scaled,
        &ordering,
        &context,
        &mut budget,
        limits,
    )
    .unwrap();
    assert_eq!(normalized, retained);
}

#[test]
fn rational_ingress_clears_a_provenance_only_denominator_and_replays_its_sources() {
    let limits = ProjectiveLimits::default();
    let context = context("projective-provenance-denominator", 1);
    let ordering = ordering(&[true], limits);
    let zero = shift(&[0], limits);
    let n = context.index(0).unwrap();
    let denominator = context.add(&n, &context.one()).unwrap();
    let first = source(
        0,
        [(zero.clone(), context.one())],
        &ordering,
        &context,
        limits,
    );
    let second = source(
        1,
        [(zero.clone(), context.one())],
        &ordering,
        &context,
        limits,
    );
    let replayed_zero = OreConsequence::try_zero(&ordering, &context, limits.involutive)
        .unwrap()
        .try_left_axpy(
            &context.one(),
            &zero,
            &first,
            &ordering,
            &context,
            limits.involutive,
        )
        .unwrap()
        .try_left_axpy(
            &context.integer(-1),
            &zero,
            &second,
            &ordering,
            &context,
            limits.involutive,
        )
        .unwrap();
    assert!(replayed_zero.row().is_zero());
    assert_eq!(replayed_zero.provenance().terms().len(), 2);
    let scaled = OreConsequence::try_zero(&ordering, &context, limits.involutive)
        .unwrap()
        .try_left_axpy(
            &context.div(&context.one(), &denominator).unwrap(),
            &zero,
            &replayed_zero,
            &ordering,
            &context,
            limits.involutive,
        )
        .unwrap();

    let mut budget = ProjectiveWorkBudget::default();
    let projective = PrimitiveOreConsequence::try_from_rational(
        &scaled,
        &ordering,
        &context,
        &mut budget,
        limits,
    )
    .unwrap();
    assert_projectively_equal(&projective, &scaled, &ordering, &context, limits);
    assert!(projective.row().is_empty());
    assert_eq!(projective.provenance().len(), 2);
    assert_eq!(projective.required_nonzero_guards().len(), 1);
    assert_eq!(
        projective.required_nonzero_guards()[0].as_ref(),
        &polynomial(&context, &denominator),
    );
    assert_eq!(
        projective.provenance()[0].left_coefficient(),
        &polynomial(&context, &context.one()),
        "canonical sign must be selected from provenance when the row is empty",
    );
    assert_projective_source_replay(
        &projective,
        &[(&first, &zero), (&second, &zero)],
        &ordering,
        &context,
        limits,
    );
}

#[test]
fn projective_context_arity_and_guard_candidate_caps_fail_before_work() {
    let defaults = ProjectiveLimits::default();
    let primary_context = context("projective-preflight", 1);
    let ordering = ordering(&[true], defaults);
    let zero = shift(&[0], defaults);
    let rational = source(
        0,
        [(zero, primary_context.one())],
        &ordering,
        &primary_context,
        defaults,
    );
    let limits = ProjectiveLimits {
        max_localization_guard_candidates: 1,
        ..defaults
    };
    let mut budget = ProjectiveWorkBudget::new(limits);
    assert_eq!(
        PrimitiveOreConsequence::try_from_rational(
            &rational,
            &ordering,
            &primary_context,
            &mut budget,
            limits,
        )
        .unwrap_err(),
        ProjectiveError::ResourceLimit {
            resource: "projective incoming localization guard candidates",
            requested: 2,
            limit: 1,
        },
    );
    assert_eq!(budget.census(), Default::default());

    let mut projective_budget = ProjectiveWorkBudget::default();
    let mut projective = PrimitiveOreConsequence::try_from_rational(
        &rational,
        &ordering,
        &primary_context,
        &mut projective_budget,
        defaults,
    )
    .unwrap();
    let foreign_context = context("projective-preflight-foreign", 1);
    assert_eq!(
        projective
            .try_validate(&ordering, &foreign_context, defaults)
            .unwrap_err(),
        ProjectiveError::ContextFingerprintMismatch,
    );
    projective.arity = 2;
    assert_eq!(
        projective
            .try_validate(&ordering, &primary_context, defaults)
            .unwrap_err(),
        ProjectiveError::ContextIndexArityMismatch {
            consequence_arity: 2,
            context_index_count: 1,
        },
    );
}

#[test]
fn cumulative_projective_work_budget_cannot_be_reset_between_ingresses() {
    let defaults = ProjectiveLimits::default();
    let context = context("projective-cumulative-budget", 1);
    let ordering = ordering(&[true], defaults);
    let rational = source(
        0,
        [(shift(&[0], defaults), context.one())],
        &ordering,
        &context,
        defaults,
    );
    let mut measurement = ProjectiveWorkBudget::default();
    PrimitiveOreConsequence::try_from_rational(
        &rational,
        &ordering,
        &context,
        &mut measurement,
        defaults,
    )
    .unwrap();
    let one_ingress = measurement.census().polynomial_operations();
    assert!(one_ingress > 0);
    let operation_cap = one_ingress.checked_mul(2).unwrap() - 1;
    let limits = ProjectiveLimits {
        max_polynomial_operations: operation_cap,
        ..defaults
    };
    let mut wrong_contract = ProjectiveWorkBudget::default();
    assert_eq!(
        PrimitiveOreConsequence::try_from_rational(
            &rational,
            &ordering,
            &context,
            &mut wrong_contract,
            limits,
        )
        .unwrap_err(),
        ProjectiveError::WorkBudgetLimitsMismatch,
    );
    assert_eq!(wrong_contract.census(), Default::default());
    let mut budget = ProjectiveWorkBudget::new(limits);
    PrimitiveOreConsequence::try_from_rational(&rational, &ordering, &context, &mut budget, limits)
        .unwrap();
    assert_eq!(
        PrimitiveOreConsequence::try_from_rational(
            &rational,
            &ordering,
            &context,
            &mut budget,
            limits,
        )
        .unwrap_err(),
        ProjectiveError::ResourceLimit {
            resource: "projective polynomial operations",
            requested: operation_cap + 1,
            limit: operation_cap,
        },
    );
    assert_eq!(budget.census().polynomial_operations(), operation_cap + 1);
}

#[test]
fn native_exact_division_and_multiple_gcd_caps_fail_closed() {
    let defaults = ProjectiveLimits::default();
    let context = context("projective-native-caps", 1);
    let n = context.index(0).unwrap();
    let numerator = polynomial(
        &context,
        &context
            .add(&context.mul(&n, &n).unwrap(), &context.one())
            .unwrap(),
    );
    let denominator = polynomial(&context, &context.add(&n, &context.one()).unwrap());
    let mut division_budget = ProjectiveWorkBudget::default();
    assert_eq!(
        PolynomialWork::try_new(&context, defaults, &mut division_budget)
            .unwrap()
            .exact_div(&numerator, &denominator)
            .unwrap_err(),
        ProjectiveError::NonExactPolynomialDivision,
    );
    let mut zero_division_budget = ProjectiveWorkBudget::default();
    assert_eq!(
        PolynomialWork::try_new(&context, defaults, &mut zero_division_budget)
            .unwrap()
            .exact_div(&numerator, &polynomial(&context, &context.zero()),)
            .unwrap_err(),
        ProjectiveError::IndexedAlgebra(IndexedAlgebraError::ExactAlgebra(
            ExactAlgebraError::DivisionByZero,
        )),
    );

    let ordering = ordering(&[true], defaults);
    let row = source(
        0,
        [(shift(&[0], defaults), n)],
        &ordering,
        &context,
        defaults,
    );
    let limits = ProjectiveLimits {
        max_gcd_multiple_inputs: 1,
        ..defaults
    };
    let mut budget = ProjectiveWorkBudget::new(limits);
    assert_eq!(
        PrimitiveOreConsequence::try_from_rational(&row, &ordering, &context, &mut budget, limits,)
            .unwrap_err(),
        ProjectiveError::ResourceLimit {
            resource: "projective polynomial multiple-GCD inputs",
            requested: 2,
            limit: 1,
        },
    );
}

#[test]
fn pseudo_reduction_preflights_the_complete_augmented_input() {
    let defaults = ProjectiveLimits::default();
    let context = context("projective-augmented-input-cap", 1);
    let ordering = ordering(&[true], defaults);
    let target = shift(&[1], defaults);
    let mut budget = ProjectiveWorkBudget::default();
    let subject = PrimitiveOreConsequence::try_from_rational(
        &source(
            0,
            [(target.clone(), context.one())],
            &ordering,
            &context,
            defaults,
        ),
        &ordering,
        &context,
        &mut budget,
        defaults,
    )
    .unwrap();
    let divisor = PrimitiveOreConsequence::try_from_rational(
        &source(
            1,
            [(target.clone(), context.one())],
            &ordering,
            &context,
            defaults,
        ),
        &ordering,
        &context,
        &mut budget,
        defaults,
    )
    .unwrap();
    let zero = shift(&[0], defaults);
    let limits = ProjectiveLimits {
        max_augmented_entries: 3,
        ..defaults
    };
    let mut reduction_budget = ProjectiveWorkBudget::new(limits);
    assert_eq!(
        subject
            .try_pseudo_reduce(
                &target,
                &zero,
                &divisor,
                &ordering,
                &context,
                ProjectiveNormalizationPolicy::EveryCancellation,
                &mut reduction_budget,
                limits,
            )
            .unwrap_err(),
        ProjectiveError::ResourceLimit {
            resource: "projective pseudo-reduction augmented inputs",
            requested: 4,
            limit: 3,
        },
    );
}

#[test]
fn replay_accepts_nonleading_reducible_targets_and_enforces_selected_target_descent() {
    let limits = ProjectiveLimits::default();
    let context = context("projective-replay-descent", 1);
    let ordering = ordering(&[true], limits);
    let zero = shift(&[0], limits);
    let unit = shift(&[1], limits);
    let squared = shift(&[2], limits);
    let subject = source(
        0,
        [
            (zero.clone(), context.integer(2)),
            (unit.clone(), context.one()),
            (squared.clone(), context.integer(3)),
        ],
        &ordering,
        &context,
        limits,
    );
    let unit_divisor = source(
        1,
        [(zero.clone(), context.one()), (unit.clone(), context.one())],
        &ordering,
        &context,
        limits,
    );
    let zero_divisor = source(
        2,
        [(zero.clone(), context.one())],
        &ordering,
        &context,
        limits,
    );
    let mut budget = ProjectiveWorkBudget::default();
    let projective_subject = PrimitiveOreConsequence::try_from_rational(
        &subject,
        &ordering,
        &context,
        &mut budget,
        limits,
    )
    .unwrap();
    assert_eq!(
        projective_subject
            .try_leading_term(&ordering)
            .unwrap()
            .unwrap()
            .shift(),
        &squared,
    );
    let projective_unit_divisor = PrimitiveOreConsequence::try_from_rational(
        &unit_divisor,
        &ordering,
        &context,
        &mut budget,
        limits,
    )
    .unwrap();
    let projective_zero_divisor = PrimitiveOreConsequence::try_from_rational(
        &zero_divisor,
        &ordering,
        &context,
        &mut budget,
        limits,
    )
    .unwrap();
    let validated_unit_divisor = ValidatedProjectiveConsequence::try_new(
        &projective_unit_divisor,
        &ordering,
        &context,
        limits,
    )
    .unwrap();
    let validated_zero_divisor = ValidatedProjectiveConsequence::try_new(
        &projective_zero_divisor,
        &ordering,
        &context,
        limits,
    )
    .unwrap();
    let mut replica_build_budget = ProjectiveWorkBudget::default();
    let measured_subject = PrimitiveOreConsequence::try_from_rational(
        &subject,
        &ordering,
        &context,
        &mut replica_build_budget,
        limits,
    )
    .unwrap();
    let capped_subject = PrimitiveOreConsequence::try_from_rational(
        &subject,
        &ordering,
        &context,
        &mut replica_build_budget,
        limits,
    )
    .unwrap();
    let capped_expected_subject = PrimitiveOreConsequence::try_from_rational(
        &subject,
        &ordering,
        &context,
        &mut replica_build_budget,
        limits,
    )
    .unwrap();
    let mut replay = ProjectiveReplayCursor::try_new(
        projective_subject,
        &ordering,
        &context,
        &mut budget,
        limits,
    )
    .unwrap();
    replay
        .try_pseudo_reduce_next(
            &unit,
            &zero,
            &validated_unit_divisor,
            &ordering,
            &context,
            ProjectiveNormalizationPolicy::AdmissionOnly,
            limits,
        )
        .unwrap();
    assert_eq!(
        replay
            .consequence()
            .try_leading_term(&ordering)
            .unwrap()
            .unwrap()
            .shift(),
        &squared,
        "a larger irreducible row term must survive reduction of the greatest reducible target",
    );
    assert_eq!(
        replay.consequence().normalization_state(),
        ProjectiveNormalizationState::Deferred,
    );
    let rational_after_unit = subject
        .try_left_axpy(
            &context.integer(-1),
            &zero,
            &unit_divisor,
            &ordering,
            &context,
            limits.involutive,
        )
        .unwrap();
    assert_projectively_equal(
        replay.consequence(),
        &rational_after_unit,
        &ordering,
        &context,
        limits,
    );

    let mut measurement_budget = ProjectiveWorkBudget::default();
    let mut measurement_replay = ProjectiveReplayCursor::try_new(
        measured_subject,
        &ordering,
        &context,
        &mut measurement_budget,
        limits,
    )
    .unwrap();
    measurement_replay
        .try_pseudo_reduce_next(
            &unit,
            &zero,
            &validated_unit_divisor,
            &ordering,
            &context,
            ProjectiveNormalizationPolicy::AdmissionOnly,
            limits,
        )
        .unwrap();
    let first_step_operations = measurement_replay.work_census().polynomial_operations();
    assert!(first_step_operations > 0);

    let work_before_rejection = replay.work_census();
    assert_eq!(
        replay
            .try_pseudo_reduce_next(
                &unit,
                &zero,
                &validated_unit_divisor,
                &ordering,
                &context,
                ProjectiveNormalizationPolicy::AdmissionOnly,
                limits,
            )
            .unwrap_err(),
        ProjectiveError::TargetExceedsPreviousSelection,
    );
    assert_eq!(replay.consequence(), measurement_replay.consequence());
    assert_eq!(replay.work_census(), work_before_rejection);

    replay
        .try_pseudo_reduce_next(
            &zero,
            &zero,
            &validated_zero_divisor,
            &ordering,
            &context,
            ProjectiveNormalizationPolicy::AdmissionOnly,
            limits,
        )
        .unwrap();
    let rational_after_zero = rational_after_unit
        .try_left_axpy(
            &context.integer(-1),
            &zero,
            &zero_divisor,
            &ordering,
            &context,
            limits.involutive,
        )
        .unwrap();
    assert_projectively_equal(
        replay.consequence(),
        &rational_after_zero,
        &ordering,
        &context,
        limits,
    );
    let normalizations_before_admission = replay.work_census().content_normalizations();
    let normalized = replay
        .try_into_fully_normalized(&ordering, &context, limits)
        .unwrap();
    assert!(normalized.is_fully_normalized());
    assert_eq!(
        budget.census().content_normalizations(),
        normalizations_before_admission + 1,
    );
    assert_projectively_equal(
        &normalized,
        &rational_after_zero,
        &ordering,
        &context,
        limits,
    );
    assert!(
        ProjectiveNormalizationPolicy::WhenAugmentedEntriesDoNotExceed { max_entries: 4 }
            .normalize_after_cancellation(4)
    );
    assert!(
        !ProjectiveNormalizationPolicy::WhenAugmentedEntriesDoNotExceed { max_entries: 4 }
            .normalize_after_cancellation(5)
    );

    let capped_limits = ProjectiveLimits {
        max_polynomial_operations: first_step_operations,
        ..limits
    };
    let mut capped_budget = ProjectiveWorkBudget::new(capped_limits);
    let mut capped_replay = ProjectiveReplayCursor::try_new(
        capped_subject,
        &ordering,
        &context,
        &mut capped_budget,
        capped_limits,
    )
    .unwrap();
    let work_before_limits_mismatch = capped_replay.work_census();
    assert_eq!(
        capped_replay
            .try_pseudo_reduce_next(
                &unit,
                &zero,
                &validated_unit_divisor,
                &ordering,
                &context,
                ProjectiveNormalizationPolicy::AdmissionOnly,
                capped_limits,
            )
            .unwrap_err(),
        ProjectiveError::ValidatedDivisorLimitsMismatch,
    );
    assert_eq!(capped_replay.consequence(), &capped_expected_subject);
    assert_eq!(capped_replay.work_census(), work_before_limits_mismatch);
    let capped_validated_unit_divisor = ValidatedProjectiveConsequence::try_new(
        &projective_unit_divisor,
        &ordering,
        &context,
        capped_limits,
    )
    .unwrap();
    let capped_validated_zero_divisor = ValidatedProjectiveConsequence::try_new(
        &projective_zero_divisor,
        &ordering,
        &context,
        capped_limits,
    )
    .unwrap();
    capped_replay
        .try_pseudo_reduce_next(
            &unit,
            &zero,
            &capped_validated_unit_divisor,
            &ordering,
            &context,
            ProjectiveNormalizationPolicy::AdmissionOnly,
            capped_limits,
        )
        .unwrap();
    assert_eq!(
        capped_replay
            .try_pseudo_reduce_next(
                &zero,
                &zero,
                &capped_validated_zero_divisor,
                &ordering,
                &context,
                ProjectiveNormalizationPolicy::AdmissionOnly,
                capped_limits,
            )
            .unwrap_err(),
        ProjectiveError::ResourceLimit {
            resource: "projective polynomial operations",
            requested: first_step_operations + 1,
            limit: first_step_operations,
        },
    );
    assert_eq!(
        capped_replay.consequence(),
        measurement_replay.consequence()
    );
    assert_eq!(
        capped_replay.work_census().polynomial_operations(),
        first_step_operations + 1,
        "failed attempted work remains charged to the shared replay budget",
    );
}

fn pseudo_reduction_differential(active: bool) {
    let limits = ProjectiveLimits::default();
    let scope = if active {
        "projective-pseudo-active"
    } else {
        "projective-pseudo-inactive"
    };
    let context = context(scope, 1);
    let ordering = ordering(&[active], limits);
    let zero = shift(&[0], limits);
    let unit = shift(&[1], limits);
    let squared = shift(&[2], limits);
    let n = context.index(0).unwrap();
    let translated_factor = if active {
        context.sub(&n, &context.one()).unwrap()
    } else {
        context.add(&n, &context.one()).unwrap()
    };
    let divisor_leader = context.mul(&n, &translated_factor).unwrap();
    let divisor = source(
        0,
        [
            (zero.clone(), context.one()),
            (unit.clone(), divisor_leader),
        ],
        &ordering,
        &context,
        limits,
    );
    let subject = source(
        1,
        [
            (zero, context.one()),
            (unit.clone(), context.one()),
            (squared.clone(), n.clone()),
        ],
        &ordering,
        &context,
        limits,
    );
    let mut budget = ProjectiveWorkBudget::default();
    let projective_subject = PrimitiveOreConsequence::try_from_rational(
        &subject,
        &ordering,
        &context,
        &mut budget,
        limits,
    )
    .unwrap();
    let projective_divisor = PrimitiveOreConsequence::try_from_rational(
        &divisor,
        &ordering,
        &context,
        &mut budget,
        limits,
    )
    .unwrap();
    let projective = projective_subject
        .try_pseudo_reduce(
            &squared,
            &unit,
            &projective_divisor,
            &ordering,
            &context,
            ProjectiveNormalizationPolicy::EveryCancellation,
            &mut budget,
            limits,
        )
        .unwrap();

    let physical_translation = ordering.try_physical_translation(&unit).unwrap();
    let effective_leader = context
        .translate(
            divisor.row().coefficient(&unit).unwrap(),
            &physical_translation,
            limits.involutive.indexed_algebra,
        )
        .unwrap();
    let multiplier = context
        .neg_with_limits(
            &context
                .div(
                    subject.row().coefficient(&squared).unwrap(),
                    &effective_leader,
                )
                .unwrap(),
            limits.involutive.indexed_algebra.exact_algebra,
        )
        .unwrap();
    let rational = subject
        .try_left_axpy(
            &multiplier,
            &unit,
            &divisor,
            &ordering,
            &context,
            limits.involutive,
        )
        .unwrap();
    assert_projectively_equal(&projective, &rational, &ordering, &context, limits);
    assert!(projective.coefficient(&squared).is_none());
    assert_eq!(projective.provenance().len(), 2);
    assert_eq!(projective.required_nonzero_guards().len(), 1);
    assert!(projective.work_census().gcd_calls() >= 2);
    assert!(projective.work_census().exact_divisions() >= 2);
}

#[test]
fn gcd_scaled_pseudo_reduction_matches_rational_axpy_in_both_sector_signs() {
    pseudo_reduction_differential(true);
    pseudo_reduction_differential(false);
}

#[test]
fn mixed_active_inactive_translation_matches_exact_rational_axpy() {
    let limits = ProjectiveLimits::default();
    let context = context("projective-pseudo-mixed-sector", 2);
    let ordering = ordering(&[true, false], limits);
    let zero = shift(&[0, 0], limits);
    let divisor_leader_shift = shift(&[1, 1], limits);
    let operator = shift(&[1, 2], limits);
    let target = shift(&[2, 3], limits);
    let n0 = context.index(0).unwrap();
    let n1 = context.index(1).unwrap();
    let n0_plus_one = context.add(&n0, &context.one()).unwrap();
    let n0_plus_two = context.add(&n0, &context.integer(2)).unwrap();
    let n1_plus_four = context.add(&n1, &context.integer(4)).unwrap();
    let subject_target = context.mul(&n0_plus_one, &n0_plus_two).unwrap();
    let divisor_leader = context.mul(&n0, &n1_plus_four).unwrap();
    let divisor = source(
        0,
        [
            (zero.clone(), context.one()),
            (divisor_leader_shift.clone(), divisor_leader),
        ],
        &ordering,
        &context,
        limits,
    );
    let subject = source(
        1,
        [
            (zero.clone(), context.one()),
            (target.clone(), subject_target),
        ],
        &ordering,
        &context,
        limits,
    );
    let mut budget = ProjectiveWorkBudget::default();
    let projective_subject = PrimitiveOreConsequence::try_from_rational(
        &subject,
        &ordering,
        &context,
        &mut budget,
        limits,
    )
    .unwrap();
    let projective_divisor = PrimitiveOreConsequence::try_from_rational(
        &divisor,
        &ordering,
        &context,
        &mut budget,
        limits,
    )
    .unwrap();
    let projective = projective_subject
        .try_pseudo_reduce(
            &target,
            &operator,
            &projective_divisor,
            &ordering,
            &context,
            ProjectiveNormalizationPolicy::WhenAugmentedEntriesDoNotExceed {
                max_entries: usize::MAX,
            },
            &mut budget,
            limits,
        )
        .unwrap();

    let translation = ordering.try_physical_translation(&operator).unwrap();
    assert_eq!(translation, [1, -2]);
    let translated_leader = context
        .translate(
            divisor.row().coefficient(&divisor_leader_shift).unwrap(),
            &translation,
            limits.involutive.indexed_algebra,
        )
        .unwrap();
    let multiplier = context
        .neg_with_limits(
            &context
                .div(
                    subject.row().coefficient(&target).unwrap(),
                    &translated_leader,
                )
                .unwrap(),
            limits.involutive.indexed_algebra.exact_algebra,
        )
        .unwrap();
    let rational = subject
        .try_left_axpy(
            &multiplier,
            &operator,
            &divisor,
            &ordering,
            &context,
            limits.involutive,
        )
        .unwrap();
    assert_projectively_equal(&projective, &rational, &ordering, &context, limits);
    assert!(projective.coefficient(&target).is_none());
    assert!(projective.is_fully_normalized());
}

#[test]
fn pseudo_reduction_translates_divisor_localization_and_retains_its_leader_guard() {
    let limits = ProjectiveLimits::default();
    let context = context("projective-translated-guards", 1);
    let ordering = ordering(&[true], limits);
    let zero = shift(&[0], limits);
    let unit = shift(&[1], limits);
    let squared = shift(&[2], limits);
    let n = context.index(0).unwrap();
    let denominator = context.add(&n, &context.one()).unwrap();
    let divisor = source(
        0,
        [
            (
                zero.clone(),
                context.div(&context.one(), &denominator).unwrap(),
            ),
            (unit.clone(), n.clone()),
        ],
        &ordering,
        &context,
        limits,
    );
    let subject = source(
        1,
        [(zero, context.one()), (squared.clone(), context.one())],
        &ordering,
        &context,
        limits,
    );
    let mut budget = ProjectiveWorkBudget::default();
    let projective_divisor = PrimitiveOreConsequence::try_from_rational(
        &divisor,
        &ordering,
        &context,
        &mut budget,
        limits,
    )
    .unwrap();
    let projective = PrimitiveOreConsequence::try_from_rational(
        &subject,
        &ordering,
        &context,
        &mut budget,
        limits,
    )
    .unwrap()
    .try_pseudo_reduce(
        &squared,
        &unit,
        &projective_divisor,
        &ordering,
        &context,
        ProjectiveNormalizationPolicy::EveryCancellation,
        &mut budget,
        limits,
    )
    .unwrap();

    let translation = ordering.try_physical_translation(&unit).unwrap();
    let translated_denominator = context
        .translate(
            &denominator,
            &translation,
            limits.involutive.indexed_algebra,
        )
        .unwrap();
    let cleared_divisor_leader = context.mul(&n, &denominator).unwrap();
    let translated_leader = context
        .translate(
            &cleared_divisor_leader,
            &translation,
            limits.involutive.indexed_algebra,
        )
        .unwrap();
    let expected = [
        polynomial(&context, &translated_denominator),
        polynomial(&context, &translated_leader),
    ];
    assert_eq!(projective.required_nonzero_guards().len(), expected.len());
    for guard in expected {
        assert!(
            projective
                .required_nonzero_guards()
                .iter()
                .any(|candidate| candidate.as_ref() == &guard),
        );
    }
}

fn complete_ordinary(generator: &ParametricIbpGenerator<'_>) -> CompletedIbpSourceRows {
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    prepared.complete(rows).unwrap()
}

fn real_family_differential(
    family: &crate::family::IntegralFamily,
    mask: &[bool],
    operator_values: &[u64],
) {
    let projective_limits = ProjectiveLimits::default();
    let mut projective_budget = ProjectiveWorkBudget::default();
    let lift_limits = OrdinaryChartLiftLimits {
        involutive: projective_limits.involutive,
        ..OrdinaryChartLiftLimits::default()
    };
    let generator = ParametricIbpGenerator::try_new(family).unwrap();
    let completed = complete_ordinary(&generator);
    let ordering = OreOrderingAdapter::try_new_for_completed(
        OrderingPolicy::default(),
        Mask::try_new(mask.iter().copied()).unwrap(),
        &completed,
        projective_limits.involutive,
    )
    .unwrap();
    let lifted = try_lift_completed_ordinary_sources(
        &completed,
        &ordering,
        generator.context(),
        lift_limits,
    )
    .unwrap();
    let replay_sources: Vec<_> = lifted
        .sources()
        .iter()
        .map(|source| (source.consequence(), source.left_shift()))
        .collect();
    for source in lifted.sources() {
        let projective = PrimitiveOreConsequence::try_from_rational(
            source.consequence(),
            &ordering,
            generator.context(),
            &mut projective_budget,
            projective_limits,
        )
        .unwrap();
        assert_projectively_equal(
            &projective,
            source.consequence(),
            &ordering,
            generator.context(),
            projective_limits,
        );
        assert_projective_source_replay(
            &projective,
            &replay_sources,
            &ordering,
            generator.context(),
            projective_limits,
        );
    }

    let divisor = lifted.sources()[0].consequence();
    let operator = shift(operator_values, projective_limits);
    let zero = ForwardShift::try_zero(mask.len(), projective_limits.involutive).unwrap();
    let subject =
        OreConsequence::try_zero(&ordering, generator.context(), projective_limits.involutive)
            .unwrap()
            .try_left_axpy(
                &generator.context().one(),
                &zero,
                divisor,
                &ordering,
                generator.context(),
                projective_limits.involutive,
            )
            .unwrap()
            .try_left_axpy(
                &generator.context().one(),
                &operator,
                divisor,
                &ordering,
                generator.context(),
                projective_limits.involutive,
            )
            .unwrap();
    let (divisor_leader, _) = divisor.row().try_leading_term(&ordering).unwrap().unwrap();
    let target = operator
        .try_checked_add(divisor_leader.shift(), projective_limits.involutive)
        .unwrap();
    let physical_translation = ordering.try_physical_translation(&operator).unwrap();
    let effective_leader = generator
        .context()
        .translate(
            divisor_leader.coefficient(),
            &physical_translation,
            projective_limits.involutive.indexed_algebra,
        )
        .unwrap();
    let subject_coefficient = subject.row().coefficient(&target).unwrap();
    let rational_multiplier = generator
        .context()
        .neg_with_limits(
            &generator
                .context()
                .div(subject_coefficient, &effective_leader)
                .unwrap(),
            projective_limits.involutive.indexed_algebra.exact_algebra,
        )
        .unwrap();
    let rational = subject
        .try_left_axpy(
            &rational_multiplier,
            &operator,
            divisor,
            &ordering,
            generator.context(),
            projective_limits.involutive,
        )
        .unwrap();
    let projective_subject = PrimitiveOreConsequence::try_from_rational(
        &OreConsequence::try_zero(&ordering, generator.context(), projective_limits.involutive)
            .unwrap()
            .try_left_axpy(
                &generator.context().one(),
                &zero,
                divisor,
                &ordering,
                generator.context(),
                projective_limits.involutive,
            )
            .unwrap()
            .try_left_axpy(
                &generator.context().one(),
                &operator,
                divisor,
                &ordering,
                generator.context(),
                projective_limits.involutive,
            )
            .unwrap(),
        &ordering,
        generator.context(),
        &mut projective_budget,
        projective_limits,
    )
    .unwrap();
    let projective_divisor = PrimitiveOreConsequence::try_from_rational(
        divisor,
        &ordering,
        generator.context(),
        &mut projective_budget,
        projective_limits,
    )
    .unwrap();
    let projective = projective_subject
        .try_pseudo_reduce(
            &target,
            &operator,
            &projective_divisor,
            &ordering,
            generator.context(),
            ProjectiveNormalizationPolicy::EveryCancellation,
            &mut projective_budget,
            projective_limits,
        )
        .unwrap();
    assert_projectively_equal(
        &projective,
        &rational,
        &ordering,
        generator.context(),
        projective_limits,
    );
    assert_projective_source_replay(
        &projective,
        &replay_sources,
        &ordering,
        generator.context(),
        projective_limits,
    );
}

#[test]
fn generated_one_and_two_loop_sources_roundtrip_and_reduce_projectively() {
    let one_loop = derive_one_loop_unit_mass_tadpole().unwrap();
    real_family_differential(one_loop.family(), &[true], &[3]);

    let two_loop = derive_two_loop_unit_mass_sunset().unwrap();
    real_family_differential(two_loop.family(), &[true, false, true], &[0, 3, 0]);
}
