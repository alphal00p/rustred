//! Lane audit-closeloop (INDEPENDENT AUDIT of the closing loop, supervisor side). Constructed toy nodes (the TOY of
//! closeloop_support: arity 3, nothing five-loop specific) and fake children attacking the closing loop's verdict path:
//! evidence pinning across passes / resume (F-EVPIN), the exit code's source (F-OUT), the statement's binding to the
//! ledger, a WELL-BOUND lying coefficient-facts producer (the trust boundary of the zero-term rule), and SIGKILL at
//! random points through the exact-evaluation phase with double resume.
//!
//! The process tests spawn the supervisor and fake children and send signals: run ONLY inside a private pid namespace
//! (`LIGHT_PIDNS=1 .../light_build.sh audit-closeloop test -p rustred-campaign --test audit_closeloop_sup`). Tests that
//! assert the CORRECT behaviour and fail on the audited code (0ceb9e79) are `#[ignore]`d with the finding id.

mod closeloop_support;
mod sup_common;

use closeloop_support::*;
use rustred_campaign::closeloop::pass::{PassIn, run_pass};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Duration;
use sup_common::*;

fn sha(s: &str) -> String {
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    h.update(s.as_bytes());
    h.finish_hex()
}

fn result_json(cdir: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(cdir.join("closeloop/toy/final/result.json")).unwrap())
        .unwrap()
}

fn statement_keys(cdir: &Path) -> BTreeSet<Vec<i32>> {
    std::fs::read_to_string(cdir.join("closeloop/toy/final/statement/statement.jsonl"))
        .unwrap()
        .lines()
        .map(|l| {
            serde_json::from_value(serde_json::from_str::<Value>(l).unwrap()["key"].clone())
                .unwrap()
        })
        .collect()
}

/// FINDING F-EVPIN (major), FIXED by lane closeloop4 (closeloop.md 22, audit M1): spec evidence is read only PINNED. A
/// directory without a manifest, a directory with a bare sha256, an unpinned file are refused at load; with a pinned
/// manifest the honest pass runs, and a fact line flipped in the directory afterwards REFUSES the pass (digest
/// mismatch against the manifest) instead of changing its verdict.
#[test]
fn f_evpin_unpinned_evidence_is_refused_and_a_changed_pinned_file_refuses_the_pass() {
    let t = Tmp::new("acevpin");
    let cfg = Toy {
        second_face: None,
        ..Toy::default()
    };
    let files = toy(&t.path.join("m"), &cfg, FAKE);
    std::fs::write(&files.hand, "").unwrap();
    let dir = files.dir.join("facts.d");
    std::fs::create_dir_all(&dir).unwrap();
    let honest = std::fs::read_to_string(files.dir.join("facts.jsonl")).unwrap();
    std::fs::write(dir.join("f.jsonl"), &honest).unwrap();
    let mut spec: Value =
        serde_json::from_slice(&std::fs::read(&files.import_spec).unwrap()).unwrap();
    let refused = |facts: Value| {
        let mut sp = spec.clone();
        sp["evidence"]["facts"] = facts;
        let ps = rustred_campaign_core::import::TableSetSpec::from_json(&sp.to_string()).unwrap();
        let set = rustred_campaign_core::import::load(&ps, &files.dir).unwrap();
        rustred_campaign_core::import::Evidence::from_spec(&set, &files.dir).is_err()
    };
    let d = dir.display().to_string();
    assert!(
        refused(json!([{"path": d, "family": "TOY"}])),
        "a directory without a manifest"
    );
    assert!(
        refused(json!([{"path": d, "sha256": "00", "family": "TOY"}])),
        "a directory with a sha256"
    );
    assert!(
        refused(json!([{"path": dir.join("f.jsonl").display().to_string(), "family": "TOY"}])),
        "an unpinned file"
    );
    // the manifest (SHA256SUMS), pinned in the spec
    let man = files.dir.join("facts.SHA256SUMS");
    std::fs::write(
        &man,
        format!("{}  f.jsonl\n", sha256_file(&dir.join("f.jsonl"))),
    )
    .unwrap();
    spec["evidence"]["facts"] = json!([{"path": d, "family": "TOY",
        "manifest": {"path": man.display().to_string(), "sha256": sha256_file(&man)}}]);
    let spec_text = serde_json::to_string_pretty(&spec).unwrap();
    let pass = |tag: &str| {
        run_pass(
            &PassIn {
                threads: 2,
                max_keys_per_job: 2,
                relation_rule: "first_admissible".into(),
                exact_tier: false,
                rows_path: t.path.join(format!("{tag}.rows.jsonl")),
                ..PassIn::first(
                    "toy",
                    spec_text.clone(),
                    files.dir.clone(),
                    t.path.join(format!("final-{tag}")),
                )
            },
            None,
        )
    };
    let a = pass("honest").expect("the pinned honest pass runs");
    assert!(
        a.final_.is_none(),
        "honest round 1 needs solves: {}",
        a.check
    );
    // an unlisted file in the directory is refused
    std::fs::write(dir.join("extra.jsonl"), &honest).unwrap();
    assert!(pass("extra").is_err(), "an unlisted evidence file was read");
    std::fs::remove_file(dir.join("extra.jsonl")).unwrap();
    // flip ONE term to zero in the pinned file: the pass is refused
    let mut v: Value = serde_json::from_str(honest.trim()).unwrap();
    v["rhs"][0]["coef"] = json!("zero");
    std::fs::write(dir.join("f.jsonl"), format!("{v}\n")).unwrap();
    let e = pass("lie")
        .map(|_| ())
        .expect_err("a changed pinned evidence file was read");
    assert!(
        e.contains("evidence") || e.contains("sha256") || e.contains("digest"),
        "{e}"
    );
}

