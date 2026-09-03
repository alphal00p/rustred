//! Proposal-only linear Janet/Ore completion primitives.
//!
//! This module owns no rule-admission or publication authority. Its exact
//! consequences remain proposals until the existing regenerated-source,
//! guard, strict-descent, immutable-predecessor, owner-cover, and artifact
//! boundaries replay and admit them independently.

mod blind;
mod chart_lift;
mod completion_loop;
#[cfg(test)]
pub(crate) mod diagnostics;
mod divisor_index;
mod error;
mod initial;
mod janet;
mod limits;
mod modular;
mod normal_form;
mod ordering;
mod ore;
mod shift;

pub(crate) use blind::{BlindDomainEntry, BlindDomainSchedule};
pub(crate) use chart_lift::{
    LiftedOrdinarySource, LiftedOrdinarySourceBatch, OrdinaryChartLiftError,
    OrdinaryChartLiftLimits, try_lift_completed_ordinary_sources,
};
pub(crate) use completion_loop::{
    JanetAutoreduction, JanetAutoreductionCensus, JanetCompletionCensus, JanetCompletionProposal,
    try_autoreduce_epoch, try_complete_janet_proposal,
    try_complete_janet_proposal_from_consequences,
};
pub(crate) use error::InvolutiveError;
pub(crate) use initial::{
    JanetInitialReduction, JanetInitialReductionCensus, try_preprocess_initial_basis,
};
pub(crate) use janet::{
    EpochId, JanetBasisElement, JanetBasisEpoch, JanetMultiplicativeMask, JanetProlongation,
    PurePowerCoverage,
};
pub(crate) use limits::{InvolutiveLimits, InvolutiveWorkCensus};
pub(crate) use normal_form::{JanetNormalForm, JanetReductionStep, try_janet_normal_form};
pub(crate) use ordering::{
    OreActionIdentity, OreLocalizationIdentity, OreOrderingAdapter, OreSourceModuleIdentity,
};
pub(crate) use ore::{
    CoefficientPayloadCensus, ConsequenceProvenance, LocalizationGuardCensus, LocalizationWitness,
    OreConsequence, OreProvenanceTerm, OreRow, OreTerm,
};
pub(crate) use shift::ForwardShift;

#[cfg(test)]
mod tests;
