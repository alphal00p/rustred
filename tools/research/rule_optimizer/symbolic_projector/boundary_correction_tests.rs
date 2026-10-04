use super::*;
use rustred::{
    algebra::CoefficientContext, foundry::artifact::OriginalSourceContribution, identity::RowId,
    sector::OrderingPolicy,
};

pub(super) fn context(tag: &str) -> IndexedCoefficientContext {
    IndexedCoefficientContext::try_new(&CoefficientContext::try_new(["d"]).unwrap(), tag, 1)
        .unwrap()
}

// Genuine native handles from an ordinary source, not forged physical keys.
pub(super) fn shift(n: i64) -> IndexShift {
    static SHIFTS: std::sync::OnceLock<BTreeMap<i64, IndexShift>> = std::sync::OnceLock::new();
    SHIFTS.get_or_init(|| {
        let base = CoefficientContext::try_new(["d"]).unwrap();
        let family = rustred::family::IntegralFamily::new(
            "boundary-correction-shifts",
            vec!["q".into()],
            vec![],
            base.clone(),
            base.parameter("d").unwrap(),
            vec![rustred::family::AffineDenominator::new(
                base.integer(-1),
                vec![base.one()],
            )],
            vec![],
            vec![base.zero()],
        )
        .unwrap();
        let generator = ParametricIbpGenerator::try_new(&family).unwrap();
        let ordinary = generator.prepare_ordinary_ibp().unwrap();
        let rows = (0..ordinary.len()).map(|i| ordinary.generate(i)).collect();
        let completed = ordinary.complete(rows).unwrap();
        let batch = generator
            .translate_selected_completed_source_rows(
                &completed,
                (-4..=3)
                    .map(|n| TranslatedSourceRequest::new(0, IntegralShift::try_new([n]).unwrap())),
                Default::default(),
            )
            .unwrap();
        batch
            .sources()
            .iter()
            .flat_map(|r| r.terms().keys().map(|s| (s.values()[0], s.clone())))
            .collect()
    })[&n]
        .clone()
}

pub(super) fn limits() -> project::Limits {
    project::Limits {
        arithmetic: Default::default(),
        rows: 64,
        columns: 4096,
        nonzeros: 100000,
        coefficient_terms: 1000000,
        operations: 1000000,
        guards: 100000,
    }
}

fn config() -> Value {
    json!({"owner_mask":"1","chart":{"lower":[0],"upper":[0],"fixed":[[0,1]]},
        "boundary_correction":{"schema":SCHEMA,"fixed_pinch_axis":0,
        "correction_sources":[{"source_row":"ordinary-ibp:0:0","offset":[-1]}],
        "cancel_rank_positive_shifts":[[-2]],"forbid_new_rank_positive":true}})
}

fn admitted_config() -> Value {
    let (_, mut r) = crate::tests::tadpole();
    r["max_refinements"] = json!(0);
    r["chart"] = config()["chart"].clone();
    r["boundary_correction"] = config()["boundary_correction"].clone();
    r
}

#[test]
fn translated_pinch_policy_requires_explicit_boolean_and_preserves_old_admission() {
    let mut r = admitted_config();
    validate(&r, 1).unwrap();
    r["boundary_correction"]["translated_pinch_sources"] = json!(false);
    validate(&r, 1).unwrap();
    r["boundary_correction"]["correction_sources"][0]["offset"] = json!([-2]);
    assert!(validate(&r, 1).is_err());
    r["boundary_correction"]
        .as_object_mut()
        .unwrap()
        .remove("translated_pinch_sources");
    assert!(validate(&r, 1).is_err());
    for bad in [Value::Null, json!(1), json!("true"), json!([])] {
        r["boundary_correction"]["translated_pinch_sources"] = bad;
        assert!(validate(&r, 1).is_err());
    }
}

#[test]
fn translated_pinch_bindings_deduplicate_full_pairs_not_rowids() {
    let mut r = admitted_config();
    r["boundary_correction"]["translated_pinch_sources"] = json!(true);
    r["boundary_correction"]["correction_sources"] = json!([
        {"source_row":"ordinary-ibp:0:0","offset":[-1]},
        {"source_row":"ordinary-ibp:0:0","offset":[-2]}]);
    validate(&r, 1).unwrap();
    r["boundary_correction"]["correction_sources"][1]["offset"] = json!([-1]);
    assert!(
        validate(&r, 1)
            .unwrap_err()
            .contains("duplicate correction RowId/offset")
    );
    r["boundary_correction"]["correction_sources"][1]["offset"] = json!([0]);
    assert!(validate(&r, 1).unwrap_err().contains("nonpositive"));
}

