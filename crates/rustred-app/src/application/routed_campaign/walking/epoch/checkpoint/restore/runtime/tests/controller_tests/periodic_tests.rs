use super::*;
use std::cell::Cell;

pub(super) fn saved_scalars(directory: &std::path::Path) -> Value {
    let manifest = publication::read_manifest(&directory.join(publication::LATEST)).unwrap();
    let meta = manifest
        .files
        .iter()
        .find(|file| file.key == "meta")
        .unwrap();
    serde_json::from_slice(&fs::read(directory.join(&meta.file)).unwrap()).unwrap()
}

#[test]
fn periodic_boundary_stops_before_save_or_before_next_refill() {
    for during_due in [true, false] {
        let fixture = Fixture::new();
        fixture.save(3, 2);
        let mut restored = fixture.open().unwrap();
        let session = restored.dispatch.checkpoint_snapshot().session;
        let cancel = AtomicBool::new(false);
        let active = AtomicUsize::new(0);
        let calls = AtomicUsize::new(0);
        let saves = Cell::new(0);
        let inspect = |bytes: &[u8], _: &AtomicBool| {
            active.fetch_add(1, Ordering::SeqCst);
            calls.fetch_add(1, Ordering::SeqCst);
            let bytes = result(&Job::<1>::decode(bytes).unwrap(), false);
            active.fetch_sub(1, Ordering::SeqCst);
            bytes
        };
        assert_eq!(
            controller::run_authorized_periodic(
                &mut restored,
                &fixture.identity(),
                16,
                2,
                config(),
                &|| Ok(()),
                &inspect,
                || stop::requested(&cancel, None),
                |k| {
                    assert_eq!(k, 1);
                    assert_eq!(active.load(Ordering::SeqCst), 0);
                    assert_eq!(calls.load(Ordering::SeqCst), 2);
                    if during_due {
                        cancel.store(true, Ordering::Release);
                    }
                    true
                },
                |_, status| {
                    assert!(status.is_empty());
                    assert_eq!(active.load(Ordering::SeqCst), 0);
                    assert_eq!(calls.load(Ordering::SeqCst), 2, "no refill during save");
                    saves.set(saves.get() + 1);
                    let meta = saved_scalars(&fixture.directory.0);
                    assert_eq!(meta["k"], 1);
                    assert_eq!(
                        meta["stop_reason"],
                        if during_due || saves.get() == 2 {
                            json!("paused")
                        } else {
                            Value::Null
                        }
                    );
                    // Simulates a stop observed during synchronous publication.
                    cancel.store(true, Ordering::Release);
                },
            )
            .unwrap(),
            Outcome::Stopped(merge::StopReason::Paused)
        );
        assert_eq!(saves.get(), if during_due { 1 } else { 2 });
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        assert_eq!(restored.state.k, 1);
        assert!(restored.state.in_flight.is_empty());
        assert_eq!(restored.dispatch.checkpoint_snapshot().session, session);
        assert_eq!(restored.dispatch.checkpoint_snapshot().counter, 2);
        assert_eq!(restored.records.total(), 2);
        assert_eq!(
            restored.state.ledger.get(2).unwrap(),
            Entry6::Pending(Default::default())
        );
        drop(restored);
        let restored = fixture.open().unwrap();
        assert!(restored.replay.is_empty());
        assert_eq!(restored.state.k, 1);
        assert_eq!(restored.records.total(), 2);
        assert_eq!(restored.stop_reason.as_deref(), Some("paused"));
    }
}

