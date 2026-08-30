use symbolica::prelude::Integer;

use crate::algebra::indexed::base_coefficients::{
    IntegerZeroSetResolution, UnivariateIntegerZeroSet,
};
use crate::algebra::{
    CoefficientContext, IndexedAlgebraError, IndexedAlgebraLimits, IndexedCoefficientContext,
    IndexedGuardLimits,
};

#[test]
fn base_coefficient_system_is_exact_and_deterministically_ordered() {
    let base = CoefficientContext::new(["d", "x"]);
    let context = IndexedCoefficientContext::try_new(&base, "base-coefficients", 2).unwrap();
    let d = context.lift(&base.parameter("d").unwrap()).unwrap();
    let x = context.lift(&base.parameter("x").unwrap()).unwrap();
    let n0 = context.index(0).unwrap();
    let n1 = context.index(1).unwrap();
    let one = context.one();

    // d^2 + d*(n0-1) + x*n1.  The coefficient of d^2 is the literal
    // constant one, so no integer assignment can annihilate the full guard.
    let d_squared = context.mul(&d, &d).unwrap();
    let n0_minus_one = context.sub(&n0, &one).unwrap();
    let d_linear = context.mul(&d, &n0_minus_one).unwrap();
    let x_linear = context.mul(&x, &n1).unwrap();
    let guard = context
        .add(&context.add(&d_squared, &d_linear).unwrap(), &x_linear)
        .unwrap();
    let guard = context
        .numerator_condition_with_limits(&guard, Default::default())
        .unwrap();
    let system = context
        .base_coefficient_system(&guard, IndexedAlgebraLimits::default(), Default::default())
        .unwrap();

    assert_eq!(system.equations().len(), 3);
    assert_eq!(
        system
            .equations()
            .iter()
            .map(|equation| equation.base_monomial())
            .collect::<Vec<_>>(),
        [vec![0, 1], vec![1, 0], vec![2, 0]]
    );
    assert_eq!(
        system
            .equations()
            .iter()
            .map(|equation| equation.index_polynomial().to_expression().to_string())
            .collect::<Vec<_>>(),
        ["n1", "-1+n0", "1"]
    );
    assert!(system.has_nonzero_constant_equation());
}

