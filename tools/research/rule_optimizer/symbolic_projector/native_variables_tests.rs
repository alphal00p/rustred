use super::*;
use crate::{direct_l, tests::tadpole};
use rustred::{
    algebra::CoefficientContext,
    identity::{IntegralShift, ParametricIbpGenerator, TranslatedSourceRequest},
};

fn context() -> IndexedCoefficientContext {
    let names = (0..15).map(|i| format!("p{i}")).collect::<Vec<_>>();
    IndexedCoefficientContext::try_new(
        &CoefficientContext::try_new(names).unwrap(),
        "compact-map-test",
        1,
    )
    .unwrap()
}
fn shift(n: i64) -> IndexShift {
    static SHIFTS: std::sync::OnceLock<BTreeMap<i64, IndexShift>> = std::sync::OnceLock::new();
    SHIFTS.get_or_init(|| {
        let b = CoefficientContext::try_new(["d"]).unwrap();
        let f = rustred::family::IntegralFamily::new(
            "compact-shift-fixture",
            vec!["q".into()],
            Vec::new(),
            b.clone(),
            b.parameter("d").unwrap(),
            vec![rustred::family::AffineDenominator::new(
                b.integer(-1),
                vec![b.one()],
            )],
            Vec::new(),
            vec![b.zero()],
        )
        .unwrap();
        let generator = ParametricIbpGenerator::try_new(&f).unwrap();
        let p = generator.prepare_ordinary_ibp().unwrap();
        let rows = (0..p.len()).map(|i| p.generate(i)).collect();
        let complete = p.complete(rows).unwrap();
        let batch = generator
            .translate_selected_completed_source_rows(
                &complete,
                (-3..=3)
                    .map(|i| TranslatedSourceRequest::new(0, IntegralShift::try_new([i]).unwrap())),
                Default::default(),
            )
            .unwrap();
        batch
            .sources()
            .iter()
            .flat_map(|s| s.terms().keys().map(|s| (s.values()[0], s.clone())))
            .collect()
    })[&n]
        .clone()
}
fn limits() -> Limits {
    Limits {
        arithmetic: Default::default(),
        rows: 64,
        columns: 4096,
        nonzeros: 100000,
        coefficient_terms: 1000000,
        operations: 1000000,
        guards: 100000,
    }
}
fn guards_equal(a: &[Guard], b: &[Guard]) {
    assert_eq!(a.len(), b.len());
    for (a, b) in a.iter().zip(b) {
        assert_eq!(a.polynomial, b.polynomial);
        assert_eq!(a.origin, b.origin);
    }
}
fn equal(a: Projection, b: Projection) {
    match (a, b) {
        (Projection::Target(a), Projection::Target(b)) => {
            assert_eq!(a.prefix_rows, b.prefix_rows);
            assert_eq!(a.weights, b.weights);
            assert_eq!(a.image, b.image);
            guards_equal(&a.guards, &b.guards);
        }
        (
            Projection::NoTarget { guards: a, rows: x },
            Projection::NoTarget { guards: b, rows: y },
        ) => {
            assert_eq!(x, y);
            guards_equal(&a, &b);
        }
        _ => panic!("compaction changed target outcome"),
    }
}

#[test]
fn compaction_round_trip_preserves_denominator_only_variable_and_rejects_foreign_map() {
    let c = context();
    let n = c.index(0).unwrap();
    let p = c.lift(&c.base().parameter("p0").unwrap()).unwrap();
    let value = c.div(&n, &p).unwrap();
    let rows = vec![Row::from([(shift(0), value.clone())])];
    let v = Variables::new(&c, &rows, true, limits()).unwrap();
    assert_eq!(v.original.len(), 16);
    assert_eq!(v.active.len(), 2);
    let raw = v.map(value.raw()).unwrap();
    assert_eq!(raw.numerator.variables().len(), 2);
    assert_eq!(v.admit(&c, &raw, limits()).unwrap(), value);
    assert!(
        c.admit_native_result_with_limits(raw.clone(), limits().arithmetic)
            .is_err()
    );
    // Reversing the compact map is not accepted as an equivalent input map.
    let mut reversed = v.active.as_ref().clone();
    reversed.reverse();
    let foreign = Variables::remap(&raw, &v.active, &Arc::new(reversed)).unwrap();
    assert!(v.restore(&foreign).is_err());
    let c2 = IndexedCoefficientContext::try_new(
        &CoefficientContext::try_new(["foreign"]).unwrap(),
        "foreign-map",
        1,
    )
    .unwrap();
    assert!(
        Variables::new(
            &c,
            &[Row::from([(shift(0), c2.index(0).unwrap())])],
            true,
            limits()
        )
        .is_err()
    );
}

