//! Locality tests share the existing native one-variable source/shift fixtures.
use super::*;
use crate::endpoint_locality::Policy;

fn request() -> Value {
    json!({"forbid_endpoint_changes_on_axes":[0], "limits":{
        "max_augmented_columns":100, "max_coordinate_cells":100,
        "max_term_operations":100}})
}

#[test]
fn endpoint_locality_axes_are_explicit_sorted_unique_and_in_range() {
    assert!(Policy::parse(&json!({}), 2).is_ok());
    assert!(Policy::parse(&json!({"forbid_endpoint_changes_on_axes":[]}), 2).is_ok());
    assert!(Policy::parse(&json!({"forbid_endpoint_changes_on_axes":[0,1]}), 2).is_ok());
    for value in [
        json!(null),
        json!(true),
        json!("0"),
        json!([true]),
        json!([-1]),
        json!([0.5]),
        json!([2]),
        json!([0, 0]),
        json!([1, 0]),
        json!([0, 1, 2]),
    ] {
        assert!(Policy::parse(&json!({"forbid_endpoint_changes_on_axes":value}), 2).is_err());
    }
    let (_, mut r) = tadpole();
    r["forbid_endpoint_changes_on_axes"] = json!([1]);
    assert!(validate(&r).is_err());
}

#[test]
fn endpoint_locality_unions_existing_forbidden_and_keeps_zero_target() {
    let r = request();
    let policy = Policy::parse(&r, 1).unwrap();
    let universe = BTreeSet::from([shift(-1), shift(0), shift(1)]);
    let mut forbidden = BTreeSet::from([shift(-1)]);
    let counts = policy
        .extend_forbidden(&r, &universe, &mut forbidden)
        .unwrap();
    assert_eq!(forbidden, BTreeSet::from([shift(-1), shift(1)]));
    let report = policy.annotate(json!({}), counts);
    assert_eq!(
        report["endpoint_locality"]["full_post_fixed_universe_columns"],
        3
    );
    assert_eq!(report["endpoint_locality"]["forbidden_columns"], 2);
    assert_eq!(report["endpoint_locality"]["new_forbidden_columns"], 1);
}

#[test]
fn endpoint_locality_admits_column_coordinate_and_operation_work_before_cloning() {
    let universe = BTreeSet::from([shift(-1), shift(0), shift(1)]);
    for name in [
        "max_augmented_columns",
        "max_coordinate_cells",
        "max_term_operations",
    ] {
        for cap in [0, 2] {
            let mut r = request();
            r["limits"][name] = json!(cap);
            let mut forbidden = BTreeSet::from([shift(-1)]);
            assert!(
                Policy::parse(&r, 1)
                    .unwrap()
                    .extend_forbidden(&r, &universe, &mut forbidden)
                    .is_err()
            );
            assert_eq!(forbidden, BTreeSet::from([shift(-1)]));
        }
    }
    let mut r = request();
    r["limits"] = json!({"max_augmented_columns":3,"max_coordinate_cells":3,
        "max_term_operations":3});
    assert!(
        Policy::parse(&r, 1)
            .unwrap()
            .extend_forbidden(&r, &universe, &mut BTreeSet::new())
            .is_ok()
    );
}

#[test]
fn endpoint_locality_absence_and_empty_skip_all_new_work() {
    // Even zero/missing new-stage limits are irrelevant when the option is off.
    for r in [json!({}), json!({"forbid_endpoint_changes_on_axes":[]})] {
        let policy = Policy::parse(&r, 1).unwrap();
        let mut forbidden = BTreeSet::from([shift(1)]);
        let counts = policy
            .extend_forbidden(&r, &BTreeSet::new(), &mut forbidden)
            .unwrap();
        assert_eq!(forbidden, BTreeSet::from([shift(1)]));
        assert_eq!(
            policy.annotate(json!({"unchanged":true}), counts),
            json!({"unchanged":true})
        );
        policy
            .verify_image(&r, &project::Row::from([(shift(1), context().one())]))
            .unwrap();
    }
}

