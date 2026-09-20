//! Source scheduling changes arrival order, never stored source identity.

use crate::algebra::CoefficientContext;
use crate::solver::CoordinateCase;
use crate::solver::discovery::exact_materialize;

use super::*;

fn row(context: &CoefficientContext, shifts: &[i16]) -> PolynomialRow<1> {
    shifts
        .iter()
        .map(|&shift| Term {
            integral: Integral::symbolic([shift]).unwrap(),
            coefficient: context.one().numerator,
        })
        .collect()
}

fn depth_zero() -> SearchOptions {
    SearchOptions {
        max_depth: Some(0),
        ..Default::default()
    }
}

fn assert_same_candidate(left: &RuleCandidate<1>, right: &RuleCandidate<1>) {
    assert_eq!(left.case, right.case);
    assert_eq!(left.target, right.target);
    assert_eq!(left.rhs, right.rhs);
    assert_eq!(left.sources, right.sources);
    assert_eq!(left.stats.seeds, right.stats.seeds);
    assert_eq!(left.stats.rows, right.stats.rows);
    assert_eq!(left.stats.independent_rows, right.stats.independent_rows);
    assert_eq!(left.stats.exact_trace_rows, right.stats.exact_trace_rows);
    assert_eq!(left.stats.direct_hit, right.stats.direct_hit);
    assert_eq!(left.stats.discovery, right.stats.discovery);
}

