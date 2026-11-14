//! Lane audit-closeloop (INDEPENDENT AUDIT of the closing loop's core dependencies): constructed toy nodes (arity 3,
//! mask 110; nothing five-loop specific) attacking what the closing loop's `closed` verdict rests on: the comparison
//! with a hand statement after a STREAMED (exact-tier) check, the binding of per-key proofs / solved relations to
//! (record_sha256 | source, key), the zero-term rule (box clause, per key not per rule), and an edge finder over real
//! material (ignored, env-driven) used to re-evaluate the coefficients of every edge into the keys the exact tier drops.
//!
//! Tests that assert the CORRECT behaviour and FAIL on the audited code (0ceb9e79) are `#[ignore]`d with the finding
//! id in their name (notes/implementer.audit-closeloop.md); the others pass and pin down a sound behaviour.

use rustred_campaign_core::closure::Tier;
use rustred_campaign_core::import::lines::{key_text, term_key_digest};
use rustred_campaign_core::import::{
    self, CheckOptions, DiffKind, Evidence, HandFormat, Level, RowStatus, TableSetSpec, check,
    compare, load, read_hand_statement,
};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

const HEADER: &str =
    r#"{"type":"header","schema":"table_dump.v1","family":"TOY","denominators":3,"mask":"110"}"#;

