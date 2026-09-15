use std::sync::Arc;

use crate::algebra::CoefficientContext;
use crate::family::{AffineDenominator, IntegralFamily};
use crate::solver::{
    CoordinateCase, ExceptionalConditions, Integral, RuleCandidate, SectorConfig, SectorRule,
    SectorSolution, SectorSolveOptions, SectorSolver, SourceSystem, Term,
};

use super::{SourcePortAudit, geometry};

fn tadpole() -> IntegralFamily {
    let context = CoefficientContext::new(["d"]);
    IntegralFamily::new(
        "source-port-certificate-tadpole",
        vec!["k".into()],
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
        vec![AffineDenominator::new(
            context.integer(-1),
            vec![context.one()],
        )],
        Vec::new(),
        vec![context.zero()],
    )
    .unwrap()
}

fn solved_tadpole() -> (SourcePortAudit<1>, SectorSolution<1>) {
    let family = tadpole();
    let zeros: Arc<[[bool; 1]]> = Arc::from([[false]]);
    let source = SourceSystem::from_family(&family).unwrap();
    let solver = SectorSolver::new(
        &source,
        [true],
        SectorConfig {
            zero_sectors: zeros.clone(),
            ..Default::default()
        },
    )
    .unwrap();
    let solution = solver.solve_sector(SectorSolveOptions::default()).unwrap();
    (SourcePortAudit::try_new(&family, zeros).unwrap(), solution)
}

#[test]
fn ordinary_replay_and_unbounded_cover_close_the_generated_tadpole_report() {
    let (audit, solution) = solved_tadpole();
    assert_eq!(audit.original_source_count(), 1);
    assert_eq!(audit.proved_zero_sector_count(), 1);
    let report = audit.audit_sector([true], None, &solution).unwrap();
    assert_eq!(report.rules, 1);
    assert_eq!(report.exact_replayed_rules, 1);
    assert_eq!(report.uniformly_descending_rules, 1);
    assert_eq!(report.additional_replay_guard_branches, 0);
    assert_eq!(report.finite_terminals, 1);
    assert_eq!(report.checked_rule_uncovered_boxes, 0);
    assert_eq!(report.stored_guard_uncovered_boxes, 0);
    assert!(report.issues.is_empty(), "{:?}", report.issues);
    audit.validate_sector_census(&[report]).unwrap();
}

#[test]
fn removing_terminal_and_removing_rule_distinguish_finite_and_infinite_gaps() {
    let (audit, mut solution) = solved_tadpole();
    let terminals = std::mem::take(&mut solution.finite_residuals);
    let finite = audit.audit_sector([true], None, &solution).unwrap();
    assert_eq!(finite.checked_rule_uncovered_boxes, 1);
    assert_eq!(finite.checked_rule_unbounded_boxes, 0);
    solution.finite_residuals = terminals;
    solution.rules.clear();
    let infinite = audit.audit_sector([true], None, &solution).unwrap();
    assert_eq!(infinite.checked_rule_uncovered_boxes, 1);
    assert_eq!(infinite.checked_rule_unbounded_boxes, 1);
}

#[test]
fn mutated_rhs_cannot_pass_regenerated_source_replay() {
    let (audit, mut solution) = solved_tadpole();
    solution.rules[0].candidate.rhs[0].coefficient =
        -solution.rules[0].candidate.rhs[0].coefficient.clone();
    let report = audit.audit_sector([true], None, &solution).unwrap();
    assert_eq!(report.exact_replayed_rules, 0);
    assert!(!report.issues.is_empty());
    assert_eq!(report.checked_rule_unbounded_boxes, 1);
}

#[test]
fn invalid_provenance_ordinal_is_reported_without_trusting_stored_equations() {
    let (audit, mut solution) = solved_tadpole();
    solution.rules[0].candidate.sources[0].basis_row = usize::MAX;
    let report = audit.audit_sector([true], None, &solution).unwrap();
    assert_eq!(report.exact_replayed_rules, 0);
    assert!(report.issues[0].contains("ordinal"));
}

