use super::*;
use rustred::{algebra::IndexedCoefficient, family::IntegralFamily};
use std::sync::Arc;

fn fixture() -> (Vec<u8>, Value) {
    let (bytes, mut r) = crate::tests::tadpole();
    r["sources"] = json!([{"source_row":"ordinary-ibp:0:0","offset":[-1]}]);
    r["max_refinements"] = json!(0);
    r["chart"] = json!({"lower":[1],"upper":[1],"fixed":[[0,2]]});
    r["fresh_original_source_certificate"] = json!(true);
    r["boundary_correction"] = json!({"schema":super::super::SCHEMA,"fixed_pinch_axis":0,
        "correction_sources":[{"source_row":"ordinary-ibp:0:0","offset":[-2]}],
        "cancel_rank_positive_shifts":[],"forbid_new_rank_positive":true});
    (bytes, r)
}

struct Typed {
    bytes: Vec<u8>,
    r: Value,
    family: Arc<IntegralFamily>,
    c: IndexedCoefficientContext,
    span: source::Span,
    request: OriginalSourceCombinationRequest,
    target: IndexShift,
    image: project::Row,
}
fn typed() -> Typed {
    let (bytes, r) = fixture();
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
    let c = generator.context().clone();
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
    let limits = projection_limits(&r).unwrap();
    let span = source::Span::ordinary(&c, &batch, &[(0, 2)], Default::default(), limits).unwrap();
    let target = span.images[0]
        .keys()
        .find(|s| s.values() == [0])
        .unwrap()
        .clone();
    let project::Projection::Target(proposal) = project::project(
        &c,
        &span.images,
        &target,
        &BTreeSet::new(),
        &span.guards,
        limits,
    )
    .unwrap() else {
        panic!("native target missing")
    };
    let (contributions, guards) = span.compose(&c, &proposal, limits).unwrap();
    let request = OriginalSourceCombinationRequest {
        root_sector: mask.clone(),
        sector: mask,
        ordering: rustred::sector::OrderingPolicy::SpiredUncutV1,
        lower: vec![1],
        upper: vec![Some(1)],
        fixed: vec![FixedIndexRestriction::new(0, 2)],
        contributions,
        rhs: proposal
            .image
            .iter()
            .filter(|(s, _)| *s != &target)
            .map(|(s, v)| (s.clone(), c.neg_with_limits(v, limits.arithmetic).unwrap()))
            .collect(),
        retained_conditions: guards.into_iter().map(|g| g.polynomial).collect(),
    };
    let request = certificate::fresh(&c, &span, request, limits).unwrap();
    Typed {
        bytes,
        r,
        family,
        c,
        span,
        request,
        target,
        image: proposal.image,
    }
}

#[test]
fn fresh_frame_flag_is_strict_and_default_false_is_exact_identity() {
    let (bytes, mut r) = fixture();
    let (old, a) = crate::run::<1>(&bytes, &r, true).unwrap();
    r["boundary_correction"]["fresh_original_frame_reproof"] = json!(false);
    let (off, b) = crate::run::<1>(&bytes, &r, true).unwrap();
    assert_eq!(old["attempts"], off["attempts"]);
    assert_eq!(a, b);
    for bad in [Value::Null, json!(1), json!("true"), json!([])] {
        r["boundary_correction"]["fresh_original_frame_reproof"] = bad;
        assert!(crate::validate(&r).is_err());
    }
}

#[test]
fn fresh_frame_native_proof_export_and_cold_load_preserve_weighted_result() {
    let (bytes, mut r) = fixture();
    let old = crate::run::<1>(&bytes, &r, false).unwrap().0;
    r["boundary_correction"]["fresh_original_frame_reproof"] = json!(true);
    let (proved, none) = crate::run::<1>(&bytes, &r, false).unwrap();
    assert!(none.is_none());
    let (exported, payload) = crate::run::<1>(&bytes, &r, true).unwrap();
    assert_eq!(
        exported["status"], "CHECKED_PRIORITY_OWNER_EXPORTED",
        "{exported}"
    );
    let b = &proved["attempts"][0]["boundary_correction"];
    assert_eq!(b["fresh_original_source_certificate"], false);
    let f = &b["fresh_original_frame_reproof"];
    assert_eq!(
        f["status"], "EXACT_FRESH_ORIGINAL_FRAME_CHART_PROVED",
        "{f}"
    );
    assert_eq!(f["original_source_replay_verified"], true);
    assert_eq!(f["cell_coverage_and_rhs_equal"], true);
    assert_eq!(f["weighted_proof_modified"], false);
    let mut stripped = proved.clone();
    stripped["attempts"][0]["boundary_correction"]
        .as_object_mut()
        .unwrap()
        .remove("fresh_original_frame_reproof");
    assert_eq!(stripped["attempts"], old["attempts"]);
    let payload = payload.unwrap();
    let mask = Mask::try_new([true]).unwrap();
    let (_, programs) = load_generated_candidate_owners::<1>(
        &[CandidateOwnerBundle {
            bytes: &payload,
            owner_sector: &mask,
        }],
        Default::default(),
        Default::default(),
    )
    .unwrap();
    let mut selections = Vec::new();
    programs
        .visit_owner_domain_matches(
            [true],
            &[1],
            &[Some(1)],
            Some(0),
            Default::default(),
            &std::sync::atomic::AtomicBool::new(false),
            |piece| {
                selections.push(piece.disposition());
                std::ops::ControlFlow::Continue(())
            },
        )
        .unwrap();
    assert_eq!(
        selections,
        vec![rustred::solver::OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 0 }]
    );
}