fn open_toy(tag: &str) -> (Tmp, PathBuf, PathBuf) {
    // no default recipe: [1,1,0] (first met while closing, outside R1's bound) stays open: verdict open, exit 10
    let t = Tmp::new(tag);
    let cfg = Toy {
        default_recipe: false,
        ..Toy::default()
    };
    let files = toy(&t.path.join("material"), &cfg, FAKE);
    std::fs::write(&files.hand, "").unwrap();
    let spec = campaign_spec(&t.campaign, &files, FAKE, &cpus());
    let sp = write_spec(&t, "spec.json", &spec);
    let cdir = t.cdir("cdir");
    (t, sp, cdir)
}

fn tamper_to_closed(cdir: &Path) {
    let p = cdir.join("closeloop/toy/final/result.json");
    let mut v: Value = serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
    assert_ne!(v["verdict"], "closed");
    v["verdict"] = json!("closed");
    std::fs::write(&p, serde_json::to_vec(&v).unwrap()).unwrap();
}

/// F-OUT, the correct behaviour: a result file that no longer matches its recorded BLAKE3 is an error (11), never 0.
#[test]
fn f_out_a_tampered_result_file_must_not_exit_0() {
    let _g = serial();
    let (_t, sp, cdir) = open_toy("acout2");
    // lane closeloop4: the uncovered key is a terminal proposal (exit 8): the operator rejects it -> open, exit 10
    let mut c = start(&sp, &cdir, &["--plain"], &[]);
    assert_eq!(
        wait(&mut c, Duration::from_secs(600)),
        ATTENTION,
        "{}",
        log_tail(&cdir)
    );
    assert_eq!(resolve_terminals(&cdir, Some("toy"), None), 0);
    assert_eq!(run_to_end(&sp, &cdir, &["--plain"]).last(), Some(&10));
    tamper_to_closed(&cdir);
    let codes = run_to_end(&sp, &cdir, &["--plain"]);
    assert_ne!(codes.last(), Some(&0), "{codes:?}");
}

