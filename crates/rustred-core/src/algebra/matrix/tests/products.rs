use super::super::*;
use crate::algebra::CoefficientContext;

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
