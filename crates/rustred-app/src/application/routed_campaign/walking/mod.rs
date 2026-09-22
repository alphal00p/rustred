//! Shared symbolic successor discovery over one immutable owner snapshot.
//! Stable streamed publication is not a family-closure certificate.
mod diagnostics;
mod execution;
mod inspection;
mod parallel;
mod queue;
mod routing;

use super::{OwnerDomainMatchRequest, RoutedCampaignRequest, input, matching, prepare};
use crate::AppError;
use queue::{Domain, Phase, Queue};
use rustred::solver::{OwnerAppliedLimits, OwnerAppliedStats};
use serde_json::{Value, json};
use std::sync::atomic::AtomicBool;
use std::time::Instant;

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
    pub max_domains: usize,
    /// Committed logical callbacks, not speculative native attempts or bytes.
    pub max_events: usize,
    /// Aggregate retained input/Apply/Route obligations, independent of events.
    pub max_frontiers: usize,
    pub max_containment_checks: usize,
    pub route_domain_overcover: bool,
    pub max_route_masks: usize,
}
impl OwnerDomainWalkRequest {
    pub fn new(matching: OwnerDomainMatchRequest) -> Self {
        Self {
            matching,
            applied_limits: Default::default(),
            workers: 1,
            max_domains: 100_000,
            max_events: 1_000_000,
            max_frontiers: 100_000,
            max_containment_checks: 10_000_000,
            route_domain_overcover: false,
            max_route_masks: 100_000,
        }
    }
}

#[derive(Clone, Debug)]
pub struct OwnerDomainWalkResult {
    /// Every admitted domain inspected without unresolved work. Does not
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
            "containment_checks",
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
            "elapsed_seconds",
            "all_scheduled_domains_resolved",
            "recursive_worklist_exhausted",
        ] {
            out[key] = document[key].clone();
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
        || !(1..=1_000_000).contains(&request.max_domains)
        || request.max_events == 0
        || !(1..=1_000_000).contains(&request.max_frontiers)
        || !(1..=64).contains(&request.workers)
        || request.max_containment_checks == 0
        || request.max_route_masks == 0
    {
        return Err(AppError::input("invalid symbolic worklist allowances"));
    }
    rustred::campaign::ParallelExecution::preflight_requested_core_budget(request.workers)
        .map_err(|e| AppError::input(e.to_string()))?;
    let (selection, arity, limits) = input::Selection::parse(&request.matching.selection_json)?;
    let queries = matching::input::parse(
        &request.matching.queries_json,
        arity,
        request.matching.max_queries,
    )?;
    observer(
        json!({"event":"admitted", "operation":"owner_domain_walk", "arity":arity,
        "input_domains":queries.len(), "workers":request.workers, "max_domains":request.max_domains,
        "max_events":request.max_events, "max_frontiers":request.max_frontiers,
        "route_domain_overcover":request.route_domain_overcover, "max_route_masks":request.max_route_masks,
        "applied_limits":limits_json(&request), "publication_policy":"stable_domain_id_stream",
        "family_closure_claim":false, "ibp_generation":false}),
    );
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
        "optional_coefficient_refusals":s.optional_coefficient_refusals,"optional_original_refusals":s.optional_original_refusals,
        "optional_coalesced_refusals":s.optional_coalesced_refusals,"coalescing_additions":s.coalescing_additions,
        "events":s.events,"successors":s.successors,"conditional_successors":s.conditional_successors,
        "problems":s.problems,"zero_terms":s.zero_terms,"cancelled_groups":s.cancelled_groups,"zero_sector_groups":s.zero_sector_groups,
        "matching":{"rules":s.matching.rules,"terminal_checks":s.matching.terminal_checks,"predicates":s.matching.predicates,
            "pieces":s.matching.pieces,"cells":s.matching.cells,"split_operations":s.matching.split_operations,
            "coordinate_cells":s.matching.coordinate_cells,"rank_empty_cells":s.matching.rank_empty_cells,
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
            "max_bounded_refinement_cells":m.max_bounded_refinement_cells,"guard_algebra":inspection::debug(&m.guard_algebra)}})
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
    let mut queue = Queue::new(request.max_domains, request.max_containment_checks);
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
                        "reached_missing_rule_claim":false}));
                    inputs.push(
                        json!({"id":query.id,"domain":null,"source_validity_unresolved":true}),
                    );
                    continue;
                }
                Domain::route_cover(domain.owner, domain.rank)
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
    let mut state = execution::State::new(queue, input_frontiers.len(), error);
    if let Some(reducer) = &reducer {
        execution::run(&mut state, reducer, request, cancellation, observer);
    }
    let exhausted = state.error.is_none() && state.queue.next == state.queue.domains.len();
    let resolved = exhausted && state.frontiers == 0;
    let mut document = json!({"schema":"rustred.owner-domain-walk.json.v1",
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
        "processed_nodes":state.queue.next,"failed_nodes":state.queue.next.saturating_sub(state.completed),
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
    document["max_events"] = json!(request.max_events);
    document["max_frontiers"] = json!(request.max_frontiers);
    document["applied_limits"] = limits_json(request);
    document["publication_policy"] = json!("stable_domain_id_stream");
    document["parallel"] = state.parallel;
    document["uncommitted_inspections"] = json!(state.uncommitted);
    document["successful_publication_matches_serial"] = json!(true);
    document["failure_or_cancellation_prefix_may_differ"] = json!(true);
    document["committed_domains"] = json!(state.queue.next);
    document["committed_events"] = json!(state.events);
    observer(OwnerDomainWalkResult::completion_progress(&document));
    Ok(OwnerDomainWalkResult {
        all_scheduled_domains_resolved: resolved,
        document,
    })
}
