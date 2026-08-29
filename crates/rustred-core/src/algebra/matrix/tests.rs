//! Matrix-boundary regression tests kept with the private algebra subsystem.

use super::field::{CheckedCoefficientField, abort_checked_field, call_native};
use super::*;
use crate::algebra::{Coefficient, CoefficientContext, ExactAlgebraError, ExactAlgebraLimits};
use std::panic::resume_unwind;
use symbolica::domains::Ring;

fn identity(context: &CoefficientContext, size: usize) -> Vec<Vec<Coefficient>> {
    (0..size)
        .map(|row| {
            (0..size)
                .map(|column| {
                    if row == column {
                        context.one()
                    } else {
                        context.zero()
                    }
                })
                .collect()
        })
        .collect()
}

#[test]
fn rectangular_symbolic_rational_rank_is_native_and_authenticated() {
    let context = CoefficientContext::new(["a", "b", "x"]);
    let matrix = vec![
        vec![
            context.zero(),
            context.coefficient_fixture("a/x"),
            context.zero(),
            context.one(),
        ],
        vec![
            context.zero(),
            context.zero(),
            context.parameter("b").unwrap(),
            context.coefficient_fixture("1/x"),
        ],
        vec![
            context.zero(),
            context.coefficient_fixture("2*a/x"),
            context.zero(),
            context.integer(2),
        ],
    ];
    let (rank, stats) = rank_of_coefficient_matrix(
        &context,
        &matrix,
        SymbolicaCoefficientMatrixLimits::default(),
    )
    .unwrap();

    assert_eq!(rank, 2);
    assert_eq!(stats.rank_calls(), 1);
    assert_eq!(stats.exact_operations(), 7);
    assert_eq!(stats.input_entries, 12);
    assert_eq!(stats.output_entries, 0);
    assert_eq!(stats.authenticated_entries, 12);
    assert!(stats.input_retained_bytes() > 0);
    assert!(stats.output_retained_bytes() > 0);
    assert_eq!(stats.non_matrix_trait_calls, 0);
}

#[test]
fn native_rank_handles_row_swaps_zero_leading_columns_and_deficiency() {
    let context = CoefficientContext::new(["x"]);
    let row_swap = vec![
        vec![context.zero(), context.zero(), context.one()],
        vec![
            context.parameter("x").unwrap(),
            context.zero(),
            context.zero(),
        ],
        vec![context.zero(), context.one(), context.zero()],
    ];
    let (rank, stats) = rank_of_coefficient_matrix(
        &context,
        &row_swap,
        SymbolicaCoefficientMatrixLimits::default(),
    )
    .unwrap();
    assert_eq!(rank, 3);
    assert_eq!(stats.exact_operations(), 3);

    let deficient = vec![
        vec![
            context.zero(),
            context.one(),
            context.integer(2),
            context.integer(3),
        ],
        vec![
            context.zero(),
            context.integer(2),
            context.integer(4),
            context.integer(6),
        ],
        vec![
            context.zero(),
            context.zero(),
            context.zero(),
            context.zero(),
        ],
    ];
    let (rank, _) = rank_of_coefficient_matrix(
        &context,
        &deficient,
        SymbolicaCoefficientMatrixLimits::default(),
    )
    .unwrap();
    assert_eq!(rank, 1);

    let zero = vec![vec![context.zero(); 4]; 2];
    let (rank, stats) = rank_of_coefficient_matrix(
        &context,
        &zero,
        SymbolicaCoefficientMatrixLimits {
            max_exact_operations: 0,
            ..SymbolicaCoefficientMatrixLimits::default()
        },
    )
    .unwrap();
    assert_eq!(rank, 0);
    assert_eq!(stats.exact_operations(), 0);
}

#[test]
fn native_rank_covers_rectangular_shapes_one_through_six() {
    let context = CoefficientContext::new(["x"]);
    for rows in 1..=6 {
        for columns in 1..=6 {
            let expected = rows.min(columns);
            let mut matrix = vec![vec![context.zero(); columns]; rows];
            for (diagonal, row) in matrix.iter_mut().enumerate().take(expected) {
                row[diagonal] = context.one();
            }
            let (rank, stats) = rank_of_coefficient_matrix(
                &context,
                &matrix,
                SymbolicaCoefficientMatrixLimits::default(),
            )
            .unwrap();
            assert_eq!(rank, expected, "shape {rows}x{columns}");
            assert_eq!(stats.exact_operations(), expected);
            assert_eq!(stats.rank_calls(), 1);
        }
    }
}

