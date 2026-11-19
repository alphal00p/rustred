//! Lane arrivals: the arrival pipeline on a constructed TWO-NODE TOY DAG (nothing five-loop specific).
//!
//! Parent P: two loops, slots D1=(k1)^2, D2=(k2)^2, D3=(k1-q)^2, D4=(k2-q)^2 (lines), D5=(k1-k2)^2 (numerator).
//! Child C = P with line D3 removed, in its OWN table frame: D1=(k1)^2, D2=(k2)^2, D3=(k2-q)^2 (lines), D4=(k1-q)^2,
//! D5=(k1+k2)^2 (numerators); the route writes P's D3 as C's D4 and P's D5 = 2 D1 + 2 D2 - D5 of C.
//! P's rule 0 (on the plane n2 = 1) has hand-down terms onto C that are: zero on the whole locus (b), zero on part of
//! it (c: factor n0 - 2), never zero (e: d - 5), and one (f) whose image CANCELS e's on one child key (exact level).
//! The child's certified set misses arrivals; its table serves only keys with slot 0 >= 2: class (c) is reported.
//! Also: determinism over thread counts, the verifier accepts the honest file and refuses forged ones, the hand-down
//! prover's loci / non-zero proofs, the edge frame check. Needs the Symbolica licence (`source WS/ops/env.sh`); SKIPS
//! without it.

#[path = "../src/arrivals.rs"]
#[allow(dead_code)]
mod arrivals;
#[path = "../src/coef_facts.rs"]
#[allow(dead_code)]
mod coef_facts;
mod inputs;
#[path = "../src/zero_proofs.rs"]
#[allow(dead_code)]
mod zero_proofs;

use std::path::{Path, PathBuf};
use std::process::Command;

use rustred_campaign_algebra::SymbolicaAlgebra;
use rustred_campaign_core::accept::rule::RuleText;
use rustred_campaign_core::closure::CoefFact;
use rustred_campaign_core::import::arrivals::format;
use rustred_campaign_core::import::check::check_streamed;
use rustred_campaign_core::import::zero_proofs::Locus;
use rustred_campaign_core::import::{CheckOptions, Evidence, TableSetSpec, load};
use rustred_campaign_core::key::NodeFrame;
use rustred_campaign_core::record::TableHeader;
use serde_json::{Value, json};
use zero_proofs::{NzOutcome, ProverCfg, RecordProver};

const PFAM: &str = "I(name(P),loops(k1,k2),externals(q),dimension(d),prop(D1,(k1)^2,1),prop(D2,(k2)^2,1),\
prop(D3,(k1-q)^2,1),prop(D4,(k2-q)^2,1),prop(D5,(k1-k2)^2,0),gram(q,q,1))";
const CFAM: &str = "I(name(C),loops(k1,k2),externals(q),dimension(d),prop(D1,(k1)^2,1),prop(D2,(k2)^2,1),\
prop(D3,(k2-q)^2,1),prop(D4,(k1-q)^2,0),prop(D5,(k1+k2)^2,0),gram(q,q,1))";

fn alg() -> Option<SymbolicaAlgebra> {
    match SymbolicaAlgebra::new() {
        Ok(a) => Some(a),
        Err(e) => {
            inputs::missing(format_args!("SKIP: {e}"));
            None
        }
    }
}

