use super::*;

const OK: NativeOutcome = NativeOutcome::Completed {
    unresolved_frontiers: 0,
};

fn ready(h: usize, count: usize) -> Ledger<u8> {
    let mut ledger = Ledger::new_ready(NonZeroUsize::new(h).unwrap(), 1000).unwrap();
    for _ in 0..count {
        admit(&mut ledger, 0);
    }
    ledger
}

fn admit(ledger: &mut Ledger<u8>, key: u8) -> usize {
    let id = ledger.len();
    ledger.reserve_admission(id).unwrap();
    ledger.admit_reserved(id, key).unwrap();
    id
}

fn finish(ledger: &mut Ledger<u8>, id: usize) {
    ledger.native_started(id).unwrap();
    ledger.publish_native(id, OK).unwrap();
}

#[test]
fn ready_completions_replenish_past_a_stalled_same_owner_head() {
    let mut ledger = ready(3, 32);
    ledger.native_started(0).unwrap();
    for id in 1..32 {
        assert!(
            ledger.can_dispatch(id),
            "later same-owner source {id} starved"
        );
        finish(&mut ledger, id);
        assert_eq!(ledger.cursor(), 0);
        assert_eq!(ledger.published_count(), id);
        assert_eq!(ledger.native_publications(), id);
        assert!(ledger.outstanding_native() <= 3);
        assert!(ledger.is_published(id));
        ledger.validate_checkpoint().unwrap();
    }
    assert_eq!(ledger.reservation_scan(), 32);
    assert_eq!(ledger.outstanding_native(), 1);
    assert_eq!(ledger.ready_reserved_ids().count(), 0);
    ledger.publish_native(0, OK).unwrap();
    assert_eq!(ledger.cursor(), 32);
    assert_eq!(ledger.published_count(), 32);
    assert_eq!(ledger.outstanding_native(), 0);
    ledger.validate_checkpoint().unwrap();
}

#[test]
fn ready_finished_but_unpublished_keeps_credit_and_sticky_reservation() {
    let mut ledger = ready(2, 8);
    ledger.native_started(0).unwrap();
    ledger.native_started(1).unwrap();
    // A pool may already hold Finished for 1, but no ledger publication yet.
    assert_eq!(ledger.outstanding_native(), 2);
    assert!(!ledger.can_dispatch(2));
    assert_eq!(ledger.native_started(2), Err(Error::InvalidNativeState));
    let representative = admit(&mut ledger, 0);
    for id in [0, 1] {
        assert_eq!(
            ledger.transfer_retired(id, representative),
            Transfer::ReservedOrStarted
        );
    }
    ledger.publish_native(1, OK).unwrap();
    assert!(ledger.can_dispatch(2));
    assert_eq!(ledger.outstanding_native(), 2);
    assert_eq!(ledger.cursor(), 0);
    assert_eq!(ledger.publish_native(1, OK), Err(Error::InvalidNativeState));
    ledger.validate_checkpoint().unwrap();
}

#[test]
fn ready_reserves_actual_states_and_skips_forward_aliases() {
    let mut ledger = ready(2, 6);
    ledger.native_started(0).unwrap();
    let representative = admit(&mut ledger, 0);
    assert_eq!(
        ledger.transfer_retired(3, representative),
        Transfer::Installed
    );
    assert_eq!(
        ledger.transfer_retired(4, representative),
        Transfer::Installed
    );
    assert_eq!(
        ledger.transfer_retired(1, representative),
        Transfer::ReservedOrStarted
    );
    finish(&mut ledger, 1);
    assert!(ledger.can_dispatch(2));
    finish(&mut ledger, 2);
    assert_eq!(ledger.reservation_scan(), 6);
    assert!(ledger.can_dispatch(5));
    assert_eq!(ledger.delegated_to(3), Some(representative));
    assert_eq!(ledger.delegated_to(4), Some(representative));
    assert_eq!(ledger.outstanding_native(), 2);
    // An alias can publish independently and never owns/releases native credit.
    ledger.publish_delegated(4).unwrap();
    ledger.publish_delegated(3).unwrap();
    assert_eq!(ledger.outstanding_native(), 2);
    assert_eq!(ledger.published_count(), 4);
    assert_eq!(ledger.cursor(), 0);
    assert_eq!(ledger.publish_delegated(4), Err(Error::InvalidNativeState));
    finish(&mut ledger, 5);
    finish(&mut ledger, representative);
    ledger.publish_native(0, OK).unwrap();
    assert_eq!(ledger.cursor(), ledger.len());
    ledger.validate_checkpoint().unwrap();
}

