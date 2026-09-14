use crate::algebra::CoefficientContext;

use super::*;
use crate::solver::{CoordinateCase, instantiate::canonicalize};

#[test]
fn affine_search_uses_transverse_sources_and_replays_exactly() {
    let context = CoefficientContext::new(["a", "b"]);
    let system = SourceSystem::new(
        vec![vec![
            Term {
                integral: Integral::symbolic([-1, 0]).unwrap(),
                coefficient: context.coefficient_fixture("a-b").numerator,
            },
            Term {
                integral: Integral::symbolic([-2, -1]).unwrap(),
                coefficient: context.one().numerator,
            },
        ]],
        [0, 1],
    )
    .unwrap();
    let original = system.rows().to_vec();
    let solver = SectorSolver::new(&system, [true; 2], SectorConfig::default()).unwrap();
    let case = Case::generic()
        .intersect(
            &[context.coefficient_fixture("a-b").numerator],
            &[0, 1],
            &[true; 2],
        )
        .unwrap()
        .unwrap();
    let candidate = solver
        .solve_case(
            case.clone(),
            SearchOptions {
                max_depth: Some(1),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(candidate.case, case);
    assert_eq!(candidate.target, Integral::symbolic([0, 0]).unwrap());
    assert_eq!(candidate.rhs.len(), 1);
    assert_eq!(
        candidate.rhs[0].integral,
        Integral::symbolic([-1, -1]).unwrap()
    );
    assert_eq!(candidate.rhs[0].coefficient, context.integer(-1));
    assert!(candidate.stats.direct_hit);
    assert_eq!(candidate.sources.len(), 1);
    let origin = candidate.sources[0];
    assert_ne!(origin.seed.shifts[0], origin.seed.shifts[1]);
    let replay = instantiate(
        &solver.basis()[origin.basis_row],
        &origin.seed,
        system.index_variables(),
        system.fixed(),
        solver.ordering(),
        &[],
        case.affine(),
    )
    .unwrap();
    assert!(case.matches(&replay[0].integral));
    let (target, rhs) = canonicalize(replay, system.index_variables()).unwrap();
    assert_eq!(target, candidate.target);
    assert_eq!(rhs, candidate.rhs);
    assert_eq!(system.rows(), original);
    assert_eq!(
        solver.ordering().compare(&target, &rhs[0].integral),
        Ordering::Less
    );
}

#[test]
fn affine_projection_preserves_distinct_physical_columns() {
    let context = CoefficientContext::new(["a", "b"]);
    let case = Case::generic()
        .intersect(
            &[context.coefficient_fixture("a-b").numerator],
            &[0, 1],
            &[true; 2],
        )
        .unwrap()
        .unwrap();
    let source = vec![
        Term {
            integral: Integral::symbolic([1, 0]).unwrap(),
            coefficient: context.one().numerator,
        },
        Term {
            integral: Integral::symbolic([0, 1]).unwrap(),
            coefficient: context.integer(-1).numerator,
        },
    ];
    let order = IntegralOrder::new([true; 2], [false; 2]);
    let row = instantiate(
        &source,
        &Seed {
            integral: CoordinateCase::generic().integral(),
            shifts: [0; 2],
        },
        &[0, 1],
        &[None; 2],
        &order,
        &[],
        case.affine(),
    )
    .unwrap();
    assert_eq!(row.len(), 2);
    assert_ne!(row[0].integral, row[1].integral);
    assert!(row.iter().all(|term| !case.matches(&term.integral)));
}

#[test]
fn affine_search_rejects_mismatched_index_map_before_discovery() {
    let context = CoefficientContext::new(["a", "b"]);
    let system = SourceSystem::new(
        vec![vec![Term {
            integral: Integral::symbolic([0, 0]).unwrap(),
            coefficient: context.one().numerator,
        }]],
        [0, 1],
    )
    .unwrap();
    let case = Case::generic()
        .intersect(
            &[context.coefficient_fixture("a-b").numerator],
            &[1, 0],
            &[true; 2],
        )
        .unwrap()
        .unwrap();
    let solver = SectorSolver::new(&system, [true; 2], SectorConfig::default()).unwrap();
    assert!(matches!(
        solver.solve_case(case, SearchOptions::default()),
        Err(SolverError::InvalidInput(_))
    ));
}
