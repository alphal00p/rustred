//! Deterministic physical translated-source frames for the bounded Gate 0
//! experiment.
//!
//! [`PhysicalFramePlan`] owns only one-sided chart offsets, raw physical
//! column keys, CSR sparsity, and source-instance provenance.  The bounded
//! [`modular`] child can sample and probe that physical pattern for discovery;
//! neither layer performs symmetry quotienting or closure inference.

mod build;
mod error;
pub(crate) mod evidence;
pub(crate) mod exact;
mod limits;
mod model;
pub(crate) mod modular;

pub(crate) use error::PhysicalFrameError;
pub(crate) use limits::PhysicalFrameLimits;
pub(crate) use model::{PhysicalFramePlan, SourceInstanceId};

#[cfg(test)]
mod tests;
