use super::super::{SavedApplicationCachePack, SavedApplicationCacheWriter};
use super::*;
use crate::{AppError, AppErrorKind};
use rustred::algebra::CoefficientContext;
use rustred::family::IntegralKey;
use rustred::persistence::application_cache::{
    ApplicationCacheBinding, ApplicationCacheKey, ApplicationCacheRecord,
};
use rustred::reduction::{
    GuardedCacheBacking, GuardedCacheBackingError, GuardedIntermediateCapture,
};
use std::{
    collections::BTreeMap,
    path::Path,
    thread,
    time::{Duration, Instant},
};

fn binding(id: u8) -> ApplicationCacheBinding {
    ApplicationCacheBinding {
        artifact_digest: [id; 32],
        family_fingerprint: format!("running-cache-test-{id}"),
        physical_arity: 1,
        layer: "raw-declared-unit-mass-v1".into(),
    }
}

fn context() -> CoefficientContext {
    CoefficientContext::try_new(["d"]).unwrap()
}

fn key(power: i64) -> ApplicationCacheKey {
    ApplicationCacheKey::route(&IntegralKey::try_new([power]).unwrap())
}

fn record(power: i64) -> ApplicationCacheRecord {
    let context = context();
    ApplicationCacheRecord {
        key: key(power),
        terms: BTreeMap::from([(IntegralKey::try_new([1]).unwrap(), context.one())]),
        nonzero_conditions: vec![context.parameter("d").unwrap()],
    }
}

fn scratch() -> super::super::tests::Scratch {
    super::super::tests::Scratch::new()
}

fn manual_options() -> SavedApplicationRunningCacheOptions {
    SavedApplicationRunningCacheOptions {
        checkpoint_interval: Duration::from_secs(60),
        ..Default::default()
    }
}

fn open(path: &Path, options: SavedApplicationRunningCacheOptions) -> SavedApplicationRunningCache {
    SavedApplicationRunningCache::open(binding(1), context(), path, Vec::new(), options).unwrap()
}

fn native_error(error: GuardedCacheBackingError) -> AppError {
    AppError::from_native(error, AppErrorKind::Execution)
}

fn capture(session: &mut SavedApplicationRunningCache, record: &ApplicationCacheRecord) {
    session
        .with_io(|_, capture| {
            assert!(
                capture
                    .wants(&record.key, &context())
                    .map_err(native_error)?
            );
            capture
                .store_completed(record, &context())
                .map_err(native_error)
        })
        .unwrap();
}

#[test]
fn exclusive_delta_lock_and_exact_base_binding_are_enforced() {
    let scratch = scratch();
    let delta = scratch.0.join("delta");
    let session = open(&delta, manual_options());
    let mut fork = session.fork();
    let control = session.control();
    drop(session);
    capture(&mut fork, &record(1));
    assert_eq!(control.report().unwrap().committed_records, 1);
    assert!(
        SavedApplicationRunningCache::open(
            binding(1),
            context(),
            &delta,
            Vec::new(),
            manual_options(),
        )
        .is_err()
    );
    control.close().unwrap();
    assert!(fork.control().report().unwrap().closed);
    let reopened = open(&delta, manual_options());
    reopened.control().close().unwrap();

    let base_path = scratch.0.join("foreign-base");
    let mut writer =
        SavedApplicationCacheWriter::create(&base_path, binding(2), Default::default()).unwrap();
    writer.append_completed(&[record(1)], &context()).unwrap();
    writer.checkpoint().unwrap();
    drop(writer);
    let base =
        SavedApplicationCachePack::open_generated(&base_path, &binding(2), Default::default())
            .unwrap();
    let invalid_delta = scratch.0.join("invalid-delta");
    assert!(
        SavedApplicationRunningCache::open(
            binding(1),
            context(),
            &invalid_delta,
            vec![SavedApplicationRunningCacheBase::Pack(base)],
            manual_options(),
        )
        .is_err()
    );
    assert!(!invalid_delta.exists());
}

