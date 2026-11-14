//! The configuration of the close-from-material source (`rustred.campaign.closeloop.v1`, interfaces/closeloop.md 2).
//! Everything node-specific (names, paths, orders, batch sizes, budgets) is DATA here (rule L8); unknown fields are
//! refused (a misspelt budget must not silently disable it).

use rustred_campaign_contract::fake::RecipeQuirk;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const CONFIG_SCHEMA: &str = "rustred.campaign.closeloop.v1";
/// The role of the campaign-spec input that names the configuration file.
pub const CONFIG_ROLE: &str = "closeloop";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CloseLoopConfig {
    pub schema: String,
    pub nodes: Vec<NodeCfg>,
}

/// What the start set of the import spec stands for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Target {
    /// The start set is the spec's box (R41 P-1): the node is closed as BOXED.
    #[default]
    Boxed,
    /// The start set is a full-coverage start set (FULL closure).
    Full,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HandCfg {
    pub path: PathBuf,
    /// An `import::HandFormat` (column map of the hand statement).
    pub format: serde_json::Value,
}

/// Work-counter budgets of one node (never a clock).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Budgets {
    #[serde(default = "d_max_rounds")]
    pub max_rounds: u32,
    #[serde(default = "d_max_solve_keys")]
    pub max_solve_keys: u64,
    #[serde(default = "d_max_solve_jobs")]
    pub max_solve_jobs: u64,
    /// Attempts (jobs) of one solve item without a per-key decision before it goes to attention.
    #[serde(default = "d_max_job_attempts")]
    pub max_job_attempts: u32,
}

impl Default for Budgets {
    fn default() -> Self {
        Budgets {
            max_rounds: d_max_rounds(),
            max_solve_keys: d_max_solve_keys(),
            max_solve_jobs: d_max_solve_jobs(),
            max_job_attempts: d_max_job_attempts(),
        }
    }
}

fn d_max_rounds() -> u32 {
    50
}
fn d_max_solve_keys() -> u64 {
    1_000_000
}
fn d_max_solve_jobs() -> u64 {
    100_000
}
fn d_max_job_attempts() -> u32 {
    2
}

/// TESTS ONLY: the fake child answers the batch from an oracle file (closeloop.md 2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FakeSolve {
    /// JSONL `{"source": <recipe source id>, "key": [..], "verdict": "SOLVED"|.., "rhs": [[..], ..]}`.
    pub oracle: PathBuf,
    #[serde(default)]
    pub ms: u64,
    /// Contract quirks the fake child emulates (a lying child).
    #[serde(default)]
    pub quirks: Vec<RecipeQuirk>,
    /// Program key of the fake child in `spec.programs` (default `rr_fake_child`).
    #[serde(default = "d_fake_program")]
    pub program: String,
}

