//! Opt-in S4 sector solve: a rule's unresolved exceptional branch is deferred
//! instead of failing the sector, and no rule is ever dispatched at a point of
//! a deferred region. The ordinary solve keeps its atomic refusal.

use crate::algebra::{CoefficientContext, CoefficientPolynomial};
use crate::solver::{
    Power, RuleQualityFeature, RuleQualityPriority, RuleTrialLimits, SearchStats, SectorConfig,
    SectorRuleParent, SourceDiscoveryStrategy, SourceSystem, Term, deferred_exit_scan,
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
    assert_eq!(solved.rule_parents, vec![None]);
    assert_eq!(
        solved.parent_chain(0),
        (vec![SectorRuleParent { rule: 0, branch: 0 }], true)
    );
    assert_eq!(solved.parent_chain(1), (Vec::new(), false));
    // Exits: the rule's only RHS term I(n0-1,n1,n2) reaches the circle from
    // (3,1,1) and (2,2,1); it is the one candidate.
    let predicates = deferred.region.predicates(&indices).unwrap();
    let scan = deferred_exit_scan(&predicates, &solved.solution.rules, &indices, &[true; 3]);
    assert_eq!(scan.pairs, solved.solution.rules[0].candidate.rhs.len());
    assert_eq!(scan.candidates.len() + scan.excluded, scan.pairs);
    println!(
        "S4_SECTOR rhs {:?} scan {scan:?}",
        solved.solution.rules[0]
            .candidate
            .rhs
            .iter()
            .map(|term| term.integral.to_string())
            .collect::<Vec<_>>()
    );
    for &(rule, term) in &scan.candidates {
        let key = solved.solution.rules[rule].candidate.rhs[term].integral;
        assert!(key.powers().iter().all(|power| power.is_symbolic()));
    }
    assert!(!scan.candidates.is_empty());
    let checks = &deferred.region.checks;
    assert!(checks.chart_in_original_parent && checks.failure.is_none());
    assert_eq!(checks.original_restricts_to_zero, Some(false));
    assert_eq!(checks.modular.len(), 4);
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
    assert_eq!(solved.rule_parents.len(), solved.solution.rules.len());
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
    // A two-loop family with exceptional children: same rules, and every
    // recorded parent is an earlier rule whose case contains the child's case.
    let system = SourceSystem::<3>::from_family(&crate::solver::tests::sunset()).unwrap();
    let solver = SectorSolver::new(&system, [true; 3], SectorConfig::default()).unwrap();
    let ordinary = solver.solve_sector(Default::default()).unwrap();
    let solved = solver.solve_sector_deferring(Default::default()).unwrap();
    assert!(solved.deferred.is_empty());
    assert_eq!(shape(&solved.solution), shape(&ordinary));
    assert_eq!(solved.rule_parents.len(), solved.solution.rules.len());
    assert_eq!(solved.rule_parents[0], None);
    let rules = &solved.solution.rules;
    let mut children = 0;
    for (index, parent) in solved.rule_parents.iter().enumerate() {
        let Some(parent) = parent else {
            continue;
        };
        children += 1;
        assert!(parent.rule < index, "{index}: {parent:?}");
        assert!(parent.branch < rules[parent.rule].exceptions.branches.len());
        assert!(
            rules[parent.rule]
                .candidate
                .case
                .contains(&rules[index].candidate.case)
                .unwrap(),
            "{index}: {parent:?}"
        );
    }
    println!(
        "S4_SUNSET rules={} with_parent={children} parents={:?}",
        rules.len(),
        solved.rule_parents
    );
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
                alternatives: vec![SourceDiscoveryStrategy::InputOrder.into()],
                limits: RuleTrialLimits {
                    max_depth: 1,
                    max_rows: 32,
                    max_exact_trace_rows: 32,
                    max_exact_trace_terms: 128,
                    max_alternatives: 2,
                    max_case_elapsed: None,
                },
                quality: vec![RuleQualityPriority {
                    feature: RuleQualityFeature::RhsTerms,
                    descending: false,
                }],
                trigger: RulePortfolioTrigger::Always,
                pre_screen: false,
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

fn power(symbolic: bool, value: i16) -> Power {
    Power::new(symbolic, value).unwrap()
}

fn rule_with_keys(
    context: &CoefficientContext,
    case: Case<3>,
    keys: &[[Power; 3]],
) -> SectorRule<3> {
    SectorRule {
        dispatch_policy: Default::default(),
        candidate: RuleCandidate {
            case,
            target: Integral::symbolic([0; 3]).unwrap(),
            rhs: keys
                .iter()
                .map(|&powers| Term {
                    integral: Integral::new(powers),
                    coefficient: context.coefficient_fixture("1"),
                })
                .collect(),
            sources: Vec::new(),
            stats: SearchStats::default(),
        },
        exceptions: ExceptionalConditions {
            branches: Vec::new(),
        },
    }
}

#[test]
fn the_exit_scan_excludes_only_by_proof() {
    let context = CoefficientContext::new(["a", "b", "c"]);
    let indices = [0, 1, 2];
    let sector = [true; 3];
    let predicates: Vec<CoefficientPolynomial> = ["a-1", "b^2-2*c^2-1"]
        .iter()
        .map(|p| context.coefficient_fixture(p).numerator)
        .collect();
    let generic = rule_with_keys(
        &context,
        Case::generic(),
        &[
            // numeric 0 on an active axis: another chart
            [power(false, 0), power(true, 0), power(true, 0)],
            // numeric 2: a - 1 = 1 there
            [power(false, 2), power(true, 0), power(true, 0)],
            // numeric 1: lands where b^2 - 2c^2 = 1
            [power(false, 1), power(true, 0), power(true, 0)],
            // a + 1 - 1 = a: lands at a = 0 only (outside the chart, kept: no proof here)
            [power(true, 1), power(true, 0), power(true, 0)],
            // b shifted: still a Pell condition
            [power(false, 1), power(true, -1), power(true, 2)],
        ],
    );
    let on_line = Case::generic()
        .intersect(
            &[context.coefficient_fixture("a-b-1").numerator],
            &indices,
            &sector,
        )
        .unwrap()
        .unwrap();
    let affine = rule_with_keys(
        &context,
        on_line,
        &[
            // on a = b + 1: (a-1) -> b, b^2-2c^2-1 stays: possible
            [power(true, 0), power(true, 0), power(true, 0)],
        ],
    );
    let scan = deferred_exit_scan(&predicates, &[generic, affine], &indices, &sector);
    assert_eq!(scan.pairs, 6);
    assert_eq!(scan.excluded, 2);
    assert_eq!(scan.candidates, vec![(0, 2), (0, 3), (0, 4), (1, 0)]);
    // A predicate that is a nonzero constant on the rule's affine case.
    let shifted: Vec<CoefficientPolynomial> = ["a-b-2"]
        .iter()
        .map(|p| context.coefficient_fixture(p).numerator)
        .collect();
    let on_line = Case::generic()
        .intersect(
            &[context.coefficient_fixture("a-b-1").numerator],
            &indices,
            &sector,
        )
        .unwrap()
        .unwrap();
    let rules = [rule_with_keys(
        &context,
        on_line,
        &[
            [power(true, 0), power(true, 0), power(true, 0)],
            [power(true, 1), power(true, 0), power(true, 0)],
        ],
    )];
    let scan = deferred_exit_scan(&shifted, &rules, &indices, &sector);
    // key n: a-b-2 = -1 on the line (excluded); key (a+1, b, c): a+1-b-2 = 0 (lands).
    assert_eq!(scan.excluded, 1);
    assert_eq!(scan.candidates, vec![(0, 1)]);
}

/// Stage 3 at the sector level: with the opt-in refinement
/// (`CaseRefinement::all()`), a rule's exceptional branch that no exact step
/// resolves is deferred WITHOUT entering the Lex F4 fallback (two nonlinear
/// equations: Lex-eligible on the established path). The deferring solve
/// returns it as a [`SectorDeferredRegion`] with the full provenance that
/// table_dump writes into every `deferred_region` record (rule, branch,
/// region with its S1/S2 checks, rule parents, complete parent chain, exit
/// scan), the same as the established deferring solve (B7) except possibly
/// the generators of the same conjunction; and its points are never
/// dispatched to a rule.
#[test]
fn a_refined_solve_defers_the_skipped_lex_branch_with_the_full_record() {
    let context = CoefficientContext::new(["a", "b", "c", "d"]);
    let indices = [0, 1, 2];
    // Exceptional branch {a^2-2b^2-1 = 0, c^2-3b^2-1 = 0}: two irreducible
    // quadrics, no affine factor, no linear coordinate.
    let sources = source(&context, "d*(a^2-2*b^2-1)+(c^2-3*b^2-1)");
    let solver = SectorSolver::new(&sources, [true; 3], SectorConfig::default()).unwrap();
    let established = solver.solve_sector_deferring(Default::default()).unwrap();
    let refined = solver
        .solve_sector_deferring_refined(Default::default(), CaseRefinement::all())
        .unwrap();
    println!("LEX_SKIP_SECTOR established {:?}", established.deferred);
    println!("LEX_SKIP_SECTOR refined {:?}", refined.deferred);
    let shape = |solved: &DeferredSectorSolution<3>| {
        solved
            .solution
            .rules
            .iter()
            .map(|rule| {
                format!(
                    "{:?} {} {:?}",
                    rule.candidate.case, rule.candidate.target, rule.exceptions.branches
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(shape(&refined), shape(&established));
    assert_eq!(refined.rule_parents, established.rule_parents);
    assert_eq!(refined.deferred.len(), 1);
    assert_eq!(established.deferred.len(), 1);
    let (left, right) = (&refined.deferred[0], &established.deferred[0]);
    assert_eq!((left.rule, left.branch), (right.rule, right.branch));
    assert_eq!(
        left.region.reason,
        CaseIntersectionFailure::UnsupportedGeometry
    );
    assert_eq!(left.region.reason, right.region.reason);
    assert_eq!(left.region.parent, right.region.parent);
    assert_eq!(left.region.chart, right.region.chart);
    assert!(left.region.exclusions.is_empty() && left.region.dependents.is_empty());
    assert_eq!(
        left.region.parent,
        refined.solution.rules[left.rule].candidate.case
    );
    // The S1/S2 record is filled exactly as on the established path.
    for region in [&left.region, &right.region] {
        let checks = &region.checks;
        assert!(
            checks.chart_in_original_parent && checks.failure.is_none(),
            "{checks:?}"
        );
        assert_eq!(checks.original_restricts_to_zero, Some(false));
        assert_eq!(
            checks
                .modular
                .iter()
                .map(|(prime, _)| *prime)
                .collect::<Vec<_>>(),
            vec![2, 3, 5, 7]
        );
    }
    assert_eq!(refined.parent_chain(0), established.parent_chain(0));
    assert!(refined.parent_chain(0).1, "complete parent chain");
    // Same zero set as the established region (same ideal, possibly other
    // generators), and the exit scan of the record is computed on it.
    let left_predicates = left.region.predicates(&indices).unwrap();
    let right_predicates = right.region.predicates(&indices).unwrap();
    let branch = &refined.solution.rules[left.rule].exceptions.branches[left.branch];
    let mut region_points = 0;
    for a in 1..=40 {
        for b in 1..=30 {
            for c in 1..=50 {
                let point = [a, b, c];
                let inside = vanishes(&left_predicates, &indices, &point);
                assert_eq!(
                    inside,
                    vanishes(&right_predicates, &indices, &point),
                    "{point:?}"
                );
                assert_eq!(inside, vanishes(branch, &indices, &point), "{point:?}");
                if inside {
                    region_points += 1;
                    assert_eq!(dispatch(&refined, &indices, &point), Dispatch::Deferred(0));
                } else {
                    assert_eq!(dispatch(&refined, &indices, &point), Dispatch::Rule(0));
                }
            }
        }
    }
    let scan = deferred_exit_scan(
        &left_predicates,
        &refined.solution.rules,
        &indices,
        &[true; 3],
    );
    assert_eq!(scan.pairs, refined.solution.rules[0].candidate.rhs.len());
    assert_eq!(scan.candidates.len() + scan.excluded, scan.pairs);
    println!("LEX_SKIP_SECTOR region_points={region_points} scan {scan:?}");
}
