//! Campaign spec `rustred.campaign.spec.v1` (schema `schemas/campaign-spec.schema.json`): plan 3.3.1 re-expressed against
//! the as-built ledger and contract, with the user's rulings R40 (U-6, U-7, U-9, U-11, U-12, U-13). Immutable for a
//! campaign; its BLAKE3 is `campaign_created.spec_blake3` of the ledger.
//!
//! Ledger mapping (what the supervisor registers at start): `campaign` -> `campaign_created.campaign_id`;
//! `terminal_bound` -> `campaign_created.terminal_bound`; every [`InputFile`] (family, tables, rows file, box) and every
//! program (`program:<name>`, role `binary`) -> `input_registered`; per node -> `node_registered {node, inputs,
//! terminal_bound: terminals.bound, spec_box: box.id}`.

use super::{Extra, InputFile};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CampaignSpec {
    pub schema: String,
    pub campaign: String,
    pub kind: CampaignKind,
    /// U-13: `close` or `optimise` (boot from an existing result, write beside it).
    pub mode: Mode,
    #[serde(default)]
    pub optimise_from: Option<OptimiseFrom>,
    pub description: String,
    pub dirs: Dirs,
    pub programs: BTreeMap<String, Program>,
    pub licence: Licence,
    pub recipe_version: String,
    pub phases: Vec<String>,
    /// Default fallback terminal bound per node (ledger `campaign_created.terminal_bound`; default 100).
    pub terminal_bound: u32,
    pub nodes: Vec<NodeSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<Value>,
    pub ladders: Ladders,
    pub acceptance: Acceptance,
    pub resources: Resources,
    pub determinism: Determinism,
    pub output: OutputOptions,
    pub economy: Economy,
    #[serde(default)]
    pub seed_policy: Option<FileSpec>,
    pub progress: ProgressSpec,
    pub outputs: Outputs,
    pub seeds: Seeds,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inputs: Option<Value>,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CampaignKind {
    Boxed,
    Reproduce,
    Test,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Close,
    Optimise,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptimiseFrom {
    pub kind: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blake3: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dirs {
    pub campaign_dir: String,
    /// The non-snapshotted large-output root; records name files under it as `large:<relative path>`.
    pub large_root: String,
}

/// A file by absolute path + digest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileSpec {
    pub path: String,
    pub blake3: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Program {
    pub path: String,
    pub blake3: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub optional: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<Vec<String>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Licence {
    pub env: String,
    #[serde(default)]
    pub env_file: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NodeSpec {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,
    /// INFORMATIONAL echo of the arity / line count the campaign derives from the family (P-5): never an input.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derived: Option<Value>,
    pub family: InputFile,
    pub order: Value,
    #[serde(default)]
    pub rows_file: Option<InputFile>,
    pub tables: Vec<TableSpec>,
    /// OPTIONAL user-provided box (R35, U-7): an input with its own id and digest; never chosen by the campaign.
    #[serde(default)]
    pub r#box: Option<BoxSpec>,
    #[serde(default)]
    pub envelope: Option<Value>,
    /// Hand-down envelope key sets, each `measured` | `proved` with the digests of the tables they came from.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub envelopes: Vec<EnvelopeFile>,
    /// P-4 / L9: preferred terminals (keys or sector masks): a preference, never a restriction.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub preferred_terminals: Vec<Value>,
    #[serde(default)]
    pub symmetry: Option<Value>,
    pub terminals: NodeTerminals,
    /// U-12: `replay` (complete nodes) or `per-key` (the exact per-key proof).
    pub certification: Certification,
    /// U-6: the full-closure attempt before the box fallback.
    pub full_attempt: FullAttempt,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<String>,
    #[serde(default)]
    pub import: Option<Value>,
    /// U-9 / U-10: comparison only; never steers anything.
    #[serde(default)]
    pub reference: Option<Value>,
    #[serde(default)]
    pub children: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase1: Option<Value>,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Certification {
    #[serde(rename = "replay")]
    Replay,
    #[serde(rename = "per-key")]
    PerKey,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FullAttempt {
    pub cpu_hours: u32,
    pub fixer_rounds: u32,
}

impl Default for FullAttempt {
    /// U-6 default: 50 CPU-hours or 5 fixer rounds, whichever first.
    fn default() -> Self {
        FullAttempt {
            cpu_hours: 50,
            fixer_rounds: 5,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableSpec {
    pub id: String,
    pub path: String,
    pub blake3: String,
    /// Format tag (e.g. `table_dump.v1`).
    pub format: String,
    pub priority: u32,
    pub trust: String,
    /// Who made it, when, from what (P-1: imported material carries provenance).
    pub provenance: Value,
    #[serde(default)]
    pub filter: Option<Value>,
}

/// The user's box (R35, U-7, P-5): a generic declarative predicate with its own id and digest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoxSpec {
    /// Input id (ledger `node_registered.spec_box`, `input_registered` role `box`).
    pub id: String,
    /// BLAKE3 of the canonical JSON of `predicate` (or of the file at `path`).
    pub blake3: String,
    #[serde(default)]
    pub path: Option<String>,
    /// Layers over (dots, rank), per-line power cap, explicit key list (kept as JSON: the box module interprets it).
    pub predicate: Value,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvelopeFile {
    pub id: String,
    pub path: String,
    pub blake3: String,
    /// `measured` (closure against it is PROVISIONAL, DEV-12) or `proved`.
    pub label: String,
    pub from_tables: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NodeTerminals {
    pub declared: Vec<Vec<i32>>,
    /// Fallback terminal bound of this node; None = the campaign's `terminal_bound`.
    pub bound: Option<u32>,
    #[serde(default)]
    pub fallback: Option<Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ladders {
    pub phase2: Vec<RungSpec>,
    #[serde(default)]
    pub phase1: Option<Value>,
    pub max_item_attempts: u32,
    pub max_redraws: u32,
}

/// One rung: the contract's work counters and wall-clock budgets (child-job 3.3) plus a memory class.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RungSpec {
    pub rung: String,
    pub work: BTreeMap<String, Option<u64>>,
    pub wall: BTreeMap<String, Option<u64>>,
    pub mem_bytes: u64,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Acceptance {
    pub policy: String,
    pub certify: String,
    pub generic_threshold: u64,
    pub verify_prime_range: [u64; 2],
    pub random_d: bool,
    pub canary_every: u64,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Resources {
    pub max_cores: u32,
    pub max_jobs: u32,
    pub mem_total_bytes: u64,
    pub host_floor_bytes: u64,
    pub cpu_policy: CpuPolicy,
    pub grace_seconds: u32,
    #[serde(flatten)]
    pub extra: Extra,
}

/// process-layer.md 9: `--cpu-policy site --site-policy FILE` or `--cpu-policy plain --cpus LIST`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CpuPolicy {
    Site { site_policy: FileSpec },
    Plain { cpus: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Determinism {
    Recorded,
    Strict,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputOptions {
    /// Always false in a campaign (R-f: elided witness loci are unusable).
    pub elide_witness_loci: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Economy {
    /// U-13: CPU multiple of the first acceptable rule spent looking for a better one (default 2.0).
    pub extra_budget_factor: f64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgressSpec {
    pub cadence_seconds: u32,
    #[serde(default)]
    pub registry: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Outputs {
    pub export: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Seeds {
    pub master: String,
}

impl CampaignSpec {
    /// Reader checks beyond serde: schema major, unique node ids, no steering inputs (U-9), elision forbidden.
    pub fn check(&self) -> Result<(), String> {
        super::check_major(&self.schema, super::SPEC_SCHEMA)?;
        let mut ids = std::collections::BTreeSet::new();
        for n in &self.nodes {
            if !ids.insert(n.id.as_str()) {
                return Err(format!("node {} listed twice", n.id));
            }
            for k in ["knockout", "lp", "master_lower_bound", "masters"] {
                if n.extra.contains_key(k) {
                    return Err(format!(
                        "node {}: `{k}` is not an input (U-9; use `reference`)",
                        n.id
                    ));
                }
            }
        }
        if self.output.elide_witness_loci {
            return Err("output.elide_witness_loci must be false in a campaign (R-f)".into());
        }
        Ok(())
    }

    /// The terminal bound that applies to `node`.
    pub fn terminal_bound_of(&self, node: &NodeSpec) -> u32 {
        node.terminals.bound.unwrap_or(self.terminal_bound)
    }
}
