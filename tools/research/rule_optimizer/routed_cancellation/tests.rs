use super::*;
use rustred::algebra::CoefficientContext;

fn context(scope: &str) -> IndexedCoefficientContext {
    IndexedCoefficientContext::try_new(&CoefficientContext::try_new(["d"]).unwrap(), scope, 2)
        .unwrap()
}
fn limits() -> Limits {
    Limits {
        max_route_calls: 10,
        max_endpoint_occurrences: 100,
        max_distinct_keys: 50,
        max_coefficient_terms: 1000,
        max_ledger_records: 1000,
        max_report_bytes: 1 << 20,
        expansion: BTreeMap::new(),
    }
}
fn key(powers: [i64; 2]) -> IntegralKey {
    IntegralKey::try_new(powers).unwrap()
}
fn sum<'a>(c: &'a IndexedCoefficientContext, l: &'a Limits) -> Sum<'a> {
    Sum {
        context: c,
        exact: Default::default(),
        limits: l,
        map: BTreeMap::new(),
        occurrences: 0,
        additions: 0,
    }
}

#[test]
fn whole_parent_opposite_weights_remove_key_not_just_duplicate_it() {
    let c = context("cancel");
    let l = limits();
    let mut s = sum(&c, &l);
    let d = c.lift(&c.base().parameter("d").unwrap()).unwrap();
    let neg = c.neg_with_limits(&d, Default::default()).unwrap();
    s.add(key([2, 1]), &d).unwrap();
    s.add(key([2, 1]), &neg).unwrap();
    assert_eq!(s.occurrences, 2);
    assert_eq!(s.map.len(), 1);
    assert!(s.map[&key([2, 1])].is_zero());
    s.add(key([2, 1]), &c.one()).unwrap();
    assert_eq!(s.map[&key([2, 1])], c.one());
}

#[test]
fn owner_support_exponent_and_parent_equations_remain_distinct() {
    let c = context("keys");
    let l = limits();
    let mut a = sum(&c, &l);
    let mut b = sum(&c, &l);
    a.add(key([2, 1]), &c.one()).unwrap();
    a.add(key([1, 2]), &c.integer(-1)).unwrap();
    a.add(key([0, 2]), &c.integer(-1)).unwrap();
    b.add(key([2, 1]), &c.integer(-1)).unwrap();
    assert_eq!(a.map.len(), 3);
    assert_eq!(b.map.len(), 1);
    assert!(a.map.values().chain(b.map.values()).all(|v| !v.is_zero()));
}

#[test]
fn foreign_context_refused_even_when_coefficient_would_cancel() {
    let c = context("first");
    let foreign = context("second");
    let l = limits();
    let mut s = sum(&c, &l);
    s.add(key([2, 1]), &c.one()).unwrap();
    assert!(s.add(key([2, 1]), &foreign.integer(-1)).is_err());
    assert_eq!(s.map[&key([2, 1])], c.one());
}

#[test]
fn rational_pole_is_retained_in_pre_cancellation_table() {
    let c = context("poles");
    let l = limits();
    let mut s = sum(&c, &l);
    let d = c.lift(&c.base().parameter("d").unwrap()).unwrap();
    let denominator = c.sub(&d, &c.integer(4)).unwrap();
    let value = c.div(&c.one(), &denominator).unwrap();
    let negative = c.neg_with_limits(&value, Default::default()).unwrap();
    let mut evidence = Evidence {
        table: Some(CoefficientTableBuilder::new(Default::default())),
        ledger: Vec::new(),
        max_records: 10,
    };
    evidence.coefficient(&value).unwrap();
    evidence.coefficient(&negative).unwrap();
    s.add(key([2, 1]), &value).unwrap();
    s.add(key([2, 1]), &negative).unwrap();
    assert!(s.map[&key([2, 1])].is_zero());
    assert_eq!(evidence.table.as_ref().unwrap().len(), 2);
    assert!(
        !evidence
            .table
            .take()
            .unwrap()
            .finish()
            .unwrap()
            .atoms
            .is_empty()
    );
    // d=4 was never admitted or specialized: cancellation is generic-field only.
    assert!(c.div(&c.one(), &c.zero()).is_err());
}

#[test]
fn endpoint_exhaustion_after_prefix_is_error_not_complete_map() {
    let c = context("budget");
    let mut l = limits();
    l.max_endpoint_occurrences = 1;
    let mut s = sum(&c, &l);
    s.add(key([2, 1]), &c.one()).unwrap();
    assert!(s.add(key([2, 1]), &c.integer(-1)).is_err());
    assert_eq!(s.occurrences, 1);
    assert!(!s.map[&key([2, 1])].is_zero());
    let mut used = 9;
    assert!(charge(&mut used, 2, 10, "native work").is_err());
    assert_eq!(used, 9);
}

#[test]
fn fixed_coefficient_check_rejects_index_dependence() {
    let c = context("fixed");
    let index = c.index(0).unwrap();
    let variables = index.raw().numerator.variables();
    let positions: Vec<_> = (0..variables.len())
        .filter(|i| {
            index
                .raw()
                .numerator
                .exponents
                .chunks_exact(variables.len())
                .any(|p| p[*i] != 0)
        })
        .collect();
    assert_eq!(positions.len(), 1);
    assert!(require_fixed_coefficient(&index, &positions).is_err());
    assert!(require_fixed_coefficient(&c.integer(3), &positions).is_ok());
}

