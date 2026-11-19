//! Kind `symmetry-composite` with the FAKE child through the REAL launcher (lane symjob; interfaces/
//! symmetry-composite-job.md 6, 7): `rr_fake_child` emulates `rr_rf_symflow run --request` over Q with the campaign core
//! (plain relations only), on a SMALL real import (test data: the T178 order-B frame table alone, read-only; skipped
//! when absent) and eight arrival keys of the T178 arrival case (test data). Checked: accepted relations pass the
//! IMPORT's verification; a forged relation (one coefficient changed, sealed validly) is REFUSED by the import's
//! verification although the contract accepted the seal; over_cap keys are typed per-key outcomes; a resource failure
//! is row 15b; unsealed / tampered results accept nothing (row 22); a result of another request is row 36; interrupt +
//! resume gives the same accepted record; the WorkSource-level functions (job files, JobRequest, route_settled).
//! Run under `LIGHT_PIDNS=1` (private pid namespace, rule L10):
//! `LIGHT_PIDNS=1 light_build.sh symjob test -p rustred-campaign --test symcomp_fake`
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

mod adapters_support;
mod inputs;
use adapters_support::*;

use rustred_campaign::adapters::job::{Ended, JobAdapter, RunOptions, run_attempt};
use rustred_campaign::adapters::symcomp::{
    SymCompBatch, SymCompJobConfig, SymCompRequest, SymmetryCompositeAdapter, VerifyOptions,
    job_files, job_request, route_settled,
};
use rustred_campaign::launch::{ChildHandle, ChildLauncher, KillReason as LaunchKill};
use rustred_campaign::ledger::FileRef;
use rustred_campaign::supervisor::worksource::Settled;
use rustred_campaign_contract::classify::recipe::FailureClass;
use rustred_campaign_contract::classify::{Accept, Action, WaitStatus};
use rustred_campaign_contract::exit::ExitCode;
use rustred_campaign_contract::fake::{FakeOp, FakeScript, OnSigterm, RssPlan};
use rustred_campaign_contract::spec::JobKind;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::time::Duration;

const WS: &str = "/shared/vhirschi/t3code/vakint_dev/p_integral_dev";

fn frame_table() -> Option<PathBuf> {
    let p = std::env::var_os("RUSTRED_SYMCOMP_FRAME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(WS).join("SCRATCHPAD/t178/regenB/R0/try1/node/F.table.jsonl")
        });
    p.exists().then_some(p)
}

/// Eight arrival keys (test data: 4 plain, 2 none, 2 composite in the real tool's T178 run; the fake ships no
/// composite: those become `none`).
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

const PLAIN: [usize; 4] = [0, 2, 4, 5];

fn spec_file(root: &Root, frame: &Path) -> PathBuf {
    let start = root.path().join("start.keys.jsonl");
    let kb: String = keys()
        .iter()
        .map(|k| format!("{}\n", serde_json::to_string(k).unwrap()))
        .collect();
    std::fs::write(&start, kb).unwrap();
    let s = json!({"start": {"path": start.display().to_string(), "format": "keys_jsonl"},"schema": "rustred.campaign.import-spec.v1", "node": "N", "frame_from": "F", "bits": 6,
        "policies": {"tier": "K"}, "required_evidence": "imported",
        "sources": [{"id": "F", "kind": "engine_table", "class": "frame", "path": frame.display().to_string(),
            "removed": []}]});
    let p = root.path().join("import-spec.json");
    std::fs::write(&p, serde_json::to_vec_pretty(&s).unwrap()).unwrap();
    p
}

fn request(root: &Root, frame: &Path, policy: Option<PathBuf>) -> SymCompRequest {
    let ks = keys();
    SymCompRequest {
        node: Some("N".into()),
        import_spec: spec_file(root, frame),
        import_base: "/".into(),
        faces: vec![
            ("f1".into(), ks[..4].to_vec()),
            ("f2".into(), ks[4..].to_vec()),
        ],
        policy,
        threads: 1,
        limit: None,
    }
}

