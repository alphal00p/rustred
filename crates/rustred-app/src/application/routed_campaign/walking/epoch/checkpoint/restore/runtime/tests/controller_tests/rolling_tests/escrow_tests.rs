use super::*;

fn escrow_fixture(count: usize, byte_limit: usize) -> Fixture {
    let mut fixture = rolling_fixture(count);
    fixture.request.epoch_window = Some(16);
    fixture.request.epoch_result_escrow_jobs = 16;
    fixture.request.epoch_result_escrow_bytes = Some(byte_limit);
    fixture
}

#[test]
fn completed_escrow_refills_behind_held_prefix_without_publishing_and_restores_all_reservations() {
    for with_snapshot in [false, true] {
        let fixture = escrow_fixture(48, 1 << 20);
        fixture.save_window(48, 0, false, 32);
        let identity = fixture.identity();
        let mut restored = fixture.open().unwrap();
        let snapshots = Publication::new();
        let gate = (Mutex::new(false), Condvar::new());
        let held = AtomicBool::new(false);
        let cancel = AtomicBool::new(false);
        let inspect = |bytes: &[u8], _: &AtomicBool| {
            let job = Job::<1>::decode(bytes).unwrap();
            let view = with_snapshot.then(|| snapshots.acquire_job(job.seq).unwrap());
            if let Some(view) = &view {
                assert_eq!(view.version, 0);
            }
            if job.parent == 0 {
                held.store(true, Ordering::Release);
                let (open, timeout) = gate
                    .1
                    .wait_timeout_while(gate.0.lock().unwrap(), Duration::from_secs(10), |open| {
                        !*open
                    })
                    .unwrap();
                assert!(!timeout.timed_out() && *open);
            } else if job.parent == 31 {
                assert!(held.load(Ordering::Acquire));
                cancel.store(true, Ordering::Release);
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
                |_| false,
                |state, _, status| {
                    let _release = ReleaseGateOnDrop(&gate);
                    assert_eq!(state.state.k, 0);
                    assert_eq!(state.records.total(), 0);
                    assert_eq!(state.state.in_flight.len(), 32);
                    assert_eq!(status.len(), 32);
                    let mut keys = status.iter().map(|s| s.key).collect::<Vec<_>>();
                    keys.sort_unstable();
                    keys.dedup();
                    assert_eq!(keys.len(), 32);
                },
                with_snapshot.then_some(&snapshots),
                |_, _, _, _| {},
                |_| {},
            )
            .unwrap(),
            Outcome::Stopped(merge::StopReason::Paused)
        );
        assert_eq!(
            restored
                .rolling_diagnostics
                .escrow
                .as_ref()
                .unwrap()
                .extra_jobs_dispatched,
            16
        );
        assert_eq!(restored.rolling_diagnostics.selected_cuts, 0);
        drop(restored);
        let mut restored = fixture.open().unwrap();
        assert_eq!(restored.replay.len(), 32);
        assert_eq!(
            restored.replay.iter().map(|j| j.parent).collect::<Vec<_>>(),
            (0..32).collect::<Vec<_>>()
        );
        assert_eq!(
            controller::run_observed(
                &mut restored,
                &identity,
                32,
                3,
                rolling_config(),
                &|| Ok(()),
                &|bytes, _| result(&Job::<1>::decode(bytes).unwrap(), false),
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
        assert_eq!(restored.records.total(), 48);
        assert!(restored.state.in_flight.is_empty());
    }
}

#[test]
fn byte_admission_limit_blocks_extra_reservations_without_losing_completed_results() {
    let fixture = escrow_fixture(48, 1);
    fixture.save_window(48, 0, false, 32);
    let identity = fixture.identity();
    let mut restored = fixture.open().unwrap();
    let gate = (Mutex::new(false), Condvar::new());
    let cancel = AtomicBool::new(false);
    let tail = AtomicUsize::new(0);
    let inspect = |bytes: &[u8], _: &AtomicBool| {
        let job = Job::<1>::decode(bytes).unwrap();
        if job.parent == 0 {
            let (open, timeout) = gate
                .1
                .wait_timeout_while(gate.0.lock().unwrap(), Duration::from_secs(10), |open| {
                    !*open
                })
                .unwrap();
            assert!(!timeout.timed_out() && *open);
        } else {
            tail.fetch_add(1, Ordering::AcqRel);
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
            |_| false,
            |state, _, status| {
                let _release = ReleaseGateOnDrop(&gate);
                assert_eq!(state.state.in_flight.len(), 16);
                assert_eq!(status.len(), 16);
                assert_eq!(state.records.total(), 0);
            },
            None,
            |state, _, phase, activity| {
                if phase == "inspect"
                    && tail.load(Ordering::Acquire) >= 15
                    && activity().is_some_and(|a| a.queued == 0 && a.computing == 1)
                {
                    assert_eq!(state.in_flight.len(), 16);
                    cancel.store(true, Ordering::Release);
                }
            },
            |_| {},
        )
        .unwrap(),
        Outcome::Stopped(merge::StopReason::Paused)
    );
    let diagnostics = restored.rolling_diagnostics.escrow.as_ref().unwrap();
    assert_eq!(diagnostics.extra_jobs_dispatched, 0);
    assert!(diagnostics.returned_results.peak_bytes > 1);
    drop(restored);
    assert_eq!(fixture.open().unwrap().replay.len(), 16);
}

#[test]
fn positive_escrow_is_inert_for_inline_execution_and_keeps_base_cuts() {
    for automatic in [false, true] {
        let mut fixture = escrow_fixture(20, 1 << 20);
        let total = if automatic {
            fixture.request.workers = 1;
            fixture.request.epoch_window = None;
            17
        } else {
            32
        };
        fixture.save_window(20, 0, false, total);
        let identity = fixture.identity();
        let mut restored = fixture.open().unwrap();
        assert_eq!(
            controller::run_observed(
                &mut restored,
                &identity,
                total,
                1,
                rolling_config(),
                &|| Ok(()),
                &|bytes, _| result(&Job::<1>::decode(bytes).unwrap(), false),
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
        assert_eq!(restored.state.k, if automatic { 20 } else { 2 });
        let diagnostics = restored.rolling_diagnostics.escrow.as_ref().unwrap();
        assert!(!diagnostics.enabled);
        assert_eq!(diagnostics.extra_jobs_dispatched, 0);
    }
}

fn hold_zero_until_extra_job(job: &Job<1>, gate: &(Mutex<bool>, Condvar)) {
    if job.parent == 0 {
        let (open, timeout) = gate
            .1
            .wait_timeout_while(gate.0.lock().unwrap(), Duration::from_secs(10), |open| {
                !*open
            })
            .unwrap();
        assert!(!timeout.timed_out() && *open);
    } else if job.parent == 16 {
        *gate.0.lock().unwrap() = true;
        gate.1.notify_all();
    }
}

#[test]
fn escrow_status_survives_p1_consumption_and_periodic_checkpoints_before_restore() {
    for during_p2 in [true, false] {
        let fixture = escrow_fixture(48, 1 << 20);
        fixture.save_window(48, 0, false, 32);
        let identity = fixture.identity();
        let mut restored = fixture.open().unwrap();
        let gate = (Mutex::new(false), Condvar::new());
        let _release_on_failure = ReleaseGateOnDrop(&gate);
        let cancel = AtomicBool::new(false);
        let periodic_saved = AtomicBool::new(false);
        assert_eq!(
            controller::run_observed(
                &mut restored,
                &identity,
                32,
                3,
                rolling_config(),
                &|| Ok(()),
                &|bytes, _| {
                    let job = Job::<1>::decode(bytes).unwrap();
                    hold_zero_until_extra_job(&job, &gate);
                    result(&job, false)
                },
                || stop::requested(&cancel, None),
                |k| !during_p2 && k == 1 && !periodic_saved.load(Ordering::Acquire),
                |state, _, status| {
                    if status.is_empty() {
                        assert!(!during_p2);
                        assert_eq!(state.state.k, 1);
                        assert!(!state.state.in_flight.is_empty());
                        // Decode this periodic generation itself, before the
                        // following stop replaces LATEST. This never adopts a
                        // live session or dispatches speculative saved work.
                        let manifest = publication::read_manifest(
                            &fixture.directory.0.join(publication::LATEST),
                        )
                        .unwrap();
                        let decoded = assembly::read_manifest::<1>(
                            &fixture.directory.0,
                            &identity,
                            32,
                            manifest,
                        )
                        .unwrap();
                        assert_eq!(decoded.scalars.k, 1);
                        let saved: Vec<_> = decoded
                            .dispatch
                            .in_flight
                            .iter()
                            .map(|(&id, meta)| (id, meta.seq, meta.v0))
                            .collect();
                        let expected: Vec<_> = state
                            .state
                            .in_flight
                            .iter()
                            .map(|(&id, meta)| (id, meta.seq, meta.v0))
                            .collect();
                        assert_eq!(saved, expected);
                        periodic_saved.store(true, Ordering::Release);
                        cancel.store(true, Ordering::Release);
                    } else {
                        assert_eq!(status.len(), state.state.in_flight.len());
                        if during_p2 {
                            assert_eq!(state.records.total(), 0);
                            assert!(status.len() >= 17, "extra reservation really occurred");
                        }
                    }
                },
                None,
                |_, _, phase, _| {
                    if during_p2 && phase == "p2" {
                        cancel.store(true, Ordering::Release);
                    }
                },
                |_| {},
            )
            .unwrap(),
            Outcome::Stopped(merge::StopReason::Paused)
        );
        assert!(
            restored
                .rolling_diagnostics
                .escrow
                .as_ref()
                .unwrap()
                .extra_jobs_dispatched
                > 0
        );
        let saved_inventory = restored.state.in_flight.len();
        drop(restored);
        let mut restored = fixture.open().unwrap();
        assert_eq!(restored.replay.len(), saved_inventory);
        assert_eq!(
            controller::run_observed(
                &mut restored,
                &identity,
                32,
                3,
                rolling_config(),
                &|| Ok(()),
                &|bytes, _| result(&Job::<1>::decode(bytes).unwrap(), false),
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
        assert_eq!(restored.records.total(), 48);
    }
}

#[test]
fn escrowed_stale_identity_or_newly_quarantined_target_is_never_published() {
    use crate::application::routed_campaign::walking::epoch::{
        job::{LookupReport, Miss},
        store::LookupCounters,
        verify::VerifyCounters,
    };
    for quarantine_target in [false, true] {
        let fixture = escrow_fixture(32, 1 << 20);
        fixture.save_window(32, 0, false, 32);
        let identity = fixture.identity();
        let mut restored = fixture.open().unwrap();
        let target = restored.state.store.domains[0];
        let gate = (Mutex::new(false), Condvar::new());
        let _release_on_failure = ReleaseGateOnDrop(&gate);
        let error = controller::run_observed(
            &mut restored,
            &identity,
            32,
            3,
            rolling_config(),
            &|| Ok(()),
            &|bytes, _| {
                let job = Job::<1>::decode(bytes).unwrap();
                hold_zero_until_extra_job(&job, &gate);
                let mut reply = JobResult::<1>::decode(&result(&job, false)).unwrap();
                if job.parent == 16 {
                    if quarantine_target {
                        reply.emitted = 1;
                        reply.accepted = 1;
                        reply.stats_events = 1;
                        reply.successors = 1;
                        reply.stats_json = br#"{"events":1,"successors":1}"#.to_vec();
                        reply.lookup = Some(LookupReport {
                            version: job.v0,
                            published_len: 32,
                            // This was a valid stored positive before the
                            // earlier cut quarantined its target. Preserve
                            // its accounting so P2 reaches revalidation.
                            lookup: LookupCounters {
                                exact_hits: 1,
                                ..Default::default()
                            },
                            verify: VerifyCounters {
                                calls: 1,
                                accepted: 1,
                                raw_inclusions: 1,
                                ..Default::default()
                            },
                            seconds: 0.0,
                        });
                        reply.misses.push(Miss {
                            ordinal: 0,
                            digest: target.digest().0,
                            image: target,
                            target: Some(0),
                        });
                    } else {
                        reply.v0 += 1;
                    }
                }
                reply.encode()
            },
            || None,
            |_| false,
            |_, _, _| {},
            None,
            |_, _, _, _| {},
            |state| {
                if quarantine_target && state.k == 1 {
                    state.store.enable_rescue_duplicates().unwrap();
                    state.store.install_quarantine(vec![1]).unwrap();
                }
            },
        )
        .err()
        .expect("mutated stale proposal must fail closed");
        assert!(
            error.to_string().contains(if quarantine_target {
                "stored target is quarantined"
            } else {
                "stale result"
            }),
            "{error}"
        );
        assert_eq!(restored.records.total(), 16);
        assert!(restored.state.poisoned);
    }
}
