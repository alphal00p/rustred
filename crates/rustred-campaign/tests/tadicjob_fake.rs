//! Lane tadicjob (interfaces/tadic-job.md 2, 3): kind `tadic-replay` through REAL child processes (`rr_fake_child`
//! emulating helper VH 2's driver with a linear test rule) and the process layer: the wrapper binds the result to the
//! pinned job and maps the driver's exits to rows 50 / 51 / 52 / 9 / 10 / 17; NOT_DECIDED outcomes are typed (never zero);
//! the certificate checks C2-C5 run on the bound result; the result store resumes. RUN WITH LIGHT_PIDNS=1 (spawns
//! children). Test data only (L8): keys of arity 3, made-up binary digest.

use rustred_campaign::adapters::default_emulation_script;
use rustred_campaign::adapters::job::{Invocation, JobAdapter, Program, RunOptions, run_attempt};
use rustred_campaign::adapters::tadicjob::{
    DataFile, ResultStore, TadicAdapter, TadicInput, TadicJobRequest, TadicLevel, TadicLimits,
    TadicRequest, WeightVectors,
};
use rustred_campaign::launch::RealLauncher;
use rustred_campaign::resources::{Sampler, SamplerConfig};
use rustred_campaign_contract::exit::ExitCode;
use rustred_campaign_contract::fake::FakeOp;
use rustred_campaign_contract::tadicjob::{RequestOutcome, certificate};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

const FAKE: &str = env!("CARGO_BIN_EXE_rr_fake_child");
const BIN_SHA: &str = "b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1";

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("tadicjob-{tag}-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&d);
    d
}

fn sampler() -> Arc<Sampler> {
    Arc::new(
        Sampler::spawn(SamplerConfig {
            interval: Duration::from_millis(200),
            collect_foreign_pinning: false,
        })
        .unwrap(),
    )
}

fn adapter(dir: &Path, directives: &[&str]) -> TadicAdapter {
    let drv = dir.join("tadic_job.py");
    std::fs::write(
        &drv,
        b"# a stand-in for the frozen driver (the fake child emulates it)\n",
    )
    .unwrap();
    let mut script = default_emulation_script();
    script.script = directives
        .iter()
        .map(|d| FakeOp::Fail {
            code: ExitCode::Input,
            message: d.to_string(),
        })
        .collect();
    if script.script.is_empty() {
        script.script = vec![FakeOp::Prepare { ms: 1 }];
    }
    TadicAdapter {
        program: Program::from_file("rr_fake_child", Path::new(FAKE), Invocation::Contract)
            .unwrap(),
        driver: DataFile::from_file(&drv).unwrap(),
        package_binary: (dir.join("p_key_batch_tadic"), BIN_SHA.into()),
        node_binary: (dir.join("rr_node_tadic"), BIN_SHA.into()),
        reader_features: vec![],
        licence_env: None,
        fake: Some(script),
    }
}

fn request(dir: &Path, seeds: &[u64]) -> TadicJobRequest {
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
        requests: vec![
            TadicRequest {
                label: "k0".into(),
                inputs: vec![k([2, 1, -1], 1)],
                weight_vectors: None,
            },
            TadicRequest {
                label: "k1".into(),
                inputs: vec![k([1, 2, 0], 1)],
                weight_vectors: None,
            },
            TadicRequest {
                label: "g".into(),
                inputs: vec![k([2, 1, -1], 3), k([1, 2, 0], -5)],
                weight_vectors: None,
            },
        ],
        d0: 4,
        order: 1,
        primes: 2,
        max_primes: 8,
        seeds: seeds.to_vec(),
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
        dependences: None,
        depths: None,
    }
}