#[test]
fn omitted_serialized_pole_is_recomputed_and_retained_as_an_extra_obligation() {
    let (audit, mut solution) = solved_tadpole();
    solution.rules[0].exceptions = ExceptionalConditions::default();
    let report = audit.audit_sector([true], None, &solution).unwrap();
    assert_eq!(report.exact_replayed_rules, 1);
    assert_eq!(report.additional_replay_guard_branches, 1);
    assert_eq!(report.checked_rule_uncovered_boxes, 0);
    assert_eq!(report.finite_terminals, 1);
}

#[test]
fn malformed_seed_and_consistently_mutated_rhs_do_not_forge_ordinary_provenance() {
    let (audit, mut solution) = solved_tadpole();
    let solver = SectorSolver::new(
        &audit.sources,
        [true],
        SectorConfig {
            zero_sectors: audit.zero_sectors.clone(),
            ..Default::default()
        },
    )
    .unwrap();
    let order = crate::solver::IntegralOrder::new([true], [false]);
    let mut source = solution.rules[0].candidate.sources[0];
    source.seed.shifts[0] += 1;
    assert_ne!(source.seed.integral[0].value(), source.seed.shifts[0]);
    // Fabricate the matching RHS with the same malformed transport. Merely
    // replaying this transport twice would reproduce an invalid IBP exactly.
    let row = crate::solver::instantiate_source_port(
        &solver.basis()[source.basis_row],
        &source.seed,
        audit.sources.index_variables(),
        audit.sources.fixed(),
        &order,
        &audit.zero_sectors,
        None,
    )
    .unwrap();
    let (target, rhs) =
        crate::solver::canonicalize_source_port(row, audit.sources.index_variables()).unwrap();
    assert_eq!(target, solution.rules[0].candidate.target);
    assert_ne!(rhs, solution.rules[0].candidate.rhs);
    solution.rules[0].candidate.rhs = rhs;
    solution.rules[0].candidate.sources = vec![source];
    solution.rules[0].exceptions = crate::solver::extract_exceptions(
        &solution.rules[0].candidate,
        audit.sources.index_variables(),
        &[true],
    )
    .unwrap();
    let report = audit.audit_sector([true], None, &solution).unwrap();
    assert_eq!(report.exact_replayed_rules, 0);
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.contains("seed coefficient translation"))
    );
    assert_eq!(report.checked_rule_unbounded_boxes, 1);
}

#[test]
fn numeric_seed_shifts_and_wrong_seed_patterns_are_rejected() {
    let (audit, mut solution) = solved_tadpole();
    solution.rules[0].candidate.sources[0].seed.integral = Integral::numeric([2]).unwrap();
    solution.rules[0].candidate.sources[0].seed.shifts = [0];
    let wrong_pattern = audit.audit_sector([true], None, &solution).unwrap();
    assert_eq!(wrong_pattern.exact_replayed_rules, 0);
    assert!(
        wrong_pattern
            .issues
            .iter()
            .any(|issue| issue.contains("seed symbolic pattern"))
    );

    // Numeric seed coordinates are replacement values. Their shifts must be
    // zero; the underlying instantiator intentionally ignores this field.
    let fixed = CoordinateCase::new([Some(2)]).unwrap();
    solution.rules[0].candidate.case = fixed.into();
    solution.rules[0].candidate.target = fixed.integral();
    solution.rules[0].candidate.sources[0].seed.shifts = [1];
    let numeric_shift = audit.audit_sector([true], None, &solution).unwrap();
    assert_eq!(numeric_shift.exact_replayed_rules, 0);
    assert!(
        numeric_shift
            .issues
            .iter()
            .any(|issue| issue.contains("seed coefficient translation"))
    );
}

#[test]
fn false_and_duplicate_zero_censuses_are_rejected() {
    assert!(SourcePortAudit::<1>::try_new(&tadpole(), Arc::from([[true]])).is_err());
    assert!(SourcePortAudit::<1>::try_new(&tadpole(), Arc::from([[false], [false]])).is_err());
    let (audit, solution) = solved_tadpole();
    assert!(audit.validate_sector_census(&[]).is_err());
    assert!(audit.audit_sector([false], None, &solution).is_err());
}

fn boundary_rule(coefficient: &str) -> (CoefficientContext, SectorRule<2>) {
    let context = CoefficientContext::new(["n0", "n1", "d"]);
    let case = CoordinateCase::generic();
    let candidate = RuleCandidate {
        case: case.into(),
        target: case.integral(),
        rhs: vec![Term {
            integral: Integral::symbolic([1, -2]).unwrap(),
            coefficient: context.coefficient_fixture(coefficient),
        }],
        sources: Vec::new(),
        stats: Default::default(),
    };
    (
        context,
        SectorRule {
            candidate,
            exceptions: ExceptionalConditions::default(),
        },
    )
}

