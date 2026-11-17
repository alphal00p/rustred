//! `rr_fake_child` emulating kind `tadic-replay` (lane tadicjob; interfaces/tadic-job.md) like helper VH 2's driver
//! `tadic_job.py run JOB RESULT`: reads the pinned job file, writes `out/tadic_result.json` (schema
//! `helper-vh2.tadic-result.v1`) and exits with the driver's codes 0 / 1 / 2 / 3, WITHOUT OUTCOME (a legacy child: the
//! supervisor-side wrapper binds, maps and seals).
//!
//! TEST RULE (a stand-in for the replay, LINEAR in the weights so that ranks are known by construction): a key with
//! integer entries k_0..k_{n-1} contributes to two terms, basis `M:fa` and `M:fb`: at t^-1 the residues
//! sum_i (i+1) k_i and k_0, at every order 0..order-1 the residue sum_i k_i (mod each prime); a request is the
//! weighted sum. Primes: two per seed from a fixed list of true primes (seed s uses pair s mod 3), disjoint across
//! seeds 0, 1, 2. Script ops: `fail {message}` = a directive: `JOB_INVALID` (exit 2), `TIMEOUT` / `OOM_KILLED` /
//! `ENGINE_FAILED` / `LENGTH_BUDGET` (job-level, exit 3), `REQ_LENGTH_BUDGET:<label>` (that request ERROR, exit 1),
//! `WRONG_JOB_SHA` (a result bound to another job), `SEED_DISAGREE:<label>` (the last seed's series of that request
//! starts one order later), `NO_RESULT` (exit 0, no file); `prepare {ms}` = a delay.

use rustred_campaign_contract::fake::{FakeOp, FakeScript};
use serde_json::{Value, json};
use std::path::Path;
use std::time::Duration;

const PRIMES: [u64; 6] = [
    2_305_843_009_213_693_951, // 2^61 - 1
    4_611_686_018_427_387_847, // 2^62 - 57
    1_152_921_504_606_846_883, // 2^60 - 93
    576_460_752_303_423_433,   // 2^59 - 55
    9_223_372_036_854_775_783, // 2^63 - 25
    288_230_376_151_711_717,   // 2^58 - 27
];

fn die(msg: &str) -> ! {
    eprintln!("fake tadic driver: {msg}");
    std::process::exit(3)
}

fn sha256_hex(b: &[u8]) -> String {
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    h.update(b);
    h.finish_hex()
}

fn modp(x: i128, p: u64) -> u64 {
    x.rem_euclid(p as i128) as u64
}

fn key_ints(input: &Value) -> Vec<i64> {
    if let Some(p) = input["powers"].as_array() {
        return p.iter().filter_map(Value::as_i64).collect();
    }
    let k = input["key"].as_str().unwrap_or("");
    k.trim_start_matches("I(")
        .trim_end_matches(')')
        .split(',')
        .filter_map(|x| x.trim().parse::<i64>().ok())
        .collect()
}

/// Residues of one key for orders -1 .. order-1 at prime p: (term a, term b).
fn key_series(k: &[i64], order: i64, p: u64) -> (Vec<u64>, Vec<u64>) {
    let n = (order + 1).max(0) as usize;
    let lin: i128 = k
        .iter()
        .enumerate()
        .map(|(i, &x)| (i as i128 + 1) * x as i128)
        .sum();
    let first = k.first().copied().unwrap_or(0) as i128;
    let sum: i128 = k.iter().map(|&x| x as i128).sum();
    let mut a = vec![modp(lin, p)];
    let mut b = vec![modp(first, p)];
    for _ in 1..n {
        a.push(modp(sum, p));
        b.push(modp(sum, p));
    }
    (a, b)
}

fn per_prime(series: &[u64], p: u64, seed: u64, order: i64, shift: i64) -> Value {
    // series[j] = residue of order j - 1; `shift` drops the first orders (a corrupted seed)
    let mut start = None;
    for (j, &c) in series.iter().enumerate() {
        if c != 0 && (j as i64) >= shift {
            start = Some(j);
            break;
        }
    }
    match start {
        None => {
            json!({"prime": p.to_string(), "seed": seed, "valuation": null, "precision": order, "coefficients": []})
        }
        Some(j) => {
            json!({"prime": p.to_string(), "seed": seed, "valuation": j as i64 - 1, "precision": order,
            "coefficients": series[j..].iter().map(|c| c.to_string()).collect::<Vec<_>>()})
        }
    }
}

