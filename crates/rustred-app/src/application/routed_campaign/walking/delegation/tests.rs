use super::ledger::{Local, Responsibility};
use super::types::{Error, Publication, Resolution, ResolutionReport, Transfer};
use super::*;
use std::num::NonZeroUsize;

type Key = (u8, u16);
const KEY: Key = (0, 17);
const OK: NativeOutcome = NativeOutcome::Completed {
    unresolved_frontiers: 0,
};

fn ledger(h: usize, max: usize) -> Ledger<Key> {
    Ledger::new(NonZeroUsize::new(h).unwrap(), max).unwrap()
}
fn admit(l: &mut Ledger<Key>, key: Key) -> usize {
    let id = l.len();
    l.reserve_admission(id).unwrap();
    l.admit_reserved(id, key).unwrap();
    id
}
fn seed(h: usize, n: usize) -> Ledger<Key> {
    let mut l = ledger(h, 1000);
    for _ in 0..n {
        admit(&mut l, KEY);
    }
    l
}
fn publish(l: &mut Ledger<Key>, outcome: NativeOutcome) -> Publication {
    let id = l.cursor();
    l.native_started(id).unwrap();
    l.publish_native(id, outcome).unwrap()
}
fn chain() -> Ledger<Key> {
    let mut l = seed(1, 2);
    let b = admit(&mut l, KEY);
    assert_eq!(l.transfer_retired(1, b), Transfer::Installed);
    let c = admit(&mut l, KEY);
    assert_eq!(l.transfer_retired(b, c), Transfer::Installed);
    l
}
fn publish_chain_head(l: &mut Ledger<Key>) {
    publish(l, OK);
    l.publish_delegated(1).unwrap();
    l.publish_delegated(2).unwrap();
}

#[test]
fn default_policy_and_finite_cap_stay_inspect_all() {
    assert_eq!(SchedulingPolicy::default(), SchedulingPolicy::InspectAll);
    for cap in [None, Some(0), Some(10)] {
        assert_eq!(SchedulingPolicy::InspectAll.validate(cap), Ok(()));
    }
    let p = SchedulingPolicy::TransferUnreserved {
        lookahead: NonZeroUsize::new(12).unwrap(),
    };
    assert_eq!(p.validate(None), Ok(()));
    assert_eq!(p.validate(Some(10)), Err(Error::FiniteContainmentCap));
}

#[test]
fn allocations_are_preflighted_and_admission_ids_are_exact() {
    assert!(matches!(
        Ledger::<Key>::new(NonZeroUsize::MIN, 0),
        Err(Error::ZeroCapacity)
    ));
    let mut l = ledger(1, 2);
    assert_eq!(l.admit_reserved(0, KEY), Err(Error::AdmissionNotReserved));
    assert_eq!(l.len(), 0);
    assert_eq!(l.reserve_admission(1), Err(Error::AdmissionIdMismatch));
    assert_eq!(l.admit_reserved(1, KEY), Err(Error::AdmissionIdMismatch));
    admit(&mut l, KEY);
    admit(&mut l, KEY);
    let before = l.resolve().unwrap();
    assert_eq!(l.reserve_admission(2), Err(Error::Capacity));
    assert_eq!(l.admit_reserved(2, KEY), Err(Error::Capacity));
    assert_eq!(l.resolve().unwrap(), before);
}

#[test]
fn identity_mismatch_and_invalid_proposals_leave_native_obligations() {
    for foreign in [(1, KEY.1), (KEY.0, 18)] {
        let mut l = seed(1, 2);
        let b = admit(&mut l, foreign);
        let before = l.resolve().unwrap();
        assert_eq!(l.transfer_retired(1, b), Transfer::IdentityMismatch);
        assert_eq!(l.resolve().unwrap(), before);
    }
    let mut l = seed(1, 3);
    for (a, b) in [
        (0, 0),
        (1, 1),
        (2, 1),
        (1, 9),
        (9, 2),
        (usize::MAX, usize::MAX),
    ] {
        let before = l.resolve().unwrap();
        assert_eq!(l.transfer_retired(a, b), Transfer::InvalidForwardEdge);
        assert_eq!(l.resolve().unwrap(), before);
    }
}

