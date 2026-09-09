//! Exact proposal geometry for coefficient-gated inactive-line activation.
//!
//! Structural decomposition records the finite boundary slices on which an
//! inactive line can activate.  The analysis bridge then uses Symbolica exact
//! numerator substitution on combined replay coefficients, returning
//! cell-local pruning plans and equality-face specifications without minting
//! a rule, worklist obligation, owner, or closure authority. Face
//! specifications deliberately carry no surrounding bounds, so a temporary
//! finite-depth search envelope cannot leak into logical case identity.

mod analyze;
mod build;
mod error;
mod limits;
mod model;

pub(crate) use analyze::try_analyze_replayed_inactive_activations;
pub(crate) use build::try_decompose_inactive_activation;
pub(crate) use error::SpiredInactiveActivationError;
pub(crate) use limits::{SpiredInactiveActivationAnalysisLimits, SpiredInactiveActivationLimits};
pub(crate) use model::{
    SpiredConditionallyAllowedInactiveActivation, SpiredInactiveActivationAnalysis,
    SpiredInactiveActivationAnalysisCensus, SpiredInactiveActivationApplicationCell,
    SpiredInactiveActivationAxis, SpiredInactiveActivationSlice,
    SpiredReplayedInactiveActivationTerm, SpiredSurvivingInactiveActivationFace,
};

#[cfg(test)]
mod tests;
