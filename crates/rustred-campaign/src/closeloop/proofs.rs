//! Proof jobs of the closing loop (lane closeloop2; interfaces/closeloop.md 13.1). Two batch kinds, each one
//! supervisor item and one legacy rr_proof job per attempt:
//! - `span` (`proof-instance-span`): ONE rule record at its keys -> per-key proofs (EXACT_IN_SPAN_AT_KEY = level
//!   per-key), for the import's proof requests and for the POINT RECORDS of our own recipe relations;
//! - `facts` (`proof-instantiate`): records of ONE table at their keys -> exact coefficient / regularity facts (the
//!   exact tier).
//!
//! Every answer is bound to (record_sha256, key) twice: by the supervisor-side wrapper against the job's request
//! (adapters::legacy) and here again against the batch file the ledger names (never trusted from the child).
//! Generic (L8): records, tables, keys and arities are data.

use crate::adapters::legacy::{SPAN_REQUEST_SCHEMA, SpanRequest, key_text_named};
use rustred_campaign_contract::fake::{FakeOp, FakeScript, OnSigterm, RssPlan};
use rustred_campaign_core::import::evidence::FACT_VERDICT_SCHEMA;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub const PBATCH_SCHEMA: &str = "rustred.campaign.closeloop-proof-batch.v1";
/// Header type of a relation point-record table.
pub const RELATIONS_SCHEMA: &str = "rustred.campaign.closeloop-relations.v1";

/// One rule record of a batch with its keys.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RecordReq {
    pub rule: i64,
    pub offset: u64,
    pub record_sha256: String,
    pub keys: Vec<Vec<i32>>,
}

/// Why a batch was asked.
pub const REASON_IMPORT_PROOF: &str = "import_proof_request";
pub const REASON_RELATION: &str = "relation";
pub const REASON_FACTS: &str = "import_fact_request";
/// lane closeloop3: fact requests of records rr_proof instantiate refuses (elided witness loci): exact evaluation.
pub const REASON_EXACT: &str = "import_fact_request_exact_eval";

/// One new proof / fact batch (output of a pass, before its item exists).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProofBatch {
    /// `span` | `facts`.
    pub kind: String,
    pub reason: String,
    pub table: String,
    pub records: Vec<RecordReq>,
    /// Relation proofs: the recipe source whose relation the record states.
    pub relation_source: Option<String>,
}

impl ProofBatch {
    pub fn keys(&self) -> u64 {
        self.records.iter().map(|r| r.keys.len() as u64).sum()
    }
}

/// The batch file of one proof item (registered input `closeloop:<item>:pbatch`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProofBatchFile {
    pub schema: String,
    pub node: String,
    pub item: String,
    pub round: u32,
    pub kind: String,
    pub reason: String,
    pub table: String,
    pub table_blake3: String,
    pub records: Vec<RecordReq>,
    #[serde(default)]
    pub relation_source: Option<String>,
    /// `engine` | `cert` (span only).
    pub rows: String,
    pub drop_zero_sectors: bool,
}

impl ProofBatchFile {
    /// Every (record_sha256, key) the batch asks.
    pub fn pairs(&self) -> BTreeSet<(String, Vec<i32>)> {
        self.records
            .iter()
            .flat_map(|r| r.keys.iter().map(|k| (r.record_sha256.clone(), k.clone())))
            .collect()
    }
}

fn k64(k: &[i32]) -> Vec<i64> {
    k.iter().map(|&x| x as i64).collect()
}

/// The `request` input of a span job.
pub fn span_request_bytes(b: &ProofBatchFile) -> Vec<u8> {
    let r = &b.records[0];
    let req = SpanRequest {
        schema: SPAN_REQUEST_SCHEMA.into(),
        rule: r.rule,
        offset: r.offset,
        record_sha256: r.record_sha256.clone(),
        rows: b.rows.clone(),
        drop_zero_sectors: b.drop_zero_sectors,
        symmetry: None,
        max_primes: None,
    };
    let mut v = serde_json::to_vec(&req).expect("json");
    v.push(b'\n');
    v
}

/// The `keys` input of a span job (rr_proof's `n0=..` form, every slot named).
pub fn span_keys_bytes(b: &ProofBatchFile) -> Vec<u8> {
    let mut s = String::new();
    for k in &b.records[0].keys {
        s.push_str(&key_text_named(&k64(k)));
        s.push('\n');
    }
    s.into_bytes()
}

