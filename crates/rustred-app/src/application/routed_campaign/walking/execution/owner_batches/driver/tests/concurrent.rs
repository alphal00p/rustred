//! Multiple inspectors share immutable rules; only their owner FIFO head emits.
use super::*;

mod retention;

fn same_owner(workers: usize, horizon: usize, points: usize) -> (OwnerDomainWalkRequest, Walk<2>) {
    let (mut request, _) = setup(workers);
    request.max_events = 8 * parallel::CHUNK_EVENTS;
    request.scheduling_policy =
        super::super::super::super::super::OwnerDomainWalkSchedulingPolicy::TransferUnreserved {
            lookahead: NonZeroUsize::new(horizon).unwrap(),
        };
    let initial = (0..points)
        .map(|n| Arc::new(point(FAST, n as u64)))
        .collect::<Vec<_>>();
    let (walk, _) = initialize(&initial, 0, 0, &request, &AtomicBool::new(false)).unwrap();
    (request, walk)
}

fn frontier(id: u64) -> Event<2> {
    Event::one(Effect::Frontier {
        value: json!({"source_marker":id}),
        successor: false,
        conditional: false,
    })
}

#[test]
fn owner_concurrent_reverse_finished_order_keeps_fifo_and_frontier_provenance() {
    let (request, mut walk) = same_owner(6, 16, 3);
    let gates = Gates::default();
    let observed_finished_ahead = AtomicBool::new(false);
    let snapshot = run_parallel(
        &mut walk,
        &request,
        &AtomicBool::new(false),
        &|event| {
            if event["event"] == "domain_progress"
                && !observed_finished_ahead.load(Ordering::Acquire)
                && event["parallel"]["returned_inspections"]
                    .as_u64()
                    .unwrap_or(0)
                    >= 2
            {
                // The two later inspectors actually returned; neither their
                // chunks nor their Finished may have reached source accounting.
                assert_eq!(event["native_processed_nodes"], 0);
                assert_eq!(event["committed_events"], 0);
                observed_finished_ahead.store(true, Ordering::Release);
                gates.set(|s| s.1 = true);
            }
        },
        |domain, stop, emit| {
            if domain.lower[0] == 0 {
                assert!(gates.wait(stop, |s| s.1));
            }
            assert!(emit(frontier(domain.lower[0])).is_continue());
            finished(1, None)
        },
    );
    assert!(observed_finished_ahead.load(Ordering::Acquire));
    assert!(walk.error.is_none(), "{:?}", walk.error);
    assert_eq!(
        walk.buckets[&(Phase::Apply, FAST)].peak_outstanding_native_jobs,
        3
    );
    assert_eq!(walk.metrics.peak_fifo_held_jobs, 2);
    let report = report::finish(walk, &request, 0.0, snapshot);
    let rows = report["domains"].as_array().unwrap();
    assert_eq!(rows.len(), 3);
    for (id, row) in rows.iter().enumerate() {
        assert_eq!(row["id"], id);
        assert_eq!(row["frontiers"][0]["source_marker"], id);
        assert_eq!(row["frontiers"].as_array().unwrap().len(), 1);
    }
    assert_eq!(report["recursive_worklist_exhausted"], true);
    assert_eq!(report["all_scheduled_domains_resolved"], false); // real frontiers retained
}

#[test]
fn owner_concurrent_later_stream_is_bounded_until_head_publication() {
    let (request, mut walk) = same_owner(3, 16, 2);
    let gates = Gates::default();
    let backpressure_seen = AtomicBool::new(false);
    let snapshot = run_parallel(
        &mut walk,
        &request,
        &AtomicBool::new(false),
        &|event| {
            if event["event"] == "domain_progress"
                && !backpressure_seen.load(Ordering::Acquire)
                && event["parallel"]["backpressured_workers"]
                    .as_u64()
                    .unwrap_or(0)
                    > 0
            {
                assert_eq!(event["committed_events"], 0);
                assert_eq!(event["native_processed_nodes"], 0);
                assert_eq!(
                    event["parallel"]["worker_buffered_events"],
                    2 * parallel::CHUNK_EVENTS
                );
                backpressure_seen.store(true, Ordering::Release);
                gates.set(|s| s.1 = true);
            }
        },
        |domain, stop, emit| {
            if domain.lower[0] == 0 {
                assert!(gates.wait(stop, |s| s.1));
                return finished(0, None);
            }
            for _ in 0..3 {
                assert!(
                    emit(Event {
                        count: parallel::CHUNK_EVENTS,
                        effect: Effect::Count
                    })
                    .is_continue()
                );
            }
            finished(3 * parallel::CHUNK_EVENTS, None)
        },
    );
    assert!(backpressure_seen.load(Ordering::Acquire));
    assert!(walk.error.is_none(), "{:?}", walk.error);
    assert_eq!(walk.budget.events, 3 * parallel::CHUNK_EVENTS);
    assert!(
        snapshot["peak_worker_buffered_logical_bytes"]
            .as_u64()
            .unwrap()
            <= (4 * parallel::CHUNK_BYTES) as u64
    );
    let report = report::finish(walk, &request, 0.0, snapshot);
    assert_eq!(report["all_scheduled_domains_resolved"], true);
    assert_eq!(report["outstanding_native_jobs"], 0);
}

