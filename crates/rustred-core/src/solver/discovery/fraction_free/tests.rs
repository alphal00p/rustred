use crate::algebra::CoefficientContext;
use crate::solver::{CoordinateCase, SectorConfig, SectorSolver, SourceSystem, extract_exceptions};

use super::super::{
    SymbolicExactBackend, exact_materialize, exact_materialize_using_with_observer,
};
use super::*;

fn integral(shift: i16) -> Integral<1> {
    Integral::symbolic([shift]).unwrap()
}

fn row(context: &CoefficientContext, terms: &[(i16, &str)]) -> ExactRow<1> {
    terms
        .iter()
        .map(|(shift, coefficient)| Term {
            integral: integral(*shift),
            coefficient: context.coefficient_fixture(coefficient),
        })
        .collect()
}

fn dense<const N: usize>(
    rows: &[ExactRow<N>],
    order: &IntegralOrder<N>,
    target: Integral<N>,
) -> Result<ExactRow<N>, MaterializationError> {
    exact_materialize_using_with_observer(
        rows,
        order,
        target,
        SymbolicExactBackend::DenseFractionFree {
            max_matrix_entries: 1000,
        },
        |_| {},
    )
}

#[test]
fn dense_native_target_matches_sparse_and_retains_the_entire_rhs() {
    let context = CoefficientContext::new(["unused0", "a", "unused1", "b", "c"]);
    let rows = vec![
        row(&context, &[(4, "a"), (3, "1"), (1, "b")]),
        row(&context, &[(4, "2*a"), (3, "a+2"), (2, "c"), (0, "1")]),
        // This later/easier row must not replace the chosen target identity.
        row(&context, &[(2, "1"), (0, "1")]),
    ];
    let order = IntegralOrder::new([true], [false]);
    let expected = row(&context, &[(3, "1"), (2, "c/a"), (1, "-2*b/a"), (0, "1/a")]);
    assert_eq!(dense(&rows, &order, integral(3)).unwrap(), expected);
    assert_eq!(
        exact_materialize(&rows, &order, integral(3)).unwrap(),
        expected
    );
    assert!(
        expected
            .iter()
            .all(|term| term.coefficient.get_variables().len() == 5)
    );
}

#[test]
fn native_rational_polynomial_frame_preserves_joint_scale_and_full_rhs() {
    let context = CoefficientContext::new(["unused0", "a", "unused1", "b", "c"]);
    // Different rational row scales test native Q arithmetic, not an accidental
    // common integer multiplier that could cancel without preserving values.
    let rows = vec![
        row(&context, &[(4, "a/2"), (3, "1/2"), (1, "b/2")]),
        row(
            &context,
            &[(4, "-2*a/3"), (3, "-(a+2)/3"), (2, "-c/3"), (0, "-1/3")],
        ),
        row(&context, &[(2, "1/5"), (0, "1/5")]),
    ];
    let order = IntegralOrder::new([true], [false]);
    let mut events = Vec::new();
    let actual = exact_materialize_using_with_observer(
        &rows,
        &order,
        integral(3),
        SymbolicExactBackend::DenseFractionFree {
            max_matrix_entries: 1000,
        },
        |event| events.push(event),
    )
    .unwrap();
    assert_eq!(
        actual,
        exact_materialize(&rows, &order, integral(3)).unwrap()
    );
    assert_eq!(
        actual,
        row(&context, &[(3, "1"), (2, "c/a"), (1, "-2*b/a"), (0, "1/a")])
    );
    assert!(
        actual
            .iter()
            .all(|term| term.coefficient.get_variables().len() == 5)
    );
    assert!(events.iter().any(|event| matches!(
        event,
        MaterializationEvent::DenseFractionFreeStarted {
            rational_coefficients: true,
            ..
        }
    )));
}

#[test]
fn malformed_zero_denominator_is_rejected_before_native_fraction_construction() {
    let context = CoefficientContext::new(["a"]);
    let mut rows = vec![row(&context, &[(3, "a"), (2, "1")])];
    rows[0][0].coefficient.denominator = context.zero().numerator;
    let order = IntegralOrder::new([true], [false]);
    assert_eq!(
        dense(&rows, &order, integral(3)),
        Err(MaterializationError::FractionFreeNonPolynomialCoefficient { row: 1, term: 1 })
    );
}

