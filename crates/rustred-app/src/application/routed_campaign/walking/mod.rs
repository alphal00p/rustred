//! Shared symbolic successor discovery over one immutable owner snapshot.
//! Stable streamed publication is not a family-closure certificate.
mod checkpoint;
mod delegation;
mod descendant_closure;
mod diagnostics;
mod execution;
mod index_report;
mod initial_orthants;
mod initial_overlap;
mod inspection;
mod parallel;
mod physical_parts;
mod publication;
mod queue;
mod reuse;
mod routing;
mod work_policy;
mod worker_budget;

use super::{OwnerDomainMatchRequest, RoutedCampaignRequest, input, matching, prepare};
use crate::AppError;
use matching::input::power_bounds_json;
use queue::{Domain, Phase, Queue};
use rustred::solver::{OwnerAppliedLimits, OwnerAppliedStats};
use serde_json::{Value, json};
use std::sync::atomic::AtomicBool;
use std::time::Instant;

pub use checkpoint::OwnerDomainWalkCheckpointOptions;
pub use delegation::SchedulingPolicy as OwnerDomainWalkSchedulingPolicy;
pub use physical_parts::ApplySubdivision as OwnerDomainWalkApplySubdivision;
pub use publication::OwnerDomainWalkPublicationPolicy;

#[derive(Clone, Debug)]
pub struct OwnerDomainWalkRequest {
    pub checkpoint: Option<OwnerDomainWalkCheckpointOptions>,
    pub apply_subdivision: Option<OwnerDomainWalkApplySubdivision>,
    /// Load policy and per-domain matcher allowances; max_total_pieces applies
    /// only to local-match reports, not this streaming worklist.
    pub matching: OwnerDomainMatchRequest,
    /// The nested matching member is overridden by matching.match_limits.
    pub applied_limits: OwnerAppliedLimits,
    /// One runs inline. More share immutable programs and bounded event slots.
    /// Caller configures affinity and native inner pools; no environment edits.
    pub workers: usize,
    /// Optional explicit compute partition, not an independent pool size cap.
    /// At W>1 reserves N inspectors, W-1-N admission helpers and one coordinator.
    /// W=1 accepts only N=1 inline. Finite containment caps require N=W-1 at W>1.
    /// None preserves the publication policy's existing automatic split.
    pub inspection_workers: Option<usize>,
    /// Ordered is the stable global stream. OwnerBatched uses independent
    /// phase/owner queues; diagnostic identities and capped prefixes may differ.
    pub publication_policy: OwnerDomainWalkPublicationPolicy,
    /// Optional responsibility transfer under exact containment. The fixed
    /// logical lookahead is independent of physical worker count.
    pub scheduling_policy: OwnerDomainWalkSchedulingPolicy,
    /// Opt-in exact high-D overlap reuse against pinned initial Apply domains.
    /// Requires TransferUnreserved; native work covers the remaining low band.
    pub reuse_initial_d_bands: bool,
    pub max_domains: usize,
    /// Committed logical callbacks, not speculative native attempts or bytes.
    pub max_events: usize,
    /// Aggregate retained input/Apply/Route obligations, independent of events.
    pub max_frontiers: usize,
    /// None leaves aggregate general comparisons unlimited. A positive finite
    /// cap is an opt-in diagnostic budget, not a restriction on actual rank.
    pub max_containment_checks: Option<usize>,
    pub route_domain_overcover: bool,
    /// Opt-in necessary degree bound over the union of source numerator rows.
    pub route_joint_source_support_pruning: bool,
    pub max_route_masks: usize,
}
impl OwnerDomainWalkRequest {
    pub fn new(matching: OwnerDomainMatchRequest) -> Self {
        Self {
            checkpoint: None,
            apply_subdivision: None,
            matching,
            applied_limits: Default::default(),
            workers: 1,
            inspection_workers: None,
            publication_policy: OwnerDomainWalkPublicationPolicy::Ordered,
            scheduling_policy: OwnerDomainWalkSchedulingPolicy::InspectAll,
            reuse_initial_d_bands: false,
            max_domains: 100_000,
            max_events: 1_000_000,
            max_frontiers: 100_000,
            max_containment_checks: None,
            route_domain_overcover: false,
            route_joint_source_support_pruning: false,
            max_route_masks: 100_000,
        }
    }

    pub(crate) fn validate_inspection_workers(
        workers: usize,
        inspection_workers: Option<usize>,
        max_containment_checks: Option<usize>,
    ) -> Result<(), &'static str> {
        worker_budget::validate(workers, inspection_workers, max_containment_checks)
    }
}