#[test]
fn owner_concurrent_cancellation_retains_finished_later_source_without_discharge() {
    let (request, mut walk) = same_owner(3, 16, 2);
    let cancel = AtomicBool::new(false);
    let gates = Gates::default();
    let snapshot = run_parallel(
        &mut walk,
        &request,
        &cancel,
        &|event| {
            if event["parallel"]["returned_inspections"]
                .as_u64()
                .unwrap_or(0)
                > 0
            {
                assert_eq!(event["native_processed_nodes"], 0);
                cancel.store(true, Ordering::Release);
            }
        },
        |domain, stop, _| {
            if domain.lower[0] == 0 {
                assert!(!gates.wait(stop, |_| false));
                finished(0, Some("head cancelled while later source finished"))
            } else {
                finished(0, None)
            }
        },
    );
    assert_eq!(walk.error.as_deref(), Some("cancelled"));
    let bucket = &walk.buckets[&(Phase::Apply, FAST)];
    assert_eq!(bucket.state.queue.next, 1);
    assert_eq!(bucket.state.native_records, 1);
    assert_eq!(bucket.state.uncommitted.len(), 1);
    assert_eq!(bucket.state.uncommitted[0]["id"], 1);
    assert_eq!(bucket.state.uncommitted[0]["committed"], false);
    assert_eq!(bucket.outstanding_native_jobs, 1);
    let report = report::finish(walk, &request, 0.0, snapshot);
    assert_eq!(report["all_scheduled_domains_resolved"], false);
    assert_eq!(report["completed_nodes"], 0);
    assert_eq!(
        report["delegation"]["all_ledger_obligations_discharged"],
        false
    );
}

#[test]
fn owner_concurrent_later_failure_stops_quiet_head_but_cannot_publish_itself() {
    let (request, mut walk) = same_owner(3, 16, 2);
    let gates = Gates::default();
    let snapshot = run_parallel(
        &mut walk,
        &request,
        &AtomicBool::new(false),
        &|_| {},
        |domain, stop, _| {
            if domain.lower[0] == 0 {
                gates.set(|s| s.0 = true);
                assert!(!gates.wait(stop, |_| false));
                finished(0, Some("quiet head stopped"))
            } else {
                assert!(gates.wait(stop, |s| s.0));
                finished(0, Some("later source failed"))
            }
        },
    );
    assert_eq!(walk.error.as_deref(), Some("later source failed"));
    let bucket = &walk.buckets[&(Phase::Apply, FAST)];
    assert_eq!(bucket.state.queue.next, 1);
    assert_eq!(bucket.state.uncommitted.len(), 1);
    assert_eq!(bucket.state.uncommitted[0]["id"], 1);
    assert_eq!(
        bucket.state.uncommitted[0]["native_error"],
        "later source failed"
    );
    let report = report::finish(walk, &request, 0.0, snapshot);
    assert_eq!(report["completed_nodes"], 0);
    assert_eq!(report["all_scheduled_domains_resolved"], false);
}

