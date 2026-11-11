//! Independent soundness review of the opt-in S4 sector solve (reviewer's
//! tests, not the implementer's). The dispatcher here is written from
//! FORMAT.md plus the S4 contract only, and membership is evaluated exactly at
//! integer points without engine geometry:
//! - every point of a deferred region is dispatched to NO rule, and lies on the
//!   exceptional locus of the rule that produced it;
//! - every other point of the box is dispatched to a rule whose domain (case,
//!   no exception branch holding) contains it: nothing is left uncovered;
//! - every (rule, RHS term) that sends a covered box point into a region is
//!   listed by the exit scan (brute force against the conservative scan);
//! - planted points ON the infinite Pell region, NEXT TO it (+-1) and OFF it.

use crate::algebra::{CoefficientContext, CoefficientPolynomial};
use crate::solver::{SectorConfig, SourceSystem, Term, deferred_exit_scan};

use super::*;

fn eval(polynomial: &CoefficientPolynomial, indices: &[usize; 3], point: &[i64; 3]) -> i128 {
    let nvars = polynomial.nvars();
    let mut total = 0_i128;
    for (term, coefficient) in polynomial.coefficients.iter().enumerate() {
        let mut product = i128::from(coefficient.to_i64().unwrap());
        for (variable, &power) in polynomial.exponents[term * nvars..(term + 1) * nvars]
            .iter()
            .enumerate()
        {
            if power != 0 {
                let axis = indices
                    .iter()
                    .position(|&index| index == variable)
                    .expect("index variables only");
                product *= i128::from(point[axis]).pow(u32::from(power));
            }
        }
        total += product;
    }
    total
}

fn zero(conjunction: &[CoefficientPolynomial], indices: &[usize; 3], point: &[i64; 3]) -> bool {
    conjunction.iter().all(|p| eval(p, indices, point) == 0)
}

fn on_case(case: &Case<3>, indices: &[usize; 3], point: &[i64; 3]) -> bool {
    case.fixed()
        .iter()
        .zip(point)
        .all(|(value, &x)| value.is_none_or(|value| i64::from(value) == x))
        && case
            .affine()
            .is_none_or(|affine| zero(affine.equations(), indices, point))
}

fn in_sector(sector: &[bool; 3], point: &[i64; 3]) -> bool {
    point
        .iter()
        .zip(sector)
        .all(|(&x, &active)| (x >= 1) == active)
}

