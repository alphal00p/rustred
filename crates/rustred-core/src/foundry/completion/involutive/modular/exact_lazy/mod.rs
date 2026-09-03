//! Exact-support, lazily represented Ore consequences.
//!
//! This private layer is deliberately narrower than the proposal-only
//! modular scout.  It may retain exact support authority, but it exposes no
//! coefficient inverse/division constructor and no artifact boundary.  ELC1
//! initially supports authenticated exact ingress; cancellation, batched
//! support classification, and cold lowering are added at later boundaries.

mod arena;
mod error;
mod import;
mod limits;
mod model;
mod owner;
mod support;

pub(self) use arena::{ExactLazySession, ExactLazyTransaction, LazyCoeff};
pub(self) use error::ExactLazyError;
pub(self) use import::try_import_exact_consequence;
pub(self) use limits::{ExactLazyCensus, ExactLazyLimits};
pub(self) use model::{
    ExactGuardDescriptor, ExactLazyConsequence, ExactLazyPayloadCensus, ImportedGuardLineage,
    ImportedSourceDerivation, ImportedSourceTerm,
};
pub(self) use owner::ExactLazyOwner;
pub(self) use support::{
    ClassifiedLazyOreRow, ExactIngressNonzero, ExactNonzeroProof, ExactZeroProof, LazyOreTerm,
    PendingLazyOreTerm, StructuralZeroProof, UnclassifiedLazyOreRow,
};

#[cfg(test)]
mod tests;
