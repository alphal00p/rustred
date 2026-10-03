use super::*;
use rustred::{
    algebra::{CoefficientContext, IndexedCoefficientContext},
    identity::RowId,
};
use std::{ops::ControlFlow, sync::atomic::AtomicBool};

fn context() -> IndexedCoefficientContext {
    IndexedCoefficientContext::try_new(
        &CoefficientContext::try_new(["d"]).unwrap(),
        "symbolic-projector-test",
        1,
    )
    .unwrap()
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
    for f in [BTreeSet::from([shift(0)]), BTreeSet::from([shift(3)])] {
        assert!(project::project(&c, &rows, &shift(0), &f, &[], limits()).is_err());
    }
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
fn tadpole() -> (Vec<u8>, Value) {
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
            .contains("free-axis bound")
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
