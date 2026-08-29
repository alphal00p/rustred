//! Generic, topology-neutral Symanzik-polynomial construction.
//!
//! The family-owned implementation builds authenticated `U`, `F`, and
//! `G = U + F` polynomials with Symbolica-backed coefficients.

mod construction;
mod context;
mod error;
mod model;
mod operations;
mod work;

#[cfg(test)]
mod tests;

pub use construction::SymanzikPolynomials;
pub use context::FeynmanPolynomialContext;
pub use error::FeynmanPolynomialError;
pub use model::{FeynmanPolynomial, FeynmanPolynomialLimits};
