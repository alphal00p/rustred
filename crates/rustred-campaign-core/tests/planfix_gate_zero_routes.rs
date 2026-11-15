//! planfix (audit r2-planners G1 / G2): the ship gate drops a first-route ZERO term only when an INDEPENDENT second route
//! confirms it (the closing loop's rule "zeros need a second route"; the route table of lane zerosfix3), and binds the
//! check it is given to its table set.
//!
//! Toy node (arity 3, lines 0 and 1, numerator slot 2; nothing five-loop specific, L8): point rules on the chain
//! [k,1,0] -> [k-1,1,0] + hand-down [k,0,0]; at the top key one more term [3,1,1] (numerator slot at a POSITIVE power:
//! never lower than the key) with the coefficient text `n2`, which vanishes on the rule point (n2 = 0).
//!
//! Cases: (a) rr_proof first-route zero + structural proof = the zeros route confirms: dropped, PASS; (b) exact-eval
//! first-route zero + the same proof: the zeros route is NOT independent of the exact evaluation (same algebra):
//! refused `all_zeros_unconfirmed`; (c) rr_proof zero confirmed by closeloop4's comparison (confirmed basis word): PASS
//! without any proof; (d) a structural proof ALONE (no first-route zero): one route, the term stays an edge: refused;
//! (e) G2: a plan names its table set; a plan of another (or no) table set is refused.

