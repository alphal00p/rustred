use super::super::*;
use crate::algebra::{CoefficientContext, ExactAlgebraError};

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
