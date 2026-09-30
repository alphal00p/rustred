use super::*;

#[test]
fn result_escrow_is_explicit_bounded_and_request_bound() {
    let mut request =
        OwnerDomainWalkRequest::new(OwnerDomainMatchRequest::new("s".into(), "q".into()));
    let default = checkpoint::epoch_request_binding(&request);
    assert_eq!(request.epoch_result_escrow_jobs, 0);
    assert_eq!(request.epoch_result_escrow_bytes, None);
    assert!(request.validate_epoch_result_escrow().is_ok());
    request.epoch_result_escrow_bytes = Some(1024);
    assert!(request.validate_epoch_result_escrow().is_err());
    request.epoch_result_escrow_jobs = 8;
    assert!(request.validate_epoch_result_escrow().is_err());
    request.publication_policy = OwnerDomainWalkPublicationPolicy::Epoch;
    request.epoch_rolling = true;
    request.checkpoint = Some(OwnerDomainWalkCheckpointOptions::new("unused"));
    request.epoch_window = Some(32);
    assert_eq!(request.resolved_epoch_total_window(16), Ok(40));
    assert!(request.validate_epoch_result_escrow().is_ok());
    let enabled = checkpoint::epoch_request_binding(&request);
    request.epoch_result_escrow_bytes = Some(2048);
    assert_ne!(checkpoint::epoch_request_binding(&request), enabled);
    request.epoch_result_escrow_bytes = Some(0);
    assert!(request.validate_epoch_result_escrow().is_err());
    request.epoch_result_escrow_bytes = Some(1024);
    request.epoch_publication_order = OwnerDomainWalkEpochPublicationOrder::OldestReady;
    assert!(request.validate_epoch_result_escrow().is_err());
    request.epoch_publication_order = OwnerDomainWalkEpochPublicationOrder::OldestPrefix;
    for bad in [4065, usize::MAX] {
        request.epoch_result_escrow_jobs = bad;
        assert!(request.resolved_epoch_total_window(16).is_err());
    }
    request.epoch_result_escrow_jobs = 8;
    request.epoch_window = None;
    request.workers = 1;
    assert_eq!(request.resolved_epoch_window(16), Ok(1));
    assert_eq!(request.resolved_epoch_total_window(16), Ok(9));
    // The runtime must keep W1's effective cut at its base1, not declared9.
    let reset = OwnerDomainWalkRequest::new(OwnerDomainMatchRequest::new("s".into(), "q".into()));
    assert_eq!(checkpoint::epoch_request_binding(&reset), default);
}

#[test]
fn rolling_publication_cut_and_window_defaults_validation_and_binding() {
    use OwnerDomainWalkEpochPublicationOrder::{OldestPrefix, OldestReady};
    let mut request =
        OwnerDomainWalkRequest::new(OwnerDomainMatchRequest::new("s".into(), "q".into()));
    assert_eq!(request.epoch_publication_order, OldestPrefix);
    assert_eq!(request.epoch_cut_size, None);
    assert_eq!(request.epoch_window, None);
    for policy in [OldestPrefix, OldestReady] {
        assert_eq!(
            OwnerDomainWalkEpochPublicationOrder::parse(policy.name()),
            Some(policy)
        );
    }
    assert_eq!(
        OwnerDomainWalkEpochPublicationOrder::parse("oldest_ready"),
        None
    );
    request.publication_policy = OwnerDomainWalkPublicationPolicy::Epoch;
    request.checkpoint = Some(OwnerDomainWalkCheckpointOptions::new("unused"));
    request.epoch_rolling = true;
    let baseline = checkpoint::epoch_request_binding(&request);
    request.epoch_cut_size = Some(16);
    assert_eq!(checkpoint::epoch_request_binding(&request), baseline);
    request.epoch_window = Some(64);
    assert_eq!(
        checkpoint::epoch_request_binding(&request),
        baseline,
        "window bound in CP6 scalars, omission inherits"
    );
    assert_eq!(request.resolved_epoch_window(16), Ok(64));
    request.epoch_publication_order = OldestReady;
    assert_ne!(checkpoint::epoch_request_binding(&request), baseline);
    request.epoch_publication_order = OldestPrefix;
    request.epoch_cut_size = Some(8);
    assert_ne!(checkpoint::epoch_request_binding(&request), baseline);
    assert_eq!(request.effective_epoch_cut_size(), Ok(8));
    assert!(request.validate_epoch_inspector_lookup().is_ok());
    for invalid in [0, 4097] {
        request.epoch_cut_size = Some(invalid);
        assert!(request.validate_epoch_inspector_lookup().is_err());
        request.epoch_cut_size = Some(16);
        request.epoch_window = Some(invalid);
        assert!(request.validate_epoch_inspector_lookup().is_err());
    }
    request.epoch_window = Some(15);
    assert!(request.validate_epoch_inspector_lookup().is_err());
    request.epoch_window = None;
    request.epoch_cut_size = None;
    request.workers = 200;
    assert_eq!(request.resolved_epoch_window(16), Ok(215));
    request.workers = 1;
    assert_eq!(
        request.resolved_epoch_window(16),
        Ok(1),
        "historical inline default preserved"
    );
    request.epoch_publication_order = OldestReady;
    request.epoch_rolling = false;
    assert!(request.validate_epoch_inspector_lookup().is_err());
}

