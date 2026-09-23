use super::*;
use crate::solver::FiniteRootAdmission;

fn open<const N: usize>(support: [bool; N]) -> RootRegionInput<N> {
    RootRegionInput {
        support,
        lower: vec![0; N],
        upper: vec![None; N],
        rank: None,
        powers: Default::default(),
    }
}
fn witness<const N: usize>(
    entry: &RootRegionInput<N>,
    proposal: &RootRegionInput<N>,
) -> Result<EntryWitnessOutcome, EntryWitnessError> {
    pick_entry_intersection_witness(
        entry,
        proposal,
        EntryWitnessLimits {
            max_projections: N + 4,
        },
        &AtomicBool::new(false),
    )
}
fn local<const N: usize>(key: &IntegralKey) -> [u64; N] {
    std::array::from_fn(|i| {
        if key.powers()[i] > 0 {
            key.powers()[i] as u64 - 1
        } else {
            key.powers()[i].unsigned_abs()
        }
    })
}
// Independent bounded-integer oracle; not used by production geometry.
fn contains<const N: usize>(domain: &RootRegionInput<N>, point: &[u64; N]) -> bool {
    if (0..N).any(|i| point[i] < domain.lower[i] || domain.upper[i].is_some_and(|u| point[i] > u)) {
        return false;
    }
    let a: i128 = (0..N)
        .filter(|&i| domain.support[i])
        .map(|i| i128::from(point[i]) + 1)
        .sum();
    let r: i128 = (0..N)
        .filter(|&i| !domain.support[i])
        .map(|i| i128::from(point[i]))
        .sum();
    domain.rank.is_none_or(|cap| r <= i128::from(cap))
        && domain
            .powers
            .max_positive_power
            .is_none_or(|cap| a <= i128::from(cap))
        && domain
            .powers
            .min_power_difference
            .is_none_or(|cap| a - r >= i128::from(cap))
        && domain
            .powers
            .max_power_difference
            .is_none_or(|cap| a - r <= i128::from(cap))
}