fn run(
    tag: &str,
    directives: &[&str],
    seeds: &[u64],
) -> (
    rustred_campaign::adapters::tadicjob::TadicBatch,
    TadicAdapter,
    TadicJobRequest,
    rustred_campaign::adapters::job::AttemptReport,
) {
    let dir = tmp(tag);
    let ad = adapter(&dir, directives);
    let rq = request(&dir, seeds);
    let plans = ad.plan(std::slice::from_ref(&rq)).unwrap();
    let opts = RunOptions {
        cpus: vec![],
        memory_cap_bytes: Some(2 << 30),
        licence_env: None,
        wall_cap: Some(Duration::from_secs(60)),
        grace: Some(Duration::from_secs(5)),
    };
    let mut l = RealLauncher::new(sampler());
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
    (b, ad, rq, rep)
}

fn series_at_minus1(o: &RequestOutcome, basis: &str, prime: u64) -> u64 {
    let RequestOutcome::Series { terms, .. } = o else {
        panic!("not a series: {o:?}")
    };
    let t = terms.iter().find(|t| t.basis == basis).unwrap();
    let pp = t.per_prime.iter().find(|p| p.prime == prime).unwrap();
    pp.coefficient(-1).unwrap()
}

#[test]
fn ok_job_is_row_50_bound_certified_and_linear_in_the_weights() {
    let (b, _, _, _) = run("ok", &[], &[1, 2]);
    assert_eq!(b.classification.row, 50, "{:?}", b.unread);
    let r = b.result.as_ref().unwrap();
    assert_eq!(r.status, "OK");
    assert_eq!(r.requests.len(), 3);
    let cert = certificate::check(r);
    assert!(cert.findings.is_empty(), "{:?}", cert.findings);
    assert!(cert.job_certified());
    // linearity (the fake's rule): residue at t^-1 of M:fa = sum (i+1) k_i; g = 3 k0 - 5 k1
    for &p in &r.primes {
        let a0 = series_at_minus1(&r.requests[0].outcome, "M:fa", p) as i128;
        let a1 = series_at_minus1(&r.requests[1].outcome, "M:fa", p) as i128;
        let g = series_at_minus1(&r.requests[2].outcome, "M:fa", p) as i128;
        assert_eq!((3 * a0 - 5 * a1).rem_euclid(p as i128), g);
    }
}

#[test]
fn a_request_length_budget_is_typed_not_decided_and_never_zero() {
    let (b, _, _, _) = run("partial", &["REQ_LENGTH_BUDGET:k1"], &[1, 2]);
    assert_eq!(b.classification.row, 50, "{:?}", b.unread);
    let r = b.result.as_ref().unwrap();
    assert_eq!(r.status, "PARTIAL");
    assert!(r.requests[0].outcome.is_decided());
    assert_eq!(r.requests[1].outcome.word(), "not decided (LENGTH_BUDGET)");
    let cert = certificate::check(r);
    assert!(!cert.certified["k1"]);
    assert!(cert.certified["k0"]);
}

#[test]
fn a_job_level_engine_answer_is_row_50_with_every_request_not_decided() {
    let (b, _, _, _) = run("jobbudget", &["LENGTH_BUDGET"], &[1, 2]);
    assert_eq!(b.classification.row, 50, "{:?}", b.unread);
    let r = b.result.as_ref().unwrap();
    assert!(
        r.requests
            .iter()
            .all(|q| q.outcome.word() == "not decided (LENGTH_BUDGET)")
    );
}

#[test]
fn driver_exits_map_to_the_contract_rows() {
    for (dir, row) in [
        ("TIMEOUT", 9u8),
        ("OOM_KILLED", 10),
        ("JOB_INVALID", 52),
        ("ENGINE_FAILED", 17),
        ("WRONG_JOB_SHA", 51),
        ("NO_RESULT", 51),
    ] {
        let (b, _, rq, _) = run(&format!("exit-{dir}"), &[dir], &[1, 2]);
        assert_eq!(
            b.classification.row, row,
            "{dir}: {:?} {:?}",
            b.classification, b.unread
        );
        assert!(b.result.is_none(), "{dir}");
        let labels: Vec<String> = rq.requests.iter().map(|q| q.label.clone()).collect();
        for (_, o) in b.outcomes(&labels) {
            assert!(!o.is_decided(), "{dir}: nothing read is never decided");
        }
    }
}

