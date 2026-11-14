//! Job record `rustred.campaign.job.v1` (schema `schemas/campaign-job.schema.json`): plan 3.3.4 with the as-built
//! ledger's names (`job_queued` + `job.finished`). No as-built writer yet.

use super::{Extra, FileRef};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CampaignJob {
    pub schema: String,
    pub job: String,
    #[serde(default)]
    pub display: Option<String>,
    pub item: String,
    pub node: String,
    pub rung: String,
    pub attempt: u32,
    pub kind: String,
    pub program: ProgramDigest,
    pub spec_blake3: String,
    pub run_dir: String,
    pub cpus: Vec<u32>,
    pub reserved_bytes: u64,
    #[serde(default)]
    pub started_utc: Option<String>,
    #[serde(default)]
    pub finished_utc: Option<String>,
    /// The contract's `OutcomeKind` word (= the ledger's FinishClass).
    pub class: String,
    pub exit_code: Option<i32>,
    pub signal: Option<i32>,
    pub sealed_output: Option<FileRef>,
    pub outcome: Option<FileRef>,
    pub cpu_ms: u64,
    pub wall_ms: u64,
    pub peak_rss_bytes: u64,
    pub time_decided: bool,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgramDigest {
    pub name: String,
    pub blake3: String,
}