#[test]
fn dense_native_path_handles_constant_dependent_and_structural_zero_rows() {
    let context = CoefficientContext::new(["unused"]);
    let rows = vec![
        row(&context, &[(3, "1"), (1, "1"), (0, "0")]),
        row(&context, &[(3, "2"), (1, "2")]),
        row(&context, &[(3, "1"), (2, "1")]),
    ];
    let order = IntegralOrder::new([true], [false]);
    assert_eq!(
        dense(&rows, &order, integral(2)).unwrap(),
        exact_materialize(&rows, &order, integral(2)).unwrap()
    );
    let zeros = vec![row(&context, &[(2, "0")])];
    assert_eq!(
        dense(&zeros, &order, integral(2)),
        Err(MaterializationError::TargetNotPivot)
    );
    assert_eq!(
        dense::<1>(&[], &order, integral(2)),
        Err(MaterializationError::TargetAbsent)
    );
}

#[test]
fn dense_rejects_variable_denominators_and_budget_before_native_elimination() {
    let context = CoefficientContext::new(["a"]);
    let order = IntegralOrder::new([true], [false]);
    for coefficient in ["1/a", "(a+1)/(a+2)"] {
        let rows = vec![row(&context, &[(3, coefficient), (2, "1")])];
        let mut events = Vec::new();
        let result = exact_materialize_using_with_observer(
            &rows,
            &order,
            integral(3),
            SymbolicExactBackend::DenseFractionFree {
                max_matrix_entries: 2,
            },
            |event| events.push(event),
        );
        assert_eq!(
            result,
            Err(MaterializationError::FractionFreeNonPolynomialCoefficient { row: 1, term: 1 })
        );
        assert!(
            !events.iter().any(|event| matches!(
                event,
                MaterializationEvent::DenseFractionFreeStarted { .. }
            ))
        );
        assert!(exact_materialize(&rows, &order, integral(3)).is_ok());
    }
    let rows = vec![row(&context, &[(3, "a"), (2, "1")])];
    assert_eq!(
        exact_materialize_using_with_observer(
            &rows,
            &order,
            integral(3),
            SymbolicExactBackend::DenseFractionFree {
                max_matrix_entries: 1
            },
            |_| {}
        ),
        Err(MaterializationError::FractionFreeMatrixBudget {
            rows: 1,
            columns: 2,
            limit: 1
        })
    );
}

#[test]
fn dense_observer_brackets_one_native_batch_without_fictitious_row_events() {
    let context = CoefficientContext::new(["a"]);
    let rows = vec![
        row(&context, &[(3, "a"), (1, "1")]),
        row(&context, &[(3, "a"), (2, "1")]),
    ];
    let order = IntegralOrder::new([true], [false]);
    let mut events = Vec::new();
    exact_materialize_using_with_observer(
        &rows,
        &order,
        integral(2),
        SymbolicExactBackend::DenseFractionFree {
            max_matrix_entries: 6,
        },
        |event| events.push(event),
    )
    .unwrap();
    assert!(matches!(
        events.as_slice(),
        [
            MaterializationEvent::FramePrepared { .. },
            MaterializationEvent::DenseFractionFreeStarted {
                rows: 2,
                columns: 3,
                reduction_columns: 2,
                rational_coefficients: false
            },
            MaterializationEvent::DenseFractionFreeFinished { rank: 2 }
        ]
    ));
    assert_eq!(
        SectorConfig::<1>::default().symbolic_exact_backend,
        SymbolicExactBackend::Sparse
    );
}

