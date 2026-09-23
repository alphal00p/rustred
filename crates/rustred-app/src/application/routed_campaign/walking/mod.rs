//! Shared symbolic successor discovery over one immutable owner snapshot.
//! Stable streamed publication is not a family-closure certificate.
mod delegation;
mod diagnostics;
mod execution;
mod initial_orthants;
mod initial_overlap;
mod inspection;
mod parallel;
mod queue;
mod reuse;
mod routing;

use super::{OwnerDomainMatchRequest, RoutedCampaignRequest, input, matching, prepare};
use crate::AppError;
use matching::input::power_bounds_json;
use queue::{Domain, Phase, Queue};
use rustred::solver::{OwnerAppliedLimits, OwnerAppliedStats};
use serde_json::{Value, json};
use std::sync::atomic::AtomicBool;
use std::time::Instant;

pub use delegation::SchedulingPolicy as OwnerDomainWalkSchedulingPolicy;
pub use execution::owner_batches::OwnerDomainWalkPublicationPolicy;

#[derive(Clone, Debug)]
pub struct OwnerDomainWalkRequest {
    /// Load policy and per-domain matcher allowances; max_total_pieces applies
    /// only to local-match reports, not this streaming worklist.
    pub matching: OwnerDomainMatchRequest,
    /// The nested matching member is overridden by matching.match_limits.
    pub applied_limits: OwnerAppliedLimits,
    /// One runs inline. More share immutable programs and bounded event slots.
    /// Caller configures affinity and native inner pools; no environment edits.
    pub workers: usize,
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
    pub max_route_masks: usize,
}
impl OwnerDomainWalkRequest {
    pub fn new(matching: OwnerDomainMatchRequest) -> Self {
        Self {
            matching,
            applied_limits: Default::default(),
            workers: 1,
            publication_policy: OwnerDomainWalkPublicationPolicy::Ordered,
            scheduling_policy: OwnerDomainWalkSchedulingPolicy::InspectAll,
            reuse_initial_d_bands: false,
            max_domains: 100_000,
            max_events: 1_000_000,
            max_frontiers: 100_000,
            max_containment_checks: None,
            route_domain_overcover: false,
            max_route_masks: 100_000,
        }
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
            "route_domain_overcover",
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
            "prepared_seconds",
            "traversal_seconds",
            "native_driver_seconds",
            "traversal_timing_boundary",
            "elapsed_seconds",
            "all_scheduled_domains_resolved",
            "recursive_worklist_exhausted",
        ] {
            out[key] = document[key].clone();
        }
        for key in [
            "publication_policy",
            "requested_publication_policy",
            "owner_batched_traversal_started",
            "owner_bucket_count",
            "nonempty_owner_buckets",
            "scheduling_policy",
            "delegation",
            "native_processed_nodes",
            "reuse_initial_d_bands",
            "partial_initial_inspections",
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
    if !(1..=10_000).contains(&request.matching.max_queries)
        || request.max_domains == 0
        || request.max_events == 0
        || !(1..=1_000_000).contains(&request.max_frontiers)
        || !(1..=64).contains(&request.workers)
        || request.max_containment_checks == Some(0)
        || request.max_route_masks == 0
    {
        return Err(AppError::input("invalid symbolic worklist allowances"));
    }
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
    )?;
    let mut admitted = json!({"event":"admitted", "operation":"owner_domain_walk", "arity":arity,
        "input_domains":queries.len(), "workers":request.workers, "max_domains":request.max_domains,
        "max_events":request.max_events, "max_frontiers":request.max_frontiers,
        "max_containment_checks":request.max_containment_checks,
        "containment_check_policy":"general_comparisons_only; null_is_unlimited; checked_counter",
        "route_domain_overcover":request.route_domain_overcover, "max_route_masks":request.max_route_masks,
        "applied_limits":limits_json(&request), "publication_policy":"stable_domain_id_stream",
        "bounded_refinement_axes":matching::refinement_axes_name(request.matching.match_limits.refinement_axes),
        "max_bounded_refinement_cells":request.matching.match_limits.max_bounded_refinement_cells,
        "family_closure_claim":false, "ibp_generation":false});
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
    observer(admitted);
    macro_rules! dispatch { ($($n:literal),*) => { match arity {
        $($n => run::<$n>(&request, &selection, limits, &queries, cancellation, &observer),)*
        _ => unreachable!("admitted arity"),
    }} }
    dispatch!(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16)
}
fn mask<const N: usize>(owner: &[bool; N]) -> String {
    owner.iter().map(|&b| if b { '1' } else { '0' }).collect()
}

