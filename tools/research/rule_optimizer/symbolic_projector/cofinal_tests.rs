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
fn finite_affine_cut_and_unsupported_order_refuse() {
    let order = OrderingPolicy::SpiredUncutV1;
    assert!(CofinalOrthant::new(&ray("1", json!([0]), json!([5]), json!([])), &order).is_err());
    let r = ray("11", json!([0, 0]), json!([null, 0]), json!([[1, 1]]));
    for key in [
        "affine",
        "cuts",
        "max_numerator_rank",
        "constraints",
        "power_bounds",
        "max_positive_power",
        "min_power_difference",
        "max_power_difference",
        "rank_bounds",
        "domain_constraints",
        "affine_constraints",
    ] {
        let mut bad = r.clone();
        bad[key] = json!(0);
        assert!(CofinalOrthant::new(&bad, &order).is_err());
    }
    let mut bad = r.clone();
    bad["chart"]["affine"] = json!([]);
    assert!(CofinalOrthant::new(&bad, &order).is_err());
    let mut bad = r.clone();
    bad["chart"]["fixed"] = json!([[1, 2]]);
    assert!(CofinalOrthant::new(&bad, &order).is_err());
    assert!(CofinalOrthant::new(&r, &OrderingPolicy::RustRedUnshiftedV1).is_err());
    let priority =
        rustred::sector::CoordinatePriority::try_new(2, &[1, 0], Default::default()).unwrap();
    assert!(
        CofinalOrthant::new(
            &r,
            &OrderingPolicy::try_spired_with_coordinate_priority(&priority).unwrap()
        )
        .is_err()
    );
}

#[test]
fn mixed_free_signs_stabilize_independently_without_changing_fixed_coordinates() {
    let order = OrderingPolicy::SpiredUncutV1;
    let domain = CofinalOrthant::new(
        &ray(
            "101",
            json!([0, 0, 0]),
            json!([null, null, 0]),
            json!([[2, 1]]),
        ),
        &order,
    )
    .unwrap();
    let shift = [-2, 3, 6];
    let witness = domain.classify(&shift).unwrap().unwrap();
    assert_eq!(witness["free_axes"], json!([0, 1]));
    assert_eq!(witness["cofinal_local_lower"], json!([2, 3]));
    assert_eq!(witness["native_carrier_parent"], json!([3, -3, 1]));
    assert_eq!(witness["native_carrier_child"], json!([1, 0, 7]));
    assert!(witness.get("free_axis").is_none());
    for a in 3..=9 {
        for b in -9..=-3 {
            assert_eq!(
                order.compare(&[a - 2, b + 3, 7], &[a, b, 1]).unwrap(),
                Ordering::Greater
            );
        }
    }
}

#[test]
fn all_free_product_chart_uses_native_degree_and_coordinate_ties() {
    let order = OrderingPolicy::SpiredUncutV1;
    let domain = CofinalOrthant::new(
        &ray(
            "110",
            json!([0, 0, 0]),
            json!([null, null, null]),
            json!([]),
        ),
        &order,
    )
    .unwrap();
    assert!(domain.classify(&[0, 0, 0]).unwrap().is_none());
    for shift in [[1, -1, 0], [-1, 1, 0], [0, 1, 1], [0, -1, -1], [0, 0, -1]] {
        let nominated = domain.classify(&shift).unwrap().is_some();
        for a in 2..=4 {
            for b in 2..=4 {
                for c in -4..=-2 {
                    let child = [a + shift[0], b + shift[1], c + shift[2]];
                    assert_eq!(
                        nominated,
                        order.compare(&child, &[a, b, c]).unwrap() == Ordering::Greater
                    );
                }
            }
        }
    }
}

#[test]
fn fixed_support_swap_uses_support_bit_tie_not_cross_sector_shift_key() {
    let order = OrderingPolicy::SpiredUncutV1;
    let a = CofinalOrthant::new(
        &ray(
            "1010",
            json!([0, 0, 0, 0]),
            json!([null, 0, 0, null]),
            json!([[1, 0], [2, 1]]),
        ),
        &order,
    )
    .unwrap();
    let witness = a.classify(&[0, 1, -1, 0]).unwrap().unwrap();
    assert_eq!(witness["child_support"], json!([true, true, false, false]));
    assert_eq!(
        witness["reason"],
        "exact native support-primary comparison after uniform sign stabilization"
    );
    let b = CofinalOrthant::new(
        &ray(
            "1100",
            json!([0, 0, 0, 0]),
            json!([null, 0, 0, null]),
            json!([[1, 1], [2, 0]]),
        ),
        &order,
    )
    .unwrap();
    assert!(b.classify(&[0, -1, 1, 0]).unwrap().is_none());
}

#[test]
fn finite_boundary_failure_is_not_banned_on_the_product_tail() {
    let order = OrderingPolicy::SpiredUncutV1;
    let r = ray("00", json!([0, 0]), json!([null, null]), json!([]));
    let domain = CofinalOrthant::new(&r, &order).unwrap();
    assert_eq!(
        order.compare(&[1, -2], &[0, -2]).unwrap(),
        Ordering::Greater
    );
    // A coefficient proportional to the first index can vanish on this face;
    // only the unchanged exact producer may decide that, not this nomination.
    assert!(domain.classify(&[1, 0]).unwrap().is_none());
    let report = annotate_for_request(json!({}), &r, true, &[], 0);
    assert!(
        report["cofinal_scope"]
            .as_str()
            .unwrap()
            .contains("Independent unbounded")
    );
    assert_eq!(report["cofinal_higher_column_witnesses"], json!([]));
}

