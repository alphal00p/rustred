use crate::algebra::{CoefficientContext, ExactAlgebraError, ExactAlgebraLimits};

use super::{
    IndexedAlgebraError, IndexedAlgebraLimits, IndexedCoefficient, IndexedCoefficientContext,
    IndexedPolynomial,
};

fn specialize(
    context: &IndexedCoefficientContext,
    value: &IndexedCoefficient,
    fixed: &[(usize, i64)],
    limits: IndexedAlgebraLimits,
    sealed: bool,
) -> Result<(IndexedCoefficient, IndexedPolynomial), IndexedAlgebraError> {
    if sealed {
        context.specialize_fixed_indices_sealed(value, fixed, limits)
    } else {
        context.specialize_fixed_indices(value, fixed, limits)
    }
}

#[test]
fn empty_fixed_specialization_preserves_canonical_values_and_denominator_guards() {
    let base = CoefficientContext::new(["x"]);
    let context = IndexedCoefficientContext::try_new(&base, "fixed-identity", 2).unwrap();
    let n0 = context.index(0).unwrap();
    let n1 = context.index(1).unwrap();
    let x = context.lift(&base.parameter("x").unwrap()).unwrap();
    let rational = context
        .div(
            &context.add(&n0, &x).unwrap(),
            &context.sub(&context.one(), &n1).unwrap(),
        )
        .unwrap();
    let values = [
        context.integer(0),
        context.integer(-3),
        context.div(&context.one(), &context.integer(2)).unwrap(),
        rational,
    ];

    for value in &values {
        for sealed in [false, true] {
            let (unchanged, guard) =
                specialize(&context, value, &[], Default::default(), sealed).unwrap();
            assert_eq!(&unchanged, value);
            assert_eq!(guard.raw(), &value.raw().denominator);
            assert_eq!(guard.context, value.context);
            assert_eq!(guard.raw().variables().as_ref(), context.variables.as_ref());
        }
    }
}

#[test]
fn empty_fixed_specialization_preserves_public_and_sealed_authentication_counts() {
    let base = CoefficientContext::new(Vec::<String>::new());
    let context = IndexedCoefficientContext::try_new(&base, "fixed-identity-counts", 1).unwrap();
    let index = context.index(0).unwrap();
    let value = context
        .div(&index, &context.add(&index, &context.one()).unwrap())
        .unwrap();

    for sealed in [false, true] {
        let before = context.authentication_scan_counts();
        let (unchanged, guard) =
            specialize(&context, &value, &[], Default::default(), sealed).unwrap();
        let after = context.authentication_scan_counts();
        assert_eq!(unchanged, value);
        assert_eq!(guard.raw(), &value.raw().denominator);
        assert_eq!(
            (after.0 - before.0, after.1 - before.1),
            (usize::from(!sealed), 1),
            "identity still authenticates its result; only public ingress scans the operand"
        );
    }
}

#[test]
fn empty_fixed_specialization_keeps_tighter_resource_refusals() {
    let base = CoefficientContext::new(Vec::<String>::new());
    let context = IndexedCoefficientContext::try_new(&base, "fixed-identity-limits", 1).unwrap();
    let index = context.index(0).unwrap();
    let square = context.mul(&index, &index).unwrap();
    let exact = IndexedAlgebraLimits {
        exact_algebra: ExactAlgebraLimits {
            max_exponent: 2,
            max_polynomial_terms: 1,
            max_term_operations: 1,
        },
        max_specialization_power_operations: 0,
        max_specialization_integer_bits: 1,
    };
    let rational = context
        .div(&index, &context.add(&index, &context.one()).unwrap())
        .unwrap();

    for sealed in [false, true] {
        assert_eq!(
            specialize(&context, &square, &[], exact, sealed).unwrap().0,
            square
        );
        let results_before = context.authentication_scan_counts().1;

        let mut tighter = exact;
        tighter.max_specialization_integer_bits = 0;
        assert_eq!(
            specialize(&context, &square, &[], tighter, sealed),
            Err(IndexedAlgebraError::ResourceLimit {
                resource: "fixed-index specialization integer bits",
                requested: 1,
                limit: 0,
            })
        );

        let mut tighter = exact;
        tighter.exact_algebra.max_term_operations = 0;
        assert_eq!(
            specialize(&context, &square, &[], tighter, sealed),
            Err(IndexedAlgebraError::ResourceLimit {
                resource: "coefficient specialization normalization input term pairs",
                requested: 1,
                limit: 0,
            })
        );

        let mut tighter = exact;
        tighter.exact_algebra.max_exponent = 1;
        assert!(matches!(
            specialize(&context, &square, &[], tighter, sealed),
            Err(IndexedAlgebraError::ExactAlgebra(
                ExactAlgebraError::ExponentLimit { .. }
            ))
        ));

        let mut tighter = exact;
        tighter.exact_algebra.max_polynomial_terms = 0;
        assert!(matches!(
            specialize(&context, &square, &[], tighter, sealed),
            Err(IndexedAlgebraError::ExactAlgebra(
                ExactAlgebraError::ResourceLimit { .. }
            ))
        ));

        // Even though identity no longer performs a GCD, retain the existing
        // normalization envelope and its resource contract for rational values.
        let normalization_limit = IndexedAlgebraLimits {
            max_specialization_integer_bits: 3,
            ..Default::default()
        };
        assert_eq!(
            specialize(&context, &rational, &[], normalization_limit, sealed),
            Err(IndexedAlgebraError::ResourceLimit {
                resource: "coefficient specialization normalized integer bits",
                requested: 4,
                limit: 3,
            })
        );
        assert_eq!(context.authentication_scan_counts().1, results_before);
    }
}

#[test]
fn empty_fixed_specialization_rejects_a_foreign_context_before_authentication() {
    let base = CoefficientContext::new(["x"]);
    let owner = IndexedCoefficientContext::try_new(&base, "fixed-identity-owner", 1).unwrap();
    let foreign = IndexedCoefficientContext::try_new(&base, "fixed-identity-foreign", 1).unwrap();
    let value = owner.index(0).unwrap();
    let before = foreign.authentication_scan_counts();

    for sealed in [false, true] {
        assert_eq!(
            specialize(&foreign, &value, &[], Default::default(), sealed),
            Err(IndexedAlgebraError::WrongContext)
        );
    }
    assert_eq!(foreign.authentication_scan_counts(), before);
}

#[test]
fn nonempty_fixed_specialization_still_cancels_factors_and_retains_poles() {
    let base = CoefficientContext::new(Vec::<String>::new());
    let context = IndexedCoefficientContext::try_new(&base, "fixed-nonidentity", 2).unwrap();
    let n0 = context.index(0).unwrap();
    let n1 = context.index(1).unwrap();
    let denominator = context.add(&n0, &context.one()).unwrap();
    // These numerator/denominator polynomials are coprime until n1 is fixed.
    let value = context
        .div(&context.add(&n0, &n1).unwrap(), &denominator)
        .unwrap();

    for sealed in [false, true] {
        let (one, guard) =
            specialize(&context, &value, &[(1, 1)], Default::default(), sealed).unwrap();
        assert_eq!(one, context.one());
        assert_eq!(guard.raw(), &denominator.raw().numerator);
        assert!(
            context
                .specialize_fixed_polynomial(&guard, &[(0, -1)], Default::default())
                .unwrap()
                .is_zero()
        );
        assert_eq!(
            specialize(
                &context,
                &value,
                &[(1, 1), (0, -1)],
                Default::default(),
                sealed,
            ),
            Err(IndexedAlgebraError::ZeroDenominator)
        );
    }
}