#[test]
fn compaction_all_constant_empty_active_and_zero_round_trip() {
    let c = context();
    let rows = vec![Row::from([(shift(0), c.integer(3))])];
    let v = Variables::new(&c, &rows, true, limits()).unwrap();
    assert_eq!(v.active.len(), 0);
    for value in [c.zero(), c.one(), c.integer(-3)] {
        assert_eq!(
            v.admit(&c, &v.map(value.raw()).unwrap(), limits()).unwrap(),
            value
        );
    }
    for backend in [project_with_compaction, direct_l::project_with_compaction] {
        equal(
            backend(&c, &rows, &shift(0), &BTreeSet::new(), &[], limits(), false).unwrap(),
            backend(&c, &rows, &shift(0), &BTreeSet::new(), &[], limits(), true).unwrap(),
        );
    }
}

#[test]
fn compaction_backends_preserve_dependent_rows_cancelled_poles_all_guards_and_full_rhs() {
    let c = context();
    let n = c.index(0).unwrap();
    let inv = c.div(&c.one(), &n).unwrap();
    let rows = vec![
        Row::new(),
        Row::from([
            (shift(1), n.clone()),
            (shift(0), c.one()),
            (shift(-1), inv.clone()),
        ]),
        Row::from([
            (shift(1), n.clone()),
            (shift(0), c.one()),
            (shift(-1), inv.clone()),
        ]),
        Row::from([(shift(1), c.one()), (shift(0), c.one()), (shift(-1), inv)]),
        Row::from([(
            shift(0),
            c.div(&c.one(), &c.sub(&n, &c.integer(7)).unwrap()).unwrap(),
        )]),
    ];
    // Guard-only p14 must remain on the original map despite being absent from all images.
    let only_guard = c.lift(&c.base().parameter("p14").unwrap()).unwrap();
    let guards = vec![Guard {
        polynomial: c
            .numerator_condition_with_limits(&only_guard, limits().arithmetic)
            .unwrap(),
        origin: "guard-only base variable".into(),
    }];
    let f = BTreeSet::from([shift(1), shift(3)]);
    for backend in [project_with_compaction, direct_l::project_with_compaction] {
        equal(
            backend(&c, &rows, &shift(0), &f, &guards, limits(), false).unwrap(),
            backend(&c, &rows, &shift(0), &f, &guards, limits(), true).unwrap(),
        );
        let miss = vec![
            Row::new(),
            Row::from([(shift(1), n.clone())]),
            Row::from([(shift(1), n.clone())]),
        ];
        equal(
            backend(&c, &miss, &shift(0), &f, &guards, limits(), false).unwrap(),
            backend(&c, &miss, &shift(0), &f, &guards, limits(), true).unwrap(),
        );
    }
    equal(
        project_with_compaction(&c, &rows, &shift(0), &f, &guards, limits(), true).unwrap(),
        direct_l::project_with_compaction(&c, &rows, &shift(0), &f, &guards, limits(), true)
            .unwrap(),
    );
}

#[test]
fn compaction_checked_export_bytes_and_default_off_reports_match() {
    let (bytes, mut r) = tadpole();
    r["sources"] = serde_json::json!([{"source_row":"ordinary-ibp:0:0","offset":[-1]}]);
    r["max_refinements"] = serde_json::json!(0);
    let (mut reference, artifact) = crate::run::<1>(&bytes, &r, true).unwrap();
    assert_eq!(reference["status"], "CHECKED_PRIORITY_OWNER_EXPORTED");
    reference.as_object_mut().unwrap().remove("seconds");
    for backend in ["augmented", "direct-l"] {
        for compact in [false, true] {
            let mut request = r.clone();
            request["projection_backend"] = serde_json::json!(backend);
            request["compact_coefficient_variables"] = serde_json::json!(compact);
            let (mut result, output) = crate::run::<1>(&bytes, &request, true).unwrap();
            assert_eq!(result["status"], "CHECKED_PRIORITY_OWNER_EXPORTED");
            assert_eq!(output, artifact);
            result.as_object_mut().unwrap().remove("seconds");
            result["request"] = reference["request"].clone();
            assert_eq!(result, reference);
        }
    }
    for invalid in [
        serde_json::json!(null),
        serde_json::json!(1),
        serde_json::json!("true"),
    ] {
        r["compact_coefficient_variables"] = invalid;
        assert!(crate::validate(&r).is_err());
    }
}
