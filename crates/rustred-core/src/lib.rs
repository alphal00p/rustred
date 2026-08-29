//! RustRed: pure-Rust, Symbolica-backed parametric IBP and LI derivation.
//!
//! The generic production path is loop-count and topology independent:
//! [`family::IntegralFamily`] authenticates a complete affine scalar-product basis and
//! [`identity::ParametricIbpGenerator`] derives reusable ordinary and
//! Lorentz-invariance identities over the exact field `K(n)`. Loop/topology-
//! authored recurrences are not part of the generic production crate and are
//! not sources of generic parametric identities or future discovered rules.

pub mod algebra;
pub mod campaign;
pub mod family;
pub mod foundry;
pub mod identity;
pub mod input;
pub mod reduction;
pub mod sector;
pub mod tensor;