fn scratch(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("arrivals-toy-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn header(name: &str, fam: &str, mask: &str) -> String {
    json!({"type":"header","schema":"table_dump.v1","family":name,"family_source":fam,"denominators":5,"mask":mask,
        "guard_version":2})
    .to_string()
}

fn rule(index: i64, fixed: [Option<i64>; 5], rhs: &[(&str, &str)], chart: &str) -> String {
    let t: Vec<String> = (0..5)
        .map(|i| fixed[i].map(|v| v.to_string()).unwrap_or(format!("n{i}")))
        .collect();
    json!({"type":"rule","chart":chart,"index":index,"fixed":fixed,"equations":[],"target":format!("I({})", t.join(",")),
        "rhs": rhs.iter().map(|(k, c)| json!([k, c])).collect::<Vec<_>>(),
        "exceptions":[],"dispatch":"Partition","witness_loci":[]})
    .to_string()
}

fn table(
    dir: &Path,
    name: &str,
    head: &str,
    chart: &str,
    guards: &[&str],
    rules: &[String],
) -> PathBuf {
    let p = dir.join(name);
    let mut s = format!("{head}\n");
    s.push_str(&json!({"type":"chart","chart":chart,"guards":guards,"zero":false}).to_string());
    s.push('\n');
    for r in rules {
        s.push_str(r);
        s.push('\n');
    }
    s.push_str("{\"type\":\"end\",\"charts\":1}\n");
    std::fs::write(&p, s).unwrap();
    p
}

fn sha(p: &Path) -> String {
    rustred_campaign_core::import::lines::file_sha256(p).unwrap()
}

fn parent_rules() -> Vec<String> {
    vec![
        // rule 0 on the plane n2 = 1 (first match)
        rule(
            0,
            [None, None, Some(1), None, None],
            &[
                ("I(n0-1,n1,1,n3,n4)", "(d-4)"),      // a: top (n0 >= 2)
                ("I(n0,n1,0,n3,n4)", "(n2-1)*(d-3)"), // b: hand-down, zero on the whole locus (n2 = 1)
                ("I(n0-1,n1,0,n3,n4-1)", "(n0-2)"),   // c: hand-down, zero where n0 = 2 only
                ("I(n0-1,n1,0,n3,n4)", "(d-5)"),      // e: hand-down, never zero
                ("I(n0,n1,0,n3,n4-1)", "(5-d)/2"),    // f: its image cancels e's on one child key
            ],
            "11110",
        ),
        // rule 1 elsewhere: descend in n2
        rule(
            1,
            [None, None, None, None, None],
            &[("I(n0,n1,n2-1,n3,n4)", "(d-4)")],
            "11110",
        ),
    ]
}

fn edge_json(parent_table: &Path, child_table: &Path) -> Value {
    let pres = |v: &[[i64; 3]]| {
        v.iter()
            .map(|m| json!({"line": m, "eps_units": 0}))
            .collect::<Vec<_>>()
    };
    json!({"schema": "handdown-routes.edge.v1", "edge": "P->C",
        "parent": {"label": "P", "table": parent_table, "slot_map_table_to_presentation": [0,1,2,3,4],
            "presentation_slots": pres(&[[1,0,0],[0,1,0],[1,0,-1],[0,1,-1],[1,-1,0]]), "table_lines": [0,1,2,3]},
        "child": {"label": "C", "table": child_table, "slot_map_table_to_presentation": [0,1,2,3,4],
            "presentation_slots": pres(&[[1,0,0],[0,1,0],[0,1,-1],[1,0,-1],[1,1,0]]), "table_lines": [0,1,2]},
        "owner": {"class": "toy"},
        "sectors": [
            {"fate": "other class", "parent_table_slots_dropped": [0]},
            {"fate": "routed onto the child's class (owner = child full sector)", "parent_table_slots_dropped": [2],
             "parent_table_lines_kept": [0,1,3], "class": "toy", "chosen_candidate_index": 0,
             "substitution": {"chosen": [{"line": 0}, {"line": 1}, {"form": {"3": "1"}, "q2": "0"}, {"line": 2},
                 {"form": {"0": "2", "1": "2", "4": "-1"}, "q2": "0"}]}}]})
}

struct Toy {
    dir: PathBuf,
    pspec: PathBuf,
    rows: PathBuf,
    edge: PathBuf,
    cspec: PathBuf,
    ctable: PathBuf,
}

fn build(tag: &str) -> Toy {
    let dir = scratch(tag);
    let pt = table(
        &dir,
        "P.table.jsonl",
        &header("P", PFAM, "11110"),
        "11110",
        &["n>=1", "n>=1", "n>=1", "n>=1", "n<=0"],
        &parent_rules(),
    );
    let ct = table(
        &dir,
        "C.table.jsonl",
        &header("C", CFAM, "11100"),
        "11100",
        &["n>=1", "n>=1", "n>=1", "n<=0", "n<=0"],
        // the child serves keys with slot 0 >= 2 only (slot 0 = 1 is uncovered: open)
        &[rule(
            0,
            [None, None, None, None, None],
            &[("I(n0-1,n1,n2,n3,n4)", "1")],
            "11100",
        )],
    );
    // the child's rule must not apply at slot 0 = 1: an exception branch n0 - 1 = 0
    let text = std::fs::read_to_string(&ct)
        .unwrap()
        .replace("\"exceptions\":[]", "\"exceptions\":[[\"n0-1\"]]");
    std::fs::write(&ct, text).unwrap();
    let mut start = String::new();
    for n0 in 1..=3 {
        for n1 in 1..=2 {
            for n2 in 1..=2 {
                for n4 in [0, -1] {
                    start.push_str(&format!("[{n0},{n1},{n2},1,{n4}]\n"));
                }
            }
        }
    }
    let sp = dir.join("start.jsonl");
    std::fs::write(&sp, start).unwrap();
    let pspec = dir.join("P.spec.json");
    std::fs::write(
        &pspec,
        json!({"schema":"rustred.campaign.import-spec.v1","node":"P","frame_from":"t","bits":6,
            "start":{"path": sp, "format":"keys_jsonl"},
            "sources":[{"id":"t","kind":"engine_table","path": pt, "sha256": sha(&pt)}]})
        .to_string(),
    )
    .unwrap();
    let cstart = dir.join("cstart.jsonl");
    std::fs::write(&cstart, "[2,1,1,0,0]\n").unwrap();
    let cspec = dir.join("C.spec.json");
    std::fs::write(
        &cspec,
        json!({"schema":"rustred.campaign.import-spec.v1","node":"C","frame_from":"t","bits":6,
            "start":{"path": cstart, "format":"keys_jsonl"},
            "sources":[{"id":"t","kind":"engine_table","path": ct, "sha256": sha(&ct)}]})
        .to_string(),
    )
    .unwrap();
    let edge = dir.join("P-C.edge.json");
    std::fs::write(&edge, edge_json(&pt, &ct).to_string()).unwrap();
    // the parent's keys-only closure rows (what the tool walks)
    let mut v: Value = serde_json::from_str(&std::fs::read_to_string(&pspec).unwrap()).unwrap();
    v["policies"] = json!({"witness_check": "off_diagnostic", "tier": "K"});
    let s = TableSetSpec::from_json(&v.to_string()).unwrap();
    let set = load(&s, &dir).unwrap();
    let ev = Evidence::from_spec(&set, &dir).unwrap();
    let rows = dir.join("rows_k.jsonl");
    let r = check_streamed(
        &set,
        &ev,
        &CheckOptions {
            threads: 2,
            ..Default::default()
        },
        &rows,
    )
    .unwrap();
    assert_eq!(
        r.summary.open_total,
        0,
        "{:?}",
        r.rows
            .iter()
            .filter(|x| x.open_status.is_some())
            .map(|x| (&x.key, &x.open_status))
            .collect::<Vec<_>>()
    );
    Toy {
        dir,
        pspec,
        rows,
        edge,
        cspec,
        ctable: ct,
    }
}

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_rr_arrivals")
}

fn run(args: &[&str]) -> (i32, Value) {
    let o = Command::new(bin()).args(args).output().unwrap();
    let text = String::from_utf8_lossy(&o.stdout).to_string();
    let last = text.lines().last().unwrap_or("{}").to_string();
    (
        o.status.code().unwrap_or(-1),
        serde_json::from_str(&last)
            .unwrap_or(json!({"raw": text, "stderr": String::from_utf8_lossy(&o.stderr)})),
    )
}

fn compute(t: &Toy, out: &Path, threads: &str) -> (i32, Value) {
    compute_x(t, out, threads, &[])
}

