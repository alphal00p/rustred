//! G2' residual anchors end to end on the two-loop sunset walk (the DRAINED
//! fixture of `e2e_tests` plus `--g2-residual-anchors union`): the walk plans
//! residual D bands against merged anchors, the oracle PASSes the gate with
//! the exact multi-target cover on every G2' record, every G2' mutation
//! FAILs, an Ordered G2' walk is identical at W1 and W4, and the final
//! checkpoint passes the restore validators (G2' log, stamps, kinds, edges,
//! exact cover).
use super::e2e_tests::*;
use super::*;
use std::collections::BTreeSet;

fn g2_fixture() -> Fixture<'static> {
    let query = |id: &str, owner: &str, upper: [u64; 3], rank: u32, a: u64, d: i64| {
        json!({"id": id, "owner": owner, "lower": [0, 0, 0], "upper": upper, "max_numerator_rank": rank,
            "power_bounds": {"max_positive_power": a, "min_power_difference": d, "max_power_difference": null}})
    };
    Fixture {
        rank: 2,
        queries: json!({"schema": "rustred.owner-domain-queries.json.v2", "queries": [
            query("anchor011", "011", [0, 5, 0], 0, 7, 5),
            query("anchor110", "110", [5, 0, 0], 0, 7, 5),
            query("anchor", "111", [3, 3, 3], 2, 5, 2),
            query("big", "111", [5, 5, 5], 2, 8, 0),
            query("big-inner", "111", [4, 4, 4], 2, 6, 1),
        ]}),
        lookahead: 1,
        omit: &[],
        routes: json!([]),
    }
}

/// The G2' walk at `workers` (the argv parser's own --workers replaced).
fn g2_run(label: &str, workers: &str, lookahead: u32) -> Run {
    let mut fixture = g2_fixture();
    fixture.lookahead = lookahead;
    let dir = scratch(label);
    let selection = write_owners(&dir.0, fixture.rank, fixture.omit, &fixture.routes);
    std::fs::write(dir.0.join("selection.json"), selection).unwrap();
    std::fs::write(dir.0.join("queries.json"), fixture.queries.to_string()).unwrap();
    let mut argv = walk_argv(
        &dir.0,
        fixture.lookahead,
        &["--g2-residual-anchors", "union"],
    );
    let at = argv.iter().position(|a| a == "--workers").unwrap();
    argv[at + 1] = workers.into();
    let request = crate::cli::walk_request_from_argv(argv).unwrap();
    let result = walk(&dir.0, &request);
    Run {
        dir,
        request,
        result,
    }
}

fn kinds(result: &Value) -> BTreeMap<String, u64> {
    let mut kinds = BTreeMap::new();
    for row in result["domains"].as_array().unwrap() {
        *kinds
            .entry(row["record_kind"].as_str().unwrap().to_owned())
            .or_insert(0) += 1;
    }
    kinds
}

#[test]
fn epoch_union_real_plans_save_resume_and_independent_cold_reinspection() {
    epoch_union_save_resume_mode(false);
    epoch_union_save_resume_mode(true);
}

fn epoch_union_save_resume_mode(ready: bool) {
    let fixture = g2_fixture();
    let dir = scratch("epoch-g2-cp6");
    let selection = write_owners(&dir.0, fixture.rank, fixture.omit, &fixture.routes);
    std::fs::write(dir.0.join("selection.json"), selection).unwrap();
    std::fs::write(dir.0.join("queries.json"), fixture.queries.to_string()).unwrap();
    let mut argv = walk_argv(&dir.0, 1, &["--g2-residual-anchors", "union"]);
    let at = argv
        .iter()
        .position(|s| s == "--publication-policy")
        .unwrap();
    argv[at + 1] = "epoch".into();
    let mut request = crate::cli::walk_request_from_argv(argv).unwrap();
    assert_eq!(request.workers, 1);
    assert!(
        !request.epoch_rolling,
        "exercise the public lockstep Union path"
    );
    if ready {
        request.epoch_rolling = true;
        request.epoch_publication_order = crate::OwnerDomainWalkEpochPublicationOrder::OldestReady;
        request.epoch_window = Some(16);
    }
    let result = walk(&dir.0, &request);
    assert_eq!(
        result["epoch"]["schedule"]["kind"],
        if ready { "rolling" } else { "lockstep" }
    );
    if ready {
        assert_eq!(
            result["epoch"]["schedule"]["publication_order"],
            "oldest_ready_sequences"
        );
    }
    assert_eq!(result["recursive_worklist_exhausted"], true, "{result}");
    assert!(
        result["g2_residual_anchors"]["logged_g2_records"]
            .as_u64()
            .unwrap()
            > 0,
        "{result}"
    );
    let mut options = OwnerDomainWalkVerifyOptions::new(dir.0.join("checkpoint"));
    options.result = None;
    options.threads = 1;
    options.require_closure = true;
    let manifest = std::fs::read(dir.0.join("checkpoint/latest.json")).unwrap();
    let session = std::fs::read(dir.0.join("checkpoint/epoch-session.bin")).unwrap();
    for _ in 0..2 {
        let report =
            owner_domain_walk_verify_closure(&request, &options, &AtomicBool::new(false), |_| {})
                .unwrap();
        assert_eq!(report["verdict"], "PASS", "{report}");
    }
    assert_eq!(
        std::fs::read(dir.0.join("checkpoint/latest.json")).unwrap(),
        manifest
    );
    assert_eq!(
        std::fs::read(dir.0.join("checkpoint/epoch-session.bin")).unwrap(),
        session
    );
    request.checkpoint.as_mut().unwrap().resume = true;
    let resumed = walk(&dir.0, &request);
    assert_eq!(resumed["recursive_worklist_exhausted"], true);
    assert_eq!(
        resumed["epoch"]["records_digest"],
        result["epoch"]["records_digest"]
    );
    assert_eq!(
        resumed["epoch"]["edge_digest"],
        result["epoch"]["edge_digest"]
    );
    request.g2_residual_anchors = crate::OwnerDomainWalkG2ResidualAnchors::Off;
    assert!(
        crate::owner_domain_walk_with_progress(request, &AtomicBool::new(false), |_| {}).is_err()
    );
}