#[test]
fn simultaneous_system_retains_genuine_and_only_collective_exceptions() {
    let base = CoefficientContext::new(["d"]);
    let context = IndexedCoefficientContext::try_new(&base, "exception-system", 1).unwrap();
    let d = context.lift(&base.parameter("d").unwrap()).unwrap();
    let n = context.index(0).unwrap();
    let one = context.one();

    for (guard, expected, literal_certificate) in [
        (
            context.mul(&d, &n).unwrap(),
            vec![(vec![1], "n0".to_owned())],
            false,
        ),
        (
            context
                .add(
                    &context.mul(&d, &n).unwrap(),
                    &context.sub(&n, &one).unwrap(),
                )
                .unwrap(),
            vec![(vec![0], "-1+n0".to_owned()), (vec![1], "n0".to_owned())],
            false,
        ),
    ] {
        let guard = context
            .numerator_condition_with_limits(&guard, Default::default())
            .unwrap();
        let system = context
            .base_coefficient_system(&guard, Default::default(), Default::default())
            .unwrap();
        assert_eq!(
            system
                .equations()
                .iter()
                .map(|equation| {
                    (
                        equation.base_monomial().to_vec(),
                        equation.index_polynomial().to_expression().to_string(),
                    )
                })
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(system.has_nonzero_constant_equation(), literal_certificate);
    }
}

#[test]
fn rational_base_field_and_zero_polynomial_have_exact_systems() {
    let base = CoefficientContext::new(Vec::<String>::new());
    let context = IndexedCoefficientContext::try_new(&base, "rational-system", 1).unwrap();
    let index = context.index(0).unwrap();
    let index = context
        .numerator_condition_with_limits(&index, Default::default())
        .unwrap();
    let system = context
        .base_coefficient_system(&index, Default::default(), Default::default())
        .unwrap();
    assert_eq!(system.equations().len(), 1);
    assert_eq!(system.equations()[0].base_monomial(), &[] as &[u16]);
    assert_eq!(
        system.equations()[0]
            .index_polynomial()
            .to_expression()
            .to_string(),
        "n0"
    );

    let zero = context.zero();
    let zero = context
        .numerator_condition_with_limits(&zero, Default::default())
        .unwrap();
    let system = context
        .base_coefficient_system(&zero, Default::default(), Default::default())
        .unwrap();
    assert!(system.equations().is_empty());
    assert!(!system.has_nonzero_constant_equation());
    assert_eq!(
        context
            .univariate_integer_zero_set(&system, Default::default())
            .unwrap(),
        IntegerZeroSetResolution::IdenticallyZero
    );
}

#[test]
fn symbolica_gcd_and_factorization_resolve_exact_univariate_integer_loci() {
    let base = CoefficientContext::new(["d"]);
    let context = IndexedCoefficientContext::try_new(&base, "integer-zero-sets", 2).unwrap();
    let d = context.lift(&base.parameter("d").unwrap()).unwrap();
    let n0 = context.index(0).unwrap();
    let n1 = context.index(1).unwrap();
    let one = context.one();
    let two = context.integer(2);
    let three = context.integer(3);

    let n0_plus_two = context.add(&n0, &two).unwrap();
    let n0_plus_three = context.add(&n0, &three).unwrap();
    let integer_roots = context.mul(&n0_plus_two, &n0_plus_three).unwrap();
    let collective_empty = context
        .add(
            &context.mul(&d, &n0).unwrap(),
            &context.sub(&n0, &one).unwrap(),
        )
        .unwrap();
    let irreducible = context.add(&context.mul(&n0, &n0).unwrap(), &one).unwrap();
    let nonintegral_linear = context.sub(&context.mul(&two, &n0).unwrap(), &one).unwrap();
    let multivariate = context.add(&context.mul(&d, &n0).unwrap(), &n1).unwrap();

    let locus = |value| {
        let polynomial = context
            .numerator_condition_with_limits(&value, Default::default())
            .unwrap();
        let system = context
            .base_coefficient_system(&polynomial, Default::default(), Default::default())
            .unwrap();
        context
            .univariate_integer_zero_set(&system, Default::default())
            .unwrap()
    };

    assert_eq!(
        locus(integer_roots),
        IntegerZeroSetResolution::Exact(UnivariateIntegerZeroSet::Finite {
            index_position: 0,
            roots: vec![Integer::from(-3), Integer::from(-2)].into_boxed_slice(),
        })
    );
    assert_eq!(
        locus(collective_empty),
        IntegerZeroSetResolution::Exact(UnivariateIntegerZeroSet::Empty)
    );
    assert_eq!(
        locus(irreducible),
        IntegerZeroSetResolution::Exact(UnivariateIntegerZeroSet::Empty)
    );
    assert_eq!(
        locus(nonintegral_linear),
        IntegerZeroSetResolution::Exact(UnivariateIntegerZeroSet::Empty)
    );
    assert_eq!(
        locus(multivariate),
        IntegerZeroSetResolution::UnsupportedMultivariate
    );

    let universally_nonzero = context.add(&context.mul(&d, &d).unwrap(), &n0).unwrap();
    assert_eq!(
        locus(universally_nonzero),
        IntegerZeroSetResolution::Exact(UnivariateIntegerZeroSet::Empty)
    );
}

#[test]
fn gcd_factor_lane_handles_shared_content_repetition_and_i64_boundary_roots() {
    let base = CoefficientContext::new(["d"]);
    let context = IndexedCoefficientContext::try_new(&base, "guard-factor-corners", 1).unwrap();
    let d = context.lift(&base.parameter("d").unwrap()).unwrap();
    let n = context.index(0).unwrap();
    let one = context.one();
    let n_minus_one = context.sub(&n, &one).unwrap();
    let n_plus_two = context.add(&n, &context.integer(2)).unwrap();
    let common = context.mul(&n_minus_one, &n_plus_two).unwrap();
    let shared = context
        .add(
            &context.mul(&d, &common).unwrap(),
            &context.mul(&context.integer(2), &common).unwrap(),
        )
        .unwrap();
    let repeated_with_content = context
        .mul(
            &context.integer(-6),
            &context
                .mul(
                    &context.mul(&n_minus_one, &n_minus_one).unwrap(),
                    &n_plus_two,
                )
                .unwrap(),
        )
        .unwrap();
    let i64_boundaries = context
        .mul(
            &context.sub(&n, &context.integer(i64::MIN)).unwrap(),
            &context.sub(&n, &context.integer(i64::MAX)).unwrap(),
        )
        .unwrap();

    let locus = |value| {
        let polynomial = context
            .numerator_condition_with_limits(&value, Default::default())
            .unwrap();
        let system = context
            .base_coefficient_system(&polynomial, Default::default(), Default::default())
            .unwrap();
        context
            .univariate_integer_zero_set(&system, Default::default())
            .unwrap()
    };
    assert_eq!(
        locus(shared),
        IntegerZeroSetResolution::Exact(UnivariateIntegerZeroSet::Finite {
            index_position: 0,
            roots: vec![Integer::from(-2), Integer::from(1)].into_boxed_slice(),
        })
    );
    assert_eq!(
        locus(repeated_with_content),
        IntegerZeroSetResolution::Exact(UnivariateIntegerZeroSet::Finite {
            index_position: 0,
            roots: vec![Integer::from(-2), Integer::from(1)].into_boxed_slice(),
        })
    );
    assert_eq!(
        locus(i64_boundaries),
        IntegerZeroSetResolution::Exact(UnivariateIntegerZeroSet::Finite {
            index_position: 0,
            roots: vec![Integer::from(i64::MIN), Integer::from(i64::MAX)].into_boxed_slice(),
        })
    );
}

#[test]
fn guard_locus_admission_fails_before_unbounded_symbolica_work() {
    let base = CoefficientContext::new(["d"]);
    let context = IndexedCoefficientContext::try_new(&base, "guard-limits", 1).unwrap();
    let n = context.index(0).unwrap();
    let one = context.one();
    let quadratic = context.add(&context.mul(&n, &n).unwrap(), &one).unwrap();
    let polynomial = context
        .numerator_condition_with_limits(&quadratic, Default::default())
        .unwrap();

    let input_limited = IndexedGuardLimits {
        max_input_terms: 1,
        ..IndexedGuardLimits::default()
    };
    assert!(matches!(
        context.base_coefficient_system(&polynomial, Default::default(), input_limited),
        Err(IndexedAlgebraError::ResourceLimit {
            resource: "guard coefficient split input terms",
            requested: 2,
            limit: 1,
        })
    ));

    let system = context
        .base_coefficient_system(&polynomial, Default::default(), Default::default())
        .unwrap();
    let degree_limited = IndexedGuardLimits {
        max_univariate_degree: 1,
        ..IndexedGuardLimits::default()
    };
    assert!(matches!(
        context.univariate_integer_zero_set(&system, degree_limited),
        Err(IndexedAlgebraError::ResourceLimit {
            resource: "guard univariate degree",
            requested: 2,
            limit: 1,
        })
    ));
    let work_limited = IndexedGuardLimits {
        max_gcd_factor_work: 44,
        ..IndexedGuardLimits::default()
    };
    assert!(matches!(
        context.univariate_integer_zero_set(&system, work_limited),
        Err(IndexedAlgebraError::ResourceLimit {
            resource: "guard gcd/factor work",
            requested: 45,
            limit: 44,
        })
    ));
    let recombination_limited = IndexedGuardLimits {
        max_factor_recombination_subsets: 3,
        ..IndexedGuardLimits::default()
    };
    assert!(matches!(
        context.univariate_integer_zero_set(&system, recombination_limited),
        Err(IndexedAlgebraError::ResourceLimit {
            resource: "guard factor recombination subsets",
            requested: 4,
            limit: 3,
        })
    ));

    let foreign = IndexedCoefficientContext::try_new(&base, "guard-limits-foreign", 1).unwrap();
    assert_eq!(
        foreign.base_coefficient_system(&polynomial, Default::default(), Default::default()),
        Err(IndexedAlgebraError::WrongContext)
    );
}
