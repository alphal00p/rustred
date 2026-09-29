//! Owned scalar telemetry snapshots. Capture observes the same scheduler lock
//! and relaxed atomics as before; serialization never retains that lock.
use super::{CHUNK_BYTES, CHUNK_EVENTS, CHUNK_RECORDS, Failure, Pool, State, Totals};
use serde_json::{Map, Value};
use std::sync::atomic::Ordering;

pub(in super::super) struct LeanSnapshot {
    workers: usize,
    active: usize,
    occupied: usize,
    finished: usize,
    totals: Totals,
    attempted: usize,
    buffered_events: usize,
    buffered_bytes: usize,
    peak_bytes: usize,
    first_failure: Option<Failure>,
    non_cancellation_failure: Option<Failure>,
    escrow: EscrowSnapshot,
}

struct EscrowSnapshot {
    entries: usize,
    events: usize,
    bytes: usize,
    peak_entries: usize,
    peak_bytes: usize,
    reclaimed: usize,
    max_entries: usize,
    max_bytes: usize,
    reserve_failed: bool,
}

impl LeanSnapshot {
    pub(super) fn capture<const N: usize>(pool: &Pool<N>, state: &State<N>) -> Self {
        Self {
            workers: state.slots.len(),
            active: state.slots.iter().filter(|s| s.running).count(),
            occupied: state.slots.iter().filter(|s| s.id.is_some()).count(),
            finished: state.slots.iter().filter(|s| s.finished.is_some()).count(),
            totals: state.totals.clone(),
            attempted: pool.attempted.load(Ordering::Relaxed),
            buffered_events: pool.buffered_events.load(Ordering::Relaxed),
            buffered_bytes: pool.buffered_bytes.load(Ordering::Relaxed),
            peak_bytes: pool.peak_bytes.load(Ordering::Relaxed),
            first_failure: state.failure.clone(),
            non_cancellation_failure: state.non_cancellation_failure.clone(),
            escrow: EscrowSnapshot {
                entries: state.escrow.len(),
                events: state.escrow.events,
                bytes: state.escrow.bytes,
                peak_entries: state.escrow.peak_entries,
                peak_bytes: state.escrow.peak_bytes,
                reclaimed: state.escrow.reclaimed,
                max_entries: state.escrow.limits.entries,
                max_bytes: state.escrow.limits.bytes,
                reserve_failed: state.escrow.reserve_failed,
            },
        }
    }

    pub(in super::super) fn into_json(self) -> Value {
        let mut out = Value::Null;
        self.write_json(&mut out);
        out
    }

