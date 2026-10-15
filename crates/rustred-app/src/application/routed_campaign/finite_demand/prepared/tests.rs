use super::*;
#[test]
fn generated_and_storage_errors_keep_limit_cancel_io_and_corruption_kinds() {
    use crate::AppErrorKind as A;
    use rustred::persistence::finite_work::{GeneratedWorkError, GeneratedWorkErrorKind as K};
    for (kind, expected) in [
        (K::Resource, A::Limit),
        (K::Cancelled, A::Cancelled),
        (K::Io, A::Execution),
        (K::Corrupt, A::Serialization),
    ] {
        let error = GeneratedWorkError::new(kind, "injected typed failure");
        assert_eq!(generated_failure(error.clone()).kind(), expected);
        assert_eq!(storage_failure(error.into()).kind(), expected);
    }
    assert_eq!(
        check(&AtomicBool::new(true)).unwrap_err().kind(),
        A::Cancelled
    );
}
fn request() -> RoutedCampaignRequest {
    RoutedCampaignRequest::new(json!({"family_fingerprint":"family","owners":[{"path":"owner.rrbin","bytes":42,"mask":"1"}],
        "initial_frontier_routes":[]}).to_string(),"1\n".into())
}
fn bound(request: &RoutedCampaignRequest) -> RawExecutionBinding {
    let (selection, n, _) = Selection::parse(&request.selection_json).unwrap();
    execution_binding(request, &selection, n, &["ab".repeat(32)]).unwrap()
}
#[test]
fn prepared_identity_excludes_paths_workers_scope_and_aggregate_budgets() {
    let original = request();
    let expected = bound(&original);
    let mut other = original.clone();
    other.owner_base = "/different/workspace".into();
    other.workers = 4;
    other.targets_csv = "2\n3\n".into();
    other.trace_limits.max_unique_nodes = 7;
    other.reduction_limits.max_cached_integrals = 5;
    let mut selection: Value = serde_json::from_str(&other.selection_json).unwrap();
    selection["owners"][0]["path"] = json!("relocated-owner.rrbin");
    other.selection_json = selection.to_string();
    assert_eq!(expected, bound(&other));
    other.reduction_limits.exact_algebra.max_term_operations += 1;
    assert_ne!(expected, bound(&other));
}
#[test]
fn prepared_identity_binds_payload_masks_order_and_route_semantics() {
    let original = request();
    let expected = bound(&original);
    let (mut selection, n, _) = Selection::parse(&original.selection_json).unwrap();
    assert_ne!(
        expected,
        execution_binding(&original, &selection, n, &["cd".repeat(32)]).unwrap()
    );
    assert!(execution_binding(&original, &selection, n, &[]).is_err());
    selection.family_fingerprint.push_str("-changed");
    assert_ne!(
        expected,
        execution_binding(&original, &selection, n, &["ab".repeat(32)]).unwrap()
    );
    let mut changed = original.clone();
    let mut value: Value = serde_json::from_str(&changed.selection_json).unwrap();
    value["initial_frontier_routes"] = json!([{"source_mask":"1","owner_mask":"1","requires_transport":false,
        "source_to_representative":[["1"]],"owner_to_representative":[["1"]]}]);
    changed.selection_json = value.to_string();
    assert_ne!(expected, bound(&changed));
}
