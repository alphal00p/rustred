use crate::algebra::{CoefficientContext, CoefficientPolynomial};
use symbolica::domains::SelfRing;

use super::*;
use crate::solver::{PolynomialRow, SectorConfig, SourceSystem};

fn fixed<const N: usize>(values: [i16; N]) -> CoordinateCase<N> {
    CoordinateCase::new(values.map(Some)).unwrap()
}

fn bounded(depth: u32) -> SearchOptions {
    SearchOptions {
        max_depth: Some(depth),
        ..Default::default()
    }
}

fn polynomial(context: &CoefficientContext, input: &str) -> CoefficientPolynomial {
    context.coefficient_fixture(input).numerator
}

fn row<const N: usize>(
    context: &CoefficientContext,
    terms: &[([i16; N], &str)],
) -> PolynomialRow<N> {
    terms
        .iter()
        .map(|(shift, coefficient)| Term {
            integral: Integral::symbolic(*shift).unwrap(),
            coefficient: polynomial(context, coefficient),
        })
        .collect()
}

#[test]
fn direct_hit_preserves_original_identity_for_other_pending_corners() {
    // I(n+1)-I(n)=0 leaves I(1) free. If direct extraction erases
    // I(2) before submitting this row to GPLU, I(1)=0 is falsely found.
    let context = CoefficientContext::new(["n"]);
    let system = SourceSystem::new(vec![row(&context, &[([1], "1"), ([0], "-1")])], [0]).unwrap();
    let solver = SectorSolver::new(&system, [true], SectorConfig::default()).unwrap();
    let result = solver
        .solve_numeric_cases(vec![fixed([1]), fixed([2]), fixed([2])], bounded(1))
        .unwrap();
    assert_eq!(result.rules.len(), 1);
    assert_eq!(result.rules[0].target, Integral::numeric([2]).unwrap());
    assert_eq!(result.rules[0].rhs.len(), 1);
    assert_eq!(
        result.rules[0].rhs[0].integral,
        Integral::numeric([1]).unwrap()
    );
    assert!(result.rules[0].rhs[0].coefficient.is_one());
    assert_eq!(result.residuals, vec![fixed([1])]);
    assert_eq!(result.stats.cases, 2);
    assert_eq!(result.stats.seeds, 2);
    assert_eq!(result.stats.duplicate_seeds, 2);
    assert_eq!(result.stats.rows, 2);
    assert_eq!(result.stats.direct_rules, 1);
    assert_eq!(result.stats.modular_rules, 0);
    let discovery = result.stats.discovery.unwrap();
    assert_eq!(discovery.rows_seen, 2);
    assert_eq!(discovery.independent_rows, 2);
    assert_eq!(discovery.retained_l_rows, 2);
    let separate_rule = solver.solve_case(fixed([2]), bounded(1)).unwrap();
    let separate_residual_rows = match solver.solve_case(fixed([1]), bounded(1)) {
        Err(SolverError::SearchExhausted { rows, .. }) => rows,
        other => panic!("expected the independent corner search to exhaust: {other:?}"),
    };
    assert_eq!(separate_rule.rhs, result.rules[0].rhs);
    assert_eq!(separate_rule.stats.rows + separate_residual_rows, 4);
}