#[test]
fn ready_transfer_never_steals_replenished_far_reservations() {
    let mut ledger = ready(2, 12);
    ledger.native_started(0).unwrap();
    for id in 1..9 {
        finish(&mut ledger, id);
    }
    assert_eq!(ledger.cursor(), 0);
    assert!(ledger.can_dispatch(9));
    let target = admit(&mut ledger, 0);
    // 9 is beyond cursor+H, but is an actual sticky native reservation.
    assert_eq!(
        ledger.transfer_retired(9, target),
        Transfer::ReservedOrStarted
    );
    assert_eq!(
        ledger.transfer_retired(8, target),
        Transfer::ReservedOrStarted
    );
    assert_eq!(ledger.transfer_retired(10, target), Transfer::Installed);
    assert_eq!(
        ledger.transfer_retired(10, target),
        Transfer::AlreadyDelegated
    );
    ledger.validate_checkpoint().unwrap();
}

#[test]
fn ready_restore_keeps_holes_credits_alias_publication_and_initial_links() {
    let mut ledger = Ledger::new_ready(NonZeroUsize::new(3).unwrap(), 1000).unwrap();
    ledger.begin_initial_admission().unwrap();
    admit(&mut ledger, 0);
    ledger.finish_initial_admission().unwrap();
    for _ in 1..12 {
        admit(&mut ledger, 0);
    }
    assert_eq!(ledger.transfer_retired(8, 11), Transfer::Installed);
    ledger.publish_delegated(8).unwrap();
    ledger.native_started(0).unwrap();
    ledger.native_started(1).unwrap();
    ledger.record_initial_overlap(1, 0).unwrap();
    for id in 2..7 {
        finish(&mut ledger, id);
    }
    let before_count = ledger.published_count();
    let before_scan = ledger.reservation_scan();
    ledger.restore_normalize_started().unwrap();
    assert_eq!(ledger.cursor(), 0);
    assert_eq!(ledger.published_count(), before_count);
    assert_eq!(ledger.reservation_scan(), before_scan);
    assert_eq!(ledger.ready_reserved_ids().collect::<Vec<_>>(), [0, 1, 7]);
    assert_eq!(ledger.entries[1].initial_anchor, NonZeroUsize::new(1));
    assert!(ledger.is_published(8));
    for id in [0, 1, 7, 9, 10, 11] {
        finish(&mut ledger, id);
    }
    assert_eq!(ledger.cursor(), 12);
    assert_eq!(ledger.published_count(), 12);
    assert_eq!(ledger.partial_initial_inspections(), 1);
    ledger.restore_normalize_started().unwrap();
    assert_eq!(ledger.outstanding_native(), 0);
}

#[test]
fn ready_initial_anchor_may_be_unfinished_but_never_transferred() {
    let mut ledger = Ledger::new_ready(NonZeroUsize::new(2).unwrap(), 20).unwrap();
    ledger.begin_initial_admission().unwrap();
    admit(&mut ledger, 0);
    ledger.finish_initial_admission().unwrap();
    admit(&mut ledger, 0);
    admit(&mut ledger, 0);
    ledger.native_started(1).unwrap();
    ledger.record_initial_overlap(1, 0).unwrap();
    ledger.publish_native(1, OK).unwrap();
    assert_eq!(ledger.cursor(), 0);
    assert_eq!(ledger.transfer_retired(0, 2), Transfer::ProtectedInitial);
    assert_eq!(
        ledger.resolve().unwrap().by_id[1].status,
        super::super::types::ResolutionStatus::Pending
    );
    ledger.validate_checkpoint().unwrap();
    finish(&mut ledger, 0);
    assert_eq!(
        ledger.resolve().unwrap().by_id[1].status,
        super::super::types::ResolutionStatus::Discharged
    );
}

