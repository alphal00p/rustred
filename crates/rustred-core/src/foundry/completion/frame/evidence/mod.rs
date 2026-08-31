//! Deterministic multi-sample telemetry for one exact target partition.
//!
//! Discovery samples may nominate one modular support for exact lift and full
//! replay. Held-out samples remain probabilistic diagnostics. Trace agreement
//! is never combined across fields, is never a no-relation proof, and is never
//! completion authority. Probe identity is the modulus plus the canonical
//! finite-field sample point; roles and noncanonical integer representatives
//! cannot make one sample count twice.

mod error;
mod limits;
mod model;
mod schedule;

pub(crate) use error::TargetEvidenceError;
pub(crate) use limits::TargetEvidenceLimits;
#[allow(unused_imports)] // Consumed by the staged sector-layer orchestrator.
pub(crate) use model::ProbeRejectionStage;
pub(crate) use model::{
    CanonicalTraceIdentity, DiscoveryTraceGroup, EvidenceProbe, EvidenceProbeOutcome,
    EvidenceProbePlan, EvidenceProbeRole, EvidenceProbeSpec, ExactProposalOutcome,
    HeldOutAssessment, HeldOutDiagnostic, TargetEvidenceReport,
};
#[allow(unused_imports)] // Consumed by the staged sector-layer orchestrator.
pub(crate) use schedule::TargetEvidenceScheduler;

#[cfg(test)]
mod tests;
