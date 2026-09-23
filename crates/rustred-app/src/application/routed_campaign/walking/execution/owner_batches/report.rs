//! Composite owner-local identities and global, non-sampled completion status.
use super::*;

fn aggregate<const N: usize>(walk: &Walk<N>) -> Value {
    let mut saturated = false;
    let mut sum = |field: fn(&State<N>) -> usize| {
        walk.buckets.values().fold(0usize, |total, bucket| {
            total.checked_add(field(&bucket.state)).unwrap_or_else(|| {
                saturated = true;
                usize::MAX
            })
        })
    };
    let mut value = json!({
        "scheduled_nodes":sum(|s|s.queue.domains.len()),
        "processed_nodes":sum(|s|s.queue.next),
        "completed_nodes":sum(|s|s.completed),
        "native_processed_nodes":sum(|s|s.native_records),
        "queued_nodes":sum(|s|s.queue.domains.len()-s.queue.next),
        "failed_nodes":sum(|s|s.native_records-s.completed),
        "deduplication_hits":sum(|s|s.queue.deduplicated),
        "exact_domain_hits":sum(|s|s.queue.exact_hits),
        "full_orthant_hits":sum(|s|s.queue.orthant_hits),
        "containment_checks":walk.budget.checks,
        "initial_prepass_containment_checks":walk.initial_prepass_checks,
        "containment_maintenance_checks":sum(|s|s.queue.containment_maintenance_checks),
        "containment_retired_candidates":sum(|s|s.queue.containment_retired_candidates),
        "containment_summary_builds":sum(|s|s.queue.containment_summary_builds),
        "containment_semantic_hits":sum(|s|s.queue.containment_semantic_hits),
        "containment_semantic_retirements":sum(|s|s.queue.containment_semantic_retirements),
        "containment_candidates":sum(|s|s.queue.containment_candidate_count()),
        "job_local_reuse_hits":sum(|s|s.job_local_reuse_hits),
        "pre_admitted_orthant_hits":sum(|s|s.pre_admitted_orthant_hits),
        "routed_domains":sum(|s|s.routed),"route_masks":sum(|s|s.route_masks),
        "successors":sum(|s|s.successors),"conditional_successors":sum(|s|s.conditional),
        "optional_coefficient_refusals":sum(|s|s.optional.total),
        "optional_original_refusals":sum(|s|s.optional.original),
        "optional_coalesced_refusals":sum(|s|s.optional.coalesced),
        "unbounded_rank_domains":sum(|s|s.queue.unbounded_rank_domains),
        "partial_initial_inspections":sum(|s|s.queue.delegation.as_ref().map_or(0,|l|l.partial_initial_inspections())),
        "events":walk.budget.events,"committed_events":walk.budget.events,
        "frontiers":walk.budget.frontiers,
        "max_scheduled_finite_rank":walk.buckets.values().filter_map(|b|b.state.queue.max_finite_rank).max(),
        "owner_bucket_count":walk.buckets.len(),
    });
    if walk
        .buckets
        .values()
        .any(|b| b.state.queue.delegation.is_some())
    {
        value["delegation"] = json!({
            "transferred_obligations":sum(|s|s.queue.delegation.as_ref().map_or(0,|l|l.transfer_count())),
            "delegated_publications":sum(|s|s.queue.delegation.as_ref().map_or(0,|l|l.delegated_publications())),
            "native_publications":sum(|s|s.queue.delegation.as_ref().map_or(0,|l|l.native_publications())),
            "pending_native_publications":sum(|s|s.queue.delegation.as_ref().map_or(0,|l|l.len()-l.transfer_count()-l.native_publications())),
            "resolution_scope":"owner-local ledgers; pending is not successful discharge"
        });
    }
    value["nonempty_owner_buckets"] = json!(
        walk.buckets
            .values()
            .filter(|b| b.state.queue.next < b.state.queue.domains.len())
            .count()
    );
    value["committed_domains"] = value["processed_nodes"].clone();
    value["diagnostic_counter_saturated"] = json!(saturated);
    value
}

fn metrics<const N: usize>(walk: &Walk<N>) -> Value {
    json!({"rounds":walk.metrics.rounds,"chunk_rounds":walk.metrics.chunk_rounds,
        "parallel_admission_batches":walk.metrics.parallel_admission_batches,
        "serial_budget_batches":walk.metrics.serial_budget_batches,
        "idle_stream_wait_seconds":walk.metrics.idle_stream_wait_seconds,
        "wait_seconds_scope":"coordinator waiting with no active ticket ready; not summed worker idle time",
        "delivery_wall_seconds":walk.metrics.delivery_seconds,
        "peak_coordinator_logical_bytes":walk.metrics.peak_coordinator_logical_bytes,
        "delivered_cross_owner_requests":walk.metrics.delivered_cross_owner_requests,
        "native_tickets":walk.metrics.native_tickets,
        "one_active_native_publisher_per_bucket":true,
        "coordination_policy":"ready producers only; at most one bounded chunk per active producer per pass; synchronous destination admission",
        "max_coordinator_chunks_per_pass":"inspection_worker_limit",
        "logical_byte_scope":"coordinator frames only; native pool buffers, allocator overhead, queue/index, rules and native scratch are separate"})
}