#[derive(Clone, Debug)]
pub struct OwnerDomainWalkResult {
    /// Every admitted obligation discharged by inspection or an explicitly
    /// tracked containing representative, without unresolved work. Does not
    /// certify provenance, global route order compatibility, or family closure.
    pub all_scheduled_domains_resolved: bool,
    pub document: Value,
}
impl OwnerDomainWalkResult {
    pub(crate) fn completion_progress(document: &Value) -> Value {
        let mut out = json!({"event":"finished", "operation":"owner_domain_walk",
            "full_result_in_output_document":true, "family_closure_claim":false});
        if document["status"] == "paused" {
            out["full_result_in_output_document"] = json!(false);
            out["full_state_in_checkpoint"] = json!(true);
        }
        for key in [
            "status",
            "workers",
            "parallel",
            "scheduled_nodes",
            "completed_nodes",
            "queued_nodes",
            "failed_nodes",
            "processed_nodes",
            "deduplication_hits",
            "exact_domain_hits",
            "full_orthant_hits",
            "job_local_reuse_hits",
            "pre_admitted_orthant_hits",
            "containment_checks",
            "initial_prepass_containment_checks",
            "containment_maintenance_checks",
            "containment_retired_candidates",
            "containment_summary_builds",
            "containment_semantic_hits",
            "containment_semantic_retirements",
            "containment_candidates",
            "containment_index_policy",
            "max_containment_checks",
            "containment_check_policy",
            "bounded_refinement_axes",
            "max_bounded_refinement_cells",
            "routed_domains",
            "route_masks",
            "route_joint_support_masks_pruned",
            "route_domain_overcover",
            "route_joint_source_support_pruning",
            "max_scheduled_finite_rank",
            "unbounded_rank_domains",
            "successors",
            "conditional_successors",
            "optional_coefficient_refusals",
            "optional_original_refusals",
            "optional_coalesced_refusals",
            "frontiers",
            "events",
            "committed_events",
            "committed_domains",
            "contiguous_publication_watermark",
            "prepared_seconds",
            "traversal_seconds",
            "native_driver_seconds",
            "traversal_timing_boundary",
            "elapsed_seconds",
            "all_scheduled_domains_resolved",
            "recursive_worklist_exhausted",
            "resume_supported",
        ] {
            out[key] = document[key].clone();
        }
        for key in [
            "publication_policy",
            "requested_publication_policy",
            "worker_allocation",
            "requested_inspection_workers",
            "owner_batched_traversal_started",
            "owner_bucket_count",
            "nonempty_owner_buckets",
            "scheduling_policy",
            "delegation",
            "native_processed_nodes",
            "reuse_initial_d_bands",
            "partial_initial_inspections",
            "initial_overlap_index",
            "requested_max_queries",
            "requested_max_query_bytes",
            "checkpoint",
            "initial_entry_domains_total",
            "initial_entry_domains_inspected",
            "initial_entry_domains_published",
            "pending_descendant_domains",
            "descendant_closure",
        ] {
            if let Some(value) = document.get(key) {
                out[key] = value.clone();
            }
        }
        if let Some(error) = document["error"].as_str() {
            out["error"] = json!(error.chars().take(512).collect::<String>());
        }
        out
    }
}

pub fn owner_domain_walk_with_progress(
    request: OwnerDomainWalkRequest,
    cancellation: &AtomicBool,
    observer: impl Fn(Value),
) -> Result<OwnerDomainWalkResult, AppError> {
    request.matching.preflight_queries()?;
    if request.max_domains == 0
        || request.max_events == 0
        || request.max_frontiers == 0
        || !(1..=64).contains(&request.workers)
        || request.max_containment_checks == Some(0)
        || request.max_route_masks == 0
    {
        return Err(AppError::input("invalid symbolic worklist allowances"));
    }
    if request.route_joint_source_support_pruning && !request.route_domain_overcover {
        return Err(AppError::input(
            "joint source-support pruning requires route domain overcover",
        ));
    }
    if let Some(checkpoint) = &request.checkpoint {
        if checkpoint.interval_seconds == 0 {
            return Err(AppError::input("checkpoint interval must be positive"));
        }
        if request.publication_policy == OwnerDomainWalkPublicationPolicy::OwnerBatched {
            return Err(AppError::input(
                "checkpointing requires Ordered or Ready publication",
            ));
        }
    }
    if request.apply_subdivision.is_some()
        && request.publication_policy != OwnerDomainWalkPublicationPolicy::Ordered
    {
        return Err(AppError::input(
            "Apply subdivision requires ordered publication",
        ));
    }
    if request.publication_policy == OwnerDomainWalkPublicationPolicy::Ready
        && request.scheduling_policy == OwnerDomainWalkSchedulingPolicy::InspectAll
    {
        return Err(AppError::input(
            "Ready publication requires TransferUnreserved scheduling",
        ));
    }
    OwnerDomainWalkRequest::validate_inspection_workers(
        request.workers,
        request.inspection_workers,
        request.max_containment_checks,
    )
    .map_err(AppError::input)?;
    request
        .scheduling_policy
        .validate(request.max_containment_checks)
        .map_err(|error| AppError::input(error.to_string()))?;
    if request.reuse_initial_d_bands
        && request.scheduling_policy == OwnerDomainWalkSchedulingPolicy::InspectAll
    {
        return Err(AppError::input(
            "initial D-band reuse requires TransferUnreserved scheduling",
        ));
    }
    rustred::campaign::ParallelExecution::preflight_requested_core_budget(request.workers)
        .map_err(|e| AppError::input(e.to_string()))?;
    let (selection, arity, limits) = input::Selection::parse(&request.matching.selection_json)?;
    let queries = matching::input::parse(
        &request.matching.queries_json,
        arity,
        request.matching.max_queries,
        request.matching.max_query_bytes,
    )?;
    let mut admitted = json!({"event":"admitted", "operation":"owner_domain_walk", "arity":arity,
        "input_domains":queries.len(), "workers":request.workers, "max_domains":request.max_domains,
        "max_events":request.max_events, "max_frontiers":request.max_frontiers,
        "max_containment_checks":request.max_containment_checks,
        "containment_check_policy":"general_comparisons_only; null_is_unlimited; checked_counter",
        "route_domain_overcover":request.route_domain_overcover, "max_route_masks":request.max_route_masks,
        "route_joint_source_support_pruning":request.route_joint_source_support_pruning,
        "applied_limits":limits_json(&request), "publication_policy":"stable_domain_id_stream",
        "bounded_refinement_axes":matching::refinement_axes_name(request.matching.match_limits.refinement_axes),
        "max_bounded_refinement_cells":request.matching.match_limits.max_bounded_refinement_cells,
        "family_closure_claim":false, "ibp_generation":false});
    admitted["worker_allocation"] =
        worker_budget::WorkerBudget::for_request(&request).json(request.inspection_workers);
    if request.scheduling_policy != OwnerDomainWalkSchedulingPolicy::InspectAll {
        admitted["scheduling_policy"] =
            execution::scheduling_policy_json(request.scheduling_policy);
        admitted["publication_policy"] = json!("stable_domain_responsibility_stream");
    }
    if request.reuse_initial_d_bands {
        admitted["reuse_initial_d_bands"] = json!(true);
        admitted["partial_inspection_policy"] =
            json!("exact_initial_high_D_overlap; pinned_anchor_plus_native_residual");
    }
    if request.publication_policy == OwnerDomainWalkPublicationPolicy::OwnerBatched {
        admitted["publication_policy"] = json!("owner_batched");
    }
    if request.publication_policy == OwnerDomainWalkPublicationPolicy::Ready {
        admitted["publication_policy"] = json!("ready_ticket_stream");
    }
    let with_allowances = |mut event: Value| {
        event["requested_max_queries"] = json!(request.matching.max_queries);
        event["requested_max_query_bytes"] = json!(request.matching.max_query_bytes);
        observer(event);
    };
    with_allowances(admitted);
    macro_rules! dispatch { ($($n:literal),*) => { match arity {
        $($n => run::<$n>(&request, &selection, limits, &queries, cancellation, &with_allowances),)*
        _ => unreachable!("admitted arity"),
    }} }
    let mut result = dispatch!(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16)?;
    result.document["requested_max_queries"] = json!(request.matching.max_queries);
    result.document["requested_max_query_bytes"] = json!(request.matching.max_query_bytes);
    Ok(result)
}
fn mask<const N: usize>(owner: &[bool; N]) -> String {
    owner.iter().map(|&b| if b { '1' } else { '0' }).collect()
}

