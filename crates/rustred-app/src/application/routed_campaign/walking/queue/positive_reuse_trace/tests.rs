use super::*;
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::Duration;

// CLI's watcher is in a private binary adapter module. This test-only owner
// watches the exact guard path and joins even if the measurement unwinds.
struct StopWatcher {
    cancellation: Arc<AtomicBool>,
    stop: mpsc::SyncSender<()>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl StopWatcher {
    fn start(path: PathBuf) -> Self {
        let requested = |path: &PathBuf| path.try_exists().unwrap_or(true);
        let cancellation = Arc::new(AtomicBool::new(requested(&path)));
        let flag = Arc::clone(&cancellation);
        let (stop, receiver) = mpsc::sync_channel(1);
        let worker = std::thread::spawn(move || {
            loop {
                if requested(&path) {
                    flag.store(true, Ordering::Relaxed);
                }
                match receiver.recv_timeout(Duration::from_millis(100)) {
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        });
        Self {
            cancellation,
            stop,
            worker: Some(worker),
        }
    }
}
impl Drop for StopWatcher {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        self.worker
            .take()
            .unwrap()
            .join()
            .expect("stop watcher panicked");
    }
}

#[test]
fn stop_watcher_observes_preexisting_and_arriving_requests_and_joins() {
    use std::time::{Instant, SystemTime, UNIX_EPOCH};
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../TMP")
        .join(format!(
            "positive-reuse-stop-{}-{unique}",
            std::process::id()
        ));
    std::fs::create_dir_all(root.parent().unwrap()).unwrap();
    std::fs::create_dir(&root).unwrap();
    let path = root.join("stop.json");
    std::fs::write(&path, b"{}").unwrap();
    let watcher = StopWatcher::start(path.clone());
    let flag = Arc::clone(&watcher.cancellation);
    assert!(flag.load(Ordering::Relaxed));
    drop(watcher);
    assert_eq!(Arc::strong_count(&flag), 1); // Thread and watcher both released it.
    std::fs::remove_file(&path).unwrap();
    let watcher = StopWatcher::start(path.clone());
    let flag = Arc::clone(&watcher.cancellation);
    assert!(!flag.load(Ordering::Relaxed));
    std::fs::write(&path, b"{}").unwrap();
    let started = Instant::now();
    while !flag.load(Ordering::Relaxed) && started.elapsed() < Duration::from_secs(2) {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(flag.load(Ordering::Relaxed));
    drop(watcher);
    assert_eq!(Arc::strong_count(&flag), 1);
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(root).unwrap();
}

fn domain<const N: usize>(lo: u64, hi: u64) -> Domain<N> {
    Domain {
        phase: Phase::Apply,
        owner: [true; N],
        lower: vec![lo; N],
        upper: vec![Some(hi); N],
        rank: None,
        powers: Default::default(),
    }
}

#[test]
fn fifo_hits_do_not_refresh_and_source_means_previous_positive() {
    let mut trace = Trace::new(1, 2).unwrap();
    let a = domain::<1>(1, 1);
    let b = domain::<1>(2, 2);
    let c = domain::<1>(3, 3);
    trace.set_source(7, &a);
    trace.positive(&a, 10, true, 11, 1, fingerprint(&a));
    trace.positive(&a, 10, true, 12, 1, fingerprint(&a));
    trace.positive(&b, 10, false, 13, 1, fingerprint(&b));
    trace.set_source(8, &a);
    trace.positive(&a, 10, true, 14, 1, fingerprint(&a));
    trace.positive(&a, 10, false, 15, 1, fingerprint(&a));
    trace.positive(&c, 10, false, 16, 1, fingerprint(&c)); // Evicts a, despite its hits.
    trace.positive(&a, 10, true, 17, 1, fingerprint(&a)); // Cold after eviction.
    assert_eq!(trace.same_source.requests, 2);
    assert_eq!(trace.same_source.forward_comparison_charges, 27);
    assert_eq!(trace.cross_source.requests, 1);
    assert_eq!(trace.cross_source.forward_comparison_charges, 14);
    assert_eq!(trace.misses.requests, 4);
    assert_eq!(trace.evictions, 2);
    assert_eq!(trace.fifo.len(), 2);
}

#[test]
fn full_keys_separate_phase_owner_all_geometry_and_hash_collisions() {
    let a = domain::<2>(1, 2);
    let mut variants = Vec::new();
    for kind in 0..8 {
        let mut b = a.clone();
        match kind {
            0 => b.phase = Phase::Route,
            1 => b.owner[1] = false,
            2 => b.lower[1] = 0,
            3 => b.upper[1] = None,
            4 => b.rank = Some(2),
            5 => b.powers.max_positive_power = Some(7),
            6 => b.powers.min_power_difference = Some(-1),
            7 => b.powers.max_power_difference = Some(4),
            _ => unreachable!(),
        }
        variants.push(b);
    }
    let mut trace = Trace::new(2, 16).unwrap();
    trace.set_source(1, &a);
    trace.positive(&a, 20, true, 1, 1, 42);
    for b in &variants {
        trace.positive(b, 20, true, 1, 1, 42);
    }
    assert_eq!(trace.misses.requests, 9); // Deliberately forced identical hash.
    trace.set_source(2, &a);
    trace.positive(&a, 20, true, 1, 1, 42);
    assert_eq!(trace.cross_source.requests, 1);
    assert_eq!(trace.buckets[&42].len(), 9);
}

#[test]
fn generic_large_arity_and_histogram_overflow_are_explicit() {
    let a = domain::<20>(0, 1);
    let mut b = a.clone();
    b.phase = Phase::Route;
    let mut trace = Trace::with_destination_limits(20, 2, 1, 1024).unwrap();
    trace.set_source(0, &a);
    trace.destination(&a).requests += 1;
    trace.positive(&a, 0, true, 2, 1, fingerprint(&a));
    trace.destination(&b).requests += 1;
    trace.positive(&b, 1, false, 3, 1, fingerprint(&b));
    let report = trace.report();
    assert_eq!(report["arity"], 20);
    assert_eq!(report["destination_retained_keys"], 1);
    assert_eq!(report["queue_requests"], 2);
    assert_eq!(report["destination_overflow"]["queue_requests"], 1);
    assert_eq!(report["destination_overflow"]["positive_nonexact"], 1);
    assert_eq!(
        report["destination_overflow"]["positive_forward_comparison_charges"],
        3
    );
    let mut byte_limited = Trace::with_destination_limits(20, 2, 8, 1).unwrap();
    byte_limited.destination(&a).requests += 1;
    assert_eq!(byte_limited.destination_entries, 0);
    assert_eq!(byte_limited.destination_overflow.requests, 1);
    assert!(Trace::new(usize::MAX, 2).is_err());
}

#[test]
fn exact_new_orthant_error_and_uncommitted_cancelled_preparation_are_not_positive() {
    let mut q = Queue::new(1, Some(10));
    q.admit(domain::<1>(0, 10)).unwrap();
    let session = Session::start(1, 4).unwrap();
    set_source(0, &q.domains[0]);
    assert_eq!(q.admit(domain::<1>(0, 10)).unwrap(), (0, false)); // Exact.
    assert!(q.admit(domain::<1>(20, 21)).is_err()); // New domain allowance.
    let cancel = AtomicBool::new(true);
    let _uncommitted = q.prepare_admission(domain::<1>(1, 2), &cancel);
    let report = session.finish();
    assert_eq!(report["positive_nonexact"]["requests"], 0);
    assert_eq!(report["insertions"], 0);

    let mut q = Queue::new(10, None);
    let mut whole = domain::<1>(0, 0);
    whole.upper[0] = None;
    q.admit(whole).unwrap();
    let session = Session::start(1, 4).unwrap();
    set_source(0, &q.domains[0]);
    assert_eq!(q.admit(domain::<1>(2, 3)).unwrap(), (0, false)); // Orthant.
    let report = session.finish();
    assert_eq!(report["positive_nonexact"]["requests"], 0);

    let mut q = Queue::new(10, None);
    let session = Session::start(1, 4).unwrap();
    set_source(0, &domain::<1>(0, 0));
    assert_eq!(q.admit(domain::<1>(2, 3)).unwrap(), (0, true)); // New.
    assert_eq!(session.finish()["positive_nonexact"]["requests"], 0);
}

#[test]
fn retired_representative_is_metadata_never_cached_authority() {
    let mut q = Queue::new(10, None);
    q.admit(domain::<1>(0, 10)).unwrap();
    let session = Session::start(1, 4).unwrap();
    set_source(0, &q.domains[0]);
    assert_eq!(q.admit(domain::<1>(1, 2)).unwrap(), (0, false));
    assert_eq!(q.admit(domain::<1>(0, 20)).unwrap(), (1, true));
    assert_eq!(q.containment_retired_candidates, 1);
    set_source(1, &q.domains[1]);
    assert_eq!(q.admit(domain::<1>(1, 2)).unwrap(), (1, false)); // Not stale memo ID0.
    let report = session.finish();
    assert_eq!(report["cross_source_hits"]["requests"], 1);
    assert_eq!(report["representative_changes_on_hit"], 1);
    assert_eq!(
        report["cross_source_examples"][0]["first_representative"],
        0
    );
    assert_eq!(
        report["cross_source_examples"][0]["current_representative"],
        1
    );
}

#[test]
fn observation_preserves_queue_image_and_forward_charges_without_maintenance() {
    let mut reference = None;
    for observed in [false, true] {
        let mut q = Queue::new(10, None);
        q.admit(domain::<1>(0, 10)).unwrap();
        let session = observed.then(|| Session::start(1, 4).unwrap());
        set_source(0, &q.domains[0]);
        let before = q.containment_checks;
        q.admit(domain::<1>(1, 2)).unwrap();
        let first_checks = q.containment_checks - before;
        q.admit(domain::<1>(0, 20)).unwrap(); // Reverse maintenance is not observed.
        set_source(1, &q.domains[1]);
        let before = q.containment_checks;
        let ready = q.prepare_admission(domain::<1>(1, 2), &AtomicBool::new(false));
        q.admit_prepared(ready).unwrap();
        let second_checks = q.containment_checks - before;
        if let Some(session) = session {
            let report = session.finish();
            assert_eq!(
                report["positive_nonexact"]["forward_comparison_charges"],
                first_checks + second_checks
            );
            assert_eq!(report["positive_nonexact"]["summary_builds"], 2);
        }
        let image = serde_json::to_value(&q).unwrap();
        if let Some(reference) = &reference {
            assert_eq!(&image, reference);
        } else {
            reference = Some(image);
        }
    }
}

#[test]
fn session_is_thread_local_and_drop_clears_unfinished_measurement() {
    assert!(!enabled());
    {
        let _session = Session::start(1, 2).unwrap();
        assert!(enabled());
        std::thread::spawn(|| assert!(!enabled())).join().unwrap();
    }
    assert!(!enabled());
    let session = Session::start(1, 2).unwrap();
    assert_eq!(session.finish()["positive_nonexact"]["requests"], 0);
    assert!(!enabled());
}

#[test]
#[ignore = "explicit source-bound native spectator; no cache or performance claim"]
fn measure_real_ordered_positive_reuse() {
    use crate::{
        OwnerDomainMatchRequest, OwnerDomainWalkCheckpointOptions,
        OwnerDomainWalkPublicationPolicy, OwnerDomainWalkRequest, OwnerDomainWalkSchedulingPolicy,
        owner_domain_walk_with_progress,
    };
    use rustred::solver::OwnerDomainRefinementAxes;
    use std::fs::{File, OpenOptions};
    use std::io::{BufReader, BufWriter, Write};
    use std::time::Instant;
    fn save(path: &std::path::Path, v: &Value) {
        let mut out = BufWriter::new(
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .unwrap(),
        );
        serde_json::to_writer(&mut out, v).unwrap();
        out.write_all(b"\n").unwrap();
        out.flush().unwrap();
        out.get_ref().sync_all().unwrap();
    }
    assert!(
        std::env::var_os("SYMBOLICA_LICENSE").is_some(),
        "licensed native execution is mandatory, not skipped"
    );
    let root = PathBuf::from(
        std::env::var_os("RUSTRED_POSITIVE_REUSE_DIRECTORY").expect("explicit spectator directory"),
    );
    let input: Value =
        serde_json::from_reader(BufReader::new(File::open(root.join("input.json")).unwrap()))
            .unwrap();
    let stop_file = PathBuf::from(input["stop_file"].as_str().unwrap());
    assert_eq!(stop_file, root.join("guard/stop-request.json"));
    let selection = std::fs::read_to_string(input["manifest"].as_str().unwrap()).unwrap();
    let queries = std::fs::read_to_string(input["queries"].as_str().unwrap()).unwrap();
    let parsed: Value = serde_json::from_str(&queries).unwrap();
    assert_eq!(parsed["queries"].as_array().unwrap().len(), 1);
    let arity = parsed["queries"][0]["owner"].as_str().unwrap().len();
    let mut matching = OwnerDomainMatchRequest::new(selection, queries.clone());
    matching.owner_base = input["owner_base"].as_str().unwrap().into();
    matching.max_queries = 1;
    matching.max_query_bytes = queries.len();
    matching.match_limits.refinement_axes = OwnerDomainRefinementAxes::FiniteAxes;
    matching.match_limits.guard_algebra.max_univariate_degree = 64;
    let mut request = OwnerDomainWalkRequest::new(matching);
    request.workers = 6;
    request.scheduling_policy = OwnerDomainWalkSchedulingPolicy::TransferUnreserved {
        lookahead: std::num::NonZeroUsize::new(256).unwrap(),
    };
    request.route_domain_overcover = true;
    request.reuse_initial_d_bands = true;
    request.checkpoint = Some(OwnerDomainWalkCheckpointOptions::new(
        root.join("checkpoint"),
    ));
    request.disable_work_limits();
    assert_eq!(
        request.publication_policy,
        OwnerDomainWalkPublicationPolicy::Ordered
    );
    assert!(request.apply_subdivision.is_none());
    let baseline: Value = serde_json::from_reader(BufReader::new(
        File::open(input["baseline_audit"].as_str().unwrap()).unwrap(),
    ))
    .unwrap();
    let capacity = usize::try_from(input["entry_capacity"].as_u64().unwrap()).unwrap();
    let session = Session::start(arity, capacity).unwrap();
    let events = RefCell::new(BufWriter::new(
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(root.join("events.jsonl"))
            .unwrap(),
    ));
    let last = RefCell::new(Instant::now() - Duration::from_secs(2));
    let watcher = StopWatcher::start(stop_file);
    let result = owner_domain_walk_with_progress(request, &watcher.cancellation, |event| {
        let milestone = matches!(
            event["event"].as_str(),
            Some(
                "checkpoint_started"
                    | "checkpoint_saved"
                    | "finished"
                    | "loaded"
                    | "routes_verified"
            )
        );
        if milestone || last.borrow().elapsed() >= Duration::from_secs(1) {
            let mut out = events.borrow_mut();
            serde_json::to_writer(&mut *out, &event).unwrap();
            out.write_all(b"\n").unwrap();
            out.flush().unwrap();
            *last.borrow_mut() = Instant::now();
        }
    });
    drop(watcher);
    events.borrow_mut().flush().unwrap();
    let trace = session.finish();
    save(&root.join("spectator.json"), &trace);
    let result = match result {
        Ok(r) => r,
        Err(e) => {
            save(&root.join("error.json"), &json!({"error":e.to_string()}));
            panic!("native spectator failed: {e}")
        }
    };
    save(&root.join("result.json"), &result.document);
    assert!(result.all_scheduled_domains_resolved);
    for (field, expected) in [
        ("completed_nodes", "native"),
        ("scheduled_nodes", "logical"),
        ("events", "events"),
    ] {
        assert_eq!(
            result.document[field], baseline[expected],
            "unchanged Ordered {field}"
        );
    }
    assert_eq!(result.document["frontiers"], 0);
    assert_eq!(result.document["queued_nodes"], 0);
    assert!(trace["positive_nonexact"]["requests"].as_u64().unwrap() > 0);
    assert_eq!(trace["test_index_counters_disabled_and_zero_before"], true);
    assert_eq!(trace["test_index_counters_disabled_and_zero_after"], true);
    eprintln!("real Ordered positive-inclusion spectator executed; no cache hits served");
}