pub(super) fn progress<const N: usize>(
    walk: &Walk<N>,
    request: &OwnerDomainWalkRequest,
    mut parallel: Value,
    active: Option<Key<N>>,
) -> Value {
    let mut value = aggregate(walk);
    value["event"] = json!("domain_progress");
    value["operation"] = json!("owner_domain_walk");
    value["publication_policy"] = json!("owner_batched");
    value["owner"] = active.map_or(Value::Null, |key| json!(mask(&key.1)));
    value["phase"] = active.map_or(Value::Null, |key| json!(format!("{:?}", key.0)));
    value["workers"] = json!(request.workers);
    parallel["owner_batches"] = metrics(walk);
    value["parallel"] = parallel;
    value["error"] = json!(walk.error);
    value["family_closure_claim"] = json!(false);
    value
}

pub(super) fn finish<const N: usize>(
    mut walk: Walk<N>,
    request: &OwnerDomainWalkRequest,
    seconds: f64,
    mut parallel: Value,
) -> Value {
    let mut all_ledgers = true;
    let mut owner_reports = Vec::new();
    let mut records = Vec::new();
    let mut uncommitted = Vec::new();
    for (key, bucket) in &mut walk.buckets {
        let delegation = bucket.state.finalize_delegation();
        all_ledgers &= delegation
            .as_ref()
            .is_none_or(|v| v["all_ledger_obligations_discharged"] == true);
        if let Some(error) = &bucket.state.error {
            walk.error.get_or_insert_with(|| error.clone());
        }
        let name = key_name(key);
        owner_reports.push(json!({"bucket":name,"phase":format!("{:?}",key.0),"owner":mask(&key.1),
            "scheduled_nodes":bucket.state.queue.domains.len(),"processed_nodes":bucket.state.queue.next,
            "native_processed_nodes":bucket.state.native_records,"completed_nodes":bucket.state.completed,
            "queued_nodes":bucket.state.queue.domains.len()-bucket.state.queue.next,
            "incoming_requests":bucket.incoming_requests,"cross_owner_requests":bucket.cross_owner_requests,
            "admission_wall_seconds":bucket.admission_seconds,"native_visitor_wall_seconds":bucket.native_seconds,
            "native_wall_includes_stream_backpressure":true,"frontiers":bucket.state.frontiers,
            "delegation":delegation,"error":bucket.state.error}));
        for mut row in std::mem::take(&mut bucket.state.records) {
            row["bucket"] = json!(name);
            records.push(row);
        }
        for mut row in std::mem::take(&mut bucket.state.uncommitted) {
            row["bucket"] = json!(name);
            uncommitted.push(row);
        }
    }
    let mut document = aggregate(&walk);
    if document["diagnostic_counter_saturated"] == true {
        walk.error
            .get_or_insert_with(|| "aggregate diagnostic counter overflow".into());
    }
    let exhausted = walk.error.is_none()
        && walk
            .buckets
            .values()
            .all(|b| b.state.queue.next == b.state.queue.domains.len())
        && uncommitted.is_empty();
    let resolved = exhausted && walk.budget.frontiers == 0 && all_ledgers;
    parallel["owner_batches"] = metrics(&walk);
    document["schema"] = json!("rustred.owner-domain-walk.json.v4");
    document["status"] = json!(if resolved {
        "locally_resolved"
    } else {
        "incomplete"
    });
    document["all_scheduled_domains_resolved"] = json!(resolved);
    document["recursive_worklist_exhausted"] = json!(exhausted);
    document["delegation"]["all_ledger_obligations_discharged"] = json!(all_ledgers);
    document["delegation"]["scope"] =
        json!("owner-local ledgers; global errors/frontiers/delivery checked separately");
    document["scheduling_policy"] = scheduling_policy_json(request.scheduling_policy);
    document["publication_policy"] = json!("owner_batched");
    document["owner_batched_traversal_started"] = json!(true);
    document["identity_policy"] =
        json!("composite (bucket, local id); initial_handles map supplied initial IDs");
    document["determinism_scope"] = json!(
        "readiness-driven diagnostic IDs, counts, cover shapes and capped prefixes may differ even at fixed worker budget; immutable rule artifacts and exact concrete reductions are unchanged"
    );
    document["successful_publication_matches_serial"] = json!(false);
    document["failure_or_cancellation_prefix_may_differ"] = json!(true);
    document["workers"] = json!(request.workers);
    document["parallel"] = parallel;
    document["owner_buckets"] = json!(owner_reports);
    document["domains"] = json!(records);
    document["uncommitted_inspections"] = json!(uncommitted);
    document["error"] = json!(walk.error);
    // The common caller adds matched post-load timing, including initial
    // admission, owner partitioning, final ledger/report work and queue cleanup.
    document["native_driver_seconds"] = json!(seconds);
    document["reuse_initial_d_bands"] = json!(request.reuse_initial_d_bands);
    document["route_domain_overcover"] = json!(request.route_domain_overcover);
    document["max_events"] = json!(request.max_events);
    document["max_frontiers"] = json!(request.max_frontiers);
    document["max_domains"] = json!(request.max_domains);
    document["max_containment_checks"] = json!(request.max_containment_checks);
    document["applied_limits"] = super::super::super::limits_json(request);
    document["bounded_refinement_axes"] =
        json!(super::super::super::matching::refinement_axes_name(
            request.matching.match_limits.refinement_axes
        ));
    document["family_closure_claim"] = json!(false);
    document["ibp_generation"] = json!(false);
    document["routing_expanded"] = json!(false);
    document["independent_certification"] = json!(false);
    document["resume_supported"] = json!(false);
    document["conditional_successors_use_conservative_domain_overcover"] = json!(true);
    document
}