#[test]
fn translated_source_rank_and_other_axis_activation_do_not_bypass_final_checks() {
    let mut r = admitted_config();
    r["owner_mask"] = json!("10");
    r["chart"] = json!({"lower":[0,0],"upper":[0,0],"fixed":[[0,1],[1,0]]});
    r["boundary_correction"]["translated_pinch_sources"] = json!(true);
    r["boundary_correction"]["cancel_rank_positive_shifts"] = json!([[-1, -1]]);
    for offset in [json!([-2, -3]), json!([-1, 1])] {
        r["boundary_correction"]["correction_sources"][0]["offset"] = offset;
        validate(&r, 2).unwrap();
    }
    let c = context("translated-native-gates");
    let mut one = config();
    one["boundary_correction"]["translated_pinch_sources"] = json!(true);
    assert!(
        verify_corrections(
            &one,
            &[project::Row::from([(shift(0), c.one())])],
            &shift(0)
        )
        .is_err()
    );
    assert!(
        verify_corrections(
            &one,
            &[project::Row::from([(shift(1), c.one())])],
            &shift(0)
        )
        .is_err()
    );
    let baseline = project::Row::from([(shift(0), c.one()), (shift(-1), c.one())]);
    let candidate = project::Proposal {
        weights: project::Weights::from([(0, c.one())]),
        image: project::Row::from([
            (shift(0), c.one()),
            (shift(-1), c.one()),
            (shift(-2), c.one()),
        ]),
        guards: vec![],
        prefix_rows: 1,
    };
    assert!(verify_preserved(&one, &baseline, &candidate).is_err());
}

#[test]
fn explicit_false_keeps_complete_native_zero_control_report_identical() {
    let (bytes, mut r) = crate::tests::tadpole();
    r["sources"] = json!([{"source_row":"ordinary-ibp:0:0","offset":[-1]}]);
    r["max_refinements"] = json!(0);
    r["chart"] = json!({"lower":[1],"upper":[1],"fixed":[[0,2]]});
    r["boundary_correction"] = config()["boundary_correction"].clone();
    r["boundary_correction"]["correction_sources"][0]["offset"] = json!([-2]);
    r["boundary_correction"]["cancel_rank_positive_shifts"] = json!([]);
    let old = crate::run::<1>(&bytes, &r, false).unwrap().0;
    r["boundary_correction"]["translated_pinch_sources"] = json!(false);
    let off = crate::run::<1>(&bytes, &r, false).unwrap().0;
    assert_eq!(old["attempts"], off["attempts"]);
    assert_eq!(old["status"], "EXACT_BOUNDARY_ZERO_OBJECTIVE_CHART_PROVED");
}

