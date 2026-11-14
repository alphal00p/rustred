//! `rr_fake_child` emulating the LEGACY rr_proof kinds (lane closeloop2; interfaces/closeloop.md 13.1):
//! `proof-instance-span` writes ONE `rustred.proof-kernel.span.v1` record to `out/span.jsonl` (one `keys[]` entry per
//! requested key, in request order), `proof-instantiate` one `rustred.proof-kernel.instantiate.v1` line per requested
//! (record, key) to `out/instantiate.jsonl`; then exit 0 WITHOUT OUTCOME (a legacy child: the supervisor-side wrapper
//! checks, seals and writes OUTCOME). `proof_answer` ops answer the requests in order (status word, optional lies);
//! `fail` = rr_proof's refusal (an ERROR line on stdout, exit 2); a signal ends the child like rr_proof's default
//! disposition (killed by that signal). Independence: record digests are computed here with the `sha2`-free code of
//! the campaign core (the same function the import uses), keys parsed by this program's own code.

use super::SIGNALS;
use rustred_campaign_contract::fake::FakeOp;
use rustred_campaign_contract::layout::RunLayout;
use rustred_campaign_contract::spec::JobSpec;
use serde_json::{Value, json};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::atomic::Ordering::SeqCst;
use std::time::Duration;

fn die(msg: &str) -> ! {
    println!("{}", json!({"verdict": "ERROR", "error": msg}));
    std::process::exit(2)
}

fn sha_at(path: &Path, offset: u64) -> Option<(String, Option<i64>)> {
    let mut f = std::fs::File::open(path).ok()?;
    f.seek(SeekFrom::Start(offset)).ok()?;
    let mut all = Vec::new();
    f.read_to_end(&mut all).ok()?;
    let end = all.iter().position(|&c| c == b'\n').unwrap_or(all.len());
    let line = &all[..end];
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    h.update(line);
    let idx = serde_json::from_slice::<Value>(line)
        .ok()
        .and_then(|v| v["index"].as_i64());
    Some((h.finish_hex(), idx))
}

fn named(t: &str) -> Option<Vec<i64>> {
    let mut v: Vec<(usize, i64)> = vec![];
    for p in t.trim().split(',') {
        let (n, x) = p.split_once('=')?;
        v.push((
            n.trim().strip_prefix('n')?.parse().ok()?,
            x.trim().parse().ok()?,
        ));
    }
    v.sort();
    v.iter()
        .enumerate()
        .all(|(i, (j, _))| i == *j)
        .then(|| v.into_iter().map(|(_, x)| x).collect())
}

fn key_object(k: &[i64]) -> Value {
    let mut m = serde_json::Map::new();
    for (i, x) in k.iter().enumerate() {
        m.insert(format!("n{i}"), json!(x));
    }
    Value::Object(m)
}

/// A signal received: end like rr_proof (default disposition: killed by the signal).
fn check_signal() {
    if SIGNALS.load(SeqCst) > 0 {
        let s = super::LAST_SIG.load(SeqCst);
        let s = if s > 0 { s } else { libc::SIGTERM };
        // SAFETY: restoring the default disposition and signalling ourselves.
        unsafe {
            libc::signal(s, libc::SIG_DFL);
            libc::raise(s);
        }
        std::process::exit(128 + s);
    }
}

fn nap(ms: u64) {
    let mut left = ms;
    while left > 0 {
        check_signal();
        let d = left.min(20);
        std::thread::sleep(Duration::from_millis(d));
        left -= d;
    }
    check_signal();
}

