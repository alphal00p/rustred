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
    // At degree one, x >= 1 forces y = 0 (and conversely). Tightening the
    // remaining boxes by the exact sum bound lets the native affine service
    // disprove the diagonal, without erasing it or enumerating simplex points.
    certify_predicate_cover_up_to_degree(
        &[false; 2],
        &owners,
        &terminal,
        EntryDegreeBound::MaxTotalExcessDegree(1),
        Default::default(),
    )
    .unwrap();
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

#[test]
fn degree_hull_matches_exact_coordinate_extrema_on_small_boxes() {
    for sector in [[false, false], [false, true], [true, false], [true, true]] {
        for lower_x in 0..=2 {
            for lower_y in 0..=2 {
                for upper_x in lower_x..=3 {
                    for upper_y in lower_y..=3 {
                        let cell =
                            LatticeBox::try_new([lower_x, lower_y], [Some(upper_x), Some(upper_y)])
                                .unwrap();
                        for limit in 0..=4 {
                            for degree in [
                                EntryDegreeBound::MaxTotalExcessDegree(limit),
                                EntryDegreeBound::MaxNegativeIndexDegree(limit),
                            ] {
                                let mut points = Vec::new();
                                for x in lower_x..=upper_x {
                                    for y in lower_y..=upper_y {
                                        let cost: u64 = [x, y]
                                            .into_iter()
                                            .zip(sector)
                                            .filter(|(_, active)| {
                                                !*active
                                                    || matches!(
                                                        degree,
                                                        EntryDegreeBound::MaxTotalExcessDegree(_)
                                                    )
                                            })
                                            .map(|(value, _)| value)
                                            .sum();
                                        if cost <= limit {
                                            points.push([x, y]);
                                        }
                                    }
                                }
                                let actual = scope::degree_hull(&sector, &cell, degree).unwrap();
                                assert_eq!(actual.is_none(), points.is_empty());
                                if let Some(actual) = actual {
                                    for axis in 0..2 {
                                        assert_eq!(
                                            actual.lower()[axis],
                                            points.iter().map(|p| p[axis]).min().unwrap()
                                        );
                                        assert_eq!(
                                            actual.upper()[axis],
                                            points.iter().map(|p| p[axis]).max()
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn degree_hull_retains_uncounted_infinite_axes_and_handles_extreme_endpoints() {
    let cell = LatticeBox::try_new([u64::MAX, 3], [None, None]).unwrap();
    let hull = scope::degree_hull(
        &[true, false],
        &cell,
        EntryDegreeBound::MaxNegativeIndexDegree(3),
    )
    .unwrap()
    .unwrap();
    assert_eq!(hull.lower(), &[u64::MAX, 3]);
    assert_eq!(hull.upper(), &[None, Some(3)]);
    assert!(
        scope::degree_hull(
            &[true, false],
            &cell,
            EntryDegreeBound::MaxTotalExcessDegree(u64::MAX),
        )
        .unwrap()
        .is_none()
    );
    let cell = LatticeBox::try_new([u64::MAX, 0], [None, None]).unwrap();
    let hull = scope::degree_hull(
        &[true; 2],
        &cell,
        EntryDegreeBound::MaxTotalExcessDegree(u64::MAX),
    )
    .unwrap()
    .unwrap();
    assert_eq!(hull.upper(), &[Some(u64::MAX), Some(0)]);
    assert!(matches!(
        scope::degree_hull(&[false], &cell, EntryDegreeBound::MaxTotalExcessDegree(0),),
        Err(PredicateCoverError::InvalidDomain(_))
    ));
}
