//! Conservative finite-shift refinement. A failed proof is not a counterexample.
use super::project::{self, Result};
use rustred::{foundry::artifact::SourcePortAuditError, identity::IndexShift};
use std::collections::BTreeSet;

pub enum Action {
    Added(IndexShift),
    Stop,
}
pub fn refine(
    error: &SourcePortAuditError,
    target: &IndexShift,
    universe: &BTreeSet<IndexShift>,
    forbidden: &mut BTreeSet<IndexShift>,
    additions: usize,
    max_additions: usize,
) -> Result<Action> {
    let SourcePortAuditError::UnprovedDescentObligation { shift, .. } = error else {
        return Ok(Action::Stop);
    };
    project::bound(
        additions
            .checked_add(1)
            .ok_or(project::Error::Budget("refinement count overflow"))?,
        max_additions,
        "forbidden-shift refinements",
    )?;
    let shift = universe
        .iter()
        .find(|s| s.values() == shift.as_slice())
        .cloned()
        .ok_or_else(|| {
            project::Error::Invalid("failed obligation outside frozen full image universe".into())
        })?;
    project::require(&shift != target, "failed obligation names target")?;
    project::require(
        universe.contains(&shift),
        "failed obligation outside frozen full image universe",
    )?;
    project::require(
        forbidden.insert(shift.clone()),
        "failed obligation gives no new forbidden shift",
    )?;
    Ok(Action::Added(shift))
}
