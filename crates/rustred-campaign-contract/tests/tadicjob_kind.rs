//! Kind `tadic-replay` (lane tadicjob; interfaces/tadic-job.md 2, 3): the spec rules of the kind, the binding of a
//! driver result to its job (every refusal case), the driver-exit mapping table, the certificate checks C2-C6 and the
//! sealed-line check. Pure: no child process. Test data only (L8): keys of arity 3, made-up digests and true primes.

mod common;

use common::*;
use rustred_campaign_contract::spec::*;
use rustred_campaign_contract::tadicjob::{
    self, Answer, Pins, RequestOutcome, WrapDecision, certificate, check_result, decide,
};
use serde_json::{Value, json};

const P1: u64 = 2_305_843_009_213_693_951;
const P2: u64 = 4_611_686_018_427_387_847;
const P3: u64 = 1_152_921_504_606_846_883;
const P4: u64 = 576_460_752_303_423_433;
const BIN: &str = "b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1";
const DRV: &str = "d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2";
const JOB: &str = "a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3";

fn job() -> Value {
    json!({"schema": tadicjob::JOB_SCHEMA, "job_id": "j1", "level": "package",
        "requests": [{"label": "a", "inputs": []}, {"label": "b", "inputs": []}],
        "expansion": {"d0": 4, "order": 1}, "primes": {"count": 2, "seeds": [1, 2]}})
}

fn pins() -> Pins {
    Pins {
        job_sha256: JOB.into(),
        binary_sha256: BIN.into(),
        driver_sha256: DRV.into(),
    }
}

fn pp(p: u64, seed: u64, v: Option<i64>, prec: i64) -> Value {
    match v {
        None => {
            json!({"prime": p.to_string(), "seed": seed, "valuation": null, "precision": prec, "coefficients": []})
        }
        Some(v) => {
            let mut c: Vec<String> = vec!["5".into(); (prec - v) as usize];
            c[0] = "3".into();
            json!({"prime": p.to_string(), "seed": seed, "valuation": v, "precision": prec, "coefficients": c})
        }
    }
}

fn term(basis: &str, vals: [Option<i64>; 4]) -> Value {
    json!({"basis": basis, "tag": "", "kind": "residual", "answer": "IGNORED", "per_prime": [
        pp(P1, 1, vals[0], 1), pp(P2, 1, vals[1], 1), pp(P3, 2, vals[2], 1), pp(P4, 2, vals[3], 1)]})
}

fn cert(seed: u64) -> Value {
    json!({"seed": seed, "trace_digests": ["t"], "plan_digests": ["p"], "plan_max_need": 3})
}

fn result() -> Value {
    json!({"schema": tadicjob::RESULT_SCHEMA, "job_id": "j1", "job_sha256": JOB, "status": "OK", "level": "package",
        "binary": {"sha256": BIN}, "driver": {"sha256": DRV}, "expansion": {"d0": 4, "order": 1},
        "primes": [P1.to_string(), P2.to_string(), P3.to_string(), P4.to_string()],
        "requests": [
            {"label": "a", "status": "OK", "terms": [term("R:1", [Some(-2); 4]), term("U", [Some(0); 4])],
             "certificate": [cert(1), cert(2)]},
            {"label": "b", "status": "OK", "terms": [term("R:2", [None; 4])], "certificate": [cert(1), cert(2)]}]})
}

#[test]
fn spec_rules_of_the_kind() {
    let k = JobKind::TadicReplay;
    assert!(k.legacy_wrapped() && k.fake_emulated());
    assert_eq!(k.level(), Level::Legacy);
    assert!(k.allowed_args().is_empty());
    let r = k.input_roles().unwrap();
    assert_eq!(r.required, &["tadic_job", "driver"]);
    let t = TempDir::new("tadickind");
    let mut spec = sample_spec(&t.path().join("run").display().to_string());
    spec.kind = k;
    spec.program = ProgramRef {
        name: "tadic_job".into(),
        blake3: D0.into(),
        features_required: vec![],
    };
    spec.budgets = Budgets::default();
    let inp = |role: &str| InputRef {
        role: role.into(),
        path: format!("in/{role}"),
        blake3: D0.into(),
        upto: None,
    };
    spec.inputs = vec![inp("tadic_job"), inp("driver"), inp("terminals")];
    spec.args = vec![];
    spec.validate().unwrap();
    spec.inputs = vec![inp("tadic_job")];
    assert!(spec.validate().is_err(), "the driver input is required");
    spec.inputs = vec![inp("tadic_job"), inp("driver"), inp("bogus")];
    assert!(spec.validate().is_err(), "unknown role");
    spec.inputs = vec![inp("tadic_job"), inp("driver")];
    spec.args = vec!["--x".into()];
    assert!(spec.validate().is_err(), "empty allow-list");
    // argv: python -I <driver> run <job> <run>/out/tadic_result.json
    spec.args = vec![];
    let a = tadicjob::argv(&spec).unwrap();
    assert_eq!(a[0], "-I");
    assert_eq!(a[2], "run");
    assert!(a[4].ends_with("/out/tadic_result.json"));
}

