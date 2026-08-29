use super::super::*;
use super::support::identity;
use crate::algebra::{CoefficientContext, ExactAlgebraError, ExactAlgebraLimits};

fn check_identity_size(size: usize) {
    let context = CoefficientContext::new(["x"]);
    let matrix = identity(&context, size);
    let result = invert_and_verify_coefficient_matrix(
        &context,
        &matrix,
        SymbolicaCoefficientMatrixLimits::default(),
    )
    .unwrap();
    assert_eq!(result.inverse, matrix);
    assert_eq!(result.determinant, context.one());
    for coefficient in result.inverse.iter().flatten() {
        context
            .validate_with_limits(coefficient, ExactAlgebraLimits::default())
            .unwrap();
    }
    assert_eq!(result.stats.determinant_calls(), 1);
    assert_eq!(result.stats.inverse_calls, 1);
    assert_eq!(result.stats.product_calls(), 2);
    assert_eq!(result.stats.non_matrix_trait_calls, 0);
}

macro_rules! identity_test {
    ($name:ident, $size:literal) => {
        #[test]
        fn $name() {
            check_identity_size($size);
        }
    };
}

identity_test!(map_aware_identity_size_1, 1);
identity_test!(map_aware_identity_size_2, 2);
identity_test!(map_aware_identity_size_3, 3);
identity_test!(map_aware_identity_size_4, 4);
identity_test!(map_aware_identity_size_5, 5);
identity_test!(map_aware_identity_size_6, 6);

#[test]
fn symbolic_nonsymmetric_inverse_and_determinant_are_exact() {
    let context = CoefficientContext::new(["a", "b", "s"]);
    let matrix = vec![
        vec![context.coefficient_fixture("a/s"), context.one()],
        vec![context.parameter("b").unwrap(), context.integer(2)],
    ];
    let result = invert_and_verify_coefficient_matrix(
        &context,
        &matrix,
        SymbolicaCoefficientMatrixLimits::default(),
    )
    .unwrap();
    assert_eq!(
        result.determinant,
        context.coefficient_fixture("(2*a-b*s)/s")
    );
    verify_coefficient_matrix_inverse(
        &context,
        &matrix,
        &result.inverse,
        SymbolicaCoefficientMatrixLimits::default(),
    )
    .unwrap();
}

#[test]
fn independent_determinant_guard_rejects_general_inverse_singularity() {
    let context = CoefficientContext::new(["x"]);
    let mut matrix = identity(&context, 4);
    matrix[3] = matrix[2].clone();
    assert!(matches!(
        invert_and_verify_coefficient_matrix(
            &context,
            &matrix,
            SymbolicaCoefficientMatrixLimits::default(),
        ),
        Err(SymbolicaCoefficientMatrixError::Singular)
    ));
}

#[test]
fn foreign_map_is_rejected_before_native_algebra() {
    let context = CoefficientContext::new(["x"]);
    let foreign = CoefficientContext::new(["y"]);
    let matrix = vec![vec![foreign.one()]];
    assert!(matches!(
        invert_and_verify_coefficient_matrix(
            &context,
            &matrix,
            SymbolicaCoefficientMatrixLimits::default(),
        ),
        Err(SymbolicaCoefficientMatrixError::InvalidCoefficient {
            row: 0,
            column: 0,
            error: ExactAlgebraError::VariableMapMismatch { .. },
        })
    ));
}