fn compute_x(t: &Toy, out: &Path, threads: &str, extra: &[&str]) -> (i32, Value) {
    let spill = out.join("spill");
    let child = format!("C={}", t.ctable.display());
    let mut args = vec![
        "compute",
        "--spec",
        t.pspec.to_str().unwrap(),
        "--rows",
        t.rows.to_str().unwrap(),
        "--edge",
        t.edge.to_str().unwrap(),
        "--out-dir",
        out.to_str().unwrap(),
        "--parent",
        "P",
        "--child-table",
        &child,
        "--edge-producer",
        "toy (test)",
        "--threads",
        threads,
        "--decisions-sample",
        "10",
        "--spill-dir",
        spill.to_str().unwrap(),
    ];
    args.extend_from_slice(extra);
    run(&args)
}

/// `compute` on another rows file (same toy).
fn compute_rows(t: &Toy, rows: &Path, out: &Path, extra: &[&str]) -> (i32, Value) {
    let child = format!("C={}", t.ctable.display());
    let mut args = vec![
        "compute",
        "--spec",
        t.pspec.to_str().unwrap(),
        "--rows",
        rows.to_str().unwrap(),
        "--edge",
        t.edge.to_str().unwrap(),
        "--out-dir",
        out.to_str().unwrap(),
        "--parent",
        "P",
        "--child-table",
        &child,
        "--edge-producer",
        "toy (test)",
        "--threads",
        "2",
    ];
    args.extend_from_slice(extra);
    run(&args)
}

#[test]
fn streamed_walk_reads_unsorted_rows_and_refuses_a_duplicate_key() {
    let Some(_a) = alg() else { return };
    let t = build("rows");
    let text = std::fs::read_to_string(&t.rows).unwrap();
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    assert!(lines.len() > 2);
    // reversed order: the compact reader sorts; the arrival lines are those of the in-memory walk on the sorted file
    let rev = t.dir.join("rows_rev.jsonl");
    let mut r: Vec<&str> = lines.clone();
    r.reverse();
    std::fs::write(&rev, r.join("\n") + "\n").unwrap();
    let (rc1, s1) = compute_rows(&t, &t.rows, &t.dir.join("o_mem"), &["--in-memory"]);
    assert_eq!(rc1, 0, "{s1}");
    let (rc2, s2) = compute_rows(&t, &rev, &t.dir.join("o_rev"), &[]);
    assert_eq!(rc2, 0, "{s2}");
    let a = format::read(&t.dir.join("o_mem").join("P-C.arrivals.jsonl")).unwrap();
    let b = format::read(&t.dir.join("o_rev").join("P-C.arrivals.jsonl")).unwrap();
    assert_eq!(a.lines, b.lines);
    assert_eq!(a.summary["lines_digest"], b.summary["lines_digest"]);
    assert_eq!(
        a.header["parent"]["closure_digest"],
        b.header["parent"]["closure_digest"]
    );
    // a key twice: refused
    let dup = t.dir.join("rows_dup.jsonl");
    std::fs::write(&dup, text.clone() + lines[0] + "\n").unwrap();
    let (rc3, s3) = compute_rows(&t, &dup, &t.dir.join("o_dup"), &[]);
    assert_ne!(rc3, 0, "a duplicate key must be refused: {s3}");
    let (rc4, s4) = compute_rows(&t, &dup, &t.dir.join("o_dup_mem"), &["--in-memory"]);
    assert_ne!(rc4, 0, "the in-memory reader refuses it too: {s4}");
}

#[test]
fn streamed_walk_equals_in_memory_walk() {
    let Some(_a) = alg() else { return };
    let t = build("stream");
    let mem = t.dir.join("mem");
    let (rc, sm) = compute_x(&t, &mem, "3", &["--in-memory"]);
    assert_eq!(rc, 0, "{sm}");
    assert!(sm.get("form").is_none(), "the in-memory form: {sm}");
    let files = [
        "P-C.arrivals.jsonl",
        "P.decisions.jsonl",
        "P.second-route.requests.jsonl",
        "P.handdown-keys.sample.json",
    ];
    // default streamed form; then a spill at every merge and every parsed record freed after each use
    for (tag, threads, extra) in [
        ("s1", "3", vec![]),
        (
            "s2",
            "1",
            vec!["--map-budget", "1", "--record-budget-mb", "0"],
        ),
        (
            "s3",
            "4",
            vec!["--map-budget", "1", "--record-budget-mb", "0"],
        ),
        (
            "s4",
            "3",
            vec![
                "--map-budget",
                "1",
                "--closure-chunk",
                "1",
                "--local-flush",
                "1",
            ],
        ),
    ] {
        let out = t.dir.join(tag);
        let (rc, ss) = compute_x(&t, &out, threads, &extra);
        assert_eq!(rc, 0, "{tag}: {ss}");
        assert_eq!(ss["form"], "streamed", "{tag}: {ss}");
        if extra.is_empty() {
            assert_eq!(ss["spills"], 0, "{tag}: {ss}");
        } else {
            assert!(ss["spills"].as_u64().unwrap() > 0, "{tag}: {ss}");
            let left = std::fs::read_dir(out.join("spill")).map_or(0, |d| d.count());
            assert_eq!(left, 0, "{tag}: spill runs removed");
        }
        for f in files {
            assert_eq!(
                std::fs::read(mem.join(f)).unwrap(),
                std::fs::read(out.join(f)).unwrap(),
                "{tag}: {f} differs from the in-memory walk"
            );
        }
        let strip = |v: &Value| {
            let mut v = v.clone();
            if let Some(o) = v.as_object_mut() {
                o.retain(|k, _| !k.starts_with("seconds"));
            }
            v
        };
        assert_eq!(strip(&sm["walk"]), strip(&ss["walk"]), "{tag}");
        assert_eq!(sm["files"][0]["sha256"], ss["files"][0]["sha256"], "{tag}");
        assert_eq!(
            sm["files"][0]["stats"]["exact_cancelled"], ss["files"][0]["stats"]["exact_cancelled"],
            "{tag}"
        );
    }
}

