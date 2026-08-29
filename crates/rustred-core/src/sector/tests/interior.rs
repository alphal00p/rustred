use std::cmp::Ordering;

use super::super::{
    ComplexityComponent, Error, InteriorBounds, Mask, OrderingPolicy, SectorInteriorDomain,
};

#[test]
fn maximal_interior_intersects_sector_and_representability_preimages() {
    let sector = Mask::try_new([true, false]).unwrap();
    let shifts = [[0, 0], [-2, 2], [1, -1]];
    let domain = SectorInteriorDomain::try_maximal_for_shifts(sector.clone(), &shifts).unwrap();

    assert_eq!(domain.sector(), &sector);
    assert_eq!(domain.bounds()[0], InteriorBounds::new(3, i64::MAX - 1));
    assert_eq!(domain.bounds()[1], InteriorBounds::new(i64::MIN + 1, -2));
    for shift in shifts {
        assert!(domain.covers_shift(&shift).unwrap());
    }
    assert!(domain.contains(&[3, -2]).unwrap());
    assert_eq!(
        domain.checked_translate(&[3, -2], &[-2, 2]).unwrap(),
        Some(vec![1, 0])
    );
    assert!(!domain.contains(&[2, -2]).unwrap());
}

#[test]
fn constructor_rejects_wrong_arity_empty_bounds_and_crossing_the_sector() {
    let sector = Mask::try_new([true, false]).unwrap();
    assert!(matches!(
        SectorInteriorDomain::try_new(sector.clone(), [InteriorBounds::new(1, 2)]),
        Err(Error::WrongArity {
            expected: 2,
            actual: 1
        })
    ));
    assert!(matches!(
        SectorInteriorDomain::try_new(
            sector.clone(),
            [InteriorBounds::new(2, 1), InteriorBounds::new(-1, 0)]
        ),
        Err(Error::InvalidInteriorBounds { position: 0, .. })
    ));
    assert!(matches!(
        SectorInteriorDomain::try_new(
            sector,
            [InteriorBounds::new(0, 2), InteriorBounds::new(-1, 0)]
        ),
        Err(Error::InteriorOutsideSector { position: 0, .. })
    ));
}

#[test]
fn extreme_shifts_report_empty_interiors_without_overflow() {
    let active = Mask::try_new([true]).unwrap();
    assert_eq!(
        SectorInteriorDomain::try_maximal_for_shifts(active.clone(), &[[i64::MIN]]),
        Err(Error::EmptyShiftInterior { position: 0 })
    );
    assert_eq!(
        SectorInteriorDomain::try_maximal_for_shifts(active, &[[i64::MAX]]),
        Err(Error::EmptyShiftInterior { position: 0 })
    );

    let inactive = Mask::try_new([false]).unwrap();
    let min_only =
        SectorInteriorDomain::try_maximal_for_shifts(inactive.clone(), &[[i64::MIN]]).unwrap();
    assert_eq!(min_only.bounds(), &[InteriorBounds::new(0, 0)]);
    assert_eq!(
        min_only.checked_translate(&[0], &[i64::MIN]).unwrap(),
        Some(vec![i64::MIN])
    );
    assert_eq!(
        SectorInteriorDomain::try_maximal_for_shifts(inactive, &[[i64::MIN], [i64::MAX]]),
        Err(Error::EmptyShiftInterior { position: 0 })
    );
}

#[test]
fn active_tadpole_plus_one_to_zero_has_uniform_strict_descent() {
    let sector = Mask::try_new([true]).unwrap();
    let domain = SectorInteriorDomain::try_maximal_for_shifts(sector, &[[1], [0]]).unwrap();
    assert_eq!(domain.bounds(), &[InteriorBounds::new(1, i64::MAX - 1)]);

    let witness = OrderingPolicy::default()
        .prove_shift_strict_descent(&domain, &[1], &[0])
        .unwrap();
    assert_eq!(
        witness.decisive_component(),
        ComplexityComponent::CornerDistance
    );
    assert!(witness.verify());
    assert_eq!(witness.source().dot_offset(), 1);
    assert_eq!(witness.target().dot_offset(), 0);

    for anchor in [1, 2, 17, i64::MAX - 1] {
        let source = domain.checked_translate(&[anchor], &[1]).unwrap().unwrap();
        let target = domain.checked_translate(&[anchor], &[0]).unwrap().unwrap();
        assert_eq!(
            OrderingPolicy::default().compare(&target, &source).unwrap(),
            Ordering::Less
        );
    }
}