#[test]
fn ready_completed_residual_keeps_late_anchor_frontier_or_failure() {
    use super::super::types::ResolutionStatus;
    for (outcome, expected) in [
        (
            NativeOutcome::Completed {
                unresolved_frontiers: 7,
            },
            ResolutionStatus::UnresolvedFrontiers { count: 7 },
        ),
        (NativeOutcome::Failed, ResolutionStatus::Failed),
        (NativeOutcome::Cancelled, ResolutionStatus::Cancelled),
    ] {
        let mut ledger = Ledger::new_ready(NonZeroUsize::new(2).unwrap(), 20).unwrap();
        ledger.begin_initial_admission().unwrap();
        admit(&mut ledger, 0);
        ledger.finish_initial_admission().unwrap();
        admit(&mut ledger, 0);
        ledger.native_started(1).unwrap();
        ledger.record_initial_overlap(1, 0).unwrap();
        ledger.publish_native(1, OK).unwrap();
        assert_eq!(
            ledger.resolve().unwrap().by_id[1].status,
            ResolutionStatus::Pending
        );
        ledger.native_started(0).unwrap();
        ledger.publish_native(0, outcome).unwrap();
        assert_eq!(ledger.resolve().unwrap().by_id[1].status, expected);
        assert!(
            !ledger
                .resolve()
                .unwrap()
                .summary
                .all_ledger_obligations_discharged()
        );
    }
}

#[test]
fn ready_later_failure_halts_without_fabricating_the_head() {
    for outcome in [NativeOutcome::Failed, NativeOutcome::Cancelled] {
        let mut ledger = ready(2, 8);
        ledger.native_started(0).unwrap();
        ledger.native_started(1).unwrap();
        ledger.publish_native(1, outcome).unwrap();
        assert_eq!(ledger.cursor(), 0);
        assert_eq!(ledger.published_count(), 1);
        assert_eq!(ledger.reservation_scan(), 2);
        assert_eq!(ledger.outstanding_native(), 1);
        assert_eq!(
            ledger.entries[0].responsibility,
            Responsibility::Local(Local::Started)
        );
        assert!(!ledger.can_dispatch(2));
        assert_eq!(ledger.publish_native(0, OK), Err(Error::Halted));
        assert!(ledger.restore_normalize_started().is_err());
    }
}

#[test]
fn checkpoint_validation_rejects_corrupt_ready_metadata_without_repair() {
    let corruptions: &[fn(&mut Ledger<u8>)] = &[
        |l| l.cursor = 1,
        |l| l.native_publications += 1,
        |l| l.delegated_publications += 1,
        |l| l.transfers += 1,
        |l| l.partial_initial_inspections += 1,
        |l| l.outstanding_native -= 1,
        |l| l.reserved_through += 1,
        |l| l.entries[0].delegated_published = true,
        |l| l.entries[7].responsibility = Responsibility::Local(Local::Published(OK)),
        |l| l.entries[7].responsibility = Responsibility::Local(Local::Reserved),
        |l| l.entries[0].responsibility = Responsibility::Local(Local::Unreserved),
        |l| l.entries[7].responsibility = Responsibility::Delegate { to: 6 },
        |l| l.entries[7].initial_anchor = NonZeroUsize::new(1),
        |l| l.ready = false,
    ];
    for corrupt in corruptions {
        let mut ledger = ready(3, 9);
        ledger.native_started(0).unwrap();
        finish(&mut ledger, 1);
        corrupt(&mut ledger);
        let source_before = ledger.entries[0].responsibility;
        assert!(ledger.restore_normalize_started().is_err());
        // Validation is fail-closed before any Started→Reserved mutation.
        assert_eq!(ledger.entries[0].responsibility, source_before);
    }
}

#[test]
fn ready_unresolved_finished_releases_credit_without_claiming_discharge() {
    let mut ledger = ready(2, 5);
    ledger.native_started(1).unwrap();
    ledger
        .publish_native(
            1,
            NativeOutcome::Completed {
                unresolved_frontiers: 3,
            },
        )
        .unwrap();
    assert!(ledger.can_dispatch(2));
    assert_eq!(ledger.outstanding_native(), 2);
    assert_eq!(ledger.cursor(), 0);
    assert_eq!(
        ledger.resolve().unwrap().by_id[1].status,
        super::super::types::ResolutionStatus::UnresolvedFrontiers { count: 3 }
    );
    assert!(
        !ledger
            .resolve()
            .unwrap()
            .summary
            .all_ledger_obligations_discharged()
    );
    ledger.validate_checkpoint().unwrap();
}