#[test]
fn toy_dag_arrivals_child_check_determinism_and_verifier() {
    let Some(_a) = alg() else { return };
    let t = build("main");
    let o1 = t.dir.join("out1");
    let (rc, s) = compute(&t, &o1, "3");
    assert_eq!(rc, 0, "{s}");
    let f = o1.join("P-C.arrivals.jsonl");
    let a = format::read(&f).unwrap();
    assert!(!a.lines.is_empty());
    // (b) zero on the whole locus: never a sender; (c) zero where n0 = 2: no sender with n0 = 2 by term 2
    let w = &s["walk"];
    assert!(w["by_structural_zero"].as_u64().unwrap() > 0, "{w}");
    assert!(w["by_structural_nonzero"].as_u64().unwrap() > 0, "{w}");
    assert!(
        w["by_key_exact"].as_u64().unwrap() > 0,
        "the partial term's zero keys are decided exactly: {w}"
    );
    for l in &a.lines {
        assert_ne!(
            l.first.term, 1,
            "the structurally zero term sends nothing: {l:?}"
        );
        if l.first.term == 2 {
            assert_ne!(l.first.parent_key[0], 2, "term c vanishes at n0 = 2: {l:?}");
        }
    }
    // e (term 3) sends from every rule-0 key with n0 >= 2; its key for u = (3,2,1,1,0) is (2,2,1,0,0)
    let k = a
        .lines
        .iter()
        .find(|l| l.key == vec![2, 2, 1, 0, 0])
        .expect("e's image arrives");
    assert_eq!(k.senders, 1, "{k:?}");
    // exact level: e and f cancel on that key at that parent key: support yes, exact no
    assert_eq!(k.exact_senders, Some(0), "{k:?}");
    let es = &s["files"][0]["stats"];
    assert!(es["exact_cancelled"].as_u64().unwrap() >= 1, "{es}");
    // frame checks recorded for both sides
    assert_eq!(s["frame_checks"].as_array().unwrap().len(), 2);
    // determinism: another thread count gives the same bytes
    let o2 = t.dir.join("out2");
    let (rc2, _) = compute(&t, &o2, "1");
    assert_eq!(rc2, 0);
    assert_eq!(
        std::fs::read(&f).unwrap(),
        std::fs::read(o2.join("P-C.arrivals.jsonl")).unwrap()
    );
    // the verifier accepts the honest file (full re-derivation, per-key decisions only)
    let (vr, vs) = run(&[
        "verify",
        "--spec",
        t.pspec.to_str().unwrap(),
        "--rows",
        t.rows.to_str().unwrap(),
        "--edge",
        t.edge.to_str().unwrap(),
        "--arrivals",
        f.to_str().unwrap(),
        "--child-table",
        &format!("C={}", t.ctable.display()),
    ]);
    assert_eq!(vr, 0, "{vs}");
    // ... and refuses a forged one: a key dropped (digest recomputed by the forger), a key added, a count changed
    let forge = |name: &str, edit: &dyn Fn(&mut Vec<format::ArrivalLine>)| -> (i32, Value) {
        let mut lines = a.lines.clone();
        edit(&mut lines);
        let p = t.dir.join(name);
        format::write(&p, &a.header, &lines, &json!({})).unwrap();
        run(&[
            "verify",
            "--spec",
            t.pspec.to_str().unwrap(),
            "--rows",
            t.rows.to_str().unwrap(),
            "--edge",
            t.edge.to_str().unwrap(),
            "--arrivals",
            p.to_str().unwrap(),
            "--child-table",
            &format!("C={}", t.ctable.display()),
        ])
    };
    let (r1, s1) = forge("forged-drop.jsonl", &|l| {
        l.remove(0);
    });
    assert_eq!(r1, 3, "{s1}");
    let (r2, s2) = forge("forged-add.jsonl", &|l| {
        let mut x = l[0].clone();
        x.key = vec![9, 9, 9, 0, 0];
        l.push(x);
    });
    assert_eq!(r2, 3, "{s2}");
    let (r3, s3) = forge("forged-count.jsonl", &|l| l[0].senders += 1);
    assert_eq!(r3, 3, "{s3}");
    // child-side check: certified set = two arrivals; the rest is (b) or (c); slot-0 = 1 keys are not served
    let cert = t.dir.join("C.statement.jsonl");
    let certified: Vec<&format::ArrivalLine> = a.lines.iter().take(2).collect();
    std::fs::write(
        &cert,
        certified
            .iter()
            .map(|l| {
                format!(
                    "[{:?}, true, \"x\", null, 0]\n",
                    format!(
                        "I({})",
                        l.key
                            .iter()
                            .map(|v| v.to_string())
                            .collect::<Vec<_>>()
                            .join(",")
                    )
                )
            })
            .collect::<String>(),
    )
    .unwrap();
    let rep = t.dir.join("child.json");
    let work = t.dir.join("work");
    std::fs::create_dir_all(&work).unwrap();
    let (cr, cs) = run(&[
        "child-check",
        "--spec",
        t.cspec.to_str().unwrap(),
        "--statement",
        cert.to_str().unwrap(),
        "--arrivals",
        &format!("P={}", f.display()),
        "--out",
        rep.to_str().unwrap(),
        "--work-dir",
        work.to_str().unwrap(),
    ]);
    assert_eq!(cr, 0, "{cs}");
    let p = &cs["parents"][0];
    assert_eq!(p["a_in_certified_set"], 2, "{p}");
    let c = p["c_not_served"].as_u64().unwrap();
    assert!(c > 0, "(c) reported: {p}");
    for x in p["c_first50"].as_array().unwrap() {
        assert_eq!(x["key"][0], 1, "only slot-0 = 1 keys are unserved: {x}");
    }
    assert_eq!(
        p["a_in_certified_set"].as_u64().unwrap() + p["b_served_outside"].as_u64().unwrap() + c,
        a.lines.len() as u64
    );
    // the child-side SERVICE SET as a pinned manifest: the same verdicts; a wrong pin is refused (fail closed)
    let man = t.dir.join("C.services.json");
    let pin = |sha: &str| {
        json!({"schema": "rustred.campaign.arrival-services.v1", "node": "C", "name": "toy child services",
            "spec": {"path": t.cspec, "sha256": sha}, "closed_sets": []})
        .to_string()
    };
    std::fs::write(&man, pin(&sha(&t.cspec))).unwrap();
    let rep2 = t.dir.join("child2.json");
    let (mr, ms) = run(&[
        "child-check",
        "--services",
        man.to_str().unwrap(),
        "--statement",
        cert.to_str().unwrap(),
        "--arrivals",
        &format!("P={}", f.display()),
        "--out",
        rep2.to_str().unwrap(),
        "--work-dir",
        work.to_str().unwrap(),
    ]);
    assert_eq!(mr, 0, "{ms}");
    assert_eq!(ms["parents"][0]["c_not_served"], p["c_not_served"]);
    assert_eq!(ms["services"]["name"], "toy child services");
    std::fs::write(&man, pin(&"0".repeat(64))).unwrap();
    let (br, bs) = run(&[
        "child-check",
        "--services",
        man.to_str().unwrap(),
        "--statement",
        cert.to_str().unwrap(),
        "--arrivals",
        &format!("P={}", f.display()),
        "--out",
        rep2.to_str().unwrap(),
        "--work-dir",
        work.to_str().unwrap(),
    ]);
    assert_eq!(br, 2, "a wrong pin must be refused: {bs}");
    // a closed set serves its keys: every (c) key put into a closed key list -> nothing unserved
    let cl = t.dir.join("closed.keys.jsonl");
    let c_keys: Vec<String> = p["c_first50"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["key"].to_string())
        .collect();
    std::fs::write(&cl, c_keys.join("\n") + "\n").unwrap();
    let (cr2, cs2) = run(&[
        "child-check",
        "--spec",
        t.cspec.to_str().unwrap(),
        "--statement",
        cert.to_str().unwrap(),
        "--arrivals",
        &format!("P={}", f.display()),
        "--closed-set",
        &format!("toy={}", cl.display()),
        "--out",
        rep2.to_str().unwrap(),
        "--work-dir",
        work.to_str().unwrap(),
    ]);
    assert_eq!(cr2, 0, "{cs2}");
    let q = &cs2["parents"][0];
    if c as usize <= 50 {
        assert_eq!(q["c_not_served"], 0, "{q}");
        assert_eq!(q["b_in_closed_sets"]["toy"].as_u64().unwrap(), c);
    }
    let _ = std::fs::remove_dir_all(&t.dir);
}