fn stats_json(s: OwnerAppliedStats) -> Value {
    json!({"selected_pieces":s.selected_pieces,"term_visits":s.term_visits,"shift_groups":s.shift_groups,
        "boundary_cells":s.boundary_cells,"sign_splits":s.sign_splits,"native_operations":s.native_operations,
        "correlation_empty_cells":s.correlation_empty_cells,
        "optional_coefficient_refusals":s.optional_coefficient_refusals,"optional_original_refusals":s.optional_original_refusals,
        "optional_coalesced_refusals":s.optional_coalesced_refusals,"coalescing_additions":s.coalescing_additions,
        "events":s.events,"successors":s.successors,"conditional_successors":s.conditional_successors,
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
    let mut load = RoutedCampaignRequest::new(String::new(), String::new());
    load.owner_base = request.matching.owner_base.clone();
    load.reduction_limits = request.matching.reduction_limits;
    let reducer = prepare::prepare::<N>(&load, selection, load_limits, cancellation, observer)?;
    let prepared = started.elapsed().as_secs_f64();
    let mut queue = Queue::with_policy(
        request.max_domains,
        request.max_containment_checks,
        request.scheduling_policy,
    )
    .map_err(AppError::input)?;
    if request.reuse_initial_d_bands {
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
    if let Some(reducer) = &reducer {
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
    if request.reuse_initial_d_bands {
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
        document["inputs"] = json!(inputs);
        document["input_frontiers"] = json!(input_frontiers);
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
    if let Some(reducer) = &reducer {
        execution::run(&mut state, reducer, request, cancellation, observer);
    }
    let delegation = state.finalize_delegation();
    let exhausted = state.error.is_none() && state.queue.next == state.queue.domains.len();
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
        "routed_domains":state.routed,"route_masks":state.route_masks,
        "max_scheduled_finite_rank":state.queue.max_finite_rank,"unbounded_rank_domains":state.queue.unbounded_rank_domains,
        "independent_certification":false,"resume_supported":false,
        "conditional_successors_use_conservative_domain_overcover":true,
        "scheduled_nodes":state.queue.domains.len(),"completed_nodes":state.completed,
        "queued_nodes":state.queue.domains.len().saturating_sub(state.queue.next),
        "processed_nodes":state.queue.next,"failed_nodes":state.native_records.saturating_sub(state.completed),
        "deduplication_hits":state.queue.deduplicated,"containment_checks":state.queue.containment_checks,
        "exact_domain_hits":state.queue.exact_hits,"full_orthant_hits":state.queue.orthant_hits,
        "successors":state.successors,"conditional_successors":state.conditional,
        "optional_coefficient_refusals":state.optional.total,"optional_original_refusals":state.optional.original,
        "optional_coalesced_refusals":state.optional.coalesced,
        "frontiers":state.frontiers,"events":state.events,"inputs":inputs,"domains":state.records,
        "input_frontiers":input_frontiers,"error":state.error,"prepared_seconds":prepared,
        "traversal_seconds":started.elapsed().as_secs_f64()-prepared,"elapsed_seconds":started.elapsed().as_secs_f64()});
    // Keep macro expansion bounded without a crate-wide recursion allowance.
    document["workers"] = json!(request.workers);
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
    document["uncommitted_inspections"] = json!(state.uncommitted);
    document["successful_publication_matches_serial"] = json!(true);
    document["failure_or_cancellation_prefix_may_differ"] = json!(true);
    document["committed_domains"] = json!(state.queue.next);
    document["committed_events"] = json!(state.events);
    if let Some(delegation) = delegation {
        document["schema"] = json!("rustred.owner-domain-walk.json.v3");
        document["publication_policy"] = json!("stable_domain_responsibility_stream");
        document["scheduling_policy"] =
            execution::scheduling_policy_json(request.scheduling_policy);
        document["native_processed_nodes"] = json!(state.native_records);
        document["delegation"] = delegation;
    }
    if request.reuse_initial_d_bands {
        document["reuse_initial_d_bands"] = json!(true);
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

#[cfg(test)]
mod policy_tests {
    use super::*;

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
