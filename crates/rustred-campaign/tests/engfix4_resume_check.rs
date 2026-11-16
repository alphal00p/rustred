//! Lane engfix4 (re-audit MJ-1): a resumed `rr_node job` attempt uses RESUME.json only where it equals what the
//! copied committed prefixes PROVE (the queue log replayed through the driver's own queue operations, the table
//! re-read). Every SELF-CONSISTENT lie below (RESUME rewritten and re-digested by "whoever wrote it", the contract's
//! digest binding satisfied) must be refused with exit 41 (row 13, nothing accepted), and the honest RESUME must still
//! resume to the canonical result of the uninterrupted run.
//!
//! Needs (all `#[ignore]`): `RR_NODE_BIN`, `RUSTRED_CAMPAIGN_FIXTURES`, the licence environment, `LIGHT_PIDNS=1`,
//! `RR_REAL_OUT`. Helpers: the audit-eng2 test module (real child, contract classifier).
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

mod audit_eng2_common;
use audit_eng2_common::*;

use rustred_campaign_contract::classify::{Accept, KillReason, WaitStatus};
use rustred_campaign_contract::layout::RunLayout;
use rustred_campaign_contract::resume::ResumeRef;
use rustred_campaign_contract::spec::JobSpec;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn run_to_end(spec: &JobSpec) -> WaitStatus {
    let mut c = spawn(spec);
    wait(&mut c)
}

/// Spawn, wait for `k` case starts (numeric or all), SIGTERM, wait.
fn stop_after(spec: &JobSpec, k: usize, numeric: bool) -> WaitStatus {
    let mut c = spawn(spec);
    let t = Instant::now();
    while units(&events_of(spec), numeric) < k && t.elapsed() < Duration::from_secs(300) {
        if c.try_wait().unwrap().is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    signal(&c, libc::SIGTERM);
    wait(&mut c)
}

fn attempt_in(root: &Path, prev: &JobSpec, attempt: u32, dir: &str, from: ResumeRef) -> JobSpec {
    let run = root.join(dir);
    std::fs::create_dir_all(&run).unwrap();
    let mut s = prev.clone();
    s.attempt = attempt;
    s.run_dir = run.to_str().unwrap().into();
    s.resume = Some(from);
    s
}

fn rref(p: &Path) -> ResumeRef {
    ResumeRef {
        from: p.to_str().unwrap().into(),
        blake3: b3(p),
    }
}

fn table_canon(spec: &JobSpec) -> Option<String> {
    canonical_of(spec, "F.table.jsonl")
}

fn note(v: Value) {
    eprintln!("ENGFIX4-PROBE {v}");
}

/// One lie: the previous attempt's RESUME replaced by `lie` (re-digested), resumed by attempt `attempt`: 41, row 13,
/// nothing accepted, the refusal names the resume check.
fn expect_refused(root: &Path, prev: &JobSpec, attempt: u32, name: &str, lie: &Value) -> Value {
    let res_path = RunLayout::new(&prev.run_dir).resume();
    let original = std::fs::read(&res_path).unwrap();
    std::fs::write(&res_path, serde_json::to_vec(lie).unwrap()).unwrap();
    let s = attempt_in(root, prev, attempt, &format!("lie-{name}"), rref(&res_path));
    let w = run_to_end(&s);
    std::fs::write(&res_path, &original).unwrap();
    let cl = classified(&s, w, KillReason::None);
    let msg = outcome(&s)
        .and_then(|o| o.job.error.map(|e| e.message))
        .unwrap_or_default();
    let v = json!({"lie": name, "wait": format!("{w:?}"), "row": cl.row, "accept": format!("{:?}", cl.accept),
                   "message": msg.chars().take(300).collect::<String>()});
    note(v.clone());
    assert_eq!(w, WaitStatus::Exited(41), "lie {name}: {v}");
    assert_eq!(cl.row, 13, "lie {name}: {v}");
    assert_eq!(cl.accept, Accept::Nothing, "lie {name}: {v}");
    assert!(
        msg.contains("resume refused"),
        "lie {name}: the refusal names the resume check: {v}"
    );
    v
}

fn l4a() -> Fx {
    Fx {
        tag: "l4a",
        node: "L4a",
        args: vec!["--order".into(), "id".into()],
        cases: vec![],
        resume_every: vec![0],
        stop_grace_s: 5,
        numeric_units: false,
    }
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let t = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &t);
        } else {
            std::fs::copy(e.path(), &t).unwrap();
        }
    }
}

