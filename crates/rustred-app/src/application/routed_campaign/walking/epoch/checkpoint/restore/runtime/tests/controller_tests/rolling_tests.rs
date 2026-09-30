use super::*;
use crate::application::routed_campaign::walking::epoch::snapshot::Publication;
mod escrow_tests;

struct ReleaseGateOnDrop<'a>(&'a (Mutex<bool>, Condvar));
impl Drop for ReleaseGateOnDrop<'_> {
    fn drop(&mut self) {
        *self.0.0.lock().unwrap() = true;
        self.0.1.notify_all();
    }
}

fn rolling_config() -> MergeConfig {
    MergeConfig {
        lockstep: false,
        ..config()
    }
}

fn rolling_fixture(count: usize) -> Fixture {
    let mut fixture = Fixture::new();
    fixture.request.epoch_rolling = true;
    fixture.request.checkpoint = Some(crate::OwnerDomainWalkCheckpointOptions::new(
        fixture.directory.0.clone(),
    ));
    fixture.queries = (0..count)
        .map(|id| Query {
            id: format!("q-{id}"),
            auxiliary: false,
            role_declared: true,
            owner: vec![true],
            lower: vec![id as u64],
            upper: vec![Some(id as u64)],
            rank: Some(2),
            powers: Default::default(),
        })
        .collect();
    // Identity binds the parsed role/root list; keep the source document in
    // agreement instead of relying on the original three-row fixture.
    fixture.request.matching.queries_json = json!({
        "schema":"rustred.owner-domain-queries.json.v2",
        "queries": fixture.queries.iter().map(|q| json!({
            "id":q.id,"owner":"1","lower":q.lower,"upper":q.upper,
            "max_numerator_rank":2
        })).collect::<Vec<_>>(),
        "query_roles":{"required":fixture.queries.iter().map(|q| &q.id).collect::<Vec<_>>(), "auxiliary":[]}
    })
    .to_string();
    fixture
}

#[test]
fn oldest_ready_publishes_refills_and_saves_around_held_sequence_zero_then_replays_holes() {
    for with_snapshot in [false, true] {
        let mut fixture = rolling_fixture(48);
        fixture.request.epoch_publication_order =
            crate::OwnerDomainWalkEpochPublicationOrder::OldestReady;
        fixture.request.epoch_window = Some(32);
        fixture.save_window(48, 0, false, 32);
        let identity = fixture.identity();
        let mut restored = fixture.open().unwrap();
        let snapshots = Publication::new();
        let gate = (Mutex::new(false), Condvar::new());
        let held = AtomicBool::new(false);
        let returned = AtomicBool::new(false);
        let refilled = AtomicBool::new(false);
        let cancel = AtomicBool::new(false);
        let expected_remaining: Vec<_> = std::iter::once(0).chain(17..48).collect();
        // Total budget three means exactly two inspectors. Once zero holds
        // one, the other processes 1..31 serially from the shared FIFO queue;
        // the first ready cut is therefore deterministically 1..16.
        let inspect = |bytes: &[u8], _: &AtomicBool| {
            let job = Job::<1>::decode(bytes).unwrap();
            let view = with_snapshot.then(|| snapshots.acquire_job(job.seq).unwrap());
            if job.parent == 0 {
                held.store(true, Ordering::Release);
                let (guard, timeout) = gate
                    .1
                    .wait_timeout_while(gate.0.lock().unwrap(), Duration::from_secs(10), |open| {
                        !*open
                    })
                    .unwrap();
                assert!(!timeout.timed_out() && *guard);
                if let Some(view) = &view {
                    assert_eq!(view.version, 0);
                    assert_eq!(view.len(), 48);
                }
                returned.store(true, Ordering::Release);
            } else if job.parent == 1 {
                let deadline = std::time::Instant::now() + Duration::from_secs(10);
                while !held.load(Ordering::Acquire) {
                    assert!(std::time::Instant::now() < deadline);
                    std::thread::yield_now();
                }
            } else if job.parent == 32 {
                assert_eq!(job.v0, 1, "new work sees the committed nonprefix cut");
                if let Some(view) = &view {
                    assert_eq!(view.version, 1);
                }
                refilled.store(true, Ordering::Release);
                cancel.store(true, Ordering::Release);
            }
            result(&job, false)
        };
        let mut saved = 0;
        assert_eq!(
            controller::run_observed(
                &mut restored,
                &identity,
                32,
                3,
                rolling_config(),
                &|| Ok(()),
                &inspect,
                || stop::requested(&cancel, None),
                |_| false,
                |state, _, status| {
                    let _release = ReleaseGateOnDrop(&gate);
                    saved += 1;
                    assert!(held.load(Ordering::Acquire));
                    assert!(!returned.load(Ordering::Acquire));
                    assert!(refilled.load(Ordering::Acquire));
                    assert_eq!(state.state.k, 1);
                    assert_eq!(state.records.total(), 16);
                    let mut remaining = state.state.in_flight.keys().copied().collect::<Vec<_>>();
                    remaining.sort_unstable();
                    assert_eq!(remaining, expected_remaining);
                    assert_eq!(status.len(), 32);
                    // Stop saves before joining this genuinely older reader.
                },
                with_snapshot.then_some(&snapshots),
                |_, _, _, _| {},
                |_| {},
            )
            .unwrap(),
            Outcome::Stopped(merge::StopReason::Paused)
        );
        assert_eq!(saved, 1);
        assert!(returned.load(Ordering::Acquire));
        // Selection is counted before P2, not at publication. Once job32
        // requests the stop, a second full cut may already have been selected
        // and then cancelled by responsive preparation. It cannot publish:
        // the callback above still requires exactly k=1 and sixteen records.
        assert!((1..=2).contains(&restored.rolling_diagnostics.selected_nonprefix_cuts));
        assert_eq!(
            restored.rolling_diagnostics.selected_cuts,
            restored.rolling_diagnostics.selected_nonprefix_cuts
        );
        assert_eq!(restored.rolling_diagnostics.selected_partial_cuts, 0);
        drop(restored);
        let mut restored = fixture.open().unwrap();
        assert_eq!(restored.window, 32);
        assert_eq!(
            restored
                .replay
                .iter()
                .map(|job| job.parent)
                .collect::<Vec<_>>(),
            expected_remaining
        );
        assert!(restored.replay.iter().all(|job| job.v0 == 1));
        let seen = Mutex::new(Vec::new());
        assert_eq!(
            controller::run_observed(
                &mut restored,
                &identity,
                32,
                1,
                rolling_config(),
                &|| Ok(()),
                &|bytes, _| {
                    let job = Job::<1>::decode(bytes).unwrap();
                    seen.lock().unwrap().push(job.parent);
                    result(&job, false)
                },
                || None,
                |_| false,
                |_, _, _| {},
                None,
                |_, _, _, _| {},
                |_| {},
            )
            .unwrap(),
            Outcome::Drained
        );
        assert_eq!(*seen.lock().unwrap(), expected_remaining);
        assert_eq!(restored.records.total(), 48);
        assert_eq!(restored.state.k, 3);
    }
}

