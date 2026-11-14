//! Lane import: synthetic positive and NEGATIVE cases on a toy node (arity 3, mask 110; nothing five-loop specific):
//! spec strictness, digests, label uniqueness, removed records, domain bounds, witness policies, facts bound by
//! (record_sha256, key) only, per-key proofs, recipe relations (domain + certification), supplied relations, terminals,
//! statement writing and the comparison with a hand statement that differs in one key.

use rustred_campaign_core::{closure, import};

use std::path::{Path, PathBuf};

use import::lines::key_text;
use import::{
    CheckOptions, DiffKind, Evidence, HandFormat, ImportError, Level, RowStatus, TableSetSpec,
    check, compare, load, read_hand_statement, write_statement,
};
use serde_json::{Value, json};

const HEADER: &str =
    r#"{"type":"header","schema":"table_dump.v1","family":"TOY","denominators":3,"mask":"110"}"#;

fn scratch(tag: &str) -> PathBuf {
    let base = std::env::var_os("RUSTRED_IMPORT_SCRATCH")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let d = base.join(format!("import-synth-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn sha(p: &Path) -> String {
    import::lines::file_sha256(p).unwrap()
}

/// A rule line: canonical target I(n0,..) over the toy node.
fn rule(
    index: i64,
    fixed: [Option<i64>; 3],
    rhs: &[&str],
    witness: Option<Value>,
    extra: Value,
) -> String {
    let t: Vec<String> = (0..3)
        .map(|i| fixed[i].map(|v| v.to_string()).unwrap_or(format!("n{i}")))
        .collect();
    let mut v = json!({"type":"rule","chart":"110","index":index,"fixed":fixed,"equations":[],
        "target": format!("I({})", t.join(",")),
        "rhs": rhs.iter().map(|k| json!([k, "1"])).collect::<Vec<_>>(),
        "exceptions":[],"dispatch":"Partition"});
    if let Some(w) = witness {
        v["witness_loci"] = w;
    }
    if let Value::Object(m) = extra {
        for (k, x) in m {
            v[k] = x;
        }
    }
    v.to_string()
}

fn write_table(dir: &Path, name: &str, rules: &[String]) -> PathBuf {
    let p = dir.join(name);
    let mut s = String::from(HEADER);
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

fn write_keys(dir: &Path, keys: &[[i32; 3]]) -> PathBuf {
    let p = dir.join("start.jsonl");
    let s: String = keys.iter().map(|k| format!("{}\n", json!(k))).collect();
    std::fs::write(&p, s).unwrap();
    p
}

/// The base toy: table A (rule 0: n0 descent on n1=1,n2=0), start keys I(3,1,0), I(2,1,0).
fn base(dir: &Path) -> Value {
    let a = write_table(
        dir,
        "A.table.jsonl",
        &[rule(
            0,
            [None, Some(1), Some(0)],
            &["I(n0-1,1,0)", "I(n0,0,0)"],
            Some(json!([])),
            json!({}),
        )],
    );
    let st = write_keys(dir, &[[3, 1, 0], [2, 1, 0]]);
    json!({"schema":"rustred.campaign.import-spec.v1","node":"toy","frame_from":"A","bits":6,
        "start":{"path": st.display().to_string(),"format":"keys_jsonl"},
        "sources":[{"id":"A","kind":"engine_table","class":"base","path":a.display().to_string(),
                    "sha256": sha(&a), "hand_name":{"template":"{source}#{label}"}}]})
}

fn run(spec: &Value, dir: &Path) -> (import::ImportedSet, Evidence, import::CheckResult) {
    let s = TableSetSpec::from_json(&spec.to_string()).unwrap();
    let set = load(&s, dir).unwrap();
    let ev = Evidence::from_spec(&set, dir).unwrap();
    let r = check(
        &set,
        &ev,
        &CheckOptions {
            threads: 2,
            ..Default::default()
        },
    )
    .unwrap();
    (set, ev, r)
}

fn row(r: &import::CheckResult, k: [i32; 3]) -> &import::KeyRow {
    r.rows
        .iter()
        .find(|x| x.key == k)
        .unwrap_or_else(|| panic!("key {k:?} not reached"))
}

#[test]
fn base_closes_tier_k_and_names_servers() {
    let d = scratch("base");
    let spec = base(&d);
    let (set, _ev, r) = run(&spec, &d);
    assert_eq!(r.rows.len(), 3, "I(3,1,0), I(2,1,0), I(1,1,0)");
    assert!(r.rows.iter().all(|x| x.status == RowStatus::Served));
    assert_eq!(row(&r, [1, 1, 0]).evidence, Some(Level::Imported));
    assert_eq!(row(&r, [1, 1, 0]).basis, Some("witness_loci"));
    assert_eq!(
        import::statement::server_hand_name(&set, row(&r, [2, 1, 0])).as_deref(),
        Some("A#0")
    );
    assert_eq!(set.rules[0][0].record_sha256.len(), 64);
    assert_eq!(r.summary.tier, closure::Tier::K);
}

#[test]
fn spec_refuses_unknown_fields_and_bad_digests() {
    let d = scratch("strict");
    let mut spec = base(&d);
    spec["sources"][0]["priority_file"] = json!("x");
    let e = TableSetSpec::from_json(&spec.to_string()).unwrap_err();
    assert!(matches!(e, ImportError::Spec(_)), "{e}");
    let mut spec = base(&d);
    spec["bogus"] = json!(1);
    assert!(TableSetSpec::from_json(&spec.to_string()).is_err());
    let mut spec = base(&d);
    spec["sources"][0]["kind"] = json!("mystery");
    assert!(TableSetSpec::from_json(&spec.to_string()).is_err());
    let mut spec = base(&d);
    spec["sources"][0]["sha256"] = json!("00".repeat(32));
    let s = TableSetSpec::from_json(&spec.to_string()).unwrap();
    assert!(matches!(
        load(&s, &d).unwrap_err(),
        ImportError::Digest { .. }
    ));
}

#[test]
fn negative_repeated_label_in_a_source_and_in_a_scope() {
    let d = scratch("labels");
    let mut spec = base(&d);
    let b = write_table(
        &d,
        "B.table.jsonl",
        &[
            rule(
                5,
                [Some(2), Some(2), Some(0)],
                &["I(1,2,0)"],
                Some(json!([])),
                json!({}),
            ),
            rule(
                5,
                [Some(3), Some(2), Some(0)],
                &["I(2,2,0)"],
                Some(json!([])),
                json!({}),
            ),
        ],
    );
    spec["sources"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"B","kind":"engine_table","path":b.display().to_string()}));
    let s = TableSetSpec::from_json(&spec.to_string()).unwrap();
    assert!(matches!(
        load(&s, &d).unwrap_err(),
        ImportError::Label { label: 5, .. }
    ));
    // the same label in two sources is fine per source, refused in a shared scope
    let c = write_table(
        &d,
        "C.table.jsonl",
        &[rule(
            0,
            [Some(2), Some(2), Some(0)],
            &["I(1,2,0)"],
            Some(json!([])),
            json!({}),
        )],
    );
    let mut spec = base(&d);
    spec["sources"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"C","kind":"engine_table","path":c.display().to_string()}));
    let s = TableSetSpec::from_json(&spec.to_string()).unwrap();
    assert!(load(&s, &d).is_ok());
    spec["sources"][0]["label_scope"] = json!("set");
    let e = TableSetSpec::from_json(&spec.to_string()).unwrap_err();
    assert!(
        matches!(e, ImportError::Spec(_)),
        "label_scope is a table_set field: {e}"
    );
}

#[test]
fn removed_records_fall_through_and_selection_order() {
    let d = scratch("removed");
    let a = write_table(
        &d,
        "A.table.jsonl",
        &[
            rule(
                0,
                [None, Some(1), Some(0)],
                &["I(n0-1,1,0)"],
                Some(json!(["elided"])),
                json!({}),
            ),
            rule(
                1,
                [None, Some(1), Some(0)],
                &["I(n0-1,1,0)", "I(n0,0,0)"],
                Some(json!([])),
                json!({}),
            ),
        ],
    );
    let st = write_keys(&d, &[[2, 1, 0]]);
    let mut spec = json!({"schema":"rustred.campaign.import-spec.v1","frame_from":"A",
        "start":{"path": st.display().to_string(),"format":"keys_jsonl"},
        "sources":[{"id":"A","kind":"engine_table","path":a.display().to_string()}]});
    // strict: the elided record 0 blocks
    let (set, _ev, r) = run(&spec, &d);
    let x = row(&r, [2, 1, 0]);
    assert_eq!(x.status, RowStatus::Open);
    assert_eq!(x.open_status.as_deref(), Some("blocked"));
    assert!(
        x.open_reason.as_deref().unwrap().contains("elided"),
        "{:?}",
        x.open_reason
    );
    assert_eq!(x.open_rule.map(|(t, rr)| set.rules[t][rr].label), Some(0));
    // removed with a reason: falls through to record 1
    spec["sources"][0]["removed"] = json!([{"label":0,"reason":"skipped by the hand statement"}]);
    let (set, _ev, r) = run(&spec, &d);
    assert!(r.rows.iter().all(|x| x.status == RowStatus::Served));
    assert_eq!(
        set.report.sources[0].removed,
        vec![(0, "skipped by the hand statement".to_string())]
    );
    // a removed label that does not exist is refused
    spec["sources"][0]["removed"] = json!([{"label":7,"reason":"x"}]);
    let s = TableSetSpec::from_json(&spec.to_string()).unwrap();
    assert!(matches!(
        load(&s, &d).unwrap_err(),
        ImportError::Label { label: 7, .. }
    ));
    // selection in a given order: [1, 0] puts record 1 first (no block)
    spec["sources"][0]["removed"] = json!([]);
    spec["sources"][0]["records"] = json!([1, 0]);
    let (_set, _ev, r) = run(&spec, &d);
    assert!(r.rows.iter().all(|x| x.status == RowStatus::Served));
    // a selected label that does not exist is refused
    spec["sources"][0]["records"] = json!([1, 9]);
    let s = TableSetSpec::from_json(&spec.to_string()).unwrap();
    assert!(matches!(
        load(&s, &d).unwrap_err(),
        ImportError::Label { label: 9, .. }
    ));
}