#[test]
fn return_barrier_flushes_accepted_records_before_reopen() {
    let scratch = scratch();
    let delta = scratch.0.join("delta");
    let mut session = open(&delta, manual_options());
    let control = session.control();
    let pending_generation = session
        .with_io(|_, capture| {
            for power in [1, 2] {
                capture
                    .store_completed(&record(power), &context())
                    .map_err(native_error)?;
            }
            let pending = control.report()?;
            assert_eq!(pending.accepted_records, 2);
            assert_eq!(pending.committed_records, 0);
            assert_eq!(pending.pending_records, 2);
            assert!(pending.pending_bytes > 0);
            assert!(pending.last_error.is_none());
            Ok(pending.generation)
        })
        .unwrap();
    let committed = control.report().unwrap();
    assert_eq!(committed.accepted_records, 2);
    assert_eq!(committed.committed_records, 2);
    assert_eq!(committed.pending_records, 0);
    assert_eq!(committed.pending_bytes, 0);
    assert!(committed.generation > pending_generation);
    assert!(committed.checkpoint_age.is_some());
    assert_eq!(control.flush().unwrap().generation, committed.generation);
    assert!(control.close().unwrap().closed);

    let mut reopened = open(&delta, manual_options());
    for power in [1, 2] {
        assert_eq!(
            reopened
                .with_io(|backing, _| backing
                    .lookup(&key(power), &context())
                    .map_err(native_error))
                .unwrap(),
            Some(record(power))
        );
    }
    reopened.control().close().unwrap();
}

#[test]
fn timer_commits_while_the_capture_callback_is_idle() {
    let scratch = scratch();
    let delta = scratch.0.join("delta");
    let mut session = open(
        &delta,
        SavedApplicationRunningCacheOptions {
            checkpoint_interval: Duration::from_millis(25),
            ..manual_options()
        },
    );
    let control = session.control();
    session
        .with_capture(|capture| {
            capture
                .store_completed(&record(1), &context())
                .map_err(native_error)?;
            let deadline = Instant::now() + Duration::from_secs(2);
            loop {
                let report = control.report()?;
                assert!(report.last_error.is_none(), "{:?}", report.last_error);
                if report.committed_records == 1 {
                    assert_eq!(report.pending_records, 0);
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "idle timer did not publish its pending record"
                );
                thread::sleep(Duration::from_millis(10));
            }
            Ok(())
        })
        .unwrap();
    let mut reader =
        SavedApplicationCachePack::open_generated(&delta, &binding(1), Default::default()).unwrap();
    assert_eq!(reader.lookup(&key(1), &context()).unwrap(), Some(record(1)));
    control.close().unwrap();
}

#[test]
fn snapshot_retains_its_generation_as_delta_continues_appending() {
    let scratch = scratch();
    let base_path = scratch.0.join("base");
    let mut writer =
        SavedApplicationCacheWriter::create(&base_path, binding(1), Default::default()).unwrap();
    writer.append_completed(&[record(1)], &context()).unwrap();
    writer.checkpoint().unwrap();
    drop(writer);
    let base =
        SavedApplicationCachePack::open_generated(&base_path, &binding(1), Default::default())
            .unwrap();
    let base = SavedApplicationRunningCacheBase::Pack(base);
    let mut read_only =
        SavedApplicationReadOnlyCache::new(binding(1), context(), vec![base.fork_reader()])
            .unwrap();
    assert_eq!(
        read_only.lookup(&key(1), &context()).unwrap(),
        Some(record(1))
    );
    read_only.store_completed(&record(2), &context()).unwrap();
    assert_eq!(read_only.lookup(&key(2), &context()).unwrap(), None);
    let writer =
        SavedApplicationCacheWriter::resume(&base_path, &binding(1), Default::default()).unwrap();
    drop(writer);
    let delta = scratch.0.join("delta");
    let mut session = SavedApplicationRunningCache::open(
        binding(1),
        context(),
        &delta,
        vec![base],
        manual_options(),
    )
    .unwrap();
    assert_eq!(
        session
            .with_io(|backing, _| backing.lookup(&key(1), &context()).map_err(native_error))
            .unwrap(),
        Some(record(1))
    );
    capture(&mut session, &record(2));
    let control = session.control();
    let snapshot = control.snapshot(&scratch.0.join("frozen.rrindex")).unwrap();
    assert_eq!(snapshot.binding, binding(1));
    assert_eq!(
        snapshot.to_document()["delta"]["generation"],
        snapshot.delta.generation
    );
    let mut frozen = SavedApplicationCachePack::open_snapshot(
        &snapshot.directory,
        &snapshot.binding,
        &snapshot.delta,
        Default::default(),
    )
    .unwrap();
    assert_eq!(frozen.lookup(&key(2), &context()).unwrap(), Some(record(2)));
    capture(&mut session, &record(3));
    let latest = control.flush().unwrap();
    assert!(latest.generation > snapshot.delta.generation);
    assert_eq!(frozen.lookup(&key(3), &context()).unwrap(), None);
    assert!(frozen.refresh().is_err());
    let mut current =
        SavedApplicationCachePack::open_generated(&delta, &binding(1), Default::default()).unwrap();
    assert_eq!(
        current.lookup(&key(3), &context()).unwrap(),
        Some(record(3))
    );
    control.close().unwrap();
}

