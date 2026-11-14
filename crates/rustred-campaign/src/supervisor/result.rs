//! The FINAL RESULT of a campaign as the interrupt tests compare it (interfaces/supervisor.md section 2; user ruling
//! U-1: identical ACCEPTED tables, by canonical digest, given the same ledger decisions; time-decided outcomes listed).
//! Derived from the reduced ledger only.

use crate::state::{CampaignState, ItemPhase};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FinalResult {
    /// item -> (rule id, record blake3) of every accepted item.
    pub accepted: BTreeMap<String, (String, String)>,
    /// item -> phase word for every item that is not escalated (the disposition of each chain).
    pub dispositions: BTreeMap<String, String>,
    /// Items declared terminal, with their node.
    pub terminals: Vec<(String, String)>,
    /// Jobs whose outcome was decided by a wall-clock limit (listed, never compared).
    pub time_decided: Vec<String>,
}

pub fn final_result(st: &CampaignState) -> FinalResult {
    let mut r = FinalResult {
        accepted: BTreeMap::new(),
        dispositions: BTreeMap::new(),
        terminals: Vec::new(),
        time_decided: Vec::new(),
    };
    for (id, it) in &st.items {
        if let Some(a) = &it.accepted
            && matches!(it.phase, ItemPhase::Accepted(_))
        {
            r.accepted
                .insert(id.clone(), (a.rule.clone(), a.record.blake3.clone()));
        }
        let superseded = matches!(it.phase, ItemPhase::Escalated(_))
            || (it.phase == ItemPhase::Stale && it.stale_from.as_deref() == Some("escalated"));
        if !superseded {
            r.dispositions
                .insert(id.clone(), it.phase.tag().to_string());
        }
        if it.phase == ItemPhase::Terminal {
            r.terminals.push((it.node.clone(), id.clone()));
        }
    }
    for (id, j) in &st.jobs {
        if j.time_decided {
            r.time_decided.push(id.clone());
        }
    }
    r
}

/// Read-only: reduce the ledger of `cdir` (no lock) and derive the result.
pub fn read_final_result(cdir: &std::path::Path) -> Result<(FinalResult, CampaignState), String> {
    let recs = crate::ledger::read_records(cdir).map_err(|e| e.to_string())?;
    let st = crate::state::reduce(recs.iter()).map_err(|e| e.to_string())?;
    Ok((final_result(&st), st))
}
