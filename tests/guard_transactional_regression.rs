//! Failure-atomicity and resource-bound regressions for parametric guards.

use rustred::{
    CoefficientContext, ExactAlgebraLimits, GuardOrigin, IndexShift, IndexShiftOperatorExpression,
    IndexShiftOperatorLimits, IndexShiftOperatorMonomial, ParametricArithmeticLimits,
    ParametricCoefficientContext, ParametricCoefficientError, ParametricRelation, ParametricRowId,
};

fn context(scope: &str) -> ParametricCoefficientContext {
    ParametricCoefficientContext::try_new(&CoefficientContext::new(Vec::<String>::new()), scope, 1)
        .unwrap()
}

fn row(label: &'static str) -> ParametricRowId {
    ParametricRowId::Derived {
        label: label.into(),
    }
}

fn strict_degree_one() -> ParametricArithmeticLimits {
    ParametricArithmeticLimits {
        exact_algebra: ExactAlgebraLimits {
            max_exponent: 1,
            ..ExactAlgebraLimits::default()
        },
        ..ParametricArithmeticLimits::default()
    }
}

#[test]
fn guarded_relation_wrong_arity_is_failure_atomic() {
    let context = context("guard-transaction-relation-arity");
    let n = context.index(0).unwrap();
    let guarded_zero = context.checked_div_guarded(&context.zero(), &n).unwrap();
    let wrong_arity = IndexShift::try_new([0, 0], 2).unwrap();
    let mut relation = ParametricRelation::new("family", row("target"), &context);
    let before = relation.clone();

    assert!(
        relation
            .add_guarded_term(&context, wrong_arity, guarded_zero)
            .is_err()
    );
    assert_eq!(relation, before);
    assert!(relation.has_identical_guard_provenance(&before));
}

#[test]
fn relation_collection_limit_is_failure_atomic() {
    let context = context("guard-transaction-relation-limit");
    let n = context.index(0).unwrap();
    let n_plus_one = context.add(&n, &context.one()).unwrap();
    let first = context.checked_div(&context.one(), &n).unwrap();
    let second = context.checked_div(&context.one(), &n_plus_one).unwrap();
    let shift = IndexShift::try_new([0], 1).unwrap();
    let mut relation = ParametricRelation::new("family", row("target"), &context);
    relation.add_term(&context, shift.clone(), first).unwrap();
    let before = relation.clone();

    assert!(
        relation
            .add_term_with_limits(&context, shift, second, strict_degree_one())
            .is_err()
    );
    assert_eq!(relation, before);
    assert!(relation.has_identical_guard_provenance(&before));
}

#[test]
fn both_scaled_relation_paths_are_failure_atomic() {
    let context = context("guard-transaction-scaled-add");
    let n = context.index(0).unwrap();
    let n_plus_one = context.add(&n, &context.one()).unwrap();
    let shift = IndexShift::try_new([0], 1).unwrap();
    let mut source = ParametricRelation::new("family", row("source"), &context);
    source.add_term(&context, shift, n.clone()).unwrap();
    source
        .add_nonzero_condition(&context, context.numerator_condition(&n_plus_one).unwrap())
        .unwrap();

    let mut ordinary_target = ParametricRelation::new("family", row("ordinary"), &context);
    let ordinary_before = ordinary_target.clone();
    assert!(
        ordinary_target
            .add_scaled_with_limits(&context, &source, &n, strict_degree_one())
            .is_err()
    );
    assert_eq!(ordinary_target, ordinary_before);
    assert!(ordinary_target.has_identical_guard_provenance(&ordinary_before));

    let guarded_factor = context.checked_div_guarded(&n, &n_plus_one).unwrap();
    let mut guarded_target = ParametricRelation::new("family", row("guarded"), &context);
    let guarded_before = guarded_target.clone();
    assert!(
        guarded_target
            .add_scaled_guarded_with_limits(&context, &source, guarded_factor, strict_degree_one(),)
            .is_err()
    );
    assert_eq!(guarded_target, guarded_before);
    assert!(guarded_target.has_identical_guard_provenance(&guarded_before));
}