fn prover_rule() -> RuleText {
    let h = TableHeader::from_value(serde_json::from_str(&header("P", PFAM, "11110")).unwrap())
        .unwrap();
    let frame = NodeFrame::from_header(&h).unwrap();
    // a general rule (no fixed slot): its hand-down loci are planes of n2
    let line = rule(
        5,
        [None, None, None, None, None],
        &[
            ("I(n0,n1,n2-1,n3,n4)", "(n2-1)*(d-3)"), // 0: zero on the plane n2 = 1
            ("I(n0,n1,n2-2,n3,n4)", "(d-5)*(n0+1)"), // 1: never zero (planes n2 = 1, 2)
            ("I(n0,n1,n2-1,n3,n4-1)", "(n0-2)*(d-1)"), // 2: zero only where n0 = 2
            ("I(n0-1,n1,n2,n3,n4)", "1"),            // 3: hand-down only where n0 = 1 (other mask)
        ],
        "11110",
    );
    RuleText::parse(line.as_bytes(), &frame).unwrap()
}

#[test]
fn handdown_loci_and_nonzero_proofs() {
    let Some(a) = alg() else { return };
    let rt = prover_rule();
    let lines = [true, true, true, true, false];
    let mut p = RecordProver::new(&a, ProverCfg::default(), &rt, &lines).unwrap();
    let pl = |v| Locus::Plane { param: 2, value: v };
    assert_eq!(p.handdown_loci(0).0, vec![pl(1)]);
    assert_eq!(p.handdown_loci(1).0, vec![pl(1), pl(2)]);
    assert_eq!(
        p.handdown_loci(3).0,
        vec![Locus::Plane { param: 0, value: 1 }]
    );
    assert!(matches!(
        p.prove(0, pl(1)),
        zero_proofs::Outcome::Proved { .. }
    ));
    assert!(
        matches!(
            p.prove_nonzero(1, pl(1)),
            NzOutcome::NonZero {
                class: "factors",
                ..
            }
        ),
        "{:?}",
        p.prove_nonzero(1, pl(1))
    );
    assert!(matches!(
        p.prove_nonzero(2, pl(1)),
        NzOutcome::NotDecided(_)
    ));
    // structural words agree with the exact per-key evaluation at every key of a grid
    for n0 in 1..=4 {
        for n2 in 1..=3 {
            for n4 in -2..=0 {
                let k = vec![n0, 1, n2, 1, n4];
                let pt = p.point(&k).unwrap();
                for j in 0..4 {
                    let tk = p.term_key(j, &pt).unwrap();
                    if !tk.iter().take(4).any(|&x| x <= 0) {
                        continue;
                    }
                    let exact = p.word_at(j, &pt);
                    let (w, by) = p.handdown_word(j, &pt, &tk);
                    assert_eq!(w, exact, "term {j} at {k:?} by {by}");
                    if j == 2 && n2 == 1 {
                        assert_eq!(w == CoefFact::Zero, n0 == 2);
                    }
                }
            }
        }
    }
    let mut c = zero_proofs::HandDownCounts::default();
    let lines_out = zero_proofs::prove_handdown_record(&mut p, &mut c, false);
    assert_eq!(c.handdown_terms, 4);
    assert!(c.zero >= 1 && c.nonzero >= 1 && c.per_key >= 1, "{c:?}");
    assert!(lines_out.iter().any(|v| v["verdict"] == "nonzero_on_locus"));
    assert!(
        lines_out
            .iter()
            .any(|v| v["class"] == "hand_down" && v["verdict"] == "zero_on_locus")
    );
}

