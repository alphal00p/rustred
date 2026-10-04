use super::super::tests::{context, limits, shift};
use super::*;
use rustred::{
    algebra::CoefficientContext,
    family::{AffineDenominator, IntegralFamily},
};

fn family() -> IntegralFamily {
    let c = CoefficientContext::try_new(["d"]).unwrap();
    IntegralFamily::new(
        "nomination-tadpole",
        vec!["q".into()],
        vec![],
        c.clone(),
        c.parameter("d").unwrap(),
        vec![AffineDenominator::new(c.integer(-1), vec![c.one()])],
        vec![],
        vec![c.zero()],
    )
    .unwrap()
}
fn complete(g: &ParametricIbpGenerator<'_>) -> CompletedIbpSourceRows {
    let p = g.prepare_ordinary_ibp().unwrap();
    let rows = (0..p.len()).map(|i| p.generate(i)).collect();
    p.complete(rows).unwrap()
}
fn inventory(
    g: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
) -> SelectedTranslatedSourceBatch {
    g.translate_selected_completed_source_rows(
        completed,
        [TranslatedSourceRequest::new(
            0,
            IntegralShift::try_new([0]).unwrap(),
        )],
        Default::default(),
    )
    .unwrap()
}
fn request() -> Value {
    let (_, mut r) = crate::tests::tadpole();
    r["chart"] = json!({"lower":[0],"upper":[0],"fixed":[[0,1]]});
    r["boundary_correction"] = json!({"fixed_pinch_axis":0});
    r
}
fn separator(values: Vec<rustred::algebra::IndexedCoefficient>) -> dual::Certificate {
    dual::Certificate {
        values,
        guards: vec![],
        dense_slots: 0,
        operation_charge: 0,
    }
}

#[test]
fn raw_preimages_are_canonical_deduplicated_and_include_negative_pin_seeds() {
    let f = family();
    let g = ParametricIbpGenerator::try_new(&f).unwrap();
    let all = complete(&g);
    let raw = inventory(&g, &all);
    let c = g.context();
    let (found, pairs) = census(
        c,
        &raw,
        &[shift(-1), shift(0)],
        &[c.one(), c.one()],
        0,
        1,
        10000,
        limits(),
    )
    .unwrap();
    assert_eq!(found.len(), 2);
    assert_eq!(pairs, 3);
    assert_eq!(
        found
            .keys()
            .map(|q| q.offset().values().to_vec())
            .collect::<Vec<_>>(),
        vec![vec![-2], vec![-1]]
    );
    assert_eq!(found.values().map(BTreeSet::len).sum::<usize>(), 3);
}

#[test]
fn native_translation_precedes_fixed_specialization_and_zero_seed_raise_disappears() {
    let f = family();
    let g = ParametricIbpGenerator::try_new(&f).unwrap();
    let all = complete(&g);
    let raw = inventory(&g, &all);
    let c = g.context();
    let report = run(
        &request(),
        &g,
        &all,
        &raw,
        &f,
        &shift(0),
        &[shift(0)],
        &separator(vec![c.one()]),
        Default::default(),
        limits(),
    );
    assert_eq!(
        report["status"], "EXACT_WITNESS_PREIMAGE_PAIRINGS_COMPLETE",
        "{report}"
    );
    assert_eq!(report["canonical_preimage_count"], 1);
    assert_eq!(report["zero_pairings"], 1);
    assert_eq!(report["rows"][0]["source_pin_seed"], 0);
    assert_eq!(report["rows"][0]["target_leak"], false);
    assert_eq!(report["rows"][0]["unpinched_leaks"], json!([]));
    // If the coefficient -2n were fixed at n=1 before translation, the
    // target coefficient would incorrectly survive. Native order kills it.
    assert!(
        !report["rows"][0]["full_image_shifts"]
            .as_array()
            .unwrap()
            .contains(&json!([0]))
    );
}

