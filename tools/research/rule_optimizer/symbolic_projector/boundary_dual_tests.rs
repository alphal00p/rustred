use super::super::tests::{context, limits, shift};
use super::*;

#[test]
fn native_one_separator_handles_dependent_rows_and_empty_columns() {
    let c = context("dual-dependent");
    let corrections = vec![
        Row::from([(shift(-2), c.one()), (shift(-1), c.one())]),
        Row::from([(shift(-2), c.integer(2)), (shift(-1), c.integer(2))]),
        Row::new(),
    ];
    let baseline = Row::from([(shift(-2), c.one())]);
    let columns = vec![shift(-2), shift(-1), shift(0)];
    let proof = solve(&c, &corrections, &baseline, &columns, &[], limits()).unwrap();
    assert_eq!(proof.values.len(), 3);
    assert_eq!(proof.values[0], c.one());
    assert_eq!(proof.values[1], c.integer(-1));
    assert!(proof.values[2].is_zero());
}

#[test]
fn baseline_only_column_is_kept_and_proves_separation() {
    let c = context("dual-baseline-only");
    let corrections = vec![Row::from([(shift(-2), c.one())])];
    let baseline = Row::from([(shift(-1), c.integer(2))]);
    let columns = BTreeSet::from([shift(-2), shift(-1), shift(0)]);
    let mut report = json!({"status":"NO_BOUNDARY_CORRECTION_IN_FROZEN_WEIGHTED_SPAN"});
    let _ = append_report(
        &c,
        &json!({"exact_dual_separator":true}),
        &corrections,
        &baseline,
        &columns,
        &[],
        limits(),
        &mut report,
    );
    let d = &report["exact_dual_separator"];
    assert_eq!(d["status"], "EXACT_FINITE_SPAN_SEPARATOR");
    assert_eq!(d["columns"][1]["baseline_nonzero"], true);
    assert_eq!(d["columns"][1]["correction_rows"], json!([]));
    assert_eq!(d["columns"].as_array().unwrap().len(), 3);
    assert_eq!(d["independent_replay"]["baseline_product_one"], true);
    assert_eq!(
        report["status"],
        "NO_BOUNDARY_CORRECTION_IN_FROZEN_WEIGHTED_SPAN"
    );
}

#[test]
fn replay_rejects_tampered_values_and_column_binding() {
    let c = context("dual-tamper");
    let rows = vec![Row::from([(shift(-2), c.one()), (shift(-1), c.one())])];
    let baseline = Row::from([(shift(-2), c.one())]);
    let columns = vec![shift(-2), shift(-1)];
    let proof = solve(&c, &rows, &baseline, &columns, &[], limits()).unwrap();
    let mut tampered = proof.values.clone();
    tampered[1] = c.zero();
    assert!(
        verify(
            &c,
            &rows,
            &baseline,
            &columns,
            &tampered,
            &mut vec![],
            limits()
        )
        .is_err()
    );
    assert!(
        verify(
            &c,
            &rows,
            &baseline,
            &[shift(-1), shift(-2)],
            &proof.values,
            &mut vec![],
            limits()
        )
        .is_err()
    );
    assert!(verify(&c, &rows, &baseline, &columns, &[], &mut vec![], limits()).is_err());
}

#[test]
fn rational_normalization_and_incoming_poles_remain_explicit() {
    let c = context("dual-poles");
    let d = c.lift(&c.base().parameter("d").unwrap()).unwrap();
    let inverse = c.div_with_limits(&c.one(), &d, Default::default()).unwrap();
    // Even an input outside F, unused by the certificate, retains its pole.
    let rows = vec![Row::from([(shift(1), inverse)])];
    let baseline = Row::from([(shift(-1), d.clone())]);
    let guard = Guard {
        polynomial: c
            .numerator_condition_with_limits(&d, Default::default())
            .unwrap(),
        origin: "incoming source condition".into(),
    };
    let proof = solve(
        &c,
        &rows,
        &baseline,
        &[shift(-1)],
        &[guard.clone()],
        limits(),
    )
    .unwrap();
    assert_eq!(
        c.mul_with_limits(&proof.values[0], &d, Default::default())
            .unwrap(),
        c.one()
    );
    assert!(
        proof
            .guards
            .iter()
            .any(|g| g.origin == guard.origin && g.polynomial == guard.polynomial)
    );
    assert!(
        proof
            .guards
            .iter()
            .any(|g| g.origin == "normalized dual witness denominator"
                && g.polynomial == guard.polynomial)
    );
    assert!(
        proof
            .guards
            .iter()
            .any(|g| g.origin == "dual original input denominator"
                && g.polynomial == guard.polynomial)
    );
}

#[test]
fn inconsistent_stack_and_foreign_context_are_diagnostic_refusals() {
    let c = context("dual-context-a");
    let foreign = context("dual-context-b");
    let rows = vec![Row::from([(shift(-1), c.one())])];
    let baseline = Row::from([(shift(-1), c.integer(2))]);
    assert!(solve(&c, &rows, &baseline, &[shift(-1)], &[], limits()).is_err());
    let foreign_row = vec![Row::from([(shift(1), foreign.one())])];
    assert!(solve(&c, &foreign_row, &baseline, &[shift(-1)], &[], limits()).is_err());
}

#[test]
fn dense_allocation_and_work_are_precharged_and_miss_status_is_preserved() {
    let c = context("dual-budget");
    let rows = vec![Row::new()];
    let baseline = Row::from([(shift(-1), c.one())]);
    for resource in 0..4 {
        let mut l = limits();
        match resource {
            0 => l.nonzeros = 1,
            1 => l.operations = 1,
            2 => l.coefficient_terms = 1,
            _ => l.columns = 0,
        }
        let mut report = json!({"status":"NO_BOUNDARY_CORRECTION_IN_FROZEN_WEIGHTED_SPAN"});
        let _ = append_report(
            &c,
            &json!({"exact_dual_separator":true}),
            &rows,
            &baseline,
            &BTreeSet::from([shift(-1)]),
            &[],
            l,
            &mut report,
        );
        assert_eq!(
            report["status"],
            "NO_BOUNDARY_CORRECTION_IN_FROZEN_WEIGHTED_SPAN"
        );
        assert_eq!(
            report["exact_dual_separator"]["status"],
            "DUAL_DIAGNOSTIC_REFUSED_OR_INCOMPLETE"
        );
        assert!(report["exact_dual_separator"].get("lambda").is_none());
    }
}

#[test]
fn optional_flag_is_strict_boolean_and_default_off_is_byte_value_identity() {
    for bad in [Value::Null, json!(0), json!("true"), json!([])] {
        assert!(enabled(&json!({"exact_dual_separator":bad})).is_err());
    }
    let c = context("dual-off");
    let original =
        json!({"status":"NO_BOUNDARY_CORRECTION_IN_FROZEN_WEIGHTED_SPAN","conditions":[]});
    for cfg in [json!({}), json!({"exact_dual_separator":false})] {
        let mut report = original.clone();
        let _ = append_report(
            &c,
            &cfg,
            &[],
            &Row::new(),
            &BTreeSet::new(),
            &[],
            limits(),
            &mut report,
        );
        assert_eq!(
            serde_json::to_vec(&report).unwrap(),
            serde_json::to_vec(&original).unwrap()
        );
    }
}
