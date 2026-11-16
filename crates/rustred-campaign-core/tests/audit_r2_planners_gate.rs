//! AUDIT (audit-r2-planners, notes/implementer.audit-r2-planners.md): the SHIP GATE of lane pieces
//! (`pieces::import_edges::gate_check` / `gate_check_with`, wire-in B2) and the PIECE PLANNER, tried against the claim
//! "a planner can make the result slower or larger, never wrong: everything it proposes passes the same accept checks".
//!
//! These tests ASSERT THE BEHAVIOUR FOUND (they pass on the audited code). A test whose name ends in `_defect`
//! documents a finding: the fixer INVERTS its marked assertion (`DEFECT:` comment) together with the fix.
//!
//! Toy node (arity 3, lines 0 and 1, numerator slot 2): point rules on the chain [k,1,0] -> [k-1,1,0] + [k,0,0]; at the
//! top key one more term [3,1,1] (numerator slot at a POSITIVE power, outside the chart: never lower than the key under
//! the node order). Nothing five-loop specific (L8).

use rustred_campaign_core::closure::Tier;
use rustred_campaign_core::import::check::{check, check_streamed};
use rustred_campaign_core::import::zero_proofs::{LoadOptions, ZeroProofFacts, ZeroProofs};
use rustred_campaign_core::import::{CheckOptions, Evidence, ImportedSet, TableSetSpec, load};
use rustred_campaign_core::pieces::import_edges::{gate_check, gate_check_with};
use rustred_campaign_core::pieces::plan::{Candidate, PlanInput, ServiceKind, UnservedKey};
use rustred_campaign_core::pieces::{GateConfig, GateVerdict, NonZero, ShipGate, Term, plan};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

const HEADER: &str =
    r#"{"type":"header","schema":"table_dump.v1","family":"TOY","denominators":3,"mask":"110"}"#;

