use super::*;

mod build;

fn stop() -> AtomicBool {
    AtomicBool::new(false)
}
fn domain(min: Option<i64>, max: Option<i64>, a: Option<u64>) -> Domain<3> {
    Domain {
        phase: Phase::Apply,
        owner: [true, true, false],
        lower: vec![0; 3],
        upper: vec![Some(2), Some(2), Some(3)],
        rank: Some(3),
        powers: DomainPowerBounds {
            max_positive_power: a,
            min_power_difference: min,
            max_power_difference: max,
        },
    }
}
fn contains(d: &Domain<3>, p: [u64; 3]) -> bool {
    let a = p[0] + p[1] + 2;
    let r = p[2];
    let diff = i128::from(a) - i128::from(r);
    p.iter()
        .enumerate()
        .all(|(i, &x)| x >= d.lower[i] && d.upper[i].is_none_or(|u| x <= u))
        && d.rank.is_none_or(|bound| r <= u64::from(bound))
        && d.powers.max_positive_power.is_none_or(|bound| a <= bound)
        && d.powers
            .min_power_difference
            .is_none_or(|bound| diff >= i128::from(bound))
        && d.powers
            .max_power_difference
            .is_none_or(|bound| diff <= i128::from(bound))
}

#[test]
fn initial_overlap_exact_partition_preserves_correlations_exhaustively() {
    let anchor = Arc::new(domain(Some(1), None, Some(6)));
    let index = InitialOverlapIndex::from_initial(&[anchor.clone()], &stop());
    let mut hits = 0;
    for min in -3..=2 {
        for max in min..=6 {
            for a in 2..=6 {
                for rank in 0..=3 {
                    let mut q = domain(Some(min), Some(max), Some(a));
                    q.rank = Some(rank);
                    let Some(plan) = index.plan(&q, &stop()) else {
                        continue;
                    };
                    hits += 1;
                    assert_eq!(plan.residual.lower, q.lower);
                    assert_eq!(plan.residual.upper, q.upper);
                    assert_eq!(plan.residual.rank, q.rank);
                    assert_eq!(
                        plan.residual.powers.max_positive_power,
                        q.powers.max_positive_power
                    );
                    let mut hi = q.clone();
                    hi.powers.min_power_difference = Some(min.max(plan.scope.cut));
                    for x in 0..=2 {
                        for y in 0..=2 {
                            for z in 0..=3 {
                                let p = [x, y, z];
                                let high = contains(&hi, p);
                                let low = contains(&plan.residual, p);
                                assert!(!(high && low));
                                assert_eq!(contains(&q, p), high || low);
                                if high {
                                    assert!(contains(&anchor, p));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(hits > 10);
}

#[test]
fn initial_overlap_rejects_coordinate_only_containment_and_bad_identity() {
    let anchor = Arc::new(domain(Some(1), None, Some(4)));
    let index = InitialOverlapIndex::from_initial(&[anchor.clone()], &stop());
    assert!(
        index
            .plan(&domain(Some(-1), None, Some(6)), &stop())
            .is_none()
    );
    assert!(index.plan(&anchor, &stop()).is_none());
    let good = domain(Some(-1), None, Some(4));
    assert!(index.plan(&good, &stop()).is_some());
    let mut route = good.clone();
    route.phase = Phase::Route;
    assert!(index.plan(&route, &stop()).is_none());
    let mut other = good;
    other.owner = [true, false, true];
    assert!(index.plan(&other, &stop()).is_none());
}

#[test]
fn initial_overlap_native_implied_cut_is_safe_and_first_anchor_stable() {
    let mut implied_safe = domain(None, None, Some(2));
    implied_safe.rank = Some(1);
    let idx = InitialOverlapIndex::from_initial(&[Arc::new(implied_safe)], &stop());
    let safe = idx.plan(&domain(Some(-1), None, Some(2)), &stop()).unwrap();
    assert_eq!(safe.scope.cut, 1);
    let mut implied = domain(None, None, Some(6));
    implied.upper[2] = Some(1);
    let index = InitialOverlapIndex::from_initial(&[Arc::new(implied)], &stop());
    // Projected D_min=1, but high points with R>1 are not in this anchor.
    assert!(
        index
            .plan(&domain(Some(-1), None, Some(6)), &stop())
            .is_none()
    );
    let mut anchored = domain(None, None, Some(6));
    anchored.lower[0] = 1;
    let query = Domain {
        lower: vec![1, 0, 0],
        ..domain(Some(-1), None, Some(6))
    };
    // A coordinate-implied D minimum can be stronger than the raw descriptor;
    // same raw coordinates cannot generate a low band, so retain full work.
    let idx = InitialOverlapIndex::from_initial(&[Arc::new(anchored)], &stop());
    assert!(idx.plan(&query, &stop()).is_none());
    let a = Arc::new(domain(Some(1), None, Some(6)));
    let b = Arc::new(domain(Some(2), None, Some(6)));
    let idx = InitialOverlapIndex::from_initial(&[b, a], &stop());
    let p = idx.plan(&domain(Some(-1), None, Some(6)), &stop()).unwrap();
    assert_eq!(p.scope.anchor_id, 0);
    assert_eq!(p.scope.cut, 2);
}

#[test]
fn initial_overlap_queue_pins_actual_deduplicated_initial_prefix() {
    use super::super::{delegation::SchedulingPolicy, queue::Queue};
    use std::num::NonZeroUsize;
    let mut q = Queue::with_policy(
        100,
        None,
        SchedulingPolicy::TransferUnreserved {
            lookahead: NonZeroUsize::MIN,
        },
    )
    .unwrap();
    q.delegation
        .as_mut()
        .unwrap()
        .begin_initial_admission()
        .unwrap();
    let narrow = domain(Some(4), Some(4), Some(6));
    let wider = domain(Some(3), Some(5), Some(6));
    let broad = domain(Some(2), Some(6), Some(6));
    assert_eq!(q.admit(narrow.clone()).unwrap(), (0, true));
    assert_eq!(q.admit(wider).unwrap(), (1, true));
    assert_eq!(q.admit(broad).unwrap(), (2, true));
    assert_eq!(q.admit(narrow).unwrap(), (0, false));
    q.delegation
        .as_mut()
        .unwrap()
        .finish_initial_admission()
        .unwrap();
    assert_eq!(q.delegation.as_ref().unwrap().initial_prefix(), Some(3));
    q.admit(domain(None, None, Some(6))).unwrap();
    assert!(q.containment_retired_candidates >= 3);
    let ledger = q.delegation.as_ref().unwrap();
    assert_eq!(ledger.transfer_count(), 0);
    assert_eq!(ledger.dispatch_fence(), 1);
    for id in 0..3 {
        assert_eq!(ledger.delegated_to(id), None);
    }
}

#[test]
fn initial_overlap_empty_bands_malformed_inputs_and_optional_caps_fall_back() {
    let anchor = Arc::new(domain(Some(1), None, Some(6)));
    let initial = [anchor.clone()];
    let index = InitialOverlapIndex::from_initial(&initial, &stop());
    for q in [
        domain(Some(2), None, Some(6)),
        domain(None, Some(0), Some(6)),
        domain(Some(2), Some(1), Some(6)),
    ] {
        assert!(index.plan(&q, &stop()).is_none());
    }
    let mut invalid = domain(Some(-1), None, Some(6));
    invalid.lower[0] = 3;
    assert!(index.plan(&invalid, &stop()).is_none());
    let q = domain(Some(-1), None, Some(6));
    for (count, bytes) in [(0, usize::MAX), (100, 0)] {
        assert!(
            InitialOverlapIndex::with_limits(&initial, &stop(), count, bytes)
                .plan(&q, &stop())
                .is_none()
        );
    }
    assert!(
        InitialOverlapIndex::from_initial(&initial, &AtomicBool::new(true))
            .plan(&q, &stop())
            .is_none()
    );
    assert!(index.plan(&q, &AtomicBool::new(true)).is_none());
}

#[test]
fn initial_overlap_unrepresentable_and_minimum_signed_cuts_do_not_wrap() {
    // Native sums exceed i64 without invalidating the original u64 coordinates.
    let huge = Domain::<1> {
        phase: Phase::Apply,
        owner: [true],
        lower: vec![i64::MAX as u64],
        upper: vec![None],
        rank: None,
        powers: Default::default(),
    };
    assert!(
        InitialOverlapIndex::from_initial(&[Arc::new(huge)], &stop())
            .anchors
            .is_empty()
    );
    let min = Domain::<1> {
        phase: Phase::Apply,
        owner: [false],
        lower: vec![0],
        upper: vec![Some(1_u64 << 63)],
        rank: None,
        powers: Default::default(),
    };
    assert!(
        InitialOverlapIndex::from_initial(&[Arc::new(min)], &stop())
            .anchors
            .is_empty()
    );
}