#[test]
fn negative_pin_source_is_admitted_and_full_native_image_stays_pinched() {
    let f = family();
    let g = ParametricIbpGenerator::try_new(&f).unwrap();
    let all = complete(&g);
    let raw = inventory(&g, &all);
    let c = g.context();
    let report = run(
        &request(),
        &g,
        &all,
        &raw,
        &f,
        &shift(0),
        &[shift(-1)],
        &separator(vec![c.one()]),
        Default::default(),
        limits(),
    );
    assert_eq!(
        report["status"], "EXACT_WITNESS_PREIMAGE_PAIRINGS_COMPLETE",
        "{report}"
    );
    assert_eq!(report["canonical_preimage_count"], 2);
    assert_eq!(report["source_pin_seed_counts"], json!({"-1":1,"0":1}));
    assert_eq!(report["nonzero_pairings"], 2);
    assert_eq!(report["leaking_rows"], 0);
    assert_eq!(
        report["rows"][0]["source"]["maximum_source_numerator_rank"],
        1
    );
    assert_eq!(report["source_bank_modified"], false);
}

#[test]
fn complete_census_cap_refuses_before_any_partial_translation() {
    let f = family();
    let g = ParametricIbpGenerator::try_new(&f).unwrap();
    let all = complete(&g);
    let raw = inventory(&g, &all);
    let c = g.context();
    let mut l = limits();
    l.rows = 1;
    let report = run(
        &request(),
        &g,
        &all,
        &raw,
        &f,
        &shift(0),
        &[shift(-1)],
        &separator(vec![c.one()]),
        Default::default(),
        l,
    );
    assert_eq!(report["status"], "WITNESS_PREIMAGE_REFUSED_OR_INCOMPLETE");
    assert_eq!(report["census_complete"], true);
    assert_eq!(report["canonical_preimage_count"], 2);
    assert_eq!(report["translation_started"], false);
    assert_eq!(report["pairing_complete"], false);
    assert!(report.get("rows").is_none());
}

#[test]
fn raw_inventory_and_witness_context_cannot_be_rebound() {
    let f = family();
    let g = ParametricIbpGenerator::try_new(&f).unwrap();
    let all = complete(&g);
    let raw = inventory(&g, &all);
    let c = g.context();
    let wrong = g
        .translate_selected_completed_source_rows(
            &all,
            [TranslatedSourceRequest::new(
                0,
                IntegralShift::try_new([-1]).unwrap(),
            )],
            Default::default(),
        )
        .unwrap();
    assert!(census(c, &wrong, &[shift(-1)], &[c.one()], 0, 1, 10000, limits()).is_err());
    assert!(
        census(
            c,
            &raw,
            &[shift(-1)],
            &[context("foreign-nomination").one()],
            0,
            1,
            10000,
            limits()
        )
        .is_err()
    );
}

#[test]
fn geometric_overlap_is_not_nonzero_pairing_and_unused_poles_remain() {
    let c = context("nomination-pairing");
    let d = c.lift(&c.base().parameter("d").unwrap()).unwrap();
    let inverse = c.div_with_limits(&c.one(), &d, Default::default()).unwrap();
    let row = project::Row::from([
        (shift(-2), c.one()),
        (shift(-1), c.integer(-1)),
        (shift(1), inverse),
    ]);
    let mut guards = vec![];
    let sum = pairing(
        &c,
        &row,
        &[shift(-2), shift(-1)],
        &[c.one(), c.one()],
        &mut guards,
        &mut 0,
        limits(),
    )
    .unwrap();
    assert!(sum.is_zero());
    let polynomial = c
        .numerator_condition_with_limits(&d, Default::default())
        .unwrap();
    assert!(guards.iter().any(|g| g.polynomial == polynomial));
}

#[test]
fn source_activation_is_reported_not_filtered_and_flag_requires_separator() {
    let r = json!({"owner_mask":"10","chart":{"lower":[0,0],"upper":[0,0],"fixed":[[0,1],[1,0]]}});
    let summary = seed_summary(&r, &[-2, 1]).unwrap();
    assert_eq!(summary["maximum_source_numerator_rank"], 1);
    assert_eq!(summary["activated_parent_inactive_axes"], json!([1]));
    assert_eq!(summary["source_within_parent_support"], false);
    assert!(!enabled(&json!({})).unwrap());
    assert!(!enabled(&json!({"witness_preimage_nomination":false})).unwrap());
    assert!(enabled(&json!({"witness_preimage_nomination":true})).is_err());
    assert!(
        enabled(&json!({"witness_preimage_nomination":true,"exact_dual_separator":true})).unwrap()
    );
    for value in [Value::Null, json!(1), json!("true")] {
        assert!(
            enabled(&json!({"witness_preimage_nomination":value,"exact_dual_separator":true}))
                .is_err()
        );
    }
}