#[test]
fn adaptive_dispatch_requires_rolling_and_changes_only_nondefault_epoch_binding() {
    let mut request =
        OwnerDomainWalkRequest::new(OwnerDomainMatchRequest::new("s".into(), "q".into()));
    assert_eq!(
        request.epoch_dispatch,
        OwnerDomainWalkEpochDispatchPolicy::Fifo
    );
    for mode in [
        OwnerDomainWalkEpochDispatchPolicy::Fifo,
        OwnerDomainWalkEpochDispatchPolicy::Adaptive,
    ] {
        assert_eq!(
            OwnerDomainWalkEpochDispatchPolicy::parse(mode.name()),
            Some(mode)
        );
    }
    assert_eq!(OwnerDomainWalkEpochDispatchPolicy::parse("Adaptive"), None);
    let baseline = checkpoint::epoch_request_binding(&request);
    request.epoch_dispatch = OwnerDomainWalkEpochDispatchPolicy::Adaptive;
    assert!(request.validate_epoch_inspector_lookup().is_err());
    assert_ne!(checkpoint::epoch_request_binding(&request), baseline);
    request.epoch_rolling = true;
    assert!(request.validate_epoch_inspector_lookup().is_err());
    request.publication_policy = OwnerDomainWalkPublicationPolicy::Epoch;
    request.checkpoint = Some(OwnerDomainWalkCheckpointOptions::new("unused"));
    assert!(request.validate_epoch_inspector_lookup().is_ok());
    request.epoch_dispatch = OwnerDomainWalkEpochDispatchPolicy::Fifo;
    request.epoch_rolling = false;
    assert_eq!(checkpoint::epoch_request_binding(&request), baseline);
}

#[test]
fn rolling_execution_is_opt_in_checkpoint_bound_and_has_bounded_window() {
    let mut request = OwnerDomainWalkRequest::new(OwnerDomainMatchRequest::new(
        "selection".into(),
        "queries".into(),
    ));
    let historical = checkpoint::epoch_request_binding(&request);
    assert!(!request.epoch_rolling);
    assert_eq!(request.epoch_window(16), 16);
    request.epoch_rolling = true;
    assert_ne!(checkpoint::epoch_request_binding(&request), historical);
    for policy in [
        OwnerDomainWalkPublicationPolicy::Ordered,
        OwnerDomainWalkPublicationPolicy::Ready,
        OwnerDomainWalkPublicationPolicy::Epoch,
    ] {
        request.publication_policy = policy;
        for checkpointed in [false, true] {
            request.checkpoint =
                checkpointed.then(|| OwnerDomainWalkCheckpointOptions::new("unused"));
            assert_eq!(
                request.validate_epoch_inspector_lookup().is_ok(),
                checkpointed && policy == OwnerDomainWalkPublicationPolicy::Epoch
            );
        }
    }
    assert_eq!(request.epoch_window(16), 1);
    request.workers = 50;
    assert_eq!(request.epoch_window(16), 65);
    assert_eq!(request.epoch_window(4096), 4096);
    request.epoch_rolling = false;
    assert_eq!(checkpoint::epoch_request_binding(&request), historical);
}