#[test]
fn forced_error_frontier_and_final_drain_precede_periodic_due() {
    for kind in ["error", "frontier", "drain"] {
        let fixture = Fixture::new();
        fixture.save(3, if kind == "drain" { 3 } else { 2 });
        let mut restored = fixture.open().unwrap();
        let inspect = |bytes: &[u8], _: &AtomicBool| {
            let job = Job::<1>::decode(bytes).unwrap();
            let mut value =
                JobResult::<1>::decode(&result(&job, kind == "error" && job.parent == 0)).unwrap();
            if kind == "frontier" && job.parent == 0 {
                value.frontiers = vec![br#"{"kind":"periodic_test_frontier"}"#.to_vec()];
                value.emitted = 1;
                value.accepted = 1;
                value.stats_events = 1;
                value.stats_json = br#"{"events":1}"#.to_vec();
            }
            value.encode()
        };
        let mut saves = 0;
        let outcome = controller::run_authorized_periodic(
            &mut restored,
            &fixture.identity(),
            16,
            2,
            config(),
            &|| Ok(()),
            &inspect,
            || None,
            |_| panic!("forced stop/final drain must bypass periodic due"),
            |_, status| {
                assert!(status.is_empty());
                saves += 1;
            },
        )
        .unwrap();
        assert_eq!(saves, 1);
        assert_eq!(restored.state.k, 1);
        assert_eq!(
            outcome,
            match kind {
                "error" => Outcome::Stopped(merge::StopReason::ErrorStop),
                "frontier" => Outcome::Stopped(merge::StopReason::FrontierStop),
                _ => Outcome::Drained,
            }
        );
        drop(restored);
        assert!(fixture.open().unwrap().replay.is_empty());
    }
}

#[test]
fn periodic_publisher_failure_keeps_prior_authority_and_warning_keeps_latest() {
    for point in [
        publication::FailPoint::BeforeLatest,
        publication::FailPoint::BeforePrevious,
    ] {
        let fixture = Fixture::new();
        fixture.save(3, 2);
        let mut restored = fixture.open().unwrap();
        restored.publisher.fail_at(point);
        let mut saves = 0;
        let calls = AtomicUsize::new(0);
        let inspect = |bytes: &[u8], _: &AtomicBool| {
            calls.fetch_add(1, Ordering::Relaxed);
            result(&Job::<1>::decode(bytes).unwrap(), false)
        };
        let outcome = controller::run_authorized_periodic(
            &mut restored,
            &fixture.identity(),
            16,
            2,
            config(),
            &|| Ok(()),
            &inspect,
            || None,
            |_| true,
            |receipt, status| {
                assert!(status.is_empty());
                assert_eq!(receipt.warnings.len(), 1);
                saves += 1;
            },
        );
        assert!(!fixture.directory.0.join("epoch-poison").exists());
        if point == publication::FailPoint::BeforeLatest {
            assert!(outcome.is_err());
            assert_eq!(saves, 0);
            assert_eq!(
                calls.load(Ordering::Relaxed),
                2,
                "failure must prevent refill"
            );
            assert_eq!(
                publication::read_manifest(&fixture.directory.0.join(publication::LATEST))
                    .unwrap()
                    .generation,
                1
            );
            drop(restored);
            let restored = fixture.open().unwrap();
            assert_eq!(restored.state.k, 0);
            assert_eq!(restored.records.total(), 0);
            assert_eq!(restored.replay.len(), 2);
        } else {
            assert_eq!(outcome.unwrap(), Outcome::Drained);
            assert_eq!(
                saves, 2,
                "periodic and final save both succeed with warnings"
            );
            assert_eq!(calls.load(Ordering::Relaxed), 3);
            drop(restored);
            let restored = fixture.open().unwrap();
            assert_eq!(restored.state.k, 2);
            assert_eq!(restored.records.total(), 3);
            assert!(restored.replay.is_empty());
        }
    }
}

#[test]
fn native_periodic_zero_interval_refuses_before_dispatch() {
    use crate::application::routed_campaign::walking::{
        epoch::inspector::Context, initial_overlap::InitialOverlapIndex,
    };
    let mut fixture = Fixture::new();
    fixture.request.workers = 2;
    let mut options = crate::OwnerDomainWalkCheckpointOptions::new(fixture.directory.0.clone());
    options.interval_seconds = 0;
    fixture.request.checkpoint = Some(options);
    fixture.save(3, 2);
    let mut restored = fixture.open().unwrap();
    let overlap = InitialOverlapIndex::empty();
    let cancellation = AtomicBool::new(false);
    let error = controller::run_native(
        &mut restored,
        &fixture.identity(),
        16,
        &Context {
            reducer: &fixture.reducer,
            request: &fixture.request,
            overlap: &overlap,
            cancellation: &cancellation,
            g2: None,
            finite_account: None,
        },
        |_, _| panic!("invalid interval cannot publish"),
    )
    .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    assert_eq!(restored.state.k, 0);
    assert_eq!(restored.records.total(), 0);
    assert_eq!(restored.replay.len(), 2);
    assert_eq!(
        publication::read_manifest(&fixture.directory.0.join(publication::LATEST))
            .unwrap()
            .generation,
        1
    );
}
