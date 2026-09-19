use super::*;

#[test]
fn simplex_coverage_does_not_require_its_rectangular_hull() {
    let boxes = (0..=2)
        .map(|x| LatticeBox::try_new([x, 0], [Some(x), Some(2 - x)]).unwrap())
        .collect::<Vec<_>>();
    let owners = [PredicateCoveragePiece {
        boxes: &boxes,
        affine_target: None,
        affine_exclusions: &[],
    }];
    for sector in [[false, false], [true, true], [true, false]] {
        certify_predicate_cover_up_to_degree(
            &sector,
            &owners,
            &[],
            EntryDegreeBound::MaxTotalExcessDegree(2),
            Default::default(),
        )
        .unwrap();
        assert!(matches!(
            certify_predicate_cover_up_to_degree(
                &sector,
                &owners,
                &[],
                EntryDegreeBound::MaxTotalExcessDegree(3),
                Default::default(),
            ),
            Err(PredicateCoverError::Uncovered {
                unbounded_boxes: 0,
                ..
            })
        ));
    }
    assert!(certify_predicate_cover(&[true; 2], &owners, &[], Default::default()).is_err());
}

#[test]
fn negative_degree_preserves_unbounded_dots() {
    let terminal = [LatticeBox::try_new([0, 0], [Some(0), Some(0)]).unwrap()];
    certify_predicate_cover_up_to_degree(
        &[true, false],
        &[],
        &terminal,
        EntryDegreeBound::MaxTotalExcessDegree(0),
        Default::default(),
    )
    .unwrap();
    assert!(matches!(
        certify_predicate_cover_up_to_degree(
            &[true, false],
            &[],
            &terminal,
            EntryDegreeBound::MaxNegativeIndexDegree(0),
            Default::default(),
        ),
        Err(PredicateCoverError::Uncovered {
            unbounded_boxes: 1,
            ..
        })
    ));
    let ray = [LatticeBox::try_new([0, 0], [None, Some(0)]).unwrap()];
    let owners = [PredicateCoveragePiece {
        boxes: &ray,
        affine_target: None,
        affine_exclusions: &[],
    }];
    certify_predicate_cover_up_to_degree(
        &[true, false],
        &owners,
        &[],
        EntryDegreeBound::MaxNegativeIndexDegree(0),
        Default::default(),
    )
    .unwrap();
}

#[test]
fn degree_cover_retains_affine_exceptions() {
    let context = CoefficientContext::new(["n0", "n1"]);
    let excluded = [affine(&context, &[false; 2], [None; 2], &["n0-n1"])];
    let boxes = [full::<2>()];
    let owners = [PredicateCoveragePiece {
        boxes: &boxes,
        affine_target: None,
        affine_exclusions: &excluded,
    }];
    let terminal = [LatticeBox::try_new([0; 2], [Some(0); 2]).unwrap()];
    certify_predicate_cover_up_to_degree(
        &[false; 2],
        &owners,
        &terminal,
        EntryDegreeBound::MaxTotalExcessDegree(0),
        Default::default(),
    )
    .unwrap();
    // At degree one the exact diagonal has only the already owned origin,
    // but a Boolean branch's rectangular hull can still contain (1,1).
    // Minimum-degree pruning alone need not prove that coupled infeasibility.
    // Preserve the conservative failure rather than erase the affine guard.
    assert!(matches!(
        certify_predicate_cover_up_to_degree(
            &[false; 2],
            &owners,
            &terminal,
            EntryDegreeBound::MaxTotalExcessDegree(1),
            Default::default(),
        ),
        Err(PredicateCoverError::Uncovered { .. })
    ));
    assert!(matches!(
        certify_predicate_cover_up_to_degree(
            &[false; 2],
            &owners,
            &terminal,
            EntryDegreeBound::MaxTotalExcessDegree(2),
            Default::default(),
        ),
        Err(PredicateCoverError::Uncovered { .. })
    ));
}

#[test]
fn degree_probe_is_budgeted_and_does_not_skip_invalid_owners() {
    let limits = PredicateCoverLimits {
        geometry: CompletionGeometryLimits {
            max_split_operations: 0,
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(matches!(
        certify_predicate_cover_up_to_degree(
            &[true; 2],
            &[],
            &[],
            EntryDegreeBound::MaxTotalExcessDegree(30),
            limits,
        ),
        Err(PredicateCoverError::Budget(_))
    ));
    let bad = [full::<1>()];
    let owners = [PredicateCoveragePiece {
        boxes: &bad,
        affine_target: None,
        affine_exclusions: &[],
    }];
    assert!(matches!(
        certify_predicate_cover_up_to_degree(
            &[true; 2],
            &owners,
            &[],
            EntryDegreeBound::MaxTotalExcessDegree(0),
            Default::default(),
        ),
        Err(PredicateCoverError::InvalidDomain(_))
    ));
}

#[test]
fn degree_cover_matches_exhaustive_small_integer_domain() {
    // All 512 subsets of a 3x3 integer grid, not random samples. Each owner
    // here is a coordinate singleton, independently compared with the simplex.
    for mask in 0_u16..512 {
        let boxes = (0..9)
            .filter(|bit| mask & (1 << bit) != 0)
            .map(|bit| {
                let point = [bit % 3, bit / 3];
                LatticeBox::try_new(point, point.map(Some)).unwrap()
            })
            .collect::<Vec<_>>();
        let owners = [PredicateCoveragePiece {
            boxes: &boxes,
            affine_target: None,
            affine_exclusions: &[],
        }];
        for degree in 0..=2 {
            let expected =
                (0..=degree).all(|x| (0..=degree - x).all(|y| mask & (1 << (x + 3 * y)) != 0));
            let actual = certify_predicate_cover_up_to_degree(
                &[false; 2],
                &owners,
                &[],
                EntryDegreeBound::MaxTotalExcessDegree(degree),
                Default::default(),
            );
            assert_eq!(
                actual.is_ok(),
                expected,
                "mask={mask}, degree={degree}: {actual:?}"
            );
            if !expected {
                assert!(matches!(actual, Err(PredicateCoverError::Uncovered { .. })));
            }
        }
    }
}
