//! Exact sector-local ordinary-source preconditioning.
//!
//! Gregor's SpIReD strategy starts its translated search with a sparse RREF
//! basis ordered for the current sector. This module builds that basis from
//! the complete ordinary corpus without case-bound or guard pruning, clears
//! and primitive-normalizes every row denominator without restoring a unit
//! pivot, and retains both directions of the exact generic row-span
//! transform.
//!
//! Fraction-field preconditioning is deliberately not closure authority.  A
//! transformation can lose rank on a guard-zero fibre, so every compiled
//! basis requires the fair raw ordinary-source stream as a fallback.

mod compile;
mod error;
mod limits;
mod model;
mod replay;

#[cfg(test)]
mod tests;

pub(crate) use compile::try_precondition_spired_ordinary_sources;
pub(crate) use error::SpiredSourceBasisError;
pub(crate) use limits::SpiredSourceBasisLimits;
pub(crate) use model::{
    SpiredRawSourceReconstructionTerm, SpiredSourceBasis, SpiredSourceBasisProvenanceTerm,
    SpiredSourceBasisRow, SpiredSourceBasisSpecializationPolicy, SpiredSourceBasisTerm,
};
