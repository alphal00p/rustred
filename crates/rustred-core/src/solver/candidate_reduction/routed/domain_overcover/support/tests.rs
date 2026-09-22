use super::*;

#[test]
fn degree_cap_matches_exhaustive_finite_source_assignment() {
    let mut cases = 0;
    for l0 in 0..=2_u64 {
        for l1 in 0..=2_u64 {
            for l2 in 0..=2_u64 {
                let lower = [l0, l1, l2];
                let lower_sum: u128 = lower.iter().copied().map(u128::from).sum();
                for u0 in l0..=3 {
                    for u1 in l1..=3 {
                        for u2 in l2..=3 {
                            let upper = [Some(u0), Some(u1), Some(u2)];
                            for rank in [None, Some(0), Some(1), Some(3), Some(6), Some(9)] {
                                if rank.is_some_and(|rank| rank < lower_sum) {
                                    continue;
                                }
                                for mask in 0..8 {
                                    let rows: Vec<_> =
                                        (0..3).filter(|axis| mask & (1 << axis) != 0).collect();
                                    let mut actual = 0_u128;
                                    for t0 in l0..=u0 {
                                        for t1 in l1..=u1 {
                                            for t2 in l2..=u2 {
                                                let values = [t0, t1, t2];
                                                let total = values
                                                    .iter()
                                                    .copied()
                                                    .map(u128::from)
                                                    .sum::<u128>();
                                                if rank.is_none_or(|rank| total <= rank) {
                                                    actual = actual.max(
                                                        rows.iter()
                                                            .map(|&i| u128::from(values[i]))
                                                            .sum(),
                                                    );
                                                }
                                            }
                                        }
                                    }
                                    assert_eq!(
                                        column_cap(&lower, &upper, lower_sum, rank, &rows).unwrap(),
                                        Some(actual),
                                        "lower={lower:?}, upper={upper:?}, rank={rank:?}, rows={rows:?}",
                                    );
                                    cases += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(cases > 10_000);
}

#[test]
fn degree_cap_distinguishes_empty_support_infinity_and_wide_finite_sums() {
    let lower = [0, 3, 4];
    assert_eq!(
        column_cap(&lower, &[None; 3], 7, None, &[]).unwrap(),
        Some(0)
    );
    assert_eq!(column_cap(&lower, &[None; 3], 7, None, &[0]).unwrap(), None);
    assert_eq!(
        column_cap(&lower, &[None; 3], 7, Some(12), &[0]).unwrap(),
        Some(5)
    );
    assert_eq!(
        column_cap(&lower, &[Some(2), None, None], 7, None, &[0]).unwrap(),
        Some(2)
    );
    assert_eq!(
        column_cap(&[0; 3], &[Some(u64::MAX); 3], 0, None, &[0, 1, 2]).unwrap(),
        Some(3 * u128::from(u64::MAX))
    );
    assert!(matches!(
        checked_sum(u128::MAX, 1),
        Err(CandidateDomainRouteFailure::CountOverflow { .. })
    ));
}

#[test]
fn surviving_lower_and_pinch_threshold_do_not_narrow_degree_caps() {
    let degrees = NumeratorDegrees {
        caps: [Some(2), Some(u128::from(u64::MAX) + 1), None, Some(0)],
    };
    assert_eq!(degrees.surviving_lower(0, 5), 3);
    assert_eq!(degrees.surviving_lower(1, u64::MAX), 0);
    assert_eq!(degrees.surviving_lower(2, 9), 0);
    assert_eq!(degrees.surviving_lower(3, 9), 9);
    assert!(!degrees.can_pinch(0, 2));
    assert!(degrees.can_pinch(0, 1));
    assert!(degrees.can_pinch(1, u64::MAX));
    assert!(degrees.can_pinch(2, u64::MAX));
    assert!(!degrees.can_pinch(3, 0));
}
