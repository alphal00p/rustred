//! Test instrumentation can be disabled without changing index work/results.
use super::super::{Domain, Phase, Queue};
use super::*;

fn signature(power: u128) -> Signature {
    Signature::Nonempty {
        positive: Upper::Finite(power),
        numerator: Upper::Infinity,
        difference: Lower::NegativeInfinity,
    }
}

#[test]
fn disabled_test_counters_preserve_forward_and_retirement_results() {
    let mut expected = None;
    for enabled in [true, false] {
        let mut index = AggregateIndex::default();
        assert!(index.work.enabled); // Existing diagnostics retain their default.
        index.work.enabled = enabled;
        for (key, id) in [(signature(1), 0), (signature(3), 1)] {
            let insertion = index.prepare(key, None).unwrap();
            index.insert(insertion, id, None);
        }
        let mut compared = Vec::new();
        assert_eq!(
            index
                .find(signature(2), None, |id| {
                    compared.push(id);
                    Ok(false)
                })
                .unwrap(),
            None
        );
        assert_eq!(compared, [1]);
        assert_eq!(index.maintenance_len(signature(2)).unwrap(), 1);
        let insertion = index.prepare(signature(2), None).unwrap();
        let mut retired = Vec::new();
        assert_eq!(
            index.retire(&insertion, None, |id| {
                retired.push(id);
                true
            }),
            1
        );
        assert_eq!(retired, [0]);
        index.insert(insertion, 2, None);
        assert_eq!(index.ids(), [1, 2]);
        let work = index.work();
        assert_eq!(
            (
                work.groups_visited,
                work.groups_rejected,
                work.blocks_visited,
                work.blocks_rejected
            ),
            if enabled { (6, 3, 2, 0) } else { (0, 0, 0, 0) }
        );
        let image = serde_json::to_value(&index).unwrap();
        if let Some(reference) = &expected {
            assert_eq!(&image, reference);
        } else {
            expected = Some(image);
        }
    }
}

fn domain(lower: u64, upper: u64) -> Domain<1> {
    Domain {
        phase: Phase::Apply,
        owner: [true],
        lower: vec![lower],
        upper: vec![Some(upper)],
        rank: None,
        powers: Default::default(),
    }
}

fn canonical(queue: &Queue<1>) -> serde_json::Value {
    let mut image = serde_json::to_value(queue).unwrap();
    image[2]
        .as_array_mut()
        .unwrap()
        .sort_by_cached_key(|bucket| serde_json::to_string(&(&bucket[0], &bucket[1])).unwrap());
    image
}

#[test]
fn restored_queue_disables_future_buckets_without_erasing_or_persisting_counters() {
    let mut empty = Queue::<1>::new(100, None);
    assert!(!empty.index_work_counters_disabled_and_zero());
    empty.disable_index_work_counters();
    assert!(empty.index_work_counters_disabled_and_zero());
    let mut original = Queue::new(100, None);
    original.admit(domain(0, 0)).unwrap();
    original.admit(domain(2, 2)).unwrap();
    let image = serde_json::to_value(&original).unwrap();
    let mut enabled: Queue<1> = serde_json::from_value(image.clone()).unwrap();
    let mut disabled: Queue<1> = serde_json::from_value(image).unwrap();
    disabled.disable_index_work_counters();
    assert!(disabled.index_work_counters_disabled_and_zero());
    assert!(!enabled.index_work_counters_disabled_and_zero());
    for d in [domain(0, 1), domain(1, 1), domain(3, 3)] {
        assert_eq!(enabled.admit(d.clone()), disabled.admit(d));
    }
    assert_eq!(canonical(&enabled), canonical(&disabled));
    assert!(disabled.index_work_counters_disabled_and_zero());
    enabled.disable_index_work_counters();
    assert!(!enabled.index_work_counters_disabled_and_zero()); // No counter reset.
    // Queue's custom image omits the preference. Every restored index and the
    // queue-wide default are enabled again; no explicit resume policy changed.
    let mut restored: Queue<1> = serde_json::from_value(canonical(&disabled)).unwrap();
    assert!(!restored.index_work_counters_disabled_and_zero());
    assert!(restored.index_work_counters_enabled);
    assert!(
        restored
            .by_owner
            .values()
            .all(|bucket| bucket.indexed.work.enabled)
    );
    for d in [domain(4, 4), domain(3, 6), domain(5, 5)] {
        let mut new_bucket = d;
        new_bucket.phase = Phase::Route;
        assert_eq!(
            restored.admit(new_bucket.clone()),
            disabled.admit(new_bucket)
        );
        assert_eq!(canonical(&restored), canonical(&disabled));
        assert!(disabled.index_work_counters_disabled_and_zero());
    }
    assert!(
        restored
            .by_owner
            .values()
            .any(|bucket| bucket.indexed.work().groups_visited > 0)
    );
}