#[test]
fn epoch_union_rescue_abandons_auxiliaries_and_cold_certifies_unchanged_required_scope() {
    epoch_union_rescue_mode(false);
    epoch_union_rescue_mode(true);
}

fn epoch_union_rescue_mode(ready: bool) {
    let mut fixture = g2_fixture();
    fixture.queries["query_roles"] = json!({"required":["anchor011"],
        "auxiliary":["anchor110","anchor","big","big-inner"]});
    let dir = scratch("epoch-g2-rescue-required");
    let selection = write_owners(&dir.0, fixture.rank, fixture.omit, &fixture.routes);
    std::fs::write(dir.0.join("selection.json"), selection).unwrap();
    std::fs::write(dir.0.join("queries.json"), fixture.queries.to_string()).unwrap();
    let mut argv = walk_argv(&dir.0, 1, &["--g2-residual-anchors", "union"]);
    let at = argv
        .iter()
        .position(|s| s == "--publication-policy")
        .unwrap();
    argv[at + 1] = "epoch".into();
    let mut request = crate::cli::walk_request_from_argv(argv).unwrap();
    if ready {
        request.epoch_rolling = true;
        request.epoch_publication_order = crate::OwnerDomainWalkEpochPublicationOrder::OldestReady;
        request.epoch_window = Some(16);
    }
    // Public allowances must be positive. One event still stops the complete
    // five-root cut before publication, leaving real obligations to abandon.
    request.max_events = 1;
    let stopped = walk(&dir.0, &request);
    assert_eq!(stopped["stop_reason"], "event_allowance", "{stopped}");
    assert_eq!(stopped["epoch"]["k"], 0);
    let p0 = stopped["epoch"]["p0"].as_u64().unwrap();
    request.max_events = 1_000_000;
    request.checkpoint.as_mut().unwrap().resume = true;
    let mut helper = fixture.queries["queries"][1].clone();
    helper["id"] = "replacement110".into();
    let amendment = json!({"schema":super::super::rescue::AMENDMENT_SCHEMA,"sequence":1,
        "parent":checkpoint::epoch_request_binding(&request),"queries":[helper],
        "supersede":["anchor110","anchor","big","big-inner"]})
    .to_string();
    request.amendments.push(crate::OwnerDomainWalkAmendment {
        path: dir.0.join("amendment.json"),
        text: amendment,
    });
    let rescued = walk(&dir.0, &request);
    assert_eq!(rescued["recursive_worklist_exhausted"], true, "{rescued}");
    assert_eq!(rescued["epoch"]["p0"], p0);
    assert!(rescued["abandoned_obligations"].as_u64().unwrap() > 0);
    assert_eq!(rescued["query_certification"]["required_queries_total"], 1);
    assert_eq!(rescued["required_queries_resolved"], true, "{rescued}");
    let (raw, sections, _) = epoch_checkpoint::read_raw::<3>(&dir.0.join("checkpoint")).unwrap();
    assert_eq!(raw.inputs[0]["role"], "required");
    assert_eq!(raw.inputs.last().unwrap()["role"], "auxiliary");
    assert_eq!(raw.domains[1], raw.domains[p0 as usize]);
    assert!(sections.ledger.iter().any(|word| word >> 61 == 7));
    let mut options = OwnerDomainWalkVerifyOptions::new(dir.0.join("checkpoint"));
    options.require_closure = true;
    options.certification_scope = OwnerDomainWalkVerifyScope::PhysicsQueries;
    let manifest = std::fs::read(dir.0.join("checkpoint/latest.json")).unwrap();
    let session = std::fs::read(dir.0.join("checkpoint/epoch-session.bin")).unwrap();
    let report =
        owner_domain_walk_verify_closure(&request, &options, &AtomicBool::new(false), |_| {})
            .unwrap();
    assert_eq!(report["verdict"], "PASS", "{report}");
    assert_eq!(
        report["certification_scope"],
        "physics_queries_through_closed_containing_roots"
    );
    options.certification_scope = OwnerDomainWalkVerifyScope::AllRoots;
    let all = owner_domain_walk_verify_closure(&request, &options, &AtomicBool::new(false), |_| {})
        .unwrap();
    assert_ne!(
        all["verdict"], "PASS",
        "abandoned helpers must not become closed"
    );
    assert_eq!(
        std::fs::read(dir.0.join("checkpoint/latest.json")).unwrap(),
        manifest
    );
    assert_eq!(
        std::fs::read(dir.0.join("checkpoint/epoch-session.bin")).unwrap(),
        session
    );
    let resumed = walk(&dir.0, &request);
    assert_eq!(
        resumed["epoch"]["records_digest"],
        rescued["epoch"]["records_digest"]
    );
    assert_eq!(
        resumed["epoch"]["edge_digest"],
        rescued["epoch"]["edge_digest"]
    );
    let mut mutated = request.clone();
    mutated.amendments[0].text.push(' ');
    assert!(
        crate::owner_domain_walk_with_progress(mutated, &AtomicBool::new(false), |_| {}).is_err()
    );
}

