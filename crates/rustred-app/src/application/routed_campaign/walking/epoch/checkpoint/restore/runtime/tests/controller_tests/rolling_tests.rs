use super::*;
use crate::application::routed_campaign::walking::epoch::snapshot::Publication;

fn rolling_config() -> MergeConfig {
    MergeConfig {
        lockstep: false,
        ..config()
    }
}

fn rolling_fixture(count: usize) -> Fixture {
    let mut fixture = Fixture::new();
    fixture.request.epoch_rolling = true;
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
                |_, _, _| {}
            )
            .unwrap(),
            Outcome::Stopped(merge::StopReason::Paused)
        );
        assert_eq!(saves, 2);
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
                |_, _, _| {}
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
    let fixture = rolling_fixture(3);
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
            |_, _, _| {}
        )
        .unwrap(),
        Outcome::Drained
    );
    assert_eq!(*seen.lock().unwrap(), [(0, 0), (1, 0), (2, 1)]);
    assert_eq!(restored.state.k, 2);
}