#[test]
fn elided_lift_needs_a_per_key_proof_bound_by_sha() {
    let d = scratch("lift");
    let a = write_table(
        &d,
        "A.table.jsonl",
        &[rule(
            0,
            [None, Some(1), Some(0)],
            &["I(n0-1,1,0)", "I(n0,0,0)"],
            Some(json!(["elided"])),
            json!({}),
        )],
    );
    let st = write_keys(&d, &[[2, 1, 0]]);
    let mut spec = json!({"schema":"rustred.campaign.import-spec.v1","frame_from":"A","required_evidence":"per-key",
        "policies":{"elided":"lift_with_per_key_proof"},
        "start":{"path": st.display().to_string(),"format":"keys_jsonl"},
        "sources":[{"id":"A","kind":"engine_table","path":a.display().to_string()}]});
    let (set, _ev, r) = run(&spec, &d);
    let sha = set.rules[0][0].record_sha256.clone();
    // the dispatcher stays strict: no witness-off table, never diagnostic
    assert!(r.summary.dispatch_not_strict.is_empty());
    assert!(!r.summary.diagnostic);
    assert_eq!(set.disp.tables().len(), 1);
    for k in [[2, 1, 0], [1, 1, 0]] {
        let x = row(&r, k);
        assert_eq!(x.status, RowStatus::Open);
        assert_eq!(
            x.open_reason.as_deref(),
            Some(import::check::REASON_NO_PER_KEY_PROOF)
        );
    }
    // Tier K follows the elided body (superset): I(1,1,0) is reached, blocked as well
    assert_eq!(r.summary.requires_per_key_proof, 2);
    assert_eq!(r.proof_requests.len(), 1);
    assert_eq!(r.proof_requests[0].keys.len(), 2);
    assert_eq!(r.proof_requests[0].record_sha256, sha);
    // proofs: one for (sha, I(2,1,0)); one for a FOREIGN digest at I(1,1,0); one non-exact status for (sha, I(1,1,0))
    let p = d.join("proof.json");
    std::fs::write(
        &p,
        json!({"schema":"rustred.proof-kernel.span.v1","keys":[
            {"record_sha256": sha, "key": {"n0":2}, "status":"EXACT_IN_SPAN_AT_KEY"},
            {"record_sha256": "ab".repeat(32), "key": {"n0":1,"n1":1,"n2":0}, "status":"EXACT_IN_SPAN_AT_KEY"},
            {"record_sha256": sha, "key": {"n0":1}, "status":"UNDECIDED (modular only)"}]})
        .to_string(),
    )
    .unwrap();
    spec["evidence"] = json!({"proofs":[{"path": p.display().to_string()}]});
    let (_set, ev, r) = run(&spec, &d);
    assert_eq!(ev.counts.proofs_foreign, 1);
    assert_eq!(ev.counts.proofs_per_key, 1);
    let x = row(&r, [2, 1, 0]);
    assert_eq!(
        (x.status, x.evidence, x.basis),
        (
            RowStatus::Served,
            Some(Level::PerKey),
            Some(import::check::BASIS_PER_KEY_PROOF)
        )
    );
    assert_eq!(r.summary.per_key_lifted.get("A"), Some(&1));
    assert_eq!(row(&r, [1, 1, 0]).status, RowStatus::Open);
    assert!(
        r.blocking().iter().any(|b| b == "import_open_keys:1"),
        "{:?}",
        r.blocking()
    );
    assert!(r.summary.dispatch_not_strict.is_empty() && !r.summary.diagnostic);
    // policy block: never served, whatever proofs exist
    spec["policies"]["elided"] = json!("block");
    let (_set, _ev, r) = run(&spec, &d);
    assert_eq!(row(&r, [2, 1, 0]).status, RowStatus::Open);
    assert_eq!(
        row(&r, [2, 1, 0]).open_reason.as_deref(),
        Some("elided witness loci (policy block)")
    );
    assert!(r.summary.per_key_lifted.is_empty());
}

/// The per-key lift is a first-class evidence level: with bound proofs at EVERY key of the elided record (and the
/// other facts a Tier C close needs) the verdict is `closed`, with the level `per-key` recorded per key; a proof for
/// another key or another record never serves; a witness-absent record is lifted by its proof in Tier K too.
#[test]
fn per_key_lift_closes_tier_c_and_serves_exactly_the_proved_keys() {
    let d = scratch("liftc");
    let a = write_table(
        &d,
        "A.table.jsonl",
        &[
            // elided face rule: I(n0,1,0) -> I(n0-1,1,0) + I(n0,0,0) for n0 >= 2
            rule(
                0,
                [None, Some(1), Some(0)],
                &["I(n0-1,1,0)", "I(n0,0,0)"],
                Some(json!(["elided"])),
                json!({"equations":["n0-2"]}),
            ),
            // complete point rule at I(1,1,0) -> subsector
            rule(
                1,
                [Some(1), Some(1), Some(0)],
                &["I(1,0,0)"],
                Some(json!([])),
                json!({}),
            ),
        ],
    );
    let st = write_keys(&d, &[[2, 1, 0]]);
    let sha0;
    let sha1;
    {
        let spec = json!({"schema":"rustred.campaign.import-spec.v1","frame_from":"A",
            "start":{"path": st.display().to_string(),"format":"keys_jsonl"},
            "sources":[{"id":"A","kind":"engine_table","path":a.display().to_string()}]});
        let (set, _ev, _r) = run(&spec, &d);
        sha0 = set.rules[0][0].record_sha256.clone();
        sha1 = set.rules[0][1].record_sha256.clone();
    }
    // facts: regularity of the point rule at I(1,1,0) (its witness loci are complete); no fact for the elided record
    let facts = d.join("facts.jsonl");
    std::fs::write(
        &facts,
        // (the answer's `rule` echo must be the record's index: 1)
        format!(
            "{}\n",
            fact_line(&sha1, [1, 1, 0], "regular", &[([1, 0, 0], "nonzero")])
                .replace("\"rule\":0", "\"rule\":1")
        ),
    )
    .unwrap();
    let write_proofs = |entries: Value| -> PathBuf {
        let p = d.join("proofs.json");
        std::fs::write(
            &p,
            json!({"schema":"rustred.proof-kernel.span.v1","keys": entries}).to_string(),
        )
        .unwrap();
        p
    };
    let spec_with = |proofs: &Path| {
        json!({"schema":"rustred.campaign.import-spec.v1","frame_from":"A","required_evidence":"imported",
            "start":{"path": st.display().to_string(),"format":"keys_jsonl"},
            "sources":[{"id":"A","kind":"engine_table","path":a.display().to_string()}],
            "evidence":{"facts":[{"path": facts.display().to_string()}],"proofs":[{"path": proofs.display().to_string()}]}})
    };
    // (1) the proof bound to (sha0, I(2,1,0)): Tier C closed, the key served at `per-key`
    let p = write_proofs(
        json!([{"record_sha256": sha0, "key": [2,1,0], "status":"EXACT_IN_SPAN_AT_KEY"}]),
    );
    let (_set, _ev, r) = run(&spec_with(&p), &d);
    assert_eq!(r.tier, closure::Tier::C);
    assert_eq!(
        r.summary.verdict,
        closure::Verdict::Closed,
        "{:?}",
        r.summary.blocking
    );
    assert!(r.blocking().is_empty(), "{:?}", r.blocking());
    let x = row(&r, [2, 1, 0]);
    assert_eq!(
        (x.evidence, x.basis),
        (Some(Level::PerKey), Some("per_key_proof"))
    );
    assert!(!x.regularity_unknown);
    assert_eq!(r.summary.per_key_lifted.get("A"), Some(&1));
    assert_eq!(row(&r, [1, 1, 0]).basis, Some("witness_loci"));
    // (2) a proof for another record at the same key, or for this record at another key: never served; Tier C does
    // not follow the blocked body
    for e in [
        json!([{"record_sha256": sha1, "key": [2,1,0], "status":"EXACT_IN_SPAN_AT_KEY"}]),
        json!([{"record_sha256": sha0, "key": [3,1,0], "status":"EXACT_IN_SPAN_AT_KEY"}]),
        json!([{"record_sha256": sha0, "key": [2,1,0], "status":"EXACT_NOT_IN_SPAN_AT_KEY (u ...)"}]),
    ] {
        let p = write_proofs(e);
        let (_set, _ev, r) = run(&spec_with(&p), &d);
        assert_eq!(r.rows.len(), 1, "Tier C: the blocked body is not followed");
        assert_eq!(row(&r, [2, 1, 0]).status, RowStatus::Open);
        assert_eq!(r.summary.verdict, closure::Verdict::Open);
        assert!(r.summary.per_key_lifted.is_empty());
    }
    // (3) a witness-ABSENT record (no witness_loci field) is lifted by its proof in Tier K as well
    let b = write_table(
        &d,
        "B.table.jsonl",
        &[rule(
            0,
            [None, Some(1), Some(0)],
            &["I(n0,0,0)"],
            None,
            json!({}),
        )],
    );
    let spec_b = json!({"schema":"rustred.campaign.import-spec.v1","frame_from":"B",
        "start":{"path": st.display().to_string(),"format":"keys_jsonl"},
        "sources":[{"id":"B","kind":"engine_table","path":b.display().to_string()}]});
    let (set, _ev, r) = run(&spec_b, &d);
    let shab = set.rules[0][0].record_sha256.clone();
    assert_eq!(row(&r, [2, 1, 0]).status, RowStatus::Open);
    let p = write_proofs(
        json!([{"record_sha256": shab, "key": [2,1,0], "status":"EXACT_PROVED_AT_KEY"}]),
    );
    let mut spec_b2 = spec_b.clone();
    spec_b2["evidence"] = json!({"proofs":[{"path": p.display().to_string()}]});
    let (_set, _ev, r) = run(&spec_b2, &d);
    assert_eq!(r.tier, closure::Tier::K);
    let x = row(&r, [2, 1, 0]);
    assert_eq!(
        (x.status, x.evidence, x.basis),
        (
            RowStatus::Served,
            Some(Level::PerKey),
            Some("per_key_proof")
        )
    );
}

