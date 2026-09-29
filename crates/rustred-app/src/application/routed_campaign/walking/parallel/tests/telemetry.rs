use super::*;

// Independent expression of the pre-capture snapshot contract, deliberately
// not implemented through LeanSnapshot or its writer.
fn reference(pool: &Pool<1>) -> Value {
    let state = pool.lock();
    json!({
        "workers":state.slots.len(),
        "active_workers":state.slots.iter().filter(|s| s.running).count(),
        "occupied_native_slots":state.slots.iter().filter(|s| s.id.is_some()).count(),
        "dispatched_uncommitted_domains":state.slots.iter().filter(|s| s.id.is_some()).count() + state.escrow.len(),
        "finished_uncommitted_domains":state.slots.iter().filter(|s| s.finished.is_some()).count() + state.escrow.len(),
        "backpressured_workers":state.totals.waiting,
        "backpressure_seconds":state.totals.wait_seconds,
        "attempted_events":pool.attempted.load(Ordering::Relaxed),
        "returned_inspections":state.totals.returned,
        "attempted_native_operations":state.totals.native,
        "attempted_rule_checks":state.totals.rules,
        "attempted_predicates":state.totals.predicates,
        "attempted_optional_coefficient_refusals":state.totals.optional,
        "native_attempt_counters_scope":"returned_inspections_including_uncommitted_and_cancelled",
        "worker_buffered_events":pool.buffered_events.load(Ordering::Relaxed),
        "worker_buffered_logical_bytes":pool.buffered_bytes.load(Ordering::Relaxed),
        "peak_worker_buffered_logical_bytes":pool.peak_bytes.load(Ordering::Relaxed),
        "per_worker_chunk_events":CHUNK_EVENTS,
        "per_worker_chunk_records":CHUNK_RECORDS,
        "per_worker_chunk_logical_bytes":CHUNK_BYTES,
        "first_failure":state.failure.as_ref().map(Failure::json),
        "non_cancellation_failure":state.non_cancellation_failure.as_ref().map(Failure::json),
        "worker_buffer_accounting_scope":"all_pool_owned_chunks_including_completed_escrow; excludes_coordinator_chunk",
        "completed_escrow_entries":state.escrow.len(),
        "completed_escrow_events":state.escrow.events,
        "completed_escrow_accounted_bytes":state.escrow.bytes,
        "completed_escrow_peak_entries":state.escrow.peak_entries,
        "completed_escrow_peak_accounted_bytes":state.escrow.peak_bytes,
        "completed_slots_reclaimed":state.escrow.reclaimed,
        "completed_escrow_max_entries":state.escrow.limits.entries,
        "completed_escrow_max_accounted_bytes":state.escrow.limits.bytes,
        "completed_escrow_reserve_fallback":state.escrow.reserve_failed
    })
}

fn populated() -> Pool<1> {
    let pool = Pool::new(3);
    assert!(pool.dispatch(7, domain(0)));
    assert_eq!(pool.take(0).unwrap().0, 7);
    pool.finish(0, finished(None));
    pool.reclaim_finished(0);
    assert!(pool.dispatch(11, domain(1)));
    assert_eq!(pool.take(0).unwrap().0, 11);
    assert!(pool.dispatch(13, domain(2)));
    assert_eq!(pool.take(1).unwrap().0, 13);
    pool.finish(1, finished(None));
    {
        let mut state = pool.lock();
        state.slots[0].blocked = true;
        state.totals = Totals {
            returned: 17,
            native: 23,
            rules: 29,
            predicates: 31,
            optional: 37,
            wait_seconds: 0.375,
            waiting: 1,
        };
        state.escrow.events = 41;
        state.escrow.reserve_failed = true;
        state.failure = Some(Failure {
            id: Some(11),
            phase: Some(Phase::Apply),
            kind: "cancelled",
            detail: "first".into(),
        });
        state.non_cancellation_failure = Some(Failure {
            id: Some(13),
            phase: Some(Phase::Route),
            kind: "native_failure",
            detail: "later".into(),
        });
    }
    pool.attempted.store(43, Ordering::Relaxed);
    pool.buffered_events.store(47, Ordering::Relaxed);
    pool.buffered_bytes.store(53, Ordering::Relaxed);
    pool.peak_bytes.store(59, Ordering::Relaxed);
    pool
}

#[test]
fn owned_lean_capture_matches_historical_scalars_without_holding_the_lock() {
    let pool = populated();
    let expected = reference(&pool);
    let snapshot = pool.capture_lean();
    {
        // Capturing owns its rare failure strings too: mutating the live pool
        // before serialization must neither deadlock nor change the snapshot.
        let mut state = pool
            .state
            .try_lock()
            .expect("capture retained scheduler lock");
        state.totals.native += 1;
        state.failure.as_mut().unwrap().detail.push_str(" changed");
        state.non_cancellation_failure = None;
    }
    pool.attempted.fetch_add(1, Ordering::Relaxed);
    assert_eq!(snapshot.into_json(), expected);
    assert_eq!(pool.snapshot_lean(), reference(&pool));
}

#[test]
fn reused_lean_map_resets_extended_fields_and_reuses_key_and_constant_storage() {
    let pool = populated();
    let mut out = pool.snapshot();
    out["admission_preparation"] = json!({"stale":true});
    out["inspection_count_scope"] = json!("old physical mode");
    out["first_failure"]["domain"] = json!(999);
    out["first_failure"]["physical_part"] = json!(1);
    out["first_failure"]["owner"] = json!("stale");
    let key_pointer = out
        .as_object()
        .unwrap()
        .keys()
        .find(|k| *k == "workers")
        .unwrap()
        .as_ptr();
    let text_pointer = out["worker_buffer_accounting_scope"]
        .as_str()
        .unwrap()
        .as_ptr();
    pool.capture_lean().write_json(&mut out);
    assert_eq!(out, reference(&pool));
    assert_eq!(
        out.as_object()
            .unwrap()
            .keys()
            .find(|k| *k == "workers")
            .unwrap()
            .as_ptr(),
        key_pointer
    );
    assert_eq!(
        out["worker_buffer_accounting_scope"]
            .as_str()
            .unwrap()
            .as_ptr(),
        text_pointer
    );
    {
        let mut state = pool.lock();
        state.failure = None;
        state.non_cancellation_failure = None;
        state.totals = Totals::default();
    }
    pool.capture_lean().write_json(&mut out);
    assert_eq!(out, reference(&pool));
    assert!(out["first_failure"].is_null());
    assert!(out["non_cancellation_failure"].is_null());
}

#[test]
fn lean_writer_accepts_nonobjects_and_repairs_stale_constant_types() {
    let pool = Pool::<1>::new(1);
    for mut out in [
        Value::Null,
        json!([]),
        json!(false),
        json!({
            "worker_buffer_accounting_scope":42,
            "native_attempt_counters_scope":"resumed scope"
        }),
    ] {
        pool.capture_lean().write_json(&mut out);
        assert_eq!(out, reference(&pool));
    }
}
