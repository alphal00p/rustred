//! AUDIT r2-sym (independent auditor of round 2's SUPPLIED relations, 2026-11-15; notes/implementer.audit-r2-sym.md):
//! the supervisor side of kind `symmetry-composite` (rows 33-36) and the accept columns of the diagnostic / supplied
//! rows, with the FAKE child through the real launcher on the small real import of `symcomp_fake` (test data: the T178
//! order-B frame table, read-only; skipped when absent). Run under `LIGHT_PIDNS=1` (rule L10):
//! `LIGHT_PIDNS=1 light_build.sh <lane> test -p rustred-campaign --test audit_r2_sym_sup -- --include-ignored`
//!
//! - F2 END TO END (`#[ignore]`, fails while the defect stands): a child result in which ONE shipped relation repeats a
//!   term (same coefficient) in place of another, RESEALED consistently, passes the contract's child-seal check
//!   (`classify::symcomp::check_child`), the classifier's tally and the supervisor's import verification
//!   (`adapters::symcomp::verify_relations` -> `composite_verify::verify_shipped`): a relation that is not the identity
//!   (a term missing) is ACCEPTED as a supplied relation.
//! - `route_settled` refuses a settled attempt whose job id / attempt is not the run directory's (stale job id).
//! - No diagnostic accept (`diagnostic_records`) ever reaches the ledger as a value that lets records be taken.

mod adapters_support;
use adapters_support::*;

use rustred_campaign::adapters::job::{JobAdapter, RunOptions, run_attempt, sha256_hex};
use rustred_campaign::adapters::symcomp::{
    SymCompBatch, SymCompRequest, SymmetryCompositeAdapter, VerifyOptions, load_import,
    route_settled, verify_relations,
};
use rustred_campaign::ledger::FileRef;
use rustred_campaign::state::{ContractAccept, accept_permits_records, contract_accepts};
use rustred_campaign::supervisor::worksource::Settled;
use rustred_campaign_contract::classify::symcomp;
use rustred_campaign_contract::classify::{self, Accept};
use rustred_campaign_contract::fake::{FakeOp, FakeScript, OnSigterm, RssPlan};
use rustred_campaign_contract::layout::RunLayout;
use rustred_campaign_contract::spec::JobSpec;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

const WS: &str = "/shared/vhirschi/t3code/vakint_dev/p_integral_dev";

fn frame_table() -> Option<PathBuf> {
    let p = std::env::var_os("RUSTRED_SYMCOMP_FRAME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(WS).join("SCRATCHPAD/t178/regenB/R0/try1/node/F.table.jsonl")
        });
    p.exists().then_some(p)
}

/// The eight arrival keys of `symcomp_fake` (test data).
fn keys() -> Vec<Vec<i64>> {
    [
        "[1,1,1,1,1,1,4,1,1,5,0,0,0,0,0,0,0,0,0,0]",
        "[1,1,1,1,6,1,2,1,1,1,0,0,0,0,0,0,0,0,0,0]",
        "[1,1,1,1,1,1,1,1,1,3,0,-1,0,-1,-2,-3,0,0,0,0]",
        "[1,1,1,1,1,1,4,1,1,3,0,0,0,0,0,0,0,0,0,0]",
        "[1,1,2,1,1,2,1,1,1,9,0,0,0,0,0,0,0,0,0,0]",
        "[1,6,1,1,1,1,1,1,1,7,0,0,0,0,0,0,0,0,0,0]",
        "[1,1,1,1,9,1,4,1,1,1,0,0,0,0,0,0,0,0,0,0]",
        "[1,1,1,1,3,1,3,1,1,1,0,0,0,0,0,0,0,0,0,0]",
    ]
    .iter()
    .map(|s| serde_json::from_str(s).unwrap())
    .collect()
}

fn request(root: &Root, frame: &Path) -> SymCompRequest {
    let start = root.path().join("start.keys.jsonl");
    let kb: String = keys()
        .iter()
        .map(|k| format!("{}\n", serde_json::to_string(k).unwrap()))
        .collect();
    std::fs::write(&start, kb).unwrap();
    let s = json!({"start": {"path": start.display().to_string(), "format": "keys_jsonl"},
        "schema": "rustred.campaign.import-spec.v1", "node": "N", "frame_from": "F", "bits": 6,
        "policies": {"tier": "K"}, "required_evidence": "imported",
        "sources": [{"id": "F", "kind": "engine_table", "class": "frame", "path": frame.display().to_string(),
            "removed": []}]});
    let p = root.path().join("import-spec.json");
    std::fs::write(&p, serde_json::to_vec_pretty(&s).unwrap()).unwrap();
    let ks = keys();
    SymCompRequest {
        node: Some("N".into()),
        import_spec: p,
        import_base: "/".into(),
        faces: vec![
            ("f1".into(), ks[..4].to_vec()),
            ("f2".into(), ks[4..].to_vec()),
        ],
        policy: None,
        threads: 1,
        limit: None,
    }
}