fn fact_line(sha: &str, key: [i32; 3], regular: &str, rhs: &[([i32; 3], &str)]) -> String {
    json!({"schema":"rustred.proof-kernel.instantiate.v1","record_sha256":sha,"rule":0,"key":key,
        "in_case":true,"chart_ok":true,"branch":null,"regular":regular,"irregular_terms":[],"hidden_loci":[],
        "rhs": rhs.iter().enumerate().map(|(i,(k,c))| json!({"term":i,"key":k,"coef":c})).collect::<Vec<_>>(),
        "zero_test":"exact","error":null})
    .to_string()
}

#[test]
fn negative_fact_for_another_rule_does_not_lift_a_witness_absent_rule() {
    let d = scratch("facts");
    // record 0 has NO witness loci (bank-like); a decoy table holds another record
    let a = write_table(
        &d,
        "A.table.jsonl",
        &[rule(
            0,
            [None, Some(1), Some(0)],
            &["I(n0-1,1,0)", "I(n0,0,0)"],
            None,
            json!({}),
        )],
    );
    let z = write_table(
        &d,
        "Z.table.jsonl",
        &[rule(
            0,
            [Some(9), Some(9), Some(0)],
            &["I(8,9,0)"],
            Some(json!([])),
            json!({}),
        )],
    );
    let st = write_keys(&d, &[[2, 1, 0]]);
    let mut spec = json!({"schema":"rustred.campaign.import-spec.v1","frame_from":"A",
        "start":{"path": st.display().to_string(),"format":"keys_jsonl"},
        "sources":[{"id":"A","kind":"engine_table","path":a.display().to_string()},
                   {"id":"Z","kind":"engine_table","path":z.display().to_string()}]});
    let (set, _ev, r) = run(&spec, &d);
    assert_eq!(
        row(&r, [2, 1, 0]).open_status.as_deref(),
        Some("blocked"),
        "Tier K: witness-absent blocks"
    );
    assert!(
        r.fact_requests
            .iter()
            .any(|f| f.reasons.iter().any(|x| x == "witness_lift"))
    );
    let sha_a = set.rules[0][0].record_sha256.clone();
    let sha_z = set.rules[1][0].record_sha256.clone();
    // facts: one for Z's record at A's key (another rule), one for an unknown digest, one without digest
    let f = d.join("facts.jsonl");
    let mut s = fact_line(
        &sha_z,
        [2, 1, 0],
        "regular",
        &[([1, 1, 0], "nonzero"), ([2, 0, 0], "nonzero")],
    );
    s.push('\n');
    s.push_str(&fact_line(&"cd".repeat(32), [2, 1, 0], "regular", &[]));
    s.push('\n');
    let mut nodigest: Value =
        serde_json::from_str(&fact_line(&sha_a, [2, 1, 0], "regular", &[])).unwrap();
    nodigest.as_object_mut().unwrap().remove("record_sha256");
    s.push_str(&nodigest.to_string());
    s.push('\n');
    std::fs::write(&f, &s).unwrap();
    spec["evidence"] = json!({"facts":[{"path": f.display().to_string(), "sha256": sha(&f)}]});
    let (_set, ev, r) = run(&spec, &d);
    assert_eq!(r.tier, closure::Tier::C);
    assert_eq!(
        (
            ev.counts.facts_foreign,
            ev.counts.facts_unbound,
            ev.counts.facts_bound
        ),
        (1, 1, 1)
    );
    let x = row(&r, [2, 1, 0]);
    assert_eq!(
        x.status,
        RowStatus::Open,
        "a fact for another rule never lifts"
    );
    // the right fact lifts: (sha_a, I(2,1,0)) and (sha_a, I(1,1,0))
    s.push_str(&fact_line(
        &sha_a,
        [2, 1, 0],
        "regular",
        &[([1, 1, 0], "nonzero"), ([2, 0, 0], "nonzero")],
    ));
    s.push('\n');
    s.push_str(&fact_line(
        &sha_a,
        [1, 1, 0],
        "regular",
        &[([0, 1, 0], "nonzero"), ([1, 0, 0], "nonzero")],
    ));
    s.push('\n');
    std::fs::write(&f, &s).unwrap();
    spec["evidence"] = json!({"facts":[{"path": f.display().to_string()}]});
    let (_set, _ev, r) = run(&spec, &d);
    let x = row(&r, [2, 1, 0]);
    assert_eq!(
        (x.status, x.basis),
        (RowStatus::Served, Some("regularity_fact"))
    );
    assert!(
        r.rows.iter().all(|x| x.status == RowStatus::Served),
        "{:?}",
        r.counts()
    );
    // a conflicting fact is an error
    s.push_str(&fact_line(&sha_a, [1, 1, 0], "irregular", &[]));
    s.push('\n');
    std::fs::write(&f, &s).unwrap();
    let sp = TableSetSpec::from_json(&spec.to_string()).unwrap();
    let set = load(&sp, &d).unwrap();
    assert!(matches!(
        Evidence::from_spec(&set, &d),
        Err(ImportError::Evidence { .. })
    ));
}

