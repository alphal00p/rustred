use super::*;
use crate::algebra::CoefficientContext;
use crate::solver::{AffineCase, AffineIntersection, CoordinateCase};

fn full<const N: usize>() -> LatticeBox {
    LatticeBox::try_new([0; N], [None; N]).unwrap()
}

fn affine<const N: usize>(
    context: &CoefficientContext,
    sector: &[bool; N],
    fixed: [Option<i16>; N],
    equations: &[&str],
) -> Arc<AffineApplicationDomain> {
    let equations = equations
        .iter()
        .map(|s| context.coefficient_fixture(s).numerator)
        .collect::<Vec<_>>();
    let case = AffineCase::from_coordinate(
        &CoordinateCase::new(fixed).unwrap(),
        &equations,
        &std::array::from_fn(|i| i),
        sector,
    )
    .unwrap();
    let AffineIntersection::Affine(case) = case else {
        panic!("expected affine fixture")
    };
    Arc::new(AffineApplicationDomain::from_case(&case, sector).unwrap())
}

#[test]
fn coordinate_cover_and_singleton_terminals() {
    let boxes = [LatticeBox::try_new([1], [None]).unwrap()];
    let owners = [PredicateCoveragePiece {
        boxes: &boxes,
        affine_target: None,
        affine_exclusions: &[],
    }];
    let terminal = [LatticeBox::try_new([0], [Some(0)]).unwrap()];
    assert!(
        certify_predicate_cover(&[true], &owners, &terminal, PredicateCoverLimits::default())
            .is_ok()
    );
    assert!(matches!(
        certify_predicate_cover(&[true], &owners, &[], PredicateCoverLimits::default()),
        Err(PredicateCoverError::Uncovered { .. })
    ));
    assert!(matches!(
        certify_predicate_cover(
            &[true],
            &owners,
            &[full::<1>()],
            PredicateCoverLimits::default()
        ),
        Err(PredicateCoverError::InvalidDomain(_))
    ));
}

#[test]
fn tautological_partition_requires_the_exact_affine_child() {
    let context = CoefficientContext::new(["n0", "n1"]);
    let domain = affine(&context, &[false; 2], [None; 2], &["1+n0-2*n1"]);
    let boxes = [full::<2>()];
    let exceptions = [Arc::clone(&domain)];
    let owners = [
        PredicateCoveragePiece {
            boxes: &boxes,
            affine_target: None,
            affine_exclusions: &exceptions,
        },
        PredicateCoveragePiece {
            boxes: &boxes,
            affine_target: Some(&domain),
            affine_exclusions: &[],
        },
    ];
    let proof = certify_predicate_cover(&[false; 2], &owners, &[], PredicateCoverLimits::default())
        .unwrap();
    assert_eq!(proof.predicates, 1);
    assert!(matches!(
        certify_predicate_cover(
            &[false; 2],
            &owners[..1],
            &[],
            PredicateCoverLimits::default()
        ),
        Err(PredicateCoverError::Uncovered {
            unbounded_boxes: 1,
            ..
        })
    ));
    assert!(matches!(
        certify_predicate_cover(
            &[false; 2],
            &owners[1..],
            &[],
            PredicateCoverLimits::default()
        ),
        Err(PredicateCoverError::Uncovered { .. })
    ));
}

#[test]
fn affine_children_can_partition_a_remaining_free_coordinate() {
    let context = CoefficientContext::new(["n0", "n1", "n2"]);
    let domain = affine(&context, &[false; 3], [None; 3], &["1+n0-2*n1"]);
    let boundary = affine(&context, &[false; 3], [None, None, Some(0)], &["1+n0-2*n1"]);
    let whole = [full::<3>()];
    let interior = [LatticeBox::try_new([0, 0, 1], [None; 3]).unwrap()];
    let exceptions = [Arc::clone(&domain)];
    let owners = [
        PredicateCoveragePiece {
            boxes: &whole,
            affine_target: None,
            affine_exclusions: &exceptions,
        },
        PredicateCoveragePiece {
            boxes: &interior,
            affine_target: Some(&domain),
            affine_exclusions: &[],
        },
        PredicateCoveragePiece {
            boxes: &whole,
            affine_target: Some(&boundary),
            affine_exclusions: &[],
        },
    ];
    assert!(
        certify_predicate_cover(&[false; 3], &owners, &[], PredicateCoverLimits::default()).is_ok()
    );
    assert!(matches!(
        certify_predicate_cover(
            &[false; 3],
            &owners[..2],
            &[],
            PredicateCoverLimits::default()
        ),
        Err(PredicateCoverError::Uncovered { .. })
    ));
}

#[test]
fn exception_fixed_face_is_not_its_entire_rectangular_prefilter() {
    let context = CoefficientContext::new(["n0", "n1", "n2"]);
    let domain = affine(&context, &[false; 3], [None, None, Some(0)], &["1+n0-2*n1"]);
    let boxes = [full::<3>()];
    let boundary = [LatticeBox::try_new([0; 3], [None, None, Some(0)]).unwrap()];
    let exceptions = [Arc::clone(&domain)];
    let owners = [
        PredicateCoveragePiece {
            boxes: &boxes,
            affine_target: None,
            affine_exclusions: &exceptions,
        },
        PredicateCoveragePiece {
            boxes: &boundary,
            affine_target: Some(&domain),
            affine_exclusions: &[],
        },
    ];
    assert!(
        certify_predicate_cover(&[false; 3], &owners, &[], PredicateCoverLimits::default()).is_ok()
    );
}

