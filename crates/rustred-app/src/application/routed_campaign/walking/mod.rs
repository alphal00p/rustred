//! Shared symbolic successor work discovery over one immutable owner snapshot.
//! Optional admitted-route overcovers share dependency work without expanding
//! numerator polynomials. This is not a family-closure certificate.
mod diagnostics;
mod queue;
mod routing;

use std::ops::ControlFlow;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use rustred::solver::{
    OwnerAppliedEvent, OwnerAppliedLimits, OwnerAppliedNonzero, OwnerAppliedStats,
    OwnerDomainMatchDisposition,
};
use serde_json::{Value, json};

use super::{OwnerDomainMatchRequest, RoutedCampaignRequest, input, matching, prepare};
use crate::AppError;
use diagnostics::{OptionalCounts, OptionalRefusals};
use queue::{Domain, Phase, Queue};

#[derive(Clone, Debug)]
pub struct OwnerDomainWalkRequest {
    /// Shares input/load policy, max_queries and native match_limits only.
    /// max_total_pieces is a local-match report limit; this walk uses max_events.
    pub matching: OwnerDomainMatchRequest,
    /// Per-domain native allowances. `matching.match_limits` is authoritative
    /// for the nested ordered matcher, overriding this field's matching member.
    pub applied_limits: OwnerAppliedLimits,
    pub max_domains: usize,
    pub max_events: usize,
    pub max_containment_checks: usize,
    /// Request conservative rank-preserving route images, not polynomial
    /// expansion or evidence that every enclosed point is reached.
    pub route_domain_overcover: bool,
    pub max_route_masks: usize,
}
impl OwnerDomainWalkRequest {
    pub fn new(matching: OwnerDomainMatchRequest) -> Self {
        Self {
            matching,
            applied_limits: Default::default(),
            max_domains: 100_000,
            max_events: 1_000_000,
            max_containment_checks: 10_000_000,
            route_domain_overcover: false,
            max_route_masks: 100_000,
        }
    }
}