#[test]
fn domain_bounds_and_qualified_recenter_are_selection_bounds() {
    let d = scratch("bounds");
    // record 0 with domain_bounds min n0 >= 3 (falls through at n0 = 2 to record 1)
    let a = write_table(
        &d,
        "A.table.jsonl",
        &[
            rule(
                0,
                [None, Some(1), Some(0)],
                &["I(n0-1,1,0)"],
                Some(json!([])),
                json!({"domain_bounds":{"min":[3,null,null],"max":null}}),
            ),
            rule(
                1,
                [None, Some(1), Some(0)],
                &["I(n0-1,1,0)", "I(n0,0,0)"],
                Some(json!([])),
                json!({}),
            ),
        ],
    );
    let st = write_keys(&d, &[[3, 1, 0]]);
    let spec = json!({"schema":"rustred.campaign.import-spec.v1","frame_from":"A",
        "start":{"path": st.display().to_string(),"format":"keys_jsonl"},
        "sources":[{"id":"A","kind":"engine_table","path":a.display().to_string()}]});
    let (set, _ev, r) = run(&spec, &d);
    let lab = |k| match row(&r, k).server {
        Some(import::Server::Rule { table, record }) => set.rules[table][record].label,
        _ => -1,
    };
    assert_eq!((lab([3, 1, 0]), lab([2, 1, 0]), lab([1, 1, 0])), (0, 1, 1));
    // qualified_recenter: recenter [-2,0,0] => n0 >= 3 as a SELECTION bound
    let b = write_table(
        &d,
        "B.table.jsonl",
        &[
            rule(
                0,
                [None, Some(1), Some(0)],
                &["I(n0-1,1,0)"],
                None,
                json!({"recenter":[-2,0,0]}),
            ),
            rule(
                1,
                [None, Some(1), Some(0)],
                &["I(n0-1,1,0)", "I(n0,0,0)"],
                Some(json!([])),
                json!({"recenter":[0,0,0]}),
            ),
        ],
    );
    let spec = json!({"schema":"rustred.campaign.import-spec.v1","frame_from":"B","policies":{"witness_absent":"block"},
        "start":{"path": st.display().to_string(),"format":"keys_jsonl"},
        "sources":[{"id":"B","kind":"engine_table","domain":"qualified_recenter","path":b.display().to_string()}]});
    let (set, _ev, r) = run(&spec, &d);
    assert_eq!(set.report.sources[0].bounds_records, 1);
    // I(3,1,0): record 0 selected (witness absent: blocked in Tier K); I(2,1,0) not reached
    assert_eq!(row(&r, [3, 1, 0]).open_status.as_deref(), Some("blocked"));
    // start at I(2,1,0): record 0 is NOT selected (outside its qualified domain) -> record 1 serves
    let st2 = write_keys(&d, &[[2, 1, 0]]);
    let mut spec2 = spec.clone();
    spec2["start"]["path"] = json!(st2.display().to_string());
    let (_set, _ev, r) = run(&spec2, &d);
    assert!(r.rows.iter().all(|x| x.status == RowStatus::Served));
    // a max on a line slot is refused like the engine
    let c = write_table(
        &d,
        "C.table.jsonl",
        &[rule(
            0,
            [None, Some(1), Some(0)],
            &["I(n0-1,1,0)"],
            Some(json!([])),
            json!({"domain_bounds":{"min":null,"max":[2,null,null]}}),
        )],
    );
    let spec3 = json!({"schema":"rustred.campaign.import-spec.v1","frame_from":"C",
        "start":{"path": st.display().to_string(),"format":"keys_jsonl"},
        "sources":[{"id":"C","kind":"engine_table","path":c.display().to_string()}]});
    let s = TableSetSpec::from_json(&spec3.to_string()).unwrap();
    assert!(matches!(
        load(&s, &d).unwrap_err(),
        ImportError::Line { .. }
    ));
}

fn recipe_file(dir: &Path, fixed: [Option<i64>; 3]) -> PathBuf {
    let p = dir.join("recipe.json");
    std::fs::write(
        &p,
        json!({"schema":"rustred.recipe.v1","id":"toy/r1","arity":3,"chart":"110",
            "domain":{"fixed":fixed,"equations":[],"exceptions":[]},
            "seeds":{"unit":"harder","centres":[],"generator":{"shells":["S0","S1"]}},
            "symmetry_rows":false,"elimination":{"convention":"rustred.recipe-elim.v1"}})
        .to_string(),
    )
    .unwrap();
    p
}

fn solved(
    src: &str,
    rsha: &str,
    key: [i32; 3],
    rhs: &[[i32; 3]],
    verdict: &str,
    ok: bool,
) -> String {
    json!({"schema":"rustred.campaign.solved-relation.v1","source":src,"recipe_sha256":rsha,
        "recipe_digest":"e".repeat(64),"determinism_id":"det1","binary_sha256":"b".repeat(64),
        "key":key,"verdict":verdict,"relation_digest":"aa".repeat(32),
        "rhs": rhs.iter().map(|k| json!({"key":k,"coef_sha256":"0".repeat(64),"mark":"top"})).collect::<Vec<_>>(),
        "proof":{"kind":"exact-weights","ok":ok,"weights":3},"regular":true,
        "results":{"path":"x","sha256":"0".repeat(64),"ordinal":0},"failure":null})
    .to_string()
}

#[test]
fn recipes_serve_only_certified_keys_in_their_domain() {
    let d = scratch("recipe");
    // table A serves n1 = 1 only from n0 >= 3 (fixed n0 = 3 point rule); recipe R's domain: n1 = 1, n2 = 0
    let a = write_table(
        &d,
        "A.table.jsonl",
        &[rule(
            0,
            [Some(3), Some(1), Some(0)],
            &["I(2,1,0)"],
            Some(json!([])),
            json!({}),
        )],
    );
    let rp = recipe_file(&d, [None, Some(1), Some(0)]);
    let rsha = sha(&rp);
    let st = write_keys(&d, &[[3, 1, 0]]);
    let mut spec = json!({"schema":"rustred.campaign.import-spec.v1","frame_from":"A","required_evidence":"per-key",
        "start":{"path": st.display().to_string(),"format":"keys_jsonl"},
        "sources":[{"id":"A","kind":"engine_table","path":a.display().to_string()},
                   {"id":"R","kind":"recipe","class":"recipe","path":rp.display().to_string(),"sha256":rsha}]});
    let (_set, _ev, r) = run(&spec, &d);
    let x = row(&r, [2, 1, 0]);
    assert_eq!(x.status, RowStatus::Open);
    assert!(
        x.open_reason.as_deref().unwrap().contains("recipe"),
        "{:?}",
        x.open_reason
    );
    assert_eq!(r.solve_requests.len(), 1);
    assert_eq!(
        (
            r.solve_requests[0].key.clone(),
            r.solve_requests[0].source.as_str()
        ),
        (vec![2, 1, 0], "R")
    );
    // solved relations: I(2,1,0) in domain (certified), I(5,2,0) OUTSIDE the domain (never served),
    // I(1,1,0) failed (proof not ok)
    let f = d.join("solved.jsonl");
    let s = [
        solved(
            "R",
            &rsha,
            [2, 1, 0],
            &[[1, 1, 0], [2, 0, 0]],
            "SOLVED",
            true,
        ),
        solved("R", &rsha, [5, 2, 0], &[[4, 2, 0]], "SOLVED", true),
        solved("R", &rsha, [1, 1, 0], &[[0, 1, 0]], "SOLVED", false),
    ]
    .join("\n");
    std::fs::write(&f, s).unwrap();
    spec["evidence"] = json!({"solved":[{"path": f.display().to_string()}],"expected_recipe_binary":"b".repeat(64)});
    let (_set, ev, r) = run(&spec, &d);
    assert_eq!(ev.counts.solved_bound, 3);
    let x = row(&r, [2, 1, 0]);
    assert_eq!(
        (x.status, x.evidence, x.basis),
        (
            RowStatus::Served,
            Some(Level::PerKey),
            Some("recipe_certified")
        )
    );
    assert!(
        r.rows.iter().all(|x| x.key != vec![5, 2, 0]),
        "a relation outside the domain is never followed"
    );
    let y = row(&r, [1, 1, 0]);
    assert_eq!(y.status, RowStatus::Open);
    assert!(
        r.solve_refusals
            .iter()
            .any(|s| s.key == vec![1, 1, 0] && s.reason.contains("proof"))
    );
    // the only candidate recipe answered (not certified) for I(1,1,0): no second request for the same recipe
    assert!(!r.solve_requests.iter().any(|s| s.key == vec![1, 1, 0]));
    assert!(
        row(&r, [1, 1, 0])
            .open_reason
            .as_deref()
            .unwrap()
            .contains("recipe")
    );
    // another binary: refused
    spec["evidence"]["expected_recipe_binary"] = json!("c".repeat(64));
    let (_set, _ev, r) = run(&spec, &d);
    assert_eq!(row(&r, [2, 1, 0]).status, RowStatus::Open);
}

#[test]
fn supplied_relations_and_terminals() {
    let d = scratch("supplied");
    let a = write_table(
        &d,
        "A.table.jsonl",
        &[rule(
            0,
            [Some(3), Some(1), Some(0)],
            &["I(2,1,0)"],
            Some(json!([])),
            json!({}),
        )],
    );
    // a supplied point relation I(2,1,0) = I(1,2,0) is NOT lower under the legacy order? use I(1,1,0) (lower)
    let s = write_table(
        &d,
        "S.table.jsonl",
        &[rule(
            0,
            [Some(2), Some(1), Some(0)],
            &["I(1,1,0)"],
            Some(json!([])),
            json!({"dispatch":"Point","kind":"symmetry-one-term"}),
        )],
    );
    let v = d.join("verify.json");
    std::fs::write(&v, "{\"proved\":true}\n").unwrap();
    let st = write_keys(&d, &[[3, 1, 0]]);
    let mut spec = json!({"schema":"rustred.campaign.import-spec.v1","frame_from":"A",
        "start":{"path": st.display().to_string(),"format":"keys_jsonl"},
        "sources":[{"id":"A","kind":"engine_table","path":a.display().to_string()},
                   {"id":"sym","kind":"supplied_relations","class":"symmetry","path":s.display().to_string(),
                    "verification":{"path": v.display().to_string()},"hand_name":{"template":"{source}#{index}"}},
                   {"id":"T","kind":"terminals","keys":[[1,1,0]]}]});
    let (set, _ev, r) = run(&spec, &d);
    let x = row(&r, [2, 1, 0]);
    assert_eq!(
        (x.status, x.evidence, x.basis),
        (RowStatus::Served, Some(Level::Imported), Some("supplied"))
    );
    assert_eq!(
        import::statement::server_hand_name(&set, x).as_deref(),
        Some("sym#0")
    );
    assert_eq!(row(&r, [1, 1, 0]).status, RowStatus::Terminal);
    // without its verification record the supplied relation is unverified (open)
    spec["sources"][1]
        .as_object_mut()
        .unwrap()
        .remove("verification");
    let (_set, _ev, r) = run(&spec, &d);
    assert_eq!(row(&r, [2, 1, 0]).status, RowStatus::Open);
}