fn script(ops: Vec<FakeOp>) -> FakeScript {
    FakeScript {
        script: ops,
        on_sigterm: OnSigterm::Graceful,
        slow_ms: 0,
        start_delay_ms: 0,
        rss: RssPlan::default(),
        helpers: vec![],
        faults: vec![],
        time_scale: 1.0,
        volatile_jitter: false,
    }
}

fn answer(status: &str, key: Option<Vec<i64>>, ms: u64) -> FakeOp {
    FakeOp::ProofAnswer {
        ms,
        status: Some(status.into()),
        record_sha256: None,
        rule: None,
        key,
        line: None,
    }
}

fn adapter(mut ops: Vec<FakeOp>) -> SymmetryCompositeAdapter {
    if ops.is_empty() {
        ops.push(FakeOp::Prepare { ms: 5 });
    }
    SymmetryCompositeAdapter {
        program: fake_program(),
        licence_env: None,
        verify: VerifyOptions::default(),
        fake: Some(script(ops)),
    }
}

fn run(
    root: &Root,
    ad: &SymmetryCompositeAdapter,
    req: &SymCompRequest,
    attempt: u32,
) -> SymCompBatch {
    let reqs = vec![req.clone()];
    let plan = ad.plan(&reqs).unwrap().remove(0);
    let mut l = launcher();
    let (out, rep) = run_attempt(
        &mut l,
        ad,
        &reqs,
        &plan,
        root.path(),
        attempt,
        &RunOptions::default(),
    )
    .unwrap();
    assert!(rep.launch_error.is_none(), "{:?}", rep.launch_error);
    let so = std::fs::read_to_string(rep.prepared.layout.stdout_log()).unwrap_or_default();
    eprintln!("child stdout: {}", so.trim());
    eprintln!(
        "wait {:?} -> {:?}: row {}{} {:?} ({}) refusal {:?}",
        rep.ended.wait,
        out.mapped_wait,
        out.classification.row,
        out.classification.sub,
        out.classification.kind,
        out.classification.why,
        out.refusal
    );
    out
}

macro_rules! setup {
    ($tag:expr) => {{
        require_namespace();
        let Some(frame) = frame_table() else {
            inputs::missing(format_args!("SKIP: frame table absent"));
            return;
        };
        (serial(), Root::new($tag), frame)
    }};
}

#[test]
fn accepted_relations_pass_the_import_verification() {
    let (_g, root, frame) = setup!("ok");
    let req = request(&root, &frame, None);
    let out = run(&root, &adapter(vec![FakeOp::Prepare { ms: 10 }]), &req, 1);
    let c = &out.classification;
    assert_eq!(
        (c.row, c.accept, c.action),
        (34, Accept::SuppliedRelations, Action::QueueFailures),
        "{}",
        c.why
    );
    let r = out.routed.expect("routed");
    let ks = keys();
    for (i, k) in r.per_key.iter().enumerate() {
        assert_eq!(k.key, ks[i]);
        if PLAIN.contains(&i) {
            assert_eq!(
                (k.outcome.as_str(), k.verdict.as_str()),
                ("plain", "accepted"),
                "{k:?}"
            );
        } else {
            assert_eq!(
                (k.outcome.as_str(), k.verdict.as_str()),
                ("none", "no_relation"),
                "{k:?}"
            );
            assert_eq!(k.failure_class.as_deref(), Some("no_symmetry_relation"));
            assert_eq!(k.route.as_deref(), Some("next_recipe_rung"));
        }
    }
    assert_eq!(r.verified.accepted.len(), 4);
    assert!(r.verified.refused.is_empty(), "{:?}", r.verified.refused);
    // the accepted supplied_relations table: header + 4 lines; verification 4 lines; records content-addressed
    assert_eq!(r.table.iter().filter(|&&b| b == b'\n').count(), 5);
    assert_eq!(r.verification.iter().filter(|&&b| b == b'\n').count(), 4);
    let recs = r.records("nodes/N/symcomp");
    assert_eq!(recs.len(), 3);
    let ci = r.composite_input(root.path(), "nodes/N/symcomp").unwrap();
    assert!(ci.table.path.ends_with(".table.jsonl"));
    assert_eq!(ci.table.sha256.len(), 64);
}

