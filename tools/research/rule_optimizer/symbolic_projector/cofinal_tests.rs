use super::*;
fn ray(mask: &str, lower: Value, upper: Value, fixed: Value) -> Value {
    json!({"owner_mask":mask,"chart":{"lower":lower,"upper":upper,"fixed":fixed}})
}
#[test]
fn opt_in_is_strict_and_default_annotation_is_identical() {
    assert!(!enabled(&json!({})).unwrap());
    assert!(!enabled(&json!({"forbid_cofinally_higher_columns":false})).unwrap());
    assert!(enabled(&json!({"forbid_cofinally_higher_columns":true})).unwrap());
    assert!(enabled(&json!({"forbid_cofinally_higher_columns":"true"})).is_err());
    let report = json!({"a":1});
    assert_eq!(annotate(report.clone(), false, &[], 0), report);
}
#[test]
fn finite_multifree_affine_cut_and_unsupported_order_refuse() {
    let order = OrderingPolicy::SpiredUncutV1;
    assert!(Ray::new(&ray("1", json!([0]), json!([5]), json!([])), &order).is_err());
    assert!(
        Ray::new(
            &ray("11", json!([0, 0]), json!([null, null]), json!([])),
            &order
        )
        .is_err()
    );
    let r = ray("11", json!([0, 0]), json!([null, 0]), json!([[1, 1]]));
    for key in ["affine", "cuts", "max_numerator_rank", "constraints"] {
        let mut bad = r.clone();
        bad[key] = json!(0);
        assert!(Ray::new(&bad, &order).is_err());
    }
    let mut bad = r.clone();
    bad["chart"]["affine"] = json!([]);
    assert!(Ray::new(&bad, &order).is_err());
    let mut bad = r.clone();
    bad["chart"]["fixed"] = json!([[1, 2]]);
    assert!(Ray::new(&bad, &order).is_err());
    assert!(Ray::new(&r, &OrderingPolicy::RustRedUnshiftedV1).is_err());
    let priority =
        rustred::sector::CoordinatePriority::try_new(2, &[1, 0], Default::default()).unwrap();
    assert!(
        Ray::new(
            &r,
            &OrderingPolicy::try_spired_with_coordinate_priority(&priority).unwrap()
        )
        .is_err()
    );
}
#[test]
fn active_and_inactive_native_keys_cover_exact_tails() {
    let order = OrderingPolicy::SpiredUncutV1;
    let active = Ray::new(&ray("1", json!([0]), json!([null]), json!([])), &order).unwrap();
    assert!(active.classify(&[0]).unwrap().is_none());
    assert!(active.classify(&[1]).unwrap().is_some());
    assert!(active.classify(&[-1]).unwrap().is_none());
    let inactive = Ray::new(&ray("0", json!([0]), json!([null]), json!([])), &order).unwrap();
    let witness = inactive.classify(&[-1]).unwrap().unwrap();
    assert_eq!(witness["cofinal_local_lower"], 0);
    assert!(inactive.classify(&[2]).unwrap().is_none());
}
#[test]
fn finite_corner_comparison_is_not_a_cofinal_decision() {
    let order = OrderingPolicy::SpiredUncutV1;
    let r = Ray::new(
        &ray("11", json!([0, 0]), json!([null, 0]), json!([[1, 1]])),
        &order,
    )
    .unwrap();
    let shift = [-2, 3];
    assert_eq!(order.compare(&[-1, 4], &[1, 1]).unwrap(), Ordering::Less);
    let witness = r.classify(&shift).unwrap().unwrap();
    assert_eq!(witness["cofinal_local_lower"], 2);
    for n in 3..=100 {
        assert_eq!(
            order.compare(&[n - 2, 4], &[n, 1]).unwrap(),
            Ordering::Greater
        );
    }
}
#[test]
fn fixed_sign_change_uses_support_order_and_all_overflow_refuses() {
    let order = OrderingPolicy::SpiredUncutV1;
    let r = Ray::new(
        &ray("10", json!([0, 0]), json!([null, 0]), json!([[1, 0]])),
        &order,
    )
    .unwrap();
    assert!(r.classify(&[0, 1]).unwrap().is_some());
    assert!(r.classify(&[i64::MIN, 0]).is_err());
    let r = Ray::new(
        &ray(
            "11",
            json!([0, i64::MAX - 1]),
            json!([null, i64::MAX - 1]),
            json!([[1, i64::MAX]]),
        ),
        &order,
    )
    .unwrap();
    assert!(r.classify(&[0, 1]).is_err());
    let r = Ray::new(
        &ray("1", json!([u64::MAX]), json!([null]), json!([])),
        &order,
    )
    .unwrap();
    assert!(r.classify(&[1]).is_err());
}
#[test]
fn all_474_actual_native_failed_shifts_are_reproduced_by_structural_nomination() {
    let fixture: Value = serde_json::from_str(include_str!("cofinal_474_fixture.json")).unwrap();
    assert_eq!(fixture["source_report_sha256"].as_str().unwrap().len(), 64);
    let r = Ray::new(&fixture, &OrderingPolicy::SpiredUncutV1).unwrap();
    let shifts = fixture["shifts"].as_array().unwrap();
    assert_eq!(shifts.len(), 474);
    for shift in shifts {
        let shift = integers(shift, 15).unwrap();
        let witness = r.classify(&shift).unwrap().unwrap();
        assert_eq!(witness["shift"], json!(shift));
        assert_eq!(witness["zero_sector_quotient"], false);
    }
}
