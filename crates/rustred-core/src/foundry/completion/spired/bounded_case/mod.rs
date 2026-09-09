//! Finite-depth, target-relative coordinate envelopes for SpIReD discovery.
//!
//! This module only partitions where a bounded signed-L1 source search may
//! run without activating a currently inactive line.  Its output is proposal
//! geometry: it cannot certify a rule, a master, an owner, or family closure.

mod build;
mod error;
mod limits;
mod model;

pub(crate) use build::try_build_spired_bounded_case_envelope;
pub(crate) use error::SpiredBoundedCaseEnvelopeError;
pub(crate) use limits::SpiredBoundedCaseEnvelopeLimits;
pub(crate) use model::{
    SpiredBoundedCaseAxisDisposition, SpiredBoundedCaseAxisEnvelope, SpiredBoundedCaseEnvelope,
    SpiredBoundedCaseEnvelopeCensus, SpiredBoundedCaseEqualityFace, SpiredBoundedCaseShiftEnvelope,
    SpiredBoundedWorkingCase,
};

#[cfg(test)]
mod tests;