    /// Overwrite session counters, not previously accumulated resume totals.
    /// Reuse existing scalar map entries/constant strings while removing every
    /// detailed/coordinator extension. Enrichment then starts from fresh raw
    /// failure IDs, never a previously decoded physical ticket.
    pub(in super::super) fn write_json(self, out: &mut Value) {
        if !out.is_object() {
            *out = Value::Object(Map::new());
        }
        let fields = out.as_object_mut().expect("telemetry object");
        fields.retain(|key, _| pool_lean_key(key));
        macro_rules! scalar {
            ($key:literal, $value:expr) => {
                put(fields, $key, Value::from($value))
            };
        }
        scalar!("workers", self.workers);
        scalar!("active_workers", self.active);
        scalar!("occupied_native_slots", self.occupied);
        scalar!(
            "dispatched_uncommitted_domains",
            self.occupied + self.escrow.entries
        );
        scalar!(
            "finished_uncommitted_domains",
            self.finished + self.escrow.entries
        );
        scalar!("backpressured_workers", self.totals.waiting);
        scalar!("backpressure_seconds", self.totals.wait_seconds);
        scalar!("attempted_events", self.attempted);
        scalar!("returned_inspections", self.totals.returned);
        scalar!("attempted_native_operations", self.totals.native);
        scalar!("attempted_rule_checks", self.totals.rules);
        scalar!("attempted_predicates", self.totals.predicates);
        scalar!(
            "attempted_optional_coefficient_refusals",
            self.totals.optional
        );
        text(
            fields,
            "native_attempt_counters_scope",
            "returned_inspections_including_uncommitted_and_cancelled",
        );
        scalar!("worker_buffered_events", self.buffered_events);
        scalar!("worker_buffered_logical_bytes", self.buffered_bytes);
        scalar!("peak_worker_buffered_logical_bytes", self.peak_bytes);
        scalar!("per_worker_chunk_events", CHUNK_EVENTS);
        scalar!("per_worker_chunk_records", CHUNK_RECORDS);
        scalar!("per_worker_chunk_logical_bytes", CHUNK_BYTES);
        put(
            fields,
            "first_failure",
            self.first_failure
                .as_ref()
                .map_or(Value::Null, Failure::json),
        );
        put(
            fields,
            "non_cancellation_failure",
            self.non_cancellation_failure
                .as_ref()
                .map_or(Value::Null, Failure::json),
        );
        text(
            fields,
            "worker_buffer_accounting_scope",
            "all_pool_owned_chunks_including_completed_escrow; excludes_coordinator_chunk",
        );
        scalar!("completed_escrow_entries", self.escrow.entries);
        scalar!("completed_escrow_events", self.escrow.events);
        scalar!("completed_escrow_accounted_bytes", self.escrow.bytes);
        scalar!("completed_escrow_peak_entries", self.escrow.peak_entries);
        scalar!(
            "completed_escrow_peak_accounted_bytes",
            self.escrow.peak_bytes
        );
        scalar!("completed_slots_reclaimed", self.escrow.reclaimed);
        scalar!("completed_escrow_max_entries", self.escrow.max_entries);
        scalar!(
            "completed_escrow_max_accounted_bytes",
            self.escrow.max_bytes
        );
        scalar!(
            "completed_escrow_reserve_fallback",
            self.escrow.reserve_failed
        );
    }
}

fn put(fields: &mut Map<String, Value>, key: &'static str, value: Value) {
    if let Some(previous) = fields.get_mut(key) {
        *previous = value;
    } else {
        fields.insert(key.to_owned(), value);
    }
}

fn text(fields: &mut Map<String, Value>, key: &'static str, value: &'static str) {
    if let Some(Value::String(previous)) = fields.get_mut(key) {
        if previous != value {
            previous.clear();
            previous.push_str(value);
        }
    } else {
        put(fields, key, Value::String(value.to_owned()));
    }
}

fn pool_lean_key(key: &str) -> bool {
    matches!(
        key,
        "workers"
            | "active_workers"
            | "occupied_native_slots"
            | "dispatched_uncommitted_domains"
            | "finished_uncommitted_domains"
            | "backpressured_workers"
            | "backpressure_seconds"
            | "attempted_events"
            | "returned_inspections"
            | "attempted_native_operations"
            | "attempted_rule_checks"
            | "attempted_predicates"
            | "attempted_optional_coefficient_refusals"
            | "native_attempt_counters_scope"
            | "worker_buffered_events"
            | "worker_buffered_logical_bytes"
            | "peak_worker_buffered_logical_bytes"
            | "per_worker_chunk_events"
            | "per_worker_chunk_records"
            | "per_worker_chunk_logical_bytes"
            | "first_failure"
            | "non_cancellation_failure"
            | "worker_buffer_accounting_scope"
            | "completed_escrow_entries"
            | "completed_escrow_events"
            | "completed_escrow_accounted_bytes"
            | "completed_escrow_peak_entries"
            | "completed_escrow_peak_accounted_bytes"
            | "completed_slots_reclaimed"
            | "completed_escrow_max_entries"
            | "completed_escrow_max_accounted_bytes"
            | "completed_escrow_reserve_fallback"
    )
}