/// FIXED by lane closeloop4 (audit m8): the statement files are ledger records: after a closed run its statement.jsonl
/// truncated on disk is detected by `rr_campaign verify` and by a resumed run (verdict error, exit 11).
#[test]
fn a_truncated_statement_after_the_run_is_detected_by_verify_and_resume() {
    let _g = serial();
    let t = Tmp::new("acstmt");
    let files = toy(&t.path.join("material"), &Toy::default(), FAKE);
    std::fs::write(&files.hand, "").unwrap();
    let spec = campaign_spec(&t.campaign, &files, FAKE, &cpus());
    let sp = write_spec(&t, "spec.json", &spec);
    let cdir = t.cdir("cdir");
    assert_eq!(run_to_end(&sp, &cdir, &["--plain"]).last(), Some(&0));
    let sp_stmt = cdir.join("closeloop/toy/final/statement/statement.jsonl");
    let full = std::fs::read(&sp_stmt).unwrap();
    std::fs::write(&sp_stmt, &full[..full.len() / 2]).unwrap();
    assert_ne!(verify(&cdir), 0, "verify must see the truncated statement");
    assert_eq!(
        run_to_end(&sp, &cdir, &["--plain"]).last(),
        Some(&11),
        "{}",
        log_tail(&cdir)
    );
    let listed = result_json(&cdir)["statement"]["files"]["statement.jsonl"]
        .as_str()
        .unwrap()
        .to_string();
    let got = sha256_file(&sp_stmt);
    assert_ne!(listed, got, "the listed digest exposes the truncation");
}

/// The elided toy of closeloop3_fake: the start record's witness loci are elided, its facts come from an exact-eval
/// (`coefficient-facts`) job; `lies`: oracle lines of the fake producer.
fn elided_setup(tag: &str, lies: &[Value]) -> (Tmp, PathBuf, PathBuf) {
    let t = Tmp::new(tag);
    let files = toy(&t.path.join("material"), &Toy::default(), FAKE);
    std::fs::write(&files.hand, "").unwrap();
    let dir = &files.dir;
    let rule = json!({"type":"rule","chart":"110","index":0,"fixed":[5,1,0],"equations":[],"target":"I(5,1,0)",
        "rhs":[["I(4,1,0)","(d-4)/(d-3)"],["I(5,0,0)","2*n0"],["I(5,1,1)","((n0-5)*(d-2))/(d-1)"]],
        "exceptions":[],"dispatch":"Partition","witness_loci":["elided"]})
    .to_string();
    let a = write_table(dir, "A.table.jsonl", std::slice::from_ref(&rule));
    std::fs::write(
        dir.join("proofs.json"),
        json!({"schema":"rustred.proof-kernel.span.v1","keys":[
            {"record_sha256": sha(&rule), "key": [5,1,0], "status":"EXACT_IN_SPAN_AT_KEY"}]})
        .to_string(),
    )
    .unwrap();
    let mut spec: Value =
        serde_json::from_slice(&std::fs::read(&files.import_spec).unwrap()).unwrap();
    spec["sources"][0]["sha256"] = json!(sha256_file(&a));
    spec["evidence"] = json!({"proofs":[pin(&dir.join("proofs.json"))]});
    std::fs::write(
        &files.import_spec,
        serde_json::to_vec_pretty(&spec).unwrap(),
    )
    .unwrap();
    let mut c: Value = serde_json::from_slice(&std::fs::read(&files.config).unwrap()).unwrap();
    let lp = dir.join("exact-lies.jsonl");
    std::fs::write(
        &lp,
        lies.iter().map(|v| format!("{v}\n")).collect::<String>(),
    )
    .unwrap();
    c["nodes"][0]["proof"]["exact_eval"] = json!({"program": "rr_coef_facts", "max_keys_per_job": 8,
        "wall_s": 60, "cap_bytes": 268435456, "expected_peak_bytes": 67108864,
        "fake": {"oracle": lp.display().to_string(), "ms": 150, "program": "rr_fake_child"}});
    std::fs::write(&files.config, serde_json::to_vec_pretty(&c).unwrap()).unwrap();
    let mut spec = campaign_spec(&t.campaign, &files, FAKE, &cpus());
    spec["programs"]["rr_coef_facts"] = json!({"path": FAKE});
    let sp = write_spec(&t, "spec.json", &spec);
    let cdir = t.cdir("cdir");
    (t, sp, cdir)
}