/// The `requests` input of a facts job (rr_proof instantiate form (A)).
pub fn fact_requests_bytes(b: &ProofBatchFile) -> Vec<u8> {
    let mut s = String::new();
    for r in &b.records {
        s.push_str(
            &json!({"table": b.table, "offset": r.offset, "rule": r.rule, "keys": r.keys})
                .to_string(),
        );
        s.push('\n');
    }
    s.into_bytes()
}

/// Fake answers (tests): JSONL `{"key": [..], "status"?: .., "record_sha256"?: .., "rule"?: .., "key_answered"?: ..,
/// "line"?: {..}}` per key (missing = EXACT_IN_SPAN_AT_KEY / a regular instantiate answer).
pub type ProofOracle = BTreeMap<Vec<i32>, Value>;

pub fn read_oracle(p: &Path) -> Result<ProofOracle, String> {
    let t = std::fs::read_to_string(p).map_err(|e| format!("proof oracle {}: {e}", p.display()))?;
    let mut m = BTreeMap::new();
    for l in t.lines().filter(|l| !l.trim().is_empty()) {
        let v: Value = serde_json::from_str(l).map_err(|e| format!("proof oracle: {e}"))?;
        let k: Vec<i32> = serde_json::from_value(v["key"].clone())
            .map_err(|e| format!("proof oracle key: {e}"))?;
        m.insert(k, v);
    }
    Ok(m)
}

/// The fake child's script of a batch (one `proof_answer` per requested (record, key), in request order).
pub fn fake_script(b: &ProofBatchFile, oracle: &ProofOracle, ms: u64) -> FakeScript {
    let mut ops = vec![FakeOp::Prepare { ms: 10 }];
    for r in &b.records {
        for k in &r.keys {
            let o = oracle.get(k).cloned().unwrap_or(Value::Null);
            ops.push(FakeOp::ProofAnswer {
                ms,
                status: o["status"].as_str().map(str::to_string),
                record_sha256: o["record_sha256"].as_str().map(str::to_string),
                rule: o["rule"].as_i64(),
                key: serde_json::from_value(o["key_answered"].clone()).ok(),
                line: o.get("line").cloned(),
            });
        }
    }
    ops.push(FakeOp::End {
        stop: None,
        verdict: None,
    });
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

fn key_of(v: &Value) -> Option<Vec<i32>> {
    serde_json::from_value::<Vec<i32>>(v.clone()).ok()
}

fn key_text(k: &[i64]) -> String {
    format!(
        "I({})",
        k.iter()
            .map(|x| x.to_string())
            .collect::<Vec<_>>()
            .join(",")
    )
}

/// The evidence lines of a settled batch, from the contract-VERIFIED canonical lines of its sealed stream, each bound
/// AGAIN to the batch (record_sha256 + key + rule echo; every asked pair at most once): span -> compact proof verdict
/// lines (import.md 13.2: record_sha256, key, verdict word, index echo, + status text); facts -> compact
/// `rustred.campaign.fact-verdict.v1` lines (import.md 13.2: the term-key digests of the NON-ZERO terms only).
pub fn evidence_lines(
    b: &ProofBatchFile,
    canonical_lines: &[String],
) -> Result<Vec<String>, String> {
    let mut rule_of: BTreeMap<String, i64> = BTreeMap::new();
    for r in &b.records {
        rule_of.insert(r.record_sha256.clone(), r.rule);
    }
    let asked = b.pairs();
    let mut seen = BTreeSet::new();
    let mut out = vec![];
    for (i, l) in canonical_lines.iter().enumerate() {
        let v: Value = serde_json::from_str(l).map_err(|e| format!("answer {}: {e}", i + 1))?;
        if v.get("type").and_then(Value::as_str) == Some("end") {
            continue;
        }
        let sha = v["record_sha256"]
            .as_str()
            .ok_or_else(|| format!("answer {}: no record_sha256", i + 1))?
            .to_string();
        let key = key_of(&v["key"]).ok_or_else(|| format!("answer {}: key", i + 1))?;
        let id = (sha.clone(), key.clone());
        if !asked.contains(&id) {
            return Err(format!(
                "answer {}: (record {sha}, key {key:?}) is not asked by the batch",
                i + 1
            ));
        }
        if v["rule"].as_i64() != rule_of.get(&sha).copied() {
            return Err(format!("answer {}: rule echo {}", i + 1, v["rule"]));
        }
        if !seen.insert(id) {
            return Err(format!("answer {}: (record, key) twice", i + 1));
        }
        let line = if b.kind == "span" {
            json!({"record_sha256": sha, "key": key, "verdict": v["verdict"], "index": v["rule"],
                   "status": v["status"], "source": "closeloop"})
        } else if b.kind == "exact" {
            // lane closeloop3: already a compact fact verdict line of the exact evaluation; its provenance is kept
            // (producer / backend / method / record_sha256), per-term detail is not recorded
            if v["schema"] != FACT_VERDICT_SCHEMA || v["producer"] != "exact_eval" {
                return Err(format!(
                    "answer {}: not an exact_eval fact verdict line ({} / {})",
                    i + 1,
                    v["schema"],
                    v["producer"]
                ));
            }
            let mut o = v.clone();
            if let Some(m) = o.as_object_mut() {
                m.remove("coef");
                m.remove("term_keys");
                m.insert("source".into(), json!("closeloop"));
            }
            o
        } else {
            let usable = v["in_case"] == true
                && v["chart_ok"] == true
                && v.get("branch").is_none_or(Value::is_null)
                && v.get("error").is_none_or(Value::is_null);
            let verdict = if usable {
                v["regular"].as_str().unwrap_or("unknown").to_string()
            } else {
                format!(
                    "unusable:{}",
                    v["error"]
                        .as_str()
                        .map(str::to_string)
                        .unwrap_or_else(|| "not_in_case_or_branch".into())
                )
            };
            let (mut nonzero, mut unknown) = (vec![], vec![]);
            let terms = v["rhs"].as_array().cloned().unwrap_or_default();
            for t in &terms {
                let tk: Vec<i64> = serde_json::from_value(t["key"].clone())
                    .map_err(|e| format!("answer {}: rhs key: {e}", i + 1))?;
                let tk32: Vec<i32> = tk.iter().map(|&x| x as i32).collect();
                let d = format!(
                    "{:016x}",
                    rustred_campaign_core::import::lines::term_key_digest(&tk32)
                );
                match t["coef"].as_str() {
                    Some("zero") => {}
                    Some("nonzero") => nonzero.push(d),
                    _ => unknown.push(d),
                }
            }
            json!({"schema": FACT_VERDICT_SCHEMA, "index": v["rule"], "record_sha256": sha, "key": key,
                   "verdict": verdict, "n_terms": terms.len(), "nonzero": nonzero, "unknown": unknown,
                   "producer": "rr_proof"})
        };
        out.push(
            rustred_campaign_core::schema::canon::canonical_json(&line, false)
                .unwrap_or_else(|_| line.to_string()),
        );
    }
    Ok(out)
}

/// The sector mask of a recipe (`chart` field of the recipe file), else the key's own (slot > 0).
pub fn chart_of(recipe: &Value, key: &[i32]) -> String {
    recipe["chart"]
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| key.iter().map(|&x| if x > 0 { '1' } else { '0' }).collect())
}

