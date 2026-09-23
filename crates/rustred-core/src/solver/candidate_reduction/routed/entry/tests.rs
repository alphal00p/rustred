use super::*;

fn key(powers: &[i64]) -> IntegralKey {
    IntegralKey::try_new(powers.iter().copied()).unwrap()
}
fn region<const N: usize>(support: [bool; N]) -> RootRegionInput<N> {
    RootRegionInput {
        support,
        lower: vec![0; N],
        upper: vec![None; N],
        rank: None,
        powers: DomainPowerBounds::default(),
    }
}
fn policy<const N: usize>(
    inputs: Vec<RootRegionInput<N>>,
) -> Result<FiniteRootAdmission<N>, RootAdmissionError> {
    FiniteRootAdmission::try_new(inputs, 1000)
}

#[test]
fn exact_membership_matches_small_integer_enumeration() {
    // Independent bounded integer oracle, not a second production geometry
    // implementation. All supports, signs and both D endpoints are exercised.
    let mut comparisons = 0;
    for mask in 0..8 {
        let support = std::array::from_fn(|axis| mask & (1 << axis) != 0);
        for a in 0..=5 {
            for r in 0..=3 {
                for (dmin, dmax) in [
                    (None, None),
                    (Some(0), Some(0)),
                    (Some(2), None),
                    (None, Some(-1)),
                    (Some(-2), Some(3)),
                ] {
                    let mut input = region::<3>(support);
                    input.rank = Some(r);
                    input.powers = DomainPowerBounds {
                        max_positive_power: Some(a),
                        min_power_difference: dmin,
                        max_power_difference: dmax,
                    };
                    let native = policy(vec![input]);
                    assert!(matches!(
                        &native,
                        Ok(_) | Err(RootAdmissionError::EmptyRegion { .. })
                    ));
                    for n0 in -4..=6 {
                        for n1 in -4..=6 {
                            for n2 in -4..=6 {
                                let powers: [i64; 3] = [n0, n1, n2];
                                let positive: i64 = powers.iter().filter(|&&n| n > 0).sum();
                                let rank: i64 = powers.iter().filter(|&&n| n < 0).map(|n| -n).sum();
                                let d = positive - rank;
                                let expected = powers.map(|n| n > 0) == support
                                    && positive <= a as i64
                                    && rank <= i64::from(r)
                                    && dmin.is_none_or(|min| d >= min)
                                    && dmax.is_none_or(|max| d <= max);
                                let actual = native
                                    .as_ref()
                                    .is_ok_and(|gate| gate.validate_entry(&key(&powers)).is_ok());
                                assert_eq!(
                                    actual, expected,
                                    "support={support:?} A={a} R={r} D={dmin:?}..{dmax:?} target={powers:?}"
                                );
                                comparisons += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(comparisons, 1_277_760);
}

#[test]
fn finiteness_uses_projected_extrema_not_presence_of_named_caps() {
    let mut ar = region([true, false]);
    ar.rank = Some(4);
    ar.powers.max_positive_power = Some(6);
    assert!(policy(vec![ar]).is_ok());
    // A<=6 and D>=1 imply R<=5 even without an R cap.
    let mut ad = region([true, false]);
    ad.powers.max_positive_power = Some(6);
    ad.powers.min_power_difference = Some(1);
    let gate = policy(vec![ad]).unwrap();
    assert!(gate.validate_entry(&key(&[6, -5])).is_ok());
    assert!(gate.validate_entry(&key(&[6, -6])).is_err());
    // R<=4 and D<=3 imply A<=7 even without an A cap.
    let mut rd = region([true, false]);
    rd.rank = Some(4);
    rd.powers.max_power_difference = Some(3);
    assert!(
        policy(vec![rd])
            .unwrap()
            .validate_entry(&key(&[7, -4]))
            .is_ok()
    );
    let mut boxed = region([true, false]);
    boxed.upper = vec![Some(4), Some(5)];
    assert!(policy(vec![boxed]).is_ok());
    // D=0 leaves an infinite diagonal, despite equality at both endpoints.
    let mut diagonal = region([true, false]);
    diagonal.powers.min_power_difference = Some(0);
    diagonal.powers.max_power_difference = Some(0);
    assert!(matches!(
        policy(vec![diagonal]),
        Err(RootAdmissionError::UnboundedRegion { .. })
    ));
    let mut rank_only = region([true, false]);
    rank_only.rank = Some(10);
    assert!(matches!(
        policy(vec![rank_only]),
        Err(RootAdmissionError::UnboundedRegion { axis: 0, .. })
    ));
}

#[test]
fn union_keeps_holes_and_selected_supports() {
    let mut a = region([true, false]);
    a.upper = vec![Some(2), Some(1)];
    let mut b = region([true, false]);
    b.lower = vec![4, 2];
    b.upper = vec![Some(5), Some(3)];
    let mut c = region([false, true]);
    c.upper = vec![Some(1), Some(2)];
    let gate = policy(vec![c, b, a]).unwrap();
    assert_eq!(gate.region_count(), 3);
    assert_eq!(gate.regions().len(), 3);
    for powers in [[1, 0], [3, -1], [5, -2], [6, -3], [-1, 3]] {
        assert!(gate.validate_entry(&key(&powers)).is_ok());
    }
    for powers in [[4, -1], [4, -2], [2, -3], [5, 0]] {
        assert!(matches!(
            gate.validate_entry(&key(&powers)),
            Err(RootAdmissionError::OutsideStartingDomain { .. })
        ));
    }
    assert!(matches!(
        gate.validate_entry(&key(&[1, 1])),
        Err(RootAdmissionError::OutsideSelectedSupport { .. })
    ));
}

#[test]
fn malformed_empty_unbounded_and_capacity_fail_typed() {
    let mut input = region([true, false]);
    input.upper = vec![Some(1); 2];
    assert!(
        FiniteRootAdmission::try_new([input.clone()], 1)
            .unwrap()
            .validate_entry(&key(&[1, 0]))
            .is_ok()
    );
    assert!(matches!(
        FiniteRootAdmission::try_new([input.clone()], 0),
        Err(RootAdmissionError::RegionAllowance { limit: 0 })
    ));
    assert!(matches!(
        policy::<2>(vec![]),
        Err(RootAdmissionError::EmptyPolicy)
    ));
    let mut malformed = input.clone();
    malformed.lower.pop();
    assert!(matches!(
        policy(vec![malformed]),
        Err(RootAdmissionError::InvalidRegion {
            error: DomainPowerError::InvalidArity,
            ..
        })
    ));
    let mut inverted = input.clone();
    inverted.lower[1] = 2;
    assert!(matches!(
        policy(vec![inverted]),
        Err(RootAdmissionError::InvalidRegion {
            error: DomainPowerError::InvertedCoordinate { axis: 1 },
            ..
        })
    ));
    let mut inverted_d = input.clone();
    inverted_d.powers.min_power_difference = Some(2);
    inverted_d.powers.max_power_difference = Some(1);
    assert!(matches!(
        policy(vec![inverted_d]),
        Err(RootAdmissionError::InvalidRegion {
            error: DomainPowerError::InvertedDifferenceBounds,
            ..
        })
    ));
    // An empty intersection is not mislabeled as an infinite domain.
    let mut empty = region([true, false]);
    empty.powers.max_positive_power = Some(0);
    assert!(matches!(
        policy(vec![empty]),
        Err(RootAdmissionError::EmptyRegion { region: 0 })
    ));
    let gate = policy(vec![input]).unwrap();
    assert_eq!(
        gate.validate_entry(&key(&[1])),
        Err(RootAdmissionError::WrongTargetArity {
            expected: 2,
            actual: 1
        })
    );
}

#[test]
fn machine_extreme_integral_powers_are_not_infinity_or_clipped() {
    let mut input = region([true, false]);
    input.upper = vec![Some(u64::MAX); 2];
    let gate = policy(vec![input]).unwrap();
    assert!(gate.validate_entry(&key(&[i64::MAX, i64::MIN])).is_ok());
    let mut restricted = region([true, false]);
    restricted.upper = vec![Some(u64::MAX); 2];
    restricted.rank = Some(u32::MAX);
    assert!(
        policy(vec![restricted])
            .unwrap()
            .validate_entry(&key(&[i64::MAX, i64::MIN]))
            .is_err()
    );
}

#[test]
fn a_r_d_are_joint_starting_constraints_not_a_descendant_budget() {
    let mut input = region([true, false]);
    input.rank = Some(15);
    input.powers.max_positive_power = Some(24);
    input.powers.min_power_difference = Some(9);
    let gate = policy(vec![input]).unwrap();
    assert!(gate.validate_entry(&key(&[24, -15])).is_ok());
    for powers in [[22, -16], [25, -15], [23, -15]] {
        assert!(matches!(
            gate.validate_entry(&key(&powers)),
            Err(RootAdmissionError::OutsideStartingDomain { .. })
        ));
    }
}
