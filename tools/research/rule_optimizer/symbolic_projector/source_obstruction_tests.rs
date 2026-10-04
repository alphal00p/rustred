use super::*;
use rustred::{
    algebra::CoefficientContext,
    family::{AffineDenominator, IntegralFamily},
};

fn diagnostic_request() -> (Vec<u8>, Value) {
    let (bytes, mut r) = crate::tests::tadpole();
    r["sources"] = json!([r["sources"][0].clone()]);
    r["forbidden_shifts"] = json!([[1]]);
    r["max_refinements"] = json!(0);
    r["limits"]["max_source_rows"] = json!(1);
    r["source_obstruction_diagnostic"] = json!(true);
    r["limits"][SOURCE_CAP] = json!(16);
    r["limits"][TERM_CAP] = json!(10000);
    (bytes, r)
}

fn three_index_family() -> IntegralFamily {
    let c = CoefficientContext::try_new(["d"]).unwrap();
    IntegralFamily::new(
        "general-obstruction-fixture",
        vec!["k".into(), "l".into()],
        vec![],
        c.clone(),
        c.parameter("d").unwrap(),
        vec![
            AffineDenominator::new(c.integer(-1), vec![c.one(), c.zero(), c.zero()]),
            AffineDenominator::new(c.integer(-1), vec![c.zero(), c.zero(), c.one()]),
            AffineDenominator::new(c.integer(-1), vec![c.one(), c.integer(-2), c.one()]),
        ],
        vec![],
        vec![c.zero(); 3],
    )
    .unwrap()
}

fn completed(g: &ParametricIbpGenerator<'_>) -> CompletedIbpSourceRows {
    let prepared = g.prepare_ordinary_ibp().unwrap();
    let rows = (0..prepared.len()).map(|i| prepared.generate(i)).collect();
    prepared.complete(rows).unwrap()
}

fn native_shift(
    g: &ParametricIbpGenerator<'_>,
    all: &CompletedIbpSourceRows,
    values: [i64; 3],
) -> IndexShift {
    let batch = g
        .translate_selected_completed_source_rows(
            all,
            [TranslatedSourceRequest::new(
                0,
                IntegralShift::try_new(values).unwrap(),
            )],
            Default::default(),
        )
        .unwrap();
    batch.sources()[0]
        .terms()
        .keys()
        .find(|s| s.values() == values)
        .unwrap()
        .clone()
}

#[test]
fn complete_native_miss_adds_diagnostic_without_changing_projection_or_bank() {
    let (bytes, r) = diagnostic_request();
    let (report, artifact) = run::<1>(&bytes, &r, false).unwrap();
    assert!(artifact.is_none());
    assert_eq!(report["status"], "NO_TARGET_IN_FROZEN_SPAN_WITH_CURRENT_F");
    assert_eq!(report["attempts"][0]["visited_rows"], 1);
    let diagnostic = &report["attempts"][0][FIELD];
    assert_eq!(
        diagnostic["status"], "EXACT_SOURCE_OBSTRUCTION_AND_PREIMAGES",
        "{report}"
    );
    assert_eq!(diagnostic["actual_source_bank_allowance"], 1);
    assert_eq!(diagnostic["independent_replay"]["source_products"], 1);
    assert_eq!(
        diagnostic["independent_replay"]["target_coordinate_one"],
        true
    );
    assert_eq!(diagnostic["source_bank_modified"], false);
    let pre = &diagnostic["preimages"];
    assert_eq!(pre["canonical_preimage_count"], 3);
    // No historical pin/source-rank/target-leak filter is applied here.
    assert!(
        pre["rows"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["offset"] == json!([1]))
    );
    assert!(
        pre["rows"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["target_present"] == true)
    );
    assert!(pre["maximum_source_numerator_rank"].is_null());
    assert_eq!(pre["rank_bound_claim"], false);
    assert_eq!(r["sources"].as_array().unwrap().len(), 1);
}

#[test]
fn twenty_five_bank_rows_get_exactly_one_normalization_equation_not_a_larger_bank() {
    let f = three_index_family();
    let g = ParametricIbpGenerator::try_new(&f).unwrap();
    let all = completed(&g);
    let c = g.context();
    let target = native_shift(&g, &all, [0, 0, 0]);
    let other = native_shift(&g, &all, [0, -1, 0]);
    let a = c.index(1).unwrap();
    let b = c.index(2).unwrap();
    let sum = c.add_with_limits(&a, &b, Default::default()).unwrap();
    let product = c.mul_with_limits(&a, &b, Default::default()).unwrap();
    let row = project::Row::from([(target.clone(), product), (other.clone(), sum)]);
    let (_, r) = diagnostic_request();
    let mut l = projection_limits(&r).unwrap();
    l.rows = 25;
    let rows = vec![row; 25];
    let (columns, proof) = separator(c, &rows, &target, &BTreeSet::from([other]), &[], l).unwrap();
    assert_eq!(
        proof.values[columns.binary_search(&target).unwrap()],
        c.one()
    );
    assert!(!proof.guards.is_empty()); // rational n1+n2 normalization, not numerical sampling
    let unit = project::Row::from([(target.clone(), c.one())]);
    let mut tampered = proof.values.clone();
    tampered[columns.binary_search(&target).unwrap()] = c.zero();
    assert!(exact_dual::verify(c, &rows, &unit, &columns, &tampered, &mut vec![], l).is_err());
    let mut excess = rows.clone();
    excess.push(rows[0].clone());
    assert!(separator(c, &excess, &target, &BTreeSet::new(), &[], l).is_err());
}