fn scratch(tag: &str) -> PathBuf {
    let base = std::env::var_os("TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let d = base.join(format!("audit-cl-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn sha_text(s: &str) -> String {
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    h.update(s.as_bytes());
    h.finish_hex()
}

fn file_sha(p: &Path) -> String {
    import::lines::file_sha256(p).unwrap()
}

/// A rule line over the toy node with explicit (term, coefficient) pairs and COMPLETE (empty) witness loci.
fn rule(index: i64, fixed: [Option<i64>; 3], rhs: &[(&str, &str)]) -> String {
    let t: Vec<String> = (0..3)
        .map(|i| fixed[i].map(|v| v.to_string()).unwrap_or(format!("n{i}")))
        .collect();
    json!({"type":"rule","chart":"110","index":index,"fixed":fixed,"equations":[],
        "target": format!("I({})", t.join(",")),
        "rhs": rhs.iter().map(|(k, c)| json!([k, c])).collect::<Vec<_>>(),
        "exceptions":[],"dispatch":"Partition","witness_loci":[]})
    .to_string()
}

fn write_table_h(dir: &Path, name: &str, header: &str, rules: &[String]) -> PathBuf {
    let p = dir.join(name);
    let mut s = String::from(header);
    s.push('\n');
    s.push_str(r#"{"type":"chart","chart":"110","guards":["n>=1","n>=1","n<=0"],"zero":false}"#);
    s.push('\n');
    for r in rules {
        s.push_str(r);
        s.push('\n');
    }
    s.push_str(r#"{"type":"end","charts":1}"#);
    s.push('\n');
    std::fs::write(&p, s).unwrap();
    p
}

fn write_lines(p: &Path, lines: &[Value]) -> PathBuf {
    std::fs::write(p, lines.iter().map(|l| format!("{l}\n")).collect::<String>()).unwrap();
    p.to_path_buf()
}

fn spec_of(dir: &Path, table: &Path, start: &[[i32; 3]], evidence: Value, required: &str) -> Value {
    let st = write_lines(
        &dir.join("start.jsonl"),
        &start.iter().map(|k| json!(k)).collect::<Vec<_>>(),
    );
    json!({"schema":"rustred.campaign.import-spec.v1","node":"toy","frame_from":"A","bits":6,
        "start":{"path": st.display().to_string(),"format":"keys_jsonl"},
        "required_evidence": required, "policies": {"tier": "C"},
        "sources":[{"id":"A","kind":"engine_table","class":"base","path":table.display().to_string(),
                    "sha256": file_sha(table), "hand_name":{"template":"{source}#{label}"}}],
        "evidence": evidence})
}

fn load_ev(spec: &Value, dir: &Path) -> (import::ImportedSet, Evidence) {
    let s = TableSetSpec::from_json(&spec.to_string()).unwrap();
    let set = load(&s, dir).unwrap();
    let ev = Evidence::from_spec(&set, dir).unwrap();
    (set, ev)
}

fn tier_c() -> CheckOptions {
    CheckOptions {
        threads: 2,
        tier: Some(Tier::C),
        max_rounds: None,
    }
}

fn hex(k: [i32; 3]) -> String {
    format!("{:016x}", term_key_digest(&k))
}

/// A compact fact verdict line (import.md 5.5): regular, the listed terms non-zero (every other term ZERO).
fn fact(sha: &str, key: [i32; 3], n_terms: usize, nonzero: &[[i32; 3]]) -> Value {
    json!({"schema":"rustred.campaign.fact-verdict.v1","index":0,"record_sha256":sha,"key":key,
        "verdict":"regular","n_terms":n_terms,"nonzero":nonzero.iter().map(|k| hex(*k)).collect::<Vec<_>>()})
}

fn proof(sha: &str, key: [i32; 3], verdict: &str) -> Value {
    json!({"record_sha256": sha, "index": 0, "key": key, "verdict": verdict})
}

/// The descent toy: rule 0 on the face n1 = 1, n2 = 0: I(n0) -> I(n0-1) + a hand-down; start I(3,1,0); closure
/// I(3,1,0), I(2,1,0), I(1,1,0); every key with a regularity fact (all terms non-zero) and an exact per-key proof.
fn descent_toy(d: &Path) -> (PathBuf, String, Value) {
    let r0 = rule(
        0,
        [None, Some(1), Some(0)],
        &[("I(n0-1,1,0)", "(d-4)/(d-3)"), ("I(n0,0,0)", "1")],
    );
    let sha = sha_text(&r0);
    let a = write_table_h(d, "A.table.jsonl", HEADER, &[r0]);
    let keys = [[3, 1, 0], [2, 1, 0], [1, 1, 0]];
    let facts: Vec<Value> = keys
        .iter()
        .map(|k| fact(&sha, *k, 2, &[[k[0] - 1, 1, 0], [k[0], 0, 0]]))
        .collect();
    let fp = write_lines(&d.join("facts.jsonl"), &facts);
    let proofs: Vec<Value> = keys
        .iter()
        .map(|k| proof(&sha, *k, "EXACT_IN_SPAN_AT_KEY"))
        .collect();
    let pp = write_lines(&d.join("proofs.jsonl"), &proofs);
    let ev = json!({"facts":[{"path": fp.display().to_string()}],"proofs":[{"path": pp.display().to_string()}]});
    (a, sha, ev)
}

fn hand_file(d: &Path, rows: &[(Vec<i32>, bool, &str, &str)]) -> (PathBuf, HandFormat) {
    let p = d.join("hand.jsonl");
    let lines: Vec<Value> = rows
        .iter()
        .map(|(k, b, label, rule)| json!([key_text(k), b, label, rule, 0]))
        .collect();
    write_lines(&p, &lines);
    let mut f = HandFormat::positional(0, 1, 2, 3);
    f.round = Some(import::spec::Col::Index(4));
    (p, f)
}

/// Sanity: the in-memory check of the descent toy closes at Tier C, per-key, and its comparison with a hand statement
/// that LACKS one key or names another server for one key reports both (the non-streamed path is right).
#[test]
fn in_memory_comparison_reports_a_missing_key_and_a_changed_server() {
    let d = scratch("cmp-mem");
    let (a, _sha, ev) = descent_toy(&d);
    let spec = spec_of(&d, &a, &[[3, 1, 0]], ev, "per-key");
    let (set, ev) = load_ev(&spec, &d);
    let r = check(&set, &ev, &tier_c()).unwrap();
    assert_eq!(r.rows.len(), 3);
    assert!(r.rows.iter().all(|x| x.status == RowStatus::Served
        && x.evidence == Some(Level::PerKey)
        && !x.regularity_unknown
        && !x.coefficient_unknown));
    assert!(r.blocking().is_empty(), "{:?}", r.blocking());
    // hand: I(1,1,0) missing, I(2,1,0) served by another record
    let (hp, f) = hand_file(
        &d,
        &[
            (vec![3, 1, 0], true, "base", "A#0"),
            (vec![2, 1, 0], false, "base", "A#7"),
        ],
    );
    let c = compare(&set, &r, &read_hand_statement(&hp, &f, 3).unwrap());
    assert_eq!(c.count(DiffKind::OnlyOurs), 1);
    assert_eq!(c.count(DiffKind::ServerDiffers), 1);
    assert!(!c.key_sets_equal);
}

/// FINDING F-CMP (major, import/statement.rs `compare` with a STREAMED check, which is what the closing loop's exact
/// tier hands it): plain rule rows are not in `res.rows`, so (a) a key of OUR closure missing from the hand statement
/// is never reported `only_ours` and `key_sets_equal` stays TRUE, and (b) a hand row naming ANOTHER server for a plain
/// key is counted "(streamed rule row)" and never compared: two different statements compare equal.
#[test]
#[ignore = "F-CMP: fails on 0ceb9e79 (streamed compare skips plain rows)"]
fn f_cmp_streamed_comparison_must_report_a_missing_key_and_a_changed_server() {
    let d = scratch("cmp-stream");
    let (a, _sha, ev) = descent_toy(&d);
    let spec = spec_of(&d, &a, &[[3, 1, 0]], ev, "per-key");
    let (set, ev) = load_ev(&spec, &d);
    let r = import::check::check_streamed(&set, &ev, &tier_c(), &d.join("rows.jsonl")).unwrap();
    assert_eq!(r.streamed.as_ref().unwrap().rows, 3);
    assert!(r.rows.is_empty(), "every row is a plain rule row");
    let (hp, f) = hand_file(
        &d,
        &[
            (vec![3, 1, 0], true, "base", "A#0"),
            (vec![2, 1, 0], false, "base", "A#7"),
        ],
    );
    let c = compare(&set, &r, &read_hand_statement(&hp, &f, 3).unwrap());
    eprintln!(
        "streamed compare: key_sets_equal {} same_server {} differences {:?} keys ours {} hand {}",
        c.key_sets_equal, c.same_server, c.counts, c.keys_ours, c.keys_hand
    );
    assert!(
        !c.key_sets_equal,
        "our closure has I(1,1,0), the hand statement does not"
    );
    assert_eq!(c.count(DiffKind::ServerDiffers), 1, "A#0 vs A#7 at I(2,1,0)");
}

/// The same streamed comparison, demonstrated as a PASSING test of the current behaviour (so the defect is visible in
/// a normal test run): key sets reported equal although 3 != 2 keys, and no server difference.
#[test]
fn f_cmp_streamed_comparison_currently_reports_equal_sets() {
    let d = scratch("cmp-stream-now");
    let (a, _sha, ev) = descent_toy(&d);
    let spec = spec_of(&d, &a, &[[3, 1, 0]], ev, "per-key");
    let (set, ev) = load_ev(&spec, &d);
    let r = import::check::check_streamed(&set, &ev, &tier_c(), &d.join("rows.jsonl")).unwrap();
    let (hp, f) = hand_file(
        &d,
        &[
            (vec![3, 1, 0], true, "base", "A#0"),
            (vec![2, 1, 0], false, "base", "A#7"),
        ],
    );
    let c = compare(&set, &r, &read_hand_statement(&hp, &f, 3).unwrap());
    assert!(c.key_sets_equal, "defect F-CMP reproduced: {:?}", c.counts);
    assert_eq!(c.count(DiffKind::ServerDiffers), 0);
    assert_eq!((c.keys_ours, c.keys_hand), (3, 2));
}

/// FINDING F-PRF (minor-major, import/evidence.rs `put_proof`): two answers for the SAME (record, key) that contradict
/// each other (EXACT_IN_SPAN_AT_KEY and IRREGULAR_KEY, a pole at the key) are not a conflict: the per-key one wins in
/// either order and the key is certified per-key AND regular (facts conflicts are an error; proofs are not).
#[test]
#[ignore = "F-PRF: fails on 0ceb9e79 (contradicting proof verdicts resolved to the positive one)"]
fn f_prf_contradicting_proof_verdicts_for_one_record_key_must_fail_closed() {
    for order in [
        ["EXACT_IN_SPAN_AT_KEY", "IRREGULAR_KEY"],
        ["IRREGULAR_KEY", "EXACT_IN_SPAN_AT_KEY"],
    ] {
        let d = scratch("prf");
        let (a, sha, _) = descent_toy(&d);
        let pp = write_lines(
            &d.join("p2.jsonl"),
            &[
                proof(&sha, [3, 1, 0], order[0]),
                proof(&sha, [3, 1, 0], order[1]),
            ],
        );
        let ev = json!({"proofs":[{"path": pp.display().to_string()}]});
        let spec = spec_of(&d, &a, &[[3, 1, 0]], ev, "per-key");
        let s = TableSetSpec::from_json(&spec.to_string()).unwrap();
        let set = load(&s, &d).unwrap();
        match Evidence::from_spec(&set, &d) {
            Err(_) => {} // fail closed: fine
            Ok(ev) => {
                let r = check(&set, &ev, &tier_c()).unwrap();
                let row = r.rows.iter().find(|x| x.key == [3, 1, 0]).unwrap();
                assert!(
                    row.evidence != Some(Level::PerKey) || row.regularity_unknown,
                    "order {order:?}: contradicting proofs certified the key per-key and regular"
                );
            }
        }
    }
}

/// F-PRF as a passing test of the current behaviour.
#[test]
fn f_prf_contradicting_proof_verdicts_currently_certify() {
    let d = scratch("prf-now");
    let (a, sha, _) = descent_toy(&d);
    let pp = write_lines(
        &d.join("p2.jsonl"),
        &[
            proof(&sha, [3, 1, 0], "IRREGULAR_KEY"),
            proof(&sha, [3, 1, 0], "EXACT_IN_SPAN_AT_KEY"),
        ],
    );
    let ev = json!({"proofs":[{"path": pp.display().to_string()}]});
    let spec = spec_of(&d, &a, &[[3, 1, 0]], ev, "per-key");
    let (set, ev) = load_ev(&spec, &d);
    let r = check(&set, &ev, &tier_c()).unwrap();
    let row = r.rows.iter().find(|x| x.key == [3, 1, 0]).unwrap();
    assert_eq!(row.evidence, Some(Level::PerKey));
    assert!(!row.regularity_unknown, "regular through the per-key proof");
}

/// FINDING F-BIND (minor; genericity): evidence binds to (record_sha256, key) where record_sha256 is the sha of the
/// RECORD LINE ONLY, not of the table header (family, guard version): a per-key proof produced for a record of family
/// TOY certifies the byte-identical record of another family TOY2. A spec pointing at another table set's evidence
/// is not refused: foreign lines are ignored (sound), byte-identical lines bind (not sound in general).
#[test]
#[ignore = "F-BIND: fails on 0ceb9e79 (a proof of family TOY binds to the identical record of family TOY2)"]
fn f_bind_a_proof_for_another_family_must_not_certify_an_identical_record() {
    let d = scratch("bind");
    let (_a, sha, _) = descent_toy(&d); // family TOY: its proofs.jsonl
    let r0 = rule(
        0,
        [None, Some(1), Some(0)],
        &[("I(n0-1,1,0)", "(d-4)/(d-3)"), ("I(n0,0,0)", "1")],
    );
    assert_eq!(sha_text(&r0), sha);
    let other =
        r#"{"type":"header","schema":"table_dump.v1","family":"TOY2","denominators":3,"mask":"110"}"#;
    let b = write_table_h(&d, "B.table.jsonl", other, &[r0]);
    let ev = json!({"proofs":[{"path": d.join("proofs.jsonl").display().to_string()}],
                    "facts":[{"path": d.join("facts.jsonl").display().to_string()}]});
    let spec = spec_of(&d, &b, &[[3, 1, 0]], ev, "per-key");
    let (set, ev) = load_ev(&spec, &d);
    assert_eq!(ev.counts.proofs_foreign, 0, "every TOY proof line bound to TOY2");
    let r = check(&set, &ev, &tier_c()).unwrap();
    let row = r.rows.iter().find(|x| x.key == [3, 1, 0]).unwrap();
    assert_ne!(
        row.evidence,
        Some(Level::PerKey),
        "a proof of family TOY certified family TOY2's record"
    );
}

/// Evidence of a DIFFERENT table set (other record bytes) is never used: counted foreign, the key stays below
/// per-key (the conservative half of F-BIND).
#[test]
fn evidence_of_another_table_set_with_other_bytes_is_foreign_and_unused() {
    let d = scratch("foreign");
    let (_a, _sha, _) = descent_toy(&d);
    let r1 = rule(
        0,
        [None, Some(1), Some(0)],
        &[("I(n0-1,1,0)", "(d-5)/(d-3)"), ("I(n0,0,0)", "1")],
    );
    let b = write_table_h(&d, "B.table.jsonl", HEADER, &[r1]);
    let ev = json!({"proofs":[{"path": d.join("proofs.jsonl").display().to_string()}],
                    "facts":[{"path": d.join("facts.jsonl").display().to_string()}]});
    let spec = spec_of(&d, &b, &[[3, 1, 0]], ev, "per-key");
    let (set, ev) = load_ev(&spec, &d);
    assert_eq!((ev.counts.proofs_foreign, ev.counts.facts_foreign), (3, 3));
    let r = check(&set, &ev, &tier_c()).unwrap();
    assert!(
        r.rows
            .iter()
            .all(|x| x.evidence != Some(Level::PerKey) || x.status != RowStatus::Served)
    );
    assert!(!r.blocking().is_empty());
}

/// The BOX clause of the zero-term rule: a START key whose only in-edge has a zero coefficient is still in the
/// closure and must be served; without a rule it stays open (the node does not close).
#[test]
fn a_box_key_reached_only_through_a_zero_edge_must_still_be_served() {
    let d = scratch("boxzero");
    let r0 = rule(
        0,
        [Some(3), Some(1), Some(0)],
        &[("I(2,1,0)", "(n0-3)*(d-2)"), ("I(3,0,0)", "1")],
    );
    let sha = sha_text(&r0);
    let a = write_table_h(&d, "A.table.jsonl", HEADER, &[r0]);
    let fp = write_lines(
        &d.join("facts.jsonl"),
        &[fact(&sha, [3, 1, 0], 2, &[[3, 0, 0]])],
    );
    let pp = write_lines(
        &d.join("proofs.jsonl"),
        &[proof(&sha, [3, 1, 0], "EXACT_IN_SPAN_AT_KEY")],
    );
    let ev = json!({"facts":[{"path": fp.display().to_string()}],"proofs":[{"path": pp.display().to_string()}]});
    // only I(3,1,0) in the start set: I(2,1,0) is not reached (zero edge), the closure is one key and closes
    let spec = spec_of(&d, &a, &[[3, 1, 0]], ev.clone(), "per-key");
    let (set, e) = load_ev(&spec, &d);
    let r = check(&set, &e, &tier_c()).unwrap();
    assert_eq!(r.rows.len(), 1);
    assert!(r.blocking().is_empty(), "{:?}", r.blocking());
    // I(2,1,0) ALSO a box key: in the closure, open (no rule), the node does not close
    let spec = spec_of(&d, &a, &[[3, 1, 0], [2, 1, 0]], ev, "per-key");
    let (set, e) = load_ev(&spec, &d);
    let r = check(&set, &e, &tier_c()).unwrap();
    let row = r.rows.iter().find(|x| x.key == [2, 1, 0]).expect("box key in the closure");
    assert!(row.in_start);
    assert_eq!(row.status, RowStatus::Open);
    assert!(!r.blocking().is_empty());
}

/// The zero-term rule is PER KEY, not per rule: a coefficient zero at one key (n0 = 3) drops that edge only there;
/// at another key of the same record (no fact) the term is followed.
#[test]
fn a_zero_coefficient_at_one_key_does_not_drop_the_edge_at_another_key_of_the_rule() {
    let d = scratch("perkeyzero");
    let r0 = rule(
        0,
        [None, Some(1), Some(0)],
        &[("I(n0-1,1,0)", "(n0-3)"), ("I(n0,0,0)", "1")],
    );
    let sha = sha_text(&r0);
    let a = write_table_h(&d, "A.table.jsonl", HEADER, &[r0]);
    // a zero fact for I(2,1,0) at I(3,1,0) only; I(5,1,0) and I(4,1,0) carry non-zero facts
    let fp = write_lines(
        &d.join("facts.jsonl"),
        &[
            fact(&sha, [3, 1, 0], 2, &[[3, 0, 0]]),
            fact(&sha, [5, 1, 0], 2, &[[4, 1, 0], [5, 0, 0]]),
            fact(&sha, [4, 1, 0], 2, &[[3, 1, 0], [4, 0, 0]]),
        ],
    );
    let ev = json!({"facts":[{"path": fp.display().to_string()}]});
    let spec = spec_of(&d, &a, &[[5, 1, 0], [3, 1, 0]], ev, "imported");
    let (set, e) = load_ev(&spec, &d);
    let r = check(&set, &e, &tier_c()).unwrap();
    let keys: Vec<Vec<i32>> = r.rows.iter().map(|x| x.key.clone()).collect();
    assert!(!keys.contains(&vec![2, 1, 0]), "{keys:?}");
    assert!(keys.contains(&vec![4, 1, 0]));
    // without the zero fact at I(3,1,0), I(2,1,0) is followed (unknown coefficient = an edge)
    let fp2 = write_lines(
        &d.join("facts2.jsonl"),
        &[
            fact(&sha, [5, 1, 0], 2, &[[4, 1, 0], [5, 0, 0]]),
            fact(&sha, [4, 1, 0], 2, &[[3, 1, 0], [4, 0, 0]]),
        ],
    );
    let ev2 = json!({"facts":[{"path": fp2.display().to_string()}]});
    let spec2 = spec_of(&d, &a, &[[5, 1, 0], [3, 1, 0]], ev2, "imported");
    let (set2, e2) = load_ev(&spec2, &d);
    let r2 = check(&set2, &e2, &tier_c()).unwrap();
    assert!(r2.rows.iter().any(|x| x.key == [2, 1, 0]));
}

/// FINDING F-SOLV (minor, import/evidence.rs `load_solved`): two DIFFERENT solved relations for the same (source,
/// key) whose `relation_digest` is equal (here: both absent) are no conflict: the first one silently wins.
#[test]
#[ignore = "F-SOLV: fails on 0ceb9e79 (different rhs, both without relation_digest: first wins)"]
fn f_solv_two_different_solved_relations_without_digest_must_conflict() {
    let d = scratch("solv");
    let (a, _sha, _) = descent_toy(&d);
    let rf = d.join("R.recipe.json");
    std::fs::write(
        &rf,
        json!({"schema":"rustred.recipe.v1","id":"R","arity":3,"chart":"110",
            "domain":{"fixed":[null,2,0],"equations":[],"exceptions":[]},
            "seeds":{"unit":"harder","centres":[],"generator":{"shells":["S0"]}},
            "symmetry_rows":false,"elimination":{"convention":"rustred.recipe-elim.v1"}})
        .to_string(),
    )
    .unwrap();
    let line = |rhs: Value| {
        json!({"schema":"rustred.campaign.solved-relation.v1","source":"R","key":[2,2,0],"verdict":"SOLVED",
            "recipe_sha256": file_sha(&rf), "rhs": rhs, "proof":{"ok":true}, "regular":true})
    };
    let sp = write_lines(
        &d.join("solved.jsonl"),
        &[
            line(json!([{"key":[1,2,0]},{"key":[2,1,0]}])),
            line(json!([{"key":[1,2,0]}])),
        ],
    );
    let mut spec = spec_of(
        &d,
        &a,
        &[[2, 2, 0]],
        json!({"solved":[{"path": sp.display().to_string()}]}),
        "imported",
    );
    spec["sources"].as_array_mut().unwrap().push(json!({"id":"R","kind":"recipe","class":"recipe",
        "path": rf.display().to_string(), "sha256": file_sha(&rf)}));
    let s = TableSetSpec::from_json(&spec.to_string()).unwrap();
    let set = load(&s, &d).unwrap();
    assert!(
        Evidence::from_spec(&set, &d).is_err(),
        "two different relations for (R, I(2,2,0)) accepted"
    );
}

// ------------------------------------------------------------------------------------------------------------------
// Edge finder over real material (ignored; env-driven; read-only inputs). For every row of a WRITTEN statement served
// by a rule, the rule's terms are instantiated at the key; every term equal to one of the TARGET keys is written out
// with the record's table path, offset, rule index, the rule point and the term index, so that the coefficient can be
// re-evaluated by an independent route. Env: AUDIT_CL_SPEC (effective import spec), AUDIT_CL_BASE (its base),
// AUDIT_CL_STATEMENT (statement.jsonl), AUDIT_CL_TARGETS (one key array per line), AUDIT_CL_OUT (output JSONL).
// ------------------------------------------------------------------------------------------------------------------

#[test]
#[ignore = "heavy: real material, run by hand (see notes/implementer.audit-closeloop.md)"]
fn edges_into_target_keys_from_a_written_statement() {
    use std::collections::{HashMap, HashSet};
    use std::io::{BufRead, Write};
    let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("{k} not set"));
    let spec = TableSetSpec::read(Path::new(&env("AUDIT_CL_SPEC"))).unwrap();
    let base = PathBuf::from(env("AUDIT_CL_BASE"));
    let t0 = std::time::Instant::now();
    let set = load(&spec, &base).unwrap();
    eprintln!("loaded in {:.1} s", t0.elapsed().as_secs_f64());
    // AUDIT_CL_TARGETS=ALL: every key of the statement (in-degree mode: only targets with ONE structural in-edge
    // are written, with that edge)
    let all = env("AUDIT_CL_TARGETS") == "ALL";
    let targets: HashSet<Vec<i32>> = if all {
        std::io::BufReader::new(std::fs::File::open(env("AUDIT_CL_STATEMENT")).unwrap())
            .lines()
            .map(|l| serde_json::from_value(serde_json::from_str::<Value>(&l.unwrap()).unwrap()["key"].clone()).unwrap())
            .collect()
    } else {
        std::fs::read_to_string(env("AUDIT_CL_TARGETS"))
            .unwrap()
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    };
    let mut indeg: HashMap<Vec<i32>, (u32, Value)> = HashMap::new();
    // credited keys: their edges are the image record's terms AT THE IMAGE (closure expand_key); the statement does not
    // carry the image, so a keys-only check without evidence gives it (AUDIT_CL_CREDITS=1)
    let images: HashMap<Vec<i32>, Vec<i32>> = if std::env::var_os("AUDIT_CL_CREDITS").is_some() {
        // the spec's evidence + the run's own proof / fact answer directories (AUDIT_CL_EXTRA_PROOFS / _FACTS):
        // the credit is granted exactly as in the run (Tier C)
        let mut ev = Evidence::from_spec(&set, &base).unwrap();
        for (var, facts) in [("AUDIT_CL_EXTRA_PROOFS", false), ("AUDIT_CL_EXTRA_FACTS", true)] {
            if let Ok(d) = std::env::var(var) {
                let mut files: Vec<PathBuf> = std::fs::read_dir(&d)
                    .unwrap()
                    .map(|e| e.unwrap().path())
                    .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
                    .collect();
                files.sort();
                for p in files {
                    let fr = import::spec::FileRef { path: p.display().to_string(), sha256: None };
                    if facts {
                        ev.load_facts(&set, Path::new("/"), &fr).unwrap();
                    } else {
                        ev.load_proofs(&set, Path::new("/"), &fr).unwrap();
                    }
                }
            }
        }
        let r = check(
            &set,
            &ev,
            &CheckOptions {
                threads: 2,
                tier: Some(Tier::C),
                max_rounds: None,
            },
        )
        .unwrap();
        r.rows
            .iter()
            .filter_map(|x| x.image.clone().map(|im| (x.key.clone(), im)))
            .collect()
    } else {
        HashMap::new()
    };
    eprintln!("credit images known: {}", images.len());
    let mut out = std::io::BufWriter::new(std::fs::File::create(env("AUDIT_CL_OUT")).unwrap());
    let f = std::fs::File::open(env("AUDIT_CL_STATEMENT")).unwrap();
    let (mut rows, mut rule_rows, mut terms, mut hits, mut other) = (0u64, 0u64, 0u64, 0u64, 0u64);
    let mut by_kind: HashMap<String, u64> = HashMap::new();
    let (mut buf, mut cm, mut pt) = (vec![], vec![], vec![]);
    let n = set.arity();
    for l in std::io::BufReader::new(f).lines() {
        let l = l.unwrap();
        let v: Value = serde_json::from_str(&l).unwrap();
        rows += 1;
        let kind = v["server"]["kind"].as_str().unwrap_or("-").to_string();
        *by_kind.entry(format!("{}:{}", v["status"].as_str().unwrap_or("-"), kind)).or_default() += 1;
        if v["status"] != "served" {
            continue;
        }
        let key: Vec<i32> = serde_json::from_value(v["key"].clone()).unwrap();
        // AUDIT_CL_TARGETS_IN=1: targets may be closure keys (in-degree analysis); else they must be absent
        if targets.contains(&key) && std::env::var_os("AUDIT_CL_TARGETS_IN").is_none() {
            panic!("target key {key:?} is IN the statement");
        }
        let row_key = key.clone();
        let applied = match kind.as_str() {
            "rule" => key.clone(),
            "credit" => match images.get(&key) {
                Some(im) => im.clone(),
                None => panic!("credited key {key:?} without a known image (set AUDIT_CL_CREDITS=1)"),
            },
            _ => {
                other += 1;
                continue;
            }
        };
        let sha = v["server"]["record_sha256"].as_str().unwrap();
        let &(t, r) = set.by_sha.get(sha).expect("statement record in the set");
        rule_rows += 1;
        buf.clear();
        cm.clear();
        let rr = set.disp.rule_ref(t, r);
        let Some(nt) = set.bodies.instantiate(&set.disp, rr, &applied, &mut buf, &mut cm).unwrap()
        else {
            continue;
        };
        let dom = &set.disp.tables()[t].domains[r];
        let key = applied;
        for j in 0..nt {
            terms += 1;
            let tk = &buf[j * n..(j + 1) * n];
            if targets.contains(tk) {
                hits += 1;
                assert!(rustred_campaign_core::closure::bodies::rule_point(dom, &key, &mut pt));
                let info = &set.rules[t][r];
                let e = json!({"table": info.path, "offset": info.offset, "rule": info.index, "record_sha256": sha,
                        "row_key": row_key, "server_kind": kind,
                        "source": info.source, "witness": format!("{:?}", info.witness),
                        "key": key, "point": pt, "term": j, "term_key": tk,
                        "fact_basis": v["facts"]["basis"]});
                if all {
                    let x = indeg.entry(tk.to_vec()).or_insert((0, Value::Null));
                    x.0 += 1;
                    if x.0 == 1 {
                        x.1 = e;
                    }
                } else {
                    writeln!(out, "{e}").unwrap();
                }
            }
        }
    }
    if all {
        let mut one: Vec<&Value> = indeg.values().filter(|x| x.0 == 1).map(|x| &x.1).collect();
        one.sort_by_key(|e| e["term_key"].to_string());
        for e in &one {
            writeln!(out, "{e}").unwrap();
        }
        eprintln!("in-degree: {} targets with an in-edge, {} with exactly one", indeg.len(), one.len());
    }
    out.flush().unwrap();
    eprintln!(
        "rows {rows} rule rows {rule_rows} other served {other} terms {terms} HITS {hits} kinds {by_kind:?} ({:.1} s)",
        t0.elapsed().as_secs_f64()
    );
}
