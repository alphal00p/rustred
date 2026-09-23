//! Retained results free inspector slots without bypassing owner publication.
use super::*;

#[test]
fn owner_retention_rotates_more_owner_heads_than_physical_slots() {
    let mut jobs = jobs::Jobs::<2>::new();
    let owners = [[false, false], [false, true], [true, false], [true, true]];
    let heads = owners
        .iter()
        .enumerate()
        .map(|(ticket, &owner)| jobs::Head {
            key: (Phase::Apply, owner),
            local_id: 0,
            ticket,
        })
        .collect::<Vec<_>>();
    let mut seen = Vec::new();
    for _ in 0..4 {
        let batch = jobs.poll_batch(heads.clone(), 1);
        assert_eq!(batch.len(), 1); // only one coordinator chunk allowed
        seen.push(batch[0].ticket);
    }
    assert_eq!(seen, [0, 1, 2, 3]);
    assert_eq!(jobs.poll_batch(heads, 1)[0].ticket, 0);
}

#[test]
fn owner_retention_normalizes_alias_gap_without_a_free_physical_slot() {
    let (_, mut walk) = same_owner(6, 2, 4);
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
    assert_eq!(id, 4);
    walk.budget.domains += 1;
    let selected = choose(&mut walk, 2, &[], &AtomicBool::new(false)).unwrap();
    for job in selected {
        walk.buckets
            .get_mut(&key)
            .unwrap()
            .state
            .note_native_started(job.local_id)
            .unwrap();
        publish(&mut walk, key, job.local_id, finished(0, None));
    }
    assert_eq!(walk.buckets[&key].state.queue.next, 2);
    // IDs2/3 are aliases. Publication must normalize them even if other owners
    // occupy all physical slots after this owner's retained head is published.
    assert!(
        choose(&mut walk, 0, &[], &AtomicBool::new(false))
            .unwrap()
            .is_empty()
    );
    assert_eq!(walk.buckets[&key].state.queue.next, 4);
    assert_eq!(walk.buckets[&key].dispatch_cursor, 4);
    assert_eq!(walk.metrics.native_tickets, 2);
}

#[test]
fn owner_retention_refills_many_jobs_behind_one_silent_head() {
    let (request, mut walk) = same_owner(3, 16, 9);
    let gates = Gates::default();
    let seen_refill = AtomicBool::new(false);
    let running = std::sync::atomic::AtomicUsize::new(0);
    let peak_running = std::sync::atomic::AtomicUsize::new(0);
    let snapshot = run_parallel(
        &mut walk,
        &request,
        &AtomicBool::new(false),
        &|event| {
            if event["event"] == "domain_started" && event["id"] == 8 {
                assert_eq!(event["native_processed_nodes"], 0);
                assert_eq!(event["committed_events"], 0);
                assert_eq!(event["parallel"]["completed_escrow_entries"], 7);
                seen_refill.store(true, Ordering::Release);
                gates.set(|s| s.1 = true);
            }
        },
        |domain, stop, emit| {
            let active = running.fetch_add(1, Ordering::AcqRel) + 1;
            peak_running.fetch_max(active, Ordering::AcqRel);
            if domain.lower[0] == 0 {
                assert!(gates.wait(stop, |s| s.1));
            }
            assert!(emit(frontier(domain.lower[0])).is_continue());
            running.fetch_sub(1, Ordering::AcqRel);
            finished(1, None)
        },
    );
    assert!(seen_refill.load(Ordering::Acquire));
    assert!(walk.error.is_none(), "{:?}", walk.error);
    assert!(peak_running.load(Ordering::Acquire) <= 2);
    assert_eq!(
        walk.buckets[&(Phase::Apply, FAST)].peak_outstanding_native_jobs,
        9
    );
    assert!(snapshot["completed_slots_reclaimed"].as_u64().unwrap() >= 7);
    assert_eq!(snapshot["completed_escrow_entries"], 0);
    assert_eq!(snapshot["worker_buffered_events"], 0);
    assert_eq!(snapshot["attempted_events"], 9);
    assert_eq!(snapshot["returned_inspections"], 9);
    let report = report::finish(walk, &request, 0.0, snapshot);
    for (id, row) in report["domains"].as_array().unwrap().iter().enumerate() {
        assert_eq!(row["id"], id);
        assert_eq!(row["frontiers"][0]["source_marker"], id);
        assert_eq!(row["frontiers"].as_array().unwrap().len(), 1);
    }
    assert_eq!(report["recursive_worklist_exhausted"], true);
}

#[test]
fn owner_retention_cancellation_preserves_all_retained_partial_responsibilities() {
    let (request, mut walk) = same_owner(3, 16, 9);
    let cancellation = AtomicBool::new(false);
    let gates = Gates::default();
    let snapshot = run_parallel(
        &mut walk,
        &request,
        &cancellation,
        &|event| {
            if event["event"] == "domain_started" && event["id"] == 8 {
                assert_eq!(event["parallel"]["completed_escrow_entries"], 7);
                cancellation.store(true, Ordering::Release);
            }
        },
        |domain, stop, _| {
            if domain.lower[0] == 0 {
                assert!(!gates.wait(stop, |_| false));
                return finished(0, Some("head cancelled"));
            }
            let mut value = finished(0, None);
            value.stats = NativeStats::ApplyPartial(Default::default(),
                crate::application::routed_campaign::walking::initial_overlap::InitialOverlapScope {
                    anchor_id: 0, cut: 7, residual_powers: Default::default(),
                });
            value
        },
    );
    assert!(walk.error.as_deref().unwrap().contains("cancelled"));
    assert_eq!(snapshot["completed_slots_reclaimed"], 7);
    assert_eq!(snapshot["returned_inspections"], 8); // ID8 was never dispatched.
    let bucket = &walk.buckets[&(Phase::Apply, FAST)];
    assert_eq!(bucket.state.queue.next, 1);
    assert_eq!(bucket.state.native_records, 1);
    assert_eq!(bucket.state.uncommitted.len(), 8);
    let rows = bucket
        .state
        .uncommitted
        .iter()
        .map(|row| (row["id"].as_u64().unwrap(), row))
        .collect::<BTreeMap<_, _>>();
    for id in 1..=7 {
        assert_eq!(rows[&id]["native_inspection_scope"], "low_D_residual_only");
        assert_eq!(rows[&id]["initial_overlap"]["anchor_id"], 0);
        assert_eq!(rows[&id]["initial_overlap"]["cut"], 7);
        assert_eq!(
            rows[&id]["initial_overlap"]["responsibility_published"],
            false
        );
    }
    assert_eq!(rows[&8]["partial_native_statistics_unavailable"], true);
    let report = report::finish(walk, &request, 0.0, snapshot);
    assert_eq!(report["all_scheduled_domains_resolved"], false);
    assert_eq!(report["completed_nodes"], 0);
    assert_eq!(report["delegation"]["pending_native_publications"], 8);
}
