use super::*;
use crate::source_product;
use rustred::{
    algebra::{CoefficientContext, IndexedPolynomial},
    family::AffineDenominator,
    identity::{RowId, TranslatedSourceRequest},
};

fn family(external: bool) -> IntegralFamily {
    let c = CoefficientContext::try_new(["d", "u", "s", "t", "r", "nu"]).unwrap();
    let n = if external { 3 } else { 6 };
    let denominators = (0..n)
        .map(|i| {
            AffineDenominator::new(
                c.integer(-(i as i64 + 1)),
                (0..n)
                    .map(|j| {
                        if i != j {
                            c.zero()
                        } else if i == 0 {
                            c.parameter("u").unwrap()
                        } else {
                            c.one()
                        }
                    })
                    .collect(),
            )
        })
        .collect();
    let mut power_shifts = vec![c.zero(); n];
    power_shifts[2] = c.parameter("nu").unwrap();
    IntegralFamily::new(
        "two-protected-public-bridge-fixture",
        (0..if external { 1 } else { 3 })
            .map(|i| format!("k{i}"))
            .collect(),
        (0..if external { 2 } else { 0 })
            .map(|i| format!("p{i}"))
            .collect(),
        c.clone(),
        c.parameter("d").unwrap(),
        denominators,
        if external {
            vec![
                vec![c.parameter("s").unwrap(), c.parameter("r").unwrap()],
                vec![c.parameter("r").unwrap(), c.parameter("t").unwrap()],
            ]
        } else {
            vec![]
        },
        power_shifts,
    )
    .unwrap()
}

fn request(f: &IntegralFamily) -> Value {
    let mut recenter = vec![0; f.denominator_count()];
    recenter[2] = 1;
    json!({"schema":SCHEMA,"owner_mask":format!("11{}","0".repeat(f.denominator_count()-2)),
        "family_fingerprint":f.fingerprint(),"expected_order":"rustred.spired-uncut-sector-order.v1",
        "nomination":{
        "differentiated_loop":0,"protected_denominators":[0,1],
        "contractions":if f.external_count()==0 {json!([
            {"kind":"loop","index":0},{"kind":"loop","index":1},{"kind":"loop","index":2}
        ])} else {json!([
            {"kind":"loop","index":0},{"kind":"external","index":0},{"kind":"external","index":1}
        ])},"recenter":recenter},
        "chart":{"lower":vec![0;f.denominator_count()],"upper":vec![Value::Null;f.denominator_count()],"fixed":[]},
        "limits":{"max_source_rows":128,"max_complete_source_rows":128,"max_terms":10000,
            "max_conditions":10000,"max_coordinate_cells":1000000,"max_cells":4096,
            "max_polynomial_terms":10000,"max_term_operations":1000000,"max_exponent":64,
            "max_owner_bytes":1048576,"max_output_bytes":1048576,"max_report_bytes":16777216}})
}

fn completed(g: &ParametricIbpGenerator<'_>) -> (CompletedIbpSourceRows, usize) {
    let p = g.prepare_ordinary_ibp().unwrap();
    let count = p.len();
    let rows = (0..count).map(|i| p.generate(i)).collect();
    (p.complete(rows).unwrap(), count)
}

fn common_product(
    g: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    count: usize,
    n: MaterializedNomination,
    r: &Value,
) -> (
    BTreeMap<IndexShift, IndexedCoefficient>,
    Vec<IndexedPolynomial>,
) {
    let zero = IntegralShift::try_new(vec![0; g.context().index_count()]).unwrap();
    let inventory = g
        .translate_selected_completed_source_rows(
            completed,
            (0..count).map(|i| TranslatedSourceRequest::new(i, zero.clone())),
            policies(r).unwrap().translated_sources,
        )
        .unwrap();
    let ids: BTreeMap<_, _> = inventory
        .sources()
        .iter()
        .map(|s| {
            (
                s.provenance().source_row().clone(),
                s.provenance().source_ordinal(),
            )
        })
        .collect();
    let weights: BTreeMap<_, _> = n
        .nomination
        .weights
        .into_iter()
        .map(|((row, offset), weight)| (TranslatedSourceRequest::new(ids[&row], offset), weight))
        .collect();
    let sources = g
        .translate_selected_completed_source_rows(
            completed,
            weights.keys().cloned(),
            policies(r).unwrap().translated_sources,
        )
        .unwrap();
    let result =
        source_product(g.context(), &sources, &weights, n.nomination.conditions, r).unwrap();
    assert_eq!(
        result.0, n.full_product,
        "independent common regeneration must equal native materialization"
    );
    result
}

#[test]
fn bridge_retains_full_mixed_product_and_explicit_family_pole() {
    let f = family(false);
    let g = ParametricIbpGenerator::try_new(&f).unwrap();
    let (completed, count) = completed(&g);
    let r = request(&f);
    let nomination = nominate(&f, &g, &completed, &r).unwrap();
    assert!(!nomination.full_product.is_empty());
    assert_eq!(
        nomination.nomination.report["maximum_emitted_polynomial_degree"],
        2
    );
    let c = g.context();
    let u = c
        .lift(&f.coefficient_context().parameter("u").unwrap())
        .unwrap();
    let inverse = c.div_with_limits(&c.one(), &u, Default::default()).unwrap();
    let pole = c
        .denominator_condition_with_limits(&inverse, Default::default())
        .unwrap();
    assert!(nomination.nomination.conditions.contains(&pole));
    let (product, conditions) = common_product(&g, &completed, count, nomination, &r);
    assert!(conditions.contains(&pole));
    assert!(
        product.values().any(|v| c
            .specialize_fixed_indices(v, &[(2, 0)], Default::default())
            .unwrap()
            .0
            != *v),
        "unprotected mixed derivative must not be erased"
    );
    for axis in [0, 1] {
        for value in product.values() {
            assert_eq!(
                c.specialize_fixed_indices(value, &[(axis, 1)], Default::default())
                    .unwrap()
                    .0,
                c.specialize_fixed_indices(value, &[(axis, 3)], Default::default())
                    .unwrap()
                    .0,
                "protected-power derivative cancels exactly"
            );
        }
    }
}

