//! Nonfinal committed checkpoint, same-session continuation and width-changed
//! cold reopen. No public CP6 activation or crash-during-CAS claim.
use super::native_equivalence::{closed_fixture, completed_snapshot, finish_native_mode};
use super::periodic_tests::saved_scalars;
use super::*;
use crate::application::routed_campaign::walking::{
    epoch::{
        inspector::{Context, inspect_job, inspect_job_with_snapshot},
        snapshot::Publication,
    },
    initial_overlap::InitialOverlapIndex,
};

#[test]
fn real_native_periodic_continuation_and_reopen_preserve_full_snapshot() {
    const TEST: &str = "real native epoch periodic continuation/reopen";
    if !crate::test_gates::workers_or_skip(TEST, 3)
        || !std::thread::spawn(|| crate::test_gates::licensed_or_skip(TEST))
            .join()
            .unwrap()
    {
        return; // Explicit skip is no native equivalence coverage.
    }
    for mode in [
        controller::LookupMode::AllMiss,
        controller::LookupMode::Snapshot,
    ] {
        periodic_native_mode(mode);
    }
}

fn periodic_native_mode(mode: controller::LookupMode) {
    let mut fixture = closed_fixture();
    // Two original replay jobs plus a still-Pending third initial ID guarantee
    // a genuinely nonfinal first merge, independently of native descendants.
    fixture.save(3, 2);
    let mut baseline = fixture.open().unwrap();
    finish_native_mode(&fixture, &mut baseline, mode);
    drop(baseline);
    let expected = completed_snapshot(&mut fixture.open().unwrap());

    fixture.directory = Directory::new();
    fixture.save(3, 2);
    let mut periodic = fixture.open().unwrap();
    let session = periodic.dispatch.checkpoint_snapshot().session;
    let intermediate = Directory::new();
    let active = AtomicUsize::new(0);
    let calls = AtomicUsize::new(0);
    let overlap = InitialOverlapIndex::empty();
    let snapshots = Publication::new();
    let authorize = || {
        symbolica::license::LicenseManager::is_licensed()
            .then_some(())
            .ok_or_else(|| "real native periodic worker lacks authorization".into())
    };
    let inspect = |bytes: &[u8], cancel: &AtomicBool| {
        active.fetch_add(1, Ordering::SeqCst);
        calls.fetch_add(1, Ordering::SeqCst);
        let context = Context {
            reducer: &fixture.reducer,
            request: &fixture.request,
            overlap: &overlap,
            cancellation: cancel,
            g2: None,
            finite_account: None,
        };
        let native = match mode {
            controller::LookupMode::AllMiss => inspect_job(&context, bytes),
            controller::LookupMode::Snapshot => {
                inspect_job_with_snapshot(&context, bytes, snapshots.acquire().unwrap())
            }
        };
        let result = JobResult::<1>::decode(&native).unwrap();
        assert!(!result.panic);
        assert_eq!(result.error_kind, ErrorKind::None);
        assert_eq!(result.break_reason, BreakReason::None);
        assert!(result.frontiers.is_empty());
        active.fetch_sub(1, Ordering::SeqCst);
        native
    };
    let mut saves = 0;
    assert_eq!(
        controller::run_authorized_lookup_periodic(
            &mut periodic,
            &fixture.identity(),
            16,
            2,
            config(),
            &authorize,
            &inspect,
            || None,
            |k| {
                assert!(k > 0);
                assert_eq!(active.load(Ordering::SeqCst), 0);
                k == 1
            },
            |receipt, status| {
                assert!(status.is_empty());
                assert_eq!(active.load(Ordering::SeqCst), 0);
                saves += 1;
                if saves == 1 {
                    assert_eq!(receipt.generation, 2);
                    assert_eq!(
                        calls.load(Ordering::SeqCst),
                        2,
                        "no next-cut work during save"
                    );
                    let meta = saved_scalars(&fixture.directory.0);
                    assert_eq!(meta["k"], 1);
                    assert!(
                        meta["ledger_counts"][0].as_u64().unwrap() > 0,
                        "nonfinal Pending work"
                    );
                    assert_eq!(meta["ledger_counts"][1], 0, "no Reserved/active cut");
                    assert_eq!(meta["stop_reason"], Value::Null);
                    // Preserve this exact nonfinal generation before normal
                    // continuation overwrites latest. Tiny fixture only; copying
                    // is not a production checkpoint/restore implementation.
                    for (count, entry) in fs::read_dir(&fixture.directory.0).unwrap().enumerate() {
                        assert!(count < 64, "bounded fixture checkpoint inventory");
                        let entry = entry.unwrap();
                        assert!(entry.file_type().unwrap().is_file());
                        fs::copy(entry.path(), intermediate.0.join(entry.file_name())).unwrap();
                    }
                }
            },
            (mode == controller::LookupMode::Snapshot).then_some(&snapshots),
        )
        .unwrap(),
        Outcome::Drained
    );
    assert_eq!(saves, 2, "one periodic plus final drain save");
    assert_eq!(periodic.dispatch.checkpoint_snapshot().session, session);
    assert!(periodic.state.in_flight.is_empty());
    drop(periodic);
    assert_eq!(completed_snapshot(&mut fixture.open().unwrap()), expected);

    fixture.directory = intermediate;
    fixture.request.workers = 3;
    let mut resumed = fixture.open().unwrap();
    assert_eq!(resumed.state.k, 1);
    assert!(resumed.state.pending_or_reserved() > 0);
    assert!(resumed.replay.is_empty());
    assert!(resumed.state.in_flight.is_empty());
    assert!(resumed.stop_reason.is_none());
    assert_eq!(resumed.dispatch.checkpoint_snapshot().session, session + 1);
    assert_eq!(resumed.dispatch.checkpoint_snapshot().counter, 0);
    let before_records = resumed.records.total();
    assert_eq!(before_records, 2);
    finish_native_mode(&fixture, &mut resumed, mode);
    assert!(resumed.records.total() > before_records);
    drop(resumed);
    assert_eq!(completed_snapshot(&mut fixture.open().unwrap()), expected);
}
