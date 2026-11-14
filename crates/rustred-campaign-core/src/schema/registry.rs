//! Campaign registry `rustred.campaign.registry.v1` (WS/campaigns/INDEX.json; schema `schemas/registry.schema.json`),
//! rewritten temp + rename under flock of INDEX.json.lock.

use super::Extra;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Registry {
    pub schema: String,
    pub written_utc: String,
    pub campaigns: Vec<RegistryEntry>,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryEntry {
    pub id: String,
    pub dir: String,
    pub kind: String,
    pub mode: String,
    pub started_utc: String,
    pub owner: String,
    #[serde(default)]
    pub optimised_from: Option<String>,
    #[serde(flatten)]
    pub extra: Extra,
}
