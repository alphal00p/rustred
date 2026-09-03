//! Bounded, proposal-only finite-field coefficient guidance.
//!
//! The arena retains a field-independent expression DAG and applies Ore
//! translations lazily.  Each [`ModularProbe`] owns an independent field,
//! point, accumulated-translation arena, and evaluation cache.  Its images
//! are scheduling evidence only: this module deliberately exposes no rule,
//! queue-discharge, exact-zero, or artifact-publication boundary.

mod arena;
mod certificate;
mod error;
mod exact_lazy;
mod limits;
mod materialize;
mod model;
mod normal_form;
mod ore;
mod payload;
mod postorder;
mod probe;
mod work;

use arena::ModularCoefficientDag;
use certificate::{
    CertifiedNonzero, CertifiedSupportBatch, NonzeroCertification, SampledZeroUnresolved,
    try_certify_batch, try_issue_support_certificates,
};
use error::ModularGuideError;
use limits::{ExactMaterializerLimits, ModularGuideLimits};
use materialize::{
    ExactMaterialization, ExactMaterializationBatch, ExactMaterializationBudget,
    ExactMaterializationCensus, try_materialize_exact, try_materialize_exact_batch,
};
use model::{
    CoeffRef, ModularImage, ModularProbeCensus, ModularProbeIdentity, ModularQueryRole,
    ModularZeroEvidence,
};
use normal_form::{
    ModularFrozenNormalFormProblem, ModularNonzeroEvidence, ModularNormalFormProposal,
    ModularNormalFormTraceIdentity, ModularReductionTraceStep,
};
use probe::{ModularProbe, RejectedProbeReport};
use work::ModularNormalFormCensus;

#[cfg(test)]
mod elc0_tests;
#[cfg(test)]
mod elc1a_tests;
#[cfg(test)]
mod normal_form_tests;
#[cfg(test)]
mod tests;