fn sha(b: &[u8]) -> String {
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    h.update(b);
    h.finish_hex()
}

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("audit-r2-gate-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A rule line at [k,1,0]; `extra` = the extra term at [k,1,1] with coefficient text `coef`.
fn rule_line(index: i64, k: i32, extra: Option<&str>) -> String {
    let mut rhs = vec![
        json!([format!("I({},1,0)", k - 1), "1"]),
        json!([format!("I({k},0,0)"), "1"]),
    ];
    if let Some(c) = extra {
        rhs.push(json!([format!("I({k},1,1)"), c]));
    }
    json!({"type":"rule","chart":"110","index":index,"fixed":[k,1,0],"equations":[],
        "target": format!("I({k},1,0)"), "rhs": rhs,
        "exceptions":[],"dispatch":"Partition","witness_loci":[]})
    .to_string()
}

fn table_text(lines: &[String]) -> String {
    let mut t = format!("{HEADER}\n");
    t.push_str(r#"{"type":"chart","chart":"110","guards":["n>=1","n>=1","n<=0"],"zero":false}"#);
    t.push('\n');
    for l in lines {
        t.push_str(l);
        t.push('\n');
    }
    t.push_str("{\"type\":\"end\",\"charts\":1}\n");
    t
}

/// An instantiate fact line (first route) of a rule at its fixed key, with the given coefficient words.
fn fact_line(line: &str, rule: i64, key: [i32; 3], terms: &[([i32; 3], &str)]) -> String {
    let rhs: Vec<Value> = terms
        .iter()
        .enumerate()
        .map(|(j, (k, w))| json!({"term": j, "key": k, "coef": w}))
        .collect();
    json!({"schema":"rustred.proof-kernel.instantiate.v1","record_sha256": sha(line.as_bytes()),
        "rule": rule, "key": key, "in_case": true, "chart_ok": true, "branch": null, "regular": "regular",
        "irregular_terms": [], "hidden_loci": [], "rhs": rhs, "zero_test": "exact", "error": null})
    .to_string()
}

/// Load a spec from (id, table text) sources in order, start [3,1,0], optional facts file.
fn material(
    dir: &Path,
    sources: &[(&str, &str, String)],
    facts: Option<&str>,
    tier: &str,
) -> (ImportedSet, Evidence) {
    std::fs::write(dir.join("start.jsonl"), "[3,1,0]\n").unwrap();
    let mut srcs = vec![];
    for (id, class, t) in sources {
        let p = dir.join(format!("{id}.table.jsonl"));
        std::fs::write(&p, t).unwrap();
        srcs.push(json!({"id": id, "kind": "engine_table", "class": class, "path": p.display().to_string(),
            "sha256": sha(t.as_bytes())}));
    }
    let mut spec = json!({"schema":"rustred.campaign.import-spec.v1","node":"toy","frame_from": sources[0].0,
        "bits":6, "start":{"path": dir.join("start.jsonl").display().to_string(),"format":"keys_jsonl"},
        "required_evidence":"imported","policies":{"tier": tier, "witness_check":"off_diagnostic"},
        "sources": srcs});
    if let Some(f) = facts {
        let p = dir.join("facts.jsonl");
        std::fs::write(&p, f).unwrap();
        spec["evidence"] = json!({"facts":[{"path": p.display().to_string(),
            "sha256": sha(f.as_bytes()), "family": "TOY"}]});
    }
    let spec = TableSetSpec::from_json(&spec.to_string()).unwrap();
    let set = load(&spec, Path::new("/")).unwrap();
    let ev = Evidence::from_spec(&set, Path::new("/")).unwrap();
    (set, ev)
}

fn opts(tier: Tier) -> CheckOptions {
    CheckOptions {
        threads: 1,
        tier: Some(tier),
        max_rounds: None,
    }
}

/// FINDING G1 (MAJOR; brief 2c, wire-in B2): the ship gate drops a term on the FIRST route's zero alone. The term
/// [3,1,1] has the coefficient text `1` (non-zero in truth) and is NOT lower than the key; one first-route instantiate
/// fact claims it "zero" and nothing confirms that (no structural zero proof, no second route). Both entries pass:
/// `gate_check` (the evidence as fact source) and `gate_check_with(ZeroProofFacts)` (B2's wiring: it returns the inner
/// fact before it ever looks at the proofs, `import/zero_proofs.rs:489-491`). The closing loop's own gate would block
/// the same zero (`all_zeros_unconfirmed`, rule "zeros need a second route"); the ship gate is the weaker one.
#[test]
fn an_unconfirmed_first_route_zero_on_a_non_descending_term_passes_the_ship_gate_defect_fixed() {
    let dir = tmp("g1");
    let l0 = rule_line(0, 3, Some("1"));
    let l1 = rule_line(1, 2, None);
    let facts = [
        fact_line(
            &l0,
            0,
            [3, 1, 0],
            &[
                ([2, 1, 0], "nonzero"),
                ([3, 0, 0], "nonzero"),
                ([3, 1, 1], "zero"),
            ],
        ),
        fact_line(
            &l1,
            1,
            [2, 1, 0],
            &[([1, 1, 0], "nonzero"), ([2, 0, 0], "nonzero")],
        ),
    ]
    .join("\n")
        + "\n";
    let (set, ev) = material(
        &dir,
        &[("A", "base", table_text(&[l0.clone(), l1]))],
        Some(&facts),
        "C",
    );
    let res = check(&set, &ev, &opts(Tier::C)).unwrap();
    // without the zero claim the term IS a non-descending edge (sanity: the order really refuses it)
    let mut g = ShipGate::new(&set.cmp, 3, GateConfig::memory()).unwrap();
    g.push_relation(&[3, 1, 0], "sanity", [(&[3, 1, 1][..], NonZero::Unknown)])
        .unwrap();
    assert_ne!(g.finish().unwrap().verdict, GateVerdict::Pass);

    let (r0, s0) = gate_check(&set, &ev, &res, GateConfig::memory()).unwrap();
    // FIXED (planfix G1; was DEFECT): the unconfirmed (first route only) zero stays an edge; the gate refuses with the
    // closing loop's word
    assert_ne!(r0.verdict, GateVerdict::Pass, "{:?}", r0.verdict);
    assert_eq!(r0.zero_terms_skipped, 0);
    assert_eq!(r0.zero_unconfirmed_non_descending, 1);
    assert!(
        r0.reasons.iter().any(|r| r == "all_zeros_unconfirmed:1"),
        "{:?}",
        r0.reasons
    );
    assert_eq!(
        s0.zero_terms.get("second_route_unconfirmed"),
        Some(&1),
        "{:?}",
        s0.zero_terms
    );

    let zp = ZeroProofs::load(&set, &[], &LoadOptions::default()).unwrap();
    assert_eq!(zp.stats.proved, 0, "no structural proof at all");
    let zf = ZeroProofFacts::new(&set, &zp, &ev);
    let (r1, _) = gate_check_with(&set, &ev, &zf, &res, GateConfig::memory()).unwrap();
    // FIXED (planfix G1; was DEFECT): B2's fact source refuses the same unconfirmed zero (no proof = no second route)
    assert_ne!(r1.verdict, GateVerdict::Pass, "{:?}", r1.verdict);
    assert_eq!(r1.zero_terms_skipped, 0);
    assert!(
        r1.reasons.iter().any(|r| r == "all_zeros_unconfirmed:1"),
        "{:?}",
        r1.reasons
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// FINDING G2 (MAJOR; brief 2d): the gate does not bind the check it is given to the table set it is given. A check
/// of spec A (one table) is gated against spec B = [X, A] where X's rule wins at the top key and carries a
/// non-descending term: the gate PASSES (it reads A's rows and finds A's record in B by sha256), while the check of B
/// itself FAILS. `res.spec_digest != set.report.spec_digest` is not looked at (`pieces/import_edges.rs:294-314`).
#[test]
fn a_gate_run_on_another_table_set_than_its_check_passes_what_that_set_ships_defect_fixed() {
    let dir = tmp("g2");
    let la0 = rule_line(0, 3, None);
    let la1 = rule_line(1, 2, None);
    let ta = table_text(&[la0, la1]);
    let (set_a, ev_a) = material(&dir.join(""), &[("A", "base", ta.clone())], None, "K");
    let rows_a = dir.join("a.rows.jsonl");
    let res_a = check_streamed(&set_a, &ev_a, &opts(Tier::K), &rows_a).unwrap();
    // B: table X FIRST (dispatch order), its rule at [3,1,0] has the non-descending term [3,1,1]
    let lx = rule_line(0, 3, Some("1"));
    let tx = table_text(&[lx]);
    let db = dir.join("b");
    std::fs::create_dir_all(&db).unwrap();
    let (set_b, ev_b) = material(&db, &[("X", "repair", tx), ("A", "base", ta)], None, "K");
    let res_b = check_streamed(&set_b, &ev_b, &opts(Tier::K), &db.join("b.rows.jsonl")).unwrap();
    let (rb, _) = gate_check(&set_b, &ev_b, &res_b, GateConfig::memory()).unwrap();
    assert_ne!(
        rb.verdict,
        GateVerdict::Pass,
        "B's own check fails the gate"
    );
    assert_ne!(res_a.spec_digest, set_b.report.spec_digest);
    // FIXED (planfix G2; was DEFECT): the gate refuses a check of another spec / table set (typed word)
    match gate_check(&set_b, &ev_b, &res_a, GateConfig::memory()) {
        Err(e) => assert!(e.starts_with("check_of_another_table_set:"), "{e}"),
        Ok((ra, _)) => panic!("gated another table set's check: {:?}", ra.verdict),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// FINDING G3 (MINOR; brief 2b): a hand-down whose arrival key is NOT in the planner's input is labelled `served`
/// (`pieces/plan.rs:504`, `None => HandDownTarget::Served`) whatever its state on the receiving side, and the ship gate
/// (descent + acyclicity only) passes the hand-down edge into a key that nothing serves. Neither refuses: the closure
/// walk of the closing loop is what keeps it open. The plan / gate reports are not closure statements.
#[test]
fn a_hand_down_into_an_unserved_key_outside_the_input_is_called_outside_input_and_passes_the_gate()
{
    let cmp = rustred_campaign_core::order::NodeComparator::legacy(3, None).unwrap();
    let k = vec![3, 1, 0];
    let t = vec![2, 1, 0]; // lower; NOT in the input; nothing serves it anywhere
    let inp = PlanInput::new(
        3,
        vec![UnservedKey {
            key: k.clone(),
            face: "f".into(),
            candidates: vec![Candidate {
                kind: ServiceKind::HandDown,
                terms: vec![Term::new(t.clone(), NonZero::Yes)],
                provenance: "hd".into(),
                regular_at_4: None,
            }],
        }],
    );
    let p = plan(&cmp, &inp).unwrap();
    let v = serde_json::to_value(&p).unwrap();
    let s = v.to_string();
    // FIXED (planfix G3): the target outside the input is `outside_input` (unknown to the planner), never "served"
    assert!(
        !s.contains("\"served\""),
        "hand-down target labelled served: {s}"
    );
    assert!(s.contains("\"outside_input\""), "{s}");
    assert!(
        p.core_open.is_empty(),
        "no key left to the core: {:?}",
        p.core_open
    );
    let mut g = ShipGate::new(&cmp, 3, GateConfig::memory()).unwrap();
    g.push_relation(&k, "hd", [(&t[..], NonZero::Yes)]).unwrap();
    assert_eq!(g.finish().unwrap().verdict, GateVerdict::Pass);
}

/// Per-piece gates are not the union gate: two coupled pieces (each acyclic) form a cycle only together. With the
/// FULL comparator one of the edges is non-descending and each per-piece gate already fails; the union gate fails too
/// and names the cycle. (Lane pieces' own tests cover the broken-order case, `Inconsistent`.)
#[test]
fn coupled_pieces_with_a_cycle_only_in_the_union_are_refused() {
    let cmp = rustred_campaign_core::order::NodeComparator::legacy(3, None).unwrap();
    let a = vec![3, 1, 0];
    let b = vec![2, 1, 0];
    let mut union = ShipGate::new(&cmp, 3, GateConfig::memory()).unwrap();
    union
        .push_relation(&a, "piece1", [(&b[..], NonZero::Yes)])
        .unwrap();
    union
        .push_relation(&b, "piece2", [(&a[..], NonZero::Yes)])
        .unwrap();
    let r = union.finish().unwrap();
    assert!(
        matches!(r.verdict, GateVerdict::Fail { cyclic: true, .. }),
        "{:?}",
        r.verdict
    );
    let mut p2 = ShipGate::new(&cmp, 3, GateConfig::memory()).unwrap();
    p2.push_relation(&b, "piece2", [(&a[..], NonZero::Yes)])
        .unwrap();
    let r2 = p2.finish().unwrap();
    assert!(
        matches!(r2.verdict, GateVerdict::Fail { cyclic: false, .. }),
        "the per-piece gate of piece 2 fails by descent alone: {:?}",
        r2.verdict
    );
}
