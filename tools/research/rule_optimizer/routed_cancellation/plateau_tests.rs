use super::*;
use rustred::algebra::CoefficientContext;

fn context() -> IndexedCoefficientContext {
    IndexedCoefficientContext::try_new(
        &CoefficientContext::try_new(["d"]).unwrap(),
        "plateau-test",
        2,
    )
    .unwrap()
}
fn key(n: [i64; 2]) -> IntegralKey {
    IntegralKey::try_new(n).unwrap()
}
fn limits() -> Limits {
    Limits {
        max_route_calls: 10,
        max_endpoint_occurrences: 1000,
        max_distinct_keys: 100,
        max_coefficient_terms: 1000,
        max_ledger_records: 1000,
        max_report_bytes: 1 << 20,
        expansion: BTreeMap::new(),
    }
}
fn config() -> input::PlateauCut {
    input::PlateauCut {
        max_depth: 2,
        max_parents: 2,
        max_apply_calls: 100,
        max_pending_terms: 100,
    }
}
fn evidence() -> Evidence {
    Evidence {
        table: Some(CoefficientTableBuilder::new(Default::default())),
        ledger: Vec::new(),
        max_records: 1000,
    }
}
fn work() -> Work {
    Work {
        apply_calls: 0,
        source_terms: 0,
        products: 0,
        boundary_occurrences: 0,
        fallbacks: 0,
        all_native_complete: true,
    }
}
fn term(c: &IndexedCoefficientContext, n: [i64; 2], weight: i64) -> SourceTerm {
    SourceTerm {
        key: key(n),
        coefficient: c.integer(weight),
    }
}

#[test]
fn two_substitutions_cancel_only_at_identical_final_cut() {
    let c = context();
    let l = limits();
    let cfg = config();
    let cancel = AtomicBool::new(false);
    let mut e = evidence();
    let mut w = work();
    let root = key([3, 2]);
    let d = c.lift(&c.base().parameter("d").unwrap()).unwrap();
    let denominator = c.sub(&d, &c.integer(4)).unwrap();
    let ratio = c.div(&c.one(), &denominator).unwrap();
    let negative = c.neg_with_limits(&ratio, Default::default()).unwrap();
    let result = cut(
        &root,
        &c,
        Default::default(),
        &l,
        &cfg,
        &cancel,
        &mut e,
        &mut w,
        |k, depth, _, _| {
            if depth == 0 {
                return Ok(Step::Applied(vec![
                    term(&c, [2, 3], 1),
                    term(&c, [1, 4], 1),
                ]));
            }
            let coefficient = if k == &key([2, 3]) {
                ratio.clone()
            } else {
                negative.clone()
            };
            Ok(Step::Applied(vec![SourceTerm {
                key: key([1, 2]),
                coefficient,
            }]))
        },
    )
    .unwrap()
    .report;
    assert_eq!(w.apply_calls, 3);
    assert_eq!(result["pre_coalescing_occurrences"], 2);
    assert_eq!(result["pre_coalescing_distinct_keys"], 1);
    assert_eq!(result["final_nonzero_keys"], 0);
    assert_eq!(result["maximum_reached_depth"], 2);
    assert!(e.table.as_ref().unwrap().len() >= 3);
    assert_eq!(
        e.ledger
            .iter()
            .filter(|x| x["kind"] == "substitution")
            .count(),
        4
    );
}

#[test]
fn distinct_parent_vectors_cannot_cancel_each_other() {
    let c = context();
    let l = limits();
    let cfg = config();
    let cancel = AtomicBool::new(false);
    let mut e = evidence();
    let mut w = work();
    for (root, sign) in [([3, 2], 1), ([2, 3], -1)] {
        let report = cut(
            &key(root),
            &c,
            Default::default(),
            &l,
            &cfg,
            &cancel,
            &mut e,
            &mut w,
            |_, _, _, _| Ok(Step::Applied(vec![term(&c, [1, 1], sign)])),
        )
        .unwrap()
        .report;
        assert_eq!(report["final_nonzero_keys"], 1);
    }
}

#[test]
fn duplicate_intermediate_paths_expand_before_final_cancellation() {
    let c = context();
    let l = limits();
    let mut cfg = config();
    cfg.max_depth = 3;
    let cancel = AtomicBool::new(false);
    let mut e = evidence();
    let mut w = work();
    let result = cut(
        &key([4, 2]),
        &c,
        Default::default(),
        &l,
        &cfg,
        &cancel,
        &mut e,
        &mut w,
        |k, depth, _, _| {
            Ok(Step::Applied(match depth {
                0 => vec![term(&c, [3, 3], 1), term(&c, [2, 4], -1)],
                1 => vec![term(&c, [1, 5], 1)],
                2 => {
                    assert_eq!(k, &key([1, 5]));
                    vec![term(&c, [1, 1], 1)]
                }
                _ => panic!("past fixed cut"),
            }))
        },
    )
    .unwrap()
    .report;
    assert_eq!(w.apply_calls, 5);
    assert_eq!(result["pre_coalescing_occurrences"], 2);
    assert_eq!(result["pre_coalescing_distinct_keys"], 1);
    assert_eq!(result["final_nonzero_keys"], 0);
}

