//! One real-native S3 comparison, not public resume or crash-during-CAS coverage.
//! Hold valid native bytes before publication, save before joining that worker,
//! discard the entire uncommitted cut, and replay it with a different width.
use super::*;
use crate::application::routed_campaign::walking::{
    epoch::{
        inspector::{Context, inspect_job, inspect_job_with_snapshot},
        job::{Writer, write_image},
        snapshot::Publication,
    },
    initial_overlap::InitialOverlapIndex,
};

pub(super) fn finish_native(fixture: &Fixture, restored: &mut Restored<1>) {
    finish_native_mode(fixture, restored, controller::LookupMode::AllMiss)
}

pub(super) fn finish_native_mode(
    fixture: &Fixture,
    restored: &mut Restored<1>,
    mode: controller::LookupMode,
) {
    let overlap = InitialOverlapIndex::empty();
    let cancellation = AtomicBool::new(false);
    let context = Context {
        reducer: &fixture.reducer,
        request: &fixture.request,
        overlap: &overlap,
        cancellation: &cancellation,
        g2: None,
        finite_account: None,
    };
    assert_eq!(
        controller::run_native_lookup(restored, &fixture.identity(), 16, &context, mode, |_, _| {})
            .unwrap(),
        Outcome::Drained
    );
}

/// Compare actual sidecar bodies, not just records_digest (id/tag/outdegree).
/// Only the native record's top-level elapsed `seconds` is nondeterministic;
/// merge epochs, v0, resolver counters, errors, geometry and scope remain exact.
pub(in super::super) fn completed_snapshot(restored: &mut Restored<1>) -> Value {
    assert!(restored.warnings.is_empty());
    assert!(restored.replay.is_empty());
    assert!(restored.state.in_flight.is_empty());
    assert_eq!(restored.state.pending_or_reserved(), 0);
    assert_eq!(restored.state.counters.native_errors, 0);
    assert_eq!(restored.state.counters.frontiers, 0);
    assert!(restored.roots.frontiers.is_empty());
    assert!(restored.state.counters.natives > 0);
    assert!(
        restored.state.edges.edges() > 0,
        "nontrivial dependency graph"
    );
    assert!(restored.stop_reason.is_none(), "certified drain required");
    assert!(restored.operational_stop.is_none());
    restored
        .state
        .tracker
        .refresh(&AtomicBool::new(false), true);
    let closed: Vec<_> = (0..restored.state.store.len())
        .map(|id| restored.state.tracker.closed(id))
        .collect();
    assert!(closed.iter().all(|flag| *flag == Some(true)));
    let mut records = Vec::new();
    restored
        .records
        .files()
        .for_each_record(|record| {
            let mut record = record.project()?;
            record
                .as_object_mut()
                .expect("record object")
                .remove("seconds");
            records.push(record);
            Ok(())
        })
        .unwrap();
    assert_eq!(records.len() as u64, restored.state.edges.runs());
    assert_eq!(records.len(), restored.state.store.len());
    let images: Vec<_> = restored
        .state
        .store
        .domains
        .iter()
        .map(|image| {
            let mut bytes = Writer::default();
            write_image(&mut bytes, image);
            bytes.0
        })
        .collect();
    let mut tracker_edges: Vec<_> = restored.state.tracker.dependencies().collect();
    tracker_edges.sort_unstable();
    json!({
        "records": records, "images": images,
        "roots": restored.roots.rows, "root_frontiers": restored.roots.frontiers,
        "ledger": restored.state.ledger.words(), "nodes": restored.state.nodes,
        "live": restored.state.live, "anchors": restored.state.anchors.encode().unwrap(),
        "edge_runs": restored.state.edges.log(),
        "edge_digest": restored.state.edges.edge_digest(),
        "records_digest": restored.state.edges.records_digest(),
        "k": restored.state.k, "p0": restored.state.p0,
        "walk_counters": restored.state.counters, "verify": restored.state.verify,
        "lookup": restored.state.lookup, "frontier_counts": restored.state.frontier_counts,
        "tracker_flags": restored.state.tracker.node_flags().collect::<Vec<_>>(),
        "tracker_edges": tracker_edges, "closed": closed
    })
}

