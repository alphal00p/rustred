use super::*;
use project::{Guard, Projection, Row};
use rustred::{
    algebra::IndexedCoefficientContext, family::IntegralFamily,
    foundry::parametric::ParametricGuardOrigin,
};

fn prepared(
    fixed: &[(usize, i64)],
    offsets: &[i64],
) -> (
    Vec<u8>,
    Value,
    std::sync::Arc<IntegralFamily>,
    IndexedCoefficientContext,
    source::Span,
) {
    let (bytes, r) = crate::tests::tadpole();
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
    let p = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..p.len()).map(|i| p.generate(i)).collect();
    let completed = p.complete(rows).unwrap();
    let batch = generator
        .translate_selected_completed_source_rows(
            &completed,
            offsets
                .iter()
                .map(|&i| TranslatedSourceRequest::new(0, IntegralShift::try_new([i]).unwrap())),
            Default::default(),
        )
        .unwrap();
    let span = source::Span::ordinary(
        &c,
        &batch,
        fixed,
        Default::default(),
        projection_limits(&r).unwrap(),
    )
    .unwrap();
    (bytes, r, family, c, span)
}

fn combination(
    r: &Value,
    c: &IndexedCoefficientContext,
    span: &source::Span,
    active: bool,
    fixed: Vec<FixedIndexRestriction>,
    lower: Vec<u64>,
    upper: Vec<Option<u64>>,
) -> (OriginalSourceCombinationRequest, project::Proposal) {
    let target = span
        .originals
        .iter()
        .flat_map(|r| r.keys())
        .find(|k| k.values() == [0])
        .unwrap()
        .clone();
    let Projection::Target(proposal) = project::project(
        c,
        &span.images,
        &target,
        &BTreeSet::new(),
        &span.guards,
        projection_limits(r).unwrap(),
    )
    .unwrap() else {
        panic!("target missing")
    };
    let (contributions, guards) = span
        .compose(c, &proposal, projection_limits(r).unwrap())
        .unwrap();
    let rhs = proposal
        .image
        .iter()
        .filter(|(k, _)| *k != &target)
        .map(|(k, v)| {
            (
                k.clone(),
                c.neg_with_limits(v, projection_limits(r).unwrap().arithmetic)
                    .unwrap(),
            )
        })
        .collect();
    (
        OriginalSourceCombinationRequest {
            root_sector: Mask::try_new([true]).unwrap(),
            sector: Mask::try_new([active]).unwrap(),
            ordering: rustred::sector::OrderingPolicy::SpiredUncutV1,
            lower,
            upper,
            fixed,
            contributions,
            rhs,
            retained_conditions: guards.iter().map(|g| g.polynomial.clone()).collect(),
        },
        proposal,
    )
}

#[test]
fn fresh_flag_is_strict_optional_and_false_preserves_complete_default_report() {
    assert!(!enabled(&json!({})).unwrap());
    assert!(!enabled(&json!({"fresh_original_source_certificate":false})).unwrap());
    assert!(enabled(&json!({"fresh_original_source_certificate":true})).unwrap());
    let (bytes, r) = crate::tests::tadpole();
    for bad in [json!(null), json!(0), json!(1), json!("true"), json!([])] {
        let mut bad_r = r.clone();
        bad_r["fresh_original_source_certificate"] = bad;
        assert!(validate(&bad_r).is_err());
    }
    let mut explicit = r.clone();
    explicit["fresh_original_source_certificate"] = json!(false);
    let (mut old, old_bytes) = run::<1>(&bytes, &r, true).unwrap();
    let (mut off, off_bytes) = run::<1>(&bytes, &explicit, true).unwrap();
    old.as_object_mut().unwrap().remove("seconds");
    off.as_object_mut().unwrap().remove("seconds");
    off["request"] = old["request"].clone();
    assert_eq!(off, old);
    assert_eq!(off_bytes, old_bytes);
}

