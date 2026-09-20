//! Successful-path nonregression beyond the prepared-matrix interface.

use super::{CoefficientVariableOrder, IntegralOrder, SymbolicExactBackend};
use crate::algebra::CoefficientContext;
use crate::solver::{
    CoordinateCase, Integral, SearchOptions, SectorConfig, SectorSolveOptions, SectorSolver,
    SourceSystem, Term, extract_exceptions,
};

fn backend() -> SymbolicExactBackend {
    SymbolicExactBackend::SemiNumericalSourceWeights {
        max_degree: 32,
        max_probes: 20_000,
        max_attempts: 4,
        max_primes: 8,
        max_cached_images: 20_000,
        max_cached_values: 2_000_000,
        max_weight_slots: 4_096,
    }
}

#[test]
fn source_weight_pipeline_preserves_fixed_face_provenance_and_exceptions() {
    let context = CoefficientContext::new(["unused", "m", "n1", "n0"]);
    let indices = [3, 2];
    let term = |shift, coefficient| Term {
        integral: Integral::symbolic(shift).unwrap(),
        coefficient: context.coefficient_fixture(coefficient).numerator,
    };
    // Keep this explicitly supplied basis unpreconditioned to force a modular
    // discovery and materialization, rather than only exercising direct hits.
    let basis = vec![
        vec![term([1, 0], "1"), term([0, 0], "n1")],
        vec![term([1, 0], "1"), term([-1, 0], "m")],
    ];
    let sources = SourceSystem::new(basis.clone(), indices).unwrap();
    let solve = |symbolic_exact_backend, coefficient_variable_order| {
        let solver = SectorSolver {
            system: &sources,
            basis: basis.clone(),
            order: IntegralOrder::new([true; 2], [false; 2]),
            config: SectorConfig {
                symbolic_exact_backend,
                coefficient_variable_order,
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
    let expected = solve(
        SymbolicExactBackend::Sparse,
        CoefficientVariableOrder::Original,
    );
    let expected_exceptions = extract_exceptions(&expected, &indices, &[true; 2]).unwrap();
    assert!(!expected_exceptions.branches.is_empty());
    for ordering in [
        CoefficientVariableOrder::Original,
        CoefficientVariableOrder::Reverse,
        CoefficientVariableOrder::IndicesFirst,
    ] {
        let actual = solve(backend(), ordering);
        assert!(!actual.stats.direct_hit);
        assert_eq!(actual.case, expected.case);
        assert_eq!(actual.target, expected.target);
        assert_eq!(actual.sources, expected.sources);
        assert_eq!(actual.stats.discovery, expected.stats.discovery);
        assert_eq!(actual.rhs, expected.rhs);
        for term in &actual.rhs {
            assert_eq!(term.coefficient.numerator.variables(), context.variables());
            assert_eq!(
                term.coefficient.denominator.variables(),
                context.variables()
            );
        }
        assert_eq!(
            extract_exceptions(&actual, &indices, &[true; 2]).unwrap(),
            expected_exceptions
        );
    }
}

#[test]
fn source_weight_pipeline_matches_complete_small_input_family_censuses() {
    use crate::family::IntegralFamily;
    use crate::sector::{Mask, zero};

    fn compare<const N: usize>(family: &IntegralFamily) {
        let analyzer = zero::Analyzer::try_unrestricted(family).unwrap();
        let mut zero_sectors = Vec::new();
        let mut sectors = Vec::new();
        for bits in 0..1_usize.checked_shl(N as u32).unwrap() {
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
        let sources = SourceSystem::<N>::from_family(family).unwrap();
        for sector in sectors {
            let solve = |symbolic_exact_backend| {
                SectorSolver::new(
                    &sources,
                    sector,
                    SectorConfig {
                        zero_sectors: zero_sectors.clone().into(),
                        symbolic_exact_backend,
                        ..Default::default()
                    },
                )
                .unwrap()
                .solve_sector(SectorSolveOptions::default())
                .unwrap()
            };
            let expected = solve(SymbolicExactBackend::Sparse);
            let actual = solve(backend());
            assert_eq!(actual.finite_residuals, expected.finite_residuals);
            assert_eq!(actual.rules.len(), expected.rules.len());
            for (actual, expected) in actual.rules.iter().zip(&expected.rules) {
                assert_eq!(actual.candidate.case, expected.candidate.case);
                assert_eq!(actual.candidate.target, expected.candidate.target);
                assert_eq!(actual.candidate.sources, expected.candidate.sources);
                assert_eq!(actual.exceptions, expected.exceptions);
                assert_eq!(actual.candidate.rhs, expected.candidate.rhs);
                for (actual, expected) in actual.candidate.rhs.iter().zip(&expected.candidate.rhs) {
                    assert_eq!(
                        actual.coefficient.numerator.variables(),
                        expected.coefficient.numerator.variables()
                    );
                    assert_eq!(
                        actual.coefficient.denominator.variables(),
                        expected.coefficient.denominator.variables()
                    );
                }
            }
        }
    }
    // Concrete families belong in test inputs, never backend dispatch.
    compare::<1>(&crate::solver::tests::tadpole());
    compare::<3>(&crate::solver::tests::sunset());
}
