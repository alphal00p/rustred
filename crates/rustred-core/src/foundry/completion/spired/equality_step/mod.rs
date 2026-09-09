//! One transactional coordinate-equality completion step.
//!
//! This is the first production bridge between targeted SpIReD discovery and
//! the live equality-case/owner state. It deliberately handles only a case
//! whose complete coordinate domain is already safe for every streamed
//! source. Inactive-line bulk construction, terminal policy, campaign
//! iteration, and publication remain outside this module.
//!
//! The current worklist entry is inspected, never popped. Search, exact
//! replay, guard-zero materialization, singleton owner compilation, and both
//! mutation preflights complete against immutable authority before either
//! live object changes. Consequently every error and every incomplete
//! outcome is a literal no-op on the worklist and owner ledger.

mod error;
mod limits;
mod model;
mod run;

pub(crate) use error::SpiredEqualityStepError;
pub(crate) use limits::SpiredEqualityStepLimits;
pub(crate) use model::{
    SpiredEqualityStepCensus, SpiredEqualityStepCommitted, SpiredEqualityStepIncomplete,
    SpiredEqualityStepOutcome, SpiredEqualityStepProbeIncomplete, SpiredEqualityStepReport,
};
pub(crate) use run::try_advance_spired_coordinate_case;

#[cfg(test)]
mod tests;
