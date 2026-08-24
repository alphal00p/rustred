use std::panic::{AssertUnwindSafe, catch_unwind};

use rustred::{
    CoefficientContext, ParametricArithmeticLimits, ParametricCoefficient,
    ParametricCoefficientContext, ParametricCoefficientError, ParametricPolynomial,
};

fn make_context(scope: &str, index_count: usize) -> ParametricCoefficientContext {
    ParametricCoefficientContext::try_new(
        &CoefficientContext::new(Vec::<String>::new()),
        scope,
        index_count,
    )
    .unwrap()
}

fn polynomial(
    context: &ParametricCoefficientContext,
    coefficient: &ParametricCoefficient,
) -> ParametricPolynomial {
    context.numerator_condition(coefficient).unwrap()
}

fn power(
    context: &ParametricCoefficientContext,
    value: &ParametricCoefficient,
    exponent: u32,
) -> ParametricCoefficient {
    let mut result = context.one();
    let mut factor = value.clone();
    let mut remaining = exponent;
    while remaining != 0 {
        if remaining & 1 == 1 {
            result = context.mul(&result, &factor).unwrap();
        }
        remaining >>= 1;
        if remaining != 0 {
            factor = context.mul(&factor, &factor).unwrap();
        }
    }
    result
}

#[test]
fn partial_specialization_combines_and_cancels_colliding_monomials_exactly() {
    let context = make_context("partial-specialization-collisions", 2);
    let n0 = context.index(0).unwrap();
    let n1 = context.index(1).unwrap();

    // The two source monomials differ only in the power of n0.  Substituting
    // n0=2 must combine them into one exact 3*n1 monomial.
    let n0_n1 = context.mul(&n0, &n1).unwrap();
    let sum = context.add(&n0_n1, &n1).unwrap();
    let specialized = context
        .specialize_polynomial_index(
            &polynomial(&context, &sum),
            0,
            2,
            ParametricArithmeticLimits::default(),
        )
        .unwrap();
    let expected = context.mul(&context.integer(3), &n1).unwrap();
    assert_eq!(specialized, polynomial(&context, &expected));

    // Collision handling must also canonicalize an exact cancellation to the
    // authenticated zero polynomial rather than retaining a zero monomial.
    let difference = context.sub(&n0_n1, &n1).unwrap();
    let cancelled = context
        .specialize_polynomial_index(
            &polynomial(&context, &difference),
            0,
            1,
            ParametricArithmeticLimits::default(),
        )
        .unwrap();
    assert_eq!(cancelled, polynomial(&context, &context.zero()));
    assert!(context.contains_polynomial(&cancelled));
}

#[test]
fn partial_specialization_handles_i64_extrema_without_panicking() {
    let context = make_context("partial-specialization-extrema", 1);
    let n = context.index(0).unwrap();
    let n_squared = power(&context, &n, 2);
    let source = polynomial(&context, &n_squared);

    let outcome = catch_unwind(AssertUnwindSafe(|| {
        context.specialize_polynomial_index(
            &source,
            0,
            i64::MIN,
            ParametricArithmeticLimits::default(),
        )
    }));
    let specialized = outcome
        .expect("i64::MIN substitution must not panic inside Symbolica")
        .unwrap();

    let minimum = context.integer(i64::MIN);
    let expected = context.mul(&minimum, &minimum).unwrap();
    assert_eq!(specialized, polynomial(&context, &expected));

    let maximum = catch_unwind(AssertUnwindSafe(|| {
        context.specialize_polynomial_index(
            &source,
            0,
            i64::MAX,
            ParametricArithmeticLimits::default(),
        )
    }))
    .expect("i64::MAX substitution must not panic inside Symbolica")
    .unwrap();
    let expected = context
        .mul(&context.integer(i64::MAX), &context.integer(i64::MAX))
        .unwrap();
    assert_eq!(maximum, polynomial(&context, &expected));
}