/// One relation of our own recipe solve as a POINT-RULE record (table_dump.v1 rule line, closeloop.md 13.1): fixed =
/// the key, target, rhs [[key, coefficient text]] from the verified results record, seeds = the key + `seeds_used`,
/// recenter 0, no exception, no witness locus (a point rule), `sym_rows` > 0 iff symmetry rows are in force.
pub fn relation_record(
    index: usize,
    key: &[i32],
    rec: &Value,
    chart: &str,
    sym: bool,
) -> Result<String, String> {
    let rhs: Vec<Value> = rec["rhs"]
        .as_array()
        .ok_or("relation record without rhs")?
        .iter()
        .map(|t| json!([t[0].clone(), t[1].clone()]))
        .collect();
    let mut seeds = vec![];
    let mut seen = BTreeSet::new();
    for off in rec["seeds_used"].as_array().into_iter().flatten() {
        let o: Vec<i64> =
            serde_json::from_value(off.clone()).map_err(|e| format!("seeds_used: {e}"))?;
        if o.len() != key.len() {
            return Err(format!("seeds_used entry of length {}", o.len()));
        }
        let s: Vec<i64> = key.iter().zip(&o).map(|(&a, &b)| a as i64 + b).collect();
        if seen.insert(s.clone()) {
            seeds.push(key_text(&s));
        }
    }
    if seeds.is_empty() {
        seeds.push(key_text(&k64(key)));
    }
    // rr_proof finds a record at its offset only if the line STARTS with `{"type":"rule"` (span.rs): type first
    let v = json!({"index": index, "chart": chart, "fixed": key, "equations": [],
        "target": key_text(&k64(key)), "rhs": rhs, "exceptions": [], "seeds": seeds,
        "recenter": vec![0; key.len()], "witness_loci": [], "sym_rows": if sym { 1 } else { 0 },
        "relation_digest": rec["relation_digest"].clone(), "relation_rule": rec["relation_rule"].clone()});
    let body = v.to_string();
    Ok(format!("{{\"type\":\"rule\",{}", &body[1..]))
}