#[test]
fn negative_statement_differing_in_one_key() {
    let d = scratch("compare");
    let spec = base(&d);
    let (set, ev, r) = run(&spec, &d);
    let out = d.join("stmt");
    let dg = write_statement(&set, &ev, &r, &out, None).unwrap();
    assert!(dg.files.contains_key("statement.jsonl") && out.join("SHA256SUMS").is_file());
    // deterministic: a second write gives the same digests
    let dg2 = write_statement(&set, &ev, &r, &d.join("stmt2"), None).unwrap();
    assert_eq!(dg.files, dg2.files);
    // a hand statement in the T178 column layout equal to ours, then with one key changed
    let hand_rows: Vec<Value> = r
        .rows
        .iter()
        .map(|x| {
            json!([
                key_text(&x.key),
                x.in_start,
                "base",
                import::statement::server_hand_name(&set, x),
                0
            ])
        })
        .collect();
    let fmt = {
        let mut f = HandFormat::positional(0, 1, 2, 3);
        f.round = Some(import::spec::Col::Index(4));
        f
    };
    let hp = d.join("hand.jsonl");
    std::fs::write(
        &hp,
        hand_rows
            .iter()
            .map(|v| format!("{v}\n"))
            .collect::<String>(),
    )
    .unwrap();
    let h = read_hand_statement(&hp, &fmt, 3).unwrap();
    let c = compare(&set, &r, &h);
    assert!(
        c.key_sets_equal && c.differences.is_empty() && c.terminals_equal,
        "{:?}",
        c.counts
    );
    assert_eq!(c.same_server, 3);
    let mut changed = hand_rows.clone();
    changed[1][3] = json!("A#99");
    std::fs::write(
        &hp,
        changed.iter().map(|v| format!("{v}\n")).collect::<String>(),
    )
    .unwrap();
    let c = compare(&set, &r, &read_hand_statement(&hp, &fmt, 3).unwrap());
    assert_eq!(c.differences.len(), 1);
    assert_eq!(c.count(DiffKind::ServerDiffers), 1);
    assert_eq!(c.unmapped_hand_rules.get("A#99"), Some(&1));
    // one key missing from the hand statement, one extra hand key, one hand terminal we serve
    let mut v2 = hand_rows.clone();
    v2.remove(0);
    v2.push(json!(["I(7,1,0)", false, "base", "A#0", 3]));
    v2[0][2] = json!("terminal");
    v2[0][3] = Value::Null;
    std::fs::write(&hp, v2.iter().map(|v| format!("{v}\n")).collect::<String>()).unwrap();
    let c = compare(&set, &r, &read_hand_statement(&hp, &fmt, 3).unwrap());
    assert_eq!(
        (
            c.count(DiffKind::OnlyOurs),
            c.count(DiffKind::OnlyHand),
            c.count(DiffKind::HandTerminalOursServed)
        ),
        (1, 1, 1)
    );
    assert!(!c.key_sets_equal && !c.terminals_equal);
    c.write(&d.join("cmp")).unwrap();
    let n = std::fs::read_to_string(d.join("cmp/differences.jsonl"))
        .unwrap()
        .lines()
        .count();
    assert_eq!(n, 3, "every difference listed");
}

#[test]
fn solve_requests_go_to_the_first_candidate_recipe_without_an_answer() {
    let d = scratch("recipe2");
    let a = write_table(
        &d,
        "A.table.jsonl",
        &[rule(
            0,
            [Some(3), Some(1), Some(0)],
            &["I(2,1,0)"],
            Some(json!([])),
            json!({}),
        )],
    );
    let r1 = recipe_file(&d, [None, Some(1), Some(0)]);
    let r2 = d.join("recipe2.json");
    let mut v: Value = serde_json::from_str(&std::fs::read_to_string(&r1).unwrap()).unwrap();
    v["id"] = json!("toy/r2");
    std::fs::write(&r2, v.to_string()).unwrap();
    let (s1, s2) = (sha(&r1), sha(&r2));
    let st = write_keys(&d, &[[3, 1, 0]]);
    let mut spec = json!({"schema":"rustred.campaign.import-spec.v1","frame_from":"A",
        "start":{"path": st.display().to_string(),"format":"keys_jsonl"},
        "sources":[{"id":"A","kind":"engine_table","path":a.display().to_string()},
                   {"id":"R1","kind":"recipe","path":r1.display().to_string()},
                   {"id":"R2","kind":"recipe","path":r2.display().to_string()}]});
    let (_set, _ev, r) = run(&spec, &d);
    let reqs: Vec<(&str, Vec<i32>)> = r
        .solve_requests
        .iter()
        .map(|q| (q.source.as_str(), q.key.clone()))
        .collect();
    assert_eq!(
        reqs,
        vec![("R1", vec![2, 1, 0])],
        "one request, to the first candidate in dispatch order"
    );
    // R1 answers NO_RELATION_IN_SEEDS at I(2,1,0): the next request goes to R2
    let f = d.join("solved.jsonl");
    std::fs::write(
        &f,
        solved("R1", &s1, [2, 1, 0], &[], "NO_RELATION_IN_SEEDS", false),
    )
    .unwrap();
    spec["evidence"] = json!({"solved":[{"path": f.display().to_string()}]});
    let (_set, _ev, r) = run(&spec, &d);
    let reqs: Vec<(&str, Vec<i32>)> = r
        .solve_requests
        .iter()
        .map(|q| (q.source.as_str(), q.key.clone()))
        .collect();
    assert_eq!(reqs, vec![("R2", vec![2, 1, 0])]);
    // R2 certifies: served by R2 (dispatch falls through R1, which has no certified relation there)
    let both = format!(
        "{}\n{}\n{}\n",
        solved("R1", &s1, [2, 1, 0], &[], "NO_RELATION_IN_SEEDS", false),
        solved(
            "R2",
            &s2,
            [2, 1, 0],
            &[[1, 1, 0], [2, 0, 0]],
            "SOLVED",
            true
        ),
        solved(
            "R2",
            &s2,
            [1, 1, 0],
            &[[0, 1, 0], [1, 0, 0]],
            "SOLVED",
            true
        )
    );
    std::fs::write(&f, both).unwrap();
    let (_set, _ev, r) = run(&spec, &d);
    assert_eq!(
        row(&r, [2, 1, 0]).server,
        Some(import::Server::Relation {
            set: "R2".into(),
            recipe: true
        })
    );
    assert!(
        r.rows.iter().all(|x| x.status == RowStatus::Served),
        "{:?}",
        r.counts()
    );
    assert!(r.blocking().contains(&"recipe_binary_unpinned".to_string()));
}

#[test]
fn partial_object_line_hand_statement_third_shape() {
    // the T266 delivery shape (solves.jsonl): object lines, only SOME keys listed, no in-box flag, a "none" result for
    // the terminals; read through field names, compared on the listed keys only
    let d = scratch("partial");
    let spec = base(&d);
    let (set, _ev, r) = run(&spec, &d);
    let fmt: HandFormat =
        serde_json::from_value(json!({"key":"key","label":"result","rule":"recipe",
        "terminal_label":"none","partial":true}))
        .unwrap();
    let hp = d.join("solves.jsonl");
    std::fs::write(
        &hp,
        format!(
            "{}\n{}\n",
            json!({"item":"0","key":"I(2,1,0)","result":"served","recipe":"A#0","seconds":"1.0"}),
            json!({"item":"1","key":"I(3,1,0)","result":"served","recipe":"other-recipe"})
        ),
    )
    .unwrap();
    let h = read_hand_statement(&hp, &fmt, 3).unwrap();
    assert!(h.partial && h.rows.iter().all(|x| x.in_box.is_none()));
    let c = compare(&set, &r, &h);
    assert!(
        !c.key_sets_equal,
        "a partial statement never establishes key-set equality"
    );
    assert_eq!(
        c.count(DiffKind::OnlyOurs),
        0,
        "unlisted keys are not differences"
    );
    assert_eq!((c.same_server, c.count(DiffKind::ServerDiffers)), (1, 1));
}