#[test]
fn endpoint_locality_uses_full_bank_before_exact_selection() {
    let (bytes, mut r) = tadpole();
    r["forbid_endpoint_changes_on_axes"] = json!([0]);
    r["exact_source_ordinals"] = json!([1]);
    r["max_refinements"] = json!(0);
    let (report, artifact) = run::<1>(&bytes, &r, false).unwrap();
    assert_eq!(
        report["status"],
        "NO_TARGET_IN_SELECTED_EXACT_FRAME_WITH_CURRENT_F"
    );
    assert_eq!(report["exact_projection_rows"], 1);
    assert_eq!(
        report["endpoint_locality"]["full_post_fixed_universe_columns"],
        3
    );
    // +1 exists only in the omitted forward source, but is still forbidden.
    assert_eq!(report["endpoint_locality"]["forbidden_columns"], 2);
    assert!(artifact.is_none());
}

#[test]
fn endpoint_locality_keeps_rows_for_native_weighted_cancellation_and_guards() {
    let c = context();
    let n = c.index(0).unwrap();
    let inv = c.div(&c.one(), &n).unwrap();
    let rows = vec![
        project::Row::from([(shift(0), c.one()), (shift(1), inv.clone())]),
        project::Row::from([(shift(1), inv)]),
    ];
    let universe = rows.iter().flat_map(|r| r.keys().cloned()).collect();
    let r = request();
    let policy = Policy::parse(&r, 1).unwrap();
    let mut forbidden = BTreeSet::new();
    policy
        .extend_forbidden(&r, &universe, &mut forbidden)
        .unwrap();
    let p = target(project::project(&c, &rows, &shift(0), &forbidden, &[], limits()).unwrap());
    assert_eq!(p.weights.len(), 2);
    assert_eq!(p.image, project::Row::from([(shift(0), c.one())]));
    assert!(!p.guards.is_empty(), "cancelled input pole must survive");
    assert_eq!(
        project::replay(&c, &rows, &p.weights, &mut Vec::new(), limits()).unwrap(),
        p.image
    );
    policy.verify_image(&r, &p.image).unwrap();
}

#[test]
fn endpoint_locality_final_replay_rejects_nonlocal_residual_and_foreign_arity() {
    let c = context();
    let mut r = request();
    let policy = Policy::parse(&r, 1).unwrap();
    let image = project::Row::from([(shift(0), c.one()), (shift(-1), c.one())]);
    assert!(
        policy
            .verify_image(&r, &image)
            .unwrap_err()
            .contains("replayed endpoint")
    );
    assert!(
        Policy::parse(&r, 2)
            .unwrap()
            .verify_image(&r, &image)
            .unwrap_err()
            .contains("arity")
    );
    r["limits"]["max_term_operations"] = json!(1);
    assert!(
        policy
            .verify_image(&r, &image)
            .unwrap_err()
            .contains("operation allowance")
    );
}

#[test]
fn endpoint_locality_empty_matches_absent_report_and_export_bytes() {
    let (bytes, mut r) = tadpole();
    r["sources"] = json!([r["sources"][1].clone()]);
    r["max_refinements"] = json!(0);
    let (mut absent, absent_bytes) = run::<1>(&bytes, &r, true).unwrap();
    r["forbid_endpoint_changes_on_axes"] = json!([]);
    let (mut empty, empty_bytes) = run::<1>(&bytes, &r, true).unwrap();
    assert_eq!(absent["status"], "CHECKED_PRIORITY_OWNER_EXPORTED");
    assert!(absent_bytes.is_some());
    assert_eq!(absent_bytes, empty_bytes);
    // Only elapsed timing and the literal request echo may differ.
    absent.as_object_mut().unwrap().remove("seconds");
    empty.as_object_mut().unwrap().remove("seconds");
    empty["request"] = absent["request"].clone();
    assert_eq!(absent, empty);
    assert!(empty.get("endpoint_locality").is_none());
}
