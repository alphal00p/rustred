//! Targeted depth-wise completion over exact guarded index cases.
//!
//! This private foundation deliberately separates the geometry a completion
//! request names from the geometry the current executor can prove it handles.
//! Axis-aligned coordinate faces reuse an authenticated
//! [`crate::foundry::completion::stratum::DecoratedStratum`]. Coupled affine
//! lattices have a typed request boundary but fail closed until integer-lattice
//! parameterization, congruences, and exact replay all gain matching support.
//!
//! Source scheduling is likewise proposal-only. A complete signed-L1 depth
//! shell carries no closure or publication authority; later execution must
//! still pass through modular discovery, exact replay, and ordinary promotion.

mod bounded_case;
mod case;
mod case_worklist;
mod compact_lift;
mod discovery;
mod equality_step;
mod error;
mod evaluate;
mod fixed_point;
mod guard_cases;
mod inactive_activation;
mod materializer;
mod modular;
mod schedule;
mod serial_driver;
mod source_basis;
mod structure;
mod target_run;

pub(crate) use bounded_case::{
    SpiredBoundedCaseAxisDisposition, SpiredBoundedCaseAxisEnvelope, SpiredBoundedCaseEnvelope,
    SpiredBoundedCaseEnvelopeCensus, SpiredBoundedCaseEnvelopeError,
    SpiredBoundedCaseEnvelopeLimits, SpiredBoundedCaseEqualityFace, SpiredBoundedCaseShiftEnvelope,
    SpiredBoundedWorkingCase, try_build_spired_bounded_case_envelope,
};
pub(crate) use case::{
    SpiredAffineLatticeCase, SpiredCase, SpiredCoordinateFace, SpiredExecutionCase,
};
pub(crate) use case_worklist::{
    SpiredCoordinateCaseEnqueueOutcome, SpiredCoordinateCaseObligation,
    SpiredCoordinateCasePopOutcome, SpiredCoordinateCasePreparedEnqueueBatch,
    SpiredCoordinateCasePreparedReplacement, SpiredCoordinateCaseValidatedEnqueueBatch,
    SpiredCoordinateCaseValidatedReplacement, SpiredCoordinateCaseWorklist,
    SpiredCoordinateCaseWorklistCensus, SpiredCoordinateCaseWorklistError,
    SpiredCoordinateCaseWorklistLimits,
};
pub(crate) use compact_lift::{
    SpiredCompactLift, SpiredCompactLiftError, SpiredCompactLiftLimits, SpiredReplayedCompactLift,
    SpiredRuleCellAuthorityError, try_lift_spired_compact_support,
    try_lift_spired_rooted_compact_support, try_promote_spired_replayed_rule_cell,
};
pub(crate) use discovery::{
    SpiredPreparedStreamingDiscovery, SpiredStreamingDiscovery, SpiredStreamingError,
    SpiredStreamingLimits,
};
pub(crate) use equality_step::{
    SpiredEqualityStepCensus, SpiredEqualityStepCommitted, SpiredEqualityStepError,
    SpiredEqualityStepIncomplete, SpiredEqualityStepLimits, SpiredEqualityStepOutcome,
    SpiredEqualityStepProbeIncomplete, SpiredEqualityStepReport,
    try_advance_spired_coordinate_case,
};
pub(crate) use error::SpiredFoundationError;
pub(crate) use evaluate::{
    DirectShiftedSourceCorpusCensus, DirectShiftedSourceError, DirectShiftedSourceEvaluator,
    DirectShiftedSourceLimits, ShiftedModularResidueBuffer, ShiftedModularSourceBuffer,
    ShiftedModularTerm, ShiftedModularTerms, ValidatedDirectShiftedSources,
};
pub(crate) use guard_cases::{
    SpiredCoordinateGuardCaseCensus, SpiredCoordinateGuardCaseError,
    SpiredCoordinateGuardCaseIncomplete, SpiredCoordinateGuardCaseLimits,
    SpiredCoordinateGuardCaseOutcome, SpiredCoordinateGuardCaseRejection,
    SpiredCoordinateGuardCases, try_materialize_spired_coordinate_guard_cases,
};
#[allow(unused_imports)]
pub(crate) use inactive_activation::{
    SpiredConditionallyAllowedInactiveActivation, SpiredInactiveActivationAnalysis,
    SpiredInactiveActivationAnalysisCensus, SpiredInactiveActivationAnalysisLimits,
    SpiredInactiveActivationApplicationCell, SpiredInactiveActivationAxis,
    SpiredInactiveActivationError, SpiredInactiveActivationLimits, SpiredInactiveActivationSlice,
    SpiredReplayedInactiveActivationTerm, SpiredSurvivingInactiveActivationFace,
    try_analyze_replayed_inactive_activations, try_decompose_inactive_activation,
};
pub(crate) use materializer::{PrunedExactMaterializer, TargetRuleMaterializer};
pub(crate) use modular::{
    SpiredForbiddenTerm, SpiredModularError, SpiredModularHit, SpiredModularKernel,
    SpiredModularLimits, SpiredModularRow, SpiredModularStreamOutcome, SpiredPostHitCandidate,
    SpiredValidatedPrime,
};
pub(crate) use schedule::{
    SignedL1DepthShell, SignedL1RequestChunk, SignedL1ScheduleLimits, SignedL1ShellScheduler,
};
pub(crate) use serial_driver::{
    SpiredExistingTerminalAuthority, SpiredSerialCaseIncomplete, SpiredSerialCaseOutcome,
    SpiredSerialCaseReport, SpiredSerialDriverCensus, SpiredSerialDriverConfig,
    SpiredSerialDriverError, SpiredSerialDriverLimits, SpiredSerialDriverReport,
    SpiredSerialDriverStop, SpiredSerialFiniteResidualReason, SpiredSerialProbeBuildError,
    SpiredSerialProbeCensus, SpiredSerialProbePortfolio, SpiredSerialProbePortfolioError,
    SpiredSerialProbePortfolioLimits, SpiredSerialResumePolicy,
    try_drive_spired_serial_coordinate_cases, try_initialize_spired_serial_worklist,
};
#[allow(unused_imports)]
pub(crate) use source_basis::{
    SpiredRawSourceReconstructionTerm, SpiredSourceBasis, SpiredSourceBasisError,
    SpiredSourceBasisLimits, SpiredSourceBasisProvenanceTerm, SpiredSourceBasisRow,
    SpiredSourceBasisSpecializationPolicy, SpiredSourceBasisTerm,
    try_precondition_spired_ordinary_sources,
};
pub(crate) use structure::{
    SpiredPreparedRequestChunk, SpiredPreparedRowPlan, SpiredPreparedRowSpan,
    SpiredPreparedRowView, SpiredPreparedTermRole, SpiredStructuralPreparation,
    SpiredStructuralPreparationCensus, SpiredStructuralPreparationError,
    SpiredStructuralPreparationLimits, SpiredStructuralScopeIdentity,
};
pub(crate) use target_run::{
    SpiredAlternativeExhaustion, SpiredTargetRunCensus, SpiredTargetRunError,
    SpiredTargetRunErrorCause, SpiredTargetRunLimits, SpiredTargetRunOutcome,
    SpiredTargetRunReport, SpiredTargetRunStage, SpiredTargetRunWorkspace,
    try_run_spired_guarded_target, try_run_spired_target,
};

#[cfg(test)]
mod tests;
