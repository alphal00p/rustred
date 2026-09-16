use super::*;
use crate::algebra::CoefficientContext;
use crate::foundry::artifact::source_port::predicate_cover::{
    PredicateCoverError, PredicateCoverLimits, PredicateCoveragePiece, certify_predicate_cover,
};
use crate::solver::{AffineCase, AffineIntersection, CoordinateCase};

fn contradiction(
    context: &CoefficientContext,
    equation: &str,
    sector: &[bool],
    cell: &LatticeBox,
    assignment: Option<bool>,
) -> bool {
    let equation = context.coefficient_fixture(equation).numerator;
    let indices: Vec<_> = (1..=sector.len()).collect();
    RestrictionCache::new(
        sector,
        &[Atom {
            indices: &indices,
            equation: &equation,
        }],
    )
    .contradicts(cell, &[assignment])
    .unwrap()
}

#[test]
fn only_exact_singletons_determine_literal_truth_with_unrelated_infinite_axes() {
    let context = CoefficientContext::new(["d", "a", "b", "c"]);
    let equal = LatticeBox::try_new([0; 3], [Some(0), Some(0), None]).unwrap();
    assert!(contradiction(
        &context,
        "a-b",
        &[false; 3],
        &equal,
        Some(false)
    ));
    assert!(!contradiction(
        &context,
        "a-b",
        &[false; 3],
        &equal,
        Some(true)
    ));
    assert!(!contradiction(&context, "a-b", &[false; 3], &equal, None));
    let unequal = LatticeBox::try_new([0, 1, 0], [Some(0), Some(1), None]).unwrap();
    assert!(contradiction(
        &context,
        "a-b",
        &[false; 3],
        &unequal,
        Some(true)
    ));
    assert!(!contradiction(
        &context,
        "a-b",
        &[false; 3],
        &unequal,
        Some(false)
    ));
    for upper in [Some(1), None] {
        let variable = LatticeBox::try_new([0; 3], [Some(0), upper, None]).unwrap();
        for assignment in [Some(false), Some(true)] {
            assert!(!contradiction(
                &context,
                "a-b",
                &[false; 3],
                &variable,
                assignment
            ));
        }
    }
}

#[test]
fn native_integer_endpoints_preserve_positive_plus_one_and_negative_u64_max() {
    let context = CoefficientContext::new(["d", "a", "b", "c"]);
    let cell = LatticeBox::try_new(
        [u64::MAX, u64::MAX, 0],
        [Some(u64::MAX), Some(u64::MAX), None],
    )
    .unwrap();
    assert!(contradiction(
        &context,
        "a+b-1",
        &[true, false, false],
        &cell,
        Some(false)
    ));
    assert!(!contradiction(
        &context,
        "a+b-1",
        &[true, false, false],
        &cell,
        Some(true)
    ));
    assert!(contradiction(
        &context,
        "a-b",
        &[false; 3],
        &cell,
        Some(false)
    ));
    assert!(contradiction(
        &context,
        "a+b",
        &[true, false, false],
        &cell,
        Some(true)
    ));
}

#[test]
fn substitution_uses_original_axis_bindings_not_native_variable_positions() {
    let context = CoefficientContext::new(["d", "b", "c", "a"]);
    let equation = context.coefficient_fixture("a-2*b").numerator;
    let cell = LatticeBox::try_new([2, 1, 0], [Some(2), Some(1), None]).unwrap();
    let atoms = [Atom {
        indices: &[3, 1, 2],
        equation: &equation,
    }];
    assert!(
        RestrictionCache::new(&[false; 3], &atoms)
            .contradicts(&cell, &[Some(false)])
            .unwrap()
    );
    assert!(
        !RestrictionCache::new(&[false; 3], &atoms)
            .contradicts(&cell, &[Some(true)])
            .unwrap()
    );
}