#[test]
fn overlapping_seed_rows_share_discovery_and_lift_a_nondirect_corner() {
    // These deliberately elementary exact recurrences are incompatible
    // except for zero integrals. Their shifted overlap yields I(1,1)=0:
    // R_x = I(a+1,b)+b I(a,b), R_y = I(a,b+1)+I(a,b).
    // The corner cannot be the leading term of an in-sector seed directly.
    let context = CoefficientContext::new(["a", "b"]);
    let system = SourceSystem::new(
        vec![
            row(&context, &[([1, 0], "1"), ([0, 0], "b")]),
            row(&context, &[([0, 1], "1"), ([0, 0], "1")]),
        ],
        [0, 1],
    )
    .unwrap();
    let solver = SectorSolver::new(&system, [true; 2], SectorConfig::default()).unwrap();
    let result = solver
        .solve_numeric_cases(vec![fixed([2, 1]), fixed([1, 1])], bounded(1))
        .unwrap();
    assert!(result.residuals.is_empty());
    assert_eq!(result.rules.len(), 2);
    let corner = result
        .rules
        .iter()
        .find(|rule| rule.target == Integral::numeric([1, 1]).unwrap())
        .unwrap();
    assert!(!corner.stats.direct_hit);
    assert!(corner.rhs.is_empty());
    assert!(corner.sources.len() >= 4);
    assert_eq!(result.stats.modular_rules, 1);
    assert_eq!(result.stats.direct_rules, 1);
    assert!(result.stats.duplicate_seeds >= 1);
    assert_eq!(result.stats.exact_trace_rows, corner.sources.len());
    assert!(result.stats.discovery.unwrap().dependency_edges > 0);
}

#[test]
fn numerical_search_rejects_missing_bound_symbolic_cases_and_invalid_field() {
    let context = CoefficientContext::new(["n"]);
    let system = SourceSystem::new(vec![row(&context, &[([0], "1")])], [0]).unwrap();
    let solver = SectorSolver::new(&system, [true], SectorConfig::default()).unwrap();
    for (cases, options) in [
        (vec![fixed([1])], SearchOptions::default()),
        (vec![CoordinateCase::generic()], bounded(0)),
        (vec![fixed([0])], bounded(0)),
        (
            vec![fixed([1])],
            SearchOptions {
                prime: 21,
                ..bounded(0)
            },
        ),
    ] {
        assert!(matches!(
            solver.solve_numeric_cases(cases, options),
            Err(SolverError::InvalidInput(_))
        ));
    }
    let result = solver.solve_numeric_cases(Vec::new(), bounded(0)).unwrap();
    assert!(result.rules.is_empty());
    assert!(result.residuals.is_empty());
    assert_eq!(result.stats.rows, 0);
    assert!(result.stats.discovery.is_none());
}

#[test]
fn removed_delta_coordinates_are_checked_and_never_seeded() {
    let context = CoefficientContext::new(["n"]);
    let system = SourceSystem::new(vec![row(&context, &[([0], "n-1")])], [0]).unwrap();
    let solver = SectorSolver::new(
        &system,
        [true],
        SectorConfig {
            deltas: [true],
            removed_deltas: [true],
            ..Default::default()
        },
    )
    .unwrap();
    assert!(matches!(
        solver.solve_numeric_cases(vec![fixed([2])], bounded(2)),
        Err(SolverError::InvalidInput(_))
    ));
    let result = solver
        .solve_numeric_cases(vec![fixed([1])], bounded(2))
        .unwrap();
    assert_eq!(result.residuals, vec![fixed([1])]);
    assert_eq!(result.stats.seeds, 1);
    assert!(result.stats.discovery.is_none());
}

#[test]
fn exact_union_replay_solves_multiple_targets_in_one_native_system() {
    let context = CoefficientContext::new(["x"]);
    let order = IntegralOrder::new([true], [false]);
    let exact = |terms: &[(i16, &str)]| -> ExactRow<1> {
        terms
            .iter()
            .map(|(power, value)| Term {
                integral: Integral::numeric([*power]).unwrap(),
                coefficient: context.coefficient_fixture(value),
            })
            .collect()
    };
    let rows = vec![
        exact(&[(4, "1"), (3, "1"), (1, "1")]),
        exact(&[(4, "1"), (3, "2"), (2, "1")]),
        exact(&[(3, "1"), (2, "2"), (1, "x")]),
    ];
    let targets = [
        Integral::numeric([3]).unwrap(),
        Integral::numeric([2]).unwrap(),
    ];
    let solutions = exact_materialize_many(&rows, &order, &targets).unwrap();
    assert_eq!(solutions.len(), 2);
    for (target, result) in solutions {
        let independent =
            super::super::discovery::exact_materialize(&rows, &order, targets[target]).unwrap();
        assert_eq!(result, independent);
    }
    assert!(exact_materialize_many(&rows, &order, &[Integral::numeric([1]).unwrap()]).is_err());
    assert!(exact_materialize_many(&rows, &order, &[Integral::numeric([5]).unwrap()]).is_err());
    assert!(exact_materialize_many(&rows, &order, &[targets[0], targets[0]]).is_err());
}

