//! Authenticated exact coefficient fields with appended integral indices.
//!
//! A family is defined over a base field `K = Q(theta)`. Indexed identity
//! coefficients live in the strictly extended field `K(n)`, whose index
//! variables are internal RustRed symbols appended after every base variable.
//! Symbolica can automatically unify variable maps; this module deliberately
//! rejects that behavior at the authenticated boundary.

mod context;
mod error;
mod limits;
mod scope;
mod specialization;
mod translation;
mod value;

pub use context::IndexedCoefficientContext;
pub use error::IndexedAlgebraError;
pub use limits::IndexedAlgebraLimits;
pub use value::{BasePolynomial, IndexedCoefficient, IndexedPolynomial};

#[cfg(test)]
mod tests;