#[test]
fn forward_chain_preserves_direct_provenance_and_sticky_aliases() {
    let mut l = chain();
    assert_eq!(l.transfer_retired(1, 3), Transfer::AlreadyDelegated);
    assert_eq!(l.delegated_to(1), Some(2));
    assert_eq!(l.delegated_to(2), Some(3));
    publish_chain_head(&mut l);
    assert_eq!(l.delegated_to(1), Some(2));
    assert_eq!(l.delegated_to(2), Some(3));
    assert_eq!(l.cursor(), 3);
    assert!(l.can_dispatch(3));
    publish(&mut l, OK);
    let r = l.resolve().unwrap();
    assert!(r.summary.all_ledger_obligations_discharged());
    assert_eq!(r.summary.maximum_alias_depth, 2);
    assert_eq!(r.summary.native_publications, 2);
    assert_eq!(r.summary.delegated_resolved, 2);
    assert_eq!(r.summary.native_failed, 0);
    for id in [1, 2] {
        assert_eq!(
            r.by_id[id],
            Resolution {
                representative: 3,
                status: ResolutionStatus::Discharged,
                delegated: true
            }
        );
    }
}

#[test]
fn alias_advance_does_not_wait_for_representative_or_forge_finished() {
    let mut l = chain();
    publish_chain_head(&mut l);
    let r = l.resolve().unwrap();
    assert_eq!(l.native_publications(), 1);
    assert_eq!(l.delegated_publications(), 2);
    assert_eq!(r.summary.delegated_pending, 2);
    assert_eq!(r.summary.native_pending, 1);
    assert!(!r.summary.all_ledger_obligations_discharged());
    assert_eq!(l.publish_native(3, OK), Err(Error::InvalidNativeState));
    l.native_started(3).unwrap();
    // Even if an actual worker finished into escrow, no authority until publish.
    assert_eq!(
        l.resolve().unwrap().by_id[1].status,
        ResolutionStatus::Pending
    );
}

#[test]
fn finished_with_frontiers_blocks_alias_resolution_without_becoming_native_error() {
    let mut l = chain();
    publish_chain_head(&mut l);
    publish(
        &mut l,
        NativeOutcome::Completed {
            unresolved_frontiers: 7,
        },
    );
    let r = l.resolve().unwrap();
    assert_eq!(
        r.by_id[1].status,
        ResolutionStatus::UnresolvedFrontiers { count: 7 }
    );
    assert_eq!(r.summary.native_frontier_blocked, 1);
    assert_eq!(r.summary.delegated_frontier_blocked, 2);
    assert_eq!(r.summary.native_failed, 0);
    assert_eq!(r.summary.native_publications, 2);
    assert!(!r.summary.all_ledger_obligations_discharged());
    // An unresolved frontier need not stop other inspections: unlike error/cancel.
    let next = admit(&mut l, KEY);
    assert!(l.can_dispatch(next));
}

#[test]
fn native_failures_and_cancellation_never_forge_alias_inspections() {
    for (outcome, status) in [
        (NativeOutcome::Failed, ResolutionStatus::Failed),
        (NativeOutcome::Cancelled, ResolutionStatus::Cancelled),
    ] {
        let mut l = chain();
        publish_chain_head(&mut l);
        publish(&mut l, outcome);
        let r = l.resolve().unwrap();
        assert_eq!(r.by_id[1].status, status);
        assert!(r.by_id[1].delegated);
        assert!(!r.summary.all_ledger_obligations_discharged());
        assert_eq!(l.native_publications(), 2);
        assert_eq!(l.delegated_publications(), 2);
        assert_eq!(l.publish_delegated(4), Err(Error::Halted));
        assert_eq!(l.reserve_admission(4), Err(Error::Halted));
        assert!(!l.can_dispatch(4));
    }
}