/// The committed-prefix figures of `bytes` as RESUME's `CommittedFile` JSON.
fn committed_json(path_rel: &str, bytes: &[u8], scratch: &Path) -> Value {
    std::fs::write(scratch, bytes).unwrap();
    let (canon, _) = rustred_campaign_contract::canon::canonical_digest_file(scratch).unwrap();
    let lines: Vec<Value> = bytes
        .split(|&b| b == b'\n')
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_slice(l).unwrap())
        .collect();
    json!({"path": path_rel, "bytes": bytes.len(), "blake3": rustred_campaign_contract::blake3_hex(bytes),
           "records": lines.len(), "rules": lines.iter().filter(|v| v["type"] == "rule").count(),
           "canonical_blake3": canon})
}

/// L4a (symbolic queue): a dozen self-consistent lies about the open queue, the counters, the descent record and the
/// committed table are refused (41); the honest RESUME resumes to the uninterrupted table.
#[test]
#[ignore]
fn engfix4_lying_resume_refused_l4a() {
    let r = root("e4-lying");
    let fx = l4a();
    let full = job_spec(&r, "ref", 1, &fx, 0);
    assert_eq!(run_to_end(&full), WaitStatus::Exited(0));
    let want = table_canon(&full).unwrap();
    let a1 = job_spec(&r, "A", 1, &fx, 0);
    assert_eq!(stop_after(&a1, 30, false), WaitStatus::Exited(30));
    let res_path = RunLayout::new(&a1.run_dir).resume();
    let res: Value = serde_json::from_slice(&std::fs::read(&res_path).unwrap()).unwrap();
    let pending = res["pending"].as_array().cloned().unwrap_or_default();
    note(
        json!({"resume": {"current": !res["current"].is_null(), "pending": pending.len(),
        "numerical": res["numerical"].as_array().map(Vec::len), "rules": res["rules_committed"],
        "engine": res["engine"]}}),
    );
    assert!(
        pending.len() >= 2,
        "the stop must leave an open queue to lie about"
    );
    let mut n = 2u32;
    let mut lie = |name: &str, f: &dyn Fn(&mut Value)| {
        let mut v = res.clone();
        f(&mut v);
        assert_ne!(v, res, "lie {name} changes RESUME");
        expect_refused(&r, &a1, n, name, &v);
        n += 1;
    };
    lie("queue_dropped", &|v| {
        v["pending"] = json!([]);
        v["current"] = Value::Null;
    });
    lie("current_dropped", &|v| v["current"] = Value::Null);
    lie("last_pending_dropped", &|v| {
        v["pending"].as_array_mut().unwrap().pop();
    });
    lie("pending_swapped", &|v| {
        v["pending"].as_array_mut().unwrap().swap(0, 1)
    });
    lie("pending_duplicated", &|v| {
        let a = v["pending"].as_array_mut().unwrap();
        let first = a[0].clone();
        a.push(first);
    });
    lie("numeric_endpoint_planted", &|v| {
        let arity = v["pending"][0]["fixed"].as_array().unwrap().len();
        let mut p = vec![json!(1); arity];
        p[arity - 1] = json!(0);
        v["numerical"].as_array_mut().unwrap().push(Value::from(p));
    });
    lie("deferred_planted", &|v| {
        let chart = v["pending"][0].clone();
        v["deferred"] =
            json!([{"chart": chart, "detail": {"kind": "region", "rule": 0, "branch": 0}}]);
    });
    lie("discarded_plus_one", &|v| {
        let d = v["discarded"].as_u64().unwrap() + 1;
        v["discarded"] = json!(d);
        v["engine"]["discarded"] = json!(d);
    });
    lie("rhs_terms_plus_one", &|v| {
        let d = v["engine"]["rhs_terms"].as_u64().unwrap() + 1;
        v["engine"]["rhs_terms"] = json!(d);
    });
    lie("symbolic_cases_plus_one", &|v| {
        let d = v["symbolic_cases"].as_u64().unwrap() + 1;
        v["symbolic_cases"] = json!(d);
    });
    lie("work_counter_rhs_terms_plus_one", &|v| {
        let d = v["counters_at_commit"]["rhs_terms"].as_u64().unwrap_or(0) + 1;
        v["counters_at_commit"]["rhs_terms"] = json!(d);
    });
    lie("not_lower_planted", &|v| {
        let k = v["pending"][0]["fixed"].as_array().unwrap().len();
        let key = format!("I({})", vec!["1"; k].join(","));
        v["engine"]["not_lower"] = json!([key]);
    });
    // a committed rule made NOT LOWER in a copy of the previous attempt's table prefix (audit-eng2 (j)), every figure
    // of RESUME recomputed: refused by the descent re-check and the queue-log record of the rule (RHS size)
    let t_path = RunLayout::new(&a1.run_dir).table();
    let table = std::fs::read(&t_path).unwrap();
    let pre_len = res["table"]["bytes"].as_u64().unwrap() as usize;
    let mut out = Vec::new();
    let mut done = false;
    for line in table[..pre_len].split_inclusive(|&b| b == b'\n') {
        let mut v: Value = serde_json::from_slice(line).unwrap();
        if !done && v["type"] == "rule" && v["index"] == 3 {
            let t = v["target"].clone();
            v["rhs"].as_array_mut().unwrap().push(json!([t, "1"]));
            done = true;
            out.extend_from_slice(serde_json::to_string(&v).unwrap().as_bytes());
            out.push(b'\n');
        } else {
            out.extend_from_slice(line);
        }
    }
    assert!(done, "rule 3 in the committed prefix");
    let pdir: PathBuf = r.join("A.a1-planted");
    copy_dir(Path::new(&a1.run_dir), &pdir);
    let mut planted = a1.clone();
    planted.run_dir = pdir.to_str().unwrap().into();
    let jp = RunLayout::new(&planted.run_dir).job_spec();
    let mut perm = std::fs::metadata(&jp).unwrap().permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    perm.set_readonly(false);
    std::fs::set_permissions(&jp, perm).unwrap();
    std::fs::write(&jp, planted.to_json_line()).unwrap();
    let mut whole = out.clone();
    whole.extend_from_slice(&table[pre_len..]);
    std::fs::write(RunLayout::new(&planted.run_dir).table(), &whole).unwrap();
    let mut v = res.clone();
    v["table"] = committed_json("out/F.table.jsonl", &out, &r.join("scratch.prefix"));
    v["spec_blake3"] = json!(rustred_campaign_contract::spec::spec_blake3(
        &std::fs::read(&jp).unwrap()
    ));
    expect_refused(&r, &planted, 40, "not_lower_rule_in_table_prefix", &v);
    // the honest RESUME (unchanged) resumes to the uninterrupted table: no false refusal
    let honest = attempt_in(&r, &a1, 50, "honest", rref(&res_path));
    assert_eq!(run_to_end(&honest), WaitStatus::Exited(0));
    assert_eq!(table_canon(&honest).as_deref(), Some(want.as_str()));
    note(json!({"honest_resume": "exit 0, canonical table equal"}));
}