pub fn run(spec_path: &Path) -> ! {
    let spec: Value = std::fs::read(spec_path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_else(|| die("unreadable job.json"));
    let run_dir = spec["run_dir"].as_str().unwrap_or(".").to_string();
    let script: Option<FakeScript> = serde_json::from_value(spec["fake"].clone()).ok();
    let mut directives: Vec<String> = vec![];
    for op in script.iter().flat_map(|s| s.script.iter()) {
        match op {
            FakeOp::Fail { message, .. } => directives.push(message.clone()),
            FakeOp::Prepare { ms } => std::thread::sleep(Duration::from_millis(*ms)),
            _ => {}
        }
    }
    let has = |d: &str| directives.iter().any(|x| x == d);
    let arg = |prefix: &str| -> Vec<String> {
        directives
            .iter()
            .filter_map(|x| x.strip_prefix(prefix).map(str::to_string))
            .collect()
    };
    let jp = spec["inputs"]
        .as_array()
        .and_then(|a| a.iter().find(|i| i["role"] == "tadic_job"))
        .and_then(|i| i["path"].as_str())
        .unwrap_or_else(|| die("no tadic_job input"));
    let jp = if Path::new(jp).is_absolute() {
        Path::new(jp).to_path_buf()
    } else {
        Path::new(&run_dir).join(jp)
    };
    let jb = std::fs::read(&jp).unwrap_or_else(|e| die(&format!("job: {e}")));
    let job: Value = serde_json::from_slice(&jb).unwrap_or_else(|e| die(&format!("job: {e}")));
    let out = Path::new(&run_dir).join("out").join("tadic_result.json");
    let mut job_sha = sha256_hex(&jb);
    if has("WRONG_JOB_SHA") {
        job_sha = sha256_hex(b"another job");
    }
    let base = json!({"schema": "helper-vh2.tadic-result.v1", "job_id": job["job_id"], "job_sha256": job_sha,
        "written_utc": "1970-01-01T00:00:00Z", "seconds": 0.0});
    let write = |v: &Value, code: i32| -> ! {
        if let Err(e) = std::fs::write(&out, serde_json::to_vec_pretty(v).unwrap()) {
            die(&format!("{}: {e}", out.display()));
        }
        std::process::exit(code)
    };
    let error = |code: &str, exit: i32| -> ! {
        let mut v = base.clone();
        v["status"] = json!("ERROR");
        v["error"] = json!({"code": code, "message": format!("fake {code}")});
        write(&v, exit)
    };
    if has("NO_RESULT") {
        std::process::exit(0)
    }
    if has("JOB_INVALID") {
        error("JOB_INVALID", 2)
    }
    for code in ["TIMEOUT", "OOM_KILLED", "ENGINE_FAILED", "LENGTH_BUDGET"] {
        if has(code) {
            let mut v = base.clone();
            v["status"] = json!("ERROR");
            v["error"] = json!({"code": code, "message": format!("fake {code}")});
            v["binary"] = json!({"path": job["binary"]["path"], "sha256": job["binary"]["sha256"]});
            write(&v, 3)
        }
    }
    let order = job["expansion"]["order"].as_i64().unwrap_or(0);
    let seeds: Vec<u64> = job["primes"]["seeds"]
        .as_array()
        .map(|a| a.iter().filter_map(Value::as_u64).collect())
        .unwrap_or_default();
    let budget = arg("REQ_LENGTH_BUDGET:");
    let disagree = arg("SEED_DISAGREE:");
    let level = job["level"].as_str().unwrap_or("package");
    let mut primes_all = vec![];
    let mut requests = vec![];
    let mut partial = false;
    for r in job["requests"].as_array().cloned().unwrap_or_default() {
        let label = r["label"].as_str().unwrap_or("").to_string();
        if budget.contains(&label) {
            partial = true;
            requests.push(
                json!({"label": label, "inputs": r["inputs"], "status": "ERROR",
                "error": {"code": "LENGTH_BUDGET", "message": "fake length budget"}}),
            );
            continue;
        }
        // job v2: weight vectors -> one entry per vector `<label>#<j>` (drawn vectors: a deterministic test rule)
        let n_in = r["inputs"].as_array().map_or(0, |a| a.len());
        let variants: Vec<(String, Option<Vec<i64>>)> = match &r["weight_vectors"] {
            Value::Null => vec![(label.clone(), None)],
            w => {
                let vs: Vec<Vec<i64>> = match w["vectors"].as_array() {
                    Some(a) => a
                        .iter()
                        .map(|v| {
                            v.as_array()
                                .into_iter()
                                .flatten()
                                .filter_map(Value::as_i64)
                                .collect()
                        })
                        .collect(),
                    None => (0..w["count"].as_u64().unwrap_or(1) as i64)
                        .map(|j| (0..n_in as i64).map(|i| (j + 1) * (i + 2)).collect())
                        .collect(),
                };
                vs.into_iter()
                    .enumerate()
                    .map(|(j, v)| (format!("{label}#{j}"), Some(v)))
                    .collect()
            }
        };
        for (vj, (lab, wv)) in variants.into_iter().enumerate() {
            let mut terms_a = vec![];
            let mut terms_b = vec![];
            let mut certs = vec![];
            for (si, &seed) in seeds.iter().enumerate() {
                let pair = (seed % 3) as usize;
                for &p in &PRIMES[2 * pair..2 * pair + 2] {
                    if !primes_all.contains(&p) {
                        primes_all.push(p);
                    }
                    let n = (order + 1).max(0) as usize;
                    let (mut a, mut b) = (vec![0u64; n], vec![0u64; n]);
                    for (ii, i) in r["inputs"]
                        .as_array()
                        .cloned()
                        .unwrap_or_default()
                        .into_iter()
                        .enumerate()
                    {
                        let w = match &wv {
                            Some(v) => v[ii],
                            None => i["weight"].as_i64().unwrap_or(1),
                        };
                        let (ka, kb) = key_series(&key_ints(&i), order, p);
                        for j in 0..n {
                            a[j] = modp(a[j] as i128 + w as i128 * ka[j] as i128, p);
                            b[j] = modp(b[j] as i128 + w as i128 * kb[j] as i128, p);
                        }
                    }
                    let shift = if disagree.contains(&label) && si + 1 == seeds.len() {
                        1
                    } else {
                        0
                    };
                    terms_a.push(per_prime(&a, p, seed, order, shift));
                    terms_b.push(per_prime(&b, p, seed, order, shift));
                }
                certs.push(if level == "node" {
                    json!({"seed": seed, "graph_digest": "fake-graph", "plan_digest": "fake-plan", "plan": {"max_need": order + 1}})
                } else {
                    json!({"seed": seed, "trace_digests": ["fake-trace"], "plan_digests": ["fake-plan"], "replays": 1})
                });
            }
            let term = |basis: &str, pp: Vec<Value>| {
                json!({"basis": basis, "tag": "", "kind": "master", "answer": "IGNORED", "valuation": null,
                    "per_prime": pp})
            };
            let mut entry = json!({"label": lab, "inputs": r["inputs"], "status": "OK",
                "terms": [term("M:fa", terms_a), term("M:fb", terms_b)], "pole": {}, "certificate": certs});
            if let Some(v) = &wv {
                entry["vector"] = json!({"of": label, "index": vj, "weights": v});
            }
            requests.push(entry);
        }
    }
    let mut v = base.clone();
    if job["schema"] == "helper-vh2.tadic-job.v2" {
        v["schema"] = json!("helper-vh2.tadic-result.v2");
    }
    v["status"] = json!(if partial { "PARTIAL" } else { "OK" });
    v["error"] = Value::Null;
    v["level"] = json!(level);
    v["binary"] = json!({"path": job["binary"]["path"], "sha256": job["binary"]["sha256"]});
    let driver = spec["inputs"]
        .as_array()
        .and_then(|a| a.iter().find(|i| i["role"] == "driver"))
        .and_then(|i| i["path"].as_str())
        .and_then(|p| std::fs::read(p).ok())
        .map(|b| sha256_hex(&b))
        .unwrap_or_default();
    v["driver"] = json!({"path": "fake", "sha256": driver});
    v["expansion"] =
        json!({"d0": job["expansion"]["d0"], "order": order, "variable": "t = d - d0"});
    v["primes"] = json!(primes_all.iter().map(|p| p.to_string()).collect::<Vec<_>>());
    v["unusable_primes"] = json!([]);
    v["requests"] = json!(requests);
    v["certificate"] = json!({"runs": [], "seeds": seeds});
    write(&v, if partial { 1 } else { 0 })
}