#[test]
fn callback_context_mismatch_and_pending_byte_refusal_are_explicit() {
    use rustred::reduction::GuardedCacheBackingErrorKind;

    let scratch = scratch();
    let mut session = open(&scratch.0.join("context"), manual_options());
    let foreign = CoefficientContext::try_new(["foreign_running_cache_d"]).unwrap();
    session
        .with_io(|backing, capture| {
            assert!(capture.wants(&key(1), &foreign).is_err());
            assert!(capture.store_completed(&record(1), &foreign).is_err());
            assert!(backing.lookup(&key(1), &foreign).is_err());
            Ok(())
        })
        .unwrap();
    assert_eq!(session.control().report().unwrap().accepted_records, 0);
    session.control().close().unwrap();

    let mut tight = open(
        &scratch.0.join("tight"),
        SavedApplicationRunningCacheOptions {
            max_pending_bytes: 1,
            batch_bytes: 1,
            ..manual_options()
        },
    );
    tight
        .with_io(|_, capture| {
            let error = capture.store_completed(&record(1), &context()).unwrap_err();
            assert_eq!(error.kind, GuardedCacheBackingErrorKind::Limit);
            Ok(())
        })
        .unwrap();
    let report = tight.control().report().unwrap();
    assert_eq!(report.accepted_records, 0);
    assert_eq!(report.committed_records, 0);
    assert_eq!(report.pending_records, 0);
    assert_eq!(report.pending_bytes, 0);
    assert_eq!(
        report.last_error.as_ref().unwrap().kind(),
        AppErrorKind::Limit
    );
    tight.control().close().unwrap();
}

#[test]
fn mathematical_errors_preserve_their_value_and_flush_completed_prefixes() {
    let scratch = scratch();
    let math_error = AppError::new(
        AppErrorKind::UnresolvedReduction,
        "native mathematical failure",
    );
    for capture_only in [false, true] {
        let delta = scratch.0.join(format!("prefix-{capture_only}"));
        let mut session = open(&delta, manual_options());
        let run = |capture: &mut dyn GuardedIntermediateCapture| {
            capture
                .store_completed(&record(1), &context())
                .map_err(native_error)?;
            Err::<(), _>(math_error.clone())
        };
        let result = if capture_only {
            session.with_capture(run)
        } else {
            session.with_io(|_, capture| run(capture))
        };
        assert_eq!(result.unwrap_err(), math_error);
        let report = session.control().report().unwrap();
        assert_eq!(report.committed_records, 1);
        assert_eq!(report.pending_records, 0);
        assert!(report.last_error.is_none());
        let mut reader =
            SavedApplicationCachePack::open_generated(&delta, &binding(1), Default::default())
                .unwrap();
        assert_eq!(reader.lookup(&key(1), &context()).unwrap(), Some(record(1)));
        session.control().close().unwrap();
    }

    let delta = scratch.0.join("writer-fault");
    let mut session = open(&delta, manual_options());
    std::fs::remove_dir(delta.join("chunks")).unwrap();
    let error = session
        .with_capture(|capture| {
            capture
                .store_completed(&record(1), &context())
                .map_err(native_error)?;
            Err::<(), _>(math_error.clone())
        })
        .unwrap_err();
    assert_eq!(error, math_error);
    let control = session.control();
    let report = control.report().unwrap();
    assert_eq!(report.accepted_records, 1);
    assert_eq!(report.committed_records, 0);
    assert_eq!(report.pending_records, 1);
    let writer_error = report.last_error.as_ref().unwrap();
    assert_eq!(writer_error.kind(), AppErrorKind::Input);
    assert!(writer_error.message().contains("chunks"));
    assert_eq!(report.to_document()["last_error"]["kind"], "input");
    assert_eq!(&control.close().unwrap_err(), writer_error);
    assert!(control.report().unwrap().closed);
}