#[test]
fn fresh_full_two_row_certificate_exports_same_bytes_as_backward_reference() {
    let (bytes, mut r) = crate::tests::tadpole();
    r["forbid_cofinally_higher_columns"] = json!(true);
    let (old, none) = run::<1>(&bytes, &r, true).unwrap();
    assert_eq!(old["status"], "EXACT_CHART_PROVED_EXPORT_REFUSED");
    assert!(none.is_none());
    r["fresh_original_source_certificate"] = json!(true);
    let (new, actual) = run::<1>(&bytes, &r, true).unwrap();
    assert_eq!(new["status"], "CHECKED_PRIORITY_OWNER_EXPORTED", "{new}");
    assert!(actual.is_some());
    assert_eq!(
        new["attempts"][0]["conditions"],
        old["attempts"][0]["conditions"]
    );
    assert_eq!(
        new["attempts"][0]["ordinary_contributions"],
        old["attempts"][0]["ordinary_contributions"]
    );
    assert_eq!(
        new["attempts"][0]["normalized_full_product"],
        old["attempts"][0]["normalized_full_product"]
    );
    assert_eq!(new["attempts"][0]["full_original_product_replayed"], true);
    let mut backward = r.clone();
    backward
        .as_object_mut()
        .unwrap()
        .remove("fresh_original_source_certificate");
    backward["sources"] = json!([r["sources"][1].clone()]);
    let (reference, expected) = run::<1>(&bytes, &backward, true).unwrap();
    assert_eq!(reference["status"], "CHECKED_PRIORITY_OWNER_EXPORTED");
    assert_eq!(actual, expected);
    for backend in ["direct-l", "source-weights"] {
        let mut alternative = r.clone();
        alternative["projection_backend"] = json!(backend);
        if backend == "source-weights" {
            alternative["source_weight_reconstruction"] = json!({"max_degree":16,"max_probes":4096,
                "max_attempts":4,"max_primes":8,"max_cached_images":512,"max_cached_values":100000,
                "max_weight_slots":1000});
        }
        let (report, artifact) = run::<1>(&bytes, &alternative, true).unwrap();
        assert_eq!(
            report["status"], "CHECKED_PRIORITY_OWNER_EXPORTED",
            "{backend}: {report}"
        );
        assert_eq!(artifact, actual);
        assert_eq!(
            report["attempts"][0]["full_original_product_replayed"],
            true
        );
    }
    // Selection/permutation preserves constructor provenance but is verified.
    r["exact_source_ordinals"] = json!([1, 0]);
    let (reordered, reordered_bytes) = run::<1>(&bytes, &r, true).unwrap();
    assert_eq!(reordered["status"], "CHECKED_PRIORITY_OWNER_EXPORTED");
    assert_eq!(reordered_bytes, actual);
}

#[test]
fn fresh_certificate_retains_genuine_index_weight_pole_and_refuses_boundary() {
    let (bytes, mut r) = crate::tests::tadpole();
    r["sources"] = json!([r["sources"][1].clone()]);
    r["chart"]["lower"] = json!([0]); // physical n=1 makes real weight 1/(n-1) undefined
    r["fresh_original_source_certificate"] = json!(true);
    let (report, artifact) = run::<1>(&bytes, &r, true).unwrap();
    assert!(artifact.is_none());
    assert_eq!(
        report["attempts"][0]["full_original_product_replayed"],
        true
    );
    assert!(report["attempts"][0].get("proof_error").is_some());
    assert_ne!(report["status"], "CHECKED_PRIORITY_OWNER_EXPORTED");
}

#[test]
fn fresh_certificate_preserves_all_original_guards_even_on_unselected_sources() {
    let (_, r, family, c, mut span) = prepared(&[], &[-1, 0]);
    let n_minus_two = c
        .sub_with_limits(
            &c.index(0).unwrap(),
            &c.integer(2),
            projection_limits(&r).unwrap().arithmetic,
        )
        .unwrap();
    let guard = c
        .numerator_condition_with_limits(&n_minus_two, Default::default())
        .unwrap();
    span.guards.push(Guard {
        polynomial: guard.clone(),
        origin: "explicit assumption on unselected row".into(),
    });
    let backward = span
        .bindings
        .iter()
        .position(|b| b.offset.values() == [-1])
        .unwrap();
    selection::apply(&mut span, &[backward]).unwrap();
    let (request, _) = combination(&r, &c, &span, true, vec![], vec![1], vec![None]);
    let fresh = fresh(&c, &span, request, projection_limits(&r).unwrap()).unwrap();
    assert_eq!(
        fresh.retained_conditions,
        span.guards
            .iter()
            .map(|g| g.polynomial.clone())
            .collect::<Vec<_>>()
    );
    assert!(fresh.retained_conditions.contains(&guard));
    assert!(check_original_source_combination(&family, fresh, policy(&r).unwrap()).is_err());
}

