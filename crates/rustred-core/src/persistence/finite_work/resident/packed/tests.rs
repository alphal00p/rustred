use super::*;

fn key(arity: usize, value: i64, apply: bool) -> GeneratedWorkKey {
    let mut powers = vec![0; arity];
    powers[0] = value;
    if arity > 1 {
        powers[1] = i64::MIN;
    }
    if arity > 2 {
        powers[2] = i64::MAX;
    }
    if apply {
        GeneratedWorkKey::Apply {
            owner: powers.iter().map(|&n| n > 0).collect(),
            powers,
        }
    } else {
        GeneratedWorkKey::Route(powers)
    }
}

#[test]
fn packed_full_keys_phase_and_hash_collisions_are_exact() {
    for arity in [1, 15, 65, 129] {
        let mut index = PackedWork::new(arity).unwrap();
        assert!(index.reserve(4, &AtomicBool::new(false)).unwrap());
        for (value, apply) in [(1, false), (1, true), (2, false)] {
            let key = key(arity, value, apply);
            key.validate(arity).unwrap();
            let mut slot = index.stage(&key);
            slot.hash = 17; // Force collision; exact powers AND phase decide.
            index.insert_reserved(slot);
        }
        assert!(index.contains_hashed(&key(arity, 1, false), 17));
        assert!(index.contains_hashed(&key(arity, 1, true), 17));
        assert!(!index.contains_hashed(&key(arity, 2, true), 17));
        assert!(!index.contains_hashed(&key(arity, 3, false), 17));
    }
}

#[test]
fn packed_growth_rehashes_cached_hashes_and_reserves_all_postcommit_capacity() {
    let mut index = PackedWork::new(65).unwrap();
    let cancel = AtomicBool::new(false);
    assert_eq!(index.retained_payload().unwrap(), 0);
    for batch in 0..4 {
        assert!(index.reserve(250, &cancel).unwrap());
        let capacity = index.capacity();
        let payload = index.retained_payload().unwrap();
        let mut slots = Vec::new();
        for n in batch * 250..(batch + 1) * 250 {
            let key = key(65, n, n % 2 == 0);
            assert!(!index.contains(&key));
            slots.push(index.stage(&key));
            assert!(
                !index.contains(&key),
                "private staging cannot publish membership"
            );
        }
        for slot in slots {
            index.insert_reserved(slot);
        }
        assert_eq!(index.capacity(), capacity);
        assert_eq!(index.retained_payload().unwrap(), payload);
        for n in 0..(batch + 1) * 250 {
            assert!(index.contains(&key(65, n, n % 2 == 0)));
        }
    }
    assert!(index.arena.chunks.len() > 1);
    assert!(index.arena.chunks.len() < index.len() / 10);
    assert!(!std::mem::needs_drop::<Slot>());
}

#[test]
fn packed_offsets_overflow_and_cancellation_fail_before_allocation() {
    let mut index = PackedWork::new(1).unwrap();
    assert_eq!(
        index.growth_payload(usize::MAX).unwrap_err().kind,
        GeneratedWorkErrorKind::Resource
    );
    assert_eq!(
        index.reserve(10, &AtomicBool::new(true)).unwrap_err().kind,
        GeneratedWorkErrorKind::Cancelled
    );
    assert_eq!(index.retained_payload().unwrap(), 0);
}