pub fn run(spec_path: &Path, kind: &str) -> ! {
    let Ok(spec) = std::fs::read(spec_path)
        .map_err(|e| e.to_string())
        .and_then(|b| serde_json::from_slice::<JobSpec>(&b).map_err(|e| e.to_string()))
    else {
        die("job.json unreadable");
    };
    let layout = RunLayout::new(&spec.run_dir);
    let input = |role: &str| -> Vec<u8> {
        spec.inputs
            .iter()
            .find(|i| i.role == role)
            .and_then(|i| std::fs::read(spec.input_path(i)).ok())
            .unwrap_or_else(|| die(&format!("input {role} missing")))
    };
    let ops: Vec<FakeOp> = spec
        .fake
        .as_ref()
        .map(|f| f.script.clone())
        .unwrap_or_default();
    let mut answers = ops
        .iter()
        .filter(|o| matches!(o, FakeOp::ProofAnswer { .. }));
    for op in &ops {
        match op {
            FakeOp::Prepare { ms } => nap(*ms),
            FakeOp::Fail { message, .. } => die(message),
            _ => {}
        }
    }
    let mut next = || answers.next().cloned();
    let out_lines: Vec<String> = if kind == "proof-instance-span" {
        let req: Value = serde_json::from_slice(&input("request")).unwrap_or(Value::Null);
        let (Some(rule), Some(sha)) = (req["rule"].as_i64(), req["record_sha256"].as_str()) else {
            die("request: rule / record_sha256");
        };
        let mut entries = vec![];
        let (mut ok, mut n) = (0, 0);
        for l in String::from_utf8_lossy(&input("keys")).lines() {
            let t = l.trim();
            if t.is_empty() || t.starts_with('#') {
                continue;
            }
            let Some(k) = named(t) else {
                die(&format!("key {t:?}"));
            };
            n += 1;
            let (mut status, mut esha, mut erule, mut ekey) = (
                "EXACT_IN_SPAN_AT_KEY".to_string(),
                sha.to_string(),
                rule,
                k.clone(),
            );
            if let Some(FakeOp::ProofAnswer {
                ms,
                status: s,
                record_sha256,
                rule: r,
                key,
                ..
            }) = next()
            {
                nap(ms);
                if let Some(s) = s {
                    status = s;
                }
                if let Some(x) = record_sha256 {
                    esha = x;
                }
                if let Some(x) = r {
                    erule = x;
                }
                if let Some(x) = key {
                    ekey = x;
                }
            }
            if status.starts_with("EXACT_IN_SPAN_AT_KEY") {
                ok += 1;
            }
            entries.push(
                json!({"rule": erule, "record_sha256": esha, "key": key_object(&ekey),
                                "status": status, "seconds": 0.001}),
            );
        }
        let verdict = if ok == n {
            "EXACT_ALL_KEYS_IN_SPAN"
        } else if ok > 0 {
            "EXACT_KEYS_IN_SPAN_PARTIAL"
        } else {
            "NOT_ALL_KEYS_IN_SPAN"
        };
        vec![
            json!({"schema": "rustred.proof-kernel.span.v1", "mode": "instance-span", "rule": rule,
                   "record_sha256": sha, "keys": entries, "verdict": verdict, "fake": true})
            .to_string(),
        ]
    } else {
        let mut lines = vec![];
        for l in String::from_utf8_lossy(&input("requests")).lines() {
            if l.trim().is_empty() {
                continue;
            }
            let v: Value = serde_json::from_str(l).unwrap_or(Value::Null);
            let (Some(table), Some(off), Some(rule)) = (
                v["table"].as_str(),
                v["offset"].as_u64(),
                v["rule"].as_i64(),
            ) else {
                die("request: table / offset / rule");
            };
            let Some((sha, idx)) = sha_at(Path::new(table), off) else {
                die(&format!("record at {off} of {table}"));
            };
            if idx != Some(rule) {
                die("rule record not found in table");
            }
            let keys: Vec<Vec<i64>> =
                serde_json::from_value(v["keys"].clone()).unwrap_or_else(|_| die("keys"));
            for k in keys {
                let mut a = json!({"schema": "rustred.proof-kernel.instantiate.v1", "record_sha256": sha,
                    "rule": rule, "key": k, "in_case": true, "chart_ok": true, "branch": null,
                    "regular": "regular", "irregular_terms": [], "hidden_loci": [], "rhs": [],
                    "zero_test": "exact", "error": null});
                if let Some(FakeOp::ProofAnswer {
                    ms,
                    record_sha256,
                    rule: r,
                    key,
                    line,
                    ..
                }) = next()
                {
                    nap(ms);
                    if let Some(Value::Object(o)) = line {
                        for (f, x) in o {
                            a[f] = x;
                        }
                    }
                    if let Some(x) = record_sha256 {
                        a["record_sha256"] = json!(x);
                    }
                    if let Some(x) = r {
                        a["rule"] = json!(x);
                    }
                    if let Some(x) = key {
                        a["key"] = json!(x);
                    }
                }
                lines.push(a.to_string());
            }
        }
        lines
    };
    check_signal();
    let name = if kind == "proof-instance-span" {
        "span.jsonl"
    } else {
        "instantiate.jsonl"
    };
    let mut body = out_lines.join("\n");
    body.push('\n');
    let p = layout.out_dir().join(name);
    if let Err(e) = std::fs::write(&p, body) {
        die(&format!("{}: {e}", p.display()));
    }
    println!(
        "{}",
        json!({"fake": true, "written": name, "lines": out_lines.len()})
    );
    std::process::exit(0)
}