/// One record of a relation table: (rule index, byte offset, record_sha256, key).
pub type RelationRecord = (i64, u64, String, Vec<i32>);

/// The records of a relation table (header and group lines skipped), with their offsets and digests.
pub fn relation_records(text: &[u8]) -> Vec<RelationRecord> {
    let mut out = vec![];
    let mut off = 0u64;
    for l in text.split(|&c| c == b'\n') {
        let here = off;
        off += l.len() as u64 + 1;
        if l.is_empty() {
            continue;
        }
        let Ok(v) = serde_json::from_slice::<Value>(l) else {
            continue;
        };
        if v["type"] != "rule" {
            continue;
        }
        let (Some(idx), Some(key)) = (v["index"].as_i64(), key_of(&v["fixed"])) else {
            continue;
        };
        let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
        h.update(l);
        out.push((idx, here, h.finish_hex(), key));
    }
    out
}

/// The header line of a relation table.
pub fn relation_header(node: &str, item: &str, job: &str, source: &str) -> String {
    json!({"type": "header", "schema": RELATIONS_SCHEMA, "node": node, "item": item, "job": job,
           "recipe_source": source})
    .to_string()
}

/// A `symmetry_group` line from a group file (a `symmetry_group` record, or a bare group object wrapped with `chart`).
pub fn symmetry_group_line(text: &str, chart: &str) -> Result<String, String> {
    let v: Value = serde_json::from_str(text.trim()).map_err(|e| format!("symmetry group: {e}"))?;
    if v["type"] == "symmetry_group" {
        return Ok(v.to_string());
    }
    Ok(json!({"type": "symmetry_group", "chart": chart, "group": v}).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relation_record_round_trip() {
        let rec = json!({"rhs": [["I(1,0,1)", "d-4", "sub"]], "seeds_used": [[0,0,0],[1,0,0],[0,0,0]],
                         "relation_digest": "ab"});
        let l = relation_record(3, &[1, 1, 1], &rec, "111", false).unwrap();
        assert!(l.starts_with("{\"type\":\"rule\","), "{l}");
        let v: Value = serde_json::from_str(&l).unwrap();
        assert_eq!(v["type"], "rule");
        assert_eq!(v["seeds"], json!(["I(1,1,1)", "I(2,1,1)"]));
        assert_eq!(v["rhs"], json!([["I(1,0,1)", "d-4"]]));
        let text = format!("{}\n{l}\n", relation_header("n", "i", "j", "s"));
        let r = relation_records(text.as_bytes());
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].0, 3);
        assert_eq!(r[0].3, vec![1, 1, 1]);
        assert_eq!(&text.as_bytes()[r[0].1 as usize..][..l.len()], l.as_bytes());
    }

    #[test]
    fn evidence_lines_bind_to_the_batch() {
        let b = ProofBatchFile {
            schema: PBATCH_SCHEMA.into(),
            node: "n".into(),
            item: "n.p1.0".into(),
            round: 1,
            kind: "span".into(),
            reason: REASON_RELATION.into(),
            table: "/t".into(),
            table_blake3: "x".into(),
            records: vec![RecordReq {
                rule: 2,
                offset: 0,
                record_sha256: "aa".into(),
                keys: vec![vec![1, 1]],
            }],
            relation_source: None,
            rows: "engine".into(),
            drop_zero_sectors: true,
        };
        let good = json!({"record_sha256": "aa", "rule": 2, "key": [1,1], "verdict": "EXACT_IN_SPAN_AT_KEY"});
        assert_eq!(evidence_lines(&b, &[good.to_string()]).unwrap().len(), 1);
        let other_key = json!({"record_sha256": "aa", "rule": 2, "key": [1,2], "verdict": "EXACT_IN_SPAN_AT_KEY"});
        assert!(evidence_lines(&b, &[other_key.to_string()]).is_err());
        let other_rule = json!({"record_sha256": "bb", "rule": 2, "key": [1,1], "verdict": "EXACT_IN_SPAN_AT_KEY"});
        assert!(evidence_lines(&b, &[other_rule.to_string()]).is_err());
        let wrong_echo = json!({"record_sha256": "aa", "rule": 3, "key": [1,1], "verdict": "EXACT_IN_SPAN_AT_KEY"});
        assert!(evidence_lines(&b, &[wrong_echo.to_string()]).is_err());
    }
}
