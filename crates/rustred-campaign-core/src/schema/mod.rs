//! Serde structs of the campaign's JSON contract (WP-0b) for the formats that have NO as-built struct elsewhere.
//!
//! The hand-written JSON Schemas in `crates/rustred-campaign-core/schemas/` are the CONTRACT (plan D-36); these structs
//! round-trip every example there byte for byte after canonicalisation (`tests/schema_roundtrip.rs`). Formats that
//! already have structs are NOT duplicated here: the ledger, lock, manifest, snapshot, site policy (crate
//! `rustred-campaign`) and the child-job contract files (crate `rustred-campaign-contract`) are round-tripped in those
//! crates' `tests/schema_roundtrip.rs`.
//!
//! Conventions: digests are lower-case hex BLAKE3 strings; files named by records are [`FileRef`]s; unknown fields of
//! tolerant documents are kept in `extra` (additive minor versions survive a round trip); `Option` fields are always
//! written (null), so a reader sees every field of the schema.
//!
//! Key representation (WP-0b decision): keys are `Vec<i32>` in documents (the as-built core uses `&[i32]` slices;
//! `key::Key` is the in-memory form); big integers (primes up to 2^63) are `u64`; no bigint type in documents.

pub mod accept;
pub mod canon;
pub mod control;
pub mod item;
pub mod job;
pub mod progress;
pub mod registry;
pub mod spec;
pub mod terminals;

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Unknown fields of a tolerant document (kept for round trips; ignored by readers).
pub type Extra = BTreeMap<String, serde_json::Value>;

/// A file named by a record: path relative to CDIR or `large:`-prefixed, size, BLAKE3 (same JSON form as the
/// ledger's `FileRef` of crate `rustred-campaign`).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileRef {
    pub path: String,
    pub bytes: u64,
    pub blake3: String,
}

/// An input named by the spec: input id (as registered in the ledger), absolute path, digest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputFile {
    pub id: String,
    pub path: String,
    pub blake3: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
}

/// `why` of an item: reason code (plan 3.P.5) and a sentence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Why {
    pub code: String,
    pub text: String,
}

/// Schema names of the documents defined in this module.
pub const SPEC_SCHEMA: &str = "rustred.campaign.spec.v1";
pub const ITEM_SCHEMA: &str = "rustred.campaign.item.v1";
pub const JOB_SCHEMA: &str = "rustred.campaign.job.v1";
pub const ACCEPT_SCHEMA: &str = "rustred.campaign.accept.v1";
pub const TERMINALS_SCHEMA: &str = "rustred.campaign.terminals.v1";
pub const CONTROL_SCHEMA: &str = "rustred.campaign.control.v1";
pub const REGISTRY_SCHEMA: &str = "rustred.campaign.registry.v1";
pub const PROGRESS_SCHEMA: &str = "rustred.campaign.progress.v1";

/// Reader rule of plan 3.3: the `schema` value `<base>.v<major>`; a known base with another major is refused.
pub fn check_major(found: &str, expected: &str) -> Result<(), String> {
    if found == expected {
        return Ok(());
    }
    let base = |s: &str| s.rsplit_once(".v").map(|(b, _)| b.to_string());
    match (base(found), base(expected)) {
        (Some(a), Some(b)) if a == b => Err(format!(
            "schema {found}: major version not understood (this reader reads {expected})"
        )),
        _ => Err(format!("schema {found:?} is not {expected:?}")),
    }
}