#[test]
fn native_multi_offset_corrections_keep_zero_control_and_full_source_proof() {
    let (bytes, mut r) = crate::tests::tadpole();
    r["sources"] = json!([{"source_row":"ordinary-ibp:0:0","offset":[-1]}]);
    r["max_refinements"] = json!(0);
    r["chart"] = json!({"lower":[1],"upper":[1],"fixed":[[0,2]]});
    r["boundary_correction"] = config()["boundary_correction"].clone();
    r["boundary_correction"]["translated_pinch_sources"] = json!(true);
    r["boundary_correction"]["correction_sources"] = json!([
        {"source_row":"ordinary-ibp:0:0","offset":[-2]},
        {"source_row":"ordinary-ibp:0:0","offset":[-3]}]);
    r["boundary_correction"]["cancel_rank_positive_shifts"] = json!([]);
    crate::validate(&r).unwrap();
    let (report, export) = crate::run::<1>(&bytes, &r, false).unwrap();
    assert!(export.is_none());
    assert_eq!(
        report["status"], "EXACT_BOUNDARY_ZERO_OBJECTIVE_CHART_PROVED",
        "{report}"
    );
    let b = &report["attempts"][0]["boundary_correction"];
    assert_eq!(b["correction_rows"], 2);
    assert_eq!(b["zero_control"]["original_contribution_map_equal"], true);
    assert_eq!(b["zero_control"]["incoming_conditions_retained"], true);
    assert_eq!(
        b["correction_source_policy"]["bindings"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        b["correction_source_policy"]["complete_native_images_target_and_unpinched_zero"],
        true
    );
    assert_eq!(b["same_support_preserved"], true);
}

fn frame(c: &IndexedCoefficientContext, offset: i64, row: project::Row) -> source::Span {
    source::Span {
        provenance: source::SpanProvenance::Ordinary,
        bindings: vec![source::SourceBinding {
            row: RowId::OrdinaryIbp {
                differentiated_loop: 0,
                contraction_momentum: 0,
            },
            offset: IntegralShift::try_new([offset]).unwrap(),
        }],
        originals: vec![row.clone()],
        images: vec![row],
        weights: vec![project::Weights::from([(0, c.one())])],
        guards: vec![],
    }
}

pub(super) fn template(c: &IndexedCoefficientContext) -> OriginalSourceCombinationRequest {
    OriginalSourceCombinationRequest {
        root_sector: Mask::try_new([true]).unwrap(),
        sector: Mask::try_new([true]).unwrap(),
        ordering: OrderingPolicy::SpiredUncutV1,
        lower: vec![0],
        upper: vec![Some(0)],
        fixed: vec![FixedIndexRestriction::new(0, 1)],
        contributions: vec![OriginalSourceContribution {
            source_row: RowId::OrdinaryIbp {
                differentiated_loop: 0,
                contraction_momentum: 0,
            },
            offset: IntegralShift::try_new([0]).unwrap(),
            weight: c.one(),
        }],
        rhs: vec![],
        retained_conditions: vec![],
    }
}

#[test]
fn scalar_chart_classifier_catches_active_pin_and_free_boundary_numerators() {
    let mut r = config();
    assert!(may_have_rank(&r, &shift(-2)).unwrap());
    assert!(!may_have_rank(&r, &shift(-1)).unwrap());
    r["chart"] = json!({"lower":[1],"upper":[null],"fixed":[]});
    assert!(may_have_rank(&r, &shift(-3)).unwrap());
    assert!(!may_have_rank(&r, &shift(-2)).unwrap());
    r["chart"]["upper"] = json!([4]);
    assert!(may_have_rank(&r, &shift(-3)).unwrap());
}

#[test]
fn free_face_pinch_does_not_hide_interior_same_support() {
    let r = json!({"owner_mask":"110010101101011",
        "chart":{"lower":[1,2,0,0,1,0,0,0,1,0,0,0,0,0,0],
        "upper":[null,null,0,0,1,0,0,0,1,0,0,0,0,0,0],
        "fixed":[[2,0],[3,0],[4,2],[5,0],[6,1],[7,0],[8,2],[9,1],[10,0],[11,1],[12,0],[13,1],[14,1]]},
        "boundary_correction":{"fixed_pinch_axis":6}});
    let values = [-2, -1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 1, 0];
    assert!(must_preserve(&r, &values).unwrap());
    let mut pin_numerator = [0; 15];
    pin_numerator[6] = -2;
    assert!(may_have_rank_values(&r, &pin_numerator).unwrap());
}

#[test]
fn correction_target_or_reactivated_sector_is_refused() {
    let c = context("leaks");
    let r = config();
    assert!(
        verify_corrections(&r, &[project::Row::from([(shift(0), c.one())])], &shift(0)).is_err()
    );
    assert!(
        verify_corrections(&r, &[project::Row::from([(shift(1), c.one())])], &shift(0)).is_err()
    );
    verify_corrections(&r, &[project::Row::from([(shift(-2), c.one())])], &shift(0)).unwrap();
}

#[test]
fn registered_objective_requires_correction_only_actual_support() {
    let c = context("support");
    let r = config();
    let parent = project::Row::from([(shift(0), c.one()), (shift(-2), c.one())]);
    let correction = project::Row::from([(shift(-1), c.one())]);
    assert!(parent.contains_key(&shift(-2)));
    assert_eq!(
        missing_correction_columns(&r, &[correction]).unwrap(),
        vec![vec![-2]]
    );
    assert!(
        missing_correction_columns(&r, &[project::Row::from([(shift(-2), c.one())])])
            .unwrap()
            .is_empty()
    );
}

#[test]
fn full_union_forbids_new_rank_column_before_projection() {
    let c = context("union");
    let r = config();
    let parent = project::Row::from([(shift(0), c.one()), (shift(-2), c.one())]);
    let universe = BTreeSet::from([shift(0), shift(-1), shift(-2), shift(-3)]);
    let f = forbidden(&r, &parent, &universe, &BTreeSet::new(), &template(&c)).unwrap();
    assert!(f.contains(&shift(-2)) && f.contains(&shift(-3)));
    assert!(!f.contains(&shift(-1)));
}

#[test]
fn native_weighted_projection_preserves_leading_recurrence_and_source_guard() {
    let c = context("weighted");
    let r = config();
    let parent = project::Row::from([
        (shift(0), c.one()),
        (shift(1), c.integer(3)),
        (shift(-2), c.one()),
    ]);
    let baseline = frame(&c, 0, parent.clone());
    let d = c.lift(&c.base().parameter("d").unwrap()).unwrap();
    let pole = c
        .numerator_condition_with_limits(&d, Default::default())
        .unwrap();
    let reciprocal = c.div_with_limits(&c.one(), &d, Default::default()).unwrap();
    let mut correction = frame(
        &c,
        -1,
        project::Row::from([(shift(-2), reciprocal), (shift(-1), c.one())]),
    );
    correction.guards.push(project::Guard {
        polynomial: pole.clone(),
        origin: "original canceled-source guard".into(),
    });
    let span = weighted(&c, &baseline, &correction, &template(&c), &parent, limits()).unwrap();
    let proposal = match project::project_with_compaction(
        &c,
        &span.images,
        &shift(0),
        &BTreeSet::from([shift(-2)]),
        &span.guards,
        limits(),
        false,
    )
    .unwrap()
    {
        project::Projection::Target(p) => p,
        _ => panic!("expected target"),
    };
    verify_preserved(&r, &parent, &proposal).unwrap();
    assert!(!proposal.image.contains_key(&shift(-2)));
    assert_eq!(proposal.image[&shift(1)], c.integer(3));
    assert!(proposal.guards.iter().any(|g| g.polynomial == pole));
    assert!(span.validate_fresh_ordinary(&c, limits()).is_err());
}

#[test]
fn native_weighted_ingress_rejects_image_binding_context_and_budget_mismatches() {
    let c = context("weighted-refusals");
    let row = project::Row::from([(shift(0), c.one())]);
    let baseline = frame(&c, 0, row.clone());
    let correction = frame(&c, -1, project::Row::from([(shift(-1), c.one())]));
    assert!(
        weighted(
            &c,
            &baseline,
            &correction,
            &template(&c),
            &project::Row::new(),
            limits()
        )
        .is_err()
    );
    let other = context("foreign");
    let foreign = frame(&other, -1, project::Row::from([(shift(-1), other.one())]));
    assert!(weighted(&c, &baseline, &foreign, &template(&c), &row, limits()).is_err());
    let collision = frame(&c, 0, project::Row::from([(shift(-1), c.one())]));
    assert!(weighted(&c, &baseline, &collision, &template(&c), &row, limits()).is_err());
    let mut tiny = limits();
    tiny.rows = 1;
    assert!(weighted(&c, &baseline, &correction, &template(&c), &row, tiny).is_err());
    let mut tiny = limits();
    tiny.coefficient_terms = 1;
    assert!(weighted(&c, &baseline, &correction, &template(&c), &row, tiny).is_err());
}

#[test]
fn correction_parser_rejects_policy_leaks_duplicates_and_wrong_pinch() {
    let (_, mut r) = crate::tests::tadpole();
    r["max_refinements"] = json!(0);
    r["chart"] = json!({"lower":[0],"upper":[0],"fixed":[[0,1]]});
    r["boundary_correction"] = config()["boundary_correction"].clone();
    validate(&r, 1).unwrap();
    for key in [
        "forbid_endpoint_changes_on_axes",
        "forbid_endpoint_increases_on_axes",
    ] {
        let mut bad = r.clone();
        bad[key] = json!([0]);
        assert!(validate(&bad, 1).is_err());
    }
    let mut bad = r.clone();
    bad["boundary_correction"] = json!([]);
    assert!(validate(&bad, 1).is_err());
    let mut bad = r.clone();
    bad["boundary_correction"]["correction_sources"]
        .as_array_mut()
        .unwrap()
        .push(config()["boundary_correction"]["correction_sources"][0].clone());
    assert!(validate(&bad, 1).is_err());
    let mut bad = r.clone();
    bad["chart"]["fixed"] = json!([[0, 2]]);
    assert!(validate(&bad, 1).is_err());
    let mut bad = r;
    bad["boundary_correction"]["cancel_rank_positive_shifts"] = json!([[-2], [-2]]);
    assert!(validate(&bad, 1).is_err());
}

#[test]
fn native_zero_objective_control_reproduces_original_chart_without_export() {
    let (bytes, mut r) = crate::tests::tadpole();
    r["sources"] = json!([{"source_row":"ordinary-ibp:0:0","offset":[-1]}]);
    r["max_refinements"] = json!(0);
    r["chart"] = json!({"lower":[1],"upper":[1],"fixed":[[0,2]]});
    let (legacy, _) = crate::run::<1>(&bytes, &r, false).unwrap();
    assert_eq!(
        legacy["status"], "EXACT_ORIGINAL_SOURCE_CHART_PROVED",
        "{legacy}"
    );
    let legacy_again = crate::run::<1>(&bytes, &r, false).unwrap().0;
    assert_eq!(legacy["attempts"], legacy_again["attempts"]);
    r["boundary_correction"] = config()["boundary_correction"].clone();
    r["boundary_correction"]["correction_sources"][0]["offset"] = json!([-2]);
    r["boundary_correction"]["cancel_rank_positive_shifts"] = json!([]);
    crate::validate(&r).unwrap();
    let (report, export) = crate::run::<1>(&bytes, &r, false).unwrap();
    assert!(export.is_none());
    assert_eq!(
        report["status"], "EXACT_BOUNDARY_ZERO_OBJECTIVE_CHART_PROVED",
        "{report}"
    );
    assert_eq!(
        report["attempts"][0]["normalized_full_product"],
        legacy["attempts"][0]["normalized_full_product"]
    );
    assert_eq!(
        report["attempts"][0]["ordinary_contributions"],
        legacy["attempts"][0]["ordinary_contributions"]
    );
    let nested = &report["attempts"][0]["boundary_correction"];
    assert_eq!(nested["zero_control"]["typed_rhs_equal"], true);
    assert_eq!(nested["zero_control"]["cell_coverage_equal"], true);
    assert_eq!(nested["zero_objective_control_only"], true);
    assert_eq!(nested["exported"], false);
    assert_eq!(nested["original_source_replay_verified"], true);
    assert_eq!(nested["fresh_original_source_certificate"], false);
}

fn native_export_fixture() -> (Vec<u8>, Value) {
    let (bytes, mut r) = crate::tests::tadpole();
    r["sources"] = json!([{"source_row":"ordinary-ibp:0:0","offset":[-1]}]);
    r["max_refinements"] = json!(0);
    r["chart"] = json!({"lower":[1],"upper":[1],"fixed":[[0,2]]});
    r["boundary_correction"] = config()["boundary_correction"].clone();
    r["boundary_correction"]["correction_sources"][0]["offset"] = json!([-2]);
    r["boundary_correction"]["cancel_rank_positive_shifts"] = json!([]);
    (bytes, r)
}

#[test]
fn boundary_typed_export_rechecks_and_cold_loads_without_changing_prove_path() {
    let (bytes, r) = native_export_fixture();
    let (proved, none) = crate::run::<1>(&bytes, &r, false).unwrap();
    assert!(none.is_none());
    let (exported, payload) = crate::run::<1>(&bytes, &r, true).unwrap();
    assert_eq!(
        exported["status"], "CHECKED_PRIORITY_OWNER_EXPORTED",
        "{exported}"
    );
    let payload = payload.unwrap();
    let b = &exported["attempts"][0]["boundary_correction"];
    assert_eq!(b["fresh_original_source_certificate"], false);
    assert_eq!(b["original_source_replay_verified"], true);
    assert_eq!(b["exported"], true);
    assert_eq!(
        b["proof_cells"],
        proved["attempts"][0]["boundary_correction"]["proof_cells"]
    );
    assert_eq!(
        b["normalized_full_product"],
        proved["attempts"][0]["boundary_correction"]["normalized_full_product"]
    );
    assert_eq!(
        exported["attempts"][0]["export_source"],
        "checked stage-two boundary correction"
    );
    let before = inspect_generated_candidate_bundle(&bytes, Default::default()).unwrap();
    let after = inspect_generated_candidate_bundle(&payload, Default::default()).unwrap();
    assert_eq!(after.generated_rules, before.generated_rules + 1);
    assert_eq!(after.finite_residuals, before.finite_residuals);
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
    let mut dispositions = Vec::new();
    programs
        .visit_owner_domain_matches(
            [true],
            &[1],
            &[Some(1)],
            Some(0),
            Default::default(),
            &std::sync::atomic::AtomicBool::new(false),
            |piece| {
                dispositions.push(piece.disposition());
                std::ops::ControlFlow::Continue(())
            },
        )
        .unwrap();
    assert_eq!(
        dispositions,
        vec![rustred::solver::OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 0 }]
    );
    let again = crate::run::<1>(&bytes, &r, false).unwrap().0;
    assert_eq!(proved["attempts"], again["attempts"]);
}

