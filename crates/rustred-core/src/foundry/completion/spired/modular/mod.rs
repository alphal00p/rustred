//! Incremental modular target-dependency discovery for SpIReD completion.

mod error;
mod kernel;
mod limits;
mod model;

pub(crate) use error::SpiredModularError;
pub(crate) use kernel::{SpiredModularKernel, SpiredValidatedPrime};
pub(crate) use limits::SpiredModularLimits;
pub(crate) use model::{
    SpiredDependencyTrace, SpiredDependencyTraceNode, SpiredForbiddenTerm, SpiredModularHit,
    SpiredModularRow, SpiredModularStreamOutcome, SpiredPostHitCandidate,
};