fn stats_json(s: OwnerAppliedStats) -> Value {
    json!({"selected_pieces":s.selected_pieces,"term_visits":s.term_visits,"shift_groups":s.shift_groups,
        "application_refinement_steps":s.application_refinement_steps,
        "application_refinement_cells":s.application_refinement_cells,
        "boundary_cells":s.boundary_cells,"sign_splits":s.sign_splits,"native_operations":s.native_operations,
        "correlation_empty_cells":s.correlation_empty_cells,
        "optional_coefficient_refusals":s.optional_coefficient_refusals,"optional_original_refusals":s.optional_original_refusals,
        "optional_coalesced_refusals":s.optional_coalesced_refusals,"coalescing_additions":s.coalescing_additions,
        "events":s.events,"successors":s.successors,"conditional_successors":s.conditional_successors,
        "same_support_successors":s.same_support_successors,"strict_subsupport_successors":s.strict_subsupport_successors,
        "unsupported_support_successors":s.unsupported_support_successors,
        "conditional_unsupported_support_successors":s.conditional_unsupported_support_successors,
        "problems":s.problems,"zero_terms":s.zero_terms,"cancelled_groups":s.cancelled_groups,"zero_sector_groups":s.zero_sector_groups,
        "matching":{"rules":s.matching.rules,"terminal_checks":s.matching.terminal_checks,"predicates":s.matching.predicates,
            "pieces":s.matching.pieces,"cells":s.matching.cells,"split_operations":s.matching.split_operations,
            "coordinate_cells":s.matching.coordinate_cells,"rank_empty_cells":s.matching.rank_empty_cells,
            "correlation_empty_cells":s.matching.correlation_empty_cells,
            "refinement_cells":s.matching.refinement_cells,"refinement_steps":s.matching.refinement_steps}})
}
fn limits_json(r: &OwnerDomainWalkRequest) -> Value {
    let a = r.applied_limits;
    let m = r.matching.match_limits;
    json!({"max_term_visits":a.max_term_visits,"max_shift_groups":a.max_shift_groups,
        "max_boundary_cells":a.max_boundary_cells,"max_sign_splits":a.max_sign_splits,
        "max_native_operations":a.max_native_operations,"max_events":a.max_events,
        "max_scratch_terms":a.max_scratch_terms,"max_scratch_boxes":a.max_scratch_boxes,
        "max_scratch_coordinate_cells":a.max_scratch_coordinate_cells,
        "cell_refinement":physical_parts::cell_refinement_json(a.cell_refinement),
        "matching":{"max_rules":m.max_rules,"max_terminal_checks":m.max_terminal_checks,
            "max_predicates":m.max_predicates,"max_pieces":m.max_pieces,"max_cells":m.max_cells,
            "max_split_operations":m.max_split_operations,"max_coordinate_cells":m.max_coordinate_cells,
            "max_bounded_refinement_cells":m.max_bounded_refinement_cells,
            "refinement_axes":matching::refinement_axes_name(m.refinement_axes),
            "guard_algebra":inspection::debug(&m.guard_algebra)}})
}

