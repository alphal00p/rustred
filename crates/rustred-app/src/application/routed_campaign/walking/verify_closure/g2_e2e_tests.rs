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