#[test]
fn g2_sunset_walk_plans_residuals_passes_the_gate_and_every_g2_mutation_fails() {
    let run = g2_run("g2-drained", "1", 1);
    let result = &run.result;
    assert_eq!(result["status"], "locally_resolved", "{}", result["error"]);
    let kinds = kinds(result);
    let g2 = kinds
        .get("g2_residual_anchor_inspection")
        .copied()
        .unwrap_or(0);
    assert!(g2 >= 2, "no G2' plans: {kinds:?}");
    assert_eq!(result["g2_residual_anchors"]["logged_g2_records"], g2);
    assert_eq!(result["delegation"]["g2_records"], g2);
    assert_eq!(result["delegation"]["g2_blocked"], 0);
    for row in result["domains"].as_array().unwrap() {
        if row["record_kind"] == "g2_residual_anchor_inspection" {
            assert_eq!(
                row["responsibility_status"],
                "discharged_by_residual_and_g2_anchors"
            );
            assert_eq!(row["descendant_closed"], true);
        }
    }
    let report = verify_run(&run, |o| o.require_closure = true);
    assert_eq!(report["verdict"], "PASS", "{}", report["violations"]);
    assert!(report["roots_independently_verified"] == report["roots_total"]);
    let union = &report["containment"]["g2_union_cover"];
    assert_eq!(union["checks"].as_u64(), Some(g2));
    assert_eq!(union["covered"].as_u64(), Some(g2));
    assert_eq!(union["brute_force_disagreements"], 0);
    assert_eq!(report["counts"]["records"]["g2_records"].as_u64(), Some(g2));
    // Every G2' mutation fails, with its structural class among the classes.
    use OwnerDomainWalkVerifyMutation as M;
    for (kind, class) in [
        (M::G2ShrunkResidual, "g2_union_cover"),
        (M::G2LateAnchor, "g2_anchor_order"),
        (M::G2InadmissibleAnchor, "g2_anchor_kind"),
        (M::G2DroppedAnchorEdge, "missing_edge"),
        (M::G2AnchorCycle, "g2_anchor_order"),
    ] {
        let mutated = verify_run(&run, |o| {
            o.require_closure = true;
            o.mutation = Some(kind);
        });
        println!(
            "{}",
            json!({"mutation": kind.name(), "verdict": mutated["verdict"],
                "applied": mutated["mutation"]["applied"], "reason": mutated["mutation"]["reason"],
                "classes": mutated["violations_by_class"]})
        );
        if mutated["mutation"]["applied"] != true {
            assert!(
                matches!(
                    kind,
                    M::G2AnchorCycle | M::G2ShrunkResidual | M::G2InadmissibleAnchor
                ),
                "{} not applied: {}",
                kind.name(),
                mutated["mutation"]["reason"]
            );
            continue;
        }
        assert_eq!(mutated["verdict"], "FAIL", "{}", kind.name());
        let classes: BTreeSet<String> = classes(&mutated);
        assert!(classes.contains(class), "{}: {classes:?}", kind.name());
    }
    // The final checkpoint restores: the G2' log, its stamps, kinds, anchor
    // edges and exact covers pass the restore validators.
    let restored = crate::application::routed_campaign::walking::checkpoint::test_support::restore_directory::<3>(
        &run.dir.0.join("checkpoint"),
    )
    .unwrap();
    let ledger = restored.state.queue.delegation.as_ref().unwrap();
    let log = ledger.g2().unwrap();
    assert_eq!(log.g2_records() as u64, g2);
}

