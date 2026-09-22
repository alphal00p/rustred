use super::*;
use rustred::solver::DomainPowerBounds;

fn signature(a: u128) -> Signature {
    Signature::Nonempty {
        positive: Upper::Finite(a),
        numerator: Upper::Infinity,
        difference: Lower::NegativeInfinity,
    }
}

fn add(index: &mut AggregateIndex, signature: Signature, id: usize) {
    let insertion = index.prepare(signature).unwrap();
    index.insert(insertion, id);
}

#[test]
fn typed_infinity_empty_and_wide_features_are_not_finite_sentinels() {
    let wide = Signature::Nonempty {
        positive: Upper::Finite(u128::MAX),
        numerator: Upper::Finite(u128::from(u64::MAX) + 1),
        difference: Lower::Finite(i128::MIN),
    };
    let unbounded = Signature::Nonempty {
        positive: Upper::Infinity,
        numerator: Upper::Infinity,
        difference: Lower::NegativeInfinity,
    };
    assert!(unbounded.may_contain(wide));
    assert!(!wide.may_contain(unbounded));
    assert!(wide.may_contain(Signature::Empty));
    assert!(Signature::Empty.may_contain(Signature::Empty));
    assert!(!Signature::Empty.may_contain(wide));

    let summary = DomainPowerSummary::try_new(
        [true; 3],
        &[0; 3],
        &[Some(u64::MAX); 3],
        None,
        DomainPowerBounds::default(),
    )
    .unwrap();
    assert!(matches!(Signature::of(&summary), Signature::Nonempty {
        positive: Upper::Finite(a), ..
    } if a == 3 * (u128::from(u64::MAX) + 1)));
}

#[test]
fn necessary_filter_has_no_false_negatives_for_small_native_geometries() {
    let mut summaries = Vec::new();
    for owner in [[true, false], [false, true], [true; 2], [false; 2]] {
        for lower in [[0, 0], [1, 0], [0, 1], [1, 1]] {
            for upper in [[Some(1); 2], [None; 2], [Some(2), None], [None, Some(2)]] {
                for rank in [Some(0), Some(2), None] {
                    for (a, d) in [(None, None), (Some(3), Some(1)), (Some(0), None)] {
                        summaries.push(
                            DomainPowerSummary::try_new(
                                owner,
                                &lower,
                                &upper,
                                rank,
                                DomainPowerBounds {
                                    max_positive_power: a,
                                    min_power_difference: d,
                                    max_power_difference: None,
                                },
                            )
                            .unwrap(),
                        );
                    }
                }
            }
        }
    }
    let mut implications = 0;
    for a in &summaries {
        for b in &summaries {
            if a.contains(b) {
                assert!(Signature::of(a).may_contain(Signature::of(b)));
                implications += 1;
            }
        }
    }
    assert!(implications > 10_000);
}

#[test]
fn minimum_id_survives_group_swap_removal_and_same_signature_replacement() {
    let mut index = AggregateIndex::default();
    for id in 0..3 {
        add(&mut index, signature(id as u128 + 1), id);
    }
    // Retire group 0; swap_remove moves ID 2's group ahead of ID 1's group.
    let insertion = index.prepare(signature(4)).unwrap();
    assert_eq!(index.retire(&insertion, |id| id == 0), 1);
    index.insert(insertion, 3);
    assert_eq!(index.groups[0].ids, [2]);
    assert_eq!(index.find(signature(0), |_| Ok(true)).unwrap(), Some(1));
    assert_eq!(index.find(Signature::Empty, |_| Ok(true)).unwrap(), Some(1));

    let insertion = index.prepare(signature(2)).unwrap();
    assert_eq!(index.retire(&insertion, |id| id == 1), 1);
    index.insert(insertion, 4); // Reserved empty same-signature group survives.
    assert_eq!(index.ids(), [2, 3, 4]);
    assert_eq!(index.live, 3);
    assert_eq!(index.groups(), 3);
    for (position, group) in index.groups.iter().enumerate() {
        assert_eq!(index.positions[&group.signature], position);
        assert!(!group.ids.is_empty());
    }
}

#[test]
fn reverse_filter_skips_ineligible_ids_and_both_directions_charge_group_work() {
    let mut index = AggregateIndex::default();
    add(&mut index, signature(1), 0);
    add(&mut index, signature(3), 1);
    assert_eq!(
        index
            .find(signature(2), |id| {
                assert_eq!(id, 1);
                Ok(false)
            })
            .unwrap(),
        None
    );
    assert_eq!(index.maintenance_len(signature(2)).unwrap(), 1);
    let insertion = index.prepare(signature(2)).unwrap();
    assert_eq!(
        index.retire(&insertion, |id| {
            assert_eq!(id, 0);
            true
        }),
        1
    );
    index.insert(insertion, 2);
    assert_eq!(index.ids(), [1, 2]);
    let work = index.work();
    assert_eq!(work.groups_visited, 6);
    assert_eq!(work.groups_rejected, 3);
}

#[test]
fn failed_storage_preflight_keeps_group_keys_membership_and_ids() {
    let mut index = AggregateIndex::default();
    add(&mut index, signature(1), 0);
    for (key, checkpoints) in [(signature(1), 1), (signature(2), 3)] {
        for fail_at in 0..checkpoints {
            let mut current = 0;
            let result = index.prepare_with(key, || {
                let fail = current == fail_at;
                current += 1;
                if fail {
                    Err("injected reservation refusal")
                } else {
                    Ok(())
                }
            });
            assert!(matches!(result, Err("injected reservation refusal")));
            assert_eq!(index.ids(), [0]);
            assert_eq!(index.live, 1);
            assert_eq!(index.positions.len(), 1);
            assert_eq!(index.groups(), 1);
            assert_eq!(index.positions[&signature(1)], 0);
        }
    }
    index.live = usize::MAX;
    assert!(matches!(
        index.prepare(signature(2)),
        Err("candidate index count overflow")
    ));
    assert_eq!(index.ids(), [0]);
    assert_eq!(index.positions.len(), 1);
}

#[test]
fn empty_containers_never_pass_nonempty_queries_or_retire_nonempty_groups() {
    let mut index = AggregateIndex::default();
    add(&mut index, Signature::Empty, 0);
    assert_eq!(
        index
            .find(signature(1), |_| panic!("empty container cannot match"))
            .unwrap(),
        None
    );
    add(&mut index, signature(1), 1);
    assert_eq!(index.maintenance_len(Signature::Empty).unwrap(), 1);
    let insertion = index.prepare(signature(2)).unwrap();
    assert_eq!(index.retire(&insertion, |id| id == 0), 1);
    index.insert(insertion, 2);
    assert_eq!(index.find(Signature::Empty, |_| Ok(true)).unwrap(), Some(1));
}
