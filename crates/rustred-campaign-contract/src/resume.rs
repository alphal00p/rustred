//! `RESUME.json` (child-job.md 3.8.1), the spec's `resume` reference (3.8.2) and lines of the cases input (3.8.3).
//!
//! Ruling R-b: nothing is salvaged from an interrupted attempt's output as accepted records; a resume re-runs from
//! the committed prefix and the queue state recorded here, and the new attempt's sealed table is what is accepted.

use crate::spec::Counters;
use crate::{RESUME_SCHEMA, is_blake3_hex};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Frame of a case's slots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Frame {
    #[serde(rename = "PRESENTED")]
    Presented,
    #[serde(rename = "ORIGINAL")]
    Original,
}

/// One case. `fixed[i] = None` is a free slot. `equations` and `seed_centre_offset` are engine objects carried
/// verbatim. Unknown fields are kept in `extra` so that a round trip loses nothing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaseSpec {
    pub frame: Frame,
    pub fixed: Vec<Option<i64>>,
    #[serde(default)]
    pub equations: Vec<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed_centre_offset: Option<Value>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
}

/// `RESUME.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResumeState {
    pub schema: String,
    pub job_id: String,
    pub attempt: u32,
    pub chart: Value,
    pub frame: Frame,
    pub rules_committed: u64,
    pub table_committed_bytes: u64,
    pub table_prefix_blake3: String,
    pub current: Option<CaseSpec>,
    #[serde(default)]
    pub pending: Vec<CaseSpec>,
    #[serde(default)]
    pub numerical: Vec<Vec<i64>>,
    #[serde(default)]
    pub deferred: Vec<Value>,
    #[serde(default)]
    pub refused_open: Vec<Value>,
    pub counters_at_commit: Counters,
    #[serde(default)]
    pub symbolic_cases: u64,
    #[serde(default)]
    pub discarded: u64,
}

impl ResumeState {
    /// Schema, digest form and frame (the frame of RESUME is always PRESENTED).
    pub fn check(&self) -> Result<(), String> {
        if self.schema != RESUME_SCHEMA {
            return Err(format!("schema {:?} is not {RESUME_SCHEMA:?}", self.schema));
        }
        if !is_blake3_hex(&self.table_prefix_blake3) {
            return Err("table_prefix_blake3 is not a BLAKE3 hex digest".into());
        }
        if self.frame != Frame::Presented {
            return Err("RESUME frame must be PRESENTED".into());
        }
        Ok(())
    }
}

/// `spec.resume`: where the previous attempt's state is, with the digests the child verifies before it starts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResumeRef {
    /// `<prev RUN>/RESUME.json`.
    pub from: String,
    /// BLAKE3 of that RESUME.json file.
    pub blake3: String,
    /// `<prev RUN>/out/F.table.jsonl`.
    pub table: String,
    pub committed_bytes: u64,
    pub prefix_blake3: String,
}

impl ResumeRef {
    pub fn validate(&self) -> Result<(), String> {
        if self.from.is_empty() || self.table.is_empty() {
            return Err("from / table must be paths".into());
        }
        if !is_blake3_hex(&self.blake3) || !is_blake3_hex(&self.prefix_blake3) {
            return Err("blake3 / prefix_blake3 must be BLAKE3 hex digests".into());
        }
        Ok(())
    }
}

/// A spill line of the cases input (table_dump-compatible).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpillLine {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub pending: Vec<CaseSpec>,
    #[serde(default)]
    pub current: Option<CaseSpec>,
    #[serde(default)]
    pub numerical: Vec<Vec<i64>>,
}

/// One line of `inputs[role=cases]`.
#[derive(Debug, Clone, PartialEq)]
pub enum CasesLine {
    Case(CaseSpec),
    Spill(SpillLine),
}

/// Parse one line of the cases input: a line with `"type":"spill"` is a spill line, anything else a case.
pub fn parse_cases_line(line: &str) -> Result<CasesLine, String> {
    let v: Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
    if v.get("type").and_then(Value::as_str) == Some("spill") {
        serde_json::from_value(v)
            .map(CasesLine::Spill)
            .map_err(|e| e.to_string())
    } else {
        serde_json::from_value(v)
            .map(CasesLine::Case)
            .map_err(|e| e.to_string())
    }
}