fn adapter() -> SymmetryCompositeAdapter {
    SymmetryCompositeAdapter {
        program: fake_program(),
        licence_env: None,
        verify: VerifyOptions::default(),
        fake: Some(FakeScript {
            script: vec![FakeOp::Prepare { ms: 5 }],
            on_sigterm: OnSigterm::Graceful,
            slow_ms: 0,
            start_delay_ms: 0,
            rss: RssPlan::default(),
            helpers: vec![],
            faults: vec![],
            time_scale: 1.0,
            volatile_jitter: false,
        }),
    }
}

/// One honest attempt; returns the batch and the attempt's run directory.
fn honest_run(root: &Root, req: &SymCompRequest, attempt: u32) -> (SymCompBatch, PathBuf, String) {
    let ad = adapter();
    let reqs = vec![req.clone()];
    let plan = ad.plan(&reqs).unwrap().remove(0);
    let mut l = launcher();
    let (out, rep) = run_attempt(
        &mut l,
        &ad,
        &reqs,
        &plan,
        root.path(),
        attempt,
        &RunOptions::default(),
    )
    .unwrap();
    assert!(rep.launch_error.is_none(), "{:?}", rep.launch_error);
    let run_dir = rustred_campaign_contract::layout::attempt_dir(
        &std::path::absolute(root.path()).unwrap(),
        &plan.job_id,
        attempt,
    );
    (out, run_dir, plan.job_id)
}

macro_rules! setup {
    ($tag:expr) => {{
        require_namespace();
        let Some(frame) = frame_table() else {
            eprintln!("SKIP: frame table absent");
            return;
        };
        (serial(), Root::new($tag), frame)
    }};
}

fn kt(k: &[i64]) -> String {
    format!(
        "I({})",
        k.iter().map(i64::to_string).collect::<Vec<_>>().join(",")
    )
}