#[test]
fn second_coefficient_is_specialized_at_child_not_original_parent() {
    let c = context();
    let l = limits();
    let cfg = config();
    let cancel = AtomicBool::new(false);
    let mut e = evidence();
    let mut w = work();
    let result = cut(
        &key([3, 2]),
        &c,
        Default::default(),
        &l,
        &cfg,
        &cancel,
        &mut e,
        &mut w,
        |k, depth, _, _| {
            if depth == 0 {
                return Ok(Step::Applied(vec![term(&c, [2, 3], 5)]));
            }
            let (factor, guard) = c
                .specialize(&c.index(0).unwrap(), k.powers(), Default::default())
                .unwrap();
            assert!(guard.is_none());
            let factor = c.lift(&factor).unwrap();
            assert_eq!(factor, c.integer(2));
            Ok(Step::Applied(vec![SourceTerm {
                key: key([1, 1]),
                coefficient: factor,
            }]))
        },
    )
    .unwrap();
    assert_eq!(result.report["final_nonzero_keys"], 1);
    let expected = e.coefficient(&c.integer(10)).unwrap();
    assert_eq!(result.report["boundary"][0]["coefficient"], expected);
}

#[test]
fn off_plateau_pinches_increases_and_depth_boundary_are_all_retained() {
    let c = context();
    let l = limits();
    let mut cfg = config();
    cfg.max_depth = 1;
    let cancel = AtomicBool::new(false);
    let mut e = evidence();
    let mut w = work();
    let report = cut(
        &key([3, 2]),
        &c,
        Default::default(),
        &l,
        &cfg,
        &cancel,
        &mut e,
        &mut w,
        |_, _, _, _| {
            Ok(Step::Applied(vec![
                term(&c, [0, 5], 1),
                term(&c, [2, 2], 1),
                term(&c, [4, 2], 1),
                term(&c, [2, 3], 1),
            ]))
        },
    )
    .unwrap()
    .report;
    assert_eq!(w.apply_calls, 1);
    assert_eq!(report["final_nonzero_keys"], 4);
    for label in [
        "support-change-unrouted",
        "strict-E-decrease",
        "E-increase",
        "depth-cut-plateau",
    ] {
        assert_eq!(report["stop_reasons"][label], 1);
    }
}

#[test]
fn conditional_transaction_discards_entire_prefix_and_carries_input_weight() {
    let c = context();
    let l = limits();
    let cfg = config();
    let cancel = AtomicBool::new(false);
    let mut transaction = Transaction::default();
    transaction.selected = Some((0, 7));
    transaction.classified = 1;
    transaction.finished = 1;
    transaction.terms = vec![term(&c, [1, 1], 17)];
    transaction.unsafe_reason = Some("conditional".into());
    let stats = OwnerAppliedStats {
        selected_pieces: 1,
        successors: 1,
        conditional_successors: 1,
        ..Default::default()
    };
    assert!(!transaction.accepted(&stats, true));
    let mut e = evidence();
    let mut w = work();
    let report = cut(
        &key([3, 2]),
        &c,
        Default::default(),
        &l,
        &cfg,
        &cancel,
        &mut e,
        &mut w,
        |_, depth, _, _| {
            if depth == 0 {
                Ok(Step::Applied(vec![term(&c, [2, 3], 5)]))
            } else {
                Ok(Step::Boundary("conditional whole-child fallback".into()))
            }
        },
    )
    .unwrap()
    .report;
    assert_eq!(report["final_nonzero_keys"], 1);
    assert_eq!(report["boundary"][0]["key"], json!([2, 3]));
    assert!(!e.ledger.iter().any(|v| v["to"] == json!([1, 1])));
    assert_eq!(w.fallbacks, 1);
}