#[test]
fn negative_active_and_both_inactive_directions_tighten_the_right_bounds() {
    let active = Mask::try_new([true]).unwrap();
    let active_domain = SectorInteriorDomain::try_maximal_for_shifts(active, &[[-2], [0]]).unwrap();
    assert_eq!(active_domain.bounds(), &[InteriorBounds::new(3, i64::MAX)]);
    assert!(
        OrderingPolicy::default()
            .prove_shift_strict_descent(&active_domain, &[0], &[-2])
            .unwrap()
            .verify()
    );

    let inactive = Mask::try_new([false]).unwrap();
    let negative_domain =
        SectorInteriorDomain::try_maximal_for_shifts(inactive.clone(), &[[-1], [0]]).unwrap();
    assert_eq!(
        negative_domain.bounds(),
        &[InteriorBounds::new(i64::MIN + 1, 0)]
    );
    assert!(
        OrderingPolicy::default()
            .prove_shift_strict_descent(&negative_domain, &[-1], &[0])
            .unwrap()
            .verify()
    );

    let positive_domain =
        SectorInteriorDomain::try_maximal_for_shifts(inactive, &[[2], [0]]).unwrap();
    assert_eq!(
        positive_domain.bounds(),
        &[InteriorBounds::new(i64::MIN, -2)]
    );
    assert!(
        OrderingPolicy::default()
            .prove_shift_strict_descent(&positive_domain, &[0], &[2])
            .unwrap()
            .verify()
    );
}

#[test]
fn structural_key_handles_i64_extremes_and_signed_aggregate_offsets() {
    let sector = Mask::try_new([true, false, true, false]).unwrap();
    let key = OrderingPolicy::default()
        .shift_complexity_key(&sector, &[i64::MIN, i64::MIN, i64::MAX, i64::MAX])
        .unwrap();
    assert_eq!(
        key.index_excess_offsets(),
        &[
            i128::from(i64::MIN),
            -i128::from(i64::MIN),
            i128::from(i64::MAX),
            -i128::from(i64::MAX),
        ]
    );
    assert_eq!(key.dot_offset(), -1);
    assert_eq!(key.numerator_offset(), 1);
    assert_eq!(key.corner_distance_offset(), 0);
    for position in 0..4 {
        assert_eq!(
            key.shift_at(position).unwrap(),
            [i64::MIN, i64::MIN, i64::MAX, i64::MAX][position]
        );
    }
}

#[test]
fn wrong_arity_uncovered_shifts_and_nondescents_are_typed_errors() {
    let sector = Mask::try_new([true, false]).unwrap();
    assert!(matches!(
        SectorInteriorDomain::try_maximal_for_shifts(sector.clone(), &[[0_i64; 1]]),
        Err(Error::WrongArity { .. })
    ));
    assert!(matches!(
        OrderingPolicy::default().shift_complexity_key(&sector, &[0]),
        Err(Error::WrongArity { .. })
    ));

    let broad = SectorInteriorDomain::try_new(
        sector,
        [
            InteriorBounds::new(1, i64::MAX),
            InteriorBounds::new(i64::MIN, 0),
        ],
    )
    .unwrap();
    assert_eq!(
        OrderingPolicy::default().prove_shift_strict_descent(&broad, &[1, 0], &[0, 0]),
        Err(Error::ShiftNotCovered {
            position: 0,
            shift: 1
        })
    );
    assert_eq!(
        OrderingPolicy::default().compare_shifts_on_domain(&broad, &[0, 0], &[0, -1]),
        Err(Error::ShiftNotCovered {
            position: 1,
            shift: -1
        })
    );

    let domain =
        SectorInteriorDomain::try_maximal_for_shifts(Mask::try_new([true]).unwrap(), &[[0], [1]])
            .unwrap();
    assert_eq!(
        OrderingPolicy::default().prove_shift_strict_descent(&domain, &[0], &[1]),
        Err(Error::NotStrictDescent)
    );
}

#[test]
fn exhaustive_finite_interiors_match_concrete_v1_order_and_verify_witnesses() {
    let policy = OrderingPolicy::default();
    for bits in 0_u8..4 {
        let sector = Mask::try_new([bits & 2 != 0, bits & 1 != 0]).unwrap();
        let bounds = sector.active_bits().iter().map(|&active| {
            if active {
                InteriorBounds::new(1, 4)
            } else {
                InteriorBounds::new(-3, 0)
            }
        });
        let domain = SectorInteriorDomain::try_new(sector.clone(), bounds).unwrap();
        let shifts = (-2_i64..=2)
            .flat_map(|left| (-2_i64..=2).map(move |right| [left, right]))
            .filter(|shift| domain.covers_shift(shift).unwrap())
            .collect::<Vec<_>>();

        for source in &shifts {
            for target in &shifts {
                let structural = policy
                    .compare_shifts_on_domain(&domain, target, source)
                    .unwrap();
                let proof = policy.prove_shift_strict_descent(&domain, source, target);
                assert_eq!(proof.is_ok(), structural == Ordering::Less);
                if let Ok(witness) = proof {
                    assert!(witness.verify());
                }
                for first in domain.bounds()[0].lower()..=domain.bounds()[0].upper() {
                    for second in domain.bounds()[1].lower()..=domain.bounds()[1].upper() {
                        let anchor = [first, second];
                        let concrete_source =
                            domain.checked_translate(&anchor, source).unwrap().unwrap();
                        let concrete_target =
                            domain.checked_translate(&anchor, target).unwrap().unwrap();
                        assert_eq!(
                            policy.compare(&concrete_target, &concrete_source).unwrap(),
                            structural
                        );
                    }
                }
            }
        }
    }
}