/// Compact evidence lines (interfaces/import.md 5.5): a FACT VERDICT line lists the term-key digests of the NON-ZERO
/// terms only, so an unlisted term has a coefficient identically zero at the key and is NOT an edge (Tier C); a listed
/// positive-numerator term fails the closure; the `index` echo must name the record; a compact PROOF verdict line with
/// rr_proof's `n0=..` key text binds by (record_sha256, key) like a span record.
#[test]
fn compact_fact_and_proof_verdict_lines() {
    use import::lines::term_key_digest;
    let d = scratch("compact");
    let a = write_table(
        &d,
        "A.table.jsonl",
        &[rule(
            0,
            [None, Some(1), Some(0)],
            &["I(n0-1,1,0)", "I(n0,0,0)", "I(n0,1,1)"],
            Some(json!([])),
            json!({"equations":["n0-2"]}),
        )],
    );
    let st = write_keys(&d, &[[2, 1, 0]]);
    let spec0 = json!({"schema":"rustred.campaign.import-spec.v1","frame_from":"A",
        "start":{"path": st.display().to_string(),"format":"keys_jsonl"},
        "sources":[{"id":"A","kind":"engine_table","path":a.display().to_string()}]});
    let (set, _ev, r) = run(&spec0, &d);
    let sha = set.rules[0][0].record_sha256.clone();
    // Tier K: every top term followed (I(1,1,0) reached, open: no rule), the positive-numerator term flagged
    assert_eq!(r.rows.len(), 2);
    let hex = |k: [i32; 3]| format!("{:016x}", term_key_digest(&k));
    let write = |lines: &[Value]| -> PathBuf {
        let p = d.join("facts.jsonl");
        let s: String = lines.iter().map(|l| format!("{l}\n")).collect();
        std::fs::write(&p, s).unwrap();
        p
    };
    let with_facts = |p: &Path| {
        let mut s = spec0.clone();
        s["evidence"] = json!({"facts":[{"path": p.display().to_string()}]});
        s
    };
    // (1) I(1,1,0) and I(2,1,1) unlisted (zero), I(2,0,0) non-zero: only the hand-down remains; closed
    let p = write(&[
        json!({"schema":"rustred.campaign.fact-verdict.v1","index":0,"record_sha256":sha,
        "key":[2,1,0],"verdict":"regular","n_terms":3,"nonzero":[hex([2,0,0])]}),
    ]);
    let (_s, ev, r) = run(&with_facts(&p), &d);
    assert_eq!(ev.counts.fact_verdict_lines, 1);
    assert_eq!(r.tier, closure::Tier::C);
    assert_eq!(r.rows.len(), 1, "a zero-coefficient term is not an edge");
    assert_eq!(
        r.summary.verdict,
        closure::Verdict::Closed,
        "{:?}",
        r.summary.blocking
    );
    // (2) the top term non-zero: followed (I(1,1,0) open); the positive-numerator term listed: the closure FAILS
    let p = write(&[
        json!({"schema":"rustred.campaign.fact-verdict.v1","index":0,"record_sha256":sha,
        "key":[2,1,0],"verdict":"regular","n_terms":3,"nonzero":[hex([1,1,0]),hex([2,0,0]),hex([2,1,1])]}),
    ]);
    let (_s, _ev, r) = run(&with_facts(&p), &d);
    assert_eq!(r.rows.len(), 2);
    assert_eq!(r.summary.verdict, closure::Verdict::Failed);
    // (3) a wrong index echo is refused; a malformed digest is refused
    for bad in [
        json!({"schema":"rustred.campaign.fact-verdict.v1","index":7,"record_sha256":sha,"key":[2,1,0],
               "verdict":"regular","nonzero":[]}),
        json!({"schema":"rustred.campaign.fact-verdict.v1","index":0,"record_sha256":sha,"key":[2,1,0],
               "verdict":"regular","nonzero":["xyz"]}),
    ] {
        let p = write(&[bad]);
        let s = TableSetSpec::from_json(&with_facts(&p).to_string()).unwrap();
        let set = load(&s, &d).unwrap();
        assert!(matches!(
            Evidence::from_spec(&set, &d),
            Err(ImportError::Evidence { .. })
        ));
    }
    // (4) compact PROOF verdict lines: `n0=..` key text (free slots only; the rest from the record), sha_ok honoured
    let pp = d.join("proofs.jsonl");
    std::fs::write(
        &pp,
        format!(
            "{}\n{}\n",
            json!({"name":"A#0","index":0,"record_sha256":sha,"key":"n0=2","verdict":"EXACT_IN_SPAN_AT_KEY","sha_ok":true}),
            json!({"name":"A#0","index":0,"record_sha256":"cd".repeat(32),"key":"n0=2,n1=1,n2=0","verdict":"EXACT_IN_SPAN_AT_KEY"})
        ),
    )
    .unwrap();
    let mut s = spec0.clone();
    s["evidence"] = json!({"proofs":[{"path": pp.display().to_string()}]});
    s["required_evidence"] = json!("per-key");
    let (_s, ev, r) = run(&s, &d);
    assert_eq!(
        (
            ev.counts.proof_verdict_lines,
            ev.counts.proofs_per_key,
            ev.counts.proofs_foreign
        ),
        (2, 1, 1)
    );
    assert_eq!(row(&r, [2, 1, 0]).evidence, Some(Level::PerKey));
}

/// `check_streamed`: every row written at once to the statement file, only the exceptional rows kept; the counts, the
/// evidence by class, the key-set digest and the statement's rows (as a set) equal the in-memory check's.
#[test]
fn streamed_check_equals_the_in_memory_check() {
    let d = scratch("stream");
    let a = write_table(
        &d,
        "A.table.jsonl",
        &[
            rule(
                0,
                [None, Some(1), Some(0)],
                &["I(n0-1,1,0)", "I(n0,0,0)"],
                Some(json!([])),
                json!({"exceptions":[["n0-1"]]}),
            ),
            rule(
                1,
                [Some(1), Some(1), Some(0)],
                &["I(1,0,0)"],
                None,
                json!({}),
            ),
        ],
    );
    let st = write_keys(&d, &[[5, 1, 0], [3, 1, 0]]);
    let spec = json!({"schema":"rustred.campaign.import-spec.v1","frame_from":"A",
        "start":{"path": st.display().to_string(),"format":"keys_jsonl"},
        "sources":[{"id":"A","kind":"engine_table","path":a.display().to_string()}]});
    let s = TableSetSpec::from_json(&spec.to_string()).unwrap();
    let set = load(&s, &d).unwrap();
    let ev = Evidence::from_spec(&set, &d).unwrap();
    let mem = check(&set, &ev, &CheckOptions::default()).unwrap();
    let rows_path = d.join("stream").join("rows.jsonl");
    let st =
        import::check::check_streamed(&set, &ev, &CheckOptions::default(), &rows_path).unwrap();
    assert_eq!(mem.rows.len(), 5, "I(5..1,1,0)");
    assert_eq!(st.counts(), mem.counts());
    assert_eq!(st.evidence_by_class(&set), mem.evidence_by_class(&set));
    assert_eq!(st.key_set_digest, mem.key_set_digest);
    assert_eq!(st.streamed.as_ref().unwrap().rows, 5);
    // kept: only the exceptional row (I(1,1,0): witness-absent record without its fact -> open)
    assert_eq!(st.rows.len(), 1);
    assert_eq!(st.rows[0].key, vec![1, 1, 0]);
    let mut a: Vec<String> = std::fs::read_to_string(&rows_path)
        .unwrap()
        .lines()
        .map(str::to_string)
        .collect();
    a.sort();
    let dir_m = d.join("stmt-mem");
    let dir_s = d.join("stmt-stream");
    write_statement(&set, &ev, &mem, &dir_m, None).unwrap();
    write_statement(&set, &ev, &st, &dir_s, None).unwrap();
    let mut b: Vec<String> = std::fs::read_to_string(dir_m.join("statement.jsonl"))
        .unwrap()
        .lines()
        .map(str::to_string)
        .collect();
    b.sort();
    let mut c: Vec<String> = std::fs::read_to_string(dir_s.join("statement.jsonl"))
        .unwrap()
        .lines()
        .map(str::to_string)
        .collect();
    c.sort();
    assert_eq!(a, b);
    assert_eq!(c, b);
    let sm: Value =
        serde_json::from_str(&std::fs::read_to_string(dir_s.join("summary.json")).unwrap())
            .unwrap();
    assert_eq!(sm["keys"], 5);
}