#[test]
fn rolling_publishes_and_saves_while_old_reader_runs_then_reissues_only_unmerged_work() {
    for with_snapshot in [false, true] {
        let fixture = rolling_fixture(20);
        fixture.save_window(20, 0, false, 32);
        let identity = fixture.identity();
        let mut restored = fixture.open().unwrap();
        assert_eq!(restored.window, 32);
        let snapshots = Publication::new();
        let gate = (Mutex::new(false), Condvar::new());
        let held = AtomicBool::new(false);
        let returned = AtomicBool::new(false);
        let cancel = AtomicBool::new(false);
        let phase_activity = std::cell::Cell::new(0u8);
        let mut saves = 0;
        let inspect = |bytes: &[u8], _: &AtomicBool| {
            let job = Job::<1>::decode(bytes).unwrap();
            let view = with_snapshot.then(|| snapshots.acquire_job(job.seq).unwrap());
            if job.parent == 15 {
                // Guarantee a reader from the following cohort is active
                // before the oldest 16-result prefix becomes publishable.
                let deadline = std::time::Instant::now() + Duration::from_secs(10);
                while !held.load(Ordering::Acquire) {
                    assert!(std::time::Instant::now() < deadline);
                    std::thread::yield_now();
                }
            }
            if job.parent == 16 {
                held.store(true, Ordering::Release);
                let (guard, timeout) = gate
                    .1
                    .wait_timeout_while(gate.0.lock().unwrap(), Duration::from_secs(10), |open| {
                        !*open
                    })
                    .unwrap();
                assert!(!timeout.timed_out() && *guard);
                if let Some(view) = &view {
                    assert_eq!(view.version, 0);
                    assert_eq!(view.len(), 20);
                }
                returned.store(true, Ordering::Release);
            }
            result(&job, false)
        };
        assert_eq!(
            controller::run_observed(
                &mut restored,
                &identity,
                32,
                3,
                rolling_config(),
                &|| Ok(()),
                &inspect,
                || stop::requested(&cancel, None),
                |k| k == 1,
                |state, receipt, status| {
                    saves += 1;
                    assert_eq!(state.state.k, 1);
                    assert_eq!(state.records.total(), 16);
                    assert_eq!(state.state.in_flight.len(), 4);
                    assert!(state.state.in_flight.values().all(|meta| meta.v0 == 0));
                    assert!(held.load(Ordering::Acquire));
                    assert!(!returned.load(Ordering::Acquire));
                    assert_eq!(
                        publication::read_manifest(&fixture.directory.0.join(publication::LATEST))
                            .unwrap()
                            .generation,
                        receipt.generation
                    );
                    if saves == 1 {
                        assert!(
                            status.is_empty(),
                            "periodic checkpoint does not drain inspectors"
                        );
                        cancel.store(true, Ordering::Release);
                    } else {
                        assert_eq!(status.len(), 4);
                        *gate.0.lock().unwrap() = true;
                        gate.1.notify_all();
                    }
                },
                with_snapshot.then_some(&snapshots),
                |state, _, phase, activity| {
                    if matches!(phase, "p1" | "p3") {
                        let value = activity().expect("live pool observation");
                        assert_eq!(state.k, 0);
                        assert!(value.computing >= 1, "held reader overlaps merge phase");
                        assert!(value.returned >= 16);
                        assert_eq!(value.occupied, 20);
                        assert_eq!(
                            value.occupied,
                            value.queued + value.computing + value.returned
                        );
                        phase_activity
                            .set(phase_activity.get() | if phase == "p1" { 1 } else { 2 });
                    }
                    if phase == "joined" {
                        assert_eq!(activity().unwrap().occupied, 0);
                    }
                },
                |_| {}
            )
            .unwrap(),
            Outcome::Stopped(merge::StopReason::Paused)
        );
        assert_eq!(saves, 2);
        assert_eq!(phase_activity.get(), 3);
        assert!(returned.load(Ordering::Acquire));
        if !with_snapshot {
            assert_eq!(
                restored.state.store.retained().0,
                0,
                "AllMiss has no lookup replicas"
            );
        }
        drop(restored);
        let mut restored = fixture.open().unwrap();
        assert_eq!(
            restored.window, 32,
            "saved logical bound is independent of W1 resume"
        );
        assert_eq!(
            restored
                .replay
                .iter()
                .map(|job| job.parent)
                .collect::<Vec<_>>(),
            [16, 17, 18, 19]
        );
        assert!(restored.replay.iter().all(|job| job.v0 == 1));
        let seen = Mutex::new(Vec::new());
        assert_eq!(
            controller::run_observed(
                &mut restored,
                &identity,
                32,
                1,
                rolling_config(),
                &|| Ok(()),
                &|bytes, _| {
                    let job = Job::<1>::decode(bytes).unwrap();
                    seen.lock().unwrap().push((job.parent, job.v0));
                    result(&job, false)
                },
                || None,
                |_| false,
                |_, _, _| {},
                None,
                |_, _, _, _| {},
                |_| {}
            )
            .unwrap(),
            Outcome::Drained
        );
        assert_eq!(*seen.lock().unwrap(), [(16, 1), (17, 1), (18, 1), (19, 1)]);
        assert_eq!(restored.records.total(), 20);
        assert_eq!(restored.state.k, 2);
    }
}