fn d_fake_program() -> String {
    "rr_fake_child".into()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SolveCfg {
    /// Key of `spec.programs` (the engine with the solve-recipe job mode).
    pub program: String,
    pub family: PathBuf,
    /// The node order: a name or a NodeOrder JSON object (passed to `--order` verbatim / as compact JSON).
    pub order: serde_json::Value,
    #[serde(default = "d_keys_per_job")]
    pub max_keys_per_job: usize,
    #[serde(default = "d_one")]
    pub threads: u32,
    #[serde(default)]
    pub max_trace_rows: Option<u64>,
    #[serde(default)]
    pub key_budget_s: Option<u64>,
    #[serde(default = "d_cap")]
    pub cap_bytes: u64,
    #[serde(default = "d_peak")]
    pub expected_peak_bytes: u64,
    #[serde(default = "d_heartbeat")]
    pub heartbeat_s: u32,
    #[serde(default = "d_grace")]
    pub stop_grace_s: u32,
    /// The determinism id the answers must carry (answers of another id are refused); None = the answers' own.
    #[serde(default)]
    pub determinism_id: Option<String>,
    /// The relation rule in force (coordinator's ruling: `m1_first` wanted): `m1_first` requires every answer to
    /// carry `relation_rule: m1_first` (engine v1.2); `first_admissible` (default) = the comparator's first hit
    /// (engine v1.1 has no field: then labelled so). Recorded in every batch file, answers line and result.
    #[serde(default = "d_relation_rule")]
    pub relation_rule: String,
    #[serde(default)]
    pub fake: Option<FakeSolve>,
}

fn d_relation_rule() -> String {
    "first_admissible".into()
}

fn d_keys_per_job() -> usize {
    16
}
fn d_one() -> u32 {
    1
}
fn d_cap() -> u64 {
    2 << 30
}
fn d_peak() -> u64 {
    1 << 30
}
fn d_heartbeat() -> u32 {
    10
}
fn d_grace() -> u32 {
    30
}

impl SolveCfg {
    /// The `--order` argument.
    pub fn order_arg(&self) -> String {
        match &self.order {
            serde_json::Value::String(s) => s.clone(),
            v => v.to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeCfg {
    pub node: String,
    pub import_spec: PathBuf,
    #[serde(default)]
    pub import_base: Option<PathBuf>,
    #[serde(default)]
    pub target: Target,
    #[serde(default)]
    pub hand: Option<HandCfg>,
    #[serde(default = "d_threads")]
    pub check_threads: usize,
    /// R49: appended as the LAST recipe source of the import spec (faces first met while closing).
    #[serde(default)]
    pub default_recipe: Option<PathBuf>,
    /// Overrides the import spec's `required_evidence` (`imported` | `replay` | `per-key`).
    #[serde(default)]
    pub required_evidence: Option<String>,
    #[serde(default)]
    pub budgets: Budgets,
    pub solve: SolveCfg,
}

fn d_threads() -> usize {
    4
}

impl NodeCfg {
    pub fn base(&self) -> PathBuf {
        self.import_base.clone().unwrap_or_else(|| {
            self.import_spec
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("/"))
        })
    }
}

impl CloseLoopConfig {
    pub fn from_json(text: &str) -> Result<CloseLoopConfig, String> {
        let c: CloseLoopConfig =
            serde_json::from_str(text).map_err(|e| format!("closeloop config: {e}"))?;
        c.validate()?;
        Ok(c)
    }

    pub fn read(path: &Path) -> Result<CloseLoopConfig, String> {
        let t = std::fs::read_to_string(path)
            .map_err(|e| format!("closeloop config {}: {e}", path.display()))?;
        Self::from_json(&t)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema != CONFIG_SCHEMA {
            return Err(format!(
                "closeloop config schema {:?}, expected {CONFIG_SCHEMA}",
                self.schema
            ));
        }
        if self.nodes.is_empty() {
            return Err("closeloop config lists no node".into());
        }
        let mut seen = std::collections::BTreeSet::new();
        for n in &self.nodes {
            if n.node.is_empty() {
                return Err("a node without a name".into());
            }
            if !seen.insert(super::ids::safe(&n.node)) {
                return Err(format!(
                    "node {:?} twice (or two names with one safe form)",
                    n.node
                ));
            }
            if n.solve.max_keys_per_job == 0 || n.solve.threads == 0 || n.check_threads == 0 {
                return Err(format!(
                    "node {}: max_keys_per_job, threads and check_threads must be >= 1",
                    n.node
                ));
            }
            if n.budgets.max_job_attempts == 0 {
                return Err(format!("node {}: max_job_attempts must be >= 1", n.node));
            }
            if !matches!(
                n.solve.relation_rule.as_str(),
                "m1_first" | "first_admissible"
            ) {
                return Err(format!(
                    "node {}: relation_rule {:?} (m1_first | first_admissible)",
                    n.node, n.solve.relation_rule
                ));
            }
            if let Some(r) = &n.required_evidence
                && !matches!(r.as_str(), "imported" | "replay" | "per-key")
            {
                return Err(format!("node {}: required_evidence {r:?}", n.node));
            }
        }
        Ok(())
    }
}