#[test]
fn a_forged_relation_is_refused_by_the_import_although_the_seal_is_valid() {
    let (_g, root, frame) = setup!("forge");
    let req = request(&root, &frame, None);
    let forged = keys()[2].clone();
    let out = run(
        &root,
        &adapter(vec![answer("forge_coefficient", Some(forged.clone()), 0)]),
        &req,
        1,
    );
    // the contract accepts the seal: the candidates are routed
    assert_eq!(
        (out.classification.row, out.classification.accept),
        (34, Accept::SuppliedRelations)
    );
    let r = out.routed.expect("routed");
    assert_eq!(r.verified.accepted.len(), 3);
    assert_eq!(r.verified.refused.len(), 1);
    let (_, key, why) = &r.verified.refused[0];
    assert_eq!(key, &forged);
    assert!(why.contains("differs"), "{why}");
    let k = r.per_key.iter().find(|k| k.key == forged).unwrap();
    assert_eq!(k.verdict, "refused");
    // the forged line is not in the accepted table
    assert!(!String::from_utf8_lossy(&r.table).contains(&serde_json::to_string(&forged).unwrap()));
}

#[test]
fn over_cap_keys_are_typed_per_key_outcomes() {
    let (_g, root, frame) = setup!("cap");
    let pol = root.path().join("policy.json");
    std::fs::write(
        &pol,
        json!({"schema": "rustred.campaign.repairflow.v1", "repair_policy": {"route0": {"max_terms": 1}},
            "nodes": {}})
        .to_string(),
    )
    .unwrap();
    let req = request(&root, &frame, Some(pol));
    let out = run(&root, &adapter(vec![]), &req, 1);
    let c = &out.classification;
    assert!(matches!(c.row, 34 | 35), "{}", c.why);
    assert_eq!(c.action, Action::QueueFailures);
    let r = out.routed.expect("routed");
    let over: Vec<_> = r
        .per_key
        .iter()
        .filter(|k| k.outcome == "over_cap")
        .collect();
    assert!(!over.is_empty(), "{:?}", r.per_key);
    for k in over {
        assert_eq!(
            k.failure_class.as_deref(),
            Some(FailureClass::OverCapRowExpansion.name().as_str())
        );
        assert_eq!(k.route.as_deref(), Some("next_work_class"));
        assert!(
            k.status.starts_with("over_cap:row_expansion"),
            "{}",
            k.status
        );
    }
}

#[test]
fn resource_failure_unsealed_tampered_and_foreign_results() {
    let (_g, root, frame) = setup!("bad");
    let req = request(&root, &frame, None);
    // resource failure (the tool's ERROR line names EMFILE): row 15b, retried after back-off
    let out = run(
        &root,
        &adapter(vec![FakeOp::Fail {
            code: ExitCode::Input,
            message: "keys: Too many open files (os error 24)".into(),
        }]),
        &req,
        1,
    );
    assert_eq!(
        (out.classification.row, out.classification.sub.as_str()),
        (15, "b"),
        "{}",
        out.classification.why
    );
    assert_eq!(out.classification.action, Action::RetryAfterResourceBackoff);
    assert!(out.routed.is_none());
    // a plain refusal: row 13 (attention, no retry)
    let out = run(
        &root,
        &adapter(vec![FakeOp::Fail {
            code: ExitCode::Input,
            message: "import load: frame".into(),
        }]),
        &req,
        2,
    );
    assert_eq!(out.classification.row, 13);
    for (attempt, status, row) in [
        (3, "unsealed", 22),
        (4, "tamper_after_seal", 22),
        (5, "pin_mismatch", 36),
    ] {
        let out = run(
            &root,
            &adapter(vec![answer(status, None, 0)]),
            &req,
            attempt,
        );
        assert_eq!(
            out.classification.row, row,
            "{status}: {}",
            out.classification.why
        );
        assert_eq!(out.classification.accept, Accept::Nothing);
        assert!(out.routed.is_none(), "{status}");
        assert!(out.refusal.is_some(), "{status}");
    }
}

fn accepted_table(out: &SymCompBatch) -> Vec<u8> {
    out.routed.as_ref().expect("routed").table.clone()
}