#[test]
fn rolling_partial_replay_retires_before_new_pending_dispatch() {
    for policy in [
        crate::OwnerDomainWalkEpochPublicationOrder::OldestPrefix,
        crate::OwnerDomainWalkEpochPublicationOrder::OldestReady,
    ] {
        rolling_partial_replay_mode(policy);
    }
}

fn rolling_partial_replay_mode(policy: crate::OwnerDomainWalkEpochPublicationOrder) {
    let mut fixture = rolling_fixture(3);
    fixture.request.epoch_publication_order = policy;
    fixture.save(3, 2);
    let mut restored = fixture.open().unwrap();
    let seen = Mutex::new(Vec::new());
    assert_eq!(
        controller::run_observed(
            &mut restored,
            &fixture.identity(),
            16,
            1,
            rolling_config(),
            &|| Ok(()),
            &|bytes, _| {
                let job = Job::<1>::decode(bytes).unwrap();
                seen.lock().unwrap().push((job.parent, job.v0));
                result(&job, false)
            },
            || None,
            |_| false,
            |_, _, _| {},
            None,
            |_, _, _, _| {},
            |_| {}
        )
        .unwrap(),
        Outcome::Drained
    );
    assert_eq!(*seen.lock().unwrap(), [(0, 0), (1, 0), (2, 1)]);
    assert_eq!(restored.state.k, 2);
}