use rustred_campaign_core::closure::Tier;
use rustred_campaign_core::import::check::check;
use rustred_campaign_core::import::lines::term_key_digest;
use rustred_campaign_core::import::zero_proofs::{
    LoadOptions, VERIFY_SUMMARY_SCHEMA, VerifiedZeroProofs, ZERO_PROOF_SCHEMA, ZeroProofFacts,
    ZeroProofs, is_confirmed_basis,
};
use rustred_campaign_core::import::{CheckOptions, Evidence, ImportedSet, TableSetSpec, load};
use rustred_campaign_core::pieces::import_edges::{
    check_plan_table_set, gate_check, gate_check_with, table_set_digest,
};
use rustred_campaign_core::pieces::plan::{Candidate, PlanInput, ServiceKind, UnservedKey};
use rustred_campaign_core::pieces::{GateConfig, GateVerdict, NonZero, Term, plan};
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
    let d = std::env::temp_dir().join(format!("planfix-gz-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn rule_line(index: i64, k: i32, extra: bool) -> String {
    let mut rhs = vec![
        json!([format!("I({},1,0)", k - 1), "1"]),
        json!([format!("I({k},0,0)"), "1"]),
    ];
    if extra {
        rhs.push(json!([format!("I({k},1,1)"), "n2"]));
    }
    json!({"type":"rule","chart":"110","index":index,"fixed":[k,1,0],"equations":[],
        "target": format!("I({k},1,0)"), "rhs": rhs,
        "exceptions":[],"dispatch":"Partition","witness_loci":[]})
    .to_string()
}

fn lines() -> [String; 2] {
    [rule_line(0, 3, true), rule_line(1, 2, false)]
}

/// rr_proof instantiate fact (first route `rr_proof`) with the given coefficient words.
fn inst(line: &str, rule: i64, key: [i32; 3], terms: &[([i32; 3], &str)]) -> Value {
    let rhs: Vec<Value> = terms
        .iter()
        .enumerate()
        .map(|(j, (k, w))| json!({"term": j, "key": k, "coef": w}))
        .collect();
    json!({"schema":"rustred.proof-kernel.instantiate.v1","record_sha256": sha(line.as_bytes()),
        "rule": rule, "key": key, "in_case": true, "chart_ok": true, "branch": null, "regular": "regular",
        "irregular_terms": [], "hidden_loci": [], "rhs": rhs, "zero_test": "exact", "error": null})
}

/// Compact fact verdict line of producer `producer`: the listed terms non-zero, every other term ZERO.
fn compact(
    line: &str,
    index: i64,
    key: [i32; 3],
    n_terms: usize,
    nonzero: &[[i32; 3]],
    producer: &str,
) -> Value {
    json!({"schema":"rustred.campaign.fact-verdict.v1","index":index,"record_sha256": sha(line.as_bytes()),"key":key,
        "verdict":"regular","n_terms":n_terms,"producer":producer,
        "nonzero":nonzero.iter().map(|k| format!("{:016x}", term_key_digest(k))).collect::<Vec<_>>()})
}

/// The toy set, Tier C with the given fact lines (none: keys only).
fn material(dir: &Path, facts: &[Value]) -> (ImportedSet, Evidence) {
    let ls = lines();
    let mut t = format!("{HEADER}\n");
    t.push_str(r#"{"type":"chart","chart":"110","guards":["n>=1","n>=1","n<=0"],"zero":false}"#);
    t.push('\n');
    for l in &ls {
        t.push_str(l);
        t.push('\n');
    }
    t.push_str("{\"type\":\"end\",\"charts\":1}\n");
    let tp = dir.join("A.table.jsonl");
    std::fs::write(&tp, &t).unwrap();
    std::fs::write(dir.join("start.jsonl"), "[3,1,0]\n").unwrap();
    let mut spec = json!({"schema":"rustred.campaign.import-spec.v1","node":"toy","frame_from":"A","bits":6,
        "start":{"path": dir.join("start.jsonl").display().to_string(),"format":"keys_jsonl"},
        "required_evidence":"imported","policies":{"tier":"auto","witness_check":"off_diagnostic"},
        "sources":[{"id":"A","kind":"engine_table","class":"base","path": tp.display().to_string(),
                    "sha256": sha(t.as_bytes())}]});
    if !facts.is_empty() {
        let text: String = facts.iter().map(|f| format!("{f}\n")).collect();
        let p = dir.join("facts.jsonl");
        std::fs::write(&p, &text).unwrap();
        spec["evidence"] = json!({"facts":[{"path": p.display().to_string(),
            "sha256": sha(text.as_bytes()), "family": "TOY"}]});
    }
    let spec = TableSetSpec::from_json(&spec.to_string()).unwrap();
    let set = load(&spec, Path::new("/")).unwrap();
    let ev = Evidence::from_spec(&set, Path::new("/")).unwrap();
    (set, ev)
}

fn proof_file(dir: &Path) -> PathBuf {
    let proof = json!({"schema": ZERO_PROOF_SCHEMA, "record_sha256": sha(lines()[0].as_bytes()), "rule": 0,
        "term": 2, "locus": {"plane": [2, 0]}, "verdict": "zero_on_locus", "denominator": "nowhere_zero",
        "tool": "planfix-test@1"});
    let pp = dir.join("toy.zero-proofs.jsonl");
    std::fs::write(&pp, format!("{proof}\n")).unwrap();
    pp
}

/// A verify summary (as `rr_coef_facts verify-zero-proofs --summary` writes it) listing `files` by sha256; (path, pin).
fn summary(dir: &Path, files: &[&Path], verified: bool) -> (PathBuf, String) {
    let shas: Vec<String> = files
        .iter()
        .map(|f| sha(&std::fs::read(f).unwrap()))
        .collect();
    let v = json!({"schema": VERIFY_SUMMARY_SCHEMA, "verified": verified, "proofs_sha256": shas,
        "counts": {"mismatches": if verified { 0 } else { 1 }, "claims_without_record": 0}, "header_mismatch": []});
    let p = dir.join("verify.summary.json");
    let t = format!("{v}\n");
    std::fs::write(&p, &t).unwrap();
    (p, sha(t.as_bytes()))
}

/// The proofs of the toy, VERIFIED (audit r2-zeros F5: only a pinned verification gives facts).
fn proofs(dir: &Path, set: &ImportedSet) -> VerifiedZeroProofs {
    let pp = proof_file(dir);
    let (sp, pin) = summary(dir, &[&pp], true);
    let zp = VerifiedZeroProofs::load(set, &[pp], &sp, &pin, &LoadOptions::default()).unwrap();
    assert_eq!(zp.proofs.stats.proved, 1);
    zp
}

fn tier(t: Tier) -> CheckOptions {
    CheckOptions {
        threads: 1,
        tier: Some(t),
        max_rounds: None,
    }
}

fn rr_proof_facts() -> Vec<Value> {
    let ls = lines();
    vec![
        inst(
            &ls[0],
            0,
            [3, 1, 0],
            &[
                ([2, 1, 0], "nonzero"),
                ([3, 0, 0], "nonzero"),
                ([3, 1, 1], "zero"),
            ],
        ),
        inst(
            &ls[1],
            1,
            [2, 1, 0],
            &[([1, 1, 0], "nonzero"), ([2, 0, 0], "nonzero")],
        ),
    ]
}

#[test]
fn a_first_route_zero_is_dropped_only_when_an_independent_second_route_confirms_it() {
    // (a) rr_proof zero + structural proof (zeros route, independent of rr_proof): dropped, pass
    let d = tmp("a");
    let (set, ev) = material(&d, &rr_proof_facts());
    let res = check(&set, &ev, &tier(Tier::C)).unwrap();
    let zp = proofs(&d, &set);
    let zf = ZeroProofFacts::verified(&set, &zp, &ev);
    let (r, st) = gate_check_with(&set, &ev, &zf, &res, GateConfig::memory()).unwrap();
    assert_eq!(
        r.verdict,
        GateVerdict::Pass,
        "{:?} {:?}",
        r.verdict,
        r.reasons
    );
    assert_eq!(r.zero_terms_skipped, 1);
    assert_eq!(
        st.zero_terms.get("zeros_route"),
        Some(&1),
        "{:?}",
        st.zero_terms
    );
    assert!(r.reasons.is_empty());
    // the same evidence WITHOUT the proof: refused, typed
    let (r0, _) = gate_check(&set, &ev, &res, GateConfig::memory()).unwrap();
    assert_ne!(r0.verdict, GateVerdict::Pass);
    assert_eq!(r0.reasons, vec!["all_zeros_unconfirmed:1".to_string()]);

    // (b) exact-eval zero + the same proof: the zeros route runs on the same algebra (not independent): refused
    let d = tmp("b");
    let ls = lines();
    let facts = vec![
        compact(
            &ls[0],
            0,
            [3, 1, 0],
            3,
            &[[2, 1, 0], [3, 0, 0]],
            "exact_eval",
        ),
        compact(
            &ls[1],
            1,
            [2, 1, 0],
            2,
            &[[1, 1, 0], [2, 0, 0]],
            "exact_eval",
        ),
    ];
    let (set, ev) = material(&d, &facts);
    let res = check(&set, &ev, &tier(Tier::C)).unwrap();
    let zp = proofs(&d, &set);
    let zf = ZeroProofFacts::verified(&set, &zp, &ev);
    let (r, st) = gate_check_with(&set, &ev, &zf, &res, GateConfig::memory()).unwrap();
    assert_ne!(r.verdict, GateVerdict::Pass, "{:?}", r.verdict);
    assert_eq!(r.reasons, vec!["all_zeros_unconfirmed:1".to_string()]);
    assert_eq!(
        st.zero_terms.get("second_route_unconfirmed"),
        Some(&1),
        "{:?}",
        st.zero_terms
    );

    // (c) rr_proof zero confirmed by closeloop4's term-by-term comparison (confirmed basis): pass without a proof
    let d = tmp("c");
    let (set, mut ev) = material(&d, &rr_proof_facts());
    assert!(is_confirmed_basis("rr_proof + exact_eval"));
    assert!(
        !is_confirmed_basis("rr_proof + rr_proof_elided"),
        "two dependent producers confirm nothing"
    );
    assert!(ev.confirm_basis(
        &sha(lines()[0].as_bytes()),
        &[3, 1, 0],
        "rr_proof + exact_eval"
    ));
    let res = check(&set, &ev, &tier(Tier::C)).unwrap();
    let (r, st) = gate_check(&set, &ev, &res, GateConfig::memory()).unwrap();
    assert_eq!(
        r.verdict,
        GateVerdict::Pass,
        "{:?} {:?}",
        r.verdict,
        r.reasons
    );
    assert_eq!(
        st.zero_terms.get("confirmed_basis"),
        Some(&1),
        "{:?}",
        st.zero_terms
    );
    for d in ["a", "b", "c"] {
        let _ = std::fs::remove_dir_all(tmp(d));
    }
}

#[test]
fn a_structural_proof_alone_is_one_route_and_drops_nothing() {
    // (d) keys only (no first-route fact): the proof turns the coefficient into Zero for ZeroProofFacts' readers, but
    // the gate keeps the term as an edge: non-descending, refused (by descent, not as an unconfirmed zero)
    let d = tmp("d");
    let (set, ev) = material(&d, &[]);
    let res = check(&set, &ev, &tier(Tier::K)).unwrap();
    let zp = proofs(&d, &set);
    let zf = ZeroProofFacts::verified(&set, &zp, &ev);
    let (r, st) = gate_check_with(&set, &ev, &zf, &res, GateConfig::memory()).unwrap();
    assert_eq!(r.zero_terms_skipped, 0);
    assert!(st.zero_terms.is_empty(), "{:?}", st.zero_terms);
    assert_ne!(r.verdict, GateVerdict::Pass);
    assert_eq!(r.reasons, vec!["non_descending:1".to_string()]);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn a_plan_is_gated_only_against_the_table_set_it_names() {
    // (e) G2, planner side: the input carries the table-set digest, the plan echoes it, the check refuses another
    let d = tmp("e");
    let (set, _) = material(&d, &[]);
    let digest = table_set_digest(&set);
    assert!(
        digest.starts_with("sha256:") && digest.len() == 71,
        "{digest}"
    );
    let cmp = rustred_campaign_core::order::NodeComparator::legacy(3, None).unwrap();
    let keys = vec![UnservedKey {
        key: vec![3, 1, 0],
        face: "f".into(),
        candidates: vec![Candidate {
            kind: ServiceKind::TableRule,
            terms: vec![Term::new(vec![2, 1, 0], NonZero::Yes)],
            provenance: "t".into(),
            regular_at_4: None,
        }],
    }];
    let bound = plan(
        &cmp,
        &PlanInput::new(3, keys.clone()).with_table_set(digest.clone()),
    )
    .unwrap();
    assert_eq!(bound.table_set_digest.as_deref(), Some(digest.as_str()));
    assert!(check_plan_table_set(&bound, &set).is_ok());
    let other = plan(
        &cmp,
        &PlanInput::new(3, keys.clone()).with_table_set("sha256:other"),
    )
    .unwrap();
    let e = check_plan_table_set(&other, &set).unwrap_err();
    assert!(e.starts_with("plan_of_another_table_set:"), "{e}");
    let none = plan(&cmp, &PlanInput::new(3, keys)).unwrap();
    let e = check_plan_table_set(&none, &set).unwrap_err();
    assert!(e.starts_with("plan_table_set_unstated:"), "{e}");
    let _ = std::fs::remove_dir_all(&d);
}

/// planfix (audit r2-zeros F5): an UNVERIFIED proof file gives no fact (the same proof that confirms when its
/// verification is pinned); a verification is accepted only when its summary is pinned, clean, and lists the file.
#[test]
fn an_unverified_proof_file_gives_no_fact_and_a_verification_must_be_pinned_and_list_the_file() {
    let d = tmp("f5");
    let (set, ev) = material(&d, &rr_proof_facts());
    let res = check(&set, &ev, &tier(Tier::C)).unwrap();
    let pp = proof_file(&d);
    let raw = ZeroProofs::load(&set, std::slice::from_ref(&pp), &LoadOptions::default()).unwrap();
    let un = ZeroProofFacts::new(&set, &raw, &ev);
    assert!(!un.is_verified());
    let (r, st) = gate_check_with(&set, &ev, &un, &res, GateConfig::memory()).unwrap();
    assert_ne!(
        r.verdict,
        GateVerdict::Pass,
        "an unverified proof confirmed a zero"
    );
    assert_eq!(r.reasons, vec!["all_zeros_unconfirmed:1".to_string()]);
    assert_eq!(st.zero_terms.get("second_route_unconfirmed"), Some(&1));
    // pinned + clean + listed: the proof confirms
    let (sp, pin) = summary(&d, &[&pp], true);
    let v = VerifiedZeroProofs::load(
        &set,
        std::slice::from_ref(&pp),
        &sp,
        &pin,
        &LoadOptions::default(),
    )
    .unwrap();
    assert_eq!(v.files.len(), 1);
    let (r, _) = gate_check_with(
        &set,
        &ev,
        &ZeroProofFacts::verified(&set, &v, &ev),
        &res,
        GateConfig::memory(),
    )
    .unwrap();
    assert_eq!(r.verdict, GateVerdict::Pass);
    // refusals (typed): another pin, a summary that is not clean, a file the summary does not list
    let load = |sp: &Path, pin: &str| {
        VerifiedZeroProofs::load(
            &set,
            std::slice::from_ref(&pp),
            sp,
            pin,
            &LoadOptions::default(),
        )
        .err()
        .unwrap_or_default()
    };
    assert!(load(&sp, &"0".repeat(64)).starts_with("zero_proofs_unverified:"));
    let (sp2, pin2) = summary(&d, &[&pp], false);
    assert!(
        load(&sp2, &pin2).contains("clean verification"),
        "{}",
        load(&sp2, &pin2)
    );
    let other = d.join("other.jsonl");
    std::fs::write(&other, "{}\n").unwrap();
    let (sp3, pin3) = summary(&d, &[&other], true);
    assert!(
        load(&sp3, &pin3).contains("is not a file the pinned summary verified"),
        "{}",
        load(&sp3, &pin3)
    );
    let _ = std::fs::remove_dir_all(&d);
}
