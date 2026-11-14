//! Work item document `rustred.campaign.item.v1` (schema `schemas/item.schema.json`). No as-built writer: items live in
//! the ledger (`item_created` / `item`) and in the reducer's `ItemRec`; this document uses the AS-BUILT phase words and
//! evidence levels with the plan's descriptive fields (plan 3.3.3).

use super::{Extra, Why};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ItemDoc {
    pub schema: String,
    pub id: String,
    #[serde(default)]
    pub short: Option<String>,
    pub node: String,
    pub kind: String,
    #[serde(default)]
    pub face: Option<Face>,
    #[serde(default)]
    pub locus: Option<Locus>,
    #[serde(default)]
    pub frozen: Value,
    #[serde(default)]
    pub points: Option<u64>,
    pub rung: String,
    pub parent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    pub label: String,
    pub weight: u64,
    pub inputs: Vec<String>,
    pub node_inputs_digest: String,
    /// As-built `ItemPhase` tag (open, in_progress, produced, accepted, rejected, escalated, terminal_candidate,
    /// terminal, stale, retired).
    pub phase: String,
    /// Evidence level of an accepted / retired item (geometry_accepted, identity_tested, identity_proved).
    pub level: Option<String>,
    pub jobs: Vec<String>,
    pub why: Why,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Face {
    pub sd: Vec<u32>,
    pub sn: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Locus {
    pub equations: Vec<String>,
    pub r#type: String,
}
