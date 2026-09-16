use super::super::{Atom, RestrictionCache, WorkExhausted};
use super::*;
use crate::algebra::CoefficientContext;
use crate::foundry::artifact::source_port::predicate_cover::{
    Clause, PredicateCoverError, PredicateCoverLimits, TraversalWork, check_valuations,
};
use symbolica::prelude::Integer;

#[test]
fn real_cover_traversal_refines_its_complement_and_preserves_budget_errors() {
    with_atoms::<2, _>(&["n0-n1", "n1", "n1+1"], |atoms| {
        let assignments = [Some(true), Some(false), Some(false)];
        let sector = [false; 2];
        for (outside_start, closed) in [(2, true), (3, false)] {
            let clauses = [Clause {
                domain: LatticeBox::try_new([outside_start, 0], [None; 2]).unwrap(),
                literals: vec![],
            }];
            let mut work = TraversalWork::new(&sector, atoms);
            let result = check_valuations(
                &sector,
                atoms,
                &clauses,
                &[],
                &mut assignments.clone(),
                &mut work,
                PredicateCoverLimits::default(),
            );
            if closed {
                assert_eq!(result, Ok(()));
            } else {
                assert!(matches!(result, Err(PredicateCoverError::Uncovered { .. })));
            }
            let mut work = TraversalWork::with_work_limit(&sector, atoms, 0);
            assert_eq!(
                check_valuations(
                    &sector,
                    atoms,
                    &clauses,
                    &[],
                    &mut assignments.clone(),
                    &mut work,
                    PredicateCoverLimits::default(),
                ),
                Err(PredicateCoverError::Budget(
                    "native affine literal consistency"
                ))
            );
        }
    });
}

