//! Opt-in S4 sector solve: a rule's unresolved exceptional branch is deferred
//! instead of failing the sector, and no rule is ever dispatched at a point of
//! a deferred region. The ordinary solve keeps its atomic refusal.

use crate::algebra::{CoefficientContext, CoefficientPolynomial};
use crate::solver::{
    RuleQualityFeature, RuleQualityPriority, RuleTrialLimits, SectorConfig,
    SourceDiscoveryStrategy, SourceSystem, Term,
};

use super::*;

fn source(context: &CoefficientContext, pivot: &str) -> SourceSystem<3> {
    SourceSystem::new(
        vec![vec![
            Term {
                integral: Integral::symbolic([0; 3]).unwrap(),
                coefficient: context.coefficient_fixture(pivot).numerator,
            },
            Term {
                integral: Integral::symbolic([-1, 0, 0]).unwrap(),
                coefficient: context.one().numerator,
            },
        ]],
        [0, 1, 2],
    )
    .unwrap()
}

fn value(polynomial: &CoefficientPolynomial, indices: &[usize; 3], point: &[i64; 3]) -> i128 {
    let nvars = polynomial.nvars();
    let mut total = 0_i128;
    for (term, coefficient) in polynomial.coefficients.iter().enumerate() {
        let mut product = i128::from(coefficient.to_i64().unwrap());
        for (variable, &power) in polynomial.exponents[term * nvars..(term + 1) * nvars]
            .iter()
            .enumerate()
        {
            if power != 0 {
                let axis = indices.iter().position(|&index| index == variable).unwrap();
                product *= i128::from(point[axis]).pow(u32::from(power));
            }
        }
        total += product;
    }
    total
}

fn vanishes(conjunction: &[CoefficientPolynomial], indices: &[usize; 3], point: &[i64; 3]) -> bool {
    conjunction
        .iter()
        .all(|polynomial| value(polynomial, indices, point) == 0)
}

fn on_case(case: &Case<3>, indices: &[usize; 3], point: &[i64; 3]) -> bool {
    case.fixed()
        .iter()
        .zip(point)
        .all(|(value, &coordinate)| value.is_none_or(|value| i64::from(value) == coordinate))
        && case
            .affine()
            .is_none_or(|affine| vanishes(affine.equations(), indices, point))
}

#[derive(Debug, PartialEq, Eq)]
enum Dispatch {
    Deferred(usize),
    Rule(usize),
    Uncovered,
}

/// FORMAT.md dispatch (first rule whose case contains the point and none of
/// whose exception branches holds there) under the S4 contract: a point of a
/// deferred region is never dispatched to any rule.
fn dispatch(
    solved: &DeferredSectorSolution<3>,
    indices: &[usize; 3],
    point: &[i64; 3],
) -> Dispatch {
    for (index, deferred) in solved.deferred.iter().enumerate() {
        if vanishes(
            &deferred.region.predicates(indices).unwrap(),
            indices,
            point,
        ) {
            return Dispatch::Deferred(index);
        }
    }
    rule_domain(&solved.solution.rules, indices, point).map_or(Dispatch::Uncovered, Dispatch::Rule)
}

fn rule_domain(rules: &[SectorRule<3>], indices: &[usize; 3], point: &[i64; 3]) -> Option<usize> {
    rules.iter().position(|rule| {
        on_case(&rule.candidate.case, indices, point)
            && !rule
                .exceptions
                .branches
                .iter()
                .any(|branch| vanishes(branch, indices, point))
    })
}

