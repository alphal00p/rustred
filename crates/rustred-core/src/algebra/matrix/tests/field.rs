use super::super::field::{CheckedCoefficientField, call_native};
use super::super::*;
use crate::algebra::{CoefficientContext, ExactAlgebraError, ExactAlgebraLimits};
use symbolica::domains::Ring;

#[test]
fn fallible_inverse_and_division_follow_the_symbolica_ring_contract() {
    let context = CoefficientContext::new(["x"]);
    let field = CheckedCoefficientField::new(
        &context,
        SymbolicaCoefficientMatrixLimits::default(),
        1,
        1,
        2,
    );
    let zero = context.zero();
    let one = context.one();
    let x = context.parameter("x").unwrap();

    assert_eq!(field.try_inv(&zero), None);
    assert_eq!(field.try_div(&one, &zero), None);
    assert_eq!(field.try_inv(&x), Some(context.coefficient_fixture("1/x")));
    assert_eq!(
        field.try_div(&one, &x),
        Some(context.coefficient_fixture("1/x"))
    );
    assert_eq!(field.state.borrow().stats.exact_operations(), 2);
}

#[test]
fn native_field_power_preflights_u64_exponents_before_symbolica() {
    let context = CoefficientContext::new(["x"]);
    let field = CheckedCoefficientField::new(
        &context,
        SymbolicaCoefficientMatrixLimits::default(),
        1,
        1,
        2,
    );
    let base = context.coefficient_fixture("x^40000");
    assert!(matches!(
        call_native("coefficient power preflight", || field.pow(&base, 2)),
        Err(SymbolicaCoefficientMatrixError::ExactAlgebra(
            ExactAlgebraError::ExponentLimit {
                operation: crate::algebra::ExactAlgebraOperation::Power,
                variable: 0,
                requested: 80_000,
                limit: crate::algebra::SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
            }
        ))
    ));
}

#[test]
fn native_field_power_caps_constant_exponents_and_linear_work() {
    let context = CoefficientContext::new(Vec::<String>::new());
    let strict = SymbolicaCoefficientMatrixLimits {
        max_exact_operations: 2,
        ..SymbolicaCoefficientMatrixLimits::default()
    };
    let field = CheckedCoefficientField::new(&context, strict, 1, 1, 2);
    let value = call_native("constant coefficient power", || {
        field.pow(&context.one(), 2)
    })
    .unwrap();
    assert_eq!(value, context.one());
    let stats = field.state.borrow().stats;
    assert_eq!(stats.exact_operations(), 2);
    assert_eq!(stats.multiplications, 2);
    assert_eq!(stats.admitted_power_exponent, 2);
    assert_eq!(stats.admitted_power_term_operations, 1);
    assert_eq!(stats.admitted_power_numerator_terms, 1);
    assert_eq!(stats.admitted_power_denominator_terms, 1);
    assert_eq!(stats.output_power_numerator_terms, 1);
    assert_eq!(stats.output_power_denominator_terms, 1);
    assert_eq!(stats.authenticated_entries, 1);
    assert_eq!(stats.output_entries, 1);
    assert!(stats.output_retained_bytes() > 0);

    let over_budget = CheckedCoefficientField::new(&context, strict, 1, 1, 2);
    assert!(matches!(
        call_native("constant coefficient power work cap", || {
            over_budget.pow(&context.one(), 3)
        }),
        Err(SymbolicaCoefficientMatrixError::ExactAlgebra(
            ExactAlgebraError::ResourceLimit {
                resource: "Symbolica coefficient matrix exact operations",
                requested: 3,
                limit: 2,
            }
        ))
    ));

    let native_cap = CheckedCoefficientField::new(
        &context,
        SymbolicaCoefficientMatrixLimits {
            max_exact_operations: usize::MAX,
            ..SymbolicaCoefficientMatrixLimits::default()
        },
        1,
        1,
        0,
    );
    assert!(matches!(
        call_native("constant coefficient native power cap", || {
            native_cap.pow(&context.one(), u64::from(u32::MAX) + 1)
        }),
        Err(SymbolicaCoefficientMatrixError::NativePowerExponentLimit {
            requested,
            limit: u32::MAX,
        }) if requested == u64::from(u32::MAX) + 1
    ));
}

