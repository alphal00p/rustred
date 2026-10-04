use super::super::tests::{context, limits};
use super::*;
use rustred::{algebra::IndexedCoefficient, identity::RowId};

fn poly(c: &IndexedCoefficientContext, value: &IndexedCoefficient) -> IndexedPolynomial {
    c.numerator_condition_with_limits(value, Default::default())
        .unwrap()
}
fn weight() -> ParametricGuardOrigin {
    ParametricGuardOrigin::SourceCombinationDenominator {
        source_ordinal: 0,
        row_id: RowId::OrdinaryIbp {
            differentiated_loop: 0,
            contraction_momentum: 0,
        },
    }
}
fn caller() -> ParametricGuardOrigin {
    ParametricGuardOrigin::OriginalDomainCondition {
        condition_ordinal: 0,
    }
}

#[test]
fn weight_units_require_exclusively_weight_origins_and_no_free_indices() {
    let c = context("guard-origin");
    let d = poly(&c, &c.lift(&c.base().parameter("d").unwrap()).unwrap());
    assert!(
        classify(&c, &d, &[weight()], &[], limits())
            .unwrap()
            .accepted
    );
    for origins in [vec![], vec![caller()], vec![weight(), caller()]] {
        assert!(!classify(&c, &d, &origins, &[], limits()).unwrap().accepted);
    }
    let n = poly(&c, &c.index(0).unwrap());
    assert!(
        !classify(&c, &n, &[weight()], &[], limits())
            .unwrap()
            .accepted
    );
    assert_eq!(
        origins_json(&[caller()])[0]["kind"],
        "OriginalDomainCondition"
    );
}

#[test]
fn an_old_guard_can_lose_its_only_runtime_denominator() {
    let c = context("lost-pole");
    let n = poly(&c, &c.index(0).unwrap());
    assert!(
        classify(&c, &n, &[caller()], &[primitive(&n).unwrap()], limits())
            .unwrap()
            .accepted
    );
    assert!(
        !classify(
            &c,
            &n,
            &[caller()],
            &[primitive(&poly(&c, &c.one())).unwrap()],
            limits()
        )
        .unwrap()
        .accepted
    );
    let scaled = poly(&c, &c.mul(&c.integer(-3), &c.index(0).unwrap()).unwrap());
    assert!(
        classify(
            &c,
            &scaled,
            &[caller()],
            &[primitive(&n).unwrap()],
            limits()
        )
        .unwrap()
        .accepted
    );
}

#[test]
fn coupled_conditions_do_not_gain_unimplemented_implication() {
    let c = context("coupled-guard");
    let n = c.index(0).unwrap();
    let a = c.add(&n, &c.one()).unwrap();
    let b = c.add(&n, &c.integer(2)).unwrap();
    let product = poly(&c, &c.mul(&a, &b).unwrap());
    let denominators = [
        primitive(&poly(&c, &a)).unwrap(),
        primitive(&poly(&c, &b)).unwrap(),
    ];
    assert!(
        !classify(&c, &product, &[caller()], &denominators, limits())
            .unwrap()
            .accepted
    );
}

#[test]
fn exact_cell_specialization_precedes_constant_and_weight_classification() {
    let c = context("guard-cell");
    let g = poly(&c, &c.sub(&c.index(0).unwrap(), &c.one()).unwrap());
    let at_two = c
        .specialize_fixed_polynomial(&g, &[(0, 2)], Default::default())
        .unwrap();
    assert!(
        classify(&c, &at_two, &[caller()], &[], limits())
            .unwrap()
            .accepted
    );
    let at_one = c
        .specialize_fixed_polynomial(&g, &[(0, 1)], Default::default())
        .unwrap();
    assert!(
        !classify(&c, &at_one, &[weight()], &[], limits())
            .unwrap()
            .accepted
    );
    let foreign = context("other-map");
    assert!(classify(&foreign, &g, &[weight()], &[], limits()).is_err());
}

fn fixture() -> (Vec<u8>, Value) {
    let (bytes, mut r) = crate::tests::tadpole();
    r["sources"] = json!([{"source_row":"ordinary-ibp:0:0","offset":[-1]}]);
    r["max_refinements"] = json!(0);
    r["chart"] = json!({"lower":[1],"upper":[1],"fixed":[[0,2]]});
    r["boundary_correction"] = json!({"schema":"rustred.boundary-correction.v1","fixed_pinch_axis":0,
        "correction_sources":[{"source_row":"ordinary-ibp:0:0","offset":[-2]}],
        "cancel_rank_positive_shifts":[],"forbid_new_rank_positive":true});
    (bytes, r)
}

