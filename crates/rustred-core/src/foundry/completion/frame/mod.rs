//! Deterministic physical translated-source frames for the bounded Gate 0
//! experiment.
//!
//! [`PhysicalFramePlan`] is a sealed construction-neutral owner of exact
//! translated sources, raw physical columns, CSR sparsity, and signed source
//! provenance. [`OneSidedChartFrame`] retains rectangular chart metadata,
//! while [`SelectedSourceFrame`] consumes only explicitly translated source
//! pairs. The bounded [`modular`] child can sample and probe either physical
//! pattern for discovery; no layer here performs symmetry quotienting or
//! closure inference.

pub(crate) mod admission;
mod assemble;
mod build;
mod error;
pub(crate) mod evidence;
pub(crate) mod exact;
mod limits;
mod model;
pub(crate) mod modular;
mod selected;

pub(crate) use error::PhysicalFrameError;
pub(crate) use limits::PhysicalFrameLimits;
pub(crate) use model::{
    OneSidedChartFrame, PhysicalFramePlan, PhysicalFramePlanIdentity, SelectedSourceFrame,
    SourceInstanceId,
};
// Structural replay comparison only. These functions confer no admission,
// ownership, terminal, or closure authority.
#[allow(unused_imports)] // Consumed by the staged canonical-replay boundary.
pub(crate) use admission::{compare_exact_circuit_content, exact_circuit_content_equal};

#[cfg(test)]
mod tests;