#[test]
fn whole_ray_descent_drops_only_exactly_vanishing_activation_boundary_terms() {
    let (_, rule) = boundary_rule("n0");
    let cells = geometry::application_boxes(&rule, &[0, 1], &[false, true], &[]).unwrap();
    geometry::prove_descent(
        &rule,
        &cells,
        &[false, true],
        crate::sector::OrderingPolicy::SpiredUncutV1,
        &[0, 1],
    )
    .unwrap();
    let (_, unsafe_rule) = boundary_rule("1");
    assert!(
        geometry::prove_descent(
            &unsafe_rule,
            &cells,
            &[false, true],
            crate::sector::OrderingPolicy::SpiredUncutV1,
            &[0, 1]
        )
        .is_err()
    );
}

#[test]
fn zero_projection_checks_all_physical_sign_cells_not_the_assumed_sector() {
    let (context, mut rule) = boundary_rule("1");
    let case = CoordinateCase::new([None, Some(1)]).unwrap();
    rule.candidate.case = case.into();
    rule.candidate.target = case.integral();
    rule.candidate.rhs.clear();
    let column = Integral::new([
        crate::solver::Power::new(true, 1).unwrap(),
        crate::solver::Power::new(false, 0).unwrap(),
    ]);
    let all = geometry::application_boxes(&rule, &[0, 1], &[false, true], &[]).unwrap();
    assert!(
        !geometry::uniformly_zero_column(&rule, column, &all, &[false, true], &[[false, false]])
            .unwrap()
    );
    rule.exceptions.branches = vec![vec![context.parameter("n0").unwrap().numerator.clone()]];
    let restricted = geometry::application_boxes(&rule, &[0, 1], &[false, true], &[]).unwrap();
    assert!(
        geometry::uniformly_zero_column(
            &rule,
            column,
            &restricted,
            &[false, true],
            &[[false, false]]
        )
        .unwrap()
    );
}

#[test]
fn zero_product_projection_uses_the_original_coefficient_on_activation_faces() {
    let (context, mut rule) = boundary_rule("1");
    let case = CoordinateCase::new([None, Some(1)]).unwrap();
    rule.candidate.case = case.into();
    rule.candidate.target = case.integral();
    rule.candidate.rhs.clear();
    let boxes = geometry::application_boxes(&rule, &[0, 1], &[false, true], &[]).unwrap();
    let mut term = Term {
        integral: Integral::new([
            crate::solver::Power::new(true, 1).unwrap(),
            crate::solver::Power::new(false, 0).unwrap(),
        ]),
        coefficient: context.coefficient_fixture("n0"),
    };
    let check = |term: &Term<2, crate::algebra::Coefficient>| {
        geometry::uniformly_zero_term(
            &rule,
            term,
            &boxes,
            &[false, true],
            &[[false, false]],
            &[0, 1],
        )
        .unwrap()
    };
    // n0 < 0: both columns absent, hence a proved-zero sector.
    // n0 = 0: the first column activates, but its own coefficient is zero.
    assert!(check(&term));
    term.coefficient = context.coefficient_fixture("1");
    assert!(
        !check(&term),
        "a nonzero activation contribution must remain"
    );
    term.coefficient = context.coefficient_fixture("n0+1");
    assert!(
        !check(&term),
        "a different coefficient root is not the boundary"
    );
    term.coefficient = context.coefficient_fixture("n0/(n1-1)");
    assert!(
        !check(&term),
        "0/0 on the fixed face is not a zero coefficient"
    );
}

