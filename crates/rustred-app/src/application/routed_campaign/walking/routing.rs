//! Bounded conservative route requests, distinct from concrete expansion.
use std::ops::ControlFlow;
use std::sync::atomic::AtomicBool;

use rustred::solver::{
    CandidateDomainRouteEvent, CandidateDomainRouteLimits, RoutedCandidateReducer,
};
use serde_json::{Value, json};

use super::{
    OwnerDomainWalkRequest, mask,
    queue::{Domain, Phase, Queue},
};

pub(super) struct Inspection {
    pub stats: Value,
    pub frontiers: Vec<Value>,
    pub error: Option<String>,
}

fn reentry<const N: usize>(
    sector: [bool; N],
    rank: Option<u32>,
    conditions: bool,
) -> Result<Domain<N>, Value> {
    if conditions {
        Err(json!({"kind":"route_reentry_source_validity_obligation",
            "owner":mask(&sector), "rank":rank, "reached_missing_rule_claim":false}))
    } else {
        Ok(Domain::route_cover(sector, rank))
    }
}

pub(super) fn inspect<const N: usize>(
    reducer: &RoutedCandidateReducer<N>,
    domain: &Domain<N>,
    queue: &mut Queue<N>,
    request: &OwnerDomainWalkRequest,
    cancellation: &AtomicBool,
    events: &mut usize,
    frontier_count: &mut usize,
    route_masks: &mut usize,
    observer: impl Fn(Value),
) -> Inspection {
    let mut error = None;
    let mut frontiers = Vec::new();
    let reentry_requires_conditions = reducer.domain_routing_requires_source_conditions();
    let result = reducer.visit_domain_route_overcover(
        domain.owner,
        domain.rank,
        CandidateDomainRouteLimits {
            max_masks: request.max_route_masks,
            max_coordinate_cells: request.max_route_masks.saturating_mul(N).saturating_mul(2),
        },
        cancellation,
        |event| {
            if *events == request.max_events {
                error = Some("aggregate successor event allowance".to_owned());
                return ControlFlow::Break(());
            }
            *events += 1;
            let next = match event {
                CandidateDomainRouteEvent::Apply {
                    owner_sector,
                    cover,
                } => Some(Domain {
                    phase: Phase::Apply,
                    owner: owner_sector,
                    lower: vec![0; N],
                    upper: vec![None; N],
                    rank: cover.actual_rank,
                }),
                CandidateDomainRouteEvent::Route { sector, cover } => {
                    // Unlike a checked original RHS term, this conservative
                    // transport image has untested source conditions. A second
                    // permutation must not erase those obligations.
                    match reentry(sector, cover.actual_rank, reentry_requires_conditions) {
                        Ok(domain) => Some(domain),
                        Err(obligation) => {
                            *frontier_count += 1;
                            frontiers.push(obligation);
                            None
                        }
                    }
                }
                CandidateDomainRouteEvent::MissingRoute {
                    source_sector,
                    actual_rank,
                } => {
                    *frontier_count += 1;
                    frontiers.push(
                        json!({"kind":"missing_route_cover", "owner":mask(&source_sector),
                        "rank":actual_rank, "reached_missing_rule_claim":false}),
                    );
                    None
                }
                CandidateDomainRouteEvent::ZeroSector {
                    sector,
                    actual_rank,
                    source_conditions_required,
                } => {
                    if source_conditions_required {
                        *frontier_count += 1;
                        frontiers.push(
                            json!({"kind":"zero_source_validity_obligation", "owner":mask(&sector),
                            "rank":actual_rank, "reached_missing_rule_claim":false}),
                        );
                    }
                    None
                }
            };
            if let Some(next) = next {
                if let Err(problem) = queue.admit(next) {
                    error = Some(problem.to_owned());
                    return ControlFlow::Break(());
                }
            }
            if events.is_multiple_of(128) {
                observer(
                    json!({"event":"domain_progress", "operation":"owner_domain_walk",
                    "phase":"Route", "id":queue.next, "scheduled_nodes":queue.domains.len(),
                    "queued_nodes":queue.domains.len()-queue.next,
                    "max_scheduled_finite_rank":queue.max_finite_rank, "unbounded_rank_domains":queue.unbounded_rank_domains,
                    "deduplication_hits":queue.deduplicated, "frontiers":frontier_count,
                    "exact_domain_hits":queue.exact_hits, "full_orthant_hits":queue.orthant_hits,
                    "containment_checks":queue.containment_checks,
                    "events":events, "route_domain_overcover":true}),
                );
            }
            ControlFlow::Continue(())
        },
    );
    let (stats, native_error) = match result {
        Ok(stats) => (stats, None),
        Err(problem) => (problem.stats, Some(format!("{:?}", problem.failure))),
    };
    *route_masks += stats.masks_examined;
    Inspection {
        stats: json!({"masks_examined":stats.masks_examined, "events":stats.events,
            "apply_domains":stats.apply_domains, "route_domains":stats.route_domains,
            "zero_sectors":stats.zero_sectors, "missing_routes":stats.missing_routes,
            "coordinate_cells":stats.coordinate_cells}),
        frontiers,
        error: error.or(native_error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn route_generated_reentry_keeps_unchecked_source_validity_and_rank() {
        for rank in [Some(11), None] {
            let obligation = reentry([true, false], rank, true).unwrap_err();
            assert_eq!(
                obligation["kind"],
                "route_reentry_source_validity_obligation"
            );
            assert_eq!(obligation["rank"], json!(rank));
            assert_eq!(obligation["reached_missing_rule_claim"], false);
            let admitted = reentry([true, false], rank, false).unwrap();
            assert_eq!(admitted.phase, Phase::Route);
            assert_eq!(admitted.rank, rank);
            assert_eq!(admitted.upper, vec![None, None]);
        }
    }
}