/// The numeric stage (audit-eng2 C1 family): lies about the exhausted endpoints, the stage size and the endpoints
/// still to search are refused (41); the honest RESUME resumes to the uninterrupted table.
#[test]
#[ignore]
fn engfix4_lying_resume_refused_numeric_stage() {
    let r = root("e4-lying-num");
    let cases: Vec<Value> = [
        [1, 1, 1, 1, 1, 1, 1, 1, 0],
        [2, 1, 1, 1, 1, 1, 1, 1, 0],
        [1, 2, 1, 1, 1, 1, 1, 1, 0],
        [1, 1, 1, 2, 1, 1, 1, 1, 0],
        [1, 1, 1, 1, 1, 1, 1, 1, -1],
        [1, 1, 1, 1, 1, 2, 1, 1, 0],
        [1, 1, 1, 1, 1, 1, 1, 2, 0],
        [2, 1, 1, 1, 1, 1, 1, 1, -1],
    ]
    .iter()
    .map(|p| json!({"frame": "ORIGINAL", "fixed": p, "equations": []}))
    .collect();
    let fx = Fx {
        tag: "l3anum",
        node: "L3a",
        args: [
            "--order",
            "id",
            "--numeric-depth",
            "0",
            "--numeric-depth-max",
            "3",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect(),
        cases,
        resume_every: vec![0],
        stop_grace_s: 5,
        numeric_units: true,
    };
    let full = job_spec(&r, "ref", 1, &fx, 0);
    assert_eq!(run_to_end(&full), WaitStatus::Exited(0));
    let want = table_canon(&full).unwrap();
    let mut got = None;
    for k in 0..12 {
        let a = job_spec(&r, &format!("N{k}"), 1, &fx, 0);
        if stop_after(&a, 8, true) != WaitStatus::Exited(30) {
            continue;
        }
        let res: Value =
            serde_json::from_slice(&std::fs::read(RunLayout::new(&a.run_dir).resume()).unwrap())
                .unwrap();
        if res["engine"]["numeric_stage"]["exhausted"]
            .as_array()
            .is_some_and(|x| !x.is_empty())
        {
            got = Some((a, res));
            break;
        }
    }
    let Some((a, res)) = got else {
        note(json!({"skipped": "no stop landed after the exhausted master"}));
        panic!("no stop landed after the exhausted endpoint in 12 tries");
    };
    note(
        json!({"resume": {"numerical": res["numerical"], "numeric_stage": res["engine"]["numeric_stage"]}}),
    );
    let mut n = 2u32;
    let mut lie = |name: &str, f: &dyn Fn(&mut Value)| {
        let mut v = res.clone();
        f(&mut v);
        assert_ne!(v, res, "lie {name} changes RESUME");
        expect_refused(&r, &a, n, name, &v);
        n += 1;
    };
    lie("exhausted_dropped", &|v| {
        v["engine"]["numeric_stage"]["exhausted"] = json!([])
    });
    lie("stage_size_plus_one", &|v| {
        let t = v["engine"]["numeric_stage"]["numerical_cases"]
            .as_u64()
            .unwrap()
            + 1;
        v["engine"]["numeric_stage"]["numerical_cases"] = json!(t);
    });
    lie("numeric_stage_removed", &|v| {
        v["engine"].as_object_mut().unwrap().remove("numeric_stage");
    });
    if res["numerical"].as_array().is_some_and(|a| !a.is_empty()) {
        lie("next_endpoint_dropped", &|v| {
            v["numerical"].as_array_mut().unwrap().remove(0);
        });
    }
    let honest = attempt_in(
        &r,
        &a,
        50,
        "honest",
        rref(&RunLayout::new(&a.run_dir).resume()),
    );
    assert_eq!(run_to_end(&honest), WaitStatus::Exited(0));
    assert_eq!(table_canon(&honest).as_deref(), Some(want.as_str()));
    note(json!({"honest_resume": "exit 0, canonical table equal"}));
}
