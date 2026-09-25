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

#[test]
fn joint_degree_cap_rejects_invalid_indices_duplicates_and_lower_accounting() {
    for rows in [vec![3], vec![usize::MAX], vec![0, 0], vec![2, 1]] {
        assert!(matches!(
            column_cap(&[0; 3], &[None; 3], 0, None, &rows),
            Err(CandidateDomainRouteFailure::InvalidAdmittedRoute(_))
        ));
    }
    assert!(matches!(
        column_cap(&[1; 3], &[None; 3], 0, None, &[0]),
        Err(CandidateDomainRouteFailure::InvalidAdmittedRoute(_))
    ));
    assert!(matches!(
        column_cap(&[1; 3], &[None; 3], 3, Some(1), &[0]),
        Err(CandidateDomainRouteFailure::InvalidDomain(_))
    ));
    let cost = 2 * (u128::from(u64::MAX) + 1);
    let cap = column_cap(&[0; 2], &[Some(u64::MAX); 2], 0, None, &[0, 1])
        .unwrap()
        .unwrap();
    assert!(cost > cap);
}

#[test]
fn joint_degree_bound_remains_necessary_with_exact_symbolica_cancellation() {
    use crate::family::{
        IntegralKey,
        numerator_expansion::{MultiAffineNumeratorFactor, try_expand_multi_affine_numerator},
    };
    let family = crate::solver::tests::sunset();
    let c = family.coefficient_context();
    // (1+y0+y1)(1+y0-y1): the mixed y0*y1 endpoint cancels exactly.
    let factors = [
        MultiAffineNumeratorFactor::try_new(c.one(), [c.one(), c.one(), c.zero()], 1).unwrap(),
        MultiAffineNumeratorFactor::try_new(c.one(), [c.one(), c.integer(-1), c.zero()], 1)
            .unwrap(),
    ];
    let cap = column_cap(&[1, 1], &[Some(1), Some(1)], 2, None, &[0, 1])
        .unwrap()
        .unwrap();
    assert_eq!(cap, 2);
    for base in [[1, 1, 0], [2, 1, 0]] {
        let endpoints = try_expand_multi_affine_numerator(
            &family,
            &IntegralKey::try_new(base).unwrap(),
            &factors,
            Default::default(),
        )
        .unwrap();
        assert_eq!(endpoints.len(), 4);
        assert!(
            !endpoints
                .iter()
                .any(|endpoint| endpoint.key().powers() == [base[0] - 1, base[1] - 1, 0])
        );
        for endpoint in &endpoints {
            let powers = endpoint.key().powers();
            let removed_degree = (base[0] - powers[0]) + (base[1] - powers[1]);
            assert!(removed_degree as u128 <= cap);
        }
        if (base[0] + base[1]) as u128 > cap {
            assert!(!endpoints.iter().any(|endpoint| {
                let powers = endpoint.key().powers();
                powers[0] <= 0 && powers[1] <= 0
            }));
        }
    }
}
