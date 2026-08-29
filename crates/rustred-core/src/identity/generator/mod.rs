//! Fully parametric integration-by-parts and Lorentz-invariance identities.
//!
//! The implementation follows LiteRed's `GenerateIBP` convention. It emits
//! reusable relations in symbolic integral indices, never concrete seed
//! equations, and applies no sector, symmetry, or zero-sector rewriting.

mod config;
mod construction;
mod counts;
mod domain;
mod error;
mod lorentz;
mod model;
mod ordinary;
mod scope;
mod source;

pub use config::ParametricIbpConfig;
pub use error::ParametricIbpError;
pub use model::{
    CompletedIbpSourceRows, IbpSourceRow, ParametricIbpGenerator, PreparedIbpSourceBatch,
    PreparedLorentzInvarianceBatch,
};

#[cfg(test)]
mod tests;