#[test]
fn seeds_that_disagree_are_a_finding_and_one_seed_is_never_certified() {
    let (b, _, _, _) = run("disagree", &["SEED_DISAGREE:k0"], &[1, 2]);
    assert_eq!(b.classification.row, 50);
    let cert = certificate::check(b.result.as_ref().unwrap());
    assert!(
        cert.findings
            .iter()
            .any(|f| f.code() == "SEED_DISAGREEMENT" && f.label() == Some("k0"))
    );
    assert!(!cert.certified["k0"]);
    assert!(cert.certified["k1"]);
    let (b, _, _, _) = run("oneseed", &[], &[1]);
    let cert = certificate::check(b.result.as_ref().unwrap());
    assert!(cert.findings.iter().any(|f| f.code() == "SINGLE_SEED"));
    assert!(!cert.job_certified());
}

#[test]
fn the_store_resumes_a_verified_job_and_refuses_a_changed_one() {
    let (b, ad, rq, rep) = run("store", &[], &[1, 2]);
    assert_eq!(b.classification.row, 50);
    let store = ResultStore {
        root: tmp("store").join("store"),
    };
    let content = ad.content_of(&rq);
    assert!(store.get(&ad, &rq).unwrap().is_none());
    store.put(&content, &rep.prepared).unwrap();
    let (c, _, rec) = store.get(&ad, &rq).unwrap().unwrap();
    assert_eq!(c, *b.result.as_ref().unwrap());
    assert_eq!(rec.content, content);
    // a changed result file is refused (sha256 against STORE.json)
    let p = store.root.join(&content).join("tadic_result.json");
    let mut bytes = std::fs::read(&p).unwrap();
    bytes.push(b' ');
    std::fs::write(&p, bytes).unwrap();
    assert!(store.get(&ad, &rq).is_err());
}

#[test]
fn job_v2_weight_vectors_give_one_entry_per_vector_equal_to_plain_requests() {
    let dir = tmp("v2");
    let ad = adapter(&dir, &[]);
    let mut rq = request(&dir, &[1, 2]);
    // the request g with 2 explicit vectors: (3, -5) = g itself and (1, 0) = k0
    rq.requests[2].weight_vectors = Some(WeightVectors::Explicit {
        vectors: vec![vec![3, -5], vec![1, 0]],
    });
    rq.release_every = Some(1);
    let plans = ad.plan(std::slice::from_ref(&rq)).unwrap();
    let opts = RunOptions {
        cpus: vec![],
        memory_cap_bytes: Some(2 << 30),
        licence_env: None,
        wall_cap: Some(Duration::from_secs(60)),
        grace: Some(Duration::from_secs(5)),
    };
    let mut l = RealLauncher::new(sampler());
    let (b, _) = run_attempt(
        &mut l,
        &ad,
        std::slice::from_ref(&rq),
        &plans[0],
        &dir.join("work"),
        1,
        &opts,
    )
    .unwrap();
    assert_eq!(b.classification.row, 50, "{:?}", b.unread);
    let r = b.result.as_ref().unwrap();
    let labels: Vec<&str> = r.requests.iter().map(|q| q.label.as_str()).collect();
    assert_eq!(labels, vec!["k0", "k1", "g#0", "g#1"]);
    for &p in &r.primes {
        assert_eq!(
            series_at_minus1(&r.requests[3].outcome, "M:fa", p),
            series_at_minus1(&r.requests[0].outcome, "M:fa", p),
            "vector (1, 0) = k0"
        );
        let a0 = series_at_minus1(&r.requests[0].outcome, "M:fa", p) as i128;
        let a1 = series_at_minus1(&r.requests[1].outcome, "M:fa", p) as i128;
        assert_eq!(
            (3 * a0 - 5 * a1).rem_euclid(p as i128),
            series_at_minus1(&r.requests[2].outcome, "M:fa", p) as i128
        );
    }
    // malformed vectors are refused at plan time
    let mut bad = request(&dir, &[1, 2]);
    bad.requests[2].weight_vectors = Some(WeightVectors::Explicit {
        vectors: vec![vec![1]],
    });
    assert!(ad.plan(std::slice::from_ref(&bad)).is_err());
}