pub(in super::super) fn closed_fixture() -> Fixture {
    let mut fixture = Fixture::new();
    // Reuse the closed one-axis S2 fixture's narrow/narrow/whole-ray queries,
    // retaining this fixture's explicit required/auxiliary roles and three IDs.
    for (query, (lower, upper)) in
        fixture
            .queries
            .iter_mut()
            .zip([(2, Some(2)), (3, Some(3)), (0, None)])
    {
        query.lower = vec![lower];
        query.upper = vec![upper];
        query.rank = Some(11);
    }
    fixture.request.matching.queries_json = json!({
        "schema":"rustred.owner-domain-queries.json.v2",
        "queries": fixture.queries.iter().map(|query| json!({
            "id":query.id,"owner":"1","lower":query.lower,"upper":query.upper,
            "max_numerator_rank":query.rank
        })).collect::<Vec<_>>(),
        "query_roles":{"required":["q-0","q-2"],"auxiliary":["q-1"]}
    })
    .to_string();
    fixture.request.workers = 2;
    fixture
}

#[test]
fn real_native_interrupted_cut_resplits_to_identical_graph_and_records() {
    const TEST: &str = "real native epoch interrupted/resplit equivalence";
    // A skip is explicitly not equivalence coverage. Strict license mode fails
    // the existing gate; capability is also checked on every actual pool worker.
    if !crate::test_gates::workers_or_skip(TEST, 3)
        || !std::thread::spawn(|| crate::test_gates::licensed_or_skip(TEST))
            .join()
            .unwrap()
    {
        return;
    }
    for mode in [
        controller::LookupMode::AllMiss,
        controller::LookupMode::Snapshot,
    ] {
        interrupted_native_mode(mode);
    }
}

fn interrupted_native_mode(mode: controller::LookupMode) {
    let mut fixture = closed_fixture();
    fixture.save(3, 3);
    let mut baseline = fixture.open().unwrap();
    finish_native_mode(&fixture, &mut baseline, mode);
    drop(baseline);
    let expected = completed_snapshot(&mut fixture.open().unwrap());

    // Same reducer, request and initial cut in a separate fresh test directory.
    fixture.directory = Directory::new();
    fixture.save(3, 3);
    let mut interrupted = fixture.open().unwrap();
    let original_replay = interrupted.replay.clone();
    let original_session = interrupted.dispatch.checkpoint_snapshot().session;
    let original_ledger = interrupted.state.ledger.words().to_vec();
    let original_counters = serde_json::to_value(interrupted.state.counters).unwrap();
    let gate = (Mutex::new(false), Condvar::new());
    let held = AtomicBool::new(false);
    let returned = AtomicBool::new(false);
    let calls = AtomicUsize::new(0);
    let overlap = InitialOverlapIndex::empty();
    let snapshots = Publication::new();
    let (saved, receiving_save) = mpsc::channel();
    let latest = fixture.directory.0.join(publication::LATEST);
    std::thread::scope(|scope| {
        let observed_gate = &gate;
        let observed_returned = &returned;
        let observer = scope.spawn(move || {
            let generation = receiving_save
                .recv_timeout(Duration::from_secs(60))
                .unwrap();
            assert!(!observed_returned.load(Ordering::Acquire));
            assert_eq!(
                publication::read_manifest(&latest).unwrap().generation,
                generation
            );
            *observed_gate.0.lock().unwrap() = true;
            observed_gate.1.notify_all();
        });
        let authorize = || {
            symbolica::license::LicenseManager::is_licensed()
                .then_some(())
                .ok_or_else(|| "real native test worker lacks Symbolica authorization".into())
        };
        let inspect = |bytes: &[u8], cancel: &AtomicBool| {
            calls.fetch_add(1, Ordering::Relaxed);
            let job = Job::<1>::decode(bytes).unwrap();
            assert!(job.parent < 2, "third Reserved job must remain queued");
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
            let decoded = JobResult::<1>::decode(&native).unwrap();
            assert_eq!(decoded.seq, job.seq);
            assert_eq!(decoded.parent, job.parent);
            assert_eq!(decoded.v0, job.v0);
            assert!(!decoded.panic);
            assert_eq!(decoded.error_kind, ErrorKind::None);
            assert_eq!(decoded.break_reason, BreakReason::None);
            assert!(decoded.frontiers.is_empty());
            if job.parent == 1 {
                // Native work really finished: hold its VALID result, not a
                // fabricated payload, until the durable cut has been saved.
                held.store(true, Ordering::Release);
                let (open, timeout) = gate
                    .1
                    .wait_timeout_while(gate.0.lock().unwrap(), Duration::from_secs(60), |open| {
                        !*open
                    })
                    .unwrap();
                assert!(!timeout.timed_out() && *open);
                assert!(cancel.load(Ordering::Acquire));
                returned.store(true, Ordering::Release);
            }
            native
        };
        let mut observed_held = 0;
        assert_eq!(
            controller::run_authorized_lookup_periodic(
                &mut interrupted,
                &fixture.identity(),
                16,
                2,
                config(),
                &authorize,
                &inspect,
                || {
                    if held.load(Ordering::Acquire) {
                        observed_held += 1;
                    }
                    // One inspector: Started(0), Result(0), Started(1) are FIFO.
                    // Three polls guarantee Result(0) reached the polled prefix.
                    stop::requested(&AtomicBool::new(observed_held >= 3), None)
                },
                |_| false,
                |receipt, status| {
                    assert_eq!(status.len(), 3);
                    assert!(status[0].started && status[0].returned);
                    assert!(status[1].started && !status[1].returned);
                    assert!(!status[2].started && !status[2].returned);
                    assert!(!returned.load(Ordering::Acquire));
                    saved.send(receipt.generation).unwrap();
                },
                (mode == controller::LookupMode::Snapshot).then_some(&snapshots),
            )
            .unwrap(),
            Outcome::Stopped(merge::StopReason::Paused)
        );
        observer.join().unwrap();
    });
    assert_eq!(calls.load(Ordering::Relaxed), 2);
    interrupted.state.store.ensure_unique().unwrap();
    assert!(
        returned.load(Ordering::Acquire),
        "held worker joined after save"
    );
    assert_eq!(interrupted.state.k, 0);
    assert_eq!(interrupted.records.total(), 0);
    assert_eq!(interrupted.state.edges.runs(), 0);
    assert_eq!(interrupted.state.ledger.words(), original_ledger);
    assert_eq!(
        serde_json::to_value(interrupted.state.counters).unwrap(),
        original_counters
    );
    assert!(!fixture.directory.0.join("epoch-poison").exists());
    drop(interrupted);

    // Width is operational, not part of epoch_request_binding. The saved cut
    // keeps parent/v0/image/attempts/order but must receive a fresh session/seq.
    fixture.request.workers = 3;
    let mut resumed = fixture.open().unwrap();
    assert!(resumed.warnings.is_empty());
    assert_eq!(
        resumed.dispatch.checkpoint_snapshot().session,
        original_session + 1
    );
    assert_eq!(resumed.replay.len(), original_replay.len());
    for (old, new) in original_replay.iter().zip(&resumed.replay) {
        assert_ne!(new.seq, old.seq);
        assert_eq!(new.seq >> 40, original_session + 1);
        let mut comparable = new.clone();
        comparable.seq = old.seq;
        assert_eq!(&comparable, old);
    }
    assert_eq!(resumed.state.ledger.words(), original_ledger);
    assert_eq!(resumed.records.total(), 0);
    assert_eq!(resumed.state.edges.runs(), 0);
    finish_native_mode(&fixture, &mut resumed, mode);
    drop(resumed);
    assert_eq!(completed_snapshot(&mut fixture.open().unwrap()), expected);
}

