//! Typed readers of the AS-BUILT progress files `rustred.campaign.progress.v1` (written by
//! `rustred_campaign::progress::derive` as `serde_json::Value`; schemas `schemas/progress-*.schema.json`). For the
//! terminal dashboard, the website side and tests; unknown fields are kept in `extra` (additive minor versions).

use super::{Extra, FileRef};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub type Counts = BTreeMap<String, u64>;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerPos {
    pub seq: u64,
    pub head: String,
    pub last_utc: String,
}

/// `progress/campaign.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CampaignProgress {
    pub schema: String,
    pub file: String,
    pub campaign_id: String,
    pub written_utc: String,
    pub source_of_truth: String,
    pub ledger: LedgerPos,
    pub campaign: CampaignBlock,
    pub resources: ResourcesBlock,
    pub jobs: JobsBlock,
    pub nodes: NodesBlock,
    pub economy: CampaignEconomy,
    pub items: ItemsBlock,
    pub rules: RulesBlock,
    pub terminals: TerminalsBlock,
    pub certified: CertifiedBlock,
    pub closure: ClosureBlock,
    pub cost: CostBlock,
    pub eta: Eta,
    pub files: Files,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CampaignBlock {
    pub phase: Option<String>,
    pub created_utc: String,
    pub spec_blake3: String,
    pub sessions: u32,
    pub crash_recoveries: u32,
    pub lock_takeovers: u32,
    pub torn_tail_recoveries: u32,
    pub segments_opened: u32,
    pub pending_operations: u64,
    pub unsettled_jobs: u64,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourcesBlock {
    pub cpus_in_use: u32,
    pub cpus_allowed: u32,
    pub live_jobs: u32,
    pub reserved_bytes: u64,
    pub measured_rss_bytes: u64,
    pub mem_cap_bytes: u64,
    pub host_mem_available_bytes: u64,
    pub host_floor_bytes: u64,
    pub sampled_utc: Option<String>,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobsBlock {
    pub total: u64,
    pub by_state: Counts,
    pub settled_by_class: Counts,
    pub reserved_bytes_running: u64,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodesBlock {
    pub total: u64,
    pub by_phase: Counts,
    /// full / boxed / open (R33 / R35).
    pub by_closure: Counts,
    pub certified: u64,
    #[serde(flatten)]
    pub extra: Extra,
}

/// Same JSON form as the ledger's `RuleEconomy`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Economy {
    pub cases: u64,
    pub rules: u64,
    pub rhs_terms: u64,
    pub monomials: u64,
    pub coefficient_degree: u64,
    pub bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CampaignEconomy {
    pub rules_before: u64,
    pub rules_after: u64,
    pub before: Economy,
    pub after: Economy,
    pub retired: u64,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemsBlock {
    pub total: u64,
    pub by_phase: Counts,
    pub by_level: BTreeMap<String, Counts>,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RulesBlock {
    pub accepted: u64,
    pub rejected: u64,
    pub rejected_by_reason: Counts,
    pub upgrades: u64,
    pub accepted_items_by_evidence: Counts,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalsBlock {
    pub declared: u64,
    pub candidates: u64,
    pub bound_per_node_default: u32,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CertifiedBlock {
    pub share: Option<f64>,
    pub identity_proved: u64,
    pub accepted: u64,
    pub basis: String,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClosureBlock {
    /// null until a closure pass (never 0 for "unknown").
    pub defect_keys: Option<u64>,
    pub nodes_checked: u64,
    pub nodes_with_defect: u64,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LevelCost {
    pub jobs: u64,
    pub cpu_seconds: f64,
    pub p25_s: f64,
    pub p50_s: f64,
    pub p75_s: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CostBlock {
    pub cpu_seconds: f64,
    pub by_level: BTreeMap<String, LevelCost>,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Range3 {
    pub low: f64,
    pub mid: f64,
    pub high: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EtaLevel {
    pub remaining: u64,
    pub samples: u64,
    pub p25_s: f64,
    pub p50_s: f64,
    pub p75_s: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Eta {
    pub estimator: String,
    pub remaining_items: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by_level: Option<BTreeMap<String, EtaLevel>>,
    pub cpu_seconds: Option<Range3>,
    pub wall_seconds: Option<Range3>,
    pub parallelism: u64,
    pub pooled_levels: Vec<String>,
    pub reason: Option<String>,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Files {
    pub events: String,
    pub nodes: BTreeMap<String, String>,
}

/// `progress/nodes/<node>.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NodeProgress {
    pub schema: String,
    pub file: String,
    pub campaign_id: String,
    pub written_utc: String,
    pub source_of_truth: String,
    pub ledger: LedgerPos,
    pub node: String,
    pub phase: String,
    /// full / boxed / open.
    pub closure_kind: String,
    pub certified: bool,
    pub target: Target,
    pub spec_box: Option<String>,
    pub attempt_ended: bool,
    pub blockers: BlockersPair,
    pub closure: NodeClosure,
    pub economy: NodeEconomy,
    pub terminals: NodeTerminals,
    pub items: NodeItems,
    pub accepted_by_evidence: Counts,
    pub inputs: Vec<String>,
    pub stale_reason: Option<String>,
    pub cpu_seconds: f64,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "target", rename_all = "snake_case", deny_unknown_fields)]
pub enum Target {
    Full,
    Boxed { box_id: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Blockers {
    pub faces: u64,
    pub loci: u64,
    pub keys: u64,
    pub list: Option<FileRef>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockersPair {
    pub full: Option<Blockers>,
    pub r#box: Option<Blockers>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NodeClosure {
    pub rounds: u32,
    pub defect_keys: Option<u64>,
    pub keyset: Option<FileRef>,
    pub closure_keys: Option<u64>,
    pub rules_used: Option<u64>,
    pub rules_total: Option<u64>,
    pub used_share: Option<f64>,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeEconomy {
    pub rules_before: u64,
    pub rules_after: u64,
    pub superseded: u64,
    pub unmeasured: u64,
    pub before: Economy,
    pub after: Economy,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeTerminals {
    pub declared: u32,
    pub bound: u32,
    pub candidates: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeItems {
    pub by_phase: Counts,
    pub by_level: BTreeMap<String, Counts>,
    pub open: u64,
}

/// `progress/events.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EventsProgress {
    pub schema: String,
    pub file: String,
    pub campaign_id: String,
    pub written_utc: String,
    pub source_of_truth: String,
    pub ledger: LedgerPos,
    pub cap: u64,
    pub events: Vec<RecentEvent>,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentEvent {
    pub seq: u64,
    pub utc: String,
    pub kind: String,
    pub text: String,
}

/// The value of `progress::item_detail` (no file; `record` = the reducer's ItemRec, kept as JSON).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ItemDetail {
    pub item: String,
    pub record: Value,
    pub jobs: Vec<ItemJob>,
    pub children: Vec<String>,
    pub why: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ItemJob {
    pub job: String,
    pub attempt: u32,
    pub state: Value,
    pub cpu_seconds: f64,
    pub run_dir: Option<String>,
}

impl NodeProgress {
    /// The FULL / BOXED / OPEN rule of ledger-and-progress.md 5.3 (also checked by validate.py).
    pub fn closure_kind_consistent(&self) -> bool {
        let closed = matches!(self.phase.as_str(), "closed" | "certified");
        match (self.closure_kind.as_str(), &self.target) {
            ("full", Target::Full) => closed,
            ("boxed", Target::Boxed { box_id }) => {
                closed && self.spec_box.as_deref() == Some(box_id)
            }
            ("open", _) => !closed,
            _ => false,
        }
    }
}