#[test]
fn empty_completed_rule_differs_from_missing_finish() {
    let mut transaction = Transaction::default();
    transaction.selected = Some((0, 1));
    transaction.classified = 1;
    let stats = OwnerAppliedStats {
        selected_pieces: 1,
        ..Default::default()
    };
    assert!(!transaction.accepted(&stats, true));
    transaction.finished = 1;
    assert!(transaction.accepted(&stats, true));
    assert!(!transaction.accepted(&stats, false));
    let c = context();
    let l = limits();
    let cfg = config();
    let cancel = AtomicBool::new(false);
    let report = cut(
        &key([3, 2]),
        &c,
        Default::default(),
        &l,
        &cfg,
        &cancel,
        &mut evidence(),
        &mut work(),
        |_, _, _, _| Ok(Step::Applied(Vec::new())),
    )
    .unwrap()
    .report;
    assert_eq!(report["root_applied"], true);
    assert_eq!(report["final_nonzero_keys"], 0);
}

#[test]
fn global_budget_or_cancellation_cannot_publish_complete_cut() {
    let c = context();
    let l = limits();
    let mut cfg = config();
    cfg.max_apply_calls = 1;
    let cancel = AtomicBool::new(false);
    assert!(
        cut(
            &key([3, 2]),
            &c,
            Default::default(),
            &l,
            &cfg,
            &cancel,
            &mut evidence(),
            &mut work(),
            |_, _, _, _| Ok(Step::Applied(vec![term(&c, [2, 3], 1)]))
        )
        .unwrap_err()
        .contains("Apply calls")
    );
    cfg.max_apply_calls = 100;
    cfg.max_pending_terms = 1;
    assert!(
        cut(
            &key([3, 2]),
            &c,
            Default::default(),
            &l,
            &cfg,
            &cancel,
            &mut evidence(),
            &mut work(),
            |_, _, _, _| Ok(Step::Applied(vec![
                term(&c, [2, 3], 1),
                term(&c, [1, 4], 1)
            ]))
        )
        .is_err()
    );
    cancel.store(true, Ordering::Relaxed);
    assert!(
        cut(
            &key([3, 2]),
            &c,
            Default::default(),
            &l,
            &cfg,
            &cancel,
            &mut evidence(),
            &mut work(),
            |_, _, _, _| panic!("must not apply after cancel")
        )
        .is_err()
    );
}

#[test]
fn singleton_caps_are_checked_but_not_reapplied_to_children() {
    let parent = key([3, -2]);
    let mut q = query(&parent, "parent".into());
    q.max_numerator_rank = Some(1);
    assert!(check_query_point(&q, &parent).is_err());
    q.max_numerator_rank = Some(2);
    assert!(check_query_point(&q, &parent).is_ok());
    let child = query(&key([4, -3]), "child".into());
    assert_eq!(child.max_numerator_rank, None);
    assert_eq!(child.power_bounds, Powers::default());
    assert_eq!(degree(&parent), Degree { a: 3, r: 2, e: 4 });
}

#[test]
fn native_redundant_singleton_caps_preserve_exact_point_not_literal_metadata() {
    // Exact failed first4L observation: input R<=1 was normalized to R<=0.
    let point = IntegralKey::try_new([0, 1, 1, 1, 1, 0, 0, 1, 2, 0]).unwrap();
    let mut q = query(&point, "actual-first-four-loop-point".into());
    q.max_numerator_rank = Some(1);
    q.power_bounds = Powers {
        max_positive_power: Some(8),
        min_power_difference: Some(7),
        max_power_difference: Some(7),
    };
    let owner = mask::<10>(&q.owner).unwrap();
    assert_eq!(degree(&point).r, 0);
    assert!(classified_scope(&q, &owner, &q.lower, &q.upper, Some(0), q.power_bounds).is_ok());
    assert!(
        classified_scope(
            &q,
            &owner,
            &q.lower,
            &q.upper,
            Some(0),
            Powers {
                max_positive_power: Some(7),
                ..q.power_bounds
            }
        )
        .is_ok()
    );
    assert!(
        classified_scope(
            &q,
            &owner,
            &q.lower,
            &q.upper,
            Some(0),
            Powers {
                max_positive_power: Some(6),
                ..q.power_bounds
            }
        )
        .is_err()
    );
    let mut wider = q.upper.clone();
    wider[0] = Some(1);
    assert!(classified_scope(&q, &owner, &q.lower, &wider, Some(0), q.power_bounds).is_err());
    let mut changed_owner = owner;
    changed_owner[0] = true;
    assert!(
        classified_scope(
            &q,
            &changed_owner,
            &q.lower,
            &q.upper,
            Some(0),
            q.power_bounds
        )
        .is_err()
    );
    // Descendants begin with no entry caps. A native finite cap can be exact.
    let child = query(&key([3, -1]), "child".into());
    let owner = [true, false];
    assert!(
        classified_scope(
            &child,
            &owner,
            &child.lower,
            &child.upper,
            Some(1),
            Powers::default()
        )
        .is_ok()
    );
    assert!(
        classified_scope(
            &child,
            &owner,
            &child.lower,
            &child.upper,
            Some(0),
            Powers::default()
        )
        .is_err()
    );
    let mut excluded = child;
    excluded.max_numerator_rank = Some(0);
    assert!(
        classified_scope(
            &excluded,
            &owner,
            &excluded.lower,
            &excluded.upper,
            Some(1),
            Powers::default()
        )
        .is_err()
    );
}

