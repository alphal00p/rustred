//! Loop-count-independent integral-family algebra.
//!
//! The family layer authenticates a complete affine denominator basis and
//! caches the exact contractions from which parametric IBPs are generated.

mod build;
mod error;
mod exact;
mod fingerprint;
pub mod isp;
mod kinematics;
mod model;
mod replay;
pub mod symanzik;

#[cfg(test)]
mod tests;

pub use error::IntegralFamilyError;
pub use model::{
    AffineDenominator, CoefficientLocation, ContractionMomentum, DenominatorExpansion,
    FamilyDomain, FamilyNonZeroCondition, IntegralFamily, IntegralFamilyFingerprintStats,
    IntegralFamilyLimits, ScalarProductCoordinate,
};

pub(crate) use model::BasePolynomial;
