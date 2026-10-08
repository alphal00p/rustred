use super::super::e2e_tests::{Fixture, Run, run, scratch, walk, walk_argv, write_owners};
use super::super::{OwnerDomainWalkVerifyScope, checkpoint};
use super::*;
use std::sync::atomic::Ordering;

fn compare(run: &Run, require_closure: bool) -> OwnerDomainWalkInventory {
    let mut options = OwnerDomainWalkInventoryOptions::new(run.dir.0.join("checkpoint"));
    options.verification.require_closure = require_closure;
    options.verification.threads = 2;
    let fast = owner_domain_walk_inventory(&run.request, &options, &AtomicBool::new(false), |_| {})
        .unwrap();
    options.deep_verification = true;
    let deep = owner_domain_walk_inventory(&run.request, &options, &AtomicBool::new(false), |_| {})
        .unwrap();
    assert_eq!(fast.terminal_keys(), deep.terminal_keys());
    assert_eq!(
        fast.rules(0, 1000).unwrap().items,
        deep.rules(0, 1000).unwrap().items
    );
    for field in [
        "rules",
        "terminals",
        "selected_rule_events",
        "terminal_events",
        "zero_sector_events",
    ] {
        assert_eq!(
            fast.summary()["encountered"][field],
            deep.summary()["encountered"][field],
            "{field}"
        );
    }
    assert_eq!(
        fast.summary()["complete"],
        deep.summary()["complete"],
        "{}\n{}",
        fast.summary(),
        deep.summary()
    );
    assert_eq!(fast.summary()["independently_verified"], false);
    assert_eq!(fast.summary()["authority"], "trusted_saved_scope");
    assert_ne!(fast.summary()["verification"]["verdict"], "PASS");
    assert_eq!(
        fast.summary()["verification"]["reinspection"]["complete"],
        false
    );
    fast
}

fn query(id: &str, owner: &str, upper: [u64; 3], rank: u32, a: u64, d: i64) -> Value {
    json!({"id":id,"owner":owner,"lower":[0,0,0],"upper":upper,"max_numerator_rank":rank,
        "power_bounds":{"max_positive_power":a,"min_power_difference":d,"max_power_difference":null}})
}

fn bands() -> Fixture<'static> {
    Fixture {
        rank: 2,
        lookahead: 1,
        omit: &[],
        routes: json!([]),
        queries: json!({"schema":"rustred.owner-domain-queries.json.v2","queries":[
            query("anchor011","011",[0,5,0],0,7,5),
            query("anchor110","110",[5,0,0],0,7,5),
            query("anchor","111",[3,3,3],2,5,2),
            query("big","111",[5,5,5],2,8,0),
            query("big-inner","111",[4,4,4],2,6,1),
        ]}),
    }
}

#[test]
fn fast_inventory_matches_deep_partial_and_g2_residuals() {
    for g2 in [false, true] {
        let run = run(
            "inventory-fast-bands",
            &bands(),
            if g2 {
                &["--g2-residual-anchors", "union"]
            } else {
                &[]
            },
        );
        assert!(
            run.result["domains"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["record_kind"]
                    == if g2 {
                        "g2_residual_anchor_inspection"
                    } else {
                        "partial_initial_overlap_inspection"
                    })
        );
        let fast = compare(&run, true);
        assert_eq!(fast.summary()["complete"], true);
        fast.normalize_terminals(Default::default()).unwrap();
    }
}

#[test]
fn fast_inventory_skips_route_replay_without_losing_apply_identities() {
    let fixture = Fixture {
        rank: 2,
        lookahead: 256,
        omit: &["011"],
        routes: json!([{"source_mask":"011","owner_mask":"110","requires_transport":true,
            "source_to_representative":[["1","-1"],["0","1"]],"owner_to_representative":[["1","0"],["0","1"]]}]),
        queries: json!({"schema":"rustred.owner-domain-queries.json.v2","queries":[
            query("anchor","111",[3,3,3],2,5,2),query("s101","101",[4,0,4],1,6,1)]}),
    };
    let run = run("inventory-fast-routed", &fixture, &[]);
    let fast = compare(&run, true);
    assert!(
        fast.summary()["verification"]["inventory_census"]["route_records_skipped"]
            .as_u64()
            .unwrap()
            > 0
    );
}

