use super::*;

#[test]
fn required_census_checks_actual_shape_without_a_private_copy_quota() {
    let scratch = Scratch::new();
    let name = "inventory-census.bin";
    let bytes = b"small native census transport fixture";
    write_file_atomically(&scratch.0.join(name), bytes, true).unwrap();
    let mut report = json!({"schema":SCHEMA,"inventory_census_cache":{
        "schema":"rustred.walk-inventory-census.finite.v1", "file":name,
        "required_for_finite_inventory":true,"prefix_reuse":false,
        "bytes":bytes.len(),"blake3":blake3::hash(bytes).to_hex().to_string()}});
    census_cache::validate_required(&scratch.0, &report).unwrap();
    // A representation-wide claim exercises admission beyond the removed
    // ceiling without creating or allocating a large input. Shape binding,
    // rather than an implicit byte quota, must reject this mismatched claim.
    report["inventory_census_cache"]["bytes"] = json!(u64::MAX);
    let error = census_cache::validate_required(&scratch.0, &report).unwrap_err();
    assert!(error.to_string().contains("file shape or size differs"));
    assert_eq!(std::fs::read(scratch.0.join(name)).unwrap(), bytes);
}

#[test]
fn trusted_native_transport_has_no_implicit_quota_and_preserves_explicit_grants() {
    let limits = profile::native_io_limits(None);
    assert_eq!(limits.max_program_bytes, usize::MAX);
    assert_eq!(limits.max_sections, usize::MAX);
    assert_eq!(limits.max_collection_entries, usize::MAX);
    assert_eq!(limits.max_state_bytes, usize::MAX);
    assert_eq!(limits.max_atom_bytes, usize::MAX);
    assert_eq!(limits.max_total_atom_bytes, usize::MAX);
    assert_eq!(
        limits.exact_algebra,
        rustred::algebra::ExactAlgebraLimits::unbounded_resources()
    );
    let native = session();
    let bytes = native.to_native_bytes(limits).unwrap();
    let restored = TerminalRelationSession::from_native_bytes(
        &bytes,
        MasterNormalizationProfile::ConservativeV1.relation_limits(),
        limits,
    )
    .unwrap();
    assert_eq!(restored.to_native_bytes(limits).unwrap(), bytes);
    let explicit = BinaryIoLimits {
        max_program_bytes: bytes.len() - 1,
        ..limits
    };
    assert_eq!(profile::native_io_limits(Some(explicit)), explicit);
    assert!(
        native
            .to_native_bytes(profile::native_io_limits(Some(explicit)))
            .is_err()
    );
    assert!(
        TerminalRelationSession::from_native_bytes(
            &bytes,
            MasterNormalizationProfile::ConservativeV1.relation_limits(),
            profile::native_io_limits(Some(explicit))
        )
        .is_err()
    );
    let explicit = BinaryIoLimits {
        max_collection_entries: 0,
        ..limits
    };
    assert_eq!(profile::native_io_limits(Some(explicit)), explicit);
    assert!(
        native
            .to_native_bytes(profile::native_io_limits(Some(explicit)))
            .is_err()
    );
}

#[test]
fn explicit_finite_control_grant_still_bounds_metadata_reads() {
    let scratch = Scratch::new();
    let path = scratch.0.join("metadata.json");
    write_json(&path, &json!({"checkpoint":"native"})).unwrap();
    assert!(storage::read_json_with_policy(&path, None).is_ok());
    let mut value: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/input/finite_campaign/high_memory.json"
    )))
    .unwrap();
    value["control"]["max_bytes"] = json!(1);
    let policy =
        crate::FiniteDemandPolicy::from_json(&serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(storage::read_json_with_policy(&path, Some(&policy)).is_err());
}

#[test]
fn operational_column_cap_can_change_without_restarting_native_state() {
    let mut native = session();
    let before = native.to_native_bytes(BinaryIoLimits::default()).unwrap();
    let count = native.statistics().columns;
    let mut options = MasterReductionOptions::new("unused", "unused");
    let mut report = json!({});
    budget::configure(&options, &mut native, &mut report).unwrap();
    assert!(report["max_relation_columns"].is_null());
    options.max_relation_columns = Some(count + 10);
    budget::configure(&options, &mut native, &mut report).unwrap();
    assert_eq!(report["max_relation_columns"], count + 10);
    options.max_relation_columns = None;
    budget::configure(&options, &mut native, &mut report).unwrap();
    assert_eq!(report["max_relation_columns"], count + 10);
    options.max_relation_columns = Some(count.saturating_sub(1).max(1));
    if count > 1 {
        assert!(budget::configure(&options, &mut native, &mut report).is_err());
        assert_eq!(report["max_relation_columns"], count + 10);
    }
    options.max_relation_columns = Some(0);
    budget::configure(&options, &mut native, &mut report).unwrap();
    assert!(report["max_relation_columns"].is_null());
    assert_eq!(
        native.to_native_bytes(BinaryIoLimits::default()).unwrap(),
        before
    );
    report["max_relation_columns"] = json!("unlimited-ish");
    assert!(budget::configure(&options, &mut native, &mut report).is_err());
}