#[test]
fn boundary_missing_objective_never_exports_the_proved_baseline() {
    let (bytes, mut r) = native_export_fixture();
    r["boundary_correction"]["cancel_rank_positive_shifts"] = json!([[-3]]);
    let (report, payload) = crate::run::<1>(&bytes, &r, true).unwrap();
    assert_eq!(
        report["status"], "BOUNDARY_CORRECTION_REFUSED_OR_INCOMPLETE",
        "{report}"
    );
    assert!(payload.is_none());
    assert_eq!(
        report["attempts"][0]["boundary_correction"]["detail"],
        "selected pinch absent from actual stage-one product"
    );
    assert_eq!(
        report["attempts"][0]["boundary_correction"]["original_source_replay_verified"],
        false
    );
    assert!(report["attempts"][0].get("export").is_none());
}

#[test]
fn boundary_export_copy_admission_counts_weights_rhs_guards_and_coordinates() {
    let c = context("export-copy");
    let mut request = template(&c);
    request.rhs.push((shift(-1), c.one()));
    request.retained_conditions.push(
        c.numerator_condition_with_limits(
            &c.lift(&c.base().parameter("d").unwrap()).unwrap(),
            Default::default(),
        )
        .unwrap(),
    );
    let copied = clone_for_export(&c, &request, 100, limits()).unwrap();
    assert_eq!(
        copied.contributions[0].weight,
        request.contributions[0].weight
    );
    assert_eq!(copied.rhs, request.rhs);
    assert_eq!(copied.retained_conditions, request.retained_conditions);
    for reduced in [
        project::Limits {
            rows: 0,
            ..limits()
        },
        project::Limits {
            columns: 0,
            ..limits()
        },
        project::Limits {
            guards: 0,
            ..limits()
        },
        project::Limits {
            coefficient_terms: 4,
            ..limits()
        },
    ] {
        assert!(clone_for_export(&c, &request, 100, reduced).is_err());
    }
    assert!(clone_for_export(&c, &request, 0, limits()).is_err());
}

