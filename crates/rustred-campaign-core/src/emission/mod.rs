//! EMISSION CONTRACT (lane gencheck; plan/auto-campaign/interfaces/emission-check.md, snapshot CONTRACT.md beside this
//! file; USER ruling 2026-11-18: every rule the automation emits carries its derivation check, the exactness class bound
//! to the right record AT THE MOMENT OF EMISSION). PURE part: no process, no clock; files only in the partition proof
//! (temporary tables in a directory the caller gives).
//!
//! - [`body`]: the body of a record and its identity (`body_sha256` from relation + domain + chart + case: stable under
//!   renumbering and member order), strip / attach the `derivation` member.
//! - [`class`]: class words, the check summary, [`class::class_of`], the [`class::Policy`].
//! - [`ladder`]: open loci and new branches -> child cases, piece exception bookkeeping, the partition proof.
//! - [`validate`]: the record-level part of `rr_campaign table-check`.
//! - [`legacy`]: LEGACY IMPORT WITH EVIDENCE BINDING (the class of a record brought in unchanged, from its bytes and the
//!   evidence files alone).
//!
//! The process part (running the producer and the checker as children, writing tables) is
//! `rustred_campaign::emission`.
pub mod body;
pub mod class;
pub mod ladder;
pub mod legacy;
pub mod validate;

use class::{CheckSummary, Classed, ConeEvidence, Kind, Origin, SampleEvidence};
use serde_json::{Value, json};

pub const SCHEMA: &str = "rustred.campaign.emission.v1";
/// Content-addressed side directory of a table `<stem>.table.jsonl`: `<stem>.emission/<area>/<sha256>.<ext>`.
pub const SIDE_SUFFIX: &str = ".emission";
pub const AREA_CERT: &str = "cert";
pub const AREA_CHECK: &str = "check";
pub const AREA_PROV: &str = "prov";
pub const AREA_POLICY: &str = "policy";
pub const AREA_KEYS: &str = "keys";
/// bound evidence rows of a legacy import (the verdict row a class rests on)
pub const AREA_EVID: &str = "evid";

/// The side directory of a table path (`x/T.table.jsonl` -> `x/T.emission`).
pub fn side_dir(table: &std::path::Path) -> std::path::PathBuf {
    let name = table
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let stem = name
        .strip_suffix(".table.jsonl")
        .or_else(|| name.strip_suffix(".jsonl"))
        .unwrap_or(&name)
        .to_string();
    table.with_file_name(format!("{stem}{SIDE_SUFFIX}"))
}

/// Relative path of a side file.
pub fn side_rel(area: &str, sha256: &str) -> String {
    let ext = if area == AREA_KEYS { "jsonl" } else { "json" };
    format!("{area}/{sha256}.{ext}")
}

fn yes() -> bool {
    true
}
fn default_cert_dir() -> String {
    "out/wcert".into()
}

/// Emission settings of a rule-producing flow (improve config `emission`): the checker, where a job's certificates are
/// (relative to its run directory; the producer must write them there), the policy (default: the contract's).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub checker: String,
    #[serde(default = "yes")]
    pub recover_rows: bool,
    #[serde(default = "default_cert_dir")]
    pub cert_dir: String,
    #[serde(default)]
    pub policy: Option<class::Policy>,
    #[serde(default)]
    pub family: Option<String>,
    #[serde(default)]
    pub floor_s: Option<f64>,
}

/// The parts of a `derivation` member (everything bulky lives in the side directory, referenced by sha256).
#[derive(Clone, Debug)]
pub struct Parts {
    pub kind: Kind,
    pub origin: Origin,
    pub body_sha256: String,
    /// producer binary sha256 and the sha256 of its full provenance record (argv, env, family, header, seeds, job)
    pub producer: Option<(String, String)>,
    /// certificate sha256, bytes, form
    pub certificate: Option<(String, u64, String)>,
    /// checker binary sha256, output sha256, the summary; or the typed reason it did not run
    pub check: Result<(String, String, CheckSummary), String>,
    /// POINT / SYMMETRY kinds: the instance-span check instead (checker sha256, output sha256, its verdict word)
    pub span: Option<(String, String, String)>,
    pub classed: Classed,
    pub cone: Option<ConeEvidence>,
    pub sample: Option<SampleEvidence>,
    pub ladder: Option<Value>,
    /// legacy import: the `legacy` member of the class (bound proof object, proof-line binding, liveness)
    pub legacy: Option<Value>,
    /// policy name and sha256
    pub policy: (String, String),
}

/// The `derivation` member (small: sha256s, verdict words, class, evidence references).
pub fn derivation(p: &Parts) -> Value {
    let check = match (&p.span, &p.check) {
        (Some((tool, out, verdict)), _) => {
            json!({"tool_sha256": tool, "output_sha256": out, "mode": "instance-span", "verdict": verdict})
        }
        (None, Ok((tool, out, s))) => {
            json!({"tool_sha256": tool, "output_sha256": out, "stages": s.record_json()})
        }
        (None, Err(why)) => json!({"ran": false, "reason": why}),
    };
    let mut class = json!({"word": p.classed.class.word(), "evidence": p.classed.evidence});
    if let Some(r) = &p.classed.reason {
        class["reason"] = json!(r);
    }
    if let Some(c) = &p.cone {
        class["cone"] = serde_json::to_value(c).unwrap_or(Value::Null);
    }
    if let Some(s) = &p.sample {
        class["sample"] = serde_json::to_value(s).unwrap_or(Value::Null);
    }
    if let Some(l) = &p.legacy {
        class["legacy"] = l.clone();
    }
    json!({
        "schema": SCHEMA,
        "body_sha256": p.body_sha256,
        "kind": p.kind.word(),
        "origin": match p.origin { Origin::Lift => "lift", Origin::Import => "import" },
        "producer": p.producer.as_ref().map(|(b, prov)| json!({"binary_sha256": b, "provenance_sha256": prov})),
        "certificate": p.certificate.as_ref().map(|(s, b, f)| json!({"sha256": s, "bytes": b, "form": f})),
        "check": check,
        "class": class,
        "ladder": p.ladder,
        "policy": {"name": p.policy.0, "sha256": p.policy.1},
    })
}

#[cfg(test)]
mod tests;
