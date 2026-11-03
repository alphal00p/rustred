//! Exact scalar-numerator lowering for unit-scale families/artifacts: the
//! common-mass vacuum, and (family lane only) the propagator-type family with
//! one external momentum of unit square.
//!
//! This module starts after tensor projection. It treats explicit loop-loop
//! and loop-external scalar products as polynomial indeterminates with
//! Symbolica, expands them
//! through the authenticated family's affine denominator basis, and emits
//! shifted integral keys. The family-bound lane proves no reduction coverage;
//! the artifact-bound lane additionally enforces its certified root domain.
//! Neither lane projects Lorentz tensors.

mod error;
mod lowering;
#[cfg(feature = "weighted-lowering-diagnostics")]
#[doc(hidden)]
pub mod lowering_diagnostics;
mod model;
mod service;
mod syntax;
mod weighted;
pub(crate) use weighted::bounds::Work as WeightedScalarWork;

pub use error::{ScalarNumeratorError, ScalarProductHeadViolation};
pub use model::{
    LoweredScalarNumeratorTerm, ScalarNumeratorLimits, ScalarNumeratorLowering,
    WeightedScalarNumeratorLowering,
};
pub use service::{FamilyScalarNumeratorService, ScalarNumeratorService};

#[cfg(test)]
mod tests;
