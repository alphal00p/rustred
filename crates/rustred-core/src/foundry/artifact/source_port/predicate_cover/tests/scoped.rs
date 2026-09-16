use super::*;

#[test]
fn requested_domain_keeps_infinite_rays_and_only_ignores_outside_holes() {
    let covered = [LatticeBox::try_new([0, 0], [Some(2), None]).unwrap()];
    let owners = [PredicateCoveragePiece {
        boxes: &covered,
        affine_target: None,
        affine_exclusions: &[],
    }];
    assert!(certify_predicate_cover(&[false, true], &owners, &[], Default::default()).is_err());
    assert!(
        certify_predicate_cover_within(&[false, true], &owners, &[], &covered, Default::default(),)
            .is_ok()
    );
    let too_wide = [LatticeBox::try_new([0, 0], [Some(3), None]).unwrap()];
    assert!(matches!(
        certify_predicate_cover_within(&[false, true], &owners, &[], &too_wide, Default::default(),),
        Err(PredicateCoverError::Uncovered {
            unbounded_boxes: 1,
            ..
        })
    ));
}

#[test]
fn affine_exception_still_needs_its_exact_scoped_leaf() {
    let context = CoefficientContext::new(["n0", "n1"]);
    let excluded = [affine(&context, &[false; 2], [None; 2], &["n0-n1"])];
    let boxes = [full::<2>()];
    let owners = [PredicateCoveragePiece {
        boxes: &boxes,
        affine_target: None,
        affine_exclusions: &excluded,
    }];
    let off = [LatticeBox::try_new([0, 1], [Some(0), Some(1)]).unwrap()];
    certify_predicate_cover_within(&[false; 2], &owners, &[], &off, Default::default()).unwrap();
    let needs_zero = [LatticeBox::try_new([0, 0], [Some(0), Some(1)]).unwrap()];
    assert!(matches!(
        certify_predicate_cover_within(&[false; 2], &owners, &[], &needs_zero, Default::default(),),
        Err(PredicateCoverError::Uncovered { .. })
    ));
    let terminal = [LatticeBox::try_new([0; 2], [Some(0); 2]).unwrap()];
    certify_predicate_cover_within(
        &[false; 2],
        &owners,
        &terminal,
        &needs_zero,
        Default::default(),
    )
    .unwrap();
}

#[test]
fn original_admission_precedes_even_empty_requested_domain() {
    let good = [full::<2>()];
    let wrong = [full::<1>()];
    let owners = [
        PredicateCoveragePiece {
            boxes: &good,
            affine_target: None,
            affine_exclusions: &[],
        },
        PredicateCoveragePiece {
            boxes: &wrong,
            affine_target: None,
            affine_exclusions: &[],
        },
    ];
    assert!(matches!(
        certify_predicate_cover_within(&[true; 2], &owners, &[], &[], Default::default(),),
        Err(PredicateCoverError::InvalidDomain(_))
    ));
    assert!(matches!(
        certify_predicate_cover_within(&[true; 2], &owners[..1], &good, &[], Default::default(),),
        Err(PredicateCoverError::InvalidDomain(_))
    ));
    let proof =
        certify_predicate_cover_within(&[true; 2], &owners[..1], &[], &[], Default::default())
            .unwrap();
    assert_eq!(proof.boolean_nodes, 0);
}

#[test]
fn all_requested_boxes_share_one_boolean_work_allowance() {
    let covered = [full::<1>()];
    let owners = [PredicateCoveragePiece {
        boxes: &covered,
        affine_target: None,
        affine_exclusions: &[],
    }];
    // Nonidentical overlapping boxes are still two obligations. Neither
    // overlap nor restarting the traversal grants a fresh resource budget.
    let requested = [
        LatticeBox::try_new([0], [Some(2)]).unwrap(),
        LatticeBox::try_new([1], [Some(3)]).unwrap(),
    ];
    let mut limits = PredicateCoverLimits {
        max_boolean_nodes: 1,
        ..Default::default()
    };
    assert!(matches!(
        certify_predicate_cover_within(&[true], &owners, &[], &requested, limits,),
        Err(PredicateCoverError::Budget("Boolean partition nodes"))
    ));
    limits.max_boolean_nodes = 2;
    let proof = certify_predicate_cover_within(&[true], &owners, &[], &requested, limits).unwrap();
    assert_eq!(proof.boolean_nodes, 2);
    limits.geometry.max_requested_boxes = 1;
    assert!(certify_predicate_cover_within(&[true], &owners, &[], &requested, limits).is_err());
}

#[test]
fn explicit_full_scope_matches_the_unrestricted_proof() {
    let boxes = [LatticeBox::try_new([1], [None]).unwrap()];
    let owners = [PredicateCoveragePiece {
        boxes: &boxes,
        affine_target: None,
        affine_exclusions: &[],
    }];
    let terminals = [LatticeBox::try_new([0], [Some(0)]).unwrap()];
    let unbounded =
        certify_predicate_cover(&[true], &owners, &terminals, Default::default()).unwrap();
    let scoped = certify_predicate_cover_within(
        &[true],
        &owners,
        &terminals,
        &[full::<1>()],
        Default::default(),
    )
    .unwrap();
    assert_eq!(unbounded, scoped);
}

#[test]
fn geometry_work_is_not_reset_for_each_requested_box() {
    let boxes = [full::<1>()];
    let owners = [PredicateCoveragePiece {
        boxes: &boxes,
        affine_target: None,
        affine_exclusions: &[],
    }];
    let requested = [
        LatticeBox::try_new([0], [Some(0)]).unwrap(),
        LatticeBox::try_new([2], [Some(2)]).unwrap(),
    ];
    let mut limits = PredicateCoverLimits::default();
    limits.geometry.max_split_operations = 1;
    assert!(certify_predicate_cover_within(&[true], &owners, &[], &requested, limits).is_err());
    limits.geometry.max_split_operations = 2;
    certify_predicate_cover_within(&[true], &owners, &[], &requested, limits).unwrap();
    let wrong_arity = [full::<2>()];
    assert!(matches!(
        certify_predicate_cover_within(&[true], &owners, &[], &wrong_arity, limits,),
        Err(PredicateCoverError::InvalidDomain("requested box arity"))
    ));
}
