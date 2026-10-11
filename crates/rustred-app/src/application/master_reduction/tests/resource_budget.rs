use super::*;

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