/// Reseal the child's directory after `table` was rewritten: result.json's output entry and SHA256SUMS (what a
/// forging child does; the contract re-derives both from the bytes, so a CONSISTENT reseal is the threat model).
fn reseal(dir: &Path) {
    let rp = dir.join(symcomp::RESULT_FILE);
    let mut res: Value = serde_json::from_slice(&std::fs::read(&rp).unwrap()).unwrap();
    let mut sums = String::new();
    for (o, name) in res["outputs"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .zip(symcomp::CHILD_FILES)
    {
        let b = std::fs::read(dir.join(name)).unwrap();
        let h = sha256_hex(&b);
        o["sha256"] = json!(h);
        o["lines"] = json!(b.iter().filter(|&&c| c == b'\n').count());
        sums.push_str(&format!("{h}  {name}\n"));
    }
    std::fs::write(dir.join(symcomp::SUMS_FILE), sums).unwrap();
    std::fs::write(&rp, serde_json::to_vec(&res).unwrap()).unwrap();
}

#[test]
#[ignore = "AUDIT r2-sym F2 (end to end): a resealed relation with a duplicated term passes check_child + verify_relations"]
fn a_resealed_relation_with_a_duplicated_term_is_refused() {
    let (_g, root, frame) = setup!("dup");
    let req = request(&root, &frame);
    let (out, run_dir, _) = honest_run(&root, &req, 1);
    assert_eq!(out.classification.accept, Accept::SuppliedRelations);
    let dir = run_dir.join("out").join(symcomp::CHILD_DIR);
    // the shipped relation with the most terms (it needs two: one is replaced by a copy of another)
    let tp = dir.join(symcomp::TABLE_FILE);
    let text = std::fs::read_to_string(&tp).unwrap();
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let (j, _) = lines
        .iter()
        .enumerate()
        .skip(1)
        .map(|(i, l)| {
            let v: Value = serde_json::from_str(l).unwrap();
            (i, v["rhs"].as_array().map_or(0, Vec::len))
        })
        .max_by_key(|x| x.1)
        .unwrap();
    let mut v: Value = serde_json::from_str(&lines[j]).unwrap();
    let rhs = v["rhs"].as_array().unwrap().clone();
    if rhs.len() < 2 {
        eprintln!("SKIP: no shipped relation with two terms on this import");
        return;
    }
    let honest = rhs.clone();
    let mut forged = rhs.clone();
    let last = forged.len() - 1;
    forged[last] = forged[0].clone();
    v["rhs"] = Value::Array(forged.clone());
    lines[j] = serde_json::to_string(&v).unwrap();
    std::fs::write(&tp, lines.join("\n") + "\n").unwrap();
    reseal(&dir);
    // the contract's own child-seal check accepts the resealed result (nothing in it looks at distinct terms)
    let spec: JobSpec =
        serde_json::from_slice(&std::fs::read(RunLayout::new(&run_dir).job_spec()).unwrap())
            .unwrap();
    let seal = match symcomp::check_child(&spec, &sha256_hex) {
        Ok(s) => s,
        Err(e) => {
            // a contract refusal here would already be a fix (typed refusal): record and stop
            eprintln!("check_child refused the forgery: {e}");
            return;
        }
    };
    let mut recs: Vec<Vec<u8>> = symcomp::results_records(&seal)
        .iter()
        .map(|r| {
            rustred_campaign_contract::canon::canonical_line(r.to_string().as_bytes()).unwrap()
        })
        .collect();
    recs.push(
        rustred_campaign_contract::canon::canonical_line(
            symcomp::end_record(&seal).to_string().as_bytes(),
        )
        .unwrap(),
    );
    let (res, _) = symcomp::tally(&recs, &spec).expect("the classifier's tally");
    let ctx = load_import(&spec).expect("the pinned import");
    let ver = verify_relations(&run_dir, &res, &ctx, &VerifyOptions::default());
    let target: Vec<i64> = serde_json::from_value(v["fixed"].clone()).unwrap();
    let accepted = ver.accepted.iter().any(|a| a.key == target);
    assert!(
        !accepted,
        "F2: the supervisor ACCEPTED at {} the relation {} (true: {})",
        kt(&target),
        Value::Array(forged),
        Value::Array(honest)
    );
}

#[test]
fn route_settled_refuses_a_stale_job_id_or_attempt() {
    let (_g, root, frame) = setup!("stale");
    let req = request(&root, &frame);
    let (out, run_dir, job_id) = honest_run(&root, &req, 1);
    let results = run_dir.join("out/results.jsonl");
    let b = std::fs::read(&results).unwrap();
    let lines: Vec<String> = String::from_utf8(b.clone())
        .unwrap()
        .lines()
        .map(|l| {
            String::from_utf8(
                rustred_campaign_contract::canon::canonical_line(l.as_bytes()).unwrap(),
            )
            .unwrap()
        })
        .collect();
    let abs = std::path::absolute(root.path()).unwrap();
    let settled = Settled {
        job: job_id.clone(),
        attempt: 1,
        item: "N.sc.0".into(),
        rung: rustred_campaign::state::Rung(0),
        classification: out.classification.clone(),
        table: Some(FileRef {
            path: results.strip_prefix(&abs).unwrap().display().to_string(),
            bytes: b.len() as u64,
            blake3: rustred_campaign_contract::blake3_hex(&b),
        }),
        canonical_lines: lines,
        canonical_blake3: None,
    };
    assert!(
        route_settled(&abs, &settled, None, &VerifyOptions::default())
            .unwrap()
            .is_some()
    );
    let mut s = settled.clone();
    s.attempt = 2;
    assert!(
        route_settled(&abs, &s, None, &VerifyOptions::default()).is_err(),
        "a settled attempt 2 routed attempt 1's results"
    );
    let mut s = settled.clone();
    s.job = format!("{job_id}x");
    assert!(
        route_settled(&abs, &s, None, &VerifyOptions::default()).is_err(),
        "a settled attempt of another job routed these results"
    );
}

/// Pure: every decision row whose accept column is `diagnostic_records` ends in the ledger as a value that takes no
/// record (the supervisor maps a value its class does not allow to `nothing`); `supplied_relations` takes records only
/// through the work source (documented reliance: the ledger cannot tell whether the import verification ran).
#[test]
fn diagnostic_rows_never_reach_the_ledger_as_record_accepts() {
    let mut diag = 0;
    for r in classify::ROWS.iter() {
        let ledger = if contract_accepts(r.kind).contains(&r.accept) {
            r.accept
        } else {
            ContractAccept::Nothing
        };
        if r.accept == Accept::DiagnosticRecords {
            diag += 1;
            assert!(
                !accept_permits_records(ledger),
                "row {}{}: diagnostic records reach the ledger as {ledger:?}",
                r.row,
                r.sub
            );
        }
        if r.accept == Accept::SuppliedRelations {
            assert!(accept_permits_records(ledger), "row {}", r.row);
        }
    }
    assert!(diag >= 4, "rows 30, 31, 32, 37 (found {diag})");
}
