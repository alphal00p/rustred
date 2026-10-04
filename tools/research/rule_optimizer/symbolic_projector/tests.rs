use super::*;
use rustred::{
    algebra::{CoefficientContext, IndexedCoefficientContext},
    identity::RowId,
};
use std::{ops::ControlFlow, sync::atomic::AtomicBool};

#[path = "endpoint_locality_tests.rs"]
mod endpoint_locality_tests;
#[path = "endpoint_no_raising_tests.rs"]
mod endpoint_no_raising_tests;

fn context() -> IndexedCoefficientContext {
    IndexedCoefficientContext::try_new(
        &CoefficientContext::try_new(["d"]).unwrap(),
        "symbolic-projector-test",
        1,
    )
    .unwrap()
}

#[test]
fn cofinal_preseed_replaces_tadpole_refinement_with_same_exact_full_product() {
    let (bytes, r) = tadpole();
    let (old, _) = run::<1>(&bytes, &r, false).unwrap();
    let mut opted = r.clone();
    opted["forbid_cofinally_higher_columns"] = json!(true);
    let (new, _) = run::<1>(&bytes, &opted, false).unwrap();
    assert_eq!(old["status"], "EXACT_ORIGINAL_SOURCE_CHART_PROVED");
    assert_eq!(new["status"], old["status"]);
    assert_eq!(old["refinements"], 1);
    assert_eq!(new["refinements"], 0);
    assert_eq!(new["cofinal_higher_new_count"], 1);
    assert_eq!(
        new["attempts"].as_array().unwrap().last().unwrap()["normalized_full_product"],
        old["attempts"].as_array().unwrap().last().unwrap()["normalized_full_product"]
    );
    assert_eq!(
        new["attempts"].as_array().unwrap().last().unwrap()["ordinary_contributions"],
        old["attempts"].as_array().unwrap().last().unwrap()["ordinary_contributions"]
    );
    let mut explicit = r.clone();
    explicit["forbid_cofinally_higher_columns"] = json!(false);
    let (mut off, _) = run::<1>(&bytes, &explicit, false).unwrap();
    let mut old = old;
    off.as_object_mut().unwrap().remove("seconds");
    old.as_object_mut().unwrap().remove("seconds");
    off["request"] = old["request"].clone();
    assert_eq!(off, old);
}