fn rule_domain(rule: &SectorRule<3>, indices: &[usize; 3], point: &[i64; 3]) -> bool {
    on_case(&rule.candidate.case, indices, point)
        && !rule
            .exceptions
            .branches
            .iter()
            .any(|branch| zero(branch, indices, point))
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum Dispatch {
    Deferred(usize),
    Rule(usize),
    Residual,
    Uncovered,
}

/// FORMAT.md dispatch ("first rule of its chart, in index order, whose domain
/// contains it") preceded by the S4 rule: a point of any deferred region is
/// reduced by no rule. Finite residuals are listed fixed points (masters).
fn dispatch(
    solved: &DeferredSectorSolution<3>,
    predicates: &[Vec<CoefficientPolynomial>],
    indices: &[usize; 3],
    point: &[i64; 3],
) -> Dispatch {
    if let Some(region) = predicates.iter().position(|p| zero(p, indices, point)) {
        return Dispatch::Deferred(region);
    }
    if let Some(rule) = solved
        .solution
        .rules
        .iter()
        .position(|rule| rule_domain(rule, indices, point))
    {
        return Dispatch::Rule(rule);
    }
    if solved.solution.finite_residuals.iter().any(|residual| {
        residual
            .powers()
            .iter()
            .zip(point)
            .all(|(power, &x)| !power.is_symbolic() && i64::from(power.value()) == x)
    }) {
        return Dispatch::Residual;
    }
    Dispatch::Uncovered
}

fn key_at(term: &Integral<3>, point: &[i64; 3]) -> [i64; 3] {
    std::array::from_fn(|axis| {
        let power = term.powers()[axis];
        if power.is_symbolic() {
            point[axis] + i64::from(power.value())
        } else {
            i64::from(power.value())
        }
    })
}

/// One source row `pivot * I(n) + I(n0-1, n1, n2)`: the rule's only exception
/// loci are the factors of `pivot` that vanish for every d.
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

/// An infinite Pell region (c = 1, a^2 - 2 b^2 = 1) beside a second exception
/// branch that is a single point ON that region, (3, 2, 1) (a definite
/// quadratic the engine resolves). That point becomes a numerical case of the
/// ordinary queue; under S4 it is still a region point, so no rule may reduce
/// it. Checked exhaustively on a box holding three Pell points.
#[test]
fn review_pell_region_with_a_numerical_point_inside_is_never_dispatched() {
    let context = CoefficientContext::new(["a", "b", "c", "d"]);
    let indices = [0, 1, 2];
    let sector = [true; 3];
    let sources = source(
        &context,
        "(d*(a^2-2*b^2-1)+(c-1))*(d*(a-3)+(b-2)^2+(c-1)^2)",
    );
    let solver = SectorSolver::new(&sources, sector, SectorConfig::default()).unwrap();
    // The ordinary solve still refuses atomically.
    match solver.solve_sector(Default::default()) {
        Err(SectorSolveError::Intersection(error)) => assert!(matches!(
            error.failure,
            CaseIntersectionFailure::UnsupportedGeometry | CaseIntersectionFailure::RepeatedState
        )),
        other => panic!("expected the atomic refusal, got {other:?}"),
    }
    let solved = solver.solve_sector_deferring(Default::default()).unwrap();
    for (index, rule) in solved.solution.rules.iter().enumerate() {
        println!(
            "REVIEW_SECTOR rule {index}: case {:?} rhs {:?} exceptions {:?} parent {:?}",
            rule.candidate.case,
            rule.candidate
                .rhs
                .iter()
                .map(|t| t.integral.to_string())
                .collect::<Vec<_>>(),
            rule.exceptions.branches,
            solved.rule_parents[index]
        );
    }
    println!(
        "REVIEW_SECTOR residuals {:?} regions {:?}",
        solved
            .solution
            .finite_residuals
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        solved
            .deferred
            .iter()
            .map(|d| (
                d.rule,
                d.branch,
                d.region.chart.clone(),
                d.region.conjunction.to_vec()
            ))
            .collect::<Vec<_>>()
    );
    assert!(!solved.deferred.is_empty());
    assert_eq!(solved.rule_parents.len(), solved.solution.rules.len());
    let rules = &solved.solution.rules;
    let predicates: Vec<Vec<CoefficientPolynomial>> = solved
        .deferred
        .iter()
        .map(|d| d.region.predicates(&indices).unwrap())
        .collect();
    // Provenance of every region: its parent chain is complete, starts at the
    // chart root and every link's case contains the region chart.
    for index in 0..solved.deferred.len() {
        let (chain, complete) = solved.parent_chain(index);
        assert!(complete && !chain.is_empty());
        assert_eq!(solved.rule_parents[chain[0].rule], None);
        assert_eq!(chain.last().unwrap().rule, solved.deferred[index].rule);
        for link in &chain {
            assert!(
                rules[link.rule]
                    .candidate
                    .case
                    .contains(&solved.deferred[index].region.chart)
                    .unwrap()
            );
        }
    }
    for deferred in &solved.deferred {
        let checks = &deferred.region.checks;
        assert!(
            checks.chart_in_original_parent && checks.failure.is_none(),
            "{checks:?}"
        );
        assert_eq!(checks.original_restricts_to_zero, Some(false), "{checks:?}");
    }
    let scans: Vec<_> = predicates
        .iter()
        .map(|p| deferred_exit_scan(p, rules, &indices, &sector))
        .collect();
    let pell = |[a, b, c]: [i64; 3]| c == 1 && a * a - 2 * b * b == 1;
    let (mut region_points, mut covered, mut residual_points, mut exits_hit) = (0, 0, 0, 0);
    let mut region_points_on_a_numerical_case = 0;
    for a in 1..=110 {
        for b in 1..=80 {
            for c in 1..=3 {
                let point = [a, b, c];
                let routed = dispatch(&solved, &predicates, &indices, &point);
                // The region is exactly the Pell curve at c = 1.
                assert_eq!(
                    matches!(routed, Dispatch::Deferred(_)),
                    pell(point),
                    "{point:?}"
                );
                match routed {
                    Dispatch::Deferred(region) => {
                        region_points += 1;
                        let deferred = &solved.deferred[region];
                        let own = &rules[deferred.rule];
                        // On the producing rule's case and its exceptional branch:
                        // that rule's own guard already excludes the point.
                        assert!(on_case(&own.candidate.case, &indices, &point));
                        assert!(zero(
                            &own.exceptions.branches[deferred.branch],
                            &indices,
                            &point
                        ));
                        assert!(!rule_domain(own, &indices, &point));
                        region_points_on_a_numerical_case += usize::from(
                            rules.iter().any(|rule| {
                                rule.candidate.case.is_numerical()
                                    && on_case(&rule.candidate.case, &indices, &point)
                            }) || solved.solution.finite_residuals.iter().any(|r| {
                                r.powers()
                                    .iter()
                                    .zip(&point)
                                    .all(|(p, &x)| !p.is_symbolic() && i64::from(p.value()) == x)
                            }),
                        );
                    }
                    Dispatch::Rule(rule) => {
                        covered += 1;
                        // Exit completeness against the conservative scan.
                        for (t, term) in rules[rule].candidate.rhs.iter().enumerate() {
                            let key = key_at(&term.integral, &point);
                            if !in_sector(&sector, &key) {
                                continue;
                            }
                            for (region, p) in predicates.iter().enumerate() {
                                if zero(p, &indices, &key) {
                                    exits_hit += 1;
                                    assert!(
                                        scans[region].candidates.contains(&(rule, t)),
                                        "exit ({rule}, {t}) from {point:?} to {key:?} not listed"
                                    );
                                }
                            }
                        }
                    }
                    Dispatch::Residual => residual_points += 1,
                    Dispatch::Uncovered => panic!("{point:?} outside every region is uncovered"),
                }
            }
        }
    }
    println!(
        "REVIEW_SECTOR box: region {region_points} (on a numerical case/residual \
         {region_points_on_a_numerical_case}) covered {covered} residual {residual_points} \
         exits_hit {exits_hit} scans {scans:?}"
    );
    // (3,2,1), (17,12,1), (99,70,1).
    assert_eq!(region_points, 3);
    assert!(exits_hit >= 3);
    // Planted ON, NEXT TO and OFF.
    for on in [[3, 2, 1], [17, 12, 1], [99, 70, 1], [577, 408, 1]] {
        assert!(matches!(
            dispatch(&solved, &predicates, &indices, &on),
            Dispatch::Deferred(_)
        ));
        for step in [
            [1, 0, 0],
            [-1, 0, 0],
            [0, 1, 0],
            [0, -1, 0],
            [0, 0, 1],
            [1, 1, 0],
        ] {
            let next: [i64; 3] = std::array::from_fn(|i| on[i] + step[i]);
            if in_sector(&sector, &next) {
                assert!(
                    matches!(
                        dispatch(&solved, &predicates, &indices, &next),
                        Dispatch::Rule(_)
                    ),
                    "neighbour {next:?}"
                );
            }
        }
    }
    // OFF: Pell pairs at c != 1 and non-Pell points at c = 1 are ordinary.
    for off in [
        [3, 2, 2],
        [17, 12, 3],
        [577, 408, 2],
        [4, 2, 1],
        [7, 5, 1],
        [41, 29, 1],
    ] {
        assert!(matches!(
            dispatch(&solved, &predicates, &indices, &off),
            Dispatch::Rule(_)
        ));
    }
}