#[test]
fn queued_resource_scopes_are_flattened_and_checked_once_per_distinct_budget() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use symbolica::utils::resources::ResourceLimits;
    let calls = Arc::new(AtomicUsize::new(0));
    let budget = |calls: &Arc<AtomicUsize>| {
        let calls = calls.clone();
        ResourceBudget::with_memory_monitor(
            ResourceLimits {
                max_process_rss_bytes: Some(1024),
                max_elapsed: None,
            },
            move || {
                calls.fetch_add(1, Ordering::Relaxed);
                Ok(1)
            },
        )
    };
    let shared = budget(&calls);
    let repeated = (0..4096).map(|_| shared.clone()).collect::<Vec<_>>();
    under_budgets(&repeated, symbolica::utils::resources::checkpoint).unwrap();
    assert_eq!(calls.load(Ordering::Relaxed), 1);

    calls.store(0, Ordering::Relaxed);
    let other = budget(&calls);
    let ambient = budget(&calls);
    ambient
        .run(|| {
            under_budgets(
                &[shared.clone(), other, shared],
                symbolica::utils::resources::checkpoint,
            )
        })
        .unwrap();
    assert_eq!(calls.load(Ordering::Relaxed), 3);
}

#[test]
fn snapshot_descriptor_roundtrips_and_rejects_malformed_metadata_without_io() {
    let snapshot = SavedApplicationRunningCacheSnapshot {
        directory: Path::new("/unopened/native-delta").to_path_buf(),
        binding: binding(1),
        delta: NativePackSnapshot {
            manifest_path: Path::new("/unopened/frozen.rrindex").to_path_buf(),
            generation: 19,
            manifest_bytes: 1234,
            manifest_digest: [23; 32],
        },
    };
    let document = snapshot.to_document();
    let parsed = SavedApplicationRunningCacheSnapshot::from_document(&document).unwrap();
    assert_eq!(parsed.to_document(), document);
    for (pointer, invalid) in [
        ("/schema", serde_json::json!("unknown")),
        ("/directory", serde_json::json!("relative/delta")),
        ("/binding/artifact_digest", serde_json::json!("abcd")),
        ("/binding/physical_arity", serde_json::json!(0)),
        ("/delta/generation", serde_json::json!(-1)),
        ("/delta/manifest_bytes", serde_json::json!("1234")),
        ("/delta/manifest_path", serde_json::json!(42)),
        ("/delta/manifest_digest", serde_json::json!("z".repeat(64))),
    ] {
        let mut changed = document.clone();
        *changed.pointer_mut(pointer).unwrap() = invalid;
        assert!(
            SavedApplicationRunningCacheSnapshot::from_document(&changed).is_err(),
            "{pointer}"
        );
    }
    let mut unknown = document;
    unknown["extra"] = serde_json::json!(true);
    assert!(SavedApplicationRunningCacheSnapshot::from_document(&unknown).is_err());
}

