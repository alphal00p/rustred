//! RustRed: pure-Rust, Symbolica-backed parametric IBP and LI derivation.
//!
//! The generic production path is loop-count and topology independent:
//! [`family::IntegralFamily`] authenticates a complete affine scalar-product basis and
//! [`identity::ParametricIbpGenerator`] derives reusable ordinary and
//! Lorentz-invariance identities over the exact field `K(n)`. Loop/topology-
//! authored recurrences are not part of the generic production crate and are
//! not sources of generic parametric identities or future discovered rules.

pub mod algebra;
#[macro_use]
pub mod arity;
pub use arity::{
    campaign_storage_arity, compiled_runtime_arities, compiled_runtime_capacities, fits_storage,
    storage_array,
};
pub mod campaign;
mod diagnostic;
pub mod family;
pub mod foundry;
pub mod identity;
pub mod input;
pub mod persistence;
mod platform;
pub mod reduction;
pub mod scalar_numerator;
pub mod sector;
pub mod solver;
/// Runtime, Symbolica-independent integral-order descriptors and compiler.
pub use rustred_order as order;
pub mod tensor;
#[cfg(test)]
mod test_gates;