#[test]
fn all_two_loop_pinches_replay_with_exact_zero_product_boundaries() {
    let context = CoefficientContext::new(["d", "m"]);
    let mass = context.parameter("m").unwrap();
    let family = IntegralFamily::new(
        "source-port-zero-product-sunset",
        vec!["k0".into(), "k1".into()],
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
        [[1, 0, 0], [0, 0, 1], [1, 2, 1]]
            .into_iter()
            .map(|row| {
                AffineDenominator::new(
                    -mass.clone(),
                    row.into_iter()
                        .map(|value| context.integer(value))
                        .collect(),
                )
            })
            .collect(),
        Vec::new(),
        vec![context.zero(); 3],
    )
    .unwrap();
    let zeros: Arc<[[bool; 3]]> = Arc::from([
        [false, false, false],
        [true, false, false],
        [false, true, false],
        [false, false, true],
    ]);
    let sources = SourceSystem::from_family(&family).unwrap();
    let audit = SourcePortAudit::try_new(&family, zeros.clone()).unwrap();
    let mut reports = Vec::new();
    for sector in [
        [false, true, true],
        [true, false, true],
        [true, true, false],
        [true, true, true],
    ] {
        let solver = SectorSolver::new(
            &sources,
            sector,
            SectorConfig {
                zero_sectors: zeros.clone(),
                ..Default::default()
            },
        )
        .unwrap();
        let solution = solver.solve_sector(SectorSolveOptions::default()).unwrap();
        let report = audit.audit_sector(sector, None, &solution).unwrap();
        assert!(report.issues.is_empty(), "{sector:?}: {:?}", report.issues);
        assert_eq!(report.exact_replayed_rules, report.rules);
        assert_eq!(report.uniformly_descending_rules, report.rules);
        assert_eq!(report.checked_rule_uncovered_boxes, 0);
        assert_eq!(report.additional_replay_guard_branches, 0);
        reports.push(report);
    }
    assert_eq!(reports.iter().map(|report| report.rules).sum::<usize>(), 18);
    audit.validate_sector_census(&reports).unwrap();
}

#[test]
fn last_three_vacuum_replay_sectors_close_via_weighted_original_product_certificates() {
    let context = CoefficientContext::new(["d", "m"]);
    let mass = context.parameter("m").unwrap();
    let momenta: [[i64; 3]; 6] = [
        [1, 0, 0],
        [0, 1, 0],
        [0, 0, 1],
        [1, 1, 0],
        [1, 0, 1],
        [0, 1, -1],
    ];
    let family = IntegralFamily::new(
        "source-port-weighted-zero-vacuum",
        (0..3).map(|i| format!("k{i}")).collect(),
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
        momenta
            .into_iter()
            .map(|momentum| {
                AffineDenominator::new(
                    -mass.clone(),
                    (0..3)
                        .flat_map(|i| {
                            (i..3).map(move |j| {
                                momentum[i] * momentum[j] * if i == j { 1 } else { 2 }
                            })
                        })
                        .map(|value| context.integer(value))
                        .collect(),
                )
            })
            .collect(),
        Vec::new(),
        vec![context.zero(); 6],
    )
    .unwrap();
    let analyzer = crate::sector::zero::Analyzer::try_unrestricted(&family).unwrap();
    let zeros: Arc<[[bool; 6]]> = Arc::from(
        (0..64)
            .map(|bits| std::array::from_fn(|axis| bits & (1 << axis) != 0))
            .filter(|mask| {
                matches!(
                    analyzer
                        .analyze(&crate::sector::Mask::try_new(*mask).unwrap())
                        .unwrap(),
                    crate::sector::zero::Decision::ProvedZero(_)
                )
            })
            .collect::<Vec<_>>(),
    );
    let sources = SourceSystem::from_family(&family).unwrap();
    let audit = SourcePortAudit::try_new(&family, zeros.clone()).unwrap();
    for sector in [
        [false, false, true, true, false, true],
        [false, true, false, false, true, true],
        [true, false, false, false, true, true],
    ] {
        let solver = SectorSolver::new(
            &sources,
            sector,
            SectorConfig {
                zero_sectors: zeros.clone(),
                ..Default::default()
            },
        )
        .unwrap();
        let solution = solver.solve_sector(SectorSolveOptions::default()).unwrap();
        let report = audit.audit_sector(sector, None, &solution).unwrap();
        assert!(report.issues.is_empty(), "{sector:?}: {:?}", report.issues);
        assert_eq!(report.exact_replayed_rules, report.rules);
        assert_eq!(report.uniformly_descending_rules, report.rules);
        assert_eq!(report.additional_replay_guard_branches, 0);
        assert_eq!(report.checked_rule_uncovered_boxes, 0);
    }
}
