//! Cold product moments at an authenticated `K_1^N` factorization.
//!
//! This schema-free prototype composes the exact transformed-denominator map
//! produced by [`super::factorized_numerator_lift`] with a bounded isotropic
//! angular incidence DP and immutable feedback from the installed closed
//! one-loop dependency reducer. It is deliberately test-only. It
//! owns no artifact region, rule cell, reducer dispatch, or closure claim.

mod angular;
mod compile;
mod error;
mod evaluate;
mod limits;
mod model;
mod radial;
mod resources;

pub(crate) use compile::compile_factorized_product_moment_chart;
pub(crate) use error::FactorizedProductMomentError;
pub(crate) use limits::FactorizedProductMomentLimits;
pub(crate) use model::{
    FactorizedProductMomentChart, ProductMomentExpansion, ProductMomentMonomial,
};

#[cfg(test)]
mod tests;
