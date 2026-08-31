//! Transactional bottom-up publication of one same-rank sector wave.
//!
//! This coordinator accepts only pointer-paired executable owners produced by
//! canonical replay. It never adapts legacy `RuleCell`s into publication
//! authority. Every declared sector is compiled against one strongly retained
//! predecessor snapshot; if any cover is incomplete, no cover is sealed and no
//! layer enters a successor snapshot.

mod coordinator;
mod error;
mod limits;
mod model;

pub(crate) use coordinator::StagedSectorClosureCoordinator;
pub(crate) use error::StagedSectorClosureError;
pub(crate) use limits::StagedSectorClosureLimits;
pub(crate) use model::{
    ClosedSectorClosureWave, StagedSectorClosureOutcome, StagedSectorClosureStop,
    StagedSectorClosureStopEvidence,
};

#[cfg(test)]
mod tests;
