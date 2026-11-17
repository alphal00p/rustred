//! The EFFECTIVE DERIVATION of every rule the tool generates or replaces (implementer 2026-11-16; TRICKS items 31, 35):
//! a rule's witness / pole loci belong to its derivation, and a T178 record (5242) could only be re-lifted with
//! `seed_schedule SymbolicFirst` because the original reached it on a symbolic-first RETRY that the table header does not
//! record. PURE: built from the producing job's own files (texts in, JSON lines out).
//!
//! Sources (frozen engine line 7d): the job spec (`job.json`: program blake3, inputs, args), the job protocol events
//! (`EVENTS.jsonl`: `case_started` {ordinal, case}, engine `RelationRetry` {case, source_order}, `time_decision`,
//! `rule_committed` {index, canon_blake3, line_bytes, monomials}, `case_finished`), the engine's stderr
//! (`EVENT <t>s SeedPose chosen=<pose>|<schedule> cost=..`, matched to the case whose time window contains it) and the
//! rule record (seeds, seed_setting, source_order, numerical, witness_loci / witness_poles / witness_d_poles,
//! guard_version). The engine writes neither the retry nor the chosen pose into the record: the derivation line does
//! (engine request: notes/engine-line.COORDINATOR_NOTE.md). `replay_args` = the producing job's own args: the engine is
//! deterministic given args + inputs and takes the same retries again (verified 2026-11-16 on T205 rule 2823 rung, reached on
//! an `input+symbolic_first` relation retry: identical canon_blake3). Forcing `--seed-schedule symfirst` instead gives a
//! DIFFERENT record: the relation retry is the lift's symbolic-first rung, not a seed-schedule change, so the retry is
//! recorded (`relation_retries`, `lift_rung`) and never translated into another flag.

use serde_json::{Value, json};
use std::collections::BTreeMap;

pub const SCHEMA: &str = "rustred.campaign.rule-derivation.v1";

/// What the derivation lines are built from.
#[derive(Clone, Debug, Default)]
pub struct JobFiles<'a> {
    pub node: &'a str,
    pub item: &'a str,
    pub job: &'a str,
    pub attempt: u32,
    /// `job.json` (the job spec).
    pub job_spec: &'a str,
    pub events: &'a str,
    pub stderr: &'a str,
    /// The sealed table's lines (header + rules).
    pub table_lines: &'a [String],
    /// The engine binary: path and sha256 (blake3 is in the job spec).
    pub engine_path: &'a str,
    pub engine_sha256: &'a str,
}

#[derive(Default, Clone)]
struct CaseRec {
    case: Value,
    t0: f64,
    t1: f64,
    retries: Vec<String>,
    time_decisions: Vec<Value>,
    pose: Option<Value>,
}

fn sha256_hex(b: &[u8]) -> String {
    let mut h = crate::closure::sha256::Sha256::new();
    h.update(b);
    h.finish_hex()
}

