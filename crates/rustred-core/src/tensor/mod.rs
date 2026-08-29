//! Typed tensor projection and family-aware scalar-product lowering.
//!
//! The first production grammar is intentionally small: a numerator is a
//! Symbolica sum of explicit products whose reserved heads occur only as
//! indexed loop/external vectors, metrics, or scalar products.  Every other
//! factor is retained as an opaque scalar [`symbolica::atom::Atom`] only when
//! it cannot hide a configured nonnumeric loop-momentum label.  The
//! single-scale vacuum lane currently supports scalar terms, exact odd-rank
//! zero, and the global rank-two isotropic projector.  Higher even rank and
//! generic external kinematics are typed unsupported boundaries.

mod error;
mod heads;
mod lowering;
mod model;
mod projection;
mod service;
mod syntax;
mod term;

#[cfg(test)]
mod tests;

pub use error::{MomentumKind, TensorError, TensorHeadError, TensorHeadKind, TensorHeadViolation};
pub use heads::TensorHeads;
pub use model::{
    ProjectedTensorTerm, ResolvedTensorLane, ScalarProductLowering, TensorGuard, TensorGuardOrigin,
    TensorLane, TensorLimits, TensorMomenta, TensorProjection, TensorReduction,
    TensorReductionTerm,
};
pub use service::TensorService;
