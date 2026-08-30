//! Deterministic physical translated-source frames for the bounded Gate 0
//! experiment.
//!
//! This module owns only one-sided chart offsets, raw physical column keys,
//! CSR sparsity, and source-instance provenance. It performs no modular
//! arithmetic, target selection, symmetry quotienting, or closure inference.

mod build;
mod error;
mod limits;
mod model;

pub(crate) use error::PhysicalFrameError;
pub(crate) use limits::PhysicalFrameLimits;
pub(crate) use model::{PhysicalFramePlan, SourceInstanceId};

#[cfg(test)]
mod tests;
