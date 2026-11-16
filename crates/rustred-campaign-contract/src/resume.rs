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

/// The committed state of one output stream at a commit point (RESUME, audit A C1): the byte length of its committed
/// prefix, the BLAKE3 of those bytes, the number of records (lines) and rule records in it, and the running
/// canonical digest of those records. The prefix never contains an end record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommittedFile {
    /// Layout path relative to the run directory (`out/F.table.jsonl`, `out/F.queue.jsonl`).
    pub path: String,
    pub bytes: u64,
    pub blake3: String,
    pub records: u64,
    #[serde(default)]
    pub rules: u64,
    pub canonical_blake3: String,
}

impl CommittedFile {
    /// The state of an empty stream at `path`.
    pub fn empty(path: &str) -> CommittedFile {
        CommittedFile {
            path: path.to_string(),
            bytes: 0,
            blake3: crate::blake3_hex(b""),
            records: 0,
            rules: 0,
            // the canonical digest of NO record (its own domain, lane contractfix2, ac2-m7)
            canonical_blake3: crate::canon::CanonicalDigest::new().hex(),
        }
    }

    fn check(&self, path: &str) -> Result<(), String> {
        if self.path != path {
            return Err(format!(
                "committed file path {:?} is not {path:?}",
                self.path
            ));
        }
        if !is_blake3_hex(&self.blake3) || !is_blake3_hex(&self.canonical_blake3) {
            return Err(format!("{path}: digests must be BLAKE3 hex"));
        }
        if (self.bytes == 0) != (self.records == 0) || self.rules > self.records {
            return Err(format!("{path}: bytes / records / rules inconsistent"));
        }
        Ok(())
    }
}

/// `RESUME.json` (`rustred.child-job.resume.v1`, revised by audit A C1 / M7): ONE consistent resume point, written
/// atomically at a commit point: the committed prefix of every output stream (`table`, `queue`) with its byte length,
/// digest, record count and running canonical digest, plus the queue state. Bound to the job spec that wrote it by
/// `spec_blake3` (the BLAKE3 of that attempt's `job.json`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResumeState {
    pub schema: String,
    pub job_id: String,
    pub attempt: u32,
    /// BLAKE3 of the `job.json` of the attempt that wrote this RESUME (M7).
    pub spec_blake3: String,
    pub chart: Value,
    pub frame: Frame,
    pub rules_committed: u64,
    /// Committed prefix of `out/F.table.jsonl`.
    pub table: CommittedFile,
    /// Committed prefix of `out/F.queue.jsonl` (L2 generate; null when the kind has no queue log).
    #[serde(default)]
    pub queue: Option<CommittedFile>,
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
    /// Engine-private state needed to continue IDENTICALLY (e.g. the running counters that end records repeat);
    /// opaque to the supervisor, absent when empty (lane eng2, INTERFACE CHANGE 2026-11-14).
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub engine: serde_json::Map<String, Value>,
}

impl ResumeState {
    /// Schema, digest forms, frame (always PRESENTED), layout paths, and the internal consistency of the resume
    /// point: `rules_committed` = `table.rules` = `counters_at_commit.rules_committed`.
    pub fn check(&self) -> Result<(), String> {
        if self.schema != RESUME_SCHEMA {
            return Err(format!("schema {:?} is not {RESUME_SCHEMA:?}", self.schema));
        }
        if !is_blake3_hex(&self.spec_blake3) {
            return Err("spec_blake3 is not a BLAKE3 hex digest".into());
        }
        if self.frame != Frame::Presented {
            return Err("RESUME frame must be PRESENTED".into());
        }
        self.table
            .check(&crate::layout::RunLayout::out_rel(crate::layout::TABLE))?;
        if let Some(q) = &self.queue {
            q.check(&crate::layout::RunLayout::out_rel(crate::layout::QUEUE))?;
        }
        if self.rules_committed != self.table.rules
            || self.counters_at_commit.rules_committed != self.rules_committed
        {
            return Err(
                "rules_committed, table.rules and counters_at_commit.rules_committed differ".into(),
            );
        }
        Ok(())
    }
}

/// `spec.resume` (revised by audit A C1): the RESUME record to continue from, by path and digest, and NOTHING else.
/// Every byte count and digest of the committed prefix comes from that record (one consistent object); the
/// previous run directory is the directory of `from`, and its files are found by the layout constants. A supervisor
/// builds it from the last RESUME of the stopped attempt, never from OUTCOME's `committed_bytes` (which may be AHEAD
/// of RESUME when `resume_every_s` > 0): [`crate::classify::resume_ref_for`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResumeRef {
    /// `<prev RUN>/RESUME.json` (absolute).
    pub from: String,
    /// BLAKE3 of that RESUME.json file.
    pub blake3: String,
}

impl ResumeRef {
    pub fn validate(&self) -> Result<(), String> {
        let p = std::path::Path::new(&self.from);
        if !p.is_absolute()
            || p.file_name().and_then(|n| n.to_str()) != Some(crate::layout::RESUME)
            || p.components().any(|c| {
                !matches!(
                    c,
                    std::path::Component::RootDir | std::path::Component::Normal(_)
                )
            })
        {
            return Err(format!(
                "from must be an absolute <prev RUN>/{} path without . or ..",
                crate::layout::RESUME
            ));
        }
        if !is_blake3_hex(&self.blake3) {
            return Err("blake3 must be a BLAKE3 hex digest".into());
        }
        Ok(())
    }

    /// The previous attempt's run directory (the directory of `from`).
    pub fn prev_run(&self) -> std::path::PathBuf {
        std::path::Path::new(&self.from)
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_default()
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
    crate::canon::check_document(line.as_bytes()).map_err(|e| e.to_string())?;
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
