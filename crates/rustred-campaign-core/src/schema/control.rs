//! Operator control request `rustred.campaign.control.v1` (schema `schemas/control.schema.json`; plan 3.3.8).

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ControlRequest {
    pub schema: String,
    pub id: String,
    pub utc: String,
    pub by: String,
    pub verb: Verb,
    pub target: Option<String>,
    pub args: BTreeMap<String, Value>,
    pub reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixture_origin: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verb {
    Pause,
    Resume,
    Drain,
    Stop,
    Retry,
    Skip,
    Terminal,
    Budget,
    Workers,
    Hold,
    Release,
    Priority,
    Note,
}