#[test]
fn sector_policy_preserves_discovery_sources_canonicalization_and_guards() {
    let context = CoefficientContext::new(["unused", "n0", "n1"]);
    let term = |shift, coefficient| Term {
        integral: Integral::symbolic(shift).unwrap(),
        coefficient: context.coefficient_fixture(coefficient).numerator,
    };
    // A fixed first coordinate forces modular cancellation of I(2,n1).
    // The surviving free n1 coefficient tests the ordinary guard pipeline.
    let basis = vec![
        vec![term([1, 0], "1"), term([0, 0], "n1")],
        vec![term([1, 0], "1"), term([-1, 0], "1")],
    ];
    let sources = SourceSystem::new(basis.clone(), [1, 2]).unwrap();
    let solve = |backend| {
        let solver = SectorSolver {
            system: &sources,
            basis: basis.clone(),
            order: IntegralOrder::new([true; 2], [false; 2]),
            config: SectorConfig {
                symbolic_exact_backend: backend,
                ..Default::default()
            },
        };
        solver
            .solve_case(
                CoordinateCase::new([Some(1), None]).unwrap(),
                crate::solver::SearchOptions {
                    max_depth: Some(0),
                    ..Default::default()
                },
            )
            .unwrap()
    };
    let sparse = solve(SymbolicExactBackend::Sparse);
    let dense = solve(SymbolicExactBackend::DenseFractionFree {
        max_matrix_entries: 1000,
    });
    assert!(!sparse.stats.direct_hit && !dense.stats.direct_hit);
    assert_eq!(dense.target, sparse.target);
    assert_eq!(dense.rhs, sparse.rhs);
    assert_eq!(dense.sources, sparse.sources);
    assert_eq!(dense.stats.discovery, sparse.stats.discovery);
    let expected = extract_exceptions(&sparse, &[1, 2], &[true; 2]).unwrap();
    assert!(!expected.branches.is_empty());
    assert_eq!(
        extract_exceptions(&dense, &[1, 2], &[true; 2]).unwrap(),
        expected
    );
    assert!(
        dense
            .rhs
            .iter()
            .all(|term| term.coefficient.get_variables().len() == 3)
    );
}

#[test]
fn affine_half_chart_reaches_native_q_lifting_without_changing_case_or_guards() {
    use crate::solver::{AffineCase, AffineIntersection, SearchEvent, SearchOptions};
    let context = CoefficientContext::new(["unused", "n0", "n1", "n2"]);
    let indices = [1, 2, 3];
    let face = CoordinateCase::new([Some(1), None, None]).unwrap();
    let equation = context.coefficient_fixture("2*n1-n2").numerator;
    let AffineIntersection::Affine(case) =
        AffineCase::from_coordinate(&face, &[equation], &indices, &[true; 3]).unwrap()
    else {
        panic!("expected an exact rational affine chart");
    };
    assert!(!case.has_integral_chart());
    let term = |shift, coefficient| Term {
        integral: Integral::symbolic(shift).unwrap(),
        coefficient: context.coefficient_fixture(coefficient).numerator,
    };
    let basis = vec![
        vec![term([1, 0, 0], "1"), term([0, 0, 0], "n1")],
        vec![term([1, 0, 0], "1"), term([-1, 0, 0], "1")],
    ];
    let sources = SourceSystem::new(basis.clone(), indices).unwrap();
    let solve = |backend| {
        let solver = SectorSolver {
            system: &sources,
            basis: basis.clone(),
            order: IntegralOrder::new([true; 3], [false; 3]),
            config: SectorConfig {
                symbolic_exact_backend: backend,
                ..Default::default()
            },
        };
        let mut events = Vec::new();
        let rule = solver
            .solve_case_with_observer(
                case.clone(),
                SearchOptions {
                    max_depth: Some(0),
                    ..Default::default()
                },
                |event| events.push(event),
            )
            .unwrap();
        (rule, events)
    };
    let (sparse, _) = solve(SymbolicExactBackend::Sparse);
    let (dense, events) = solve(SymbolicExactBackend::DenseFractionFree {
        max_matrix_entries: 1000,
    });
    assert!(!sparse.stats.direct_hit && !dense.stats.direct_hit);
    assert_eq!(dense.target, sparse.target);
    assert_eq!(dense.case, sparse.case);
    assert_eq!(dense.rhs, sparse.rhs);
    assert_eq!(dense.sources, sparse.sources);
    assert_eq!(dense.stats.discovery, sparse.stats.discovery);
    assert_eq!(
        extract_exceptions(&dense, &indices, &[true; 3]).unwrap(),
        extract_exceptions(&sparse, &indices, &[true; 3]).unwrap()
    );
    assert!(events.iter().any(|event| matches!(
        event,
        SearchEvent::ExactProgress(MaterializationEvent::DenseFractionFreeStarted {
            rational_coefficients: true,
            ..
        })
    )));
}