#[test]
fn a_bound_result_is_rederived_and_typed() {
    let c = check_result(&job(), &result(), &pins()).unwrap();
    assert_eq!(c.requests.len(), 2);
    let RequestOutcome::Series {
        terms,
        max_pole_order_lower_bound,
        precision,
    } = &c.requests[0].outcome
    else {
        panic!()
    };
    assert_eq!(*max_pole_order_lower_bound, 2);
    assert_eq!(*precision, 1);
    assert_eq!(
        terms[0].answer,
        Answer::Pole(2),
        "the driver's 'answer' is never read"
    );
    assert_eq!(terms[1].answer, Answer::NoPoleModP);
    let RequestOutcome::Series { terms, .. } = &c.requests[1].outcome else {
        panic!()
    };
    assert_eq!(terms[0].answer, Answer::ZeroModPToPrecision);
    let rep = certificate::check(&c);
    assert!(rep.findings.is_empty(), "{:?}", rep.findings);
    assert!(rep.job_certified());
}

#[test]
fn binding_refusals() {
    let j = job();
    let refuse = |f: &dyn Fn(&mut Value)| {
        let mut r = result();
        f(&mut r);
        check_result(&j, &r, &pins()).is_err()
    };
    assert!(refuse(&|r| r["job_sha256"] = json!("00")), "another job");
    assert!(
        refuse(&|r| r["binary"]["sha256"] = json!("00")),
        "another binary"
    );
    assert!(
        refuse(&|r| r["driver"]["sha256"] = json!("00")),
        "another driver"
    );
    assert!(refuse(&|r| r["schema"] = json!("x")), "schema");
    assert!(refuse(&|r| r["job_id"] = json!("j2")), "job id");
    assert!(
        refuse(&|r| r["requests"].as_array_mut().unwrap().reverse()),
        "labels out of order"
    );
    assert!(
        refuse(&|r| r["requests"].as_array_mut().unwrap().truncate(1)),
        "a request missing"
    );
    assert!(
        refuse(&|r| r["status"] = json!("PARTIAL")),
        "PARTIAL without an ERROR request"
    );
    assert!(
        refuse(&|r| r["expansion"]["order"] = json!(0)),
        "another order"
    );
    assert!(
        refuse(
            &|r| r["requests"][0]["terms"][0]["per_prime"][0]["coefficients"][1] =
                json!(P1.to_string())
        ),
        "a residue not below its prime"
    );
    assert!(
        refuse(&|r| r["requests"][0]["terms"][0]["per_prime"][0]["coefficients"][0] = json!("0")),
        "a zero coefficient at the valuation"
    );
    assert!(
        refuse(&|r| {
            r["requests"][0]["terms"][0]["per_prime"][0]["coefficients"]
                .as_array_mut()
                .unwrap()
                .pop();
        }),
        "a coefficient count that is not precision - valuation"
    );
    assert!(
        refuse(&|r| r["requests"][0]["terms"][0]["per_prime"][0]["prime"] = json!("97")),
        "a prime not among the result's"
    );
    // an ERROR request is typed (PARTIAL), never zero
    let mut r = result();
    r["status"] = json!("PARTIAL");
    r["requests"][1] = json!({"label": "b", "status": "ERROR", "error": {"code": "LENGTH_BUDGET", "message": "m"}});
    let c = check_result(&j, &r, &pins()).unwrap();
    assert_eq!(c.requests[1].outcome.word(), "not decided (LENGTH_BUDGET)");
    r["requests"][1]["error"]["code"] = json!("ERASURE_FAILED");
    let c = check_result(&j, &r, &pins()).unwrap();
    assert_eq!(
        c.requests[1].outcome.word(),
        "not decided (PARTIAL:ERASURE_FAILED)"
    );
}