#[test]
fn owner_concurrent_missing_head_retains_its_details_and_later_real_statistics() {
    let (request, mut walk) = same_owner(3, 16, 2);
    let gates = Gates::default();
    let snapshot = run_parallel(
        &mut walk,
        &request,
        &AtomicBool::new(false),
        &|event| {
            if event["parallel"]["returned_inspections"]
                .as_u64()
                .unwrap_or(0)
                > 0
                && event["committed_events"].as_u64().unwrap_or(0) > 0
            {
                gates.set(|s| s.1 = true);
            }
        },
        |domain, stop, emit| {
            if domain.lower[0] == 0 {
                assert!(emit(frontier(0)).is_continue());
                assert!(
                    emit(Event {
                        count: parallel::CHUNK_EVENTS,
                        effect: Effect::Count
                    })
                    .is_continue()
                );
                assert!(gates.wait(stop, |s| s.1));
                panic!("injected owner-head panic after a real admitted frontier");
            }
            finished(0, None)
        },
    );
    assert!(walk.error.as_deref().unwrap().contains("panicked"));
    let bucket = &walk.buckets[&(Phase::Apply, FAST)];
    assert_eq!(bucket.state.queue.next, 0);
    assert_eq!(bucket.state.native_records, 0);
    assert_eq!(bucket.state.uncommitted.len(), 2);
    let head = bucket
        .state
        .uncommitted
        .iter()
        .find(|row| row["id"] == 0)
        .unwrap();
    let later = bucket
        .state
        .uncommitted
        .iter()
        .find(|row| row["id"] == 1)
        .unwrap();
    assert_eq!(head["stats"], Value::Null);
    assert_eq!(head["frontiers"][0]["source_marker"], 0);
    assert_eq!(later["frontiers"], json!([]));
    assert!(later["stats"].is_object());
    let report = report::finish(walk, &request, 0.0, snapshot);
    assert_eq!(report["delegation"]["pending_native_publications"], 2);
    assert_eq!(report["all_scheduled_domains_resolved"], false);
}

#[test]
fn owner_concurrent_horizon_one_never_dispatches_past_the_fifo_fence() {
    let (request, mut walk) = same_owner(6, 1, 3);
    let snapshot = run_parallel(
        &mut walk,
        &request,
        &AtomicBool::new(false),
        &|event| {
            if event["event"] == "domain_started" {
                assert_eq!(event["id"], event["native_processed_nodes"]);
            }
        },
        |_, _, _| finished(0, None),
    );
    assert!(walk.error.is_none(), "{:?}", walk.error);
    assert_eq!(
        walk.buckets[&(Phase::Apply, FAST)].peak_outstanding_native_jobs,
        1
    );
    assert_eq!(walk.metrics.peak_fifo_held_jobs, 0);
    assert_eq!(
        report::finish(walk, &request, 0.0, snapshot)["all_scheduled_domains_resolved"],
        true
    );
}

#[test]
fn owner_concurrent_aliases_wait_for_head_and_do_not_receive_native_jobs() {
    let (request, mut walk) = same_owner(6, 2, 4);
    let key = (Phase::Apply, FAST);
    let mut broad = point(FAST, 0);
    broad.upper[0] = None;
    let (id, added) = walk
        .buckets
        .get_mut(&key)
        .unwrap()
        .state
        .queue
        .admit(broad)
        .unwrap();
    assert!(added);
    walk.budget.domains += 1;
    assert_eq!(id, 4);
    let ledger = walk.buckets[&key].state.queue.delegation.as_ref().unwrap();
    assert_eq!(ledger.delegated_to(2), Some(4));
    assert_eq!(ledger.delegated_to(3), Some(4));
    let snapshot = run_parallel(
        &mut walk,
        &request,
        &AtomicBool::new(false),
        &|event| {
            if event["event"] == "domain_started" {
                let id = event["id"].as_u64().unwrap();
                assert!([0, 1, 4].contains(&id));
                if id == 1 {
                    assert_eq!(event["native_processed_nodes"], 0);
                }
                if id == 4 {
                    assert_eq!(event["processed_nodes"], 4);
                    assert_eq!(event["native_processed_nodes"], 2);
                }
            }
        },
        |_, _, _| finished(0, None),
    );
    assert!(walk.error.is_none(), "{:?}", walk.error);
    let report = report::finish(walk, &request, 0.0, snapshot);
    assert_eq!(report["native_processed_nodes"], 3);
    assert_eq!(report["delegation"]["delegated_publications"], 2);
    assert_eq!(
        report["delegation"]["all_ledger_obligations_discharged"],
        true
    );
    assert_eq!(report["all_scheduled_domains_resolved"], true);
}

