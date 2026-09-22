use super::*;

#[test]
fn unbounded_groups_keep_the_finite_difference_band_without_fabricating_upper_caps() {
    let powers = DomainPowerBounds {
        max_positive_power: None,
        min_power_difference: Some(5),
        max_power_difference: Some(5),
    };
    let projected = project(&[true, false], &[0, 100], &[None, None], None, powers)
        .unwrap()
        .unwrap();
    assert_eq!(projected.lower, [104, 100]);
    assert_eq!(projected.upper, [None, None]);
    assert_eq!(projected.positive_lower, 105);
    assert_eq!(projected.numerator_lower, 100);
    assert_eq!(projected.positive_upper, None);
    assert_eq!(projected.numerator_upper, None);
    assert_eq!(projected.effective_rank, None);
    assert_eq!(projected.difference_lower, Some(5));
    assert_eq!(projected.difference_upper, Some(5));
    assert_eq!(projected.powers, powers);
}

fn measures<const N: usize>(owner: &[bool; N], x: &[u64; N]) -> (u128, u128, i128) {
    let mut a = 0u128;
    let mut r = 0u128;
    for (&on, &x) in owner.iter().zip(x) {
        if on {
            a += u128::from(x) + 1;
        } else {
            r += u128::from(x);
        }
    }
    (
        a,
        r,
        i128::try_from(a).unwrap() - i128::try_from(r).unwrap(),
    )
}

fn allowed<const N: usize>(
    owner: &[bool; N],
    x: &[u64; N],
    rank: Option<u32>,
    powers: DomainPowerBounds,
) -> bool {
    let (a, r, d) = measures(owner, x);
    rank.is_none_or(|cap| r <= u128::from(cap))
        && powers
            .max_positive_power
            .is_none_or(|cap| a <= u128::from(cap))
        && powers
            .min_power_difference
            .is_none_or(|cap| d >= i128::from(cap))
        && powers
            .max_power_difference
            .is_none_or(|cap| d <= i128::from(cap))
}

