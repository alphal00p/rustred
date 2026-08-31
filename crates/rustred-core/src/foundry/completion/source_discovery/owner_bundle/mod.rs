//! Owned pairing between exact semantic authority and executable rule cells.
//!
//! Canonical replay deliberately emits non-authoritative proposals.  This
//! boundary promotes every candidate, compiles the corresponding semantic
//! DAG, preserves circuit-to-cell identity through both canonical sorts, and
//! exposes only whole-cover transactions.  A failed rebuild leaves the
//! previously published in-memory cover untouched.

mod closed;
mod compile;
mod error;
mod layer;
mod limits;
mod model;

pub(crate) use closed::ClosedExactExecutableOwnerCover;
pub(crate) use compile::try_compile_canonical_executable_owner;
pub(crate) use error::ExactExecutableOwnerError;
pub(crate) use layer::{ClosedSectorLayer, ClosedSectorLayerContentId};
pub(crate) use limits::ExactExecutableOwnerLimits;
pub(crate) use model::{
    ExactExecutableCandidateObstruction, ExactExecutableOwnerCover,
    ExactExecutableOwnerObstruction, ExactExecutableOwnerProposal, ExactExecutableOwnerSelection,
    ExactSemanticExecutableOwner, UnpublishedCanonicalOwnerProposal,
};

#[cfg(test)]
mod tests;
