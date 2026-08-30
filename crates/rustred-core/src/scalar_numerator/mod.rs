//! Exact scalar-numerator lowering for sealed common-mass vacuum artifacts.
//!
//! This module starts after tensor projection. It treats explicit loop-loop
//! scalar products as polynomial indeterminates with Symbolica, expands them
//! through the artifact's authenticated affine denominator basis, and emits
//! shifted integral keys. It does not project Lorentz tensors.

mod error;
mod lowering;
mod model;
mod service;
mod syntax;

pub use error::{ScalarNumeratorError, ScalarProductHeadViolation};
pub use model::{LoweredScalarNumeratorTerm, ScalarNumeratorLimits, ScalarNumeratorLowering};
pub use service::ScalarNumeratorService;

#[cfg(test)]
mod tests;