/// Both publication policies use the same post-load boundary. In particular,
/// owner partitioning and ledger/report finalization are not free setup work.
fn finish_timing(document: &mut Value, started: Instant, prepared: f64) {
    let elapsed = started.elapsed().as_secs_f64();
    document["prepared_seconds"] = json!(prepared);
    document["traversal_seconds"] = json!(elapsed - prepared);
    document["elapsed_seconds"] = json!(elapsed);
    document["traversal_timing_boundary"] = json!(
        "after_owner_preparation_through_initial_admission_walk_report_and_queue_cleanup; excludes_owner_unload_and_output_write"
    );
}

fn run<const N: usize>(
    request: &OwnerDomainWalkRequest,
    selection: &input::Selection,
    load_limits: crate::CandidateOwnerLoadLimits,
    queries: &[matching::input::Query],
    cancellation: &AtomicBool,
    observer: &impl Fn(Value),
) -> Result<OwnerDomainWalkResult, AppError> {
    let started = Instant::now();
    let mut checkpoint = checkpoint::Store::open(request).map_err(AppError::input)?;
    // Authenticate and decode before native owner import. No corrupted or
    // incompatible checkpoint is allowed to begin a new inspection.
    let restored = checkpoint
        .as_ref()
        .map(|store| store.resume::<N>())
        .transpose()
        .map_err(AppError::input)?
        .flatten();
    let latest_checkpoint =
        std::cell::RefCell::new(checkpoint.as_ref().and_then(|s| s.metadata()).cloned());
    let checkpoint_write = std::cell::RefCell::new(None::<Value>);
    let original_observer = observer;
    let enriched_observer = |mut event: Value| {
        if let Some(writing) = event.get("checkpoint_write") {
            *checkpoint_write.borrow_mut() = Some(writing.clone());
        }
        if event["event"] == "checkpoint_saved" {
            *checkpoint_write.borrow_mut() = None;
        }
        if let Some(writing) = checkpoint_write.borrow().as_ref() {
            event["checkpoint_write"] = writing.clone();
        }
        if let Some(metadata) = event.get("checkpoint") {
            *latest_checkpoint.borrow_mut() = Some(metadata.clone());
        } else if let Some(metadata) = latest_checkpoint.borrow().as_ref() {
            event["checkpoint"] = metadata.clone();
        }
        original_observer(event);
    };
    let observer = &enriched_observer;
    if let Some(store) = checkpoint.as_mut() {
        if let Some(event) = store.bootstrap().map_err(AppError::input)? {
            observer(event);
        }
    }
    let mut load = RoutedCampaignRequest::new(String::new(), String::new());
    load.owner_base = request.matching.owner_base.clone();
    load.reduction_limits = request.matching.reduction_limits;
    let reducer = if let Some(store) = checkpoint.as_mut() {
        let mut bind = |owners| store.bind_owners(owners);
        prepare::prepare_with_fingerprints::<N>(
            &load,
            selection,
            load_limits,
            cancellation,
            observer,
            Some(&mut bind),
        )?
    } else {
        prepare::prepare::<N>(&load, selection, load_limits, cancellation, observer)?
    };
    let prepared = started.elapsed().as_secs_f64();
    let mut queue = Queue::with_policy(
        request.max_domains,
        request.max_containment_checks,
        request.scheduling_policy,
    )
    .map_err(AppError::input)?;
    if request.publication_policy == OwnerDomainWalkPublicationPolicy::Ready {
        let OwnerDomainWalkSchedulingPolicy::TransferUnreserved { lookahead } =
            request.scheduling_policy
        else {
            unreachable!("validated Ready scheduling policy")
        };
        queue.delegation = Some(
            delegation::Ledger::new_ready(lookahead, request.max_domains)
                .map_err(|e| AppError::input(e.to_string()))?,
        );
    }
    if request.reuse_initial_d_bands && restored.is_none() {
        queue
            .delegation
            .as_mut()
            .expect("validated transfer policy")
            .begin_initial_admission()
            .map_err(|e| AppError::input(e.to_string()))?;
    }
    let mut inputs = Vec::new();
    let mut input_frontiers = Vec::new();
    let mut error = reducer
        .is_none()
        .then(|| "cancelled during preparation".to_owned());
    if let Some(reducer) = &reducer
        && restored.is_none()
    {
        for query in queries {
            let domain = Domain {
                phase: Phase::Apply,
                owner: query.owner.as_slice().try_into().expect("validated arity"),
                lower: query.lower.clone(),
                upper: query.upper.clone(),
                rank: query.rank,
                powers: query.powers,
            };
            let domain = if request.route_domain_overcover
                && !reducer
                    .programs()
                    .owner_sectors()
                    .any(|o| o == &domain.owner)
            {
                if reducer.domain_routing_requires_source_conditions() {
                    if input_frontiers.len() == request.max_frontiers {
                        error = Some("retained frontier allowance".into());
                        break;
                    }
                    input_frontiers.push(json!({"id":query.id,"kind":"initial_route_source_validity_obligation",
                        "owner":mask(&domain.owner),"lower":domain.lower,"upper":domain.upper,"rank":domain.rank,
                        "power_bounds":power_bounds_json(domain.powers),
                        "reached_missing_rule_claim":false}));
                    inputs.push(
                        json!({"id":query.id,"domain":null,"source_validity_unresolved":true}),
                    );
                    continue;
                }
                Domain {
                    phase: Phase::Route,
                    ..domain
                }
            } else {
                domain
            };
            match queue.admit(domain) {
                Ok((id, _)) => inputs.push(json!({"id":query.id,"domain":id})),
                Err(e) => {
                    error = Some(e.into());
                    break;
                }
            }
        }
    }
    if request.reuse_initial_d_bands && restored.is_none() {
        queue
            .delegation
            .as_mut()
            .expect("validated transfer policy")
            .finish_initial_admission()
            .map_err(|e| AppError::input(e.to_string()))?;
    }
    // A failed initial admission must never become a successful walk over its
    // retained prefix. Preserve the existing incomplete-result path below.
    if request.publication_policy == OwnerDomainWalkPublicationPolicy::OwnerBatched
        && error.is_none()
        && let Some(reducer) = &reducer
    {
        let result = execution::owner_batches::run(
            &queue.domains,
            input_frontiers.len(),
            queue.containment_checks,
            reducer,
            request,
            cancellation,
            observer,
        )
        .map_err(AppError::input)?;
        for input in &mut inputs {
            if let Some(id) = input["domain"].as_u64() {
                input["domain"] = result.initial_handles[id as usize].clone();
            }
        }
        let mut document = result.document;
        document["worker_allocation"] =
            worker_budget::WorkerBudget::for_request(request).json(request.inspection_workers);
        document["inputs"] = Value::Array(inputs);
        document["input_frontiers"] = Value::Array(input_frontiers);
        document["max_bounded_refinement_cells"] =
            json!(request.matching.match_limits.max_bounded_refinement_cells);
        drop(queue);
        finish_timing(&mut document, started, prepared);
        observer(OwnerDomainWalkResult::completion_progress(&document));
        return Ok(OwnerDomainWalkResult {
            all_scheduled_domains_resolved: document["all_scheduled_domains_resolved"] == true,
            document,
        });
    }
    let mut state = execution::State::new(queue, input_frontiers.len(), error);
    if let Some(restored) = restored {
        state = restored.state;
        inputs = restored.inputs;
        input_frontiers = restored.input_frontiers;
    }
    if let Some(reducer) = &reducer {
        if let Some(store) = checkpoint.as_mut() {
            if state.error.is_none() {
                if let Some(event) = store
                    .save(&state, &inputs, &input_frontiers, true, observer)
                    .map_err(AppError::input)?
                {
                    observer(event);
                }
            }
            execution::run_checkpointed(
                &mut state,
                reducer,
                request,
                cancellation,
                observer,
                &mut |state| {
                    if let Some(event) =
                        store.save(state, &inputs, &input_frontiers, false, observer)?
                    {
                        observer(event);
                    }
                    Ok(())
                },
            );
            if state.error.is_none() {
                if let Some(event) = store
                    .save(&state, &inputs, &input_frontiers, true, observer)
                    .map_err(AppError::input)?
                {
                    observer(event);
                }
            }
        } else {
            execution::run(&mut state, reducer, request, cancellation, observer);
        }
    }
    state.refresh_closure(cancellation, true);
    if checkpoint.is_some() && (state.checkpoint_paused || reducer.is_none()) {
        // A checkpoint is the state; this receipt must not duplicate the full
        // retained queue and diagnostics (which can be many gigabytes).
        let mut document = json!({"schema":"rustred.owner-domain-walk.paused.json.v1","status":"paused",
            "resume_supported":true,"checkpoint":latest_checkpoint.borrow().clone(),
            "full_state_in_checkpoint":true,"independent_certification":false,
            "family_closure_claim":false,"all_scheduled_domains_resolved":false,"recursive_worklist_exhausted":false,
            "scheduled_nodes":state.queue.domains.len(),"completed_nodes":state.completed,"processed_nodes":state.published_count(),
            "queued_nodes":state.queue.domains.len()-state.published_count(),"committed_domains":state.published_count(),"committed_events":state.events,
            "contiguous_publication_watermark":state.queue.next,
            "events":state.events,"successors":state.successors,"conditional_successors":state.conditional,"frontiers":state.frontiers,
            "initial_entry_domains_total":state.initial_domain_count,"initial_entry_domains_inspected":state.initial_entry_domains_inspected,
            "initial_entry_domains_published":state.initial_published(),
            "pending_descendant_domains":state.pending_descendants(),
            "workers":request.workers,"preparation_interrupted":reducer.is_none(),
            "timing_scope":"this_process_session; canonical counters span checkpoint resumes"});
        document["parallel"] = std::mem::take(&mut state.parallel);
        document["publication_policy"] = json!(if state.ready() {
            "ready_ticket_stream"
        } else {
            "stable_domain_id_stream"
        });
        state.add_delegation_progress(&mut document);
        state.add_ready_progress(&mut document);
        document["descendant_closure"] = state.closure_json();
        drop(state);
        finish_timing(&mut document, started, prepared);
        observer(OwnerDomainWalkResult::completion_progress(&document));
        return Ok(OwnerDomainWalkResult {
            all_scheduled_domains_resolved: false,
            document,
        });
    }
    let delegation = state.finalize_delegation();
    for row in &mut state.records {
        if let Some(id) = row["id"].as_u64().and_then(|id| usize::try_from(id).ok()) {
            row["descendant_closed"] = json!(state.closure.borrow().closed(id));
        }
    }
    let exhausted = state.error.is_none() && state.published_count() == state.queue.domains.len();
    let resolved = exhausted
        && state.frontiers == 0
        && delegation
            .as_ref()
            .is_none_or(|value| value["all_ledger_obligations_discharged"] == true);
    let mut document = json!({"schema":"rustred.owner-domain-walk.json.v2",
        "status":if resolved {"locally_resolved"} else {"incomplete"},
        "all_scheduled_domains_resolved":resolved,"recursive_worklist_exhausted":exhausted,
        "family_closure_claim":false,"ibp_generation":false,"routing_expanded":false,
        "route_domain_overcover":request.route_domain_overcover,
        "route_joint_source_support_pruning":request.route_joint_source_support_pruning,
        "routed_domains":state.routed,"route_masks":state.route_masks,
        "route_joint_support_masks_pruned":state.route_joint_support_masks_pruned,
        "max_scheduled_finite_rank":state.queue.max_finite_rank,"unbounded_rank_domains":state.queue.unbounded_rank_domains,
        "independent_certification":false,"resume_supported":false,
        "conditional_successors_use_conservative_domain_overcover":true,
        "scheduled_nodes":state.queue.domains.len(),"completed_nodes":state.completed,
        "queued_nodes":state.queue.domains.len().saturating_sub(state.published_count()),
        "processed_nodes":state.published_count(),"failed_nodes":state.native_records.saturating_sub(state.completed),
        "deduplication_hits":state.queue.deduplicated,"containment_checks":state.queue.containment_checks,
        "exact_domain_hits":state.queue.exact_hits,"full_orthant_hits":state.queue.orthant_hits,
        "successors":state.successors,"conditional_successors":state.conditional,
        "optional_coefficient_refusals":state.optional.total,"optional_original_refusals":state.optional.original,
        "optional_coalesced_refusals":state.optional.coalesced,
        "frontiers":state.frontiers,"events":state.events,
        "error":state.error,"prepared_seconds":prepared,
        "traversal_seconds":started.elapsed().as_secs_f64()-prepared,"elapsed_seconds":started.elapsed().as_secs_f64()});
    // These trees can dominate campaign RAM. Move their allocations directly;
    // json!(mem::take(...)) would still serialize and clone every nested Value.
    document["inputs"] = Value::Array(inputs);
    document["input_frontiers"] = Value::Array(input_frontiers);
    document["domains"] = take_report_array(&mut state.records);
    // Keep macro expansion bounded without a crate-wide recursion allowance.
    document["workers"] = json!(request.workers);
    if checkpoint.is_some() {
        document["resume_supported"] = json!(true);
        document["checkpoint"] = latest_checkpoint.borrow().clone().unwrap_or(Value::Null);
        document["timing_scope"] =
            json!("this_process_session; canonical counters span checkpoint resumes");
    }
    document["initial_entry_domains_total"] = json!(state.initial_domain_count);
    document["initial_entry_domains_inspected"] = json!(state.initial_entry_domains_inspected);
    document["initial_entry_domains_published"] = json!(state.initial_published());
    document["pending_descendant_domains"] = json!(state.pending_descendants());
    document["descendant_closure"] = state.closure_json();
    document["worker_allocation"] =
        worker_budget::WorkerBudget::for_request(request).json(request.inspection_workers);
    if request.publication_policy == OwnerDomainWalkPublicationPolicy::OwnerBatched {
        document["requested_publication_policy"] = json!("owner_batched");
        document["owner_batched_traversal_started"] = json!(false);
    }
    document["job_local_reuse_hits"] = json!(state.job_local_reuse_hits);
    document["pre_admitted_orthant_hits"] = json!(state.pre_admitted_orthant_hits);
    document["pre_admitted_orthant_policy"] =
        json!("immutable_initial_admitted_phase_owner_rank; pending_not_completed");
    document["pre_admitted_orthant_limits"] = json!({"max_buckets":initial_orthants::MAX_BUCKETS,
        "max_logical_entry_bytes":initial_orthants::MAX_ENTRY_BYTES,
        "container_overhead_and_rss_excluded":true});
    document["job_local_reuse_policy"] =
        json!("per_inspection_after_ordered_emit; pending_not_completed");
    document["job_local_reuse_limits"] = json!({"max_keys":reuse::MAX_KEYS,
        "max_logical_key_bytes":reuse::MAX_KEY_BYTES,"container_overhead_and_rss_excluded":true});
    document["reuse_counter_scope"] = json!(
        "job_local and pre_admitted hits increment aggregate reuse only; skipped exact/orthant/general lookups are not attributed"
    );
    document["max_events"] = json!(request.max_events);
    document["max_frontiers"] = json!(request.max_frontiers);
    document["max_containment_checks"] = json!(request.max_containment_checks);
    document["containment_maintenance_checks"] = json!(state.queue.containment_maintenance_checks);
    document["containment_retired_candidates"] = json!(state.queue.containment_retired_candidates);
    document["containment_summary_builds"] = json!(state.queue.containment_summary_builds);
    document["containment_semantic_hits"] = json!(state.queue.containment_semantic_hits);
    document["containment_semantic_retirements"] = json!(state.queue.containment_semantic_retirements);
    document["containment_candidates"] = json!(state.queue.containment_candidate_count());
    document["containment_index_policy"] = json!(state.queue.containment_index_policy());
    document["containment_check_policy"] =
        json!("general_comparisons_only; null_is_unlimited; checked_counter");
    document["applied_limits"] = limits_json(request);
    document["bounded_refinement_axes"] = json!(matching::refinement_axes_name(
        request.matching.match_limits.refinement_axes
    ));
    document["max_bounded_refinement_cells"] =
        json!(request.matching.match_limits.max_bounded_refinement_cells);
    document["publication_policy"] = json!("stable_domain_id_stream");
    document["parallel"] = std::mem::take(&mut state.parallel);
    document["uncommitted_inspections"] = take_report_array(&mut state.uncommitted);
    document["successful_publication_matches_serial"] = json!(true);
    document["failure_or_cancellation_prefix_may_differ"] = json!(true);
    document["committed_domains"] = json!(state.published_count());
    document["contiguous_publication_watermark"] = json!(state.queue.next);
    document["committed_events"] = json!(state.events);
    if let Some(delegation) = delegation {
        document["schema"] = json!("rustred.owner-domain-walk.json.v3");
        document["publication_policy"] = json!("stable_domain_responsibility_stream");
        document["scheduling_policy"] =
            execution::scheduling_policy_json(request.scheduling_policy);
        document["native_processed_nodes"] = json!(state.native_records);
        document["delegation"] = delegation;
    }
    if state.ready() {
        document["schema"] = json!("rustred.owner-domain-walk.json.v5");
        document["publication_policy"] = json!("ready_ticket_stream");
        document["successful_publication_matches_serial"] = json!(false);
        document["queue_ids_and_receipt_order_depend_on_readiness"] = json!(true);
    }
    if request.reuse_initial_d_bands {
        document["reuse_initial_d_bands"] = json!(true);
        document["initial_overlap_index"] = index_report::render(
            state.initial_overlap_report,
            index_report::Scope::GlobalInitial,
        );
        document["partial_initial_inspections"] = json!(
            state
                .queue
                .delegation
                .as_ref()
                .map_or(0, |l| l.partial_initial_inspections())
        );
        document["partial_inspection_policy"] =
            json!("exact_initial_high_D_overlap; pinned_anchor_plus_native_residual");
        document["initial_overlap_limits"] = json!({"max_initial_domains":initial_overlap::MAX_INITIAL_DOMAINS,
            "count_scope":"initial_apply_only",
            "max_logical_entry_bytes":initial_overlap::MAX_ENTRY_BYTES,"container_overhead_and_rss_excluded":true});
    }
    drop(state);
    finish_timing(&mut document, started, prepared);
    observer(OwnerDomainWalkResult::completion_progress(&document));
    Ok(OwnerDomainWalkResult {
        all_scheduled_domains_resolved: resolved,
        document,
    })
}