#[test]
fn epoch_lookup_policy_defaults_parses_and_refuses_unsupported_native_paths() {
    use OwnerDomainWalkEpochInspectorLookup::{AllMiss, Snapshot};
    let mut request =
        OwnerDomainWalkRequest::new(OwnerDomainMatchRequest::new(String::new(), String::new()));
    assert_eq!(request.epoch_inspector_lookup, AllMiss);
    for mode in [AllMiss, Snapshot] {
        assert_eq!(
            OwnerDomainWalkEpochInspectorLookup::parse(mode.name()),
            Some(mode)
        );
    }
    for invalid in ["", "off", "on", "Snapshot", "snapshot "] {
        assert_eq!(OwnerDomainWalkEpochInspectorLookup::parse(invalid), None);
    }
    request.epoch_inspector_lookup = Snapshot;
    for policy in [
        OwnerDomainWalkPublicationPolicy::Ordered,
        OwnerDomainWalkPublicationPolicy::Ready,
        OwnerDomainWalkPublicationPolicy::OwnerBatched,
        OwnerDomainWalkPublicationPolicy::Epoch,
    ] {
        request.publication_policy = policy;
        for checkpointed in [false, true] {
            request.checkpoint = checkpointed
                .then(|| OwnerDomainWalkCheckpointOptions::new(std::path::PathBuf::from("unused")));
            let allowed = checkpointed && policy == OwnerDomainWalkPublicationPolicy::Epoch;
            assert_eq!(request.validate_epoch_inspector_lookup().is_ok(), allowed);
            if !allowed {
                assert!(
                    admit_request(&request)
                        .unwrap_err()
                        .to_string()
                        .contains("Snapshot inspector lookup requires")
                );
                let options = OwnerDomainWalkVerifyOptions::new(std::path::PathBuf::from("unused"));
                assert!(
                    owner_domain_walk_verify_closure(
                        &request,
                        &options,
                        &AtomicBool::new(false),
                        |_| {}
                    )
                    .unwrap_err()
                    .to_string()
                    .contains("Snapshot inspector lookup requires")
                );
            }
        }
    }
    request.checkpoint = None;
    assert!(epoch::admit_extensions(&request).is_err());
    request.epoch_inspector_lookup = AllMiss;
    assert!(epoch::admit_extensions(&request).is_ok());
}

#[test]
fn all_miss_binds_current_semantics_and_snapshot_is_epoch_only_bound() {
    let mut request = OwnerDomainWalkRequest::new(OwnerDomainMatchRequest::new(
        "selection".into(),
        "queries with exact order and roles".into(),
    ));
    let current = json!({"selection":request.matching.selection_json,
        "queries":request.matching.queries_json,"limits":limits_json(&request),
        "reduction":format!("{:?}",request.matching.reduction_limits),
        "publication":"epoch","walk_semantics_version":EPOCH_WALK_SEMANTICS_VERSION,
        "reuse_initial_d_bands":request.reuse_initial_d_bands,
        "route_domain_overcover":request.route_domain_overcover,
        "route_joint_source_support_pruning":request.route_joint_source_support_pruning,
        "max_route_masks":request.max_route_masks,"subdivision":request.apply_subdivision,
        "max_queries":request.matching.max_queries,"max_query_bytes":request.matching.max_query_bytes,
        "frontier_policy":request.frontier_policy.name()});
    let current_digest = blake3::hash(current.to_string().as_bytes())
        .to_hex()
        .to_string();
    assert_eq!(checkpoint::epoch_request_binding(&request), current_digest);
    // An older native format is not a compatibility target. It must not share
    // the current mathematical request identity, even with identical queries.
    let mut previous = current.clone();
    previous["walk_semantics_version"] = json!(3);
    assert_ne!(
        current_digest,
        blake3::hash(previous.to_string().as_bytes())
            .to_hex()
            .to_string()
    );
    request.epoch_inspector_lookup = OwnerDomainWalkEpochInspectorLookup::Snapshot;
    assert_ne!(checkpoint::epoch_request_binding(&request), current_digest);
    let mut snapshot = current;
    snapshot["epoch_inspector_lookup"] = json!("snapshot");
    assert_eq!(
        checkpoint::epoch_request_binding(&request),
        blake3::hash(snapshot.to_string().as_bytes())
            .to_hex()
            .to_string()
    );
    request.epoch_inspector_lookup = OwnerDomainWalkEpochInspectorLookup::AllMiss;
    assert_eq!(checkpoint::epoch_request_binding(&request), current_digest);
}
