//! Lane tadicjob (2026-11-18): the pole report's RANK-ONLY route. A stored driver attempt (job + result) answers a
//! cell's job only through `polereport::verify_reuse`: equal job up to the rank-only fields (job id, wall cap, the
//! dependence table the driver binds but does not apply), result bound to ITS job bytes and to the adapter's pins,
//! certificate without findings, every request decided. Driven with a REAL attempt of the fake child through the
//! process layer (RUN WITH LIGHT_PIDNS=1). Test data only (L8): keys of arity 3, made-up binary digest.

use rustred_campaign::adapters::default_emulation_script;
use rustred_campaign::adapters::job::{Invocation, JobAdapter, Program, RunOptions, run_attempt};
use rustred_campaign::adapters::tadicjob::{
    DataFile, TadicAdapter, TadicInput, TadicJobRequest, TadicLevel, TadicLimits, TadicRequest,
};
use rustred_campaign::launch::RealLauncher;
use rustred_campaign::polereport::verify_reuse;
use rustred_campaign::resources::{Sampler, SamplerConfig};
use rustred_campaign_contract::fake::FakeOp;
use rustred_campaign_contract::tadicjob;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

const FAKE: &str = env!("CARGO_BIN_EXE_rr_fake_child");
const BIN_SHA: &str = "c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1";

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("polereport-reuse-{tag}-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&d);
    d
}

fn adapter(dir: &Path) -> TadicAdapter {
    let drv = dir.join("tadic_job.py");
    std::fs::write(
        &drv,
        b"# a stand-in for the frozen driver (the fake child emulates it)\n",
    )
    .unwrap();
    let mut script = default_emulation_script();
    script.script = vec![FakeOp::Prepare { ms: 1 }];
    TadicAdapter {
        program: Program::from_file("rr_fake_child", Path::new(FAKE), Invocation::Contract)
            .unwrap(),
        driver: DataFile::from_file(&drv).unwrap(),
        package_binary: (dir.join("p_key_batch_tadic"), BIN_SHA.into()),
        node_binary: (dir.join("rr_node_tadic"), BIN_SHA.into()),
        licence_env: None,
        fake: Some(script),
    }
}

fn request(dir: &Path, w: i64, deps: Option<DataFile>) -> TadicJobRequest {
    let lib = dir.join("library.json");
    std::fs::write(&lib, b"{\"terminals\": [\"unit\"]}").unwrap();
    let k = |p: [i64; 3], w: i64| TadicInput::Package {
        top: "X".into(),
        powers: p.to_vec(),
        weight: w,
    };
    TadicJobRequest {
        level: TadicLevel::Package {
            path: dir.to_path_buf(),
            relations: None,
            library_sha256: DataFile::from_file(&lib).unwrap().sha256,
        },
        requests: vec![TadicRequest {
            label: "c0".into(),
            inputs: vec![k([2, 1, -1], w), k([1, 2, 0], 7)],
            weight_vectors: None,
        }],
        d0: 4,
        order: 1,
        primes: 2,
        max_primes: 8,
        seeds: vec![1, 2],
        limits: TadicLimits {
            wall_seconds: 60,
            max_length: 64,
            threads: 1,
            memory_gib: None,
            no_step_cache: false,
        },
        crt: false,
        release_every: None,
        record_per_seed: false,
        terminals: Some(DataFile::from_file(&lib).unwrap()),
        dependences: deps,
        depths: None,
    }
}

/// One real attempt; returns (adapter, the request, the job bytes the driver ran, the result bytes it wrote).
fn attempt(dir: &Path) -> (TadicAdapter, TadicJobRequest, Vec<u8>, Vec<u8>) {
    let deps0 = dir.join("deps.v0.json");
    std::fs::write(&deps0, b"{\"v\": 0}").unwrap();
    let ad = adapter(dir);
    let rq = request(dir, 3, Some(DataFile::from_file(&deps0).unwrap()));
    let plans = ad.plan(std::slice::from_ref(&rq)).unwrap();
    let opts = RunOptions {
        cpus: vec![],
        memory_cap_bytes: Some(2 << 30),
        licence_env: None,
        wall_cap: Some(Duration::from_secs(60)),
        grace: Some(Duration::from_secs(5)),
    };
    let sampler = Arc::new(
        Sampler::spawn(SamplerConfig {
            interval: Duration::from_millis(200),
            collect_foreign_pinning: false,
        })
        .unwrap(),
    );
    let mut l = RealLauncher::new(sampler);
    let (b, rep) = run_attempt(
        &mut l,
        &ad,
        std::slice::from_ref(&rq),
        &plans[0],
        &dir.join("work"),
        1,
        &opts,
    )
    .unwrap();
    assert!(
        b.result.is_some(),
        "the attempt itself must be row 50: {:?}",
        b.unread
    );
    let ji = rep
        .prepared
        .spec
        .inputs
        .iter()
        .find(|i| i.role == tadicjob::ROLE_JOB)
        .unwrap();
    let jb = std::fs::read(rep.prepared.spec.input_path(ji)).unwrap();
    let rb = std::fs::read(rep.prepared.layout.out_dir().join(tadicjob::CHILD_OUT)).unwrap();
    (ad, rq, jb, rb)
}

#[test]
fn a_stored_attempt_is_reused_only_for_its_own_job_and_only_bound() {
    let d = tmp("reuse");
    let (ad, rq, jb, rb) = attempt(&d);
    // the same job: reused
    let checked = verify_reuse(&ad, &rq, &jb, &rb).unwrap();
    assert_eq!(checked.requests.len(), 1);
    // the dependence table REPUBLISHED (another sha256): a rank-step role only, the driver attempt is still this job's
    let deps1 = d.join("deps.v1.json");
    std::fs::write(&deps1, b"{\"v\": 1}").unwrap();
    let mut republished = rq.clone();
    republished.dependences = Some(DataFile::from_file(&deps1).unwrap());
    assert_ne!(
        ad.content_of(&republished),
        ad.content_of(&rq),
        "the content id hashes the rank-step roles"
    );
    assert!(verify_reuse(&ad, &republished, &jb, &rb).is_ok());
    // another weight (another combination): refused, naming the field
    let other = request(&d, 4, rq.dependences.clone());
    let e = verify_reuse(&ad, &other, &jb, &rb).unwrap_err();
    assert!(e.contains("requests"), "{e}");
    // another pinned binary: refused
    let mut ad2 = adapter(&d);
    ad2.package_binary.1 = "d2".repeat(32);
    assert!(verify_reuse(&ad2, &rq, &jb, &rb).is_err());
    // the same job VALUE but other job BYTES (re-serialised): the result is bound to the bytes it ran, refused
    let v: serde_json::Value = serde_json::from_slice(&jb).unwrap();
    let pretty = serde_json::to_vec_pretty(&v).unwrap();
    assert_ne!(pretty, jb);
    assert!(verify_reuse(&ad, &rq, &pretty, &rb).is_err());
    // a result that is not this job's (its job digest altered): refused
    let mut r: serde_json::Value = serde_json::from_slice(&rb).unwrap();
    r["job_sha256"] = serde_json::json!("0".repeat(64));
    assert!(verify_reuse(&ad, &rq, &jb, &serde_json::to_vec(&r).unwrap()).is_err());
}
