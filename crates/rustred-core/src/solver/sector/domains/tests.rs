use super::*;
use crate::algebra::CoefficientContext;
use crate::solver::tests::tadpole;
use crate::solver::{
    AffineCase, AffineIntersection, CoordinateCase, FiniteRetentionError, SectorConfig,
    SolverError, SourceSystem, Term,
};

fn tadpole_source() -> SourceSystem<1> {
    SourceSystem::from_family(&tadpole()).unwrap()
}

fn zero_source(context: &CoefficientContext) -> SourceSystem<2> {
    SourceSystem::new(
        vec![vec![Term {
            integral: Integral::symbolic([0, 0]).unwrap(),
            coefficient: context.one().numerator,
        }]],
        [0, 1],
    )
    .unwrap()
}

#[test]
fn generic_domain_matches_full_sector_without_gaining_its_output_type() {
    let source = tadpole_source();
    let solver = SectorSolver::new(
        &source,
        [true],
        SectorConfig {
            zero_sectors: vec![[false]].into(),
            ..Default::default()
        },
    )
    .unwrap();
    let full = solver.solve_sector(Default::default()).unwrap();
    let partial = solver
        .solve_domains(vec![Case::generic()], Default::default())
        .unwrap();
    assert_eq!(partial.requested_cases, [Case::generic()]);
    assert_eq!(partial.rules.len(), full.rules.len());
    assert_eq!(partial.finite_residuals, full.finite_residuals);
    assert_eq!(partial.stats.symbolic_cases, full.stats.symbolic_cases);
    for (left, right) in partial.rules.iter().zip(full.rules.iter()) {
        assert_eq!(left.candidate.target, right.candidate.target);
        assert_eq!(left.candidate.rhs, right.candidate.rhs);
        assert_eq!(left.exceptions, right.exceptions);
    }
}

#[test]
fn initial_duplicate_and_subsumed_nominations_do_not_repeat_search() {
    let source = tadpole_source();
    let solver = SectorSolver::new(&source, [true], SectorConfig::default()).unwrap();
    let corner = CoordinateCase::new([Some(1)]).unwrap().into();
    let mut starts = Vec::new();
    let result = solver
        .solve_domains_with_observer(
            vec![corner, Case::generic(), Case::generic()],
            Default::default(),
            |event| {
                if let SectorEvent::CaseStarted { case, .. } = event {
                    starts.push(case);
                }
            },
        )
        .unwrap();
    assert_eq!(starts, [Case::generic()]);
    assert_eq!(result.stats.symbolic_cases, 1);
    assert_eq!(result.stats.numerical_cases, 1);
    assert_eq!(result.finite_residuals, [Integral::numeric([1]).unwrap()]);
    let reverse = solver
        .solve_domains(
            vec![
                Case::generic(),
                CoordinateCase::new([Some(1)]).unwrap().into(),
            ],
            Default::default(),
        )
        .unwrap();
    assert_eq!(reverse.stats.symbolic_cases, result.stats.symbolic_cases);
    assert_eq!(reverse.stats.numerical_cases, result.stats.numerical_cases);
    assert_eq!(reverse.finite_residuals, result.finite_residuals);
    assert_eq!(reverse.rules.len(), result.rules.len());
}

#[test]
fn a_missing_domain_keeps_positive_powers_parametric() {
    let context = CoefficientContext::new(["a", "b"]);
    let sources = zero_source(&context);
    let solver = SectorSolver::new(&sources, [true, false], Default::default()).unwrap();
    let domain = Case::from(CoordinateCase::new([None, Some(-11)]).unwrap());
    let result = solver
        .solve_domains(vec![domain.clone()], Default::default())
        .unwrap();
    assert_eq!(result.max_numerator_rank, None);
    assert_eq!(result.rules.len(), 1);
    assert_eq!(result.rules[0].candidate.case, domain);
    assert!(result.rules[0].candidate.target.powers()[0].is_symbolic());
    assert_eq!(result.rules[0].candidate.target.powers()[1].value(), -11);
    assert!(result.finite_residuals.is_empty());
}

