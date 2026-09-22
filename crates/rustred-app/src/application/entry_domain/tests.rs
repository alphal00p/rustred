use super::*;
use std::collections::BTreeSet;

fn key(powers: &[i64]) -> IntegralKey {
    IntegralKey::try_new(powers.iter().copied()).unwrap()
}

fn budget(a: u32, r: u32) -> EntryPowerBudget {
    EntryPowerBudget {
        max_positive_power: a,
        max_numerator_rank: r,
        min_power_difference: None,
        max_power_difference: None,
    }
}

#[test]
fn finite_envelope_exhaustive_membership_and_symbolica_count_agree() {
    for mask in 0..8 {
        let support: Vec<_> = (0..3).map(|i| mask & (1 << i) != 0).collect();
        for lower in [None, Some(-1), Some(2), Some(6)] {
            for upper in [None, Some(-2), Some(0), Some(2), Some(6)] {
                let mut b = budget(4, 3);
                b.min_power_difference = lower;
                b.max_power_difference = upper;
                if lower.zip(upper).is_some_and(|(l, u)| l > u) {
                    assert!(FiniteEntryDomain::new(support.clone(), b).is_err());
                    continue;
                }
                let domain = FiniteEntryDomain::new(support.clone(), b).unwrap();
                let emitted: Vec<_> = domain.targets().collect::<Result<_, _>>().unwrap();
                let actual: BTreeSet<_> = emitted.iter().cloned().collect();
                assert_eq!(actual.len(), emitted.len(), "no duplicate seeds");
                let mut expected = BTreeSet::new();
                for a in -3..=4 {
                    for b in -3..=4 {
                        for c in -3..=4 {
                            let target = key(&[a, b, c]);
                            if domain.contains(&target) {
                                expected.insert(target);
                            }
                        }
                    }
                }
                assert_eq!(actual, expected, "mask={mask}, lower={lower:?}");
                assert_eq!(
                    domain.target_count(5).unwrap(),
                    Integer::from(actual.len() as u64)
                );
                assert_eq!(
                    emitted,
                    domain.targets().collect::<Result<Vec<_>, _>>().unwrap()
                );
            }
        }
    }
}

#[test]
fn finite_envelope_retains_total_power_not_uniform_dots_per_sector() {
    let domain = FiniteEntryDomain::new(vec![true, false, true], budget(4, 2)).unwrap();
    assert!(domain.contains(&key(&[3, -2, 1])));
    assert!(!domain.contains(&key(&[3, -2, 2])));
    assert!(!domain.contains(&key(&[3, -3, 1])));
    assert!(!domain.contains(&key(&[3, 1, 0])));
    assert!(!domain.contains(&key(&[1, 0, i64::MAX])));
    assert!(!domain.contains(&key(&[1, i64::MIN, 1])));
    assert!(!domain.contains(&key(&[1])));
}

#[test]
fn finite_envelope_zero_degree_empty_support_and_admission() {
    let domain = FiniteEntryDomain::new(vec![true, true], budget(2, 0)).unwrap();
    assert_eq!(
        domain.targets().collect::<Result<Vec<_>, _>>().unwrap(),
        vec![key(&[1, 1])]
    );
    let empty = FiniteEntryDomain::new(vec![false, false], budget(0, 0)).unwrap();
    assert_eq!(
        empty.targets().collect::<Result<Vec<_>, _>>().unwrap(),
        vec![key(&[0, 0])]
    );
    assert!(domain.target_count(0).is_err());
    assert!(FiniteEntryDomain::new(vec![], budget(2, 0)).is_err());
    let mut b = budget(2, 0);
    b.min_power_difference = Some(2);
    b.max_power_difference = Some(1);
    assert!(FiniteEntryDomain::new(vec![true], b).is_err());
}

#[test]
fn finite_envelope_count_exceeds_machine_integer_without_overflow() {
    let domain = FiniteEntryDomain::new(vec![true; 40], budget(80, 0)).unwrap();
    assert_eq!(domain.target_count(41).unwrap(), Integer::binom(80, 40));
    assert!(domain.target_count(40).is_err());
    assert_eq!(domain.targets().take(3).count(), 3);
}

#[test]
fn finite_envelope_correlated_band_excludes_rectangular_extras() {
    let mut b = budget(5, 4);
    b.min_power_difference = Some(2);
    b.max_power_difference = Some(2);
    let domain = FiniteEntryDomain::new(vec![true, false], b).unwrap();
    assert_eq!(
        domain.targets().collect::<Result<Vec<_>, _>>().unwrap(),
        vec![key(&[2, 0]), key(&[3, -1]), key(&[4, -2]), key(&[5, -3])]
    );
}

#[test]
fn finite_envelope_prunes_impossible_positive_layers_before_iteration() {
    let mut b = budget(u32::MAX, 0);
    b.min_power_difference = Some(0);
    b.max_power_difference = Some(0);
    let empty = FiniteEntryDomain::new(vec![true], b).unwrap();
    assert!(empty.targets().next().is_none());
    assert_eq!(empty.target_count(0).unwrap(), Integer::from(0));
    b.min_power_difference = Some(i32::MAX);
    b.max_power_difference = Some(i32::MAX);
    let high = FiniteEntryDomain::new(vec![true], b).unwrap();
    assert_eq!(
        high.targets().next().unwrap().unwrap(),
        key(&[i64::from(i32::MAX)])
    );
    assert_eq!(high.target_count(1).unwrap(), Integer::from(1));
}

#[test]
fn finite_envelope_physical_profile_is_generic_and_checked() {
    for loops in [1, 2, 3, 4, 5, 6, 10, 100] {
        let b = EntryPowerBudget::renormalizable_marginal_feynman(loops).unwrap();
        assert_eq!(b.max_positive_power, 5 * loops - 1);
        assert_eq!(b.max_numerator_rank, 3 * loops - 1);
        assert_eq!(b.min_power_difference, Some(2 * loops as i32));
    }
    assert!(EntryPowerBudget::renormalizable_marginal_feynman(0).is_err());
    assert!(EntryPowerBudget::renormalizable_marginal_feynman(u32::MAX).is_err());
}
