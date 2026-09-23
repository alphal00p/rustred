use super::{
    ledger::{Local, Responsibility},
    types::{Error, Transfer},
    *,
};
use std::num::NonZeroUsize;

const OK: NativeOutcome = NativeOutcome::Completed {
    unresolved_frontiers: 0,
};
fn admit(l: &mut Ledger<u8>, key: u8) -> usize {
    let id = l.len();
    l.reserve_admission(id).unwrap();
    l.admit_reserved(id, key).unwrap();
    id
}
fn publish(l: &mut Ledger<u8>, outcome: NativeOutcome) {
    let id = l.cursor();
    l.native_started(id).unwrap();
    l.publish_native(id, outcome).unwrap();
}

#[test]
fn protected_initial_prefix_exceeds_h_without_expanding_dispatch_fence() {
    let mut l = Ledger::new(NonZeroUsize::MIN, 100).unwrap();
    l.begin_initial_admission().unwrap();
    for _ in 0..5 {
        let id = admit(&mut l, 0);
        if id > 0 {
            assert_eq!(l.transfer_retired(id - 1, id), Transfer::ProtectedInitial);
        }
    }
    assert_eq!(l.dispatch_fence(), 1);
    assert!(!l.can_dispatch(1));
    l.finish_initial_admission().unwrap();
    assert_eq!(l.initial_prefix(), Some(5));
    let later = admit(&mut l, 0);
    assert_eq!(l.transfer_retired(4, later), Transfer::ProtectedInitial);
    assert_eq!(l.transfer_count(), 0);
    for _ in 0..6 {
        publish(&mut l, OK);
    }
    assert!(
        l.resolve()
            .unwrap()
            .summary
            .all_ledger_obligations_discharged()
    );
    assert_eq!(l.begin_initial_admission(), Err(Error::InvalidInitialPhase));
}

fn linked() -> Ledger<u8> {
    let mut l = Ledger::new(NonZeroUsize::MIN, 100).unwrap();
    l.begin_initial_admission().unwrap();
    admit(&mut l, 0);
    l.finish_initial_admission().unwrap();
    let a = admit(&mut l, 0);
    let b = admit(&mut l, 0);
    assert_eq!(l.transfer_retired(a, b), Transfer::Installed);
    let q = admit(&mut l, 0);
    assert_eq!(l.transfer_retired(b, q), Transfer::Installed);
    publish(&mut l, OK);
    l.publish_delegated(1).unwrap();
    l.publish_delegated(2).unwrap();
    l.native_started(q).unwrap();
    l.record_initial_overlap(q, 0).unwrap();
    l.publish_native(q, OK).unwrap();
    l
}

#[test]
fn partial_scope_and_forward_aliases_read_direct_pinned_anchor_status() {
    for (anchor, status) in [
        (Local::Published(OK), ResolutionStatus::Discharged),
        (
            Local::Published(NativeOutcome::Completed {
                unresolved_frontiers: 2,
            }),
            ResolutionStatus::UnresolvedFrontiers { count: 2 },
        ),
        (
            Local::Published(NativeOutcome::Failed),
            ResolutionStatus::Failed,
        ),
        (
            Local::Published(NativeOutcome::Cancelled),
            ResolutionStatus::Cancelled,
        ),
        (Local::Started, ResolutionStatus::Pending),
    ] {
        let mut l = linked();
        // Direct protocol adversarial state: real canonical scheduling cannot
        // publish Q after an earlier failed/cancelled/pending anchor.
        l.entries[0].responsibility = Responsibility::Local(anchor);
        let r = l.resolve().unwrap();
        for id in 1..=3 {
            assert_eq!(r.by_id[id].status, status);
            assert_eq!(r.by_id[id].representative, 3);
        }
        assert_eq!(r.summary.partial_initial_inspections, 1);
        assert_eq!(
            r.summary.partial_initial_blocked,
            usize::from(status != ResolutionStatus::Discharged)
        );
        assert_eq!(
            r.summary.all_ledger_obligations_discharged(),
            status == ResolutionStatus::Discharged
        );
    }
}

#[test]
fn partial_scope_own_blocker_wins_and_anchor_cycles_are_rejected() {
    let mut l = linked();
    l.entries[0].responsibility = Responsibility::Local(Local::Published(NativeOutcome::Failed));
    l.entries[3].responsibility =
        Responsibility::Local(Local::Published(NativeOutcome::Completed {
            unresolved_frontiers: 7,
        }));
    assert_eq!(
        l.resolve().unwrap().by_id[3].status,
        ResolutionStatus::UnresolvedFrontiers { count: 7 }
    );
    l.entries[0].responsibility = Responsibility::Delegate { to: 3 };
    assert_eq!(l.resolve().unwrap_err(), Error::InvalidInitialAnchor);
}

#[test]
fn partial_link_registration_is_infallible_after_validation_and_scope_checked() {
    let mut l = Ledger::new(NonZeroUsize::MIN, 20).unwrap();
    l.begin_initial_admission().unwrap();
    admit(&mut l, 0);
    l.finish_initial_admission().unwrap();
    admit(&mut l, 1);
    publish(&mut l, OK);
    l.native_started(1).unwrap();
    assert_eq!(
        l.record_initial_overlap(1, 0),
        Err(Error::InvalidInitialAnchor)
    );
    assert_eq!(
        l.record_initial_overlap(1, 1),
        Err(Error::InvalidInitialAnchor)
    );
    assert_eq!(l.partial_initial_inspections(), 0);
    assert_eq!(l.entries[1].initial_anchor, None);
}