#[test]
fn owner_concurrent_choose_favors_ready_keys_before_extra_hot_owner_jobs() {
    let (request, mut walk) = setup(6);
    for n in 1..=3 {
        for owner in [SLOW, FAST] {
            let mut domain = point(owner, n);
            if owner == SLOW {
                // Vary an active coordinate. Positive powers on SLOW's
                // inactive axis at rank zero would instead be an empty region.
                domain.lower = vec![0, n];
                domain.upper = vec![Some(0), Some(n)];
            }
            let (_, added) = walk
                .buckets
                .get_mut(&(Phase::Apply, owner))
                .unwrap()
                .state
                .queue
                .admit(domain)
                .unwrap();
            assert!(added, "fairness fixture must add independent native work");
            walk.budget.domains += usize::from(added);
        }
    }
    let jobs = choose(&mut walk, 3, &[], &AtomicBool::new(false)).unwrap();
    assert_eq!(
        jobs.iter().map(|job| job.key.1).collect::<Vec<_>>(),
        [SLOW, FAST, SLOW]
    );
    assert_eq!(
        jobs.iter().map(|job| job.local_id).collect::<Vec<_>>(),
        [0, 0, 1]
    );
    assert_eq!(request.workers, 6); // configured 3 inspectors + 2 helpers + coordinator
}

#[test]
fn owner_concurrent_admitted_successor_inspects_before_parent_finished_but_publishes_after() {
    let (request, mut walk) = same_owner(3, 16, 1);
    let gates = Gates::default();
    let snapshot = run_parallel(
        &mut walk,
        &request,
        &AtomicBool::new(false),
        &|event| {
            if event["event"] == "domain_started" && event["id"] == 1 {
                assert_eq!(event["native_processed_nodes"], 0);
                assert!(event["committed_events"].as_u64().unwrap() > 0);
            }
        },
        |domain, stop, emit| {
            if domain.lower[0] == 0 {
                assert!(
                    emit(Event::one(Effect::Admit {
                        domain: point(FAST, 1),
                        successor: true,
                        conditional: false,
                    }))
                    .is_continue()
                );
                assert!(
                    emit(Event {
                        count: parallel::CHUNK_EVENTS,
                        effect: Effect::Count
                    })
                    .is_continue()
                );
                assert!(gates.wait(stop, |s| s.1));
                finished(parallel::CHUNK_EVENTS + 1, None)
            } else {
                gates.set(|s| s.1 = true);
                finished(0, None)
            }
        },
    );
    assert!(walk.error.is_none(), "{:?}", walk.error);
    assert!(gates.state.lock().unwrap().1);
    assert_eq!(
        walk.buckets[&(Phase::Apply, FAST)].peak_outstanding_native_jobs,
        2
    );
    let report = report::finish(walk, &request, 0.0, snapshot);
    assert_eq!(report["domains"][0]["id"], 0);
    assert_eq!(report["domains"][1]["id"], 1);
    assert_eq!(report["all_scheduled_domains_resolved"], true);
}

#[test]
fn owner_concurrent_global_cap_retains_later_partial_inspection_as_uncommitted() {
    let (mut request, mut walk) = same_owner(3, 16, 2);
    request.max_events = 1;
    let gates = Gates::default();
    let snapshot = run_parallel(
        &mut walk,
        &request,
        &AtomicBool::new(false),
        &|event| {
            if event["parallel"]["returned_inspections"]
                .as_u64()
                .unwrap_or(0)
                > 0
            {
                gates.set(|s| s.1 = true);
            }
        },
        |domain, stop, emit| {
            if domain.lower[0] == 0 {
                assert!(gates.wait(stop, |s| s.1));
                let _ = emit(Event {
                    count: 2,
                    effect: Effect::Count,
                });
                finished(2, None)
            } else {
                let mut value = finished(0, None);
                value.stats = NativeStats::ApplyPartial(Default::default(),
                    crate::application::routed_campaign::walking::initial_overlap::InitialOverlapScope {
                        anchor_id: 0, cut: 7, residual_powers: Default::default(),
                    });
                value
            }
        },
    );
    assert!(walk.error.as_deref().unwrap().contains("event allowance"));
    assert_eq!(walk.budget.events, 1);
    let bucket = &walk.buckets[&(Phase::Apply, FAST)];
    assert_eq!(bucket.state.queue.next, 1);
    assert_eq!(bucket.state.native_records, 1);
    assert_eq!(bucket.state.uncommitted.len(), 1);
    let later = &bucket.state.uncommitted[0];
    assert_eq!(later["id"], 1);
    assert_eq!(later["native_inspection_scope"], "low_D_residual_only");
    assert_eq!(later["initial_overlap"]["anchor_id"], 0);
    assert_eq!(later["initial_overlap"]["cut"], 7);
    assert_eq!(later["initial_overlap"]["responsibility_published"], false);
    let report = report::finish(walk, &request, 0.0, snapshot);
    assert_eq!(report["all_scheduled_domains_resolved"], false);
    assert_eq!(report["completed_nodes"], 0);
    assert_eq!(report["delegation"]["pending_native_publications"], 1);
}