#[test]
fn real_native_snapshot_lookup_preserves_all_miss_math_and_records() {
    const TEST: &str = "real native epoch snapshot lookup differential";
    if !crate::test_gates::workers_or_skip(TEST, 2)
        || !std::thread::spawn(|| crate::test_gates::licensed_or_skip(TEST))
            .join()
            .unwrap()
    {
        return;
    }
    let mut fixture = closed_fixture();
    fixture.save(3, 2);
    let mut off = fixture.open().unwrap();
    finish_native_mode(&fixture, &mut off, controller::LookupMode::AllMiss);
    let mut expected = completed_snapshot(&mut off);
    fixture.directory = Directory::new();
    fixture.save(3, 2);
    let mut on = fixture.open().unwrap();
    finish_native_mode(&fixture, &mut on, controller::LookupMode::Snapshot);
    assert!(
        on.state.inspector_lookup.stored_hits > 0,
        "real native lookup path exercised"
    );
    assert!(on.state.inspector_lookup.queries >= on.state.inspector_lookup.stored_hits);
    assert_eq!(
        on.state.inspector_lookup.queries,
        on.state.inspector_lookup.stored_hits
            + on.state.inspector_lookup.coordinator_miss_rechecks_skipped,
        "every accepted distinct worker query is a positive or a skipped recheck"
    );
    assert_eq!(
        off.state.inspector_lookup.coordinator_miss_rechecks_skipped,
        0
    );
    let mut actual = completed_snapshot(&mut on);
    // Explicitly different work accounting, not mathematical state. Keep every
    // record field, domain, ledger, edge, root and closure comparison intact.
    for snapshot in [&mut expected, &mut actual] {
        snapshot.as_object_mut().unwrap().remove("lookup");
        snapshot.as_object_mut().unwrap().remove("verify");
        snapshot["walk_counters"]
            .as_object_mut()
            .unwrap()
            .remove("miss_requests");
    }
    assert_eq!(actual, expected);
}
