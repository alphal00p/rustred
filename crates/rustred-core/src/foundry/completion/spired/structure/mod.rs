//! Case-local structural preparation shared by modular probe portfolios.
//!
//! A single serial planner validates exact ordinary sources, classifies
//! translated structural shifts in scheduler chronology, and assigns compact
//! forbidden-column IDs. It emits immutable row plans. Probe-local discovery
//! owns only modular residues, evaluator scratch, and Symbolica reducers.

mod error;
mod limits;
mod model;
mod prepare;
mod registry;

pub(crate) use error::SpiredStructuralPreparationError;
pub(crate) use limits::SpiredStructuralPreparationLimits;
pub(crate) use model::{
    SpiredPreparedRequestChunk, SpiredPreparedRowPlan, SpiredPreparedRowSpan,
    SpiredPreparedRowView, SpiredPreparedTermRole, SpiredStructuralPreparationCensus,
    SpiredStructuralScopeIdentity,
};
pub(crate) use prepare::SpiredStructuralPreparation;

#[cfg(test)]
mod tests;
