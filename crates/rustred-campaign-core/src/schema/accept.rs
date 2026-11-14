//! Acceptance record `rustred.campaign.accept.v1` (schema `schemas/accept.schema.json`; plan 3.3.5, 2.5; R8, R9).
//! One record per check run; bound to the rule by table digest + rule-line digest + byte offset.

use super::{Extra, FileRef};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AcceptRecord {
    pub schema: String,
    pub utc: String,
    pub seq_ref: Option<u64>,
    pub rule: RuleRef,
    pub check: Check,
    pub verdict: Verdict,
    pub cost: Cost,
    pub artefact: Option<FileRef>,
    pub credits: Credits,
    pub imported_from: Option<ImportedFrom>,
    pub level_policy: String,
    /// Plan 2.5.2 level A0..A4 after this record.
    pub level_after: String,
    /// The ledger's evidence word for `level_after` (A1 geometry_accepted, A2 identity_tested, A3/A4 identity_proved).
    pub evidence_level: Option<String>,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleRef {
    pub id: String,
    pub node: String,
    pub table_blake3: String,
    pub rule_line_blake3: String,
    pub ordinal: u64,
    pub byte_offset: u64,
    pub chart: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub case_blake3: Option<String>,
    pub producer: Producer,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Producer {
    /// BLAKE3 of the producing binary; `None` for historical material whose producer was never recorded (then
    /// `provenance` is required, and the line can never by itself yield a level above A0 / imported).
    pub bin_blake3: Option<String>,
    /// Provenance class: `recorded`, `unknown: historical import`, `unknown: not recorded by the producer`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<String>,
    #[serde(default)]
    pub backend: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guard_version: Option<u32>,
    /// `"unknown"` or the list of lift primes.
    pub lift_primes: Value,
    #[serde(default)]
    pub seed_recipe: Option<String>,
    #[serde(default)]
    pub seeds_blake3: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeySet {
    pub file: Option<FileRef>,
    pub count: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample: Option<Vec<Vec<i32>>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Check {
    pub kind: String,
    pub tool: String,
    pub tool_blake3: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_version: Option<String>,
    pub arithmetic: String,
    pub primes: Vec<u64>,
    pub d_values: Vec<String>,
    pub scope: String,
    pub keys: KeySet,
    pub drawn_before_start: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Verdict {
    pub tool_word: String,
    /// PASS, FAIL (with disproof), UNKNOWN, UNDECIDED, REFUSED.
    pub normalised: String,
    pub per_key_counts: BTreeMap<String, u64>,
    #[serde(default)]
    pub error_bound_log10: Option<i64>,
    pub disproof: Option<Value>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cost {
    pub cpu_ms: u64,
    pub peak_rss_bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_class: Option<String>,
}

/// Every credit names this rule; symmetry credits name the group element and the mapped key (R8, R9 (a)).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Credits {
    pub keys: KeySet,
    pub via_symmetry: Vec<SymmetryCredit>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymmetryCredit {
    pub key: Vec<i32>,
    pub group_element: Vec<u32>,
    pub mapped_key: Vec<i32>,
    pub image_rule_lower: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportedFrom {
    pub source: String,
    pub producer: String,
    pub label: String,
    pub file: FileRef,
    pub original_verdict: String,
    #[serde(default)]
    pub evidence_id: Option<String>,
}

/// The ledger evidence word of a plan level (A0 -> None).
pub fn evidence_word(level: &str) -> Option<&'static str> {
    match level {
        "A1" => Some("geometry_accepted"),
        "A2" => Some("identity_tested"),
        "A3" | "A4" => Some("identity_proved"),
        _ => None,
    }
}