#[test]
fn boundary_native_proof_refusal_cannot_release_retained_export_request() {
    let (bytes, _) = native_export_fixture();
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
    let invalid = template(generator.context()); // Empty RHS is not this ordinary identity.
    let retained = clone_for_export(generator.context(), &invalid, 100, limits()).unwrap();
    let (outcome, _) = finish_proof(
        generator.context(),
        &family,
        invalid,
        Default::default(),
        Some(retained),
        false,
        json!({"fresh_original_source_certificate":false}),
        None,
        limits(),
    );
    assert_eq!(
        outcome.report["status"],
        "BOUNDARY_CORRECTION_PROOF_REFUSED"
    );
    assert_eq!(outcome.report["original_source_replay_verified"], false);
    assert_eq!(outcome.report["fresh_original_source_certificate"], false);
    assert!(outcome.export_request.is_none());
}

#[test]
fn relaxed_policy_requires_one_explicit_nonnegative_integer_cap() {
    let mut r = config();
    assert_eq!(rank_cap(&r).unwrap(), None);
    r["boundary_correction"]["max_numerator_rank"] = json!(1);
    assert!(rank_cap(&r).is_err());
    r["boundary_correction"]["forbid_new_rank_positive"] = json!(false);
    assert_eq!(rank_cap(&r).unwrap(), Some(1));
    for invalid in [json!(null), json!(-1), json!(1.5), json!("1"), json!(true)] {
        r["boundary_correction"]["max_numerator_rank"] = invalid;
        assert!(rank_cap(&r).is_err());
    }
    r["boundary_correction"]
        .as_object_mut()
        .unwrap()
        .remove("max_numerator_rank");
    assert!(rank_cap(&r).is_err());
    r["boundary_correction"]["max_numerator_rank"] = json!(0);
    assert_eq!(rank_cap(&r).unwrap(), Some(0));
}

