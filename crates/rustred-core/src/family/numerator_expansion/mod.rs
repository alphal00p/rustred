//! Cold exact expansion of products of affine denominator numerators.
//!
//! This topology-neutral boundary materializes
//! `prod_a (c_a + sum_i c_ai D_i)^n_a` on one parent integral key.  Symbolica
//! owns sparse-polynomial powers, multiplication, collision coalescing, and
//! cancellation. RustRed only authenticates the coefficient context, admits
//! caller-bounded native work, and routes monomial exponents to typed keys.
//! The native expansion accepts parameter-independent rational affine
//! coefficients only. A separate opt-in lane admits parameter-dependent
//! affine CONSTANTS (for example a deformed mass `-1 - eta` in a transported
//! numerator): it expands them as virtual native-Q axes under the same
//! prospective budgets and then substitutes them exactly in the coefficient
//! context. Parameter-dependent linear coefficients remain unsupported.
//!
//! An expansion is a structural identity only. It has no rule-cell, artifact,
//! closure, or reducer-dispatch semantics.

mod error;
mod expand;
mod limits;
mod model;

pub use error::MultiAffineNumeratorExpansionError;
pub(crate) use expand::preflight_coefficient_clones;
#[cfg(test)]
pub(crate) use expand::try_expand_multi_affine_numerator;
pub(crate) use expand::{AdmittedSupport, try_expand_multi_affine_support_with_usage};
pub(crate) use expand::{
    ExpansionUsage, try_expand_multi_affine_numerator_with_usage,
    try_expand_parametric_multi_affine_numerator_with_usage,
};
pub use limits::MultiAffineNumeratorExpansionLimits;
pub use model::MultiAffineNumeratorEndpoint;
pub(crate) use model::MultiAffineNumeratorFactor;

#[cfg(test)]
mod parametric_tests;
#[cfg(test)]
mod tests;