/// One derivation line per rule of the job's table (rules without a `rule_committed` event get `case: null`).
pub fn rule_derivations(f: &JobFiles<'_>) -> Result<Vec<Value>, String> {
    let spec: Value = serde_json::from_str(f.job_spec).map_err(|e| format!("job.json: {e}"))?;
    let args: Vec<String> = spec
        .get("args")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let mut cases: BTreeMap<u64, CaseRec> = BTreeMap::new();
    let mut by_rule: BTreeMap<i64, (Option<u64>, Value)> = BTreeMap::new();
    let mut current: Option<u64> = None;
    for l in f.events.lines() {
        let Ok(v) = serde_json::from_str::<Value>(l) else {
            continue;
        };
        let t = v.get("t").and_then(Value::as_f64).unwrap_or(0.0);
        match v.get("kind").and_then(Value::as_str) {
            Some("case_started") => {
                let o = v.get("ordinal").and_then(Value::as_u64).unwrap_or(0);
                cases.insert(
                    o,
                    CaseRec {
                        case: v.get("case").cloned().unwrap_or(Value::Null),
                        t0: t,
                        t1: f64::MAX,
                        ..CaseRec::default()
                    },
                );
                current = Some(o);
            }
            Some("case_finished") => {
                if let Some(c) = v
                    .get("ordinal")
                    .and_then(Value::as_u64)
                    .and_then(|o| cases.get_mut(&o))
                {
                    c.t1 = t;
                }
            }
            Some("time_decision") => {
                let o = v.get("case_ordinal").and_then(Value::as_u64).or(current);
                if let Some(c) = o.and_then(|o| cases.get_mut(&o)) {
                    c.time_decisions
                        .push(json!({"what": v.get("what"), "taken": v.get("taken"),
                        "limit_s": v.get("limit_s"), "measured_s": v.get("measured_s")}));
                }
            }
            Some("engine") => {
                if let Some(r) = v.get("event").and_then(|e| e.get("RelationRetry"))
                    && let Some(c) = current.and_then(|o| cases.get_mut(&o))
                {
                    c.retries.push(
                        r.get("source_order")
                            .and_then(Value::as_str)
                            .unwrap_or("?")
                            .to_string(),
                    );
                }
            }
            Some("rule_committed") => {
                if let Some(i) = v.get("index").and_then(Value::as_i64) {
                    by_rule.insert(i, (current, json!({"canon_blake3": v.get("canon_blake3"),
                        "line_bytes": v.get("line_bytes"), "monomials": v.get("monomials"), "t": t})));
                }
            }
            _ => {}
        }
    }
    // seed poses from stderr, matched by time window
    for l in f.stderr.lines() {
        let Some(rest) = l.strip_prefix("EVENT ") else {
            continue;
        };
        let mut it = rest.splitn(3, ' ');
        let (Some(ts), Some(kind)) = (it.next(), it.next()) else {
            continue;
        };
        if kind != "SeedPose" {
            continue;
        }
        let Ok(t) = ts.trim_end_matches('s').parse::<f64>() else {
            continue;
        };
        let mut fields = serde_json::Map::new();
        for kv in it.next().unwrap_or("").split_whitespace() {
            if let Some((k, v)) = kv.split_once('=') {
                fields.insert(k.into(), Value::String(v.into()));
            }
        }
        if let Some(c) = cases.values_mut().find(|c| c.t0 <= t && t <= c.t1) {
            c.pose = Some(Value::Object(fields));
        }
    }
    let header = f.table_lines.iter().find_map(|l| {
        serde_json::from_str::<Value>(l)
            .ok()
            .filter(|v| v["type"] == "header")
    });
    let opts = header
        .as_ref()
        .map(|h| h["node_options"].clone())
        .unwrap_or(Value::Null);
    let base_schedule = opts
        .get("seed_schedule")
        .and_then(Value::as_str)
        .unwrap_or("Spired")
        .to_lowercase();
    let mut out = Vec::new();
    for l in f.table_lines {
        let Ok(r) = serde_json::from_str::<Value>(l) else {
            continue;
        };
        if r["type"] != "rule" {
            continue;
        }
        let index = r["index"].as_i64().unwrap_or(-1);
        let (ord, committed) = by_rule.get(&index).cloned().unwrap_or((None, Value::Null));
        let c = ord.and_then(|o| cases.get(&o)).cloned().unwrap_or_default();
        let sym_retry = c.retries.iter().any(|s| s.contains("symbolic_first"));
        let produced_by = match c.retries.last() {
            None => "first attempt".to_string(),
            Some(r) => format!("retry {} (relation retry #{})", r, c.retries.len()),
        };
        let seeds = serde_json::to_vec(&r["seeds"]).unwrap_or_default();
        out.push(json!({
            "schema": SCHEMA, "node": f.node, "item": f.item, "job": f.job, "attempt": f.attempt,
            "engine": {"path": f.engine_path, "sha256": f.engine_sha256,
                       "blake3": spec.get("program").and_then(|p| p.get("blake3"))},
            "inputs": spec.get("inputs"), "args": args,
            "job_settings": {"budgets": spec.get("budgets"), "seed": spec.get("seed"), "output": spec.get("output"),
                             "on_case_budget": spec.get("on_case_budget"), "kind": spec.get("kind")},
            "rule": {"index": index, "target": r.get("target"), "record_sha256": sha256_hex(l.as_bytes()),
                     "committed": committed},
            "case": {"ordinal": ord, "case": if ord.is_some() { c.case.clone() } else { Value::Null }},
            "seed_schedule": {"header": base_schedule.clone(), "effective": base_schedule},
            "relation_retries": c.retries, "lift_rung": if sym_retry { "symbolic_first" } else { "default" },
            "attempt_that_produced": produced_by,
            "source_order": r.get("source_order"), "pose": c.pose, "time_decisions": c.time_decisions,
            "seeds_sha256": sha256_hex(&seeds), "nsources": r.get("nsources"), "seed_setting": r.get("seed_setting"),
            "recenter": r.get("recenter"), "depth": {"numerical_depth": header.as_ref().map(|h| h["numerical_depth"].clone()),
                "max_depth": opts.get("max_depth"), "numerical": r.get("numerical")},
            "loci": {"witness_loci": r.get("witness_loci"), "witness_poles": r.get("witness_poles"),
                     "witness_d_poles": r.get("witness_d_poles"), "guard_version": r.get("guard_version")},
            "replay_args": args,
        }));
    }
    Ok(out)
}