#[test]
fn total_rank_sums_simultaneous_negatives_and_free_boundary() {
    let r = json!({"owner_mask":"110","chart":{"lower":[1,0,0],
        "upper":[null,0,0],"fixed":[[1,1],[2,0]]}});
    assert_eq!(maximum_rank(&r, &[-3, -2, -1]).unwrap(), 3);
    assert_eq!(maximum_rank(&r, &[-2, -2, 0]).unwrap(), 1);
    assert_eq!(maximum_rank(&r, &[-1, -1, 0]).unwrap(), 0);
}

#[test]
fn cap_applies_to_old_and_new_columns_without_changing_strict_f() {
    let c = context("rank-cap");
    let mut r = config();
    r["boundary_correction"]["cancel_rank_positive_shifts"] = json!([]);
    let baseline = project::Row::from([(shift(0), c.one()), (shift(-3), c.one())]);
    let universe = BTreeSet::from([shift(0), shift(-1), shift(-2), shift(-3)]);
    let strict = forbidden(&r, &baseline, &universe, &BTreeSet::new(), &template(&c)).unwrap();
    assert_eq!(strict, BTreeSet::from([shift(-2)]));
    r["boundary_correction"]["forbid_new_rank_positive"] = json!(false);
    r["boundary_correction"]["max_numerator_rank"] = json!(1);
    let capped = forbidden(&r, &baseline, &universe, &BTreeSet::new(), &template(&c)).unwrap();
    assert_eq!(capped, BTreeSet::from([shift(-3)]));
    r["boundary_correction"]["max_numerator_rank"] = json!(2);
    assert!(
        forbidden(&r, &baseline, &universe, &BTreeSet::new(), &template(&c))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn capped_candidate_allows_rank_one_trade_but_checks_final_full_image() {
    let c = context("rank-final");
    let mut r = config();
    r["boundary_correction"]["cancel_rank_positive_shifts"] = json!([]);
    let baseline = project::Row::from([(shift(0), c.one())]);
    let mut proposal = project::Proposal {
        weights: project::Weights::from([(0, c.one())]),
        image: project::Row::from([(shift(0), c.one()), (shift(-2), c.one())]),
        guards: vec![],
        prefix_rows: 1,
    };
    assert!(verify_preserved(&r, &baseline, &proposal).is_err());
    r["boundary_correction"]["forbid_new_rank_positive"] = json!(false);
    r["boundary_correction"]["max_numerator_rank"] = json!(1);
    verify_preserved(&r, &baseline, &proposal).unwrap();
    proposal.image.insert(shift(-3), c.one());
    assert!(verify_preserved(&r, &baseline, &proposal).is_err());
}

#[test]
fn zero_control_is_not_subject_to_candidate_cap() {
    let c = context("rank-zero-control");
    let mut r = config();
    r["boundary_correction"]["forbid_new_rank_positive"] = json!(false);
    r["boundary_correction"]["max_numerator_rank"] = json!(0);
    let baseline = project::Row::from([(shift(0), c.one()), (shift(-2), c.one())]);
    let zero = project::Proposal {
        weights: project::Weights::from([(0, c.one())]),
        image: baseline.clone(),
        guards: vec![],
        prefix_rows: 1,
    };
    verify_leading(&r, &baseline, &zero).unwrap();
    assert!(verify_preserved(&r, &baseline, &zero).is_err());
    let correction = project::Row::from([(shift(-2), c.one()), (shift(-1), c.one())]);
    let rows = vec![baseline.clone(), correction];
    let universe = rows.iter().flat_map(|r| r.keys().cloned()).collect();
    let f = forbidden(&r, &baseline, &universe, &BTreeSet::new(), &template(&c)).unwrap();
    let proposal =
        match project::project_with_compaction(&c, &rows, &shift(0), &f, &[], limits(), false)
            .unwrap()
        {
            project::Projection::Target(p) => p,
            _ => panic!("expected rank-zero correction"),
        };
    verify_preserved(&r, &baseline, &proposal).unwrap();
}
