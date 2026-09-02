//! Cold exact expansion of products of affine denominator numerators.
//!
//! This topology-neutral boundary materializes
//! `prod_a (c_a + sum_i c_ai D_i)^n_a` on one parent integral key.  Symbolica
//! owns sparse-polynomial powers, multiplication, collision coalescing, and
//! cancellation. RustRed only authenticates the coefficient context, admits
//! caller-bounded native work, and routes monomial exponents to typed keys.
//! This first bounded seam accepts parameter-independent rational affine
//! coefficients only. Supporting parameter-dependent outer coefficients
//! requires a separate prospective Symbolica coefficient-work proof.
//!
//! An expansion is a structural identity only. It has no rule-cell, artifact,
//! closure, or reducer-dispatch semantics.

mod error;
mod expand;
mod limits;
mod model;

pub(crate) use error::MultiAffineNumeratorExpansionError;
pub(crate) use expand::try_expand_multi_affine_numerator;
pub(crate) use limits::MultiAffineNumeratorExpansionLimits;
pub(crate) use model::{MultiAffineNumeratorEndpoint, MultiAffineNumeratorFactor};

#[cfg(test)]
mod tests;
