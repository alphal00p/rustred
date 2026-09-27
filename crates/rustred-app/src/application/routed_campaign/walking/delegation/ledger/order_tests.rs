use super::*;

const OK: NativeOutcome = NativeOutcome::Completed {
    unresolved_frontiers: 0,
};

fn ready(h: usize, order: DispatchOrder) -> Ledger<u8> {
    let mut ledger = Ledger::new_ready(NonZeroUsize::new(h).unwrap(), 1000).unwrap();
    ledger.set_dispatch_order(order).unwrap();
    ledger
}

fn admit(ledger: &mut Ledger<u8>, priority: u64) -> usize {
    let id = ledger.len();
    ledger.reserve_admission(id).unwrap();
    ledger
        .admit_reserved_with_priority(id, 0, priority)
        .unwrap();
    id
}

/// Dispatch every reserved ID in reservation order and publish it natively;
/// returns the dispatch order.
fn drain(ledger: &mut Ledger<u8>) -> Vec<usize> {
    let mut order = Vec::new();
    loop {
        for id in ledger.take_transferred() {
            ledger.publish_delegated(id).unwrap();
        }
        let Some(id) = ledger.pop_reserved() else {
            break;
        };
        assert!(ledger.can_dispatch(id));
        ledger.native_started(id).unwrap();
        ledger.publish_native(id, OK).unwrap();
        order.push(id);
    }
    order
}

#[test]
fn order_names_parse_and_default_to_fifo() {
    assert_eq!(DispatchOrder::parse("").unwrap(), DispatchOrder::Fifo);
    assert_eq!(DispatchOrder::parse("fifo").unwrap(), DispatchOrder::Fifo);
    for order in [
        DispatchOrder::SupportVolume,
        DispatchOrder::ClosureBoost,
        DispatchOrder::DepthFirst,
    ] {
        assert_eq!(DispatchOrder::parse(order.name()).unwrap(), order);
    }
    assert!(DispatchOrder::parse("random").is_err());
}

#[test]
fn non_fifo_orders_are_ready_only_and_fresh_only() {
    let mut ordered = Ledger::<u8>::new(NonZeroUsize::new(2).unwrap(), 10).unwrap();
    assert!(
        ordered
            .set_dispatch_order(DispatchOrder::DepthFirst)
            .is_err()
    );
    ordered.set_dispatch_order(DispatchOrder::Fifo).unwrap();
    let mut late = Ledger::<u8>::new_ready(NonZeroUsize::new(2).unwrap(), 10).unwrap();
    admit(&mut late, 0);
    assert!(
        late.set_dispatch_order(DispatchOrder::SupportVolume)
            .is_err()
    );
    assert!(!late.prioritized());
}

#[test]
fn support_volume_reserves_by_priority_then_id() {
    let mut ledger = ready(1, DispatchOrder::SupportVolume);
    // One credit: ID 0 is reserved at its own admission.
    for priority in [1, 5, 3, 5, 0, 9] {
        admit(&mut ledger, priority);
    }
    assert_eq!(ledger.next_reserved(), Some(0));
    assert_eq!(drain(&mut ledger), vec![0, 5, 1, 3, 2, 4]);
    assert_eq!(ledger.cursor(), 6);
    assert_eq!(ledger.published_count(), 6);
}

#[test]
fn depth_first_reserves_the_newest_pending_id() {
    let mut ledger = ready(1, DispatchOrder::DepthFirst);
    for _ in 0..4 {
        admit(&mut ledger, 0);
    }
    let first = ledger.pop_reserved().unwrap();
    assert_eq!(first, 0);
    ledger.native_started(0).unwrap();
    // New work admitted while 0 runs is taken before the older 1..3.
    admit(&mut ledger, 0);
    ledger.publish_native(0, OK).unwrap();
    assert_eq!(drain(&mut ledger), vec![4, 3, 2, 1]);
    assert_eq!(ledger.cursor(), 5);
}