#[test]
fn partial_specialization_enforces_large_integer_and_collision_bit_budgets() {
    let context = make_context("partial-specialization-integer-budget", 1);
    let n = context.index(0).unwrap();
    let source = polynomial(&context, &power(&context, &n, 2));

    // The implementation's conservative bound for 1*(-2^63)^2 is 129 bits:
    // one source-coefficient bit plus 2*64 substitution bits.
    let mut limits = ParametricArithmeticLimits::default();
    limits.max_specialization_integer_bits = 128;
    assert_eq!(
        context.specialize_polynomial_index(&source, 0, i64::MIN, limits),
        Err(ParametricCoefficientError::ResourceLimit {
            resource: "partial polynomial specialization integer bits",
            requested: 129,
            limit: 128,
        })
    );
    limits.max_specialization_integer_bits = 129;
    context
        .specialize_polynomial_index(&source, 0, i64::MIN, limits)
        .unwrap();

    // A substitution can merge input monomials.  n+1 at n=1 produces the
    // two-bit integer 2, so a one-bit output budget must fail closed even
    // though each input monomial separately has a one-bit coefficient.
    let collision = context.add(&n, &context.one()).unwrap();
    limits = ParametricArithmeticLimits::default();
    limits.max_specialization_integer_bits = 1;
    let error = context
        .specialize_polynomial_index(&polynomial(&context, &collision), 0, 1, limits)
        .expect_err("collision growth must be included in the integer-bit budget");
    match error {
        ParametricCoefficientError::ResourceLimit {
            resource,
            requested,
            limit: 1,
        } => {
            assert!(resource.contains("partial polynomial specialization"));
            assert!(requested >= 2);
        }
        other => panic!("unexpected collision-budget error: {other:?}"),
    }
}

#[test]
fn partial_specialization_rejects_limits_and_foreign_inputs_without_panicking() {
    let context = make_context("partial-specialization-preflight", 2);
    let foreign = make_context("partial-specialization-foreign", 2);
    let n0 = context.index(0).unwrap();
    let n1 = context.index(1).unwrap();
    let source_coefficient = context.add(&context.mul(&n0, &n1).unwrap(), &n1).unwrap();
    let source = polynomial(&context, &source_coefficient);

    assert_eq!(
        foreign.specialize_polynomial_index(&source, 0, 1, ParametricArithmeticLimits::default(),),
        Err(ParametricCoefficientError::WrongContext)
    );
    assert_eq!(
        context.specialize_polynomial_index(&source, 2, 1, ParametricArithmeticLimits::default(),),
        Err(ParametricCoefficientError::WrongIndexArity {
            expected: 2,
            actual: 3,
        })
    );

    let mut limits = ParametricArithmeticLimits::default();
    limits.max_source_terms = 1;
    assert!(matches!(
        catch_unwind(AssertUnwindSafe(
            || context.specialize_polynomial_index(&source, 0, 1, limits,)
        )),
        Ok(Err(ParametricCoefficientError::ResourceLimit {
            resource: "partial polynomial specialization source terms",
            requested: 2,
            limit: 1,
        }))
    ));

    limits = ParametricArithmeticLimits::default();
    limits.max_specialization_power_operations = 1;
    assert!(matches!(
        catch_unwind(AssertUnwindSafe(
            || context.specialize_polynomial_index(&source, 0, 1, limits,)
        )),
        Ok(Err(ParametricCoefficientError::ResourceLimit {
            resource: "partial polynomial specialization power operations",
            requested: 2,
            limit: 1,
        }))
    ));

    limits = ParametricArithmeticLimits::default();
    limits.max_output_terms = 0;
    assert!(matches!(
        catch_unwind(AssertUnwindSafe(
            || context.specialize_polynomial_index(&source, 0, 1, limits,)
        )),
        Ok(Err(ParametricCoefficientError::ResourceLimit {
            resource: "partial polynomial specialization output terms",
            // Preflight uses the conservative source-term upper bound before
            // Symbolica can allocate and collect the one-term result.
            requested: 2,
            limit: 0,
        }))
    ));
}
