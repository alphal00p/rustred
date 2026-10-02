//! Bounded polynomial tangent vectors and their original ordinary-IBP products.
//!
//! This is a two-protected-denominator/three-contraction capability, independent
//! of loop count and topology. It neither selects a target nor admits a rule.
//! Unprotected denominator derivatives and all original conditions survive.

mod construction;
mod error;
mod materialize;
mod model;
mod polynomial;

pub use error::TangentSourceError;
pub use model::{
    TangentConditionOrigin, TangentPolynomial, TangentSourceCombination, TangentSourceCondition,
    TangentSourceLimits, TangentSourcePlan, TangentSourceSpec, WeightedTangentSource,
};

#[cfg(test)]
mod tests;