/// Per-key recipe assignment (recipe v1.2 `domain.keys`, or the spec's side file until v1.2 is frozen): a recipe is
/// asked for, and serves, ONLY its assigned keys; the next candidate recipe is asked for the others; a solved relation
/// for an unassigned key is refused.
#[test]
fn per_key_recipe_assignment_side_file_and_domain_keys() {
    let d = scratch("assign");
    let a = write_table(
        &d,
        "A.table.jsonl",
        &[rule(
            0,
            [Some(3), Some(1), Some(0)],
            &["I(2,1,0)"],
            Some(json!([])),
            json!({}),
        )],
    );
    let r1 = recipe_file(&d, [None, Some(1), Some(0)]);
    let rid: String = serde_json::from_str::<Value>(&std::fs::read_to_string(&r1).unwrap())
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    // R2: the same domain, v1.2 domain.keys = [I(2,1,0)]
    let r2 = d.join("recipe2.json");
    let mut v: Value = serde_json::from_str(&std::fs::read_to_string(&r1).unwrap()).unwrap();
    v["id"] = json!("toy/r2");
    v["domain"]["keys"] = json!([[2, 1, 0]]);
    std::fs::write(&r2, v.to_string()).unwrap();
    // side file: R1 is assigned I(5,1,0) only (another recipe's line is ignored)
    let side = d.join("assign.jsonl");
    std::fs::write(
        &side,
        format!(
            "{}\n{}\n",
            json!({"recipe": rid, "keys": [[5,1,0]]}),
            json!({"recipe": "someone/else", "keys": [[2,1,0]]})
        ),
    )
    .unwrap();
    let st = write_keys(&d, &[[3, 1, 0]]);
    let mut spec = json!({"schema":"rustred.campaign.import-spec.v1","frame_from":"A",
        "start":{"path": st.display().to_string(),"format":"keys_jsonl"},
        "sources":[{"id":"A","kind":"engine_table","path":a.display().to_string()},
                   {"id":"R1","kind":"recipe","path":r1.display().to_string(),
                    "assignment":{"path": side.display().to_string()}},
                   {"id":"R2","kind":"recipe","path":r2.display().to_string()}]});
    let (set, _ev, r) = run(&spec, &d);
    assert_eq!(set.recipes[0].assigned.as_ref().map(|a| a.len()), Some(1));
    assert_eq!(set.recipes[1].assigned.as_ref().map(|a| a.len()), Some(1));
    let reqs: Vec<(&str, Vec<i32>)> = r
        .solve_requests
        .iter()
        .map(|q| (q.source.as_str(), q.key.clone()))
        .collect();
    assert_eq!(
        reqs,
        vec![("R2", vec![2, 1, 0])],
        "R1 is not assigned I(2,1,0)"
    );
    // a SOLVED answer of R1 at the unassigned key is refused
    let f = d.join("solved.jsonl");
    std::fs::write(
        &f,
        solved(
            "R1",
            &sha(&r1),
            [2, 1, 0],
            &[[1, 1, 0], [2, 0, 0]],
            "SOLVED",
            true,
        ),
    )
    .unwrap();
    spec["evidence"] = json!({"solved":[{"path": f.display().to_string()}],
        "expected_recipe_binary": "bb".repeat(32)});
    let (_set, _ev, r) = run(&spec, &d);
    assert!(
        r.solve_refusals
            .iter()
            .any(|x| x.source == "R1" && x.reason.contains("not assigned")),
        "{:?}",
        r.solve_refusals
    );
    // an unknown recipe domain field is refused, never ignored
    let r3 = d.join("recipe3.json");
    let mut v: Value = serde_json::from_str(&std::fs::read_to_string(&r1).unwrap()).unwrap();
    v["domain"]["centre_rule"] = json!("first_dotted_line");
    std::fs::write(&r3, v.to_string()).unwrap();
    spec["sources"][2]["path"] = json!(r3.display().to_string());
    let s = TableSetSpec::from_json(&spec.to_string()).unwrap();
    assert!(load(&s, &d).is_err());
}

/// Recipe steps (interfaces/import.md 14): the step grammar and the explicit seeds at a key (toy arity 3, mask 110).
#[test]
fn recipe_step_grammar_and_explicit_seeds() {
    use import::recipe_steps::{Centre, RecipeForm, explicit_seeds, parse_step, recipes_for_steps};
    let s = parse_step("d2_cfirst_fon_sfirst_mM1C").unwrap();
    assert_eq!(
        (s.depth, s.centre, s.floor, s.symmetry_first),
        (2, Centre::First, true, true)
    );
    assert!(parse_step("d2_cweird").is_err() && parse_step("x2").is_err());
    let lines = [true, true, false];
    // key I(3,1,-1): off-floor line 0, numerator 2
    let (c, f, sh, _) = explicit_seeds(&s, &[3, 1, -1], &lines, &[1, 0], &[2]);
    assert_eq!(
        (c, f, sh),
        (
            vec![vec![-1, 0, 0]],
            vec![0],
            vec!["S0".into(), "S1".into(), "S2".into()]
        )
    );
    let u = parse_step("d1_cunion_foff").unwrap();
    let (c, f, _, sym) = explicit_seeds(&u, &[3, 1, -1], &lines, &[1, 0], &[2]);
    assert_eq!(
        (c, f.len(), sym),
        (vec![vec![-1, 0, 0], vec![0, 0, 1]], 0, false)
    );
    let n = parse_step("d0_cnum").unwrap();
    assert_eq!(
        explicit_seeds(&n, &[3, 1, -1], &lines, &[1, 0], &[2]).0,
        vec![vec![0, 0, 1]]
    );
    // grouping: two keys with the same explicit seed set -> one v1.2 recipe with domain.keys
    let asg = vec![
        ("d1_cnone_foff".to_string(), vec![2, 1, 0]),
        ("d1_cnone_foff".to_string(), vec![3, 1, 0]),
        ("d1_cfirst_foff".to_string(), vec![3, 1, 0]),
    ];
    let g = recipes_for_steps(&asg, &lines, &[0, 1], &[2], RecipeForm::V1_2, "toy/").unwrap();
    assert_eq!(g.len(), 2);
    assert_eq!(g[0].recipe["domain"]["keys"], json!([[2, 1, 0], [3, 1, 0]]));
    assert_eq!(g[0].recipe["relation_rule"], json!("first_admissible"));
    let g = recipes_for_steps(&asg, &lines, &[0, 1], &[2], RecipeForm::V1_1, "toy/").unwrap();
    assert_eq!(g.len(), 3);
    assert_eq!(g[0].recipe["domain"]["fixed"], json!([2, 1, 0]));
}

/// The STREAMING fact pass (interfaces/import.md 13.8) after a Tier K check: per compact fact line, regularity at the
/// winning record, required-zero terms, non-zero edges; keys reached only through zero terms are reported unreached.
#[test]
fn streaming_fact_pass_after_a_tier_k_check() {
    use import::factpass::fact_pass;
    use import::lines::term_key_digest;
    let d = scratch("factpass");
    // witness-absent rule (pre-guard): I(n0,1,0) -> I(n0-1,1,0) + I(n0,0,0) + I(n0,1,1) (positive numerator), n0 >= 2
    let a = write_table(
        &d,
        "A.table.jsonl",
        &[rule(
            0,
            [None, Some(1), Some(0)],
            &["I(n0-1,1,0)", "I(n0,0,0)", "I(n0,1,1)"],
            None,
            json!({"exceptions":[["n0-1"]]}),
        )],
    );
    let st = write_keys(&d, &[[3, 1, 0]]);
    let spec = json!({"schema":"rustred.campaign.import-spec.v1","frame_from":"A","policies":{"tier":"K"},
        "start":{"path": st.display().to_string(),"format":"keys_jsonl"},
        "sources":[{"id":"A","kind":"engine_table","path":a.display().to_string()}]});
    let (set, ev, _r) = run(&spec, &d);
    let sha = set.rules[0][0].record_sha256.clone();
    // Tier K superset: I(3,1,0) -> I(2,1,0) -> I(1,1,0) (the last uncovered: exception n0 = 1)
    let res =
        import::check::check_streamed(&set, &ev, &CheckOptions::default(), &d.join("rows.jsonl"))
            .unwrap();
    assert_eq!(res.summary.keys_total, 3);
    let hex = |k: [i32; 3]| format!("{:016x}", term_key_digest(&k));
    let line = |k: [i32; 3], nz: Vec<String>| {
        json!({"schema":"rustred.campaign.fact-verdict.v1","index":0,"record_sha256":sha,"key":k,
            "verdict":"regular","n_terms":3,"nonzero":nz})
        .to_string()
    };
    // (1) I(3,1,0): the top term non-zero (edge), I(2,1,0): the top term ZERO -> I(1,1,0) unreached (the open key goes)
    let f1 = d.join("facts1.jsonl");
    std::fs::write(
        &f1,
        format!(
            "{}\n{}\n",
            line([3, 1, 0], vec![hex([2, 1, 0]), hex([3, 0, 0])]),
            line([2, 1, 0], vec![hex([2, 0, 0])])
        ),
    )
    .unwrap();
    let rep = fact_pass(&set, &res, &[f1.as_path()]).unwrap();
    assert_eq!(
        (
            rep.regular,
            rep.served_without_regularity,
            rep.blocked_lifted
        ),
        (2, 0, 2)
    );
    assert_eq!((rep.failures_total, rep.edges_nonzero), (0, 1));
    assert_eq!(
        (rep.unreached_total, rep.open_unreached, rep.open_total),
        (1, 1, 1)
    );
    assert_eq!(rep.unreached, vec![vec![1, 1, 0]]);
    // (2) the positive-numerator term listed non-zero at I(3,1,0): a failure; I(2,1,0) without a line: no regularity
    let f2 = d.join("facts2.jsonl");
    std::fs::write(
        &f2,
        format!(
            "{}\n",
            line([3, 1, 0], vec![hex([2, 1, 0]), hex([3, 1, 1])])
        ),
    )
    .unwrap();
    let rep = fact_pass(&set, &res, &[f2.as_path()]).unwrap();
    assert_eq!(rep.failures_total, 1);
    assert_eq!(rep.failures[0].2, vec![3, 1, 1]);
    assert_eq!(rep.served_without_regularity, 1);
    // the in-memory check result gives the same report
    let mem = check(&set, &ev, &CheckOptions::default()).unwrap();
    let rep_mem = fact_pass(&set, &mem, &[f1.as_path()]).unwrap();
    assert_eq!(
        (
            rep_mem.regular,
            rep_mem.unreached_total,
            rep_mem.edges_nonzero
        ),
        (2, 1, 1)
    );
}

