//! One candidate rule as the acceptance chain sees it: the raw record line (bytes as the producer wrote them, never
//! re-serialised), its parsed fields, its case domain and its right-hand side (key pattern, coefficient text).

use serde_json::Value;

use crate::closure::sha256::Sha256;
use crate::domain::{CaseDomain, DomainOptions};
use crate::key::NodeFrame;
use crate::record::{ReadOptions, RecordReader, RhsMode, RuleRecord, TableLine};

/// A rule record with everything the chain needs. Built once per candidate ([`RuleText::parse`]).
#[derive(Clone, Debug)]
pub struct RuleText {
    /// The record line exactly as read (without the trailing newline).
    pub line: Vec<u8>,
    /// `sha256:<hex>` of [`RuleText::line`].
    pub line_sha256: String,
    /// The core reader's view (fixed, equations, exceptions, target, witness loci, dispatch, index).
    pub record: RuleRecord,
    /// The whole record (for the identity test and the coefficient texts).
    pub value: Value,
    /// (key pattern, coefficient text) in record order.
    pub rhs: Vec<(String, String)>,
    /// `guard_version` of the record (absent = pre-guard-fix).
    pub guard_version: Option<i64>,
    /// The record declares symmetry rows (`sym_rows` non-null, nonzero, not false).
    pub sym_rows: bool,
    /// The record holds an exception AND-list with no literal (refused: rr_proof EMPTY_EXCEPTION_BRANCH).
    pub empty_exception_branch: bool,
    /// The case domain (None when it could not be built: see [`RuleText::domain_error`]).
    pub domain: Option<CaseDomain>,
    pub domain_error: Option<String>,
}

/// Why a line is not a rule record at all (caller mistake: the chain needs a record to judge).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuleTextError {
    NotJson(String),
    NotARule(String),
    Reader(String),
}

impl std::fmt::Display for RuleTextError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuleTextError::NotJson(e) => write!(f, "rule line is not JSON: {e}"),
            RuleTextError::NotARule(e) => write!(f, "not a rule record: {e}"),
            RuleTextError::Reader(e) => write!(f, "record reader: {e}"),
        }
    }
}
impl std::error::Error for RuleTextError {}

/// `sha256:<hex>` of bytes.
pub fn sha256_tag(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("sha256:{}", h.finish_hex())
}

fn rhs_pairs(v: &Value) -> Result<Vec<(String, String)>, String> {
    let arr = match v.get("rhs") {
        Some(Value::Array(a)) => a.clone(),
        Some(Value::String(s)) => match serde_json::from_str::<Value>(s) {
            Ok(Value::Array(a)) => a,
            _ => return Err("rhs is a string but not a JSON array".into()),
        },
        None | Some(Value::Null) => return Err("rule without rhs".into()),
        Some(_) => return Err("rhs is not an array".into()),
    };
    arr.iter()
        .map(|t| match (t.get(0).and_then(Value::as_str), t.get(1).and_then(Value::as_str)) {
            (Some(k), Some(c)) => Ok((k.to_string(), c.to_string())),
            _ => Err("rhs term is not [pattern, coefficient]".to_string()),
        })
        .collect()
}

impl RuleText {
    /// Parse one rule line in the node's frame. The domain is built with the default [`DomainOptions`] (target from
    /// the record, chart test, witness loci as pole loci); a domain error is KEPT (the chain rejects with it), only a
    /// line that is not a rule record at all is an error.
    pub fn parse(line: &[u8], frame: &NodeFrame) -> Result<RuleText, RuleTextError> {
        let line = line.strip_suffix(b"\n").unwrap_or(line);
        let value: Value =
            serde_json::from_slice(line).map_err(|e| RuleTextError::NotJson(e.to_string()))?;
        if value.get("type").and_then(Value::as_str) != Some("rule") {
            return Err(RuleTextError::NotARule("type is not \"rule\"".into()));
        }
        let mut rd = RecordReader::new(line, ReadOptions { rhs: RhsMode::KeysOnly });
        let record = match rd.next() {
            Some(Ok(TableLine::Rule(r))) => *r,
            Some(Ok(_)) => return Err(RuleTextError::NotARule("reader saw another line kind".into())),
            Some(Err(e)) => return Err(RuleTextError::Reader(e.to_string())),
            None => return Err(RuleTextError::NotARule("empty line".into())),
        };
        let rhs = rhs_pairs(&value).map_err(RuleTextError::NotARule)?;
        let guard_version = value.get("guard_version").and_then(Value::as_i64);
        let sym_rows = match value.get("sym_rows") {
            None | Some(Value::Null) | Some(Value::Bool(false)) => false,
            Some(Value::Number(n)) => n.as_f64() != Some(0.0),
            Some(Value::String(s)) => !(s.is_empty() || s == "0"),
            Some(_) => true,
        };
        let empty_exception_branch = record.exceptions.iter().any(|b| b.is_empty());
        let (domain, domain_error) =
            match CaseDomain::from_rule_record(&record, frame, DomainOptions::default()) {
                Ok(d) => (Some(d), None),
                Err(e) => (None, Some(e.to_string())),
            };
        Ok(RuleText {
            line_sha256: sha256_tag(line),
            line: line.to_vec(),
            record,
            value,
            rhs,
            guard_version,
            sym_rows,
            empty_exception_branch,
            domain,
            domain_error,
        })
    }

    /// The record's `index` (else `i`).
    pub fn index(&self) -> Option<i64> {
        self.record.index.or(self.record.i)
    }

    /// True iff every slot is fixed (a POINT record: its domain is at most one key).
    pub fn is_point(&self) -> bool {
        !self.record.fixed.is_empty() && self.record.fixed.iter().all(Option::is_some)
    }

    /// Witness loci status of the record (A0, R24 (6)).
    pub fn witness_status(&self) -> WitnessStatus {
        match &self.record.witness_loci {
            None => WitnessStatus::Absent,
            Some(w) if w.iter().any(|s| s.trim() == "elided" || s.contains("...")) => {
                WitnessStatus::Elided
            }
            Some(_) if self.value.get("witness_loci_elided").is_some_and(|v| v != &Value::Bool(false)) => {
                WitnessStatus::Elided
            }
            Some(w) => WitnessStatus::Present { count: w.len() },
        }
    }
}

/// Whether the record carries its witness loci in full.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WitnessStatus {
    Absent,
    Elided,
    Present { count: usize },
}