#[test]
fn plateau_schema_is_explicit_and_bounded_without_changing_legacy_shape() {
    assert!(
        serde_json::from_value::<input::PlateauCut>(
            json!({"max_depth":2,"max_parents":29,"max_apply_calls":100,"max_pending_terms":100})
        )
        .is_ok()
    );
    assert!(
        serde_json::from_value::<input::PlateauCut>(
            json!({"max_depth":-1,"max_parents":29,"max_apply_calls":100,"max_pending_terms":100})
        )
        .is_err()
    );
    assert!(
        serde_json::from_value::<input::PlateauCut>(
            json!({"max_depth":2,"max_parents":29,"max_apply_calls":100})
        )
        .is_err()
    );
    assert!(serde_json::from_value::<input::PlateauCut>(Value::Null).is_err());

    use crate::applied_observer::input::{NativeLimits, RecorderLimits, Request as Observation};
    let map = |names: &str| {
        names
            .split_whitespace()
            .map(|x| (x.to_owned(), 1))
            .collect()
    };
    let mut request = Request {
        schema: "rustred.routed-cancellation.request.v1".into(),
        observation: Observation {
            schema: "rustred.owner-applied-observer.request.v1".into(),
            selection: "selection".into(),
            queries: "queries".into(),
            preparation_targets_csv: "targets".into(),
            owner_base: "owners".into(),
            workers: 1,
            native_limits: NativeLimits {
                applied: map(
                    "max_term_visits max_shift_groups max_boundary_cells max_sign_splits max_native_operations max_events max_scratch_terms max_scratch_boxes max_scratch_coordinate_cells",
                ),
                matching: map(
                    "max_rules max_terminal_checks max_predicates max_pieces max_cells max_split_operations max_coordinate_cells max_bounded_refinement_cells",
                ),
                guard: map(
                    "max_input_terms max_coefficient_equations max_univariate_degree max_factor_variables max_factor_total_degree max_factor_dense_slots max_total_integer_bits max_factor_recombination_subsets max_gcd_factor_work max_factor_terms max_exact_hyperplane_replay_substitutions max_exact_hyperplane_replay_terms max_exact_hyperplane_replay_work",
                ),
                refinement_axes: "inactive-only".into(),
                cell_refinement_max_cardinality: None,
            },
            recorder_limits: RecorderLimits {
                max_input_bytes: 1 << 20,
                max_queries: 100,
                max_events: 1000,
                max_json_bytes: 1 << 20,
                max_record_bytes: 1 << 16,
                max_coefficients: 1000,
                max_atom_bytes: 1 << 16,
                max_total_atom_bytes: 1 << 20,
                max_state_bytes: 1 << 20,
                max_error_bytes: 128,
            },
        },
        expected_successors: 1,
        expected_strict_subsupport_successors: 0,
        limits: limits(),
        plateau_cut: None,
    };
    request.limits.expansion = map(
        "max_factors max_relation_coefficient_entries max_total_power max_native_polynomial_terms max_native_polynomial_operations max_native_exponent_entries max_endpoints max_endpoint_power_entries max_retained_endpoint_key_bytes max_retained_coefficient_terms max_retained_coefficient_clone_owned_bytes",
    );
    let mut queries = Queries {
        schema: "rustred.owner-domain-queries.json.v2".into(),
        queries: vec![query(&key([3, 2]), "root".into())],
        query_roles: None,
    };
    assert!(request.validate(&queries).is_ok());
    let legacy = serde_json::to_value(&request).unwrap();
    assert!(legacy.get("plateau_cut").is_none());
    assert_eq!(
        serde_json::to_value(serde_json::from_value::<Request>(legacy.clone()).unwrap()).unwrap(),
        legacy
    );
    request.plateau_cut = Some(config());
    request.expected_successors = 0;
    assert!(request.validate(&queries).is_ok());
    request.plateau_cut.as_mut().unwrap().max_depth = 0;
    assert!(request.validate(&queries).is_err());
    request.plateau_cut.as_mut().unwrap().max_depth = 2;
    queries
        .queries
        .push(query(&key([3, 2]), "duplicate".into()));
    assert!(
        request
            .validate(&queries)
            .unwrap_err()
            .contains("duplicate")
    );
}