#[test]
fn truncated_expansion_equals_the_top_sector_part_of_the_full_one() {
    let Some(a) = alg() else { return };
    let a: &'static SymbolicaAlgebra = Box::leak(Box::new(a));
    let e = rustred_campaign_core::import::arrivals::edge::EdgeFile::parse(
        edge_json(Path::new("p"), Path::new("c"))
            .to_string()
            .as_bytes(),
        "toy",
        "chosen",
    )
    .unwrap();
    let sec = &e.sectors[0];
    let tr = arrivals::Transporter::new(a, 5).unwrap();
    let mut seen_lower = 0;
    for n0 in 1..=4 {
        for n1 in 1..=3 {
            for n3 in 1..=2 {
                for d2 in [0, -1, -2] {
                    for n4 in [0, -1, -2, -3] {
                        let key = [n0, n1, d2, n3, n4];
                        let full = tr.apply(sec, &key).unwrap();
                        let top: Vec<_> = full
                            .iter()
                            .filter(|(k, _)| k[..3].iter().all(|&x| x >= 1))
                            .cloned()
                            .collect();
                        seen_lower += full.len() - top.len();
                        assert_eq!(tr.apply_top(sec, &key, &[0, 1, 2]).unwrap(), top, "{key:?}");
                    }
                }
            }
        }
    }
    assert!(seen_lower > 0, "the grid must exercise the pruning");
}

/// The edge bound behind the arrival cones (core `import::arrivals::cone`, section 2): every top-sector image of a parent
/// key of a routed sector has child D + r <= D + r(sub) of the parent key (dropped lines counted as numerators), on a grid;
/// `edge_image` says the bound holds structurally for this sector.
#[test]
fn edge_bound_child_dr_never_exceeds_the_parent_terms_sub_sector_dr() {
    let Some(a) = alg() else { return };
    let a: &'static SymbolicaAlgebra = Box::leak(Box::new(a));
    let e = rustred_campaign_core::import::arrivals::edge::EdgeFile::parse(
        edge_json(Path::new("p"), Path::new("c"))
            .to_string()
            .as_bytes(),
        "toy",
        "chosen",
    )
    .unwrap();
    let im = rustred_campaign_core::import::arrivals::cone::edge_image(&e);
    assert!(im.iter().all(|s| s.bound_holds), "{im:?}");
    let sec = &e.sectors[0];
    let plines = [true, true, true, true, false];
    let clines: Vec<bool> = (0..e.child.momenta.len())
        .map(|j| e.child.table_lines.contains(&j))
        .collect();
    let dr = |k: &[i32], lines: &[bool]| -> i64 {
        k.iter()
            .zip(lines)
            .map(|(&v, &l)| {
                let v = i64::from(v);
                if l && v >= 1 { v - 1 } else { -v }
            })
            .sum()
    };
    let tr = arrivals::Transporter::new(a, 5).unwrap();
    let (mut images, mut attained) = (0u64, 0u64);
    for n0 in 1..=4 {
        for n1 in 1..=3 {
            for n3 in 1..=2 {
                for d2 in [0, -1, -2] {
                    for n4 in [0, -1, -2, -3] {
                        let key = [n0, n1, d2, n3, n4];
                        let bound = dr(&key, &plines);
                        for (k, _) in tr.apply_top(sec, &key, &[0, 1, 2]).unwrap() {
                            images += 1;
                            let c = dr(&k, &clines);
                            assert!(c <= bound, "{key:?} -> {k:?}: {c} > {bound}");
                            attained += (c == bound) as u64;
                        }
                    }
                }
            }
        }
    }
    assert!(
        images > 0 && attained > 0,
        "the bound is attained on the grid ({attained} of {images})"
    );
}

