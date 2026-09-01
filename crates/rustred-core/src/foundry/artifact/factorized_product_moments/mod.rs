//! Cold product moments at an authenticated closed-block factorization.
//!
//! This schema-free prototype composes the exact transformed-denominator map
//! produced by [`super::factorized_numerator_lift`] with a bounded isotropic
//! angular incidence DP and immutable feedback from installed closed
//! dependency reducers. The admitted lanes are all-`K_1` products and exactly
//! one correlated scalar block accompanied by independent `K_1` blocks. It is
//! deliberately test-only. It
//! owns no artifact region, rule cell, reducer dispatch, or closure claim.

mod angular;
mod compile;
mod correlated;
mod error;
mod evaluate;
mod limits;
mod model;
mod partial_angular;
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