/// Lane import3 (item 1): a DOMAIN literal whose coefficients exceed i128 (here 45 and 46 digits, as in four T266
/// records) is read EXACTLY, never refused and never truncated: the record loads, its exception is evaluated exactly at
/// the rule point and the dispatch falls through exactly on its zero set.
#[test]
fn big_exception_literals_load_and_dispatch_exactly() {
    let d = scratch("biglit");
    let big = "123456789012345678901234567890123456789012345";
    let a = write_table(
        &d,
        "A.table.jsonl",
        &[rule(
            0,
            [None, Some(1), Some(0)],
            &["I(n0-1,1,0)", "I(n0,0,0)"],
            Some(json!([])),
            // branch 0 vanishes at n0 = 2 only (a big multiple of n0 - 2); branch 1 never vanishes on keys (a big
            // constant offset); branch 2 is an AND whose second literal (big, nonzero at n0 = 4) keeps n0 = 4 selected
            json!({"exceptions": [[format!("{big}*n0 - 2*{big}")], [format!("{big}0*n0^2 + 1")],
                                  ["n0 - 4".to_string(), format!("{big}*n0 - 1")]]}),
        )],
    );
    let st = write_keys(&d, &[[4, 1, 0], [3, 1, 0]]);
    let spec = json!({"schema":"rustred.campaign.import-spec.v1","node":"toy","frame_from":"A","bits":6,
        "start":{"path": st.display().to_string(),"format":"keys_jsonl"},
        "sources":[{"id":"A","kind":"engine_table","class":"base","path":a.display().to_string(),
                    "sha256": sha(&a)}]});
    let (_set, _ev, r) = run(&spec, &d);
    assert_eq!(row(&r, [4, 1, 0]).status, RowStatus::Served);
    assert_eq!(row(&r, [3, 1, 0]).status, RowStatus::Served);
    let k2 = row(&r, [2, 1, 0]);
    assert_eq!(k2.status, RowStatus::Open, "on the big exception: the dispatch falls through");
    assert_eq!(k2.open_status.as_deref(), Some("uncovered"));
    // the core domain itself: exact literal, exact verdicts
    use rustred_campaign_core::poly::{Poly, PolyError};
    let lit = format!("{big}*n0 - 2*{big}");
    assert_eq!(Poly::parse_d_free(&lit, 3), Err(PolyError::CoefficientOverflow));
    let p = Poly::parse_d_free_exact(&lit, 3).unwrap();
    assert!(p.is_big() && p.small_terms().is_none());
    assert!(p.is_zero_at(&[2, 1, 0]).unwrap());
    assert!(!p.is_zero_at(&[3, 1, 0]).unwrap());
    assert_eq!(p.to_string(), format!("-{}+{big}*n0", "246913578024691357802469135780246913578024690"));
}

/// Lane import3 (item 2): the table index. One pass builds it (domains + digests + rhs keys); the records read back
/// from it equal a direct scan field by field (record_sha256 of the ORIGINAL line, offset, bytes, domain fields,
/// rhs keys, file sha256); a second open REUSES it (size, mtime, sampled digest unchanged); a changed file is
/// re-indexed; a load through a persistent index gives the same check as a load without one.
#[test]
fn table_index_one_pass_reuse_and_equality() {
    use import::index::{INDEX_VERSION, open_or_build};
    use import::lines::scan_table;
    let d = scratch("index");
    let mut spec = base(&d);
    let a = d.join("A.table.jsonl");
    // a second, larger record (with origin / domain_bounds fields) and a non-rule line between the records
    let mut txt = std::fs::read_to_string(&a).unwrap();
    let extra = rule(
        7,
        [None, None, Some(0)],
        &["I(n0-1,n1,0)", "I(n0,n1-1,0)", "I(2,1,0)"],
        None,
        json!({"origin":"tb-x","origin_index":4,"domain_bounds":{"min":[2,null,null],"max":null}}),
    );
    txt = txt.replace(
        r#"{"type":"end""#,
        &format!("{{\"type\":\"residual\",\"key\":[1,1,0]}}\n\n{extra}\n{{\"type\":\"end\""),
    );
    std::fs::write(&a, &txt).unwrap();
    spec["sources"][0]["sha256"] = json!(sha(&a));
    let direct = scan_table("A", &a, true, &|_, _| true).unwrap();
    let cache = d.join("idx");
    let (ix, o1) = open_or_build("A", &a, Some(&cache)).unwrap();
    assert_eq!(o1.how, "built");
    assert_eq!(ix.meta.version, INDEX_VERSION);
    assert_eq!(ix.meta.file_sha256, direct.file_sha256);
    assert_eq!(ix.meta.rule_records, 2);
    let mut recs = vec![];
    let t = ix
        .scan_domains("A", &|_, _| true, &mut |r| {
            recs.push(r);
            Ok(())
        })
        .unwrap();
    assert_eq!(t.kinds, direct.kinds);
    assert_eq!(t.file_sha256, direct.file_sha256);
    assert_eq!(t.file_bytes, direct.file_bytes);
    assert_eq!(recs.len(), direct.records.len());
    for (x, y) in recs.iter().zip(&direct.records) {
        assert_eq!(
            (x.ordinal, x.line_no, x.offset, x.bytes, &x.sha256),
            (y.ordinal, y.line_no, y.offset, y.bytes, &y.sha256)
        );
        assert_eq!((&x.origin, x.origin_index, &x.domain_bounds), (&y.origin, y.origin_index, &y.domain_bounds));
        let mut yr = y.rec.clone();
        yr.rhs_keys = None;
        assert_eq!(x.rec, yr, "domain fields");
    }
    let mut bodies = vec![];
    ix.scan_bodies(&|_| true, &mut |o, s, k| {
        bodies.push((o, s.to_string(), k));
        Ok(())
    })
    .unwrap();
    for (b, y) in bodies.iter().zip(&direct.records) {
        assert_eq!((b.0, &b.1, Some(&b.2)), (y.ordinal, &y.sha256, y.rec.rhs_keys.as_ref()));
    }
    drop(ix);
    let (ix2, o2) = open_or_build("A", &a, Some(&cache)).unwrap();
    assert_eq!(o2.how, "reused");
    assert_eq!(ix2.meta.file_sha256, direct.file_sha256);
    drop(ix2);
    // the persistent-index load equals the temporary-index load
    let s = TableSetSpec::from_json(&spec.to_string()).unwrap();
    let opts = import::LoadOptions {
        index_dir: Some(cache.clone()),
    };
    let set1 = import::load_with(&s, &d, &opts).unwrap();
    assert_eq!(set1.report.sources[0].indexes[0].how, "reused");
    let set0 = load(&s, &d).unwrap();
    assert_eq!(set0.report.sources[0].indexes[0].how, "built_temporary");
    let ev = Evidence::from_spec(&set0, &d).unwrap();
    let r0 = check(&set0, &ev, &CheckOptions::default()).unwrap();
    let ev1 = Evidence::from_spec(&set1, &d).unwrap();
    let r1 = check(&set1, &ev1, &CheckOptions::default()).unwrap();
    assert_eq!(r0.key_set_digest, r1.key_set_digest);
    assert_eq!(r0.counts(), r1.counts());
    // a changed file (same size, other bytes and mtime) is indexed again
    std::thread::sleep(std::time::Duration::from_millis(20));
    std::fs::write(&a, txt.replace("\"origin_index\":4", "\"origin_index\":5")).unwrap();
    let (ix3, o3) = open_or_build("A", &a, Some(&cache)).unwrap();
    assert_eq!(o3.how, "built");
    assert_ne!(ix3.meta.file_sha256, direct.file_sha256);
}
