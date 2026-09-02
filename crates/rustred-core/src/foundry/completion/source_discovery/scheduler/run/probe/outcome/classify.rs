//! Recursive classification of typed nested resource failures.
//!
//! Every resource-bearing wrapper is traversed explicitly. The exhaustive
//! matches deliberately force review when an upstream error enum grows; no
//! display-string inspection participates in stop authority.

use crate::algebra::{ExactAlgebraError, IndexedAlgebraError};
use crate::foundry::completion::frame::PhysicalFrameError;
use crate::foundry::completion::frame::modular::{
    ModularKernelError, ModularSourceEvaluationError,
};
use crate::foundry::completion::stratum::StratumRegistryError;
use crate::identity::{IdentityConditionError, ParametricRelationError, TranslatedSourceError};
use crate::sector;

use super::super::super::super::super::{
    CampaignError, SampledDeclaredModuleDualError, SourceDiscoveryError,
};
use super::super::super::super::{ProbeLocalBudgetCause, ProbeLocalBudgetScope};

pub(super) fn campaign_budget_cause(error: &CampaignError) -> Option<ProbeLocalBudgetCause> {
    match error {
        CampaignError::ResourceCountOverflow { resource } => Some(count_overflow(resource)),
        CampaignError::AllocationFailure {
            resource,
            requested,
        } => Some(allocation_failure(resource, *requested)),
        CampaignError::BudgetExhausted(exhaustion) => {
            Some(ProbeLocalBudgetCause::Campaign(exhaustion.clone()))
        }
        CampaignError::TranslatedSources(error) => translated_source_budget_cause(error),
        CampaignError::PhysicalFrame(error) => physical_frame_budget_cause(error),
        CampaignError::Stratum(error) => stratum_budget_cause(error),
        CampaignError::Modular(error) => modular_kernel_budget_cause(error),
        CampaignError::EmptyRequestArity
        | CampaignError::EmptyAccumulatedRequests
        | CampaignError::WrongRequestArity { .. }
        | CampaignError::WrongTargetArity { .. }
        | CampaignError::WrongProbeChartArity { .. }
        | CampaignError::WrongSourceLayout { .. }
        | CampaignError::FixedTaskScopeMismatch { .. }
        | CampaignError::SourceChronologyMismatch
        | CampaignError::NonMonotoneGrowingRequests { .. }
        | CampaignError::TargetColumnAbsent
        | CampaignError::FixedStratumDoesNotCoverColumn { .. }
        | CampaignError::SampleCoordinateNotRepresentable { .. }
        | CampaignError::SampleOutsideFixedStratum { .. }
        | CampaignError::Invariant { .. } => None,
    }
}

pub(super) fn source_budget_cause(error: &SourceDiscoveryError) -> Option<ProbeLocalBudgetCause> {
    match error {
        SourceDiscoveryError::ResourceLimit {
            resource,
            requested,
            limit,
        } => Some(resource_limit(resource, *requested, *limit)),
        SourceDiscoveryError::ResourceCountOverflow { resource } => Some(count_overflow(resource)),
        SourceDiscoveryError::AllocationFailure {
            resource,
            requested,
        } => Some(allocation_failure(resource, *requested)),
        SourceDiscoveryError::ShiftConstruction(error)
        | SourceDiscoveryError::SourceTranslation(error) => translated_source_budget_cause(error),
        SourceDiscoveryError::CandidateEvaluation { error, .. } => {
            modular_source_evaluation_budget_cause(error)
        }
        SourceDiscoveryError::ProposalClassification(error) => stratum_budget_cause(error),
        SourceDiscoveryError::WrongSourceLayout { .. }
        | SourceDiscoveryError::ScopeMismatch { .. }
        | SourceDiscoveryError::WrongArity { .. }
        | SourceDiscoveryError::ShiftOverflow { .. }
        | SourceDiscoveryError::NominationIncidenceMismatch
        | SourceDiscoveryError::TargetUnitNominationForObstruction
        | SourceDiscoveryError::NominationObstructionMismatch
        | SourceDiscoveryError::CompletedSourceChronologyMismatch
        | SourceDiscoveryError::SelectedRequestProvenanceMismatch { .. }
        | SourceDiscoveryError::SelectedSourceRowMismatch { .. }
        | SourceDiscoveryError::ObstructionPlanMismatch
        | SourceDiscoveryError::ObstructionSampleMismatch
        | SourceDiscoveryError::ProposalPartitionMismatch
        | SourceDiscoveryError::Invariant { .. } => None,
    }
}