#[test]
fn exhaustive_two_axis_projection_matches_integer_membership() {
    let intervals: Vec<_> = (0..=2)
        .flat_map(|lo| (lo..=2).map(move |hi| (lo, hi)))
        .collect();
    let a_caps: Vec<_> = [None].into_iter().chain((0..=6).map(Some)).collect();
    let r_caps: Vec<_> = [None].into_iter().chain((0..=4).map(Some)).collect();
    let d_ends: Vec<_> = [None].into_iter().chain((-4..=4).map(Some)).collect();
    let mut checked = 0usize;
    for mask in 0..4 {
        let owner = [mask & 1 != 0, mask & 2 != 0];
        for &(l0, u0) in &intervals {
            for &(l1, u1) in &intervals {
                let lower = [l0, l1];
                let upper = [Some(u0), Some(u1)];
                for &max_positive_power in &a_caps {
                    for &rank in &r_caps {
                        for &min_power_difference in &d_ends {
                            for &max_power_difference in &d_ends {
                                if min_power_difference
                                    .zip(max_power_difference)
                                    .is_some_and(|(lo, hi)| lo > hi)
                                {
                                    continue;
                                }
                                let powers = DomainPowerBounds {
                                    max_positive_power,
                                    min_power_difference,
                                    max_power_difference,
                                };
                                let points: Vec<_> = (l0..=u0)
                                    .flat_map(|x| (l1..=u1).map(move |y| [x, y]))
                                    .filter(|x| allowed(&owner, x, rank, powers))
                                    .collect();
                                let result = project(&owner, &lower, &upper, rank, powers).unwrap();
                                assert_eq!(result.is_none(), points.is_empty());
                                if let Some(result) = result {
                                    assert_eq!(result.powers, powers);
                                    for axis in 0..2 {
                                        assert_eq!(
                                            result.lower[axis],
                                            points.iter().map(|x| x[axis]).min().unwrap()
                                        );
                                        assert_eq!(
                                            result.upper[axis],
                                            points.iter().map(|x| x[axis]).max()
                                        );
                                    }
                                    let measured: Vec<_> =
                                        points.iter().map(|x| measures(&owner, x)).collect();
                                    assert_eq!(
                                        result.positive_lower,
                                        measured.iter().map(|m| m.0).min().unwrap()
                                    );
                                    assert_eq!(
                                        result.positive_upper,
                                        measured.iter().map(|m| m.0).max()
                                    );
                                    assert_eq!(
                                        result.numerator_lower,
                                        measured.iter().map(|m| m.1).min().unwrap()
                                    );
                                    assert_eq!(
                                        result.numerator_upper,
                                        measured.iter().map(|m| m.1).max()
                                    );
                                    assert_eq!(
                                        result.difference_lower,
                                        measured.iter().map(|m| m.2).min()
                                    );
                                    assert_eq!(
                                        result.difference_upper,
                                        measured.iter().map(|m| m.2).max()
                                    );
                                    assert_eq!(
                                        project(
                                            &owner,
                                            &result.lower,
                                            &result.upper,
                                            result.effective_rank,
                                            powers
                                        )
                                        .unwrap(),
                                        Some(result),
                                    );
                                }
                                checked += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(checked, 442_368);
}

#[test]
fn projection_does_not_replace_correlations_by_a_rectangle() {
    let powers = DomainPowerBounds {
        max_positive_power: Some(4),
        ..Default::default()
    };
    let projected = project(&[true, true], &[0, 0], &[Some(2), Some(2)], None, powers)
        .unwrap()
        .unwrap();
    assert_eq!(projected.lower, [0, 0]);
    assert_eq!(projected.upper, [Some(2), Some(2)]);
    assert!(!allowed(
        &[true, true],
        &[2, 2],
        projected.effective_rank,
        projected.powers
    ));
    assert!(allowed(
        &[true, true],
        &[0, 2],
        projected.effective_rank,
        projected.powers
    ));
}

#[test]
fn unbounded_axes_can_be_projected_by_the_difference_band() {
    let bounds = DomainPowerBounds {
        max_positive_power: Some(6),
        min_power_difference: Some(5),
        max_power_difference: Some(5),
    };
    let projected = project(&[true, true, false], &[0; 3], &[None; 3], None, bounds)
        .unwrap()
        .unwrap();
    assert_eq!(projected.positive_lower, 5);
    assert_eq!(projected.positive_upper, Some(6));
    assert_eq!(projected.numerator_lower, 0);
    assert_eq!(projected.numerator_upper, Some(1));
    assert_eq!(projected.effective_rank, Some(1));
    assert_eq!(projected.lower, [0, 0, 0]);
    assert_eq!(projected.upper, [Some(4), Some(4), Some(1)]);
    let default = project(
        &[true, false],
        &[0; 2],
        &[None; 2],
        None,
        Default::default(),
    )
    .unwrap()
    .unwrap();
    assert_eq!(default.positive_upper, None);
    assert_eq!(default.numerator_upper, None);
    assert_eq!(default.difference_lower, None);
    assert_eq!(default.difference_upper, None);
}

#[test]
fn empty_coordinate_groups_have_zero_sum_not_infinity() {
    let active = project(&[true; 2], &[0; 2], &[None; 2], None, Default::default())
        .unwrap()
        .unwrap();
    assert_eq!(active.numerator_upper, Some(0));
    assert_eq!(active.effective_rank, Some(0));
    assert_eq!(active.difference_lower, Some(2));
    let inactive = project(&[false; 2], &[0; 2], &[None; 2], None, Default::default())
        .unwrap()
        .unwrap();
    assert_eq!(inactive.positive_upper, Some(0));
    assert_eq!(inactive.difference_upper, Some(0));
    assert!(
        project(
            &[false; 2],
            &[0; 2],
            &[None; 2],
            None,
            DomainPowerBounds {
                min_power_difference: Some(1),
                ..Default::default()
            }
        )
        .unwrap()
        .is_none()
    );
}

#[test]
fn finite_rank_above_u32_is_not_clipped() {
    let high = u64::from(u32::MAX) + 1;
    let projected = project(
        &[true, false],
        &[0, 0],
        &[Some(0), None],
        None,
        DomainPowerBounds {
            min_power_difference: Some(1 - i64::try_from(high).unwrap()),
            ..Default::default()
        },
    )
    .unwrap()
    .unwrap();
    assert_eq!(projected.numerator_upper, Some(u128::from(high)));
    assert_eq!(projected.upper[1], Some(high));
    assert_eq!(projected.effective_rank, None);
    let capped = project(
        &[true, false],
        &[0, 0],
        &[Some(0), None],
        Some(u32::MAX),
        projected.powers,
    )
    .unwrap()
    .unwrap();
    assert_eq!(capped.effective_rank, Some(u32::MAX));
    assert_eq!(capped.numerator_upper, Some(u128::from(u32::MAX)));
}

#[test]
fn maximal_coordinate_endpoints_use_wide_physical_sums() {
    let x = u64::MAX;
    let projected = project(
        &[true, true],
        &[x, x],
        &[Some(x), Some(x)],
        None,
        Default::default(),
    )
    .unwrap()
    .unwrap();
    assert_eq!(projected.positive_lower, 2 * (u128::from(x) + 1));
    assert_eq!(projected.lower, [x, x]);
    assert_eq!(projected.upper, [Some(x), Some(x)]);
    // A finite projection beyond the coordinate representation is refused,
    // rather than represented by a clipped finite endpoint.
    assert!(matches!(
        project(
            &[true, false],
            &[0, 0],
            &[None, None],
            None,
            DomainPowerBounds {
                max_positive_power: Some(u64::MAX),
                min_power_difference: Some(-2),
                ..Default::default()
            }
        ),
        Err(DomainPowerError::OutOfRange("projected coordinate upper"))
    ));
}

#[test]
fn malformed_and_empty_domains_are_distinct() {
    assert!(matches!(
        project(&[], &[], &[], None, Default::default()),
        Err(DomainPowerError::InvalidArity)
    ));
    assert!(matches!(
        project(&[true], &[], &[None], None, Default::default()),
        Err(DomainPowerError::InvalidArity)
    ));
    assert!(matches!(
        project(&[true], &[2], &[Some(1)], None, Default::default()),
        Err(DomainPowerError::InvertedCoordinate { axis: 0 })
    ));
    assert!(matches!(
        project(
            &[true],
            &[0],
            &[None],
            None,
            DomainPowerBounds {
                min_power_difference: Some(2),
                max_power_difference: Some(1),
                ..Default::default()
            }
        ),
        Err(DomainPowerError::InvertedDifferenceBounds)
    ));
    assert!(
        project(
            &[true],
            &[0],
            &[None],
            None,
            DomainPowerBounds {
                max_positive_power: Some(0),
                ..Default::default()
            }
        )
        .unwrap()
        .is_none()
    );
    assert!(
        project(&[false], &[2], &[None], Some(1), Default::default())
            .unwrap()
            .is_none()
    );
    assert!(
        project(
            &[true],
            &[0],
            &[Some(0)],
            None,
            DomainPowerBounds {
                max_power_difference: Some(-1),
                ..Default::default()
            }
        )
        .unwrap()
        .is_none()
    );
}

#[test]
fn exact_shift_bounds_follow_current_domain_not_entry_caps() {
    let source = DomainPowerBounds {
        max_positive_power: Some(2),
        min_power_difference: Some(-1),
        max_power_difference: Some(-1),
    };
    // (2,-3)+(-3,4)=(-1,1): delta_R=-2, delta_D=1, delta_A=-1.
    assert_eq!(
        source.shifted(-1, 1).unwrap().unwrap(),
        DomainPowerBounds {
            max_positive_power: Some(1),
            min_power_difference: Some(0),
            max_power_difference: Some(0)
        }
    );
    let descendant = source.shifted(7, 2).unwrap().unwrap();
    assert_eq!(descendant.max_positive_power, Some(9));
    assert_eq!(descendant.min_power_difference, Some(1));
    assert!(source.shifted(-3, 0).unwrap().is_none());
    for x in -2i64..=2 {
        for y in -2i64..=2 {
            for sx in -2i64..=2 {
                for sy in -2i64..=2 {
                    let a = x.max(0) + y.max(0);
                    let target_a = (x + sx).max(0) + (y + sy).max(0);
                    let d = x + y;
                    let exact = DomainPowerBounds {
                        max_positive_power: Some(a as u64),
                        min_power_difference: Some(d),
                        max_power_difference: Some(d),
                    };
                    let image = exact
                        .shifted(i128::from(target_a - a), i128::from(sx + sy))
                        .unwrap()
                        .unwrap();
                    assert_eq!(image.max_positive_power, Some(target_a as u64));
                    assert_eq!(image.min_power_difference, Some(d + sx + sy));
                    assert_eq!(image.max_power_difference, Some(d + sx + sy));
                }
            }
        }
    }
}

#[test]
fn shifted_bounds_refuse_machine_range_loss() {
    assert!(matches!(
        DomainPowerBounds {
            max_positive_power: Some(u64::MAX),
            ..Default::default()
        }
        .shifted(1, 0),
        Err(DomainPowerError::OutOfRange("translated A bound"))
    ));
    for (min, max, delta) in [(Some(i64::MIN), None, -1), (None, Some(i64::MAX), 1)] {
        assert!(matches!(
            DomainPowerBounds {
                min_power_difference: min,
                max_power_difference: max,
                ..Default::default()
            }
            .shifted(0, delta),
            Err(DomainPowerError::OutOfRange("translated D bound"))
        ));
    }
    assert!(matches!(
        DomainPowerBounds {
            max_positive_power: Some(1),
            ..Default::default()
        }
        .shifted(i128::MAX, 0),
        Err(DomainPowerError::ArithmeticOverflow(_))
    ));
}

#[test]
fn componentwise_containment_requires_every_predicate() {
    let broad = DomainPowerBounds {
        max_positive_power: Some(10),
        min_power_difference: Some(-2),
        max_power_difference: Some(8),
    };
    let narrow = DomainPowerBounds {
        max_positive_power: Some(8),
        min_power_difference: Some(0),
        max_power_difference: Some(5),
    };
    assert!(DomainPowerBounds::default().contains(&broad));
    assert!(broad.contains(&narrow));
    assert!(!narrow.contains(&broad));
    assert!(!broad.contains(&DomainPowerBounds::default()));
    for changed in [
        DomainPowerBounds {
            max_positive_power: Some(11),
            ..narrow
        },
        DomainPowerBounds {
            min_power_difference: Some(-3),
            ..narrow
        },
        DomainPowerBounds {
            max_power_difference: Some(9),
            ..narrow
        },
    ] {
        assert!(!broad.contains(&changed));
    }
    assert!(DomainPowerBounds::default().is_unconstrained());
    assert!(!broad.is_unconstrained());
}
