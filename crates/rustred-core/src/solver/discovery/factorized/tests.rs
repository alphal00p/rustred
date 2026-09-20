use crate::algebra::CoefficientContext;
use crate::solver::{
    CoordinateCase, SectorConfig, SectorSolveOptions, SectorSolver, Seed, SourceSystem,
};

use super::super::{
    CoefficientVariableOrder, SymbolicExactBackend, exact_materialize_using_with_observer,
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

fn lift(
    rows: &[ExactRow<1>],
    target: i16,
    backend: SymbolicExactBackend,
    policy: CoefficientVariableOrder,
    priority: &[usize],
) -> Result<(ExactRow<1>, Vec<MaterializationEvent<1>>), MaterializationError> {
    let mut events = Vec::new();
    let result = exact_materialize_using_with_observer(
        rows,
        &IntegralOrder::new([true], [false]),
        integral(target),
        backend,
        policy,
        priority,
        |event| events.push(event),
    )?;
    Ok((result, events))
}

fn compare_maps<const N: usize>(
    actual: &ExactRow<N>,
    expected: &ExactRow<N>,
    variables: &Arc<Vec<PolyVariable>>,
) {
    assert_eq!(actual, expected);
    for term in actual.iter().chain(expected) {
        assert_eq!(term.coefficient.numerator.variables(), variables);
        assert_eq!(term.coefficient.denominator.variables(), variables);
    }
}

#[test]
fn full_target_row_and_pivot_events_match_with_dependent_zero_rows_and_early_stop() {
    let context = CoefficientContext::new(["unused", "a", "b", "c"]);
    let rows = vec![
        row(&context, &[(4, "a/(b-1)"), (3, "1/(b-1)"), (1, "c/(b-1)")]),
        row(
            &context,
            &[(4, "2*a/(b-1)"), (3, "2/(b-1)"), (1, "2*c/(b-1)")],
        ),
        vec![],
        row(&context, &[(5, "0")]),
        row(
            &context,
            &[(4, "-2*a/3"), (3, "-(a+2)/3"), (2, "-c/3"), (0, "-1/3")],
        ),
        // A later pivot must not change the returned target row.
        row(&context, &[(2, "1"), (0, "1")]),
    ];
    let expected = row(&context, &[(3, "1"), (2, "c/a"), (1, "-2*c/a"), (0, "1/a")]);
    for policy in [
        CoefficientVariableOrder::Original,
        CoefficientVariableOrder::Reverse,
        CoefficientVariableOrder::IndicesFirst,
    ] {
        let sparse = lift(
            &rows,
            3,
            SymbolicExactBackend::Sparse,
            policy,
            &[2, 1, 3, 0],
        )
        .unwrap();
        let actual = lift(
            &rows,
            3,
            SymbolicExactBackend::SparseFactorized,
            policy,
            &[2, 1, 3, 0],
        )
        .unwrap();
        compare_maps(&actual.0, &expected, context.variables());
        assert_eq!(actual, sparse);
        assert_eq!(
            actual
                .1
                .iter()
                .filter(|event| matches!(event, MaterializationEvent::RowStarted { .. }))
                .count(),
            5
        );
        assert_eq!(
            actual
                .1
                .iter()
                .filter(|event| matches!(
                    event,
                    MaterializationEvent::RowFinished { pivot: None, .. }
                ))
                .count(),
            3
        );
    }
}

#[test]
fn shifted_native_sources_keep_the_registered_index_map() {
    let context = CoefficientContext::new(["unused", "d", "n"]);
    let template = |terms: &[(i16, &str)]| {
        row(&context, terms)
            .into_iter()
            .map(|term| Term {
                integral: term.integral,
                coefficient: term.coefficient.numerator,
            })
            .collect::<Vec<_>>()
    };
    let seed = Seed {
        integral: integral(2),
        shifts: [2],
    };
    let order = IntegralOrder::new([true], [false]);
    let rows = [
        template(&[(2, "n"), (1, "d"), (0, "1")]),
        template(&[(2, "n"), (1, "d+n"), (-1, "d")]),
    ]
    .iter()
    .map(|row| {
        crate::solver::instantiate_source_port(row, &seed, &[2], &[None], &order, &[], None)
            .unwrap()
    })
    .collect::<Vec<_>>();
    assert_eq!(rows[0][0].coefficient, context.coefficient_fixture("n+2"));
    let sparse = lift(
        &rows,
        3,
        SymbolicExactBackend::Sparse,
        CoefficientVariableOrder::Original,
        &[],
    )
    .unwrap();
    let factored = lift(
        &rows,
        3,
        SymbolicExactBackend::SparseFactorized,
        CoefficientVariableOrder::Reverse,
        &[],
    )
    .unwrap();
    compare_maps(&factored.0, &sparse.0, context.variables());
    assert_eq!(factored.1, sparse.1);
}

#[test]
fn constants_denominator_only_variables_and_repeated_factors_restore_both_maps() {
    for parameters in [vec![], vec!["unused", "a"]] {
        let context = CoefficientContext::new(parameters);
        let rows = [row(&context, &[(2, "-2/3"), (0, "5/7")])];
        let actual = lift(
            &rows,
            2,
            SymbolicExactBackend::SparseFactorized,
            CoefficientVariableOrder::Original,
            &[],
        )
        .unwrap()
        .0;
        compare_maps(
            &actual,
            &row(&context, &[(2, "1"), (0, "-15/14")]),
            context.variables(),
        );
    }
    let context = CoefficientContext::new(["unused", "a", "b"]);
    let rows = [row(&context, &[(2, "-1/(a-1)^3"), (0, "b/(a-1)^2")])];
    let actual = lift(
        &rows,
        2,
        SymbolicExactBackend::SparseFactorized,
        CoefficientVariableOrder::Original,
        &[],
    )
    .unwrap()
    .0;
    compare_maps(
        &actual,
        &row(&context, &[(2, "1"), (0, "-b*(a-1)")]),
        context.variables(),
    );
    for text in ["0", "1", "-2/3", "(a^2-1)/(a-1)", "1/(a-1)^3"] {
        let source = context.coefficient_fixture(text);
        let factored = NativeCoefficient::from_num_den(
            source.numerator.clone(),
            vec![(source.denominator.clone(), 1)],
            &Z,
            true,
        );
        let actual = ordinary(&factored, context.variables()).unwrap();
        assert_eq!(actual, source);
        assert_eq!(actual.numerator.variables(), context.variables());
        assert_eq!(actual.denominator.variables(), context.variables());
    }
}

#[test]
fn invalid_contexts_including_constant_denominator_maps_fail_closed() {
    let context = CoefficientContext::new(["a", "b"]);
    let other = CoefficientContext::new(["b", "a"]);
    for wrong_numerator in [false, true] {
        let mut coefficient = context.one();
        if wrong_numerator {
            coefficient.numerator = other.one().numerator;
        } else {
            coefficient.denominator = other.one().denominator;
        }
        let rows = [vec![Term {
            integral: integral(1),
            coefficient,
        }]];
        assert_eq!(
            lift(
                &rows,
                1,
                SymbolicExactBackend::SparseFactorized,
                CoefficientVariableOrder::Original,
                &[]
            ),
            Err(MaterializationError::CoefficientVariableMapMismatch)
        );
    }
    let mut factored = NativeCoefficient::from_num_den(
        context.one().numerator,
        vec![(context.coefficient_fixture("a").numerator, 1)],
        &Z,
        true,
    );
    factored.denominators[0].0 = other.coefficient_fixture("a").numerator;
    assert_eq!(
        ordinary(&factored, context.variables()),
        Err(MaterializationError::CoefficientVariableMapMismatch)
    );
}

#[test]
fn malformed_denominators_missing_targets_and_native_panics_are_typed_errors() {
    let context = CoefficientContext::new(["a"]);
    for numerator in [context.zero(), context.one()] {
        let mut coefficient = numerator;
        coefficient.denominator = context.zero().numerator;
        let rows = [vec![Term {
            integral: integral(1),
            coefficient,
        }]];
        assert!(matches!(
            lift(
                &rows,
                1,
                SymbolicExactBackend::SparseFactorized,
                CoefficientVariableOrder::Original,
                &[]
            ),
            Err(MaterializationError::InvalidFactorizedCoefficient(_))
        ));
    }
    let rows = [row(&context, &[(2, "1"), (0, "a")])];
    assert_eq!(
        lift(
            &rows,
            1,
            SymbolicExactBackend::SparseFactorized,
            CoefficientVariableOrder::Original,
            &[]
        ),
        Err(MaterializationError::TargetAbsent)
    );
    assert_eq!(
        lift(
            &rows,
            0,
            SymbolicExactBackend::SparseFactorized,
            CoefficientVariableOrder::Original,
            &[]
        ),
        Err(MaterializationError::TargetNotPivot)
    );
    let unordered = [row(&context, &[(0, "1"), (2, "1")])];
    assert_eq!(
        lift(
            &unordered,
            2,
            SymbolicExactBackend::SparseFactorized,
            CoefficientVariableOrder::Original,
            &[]
        ),
        Err(MaterializationError::NonCanonicalRow { row: 0 })
    );
    assert_eq!(
        native::<()>("test panic", || panic!("native failure")),
        Err(MaterializationError::FactorizedNativePanic {
            operation: "test panic"
        })
    );
}

#[test]
fn native_opt_in_does_not_change_the_sparse_default_or_suppress_observer_panics() {
    assert_eq!(
        SymbolicExactBackend::default(),
        SymbolicExactBackend::Sparse
    );
    assert_eq!(
        SectorConfig::<1>::default().symbolic_exact_backend,
        SymbolicExactBackend::Sparse
    );
    let context = CoefficientContext::new(["a"]);
    let result = catch_unwind(AssertUnwindSafe(|| {
        exact_materialize_using_with_observer(
            &[row(&context, &[(1, "1")])],
            &IntegralOrder::new([true], [false]),
            integral(1),
            SymbolicExactBackend::SparseFactorized,
            CoefficientVariableOrder::Original,
            &[],
            |event| {
                if matches!(event, MaterializationEvent::RowStarted { .. }) {
                    panic!("observer panic");
                }
            },
        )
    }));
    assert!(result.is_err());
}

#[test]
fn complete_k1_and_k3_test_sector_censuses_match_ordinary_rules_guards_and_finite_leaves() {
    use crate::sector::{Mask, zero};
    fn compare<const N: usize>(family: &crate::family::IntegralFamily) {
        assert!(N <= 3);
        let analyzer = zero::Analyzer::try_unrestricted(family).unwrap();
        let mut zero_sectors = Vec::new();
        let mut sectors = Vec::new();
        for bits in 0..1_usize << N {
            let sector = std::array::from_fn(|axis| bits & (1 << axis) != 0);
            if matches!(
                analyzer.analyze(&Mask::try_new(sector).unwrap()).unwrap(),
                zero::Decision::ProvedZero(_)
            ) {
                zero_sectors.push(sector);
            } else {
                sectors.push(sector);
            }
        }
        let source = SourceSystem::<N>::from_family(family).unwrap();
        let variables = source
            .rows()
            .iter()
            .flatten()
            .next()
            .unwrap()
            .coefficient
            .variables();
        for sector in sectors {
            let solve = |backend| {
                SectorSolver::new(
                    &source,
                    sector,
                    SectorConfig {
                        zero_sectors: zero_sectors.clone().into(),
                        symbolic_exact_backend: backend,
                        ..Default::default()
                    },
                )
                .unwrap()
                .solve_sector(SectorSolveOptions::default())
                .unwrap()
            };
            let sparse = solve(SymbolicExactBackend::Sparse);
            for backend in [
                SymbolicExactBackend::SparseFactorized,
                SymbolicExactBackend::SparseTargetOnlyFactorized,
            ] {
                let factored = solve(backend);
                assert_eq!(sparse.finite_residuals, factored.finite_residuals);
                assert_eq!(sparse.rules.len(), factored.rules.len());
                for (a, b) in sparse.rules.iter().zip(&factored.rules) {
                    assert_eq!(a.candidate.case, b.candidate.case);
                    assert_eq!(a.candidate.target, b.candidate.target);
                    assert_eq!(a.candidate.sources, b.candidate.sources);
                    assert_eq!(a.candidate.stats.discovery, b.candidate.stats.discovery);
                    assert_eq!(a.exceptions, b.exceptions);
                    compare_maps(&a.candidate.rhs, &b.candidate.rhs, variables);
                }
            }
        }
    }
    compare::<1>(&crate::solver::tests::tadpole());
    compare::<3>(&crate::solver::tests::sunset());
}

#[test]
fn fixed_face_source_trace_and_exceptional_guards_match_after_modular_discovery() {
    use crate::solver::{SearchOptions, extract_exceptions};
    let context = CoefficientContext::new(["unused", "m", "n1", "n0"]);
    let indices = [3, 2];
    let term = |shift, coefficient| Term {
        integral: Integral::symbolic(shift).unwrap(),
        coefficient: context.coefficient_fixture(coefficient).numerator,
    };
    let basis = vec![
        vec![term([1, 0], "1"), term([0, 0], "n1")],
        vec![term([1, 0], "1"), term([-1, 0], "m")],
    ];
    let source = SourceSystem::new(basis.clone(), indices).unwrap();
    let solve = |backend| {
        let solver = SectorSolver {
            system: &source,
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
                SearchOptions {
                    max_depth: Some(0),
                    ..Default::default()
                },
            )
            .unwrap()
    };
    let sparse = solve(SymbolicExactBackend::Sparse);
    let guards = extract_exceptions(&sparse, &indices, &[true; 2]).unwrap();
    assert!(!guards.branches.is_empty());
    for backend in [
        SymbolicExactBackend::SparseFactorized,
        SymbolicExactBackend::SparseTargetOnlyFactorized,
    ] {
        let factored = solve(backend);
        assert!(!sparse.stats.direct_hit && !factored.stats.direct_hit);
        assert_eq!(sparse.target, factored.target);
        assert_eq!(sparse.case, factored.case);
        assert_eq!(sparse.sources, factored.sources);
        assert_eq!(sparse.stats.discovery, factored.stats.discovery);
        compare_maps(&sparse.rhs, &factored.rhs, context.variables());
        assert_eq!(
            guards,
            extract_exceptions(&factored, &indices, &[true; 2]).unwrap()
        );
    }
}