#[test]
fn exhausted_numeric_search_returns_residuals_without_master_authority() {
    let context = CoefficientContext::new(["n"]);
    let system = SourceSystem::new(vec![row(&context, &[([-1], "1")])], [0]).unwrap();
    let solver = SectorSolver::new(&system, [true], SectorConfig::default()).unwrap();
    let result = solver
        .solve_numeric_cases(vec![fixed([63])], bounded(0))
        .unwrap();
    assert_eq!(result.residuals, vec![fixed([63])]);
    assert!(result.rules.is_empty());
    assert_eq!(result.stats.seeds, 1);
}

#[test]
fn actual_vac3_numeric_exception_matches_the_reference_equation() {
    // The sole fully fixed rule in SpIRed's vac3 output lies in 011100.
    // Here the three active denominator momenta are independent, so every
    // strict pinch loses a loop direction and is scaleless. These test-only
    // zero sectors are constructed without reading vendored output files.
    let sector = [false, true, true, true, false, false];
    let zero_sectors = (0..7)
        .map(|mask| {
            [
                false,
                mask & 1 != 0,
                mask & 2 != 0,
                mask & 4 != 0,
                false,
                false,
            ]
        })
        .collect();
    let sources = SourceSystem::<6>::from_family(&crate::solver::tests::vac3()).unwrap();
    let solver = SectorSolver::new(
        &sources,
        sector,
        SectorConfig {
            zero_sectors,
            ..Default::default()
        },
    )
    .unwrap();
    let target = fixed([0, 1, 1, 1, 0, -1]);
    let corner = fixed([0, 1, 1, 1, 0, 0]);
    let result = solver
        .solve_numeric_cases(vec![target, corner], bounded(3))
        .unwrap();
    assert_eq!(result.residuals, [corner]);
    assert_eq!(result.rules.len(), 1);
    let rule = &result.rules[0];
    assert_eq!(rule.target, target.integral());
    assert_eq!(rule.rhs.len(), 2);
    let positive = rule
        .rhs
        .iter()
        .find(|term| term.integral == Integral::numeric([0, 1, 1, 1, -1, 0]).unwrap())
        .unwrap();
    assert!(positive.coefficient.is_one());
    let negative = rule
        .rhs
        .iter()
        .find(|term| term.integral == Integral::numeric([-1, 1, 1, 1, 0, 0]).unwrap())
        .unwrap();
    assert!((-negative.coefficient.clone()).is_one());
}

#[test]
fn multiple_centre_changes_take_effect_only_after_the_current_seed_batch() {
    let cases = [fixed([3]), fixed([2]), fixed([1])];
    let mut pending = [true; 3];
    let mut current = Some(0);
    let mut seeds = Seeds::new(cases[0].integral(), [true], [false]);
    let old_seed = seeds.next().unwrap().unwrap();
    for solved in [0, 1] {
        pending[solved] = false;
        switch_solved_centre(
            solved,
            &cases,
            &pending,
            &mut current,
            &mut seeds,
            [true],
            [false],
        );
        assert_eq!(old_seed.integral, cases[0].integral());
    }
    assert_eq!(current, Some(2));
    assert_eq!(seeds.next().unwrap().unwrap().integral, cases[2].integral());

    // An earlier exhausted case stays pending and matchable even after the
    // final centre is solved. Its current stream is not silently reset.
    let pending = [true, false, false];
    switch_solved_centre(
        2,
        &cases,
        &pending,
        &mut current,
        &mut seeds,
        [true],
        [false],
    );
    assert_eq!(current, None);
    let shifted = seeds.next().unwrap().unwrap();
    assert_eq!(shifted.integral, Integral::numeric([2]).unwrap());
    assert_eq!(
        matching_case(&cases, &pending, cases[0].integral()),
        Some(0)
    );
}
