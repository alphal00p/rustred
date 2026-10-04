//! Policy identity tests, not assertions of an admitted physical program.
use super::super::{OwnerDomainMatchRequest, queue::Queue};
use super::test_support::Fixture;
use super::*;

fn request(selector: Option<Value>) -> OwnerDomainWalkRequest {
    let mut preference = json!({"owner_mask":"1","path":"same.rrbin","bytes":1,
        "residual_policy":"defer-to-baseline"});
    if let Some(value) = selector {
        preference["rule_ordinals"] = value;
    }
    OwnerDomainWalkRequest::new(OwnerDomainMatchRequest::new(
        json!({"preferred_owner_programs":[preference]}).to_string(),
        "queries".into(),
    ))
}
fn digest(value: &Value) -> String {
    blake3::hash(value.to_string().as_bytes())
        .to_hex()
        .to_string()
}

// Independently spelled historical default CP5 value, before opt-in markers.
fn legacy_cp5_value(r: &OwnerDomainWalkRequest) -> Value {
    json!({"selection":r.matching.selection_json,"queries":r.matching.queries_json,
        "limits":super::super::limits_json(r),"reduction":format!("{:?}",r.matching.reduction_limits),
        "workers":r.workers,"inspection_workers":r.inspection_workers,
        "publication":format!("{:?}",r.publication_policy),"scheduling":format!("{:?}",r.scheduling_policy),
        "reuse_initial_d_bands":r.reuse_initial_d_bands,"max_domains":r.max_domains,
        "max_events":r.max_events,"max_frontiers":r.max_frontiers,
        "max_containment_checks":r.max_containment_checks,"route_domain_overcover":r.route_domain_overcover,
        "route_joint_source_support_pruning":r.route_joint_source_support_pruning,
        "max_route_masks":r.max_route_masks,"subdivision":r.apply_subdivision,
        "max_queries":r.matching.max_queries,"max_query_bytes":r.matching.max_query_bytes})
}

// Independently spelled historical default CP6 value: this has no subset marker.
fn legacy_epoch_value(r: &OwnerDomainWalkRequest) -> Value {
    json!({"selection":r.matching.selection_json,"queries":r.matching.queries_json,
        "limits":super::super::limits_json(r),"reduction":format!("{:?}",r.matching.reduction_limits),
        "publication":"epoch","walk_semantics_version":super::super::epoch::EPOCH_WALK_SEMANTICS_VERSION,
        "reuse_initial_d_bands":r.reuse_initial_d_bands,"route_domain_overcover":r.route_domain_overcover,
        "route_joint_source_support_pruning":r.route_joint_source_support_pruning,
        "max_route_masks":r.max_route_masks,"subdivision":r.apply_subdivision,
        "max_queries":r.matching.max_queries,"max_query_bytes":r.matching.max_query_bytes,
        "frontier_policy":r.frontier_policy.name()})
}

#[test]
fn preferred_subset_default_binding_is_unchanged_and_active_marker_excludes_legacy() {
    for selector in [None, Some(Value::Null)] {
        let r = request(selector);
        let old = legacy_cp5_value(&r);
        assert_eq!(binding_value(&r), old);
        assert_eq!(binding(&r), digest(&old));
        assert_eq!(epoch_request_binding(&r), digest(&legacy_epoch_value(&r)));
    }
    for selector in [json!([]), json!([0]), json!([2, 110])] {
        let r = request(Some(selector));
        let mut old = binding_value(&r);
        assert_eq!(
            old.as_object_mut()
                .unwrap()
                .remove("preferred_rule_subset_policy"),
            Some(json!(
                crate::application::routed_campaign::input::PREFERRED_RULE_SUBSET_POLICY
            ))
        );
        // Identical raw JSON and payload names, but old binaries ignored this field.
        assert_ne!(binding(&r), digest(&old));
        old["preferred_rule_subset_policy"] = json!("saved-ordinal-subset-v0");
        assert_ne!(binding(&r), digest(&old));
        let mut epoch = legacy_epoch_value(&r);
        assert_ne!(epoch_request_binding(&r), digest(&epoch));
        epoch["preferred_rule_subset_policy"] =
            json!(crate::application::routed_campaign::input::PREFERRED_RULE_SUBSET_POLICY);
        assert_eq!(epoch_request_binding(&r), digest(&epoch));
        epoch["preferred_rule_subset_policy"] = json!("saved-ordinal-subset-v0");
        assert_ne!(epoch_request_binding(&r), digest(&epoch));
    }
}

#[test]
fn preferred_subset_cp5_same_policy_restores_changed_or_legacy_policy_refuses() {
    let state = State::<1>::new(Queue::new(8, None), 0, None);
    let mut fixture = Fixture::save_with(&state, request(Some(json!([110]))), &[], &[]);
    fixture.resume::<1>().unwrap();
    let original = fixture.request.matching.selection_json.clone();
    fixture.request.matching.selection_json = request(Some(json!([109]))).matching.selection_json;
    assert!(fixture.resume::<1>().is_err());
    fixture.request.matching.selection_json = original;
    fixture.resume::<1>().unwrap();
    let mut manifest = fixture.manifest();
    let mut legacy = binding_value(&fixture.request);
    legacy
        .as_object_mut()
        .unwrap()
        .remove("preferred_rule_subset_policy");
    manifest["request"] = digest(&legacy).into();
    fixture.write_manifest(&manifest);
    assert!(fixture.resume::<1>().is_err());
}
