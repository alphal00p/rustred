//! Exact-support, lazily represented Ore consequences.
//!
//! This private layer is deliberately narrower than the proposal-only
//! modular scout.  It may retain exact support authority, but it exposes no
//! coefficient inverse/division constructor and no artifact boundary.  ELC1
//! contains authenticated exact ingress, batched support classification, one
//! frozen-epoch cancellation, and cold sealed-source replay; it still grants
//! no basis-insertion or completion authority.

mod arena;
mod cancellation;
mod classify;
mod epoch;
mod error;
mod frozen;
mod guards;
mod import;
mod ledger;
mod limits;
mod lowering;
mod model;
mod normalization;
mod owner;
mod provenance;
mod schedule;
mod support;
mod work;

pub(self) use arena::{ExactLazySession, ExactLazyTransaction, LazyCoeff};
pub(self) use cancellation::{
    ExactLazyCancellationOutcome, ExactLazyFullJanetNormalForm, ExactLazyJanetCursor,
    ExactLazyReductionStep, ExactLazySelfExcludedJanetNormalForm,
    try_exact_lazy_full_janet_normal_form, try_exact_lazy_self_excluded_janet_normal_form,
};
pub(self) use classify::try_classify_support;
pub(self) use epoch::{ExactLazyJanetDivisionEpoch, ExactLazyJanetElement, ExactLazyJanetEpoch};
pub(self) use error::ExactLazyError;
pub(self) use frozen::ExactLazyFrozenJanetEpoch;
pub(self) use guards::{GuardLineageRef, GuardProbeRequirement};
pub(self) use import::try_import_exact_consequence;
pub(self) use ledger::{ExactLazyCompletionLedger, ExactLazyCompletionLedgerId};
pub(self) use limits::{ExactLazyCensus, ExactLazyLimits, ExactLazySupportLimits};
pub(self) use lowering::{
    AuthenticatedLoweredConsequence, ExactLazyLoweringBudget, ExactLazyLoweringCensus,
    ExactLazyLoweringLimits, try_lower_for_exact_replay,
};
pub(self) use model::{
    ExactLazyConsequence, ExactLazyPayloadCensus, ImportedGuardLineage, ImportedSourceDerivation,
    ImportedSourceTerm,
};
#[cfg(test)]
pub(self) use normalization::try_normalize_monic_test_local;
pub(self) use normalization::{
    try_normalize_full_normal_form_monic, try_normalize_self_excluded_normal_form_monic,
};
pub(self) use owner::ExactLazyOwner;
pub(self) use provenance::SourceDerivationRef;
pub(self) use schedule::{ExactLazyProbeSchedule, ExactLazyProbeSpec};
pub(self) use support::{
    ClassifiedLazyOreRow, ExactFallbackNonzeroProof, ExactFallbackZeroAuthority,
    ExactFallbackZeroProof, ExactIngressNonzero, ExactNonzeroProof, ExactZeroProof, LazyOreTerm,
    ModularNonzeroProof, PendingLazyOreTerm, StructuralZeroProof, UnclassifiedLazyOreRow,
};
pub(self) use work::{AccountedProbeOutcome, ExactLazySupportBudget, ExactLazySupportCensus};

#[cfg(test)]
mod cancellation_tests;
#[cfg(test)]
mod classification_tests;
#[cfg(test)]
mod epoch_tests;
#[cfg(test)]
mod frozen_tests;
#[cfg(test)]
mod lowering_tests;
#[cfg(test)]
mod normal_form_tests;
#[cfg(test)]
mod normalization_tests;
#[cfg(test)]
mod tests;