#[derive(Clone, Debug)]
pub struct OwnerDomainWalkResult {
    /// All admitted domains inspected without unresolved local or routing work.
    /// This does not certify provenance, global order compatibility or closure.
    pub all_scheduled_domains_resolved: bool,
    pub document: Value,
}
impl OwnerDomainWalkResult {
    pub(crate) fn completion_progress(document: &Value) -> Value {
        let mut out = json!({"event":"finished", "operation":"owner_domain_walk",
            "full_result_in_output_document":true, "family_closure_claim":false});
        for key in [
            "status",
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
        || !(1..=10_000_000).contains(&request.max_events)
        || request.max_containment_checks == 0
        || request.max_route_masks == 0
    {
        return Err(AppError::input("invalid symbolic worklist allowances"));
    }
    rustred::campaign::ParallelExecution::preflight_requested_core_budget(1)
        .map_err(|error| AppError::input(error.to_string()))?;
    let (selection, arity, limits) = input::Selection::parse(&request.matching.selection_json)?;
    let queries = matching::input::parse(
        &request.matching.queries_json,
        arity,
        request.matching.max_queries,
    )?;
    observer(
        json!({"event":"admitted", "operation":"owner_domain_walk", "arity":arity,
        "input_domains":queries.len(), "max_domains":request.max_domains, "max_events":request.max_events,
        "route_domain_overcover":request.route_domain_overcover, "max_route_masks":request.max_route_masks,
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

fn stats_json(stats: OwnerAppliedStats) -> Value {
    json!({"selected_pieces":stats.selected_pieces, "term_visits":stats.term_visits,
        "shift_groups":stats.shift_groups, "boundary_cells":stats.boundary_cells,
        "sign_splits":stats.sign_splits, "native_operations":stats.native_operations,
        "optional_coefficient_refusals":stats.optional_coefficient_refusals,
        "optional_original_refusals":stats.optional_original_refusals,
        "optional_coalesced_refusals":stats.optional_coalesced_refusals,
        "coalescing_additions":stats.coalescing_additions, "events":stats.events,
        "successors":stats.successors, "conditional_successors":stats.conditional_successors,
        "problems":stats.problems, "zero_terms":stats.zero_terms,
        "cancelled_groups":stats.cancelled_groups, "zero_sector_groups":stats.zero_sector_groups,
        "matching":{"rules":stats.matching.rules, "terminal_checks":stats.matching.terminal_checks,
            "predicates":stats.matching.predicates, "pieces":stats.matching.pieces,
            "cells":stats.matching.cells, "split_operations":stats.matching.split_operations,
            "coordinate_cells":stats.matching.coordinate_cells, "rank_empty_cells":stats.matching.rank_empty_cells,
            "refinement_cells":stats.matching.refinement_cells, "refinement_steps":stats.matching.refinement_steps}})
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
    let mut records = Vec::new();
    let mut inputs = Vec::new();
    let mut input_frontiers = Vec::new();
    let mut error = reducer
        .is_none()
        .then(|| "cancelled during preparation".to_owned());
    let mut events = 0usize;
    let mut successors = 0usize;
    let mut conditional = 0usize;
    let mut optional_counts = OptionalCounts::default();
    let mut frontiers = 0usize;
    let mut completed = 0usize;
    let mut routed_domains = 0usize;
    let mut route_masks = 0usize;
    let mut applied_limits = request.applied_limits;
    applied_limits.matching = request.matching.match_limits;
    if error.is_none() {
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
                    .as_ref()
                    .expect("prepared")
                    .programs()
                    .owner_sectors()
                    .any(|owner| owner == &domain.owner)
            {
                if reducer
                    .as_ref()
                    .expect("prepared")
                    .domain_routing_requires_source_conditions()
                {
                    frontiers += 1;
                    input_frontiers.push(
                        json!({"id":query.id, "kind":"initial_route_source_validity_obligation",
                        "owner":mask(&domain.owner), "lower":domain.lower, "upper":domain.upper,
                        "rank":domain.rank, "reached_missing_rule_claim":false}),
                    );
                    inputs.push(
                        json!({"id":query.id, "domain":null, "source_validity_unresolved":true}),
                    );
                    continue;
                }
                Domain::route_cover(domain.owner, domain.rank)
            } else {
                domain
            };
            match queue.admit(domain) {
                Ok((id, _)) => inputs.push(json!({"id":query.id, "domain":id})),
                Err(problem) => {
                    error = Some(problem.to_owned());
                    break;
                }
            }
        }
    }
    while error.is_none() && queue.next < queue.domains.len() {
        if cancellation.load(Ordering::Acquire) {
            error = Some("cancelled".into());
            break;
        }
        let id = queue.next;
        let domain = queue.domains[id].clone();
        observer(
            json!({"event":"domain_started", "operation":"owner_domain_walk", "id":id,
            "owner":mask(&domain.owner), "scheduled_nodes":queue.domains.len(),
            "phase":format!("{:?}",domain.phase), "routed_domains":routed_domains, "route_masks":route_masks,
            "max_scheduled_finite_rank":queue.max_finite_rank, "unbounded_rank_domains":queue.unbounded_rank_domains,
            "completed_nodes":completed, "queued_nodes":queue.domains.len()-queue.next,
            "deduplication_hits":queue.deduplicated, "successors":successors,
            "exact_domain_hits":queue.exact_hits, "full_orthant_hits":queue.orthant_hits,
            "containment_checks":queue.containment_checks,
            "conditional_successors":conditional, "frontiers":frontiers, "events":events}),
        );
        let mut details = Vec::new();
        let mut optional_refusals = OptionalRefusals::default();
        let mut node_error = None;
        let node_started = Instant::now();
        if domain.phase == Phase::Route {
            let inspected = routing::inspect(
                reducer.as_ref().expect("prepared"),
                &domain,
                &mut queue,
                request,
                cancellation,
                &mut events,
                &mut frontiers,
                &mut route_masks,
                |mut event| {
                    event["completed_nodes"] = json!(completed);
                    event["successors"] = json!(successors);
                    event["conditional_successors"] = json!(conditional);
                    event["routed_domains"] = json!(routed_domains);
                    observer(event);
                },
            );
            error = inspected.error;
            completed += usize::from(error.is_none());
            routed_domains += 1;
            records.push(
                json!({"id":id, "phase":"Route", "owner":mask(&domain.owner),
                "lower":domain.lower, "upper":domain.upper, "rank":domain.rank,
                "conservative_route_overcover":true, "local_inspection_finished":error.is_none(),
                "stats":inspected.stats, "seconds":node_started.elapsed().as_secs_f64(),
                "frontiers":inspected.frontiers, "error":error}),
            );
            queue.next += 1;
            continue;
        }
        let result = reducer.as_ref().expect("prepared").programs().visit_owner_applied_successors(
            domain.owner, &domain.lower, &domain.upper, domain.rank,
            applied_limits, cancellation, |event| {
                if events == request.max_events {
                    node_error = Some("aggregate successor event allowance".to_owned());
                    return ControlFlow::Break(());
                }
                events += 1;
                match event {
                    OwnerAppliedEvent::Classified(piece) => match piece.disposition() {
                        OwnerDomainMatchDisposition::SelectedRule { .. }
                        | OwnerDomainMatchDisposition::Terminal { .. }
                        | OwnerDomainMatchDisposition::ExactZeroSector => {},
                        other => {
                            frontiers += 1;
                            details.push(json!({"kind":"local_dispatch_frontier", "disposition":format!("{other:?}"),
                                "lower":piece.lower(), "upper":piece.upper(), "rank":piece.max_numerator_rank(),
                                "reached_missing_rule_claim":false}));
                        }
                    },
                    OwnerAppliedEvent::OptionalCoefficientRefusal {
                        source, source_lower, source_upper, shift, original_term_ordinal, failure,
                    } => {
                        if let Err(problem) = optional_refusals.record(
                            source.disposition(), source.max_numerator_rank(), source_lower,
                            source_upper, shift, original_term_ordinal, failure,
                        ) {
                            node_error = Some(problem.to_owned());
                            return ControlFlow::Break(());
                        }
                    },
                    OwnerAppliedEvent::Successor(child) => {
                        successors += 1;
                        conditional += usize::from(child.coefficient_nonzero == OwnerAppliedNonzero::Conditional);
                        if child.has_installed_target_owner {
                            let target = Domain { phase: Phase::Apply, owner: *child.target_sector, lower: child.target_lower.to_vec(),
                                upper: child.target_upper.to_vec(), rank: child.target_rank_limit };
                            if let Err(problem) = queue.admit(target) {
                                node_error = Some(problem.to_owned());
                                return ControlFlow::Break(());
                            }
                        } else if request.route_domain_overcover {
                            // Route images depend on support and actual rank,
                            // not separate concrete numerator assignments.
                            if let Err(problem) = queue.admit(Domain::route_cover(*child.target_sector, child.target_rank_limit)) {
                                node_error = Some(problem.to_owned());
                                return ControlFlow::Break(());
                            }
                        } else {
                            frontiers += 1;
                            details.push(json!({"kind":"routing_frontier", "target_owner":mask(child.target_sector),
                                "target_lower":child.target_lower, "target_upper":child.target_upper,
                                "target_rank":child.target_rank_limit, "source_lower":child.source_lower,
                                "source_upper":child.source_upper, "shift":child.shift.as_slice(),
                                "coefficient_nonzero":format!("{:?}",child.coefficient_nonzero),
                                "selected_rule":format!("{:?}",child.source.disposition()),
                                "reached_missing_rule_claim":false}));
                        }
                    },
                    OwnerAppliedEvent::Problem(problem) => {
                        frontiers += 1;
                        details.push(json!({"kind":"rhs_obligation", "problem":format!("{:?}",problem.kind),
                            "source_lower":problem.source_lower, "source_upper":problem.source_upper,
                            "shift":problem.shift.as_slice(), "original_term":problem.original_term_ordinal,
                            "coefficient_nonzero":format!("{:?}",problem.coefficient_nonzero),
                            "selected_rule":format!("{:?}",problem.source.disposition()),
                            "reached_missing_rule_claim":false}));
                    },
                    OwnerAppliedEvent::RuleFinished { .. } => {},
                }
                if events.is_multiple_of(128) {
                    observer(json!({"event":"domain_progress", "operation":"owner_domain_walk", "id":id,
                        "scheduled_nodes":queue.domains.len(), "completed_nodes":completed,
                        "queued_nodes":queue.domains.len()-queue.next, "deduplication_hits":queue.deduplicated,
                        "exact_domain_hits":queue.exact_hits, "full_orthant_hits":queue.orthant_hits,
                        "containment_checks":queue.containment_checks,
                        "successors":successors, "conditional_successors":conditional,
                        "routed_domains":routed_domains, "route_masks":route_masks,
                        "max_scheduled_finite_rank":queue.max_finite_rank, "unbounded_rank_domains":queue.unbounded_rank_domains,
                        "frontiers":frontiers, "events":events}));
                }
                ControlFlow::Continue(())
            });
        let (stats, native_error) = match result {
            Ok(stats) => (stats, None),
            Err(e) => (e.stats, Some(format!("{:?}", e.failure))),
        };
        error = node_error.or(native_error);
        if let Err(problem) = optional_counts.add(stats) {
            // Keep the native per-domain counters even if their aggregate
            // cannot be represented; an incomplete report must not wrap.
            error.get_or_insert_with(|| problem.to_owned());
        }
        completed += usize::from(error.is_none());
        let provenance_truncated = optional_refusals.truncated(stats);
        records.push(
            json!({"id":id, "phase":"Apply", "owner":mask(&domain.owner), "lower":domain.lower,
            "upper":domain.upper, "rank":domain.rank, "local_inspection_finished":error.is_none(),
            "stats":stats_json(stats), "seconds":node_started.elapsed().as_secs_f64(),
            "optional_refusals":optional_refusals.records,
            "optional_refusal_provenance_scope":"first_per_phase_per_query",
            "optional_refusal_provenance_truncated":provenance_truncated,
            "frontiers":details, "error":error}),
        );
        queue.next += 1;
    }
    let exhausted = error.is_none() && queue.next == queue.domains.len();
    let resolved = exhausted && frontiers == 0;
    let document = json!({"schema":"rustred.owner-domain-walk.json.v1",
        "status":if resolved {"locally_resolved"} else {"incomplete"},
        "all_scheduled_domains_resolved":resolved, "recursive_worklist_exhausted":exhausted,
        "family_closure_claim":false, "ibp_generation":false, "routing_expanded":false,
        "route_domain_overcover":request.route_domain_overcover,
        "routed_domains":routed_domains, "route_masks":route_masks,
        "max_scheduled_finite_rank":queue.max_finite_rank, "unbounded_rank_domains":queue.unbounded_rank_domains,
        "independent_certification":false, "resume_supported":false,
        "conditional_successors_use_conservative_domain_overcover":true,
        "scheduled_nodes":queue.domains.len(), "completed_nodes":completed,
        "queued_nodes":queue.domains.len().saturating_sub(queue.next),
        "processed_nodes":queue.next, "failed_nodes":queue.next.saturating_sub(completed),
        "deduplication_hits":queue.deduplicated, "containment_checks":queue.containment_checks,
        "exact_domain_hits":queue.exact_hits, "full_orthant_hits":queue.orthant_hits,
        "successors":successors, "conditional_successors":conditional,
        "optional_coefficient_refusals":optional_counts.total,
        "optional_original_refusals":optional_counts.original,
        "optional_coalesced_refusals":optional_counts.coalesced,
        "frontiers":frontiers, "events":events, "inputs":inputs, "domains":records,
        "input_frontiers":input_frontiers,
        "error":error, "prepared_seconds":prepared,
        "traversal_seconds":started.elapsed().as_secs_f64()-prepared,
        "elapsed_seconds":started.elapsed().as_secs_f64()});
    observer(OwnerDomainWalkResult::completion_progress(&document));
    Ok(OwnerDomainWalkResult {
        all_scheduled_domains_resolved: resolved,
        document,
    })
}