#[test]
fn weighted_identity_and_mutated_ordinary_frames_cannot_freshen() {
    let (_, r, _, c, span) = prepared(&[], &[-1, 0]);
    let limits = projection_limits(&r).unwrap();
    let weighted = source::Span::weighted(
        &c,
        span.bindings.clone(),
        span.originals.clone(),
        span.weights.clone(),
        span.guards.clone(),
        limits,
    )
    .unwrap();
    assert!(weighted.validate_fresh_ordinary(&c, limits).is_err());
    for mutation in 0..6 {
        let (_, r, _, c, mut span) = prepared(&[], &[-1, 0]);
        let limits = projection_limits(&r).unwrap();
        match mutation {
            0 => {
                let ordinal = *span.weights[0].first_key_value().unwrap().0;
                span.weights[0].insert(ordinal, c.integer(2));
            }
            1 => span.images[0] = Row::new(),
            2 => span.weights[1] = span.weights[0].clone(),
            3 => span.bindings[1] = span.bindings[0].clone(),
            4 => {
                span.weights[0] = BTreeMap::from([(usize::MAX, c.one())]);
            }
            _ => {
                let foreign = IndexedCoefficientContext::try_new(
                    &rustred::algebra::CoefficientContext::try_new(["d"]).unwrap(),
                    "foreign-fresh-span",
                    1,
                )
                .unwrap();
                let ordinal = *span.weights[0].first_key_value().unwrap().0;
                span.weights[0].insert(ordinal, foreign.one());
            }
        }
        assert!(
            span.validate_fresh_ordinary(&c, limits).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn native_final_base_field_weight_origin_is_distinct_from_genuine_caller_condition() {
    // At physical n=2 the native rows are -2*I(2)+(d-2)*I(1)
    // and d*I(0). Normalize the actual first-row target coefficient, then
    // add the second row with weight 1/d. The full nonempty RHS retains I(0),
    // without a zero-sector quotient or any runtime denominator d.
    let (bytes, r, family, c, mut span) = prepared(&[(0, 2)], &[-1, -2]);
    let limits = projection_limits(&r).unwrap();
    let backward = span
        .bindings
        .iter()
        .position(|b| b.offset.values() == [-1])
        .unwrap();
    let constant = span
        .bindings
        .iter()
        .position(|b| b.offset.values() == [-2])
        .unwrap();
    let target = span.originals[backward]
        .keys()
        .find(|k| k.values() == [0])
        .unwrap()
        .clone();
    let zero_integral = span.originals[constant]
        .keys()
        .find(|k| k.values() == [-2])
        .unwrap()
        .clone();
    let d = c.lift(&c.base().parameter("d").unwrap()).unwrap();
    assert_eq!(span.originals[backward][&target], c.integer(-2));
    assert_eq!(span.originals[constant].len(), 1);
    assert_eq!(span.originals[constant][&zero_integral], d);
    let weights = project::Weights::from([
        (
            backward,
            c.div(&c.one(), &span.originals[backward][&target]).unwrap(),
        ),
        (constant, c.div(&c.one(), &d).unwrap()),
    ]);
    let mut guards = span.guards.clone();
    let image = project::replay(&c, &span.images, &weights, &mut guards, limits).unwrap();
    assert_eq!(image.len(), 3);
    assert_eq!(image[&target], c.one());
    assert_eq!(image[&zero_integral], c.one());
    let proposal = project::Proposal {
        weights,
        image,
        guards,
        prefix_rows: 2,
    };
    let (contributions, guards) = span.compose(&c, &proposal, limits).unwrap();
    assert_eq!(contributions.len(), 2);
    let rhs: Vec<_> = proposal
        .image
        .iter()
        .filter(|(k, _)| *k != &target)
        .map(|(k, v)| (k.clone(), c.neg_with_limits(v, limits.arithmetic).unwrap()))
        .collect();
    assert_eq!(rhs.len(), 2);
    assert!(rhs.iter().all(|(_, v)| c
        .denominator_condition_with_limits(v, limits.arithmetic)
        .unwrap()
        .is_nonzero_constant()));
    let conservative = OriginalSourceCombinationRequest {
        root_sector: Mask::try_new([true]).unwrap(),
        sector: Mask::try_new([true]).unwrap(),
        ordering: rustred::sector::OrderingPolicy::SpiredUncutV1,
        lower: vec![1],
        upper: vec![Some(1)],
        fixed: vec![FixedIndexRestriction::new(0, 2)],
        contributions,
        rhs,
        retained_conditions: guards.iter().map(|g| g.polynomial.clone()).collect(),
    };
    let request = fresh(&c, &span, conservative, limits).unwrap();
    let proof =
        check_original_source_combination(&family, request.clone(), policy(&r).unwrap()).unwrap();
    assert!(proof
        .cells()
        .any(|cell| cell
            .rule()
            .nonzero_guards()
            .iter()
            .any(|g| g.origins().iter().any(|o| matches!(
                o,
                ParametricGuardOrigin::SourceCombinationDenominator { .. }
            )))));
    assert!(proof
        .cells()
        .all(|cell| cell.rule().nonzero_guards().iter().all(|g| g
            .origins()
            .iter()
            .all(|o| !matches!(o, ParametricGuardOrigin::OriginalDomainCondition { .. })))));
    // The actual weight-origin d pole is a generic-field condition; the same
    // polynomial as a genuine original assumption below must not be waived.
    encode_checked_priority_owner_with_policy::<1>(
        &bytes,
        request.clone(),
        policy(&r).unwrap(),
        owner_limits(&r).unwrap().bundle,
        RuleDispatchPolicy::AfterBaselinePartitionWholePiece,
    )
    .unwrap();
    span.guards.push(Guard {
        polynomial: c
            .numerator_condition_with_limits(&d, Default::default())
            .unwrap(),
        origin: "genuine original base-field assumption".into(),
    });
    let mixed = fresh(&c, &span, request, projection_limits(&r).unwrap()).unwrap();
    let proof =
        check_original_source_combination(&family, mixed.clone(), policy(&r).unwrap()).unwrap();
    assert!(proof
        .cells()
        .any(|cell| cell.rule().nonzero_guards().iter().any(|g| g
            .origins()
            .iter()
            .any(|o| matches!(o, ParametricGuardOrigin::OriginalDomainCondition { .. })))));
    assert!(encode_checked_priority_owner_with_policy::<1>(
        &bytes,
        mixed,
        policy(&r).unwrap(),
        owner_limits(&r).unwrap().bundle,
        RuleDispatchPolicy::AfterBaselinePartitionWholePiece
    )
    .is_err());
}
