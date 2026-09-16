use super::super::RestrictionCache;
use super::*;
use crate::algebra::CoefficientContext;

fn decide(
    expressions: &[&str],
    assignments: &[Option<bool>],
    cell: &LatticeBox,
    sector: &[bool; 3],
) -> bool {
    let context = CoefficientContext::new(["d", "a", "b", "c"]);
    let equations: Vec<_> = expressions
        .iter()
        .map(|s| context.coefficient_fixture(s).numerator)
        .collect();
    let atoms: Vec<_> = equations
        .iter()
        .map(|equation| Atom {
            indices: &[1, 2, 3],
            equation,
        })
        .collect();
    RestrictionCache::new(sector, &atoms)
        .contradicts(cell, assignments)
        .unwrap()
}

#[test]
fn observed_proportional_literals_become_contradictory_on_the_fixed_face() {
    let face = LatticeBox::try_new([1, 0, 0], [Some(1), None, None]).unwrap();
    assert!(decide(
        &["-1-c-b+2*a", "3+c+b"],
        &[Some(true), Some(false)],
        &face,
        &[false; 3]
    ));
    let neighbor = LatticeBox::try_new([2, 0, 0], [Some(2), None, None]).unwrap();
    assert!(!decide(
        &["-1-c-b+2*a", "3+c+b"],
        &[Some(true), Some(false)],
        &neighbor,
        &[false; 3]
    ));
}

#[test]
fn observed_fg_literals_match_after_fixing_only_the_original_coordinate() {
    // Actual FG witness: n3=-1, n5<=-2, n8<=-1. Both remaining axes
    // are infinite, so a lower-corner test would not establish this proof.
    let face = LatticeBox::try_new([1, 2, 1], [Some(1), None, None]).unwrap();
    assert!(decide(
        &["-1-c+b+2*a", "-3-c+b"],
        &[Some(true), Some(false)],
        &face,
        &[false; 3]
    ));
    let neighbor = LatticeBox::try_new([2, 2, 1], [Some(2), None, None]).unwrap();
    assert!(!decide(
        &["-1-c+b+2*a", "-3-c+b"],
        &[Some(true), Some(false)],
        &neighbor,
        &[false; 3]
    ));
}

#[test]
fn full_true_span_is_used_without_any_singletons_or_sampling() {
    let cell = LatticeBox::try_new([0; 3], [None; 3]).unwrap();
    let equations = ["a-b", "b-c", "a-c"];
    assert!(decide(
        &equations,
        &[Some(true), Some(true), Some(false)],
        &cell,
        &[false; 3]
    ));
    assert!(!decide(
        &equations,
        &[Some(true), None, Some(false)],
        &cell,
        &[false; 3]
    ));
    assert!(!decide(
        &["a-b", "b-c"],
        &[Some(true), Some(false)],
        &cell,
        &[false; 3]
    ));
    assert!(decide(
        &["a-b", "2*a-2*b", "a-c", "b-c"],
        &[Some(true), Some(true), Some(true), Some(false)],
        &cell,
        &[false; 3]
    ));
}

#[test]
fn only_base_inconsistency_refutes_a_disequality_branch() {
    let cell = LatticeBox::try_new([0; 3], [None; 3]).unwrap();
    for false_atom in ["a-1", "2*b-1"] {
        assert!(!decide(
            &["a", false_atom],
            &[Some(true), Some(false)],
            &cell,
            &[false; 3]
        ));
    }
    assert!(decide(&["a", "a-1"], &[Some(true); 2], &cell, &[false; 3]));
    // Rationally consistent at a=b=1/2, but impossible for integer indices.
    assert!(decide(
        &["a-b", "a+b-1"],
        &[Some(true); 2],
        &cell,
        &[false; 3]
    ));
}