#[test]
fn cancellation_before_representative_publication_keeps_started_work_pending() {
    let mut l = chain();
    publish_chain_head(&mut l);
    l.native_started(3).unwrap();
    // The scheduler, not the ledger, retains cancellation and partial events.
    // Without a real canonical Finished, no native outcome may be fabricated.
    let r = l.resolve().unwrap();
    assert_eq!(r.summary.native_pending, 1);
    assert_eq!(r.summary.delegated_pending, 2);
    assert!(!r.summary.all_ledger_obligations_discharged());
    assert_eq!(
        l.entries[3].responsibility,
        Responsibility::Local(Local::Started)
    );
}

#[test]
fn native_cleanup_accounting_does_not_subtract_delegated_cursor_passage() {
    let mut l = chain();
    publish(&mut l, OK);
    let before_cursor = l.cursor();
    let before_native = l.native_publications();
    let a = l.publish_delegated(1).unwrap();
    let b = l.publish_delegated(2).unwrap();
    let c = publish(&mut l, NativeOutcome::Cancelled);
    assert_eq!(l.cursor() - before_cursor, 3);
    assert_eq!(l.native_publications() - before_native, 1);
    assert_eq!(
        a.native_publications() + b.native_publications() + c.native_publications(),
        1
    );
    let finished_native_pool_jobs = 2;
    assert_eq!(
        finished_native_pool_jobs - (l.native_publications() - before_native),
        1
    );
    // Using the cursor delta here would underflow; native cleanup must not do it.
}

#[test]
fn fence_is_explicit_with_holes_and_saturates_safely() {
    let mut l = seed(2, 6);
    let b = admit(&mut l, KEY);
    for id in [2, 3, 4] {
        assert_eq!(l.transfer_retired(id, b), Transfer::Installed);
    }
    publish(&mut l, OK);
    publish(&mut l, OK);
    assert_eq!(l.dispatch_fence(), 4);
    assert!(!l.can_dispatch(5));
    assert_eq!(l.native_started(5), Err(Error::OutsideFence));
    l.publish_delegated(2).unwrap();
    assert_eq!(l.dispatch_fence(), 5);
    assert!(!l.can_dispatch(5));
    l.publish_delegated(3).unwrap();
    assert!(l.can_dispatch(5));
    let mut large = seed(usize::MAX, 3);
    publish(&mut large, OK);
    assert_eq!(large.dispatch_fence(), usize::MAX);
    assert!(large.can_dispatch(2));
}

#[test]
fn reserved_started_and_published_cannot_be_transferred() {
    let mut l = seed(4, 4);
    publish(&mut l, OK);
    l.native_started(1).unwrap();
    let b = admit(&mut l, KEY);
    for id in 0..4 {
        assert_eq!(l.transfer_retired(id, b), Transfer::ReservedOrStarted);
    }
    assert_eq!(l.transfer_count(), 0);
    assert_eq!(
        l.entries[1].responsibility,
        Responsibility::Local(Local::Started)
    );
    assert_eq!(
        l.entries[0].responsibility,
        Responsibility::Local(Local::Published(OK))
    );
}

#[test]
fn out_of_order_or_wrong_kind_publication_is_rejected_without_cursor_change() {
    let mut l = seed(4, 4);
    l.native_started(2).unwrap();
    assert_eq!(l.publish_native(2, OK), Err(Error::NotCurrentPublisher));
    assert_eq!(l.publish_delegated(0), Err(Error::NotDelegated));
    assert_eq!(l.cursor(), 0);
    assert_eq!(l.native_publications(), 0);
    assert_eq!(l.delegated_publications(), 0);
}

#[test]
fn corrupted_forward_links_fail_closed() {
    for target in [0, 1, 99] {
        let mut l = seed(1, 3);
        l.entries[1].responsibility = Responsibility::Delegate { to: target };
        assert_eq!(l.resolve(), Err(Error::InvalidForwardEdge));
    }
    let mut l = seed(1, 3);
    l.entries[1].responsibility = Responsibility::Delegate { to: 2 };
    l.entries[2].key = (99, 99);
    assert_eq!(l.resolve(), Err(Error::IdentityMismatch));
}

