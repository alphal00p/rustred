//! Child-job contract `rustred.child-job.v1` shared by the engine's child programs, the campaign supervisor and the
//! fake child. Normative text: `plan/auto-campaign/interfaces/child-job.md` of the p-integral workspace (section
//! numbers in the docs below refer to it).
//!
//! This crate must stay tiny: `rustred` (core) depends on it (ruling R-a: serde, serde_json, blake3 only). It holds
//! types, pure functions and plain file helpers; no process spawning, no signals, no terminal code.
//!
//! Map of the modules:
//! - [`exit`]: the exit-code table (3.6); [`stop`]: stop codes, stop sources, job status (3.7);
//! - [`spec`]: the job spec `job.json` (3.1.2) with strict validation; budgets and work counters (3.3);
//! - [`outcome`]: `OUTCOME.json` (3.7); [`events`]: `EVENTS.jsonl` lines (3.5); [`resume`]: `RESUME.json` and case
//!   lines (3.8); [`layout`]: the run directory (3.1.3);
//! - [`canon`]: volatile keys and the canonical record form + digest (3.2.3, ruling R-c);
//! - [`seal`]: the record writer with running digests, table sealing and the sealed-table verifier (3.4.4, 3.9, R-b);
//! - [`atomic`]: atomic file writes, ENOSPC classification, the disk reserve (3.9, 3.10);
//! - [`preflight`]: the child's start checks that end in exit 40/41/42 (3.1.3, 3.2.2);
//! - [`classify`]: the supervisor's decision table (6.5) and the file assessment feeding it;
//! - [`fake`]: the script of the fake child `rr_fake_child` (section 4) and the witness-locus elision policy (R-f).

pub mod atomic;
pub mod canon;
pub mod classify;
pub mod events;
pub mod exit;
pub mod fake;
pub mod layout;
pub mod outcome;
pub mod preflight;
pub mod resume;
pub mod seal;
pub mod spec;
pub mod stop;
pub mod time;

pub use canon::{CanonError, canonical_line};
pub use classify::{Classification, Evidence, KillReason, OutcomeKind, WaitStatus, classify};
pub use exit::ExitCode;
pub use outcome::{JobOutcome, Outcome};
pub use seal::{SealVerdict, TableSeal, verify_table};
pub use spec::{JobKind, JobSpec, SpecError};
pub use stop::{JobStatus, StopCode, StopSource};

/// The contract name (`schema` of the job spec; listed in `version --json`).
pub const CONTRACT: &str = "rustred.child-job.v1";
/// `schema` of `OUTCOME.json`'s `job` object.
pub const OUTCOME_SCHEMA: &str = "rustred.child-job.outcome.v1";
/// `schema` of `RESUME.json`.
pub const RESUME_SCHEMA: &str = "rustred.child-job.resume.v1";
/// `v` of every EVENTS line.
pub const EVENTS_VERSION: u32 = 1;
/// Build-time environment variable carrying the git commit (ruling R-e); absent -> [`UNKNOWN_COMMIT`].
pub const BUILD_GIT_COMMIT_ENV: &str = "RUSTRED_BUILD_GIT_COMMIT";
/// Value reported when the build environment had no commit.
pub const UNKNOWN_COMMIT: &str = "unknown";

/// Lower-case hex BLAKE3 of a byte string.
pub fn blake3_hex(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

/// True iff `s` is 64 lower-case hex characters (the form of every digest in the contract).
pub fn is_blake3_hex(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// The git commit to report, from the value of [`BUILD_GIT_COMMIT_ENV`] captured at build time
/// (`option_env!("RUSTRED_BUILD_GIT_COMMIT")` in the binary): empty or absent -> "unknown".
pub fn git_commit_from_build_env(v: Option<&str>) -> String {
    match v.map(str::trim) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => UNKNOWN_COMMIT.to_string(),
    }
}

/// The one line printed by `<program> version --json` (3.2.1).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VersionInfo {
    pub program: String,
    pub package_version: String,
    pub git_commit: String,
    pub contract: Vec<String>,
    pub features: Vec<String>,
    pub job_kinds: Vec<spec::JobKind>,
    #[serde(default)]
    pub arities: Vec<u32>,
}