#[test]
fn actual_manifest_route_field_required_and_literal_preserved() {
    let text = r#"{"initial_frontier_routes":[{"source_mask":"10","owner_mask":"01","requires_transport":true,"source_to_representative":[["1"]],"owner_to_representative":[["-1"]]}],"owners":[]}"#;
    let selection: Selection = serde_json::from_str(text).unwrap();
    assert_eq!(selection.routing.len(), 1);
    assert_eq!(selection.routing[0].owner_to_representative[0][0], "-1");
    assert!(serde_json::from_str::<Selection>(r#"{"routing":[]}"#).is_err());
}

#[test]
fn final_original_encoding_refusal_is_not_complete_evidence() {
    use crate::applied_observer::input::RecorderLimits;
    let directory = std::env::temp_dir().join(format!(
        "routed-cancellation-encoding-{}",
        std::process::id()
    ));
    fs::create_dir(&directory).unwrap();
    let limits = RecorderLimits {
        max_input_bytes: 1 << 20,
        max_queries: 1,
        max_events: 100,
        max_json_bytes: 1 << 20,
        max_record_bytes: 1 << 16,
        max_coefficients: 10,
        max_atom_bytes: 1 << 16,
        max_total_atom_bytes: 1 << 16,
        max_state_bytes: 1,
        max_error_bytes: 128,
    };
    let recorder = Recorder::new(&directory, limits).unwrap();
    let recording_complete = recorder.finish(json!({}), true).unwrap();
    assert!(!recording_complete);
    let result: Value =
        serde_json::from_slice(&fs::read(directory.join("result.json")).unwrap()).unwrap();
    assert_eq!(result["complete_outcome"], false);
    assert!(!result["coefficient_encoding_error"].is_null());
    // Deliberately retain this tiny fixture beneath the guarded workspace TMPDIR.
}

#[test]
fn singleton_signs_and_overflow_fail_closed() {
    assert_eq!(
        singleton(&[true, false], &[1, 3], &[Some(1), Some(3)]).unwrap(),
        key([2, -3])
    );
    assert!(singleton(&[true, false], &[1, 3], &[Some(2), Some(3)]).is_err());
    assert!(singleton(&[true], &[i64::MAX as u64], &[Some(i64::MAX as u64)]).is_err());
}

fn symmetric_family() -> Arc<IntegralFamily> {
    use rustred::family::AffineDenominator;
    let c = CoefficientContext::try_new(["d"]).unwrap();
    let z = c.zero();
    let o = c.one();
    Arc::new(
        IntegralFamily::new(
            "routed-cancellation-test",
            vec!["k1".into(), "k2".into()],
            vec![],
            c.clone(),
            c.parameter("d").unwrap(),
            vec![
                AffineDenominator::new(o.clone(), vec![o.clone(), z.clone(), z.clone()]),
                AffineDenominator::new(o.clone(), vec![z.clone(), z.clone(), o.clone()]),
                AffineDenominator::new(z.clone(), vec![z.clone(), o.clone(), z.clone()]),
            ],
            vec![],
            vec![z.clone(), z.clone(), z],
        )
        .unwrap(),
    )
}

#[test]
fn distinct_native_verified_routes_really_cancel_at_one_target() {
    let family = symmetric_family();
    let identity = vec![vec!["1".into(), "0".into()], vec!["0".into(), "1".into()]];
    let swap = vec![vec!["0".into(), "1".into()], vec!["1".into(), "0".into()]];
    let first = Route {
        source_mask: "100".into(),
        owner_mask: "100".into(),
        requires_transport: true,
        source_to_representative: identity.clone(),
        owner_to_representative: identity.clone(),
    };
    let second = Route {
        source_mask: "010".into(),
        owner_mask: "100".into(),
        requires_transport: true,
        source_to_representative: swap,
        owner_to_representative: identity,
    };
    let a = reverify::<3>(&family, &first).unwrap();
    let b = reverify::<3>(&family, &second).unwrap();
    let ka = IntegralKey::try_new([2, 0, 0]).unwrap();
    let kb = IntegralKey::try_new([0, 2, 0]).unwrap();
    let ta = a.transport(&ka, Default::default()).unwrap();
    let tb = b.transport(&kb, Default::default()).unwrap();
    assert_eq!(ta.terms().len(), 1);
    assert_eq!(tb.terms().len(), 1);
    assert_eq!(ta.terms()[0].key(), tb.terms()[0].key());
    let c =
        IndexedCoefficientContext::try_new(family.coefficient_context(), "transport-fixture", 3)
            .unwrap();
    let l = limits();
    let mut s = sum(&c, &l);
    let positive = c.lift(ta.terms()[0].coefficient()).unwrap();
    let negative = c
        .neg_with_limits(
            &c.lift(tb.terms()[0].coefficient()).unwrap(),
            Default::default(),
        )
        .unwrap();
    s.add(ta.terms()[0].key().clone(), &positive).unwrap();
    s.add(tb.terms()[0].key().clone(), &negative).unwrap();
    assert_eq!(s.map.len(), 1);
    assert!(s.map.values().all(|v| v.is_zero()));
    let mut budget = integral_transport::ExpansionLimits::default();
    budget.max_endpoint_power_entries = 0;
    assert!(b.transport(&kb, budget).is_err());
    let mut invalid = second.clone();
    invalid.owner_mask = "110".into();
    assert!(reverify::<3>(&family, &invalid).is_err());
}