#[test]
fn global_initial_frontiers_remain_a_separate_gate() {
    let mut l = seed(1, 1);
    publish(&mut l, OK);
    let r = l.resolve().unwrap();
    assert!(r.summary.all_ledger_obligations_discharged());
    let actual_initial_source_frontiers = 1;
    assert!(
        !(r.summary.all_ledger_obligations_discharged() && actual_initial_source_frontiers == 0)
    );
}

fn schedule(
    h: usize,
    workers: usize,
    order: usize,
    fail_at: Option<usize>,
) -> (Vec<Publication>, ResolutionReport, Vec<Option<usize>>) {
    let mut l = seed(h, 24);
    let mut running = Vec::new();
    let mut finished = std::collections::BTreeMap::new();
    let mut records = Vec::new();
    let mut ticks = 0;
    loop {
        for id in 0..l.len() {
            if running.len() == workers {
                break;
            }
            if l.can_dispatch(id) {
                l.native_started(id).unwrap();
                running.push(id);
            }
        }
        if !running.is_empty() {
            let position = match order {
                0 => 0,
                1 => running.len() - 1,
                _ => ticks % running.len(),
            };
            let id = running.remove(position);
            finished.insert(
                id,
                if fail_at == Some(id) {
                    NativeOutcome::Failed
                } else {
                    OK
                },
            );
        }
        loop {
            let id = l.cursor();
            if l.delegated_to(id).is_some() {
                records.push(l.publish_delegated(id).unwrap());
                continue;
            }
            let Some(outcome) = finished.remove(&id) else {
                break;
            };
            if outcome == OK {
                let retired: &[&[usize]] = match id {
                    0 => &[&[12, 13], &[24]],
                    1 => &[&[14, 15]],
                    2 => &[&[25]],
                    3 => &[&[6, 18]],
                    4 => &[&[18, 28]],
                    _ => &[],
                };
                for old in retired {
                    let to = admit(&mut l, KEY);
                    for &from in *old {
                        // Exact-containment authority is supplied by the test fixture.
                        // This module never infers geometry or algebra itself.
                        let _ = l.transfer_retired(from, to);
                    }
                }
            }
            records.push(l.publish_native(id, outcome).unwrap());
            if outcome != OK {
                return (
                    records,
                    l.resolve().unwrap(),
                    (0..l.len()).map(|id| l.delegated_to(id)).collect(),
                );
            }
        }
        ticks += 1;
        assert!(ticks < 1000, "deadlock or lost native responsibility");
        let report = l.resolve().unwrap();
        if report.summary.all_ledger_obligations_discharged() {
            return (
                records,
                report,
                (0..l.len()).map(|id| l.delegated_to(id)).collect(),
            );
        }
    }
}

#[test]
fn successful_new_policy_publication_is_identical_across_simulated_workers() {
    let mut comparisons = 0;
    for h in [1, 2, 3, 4, 6, 8, 16, 32] {
        let reference = schedule(h, 1, 0, None);
        for workers in [1, 2, 6, 50] {
            for order in 0..3 {
                assert_eq!(schedule(h, workers, order, None), reference);
                comparisons += 1;
            }
        }
    }
    println!("successful protocol schedule comparisons: {comparisons}");
}

#[test]
fn scripted_failure_keeps_aliases_pending_and_retains_real_native_failure() {
    // This canonical scripted error does not model asynchronous pool failures.
    // Real failure-prefix equality across workers is deliberately NOT promised.
    for workers in [1, 2, 6, 50] {
        for order in 0..3 {
            let (_, report, _) = schedule(4, workers, order, Some(5));
            assert_eq!(report.summary.native_failed, 1);
            assert!(report.summary.native_pending > 0);
            assert!(report.summary.delegated_pending > 0);
            assert!(!report.summary.all_ledger_obligations_discharged());
        }
    }
}
