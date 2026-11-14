//! sym-7master: exact symmetry rows in the SYMBOLIC case search (case-stabilising elements only).

use crate::algebra::CoefficientContext;

use super::*;
use crate::solver::{CoordinateCase, Power, SYMMETRY_ROW};

/// A toy 3-slot system whose only source row raises the two fixed lines (`I(y+e0) + I(y+e1)`): at depth 0 it never
/// relates `I(2,1,c)` and `I(1,2,c)` (two "masters" of the restricted search); the line swap (0 1), which fixes the
/// free slot 2, does.
fn toy() -> (CoefficientContext, SourceSystem<3>) {
    let context = CoefficientContext::new(["a", "b", "c"]);
    let system = SourceSystem::new(
        vec![vec![
            Term {
                integral: Integral::symbolic([1, 0, 0]).unwrap(),
                coefficient: context.one().numerator,
            },
            Term {
                integral: Integral::symbolic([0, 1, 0]).unwrap(),
                coefficient: context.one().numerator,
            },
        ]],
        [0, 1, 2],
    )
    .unwrap();
    (context, system)
}

fn depth_zero() -> SearchOptions {
    SearchOptions {
        max_depth: Some(0),
        ..Default::default()
    }
}

const SWAP01: [Option<(usize, bool)>; 3] = [Some((1, false)), Some((0, false)), Some((2, false))];
const SWAP02: [Option<(usize, bool)>; 3] = [Some((2, false)), Some((1, false)), Some((0, false))];

fn with_actions(actions: &[[Option<(usize, bool)>; 3]]) -> SectorConfig<3> {
    SectorConfig {
        case_symmetry: Some(
            actions
                .iter()
                .map(|a| a.to_vec())
                .collect::<Vec<_>>()
                .into(),
        ),
        ..Default::default()
    }
}

fn key(values: [i16; 3], symbolic: [bool; 3]) -> Integral<3> {
    Integral::new(std::array::from_fn(|i| {
        Power::new(symbolic[i], values[i]).unwrap()
    }))
}

#[test]
fn stabiliser_row_removes_one_of_two_symmetric_symbolic_masters() {
    let (_, system) = toy();
    let a = CoordinateCase::new([Some(2), Some(1), None]).unwrap();
    let b = CoordinateCase::new([Some(1), Some(2), None]).unwrap();
    // IBP only: neither case is reachable at depth 0 (two independent keys)
    let off = SectorSolver::new(&system, [true; 3], SectorConfig::default()).unwrap();
    for case in [a, b] {
        assert!(
            matches!(
                off.solve_case(case, depth_zero()),
                Err(SolverError::SearchExhausted { .. })
            ),
            "{case:?}"
        );
    }
    // with the stabilising swap (0 1): exactly one of the two is written onto the other, by one symmetry row
    let on = SectorSolver::new(&system, [true; 3], with_actions(&[SWAP01])).unwrap();
    let results: Vec<_> = [a, b]
        .iter()
        .map(|c| on.solve_case(*c, depth_zero()))
        .collect();
    let solved: Vec<&RuleCandidate<3>> = results.iter().filter_map(|r| r.as_ref().ok()).collect();
    assert_eq!(solved.len(), 1, "{results:?}");
    assert!(
        results
            .iter()
            .any(|r| matches!(r, Err(SolverError::SearchExhausted { .. })))
    );
    let rule = solved[0];
    assert!(
        !rule.stats.direct_hit,
        "symmetry rows go through the modular first-hit path"
    );
    assert_eq!(rule.sources.len(), 1);
    assert_eq!(rule.sources[0].basis_row, SYMMETRY_ROW);
    assert_eq!(rule.rhs.len(), 1);
    let ka = key([2, 1, 0], [false, false, true]);
    let kb = key([1, 2, 0], [false, false, true]);
    let (t, r) = if rule.target == ka {
        (ka, kb)
    } else {
        (kb, ka)
    };
    assert_eq!(rule.target, t);
    assert_eq!(rule.rhs[0].integral, r);
    // the written image is lower than the target in the node order (descent unchanged)
    assert_eq!(
        on.ordering().compare(&rule.target, &rule.rhs[0].integral),
        Ordering::Less
    );
}