#[test]
fn guarded_operator_wrong_arity_is_failure_atomic() {
    let context = context("guard-transaction-operator-arity");
    let guarded_zero = context
        .checked_div_guarded(&context.zero(), &context.index(0).unwrap())
        .unwrap();
    let monomial = IndexShiftOperatorMonomial::Shift(IndexShift::try_new([0, 0], 2).unwrap());
    let mut expression = IndexShiftOperatorExpression::new("family", row("operator"), &context);
    let before = expression.clone();

    assert!(
        expression
            .add_guarded_monomial(&context, monomial, guarded_zero)
            .is_err()
    );
    assert_eq!(expression, before);
    assert!(expression.has_identical_guard_provenance(&before));
}

#[test]
fn operator_collection_limit_is_failure_atomic() {
    let context = context("guard-transaction-operator-limit");
    let n = context.index(0).unwrap();
    let n_plus_one = context.add(&n, &context.one()).unwrap();
    let first = context.checked_div(&context.one(), &n).unwrap();
    let second = context.checked_div(&context.one(), &n_plus_one).unwrap();
    let limits = IndexShiftOperatorLimits {
        arithmetic_limits: strict_degree_one(),
        ..IndexShiftOperatorLimits::default()
    };
    let mut expression =
        IndexShiftOperatorExpression::new_with_limits("family", row("operator"), &context, limits);
    let monomial = IndexShiftOperatorMonomial::Shift(IndexShift::try_new([0], 1).unwrap());
    expression
        .add_monomial(&context, monomial.clone(), first)
        .unwrap();
    let before = expression.clone();

    assert!(expression.add_monomial(&context, monomial, second).is_err());
    assert_eq!(expression, before);
    assert!(expression.has_identical_guard_provenance(&before));
}

#[test]
fn specialization_preflights_integer_output_bits_before_pow() {
    let context = context("specialization-integer-bits");
    let n = context.index(0).unwrap();
    let mut power = context.one();
    for _ in 0..20 {
        power = context.mul(&power, &n).unwrap();
    }
    let limits = ParametricArithmeticLimits {
        max_specialization_integer_bits: 8,
        ..ParametricArithmeticLimits::default()
    };

    assert!(matches!(
        context.specialize(&power, &[2], limits),
        Err(ParametricCoefficientError::ResourceLimit {
            resource: "coefficient specialization integer bits",
            limit: 8,
            ..
        })
    ));

    // Powers of +/-1 do not grow, even for a large polynomial exponent.
    let unit_limits = ParametricArithmeticLimits {
        max_specialization_integer_bits: 1,
        ..ParametricArithmeticLimits::default()
    };
    assert!(context.specialize(&power, &[-1], unit_limits).is_ok());
}

#[test]
fn public_provenance_construction_is_explicitly_bounded() {
    let context = context("bounded-public-provenance");
    let polynomial = context
        .numerator_condition(&context.index(0).unwrap())
        .unwrap();
    let first = GuardOrigin::GuardedDivisionDivisorNumerator;
    let second = GuardOrigin::GuardedDivisionDividendDenominator;

    assert!(matches!(
        context.nonzero_condition_with_origins_and_origin_limit(
            polynomial.clone(),
            [first.clone(), second.clone()],
            ExactAlgebraLimits::default(),
            1,
        ),
        Err(ParametricCoefficientError::ResourceLimit {
            resource: "parametric guard origin inputs",
            requested: 2,
            limit: 1,
        })
    ));

    let condition = context.nonzero_condition(polynomial, first).unwrap();
    assert!(matches!(
        condition.try_with_origin(second, 1),
        Err(ParametricCoefficientError::ResourceLimit {
            resource: "parametric guard origins",
            requested: 2,
            limit: 1,
        })
    ));
}
