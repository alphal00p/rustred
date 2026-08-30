//! Authenticated exact coefficient fields with appended integral indices.
//!
//! A family is defined over a base field `K = Q(theta)`. Indexed identity
//! coefficients live in the strictly extended field `K(n)`, whose index
//! variables are internal RustRed symbols appended after every base variable.
//! Symbolica can automatically unify variable maps; this module deliberately
//! rejects that behavior at the authenticated boundary.

mod base_coefficients;
mod context;
mod error;
mod limits;
mod scope;
mod specialization;
mod translation;
mod value;

#[cfg(test)]
pub(crate) use base_coefficients::BaseCoefficientSystem;
pub use base_coefficients::IndexedGuardLimits;
pub(crate) use base_coefficients::IntegerZeroSetResolution;
pub use context::IndexedCoefficientContext;
pub use error::IndexedAlgebraError;
pub use limits::{IndexedAlgebraLimits, IndexedContextLimits};
pub use value::{IndexedCoefficient, IndexedPolynomial};

#[cfg(test)]
mod tests;
