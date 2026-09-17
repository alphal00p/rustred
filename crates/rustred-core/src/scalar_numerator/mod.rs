//! Exact scalar-numerator lowering for common-mass vacuum families/artifacts.
//!
//! This module starts after tensor projection. It treats explicit loop-loop
//! scalar products as polynomial indeterminates with Symbolica, expands them
//! through the authenticated family's affine denominator basis, and emits
//! shifted integral keys. The family-bound lane proves no reduction coverage;
//! the artifact-bound lane additionally enforces its certified root domain.
//! Neither lane projects Lorentz tensors.

mod error;
mod lowering;
mod model;
mod service;
mod syntax;

pub use error::{ScalarNumeratorError, ScalarProductHeadViolation};
pub use model::{LoweredScalarNumeratorTerm, ScalarNumeratorLimits, ScalarNumeratorLowering};
pub use service::{FamilyScalarNumeratorService, ScalarNumeratorService};

#[cfg(test)]
mod tests;