#[test]
fn fast_inventory_preserves_failed_prefixes_and_cell_refinement_classifications() {
    let fixture = Fixture {
        rank: 0,
        lookahead: 1,
        omit: &[],
        routes: json!([]),
        queries: json!({"schema":"rustred.owner-domain-queries.json.v2","queries":[
            query("dots","111",[2,0,0],0,5,3)]}),
    };
    for (extra, failed) in [
        (vec!["--apply-cell-refinement-max-cardinality", "3"], false),
        (vec!["--max-native-operations-per-query", "1"], true),
    ] {
        let run = if failed {
            // Ordered/CP5 failures remain uncommitted, so they correctly add
            // no saved-record classifications. CP6 commits native_error rows
            // and exercises the prefix-preserving fallback itself.
            let dir = scratch("inventory-fast-prefix-cp6");
            let selection = write_owners(&dir.0, fixture.rank, fixture.omit, &fixture.routes);
            std::fs::write(dir.0.join("selection.json"), selection).unwrap();
            std::fs::write(dir.0.join("queries.json"), fixture.queries.to_string()).unwrap();
            let mut argv = walk_argv(&dir.0, 1, &extra);
            let at = argv
                .iter()
                .position(|s| s == "--publication-policy")
                .unwrap();
            argv[at + 1] = "epoch".into();
            let request = crate::cli::walk_request_from_argv(argv).unwrap();
            let result = walk(&dir.0, &request);
            assert_eq!(result["failed_nodes"], 1);
            Run {
                dir,
                request,
                result,
            }
        } else {
            run("inventory-fast-prefix", &fixture, &extra)
        };
        let fast = compare(&run, !failed);
        assert_eq!(
            fast.summary()["verification"]["inventory_census"]["failed_record_replays"]
                .as_u64()
                .unwrap()
                > 0,
            failed
        );
    }
}

#[test]
fn fast_inventory_cancellation_and_foreign_query_never_claim_completion() {
    let fixture = Fixture {
        rank: 0,
        lookahead: 1,
        omit: &[],
        routes: json!([]),
        queries: json!({"schema":"rustred.owner-domain-queries.json.v2","queries":[query("corner","111",[0,0,0],0,3,3)]}),
    };
    let run = run("inventory-fast-stop", &fixture, &[]);
    let options = OwnerDomainWalkInventoryOptions::new(run.dir.0.join("checkpoint"));
    let cancellation = AtomicBool::new(false);
    let stopped = owner_domain_walk_inventory(&run.request, &options, &cancellation, |event| {
        if event["event"] == "inventory_census_started" {
            cancellation.store(true, Ordering::Relaxed);
        }
    })
    .unwrap();
    assert_eq!(stopped.summary()["complete"], false);
    assert!(stopped.normalize_terminals(Default::default()).is_err());
    let mut changed = run.request.clone();
    changed.matching.queries_json = changed.matching.queries_json.replace("corner", "foreign");
    let invalid =
        owner_domain_walk_inventory(&changed, &options, &AtomicBool::new(false), |_| {}).unwrap();
    assert_eq!(invalid.summary()["complete"], false);
    assert_eq!(invalid.summary()["verification"]["verdict"], "FAIL");
    assert_eq!(
        invalid.summary()["verification"]["inventory_census"]["completed"],
        0
    );
}

#[test]
fn fast_cp6_rescue_trusts_required_scope_and_excludes_abandoned_helpers() {
    let mut fixture = bands();
    fixture.queries["query_roles"] =
        json!({"required":["anchor011"],"auxiliary":["anchor110","anchor","big","big-inner"]});
    let dir = scratch("inventory-fast-cp6-rescue");
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
    request.max_events = 1;
    let stopped = walk(&dir.0, &request);
    assert_eq!(stopped["stop_reason"], "event_allowance");
    request.max_events = 1_000_000;
    request.checkpoint.as_mut().unwrap().resume = true;
    let mut helper = fixture.queries["queries"][1].clone();
    helper["id"] = "replacement110".into();
    request.amendments.push(crate::OwnerDomainWalkAmendment {
        path: dir.0.join("amendment.json"),
        text: json!({"schema":super::super::super::rescue::AMENDMENT_SCHEMA,"sequence":1,
            "parent":checkpoint::epoch_request_binding(&request),"queries":[helper],
            "supersede":["anchor110","anchor","big","big-inner"]})
        .to_string(),
    });
    let result = walk(&dir.0, &request);
    assert!(result["abandoned_obligations"].as_u64().unwrap() > 0);
    let run = Run {
        dir,
        request,
        result,
    };
    let fast = compare(&run, true);
    assert_eq!(fast.summary()["complete"], true);
    let mut options = OwnerDomainWalkInventoryOptions::new(run.dir.0.join("checkpoint"));
    options.verification.certification_scope = OwnerDomainWalkVerifyScope::AllRoots;
    let all = owner_domain_walk_inventory(&run.request, &options, &AtomicBool::new(false), |_| {})
        .unwrap();
    assert_eq!(
        all.summary()["complete"],
        false,
        "abandoned helper roots must stay open"
    );
}