#[test]
fn an_element_moving_a_free_slot_adds_no_row() {
    let (_, system) = toy();
    let a = CoordinateCase::new([Some(2), Some(1), None]).unwrap();
    // (0 2) moves the free slot 2: not a stabiliser of the case, so the search is exactly the IBP-only one
    assert!(case_stabiliser(&[SWAP02.to_vec()], &Case::from(a)).is_empty());
    assert_eq!(
        case_stabiliser(&[SWAP01.to_vec(), SWAP02.to_vec()], &Case::from(a)),
        vec![0]
    );
    let neg = SectorSolver::new(&system, [true; 3], with_actions(&[SWAP02])).unwrap();
    let off = SectorSolver::new(&system, [true; 3], SectorConfig::default()).unwrap();
    for case in [a, CoordinateCase::new([Some(1), Some(2), None]).unwrap()] {
        let (x, y) = (
            neg.solve_case(case, depth_zero()),
            off.solve_case(case, depth_zero()),
        );
        assert!(
            matches!(x, Err(SolverError::SearchExhausted { .. })),
            "{x:?}"
        );
        assert_eq!(format!("{x:?}"), format!("{y:?}"));
    }
    // a fully fixed point: every element stabilises it (rows wherever the action is exact on the seed)
    let point = CoordinateCase::new([Some(2), Some(1), Some(1)]).unwrap();
    assert_eq!(
        case_stabiliser(&[SWAP01.to_vec(), SWAP02.to_vec()], &Case::from(point)),
        vec![0, 1]
    );
}

#[test]
fn per_point_search_identifies_a_numeric_point_with_its_image() {
    let (_, system) = toy();
    let a = CoordinateCase::new([Some(2), Some(1), Some(1)]).unwrap();
    let b = CoordinateCase::new([Some(1), Some(2), Some(1)]).unwrap();
    let off = SectorSolver::new(&system, [true; 3], SectorConfig::default()).unwrap();
    let on = SectorSolver::new(&system, [true; 3], with_actions(&[SWAP01])).unwrap();
    for case in [a, b] {
        assert!(matches!(
            off.solve_case(case, depth_zero()),
            Err(SolverError::SearchExhausted { .. })
        ));
    }
    let solved: Vec<RuleCandidate<3>> = [a, b]
        .iter()
        .filter_map(|c| on.solve_case(*c, depth_zero()).ok())
        .collect();
    assert_eq!(solved.len(), 1);
    assert_eq!(solved[0].sources[0].basis_row, SYMMETRY_ROW);
    assert_eq!(solved[0].rhs.len(), 1);
}

#[test]
fn stabiliser_images_keep_symbolic_powers_and_refuse_unmapped_or_subsector_seeds() {
    let sector = [true; 3];
    let y = key([2, 1, -1], [false, false, true]);
    assert_eq!(
        stabiliser_image(&SWAP01, &y, &sector),
        Some((key([1, 2, -1], [false, false, true]), false))
    );
    // a nonzero numeric slot without image
    let partial = [None, Some((0, false)), Some((2, false))];
    assert_eq!(stabiliser_image(&partial, &y, &sector), None);
    // a numeric sector line below 1 (subsector seed)
    assert_eq!(
        stabiliser_image(&SWAP01, &key([0, 1, 0], [false, false, true]), &sector),
        None
    );
    // an odd flipped power gives the sign
    let flip = [Some((1, true)), Some((0, false)), Some((2, false))];
    assert_eq!(
        stabiliser_image(&flip, &key([3, 1, 0], [false, false, true]), &sector),
        Some((key([1, 3, 0], [false, false, true]), true))
    );
}