pub(super) fn sampled_dual_budget_cause(
    error: &SampledDeclaredModuleDualError,
) -> Option<ProbeLocalBudgetCause> {
    match error {
        SampledDeclaredModuleDualError::IncidenceVerification(error)
        | SampledDeclaredModuleDualError::NominationVerification(error)
        | SampledDeclaredModuleDualError::Retention(error) => source_budget_cause(error),
        SampledDeclaredModuleDualError::PartitionVerification(error) => stratum_budget_cause(error),
        SampledDeclaredModuleDualError::IncidenceTaskScopeMismatch { .. }
        | SampledDeclaredModuleDualError::GuardedStratumRequiresSampleWitness { .. }
        | SampledDeclaredModuleDualError::QueryIsModularHit
        | SampledDeclaredModuleDualError::PartitionNotVerified
        | SampledDeclaredModuleDualError::PartitionPlanMismatch
        | SampledDeclaredModuleDualError::SamplePlanMismatch
        | SampledDeclaredModuleDualError::ObstructionPlanMismatch
        | SampledDeclaredModuleDualError::ObstructionSampleMismatch
        | SampledDeclaredModuleDualError::ObstructionPartitionMismatch
        | SampledDeclaredModuleDualError::TargetColumnOutOfRange
        | SampledDeclaredModuleDualError::TargetColumnMismatch
        | SampledDeclaredModuleDualError::TargetShiftMismatch
        | SampledDeclaredModuleDualError::FixedStratumMismatch
        | SampledDeclaredModuleDualError::FixedOrderingMismatch
        | SampledDeclaredModuleDualError::FixedOwnerSnapshotMismatch
        | SampledDeclaredModuleDualError::MaterializedSourceChronologyMismatch
        | SampledDeclaredModuleDualError::NominationIncidenceMismatch
        | SampledDeclaredModuleDualError::NominationIsTargetUnit
        | SampledDeclaredModuleDualError::NominationObstructionMismatch
        | SampledDeclaredModuleDualError::ResidualNominationMismatch
        | SampledDeclaredModuleDualError::ResidualIncidenceMismatch
        | SampledDeclaredModuleDualError::ResidualPlanMismatch
        | SampledDeclaredModuleDualError::ResidualObstructionMismatch
        | SampledDeclaredModuleDualError::ResidualSampleMismatch
        | SampledDeclaredModuleDualError::IncompleteNominationCensus
        | SampledDeclaredModuleDualError::ResidualTelemetryMismatch
        | SampledDeclaredModuleDualError::ResidualPairingShiftOverflow { .. }
        | SampledDeclaredModuleDualError::CuttingResiduals { .. }
        | SampledDeclaredModuleDualError::RawObstructionMismatch
        | SampledDeclaredModuleDualError::RankDiagnosticsMismatch { .. } => None,
    }
}

fn translated_source_budget_cause(error: &TranslatedSourceError) -> Option<ProbeLocalBudgetCause> {
    match error {
        TranslatedSourceError::ResourceCountOverflow { resource } => Some(count_overflow(resource)),
        TranslatedSourceError::ResourceLimit {
            resource,
            requested,
            limit,
        } => Some(resource_limit(resource, *requested, *limit)),
        TranslatedSourceError::AllocationFailure {
            resource,
            requested,
        } => Some(allocation_failure(resource, *requested)),
        TranslatedSourceError::RelationTranslation { error, .. }
        | TranslatedSourceError::RequestTranslation { error, .. } => {
            parametric_relation_budget_cause(error)
        }
        TranslatedSourceError::EmptyIntegralShift
        | TranslatedSourceError::EmptySourceRows
        | TranslatedSourceError::EmptyOffsets
        | TranslatedSourceError::EmptySourceRequests
        | TranslatedSourceError::WrongOffsetArity { .. }
        | TranslatedSourceError::WrongRequestOffsetArity { .. }
        | TranslatedSourceError::SourceOrdinalOutOfRange { .. }
        | TranslatedSourceError::CompletedSourceFamilyMismatch
        | TranslatedSourceError::CompletedSourceContextMismatch => None,
    }
}