#[test]
fn huge_singletons_and_permuted_original_axis_maps_keep_exact_meaning() {
    let cell = LatticeBox::try_new([u64::MAX, 0, 0], [Some(u64::MAX), None, None]).unwrap();
    assert!(decide(
        &["a+b", "2*b+36893488147419103232"],
        &[Some(true), Some(false)],
        &cell,
        &[true, false, false]
    ));
    assert!(!decide(
        &["a+b", "2*b+36893488147419103230"],
        &[Some(true), Some(false)],
        &cell,
        &[true, false, false]
    ));
    let context = CoefficientContext::new(["d", "b", "c", "a"]);
    let equations = ["-1-c-b+2*a", "3+c+b"].map(|s| context.coefficient_fixture(s).numerator);
    let face = LatticeBox::try_new([1, 0, 0], [Some(1), None, None]).unwrap();
    let atoms = equations
        .iter()
        .map(|equation| Atom {
            indices: &[3, 1, 2],
            equation,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        RestrictionCache::new(&[false; 3], &atoms).contradicts(&face, &[Some(true), Some(false)]),
        Ok(true)
    );
    let mixed = [
        Atom {
            indices: &[3, 1, 2],
            equation: &equations[0],
        },
        Atom {
            indices: &[1, 3, 2],
            equation: &equations[1],
        },
    ];
    assert_eq!(
        RestrictionCache::new(&[false; 3], &mixed).contradicts(&face, &[Some(true), Some(false)]),
        Ok(false)
    );
}

#[test]
fn unsupported_original_atoms_and_foreign_maps_cannot_gain_authority() {
    let face = LatticeBox::try_new([0; 3], [Some(0), None, None]).unwrap();
    for expression in ["a*b", "d*a", "a^2"] {
        assert!(!decide(
            &[expression, "b"],
            &[Some(true), Some(false)],
            &face,
            &[false; 3]
        ));
    }
    let context = CoefficientContext::new(["d", "a", "b", "c"]);
    let foreign = CoefficientContext::new(["q", "x", "y", "z"]);
    let a = context.coefficient_fixture("a-b").numerator;
    let mut b = foreign.coefficient_fixture("x-y").numerator;
    let cell = LatticeBox::try_new([0; 3], [None; 3]).unwrap();
    for _ in 0..2 {
        let atoms = [
            Atom {
                indices: &[1, 2, 3],
                equation: &a,
            },
            Atom {
                indices: &[1, 2, 3],
                equation: &b,
            },
        ];
        assert_eq!(
            RestrictionCache::new(&[false; 3], &atoms)
                .contradicts(&cell, &[Some(true), Some(false)]),
            Ok(false)
        );
        b = a.clone();
        b.exponents.pop();
    }
}

#[test]
fn shared_native_work_and_local_admission_caps_fail_explicitly() {
    let context = CoefficientContext::new(["d", "a", "b", "c"]);
    let equations = ["a-b", "2*a-2*b"].map(|s| context.coefficient_fixture(s).numerator);
    let atoms = equations
        .iter()
        .map(|equation| Atom {
            indices: &[1, 2, 3],
            equation,
        })
        .collect::<Vec<_>>();
    let cell = LatticeBox::try_new([0; 3], [None; 3]).unwrap();
    let mut cache = RestrictionCache::new(&[false; 3], &atoms);
    cache.budget.remaining = 1;
    assert_eq!(
        cache.contradicts(&cell, &[Some(true), Some(false)]),
        Err(WorkExhausted)
    );
    let mut budget = WorkBudget::default();
    assert_eq!(
        contradicts(
            &[false; 3],
            &atoms,
            &cell,
            &vec![(0, true); MAX_EQUATIONS + 1],
            &mut budget
        ),
        Err(WorkExhausted)
    );
    let before = budget.remaining;
    assert_eq!(
        contradicts(
            &[false; 3],
            &atoms,
            &cell,
            &[(0, false), (1, false)],
            &mut budget
        ),
        Ok(false)
    );
    assert_eq!(budget.remaining, before);
}

#[test]
fn equivalent_predicate_partition_closes_but_a_real_missing_point_still_fails() {
    use crate::foundry::artifact::source_port::predicate_cover::{
        PredicateCoverError, PredicateCoverLimits, PredicateCoveragePiece, certify_predicate_cover,
    };
    use crate::foundry::parametric::AffineApplicationDomain;
    use crate::solver::{AffineCase, AffineIntersection, CoordinateCase};
    use std::sync::Arc;
    let context = CoefficientContext::new(["d", "a", "b", "c"]);
    let sector = [false; 3];
    let domain = |text| {
        let AffineIntersection::Affine(case) = AffineCase::from_coordinate(
            &CoordinateCase::generic(),
            &[context.coefficient_fixture(text).numerator],
            &[1, 2, 3],
            &sector,
        )
        .unwrap() else {
            panic!("expected affine predicate")
        };
        Arc::new(AffineApplicationDomain::from_case(&case, &sector).unwrap())
    };
    let original = domain("-1-c-b+2*a");
    let equivalent = domain("3+c+b");
    let exclusions = [original];
    let outside = [
        LatticeBox::try_new([0; 3], [Some(0), None, None]).unwrap(),
        LatticeBox::try_new([2, 0, 0], [None; 3]).unwrap(),
    ];
    let face = [LatticeBox::try_new([1, 0, 0], [Some(1), None, None]).unwrap()];
    let missing = [LatticeBox::try_new([1, 0, 1], [Some(1), None, None]).unwrap()];
    let owners = |boxes| {
        [
            PredicateCoveragePiece {
                boxes: &outside,
                affine_target: None,
                affine_exclusions: &[],
            },
            PredicateCoveragePiece {
                boxes: &face,
                affine_target: None,
                affine_exclusions: &exclusions,
            },
            PredicateCoveragePiece {
                boxes,
                affine_target: Some(&equivalent),
                affine_exclusions: &[],
            },
        ]
    };
    let proof = certify_predicate_cover(
        &sector,
        &owners(&face),
        &[],
        PredicateCoverLimits::default(),
    )
    .unwrap();
    assert_eq!(proof.predicates, 2);
    assert!(matches!(
        certify_predicate_cover(
            &sector,
            &owners(&missing),
            &[],
            PredicateCoverLimits::default()
        ),
        Err(PredicateCoverError::Uncovered { .. })
    ));
    let terminal = [LatticeBox::try_new([1, 3, 0], [Some(1), Some(3), Some(0)]).unwrap()];
    assert!(
        certify_predicate_cover(
            &sector,
            &owners(&missing),
            &terminal,
            PredicateCoverLimits::default()
        )
        .is_ok()
    );
}