#[test]
fn nested_and_overlapping_exceptions_keep_conjunction_semantics() {
    let context = CoefficientContext::new(["n0", "n1", "n2", "n3"]);
    let sector = [false; 4];
    let a = affine(&context, &sector, [None; 4], &["n0-n1"]);
    let b = affine(&context, &sector, [None; 4], &["n2-n3"]);
    let ab = affine(&context, &sector, [None; 4], &["n0-n1", "n2-n3"]);
    let boxes = [full::<4>()];
    let parent_exceptions = [Arc::clone(&a), Arc::clone(&b)];
    let b_exception = [Arc::clone(&b)];
    let owners = [
        PredicateCoveragePiece {
            boxes: &boxes,
            affine_target: None,
            affine_exclusions: &parent_exceptions,
        },
        PredicateCoveragePiece {
            boxes: &boxes,
            affine_target: Some(&a),
            affine_exclusions: &b_exception,
        },
        PredicateCoveragePiece {
            boxes: &boxes,
            affine_target: Some(&b),
            affine_exclusions: &[],
        },
        PredicateCoveragePiece {
            boxes: &boxes,
            affine_target: Some(&ab),
            affine_exclusions: &[],
        },
    ];
    assert!(
        certify_predicate_cover(&sector, &owners, &[], PredicateCoverLimits::default()).is_ok()
    );
    assert!(matches!(
        certify_predicate_cover(&sector, &owners[..2], &[], PredicateCoverLimits::default()),
        Err(PredicateCoverError::Uncovered { .. })
    ));
}

#[test]
fn self_excluding_owner_cannot_create_a_coverage_cycle() {
    let context = CoefficientContext::new(["n0", "n1"]);
    let domain = affine(&context, &[true; 2], [None; 2], &["n0-n1"]);
    let exceptions = [Arc::clone(&domain)];
    let boxes = [full::<2>()];
    let owners = [
        PredicateCoveragePiece {
            boxes: &boxes,
            affine_target: None,
            affine_exclusions: &exceptions,
        },
        PredicateCoveragePiece {
            boxes: &boxes,
            affine_target: Some(&domain),
            affine_exclusions: &exceptions,
        },
    ];
    assert!(matches!(
        certify_predicate_cover(&[true; 2], &owners, &[], PredicateCoverLimits::default()),
        Err(PredicateCoverError::Uncovered { .. })
    ));
}

#[test]
fn predicate_mutation_and_resource_exhaustion_fail_closed() {
    let context = CoefficientContext::new(["n0", "n1"]);
    let domain = affine(&context, &[false; 2], [None; 2], &["n0-n1"]);
    let changed = affine(&context, &[false; 2], [None; 2], &["1+n0-n1"]);
    let boxes = [full::<2>()];
    let exceptions = [Arc::clone(&domain)];
    let owners = [
        PredicateCoveragePiece {
            boxes: &boxes,
            affine_target: None,
            affine_exclusions: &exceptions,
        },
        PredicateCoveragePiece {
            boxes: &boxes,
            affine_target: Some(&changed),
            affine_exclusions: &[],
        },
    ];
    assert!(matches!(
        certify_predicate_cover(&[false; 2], &owners, &[], PredicateCoverLimits::default()),
        Err(PredicateCoverError::Uncovered { .. })
    ));
    assert!(matches!(
        certify_predicate_cover(
            &[false; 2],
            &owners,
            &[],
            PredicateCoverLimits {
                max_predicates: 0,
                ..Default::default()
            }
        ),
        Err(PredicateCoverError::Budget("predicate atoms"))
    ));
    assert!(matches!(
        certify_predicate_cover(&[true; 2], &owners, &[], PredicateCoverLimits::default()),
        Err(PredicateCoverError::InvalidDomain(_))
    ));
}

#[test]
fn integer_empty_equality_slice_does_not_require_a_phantom_child() {
    let context = CoefficientContext::new(["n0", "n1"]);
    let domain = affine(&context, &[false; 2], [None; 2], &["1+n0-2*n1"]);
    let boxes = [full::<2>()];
    let exceptions = [Arc::clone(&domain)];
    // On n0=0 the equality would require 2*n1=1, so no integer
    // point is removed there. The actual equality child needs n0<=-1.
    let child = [LatticeBox::try_new([1, 0], [None; 2]).unwrap()];
    let owners = [
        PredicateCoveragePiece {
            boxes: &boxes,
            affine_target: None,
            affine_exclusions: &exceptions,
        },
        PredicateCoveragePiece {
            boxes: &child,
            affine_target: Some(&domain),
            affine_exclusions: &[],
        },
    ];
    assert!(
        certify_predicate_cover(&[false; 2], &owners, &[], PredicateCoverLimits::default()).is_ok()
    );
    // Removing n0=-1 as well really loses (-1,0), and must fail.
    let smaller = [LatticeBox::try_new([2, 0], [None; 2]).unwrap()];
    let owners = [
        PredicateCoveragePiece {
            boxes: &boxes,
            affine_target: None,
            affine_exclusions: &exceptions,
        },
        PredicateCoveragePiece {
            boxes: &smaller,
            affine_target: Some(&domain),
            affine_exclusions: &[],
        },
    ];
    assert!(matches!(
        certify_predicate_cover(&[false; 2], &owners, &[], PredicateCoverLimits::default()),
        Err(PredicateCoverError::Uncovered { .. })
    ));
}
