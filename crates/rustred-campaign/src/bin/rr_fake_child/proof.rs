//! `rr_fake_child` emulating the LEGACY kinds (lane closeloop2; interfaces/closeloop.md 13.1; lane closeloop3: 16):
//! `proof-instance-span` writes ONE `rustred.proof-kernel.span.v1` record to `out/span.jsonl` (one `keys[]` entry per
//! requested key, in request order), `proof-instantiate` one `rustred.proof-kernel.instantiate.v1` line per requested
//! (record, key) to `out/instantiate.jsonl`; then exit 0 WITHOUT OUTCOME (a legacy child: the supervisor-side wrapper
//! checks, seals and writes OUTCOME). `proof_answer` ops answer the requests in order (status word, optional lies);
//! `fail` = rr_proof's refusal (an ERROR line on stdout, exit 2); a signal ends the child like rr_proof's default
//! disposition (killed by that signal). Independence: record digests are computed here with the `sha2`-free code of
//! the campaign core (the same function the import uses), keys parsed by this program's own code.
//! `coefficient-facts` (lane closeloop3) writes one `rustred.campaign.fact-verdict.v1` line per requested (record, key)
//! to `out/coefficient_facts.jsonl` (`producer: exact_eval`, `backend: fake-core-poly`), computed with the campaign
//! CORE's exact evaluation (`accept::facts::ExactRecordFacts`: `poly::Poly`, not Symbolica): an independent route to the
//! real child's facts; `proof_answer` ops may replace fields of a line (`line`) or lie about its binding.

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
        // the fake child's one self-signal helper (procfix3's tripwire: no raw raise)
        super::die_by_signal(s);
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
    let out_lines: Vec<String> = if kind == "coefficient-facts" {
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
            let keys: Vec<Vec<i32>> =
                serde_json::from_value(v["keys"].clone()).unwrap_or_else(|_| die("keys"));
            for k in keys {
                let mut a =
                    core_fact_line(Path::new(table), off, rule, &k).unwrap_or_else(|e| die(&e));
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
                        a["index"] = json!(x);
                    }
                    if let Some(x) = key {
                        a["key"] = json!(x);
                    }
                }
                lines.push(a.to_string());
            }
        }
        lines
    } else if kind == "proof-instance-span" {
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
            // lane closeloop4: `--elided-loci facts` (a second-route job): the answer carries the record's REAL
            // per-term coefficient facts (the campaign core's exact evaluation) and an elided record answers as rr_proof
            // does in facts mode (`elided_cover`, never `regular`); without it a record with elided loci is refused
            let facts_mode = spec.arg_value("--elided-loci") == Some("facts");
            for k in keys {
                let mut a = json!({"schema": "rustred.proof-kernel.instantiate.v1", "record_sha256": sha,
                    "rule": rule, "key": k, "in_case": true, "chart_ok": true, "branch": null,
                    "regular": "regular", "irregular_terms": [], "hidden_loci": [], "rhs": [],
                    "zero_test": "exact", "error": null});
                if facts_mode {
                    let k32: Vec<i32> = k.iter().map(|&x| x as i32).collect();
                    let (terms, elided, regular) =
                        core_terms(Path::new(table), off, rule, &k32).unwrap_or_else(|e| die(&e));
                    a["rhs"] = json!(
                        terms
                            .iter()
                            .enumerate()
                            .map(|(j, (tk, c))| json!({"term": j, "key": tk, "coef": c}))
                            .collect::<Vec<_>>()
                    );
                    a["regular"] = json!(regular);
                    if elided {
                        a["regular"] = json!(if regular == "irregular" {
                            "irregular"
                        } else {
                            "hidden"
                        });
                        a["elided_cover"] = json!("guards_v2");
                    }
                }
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
    let name = match kind {
        "proof-instance-span" => "span.jsonl",
        "coefficient-facts" => "coefficient_facts.jsonl",
        _ => "instantiate.jsonl",
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

/// lane closeloop4: the per-term (term key, `zero` | `nonzero` | `unknown`) facts of (record at `off` of `table`, key)
/// from the campaign core's exact evaluation, whether the record's witness loci are elided, and its regularity word.
#[allow(clippy::type_complexity)]
fn core_terms(
    table: &Path,
    off: u64,
    rule: i64,
    key: &[i32],
) -> Result<(Vec<(Vec<i32>, &'static str)>, bool, String), String> {
    use rustred_campaign_core::accept::facts::{
        CoefficientFacts, ExactRecordFacts, RegVerdict, Regularity,
    };
    use rustred_campaign_core::accept::rule::RuleText;
    use rustred_campaign_core::closure::CoefFact;
    use rustred_campaign_core::closure::bodies::{RhsSlot, parse_rhs_pattern, rule_point};
    let h = rustred_campaign_core::import::lines::read_header("fake", table)
        .map_err(|e| e.to_string())?;
    let frame =
        rustred_campaign_core::key::NodeFrame::from_header(&h).map_err(|e| format!("{e:?}"))?;
    let mut f = std::fs::File::open(table).map_err(|e| e.to_string())?;
    f.seek(SeekFrom::Start(off)).map_err(|e| e.to_string())?;
    let mut all = Vec::new();
    f.read_to_end(&mut all).map_err(|e| e.to_string())?;
    let end = all.iter().position(|&c| c == b'\n').unwrap_or(all.len());
    let line = &all[..end];
    let elided = serde_json::from_slice::<Value>(line)
        .ok()
        .and_then(|v| v["witness_loci"].as_array().cloned())
        .is_some_and(|w| w.iter().any(|x| x == "elided"));
    let rt = RuleText::parse(line, &frame).map_err(|e| format!("{e:?}"))?;
    if rt.index() != Some(rule) {
        return Err("rule record not found in table".into());
    }
    let dom = rt.domain.as_ref().ok_or("record without a domain")?;
    let mut pt = vec![];
    if !rule_point(dom, key, &mut pt) {
        return Err(format!("no rule point at {key:?}"));
    }
    let mut ex = ExactRecordFacts::new();
    let mut out = vec![];
    for (j, (pattern, _)) in rt.rhs.iter().enumerate() {
        let slots = parse_rhs_pattern(pattern, pt.len())?;
        let tk: Vec<i32> = slots
            .iter()
            .map(|s| match *s {
                RhsSlot::Param { param, offset } => pt[param] + offset,
                RhsSlot::Const(c) => c,
            })
            .collect();
        let c = match ex.coefficient(&rt, key, j) {
            CoefFact::Zero => "zero",
            CoefFact::NonZero => "nonzero",
            CoefFact::Unknown => "unknown",
        };
        out.push((tk, c));
    }
    let regular = match ex.regular(&rt, key) {
        RegVerdict::Pole { .. } => "irregular",
        RegVerdict::Regular => "regular",
        _ => "hidden",
    };
    Ok((out, elided, regular.to_string()))
}

/// The fact line of (record at `off` of `table`, key) from the campaign core's exact evaluation (lane closeloop3).
fn core_fact_line(table: &Path, off: u64, rule: i64, key: &[i32]) -> Result<Value, String> {
    use rustred_campaign_core::accept::facts::{
        CoefficientFacts, ExactRecordFacts, RegVerdict, Regularity,
    };
    use rustred_campaign_core::accept::rule::{CreditWitness, RuleText};
    use rustred_campaign_core::closure::CoefFact;
    use rustred_campaign_core::closure::bodies::{RhsSlot, parse_rhs_pattern, rule_point};
    let h = rustred_campaign_core::import::lines::read_header("fake", table)
        .map_err(|e| e.to_string())?;
    let guard = h.guard_version;
    let frame =
        rustred_campaign_core::key::NodeFrame::from_header(&h).map_err(|e| format!("{e:?}"))?;
    let mut f = std::fs::File::open(table).map_err(|e| e.to_string())?;
    f.seek(SeekFrom::Start(off)).map_err(|e| e.to_string())?;
    let mut all = Vec::new();
    f.read_to_end(&mut all).map_err(|e| e.to_string())?;
    let end = all.iter().position(|&c| c == b'\n').unwrap_or(all.len());
    let line = &all[..end];
    let mut hs = rustred_campaign_core::closure::sha256::Sha256::new();
    hs.update(line);
    let sha = hs.finish_hex();
    let rt = RuleText::parse(line, &frame).map_err(|e| format!("{e:?}"))?;
    if rt.index() != Some(rule) {
        return Err("rule record not found in table".into());
    }
    let dom = rt.domain.as_ref().ok_or("record without a domain")?;
    let mut pt = vec![];
    if !rule_point(dom, key, &mut pt) {
        return Err(format!("no rule point at {key:?}"));
    }
    let mut ex = ExactRecordFacts::new();
    let (mut nonzero, mut unknown, mut zero) = (vec![], vec![], 0usize);
    for (j, (pattern, _)) in rt.rhs.iter().enumerate() {
        let slots = parse_rhs_pattern(pattern, pt.len())?;
        let tk: Vec<i32> = slots
            .iter()
            .map(|s| match *s {
                RhsSlot::Param { param, offset } => pt[param] + offset,
                RhsSlot::Const(c) => c,
            })
            .collect();
        let d = format!(
            "{:016x}",
            rustred_campaign_core::import::lines::term_key_digest(&tk)
        );
        match ex.coefficient(&rt, key, j) {
            CoefFact::Zero => zero += 1,
            CoefFact::NonZero => nonzero.push(d),
            CoefFact::Unknown => unknown.push(d),
        }
    }
    nonzero.sort();
    nonzero.dedup();
    unknown.sort();
    unknown.dedup();
    let verdict = match ex.regular(&rt, key) {
        RegVerdict::Pole { .. } => "irregular".to_string(),
        _ if rt.credit_witness(guard) != CreditWitness::Complete => "hidden".into(),
        RegVerdict::Regular => "regular".into(),
        RegVerdict::Hidden { .. } => "hidden".into(),
        RegVerdict::Unknown(m) => format!("unusable:{m}"),
    };
    Ok(
        json!({"schema": rustred_campaign_core::import::evidence::FACT_VERDICT_SCHEMA, "producer": "exact_eval",
        "backend": "fake-core-poly", "record_sha256": sha, "rule": rule, "index": rule, "key": key,
        "in_case": true, "branch": null, "verdict": verdict, "n_terms": rt.rhs.len(), "nonzero": nonzero,
        "unknown": unknown, "zero_terms": zero}),
    )
}