#[test]
fn interrupt_then_resume_gives_the_same_record() {
    let (_g, root, frame) = setup!("resume");
    let req = request(&root, &frame, None);
    let reference = accepted_table(&run(&root, &adapter(vec![]), &req, 1));
    assert!(!reference.is_empty());
    // attempt 2: slow, stopped mid-run (K = stop): interrupted, nothing accepted
    let ad = adapter(vec![answer("slow", None, 400)]);
    let reqs = vec![req.clone()];
    let plan = ad.plan(&reqs).unwrap().remove(0);
    let prep = ad.prepare(&reqs, &plan, root.path(), 2).unwrap();
    let mut l = launcher();
    let mut h = l.launch(&prep.launch).unwrap();
    std::thread::sleep(Duration::from_millis(1500));
    h.terminate(Duration::from_secs(2), LaunchKill::Stop);
    let fin = rustred_campaign::adapters::job::drive(&mut h, &RunOptions::default());
    let out = ad.finish(&reqs, &plan, &prep, &Ended::from_report(&fin));
    assert_eq!(
        (out.classification.row, out.classification.action),
        (20, Action::ResumeOnCampaignResume),
        "{}",
        out.classification.why
    );
    assert!(out.routed.is_none());
    // resume = re-run (a new attempt of the same request): the same accepted record, byte for byte
    let again = accepted_table(&run(&root, &ad, &req, 3));
    assert_eq!(again, reference);
}

#[test]
fn worksource_level_job_request_and_route_settled() {
    let (_g, root, frame) = setup!("ws");
    let req = request(&root, &frame, None);
    // the job files a work source writes under its node directory, and the JobRequest for the supervisor
    let prog = fake_program();
    let files = job_files(
        &root.path().join("nodes/N/symcomp"),
        &req,
        prog.sha256.as_deref().unwrap(),
    )
    .unwrap();
    let roles: Vec<&str> = files.inputs.iter().map(|i| i.role.as_str()).collect();
    assert_eq!(roles, vec!["request", "import_spec", "keys"]);
    let jr = job_request(
        "N.sc.0",
        "N.sc.0-t0",
        &files,
        &SymCompJobConfig {
            program: "rr_rf_symflow".into(),
            cap_bytes: 8 << 30,
            expected_peak_bytes: 2 << 30,
            cores: 2,
            heartbeat_s: 30,
            priority: 5,
            fake: None,
        },
        vec![],
    );
    assert_eq!(jr.kind, JobKind::SymmetryComposite);
    assert!(jr.args.is_empty());
    assert_eq!(jr.inputs.len(), 3);
    // a settled attempt as the supervisor hands it to the source: route_settled = the adapter's own routing
    let ad = adapter(vec![]);
    let out = run(&root, &ad, &req, 1);
    let plan = ad.plan(std::slice::from_ref(&req)).unwrap().remove(0);
    let run_dir = rustred_campaign_contract::layout::attempt_dir(
        &std::path::absolute(root.path()).unwrap(),
        &plan.job_id,
        1,
    );
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
    let settled = Settled {
        job: plan.job_id.clone(),
        attempt: 1,
        item: "N.sc.0".into(),
        rung: rustred_campaign::state::Rung(0),
        classification: out.classification.clone(),
        table: Some(FileRef {
            path: results
                .strip_prefix(std::path::absolute(root.path()).unwrap())
                .unwrap()
                .display()
                .to_string(),
            bytes: b.len() as u64,
            blake3: rustred_campaign_contract::blake3_hex(&b),
        }),
        canonical_lines: lines,
        canonical_blake3: None,
    };
    let routed = route_settled(
        &std::path::absolute(root.path()).unwrap(),
        &settled,
        None,
        &VerifyOptions::default(),
    )
    .unwrap()
    .unwrap();
    assert_eq!(routed, out.routed.unwrap());
    // a settled refusal routes nothing
    let mut s2 = settled.clone();
    s2.classification.accept = Accept::Nothing;
    s2.classification.row = 22;
    assert!(
        route_settled(root.path(), &s2, None, &VerifyOptions::default())
            .unwrap()
            .is_none()
    );
    let _ = WaitStatus::Unknown;
}