#[test]
fn malformed_maps_stay_unknown_and_budget_exhaustion_is_explicit() {
    let context = CoefficientContext::new(["d", "a", "b", "c"]);
    let cell = LatticeBox::try_new([0; 3], [Some(0), Some(0), None]).unwrap();
    let equation = context.coefficient_fixture("a-b").numerator;
    for indices in [&[1, 1, 3][..], &[1, 2, 4][..], &[0, 2, 3][..], &[1, 2][..]] {
        assert!(
            !RestrictionCache::new(
                &[false; 3],
                &[Atom {
                    indices,
                    equation: &equation
                }],
            )
            .contradicts(&cell, &[Some(false)])
            .unwrap()
        );
    }
    for equation in ["d*a-b", "a^2-b", "a*b"] {
        assert!(!contradiction(
            &context,
            equation,
            &[false; 3],
            &cell,
            Some(false)
        ));
    }
    let atoms = [Atom {
        indices: &[1, 2, 3],
        equation: &equation,
    }];
    let mut budget = RestrictionCache::new(&[false; 3], &atoms);
    budget.budget.remaining = 1;
    assert_eq!(
        budget.contradicts(&cell, &[Some(false)]),
        Err(WorkExhausted)
    );
    assert_eq!(budget.budget.remaining, 0);
    assert_eq!(
        budget.contradicts(&cell, &[Some(false)]),
        Err(WorkExhausted)
    );
    assert!(
        !RestrictionCache::new(&[false; 3], &atoms)
            .contradicts(&cell, &[])
            .unwrap()
    );
    let mut malformed = equation.clone();
    malformed.exponents.pop();
    assert!(
        !RestrictionCache::new(
            &[false; 3],
            &[Atom {
                indices: &[1, 2, 3],
                equation: &malformed
            }],
        )
        .contradicts(&cell, &[Some(false)])
        .unwrap()
    );
}

#[test]
fn the_observed_first_failed_literal_is_impossible_at_its_exact_index_point() {
    let context = CoefficientContext::new([
        "d", "n0", "n1", "n2", "n3", "n4", "n5", "n6", "n7", "n8", "n9",
    ]);
    let sector = [
        false, true, true, false, true, false, true, true, false, false,
    ];
    let lower = [1, 0, 0, 0, 0, 1, 0, 0, 2, 0];
    let cell = LatticeBox::try_new(lower, lower.map(Some)).unwrap();
    for equation in ["1-n9+n5", "-1-n8+n5+2*n3", "-1-n8-n5+2*n0", "3+n8+n5"] {
        assert!(contradiction(
            &context,
            equation,
            &sector,
            &cell,
            Some(false)
        ));
        assert!(!contradiction(
            &context,
            equation,
            &sector,
            &cell,
            Some(true)
        ));
    }
}

#[test]
fn whole_cover_discharges_false_atom_on_face_but_keeps_a_genuine_missing_point() {
    let context = CoefficientContext::new(["d", "a", "b", "c"]);
    let sector = [false; 3];
    let AffineIntersection::Affine(case) = AffineCase::from_coordinate(
        &CoordinateCase::generic(),
        &[context.coefficient_fixture("a-b").numerator],
        &[1, 2, 3],
        &sector,
    )
    .unwrap() else {
        panic!("expected coupled equality")
    };
    let domain = super::super::super::AffineApplicationDomain::from_case(&case, &sector).unwrap();
    let outside = [
        LatticeBox::try_new([1, 0, 0], [None; 3]).unwrap(),
        LatticeBox::try_new([0, 1, 0], [Some(0), None, None]).unwrap(),
    ];
    let face = [LatticeBox::try_new([0; 3], [Some(0), Some(0), None]).unwrap()];
    let owners = [
        PredicateCoveragePiece {
            boxes: &outside,
            affine_target: None,
            affine_exclusions: &[],
        },
        PredicateCoveragePiece {
            boxes: &face,
            affine_target: Some(&domain),
            affine_exclusions: &[],
        },
    ];
    assert!(
        certify_predicate_cover(&sector, &owners, &[], PredicateCoverLimits::default()).is_ok()
    );
    let missing_point = [LatticeBox::try_new([0, 0, 1], [Some(0), Some(0), None]).unwrap()];
    let owners = [
        PredicateCoveragePiece {
            boxes: &outside,
            affine_target: None,
            affine_exclusions: &[],
        },
        PredicateCoveragePiece {
            boxes: &missing_point,
            affine_target: Some(&domain),
            affine_exclusions: &[],
        },
    ];
    assert!(matches!(
        certify_predicate_cover(&sector, &owners, &[], PredicateCoverLimits::default()),
        Err(PredicateCoverError::Uncovered { .. })
    ));
    let terminal = [LatticeBox::try_new([0; 3], [Some(0); 3]).unwrap()];
    assert!(
        certify_predicate_cover(&sector, &owners, &terminal, PredicateCoverLimits::default())
            .is_ok()
    );
}