#[test]
fn native_field_power_enforces_conservative_term_work_before_symbolica() {
    let context = CoefficientContext::new(["x", "y"]);
    let base = context.coefficient_fixture("x+y");
    let exact = ExactAlgebraLimits {
        max_term_operations: 36,
        ..ExactAlgebraLimits::default()
    };
    let field = CheckedCoefficientField::new(
        &context,
        SymbolicaCoefficientMatrixLimits {
            exact_algebra: exact,
            ..SymbolicaCoefficientMatrixLimits::default()
        },
        1,
        1,
        3,
    );
    let value = call_native("bounded coefficient power", || field.pow(&base, 3)).unwrap();
    assert_eq!(value, context.coefficient_fixture("(x+y)^3"));
    let stats = field.state.borrow().stats;
    assert_eq!(stats.exact_operations(), 3);
    assert_eq!(stats.multiplications, 3);
    assert_eq!(stats.admitted_power_exponent, 3);
    assert_eq!(stats.admitted_power_term_operations, 36);
    assert_eq!(stats.admitted_power_numerator_terms, 16);
    assert_eq!(stats.admitted_power_denominator_terms, 1);
    assert_eq!(stats.output_power_numerator_terms, 4);
    assert_eq!(stats.output_power_denominator_terms, 1);

    let rejected = CheckedCoefficientField::new(
        &context,
        SymbolicaCoefficientMatrixLimits {
            exact_algebra: ExactAlgebraLimits {
                max_term_operations: 35,
                ..ExactAlgebraLimits::default()
            },
            ..SymbolicaCoefficientMatrixLimits::default()
        },
        1,
        1,
        3,
    );
    assert!(matches!(
        call_native("coefficient power term-work cap", || rejected.pow(&base, 3)),
        Err(SymbolicaCoefficientMatrixError::ExactAlgebra(
            ExactAlgebraError::ResourceLimit {
                resource: "exact coefficient power numerator term operations",
                requested: 36,
                limit: 35,
            }
        ))
    ));
    let rejected_stats = rejected.state.borrow().stats;
    assert_eq!(rejected_stats.exact_operations(), 0);
    assert_eq!(rejected_stats.output_retained_bytes(), 0);
}

#[test]
fn native_field_power_enforces_output_retained_bytes() {
    let context = CoefficientContext::new(Vec::<String>::new());
    let base = context.integer(2);
    let baseline = CheckedCoefficientField::new(
        &context,
        SymbolicaCoefficientMatrixLimits::default(),
        1,
        1,
        64,
    );
    let value = call_native("coefficient power byte baseline", || {
        baseline.pow(&base, 64)
    })
    .unwrap();
    assert_eq!(value, context.coefficient_fixture("18446744073709551616"));
    let stats = baseline.state.borrow().stats;
    let output_bytes = stats.output_retained_bytes();
    assert!(output_bytes > 0);
    assert_eq!(stats.admitted_power_exponent, 64);
    assert_eq!(stats.admitted_power_term_operations, 1);
    assert_eq!(stats.admitted_power_numerator_terms, 1);
    assert_eq!(stats.admitted_power_denominator_terms, 1);
    assert_eq!(stats.output_power_numerator_terms, 1);
    assert_eq!(stats.output_power_denominator_terms, 1);
    assert_eq!(stats.authenticated_entries, 1);
    assert_eq!(stats.output_entries, 1);

    let rejected = CheckedCoefficientField::new(
        &context,
        SymbolicaCoefficientMatrixLimits {
            max_output_retained_bytes: output_bytes - 1,
            ..SymbolicaCoefficientMatrixLimits::default()
        },
        1,
        1,
        64,
    );
    assert!(matches!(
        call_native("coefficient power retained-byte cap", || rejected.pow(&base, 64)),
        Err(SymbolicaCoefficientMatrixError::ResourceLimit {
            resource: "coefficient matrix output retained bytes",
            requested,
            limit,
        }) if requested == output_bytes && limit == output_bytes - 1
    ));
    let rejected_stats = rejected.state.borrow().stats;
    assert_eq!(rejected_stats.exact_operations(), 64);
    assert_eq!(rejected_stats.output_retained_bytes(), 0);
    assert_eq!(rejected_stats.output_entries, 0);
}

#[test]
fn native_field_power_handles_zero_and_rational_coefficients() {
    let context = CoefficientContext::new(["x", "y"]);

    let zero_to_zero = CheckedCoefficientField::new(
        &context,
        SymbolicaCoefficientMatrixLimits::default(),
        1,
        1,
        0,
    );
    assert_eq!(
        call_native("zero coefficient power zero", || {
            zero_to_zero.pow(&context.zero(), 0)
        })
        .unwrap(),
        context.one(),
    );
    assert_eq!(zero_to_zero.state.borrow().stats.exact_operations(), 0);

    let zero_to_positive = CheckedCoefficientField::new(
        &context,
        SymbolicaCoefficientMatrixLimits::default(),
        1,
        1,
        3,
    );
    assert!(
        call_native("zero coefficient positive power", || {
            zero_to_positive.pow(&context.zero(), 3)
        })
        .unwrap()
        .is_zero()
    );
    assert_eq!(zero_to_positive.state.borrow().stats.exact_operations(), 3);

    let rational = context.coefficient_fixture("(x+y)/(1-x)");
    let rational_field = CheckedCoefficientField::new(
        &context,
        SymbolicaCoefficientMatrixLimits::default(),
        1,
        1,
        3,
    );
    assert_eq!(
        call_native("rational coefficient power", || {
            rational_field.pow(&rational, 3)
        })
        .unwrap(),
        context.coefficient_fixture("(x+y)^3/(1-x)^3"),
    );
    let stats = rational_field.state.borrow().stats;
    assert_eq!(stats.admitted_power_numerator_terms, 16);
    assert_eq!(stats.admitted_power_denominator_terms, 4);
    assert_eq!(stats.output_power_numerator_terms, 4);
    assert_eq!(stats.output_power_denominator_terms, 4);
}