fn codec_round_trip(ledger: &Ledger<u8>) -> Ledger<u8> {
    let bytes = serde_json::to_vec(&LedgerRef(ledger)).unwrap();
    let stored: StoredLedger = serde_json::from_slice(&bytes).unwrap();
    stored
        .restore(ledger.entries.iter().map(|entry| entry.key))
        .unwrap()
}

#[test]
fn ready_codec_round_trip_repeated_interrupts_preserves_completed_holes() {
    let mut ledger = Ledger::new_ready(NonZeroUsize::new(3).unwrap(), 1000).unwrap();
    ledger.begin_initial_admission().unwrap();
    admit(&mut ledger, 0);
    ledger.finish_initial_admission().unwrap();
    for _ in 1..12 {
        admit(&mut ledger, 0);
    }
    assert_eq!(ledger.transfer_retired(8, 11), Transfer::Installed);
    ledger.publish_delegated(8).unwrap();
    ledger.native_started(0).unwrap();
    ledger.native_started(1).unwrap();
    ledger.record_initial_overlap(1, 0).unwrap();
    for id in 2..7 {
        finish(&mut ledger, id);
    }
    let published = ledger.published_count();
    let mut ledger = codec_round_trip(&ledger);
    assert!(ledger.is_ready());
    assert_eq!(ledger.published_count(), published);
    assert_eq!(ledger.cursor(), 0);
    assert_eq!(ledger.ready_reserved_ids().collect::<Vec<_>>(), [0, 1, 7]);
    assert_eq!(ledger.entries[1].initial_anchor, NonZeroUsize::new(1));
    assert!(ledger.is_published(8));
    ledger.native_started(0).unwrap();
    ledger.native_started(1).unwrap();
    finish(&mut ledger, 7);
    ledger.native_started(9).unwrap();
    let published = ledger.published_count();
    let mut ledger = codec_round_trip(&ledger);
    assert_eq!(ledger.published_count(), published);
    assert_eq!(ledger.ready_reserved_ids().collect::<Vec<_>>(), [0, 1, 9]);
    for id in [0, 1, 9, 10, 11] {
        finish(&mut ledger, id);
    }
    assert_eq!(ledger.published_count(), 12);
    assert_eq!(ledger.cursor(), 12);
    assert_eq!(ledger.partial_initial_inspections(), 1);
    assert!(
        ledger
            .resolve()
            .unwrap()
            .summary
            .all_ledger_obligations_discharged()
    );
    let ledger = codec_round_trip(&ledger);
    assert_eq!(ledger.outstanding_native(), 0);
    assert_eq!(ledger.native_publications(), 11);
    assert_eq!(ledger.delegated_publications(), 1);
}

#[test]
fn codec_rejects_corrupt_alias_publication_counters_and_keys() {
    let mut ledger = ready(2, 6);
    assert_eq!(ledger.transfer_retired(4, 5), Transfer::Installed);
    ledger.publish_delegated(4).unwrap();
    ledger.native_started(0).unwrap();
    finish(&mut ledger, 1);
    let original = serde_json::to_value(LedgerRef(&ledger)).unwrap();
    for (pointer, replacement) in [
        ("/entries/4/delegated_published", serde_json::json!(false)),
        ("/entries/0/delegated_published", serde_json::json!(true)),
        ("/outstanding_native", serde_json::json!(1)),
        ("/cursor", serde_json::json!(2)),
        ("/reserved_through", serde_json::json!(4)),
        ("/ready", serde_json::json!(false)),
    ] {
        let mut corrupted = original.clone();
        *corrupted
            .pointer_mut(pointer)
            .expect("serialized ledger field") = replacement;
        let stored: StoredLedger = serde_json::from_value(corrupted).unwrap();
        assert!(
            stored.restore(std::iter::repeat_n(0u8, 6)).is_err(),
            "{pointer}"
        );
    }
    let stored: StoredLedger = serde_json::from_value(original).unwrap();
    // Supplied immutable queue keys still authenticate every forward edge.
    assert!(stored.restore([0, 0, 0, 0, 0, 1].into_iter()).is_err());
}