#[test]
fn default_prefix_never_publishes_later_results_while_sequence_zero_is_held() {
    let fixture = rolling_fixture(48);
    fixture.save_window(48, 0, false, 32);
    let mut restored = fixture.open().unwrap();
    let gate = (Mutex::new(false), Condvar::new());
    let held = AtomicBool::new(false);
    let cancel = AtomicBool::new(false);
    let inspect = |bytes: &[u8], _: &AtomicBool| {
        let job = Job::<1>::decode(bytes).unwrap();
        if job.parent == 0 {
            held.store(true, Ordering::Release);
            let (guard, timeout) = gate
                .1
                .wait_timeout_while(gate.0.lock().unwrap(), Duration::from_secs(10), |open| {
                    !*open
                })
                .unwrap();
            assert!(!timeout.timed_out() && *guard);
        } else if job.parent == 31 {
            let deadline = std::time::Instant::now() + Duration::from_secs(10);
            while !held.load(Ordering::Acquire) {
                assert!(std::time::Instant::now() < deadline);
                std::thread::yield_now();
            }
            cancel.store(true, Ordering::Release);
        }
        result(&job, false)
    };
    assert_eq!(
        controller::run_observed(
            &mut restored,
            &fixture.identity(),
            32,
            3,
            rolling_config(),
            &|| Ok(()),
            &inspect,
            || stop::requested(&cancel, None),
            |_| false,
            |state, _, status| {
                let _release = ReleaseGateOnDrop(&gate);
                assert_eq!(state.state.k, 0);
                assert_eq!(state.records.total(), 0);
                assert_eq!(state.state.in_flight.len(), 32);
                assert_eq!(status.len(), 32);
            },
            None,
            |_, _, _, _| {},
            |_| {},
        )
        .unwrap(),
        Outcome::Stopped(merge::StopReason::Paused)
    );
    assert_eq!(restored.rolling_diagnostics.selected_cuts, 0);
}

#[test]
fn monitoring_runs_once_per_committed_boundary_not_per_save_and_preserves_decisions() {
    for rolling in [false, true] {
        let mut expected = None;
        for monitor in [false, true] {
            let mut fixture = rolling_fixture(20);
            fixture.request.epoch_rolling = rolling;
            let window = if rolling { 32 } else { 16 };
            fixture.save_window(20, 0, false, window);
            let mut restored = fixture.open().unwrap();
            let mut boundaries = Vec::new();
            let mut saved_counts = Vec::new();
            assert_eq!(
                controller::run_observed(
                    &mut restored,
                    &fixture.identity(),
                    window,
                    1,
                    MergeConfig {
                        lockstep: !rolling,
                        ..config()
                    },
                    &|| Ok(()),
                    &|bytes, _| result(&Job::<1>::decode(bytes).unwrap(), false),
                    || None,
                    |k| k == 1,
                    |state, _, _| saved_counts.push(state.state.tracker.counters().refresh_count),
                    None,
                    |_, _, _, _| {},
                    |state| {
                        boundaries.push(state.k);
                        if monitor {
                            state
                                .tracker
                                .refresh_periodic_monitor(&AtomicBool::new(false));
                        }
                    },
                )
                .unwrap(),
                Outcome::Drained
            );
            assert_eq!(
                boundaries,
                [1, 2],
                "never during initial/repeated/saved boundary"
            );
            assert_eq!(saved_counts, if monitor { vec![1, 1] } else { vec![0, 0] });
            assert_eq!(
                restored.state.tracker.counters().total_closed,
                if monitor { 16 } else { 0 }
            );
            let decisions = json!({"domains":restored.state.watermark(),
                "nodes":restored.state.nodes,"ledger":restored.state.ledger.counts().json(),
                "walk":restored.state.counters,"edges":restored.state.edges.edge_digest(),
                "records":restored.state.edges.records_digest(),"roots":restored.roots.rows});
            if let Some(expected) = &expected {
                assert_eq!(&decisions, expected);
            } else {
                expected = Some(decisions);
            }
            drop(restored);
            let restored = fixture.open().unwrap();
            assert_eq!(
                restored.state.tracker.counters().total_closed,
                if monitor { 16 } else { 0 }
            );
            assert_eq!(
                restored.state.tracker.counters().refresh_count,
                u64::from(monitor)
            );
            assert!(restored.state.tracker.snapshot_age_seconds().is_none());
        }
    }
}
