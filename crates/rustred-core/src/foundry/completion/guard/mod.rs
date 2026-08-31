//! Semantic generic-parameter guard atoms for completion.
//!
//! An already Ore-pulled guard is unusable after fixing the integral indices
//! precisely when every coefficient in its expansion over the generic base
//! parameters vanishes. This module retains that simultaneous coefficient
//! ideal as one atom. It performs no radical, Gröbner, or integer-locus
//! inference and therefore supplies no closure authority by itself. The base
//! context declares algebraically independent parameters; callers must impose
//! physical parameter quotients and exact mass specializations first.

mod build;
pub(crate) mod decision;
mod error;
mod limits;
mod model;

pub(crate) use error::CoefficientIdealGuardError;
pub(crate) use limits::CoefficientIdealGuardLimits;
pub(crate) use model::CoefficientIdealGuardAtom;

#[cfg(test)]
mod tests;
