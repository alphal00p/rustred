//! Exact executor-safe programs for authenticated product factorizations.
//!
//! Programs are derived once, after the durable factorization recipe and all
//! lower artifacts have been authenticated. The hot reducer executes compact
//! affine and angular recurrences through its existing dependency reducers;
//! it never regenerates the program or materializes a rank-sized polynomial.

mod angular;
mod compile;
mod error;
mod limits;
mod model;
mod partial_angular;
mod resources;
mod runtime;

#[cfg(test)]
mod tests;

pub(crate) use compile::compile_factorized_product_moment_programs;
pub(crate) use model::{FactorizedProductMomentProgram, ProductApplicationDomain};
