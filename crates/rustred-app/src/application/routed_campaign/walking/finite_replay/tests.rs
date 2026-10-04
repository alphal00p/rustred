use super::*;

#[test]
fn finite_replay_late_cancel_prevents_resource_fallback_without_masking_hard_error() {
    let resource = CandidateRoutedCampaignFailure::Trace(CandidateRoutedError::ResourceLimit {
        resource: "test nodes",
        requested: 2,
        limit: 1,
    });
    assert_eq!(
        failure_disposition(&resource, false),
        FailureDisposition::Decline
    );
    assert_eq!(
        failure_disposition(&resource, true),
        FailureDisposition::Cancelled
    );
    let hard = CandidateRoutedCampaignFailure::Trace(CandidateRoutedError::Candidate(
        rustred::solver::CandidateReductionError::InvalidInput(
            "hard source/context failure".into(),
        ),
    ));
    assert_eq!(failure_disposition(&hard, false), FailureDisposition::Hard);
    assert_eq!(failure_disposition(&hard, true), FailureDisposition::Hard);
    assert_eq!(
        failure_disposition(&CandidateRoutedCampaignFailure::WorkerPanicked, true),
        FailureDisposition::Hard
    );
}

#[test]
fn finite_replay_preparation_maps_only_explicit_trace_aggregate_policy() {
    let mut request = OwnerDomainWalkRequest::new(crate::OwnerDomainMatchRequest::new(
        "{}".into(),
        "{}".into(),
    ));
    let mut online = super::super::RoutedCampaignRequest::new(String::new(), String::new());
    online.reduction_limits.max_rule_applications = 7;
    let original = online.clone();
    configure_load(&request, &mut online);
    assert_eq!(online.trace_limits, original.trace_limits);
    assert_eq!(
        format!("{:?}", online.reduction_limits),
        format!("{:?}", original.reduction_limits)
    );
    request.finite_replay = Some(OwnerDomainWalkFiniteReplayLimits {
        max_nodes: 1_000_000,
        max_rule_applications: 1_000_000,
        max_transport_calls: 1_000_000,
        max_transport_operations: 64_000_000,
        max_transport_endpoints: 4_000_000,
        max_coalescing_additions: 16_000_000,
        max_positive_layers: 64,
        max_seed_points: 1024,
        max_seed_bytes: 1024 * 1024,
    });
    let mut cold = original.clone();
    configure_load(&request, &mut online);
    configure_load(&request, &mut cold);
    assert_eq!(online.trace_limits, cold.trace_limits);
    assert_eq!(online.trace_limits.max_input_targets, 1_000_000);
    assert_eq!(online.trace_limits.max_unique_nodes, 1_000_000);
    assert_eq!(online.trace_limits.max_transport_calls, 1_000_000);
    assert_eq!(online.trace_limits.max_transport_operations, 64_000_000);
    assert_eq!(online.trace_limits.max_transport_endpoints, 4_000_000);
    assert_eq!(
        online.trace_limits.expansion,
        original.trace_limits.expansion
    );
    // The core intersects attempt allowances with these admitted reduction
    // limits; app trace mapping cannot enlarge the caller's formula limits.
    assert_eq!(online.reduction_limits.max_rule_applications, 7);
    assert_eq!(
        format!("{:?}", online.reduction_limits),
        format!("{:?}", original.reduction_limits)
    );
}