fn parametric_relation_budget_cause(
    error: &ParametricRelationError,
) -> Option<ProbeLocalBudgetCause> {
    match error {
        ParametricRelationError::ResourceCountOverflow { resource } => {
            Some(count_overflow(resource))
        }
        ParametricRelationError::AllocationFailure {
            resource,
            requested,
        } => Some(allocation_failure(resource, *requested)),
        ParametricRelationError::IdentityCondition(error) => identity_condition_budget_cause(error),
        ParametricRelationError::Coefficient(error) => indexed_algebra_budget_cause(error),
        ParametricRelationError::EmptyIndexSpace
        | ParametricRelationError::WrongArity { .. }
        | ParametricRelationError::IndexOutOfRange { .. }
        | ParametricRelationError::IndexOverflow { .. }
        | ParametricRelationError::WrongContext
        | ParametricRelationError::WrongFamily
        | ParametricRelationError::UnsatisfiableDomain => None,
    }
}

fn identity_condition_budget_cause(
    error: &IdentityConditionError,
) -> Option<ProbeLocalBudgetCause> {
    match error {
        IdentityConditionError::ResourceLimit {
            resource,
            requested,
            limit,
        } => Some(resource_limit(resource, *requested, *limit)),
        IdentityConditionError::ResourceCountOverflow { resource } => {
            Some(count_overflow(resource))
        }
        IdentityConditionError::Coefficient(error) => indexed_algebra_budget_cause(error),
        IdentityConditionError::MissingSource => None,
    }
}

fn indexed_algebra_budget_cause(error: &IndexedAlgebraError) -> Option<ProbeLocalBudgetCause> {
    match error {
        IndexedAlgebraError::ResourceLimit {
            resource,
            requested,
            limit,
        } => Some(resource_limit(resource, *requested, *limit)),
        IndexedAlgebraError::ResourceCountOverflow { resource } => Some(count_overflow(resource)),
        IndexedAlgebraError::AllocationFailure {
            resource,
            requested,
        } => Some(allocation_failure(resource, *requested)),
        IndexedAlgebraError::ExactAlgebra(error) => exact_algebra_budget_cause(error),
        IndexedAlgebraError::EmptyIndexSpace
        | IndexedAlgebraError::InvalidScope
        | IndexedAlgebraError::IndexSymbolRegistrationFailure { .. }
        | IndexedAlgebraError::IndexSymbolCollision { .. }
        | IndexedAlgebraError::WrongContext
        | IndexedAlgebraError::WrongIndexArity { .. }
        | IndexedAlgebraError::FixedIndexOutOfRange { .. }
        | IndexedAlgebraError::DuplicateFixedIndex { .. }
        | IndexedAlgebraError::ZeroDenominator
        | IndexedAlgebraError::Symbolica(_) => None,
    }
}

fn exact_algebra_budget_cause(error: &ExactAlgebraError) -> Option<ProbeLocalBudgetCause> {
    const EXPONENT: &str = "exact algebra exponent";
    const EXPONENT_ARITHMETIC: &str = "exact algebra exponent arithmetic";

    match error {
        ExactAlgebraError::ResourceLimit {
            resource,
            requested,
            limit,
        } => Some(resource_limit(resource, *requested, *limit)),
        ExactAlgebraError::ResourceCountOverflow { resource } => Some(count_overflow(resource)),
        ExactAlgebraError::ExponentLimit {
            requested, limit, ..
        } => match usize::try_from(*requested) {
            Ok(requested) => Some(resource_limit(EXPONENT, requested, usize::from(*limit))),
            Err(_) => Some(count_overflow(EXPONENT)),
        },
        ExactAlgebraError::ExponentArithmeticOverflow { .. } => {
            Some(count_overflow(EXPONENT_ARITHMETIC))
        }
        ExactAlgebraError::VariableMapMismatch { .. }
        | ExactAlgebraError::MalformedExponentLayout { .. }
        | ExactAlgebraError::ZeroCoefficient { .. }
        | ExactAlgebraError::NonCanonicalMonomialOrder { .. }
        | ExactAlgebraError::ZeroDenominator
        | ExactAlgebraError::DivisionByZero => None,
    }
}