#[test]
fn typed_external_directions_and_arbitrary_recenter_use_native_chronology() {
    let f = family(true);
    let g = ParametricIbpGenerator::try_new(&f).unwrap();
    let (completed, count) = completed(&g);
    let mut r = request(&f);
    r["nomination"]["recenter"] = json!([-2, 0, 3]);
    let n = nominate(&f, &g, &completed, &r).unwrap();
    assert_eq!(n.nomination.report["recenter"], json!([-2, 0, 3]));
    assert!(n.nomination.weights.keys().any(|(row, _)| matches!(
        row,
        RowId::OrdinaryIbp {
            contraction_momentum: 2,
            differentiated_loop: 0
        }
    )));
    let (product, _) = common_product(&g, &completed, count, n, &r);
    assert!(!product.is_empty());
}

#[test]
fn protection_order_reversal_negates_full_native_source_identity() {
    let f = family(false);
    let g = ParametricIbpGenerator::try_new(&f).unwrap();
    let (completed, _) = completed(&g);
    let mut r = request(&f);
    let first = nominate(&f, &g, &completed, &r).unwrap();
    r["nomination"]["protected_denominators"] = json!([1, 0]);
    let reverse = nominate(&f, &g, &completed, &r).unwrap();
    assert_eq!(
        first.nomination.weights.len(),
        reverse.nomination.weights.len()
    );
    for (key, value) in &first.nomination.weights {
        assert_eq!(
            g.context()
                .neg_with_limits(value, Default::default())
                .unwrap(),
            reverse.nomination.weights[key]
        );
    }
    assert_eq!(first.full_product.len(), reverse.full_product.len());
    for (key, value) in &first.full_product {
        assert_eq!(
            g.context()
                .neg_with_limits(value, Default::default())
                .unwrap(),
            reverse.full_product[key]
        );
    }
}

#[test]
fn degree_three_verification_is_not_degree_three_nomination() {
    let f = family(false);
    let r = request(&f);
    let spec = parse(&r, f.denominator_count()).unwrap();
    let mut bounded = limits(&r, f.denominator_count()).unwrap();
    bounded.max_polynomial_degree = 2;
    let error = TangentSourcePlan::try_new(&f, &spec, bounded)
        .unwrap_err()
        .to_string();
    assert!(error.contains("degree"), "{error}");
    let plan =
        TangentSourcePlan::try_new(&f, &spec, limits(&r, f.denominator_count()).unwrap()).unwrap();
    assert!(
        plan.vector_coefficients()
            .iter()
            .flat_map(|p| p.terms())
            .any(|(_, e)| e.iter().copied().sum::<u16>() == 2)
    );
    assert!(
        plan.vector_coefficients()
            .iter()
            .flat_map(|p| p.terms())
            .all(|(_, e)| e.iter().copied().sum::<u16>() <= 2)
    );
}

#[test]
fn malformed_or_degenerate_nomination_and_allowance_misses_refuse() {
    let f = family(false);
    let g = ParametricIbpGenerator::try_new(&f).unwrap();
    let (completed, _) = completed(&g);
    let mut shape = request(&f);
    // No radial numerator or strictly-negative recenter convention is imposed.
    assert_eq!(crate::validate(&shape).unwrap(), f.denominator_count());
    shape["sources"] = json!([]);
    assert!(crate::validate(&shape).is_err());
    for bad in [
        json!({"protected_denominators":[0,0]}),
        json!({"protected_denominators":[0,99]}),
        json!({"differentiated_loop":99}),
        json!({"recenter":[0]}),
        json!({"multiplier":null}),
        json!({"numerator_axis":2}),
        json!({"mode":"radial"}),
        json!({"contractions":[{"kind":"loop","index":0},{"kind":"loop","index":0},{"kind":"loop","index":2}]}),
        json!({"contractions":[{"kind":"loop","index":0},{"kind":"loop","index":1},{"kind":"external","index":0}]}),
    ] {
        let mut r = request(&f);
        r["nomination"]
            .as_object_mut()
            .unwrap()
            .extend(bad.as_object().unwrap().clone());
        assert!(nominate(&f, &g, &completed, &r).is_err(), "{bad}");
    }
    let mut r = request(&f);
    r["nomination"]["protected_denominators"] = json!([3, 5]); // Both independent of k0.
    assert!(nominate(&f, &g, &completed, &r).is_err());
    for key in [
        "max_source_rows",
        "max_terms",
        "max_conditions",
        "max_term_operations",
        "max_coordinate_cells",
    ] {
        let mut r = request(&f);
        r["limits"][key] = json!(1);
        assert!(nominate(&f, &g, &completed, &r).is_err(), "{key}");
    }
}
