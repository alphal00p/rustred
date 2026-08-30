//! Loop-count-independent integral-family algebra.
//!
//! The family layer authenticates a complete affine denominator basis and
//! caches the exact contractions from which parametric IBPs are generated.

mod build;
mod error;
mod exact;
mod fingerprint;
mod integral;
pub mod isp;
mod kinematics;
mod model;
pub mod presentation;
pub mod symanzik;

#[cfg(test)]
mod tests;

pub use error::IntegralFamilyError;
pub(crate) use exact::{congruence_symbolic_matrix, invert_symbolic_matrix};
pub use integral::{IntegralKey, IntegralKeyError};
pub use model::{
    AffineDenominator, CoefficientLocation, ContractionMomentum, DenominatorExpansion,
    FamilyDomain, FamilyNonZeroCondition, IntegralFamily, IntegralFamilyLimits,
    ScalarProductCoordinate,
};