#[test]
fn the_exit_mapping_table() {
    let ok = check_result(&job(), &result(), &pins());
    let err = |code: &str| {
        check_result(
            &job(),
            &json!({"schema": tadicjob::RESULT_SCHEMA, "job_id": "j1", "job_sha256": JOB, "status": "ERROR",
                "error": {"code": code, "message": "m"}}),
            &pins(),
        )
    };
    assert_eq!(decide(0, &ok), WrapDecision::Seal);
    assert_eq!(decide(3, &err("LENGTH_BUDGET")), WrapDecision::Seal);
    assert!(matches!(
        decide(3, &err("TIMEOUT")),
        WrapDecision::Killed { exit: 21, .. }
    ));
    assert!(matches!(
        decide(3, &err("OOM_KILLED")),
        WrapDecision::Killed { exit: 22, .. }
    ));
    assert!(matches!(
        decide(2, &err("JOB_INVALID")),
        WrapDecision::Refused { .. }
    ));
    assert!(matches!(
        decide(2, &err("UNSUPPORTED")),
        WrapDecision::Refused { .. }
    ));
    assert!(matches!(
        decide(3, &err("INPUT_HASH_MISMATCH")),
        WrapDecision::Refused { .. }
    ));
    assert!(matches!(
        decide(3, &err("ENGINE_FAILED")),
        WrapDecision::Crashed { .. }
    ));
    assert!(matches!(
        decide(0, &Err("x".into())),
        WrapDecision::Unverified { .. }
    ));
    assert!(matches!(
        decide(3, &Err("x".into())),
        WrapDecision::Crashed { .. }
    ));
    assert!(
        matches!(
            decide(0, &err("LENGTH_BUDGET")),
            WrapDecision::Unverified { .. }
        ),
        "exit 0 with ERROR"
    );
    // a job-level engine answer leaves every request NOT_DECIDED with its code
    let c = err("LENGTH_BUDGET").unwrap();
    assert!(
        c.requests
            .iter()
            .all(|q| q.outcome.word() == "not decided (LENGTH_BUDGET)")
    );
}

#[test]
fn certificate_findings() {
    let j = job();
    let check = |f: &dyn Fn(&mut Value)| {
        let mut r = result();
        f(&mut r);
        certificate::check(&check_result(&j, &r, &pins()).unwrap())
    };
    let r = check(&|r| {
        r["requests"][0]["terms"][0] = term("R:1", [Some(-2), Some(-1), Some(-2), Some(-2)])
    });
    assert!(r.findings.iter().any(|f| f.code() == "PRIME_DISAGREEMENT"));
    assert!(!r.certified["a"] && r.certified["b"]);
    let r = check(&|r| {
        r["requests"][0]["terms"][0] = term("R:1", [Some(-2), Some(-2), Some(-1), Some(-1)])
    });
    assert!(r.findings.iter().any(|f| f.code() == "SEED_DISAGREEMENT"));
    // a term absent at one seed counts as zero there
    let r = check(&|r| {
        r["requests"][0]["terms"][0]["per_prime"]
            .as_array_mut()
            .unwrap()
            .truncate(2);
    });
    assert!(r.findings.iter().any(|f| f.code() == "SEED_DISAGREEMENT"));
    let r = check(&|r| {
        r["requests"][0]["certificate"] = json!([cert(1), {"seed": 2, "plan_digests": ["p"]}])
    });
    assert!(r.findings.iter().any(|f| f.code() == "MISSING_DIGEST"));
    let r = check(&|r| r["requests"][1]["terms"][0]["per_prime"][2]["seed"] = json!(1));
    assert!(r.findings.iter().any(|f| f.code() == "SEED_PRIMES_OVERLAP"));
    assert!(!r.job_certified());
    // C6: a rerun differs only in timing and argv: identical; a residue change is found
    let a = result();
    let mut b = result();
    b["seconds"] = json!(9.0);
    b["requests"][0]["certificate"][0]["seconds"] = json!(1.0);
    assert!(certificate::compare_rerun(&a, &b).is_empty());
    b["requests"][0]["terms"][0]["per_prime"][0]["coefficients"][1] = json!("6");
    assert_eq!(
        certificate::compare_rerun(&a, &b),
        vec!["a: terms".to_string()]
    );
}

#[test]
fn sealed_lines_check() {
    let c = check_result(&job(), &result(), &pins()).unwrap();
    let mut lines: Vec<Vec<u8>> = c
        .requests
        .iter()
        .map(|q| {
            tadicjob::request_line(q, Some("x"))
                .to_string()
                .into_bytes()
        })
        .collect();
    lines.push(
        json!({"type": "end", "records": 2, "job_blake3": "jb", "result_blake3": "rb"})
            .to_string()
            .into_bytes(),
    );
    assert_eq!(
        tadicjob::check_sealed(&lines, &job(), "jb", "rb")
            .unwrap()
            .len(),
        2
    );
    assert!(
        tadicjob::check_sealed(&lines, &job(), "other", "rb").is_err(),
        "another job"
    );
    assert!(
        tadicjob::check_sealed(&lines, &job(), "jb", "other").is_err(),
        "another result file"
    );
    lines.swap(0, 1);
    assert!(
        tadicjob::check_sealed(&lines, &job(), "jb", "rb").is_err(),
        "labels out of order"
    );
}