#[test]
fn plan_refuses_malformed_requests() {
    let dir = tmp("plan");
    let ad = adapter(&dir, &[]);
    let mut rq = request(&dir, &[1, 2]);
    rq.requests[1].label = "k0".into();
    assert!(
        ad.plan(std::slice::from_ref(&rq)).is_err(),
        "duplicate label"
    );
    let mut rq = request(&dir, &[1, 2]);
    rq.seeds.clear();
    assert!(ad.plan(std::slice::from_ref(&rq)).is_err(), "no seed");
    let mut rq = request(&dir, &[1, 2]);
    rq.requests[0].inputs = vec![TadicInput::Package {
        top: "X".into(),
        powers: vec![1, 1, 1],
        weight: 1i64 << 62,
    }];
    assert!(
        ad.plan(std::slice::from_ref(&rq)).is_err(),
        "weight out of range"
    );
    let mut rq = request(&dir, &[1, 2]);
    rq.requests[0].inputs = vec![TadicInput::Node {
        key: "I(1,1,1)".into(),
        weight: 1,
    }];
    assert!(
        ad.plan(std::slice::from_ref(&rq)).is_err(),
        "input of another level"
    );
    // the content id is deterministic and changes with the requests
    let a = ad.content_of(&request(&dir, &[1, 2]));
    assert_eq!(a, ad.content_of(&request(&dir, &[1, 2])));
    assert_ne!(a, ad.content_of(&request(&dir, &[1, 3])));
}

/// TRICKS 65 residual (int8, int/10): a package whose library.json REQUIRES a reader feature that the pinned binaries do
/// not declare is refused at PLANNING with the typed error (nothing is launched); declared, the job is planned; a
/// malformed declaration is an I/O error, never "nothing required". Planning only: no child, no namespace needed.
#[test]
fn a_package_requiring_an_undeclared_reader_feature_is_refused_at_planning() {
    use rustred_campaign::adapters::job::AdapterError;
    let dir = tmp("readerfeat");
    let mut ad = adapter(&dir, &[]);
    let rq = request(&dir, &[1]);
    // the package states what it requires (request() wrote a library.json without the field: nothing required)
    assert!(ad.plan(std::slice::from_ref(&rq)).is_ok());
    std::fs::write(
        dir.join("library.json"),
        b"{\"terminals\": [\"unit\"], \"required_features\": [\"slot_aliases\", \"alias_equations\"]}",
    )
    .unwrap();
    ad.reader_features = vec!["slot_aliases".into()];
    match ad.plan(std::slice::from_ref(&rq)) {
        Err(AdapterError::ReaderFeatures {
            missing, supported, ..
        }) => {
            assert_eq!(missing, vec!["alias_equations".to_string()]);
            assert_eq!(supported, vec!["slot_aliases".to_string()]);
        }
        other => panic!("expected the typed reader-features refusal, got {other:?}"),
    }
    ad.reader_features = vec!["slot_aliases".into(), "alias_equations".into()];
    assert!(ad.plan(std::slice::from_ref(&rq)).is_ok());
    std::fs::write(
        dir.join("library.json"),
        b"{\"required_features\": \"slot_aliases\"}",
    )
    .unwrap();
    assert!(matches!(
        ad.plan(std::slice::from_ref(&rq)),
        Err(AdapterError::Io(_))
    ));
}