#[test]
fn explicit_descendant_scope_is_not_implicitly_replaced_by_rank_ten() {
    let context = CoefficientContext::new(["a", "b"]);
    let sources = zero_source(&context);
    let solver = SectorSolver::new(&sources, [true, false], Default::default()).unwrap();
    let domain = Case::from(CoordinateCase::new([None, Some(-11)]).unwrap());
    let outside = solver
        .solve_domains(
            vec![domain.clone()],
            SectorSolveOptions {
                max_numerator_rank: Some(10),
                ..Default::default()
            },
        )
        .unwrap();
    assert!(outside.rules.is_empty());
    let descendant = solver
        .solve_domains(
            vec![domain],
            SectorSolveOptions {
                max_numerator_rank: Some(11),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(descendant.rules.len(), 1);
}

#[test]
fn invalid_later_nomination_fails_before_any_search_event() {
    let source = tadpole_source();
    let solver = SectorSolver::new(&source, [true], Default::default()).unwrap();
    let mut events = 0;
    let error = solver
        .solve_domains_with_observer(
            vec![
                Case::generic(),
                CoordinateCase::new([Some(0)]).unwrap().into(),
            ],
            Default::default(),
            |_| events += 1,
        )
        .unwrap_err();
    assert_eq!(events, 0);
    assert!(matches!(
        error,
        SectorSolveError::Search {
            source: SolverError::InvalidInput(_),
            ..
        }
    ));
}

#[test]
fn incomplete_domain_queue_is_an_error_not_a_partial_success() {
    let source = tadpole_source();
    let solver = SectorSolver::new(&source, [true], Default::default()).unwrap();
    let error = solver
        .solve_domains(
            vec![Case::generic()],
            SectorSolveOptions {
                max_symbolic_cases: Some(0),
                ..Default::default()
            },
        )
        .unwrap_err();
    assert!(matches!(error, SectorSolveError::CaseBudget { .. }));
}

#[test]
fn numerical_domain_enters_shared_numeric_search_without_symbolic_widening() {
    let source = tadpole_source();
    let solver = SectorSolver::new(&source, [true], Default::default()).unwrap();
    let result = solver
        .solve_domains(
            vec![CoordinateCase::new([Some(1)]).unwrap().into()],
            SectorSolveOptions {
                numerical_depth: 0,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(result.stats.symbolic_cases, 0);
    assert_eq!(result.stats.numerical_cases, 1);
    assert_eq!(result.finite_residuals, [Integral::numeric([1]).unwrap()]);
}

#[test]
fn empty_domain_request_is_empty_not_full_sector_search() {
    let source = tadpole_source();
    let solver = SectorSolver::new(&source, [true], Default::default()).unwrap();
    let mut events = 0;
    let result = solver
        .solve_domains_with_observer(Vec::new(), Default::default(), |_| events += 1)
        .unwrap();
    assert!(result.requested_cases.is_empty());
    assert!(result.rules.is_empty());
    assert!(result.finite_residuals.is_empty());
    assert_eq!(result.stats.symbolic_cases, 0);
    assert!(events <= 1); // An empty shared-numerical phase is harmless.
}

#[test]
fn foreign_affine_variable_binding_is_rejected_before_search() {
    let context = CoefficientContext::new(["a", "b"]);
    let sources = zero_source(&context);
    let solver = SectorSolver::new(&sources, [true, true], Default::default()).unwrap();
    let foreign = AffineCase::from_coordinate(
        &CoordinateCase::generic(),
        &[context.coefficient_fixture("a-b").numerator],
        &[1, 0],
        &[true, true],
    )
    .unwrap();
    let AffineIntersection::Affine(foreign) = foreign else {
        panic!("fixture must retain a coupled affine domain");
    };
    let mut events = 0;
    let error = solver
        .solve_domains_with_observer(
            vec![Case::generic(), foreign.into()],
            Default::default(),
            |_| events += 1,
        )
        .unwrap_err();
    assert_eq!(events, 0);
    assert!(matches!(
        error,
        SectorSolveError::Search {
            source: SolverError::InvalidInput(_),
            ..
        }
    ));
}

#[test]
fn finite_retention_without_a_bound_does_not_invent_the_entry_rank() {
    let source = tadpole_source();
    let solver = SectorSolver::new(&source, [true], Default::default()).unwrap();
    let error = solver
        .solve_domains(
            vec![CoordinateCase::new([Some(1)]).unwrap().into()],
            SectorSolveOptions {
                finite_case_policy: FiniteCasePolicy::RetainRankFinite,
                max_numerator_rank: None,
                ..Default::default()
            },
        )
        .unwrap_err();
    assert!(matches!(
        error,
        SectorSolveError::FiniteRetention {
            source: FiniteRetentionError::MissingNumeratorRank,
            ..
        }
    ));
}