/// CHAIN DEPTH (`closure-rows --arrivals` + `compute --depth`): box keys start at depth 0, an arriving key at the minimum
/// depth of its arrival lines (a box key keeps 0); a closure key inherits the least start depth reaching it, an arrival
/// gets min over its senders of (sender depth + 1). The keys, senders and first senders equal those of the untracked walk;
/// `--trace-key` gives a closure path back to a start key; an arrival file without depths is refused under `--depth`.
#[test]
fn chain_depth_layers_box_and_arrival_starts() {
    let Some(_a) = alg() else { return };
    let t = build("depth");
    let base = t.dir.join("base");
    let (rc, s) = compute_x(&t, &base, "2", &[]);
    assert_eq!(rc, 0, "{s}");
    let src = format::read(&base.join("P-C.arrivals.jsonl")).unwrap();
    let l0 = src.lines[0].clone();
    // arrivals INTO P (P's frame): [1,1,1,1,0] is a box key (depth stays 0); [4,1,1,1,0] lies outside P's box (depth 3)
    let mk = |key: Vec<i32>, d: Option<u32>| format::ArrivalLine {
        key,
        depth: d,
        ..l0.clone()
    };
    let write = |name: &str, lines: Vec<format::ArrivalLine>| {
        let p = t.dir.join(name);
        format::write_stream(&p, &src.header, &mut lines.into_iter().map(Ok), &json!({})).unwrap();
        p
    };
    let arr = write(
        "into-P.arrivals.jsonl",
        vec![
            mk(vec![1, 1, 1, 1, 0], Some(2)),
            mk(vec![4, 1, 1, 1, 0], Some(3)),
        ],
    );
    let rows = t.dir.join("rows_arr.jsonl");
    let (rc, s) = run(&[
        "closure-rows",
        "--spec",
        t.pspec.to_str().unwrap(),
        "--arrivals",
        arr.to_str().unwrap(),
        "--out",
        rows.to_str().unwrap(),
        "--threads",
        "2",
    ]);
    assert_eq!(rc, 0, "{s}");
    let (rc, plain) = compute_rows(&t, &rows, &t.dir.join("o_plain"), &[]);
    assert_eq!(rc, 0, "{plain}");
    let (rc, dep) = compute_rows(
        &t,
        &rows,
        &t.dir.join("o_dep"),
        &[
            "--depth",
            "--arrivals",
            arr.to_str().unwrap(),
            "--trace-key",
            "[4,1,1,1,0]",
            "--trace-key",
            "[2,1,1,1,0]",
        ],
    );
    assert_eq!(rc, 0, "{dep}");
    let a = format::read(&t.dir.join("o_plain").join("P-C.arrivals.jsonl")).unwrap();
    let b = format::read(&t.dir.join("o_dep").join("P-C.arrivals.jsonl")).unwrap();
    assert_eq!(a.lines.len(), b.lines.len());
    let mut depths = std::collections::BTreeSet::new();
    for (x, y) in a.lines.iter().zip(&b.lines) {
        assert_eq!((&x.key, x.senders, &x.first), (&y.key, y.senders, &y.first));
        assert_eq!(x.depth, None);
        depths.insert(y.depth.expect("a depth on every tracked line"));
    }
    // box-reached senders give depth 1; keys sent only from [4,1,1,1,0] (n0 - 1 = 3 on the hand-down terms) depth 4
    assert_eq!(depths, [1, 4].into_iter().collect(), "{dep}");
    assert!(
        b.lines
            .iter()
            .filter(|l| l.depth == Some(4))
            .all(|l| l.first.parent_key == vec![4, 1, 1, 1, 0])
    );
    // the closure histogram: every closure key at depth 0 except the arriving key [4,1,1,1,0] at depth 3
    let hist = &dep["walk"]["depth_hist"];
    assert_eq!(hist["3"], 1, "{dep}");
    assert_eq!(
        hist["0"].as_u64().unwrap() + 1,
        dep["walk"]["closure_keys"].as_u64().unwrap(),
        "{dep}"
    );
    assert!(plain["walk"].get("depth_hist").is_none(), "{plain}");
    // traces: the arriving key is a start key of depth 3; [2,1,1,1,0] is a box start key
    let tr = dep["trace"].as_array().unwrap();
    assert_eq!(tr[0]["path"][0]["start"], true, "{dep}");
    assert_eq!(tr[0]["depth"], 3, "{dep}");
    assert_eq!(tr[1]["depth"], 0, "{dep}");
    // --closure-only: no edge file
    let (rc, co) = compute_rows(
        &t,
        &rows,
        &t.dir.join("o_co"),
        &[
            "--depth",
            "--arrivals",
            arr.to_str().unwrap(),
            "--closure-only",
        ],
    );
    assert_eq!(rc, 0, "{co}");
    assert_eq!(co["files"].as_array().unwrap().len(), 0, "{co}");
    assert_eq!(co["walk"]["depth_hist"], *hist, "{co}");
    // an arrival file without depths: refused under --depth
    let nodepth = write(
        "into-P-nodepth.arrivals.jsonl",
        vec![mk(vec![4, 1, 1, 1, 0], None)],
    );
    let (rc, s) = compute_rows(
        &t,
        &rows,
        &t.dir.join("o_nd"),
        &["--depth", "--arrivals", nodepth.to_str().unwrap()],
    );
    assert_ne!(rc, 0, "an arrival file without depths must be refused: {s}");
}

/// `--structural-min-keys N`: records serving fewer than N closure keys decide their hand-down terms per key (exact
/// evaluation) instead of by structural loci proofs: the same arrival keys, sender counts and first senders' keys / terms
/// (only the decision provenance `by` may differ); the header records N.
#[test]
fn structural_min_keys_changes_only_the_provenance() {
    let Some(_a) = alg() else { return };
    let t = build("smk");
    let (rc, a) = compute_x(&t, &t.dir.join("o_s"), "2", &[]);
    assert_eq!(rc, 0, "{a}");
    let (rc, b) = compute_x(
        &t,
        &t.dir.join("o_k"),
        "2",
        &["--structural-min-keys", "1000000"],
    );
    assert_eq!(rc, 0, "{b}");
    let fa = format::read(&t.dir.join("o_s").join("P-C.arrivals.jsonl")).unwrap();
    let fb = format::read(&t.dir.join("o_k").join("P-C.arrivals.jsonl")).unwrap();
    assert_eq!(fa.lines.len(), fb.lines.len());
    for (x, y) in fa.lines.iter().zip(&fb.lines) {
        assert_eq!((&x.key, x.senders), (&y.key, y.senders));
        assert_eq!(
            (&x.first.parent_key, x.first.term, &x.first.term_key),
            (&y.first.parent_key, y.first.term, &y.first.term_key)
        );
    }
    assert_eq!(fb.header["decision"]["structural_min_keys"], 1000000);
    assert!(fa.header["decision"].get("structural_min_keys").is_none());
    // every term decided per key under the threshold
    assert_eq!(b["walk"]["by_structural_zero"], 0, "{b}");
    assert_eq!(b["walk"]["by_structural_nonzero"], 0, "{b}");
}

/// `--closure-out F` (with `--closure-only`): every reached key once, in key order, with its depth and row kind; the counts agree
/// with the walk's closure size and depth histogram; the uncovered list holds exactly the kind-2 records.
#[test]
fn closure_out_lists_the_exact_closure_with_depth_and_kind() {
    let Some(_a) = alg() else { return };
    let t = build("clout");
    let out = t.dir.join("o_cl");
    let f = t.dir.join("closure.keys.bin");
    let (rc, s) = compute_rows(
        &t,
        &t.rows,
        &out,
        &[
            "--depth",
            "--closure-only",
            "--closure-out",
            f.to_str().unwrap(),
        ],
    );
    assert_eq!(rc, 0, "{s}");
    let b = std::fs::read(&f).unwrap();
    assert_eq!(&b[..8], b"RRKEYS01");
    let hl = u32::from_le_bytes(b[8..12].try_into().unwrap()) as usize;
    let hdr: Value = serde_json::from_slice(&b[12..12 + hl]).unwrap();
    let n = hdr["arity"].as_u64().unwrap() as usize;
    let recs: Vec<&[u8]> = b[12 + hl..].chunks(n + 2).collect();
    assert_eq!(
        recs.len() as u64,
        s["walk"]["closure_keys"].as_u64().unwrap(),
        "{s}"
    );
    assert!(recs.windows(2).all(|w| {
        let k = |r: &[u8]| r[..n].iter().map(|&x| x as i8).collect::<Vec<i8>>();
        k(w[0]) < k(w[1])
    }));
    let kinds: Vec<u8> = recs.iter().map(|r| r[n + 1]).collect();
    let unc = kinds.iter().filter(|&&k| k == 2).count();
    let ul = std::fs::read_to_string(format!("{}.uncovered.jsonl", f.display())).unwrap();
    assert_eq!(ul.lines().count(), unc);
    assert_eq!(s["closure_out"]["uncovered_keys"], unc as u64, "{s}");
    let d0 = recs.iter().filter(|r| r[n] == 0).count() as u64;
    assert_eq!(s["walk"]["depth_hist"]["0"].as_u64().unwrap(), d0, "{s}");
    // first match: one line per serving record, keys summing to the covered closure keys
    let fm = std::fs::read_to_string(format!("{}.first_match.jsonl", f.display())).unwrap();
    let total: u64 = fm
        .lines()
        .map(|l| {
            serde_json::from_str::<Value>(l).unwrap()["keys"]
                .as_u64()
                .unwrap()
        })
        .sum();
    assert_eq!(
        total,
        kinds.iter().filter(|&&k| k == 0).count() as u64,
        "{fm}"
    );
    assert_eq!(
        s["closure_out"]["records_used"].as_u64().unwrap() as usize,
        fm.lines().count()
    );
}