#[test]
fn cofinal_witness_storage_is_admitted_before_per_column_json() {
    let (_, mut r) = tadpole();
    let universe = BTreeSet::from([shift(0), shift(1)]);
    r["limits"]["max_augmented_columns"] = json!(1);
    assert!(
        cofinal::columns(
            &r,
            &rustred::sector::OrderingPolicy::SpiredUncutV1,
            &universe
        )
        .unwrap_err()
        .contains("column allowance")
    );
    r["limits"]["max_augmented_columns"] = json!(2);
    r["limits"]["max_coordinate_cells"] = json!(7);
    assert!(
        cofinal::columns(
            &r,
            &rustred::sector::OrderingPolicy::SpiredUncutV1,
            &universe
        )
        .unwrap_err()
        .contains("witness coordinates")
    );
    r["limits"]["max_coordinate_cells"] = json!(8);
    let (columns, witnesses) = cofinal::columns(
        &r,
        &rustred::sector::OrderingPolicy::SpiredUncutV1,
        &universe,
    )
    .unwrap();
    assert_eq!(columns, BTreeSet::from([shift(1)]));
    assert_eq!(witnesses.len(), 1);
}
fn shift(n: i64) -> IndexShift {
    static SHIFTS: std::sync::OnceLock<BTreeMap<i64, IndexShift>> = std::sync::OnceLock::new();
    SHIFTS.get_or_init(|| {
        let base = CoefficientContext::try_new(["d"]).unwrap();
        let f = rustred::family::IntegralFamily::new(
            "symbolic-shift-fixture",
            vec!["q".into()],
            Vec::new(),
            base.clone(),
            base.parameter("d").unwrap(),
            vec![rustred::family::AffineDenominator::new(
                base.integer(-1),
                vec![base.one()],
            )],
            Vec::new(),
            vec![base.zero()],
        )
        .unwrap();
        let generator = ParametricIbpGenerator::try_new(&f).unwrap();
        let prepared = generator.prepare_ordinary_ibp().unwrap();
        let rows = (0..prepared.len()).map(|i| prepared.generate(i)).collect();
        let completed = prepared.complete(rows).unwrap();
        let batch = generator
            .translate_selected_completed_source_rows(
                &completed,
                (-3..=3)
                    .map(|a| TranslatedSourceRequest::new(0, IntegralShift::try_new([a]).unwrap())),
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
fn limits() -> project::Limits {
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
fn target(p: project::Projection) -> project::Proposal {
    match p {
        project::Projection::Target(p) => p,
        _ => panic!("expected symbolic target"),
    }
}

#[test]
fn symbolic_forbidden_cancellation_uses_genuine_index_weights() {
    let c = context();
    let n = c.index(0).unwrap();
    let rows = vec![
        project::Row::from([
            (shift(1), n.clone()),
            (shift(0), c.one()),
            (shift(-1), c.one()),
        ]),
        project::Row::from([
            (shift(1), c.one()),
            (shift(0), c.one()),
            (shift(-1), c.integer(2)),
        ]),
    ];
    let p = target(
        project::project(
            &c,
            &rows,
            &shift(0),
            &BTreeSet::from([shift(1)]),
            &[],
            limits(),
        )
        .unwrap(),
    );
    assert!(!p.image.contains_key(&shift(1)));
    let expected = c
        .div(
            &c.sub(&c.mul(&c.integer(2), &n).unwrap(), &c.one()).unwrap(),
            &c.sub(&n, &c.one()).unwrap(),
        )
        .unwrap();
    assert_eq!(p.image[&shift(-1)], expected);
    assert!(p.guards.iter().any(|g| !g.polynomial.is_nonzero_constant()));
    // Constants nominated at n=2 are legal source weights, but their claimed
    // point-only F cancellation is NOT a free-index identity.
    let point = project::Weights::from([(0, c.integer(-1)), (1, c.integer(2))]);
    assert!(
        project::replay(&c, &rows, &point, &mut Vec::new(), limits())
            .unwrap()
            .contains_key(&shift(1))
    );
}

#[test]
fn zero_images_and_dependent_span_rows_do_not_invent_target() {
    let c = context();
    let rows = vec![
        project::Row::new(),
        project::Row::from([(shift(1), c.one())]),
    ];
    assert!(matches!(
        project::project(&c, &rows, &shift(0), &BTreeSet::new(), &[], limits()).unwrap(),
        project::Projection::NoTarget { rows: 2, .. }
    ));
}

#[test]
fn weighted_span_bulk_product_and_original_composition_keep_cancelled_poles() {
    let c = context();
    let n = c.index(0).unwrap();
    let inv = c.div(&c.one(), &n).unwrap();
    let bindings = vec![source::SourceBinding {
        row: RowId::OrdinaryIbp {
            differentiated_loop: 0,
            contraction_momentum: 0,
        },
        offset: IntegralShift::try_new([0]).unwrap(),
    }];
    let originals = vec![project::Row::from([
        (shift(0), c.one()),
        (shift(-1), c.integer(2)),
    ])];
    let weights = vec![
        project::Weights::from([(0, inv.clone())]),
        project::Weights::from([(0, c.neg_with_limits(&inv, Default::default()).unwrap())]),
    ];
    let span =
        source::Span::weighted(&c, bindings, originals, weights, Vec::new(), limits()).unwrap();
    let p = target(
        project::project(
            &c,
            &span.images,
            &shift(0),
            &BTreeSet::new(),
            &span.guards,
            limits(),
        )
        .unwrap(),
    );
    let (contributions, guards) = span.compose(&c, &p, limits()).unwrap();
    assert_eq!(contributions.len(), 1);
    assert_eq!(contributions[0].weight, c.one());
    assert!(
        guards
            .iter()
            .any(|g| g.polynomial.raw() == &n.raw().numerator)
    );
    let canceled = project::replay_many(
        &c,
        &span.images,
        &[project::Weights::from([(0, c.one()), (1, c.one())])],
        &mut Vec::new(),
        limits(),
    )
    .unwrap();
    assert!(canceled[0].is_empty());
}

#[test]
fn native_ingress_membership_does_not_remove_index_poles() {
    let c = context();
    // Raw native results have no semantic scope seal. A genuinely different
    // variable map (here a second index) must fail membership admission.
    let foreign = IndexedCoefficientContext::try_new(c.base(), "foreign-projector", 2).unwrap();
    assert!(
        c.admit_native_result_with_limits(
            foreign.index(0).unwrap().raw().clone(),
            Default::default()
        )
        .is_err()
    );
    let q = c.div(&c.one(), &c.index(0).unwrap()).unwrap();
    let sealed = c
        .admit_native_result_with_limits(q.raw().clone(), Default::default())
        .unwrap();
    assert_eq!(q, sealed);
    assert!(
        !c.denominator_condition_with_limits(&sealed, Default::default())
            .unwrap()
            .is_nonzero_constant()
    );
    assert!(
        c.admit_native_result_with_limits(
            q.raw().clone(),
            ExactAlgebraLimits {
                max_polynomial_terms: 0,
                ..Default::default()
            }
        )
        .is_err()
    );
}

#[test]
fn refinement_is_typed_monotone_and_non_descent_stops_do_not_mutate_f() {
    let error = SourcePortAuditError::UnprovedDescentObligation {
        term_ordinal: 3,
        shift: vec![1],
        local_lower: vec![1],
        local_upper: vec![None],
        child_sector: vec![true],
    };
    let universe = BTreeSet::from([shift(-1), shift(0), shift(1)]);
    let mut f = BTreeSet::new();
    assert!(matches!(
        refine::refine(&error, &shift(0), &universe, &mut f, 0, 1).unwrap(),
        refine::Action::Added(_)
    ));
    assert!(refine::refine(&error, &shift(0), &universe, &mut f, 0, 1).is_err());
    for e in [
        SourcePortAuditError::Message("normalization guard zero".into()),
        SourcePortAuditError::Message("unsupported nonlinear guard".into()),
        SourcePortAuditError::ResourceBudgetExhausted { resource: "test" },
    ] {
        let old = f.clone();
        assert!(matches!(
            refine::refine(&e, &shift(0), &universe, &mut f, 0, 1).unwrap(),
            refine::Action::Stop
        ));
        assert_eq!(f, old);
    }
}

#[test]
fn wrong_forbidden_shift_and_resource_caps_refuse_before_authority() {
    let c = context();
    let rows = vec![project::Row::from([(shift(0), c.one())])];
    // Absent columns are now accepted as exact structural zeros; the separate
    // selected-subbank fixture covers that. The target itself remains forbidden.
    let f = BTreeSet::from([shift(0)]);
    assert!(project::project(&c, &rows, &shift(0), &f, &[], limits()).is_err());
    let mut cap = limits();
    cap.nonzeros = 1;
    assert!(matches!(
        project::project(&c, &rows, &shift(0), &BTreeSet::new(), &[], cap),
        Err(project::Error::Budget(_))
    ));
    let zero = c
        .numerator_condition_with_limits(&c.zero(), Default::default())
        .unwrap();
    assert!(
        project::project(
            &c,
            &rows,
            &shift(0),
            &BTreeSet::new(),
            &[project::Guard {
                polynomial: zero,
                origin: "retained zero".into()
            }],
            limits()
        )
        .is_err()
    );
}

const TADPOLE: &str = r#"
schema="rustred.project.toml.v1"
[family]
name="symbolic_projector_tadpole"
loop_momenta=["q"]
external_momenta=[]
dimension="d"
[[family.denominators]]
id="P"
expression="q^2-1"
[target]
powers=[1]
"#;
pub(crate) fn tadpole() -> (Vec<u8>, Value) {
    let bytes = rustred_app::family_candidates(rustred_app::FamilyCandidatesRequest::new(TADPOLE))
        .unwrap()
        .bundle()
        .to_vec();
    let mask = Mask::try_new([true]).unwrap();
    let (f, p) = load_generated_candidate_owners::<1>(
        &[CandidateOwnerBundle {
            bytes: &bytes,
            owner_sector: &mask,
        }],
        Default::default(),
        Default::default(),
    )
    .unwrap();
    let p = Arc::new(p);
    let owner = p.bind_owner_search([true], Default::default()).unwrap();
    let r = json!({"schema":SCHEMA,"owner_mask":"1","family_fingerprint":f.fingerprint(),"expected_order":owner.owner_ordering().stable_id().to_string(),
        "sources":[{"source_row":"ordinary-ibp:0:0","offset":[0]},{"source_row":"ordinary-ibp:0:0","offset":[-1]}],
        "chart":{"lower":[1],"upper":[null],"fixed":[]},"forbidden_shifts":[],"max_refinements":1,
        "limits":{"max_owner_bytes":16777216,"max_output_bytes":16777216,"max_source_rows":64,"max_complete_source_rows":64,
            "max_terms":10000,"max_conditions":100000,"max_coordinate_cells":100000,"max_cells":10000,
            "max_polynomial_terms":100000,"max_term_operations":1000000,"max_exponent":100,"max_report_bytes":16777216,
            "max_augmented_columns":4096,"max_matrix_nonzeros":100000,"max_coefficient_terms":1000000},
        "owner_load_limits":{"max_bundle_bytes":16777216,"max_total_input_bytes":16777216,"max_total_coefficient_bytes":16777216,
            "max_collection_entries":1000000,"max_total_symbolica_state_bytes":16777216,"max_zero_sector_visits":10000,"max_coefficient_bytes":1048576}});
    (bytes, r)
}

#[test]
fn optional_domain_endpoint_limit_preserves_default_and_rejects_invalid() {
    let (_, mut r) = tadpole();
    assert_eq!(
        policy(&r).unwrap().rule.max_domain_bound_endpoint_cells,
        8192
    );
    r["limits"]["max_domain_bound_endpoint_cells"] = json!(1000000);
    validate(&r).unwrap();
    assert_eq!(
        policy(&r).unwrap().rule.max_domain_bound_endpoint_cells,
        1000000
    );
    for invalid in [
        json!(0),
        json!(-1),
        json!(null),
        json!(1.5),
        json!("1000000"),
    ] {
        r["limits"]["max_domain_bound_endpoint_cells"] = invalid;
        assert!(validate(&r).is_err());
    }
}

#[test]
fn native_tadpole_forward_rule_refines_to_backward_full_chart_proof() {
    let (bytes, r) = tadpole();
    validate(&r).unwrap();
    let (report, _) = run::<1>(&bytes, &r, false).unwrap();
    assert_eq!(
        report["status"], "EXACT_ORIGINAL_SOURCE_CHART_PROVED",
        "{report}"
    );
    assert_eq!(report["refinements"], 1);
    assert_eq!(
        report["attempts"][0]["proof_error"]["kind"],
        "UNPROVED_DESCENT_OBLIGATION"
    );
    assert_eq!(report["attempts"][0]["next_forbidden_shift"], json!([1]));
    assert_eq!(
        report["attempts"][1]["ordinary_contributions"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        report["attempts"][1]["ordinary_contributions"][0]["offset"],
        json!([-1])
    );
}

#[test]
fn trace_default_and_full_are_identical_and_reject_unknown_detail() {
    let (bytes, mut r) = tadpole();
    let (default, _) = run::<1>(&bytes, &r, false).unwrap();
    assert_eq!(trace::detail(&r).unwrap(), trace::Detail::Full);
    r["trace_detail"] = json!("full");
    let (full, _) = run::<1>(&bytes, &r, false).unwrap();
    assert_eq!(default["attempts"], full["attempts"]);
    assert_eq!(default["status"], full["status"]);
    assert_eq!(default["refinements"], full["refinements"]);
    for invalid in [json!(null), json!(true), json!(1), json!("short")] {
        r["trace_detail"] = invalid;
        assert!(validate(&r).is_err());
    }
}

#[test]
fn summary_trace_keeps_refinement_witnesses_final_proof_and_export_bytes() {
    let (bytes, mut r) = tadpole();
    let (full, _) = run::<1>(&bytes, &r, false).unwrap();
    r["trace_detail"] = json!("summary");
    let (summary, _) = run::<1>(&bytes, &r, false).unwrap();
    assert_eq!(summary["status"], full["status"]);
    assert_eq!(summary["refinements"], full["refinements"]);
    let before = full["attempts"].as_array().unwrap();
    let after = summary["attempts"].as_array().unwrap();
    assert_eq!(before.len(), after.len());
    for (old, new) in before.iter().zip(after) {
        for field in [
            "forbidden_shifts",
            "first_target_prefix",
            "proof_error",
            "next_forbidden_shift",
            "full_original_product_replayed",
            "status",
            "proof_cells",
        ] {
            assert_eq!(old[field], new[field], "changed {field}");
        }
        for (payload, count) in [
            ("normalized_full_product", "full_product_term_count"),
            ("conditions", "retained_condition_count"),
            ("ordinary_contributions", "ordinary_contribution_count"),
        ] {
            assert_eq!(new[count], json!(old[payload].as_array().unwrap().len()));
        }
    }
    assert_eq!(after[0]["trace_compacted"], true);
    for field in [
        "normalized_full_product",
        "conditions",
        "ordinary_contributions",
    ] {
        assert!(after[0].get(field).is_none());
        assert_eq!(after.last().unwrap()[field], before.last().unwrap()[field]);
    }
    let failed_shift = &before[0]["proof_error"]["shift"];
    let term = before[0]["normalized_full_product"]
        .as_array()
        .unwrap()
        .iter()
        .find(|term| &term["shift"] == failed_shift)
        .unwrap();
    assert_eq!(&after[0]["failed_shift_full_product_term"], term);

    // Display compaction cannot alter checked-owner authority or bytes.
    r["sources"] = json!([{"source_row":"ordinary-ibp:0:0","offset":[-1]}]);
    r["max_refinements"] = json!(0);
    let (summary_export, summary_bytes) = run::<1>(&bytes, &r, true).unwrap();
    r["trace_detail"] = json!("full");
    let (full_export, full_bytes) = run::<1>(&bytes, &r, true).unwrap();
    assert_eq!(summary_export["status"], "CHECKED_PRIORITY_OWNER_EXPORTED");
    assert_eq!(summary_export["status"], full_export["status"]);
    assert_eq!(summary_bytes, full_bytes);
}

#[test]
fn summary_terminal_miss_keeps_last_actual_candidate_and_every_record() {
    let candidate = |prefix| {
        json!({"first_target_prefix":prefix,"forbidden_shifts":[[1]],
        "normalized_full_product":[{"shift":[0],"coefficient":"1","display_only":true},
            {"shift":[2],"coefficient":"n0/(n0-1)","display_only":true}],
        "conditions":[{"polynomial":"n0-1"}],"ordinary_contributions":[{"source_row":"test"}],
        "proof_error":{"kind":"UNPROVED_DESCENT_OBLIGATION","shift":[2],"local_lower":[1],"local_upper":[null],"child_sector":[true]},
        "next_forbidden_shift":[2],"full_original_product_replayed":true})
    };
    for terminal in [
        json!({"status":"NO_TARGET_IN_FROZEN_SPAN_WITH_CURRENT_F","visited_rows":75,"conditions":[{"polynomial":"n0"}],"nonexistence_claim":false}),
        json!({"status":"SYMBOLIC_PROJECTION_REFUSED","detail":"budget","refinement_permitted":false}),
    ] {
        let mut attempts = Vec::new();
        trace::append(&mut attempts, candidate(3), trace::Detail::Summary);
        let last = candidate(5);
        trace::append(&mut attempts, last.clone(), trace::Detail::Summary);
        trace::append(&mut attempts, terminal.clone(), trace::Detail::Summary);
        assert_eq!(attempts.len(), 3);
        assert_eq!(attempts[0]["trace_compacted"], true);
        assert_eq!(attempts[0]["full_product_term_count"], 2);
        assert_eq!(
            attempts[0]["failed_shift_full_product_term"],
            last["normalized_full_product"][1]
        );
        for (field, value) in last.as_object().unwrap() {
            assert_eq!(&attempts[1][field], value);
        }
        assert!(attempts[1].get("trace_compacted").is_none());
        for (field, value) in terminal.as_object().unwrap() {
            assert_eq!(&attempts[2][field], value);
        }
    }
    let mut missing = candidate(3);
    missing["proof_error"]["shift"] = json!([9]);
    let mut attempts = Vec::new();
    trace::append(&mut attempts, missing.clone(), trace::Detail::Summary);
    trace::append(&mut attempts, candidate(5), trace::Detail::Summary);
    assert_eq!(
        attempts[0]["normalized_full_product"],
        missing["normalized_full_product"]
    );
    assert!(attempts[0]["trace_compaction_refused"].is_string());
    assert!(attempts[0].get("trace_compacted").is_none());
}

#[test]
fn native_backward_export_keeps_baseline_and_outside_chart_fallback() {
    let (bytes, mut r) = tadpole();
    r["sources"] = json!([{"source_row":"ordinary-ibp:0:0","offset":[-1]}]);
    r["max_refinements"] = json!(0);
    let (report, candidate) = run::<1>(&bytes, &r, true).unwrap();
    assert_eq!(
        report["status"], "CHECKED_PRIORITY_OWNER_EXPORTED",
        "{report}"
    );
    let candidate = candidate.unwrap();
    let before = inspect_generated_candidate_bundle(&bytes, Default::default()).unwrap();
    let after = inspect_generated_candidate_bundle(&candidate, Default::default()).unwrap();
    assert_eq!(after.generated_rules, before.generated_rules + 1);
    assert_eq!(after.finite_residuals, before.finite_residuals);
    let mask = Mask::try_new([true]).unwrap();
    let (_, p) = load_generated_candidate_owners::<1>(
        &[CandidateOwnerBundle {
            bytes: &candidate,
            owner_sector: &mask,
        }],
        Default::default(),
        Default::default(),
    )
    .unwrap();
    let dispositions = |power: u64| {
        let mut out = Vec::new();
        p.visit_owner_domain_matches(
            [true],
            &[power - 1],
            &[Some(power - 1)],
            Some(0),
            Default::default(),
            &AtomicBool::new(false),
            |piece| {
                out.push(piece.disposition());
                ControlFlow::Continue(())
            },
        )
        .unwrap();
        out
    };
    assert_eq!(
        dispositions(2),
        vec![rustred::solver::OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 0 }]
    );
    assert_ne!(
        dispositions(1),
        vec![rustred::solver::OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 0 }]
    );
}

#[test]
fn native_lower_child_H_is_cancelled_symbolically_without_widening_export_chart() {
    let (bytes, mut r) = tadpole();
    r["sources"] = json!([{"source_row":"ordinary-ibp:0:0","offset":[-1]},{"source_row":"ordinary-ibp:0:0","offset":[-2]}]);
    r["chart"]["lower"] = json!([2]);
    r["forbidden_shifts"] = json!([[-1]]);
    r["max_refinements"] = json!(0);
    let (report, candidate) = run::<1>(&bytes, &r, true).unwrap();
    assert_eq!(
        report["status"], "EXACT_CHART_PROVED_EXPORT_REFUSED",
        "{report}"
    );
    assert!(candidate.is_none());
    assert!(
        report["attempts"][0]["export_error"]
            .as_str()
            .unwrap()
            .contains("proof guard is not retained by a surviving runtime RHS denominator")
    );
    let product = report["attempts"][0]["normalized_full_product"]
        .as_array()
        .unwrap();
    assert_eq!(product.len(), 2);
    assert!(product.iter().any(|v| v["shift"] == json!([-2])));
    assert!(!product.iter().any(|v| v["shift"] == json!([-1])));
}

#[test]
fn input_shape_freezes_bank_chart_and_native_names() {
    let (_, r) = tadpole();
    validate(&r).unwrap();
    let mut bad = r.clone();
    bad["sources"][0]["weight"] = json!([1, 2]);
    assert!(validate(&bad).is_err());
    let mut bad = r.clone();
    bad["sources"][1] = bad["sources"][0].clone();
    assert!(validate(&bad).is_err());
    let mut bad = r.clone();
    bad["forbidden_shifts"] = json!([[0]]);
    assert!(validate(&bad).is_err());
    let mut bad = r;
    bad["chart"]["fixed"] = json!([[0, 2]]);
    assert!(validate(&bad).is_err());
}

#[test]
fn saved_rule_inspection_is_bounded_read_only_and_does_not_claim_source_authority() {
    let (bytes, mut r) = tadpole();
    r["schema"] = json!(inspect::SCHEMA);
    r["rule_ordinal"] = json!(0);
    r["max_rhs_sample_terms"] = json!(1);
    r["limits"]["max_inspection_bytes"] = json!(1 << 20);
    let view = inspect::run(&bytes, &r).unwrap();
    assert_eq!(view["status"], "SAVED_RULE_SHAPE_INSPECTED");
    assert_eq!(view["rule_ordinal"], 0);
    assert_eq!(view["source_replay_claim"], false);
    assert_eq!(view["dispatch_claim"], false);
    assert!(view["rhs_sample"].as_array().unwrap().len() <= 1);
    assert!(view["case"]["fixed"].is_array());
    r["family_fingerprint"] = json!("wrong");
    assert!(inspect::run(&bytes, &r).is_err());
}

#[test]
fn saved_seed_support_nomination_is_bound_and_has_no_authority() {
    let (bytes, mut r) = tadpole();
    r["schema"] = json!(support_inspect::SCHEMA);
    r["rule_ordinal"] = json!(0);
    for (name, value) in [
        ("max_retained_seeds", 10000),
        ("max_unique_offsets", 1000),
        ("max_original_rows", 100),
        ("max_nominated_pairs", 10000),
        ("max_generated_terms", 10000),
        ("max_generated_conditions", 10000),
        ("max_inspection_bytes", 1 << 20),
    ] {
        r["limits"][name] = json!(value);
    }
    let report = support_inspect::run(&bytes, &r).unwrap();
    assert_eq!(report["status"], "SAVED_SEED_SUPPORT_NOMINATED");
    assert_eq!(
        report["support"]["ordinary_source_rows"],
        json!(["ordinary-ibp:0:0"])
    );
    assert_eq!(report["support"]["saved_basis_ordinals_validated"], false);
    assert_eq!(report["source_replay_claim"], false);
    assert_eq!(report["dispatch_claim"], false);
    assert_eq!(report["closure_claim"], false);
    assert!(validate(&r).is_err());
    r["family_fingerprint"] = json!("wrong");
    assert!(support_inspect::run(&bytes, &r).is_err());
    r["limits"]["max_unique_offsets"] = json!(0);
    assert!(support_inspect::validate(&r).is_err());
}

#[test]
fn fixed_root_policy_does_not_classify_unfixed_or_root_allowed_axes() {
    let root = [false, false, true];
    assert!(root_policy::activates_fixed_outside_root(&root, &[(0, 0)], &[1, 0, 0]).unwrap());
    assert!(!root_policy::activates_fixed_outside_root(&root, &[(0, 0)], &[0, 1, 0]).unwrap());
    assert!(!root_policy::activates_fixed_outside_root(&root, &[(2, 0)], &[0, 0, 1]).unwrap());
    assert!(!root_policy::activates_fixed_outside_root(&root, &[(0, -2)], &[2, 0, 0]).unwrap());
    assert!(root_policy::activates_fixed_outside_root(&root, &[(0, -2)], &[3, 0, 0]).unwrap());
    assert!(
        root_policy::activates_fixed_outside_root(&[false], &[(0, i64::MAX)], &[i64::MAX]).unwrap()
    );
    assert!(
        !root_policy::activates_fixed_outside_root(&[false], &[(0, i64::MIN)], &[i64::MIN])
            .unwrap()
    );
    assert!(root_policy::activates_fixed_outside_root(&root, &[(3, 0)], &[0, 0, 0]).is_err());
    assert!(root_policy::activates_fixed_outside_root(&root, &[], &[0]).is_err());
}

#[test]
fn fixed_root_columns_use_only_actual_native_universe_and_leave_target() {
    let universe = BTreeSet::from([shift(-1), shift(0), shift(1)]);
    let selected = root_policy::columns(&[false], &[(0, 0)], &universe).unwrap();
    assert_eq!(selected, BTreeSet::from([shift(1)]));
    assert!(!selected.contains(&shift(0)));
    assert!(
        root_policy::columns(&[false], &[], &universe)
            .unwrap()
            .is_empty()
    );
    assert!(
        root_policy::columns(&[true], &[(0, 0)], &universe)
            .unwrap()
            .is_empty()
    );
    assert!(
        root_policy::columns(&[false], &[(0, -1)], &universe)
            .unwrap()
            .is_empty()
    );
    assert_eq!(universe.len(), 3); // No source/image/term was removed.
}

#[test]
fn optional_fixed_root_policy_defaults_off_and_rejects_nonboolean() {
    let (_, mut request) = tadpole();
    assert!(!root_policy::enabled(&request).unwrap());
    for value in [false, true] {
        request["forbid_fixed_outside_root_activations"] = json!(value);
        validate(&request).unwrap();
        assert_eq!(root_policy::enabled(&request).unwrap(), value);
    }
    for value in [json!(null), json!(0), json!("true"), json!([])] {
        request["forbid_fixed_outside_root_activations"] = value;
        assert!(validate(&request).is_err());
    }
}

#[test]
fn native_constant_elision_preserves_zero_context_and_parameter_pole_gates() {
    let c = context();
    let mut guards = Vec::new();
    let mut small = limits();
    small.guards = 1;
    for value in [c.one(), c.integer(-1), c.integer(7)] {
        let polynomial = c
            .numerator_condition_with_limits(&value, Default::default())
            .unwrap();
        project::retain(&c, &mut guards, polynomial, "constant", small).unwrap();
    }
    assert!(guards.is_empty());
    let zero = c
        .numerator_condition_with_limits(&c.zero(), Default::default())
        .unwrap();
    assert!(project::retain(&c, &mut guards, zero, "zero", small).is_err());
    let foreign = IndexedCoefficientContext::try_new(c.base(), "foreign", 2).unwrap();
    let one = foreign
        .numerator_condition_with_limits(&foreign.one(), Default::default())
        .unwrap();
    assert!(project::retain(&c, &mut guards, one, "foreign constant", small).is_err());
    let d = c.lift(&c.base().parameter("d").unwrap()).unwrap();
    let inv_d = c.div(&c.one(), &d).unwrap();
    project::denominator(&c, &mut guards, &inv_d, "base parameter pole", small).unwrap();
    assert_eq!(guards.len(), 1);
    assert!(!guards[0].polynomial.is_nonzero_constant());
    let inv_n = c.div(&c.one(), &c.index(0).unwrap()).unwrap();
    assert!(matches!(
        project::denominator(&c, &mut guards, &inv_n, "index pole", small),
        Err(project::Error::Budget(_))
    ));
    project::denominator(&c, &mut guards, &inv_n, "index pole", limits()).unwrap();
    assert_eq!(guards.len(), 2);
}

#[test]
fn fixed_root_policy_checked_tadpole_export_fixture_and_default_identity() {
    let (bytes, mut request) = tadpole();
    request["sources"] = json!([{"source_row":"ordinary-ibp:0:0","offset":[-1]}]);
    request["max_refinements"] = json!(0);
    let (off, off_bytes) = run::<1>(&bytes, &request, true).unwrap();
    request["forbid_fixed_outside_root_activations"] = json!(true);
    let (on, on_bytes) = run::<1>(&bytes, &request, true).unwrap();
    assert_eq!(off["status"], "CHECKED_PRIORITY_OWNER_EXPORTED");
    assert_eq!(on["status"], off["status"]);
    assert_eq!(on["derived_fixed_outside_root_forbidden_shifts"], json!([]));
    assert_eq!(on_bytes, off_bytes);
    // Optional fresh fixture for external comparison against the immutable
    // pre-elision binary; that cross-binary gate belongs to the owned runner.
    if let Some(directory) = std::env::var_os("RUSTRED_PROJECTOR_CONTROL_DIRECTORY") {
        let directory = std::path::PathBuf::from(directory);
        assert!(directory.is_absolute());
        fs::create_dir(&directory).unwrap();
        fresh(&directory.join("base.rrbin"), &bytes).unwrap();
        request
            .as_object_mut()
            .unwrap()
            .remove("forbid_fixed_outside_root_activations");
        fresh(
            &directory.join("request.json"),
            &serde_json::to_vec_pretty(&request).unwrap(),
        )
        .unwrap();
        fresh(
            &directory.join("new-test-candidate.rrbin"),
            &off_bytes.unwrap(),
        )
        .unwrap();
    }
}

fn compare_direct(
    c: &IndexedCoefficientContext,
    rows: &[project::Row],
    f: &BTreeSet<IndexShift>,
    l: project::Limits,
) -> (project::Proposal, project::Proposal) {
    let a = target(project::project(c, rows, &shift(0), f, &[], l).unwrap());
    let b = target(direct_l::project(c, rows, &shift(0), f, &[], l).unwrap());
    assert_eq!(a.prefix_rows, b.prefix_rows);
    assert_eq!(a.weights, b.weights);
    assert_eq!(a.image, b.image);
    assert_eq!(a.guards.len(), b.guards.len());
    for (g, h) in a.guards.iter().zip(&b.guards) {
        assert_eq!(g.polynomial, h.polynomial);
        assert_eq!(g.origin, h.origin);
    }
    (a, b)
}

#[test]
fn direct_l_policy_is_explicit_and_tadpole_checked_bytes_match() {
    let (bytes, mut r) = tadpole();
    assert!(!direct_l::enabled(&r).unwrap());
    let (a, a_bytes) = run::<1>(&bytes, &r, true).unwrap();
    r["projection_backend"] = json!("augmented");
    assert!(!direct_l::enabled(&r).unwrap());
    let (explicit, explicit_bytes) = run::<1>(&bytes, &r, true).unwrap();
    assert_eq!(a["attempts"], explicit["attempts"]);
    assert_eq!(a_bytes, explicit_bytes);
    r["projection_backend"] = json!("direct-l");
    let (b, b_bytes) = run::<1>(&bytes, &r, true).unwrap();
    assert_eq!(a["status"], b["status"]);
    assert_eq!(a["attempts"], b["attempts"]);
    eprintln!(
        "two-source tadpole common export result: {} / {}",
        a["status"],
        a["attempts"].as_array().unwrap().last().unwrap()["export_error"]
    );
    assert_eq!(a["refinements"], b["refinements"]);
    assert_eq!(a_bytes, b_bytes);
    // Use the already established checked-export control, without an unused
    // forward source and its conservative prefix guard.
    r["sources"] = json!([{"source_row":"ordinary-ibp:0:0","offset":[-1]}]);
    r["max_refinements"] = json!(0);
    r["projection_backend"] = json!("augmented");
    let (valid_a, bytes_a) = run::<1>(&bytes, &r, true).unwrap();
    r["projection_backend"] = json!("direct-l");
    let (valid_b, bytes_b) = run::<1>(&bytes, &r, true).unwrap();
    assert_eq!(valid_a["status"], "CHECKED_PRIORITY_OWNER_EXPORTED");
    assert_eq!(valid_a["status"], valid_b["status"]);
    assert_eq!(valid_a["attempts"], valid_b["attempts"]);
    assert_eq!(bytes_a, bytes_b);
    for invalid in [json!(null), json!(true), json!(0), json!("inverse")] {
        r["projection_backend"] = invalid;
        assert!(validate(&r).is_err());
    }
}

#[test]
fn direct_l_empty_and_dependent_rows_preserve_original_input_mapping() {
    let c = context();
    let rows = vec![
        project::Row::new(),
        project::Row::from([(shift(1), c.integer(2)), (shift(-1), c.one())]),
        project::Row::from([(shift(1), c.integer(4)), (shift(-1), c.integer(2))]),
        project::Row::new(),
        project::Row::from([
            (shift(1), c.integer(6)),
            (shift(0), c.integer(3)),
            (shift(-1), c.integer(5)),
        ]),
    ];
    let (a, _) = compare_direct(&c, &rows, &BTreeSet::from([shift(1)]), limits());
    assert_eq!(a.prefix_rows, 5);
    assert_eq!(a.weights.keys().copied().collect::<Vec<_>>(), vec![1, 4]);
}

#[test]
fn direct_l_nonlex_pivot_chronology_sorts_native_l_transpose() {
    let c = context();
    let rows = vec![
        project::Row::from([(shift(-1), c.integer(2))]),
        project::Row::from([(shift(-2), c.integer(3)), (shift(-1), c.one())]),
        project::Row::from([(shift(-2), c.integer(6)), (shift(-1), c.integer(2))]),
        project::Row::from([
            (shift(-2), c.one()),
            (shift(-1), c.one()),
            (shift(0), c.integer(5)),
            (shift(1), c.one()),
        ]),
    ];
    compare_direct(&c, &rows, &BTreeSet::from([shift(-2), shift(-1)]), limits());
}

#[test]
fn direct_l_symbolic_pivots_and_cancelled_input_poles_remain_live() {
    let c = context();
    let n = c.index(0).unwrap();
    let inv = c.div(&c.one(), &n).unwrap();
    let rows = vec![
        project::Row::from([
            (shift(1), n.clone()),
            (shift(0), c.one()),
            (shift(-1), inv.clone()),
        ]),
        project::Row::from([(shift(1), c.one()), (shift(0), c.one()), (shift(-1), inv)]),
        project::Row::from([(
            shift(0),
            c.div(&c.one(), &c.sub(&n, &c.integer(7)).unwrap()).unwrap(),
        )]),
    ];
    let (a, b) = compare_direct(&c, &rows, &BTreeSet::from([shift(1)]), limits());
    assert_eq!(a.prefix_rows, 2);
    // Even an unvisited source after target retains its original input pole.
    let after = c.sub(&n, &c.integer(7)).unwrap().raw().numerator.clone();
    assert!(b.guards.iter().any(|g| g.polynomial.raw() == &after));
}

#[test]
fn direct_l_no_target_and_foreign_context_refusals_agree() {
    let c = context();
    let rows = vec![
        project::Row::new(),
        project::Row::from([(shift(1), c.one())]),
        project::Row::from([(shift(1), c.integer(2))]),
    ];
    for result in [
        project::project(
            &c,
            &rows,
            &shift(0),
            &BTreeSet::from([shift(1)]),
            &[],
            limits(),
        ),
        direct_l::project(
            &c,
            &rows,
            &shift(0),
            &BTreeSet::from([shift(1)]),
            &[],
            limits(),
        ),
    ] {
        assert!(matches!(
            result.unwrap(),
            project::Projection::NoTarget { rows: 3, .. }
        ));
    }
    let foreign = IndexedCoefficientContext::try_new(
        &CoefficientContext::try_new(["x"]).unwrap(),
        "foreign",
        1,
    )
    .unwrap();
    let bad = vec![project::Row::from([(shift(0), foreign.index(0).unwrap())])];
    assert!(direct_l::project(&c, &bad, &shift(0), &BTreeSet::new(), &[], limits()).is_err());
    assert!(project::project(&c, &bad, &shift(0), &BTreeSet::new(), &[], limits()).is_err());
    let one = vec![project::Row::from([(shift(0), c.one())])];
    let mut storage = limits();
    storage.nonzeros = 4; // U + L + Lt + Lt.clone + unit = 5.
    assert!(matches!(
        direct_l::project(&c, &one, &shift(0), &BTreeSet::new(), &[], storage),
        Err(project::Error::Budget(
            "retained reduction plus native solve input"
        ))
    ));
    let mut terms = limits();
    terms.coefficient_terms = 9;
    assert!(matches!(
        direct_l::project(&c, &one, &shift(0), &BTreeSet::new(), &[], terms),
        Err(project::Error::Budget(
            "retained native U/L coefficient terms"
        ))
    ));
}

#[test]
fn direct_l_1062_row_fixture_matches_augmented_prefix_weights_and_full_image() {
    let c = context();
    let mut rows = vec![project::Row::new()];
    rows.extend(
        (0..1060).map(|_| project::Row::from([(shift(1), c.one()), (shift(-1), c.integer(2))])),
    );
    rows.push(project::Row::from([
        (shift(1), c.integer(3)),
        (shift(0), c.integer(2)),
        (shift(-1), c.integer(8)),
    ]));
    let mut l = limits();
    l.rows = 1062;
    // Synthetic augmented identity stress: at most2 triangular1062x1062
    // U/L envelopes (<1.13M entries), each scalar has2constant terms.
    // These are TEST-ONLY ceilings; no experiment or production limit changes.
    l.nonzeros = 2_000_000;
    l.coefficient_terms = 4_000_000;
    let (a, _) = compare_direct(&c, &rows, &BTreeSet::from([shift(1)]), l);
    assert_eq!(a.prefix_rows, 1062);
    let mut mutated = rows.clone();
    mutated[1061].insert(shift(-1), c.integer(9));
    assert_ne!(
        project::replay(&c, &mutated, &a.weights, &mut Vec::new(), l).unwrap(),
        a.image
    );
}

#[test]
fn direct_l_native_empty_dependent_and_full_rank_append_contract() {
    use symbolica::{
        domains::SelfRing,
        prelude::Z,
        tensors::sparse::{LuLMode, SparseRowReducer},
    };
    let c = context();
    let mut reducer = SparseRowReducer::new(2, project::Field::new(Z), LuLMode::Full);
    assert_eq!(reducer.add_row(&[], &[]), None);
    assert_eq!(reducer.l().nrows(), 0);
    assert_eq!(reducer.add_row(&[c.one().raw().clone()], &[0]), Some(0));
    assert_eq!(reducer.add_row(&[c.integer(2).raw().clone()], &[0]), None);
    assert_eq!(reducer.u().nrows(), 1);
    assert_eq!(reducer.l().nrows(), 2);
    assert_eq!(reducer.add_row(&[c.one().raw().clone()], &[1]), Some(1));
    assert_eq!(reducer.add_row(&[c.one().raw().clone()], &[0]), None);
    assert_eq!(reducer.u().nrows(), 2);
    assert_eq!(reducer.l().nrows(), 3);
}

#[test]
fn selected_subbank_preserves_full_forbidden_structural_zero_columns() {
    let c = context();
    let rows = vec![project::Row::from([
        (shift(0), c.integer(2)),
        (shift(-1), c.one()),
    ])];
    let full_f = BTreeSet::from([shift(1), shift(2)]);
    let (a, b) = compare_direct(&c, &rows, &full_f, limits());
    assert_eq!(a.prefix_rows, 1);
    assert_eq!(a.image.get(&shift(0)), Some(&c.one()));
    assert!(
        full_f
            .iter()
            .all(|s| !a.image.contains_key(s) && !b.image.contains_key(s))
    );
    for backend in [project::project, direct_l::project] {
        assert!(
            backend(
                &c,
                &rows,
                &shift(0),
                &BTreeSet::from([shift(0)]),
                &[],
                limits()
            )
            .is_err()
        );
        let mut narrow = limits();
        narrow.columns = 2;
        assert!(backend(&c, &rows, &shift(0), &full_f, &[], narrow).is_err());
        let no_target = vec![project::Row::from([(shift(-1), c.one())])];
        assert!(matches!(
            backend(&c, &no_target, &shift(0), &full_f, &[], limits()).unwrap(),
            project::Projection::NoTarget { .. }
        ));
    }
}

#[test]
fn structurally_absent_forbidden_column_must_have_bound_context_arity() {
    let base = CoefficientContext::try_new(["d"]).unwrap();
    let family = rustred::family::IntegralFamily::new(
        "forbidden-arity-fixture",
        vec!["k1".into(), "k2".into()],
        Vec::new(),
        base.clone(),
        base.parameter("d").unwrap(),
        vec![
            rustred::family::AffineDenominator::new(
                base.integer(-1),
                vec![base.one(), base.zero(), base.zero()],
            ),
            rustred::family::AffineDenominator::new(
                base.integer(-1),
                vec![base.zero(), base.one(), base.zero()],
            ),
            rustred::family::AffineDenominator::new(
                base.zero(),
                vec![base.zero(), base.zero(), base.one()],
            ),
        ],
        Vec::new(),
        vec![base.zero(); 3],
    )
    .unwrap();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let generated = (0..prepared.len()).map(|i| prepared.generate(i)).collect();
    let completed = prepared.complete(generated).unwrap();
    let batch = generator
        .translate_selected_completed_source_rows(
            &completed,
            [TranslatedSourceRequest::new(
                0,
                IntegralShift::try_new([0, 0, 0]).unwrap(),
            )],
            Default::default(),
        )
        .unwrap();
    let target = batch.sources()[0]
        .terms()
        .keys()
        .find(|s| s.values().iter().all(|&v| v == 0))
        .unwrap();
    let c = generator.context();
    let rows = vec![project::Row::from([(target.clone(), c.one())])];
    for backend in [project::project, direct_l::project] {
        let error = backend(c, &rows, target, &BTreeSet::from([shift(1)]), &[], limits())
            .err()
            .unwrap();
        assert!(error.to_string().contains("forbidden shift arity differs"));
    }
}
