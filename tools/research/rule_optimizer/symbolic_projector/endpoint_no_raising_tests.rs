//! No-raising constrains collected endpoints, not individual ordinary sources.
use super::*;
use crate::endpoint_locality::Policy;

fn request() -> Value {
    json!({"forbid_endpoint_increases_on_axes":[0], "limits":{
        "max_augmented_columns":100, "max_coordinate_cells":100,
        "max_term_operations":100}})
}

#[test]
fn endpoint_no_raising_validates_sorted_unique_in_range_axes() {
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
    ] {
        assert!(Policy::parse(&json!({"forbid_endpoint_increases_on_axes":value}), 2).is_err());
    }
    assert!(Policy::parse(&json!({"forbid_endpoint_increases_on_axes":[0,1]}), 2).is_ok());
    let (_, mut r) = tadpole();
    r["forbid_endpoint_increases_on_axes"] = json!([1]);
    assert!(validate(&r).is_err());
}

#[test]
fn endpoint_no_raising_allows_negative_shifts_and_keeps_mandatory_forbidden() {
    let r = request();
    let p = Policy::parse(&r, 1).unwrap();
    let universe = BTreeSet::from([shift(-1), shift(0), shift(1)]);
    let mut forbidden = BTreeSet::new();
    let counts = p.extend_forbidden(&r, &universe, &mut forbidden).unwrap();
    assert_eq!(forbidden, BTreeSet::from([shift(1)]));
    let report = p.annotate(json!({}), counts);
    assert!(report.get("endpoint_locality").is_none());
    assert_eq!(report["endpoint_no_raising"]["forbidden_columns"], 1);
    forbidden.insert(shift(-1));
    p.extend_forbidden(&r, &universe, &mut forbidden).unwrap();
    assert_eq!(forbidden, BTreeSet::from([shift(-1), shift(1)]));
    p.verify_image(&r, &project::Row::from([(shift(-1), context().one())]))
        .unwrap();
}

#[test]
fn endpoint_no_raising_and_strict_locality_union_and_charge_both_scans() {
    let mut r = request();
    r["forbid_endpoint_changes_on_axes"] = json!([0]);
    let p = Policy::parse(&r, 1).unwrap();
    let universe = BTreeSet::from([shift(-1), shift(0), shift(1)]);
    let mut forbidden = BTreeSet::new();
    r["limits"]["max_term_operations"] = json!(5);
    assert!(p.extend_forbidden(&r, &universe, &mut forbidden).is_err());
    assert!(forbidden.is_empty());
    r["limits"]["max_term_operations"] = json!(6);
    let counts = p.extend_forbidden(&r, &universe, &mut forbidden).unwrap();
    assert_eq!(forbidden, BTreeSet::from([shift(-1), shift(1)]));
    let report = p.annotate(json!({}), counts);
    assert_eq!(report["endpoint_locality"]["new_forbidden_columns"], 2);
    assert_eq!(report["endpoint_no_raising"]["forbidden_columns"], 1);
    assert_eq!(report["endpoint_no_raising"]["new_forbidden_columns"], 0);
}

#[test]
fn endpoint_no_raising_uses_full_post_fixed_universe_before_selection() {
    let (bytes, mut r) = tadpole();
    r["forbid_endpoint_increases_on_axes"] = json!([0]);
    r["exact_source_ordinals"] = json!([1]);
    r["max_refinements"] = json!(0);
    let (report, artifact) = run::<1>(&bytes, &r, false).unwrap();
    assert_eq!(report["exact_projection_rows"], 1);
    assert_eq!(
        report["endpoint_no_raising"]["full_post_fixed_universe_columns"],
        3
    );
    // Omitted forward source is the only source of +1; still constrained.
    assert_eq!(report["endpoint_no_raising"]["forbidden_columns"], 1);
    assert!(artifact.is_none());
}

#[test]
fn endpoint_no_raising_cancels_weighted_columns_without_losing_poles_or_pinches() {
    let c = context();
    let inv = c.div(&c.one(), &c.index(0).unwrap()).unwrap();
    let rows = vec![
        project::Row::from([(shift(0), c.one()), (shift(1), inv.clone())]),
        project::Row::from([(shift(1), inv), (shift(-1), c.one())]),
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
    assert_eq!(p.image.len(), 2);
    assert!(p.image.contains_key(&shift(-1)));
    assert!(!p.image.contains_key(&shift(1)));
    assert!(!p.guards.is_empty());
    assert_eq!(
        project::replay(&c, &rows, &p.weights, &mut Vec::new(), limits()).unwrap(),
        p.image
    );
    policy.verify_image(&r, &p.image).unwrap();
}

#[test]
fn endpoint_no_raising_final_replay_refuses_positive_residual_wrong_arity_and_overbudget() {
    let c = context();
    let mut r = request();
    let p = Policy::parse(&r, 1).unwrap();
    assert!(
        p.verify_image(&r, &project::Row::from([(shift(1), c.one())]))
            .is_err()
    );
    assert!(
        Policy::parse(&r, 2)
            .unwrap()
            .verify_image(&r, &project::Row::from([(shift(-1), c.one())]))
            .is_err()
    );
    let image = project::Row::from([(shift(0), c.one()), (shift(-1), c.one())]);
    r["limits"]["max_term_operations"] = json!(1);
    assert!(p.verify_image(&r, &image).is_err());
}

#[test]
fn endpoint_no_raising_empty_skips_work_and_preserves_mode_off_report_export() {
    let empty = json!({"forbid_endpoint_increases_on_axes":[]});
    let p = Policy::parse(&empty, 1).unwrap();
    let counts = p
        .extend_forbidden(&empty, &BTreeSet::new(), &mut BTreeSet::new())
        .unwrap();
    assert_eq!(
        p.annotate(json!({"unchanged":true}), counts),
        json!({"unchanged":true})
    );
    p.verify_image(&empty, &project::Row::from([(shift(1), context().one())]))
        .unwrap();
    let (bytes, mut r) = tadpole();
    r["sources"] = json!([r["sources"][1].clone()]);
    r["max_refinements"] = json!(0);
    let (mut absent, absent_bytes) = run::<1>(&bytes, &r, true).unwrap();
    r["forbid_endpoint_increases_on_axes"] = json!([]);
    let (mut empty, empty_bytes) = run::<1>(&bytes, &r, true).unwrap();
    assert_eq!(absent["status"], "CHECKED_PRIORITY_OWNER_EXPORTED");
    assert_eq!(absent_bytes, empty_bytes);
    absent.as_object_mut().unwrap().remove("seconds");
    empty.as_object_mut().unwrap().remove("seconds");
    empty["request"] = absent["request"].clone();
    assert_eq!(absent, empty);
}