#[test]
fn close_rejects_a_reserved_producer_before_releasing_the_writer() {
    use std::sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    };
    use symbolica::utils::resources::ResourceLimits;

    #[derive(Clone)]
    struct Gate(Arc<(Mutex<bool>, Condvar)>);
    impl Gate {
        fn new() -> Self {
            Self(Arc::new((Mutex::new(false), Condvar::new())))
        }
        fn wait(&self) -> Result<(), String> {
            let (released, changed) = &*self.0;
            let mut released = released.lock().map_err(|error| error.to_string())?;
            while !*released {
                released = changed.wait(released).map_err(|error| error.to_string())?;
            }
            Ok(())
        }
        fn release(&self) {
            let (released, changed) = &*self.0;
            if let Ok(mut released) = released.lock() {
                *released = true;
                changed.notify_all();
            }
        }
    }
    struct ReleaseOnDrop(Gate);
    impl Drop for ReleaseOnDrop {
        fn drop(&mut self) {
            self.0.release();
        }
    }

    let scratch = scratch();
    let delta = scratch.0.join("close-race");
    let options = SavedApplicationRunningCacheOptions {
        batch_records: 1,
        ..manual_options()
    };
    let session = open(&delta, options);
    let control = session.control();
    let coefficient_context = context();
    let worker_gate = Gate::new();
    let producer_gate = Gate::new();
    // These guards also release a blocked thread if a bounded notification
    // wait fails, so an assertion cannot strand the worker or producer.
    let _worker_release = ReleaseOnDrop(worker_gate.clone());
    let _producer_release = ReleaseOnDrop(producer_gate.clone());
    let (worker_blocked_send, worker_blocked_receive) = mpsc::channel();
    let worker_once = AtomicBool::new(false);
    let worker_wait = worker_gate.clone();
    let worker_budget = ResourceBudget::with_memory_monitor(
        ResourceLimits {
            max_process_rss_bytes: Some(u64::MAX),
            max_elapsed: None,
        },
        move || {
            if thread::current().name() == Some("rustred-cache-writer")
                && !worker_once.swap(true, Ordering::SeqCst)
            {
                let _ = worker_blocked_send.send(());
                worker_wait.wait()?;
            }
            Ok(0)
        },
    );
    let first = record(1);
    let capture = Capture {
        context: &coefficient_context,
        control: control.clone(),
        options,
    };
    worker_budget
        .run(|| capture.enqueue(&first, &coefficient_context))
        .unwrap();
    worker_blocked_receive
        .recv_timeout(Duration::from_secs(2))
        .expect("worker did not reach its captured resource checkpoint");

    let (producer_reserved_send, producer_reserved_receive) = mpsc::channel();
    let producer_control = control.clone();
    let producer_wait = producer_gate.clone();
    let producer = thread::spawn(move || {
        let coefficient_context = context();
        let second = record(2);
        let monitor_control = producer_control.clone();
        let producer_once = AtomicBool::new(false);
        let budget = ResourceBudget::with_memory_monitor(
            ResourceLimits {
                max_process_rss_bytes: Some(u64::MAX),
                max_elapsed: None,
            },
            move || {
                let reserved = monitor_control
                    .shared
                    .lock()
                    .map_err(|error| error.to_string())?
                    .reserved
                    > 0;
                if reserved && !producer_once.swap(true, Ordering::SeqCst) {
                    let _ = producer_reserved_send.send(());
                    producer_wait.wait()?;
                }
                Ok(0)
            },
        );
        let capture = Capture {
            context: &coefficient_context,
            control: producer_control,
            options,
        };
        budget.run(|| capture.enqueue(&second, &coefficient_context))
    });
    producer_reserved_receive
        .recv_timeout(Duration::from_secs(2))
        .expect("producer did not reserve its record before cloning");

    let (close_send, close_receive) = mpsc::channel();
    control
        .command(Message::Close(
            ResourceBudget::capture_current(),
            close_send,
        ))
        .unwrap();
    producer_gate.release();
    let producer_result = producer.join();
    let reservation = {
        let state = control.shared.lock().unwrap();
        (
            state.reserved,
            state.reserved_bytes,
            state.keys.contains(&key(2)),
            state.accepted,
        )
    };
    // Release the real writer before checking the race's outcome.
    worker_gate.release();
    let closed = control.receive(close_receive).unwrap();
    assert!(producer_result.unwrap().is_err());
    assert_eq!(reservation, (0, 0, false, 1));
    assert_eq!(closed.accepted_records, 1);
    assert_eq!(closed.committed_records, 1);
    assert_eq!(closed.pending_records, 0);
    assert!(closed.closed);
    let mut reader =
        SavedApplicationCachePack::open_generated(&delta, &binding(1), Default::default()).unwrap();
    assert_eq!(reader.lookup(&key(1), &context()).unwrap(), Some(record(1)));
    assert_eq!(reader.lookup(&key(2), &context()).unwrap(), None);
}

