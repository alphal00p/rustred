//! Pure numeric diagnostics. No admission, CAS, allocation proportional to caps,
//! or proof authority. The same projection describes preflight and actual work.
use super::*;
use rustred::{reduction::ReductionLimits, solver::RoutedCandidateLimits};
#[cfg(test)]
mod tests;

pub(super) fn summary(
    requested: OwnerDomainWalkFiniteReplayLimits,
    admitted: RoutedCandidateLimits,
    reduction: ReductionLimits,
) -> Value {
    let input = admitted.max_input_targets.min(requested.max_nodes);
    let nodes = admitted.max_unique_nodes.min(requested.max_nodes);
    let pending = reduction.max_pending_frames.min(requested.max_nodes);
    json!({
        "requested": requested,
        "admitted": {
            "max_input_targets": admitted.max_input_targets,
            "max_unique_nodes": admitted.max_unique_nodes,
            "max_pending_frames": reduction.max_pending_frames,
            "max_rule_applications": reduction.max_rule_applications,
            "max_transport_calls": admitted.max_transport_calls,
            "max_transport_operations": admitted.max_transport_operations,
            "max_transport_endpoints": admitted.max_transport_endpoints,
            "max_coalescing_additions": reduction.max_coalescing_additions
        },
        "effective": {
            "max_input_targets": input,
            "max_unique_nodes": nodes,
            "max_pending_frames": pending,
            "max_rule_applications": reduction.max_rule_applications.min(requested.max_rule_applications),
            "max_transport_calls": admitted.max_transport_calls.min(requested.max_transport_calls),
            "max_transport_operations": admitted.max_transport_operations.min(requested.max_transport_operations),
            "max_transport_endpoints": admitted.max_transport_endpoints.min(requested.max_transport_endpoints),
            "max_coalescing_additions": reduction.max_coalescing_additions.min(requested.max_coalescing_additions),
            "max_positive_layers": requested.max_positive_layers,
            "max_seed_points": requested.max_seed_points.min(input).min(nodes).min(pending),
            "max_seed_bytes": requested.max_seed_bytes
        },
        "scope": "numeric request allowances only; no input/owner admission or completeness proof; native per-formula limits remain authoritative"
    })
}

pub(super) fn preparation(request: &OwnerDomainWalkRequest) -> Option<Value> {
    let limits = request.finite_replay?;
    let mut load = super::super::RoutedCampaignRequest::new(String::new(), String::new());
    load.reduction_limits = request.matching.reduction_limits;
    configure_load(request, &mut load);
    Some(summary(limits, load.trace_limits, load.reduction_limits))
}

/// Preserve native numeric refusal data without parsing a Display string.
pub(super) fn refusal(reason: &CandidateRoutedCampaignFailure) -> Option<Value> {
    match reason {
        CandidateRoutedCampaignFailure::Trace(CandidateRoutedError::ResourceLimit {
            resource,
            requested,
            limit,
        }) => Some(
            json!({"kind":"routed_resource_limit","resource":resource,"requested":requested,"limit":limit}),
        ),
        CandidateRoutedCampaignFailure::Trace(CandidateRoutedError::Transport(
            rustred::sector::symmetry::integral_transport::Error::Expansion(
                rustred::sector::symmetry::integral_transport::ExpansionError::ResourceLimit {
                    resource,
                    requested,
                    limit,
                },
            ),
        )) => Some(
            json!({"kind":"transport_expansion_resource_limit","resource":resource,"requested":requested,"limit":limit}),
        ),
        _ => None,
    }
}