#[test]
fn complete_preimage_refusal_keeps_exact_separator_and_original_miss() {
    let (bytes, mut r) = diagnostic_request();
    r["limits"][SOURCE_CAP] = json!(1);
    let (report, artifact) = run::<1>(&bytes, &r, false).unwrap();
    assert!(artifact.is_none());
    assert_eq!(report["status"], "NO_TARGET_IN_FROZEN_SPAN_WITH_CURRENT_F");
    let d = &report["attempts"][0][FIELD];
    assert_eq!(d["separator_verified"], true);
    assert_eq!(d["status"], "EXACT_SOURCE_OBSTRUCTION_PREIMAGES_INCOMPLETE");
    assert_eq!(d["preimages"]["census_complete"], true);
    assert_eq!(d["preimages"]["canonical_preimage_count"], 3);
    assert_eq!(d["preimages"]["translation_started"], false);
    assert_eq!(d["preimages"]["pairing_complete"], false);
    assert!(d["preimages"].get("rows").is_none());
    r["limits"][SOURCE_CAP] = json!(16);
    r["limits"][TERM_CAP] = json!(1);
    let (report, _) = run::<1>(&bytes, &r, false).unwrap();
    let d = &report["attempts"][0][FIELD];
    assert_eq!(d["separator_verified"], true);
    assert_eq!(d["preimages"]["pairing_complete"], false);
    assert!(d["preimages"].get("rows").is_none());
}

#[test]
fn shared_unfiltered_census_translates_before_fixed_and_has_no_source_sign_gate() {
    let f = three_index_family();
    let g = ParametricIbpGenerator::try_new(&f).unwrap();
    let all = completed(&g);
    let c = g.context();
    let raw = g
        .translate_selected_completed_source_rows(
            &all,
            (0..g.prepare_ordinary_ibp().unwrap().len()).map(|i| {
                TranslatedSourceRequest::new(i, IntegralShift::try_new([0, 0, 0]).unwrap())
            }),
            Default::default(),
        )
        .unwrap();
    let target = native_shift(&g, &all, [0, 0, 0]);
    let (_, mut r) = diagnostic_request();
    r["owner_mask"] = json!("100");
    r["chart"] = json!({"lower":[0,0,0],"upper":[0,null,null],"fixed":[[0,1]]});
    r["limits"][SOURCE_CAP] = json!(1000);
    let l = projection_limits(&r).unwrap();
    let certificate = exact_dual::Certificate {
        values: vec![c.one()],
        guards: vec![],
        dense_slots: 0,
        operation_charge: 0,
    };
    let report = preimages(
        &r,
        &g,
        &all,
        &raw,
        &f,
        &target,
        &BTreeSet::from([target.clone()]),
        &[target.clone()],
        &certificate,
        policy(&r).unwrap(),
        l,
    );
    assert_eq!(
        report["status"], "EXACT_SOURCE_PREIMAGE_PAIRINGS_COMPLETE",
        "{report}"
    );
    assert!(report["maximum_source_numerator_rank"].is_null());
    assert_eq!(report["rank_bound_claim"], false);
    let rows = report["rows"].as_array().unwrap();
    assert!(rows.iter().any(|row| row["target_present"] == true));
    assert!(
        rows.iter()
            .any(|row| row["offset"][0] == -1 && row["target_present"] == false)
    );
    assert!(rows.iter().any(|row| {
        row["offset"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v.as_i64().unwrap() > 0)
    }));
}

#[test]
fn missing_flag_is_identity_and_invalid_or_ambiguous_modes_refuse() {
    let (bytes, mut r) = diagnostic_request();
    r.as_object_mut().unwrap().remove(FIELD);
    let (old, _) = run::<1>(&bytes, &r, false).unwrap();
    r[FIELD] = json!(false);
    let (mut off, _) = run::<1>(&bytes, &r, false).unwrap();
    off["request"] = old["request"].clone();
    off["seconds"] = old["seconds"].clone();
    assert_eq!(off, old);
    for invalid in [Value::Null, json!(1), json!("true"), json!([])] {
        r[FIELD] = invalid;
        assert!(validate(&r).is_err());
    }
    r[FIELD] = json!(true);
    for key in [SOURCE_CAP, TERM_CAP] {
        for invalid in [Value::Null, json!(0), json!(-1), json!(1.5)] {
            let mut bad = r.clone();
            bad["limits"][key] = invalid;
            assert!(validate(&bad).is_err());
        }
    }
    for key in [
        "boundary_correction",
        "boundary_polynomial",
        "modular_nomination",
        "exact_source_ordinals",
        "source_weight_reconstruction",
    ] {
        let mut bad = r.clone();
        bad[key] = json!({});
        assert!(validate(&bad).is_err());
    }
    let mut bad = r.clone();
    bad["max_refinements"] = json!(1);
    assert!(validate(&bad).is_err());
    assert!(run::<1>(&bytes, &r, true).is_err());
}

#[test]
fn diagnostic_dense_refusal_is_nested_and_does_not_turn_a_miss_into_a_result() {
    let (bytes, mut r) = diagnostic_request();
    // The sparse one-row original projection fits; the independently charged
    // dense separator (including its normalization equation) does not.
    r["limits"]["max_matrix_nonzeros"] = json!(8);
    let (report, artifact) = run::<1>(&bytes, &r, false).unwrap();
    assert!(artifact.is_none());
    assert_eq!(report["status"], "NO_TARGET_IN_FROZEN_SPAN_WITH_CURRENT_F");
    let d = &report["attempts"][0][FIELD];
    assert_eq!(d["status"], "SOURCE_OBSTRUCTION_REFUSED_OR_INCOMPLETE");
    assert_eq!(d["separator_verified"], false);
    assert!(d["error"].as_str().unwrap().contains("dense"));
}