#[test]
fn fresh_frame_refuses_ambiguous_baseline_and_never_exports_fallback() {
    let (bytes, mut r) = fixture();
    r["fresh_original_source_certificate"] = json!(false);
    r["boundary_correction"]["fresh_original_frame_reproof"] = json!(true);
    let (report, payload) = crate::run::<1>(&bytes, &r, true).unwrap();
    assert!(payload.is_none());
    let b = &report["attempts"][0]["boundary_correction"];
    assert_eq!(b["original_source_replay_verified"], true, "{report}");
    assert_eq!(
        b["fresh_original_frame_reproof"]["original_source_replay_verified"],
        false
    );
    assert!(
        b["fresh_original_frame_reproof"]["error"]
            .as_str()
            .unwrap()
            .contains("fresh stage-one")
    );
    assert!(report["attempts"][0].get("export").is_none());
}

#[test]
fn canonical_weights_refuse_duplicate_missing_and_foreign_bindings() {
    let mut t = typed();
    let limits = projection_limits(&t.r).unwrap();
    mapped_weights(&t.c, &t.span, &t.request, limits).unwrap();
    t.request
        .contributions
        .push(t.request.contributions[0].clone());
    assert!(mapped_weights(&t.c, &t.span, &t.request, limits).is_err());
    t.request.contributions.pop();
    t.request.contributions[0].offset = IntegralShift::try_new([9]).unwrap();
    assert!(mapped_weights(&t.c, &t.span, &t.request, limits).is_err());
    t.request.contributions[0].offset = IntegralShift::try_new([-1]).unwrap();
    let foreign = super::super::tests::context("foreign-frame");
    t.request.contributions[0].weight = foreign.index(0).unwrap();
    assert!(mapped_weights(&t.c, &t.span, &t.request, limits).is_err());
}

#[test]
fn changed_weight_target_rhs_or_full_image_cannot_be_reproved() {
    let mut t = typed();
    let limits = projection_limits(&t.r).unwrap();
    verify_image(&t.c, &t.span, &t.request, &t.target, &t.image, limits).unwrap();
    let old = t.request.contributions[0].weight.clone();
    t.request.contributions[0].weight = t.c.mul(&old, &t.c.integer(2)).unwrap();
    assert!(verify_image(&t.c, &t.span, &t.request, &t.target, &t.image, limits).is_err());
    t.request.contributions[0].weight = old;
    let old = t.request.rhs[0].1.clone();
    t.request.rhs[0].1 = t.c.mul(&old, &t.c.integer(2)).unwrap();
    assert!(verify_image(&t.c, &t.span, &t.request, &t.target, &t.image, limits).is_err());
    t.request.rhs[0].1 = old;
    t.image.insert(t.target.clone(), t.c.integer(2));
    assert!(verify_image(&t.c, &t.span, &t.request, &t.target, &t.image, limits).is_err());
}

#[test]
fn ordinary_constructor_and_known_assumption_inventory_are_not_inferred() {
    let mut t = typed();
    let limits = projection_limits(&t.r).unwrap();
    let genuine =
        t.c.numerator_condition_with_limits(
            &t.c.lift(&t.c.base().parameter("d").unwrap()).unwrap(),
            Default::default(),
        )
        .unwrap();
    project::retain(
        &t.c,
        &mut t.span.guards,
        genuine,
        "registered caller fixture",
        limits,
    )
    .unwrap();
    t.request.retained_conditions = t.span.guards.iter().map(|g| g.polynomial.clone()).collect();
    baseline_assumptions(&t.c, &t.span, &t.request, limits).unwrap();
    t.request.retained_conditions.clear();
    assert!(baseline_assumptions(&t.c, &t.span, &t.request, limits).is_err());
    t.request.retained_conditions = t.span.guards.iter().map(|g| g.polynomial.clone()).collect();
    t.span.provenance = source::SpanProvenance::Weighted;
    assert!(baseline_assumptions(&t.c, &t.span, &t.request, limits).is_err());
    assert!(certificate::fresh(&t.c, &t.span, t.request, limits).is_err());
}

#[test]
fn genuine_caller_condition_survives_and_export_refusal_stays_authoritative() {
    let mut t = typed();
    let limits = projection_limits(&t.r).unwrap();
    let value: IndexedCoefficient =
        t.c.sub(
            &t.c.lift(&t.c.base().parameter("d").unwrap()).unwrap(),
            &t.c.integer(3),
        )
        .unwrap();
    let condition =
        t.c.numerator_condition_with_limits(&value, Default::default())
            .unwrap();
    project::retain(
        &t.c,
        &mut t.span.guards,
        condition.clone(),
        "genuine caller fixture",
        limits,
    )
    .unwrap();
    t.request.retained_conditions = t.span.guards.iter().map(|g| g.polynomial.clone()).collect();
    baseline_assumptions(&t.c, &t.span, &t.request, limits).unwrap();
    let request = certificate::fresh(&t.c, &t.span, t.request, limits).unwrap();
    assert!(request.retained_conditions.contains(&condition));
    check_original_source_combination(&t.family, request.clone(), Default::default()).unwrap();
    let error = encode_checked_priority_owner_with_policy::<1>(
        &t.bytes,
        request,
        Default::default(),
        Default::default(),
        RuleDispatchPolicy::AfterBaselinePartitionWholePiece,
    )
    .err()
    .unwrap();
    assert!(
        error.to_string().contains("guard is not retained"),
        "{error}"
    );
}

#[test]
fn fresh_frame_resource_and_native_proof_failures_are_not_success() {
    let t = typed();
    let mut limits = projection_limits(&t.r).unwrap();
    limits.rows = 0;
    assert!(mapped_weights(&t.c, &t.span, &t.request, limits).is_err());
    let mut invalid = t.request;
    invalid.rhs.clear();
    assert!(check_original_source_combination(&t.family, invalid, Default::default()).is_err());
}