fn assert_regenerated_equation(solver: &SectorSolver<'_, 1>, candidate: &RuleCandidate<1>) {
    let selected = candidate
        .sources
        .iter()
        .map(|source| {
            instantiate(
                &solver.basis()[source.basis_row],
                &source.seed,
                solver.system.index_variables(),
                solver.system.fixed(),
                solver.ordering(),
                &[],
                candidate.case.affine(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let exact = if candidate.stats.direct_hit {
        assert_eq!(selected.len(), 1);
        selected[0].clone()
    } else {
        // The GPLU fixture is a fixed numerical target, so its discovery
        // pivot equals the canonical target. Retain selected arrival order.
        assert!(candidate.case.is_numerical());
        exact_materialize(&selected, solver.ordering(), candidate.target).unwrap()
    };
    let (target, rhs) = canonicalize(exact, solver.system.index_variables()).unwrap();
    assert_eq!(target, candidate.target);
    assert_eq!(rhs, candidate.rhs);
    assert!(
        rhs.iter()
            .all(|term| { solver.ordering().compare(&target, &term.integral) == Ordering::Less })
    );
}

#[test]
fn explicit_identity_preserves_default_direct_hit_and_observation() {
    let system = SourceSystem::<1>::from_family(&crate::solver::tests::tadpole()).unwrap();
    let solver = SectorSolver::new(&system, [true], SectorConfig::default()).unwrap();
    let source_order = (0..solver.basis().len()).collect::<Vec<_>>();
    let mut default_events = Vec::new();
    let baseline = solver
        .solve_case_with_observer(CoordinateCase::generic(), depth_zero(), |event| {
            default_events.push(event);
        })
        .unwrap();
    let mut ordered_events = Vec::new();
    let ordered = solver
        .solve_case_with_source_order_and_observer(
            CoordinateCase::generic(),
            depth_zero(),
            &source_order,
            |event| ordered_events.push(event),
        )
        .unwrap();
    assert_same_candidate(&baseline, &ordered);
    assert_eq!(default_events, ordered_events);
    assert!(ordered.stats.direct_hit);
    assert_regenerated_equation(&solver, &ordered);
}

#[test]
fn invalid_source_orders_fail_before_any_observed_work() {
    let context = CoefficientContext::new(["n"]);
    let system = SourceSystem::new(
        vec![
            row(&context, &[0, -1]),
            row(&context, &[0, -2]),
            row(&context, &[0, -3]),
        ],
        [0],
    )
    .unwrap();
    let solver = SectorSolver::new(&system, [true], SectorConfig::default()).unwrap();
    assert_eq!(solver.basis().len(), 3);
    for order in [
        vec![],
        vec![0, 1],
        vec![0, 1, 2, 3],
        vec![0, 1, 1],
        vec![0, 1, 3],
        vec![usize::MAX, 1, 2],
    ] {
        let mut observed = 0;
        let result = solver.solve_case_with_source_order_and_observer(
            CoordinateCase::generic(),
            depth_zero(),
            &order,
            |_| observed += 1,
        );
        assert!(
            matches!(result, Err(SolverError::InvalidInput(_))),
            "{order:?}"
        );
        assert_eq!(observed, 0, "invalid schedule reached search: {order:?}");
    }
}

#[test]
fn reverse_and_rotation_keep_direct_basis_ids_and_cold_precondition_provenance() {
    let context = CoefficientContext::new(["n"]);
    let system = SourceSystem::new(
        vec![
            row(&context, &[0, -1]),
            row(&context, &[0, -2]),
            row(&context, &[0, -3]),
        ],
        [0],
    )
    .unwrap();
    let original = system.rows().to_vec();
    let solver = SectorSolver::new(&system, [true], SectorConfig::default()).unwrap();
    let stored_basis = solver.basis().to_vec();
    let (cold, provenance) =
        SectorSolver::new_with_provenance(&system, [true], SectorConfig::default()).unwrap();
    assert_eq!(cold.basis(), stored_basis);
    for order in [[0, 1, 2], [2, 1, 0], [1, 2, 0]] {
        let candidate = solver
            .solve_case_with_source_order_and_observer(
                CoordinateCase::generic(),
                depth_zero(),
                &order,
                |_| {},
            )
            .unwrap();
        assert!(candidate.stats.direct_hit);
        assert_eq!(candidate.sources.len(), 1);
        let source_id = candidate.sources[0].basis_row;
        assert_eq!(source_id, order[0]);
        assert_regenerated_equation(&cold, &candidate);

        // Recover this stored basis root from the unchanged original corpus
        // using the existing native-coefficient provenance composition.
        let weights = provenance
            .compose(&[(source_id, context.one())], &context.one(), |scale| {
                Ok(scale.clone().into())
            })
            .unwrap();
        let basis_row = &cold.basis()[source_id];
        for column in original.iter().chain(std::iter::once(basis_row)).flatten() {
            let actual =
                original
                    .iter()
                    .zip(&weights)
                    .fold(context.zero(), |sum, (row, weight)| {
                        let coefficient = row
                            .iter()
                            .find(|term| term.integral == column.integral)
                            .map_or_else(|| context.zero(), |term| term.coefficient.clone().into());
                        &sum + &(weight * &coefficient)
                    });
            let expected = basis_row
                .iter()
                .find(|term| term.integral == column.integral)
                .map_or_else(|| context.zero(), |term| term.coefficient.clone().into());
            assert_eq!(actual, expected);
        }
        assert_eq!(solver.basis(), stored_basis);
        assert_eq!(system.rows(), original);
    }
}

#[test]
fn modular_source_ids_remain_stored_ordinals_not_arrival_positions() {
    let context = CoefficientContext::new(["n"]);
    // Keep an intentionally dependent diagnostic basis, as in observation
    // tests. At fixed n=1, R=I(2)+I(1) and S=I(2)+I(0) solve I(1)=I(0).
    let basis = vec![
        row(&context, &[1, 0]),
        row(&context, &[1, 0]),
        row(&context, &[1, -1]),
    ];
    let system = SourceSystem::new(basis.clone(), [0]).unwrap();
    let solver = SectorSolver {
        system: &system,
        basis: basis.clone(),
        order: IntegralOrder::new([true], [false]),
        config: SectorConfig::default(),
    };
    let case = CoordinateCase::new([Some(1)]).unwrap();
    let baseline = solver.solve_case(case, depth_zero()).unwrap();
    for (order, expected_ids, visited) in [
        ([0, 1, 2], [0, 2], 3),
        ([2, 1, 0], [2, 1], 2),
        ([1, 2, 0], [1, 2], 2),
    ] {
        let candidate = solver
            .solve_case_with_source_order_and_observer(case, depth_zero(), &order, |_| {})
            .unwrap();
        let repeated = solver
            .solve_case_with_source_order_and_observer(case, depth_zero(), &order, |_| {})
            .unwrap();
        assert_same_candidate(&candidate, &repeated);
        if order == [0, 1, 2] {
            assert_same_candidate(&candidate, &baseline);
        }
        assert!(!candidate.stats.direct_hit);
        assert_eq!(candidate.stats.rows, visited);
        assert_eq!(candidate.stats.exact_trace_rows, 2);
        assert_eq!(
            candidate
                .sources
                .iter()
                .map(|source| source.basis_row)
                .collect::<Vec<_>>(),
            expected_ids
        );
        assert!(
            candidate
                .sources
                .iter()
                .all(|source| source.seed.integral == case.integral())
        );
        assert_eq!(candidate.target, Integral::numeric([1]).unwrap());
        assert_eq!(candidate.rhs.len(), 1);
        assert_eq!(candidate.rhs[0].integral, Integral::numeric([0]).unwrap());
        assert_eq!(candidate.rhs[0].coefficient, context.one());
        assert_regenerated_equation(&solver, &candidate);
    }
    assert_eq!(solver.basis(), basis);
    assert_eq!(system.rows(), basis);
}

#[test]
fn every_schedule_visits_every_basis_row_at_each_seed_until_the_same_bound() {
    let context = CoefficientContext::new(["n"]);
    let system = SourceSystem::new(
        vec![
            row(&context, &[3]),
            row(&context, &[4]),
            row(&context, &[5]),
        ],
        [0],
    )
    .unwrap();
    let solver = SectorSolver::new(&system, [true], SectorConfig::default()).unwrap();
    for order in [[0, 1, 2], [2, 1, 0], [1, 2, 0]] {
        let mut milestones = Vec::new();
        let result = solver.solve_case_with_source_order_and_observer(
            CoordinateCase::new([Some(1)]).unwrap(),
            SearchOptions {
                max_depth: Some(2),
                ..Default::default()
            },
            &order,
            |event| match event {
                SearchEvent::DiscoveryProgress {
                    depth, seeds, rows, ..
                } => {
                    milestones.push((depth, seeds, rows));
                }
                _ => panic!("bounded miss must not enter exact materialization"),
            },
        );
        assert!(matches!(
            result,
            Err(SolverError::SearchExhausted { depth: 2, rows: 9 })
        ));
        assert_eq!(milestones, [(0, 1, 0), (1, 2, 3), (2, 3, 6)]);
    }
}

#[test]
fn empty_basis_accepts_only_the_empty_schedule_without_panicking() {
    let context = CoefficientContext::new(["n"]);
    let system = SourceSystem::new(vec![row(&context, &[0])], [0]).unwrap();
    let solver = SectorSolver {
        system: &system,
        basis: Vec::new(),
        order: IntegralOrder::new([true], [false]),
        config: SectorConfig::default(),
    };
    let mut observed = 0;
    let result = solver.solve_case_with_source_order_and_observer(
        CoordinateCase::generic(),
        depth_zero(),
        &[],
        |_| observed += 1,
    );
    assert!(matches!(
        result,
        Err(SolverError::SearchExhausted { depth: 0, rows: 0 })
    ));
    assert_eq!(observed, 1);
    let result = solver.solve_case_with_source_order_and_observer(
        CoordinateCase::generic(),
        depth_zero(),
        &[0],
        |_| panic!("invalid order"),
    );
    assert!(matches!(result, Err(SolverError::InvalidInput(_))));
}
