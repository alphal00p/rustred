//! Terminal list `rustred.campaign.terminals.v1` (schema `schemas/terminals.schema.json`; plan 3.3.7, 3.P.4): node
//! records shaped as a superset of `p5l-terminals.v1`; never written into WS/P5L_TERMINALS.json (U-11).

use super::{Extra, FileRef};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TerminalsDoc {
    pub schema: String,
    pub campaign_id: String,
    pub written_utc: String,
    pub seq: u64,
    pub source_of_truth: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compatible_with: Option<String>,
    pub nodes: Vec<TerminalNode>,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TerminalNode {
    pub id: String,
    pub provisional: bool,
    pub closure_kind: String,
    #[serde(default)]
    pub box_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lane: Option<String>,
    #[serde(default)]
    pub graph: Option<Value>,
    #[serde(default)]
    pub r#box: Option<Value>,
    pub keys_in_box: Option<u64>,
    #[serde(default)]
    pub keys_covered: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bound: Option<u32>,
    #[serde(default)]
    pub table: Option<FileRef>,
    pub certification: String,
    /// Read-only comparison column (U-10).
    #[serde(default)]
    pub reference: Option<Value>,
    pub terminals: Vec<Terminal>,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Terminal {
    pub id: String,
    pub key: Vec<i32>,
    pub frame: String,
    #[serde(default)]
    pub presented_key: Option<Value>,
    pub label: String,
    pub origin: String,
    pub evaluation_status: String,
    pub item: Option<String>,
    pub reason: String,
    pub drawing: Value,
    #[serde(flatten)]
    pub extra: Extra,
}