#[test]
fn closure_boost_takes_boosted_ids_first_then_fifo() {
    let mut ledger = ready(1, DispatchOrder::ClosureBoost);
    for _ in 0..6 {
        admit(&mut ledger, 0);
    }
    // 0 is already reserved; 9 is out of range and 0 is not Unreserved.
    ledger.boost([4, 2, 4, 9, 0]);
    assert_eq!(drain(&mut ledger), vec![0, 4, 2, 1, 3, 5]);
    let json = ledger.order_json();
    assert_eq!(json["boost_pushed"], 2);
    assert_eq!(json["boost_reserved"], 2);
}

#[test]
fn transfers_skip_pending_ids_and_queue_their_alias_publication() {
    for order in [
        DispatchOrder::SupportVolume,
        DispatchOrder::ClosureBoost,
        DispatchOrder::DepthFirst,
    ] {
        let mut ledger = ready(1, order);
        for _ in 0..3 {
            admit(&mut ledger, 0);
        }
        let representative = admit(&mut ledger, 0);
        // 0 is Reserved (mistake if retired), 1 transfers.
        assert_eq!(
            ledger.transfer_retired(0, representative),
            Transfer::ReservedOrStarted
        );
        assert_eq!(
            ledger.transfer_retired(1, representative),
            Transfer::Installed
        );
        let dispatched = drain(&mut ledger);
        assert!(!dispatched.contains(&1), "{order:?}: {dispatched:?}");
        assert_eq!(dispatched.len(), 3);
        assert_eq!(ledger.cursor(), 4);
        assert_eq!(ledger.delegated_to(1), Some(representative));
        assert!(ledger.is_published(1));
        let json = ledger.order_json();
        assert_eq!(json["retired_transferred"], 1);
        assert_eq!(json["retired_reserved"], 1);
        assert_eq!(json["mistakes"], 1);
        assert_eq!(json["unreserved"], 0);
    }
}

#[test]
fn fifo_statistics_classify_retirements_without_changing_the_scan() {
    let mut ledger = Ledger::<u8>::new_ready(NonZeroUsize::new(2).unwrap(), 100).unwrap();
    for _ in 0..4 {
        admit(&mut ledger, 7);
    }
    ledger.native_started(0).unwrap();
    ledger.native_started(1).unwrap();
    ledger.publish_native(1, OK).unwrap(); // 2 becomes Reserved
    let representative = admit(&mut ledger, 7);
    assert_eq!(
        ledger.transfer_retired(0, representative),
        Transfer::ReservedOrStarted
    );
    assert_eq!(
        ledger.transfer_retired(1, representative),
        Transfer::ReservedOrStarted
    );
    assert_eq!(
        ledger.transfer_retired(2, representative),
        Transfer::ReservedOrStarted
    );
    assert_eq!(
        ledger.transfer_retired(3, representative),
        Transfer::Installed
    );
    assert!(ledger.take_transferred().is_empty());
    assert_eq!(ledger.reservation_scan(), 3);
    let json = ledger.order_json();
    assert_eq!(json["order"], "fifo");
    assert_eq!(json["retired_started"], 1);
    assert_eq!(json["retired_published"], 1);
    assert_eq!(json["retired_reserved"], 1);
    assert_eq!(json["retired_transferred"], 1);
    assert_eq!(json["mistakes"], 3);
    assert_eq!(json["peak_pending"], 4);
    ledger.validate_checkpoint().unwrap();
}

#[test]
fn support_volume_priority_orders_support_before_volume() {
    let low_support = support_volume_priority(3, 1e6);
    let high_support = support_volume_priority(4, 0.0);
    assert!(high_support > low_support);
    assert!(support_volume_priority(4, 10.0) > support_volume_priority(4, 9.9));
    assert_eq!(
        support_volume_priority(4, f64::NAN),
        support_volume_priority(4, 0.0)
    );
}