fn physical_frame_budget_cause(error: &PhysicalFrameError) -> Option<ProbeLocalBudgetCause> {
    match error {
        PhysicalFrameError::ResourceCountOverflow { resource }
        | PhysicalFrameError::U32NotRepresentable { resource, .. } => {
            Some(count_overflow(resource))
        }
        PhysicalFrameError::ResourceLimit {
            resource,
            requested,
            limit,
        } => Some(resource_limit(resource, *requested, *limit)),
        PhysicalFrameError::AllocationFailure {
            resource,
            requested,
        } => Some(allocation_failure(resource, *requested)),
        PhysicalFrameError::IntegralShift(error) | PhysicalFrameError::TranslatedSources(error) => {
            translated_source_budget_cause(error)
        }
        PhysicalFrameError::WrongSourceLayout { .. }
        | PhysicalFrameError::WrongSectorArity { .. }
        | PhysicalFrameError::WrongSourceOffsetArity { .. }
        | PhysicalFrameError::WrongSourceTermArity { .. }
        | PhysicalFrameError::DegreeNotRepresentable { .. }
        | PhysicalFrameError::ZeroSourceTerm { .. }
        | PhysicalFrameError::Invariant { .. } => None,
    }
}

fn stratum_budget_cause(error: &StratumRegistryError) -> Option<ProbeLocalBudgetCause> {
    match error {
        StratumRegistryError::ResourceCountOverflow { resource } => Some(count_overflow(resource)),
        StratumRegistryError::ResourceLimit {
            resource,
            requested,
            limit,
        } => Some(resource_limit(resource, *requested, *limit)),
        StratumRegistryError::AllocationFailure {
            resource,
            requested,
        } => Some(allocation_failure(resource, *requested)),
        StratumRegistryError::IndexedAlgebra(error) => indexed_algebra_budget_cause(error),
        StratumRegistryError::Sector(error) => sector_budget_cause(error),
        StratumRegistryError::EmptyIdentity { .. }
        | StratumRegistryError::DuplicateGuardPredicate { .. }
        | StratumRegistryError::ContradictoryGuardPredicate { .. }
        | StratumRegistryError::ZeroGuardPolynomial
        | StratumRegistryError::WrongFrameFamily
        | StratumRegistryError::WrongFrameContext
        | StratumRegistryError::WrongOwnerFamily
        | StratumRegistryError::WrongOwnerContext
        | StratumRegistryError::WrongOwnerRouteCanonicalizer
        | StratumRegistryError::WrongFrameSector
        | StratumRegistryError::WrongOwnerArity { .. }
        | StratumRegistryError::EmptyClosedSectorLayerBatch
        | StratumRegistryError::WrongClosedSectorLayerFamily { .. }
        | StratumRegistryError::WrongClosedSectorLayerContext { .. }
        | StratumRegistryError::WrongClosedSectorLayerPredecessor { .. }
        | StratumRegistryError::MixedClosedSectorLayerFrontier { .. }
        | StratumRegistryError::NonIncreasingClosedSectorLayerFrontier { .. }
        | StratumRegistryError::DuplicateClosedSectorOwner { .. }
        | StratumRegistryError::TargetColumnOutOfRange { .. }
        | StratumRegistryError::UncoveredPhysicalShift { .. }
        | StratumRegistryError::InitialMaximalDomainMismatch
        | StratumRegistryError::NonMonotoneMaximalDomain
        | StratumRegistryError::Invariant { .. } => None,
    }
}