#[test]
fn g2_ordered_walk_is_identical_at_one_and_four_workers() {
    // Ordered snapshot `id + 1 - lookahead`: a short lookahead leaves plans.
    let serial = g2_run("g2-w1", "1", 3);
    let pool = g2_run("g2-w4", "4", 3);
    let planned = serial.result["g2_residual_anchors"]["residual_plans"]
        .as_u64()
        .unwrap()
        + serial.result["g2_residual_anchors"]["full_cover_plans"]
            .as_u64()
            .unwrap();
    assert!(planned > 0, "{}", serial.result["g2_residual_anchors"]);
    let records = |run: &Run| {
        run.result["domains"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| {
                let mut row = row.clone();
                row.as_object_mut()
                    .unwrap()
                    .retain(|key, _| key != "seconds" && !key.ends_with("_seconds"));
                if let Some(block) = row.get_mut("g2_residual_anchors") {
                    block.as_object_mut().unwrap().remove("plan_seconds");
                }
                row
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(records(&serial), records(&pool));
    assert_eq!(
        serial.result["g2_residual_anchors"]["residual_plans"],
        pool.result["g2_residual_anchors"]["residual_plans"]
    );
}

/// G2' activation: a walk checkpointed WITHOUT G2' pauses, resumes with
/// `--g2-residual-anchors union --g2-activate-on-resume` (the log is
/// back-filled, the binding amended and recorded), drains, and PASSes the
/// oracle gate under the union request; a later resume needs no activation.
#[test]
fn g2_activation_on_resume_of_a_checkpoint_written_without_g2() {
    let fixture = g2_fixture();
    let dir = scratch("g2-activate");
    let selection = write_owners(&dir.0, fixture.rank, fixture.omit, &fixture.routes);
    std::fs::write(dir.0.join("selection.json"), selection).unwrap();
    std::fs::write(dir.0.join("queries.json"), fixture.queries.to_string()).unwrap();
    let off =
        crate::cli::walk_request_from_argv(walk_argv(&dir.0, fixture.lookahead, &[])).unwrap();
    let stop = AtomicBool::new(false);
    let paused = crate::owner_domain_walk_with_progress(off, &stop, |event| {
        if event["committed_domains"].as_u64().is_some_and(|c| c >= 12) {
            stop.store(true, Ordering::Release);
        }
    })
    .unwrap();
    assert_eq!(paused.document["status"], "paused", "{}", paused.document);
    let resume_argv = |extra: &[&str]| {
        let mut argv = walk_argv(&dir.0, fixture.lookahead, extra);
        let at = argv.iter().position(|a| a == "--checkpoint").unwrap();
        argv[at] = "--resume".into();
        argv
    };
    // Without the activation flag a union resume is refused (binding differs).
    let refused =
        crate::cli::walk_request_from_argv(resume_argv(&["--g2-residual-anchors", "union"]))
            .unwrap();
    assert!(
        crate::owner_domain_walk_with_progress(refused, &AtomicBool::new(false), |_| {}).is_err()
    );
    let request = crate::cli::walk_request_from_argv(resume_argv(&[
        "--g2-residual-anchors",
        "union",
        "--g2-activate-on-resume",
    ]))
    .unwrap();
    let result = walk(&dir.0, &request);
    assert_eq!(result["status"], "locally_resolved", "{}", result["error"]);
    assert_eq!(result["checkpoint"]["g2_activation"]["from"], "off");
    assert!(
        result["g2_residual_anchors"]["logged_apply_natives"]
            .as_u64()
            .unwrap()
            > 0
    );
    // The amended binding is the union binding: a plain union resume opens.
    let again =
        crate::cli::walk_request_from_argv(resume_argv(&["--g2-residual-anchors", "union"]))
            .unwrap();
    let run = Run {
        dir,
        request,
        result,
    };
    let report = verify_run(&run, |o| o.require_closure = true);
    assert_eq!(report["verdict"], "PASS", "{}", report["violations"]);
    assert!(report["roots_independently_verified"] == report["roots_total"]);
    let rerun =
        crate::owner_domain_walk_with_progress(again, &AtomicBool::new(false), |_| {}).unwrap();
    assert_eq!(rerun.document["status"], "locally_resolved");
}
