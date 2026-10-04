use super::*;
use crate::{OwnerDomainMatchRequest, OwnerDomainWalkFiniteReplayLimits};

fn request() -> OwnerDomainWalkRequest {
    let mut request =
        OwnerDomainWalkRequest::new(OwnerDomainMatchRequest::new("{}".into(), "{}".into()));
    request.publication_policy = OwnerDomainWalkPublicationPolicy::Epoch;
    request.checkpoint = Some(OwnerDomainWalkCheckpointOptions::new("unused-replay-test"));
    request
}

fn limits() -> OwnerDomainWalkFiniteReplayLimits {
    OwnerDomainWalkFiniteReplayLimits {
        max_nodes: 101,
        max_rule_applications: 102,
        max_transport_calls: 103,
        max_transport_operations: 104,
        max_transport_endpoints: 105,
        max_coalescing_additions: 106,
        max_positive_layers: 107,
        max_seed_points: 108,
        max_seed_bytes: 109,
    }
}

#[test]
fn finite_replay_bound_version_and_every_allowance_change_identity() {
    let mut request = request();
    let legacy = (binding(&request), epoch_request_binding(&request));
    assert!(binding_value(&request).get("finite_replay").is_none());
    request.finite_replay = Some(limits());
    let enabled = (binding(&request), epoch_request_binding(&request));
    assert_ne!(legacy.0, enabled.0);
    assert_ne!(legacy.1, enabled.1);
    let value = binding_value(&request);
    assert_eq!(
        value["finite_replay"]["version"],
        crate::OWNER_DOMAIN_WALK_FINITE_REPLAY_VERSION
    );
    assert_eq!(
        value["finite_replay"]["scope"],
        "whole_initial_id0_finite_domain"
    );
    for key in [
        "max_nodes",
        "max_rule_applications",
        "max_transport_calls",
        "max_transport_operations",
        "max_transport_endpoints",
        "max_coalescing_additions",
        "max_positive_layers",
        "max_seed_points",
        "max_seed_bytes",
    ] {
        let mut changed = serde_json::to_value(limits()).unwrap();
        changed[key] = json!(changed[key].as_u64().unwrap() + 1);
        request.finite_replay = Some(serde_json::from_value(changed).unwrap());
        assert_ne!(binding(&request), enabled.0, "unbound {key}");
        assert_ne!(epoch_request_binding(&request), enabled.1, "unbound {key}");
    }
    request.finite_replay = None;
    assert_eq!((binding(&request), epoch_request_binding(&request)), legacy);
}

#[test]
fn finite_replay_rust_api_admission_is_fresh_checkpoint_only() {
    let mut request = request();
    request.finite_replay = Some(limits());
    assert!(request.validate_finite_replay().is_ok());
    request.checkpoint.as_mut().unwrap().resume = true;
    assert!(request.validate_finite_replay().is_err());
    request.checkpoint = None;
    assert!(request.validate_finite_replay().is_err());
    request.checkpoint = Some(OwnerDomainWalkCheckpointOptions::new("unused-replay-test"));
    request.publication_policy = OwnerDomainWalkPublicationPolicy::Ready;
    assert!(request.validate_finite_replay().is_err());
    request.publication_policy = OwnerDomainWalkPublicationPolicy::Epoch;
    request.g2_activate_on_resume = true;
    assert!(request.validate_finite_replay().is_err());
    request.finite_replay = None;
    assert!(
        request.validate_finite_replay().is_ok(),
        "must not change flag-off admission"
    );
}

#[test]
fn finite_replay_limits_decode_rejects_missing_and_unknown_fields() {
    let value = serde_json::to_value(limits()).unwrap();
    let mut missing = value.clone();
    missing.as_object_mut().unwrap().remove("max_nodes");
    assert!(serde_json::from_value::<OwnerDomainWalkFiniteReplayLimits>(missing).is_err());
    let mut extra = value;
    extra["skip_replay"] = json!(true);
    assert!(serde_json::from_value::<OwnerDomainWalkFiniteReplayLimits>(extra).is_err());
}

#[test]
fn finite_replay_keeps_other_domain_reuse_policies_available() {
    let mut request = request();
    request.finite_replay = Some(limits());
    request.g2_residual_anchors = super::super::OwnerDomainWalkG2ResidualAnchors::Union;
    request.reuse_initial_d_bands = true;
    assert!(request.validate_finite_replay().is_ok());
}

#[test]
fn finite_replay_existing_reduction_aggregate_fields_remain_bound_in_both_formats() {
    let mut request = request();
    request.finite_replay = Some(limits());
    let original = (binding(&request), epoch_request_binding(&request));
    for index in 0..3 {
        let mut changed = request.clone();
        match index {
            0 => changed.matching.reduction_limits.max_rule_applications += 1,
            1 => changed.matching.reduction_limits.max_pending_frames += 1,
            _ => changed.matching.reduction_limits.max_coalescing_additions += 1,
        }
        assert_ne!(binding(&changed), original.0);
        assert_ne!(epoch_request_binding(&changed), original.1);
    }
}