fn sector_budget_cause(error: &sector::Error) -> Option<ProbeLocalBudgetCause> {
    match error {
        sector::Error::AllocationFailure {
            resource,
            requested,
        } => Some(allocation_failure(resource, *requested)),
        sector::Error::ComplexityOverflow { measure } => Some(count_overflow(measure)),
        sector::Error::EmptyIndexSpace
        | sector::Error::WrongArity { .. }
        | sector::Error::IndexOutOfRange { .. }
        | sector::Error::DuplicateIndex { .. }
        | sector::Error::InvalidInteriorBounds { .. }
        | sector::Error::InteriorOutsideSector { .. }
        | sector::Error::EmptyShiftInterior { .. }
        | sector::Error::ShiftNotCovered { .. }
        | sector::Error::PivotLeavesParentSector { .. }
        | sector::Error::InactiveLineActivation { .. }
        | sector::Error::TargetSectorCellOutOfRange { .. }
        | sector::Error::UnknownOrderingPolicy { .. }
        | sector::Error::OrderingPriorityArityLimit { .. }
        | sector::Error::NotStrictDescent => None,
    }
}

fn modular_kernel_budget_cause(error: &ModularKernelError) -> Option<ProbeLocalBudgetCause> {
    match error {
        ModularKernelError::ResourceCountOverflow { resource }
        | ModularKernelError::U32NotRepresentable { resource, .. } => {
            Some(count_overflow(resource))
        }
        ModularKernelError::ResourceLimit {
            resource,
            requested,
            limit,
        } => Some(resource_limit(resource, *requested, *limit)),
        ModularKernelError::AllocationFailure {
            resource,
            requested,
        } => Some(allocation_failure(resource, *requested)),
        ModularKernelError::WrongFrameContext
        | ModularKernelError::UnsupportedEvenModulus { .. }
        | ModularKernelError::NonPrimeModulus { .. }
        | ModularKernelError::WrongBaseParameterArity { .. }
        | ModularKernelError::WrongChartCoordinateArity { .. }
        | ModularKernelError::WrongContextIndexArity { .. }
        | ModularKernelError::WrongIndexedContext { .. }
        | ModularKernelError::CoefficientDenominatorZero { .. }
        | ModularKernelError::SourceConditionZero { .. }
        | ModularKernelError::TargetColumnOutOfRange { .. }
        | ModularKernelError::ForbiddenColumnOutOfRange { .. }
        | ModularKernelError::DuplicateForbiddenColumn { .. }
        | ModularKernelError::TargetIsForbidden { .. }
        | ModularKernelError::NativePanic { .. }
        | ModularKernelError::Invariant { .. } => None,
    }
}

fn modular_source_evaluation_budget_cause(
    error: &ModularSourceEvaluationError,
) -> Option<ProbeLocalBudgetCause> {
    const POINT_ARITY: &str = "source-discovery modular candidate point arity";
    const EVALUATED_TERMS: &str = "source-discovery modular candidate evaluated terms";

    match error {
        ModularSourceEvaluationError::PointArityOverflow => Some(count_overflow(POINT_ARITY)),
        ModularSourceEvaluationError::AllocationFailure { requested } => {
            Some(allocation_failure(EVALUATED_TERMS, *requested))
        }
        ModularSourceEvaluationError::FrameContextMismatch
        | ModularSourceEvaluationError::WrongPointArity { .. }
        | ModularSourceEvaluationError::ConditionContextMismatch { .. }
        | ModularSourceEvaluationError::ConditionZero { .. }
        | ModularSourceEvaluationError::TermContextMismatch { .. }
        | ModularSourceEvaluationError::TermDenominatorZero { .. } => None,
    }
}

fn resource_limit(resource: &'static str, requested: usize, limit: usize) -> ProbeLocalBudgetCause {
    ProbeLocalBudgetCause::SourceDiscovery {
        resource,
        requested,
        limit,
    }
}

const fn count_overflow(resource: &'static str) -> ProbeLocalBudgetCause {
    ProbeLocalBudgetCause::CountOverflow {
        scope: ProbeLocalBudgetScope::Probe,
        resource,
    }
}

const fn allocation_failure(resource: &'static str, requested: usize) -> ProbeLocalBudgetCause {
    ProbeLocalBudgetCause::AllocationFailure {
        scope: ProbeLocalBudgetScope::Probe,
        resource,
        requested,
    }
}

#[cfg(test)]
mod tests;
