use super::*;

fn steering() -> Value {
    json!({"mode":"create","checkpoint":"/explicit/test-only/checkpoint",
        "stop_after_committed_windows":0,"workers":1,
        "windows":{"max_seed_points":2,"max_seed_bytes":65536,"max_positive_layers":64,"max_partition_checks":1000},
        "per_batch_work":{"max_nodes":1000,"max_rule_applications":1000,"max_transport_calls":1000,
            "max_transport_operations":100000,"max_transport_endpoints":100000,"max_coalescing_additions":100000},
        "prepared_reduction_aggregates":null,
        "census":{"max_unique_selections":1000,"max_identity_payload_bytes":1000000},
        "export":{"max_records":1000,"max_live_payload_bytes":1000000},
        "storage":{"raw":{"max_page_records":2,"max_page_bytes":65536,"max_runs":100,
            "max_pages":1000,"max_directory_bytes":1000000,"max_stored_references":100000,
            "max_merge_runs":100,"max_merge_cached_pages":2,"max_merge_payload_bytes":1000000,
            "max_merge_read_bytes":10000000,"max_decode_transient_bytes":134217728},
            "max_arity":16,"max_delta_records":1000,"max_delta_payload_bytes":1000000,
            "max_page_records":2,"max_page_bytes":65536,"max_cached_pages":0,"max_cached_page_bytes":0,
            "max_directory_bytes":1000000,"max_manifest_bytes":1000000,"max_state_bytes":1000000,
            "max_scope_rows":16,"max_census_records":1000,"max_runs":100,"max_pages":1000,
            "max_physical_files":10000,"max_disk_bytes":10000000,"max_lookup_pages":100,
            "max_lookup_bytes":1000000,"max_transient_bytes":134217728},
        "max_total_rows":16,"max_source_bytes":1048576,"max_output_bytes":1048576})
}

#[test]
fn durable_driver_strict_steering_rejects_unknown_missing_negative_and_overflow() {
    let original = steering();
    let decoded: Steering = serde_json::from_value(original.clone()).unwrap();
    assert_eq!(serde_json::to_value(&decoded).unwrap(), original);
    for path in [
        "",
        "/windows",
        "/per_batch_work",
        "/census",
        "/export",
        "/storage",
        "/storage/raw",
    ] {
        let mut changed = original.clone();
        changed
            .pointer_mut(path)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("unknown".into(), json!(1));
        assert!(
            serde_json::from_value::<Steering>(changed).is_err(),
            "{path}"
        );
    }
    let mut missing = original.clone();
    missing.as_object_mut().unwrap().remove("storage");
    assert!(serde_json::from_value::<Steering>(missing).is_err());
    let mut negative = original.clone();
    negative["workers"] = json!(-1);
    assert!(serde_json::from_value::<Steering>(negative).is_err());
    let overflow = original
        .to_string()
        .replace("\"workers\":1", "\"workers\":18446744073709551616");
    assert!(serde_json::from_str::<Steering>(&overflow).is_err());
}

#[test]
fn durable_driver_zero_and_max_grants_convert_without_capacity_allocation() {
    for n in [0usize, usize::MAX] {
        let mut value = steering();
        for field in [
            "max_nodes",
            "max_rule_applications",
            "max_transport_calls",
            "max_transport_operations",
            "max_transport_endpoints",
            "max_coalescing_additions",
        ] {
            value["per_batch_work"][field] = json!(n);
        }
        value["prepared_reduction_aggregates"] = json!({"max_rule_applications":n,
            "max_pending_frames":n,"max_coalescing_additions":n});
        let p = serde_json::from_value::<Steering>(value).unwrap().policy();
        assert_eq!(p.per_batch_work.max_nodes, n);
        assert_eq!(p.per_batch_work.max_transport_operations, n);
        assert_eq!(
            p.prepared_reduction_aggregates.unwrap().max_pending_frames,
            n
        );
        assert_eq!(p.windows.max_seed_points, 2);
        assert_eq!(p.storage.max_cached_pages, 0);
    }
}