fn take_report_array(values: &mut Vec<Value>) -> Value {
    Value::Array(std::mem::take(values))
}

#[cfg(test)]
mod policy_tests {
    use super::*;

    #[test]
    fn ready_requires_explicit_transfer_and_rejects_subdivision_before_loading() {
        for subdivision in [false, true] {
            let mut request = OwnerDomainWalkRequest::new(OwnerDomainMatchRequest::new(
                String::new(),
                String::new(),
            ));
            request.publication_policy = OwnerDomainWalkPublicationPolicy::Ready;
            if subdivision {
                request.scheduling_policy = OwnerDomainWalkSchedulingPolicy::TransferUnreserved {
                    lookahead: std::num::NonZeroUsize::new(8).unwrap(),
                };
                request.apply_subdivision =
                    Some(OwnerDomainWalkApplySubdivision { axis: 0, cut: 1 });
            }
            let error = owner_domain_walk_with_progress(request, &AtomicBool::new(false), |_| {
                panic!("unsupported Ready policy must fail before loading owners")
            })
            .unwrap_err();
            let expected = if subdivision {
                "Apply subdivision requires ordered publication"
            } else {
                "Ready publication requires TransferUnreserved scheduling"
            };
            assert!(error.to_string().contains(expected), "{error}");
        }
    }

