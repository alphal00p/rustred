//! Rescue quarantine (`rescue.rs`): lookups never resolve into a quarantined
//! ID; exact duplicates of one are admitted and restored; the helper
//! preparation path agrees with the serial commit.
use super::*;
use std::sync::atomic::AtomicBool;

fn bits(len: usize, ids: &[usize]) -> Vec<u64> {
    let mut words = vec![0u64; len.div_ceil(64)];
    for &id in ids {
        words[id / 64] |= 1 << (id % 64);
    }
    words
}

fn boxed(upper: u64, rank: u32) -> Domain<2> {
    Domain {
        upper: vec![Some(upper), Some(upper)],
        ..domain(Some(rank))
    }
}

#[test]
fn quarantined_containers_are_skipped_by_every_lookup() {
    let mut queue = Queue::new(16, None);
    // 0: the full orthant (dominant orthant slot); 1: a box inside it.
    assert_eq!(queue.admit(domain(None)), Ok((0, true)));
    assert_eq!(queue.admit(boxed(5, 3)), Ok((0, false)), "orthant hit");
    assert_eq!(queue.orthant_hits, 1);
    let empty = queue.install_quarantine(bits(1, &[])).unwrap();
    assert_eq!(empty, 0);
    assert!(
        !queue.quarantine_active(),
        "an empty quarantine is no quarantine"
    );
    assert_eq!(queue.install_quarantine(bits(1, &[0])), Ok(1));
    assert!(queue.is_quarantined(0) && queue.quarantine_active());
    // A contained box is no longer an orthant hit: it gets a fresh ID.
    assert_eq!(queue.admit(boxed(5, 3)), Ok((1, true)));
    // A smaller box now resolves to the live box, never to the orthant.
    assert_eq!(queue.admit(boxed(4, 2)), Ok((1, false)));
    // An exact duplicate of the quarantined orthant is a fresh live ID that
    // replaces it as the bucket's dominant orthant; exact lookups find it.
    assert_eq!(queue.admit(domain(None)), Ok((2, true)));
    assert_eq!(queue.admit(domain(None)), Ok((2, false)));
    let bucket = &queue.by_owner[&(Phase::Apply, [true, false])];
    assert_eq!(bucket.orthant, Some(2));
    assert_eq!(queue.admit(boxed(9, 9)), Ok((2, false)));
    assert_eq!(
        queue.exact.duplicate_groups(&queue.domains),
        vec![vec![0, 2]]
    );
}

#[test]
fn duplicate_groups_resolve_to_their_oldest_live_member() {
    let mut queue = Queue::new(16, None);
    queue.admit(domain(None)).unwrap();
    queue.install_quarantine(bits(1, &[0])).unwrap();
    assert_eq!(queue.admit(domain(None)), Ok((1, true)));
    // The quarantine is not monotone: a later resume may release 0.
    queue.install_quarantine(bits(2, &[1])).unwrap();
    assert_eq!(queue.admit(domain(None)), Ok((0, false)));
    queue.install_quarantine(bits(2, &[])).unwrap();
    assert_eq!(queue.admit(domain(None)), Ok((0, false)));
    queue.install_quarantine(bits(2, &[0])).unwrap();
    assert_eq!(queue.admit(domain(None)), Ok((1, false)));
    queue.install_quarantine(bits(2, &[0, 1])).unwrap();
    assert_eq!(queue.admit(domain(None)), Ok((2, true)));
    // Wrong size or stray bits are refused.
    assert!(queue.install_quarantine(bits(128, &[0])).is_err());
    assert!(queue.install_quarantine(vec![1 << 5]).is_err());
}

#[test]
fn helper_preparations_agree_with_the_serial_quarantined_commit() {
    let domains = [
        domain(None),
        boxed(5, 3),
        boxed(6, 3),
        domain(None),
        boxed(2, 1),
        boxed(7, 4),
    ];
    let mut serial = Queue::new(32, None);
    let mut prepared = Queue::new(32, None);
    for queue in [&mut serial, &mut prepared] {
        queue.admit(domain(None)).unwrap();
        queue.admit(boxed(3, 3)).unwrap();
        queue.install_quarantine(bits(1, &[0])).unwrap();
    }
    let never = AtomicBool::new(false);
    for domain in domains {
        let expected = serial.admit(domain.clone());
        let proposal = prepared.prepare_admission(domain, &never);
        assert_eq!(prepared.admit_prepared(proposal), expected);
    }
    super::prepared::same_state(&serial, &prepared);
    for id in 1..serial.domains.len() {
        assert!(!serial.is_quarantined(id));
    }
}

#[test]
fn amended_restore_accepts_exact_duplicates_and_plain_restore_refuses_them() {
    let mut queue = Queue::new(16, None);
    queue.admit(domain(None)).unwrap();
    queue.install_quarantine(bits(1, &[0])).unwrap();
    queue.admit(domain(None)).unwrap();
    let parts = |queue: &Queue<2>| {
        let image = serde_json::to_value(queue).unwrap();
        let (m, domains, buckets, ledger): (
            QueueMetadata,
            Vec<Domain<2>>,
            StoredBuckets,
            Option<super::super::super::delegation::StoredLedger>,
        ) = serde_json::from_value(image).unwrap();
        let domains: Vec<CompactDomain<2>> = domains
            .iter()
            .map(CompactDomain::restore)
            .collect::<Result<_, _>>()
            .unwrap();
        (m, domains, buckets, ledger)
    };
    let (m, domains, buckets, ledger) = parts(&queue);
    let error = Queue::<2>::restore_from_parts(
        m,
        domains,
        buckets,
        ledger,
        &mut super::super::super::checkpoint::restore::Phases::default(),
    )
    .err()
    .unwrap();
    assert!(error.contains("duplicate"), "{error}");
    let (m, domains, buckets, ledger) = parts(&queue);
    let mut restored = Queue::<2>::restore_from_parts_amended(
        m,
        domains,
        buckets,
        ledger,
        true,
        &mut super::super::super::checkpoint::restore::Phases::default(),
    )
    .unwrap();
    restored.install_quarantine(bits(2, &[0])).unwrap();
    assert_eq!(restored.admit(domain(None)), Ok((1, false)));
    assert_eq!(restored.admit(boxed(3, 3)), Ok((1, false)));
}