/// Merged shard line: senders, depth, least first sender (parent key, term).
type Merged = (u64, Option<u32>, (Vec<i32>, u32));

/// `--shard I/K`: the shards' arrival files merged (keys union, senders summed, depth min, least first sender by (parent key,
/// term)) are exactly the unsharded walk's.
#[test]
fn shards_merged_equal_the_unsharded_walk() {
    let Some(_a) = alg() else { return };
    let t = build("shard");
    let (rc, s) = compute_rows(&t, &t.rows, &t.dir.join("o_full"), &["--depth"]);
    assert_eq!(rc, 0, "{s}");
    let full = format::read(&t.dir.join("o_full").join("P-C.arrivals.jsonl")).unwrap();
    let mut merged: std::collections::BTreeMap<Vec<i32>, Merged> = Default::default();
    for i in 0..3 {
        let out = t.dir.join(format!("o_s{i}"));
        let sh = format!("{i}/3");
        let (rc, s) = compute_rows(&t, &t.rows, &out, &["--depth", "--shard", &sh]);
        assert_eq!(rc, 0, "{s}");
        let f = format::read(&out.join("P-C.arrivals.jsonl")).unwrap();
        assert_eq!(f.header["shard"], sh.as_str());
        for l in f.lines {
            let fk = (l.first.parent_key.clone(), l.first.term);
            let e = merged
                .entry(l.key.clone())
                .or_insert((0, l.depth, fk.clone()));
            e.0 += l.senders;
            e.1 = e.1.min(l.depth);
            if fk < e.2 {
                e.2 = fk;
            }
        }
    }
    assert_eq!(merged.len(), full.lines.len());
    for l in &full.lines {
        let m = &merged[&l.key];
        assert_eq!((m.0, m.1), (l.senders, l.depth), "{:?}", l.key);
        assert_eq!(
            m.2,
            (l.first.parent_key.clone(), l.first.term),
            "{:?}",
            l.key
        );
    }
}

/// `arrivals-bin` (full and `--fast`): the key file of an arrival file; `closure-rows` and `compute --depth` accept it in place of
/// the arrival file with the same rows and the same walk.
#[test]
fn key_files_stand_in_for_arrival_files() {
    let Some(_a) = alg() else { return };
    let t = build("keybin");
    let (rc, s) = compute_x(&t, &t.dir.join("base"), "2", &[]);
    assert_eq!(rc, 0, "{s}");
    let src = format::read(&t.dir.join("base").join("P-C.arrivals.jsonl")).unwrap();
    let l0 = src.lines[0].clone();
    let mk = |key: Vec<i32>, d: u32| format::ArrivalLine {
        key,
        depth: Some(d),
        ..l0.clone()
    };
    let arr = t.dir.join("into-P.arrivals.jsonl");
    let lines = vec![mk(vec![1, 1, 1, 1, 0], 2), mk(vec![4, 1, 1, 1, 0], 3)];
    format::write_stream(
        &arr,
        &src.header,
        &mut lines.into_iter().map(Ok),
        &json!({}),
    )
    .unwrap();
    let kb = t.dir.join("into-P.keys.bin");
    let kf = t.dir.join("into-P.fast.keys.bin");
    let (rc, s1) = run(&[
        "arrivals-bin",
        "--arrivals",
        arr.to_str().unwrap(),
        "--out",
        kb.to_str().unwrap(),
    ]);
    assert_eq!(rc, 0, "{s1}");
    let (rc, s2) = run(&[
        "arrivals-bin",
        "--fast",
        "--arrivals",
        arr.to_str().unwrap(),
        "--out",
        kf.to_str().unwrap(),
    ]);
    assert_eq!(rc, 0, "{s2}");
    let body = |p: &Path| {
        let b = std::fs::read(p).unwrap();
        let hl = u32::from_le_bytes(b[8..12].try_into().unwrap()) as usize;
        b[12 + hl..].to_vec()
    };
    assert_eq!(body(&kb), body(&kf));
    assert_eq!(s1["keys"], 2);
    let mut outs = vec![];
    for (tag, input) in [("j", &arr), ("b", &kb)] {
        let rows = t.dir.join(format!("rows_{tag}.jsonl"));
        let (rc, s) = run(&[
            "closure-rows",
            "--spec",
            t.pspec.to_str().unwrap(),
            "--arrivals",
            input.to_str().unwrap(),
            "--out",
            rows.to_str().unwrap(),
            "--threads",
            "2",
        ]);
        assert_eq!(rc, 0, "{s}");
        let (rc, c) = compute_rows(
            &t,
            &rows,
            &t.dir.join(format!("o_{tag}")),
            &["--depth", "--arrivals", input.to_str().unwrap()],
        );
        assert_eq!(rc, 0, "{c}");
        outs.push((
            std::fs::read(&rows).unwrap(),
            c["walk"]["depth_hist"].clone(),
            c["files"][0]["keys"].clone(),
        ));
    }
    assert_eq!(outs[0], outs[1]);
}