#[test]
fn ordered_codec_round_trip_preserves_prefix_and_explicit_alias_publication() {
    let mut ledger = Ledger::new(NonZeroUsize::new(2).unwrap(), 20).unwrap();
    for _ in 0..5 {
        admit(&mut ledger, 0);
    }
    assert_eq!(ledger.transfer_retired(3, 4), Transfer::Installed);
    for id in 0..3 {
        finish(&mut ledger, id);
    }
    ledger.publish_delegated(3).unwrap();
    ledger.native_started(4).unwrap();
    let mut corrupted = serde_json::to_value(LedgerRef(&ledger)).unwrap();
    corrupted["entries"][3]["delegated_published"] = serde_json::json!(false);
    let stored: StoredLedger = serde_json::from_value(corrupted).unwrap();
    assert!(
        stored.restore(std::iter::repeat_n(0u8, 5)).is_err(),
        "modern Ordered false alias bit must not be repaired from cursor"
    );
    let mut ledger = codec_round_trip(&ledger);
    assert!(!ledger.is_ready());
    assert_eq!(ledger.cursor(), 4);
    assert_eq!(ledger.published_count(), 4);
    assert!(ledger.is_published(3));
    assert_eq!(ledger.ready_reserved_ids().collect::<Vec<_>>(), [4]);
    finish(&mut ledger, 4);
    assert!(
        ledger
            .resolve()
            .unwrap()
            .summary
            .all_ledger_obligations_discharged()
    );
}

#[test]
fn ready_codec_requires_explicit_alias_bits_and_credit_count() {
    let mut ledger = ready(2, 6);
    assert_eq!(ledger.transfer_retired(4, 5), Transfer::Installed);
    let original = serde_json::to_value(LedgerRef(&ledger)).unwrap();
    let mut missing_bit = original.clone();
    missing_bit["entries"][4]
        .as_object_mut()
        .unwrap()
        .remove("delegated_published");
    let stored: StoredLedger = serde_json::from_value(missing_bit).unwrap();
    assert!(stored.restore(std::iter::repeat_n(0u8, 6)).is_err());
    let mut missing_count = original;
    missing_count
        .as_object_mut()
        .unwrap()
        .remove("outstanding_native");
    let stored: StoredLedger = serde_json::from_value(missing_count).unwrap();
    assert!(stored.restore(std::iter::repeat_n(0u8, 6)).is_err());
}

#[test]
fn legacy_ordered_codec_restores_only_its_original_prefix_semantics() {
    let mut ledger = Ledger::new(NonZeroUsize::new(2).unwrap(), 20).unwrap();
    for _ in 0..5 {
        admit(&mut ledger, 0);
    }
    assert_eq!(ledger.transfer_retired(3, 4), Transfer::Installed);
    for id in 0..3 {
        finish(&mut ledger, id);
    }
    ledger.publish_delegated(3).unwrap();
    ledger.native_started(4).unwrap();
    let mut legacy = serde_json::to_value(LedgerRef(&ledger)).unwrap();
    legacy.as_object_mut().unwrap().remove("ready");
    legacy.as_object_mut().unwrap().remove("outstanding_native");
    for entry in legacy["entries"].as_array_mut().unwrap() {
        entry.as_object_mut().unwrap().remove("delegated_published");
    }
    let stored: StoredLedger = serde_json::from_value(legacy).unwrap();
    let mut restored = stored.restore(std::iter::repeat_n(0u8, 5)).unwrap();
    assert!(!restored.is_ready());
    assert_eq!(restored.cursor(), 4);
    assert_eq!(restored.published_count(), 4);
    assert_eq!(restored.ready_reserved_ids().collect::<Vec<_>>(), [4]);
    finish(&mut restored, 4);
    assert!(
        restored
            .resolve()
            .unwrap()
            .summary
            .all_ledger_obligations_discharged()
    );
}

#[test]
fn ordered_constructor_retains_numeric_horizon_and_rejects_holes() {
    let mut ledger = Ledger::new(NonZeroUsize::new(2).unwrap(), 20).unwrap();
    for _ in 0..8 {
        admit(&mut ledger, 0);
    }
    ledger.native_started(1).unwrap();
    assert!(!ledger.is_ready());
    assert_eq!(
        ledger.publish_native(1, OK),
        Err(Error::NotCurrentPublisher)
    );
    assert_eq!(ledger.dispatch_fence(), 2);
    assert!(!ledger.can_dispatch(2));
    ledger.validate_checkpoint().unwrap();
    ledger.restore_normalize_started().unwrap();
    assert_eq!(ledger.ready_reserved_ids().collect::<Vec<_>>(), [0, 1]);
}