    #[test]
    fn report_array_transfer_preserves_allocations_instead_of_cloning() {
        let mut records = vec![json!({"payload":"x".repeat(65_536)})];
        let vector_pointer = records.as_ptr();
        let string_pointer = records[0]["payload"].as_str().unwrap().as_ptr();
        let report = take_report_array(&mut records);
        assert!(records.is_empty());
        assert_eq!(report.as_array().unwrap().as_ptr(), vector_pointer);
        assert_eq!(
            report[0]["payload"].as_str().unwrap().as_ptr(),
            string_pointer
        );
    }

    #[test]
    fn subdivision_owner_batched_is_rejected_by_public_api_before_loading() {
        let mut request =
            OwnerDomainWalkRequest::new(OwnerDomainMatchRequest::new(String::new(), String::new()));
        request.apply_subdivision = Some(OwnerDomainWalkApplySubdivision { axis: 0, cut: 1 });
        request.publication_policy = OwnerDomainWalkPublicationPolicy::OwnerBatched;
        let error = owner_domain_walk_with_progress(request, &AtomicBool::new(false), |_| {
            panic!("unsupported policy must fail before owner preparation")
        })
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Apply subdivision requires ordered publication")
        );
    }

    #[test]
    fn bounded_refinement_policy_reports_effective_match_limits_not_applied_defaults() {
        use rustred::solver::OwnerDomainRefinementAxes;
        let mut request =
            OwnerDomainWalkRequest::new(OwnerDomainMatchRequest::new(String::new(), String::new()));
        assert_eq!(
            limits_json(&request)["matching"]["refinement_axes"],
            "inactive-only"
        );
        request.matching.match_limits.refinement_axes = OwnerDomainRefinementAxes::FiniteAxes;
        request.matching.match_limits.max_bounded_refinement_cells = 23;
        assert_eq!(
            limits_json(&request)["matching"]["refinement_axes"],
            "finite-axes"
        );
        assert_eq!(
            limits_json(&request)["matching"]["max_bounded_refinement_cells"],
            23
        );
        assert!(!request.route_domain_overcover);
        assert!(!request.route_joint_source_support_pruning);
        let document =
            json!({"bounded_refinement_axes":"finite-axes", "max_bounded_refinement_cells":23});
        let progress = OwnerDomainWalkResult::completion_progress(&document);
        assert_eq!(progress["bounded_refinement_axes"], "finite-axes");
        assert_eq!(progress["max_bounded_refinement_cells"], 23);
        assert_eq!(progress["family_closure_claim"], false);
    }

    #[test]
    fn initial_d_band_reuse_is_opt_in_and_requires_responsibility_ledger() {
        let mut request =
            OwnerDomainWalkRequest::new(OwnerDomainMatchRequest::new(String::new(), String::new()));
        assert!(!request.reuse_initial_d_bands);
        request.reuse_initial_d_bands = true;
        let error = owner_domain_walk_with_progress(request, &AtomicBool::new(false), |_| {
            panic!("must reject before preparation")
        })
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("initial D-band reuse requires TransferUnreserved")
        );
        let progress = OwnerDomainWalkResult::completion_progress(
            &json!({"reuse_initial_d_bands":true,"partial_initial_inspections":7}),
        );
        assert_eq!(progress["reuse_initial_d_bands"], true);
        assert_eq!(progress["partial_initial_inspections"], 7);
    }

    #[test]
    fn explicit_domain_storage_budget_has_no_hidden_million_domain_ceiling() {
        for limit in [1_000_001, 10_000_000, usize::MAX] {
            let mut request = OwnerDomainWalkRequest::new(OwnerDomainMatchRequest::new(
                r#"{"family_fingerprint":"unused","owners":[],"initial_frontier_routes":[]}"#
                    .into(),
                String::new(),
            ));
            request.max_domains = limit;
            let error = owner_domain_walk_with_progress(request, &AtomicBool::new(false), |_| {
                panic!("empty owner input must fail before loading or allocation")
            })
            .unwrap_err();
            assert!(error.to_string().contains("no owners"), "{error}");
        }
        let mut request =
            OwnerDomainWalkRequest::new(OwnerDomainMatchRequest::new(String::new(), String::new()));
        request.max_domains = 0;
        let error = owner_domain_walk_with_progress(request, &AtomicBool::new(false), |_| {
            panic!("zero budget must fail before loading")
        })
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("invalid symbolic worklist allowances")
        );
    }

    #[test]
    fn containment_defaults_to_unlimited_and_zero_is_rejected_before_loading() {
        let mut request =
            OwnerDomainWalkRequest::new(OwnerDomainMatchRequest::new(String::new(), String::new()));
        assert_eq!(request.max_containment_checks, None);
        request.max_containment_checks = Some(0);
        let error = owner_domain_walk_with_progress(request, &AtomicBool::new(false), |_| {
            panic!("invalid policy must be rejected before admission")
        })
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("invalid symbolic worklist allowances")
        );
    }

    #[test]
    fn completion_retains_effective_containment_policy_and_distinct_reuse_counter() {
        for limit in [None, Some(17)] {
            let document = json!({"max_containment_checks":limit,
                "containment_check_policy":"general_comparisons_only; null_is_unlimited; checked_counter",
                "containment_checks":19, "pre_admitted_orthant_hits":3,
                "containment_maintenance_checks":4, "containment_retired_candidates":2,
                "containment_summary_builds":11, "containment_semantic_hits":7,
                "containment_semantic_retirements":1,
                "containment_candidates":5, "containment_index_policy":"test_policy"});
            let completion = OwnerDomainWalkResult::completion_progress(&document);
            assert_eq!(completion["max_containment_checks"], json!(limit));
            assert_eq!(
                completion["containment_check_policy"],
                document["containment_check_policy"]
            );
            assert_eq!(completion["containment_checks"], 19);
            for key in [
                "containment_maintenance_checks",
                "containment_retired_candidates",
                "containment_summary_builds",
                "containment_semantic_hits",
                "containment_semantic_retirements",
                "containment_candidates",
                "containment_index_policy",
            ] {
                assert_eq!(completion[key], document[key]);
            }
            assert_eq!(completion["pre_admitted_orthant_hits"], 3);
        }
    }
}