#[test]
fn matrix_resource_limits_are_preflighted_exactly() {
    let context = CoefficientContext::new(["x"]);
    let matrix = identity(&context, 2);
    let exact = invert_and_verify_coefficient_matrix(
        &context,
        &matrix,
        SymbolicaCoefficientMatrixLimits {
            max_single_matrix_entries: 8,
            max_live_matrix_entries: 16,
            max_exact_operations: 45,
            ..SymbolicaCoefficientMatrixLimits::default()
        },
    )
    .unwrap();
    assert_eq!(exact.stats.admitted_single_matrix_entries(), 8);
    assert_eq!(exact.stats.admitted_peak_live_entries(), 16);
    assert_eq!(exact.stats.admitted_exact_operations(), 45);

    for (limits, resource, requested, limit) in [
        (
            SymbolicaCoefficientMatrixLimits {
                max_single_matrix_entries: 7,
                ..SymbolicaCoefficientMatrixLimits::default()
            },
            "single Symbolica matrix entries",
            8,
            7,
        ),
        (
            SymbolicaCoefficientMatrixLimits {
                max_live_matrix_entries: 15,
                ..SymbolicaCoefficientMatrixLimits::default()
            },
            "live Symbolica matrix entries",
            16,
            15,
        ),
        (
            SymbolicaCoefficientMatrixLimits {
                max_exact_operations: 44,
                ..SymbolicaCoefficientMatrixLimits::default()
            },
            "Symbolica coefficient matrix exact operations",
            45,
            44,
        ),
    ] {
        assert!(matches!(
            invert_and_verify_coefficient_matrix(&context, &matrix, limits),
            Err(SymbolicaCoefficientMatrixError::ResourceLimit {
                resource: actual_resource,
                requested: actual_requested,
                limit: actual_limit,
            }) if actual_resource == resource && actual_requested == requested && actual_limit == limit
        ));
    }
}

#[test]
fn exact_operation_envelopes_pin_every_native_inverse_branch() {
    let context = CoefficientContext::new(["x"]);
    // Size one and sizes four and above use Symbolica's augmented-matrix
    // inverse, while two and three use its specialized formulas.
    for (size, expected_operations) in [(1, 8), (2, 45), (3, 164), (4, 476)] {
        let matrix = identity(&context, size);
        let exact = invert_and_verify_coefficient_matrix(
            &context,
            &matrix,
            SymbolicaCoefficientMatrixLimits {
                max_exact_operations: expected_operations,
                ..SymbolicaCoefficientMatrixLimits::default()
            },
        )
        .unwrap();
        assert_eq!(exact.stats.admitted_exact_operations(), expected_operations);

        assert!(matches!(
            invert_and_verify_coefficient_matrix(
                &context,
                &matrix,
                SymbolicaCoefficientMatrixLimits {
                    max_exact_operations: expected_operations - 1,
                    ..SymbolicaCoefficientMatrixLimits::default()
                },
            ),
            Err(SymbolicaCoefficientMatrixError::ResourceLimit {
                resource: "Symbolica coefficient matrix exact operations",
                requested,
                limit,
            }) if requested == expected_operations && limit == expected_operations - 1
        ));
    }
}

#[test]
fn retained_byte_limits_have_exact_and_one_below_boundaries() {
    let context = CoefficientContext::new(["x"]);
    let matrix = identity(&context, 2);
    let baseline = invert_and_verify_coefficient_matrix(
        &context,
        &matrix,
        SymbolicaCoefficientMatrixLimits::default(),
    )
    .unwrap();
    let input_bytes = baseline.stats.input_retained_bytes();
    let output_bytes = baseline.stats.output_retained_bytes();
    assert!(input_bytes > 0);
    assert!(output_bytes > 0);

    invert_and_verify_coefficient_matrix(
        &context,
        &matrix,
        SymbolicaCoefficientMatrixLimits {
            max_input_retained_bytes: input_bytes,
            max_output_retained_bytes: output_bytes,
            ..SymbolicaCoefficientMatrixLimits::default()
        },
    )
    .unwrap();

    assert!(matches!(
        invert_and_verify_coefficient_matrix(
            &context,
            &matrix,
            SymbolicaCoefficientMatrixLimits {
                max_input_retained_bytes: input_bytes - 1,
                ..SymbolicaCoefficientMatrixLimits::default()
            },
        ),
        Err(SymbolicaCoefficientMatrixError::ResourceLimit {
            resource: "coefficient matrix input retained bytes",
            requested,
            limit,
        }) if requested == input_bytes && limit == input_bytes - 1
    ));
    assert!(matches!(
        invert_and_verify_coefficient_matrix(
            &context,
            &matrix,
            SymbolicaCoefficientMatrixLimits {
                max_output_retained_bytes: output_bytes - 1,
                ..SymbolicaCoefficientMatrixLimits::default()
            },
        ),
        Err(SymbolicaCoefficientMatrixError::ResourceLimit {
            resource: "coefficient matrix output retained bytes",
            limit,
            ..
        }) if limit == output_bytes - 1
    ));
}