/// THE TRUST BOUNDARY of the zero-term rule, FIXED by lane closeloop4 (audit M3, closeloop.md 18): a coefficient-facts
/// answer that is correctly BOUND but claims every term zero is now contradicted by the SECOND route (rr_proof
/// `--elided-loci facts`): the pair is disputed, its terms stay edges, the node is blocked (never closed on fewer keys).
#[test]
fn a_well_bound_lying_exact_eval_answer_is_blocked_by_the_second_route() {
    let _g = serial();
    let (_ta, spa, ca) = elided_setup("acexh", &[]);
    assert_eq!(
        run_to_end(&spa, &ca, &["--plain"]).last(),
        Some(&0),
        "{}",
        log_tail(&ca)
    );
    assert_eq!(result_json(&ca)["verdict"], "closed");
    let honest = statement_keys(&ca);
    let lie = json!({"key": [5, 1, 0], "line": {"nonzero": [], "zero_terms": 3}});
    let (_tb, spb, cb) = elided_setup("acexl", &[lie]);
    assert_eq!(
        run_to_end(&spb, &cb, &["--plain"]).last(),
        Some(&10),
        "{}",
        log_tail(&cb)
    );
    let r = result_json(&cb);
    let lied = statement_keys(&cb);
    eprintln!(
        "honest closure {} keys, lie closure {} keys (missing {:?}); lie verdict {}",
        honest.len(),
        lied.len(),
        honest.difference(&lied).collect::<Vec<_>>(),
        r["verdict"]
    );
    assert_eq!(r["verdict"], "open", "{r:#}");
    assert!(
        r["blockers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|b| b.as_str().unwrap().starts_with("second_route_disagreement")),
        "{r:#}"
    );
    assert!(
        lied.contains(&vec![4, 1, 0]),
        "the disputed edge must stay in the closure"
    );
}

/// SIGKILL (a crash, no graceful stop) at seeded random points through the exact-evaluation toy, then resume, then a
/// SECOND resume after the end: the same statement bytes and result verdict as the uninterrupted run.
#[test]
fn sigkill_at_random_points_and_double_resume_give_the_same_statement() {
    let _g = serial();
    let (_ta, spa, ca) = elided_setup("acskref", &[]);
    assert_eq!(
        run_to_end(&spa, &ca, &["--plain"]).last(),
        Some(&0),
        "{}",
        log_tail(&ca)
    );
    let ref_stmt = std::fs::read(ca.join("closeloop/toy/final/statement/statement.jsonl")).unwrap();
    let ref_res = result_json(&ca);
    for seed in [3u64, 17, 29] {
        let (_tb, spb, cb) = elided_setup(&format!("acsk{seed}"), &[]);
        let mut x = seed;
        let mut codes = vec![];
        for round in 0..16 {
            let mut c = start(&spb, &cb, &["--plain"], &[]);
            if round < 5 {
                x = mix(x);
                std::thread::sleep(Duration::from_millis(250 + x % 2500));
                signal(&c, libc::SIGKILL);
            }
            let code = wait(&mut c, Duration::from_secs(300));
            codes.push(code);
            if code == 0 {
                break;
            }
            if code == ATTENTION {
                let _ = resolve_unknown(&cb);
            }
        }
        assert_eq!(
            codes.last(),
            Some(&0),
            "seed {seed}: {codes:?} {}",
            log_tail(&cb)
        );
        let stmt = std::fs::read(cb.join("closeloop/toy/final/statement/statement.jsonl")).unwrap();
        assert!(
            stmt == ref_stmt,
            "seed {seed}: statements differ ({codes:?})"
        );
        // double resume: nothing changes, exit 0
        let again = run_to_end(&spb, &cb, &["--plain"]);
        assert_eq!(again.last(), Some(&0));
        let stmt2 =
            std::fs::read(cb.join("closeloop/toy/final/statement/statement.jsonl")).unwrap();
        assert!(
            stmt2 == ref_stmt,
            "seed {seed}: statement changed by the second resume"
        );
        let r = result_json(&cb);
        assert_eq!(r["verdict"], ref_res["verdict"]);
        // summary.json embeds the campaign directory's paths (and the load-order fact label, F-SUMM): every other
        // statement file must be byte-identical
        for f in [
            "statement.jsonl",
            "terminals.json",
            "fact-requests.jsonl",
            "proof-requests.jsonl",
            "solve-requests.jsonl",
        ] {
            assert_eq!(
                r["statement"]["files"][f], ref_res["statement"]["files"][f],
                "seed {seed}: {f}"
            );
        }
        assert_eq!(verify(&cb), 0, "seed {seed}");
        eprintln!("seed {seed}: codes {codes:?} then {again:?}");
    }
}