#[test]
fn native_rank_preserves_gmp_coefficients_and_rejects_foreign_maps() {
    let context = CoefficientContext::new(["x"]);
    let large = context.coefficient_fixture("340282366920938463463374607431768211507");
    let matrix = vec![
        vec![large, context.zero()],
        vec![context.zero(), context.one()],
    ];
    let (rank, _) = rank_of_coefficient_matrix(
        &context,
        &matrix,
        SymbolicaCoefficientMatrixLimits::default(),
    )
    .unwrap();
    assert_eq!(rank, 2);

    let foreign = CoefficientContext::new(["y"]);
    assert!(matches!(
        rank_of_coefficient_matrix(
            &context,
            &[vec![foreign.one()]],
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
fn native_rank_limits_cover_entries_live_bytes_and_exact_operations() {
    let context = CoefficientContext::new(["x"]);
    let matrix = vec![vec![context.one()]];
    let (_, baseline) = rank_of_coefficient_matrix(
        &context,
        &matrix,
        SymbolicaCoefficientMatrixLimits::default(),
    )
    .unwrap();
    let input_bytes = baseline.input_retained_bytes();
    let output_bytes = baseline.output_retained_bytes();
    assert!(input_bytes > 0);
    assert!(output_bytes > 0);

    let (rank, exact) = rank_of_coefficient_matrix(
        &context,
        &matrix,
        SymbolicaCoefficientMatrixLimits {
            max_single_matrix_entries: 1,
            max_live_matrix_entries: 1,
            max_exact_operations: 1,
            max_input_retained_bytes: input_bytes,
            max_output_retained_bytes: output_bytes,
            ..SymbolicaCoefficientMatrixLimits::default()
        },
    )
    .unwrap();
    assert_eq!(rank, 1);
    assert_eq!(exact.admitted_single_matrix_entries(), 1);
    assert_eq!(exact.admitted_peak_live_entries(), 1);
    assert_eq!(exact.admitted_exact_operations(), 1);
    assert_eq!(exact.exact_operations(), 1);

    for (limits, resource) in [
        (
            SymbolicaCoefficientMatrixLimits {
                max_single_matrix_entries: 0,
                ..SymbolicaCoefficientMatrixLimits::default()
            },
            "single Symbolica matrix entries",
        ),
        (
            SymbolicaCoefficientMatrixLimits {
                max_live_matrix_entries: 0,
                ..SymbolicaCoefficientMatrixLimits::default()
            },
            "live Symbolica matrix entries",
        ),
        (
            SymbolicaCoefficientMatrixLimits {
                max_input_retained_bytes: input_bytes - 1,
                ..SymbolicaCoefficientMatrixLimits::default()
            },
            "coefficient matrix input retained bytes",
        ),
        (
            SymbolicaCoefficientMatrixLimits {
                max_output_retained_bytes: output_bytes - 1,
                ..SymbolicaCoefficientMatrixLimits::default()
            },
            "coefficient matrix output retained bytes",
        ),
    ] {
        assert!(matches!(
            rank_of_coefficient_matrix(&context, &matrix, limits),
            Err(SymbolicaCoefficientMatrixError::ResourceLimit {
                resource: actual_resource,
                ..
            }) if actual_resource == resource
        ));
    }

    assert!(matches!(
        rank_of_coefficient_matrix(
            &context,
            &matrix,
            SymbolicaCoefficientMatrixLimits {
                max_exact_operations: 0,
                ..SymbolicaCoefficientMatrixLimits::default()
            },
        ),
        Err(SymbolicaCoefficientMatrixError::ExactAlgebra(
            ExactAlgebraError::ResourceLimit {
                resource: "Symbolica coefficient matrix exact operations",
                requested: 1,
                limit: 0,
            }
        ))
    ));
}

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

#[test]
fn checked_field_abort_recovers_the_exact_error_without_formatting_payloads() {
    let expected = ExactAlgebraError::ResourceLimit {
        resource: "sentinel-test",
        requested: 2,
        limit: 1,
    };
    let error = call_native("sentinel transport", || {
        abort_checked_field(expected.clone())
    })
    .unwrap_err();
    assert_eq!(
        error,
        SymbolicaCoefficientMatrixError::ExactAlgebra(expected)
    );
    assert!(!error.to_string().contains("matrix payload"));
}

#[test]
fn unexpected_native_panic_is_redacted() {
    struct UnexpectedPanic;
    let error = call_native("panic test", || {
        resume_unwind(Box::new(UnexpectedPanic));
    })
    .unwrap_err();
    assert_eq!(
        error,
        SymbolicaCoefficientMatrixError::NativePanic {
            operation: "panic test"
        }
    );
    assert!(!error.to_string().contains("UnexpectedPanic"));
}

#[test]
fn exact_algebra_failure_crosses_native_boundary_as_typed_error() {
    let context = CoefficientContext::new(["x"]);
    let matrix = vec![
        vec![context.parameter("x").unwrap(), context.zero()],
        vec![context.zero(), context.one()],
    ];
    let error = invert_and_verify_coefficient_matrix(
        &context,
        &matrix,
        SymbolicaCoefficientMatrixLimits {
            exact_algebra: ExactAlgebraLimits {
                max_term_operations: 0,
                ..ExactAlgebraLimits::default()
            },
            ..SymbolicaCoefficientMatrixLimits::default()
        },
    )
    .unwrap_err();
    assert!(matches!(
        error,
        SymbolicaCoefficientMatrixError::ExactAlgebra(ExactAlgebraError::ResourceLimit {
            limit: 0,
            ..
        })
    ));
}

#[test]
fn checked_field_abort_is_contained_across_parallel_native_sessions() {
    let workers = (0..8)
        .map(|worker| {
            std::thread::spawn(move || {
                let parameter = format!("x{worker}");
                let context = CoefficientContext::new([parameter.as_str()]);
                let matrix = vec![
                    vec![context.parameter(&parameter).unwrap(), context.zero()],
                    vec![context.zero(), context.one()],
                ];
                let error = invert_and_verify_coefficient_matrix(
                    &context,
                    &matrix,
                    SymbolicaCoefficientMatrixLimits {
                        exact_algebra: ExactAlgebraLimits {
                            max_term_operations: 0,
                            ..ExactAlgebraLimits::default()
                        },
                        ..SymbolicaCoefficientMatrixLimits::default()
                    },
                )
                .unwrap_err();
                assert!(matches!(
                    error,
                    SymbolicaCoefficientMatrixError::ExactAlgebra(
                        ExactAlgebraError::ResourceLimit { limit: 0, .. }
                    )
                ));

                let expected = ExactAlgebraError::ResourceLimit {
                    resource: "parallel-sentinel-test",
                    requested: worker + 1,
                    limit: worker,
                };
                assert_eq!(
                    call_native("parallel sentinel transport", || {
                        abort_checked_field(expected.clone())
                    })
                    .unwrap_err(),
                    SymbolicaCoefficientMatrixError::ExactAlgebra(expected)
                );
            })
        })
        .collect::<Vec<_>>();

    for worker in workers {
        worker.join().expect("parallel native session panicked");
    }
}

#[test]
fn rectangular_product_is_symbolica_owned_and_authenticated() {
    let context = CoefficientContext::new(["x"]);
    let left = vec![
        vec![context.one(), context.integer(2), context.integer(3)],
        vec![context.integer(4), context.integer(5), context.integer(6)],
    ];
    let right = vec![
        vec![context.integer(7)],
        vec![context.integer(8)],
        vec![context.integer(9)],
    ];
    let (product, stats) = multiply_coefficient_matrices(
        &context,
        &left,
        &right,
        SymbolicaCoefficientMatrixLimits::default(),
    )
    .unwrap();
    assert_eq!(
        product,
        vec![vec![context.integer(50)], vec![context.integer(122)]]
    );
    assert_eq!(stats.exact_operations(), 12);
    assert_eq!(stats.product_calls(), 1);
}

#[test]
fn symbolic_three_matrix_product_is_native_and_exactly_bounded() {
    let context = CoefficientContext::new(["x", "y"]);
    let left = vec![
        vec![context.parameter("x").unwrap(), context.one()],
        vec![context.zero(), context.integer(2)],
    ];
    let middle = vec![
        vec![context.one(), context.zero()],
        vec![context.parameter("y").unwrap(), context.one()],
    ];
    let right = vec![
        vec![context.coefficient_fixture("1/2"), context.zero()],
        vec![context.one(), context.one()],
    ];
    let (product, stats) = multiply_three_coefficient_matrices(
        &context,
        &left,
        &middle,
        &right,
        SymbolicaCoefficientMatrixLimits::default(),
    )
    .unwrap();
    assert_eq!(
        product,
        vec![
            vec![context.coefficient_fixture("1+(x+y)/2"), context.one(),],
            vec![context.coefficient_fixture("y+2"), context.integer(2)],
        ]
    );
    assert_eq!(stats.product_calls(), 2);
    assert_eq!(stats.transpose_calls(), 0);
    assert_eq!(stats.exact_operations(), 32);
    assert_eq!(stats.admitted_exact_operations(), 32);
    assert_eq!(stats.admitted_single_matrix_entries(), 4);
    assert_eq!(stats.admitted_peak_live_entries(), 16);
    assert!(stats.input_retained_bytes() > 0);
    assert!(stats.output_retained_bytes() > 0);

    let exact = SymbolicaCoefficientMatrixLimits {
        max_single_matrix_entries: stats.admitted_single_matrix_entries(),
        max_live_matrix_entries: stats.admitted_peak_live_entries(),
        max_exact_operations: stats.admitted_exact_operations(),
        max_input_retained_bytes: stats.input_retained_bytes(),
        max_output_retained_bytes: stats.output_retained_bytes(),
        ..SymbolicaCoefficientMatrixLimits::default()
    };
    let (_, replayed_stats) =
        multiply_three_coefficient_matrices(&context, &left, &middle, &right, exact).unwrap();
    assert_eq!(replayed_stats, stats);

    for (limits, resource) in [
        (
            SymbolicaCoefficientMatrixLimits {
                max_single_matrix_entries: stats.admitted_single_matrix_entries() - 1,
                ..SymbolicaCoefficientMatrixLimits::default()
            },
            "single Symbolica matrix entries",
        ),
        (
            SymbolicaCoefficientMatrixLimits {
                max_live_matrix_entries: stats.admitted_peak_live_entries() - 1,
                ..SymbolicaCoefficientMatrixLimits::default()
            },
            "live Symbolica matrix entries",
        ),
        (
            SymbolicaCoefficientMatrixLimits {
                max_exact_operations: stats.admitted_exact_operations() - 1,
                ..SymbolicaCoefficientMatrixLimits::default()
            },
            "Symbolica coefficient matrix exact operations",
        ),
        (
            SymbolicaCoefficientMatrixLimits {
                max_input_retained_bytes: stats.input_retained_bytes() - 1,
                ..SymbolicaCoefficientMatrixLimits::default()
            },
            "coefficient matrix input retained bytes",
        ),
        (
            SymbolicaCoefficientMatrixLimits {
                max_output_retained_bytes: stats.output_retained_bytes() - 1,
                ..SymbolicaCoefficientMatrixLimits::default()
            },
            "coefficient matrix output retained bytes",
        ),
    ] {
        assert!(matches!(
            multiply_three_coefficient_matrices(&context, &left, &middle, &right, limits),
            Err(SymbolicaCoefficientMatrixError::ResourceLimit {
                resource: actual,
                ..
            }) if actual == resource
        ));
    }

    assert!(matches!(
        multiply_three_coefficient_matrices(
            &context,
            &left,
            &[vec![context.one(), context.zero(), context.zero()]],
            &right,
            SymbolicaCoefficientMatrixLimits::default(),
        ),
        Err(SymbolicaCoefficientMatrixError::ShapeMismatch { .. })
    ));
}

#[test]
fn symbolic_congruence_uses_native_transpose_and_censuses_its_output() {
    let context = CoefficientContext::new(["x", "y"]);
    let transform = vec![
        vec![context.one(), context.parameter("x").unwrap()],
        vec![context.zero(), context.one()],
    ];
    let middle = vec![
        vec![context.integer(2), context.parameter("y").unwrap()],
        vec![context.parameter("y").unwrap(), context.integer(3)],
    ];
    let (product, stats) = congruence_of_coefficient_matrix(
        &context,
        &transform,
        &middle,
        SymbolicaCoefficientMatrixLimits::default(),
    )
    .unwrap();
    assert_eq!(
        product,
        vec![
            vec![
                context.coefficient_fixture("2+2*x*y+3*x^2"),
                context.coefficient_fixture("y+3*x"),
            ],
            vec![context.coefficient_fixture("y+3*x"), context.integer(3)],
        ]
    );
    assert_eq!(stats.product_calls(), 2);
    assert_eq!(stats.transpose_calls(), 1);
    assert_eq!(stats.exact_operations(), 32);
    assert_eq!(stats.admitted_exact_operations(), 32);
    assert_eq!(stats.admitted_single_matrix_entries(), 4);
    assert_eq!(stats.admitted_peak_live_entries(), 16);

    let exact = SymbolicaCoefficientMatrixLimits {
        max_single_matrix_entries: stats.admitted_single_matrix_entries(),
        max_live_matrix_entries: stats.admitted_peak_live_entries(),
        max_exact_operations: stats.admitted_exact_operations(),
        max_input_retained_bytes: stats.input_retained_bytes(),
        max_output_retained_bytes: stats.output_retained_bytes(),
        ..SymbolicaCoefficientMatrixLimits::default()
    };
    let (_, replayed_stats) =
        congruence_of_coefficient_matrix(&context, &transform, &middle, exact).unwrap();
    assert_eq!(replayed_stats, stats);

    let one_below_output = SymbolicaCoefficientMatrixLimits {
        max_output_retained_bytes: stats.output_retained_bytes() - 1,
        ..SymbolicaCoefficientMatrixLimits::default()
    };
    assert!(matches!(
        congruence_of_coefficient_matrix(&context, &transform, &middle, one_below_output,),
        Err(SymbolicaCoefficientMatrixError::ResourceLimit {
            resource: "coefficient matrix output retained bytes",
            ..
        })
    ));

    assert!(matches!(
        congruence_of_coefficient_matrix(
            &context,
            &transform,
            &[vec![context.one()]],
            SymbolicaCoefficientMatrixLimits::default(),
        ),
        Err(SymbolicaCoefficientMatrixError::ShapeMismatch { .. })
    ));
}
