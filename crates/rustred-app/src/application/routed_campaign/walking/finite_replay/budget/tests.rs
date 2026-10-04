use super::*;

fn caps() -> OwnerDomainWalkFiniteReplayLimits {
    OwnerDomainWalkFiniteReplayLimits {
        max_nodes: 100,
        max_rule_applications: 100,
        max_transport_calls: 100,
        max_transport_operations: 100,
        max_transport_endpoints: 100,
        max_coalescing_additions: 100,
        max_positive_layers: 64,
        max_seed_points: 50,
        max_seed_bytes: 1024,
    }
}

#[test]
fn finite_replay_budget_summary_intersects_both_tiers_and_all_seed_caps() {
    let mut request = caps();
    let mut trace = RoutedCandidateLimits::default();
    let mut reduction = ReductionLimits::default();
    reduction.max_rule_applications = 7;
    reduction.max_pending_frames = 11;
    reduction.max_coalescing_additions = 13;
    trace.max_input_targets = 17;
    trace.max_unique_nodes = 19;
    trace.max_transport_calls = 23;
    trace.max_transport_operations = 29;
    trace.max_transport_endpoints = 31;
    let admitted = summary(request, trace, reduction);
    assert_eq!(admitted["effective"]["max_rule_applications"], 7);
    assert_eq!(admitted["effective"]["max_seed_points"], 11);
    for (name, value) in [
        ("max_pending_frames", 11),
        ("max_coalescing_additions", 13),
        ("max_input_targets", 17),
        ("max_unique_nodes", 19),
        ("max_transport_calls", 23),
        ("max_transport_operations", 29),
        ("max_transport_endpoints", 31),
    ] {
        assert_eq!(admitted["effective"][name], value);
    }
    request.max_rule_applications = 3;
    request.max_nodes = 5;
    request.max_coalescing_additions = 2;
    let smaller = summary(request, trace, reduction);
    assert_eq!(smaller["effective"]["max_rule_applications"], 3);
    assert_eq!(smaller["effective"]["max_seed_points"], 5);
    assert_eq!(smaller["effective"]["max_pending_frames"], 5);
    assert_eq!(smaller["effective"]["max_coalescing_additions"], 2);
    request.max_nodes = 0;
    assert_eq!(
        summary(request, trace, reduction)["effective"]["max_seed_points"],
        0
    );
}

#[test]
fn finite_replay_budget_preparation_is_pure_and_matches_actual_load_projection() {
    let mut request = OwnerDomainWalkRequest::new(crate::OwnerDomainMatchRequest::new(
        "{}".into(),
        "{}".into(),
    ));
    assert!(preparation(&request).is_none());
    request.finite_replay = Some(caps());
    request.matching.reduction_limits.max_rule_applications = 7;
    let before = format!("{:?}", request.matching.reduction_limits);
    let mut online = crate::RoutedCampaignRequest::new(String::new(), String::new());
    online.reduction_limits = request.matching.reduction_limits;
    let mut cold = online.clone();
    configure_load(&request, &mut online);
    configure_load(&request, &mut cold);
    let expected = preparation(&request).unwrap();
    assert_eq!(
        expected,
        summary(caps(), online.trace_limits, online.reduction_limits)
    );
    assert_eq!(
        expected,
        summary(caps(), cold.trace_limits, cold.reduction_limits)
    );
    assert_eq!(before, format!("{:?}", request.matching.reduction_limits));
    assert_eq!(before, format!("{:?}", online.reduction_limits));
    assert_eq!(
        online.trace_limits.expansion,
        RoutedCandidateLimits::default().expansion
    );
}

#[test]
fn finite_replay_typed_budget_refusal_retains_resource_and_native_tier() {
    for resource in [
        "rule attempts",
        "pending frames",
        "conservative coalescing reservation",
    ] {
        let reason = CandidateRoutedCampaignFailure::Trace(CandidateRoutedError::ResourceLimit {
            resource,
            requested: 8,
            limit: 7,
        });
        assert_eq!(
            refusal(&reason).unwrap(),
            json!({"kind":"routed_resource_limit","resource":resource,"requested":8,"limit":7})
        );
    }
    let reason = CandidateRoutedCampaignFailure::Trace(CandidateRoutedError::Transport(
        rustred::sector::symmetry::integral_transport::Error::Expansion(
            rustred::sector::symmetry::integral_transport::ExpansionError::ResourceLimit {
                resource: "native expansion terms",
                requested: 8,
                limit: 7,
            },
        ),
    ));
    assert_eq!(
        refusal(&reason).unwrap()["kind"],
        "transport_expansion_resource_limit"
    );
    assert!(refusal(&CandidateRoutedCampaignFailure::WorkerPanicked).is_none());
}