#[test]
fn a_full_close_queue_remains_open_until_close_is_admitted() {
    use std::sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    };
    use symbolica::utils::resources::ResourceLimits;
    struct ReleaseOnDrop(Arc<(Mutex<bool>, Condvar)>);
    impl Drop for ReleaseOnDrop {
        fn drop(&mut self) {
            *self.0.0.lock().unwrap() = true;
            self.0.1.notify_all();
        }
    }
    let shared = Arc::new(Shared {
        state: Mutex::new(State {
            accepted: 0,
            committed: 0,
            pending: 0,
            bytes: 0,
            reserved: 0,
            reserved_bytes: 0,
            keys: Default::default(),
            generation: 1,
            checkpoint: Instant::now(),
            last_error: None,
            failure: None,
            capture_skipped_records: 0,
            capture_metadata_exhausted: false,
            closing: false,
            closed: false,
        }),
        changed: Condvar::new(),
    });
    let (sender, receiver) = mpsc::sync_channel(1);
    let (ignored, _) = mpsc::channel();
    assert!(
        sender
            .try_send(Message::Flush(ResourceBudget::capture_current(), ignored))
            .is_ok()
    );
    let control = SavedApplicationRunningCacheControl { sender, shared };
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let release = ReleaseOnDrop(gate.clone());
    let (full_send, full_receive) = mpsc::channel();
    let waiting_control = control.clone();
    let pending = thread::spawn(move || {
        let checks = AtomicUsize::new(0);
        let budget = ResourceBudget::with_memory_monitor(
            ResourceLimits {
                max_process_rss_bytes: Some(u64::MAX),
                max_elapsed: None,
            },
            move || {
                // The second checkpoint follows one actual Full response and the
                // bounded condvar wait, before retrying queue admission.
                if checks.fetch_add(1, Ordering::SeqCst) == 1 {
                    let _ = full_send.send(());
                    let mut released = gate.0.lock().map_err(|error| error.to_string())?;
                    while !*released {
                        released = gate.1.wait(released).map_err(|error| error.to_string())?;
                    }
                }
                Ok(0)
            },
        );
        let (reply, _) = mpsc::channel();
        budget.run(|| {
            waiting_control.command(Message::Close(ResourceBudget::capture_current(), reply))
        })
    });
    full_receive
        .recv_timeout(Duration::from_secs(2))
        .expect("close did not observe the full queue");
    let remained_open = {
        let state = control.shared.lock().unwrap();
        !state.closing && !state.closed
    };
    assert!(matches!(receiver.recv().unwrap(), Message::Flush(..)));
    drop(release);
    let admitted = pending.join().unwrap();
    assert!(remained_open);
    admitted.unwrap();
    assert!(control.shared.lock().unwrap().closing);
    assert!(matches!(receiver.recv().unwrap(), Message::Close(..)));
    let (reply, _) = mpsc::channel();
    assert!(
        control
            .command(Message::Flush(ResourceBudget::capture_current(), reply))
            .is_err()
    );
    let (reply, _) = mpsc::channel();
    assert!(
        control
            .command(Message::Close(ResourceBudget::capture_current(), reply))
            .is_err()
    );
}

#[test]
fn reopened_warm_root_reuses_committed_records_without_recapturing_them() {
    let scratch = scratch();
    let mut application = super::super::tests::prepared_fixture(&scratch.0.join("application"));
    application.options.unresolved_policy = rustred::solver::UnresolvedPolicy::Preserve;
    application
        .application
        .set_unresolved_policy(rustred::solver::UnresolvedPolicy::Preserve)
        .unwrap();
    let context = application.family().coefficient_context().clone();
    let binding = application.cache_binding([71; 32]);
    let input = [(IntegralKey::try_new([4]).unwrap(), context.one())];
    let delta = scratch.0.join("warm-delta");
    let mut cold = SavedApplicationRunningCache::open(
        binding.clone(),
        context.clone(),
        &delta,
        Vec::new(),
        manual_options(),
    )
    .unwrap();
    let expected = cold
        .with_io(|backing, capture| {
            application.reduce_linear_combination_with_cache(
                input.clone(),
                &binding,
                Some(backing),
                Some(capture),
            )
        })
        .unwrap();
    let cold_report = cold.control().close().unwrap();
    assert!(cold_report.committed_records > 0);
    drop(cold);
    application.clear_cache().unwrap();
    let before = SavedApplicationCachePack::open_generated(&delta, &binding, Default::default())
        .unwrap()
        .statistics();
    let mut warm = SavedApplicationRunningCache::open(
        binding.clone(),
        context,
        &delta,
        Vec::new(),
        manual_options(),
    )
    .unwrap();
    let actual = warm
        .with_io(|backing, capture| {
            application.reduce_linear_combination_with_cache(
                input,
                &binding,
                Some(backing),
                Some(capture),
            )
        })
        .unwrap();
    assert_eq!(actual.terms(), expected.terms());
    assert_eq!(actual.nonzero_conditions(), expected.nonzero_conditions());
    assert_eq!(actual.statistics().persistent_cache.requested_root_hits, 1);
    let report = warm.control().close().unwrap();
    assert_eq!(report.accepted_records, 0);
    assert_eq!(report.committed_records, 0);
    assert_eq!(report.pending_records, 0);
    assert_eq!(report.generation, cold_report.generation);
    let after = SavedApplicationCachePack::open_generated(&delta, &binding, Default::default())
        .unwrap()
        .statistics();
    assert_eq!(after.chunks, before.chunks);
    assert_eq!(after.records, before.records);
}