#[test]
fn an_unresolved_exceptional_factor_is_deferred_and_never_dispatched() {
    let context = CoefficientContext::new(["a", "b", "c", "d"]);
    let indices = [0, 1, 2];
    // The pivot vanishes for every d exactly where a^2+b^2 = 5 and c = 1: a
    // circle beyond the exact engine (atomic refusal before S4). Its two
    // positive points have no other rule, so the solve finishes with one.
    let sources = source(&context, "d*(a^2+b^2-5)+(c-1)");
    let solver = SectorSolver::new(&sources, [true; 3], SectorConfig::default()).unwrap();
    match solver.solve_sector(Default::default()).unwrap_err() {
        SectorSolveError::Intersection(error) => {
            assert_eq!(error.failure, CaseIntersectionFailure::UnsupportedGeometry);
        }
        other => panic!("expected the atomic refusal, got {other:?}"),
    }
    let solved = solver.solve_sector_deferring(Default::default()).unwrap();
    for (index, rule) in solved.solution.rules.iter().enumerate() {
        println!(
            "S4_SECTOR rule {index}: case {:?} target {} exceptions {:?}",
            rule.candidate.case, rule.candidate.target, rule.exceptions.branches
        );
    }
    println!("S4_SECTOR deferred {:?}", solved.deferred);
    assert_eq!(solved.deferred.len(), 1);
    let deferred = &solved.deferred[0];
    let parent = &solved.solution.rules[deferred.rule];
    assert_eq!(deferred.rule, 0);
    assert_eq!(deferred.region.parent, parent.candidate.case);
    assert_eq!(
        deferred.region.reason,
        CaseIntersectionFailure::UnsupportedGeometry
    );
    assert_eq!(
        deferred.region.conjunction.as_ref(),
        [context.coefficient_fixture("a^2+b^2-5").numerator].as_slice()
    );
    let branch = &parent.exceptions.branches[deferred.branch];
    assert_eq!(solved.solution.rules.len(), 1);
    // Every region point of the box is exceptional for its own rule (so no
    // rule's own guard admits it) and the region is never dispatched.
    let mut region_points = Vec::new();
    for a in 1..=6 {
        for b in 1..=6 {
            for c in 1..=3 {
                let point = [a, b, c];
                if vanishes(
                    &deferred.region.predicates(&indices).unwrap(),
                    &indices,
                    &point,
                ) {
                    assert!(vanishes(branch, &indices, &point), "{point:?}");
                    assert_eq!(dispatch(&solved, &indices, &point), Dispatch::Deferred(0));
                    assert_eq!(rule_domain(&solved.solution.rules, &indices, &point), None);
                    region_points.push(point);
                } else {
                    assert_eq!(dispatch(&solved, &indices, &point), Dispatch::Rule(0));
                }
            }
        }
    }
    assert_eq!(region_points, vec![[1, 2, 1], [2, 1, 1]]);
    assert_eq!(dispatch(&solved, &indices, &[3, 3, 3]), Dispatch::Rule(0));
}

#[test]
fn deferral_without_stuck_branches_is_the_ordinary_solution() {
    let system = SourceSystem::<1>::from_family(&crate::solver::tests::tadpole()).unwrap();
    let solver = SectorSolver::new(&system, [true], SectorConfig::default()).unwrap();
    let ordinary = solver.solve_sector(Default::default()).unwrap();
    let solved = solver.solve_sector_deferring(Default::default()).unwrap();
    assert!(solved.deferred.is_empty());
    let shape = |solution: &SectorSolution<1>| {
        solution
            .rules
            .iter()
            .map(|rule| {
                format!(
                    "{:?} {} {:?} {:?}",
                    rule.candidate.case,
                    rule.candidate.target,
                    rule.candidate
                        .rhs
                        .iter()
                        .map(|term| format!("{} {}", term.integral, term.coefficient))
                        .collect::<Vec<_>>(),
                    rule.exceptions.branches
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(shape(&solved.solution), shape(&ordinary));
    assert_eq!(solved.solution.finite_residuals, ordinary.finite_residuals);
    let context = CoefficientContext::new(["a", "b", "c", "d"]);
    // Its exceptional branch a+b+5 = 0 = c-1 is empty in the sector.
    let sources = source(&context, "d*(a+b+5)+(c-1)");
    let solver = SectorSolver::new(&sources, [true; 3], SectorConfig::default()).unwrap();
    let ordinary = solver.solve_sector(Default::default()).unwrap();
    let solved = solver.solve_sector_deferring(Default::default()).unwrap();
    assert!(solved.deferred.is_empty());
    let shape = |solution: &SectorSolution<3>| {
        solution
            .rules
            .iter()
            .map(|rule| format!("{:?} {:?}", rule.candidate.case, rule.exceptions.branches))
            .collect::<Vec<_>>()
    };
    assert_eq!(shape(&solved.solution), shape(&ordinary));
}

#[test]
fn deferral_requires_first_valid_rule_selection() {
    let context = CoefficientContext::new(["a", "b", "c", "d"]);
    let sources = source(&context, "d*(a^2+b^2-5)+(c-1)");
    let solver = SectorSolver {
        system: &sources,
        basis: sources.rows().to_vec(),
        order: crate::solver::IntegralOrder::new([true; 3], [false; 3]),
        config: SectorConfig {
            rule_selection: RuleSelectionPolicy::BoundedPortfolio {
                alternatives: vec![SourceDiscoveryStrategy::InputOrder],
                limits: RuleTrialLimits {
                    max_depth: 1,
                    max_rows: 32,
                    max_exact_trace_rows: 32,
                    max_exact_trace_terms: 128,
                },
                quality: vec![RuleQualityPriority {
                    feature: RuleQualityFeature::RhsTerms,
                    descending: false,
                }],
                trigger: RulePortfolioTrigger::Always,
            },
            ..Default::default()
        },
    };
    match solver.solve_sector_deferring(Default::default()) {
        Err(SectorSolveError::Search {
            source: SolverError::InvalidInput(message),
            ..
        }) => assert!(message.contains("FirstValid"), "{message}"),
        other => panic!("expected the portfolio refusal, got {other:?}"),
    }
}