fn with_atoms<const N: usize, R>(expressions: &[&str], action: impl FnOnce(&[Atom<'_>]) -> R) -> R {
    let names: Vec<_> = std::iter::once("d".to_owned())
        .chain((0..N).map(|n| format!("n{n}")))
        .collect();
    let context = CoefficientContext::new(names);
    let equations: Vec<_> = expressions
        .iter()
        .map(|s| context.coefficient_fixture(s).numerator)
        .collect();
    let indices: Vec<_> = (1..=N).collect();
    let atoms: Vec<_> = equations
        .iter()
        .map(|equation| Atom {
            equation,
            indices: &indices,
        })
        .collect();
    action(&atoms)
}

const X_ATOMS: [&str; 12] = [
    "-1-n6+n0",
    "-4-4*n6-n4+2*n2",
    "1+n6-2*n2+2*n0",
    "1+n6+n4",
    "-n4+n2",
    "4+4*n6+n4",
    "6+4*n6+n4",
    "-1-n6+2*n2",
    "1-n6+2*n2",
    "4-n4+2*n2",
    "-n4+2*n2",
    "-n2+n0",
];
const X_ASSIGNMENTS: [Option<bool>; 12] = [
    Some(true),
    Some(true),
    Some(false),
    Some(false),
    Some(false),
    Some(false),
    Some(false),
    None,
    None,
    Some(false),
    Some(false),
    Some(false),
];
const X_SECTOR: [bool; 10] = [
    false, true, false, true, false, false, false, true, true, false,
];

fn x_box(width: u64) -> LatticeBox {
    LatticeBox::try_new(
        [2, 0, 0, 0, 0, 0, 1, 0, 0, 0],
        [
            None,
            Some(0),
            Some(width),
            Some(0),
            None,
            Some(0),
            None,
            Some(0),
            Some(0),
            Some(0),
        ],
    )
    .unwrap()
}

#[test]
fn captured_infinite_complement_is_empty_on_both_complete_integer_faces() {
    with_atoms::<10, _>(&X_ATOMS, |atoms| {
        let mut cache = RestrictionCache::new(&X_SECTOR, atoms);
        assert_eq!(
            cache.contradicts_without_refinement(&x_box(1), &X_ASSIGNMENTS),
            Ok(false)
        );
        assert_eq!(cache.contradicts(&x_box(1), &X_ASSIGNMENTS), Ok(true));
        // A genuinely admissible point survives the widened finite interval.
        let witness = [-2, 1, -5, 1, -2, 0, -3, 1, 1, 0];
        for (atom, assignment) in atoms.iter().zip(X_ASSIGNMENTS) {
            let Some(zero) = assignment else { continue };
            let mut value = atom.equation.clone();
            for (&index, power) in atom.indices.iter().zip(witness) {
                value = value.replace(index, &Integer::from(power));
            }
            assert!(value.is_constant());
            assert_eq!(value.is_zero(), zero);
        }
        assert_eq!(cache.contradicts(&x_box(5), &X_ASSIGNMENTS), Ok(false));
    });
}

#[test]
fn all_faces_share_one_work_budget_and_cached_proofs_remain_reusable() {
    with_atoms::<10, _>(&X_ATOMS, |atoms| {
        let mut measured = RestrictionCache::new(&X_SECTOR, atoms);
        assert_eq!(measured.contradicts(&x_box(1), &X_ASSIGNMENTS), Ok(true));
        let cost = measured.statistics.native_work;
        assert!(cost > 1);
        let mut enough = RestrictionCache::with_work_limit(&X_SECTOR, atoms, cost);
        assert_eq!(enough.contradicts(&x_box(1), &X_ASSIGNMENTS), Ok(true));
        assert_eq!(enough.budget.remaining, 0);
        assert_eq!(enough.contradicts(&x_box(1), &X_ASSIGNMENTS), Ok(true));
        assert_eq!(enough.statistics.native_work, cost);
        let mut short = RestrictionCache::with_work_limit(&X_SECTOR, atoms, cost - 1);
        assert_eq!(
            short.contradicts(&x_box(1), &X_ASSIGNMENTS),
            Err(WorkExhausted)
        );
        let mut uncached = RestrictionCache::new(&X_SECTOR, atoms);
        uncached.limits.boxes = 0;
        assert_eq!(uncached.contradicts(&x_box(1), &X_ASSIGNMENTS), Ok(true));
        let mut zero = RestrictionCache::with_work_limit(&X_SECTOR, atoms, 0);
        assert_eq!(
            zero.contradicts(&x_box(1), &X_ASSIGNMENTS),
            Err(WorkExhausted)
        );
        assert_eq!(zero.statistics.native_work, 0);
    });
}

#[test]
fn finite_refinement_is_exhaustive_not_endpoint_sampling() {
    with_atoms::<2, _>(&["n0-n1", "n1", "n1+2"], |atoms| {
        let assignments = [Some(true), Some(false), Some(false)];
        let mut cache = RestrictionCache::new(&[false; 2], atoms);
        for endpoint in [0, 2] {
            let face = LatticeBox::try_new([endpoint, 0], [Some(endpoint), None]).unwrap();
            assert_eq!(cache.contradicts(&face, &assignments), Ok(true));
        }
        let whole = LatticeBox::try_new([0, 0], [Some(2), None]).unwrap();
        assert_eq!(cache.contradicts(&whole, &assignments), Ok(false)); // n0=n1=-1
    });
}

#[test]
fn single_axis_refinement_never_recurses_or_selects_irrelevant_axes() {
    with_atoms::<3, _>(&["n0-n1", "n1+n2", "n1+n2+1", "n1+n2+2"], |atoms| {
        let cell = LatticeBox::try_new([0; 3], [Some(1), None, Some(1)]).unwrap();
        let assignments = [Some(true), Some(false), Some(false), Some(false)];
        // All four (n0,n2) corners contradict, but proving either entire n0
        // face would require another split, which this lane never performs.
        assert_eq!(
            RestrictionCache::new(&[false; 3], atoms).contradicts(&cell, &assignments),
            Ok(false)
        );
    });
    with_atoms::<3, _>(&["n1-n2", "n2", "n2+1"], |atoms| {
        let assignments = [Some(true), Some(false), Some(false)];
        // n0 has a shorter interval but does not occur; n1 must be selected.
        let cell = LatticeBox::try_new([0; 3], [Some(1), Some(1), None]).unwrap();
        assert_eq!(
            RestrictionCache::new(&[false; 3], atoms).contradicts(&cell, &assignments),
            Ok(true)
        );
        for upper in [None, Some(8)] {
            let cell = LatticeBox::try_new([0; 3], [Some(1), upper, None]).unwrap();
            assert_eq!(
                RestrictionCache::new(&[false; 3], atoms).contradicts(&cell, &assignments),
                Ok(false)
            );
        }
    });
}

#[test]
fn original_unsupported_support_is_not_hidden_by_singleton_children() {
    for equation in ["n0*n1", "d*n0+n1"] {
        with_atoms::<2, _>(&[equation, "n1", "n1+1"], |atoms| {
            let cell = LatticeBox::try_new([0; 2], [Some(1), None]).unwrap();
            assert_eq!(
                RestrictionCache::new(&[false; 2], atoms)
                    .contradicts(&cell, &[Some(true), Some(false), Some(false)]),
                Ok(false)
            );
        });
    }
}

#[test]
fn largest_local_endpoint_keeps_native_positive_and_negative_coordinates_exact() {
    for (sector, constants) in [
        (
            [true, false],
            ["n1-18446744073709551615", "n1-18446744073709551616"],
        ),
        (
            [false, false],
            ["n1+18446744073709551614", "n1+18446744073709551615"],
        ),
    ] {
        with_atoms::<2, _>(&["n0-n1", constants[0], constants[1]], |atoms| {
            let cell = LatticeBox::try_new([u64::MAX - 1, 0], [Some(u64::MAX), None]).unwrap();
            assert_eq!(
                RestrictionCache::new(&sector, atoms)
                    .contradicts(&cell, &[Some(true), Some(false), Some(false)]),
                Ok(true)
            );
        });
    }
}

#[test]
fn all_eight_faces_are_allowed_but_a_ninth_is_not_sampled() {
    let mut expressions = vec!["n0-n1".to_owned()];
    expressions.extend((0..8).map(|n| format!("n1+{n}")));
    let borrowed: Vec<_> = expressions.iter().map(String::as_str).collect();
    with_atoms::<2, _>(&borrowed, |atoms| {
        let mut assignments = vec![Some(false); atoms.len()];
        assignments[0] = Some(true);
        for (upper, expected, faces) in [(7, true, 8), (8, false, 0)] {
            let cell = LatticeBox::try_new([0; 2], [Some(upper), None]).unwrap();
            let mut cache = RestrictionCache::new(&[false; 2], atoms);
            assert_eq!(cache.contradicts(&cell, &assignments), Ok(expected));
            assert_eq!(cache.statistics.finite_axis_faces, faces);
        }
    });
}

#[test]
fn aggregate_shape_caps_reject_before_any_face_or_native_work() {
    for matrix_cap in [true, false] {
        let expressions = if matrix_cap {
            vec!["n0-n1"; 8]
        } else {
            std::iter::once("n0-n1")
                .chain(std::iter::repeat_n("n1+n2", 31))
                .collect()
        };
        with_atoms::<1024, _>(&expressions, |atoms| {
            let mut assignments = vec![Some(false); atoms.len()];
            if matrix_cap {
                assignments[..7].fill(Some(true));
            } else {
                assignments[0] = Some(true);
            }
            let sector = [false; 1024];
            let mut upper = [None; 1024];
            upper[0] = Some(7);
            let cell = LatticeBox::try_new([0; 1024], upper).unwrap();
            let mut cache = RestrictionCache::new(&sector, atoms);
            assert_eq!(
                cache.contradicts_finite_axis(&cell, &assignments),
                Err(WorkExhausted)
            );
            assert_eq!(cache.statistics.finite_axis_refinements, 0);
            assert_eq!(cache.statistics.finite_axis_faces, 0);
            assert_eq!(cache.statistics.native_work, 0);
            assert_eq!(cache.statistics.classification_materializations, 0);
            assert_eq!(
                cache.statistics.exhaustion.unwrap().local_cap,
                Some(if matrix_cap {
                    LocalCap::MatrixCells
                } else {
                    LocalCap::PolynomialCells
                })
            );
        });
    }
}