#[test]
fn exhaustive_466560_correlated_domains_match_integer_enumeration() {
    let intervals = [(0, 0), (0, 1), (0, 2), (1, 1), (1, 2), (2, 2)];
    let bands = [
        (None, None),
        (Some(-3), None),
        (Some(0), None),
        (Some(2), None),
        (None, Some(-1)),
        (None, Some(1)),
        (Some(-1), Some(1)),
        (Some(0), Some(0)),
        (Some(2), Some(3)),
    ];
    let mut cases = 0;
    for bits in 0..8 {
        let support: [bool; 3] = std::array::from_fn(|i| bits & (1 << i) != 0);
        let proposal = open(support);
        for (l0, u0) in intervals {
            for (l1, u1) in intervals {
                for (l2, u2) in intervals {
                    for rank in [None, Some(0), Some(1), Some(2), Some(4)] {
                        for a in [None, Some(0), Some(1), Some(3), Some(5), Some(8)] {
                            for (dmin, dmax) in bands {
                                let domain = RootRegionInput {
                                    support,
                                    lower: vec![l0, l1, l2],
                                    upper: vec![Some(u0), Some(u1), Some(u2)],
                                    rank,
                                    powers: DomainPowerBounds {
                                        max_positive_power: a,
                                        min_power_difference: dmin,
                                        max_power_difference: dmax,
                                    },
                                };
                                let expected = (l0..=u0)
                                    .flat_map(|x0| {
                                        (l1..=u1).flat_map(move |x1| {
                                            (l2..=u2).map(move |x2| [x0, x1, x2])
                                        })
                                    })
                                    .find(|point| contains(&domain, point));
                                match (expected, witness(&domain, &proposal).unwrap()) {
                                    (None, EntryWitnessOutcome::Empty { projection_calls }) => {
                                        assert_eq!(projection_calls, 2)
                                    }
                                    (
                                        Some(point),
                                        EntryWitnessOutcome::Point {
                                            key,
                                            projection_calls,
                                        },
                                    ) => {
                                        assert_eq!(local::<3>(&key), point);
                                        assert_eq!(
                                            std::array::from_fn::<_, 3, _>(|i| key.powers()[i] > 0),
                                            support
                                        );
                                        assert!(projection_calls <= 7);
                                    }
                                    (expected, actual) => {
                                        panic!("{domain:?}: {expected:?} != {actual:?}")
                                    }
                                }
                                cases += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(cases, 466_560);
}

#[test]
fn exhaustive_82944_pair_intersections_preserve_both_memberships() {
    let intervals = [(0, 0), (0, 1), (0, 2), (1, 1), (1, 2), (2, 2)];
    let mut cases = 0;
    for bits in 0..4 {
        let support: [bool; 2] = std::array::from_fn(|i| bits & (1 << i) != 0);
        let mut domains = Vec::new();
        for (l0, u0) in intervals {
            for (l1, u1) in intervals {
                for (rank, a, dmin, dmax) in [
                    (None, None, None, None),
                    (Some(1), Some(3), Some(0), None),
                    (Some(2), Some(4), Some(-1), Some(1)),
                    (Some(0), Some(2), Some(1), None),
                ] {
                    domains.push(RootRegionInput {
                        support,
                        lower: vec![l0, l1],
                        upper: vec![Some(u0), Some(u1)],
                        rank,
                        powers: DomainPowerBounds {
                            max_positive_power: a,
                            min_power_difference: dmin,
                            max_power_difference: dmax,
                        },
                    });
                }
            }
        }
        for a in &domains {
            for b in &domains {
                let expected = (0..=2)
                    .flat_map(|x0| (0..=2).map(move |x1| [x0, x1]))
                    .find(|p| contains(a, p) && contains(b, p));
                match (expected, witness(a, b).unwrap()) {
                    (None, EntryWitnessOutcome::Empty { .. }) => {}
                    (Some(point), EntryWitnessOutcome::Point { key, .. }) => {
                        let actual = local::<2>(&key);
                        assert_eq!(actual, point);
                        assert!(contains(a, &actual) && contains(b, &actual));
                    }
                    (expected, actual) => panic!("a={a:?}, b={b:?}: {expected:?} != {actual:?}"),
                }
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 82_944);
}

#[test]
fn jointly_infeasible_coordinate_minima_are_never_combined() {
    let mut entry = open([true, true]);
    entry.upper = vec![Some(2); 2];
    entry.powers.max_positive_power = Some(4);
    let mut proposal = open([true, true]);
    proposal.powers.min_power_difference = Some(3);
    let EntryWitnessOutcome::Point { key, .. } = witness(&entry, &proposal).unwrap() else {
        panic!("inhabited")
    };
    assert_eq!(key.powers(), &[1, 2]);
    assert!(!contains(&proposal, &[0, 0]));
    let mut original = entry.clone();
    original.powers.min_power_difference = Some(3);
    FiniteRootAdmission::try_new([original], 1)
        .unwrap()
        .validate_entry(&key)
        .unwrap();
}

#[test]
fn both_original_inputs_are_validated_before_empty_or_disjoint_shortcuts() {
    let mut empty = open([true, false]);
    empty.powers.max_positive_power = Some(0);
    let mut malformed = open([false, true]);
    malformed.lower.pop();
    assert_eq!(
        witness(&empty, &malformed),
        Err(EntryWitnessError::Geometry(DomainPowerError::InvalidArity))
    );
    assert_eq!(
        witness(&malformed, &empty),
        Err(EntryWitnessError::Geometry(DomainPowerError::InvalidArity))
    );
    let mut inverted = open([false, true]);
    inverted.powers.min_power_difference = Some(2);
    inverted.powers.max_power_difference = Some(1);
    assert_eq!(
        witness(&empty, &inverted),
        Err(EntryWitnessError::Geometry(
            DomainPowerError::InvertedDifferenceBounds
        ))
    );
    let mut coordinate = open([false, true]);
    coordinate.lower[0] = 2;
    coordinate.upper[0] = Some(1);
    assert_eq!(
        witness(&empty, &coordinate),
        Err(EntryWitnessError::Geometry(
            DomainPowerError::InvertedCoordinate { axis: 0 }
        ))
    );
    let zero = open::<0>([]);
    assert_eq!(
        witness(&zero, &zero),
        Err(EntryWitnessError::Geometry(DomainPowerError::InvalidArity))
    );
}

#[test]
fn intersection_introduced_conflicts_are_empty_not_invalid_inputs() {
    let mut a = open([true, false]);
    let mut b = a.clone();
    a.powers.max_power_difference = Some(0);
    b.powers.min_power_difference = Some(1);
    assert_eq!(
        witness(&a, &b),
        Ok(EntryWitnessOutcome::Empty {
            projection_calls: 2
        })
    );
    a = open([true, false]);
    b = a.clone();
    a.upper[0] = Some(1);
    b.lower[0] = 2;
    assert_eq!(
        witness(&a, &b),
        Ok(EntryWitnessOutcome::Empty {
            projection_calls: 2
        })
    );
    assert_eq!(
        witness(&a, &open([false, true])),
        Ok(EntryWitnessOutcome::Empty {
            projection_calls: 2
        })
    );
    // No direct bound inversion: emptiness requires combined native A/R/D.
    a = open([true, false]);
    b = a.clone();
    a.powers.max_positive_power = Some(2);
    b.powers.min_power_difference = Some(3);
    assert_eq!(
        witness(&a, &b),
        Ok(EntryWitnessOutcome::Empty {
            projection_calls: 3
        })
    );
}

#[test]
fn i64_extremes_and_unbounded_domains_keep_physical_conventions() {
    for (support, coordinate, power) in [
        (true, i64::MAX as u64 - 1, i64::MAX),
        (false, i64::MIN.unsigned_abs(), i64::MIN),
        (false, 0, 0),
        (true, 0, 1),
    ] {
        let mut entry = open([support]);
        entry.lower[0] = coordinate;
        entry.upper[0] = Some(coordinate);
        let EntryWitnessOutcome::Point { key, .. } = witness(&entry, &open([support])).unwrap()
        else {
            panic!("point")
        };
        assert_eq!(key.powers(), &[power]);
        entry.lower[0] = u64::MAX;
        entry.upper[0] = None;
        assert_eq!(
            witness(&entry, &open([support])),
            Err(EntryWitnessError::UnrepresentableIntegralKey)
        );
    }
    let open = open([true, false, true]);
    let EntryWitnessOutcome::Point { key, .. } = witness(&open, &open).unwrap() else {
        panic!("inhabited")
    };
    assert_eq!(key.powers(), &[1, 0, 1]);
}

#[test]
fn representability_can_require_a_different_first_coordinate() {
    let mut entry = open([true, true, false]);
    entry.lower[2] = 10;
    entry.upper[2] = Some(10);
    entry.powers.min_power_difference = Some(i64::MAX);
    let EntryWitnessOutcome::Point { key, .. } = witness(&entry, &open(entry.support)).unwrap()
    else {
        panic!("representable")
    };
    assert_eq!(key.powers(), &[10, i64::MAX, -10]);
    // Both positive powers would have to sum above their representable maxima.
    let mut impossible = open([true, true, false]);
    impossible.lower[2] = i64::MIN.unsigned_abs();
    impossible.upper[2] = Some(i64::MIN.unsigned_abs());
    impossible.powers.min_power_difference = Some(i64::MAX);
    assert_eq!(
        witness(&impossible, &open(impossible.support)),
        Err(EntryWitnessError::UnrepresentableIntegralKey)
    );
}

#[test]
fn projection_allowance_counts_pair_validation_and_never_becomes_empty() {
    let domain = open([true, false]);
    for cap in 0..6 {
        assert_eq!(
            pick_entry_intersection_witness(
                &domain,
                &domain,
                EntryWitnessLimits {
                    max_projections: cap
                },
                &AtomicBool::new(false)
            ),
            Err(EntryWitnessError::ProjectionAllowance {
                used: cap,
                limit: cap
            })
        );
    }
    assert!(matches!(
        witness(&domain, &domain),
        Ok(EntryWitnessOutcome::Point {
            projection_calls: 6,
            ..
        })
    ));
    let mut empty = domain.clone();
    empty.powers.max_positive_power = Some(0);
    assert_eq!(
        pick_entry_intersection_witness(
            &empty,
            &domain,
            EntryWitnessLimits { max_projections: 1 },
            &AtomicBool::new(false)
        ),
        Err(EntryWitnessError::ProjectionAllowance { used: 1, limit: 1 })
    );
    assert_eq!(
        pick_entry_intersection_witness(
            &domain,
            &domain,
            EntryWitnessLimits { max_projections: 0 },
            &AtomicBool::new(true)
        ),
        Err(EntryWitnessError::Cancelled)
    );
}

#[test]
fn original_entry_union_holes_are_not_replaced_by_a_hull() {
    let mut first = open([true, false]);
    first.upper = vec![Some(1), Some(0)];
    let mut second = first.clone();
    second.lower[0] = 4;
    second.upper[0] = Some(5);
    let policy = FiniteRootAdmission::try_new([first.clone(), second.clone()], 2).unwrap();
    let mut hole = first.clone();
    hole.lower[0] = 2;
    hole.upper[0] = Some(3);
    for member in [&first, &second] {
        assert!(matches!(
            witness(member, &hole),
            Ok(EntryWitnessOutcome::Empty { .. })
        ));
        let EntryWitnessOutcome::Point { key, .. } =
            witness(member, &open(member.support)).unwrap()
        else {
            panic!("member inhabited")
        };
        policy.validate_entry(&key).unwrap();
    }
    let mut hull = first;
    hull.upper[0] = Some(5);
    let EntryWitnessOutcome::Point { key, .. } = witness(&hull, &hole).unwrap() else {
        panic!("hull overlap")
    };
    assert!(policy.validate_entry(&key).is_err());
}
