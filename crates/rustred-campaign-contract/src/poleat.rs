//! Kind `pole-regular-at` (lane coreseed; interfaces/coreseed.md 10): lane poles' `rr_poles regular-at` as a LEGACY
//! DIAGNOSTIC child. One job = MANY (relation record, key) pairs, written by the supervisor into the pinned `requests`
//! input (`rustred.campaign.pole-request.v1` lines `{"id", "key", "d0"?, "record"}`); the child answers one
//! `rustred.campaign.pole-answer.v1` line per request, in request order: the pole order of THAT relation at THAT key at
//! d0 (`regular` = order 0, `pole` = order p >= 1, `undecided` = no verdict). The supervisor-side wrapper checks every
//! answer against its request (count, order, id, key, the relation's digest) and seals `out/results.jsonl` (one
//! canonical answer per record + the end record); accept mode `diagnostic_records` (rows 37-39): NEVER rule or relation
//! evidence, never counts a key as solved; a missing or refused diagnostic leaves the relations `undecided` and never
//! holds a node.
//!
//! Everything here is pure (bytes in, verdicts out). Generic: a key is an integer vector of any length.

use crate::spec::JobSpec;
use serde_json::{Value, json};

/// `schema` of a request line (written by the supervisor; rr_poles reads it).
pub const REQUEST_SCHEMA: &str = "rustred.campaign.pole-request.v1";
/// `schema` of an answer line (rr_poles writes it).
pub const ANSWER_SCHEMA: &str = "rustred.campaign.pole-answer.v1";
/// The child's raw output file in `out/`.
pub const CHILD_OUT: &str = "pole_answers.jsonl";
/// The word an OUTCOME error carries when the pinned `requests` input is not the one the spec names (row 39).
pub const INPUT_MISMATCH: &str = "POLE_INPUT_MISMATCH";
/// Answer verdicts (closed list).
pub const VERDICTS: [&str; 3] = ["regular", "pole", "undecided"];

/// The legacy argv: `regular-at --requests <in> --out out/<CHILD_OUT> [--d0 X] [--threads N]` (the spec's args, in
/// their allow-listed form, follow the fixed part).
pub fn argv(spec: &JobSpec) -> Result<Vec<String>, String> {
    let rq = spec
        .inputs
        .iter()
        .find(|i| i.role == "requests")
        .ok_or("no requests input")?;
    let mut v = vec![
        "regular-at".to_string(),
        "--requests".into(),
        spec.input_path(rq).display().to_string(),
        "--out".into(),
        format!("{}/out/{CHILD_OUT}", spec.run_dir),
    ];
    v.extend(spec.args.iter().cloned());
    Ok(v)
}

fn lines_of(bytes: &[u8]) -> Result<Vec<Value>, String> {
    let text = std::str::from_utf8(bytes).map_err(|e| format!("not UTF-8: {e}"))?;
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .enumerate()
        .map(|(i, l)| serde_json::from_str(l).map_err(|e| format!("line {}: {e}", i + 1)))
        .collect()
}

/// One checked answer: the request's id and key, the verdict and the pole order (None = undecided).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PoleAnswer {
    pub id: Value,
    pub key: Vec<i64>,
    pub relation_digest: Option<String>,
    pub verdict: String,
    pub pole_order: Option<u64>,
    /// The canonical answer line.
    pub canonical: Vec<u8>,
}

/// Check the child's answers against the requests: one answer per request, in order, with the request's `id` and
/// `key`, schema [`ANSWER_SCHEMA`], a verdict of [`VERDICTS`] with a consistent `pole_order` (regular: 0 or null;
/// pole: an integer >= 1; undecided: null), and the request record's `relation_digest` echoed in `relation` when the
/// record has one. `Err` = the answers are not this job's (nothing is read).
pub fn check_answers(requests: &[u8], answers: &[u8]) -> Result<Vec<PoleAnswer>, String> {
    let reqs = lines_of(requests).map_err(|e| format!("requests: {e}"))?;
    let ans = lines_of(answers).map_err(|e| format!("answers: {e}"))?;
    if reqs.len() != ans.len() {
        return Err(format!("{} answers for {} requests", ans.len(), reqs.len()));
    }
    let mut out = Vec::with_capacity(ans.len());
    for (i, (q, a)) in reqs.iter().zip(&ans).enumerate() {
        let at = |m: &str| format!("answer {i}: {m}");
        if a["schema"] != ANSWER_SCHEMA {
            return Err(at("schema"));
        }
        if a["id"] != q["id"] {
            return Err(at("id is not the request's"));
        }
        let key: Vec<i64> = serde_json::from_value(q["key"].clone())
            .map_err(|e| at(&format!("request key: {e}")))?;
        if a["key"] != json!(key) {
            return Err(at("key is not the request's"));
        }
        let verdict = a["verdict"].as_str().unwrap_or("");
        if !VERDICTS.contains(&verdict) {
            return Err(at(&format!("verdict {verdict:?} not in {VERDICTS:?}")));
        }
        let po = &a["pole_order"];
        let pole_order = match verdict {
            "regular" if po.is_null() || po.as_u64() == Some(0) => Some(0),
            "pole" if po.as_u64().is_some_and(|p| p >= 1) => po.as_u64(),
            "undecided" if po.is_null() => None,
            _ => return Err(at(&format!("pole_order {po} inconsistent with {verdict}"))),
        };
        let want = q["record"]["relation_digest"].as_str();
        if let Some(w) = want
            && a["relation"]["relation_digest"].as_str() != Some(w)
        {
            return Err(at("relation_digest is not the request record's"));
        }
        let canonical = crate::canon::canonical_line(a.to_string().as_bytes())
            .map_err(|e| at(&format!("{e:?}")))?;
        out.push(PoleAnswer {
            id: q["id"].clone(),
            key,
            relation_digest: want.map(str::to_string),
            verdict: verdict.to_string(),
            pole_order,
            canonical,
        });
    }
    Ok(out)
}

/// S of the kind: the verified canonical lines of the sealed results (answers, end record last) checked again against
/// the pinned requests (whose raw bytes must have the spec's digest).
pub fn check_results(
    lines: &[Vec<u8>],
    requests: &[u8],
    spec: &JobSpec,
) -> Result<Vec<PoleAnswer>, String> {
    let rq = spec
        .inputs
        .iter()
        .find(|i| i.role == "requests")
        .ok_or("no requests input")?;
    if crate::blake3_hex(requests) != rq.blake3 {
        return Err(format!("{INPUT_MISMATCH}: requests input digest differs"));
    }
    let (end, body) = lines.split_last().ok_or("results without records")?;
    let end: Value = serde_json::from_slice(end).map_err(|e| format!("end record: {e}"))?;
    if end["type"] != "end" || end["records"].as_u64() != Some(body.len() as u64) {
        return Err("end record does not count the answers".into());
    }
    let mut b = Vec::new();
    for l in body {
        b.extend_from_slice(l);
        b.push(b'\n');
    }
    check_answers(requests, &b)
}