#[test]
fn diagnostic_default_off_identity_and_native_success_are_separate_from_export() {
    let (bytes, mut r) = fixture();
    let old = crate::run::<1>(&bytes, &r, false).unwrap().0;
    r["boundary_correction"]["export_guard_diagnostic"] = json!(false);
    let off = crate::run::<1>(&bytes, &r, false).unwrap().0;
    assert_eq!(old["attempts"], off["attempts"]);
    r["boundary_correction"]["export_guard_diagnostic"] = json!(true);
    let (mut on, payload) = crate::run::<1>(&bytes, &r, false).unwrap();
    assert!(payload.is_none());
    let d = on["attempts"][0]["boundary_correction"]
        .as_object_mut()
        .unwrap()
        .remove("export_guard_diagnostic")
        .unwrap();
    assert_eq!(d["complete"], true, "{d}");
    assert_eq!(d["mirrored_transport_accepts"], true);
    assert_eq!(d["mirrored_priority_source_sha256"], SOURCE_SHA256);
    assert_eq!(d["native_export_authority"], false);
    assert_eq!(old["attempts"], on["attempts"]);
    for bad in [Value::Null, json!(1), json!("true")] {
        r["boundary_correction"]["export_guard_diagnostic"] = bad;
        assert!(crate::validate(&r).is_err());
    }
}

#[test]
fn diagnostic_bounds_fail_closed_without_changing_the_native_proof() {
    let (bytes, mut r) = fixture();
    r["boundary_correction"]["export_guard_diagnostic"] = json!(true);
    // Insufficient diagnostic work is nested; it cannot turn into guard admission.
    let c = context("diagnostic-budget");
    let mut count = 0;
    assert!(charge(&mut count, 2, 1, "fixture work").is_err());
    count = usize::MAX;
    assert!(charge(&mut count, 1, usize::MAX, "fixture overflow").is_err());
    let request = super::super::tests::template(&c);
    assert!(prepare(&c, "fixture", &request, Default::default(), 0, limits()).is_err());
    let report = crate::run::<1>(&bytes, &r, false).unwrap().0;
    assert_eq!(
        report["attempts"][0]["boundary_correction"]["original_source_replay_verified"],
        true
    );
    assert_eq!(
        report["attempts"][0]["boundary_correction"]["exported"],
        false
    );

    // Exercise the actual successful-checker hook with a refused diagnostic.
    use super::super::{
        CandidateOwnerBundle, IntegralShift, Mask, ParametricIbpGenerator, TranslatedSourceRequest,
        load_generated_candidate_owners,
    };
    let mask = Mask::try_new([true]).unwrap();
    let (family, _) = load_generated_candidate_owners::<1>(
        &[CandidateOwnerBundle {
            bytes: &bytes,
            owner_sector: &mask,
        }],
        Default::default(),
        Default::default(),
    )
    .unwrap();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let ordinary = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..ordinary.len()).map(|i| ordinary.generate(i)).collect();
    let completed = ordinary.complete(rows).unwrap();
    let batch = generator
        .translate_selected_completed_source_rows(
            &completed,
            [TranslatedSourceRequest::new(
                0,
                IntegralShift::try_new([-1]).unwrap(),
            )],
            Default::default(),
        )
        .unwrap();
    let c = generator.context();
    let terms = batch.sources()[0].terms();
    let target = terms.iter().find(|(s, _)| s.values() == [0]).unwrap().1;
    let weight = c.div(&c.one(), target).unwrap();
    let mut request = super::super::tests::template(c);
    request.lower = vec![1];
    request.upper = vec![Some(1)];
    request.fixed = vec![rustred::foundry::cell::FixedIndexRestriction::new(0, 2)];
    request.contributions[0].offset = IntegralShift::try_new([-1]).unwrap();
    request.contributions[0].weight = weight.clone();
    request.rhs = terms
        .iter()
        .filter(|(s, _)| s.values() != [0])
        .map(|(s, v)| {
            (
                s.clone(),
                c.mul(&c.integer(-1), &c.mul(&weight, v).unwrap()).unwrap(),
            )
        })
        .collect();
    let (refused, _) = super::super::finish_proof(
        c,
        &family,
        request,
        Default::default(),
        None,
        true,
        json!({"exported":false,"fresh_original_source_certificate":false}),
        Some(Err("fixture diagnostic limit".into())),
        limits(),
    );
    assert_eq!(
        refused.report["original_source_replay_verified"], true,
        "{}",
        refused.report
    );
    assert_eq!(refused.report["export_guard_diagnostic"]["complete"], false);
    assert_eq!(
        refused.report["export_guard_diagnostic"]["status"],
        "GUARD_DIAGNOSTIC_REFUSED_OR_INCOMPLETE"
    );
    assert_eq!(refused.report["exported"], false);
    assert!(refused.export_request.is_none());
}