#[test]
fn multi_free_charts_reject_missing_conflicting_finite_and_correlated_geometry() {
    let order = OrderingPolicy::SpiredUncutV1;
    let base = ray(
        "110",
        json!([0, 0, 0]),
        json!([null, null, 0]),
        json!([[2, 0]]),
    );
    assert!(CofinalOrthant::new(&base, &order).is_ok());
    for fixed in [
        json!([]),
        json!([[2, 0], [2, 0]]),
        json!([[0, 1], [2, 0]]),
        json!([[2, 1]]),
    ] {
        let mut bad = base.clone();
        bad["chart"]["fixed"] = fixed;
        assert!(CofinalOrthant::new(&bad, &order).is_err());
    }
    let mut bad = base.clone();
    bad["chart"]["upper"][2] = json!(1);
    assert!(CofinalOrthant::new(&bad, &order).is_err());
    for key in [
        "affine",
        "diagonal",
        "rank_bound",
        "power_bounds",
        "unknown_restriction",
    ] {
        let mut bad = base.clone();
        bad["chart"][key] = json!([]);
        assert!(CofinalOrthant::new(&bad, &order).is_err());
    }
}

#[test]
fn multi_free_overflow_and_witness_admission_fail_closed() {
    let order = OrderingPolicy::SpiredUncutV1;
    let r = ray("10", json!([0, 0]), json!([null, null]), json!([]));
    let domain = CofinalOrthant::new(&r, &order).unwrap();
    assert!(domain.classify(&[i64::MIN, 0]).is_err());
    assert!(domain.classify(&[i64::MAX, 0]).is_err());
    let mut overflow = r.clone();
    overflow["chart"]["lower"][1] = json!(u64::MAX);
    assert!(
        CofinalOrthant::new(&overflow, &order)
            .unwrap()
            .classify(&[0, -1])
            .is_err()
    );
    let mut limited = r;
    limited["limits"] = json!({"max_augmented_columns":1,"max_coordinate_cells":11});
    assert!(admit_witness_storage(&limited, &domain, 1).is_err());
    limited["limits"]["max_coordinate_cells"] = json!(12);
    assert!(admit_witness_storage(&limited, &domain, 1).is_ok());
    assert!(admit_witness_storage(&limited, &domain, usize::MAX).is_err());
}

#[test]
fn single_axis_witness_and_annotation_remain_exactly_unchanged() {
    let r = ray("1", json!([0]), json!([null]), json!([]));
    let witness = CofinalOrthant::new(&r, &OrderingPolicy::SpiredUncutV1)
        .unwrap()
        .classify(&[1])
        .unwrap()
        .unwrap();
    assert_eq!(
        witness,
        json!({"shift":[1],"free_axis":0,"cofinal_local_lower":0,
        "child_support":[true],"native_carrier_parent":[1],"native_carrier_child":[2],
        "reason":"exact native n-independent shift key after uniform sign stabilization",
        "infinite_ray_reason":"All fixed coordinates stay fixed; the translated free coordinate has its parent sign forever after the exact threshold. The native structural key difference is then independent of the free index. A nonzero rational coefficient cannot vanish on that infinite integer tail.",
        "zero_sector_quotient":false,"rule_authority":false})
    );
    assert_eq!(
        annotate_for_request(json!({}), &r, true, &[witness.clone()], 1),
        annotate(json!({}), true, &[witness], 1)
    );
    let multi = ray("11", json!([0, 0]), json!([null, null]), json!([]));
    assert_eq!(
        annotate_for_request(json!({"x":1}), &multi, false, &[], 0),
        json!({"x":1})
    );
}
#[test]
fn active_and_inactive_native_keys_cover_exact_tails() {
    let order = OrderingPolicy::SpiredUncutV1;
    let active =
        CofinalOrthant::new(&ray("1", json!([0]), json!([null]), json!([])), &order).unwrap();
    assert!(active.classify(&[0]).unwrap().is_none());
    assert!(active.classify(&[1]).unwrap().is_some());
    assert!(active.classify(&[-1]).unwrap().is_none());
    let inactive =
        CofinalOrthant::new(&ray("0", json!([0]), json!([null]), json!([])), &order).unwrap();
    let witness = inactive.classify(&[-1]).unwrap().unwrap();
    assert_eq!(witness["cofinal_local_lower"], 0);
    assert!(inactive.classify(&[2]).unwrap().is_none());
}
#[test]
fn finite_corner_comparison_is_not_a_cofinal_decision() {
    let order = OrderingPolicy::SpiredUncutV1;
    let r = CofinalOrthant::new(
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
    let r = CofinalOrthant::new(
        &ray("10", json!([0, 0]), json!([null, 0]), json!([[1, 0]])),
        &order,
    )
    .unwrap();
    assert!(r.classify(&[0, 1]).unwrap().is_some());
    assert!(r.classify(&[i64::MIN, 0]).is_err());
    let r = CofinalOrthant::new(
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
    let r = CofinalOrthant::new(
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
    let r = CofinalOrthant::new(&fixture, &OrderingPolicy::SpiredUncutV1).unwrap();
    let shifts = fixture["shifts"].as_array().unwrap();
    assert_eq!(shifts.len(), 474);
    for shift in shifts {
        let shift = integers(shift, 15).unwrap();
        let witness = r.classify(&shift).unwrap().unwrap();
        assert_eq!(witness["shift"], json!(shift));
        assert_eq!(witness["zero_sector_quotient"], false);
    }
}
