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

/// FINDING F-EVPIN (major): the closing loop pins the import spec TEXT (ledger input `import_spec`, BLAKE3) but not
/// the content of the evidence it names; a spec entry may name a DIRECTORY, which cannot carry a sha256 at all. Two
/// passes of the SAME spec text (same pinned digest) over a changed evidence file give different verdicts: here an
/// honest fact makes round 1 need solves (not final), a fact line flipped to "zero" makes the SAME round-1 pass
/// `closed` on a smaller key set. Nothing refuses; a resumed campaign would re-read the changed content.
#[test]
fn f_evpin_same_pinned_spec_other_evidence_content_changes_the_verdict() {
    let t = Tmp::new("acevpin");
    let cfg = Toy {
        second_face: None,
        ..Toy::default()
    };
    let files = toy(&t.path.join("m"), &cfg, FAKE);
    std::fs::write(&files.hand, "").unwrap();
    // move the facts into a DIRECTORY named by the spec (as the T162 / T178 specs name theirs)
    let dir = files.dir.join("facts.d");
    std::fs::create_dir_all(&dir).unwrap();
    let honest = std::fs::read_to_string(files.dir.join("facts.jsonl")).unwrap();
    std::fs::write(dir.join("f.jsonl"), &honest).unwrap();
    let mut spec: Value =
        serde_json::from_slice(&std::fs::read(&files.import_spec).unwrap()).unwrap();
    spec["evidence"]["facts"] = json!([{"path": dir.display().to_string()}]);
    // a directory cannot be pinned: the spec refuses a sha256 on it at load
    let mut pinned = spec.clone();
    pinned["evidence"]["facts"] = json!([{"path": dir.display().to_string(), "sha256": "00"}]);
    let ps = rustred_campaign_core::import::TableSetSpec::from_json(&pinned.to_string()).unwrap();
    let set = rustred_campaign_core::import::load(&ps, &files.dir).unwrap();
    assert!(rustred_campaign_core::import::Evidence::from_spec(&set, &files.dir).is_err());
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
        .unwrap()
    };
    let a = pass("honest");
    assert!(
        a.final_.is_none(),
        "honest round 1 needs solves: {}",
        a.check
    );
    assert!(!a.batches.is_empty());
    // flip ONE term of the fact line to zero (the lower key [4,1,0]): same spec text, same pinned digest
    let lie = honest.replacen(
        r#"{"coef":"nonzero","key":[4,1,0],"term":0}"#,
        r#"{"coef":"zero","key":[4,1,0],"term":0}"#,
        1,
    );
    let lie = if lie == honest {
        // canonical field order of the support file differs: edit through JSON
        let mut v: Value = serde_json::from_str(honest.trim()).unwrap();
        v["rhs"][0]["coef"] = json!("zero");
        format!("{v}\n")
    } else {
        lie
    };
    assert_ne!(lie, honest);
    std::fs::write(dir.join("f.jsonl"), &lie).unwrap();
    let b = pass("lie");
    let fb = b.final_.as_ref().expect("the lie makes round 1 final");
    eprintln!(
        "honest: keys {} certified {} closed {}; lie: keys {} certified {} verdict {}",
        a.check["keys"],
        a.check["certified"],
        a.check["closed"],
        b.check["keys"],
        b.check["certified"],
        fb.verdict
    );
    assert_eq!(fb.verdict, "closed", "{}", b.check);
    assert!(b.check["keys"].as_u64() < a.check["keys"].as_u64());
    // the only trace: the evidence file digest the check record REPORTS
    assert_ne!(
        a.check["evidence_import"]["files"],
        b.check["evidence_import"]["files"]
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

/// FINDING F-OUT (minor) as the current behaviour: an OPEN node's recorded final/result.json overwritten on disk with
/// `"verdict":"closed"` -> the resumed run exits 0 (the outcome reads the file, not the ledger's recorded digest).
#[test]
fn f_out_a_tampered_result_file_currently_turns_exit_10_into_0() {
    let _g = serial();
    let (_t, sp, cdir) = open_toy("acout");
    assert_eq!(
        run_to_end(&sp, &cdir, &["--plain"]).last(),
        Some(&10),
        "{}",
        log_tail(&cdir)
    );
    tamper_to_closed(&cdir);
    let codes = run_to_end(&sp, &cdir, &["--plain"]);
    eprintln!(
        "after tamper: exit codes {codes:?}; verify {}",
        verify(&cdir)
    );
    assert_eq!(
        codes.last(),
        Some(&0),
        "defect F-OUT reproduced if 0: {codes:?}"
    );
}

/// F-OUT, the correct behaviour: a result file that no longer matches its recorded BLAKE3 is an error (11), never 0.
#[test]
#[ignore = "F-OUT: fails on 0ceb9e79 (outcome reads final/result.json without its recorded digest)"]
fn f_out_a_tampered_result_file_must_not_exit_0() {
    let _g = serial();
    let (_t, sp, cdir) = open_toy("acout2");
    assert_eq!(run_to_end(&sp, &cdir, &["--plain"]).last(), Some(&10));
    tamper_to_closed(&cdir);
    let codes = run_to_end(&sp, &cdir, &["--plain"]);
    assert_ne!(codes.last(), Some(&0), "{codes:?}");
}

/// The statement is NOT a ledger record: after a closed run its statement.jsonl truncated on disk is not detected by
/// `rr_campaign verify` nor by a resumed run (exit 0); only the digests listed in result.json (and SHA256SUMS) show it.
#[test]
fn a_truncated_statement_after_the_run_is_only_visible_through_the_listed_digests() {
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
    assert_eq!(verify(&cdir), 0, "verify does not look at the statement");
    assert_eq!(run_to_end(&sp, &cdir, &["--plain"]).last(), Some(&0));
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
    spec["evidence"] = json!({"proofs":[{"path": dir.join("proofs.json").display().to_string()}]});
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

/// THE TRUST BOUNDARY of the zero-term rule: a coefficient-facts answer that is correctly BOUND (record, key, rule
/// echo, producer) but claims every term zero is accepted by the wrapper (it checks binding, not content) and the
/// node closes on a SMALLER key set; nothing in the loop cross-checks a producer's zeros (one route per record).
#[test]
fn a_well_bound_lying_exact_eval_answer_closes_the_node_on_fewer_keys() {
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
        Some(&0),
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
    assert_eq!(r["verdict"], "closed", "{r:#}");
    assert!(lied.len() < honest.len());
    assert!(!lied.contains(&vec![4, 1, 0]));
    assert_eq!(verify(&cb), 0);
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
