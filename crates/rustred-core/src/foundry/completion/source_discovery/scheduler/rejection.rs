//! Ordinal-free scheduler-rejection diagnostics.
//!
//! A rejection remains a hard, non-resource probe outcome.  This module
//! detaches only its typed outer category, probe-local stage, and an
//! exhaustively selected stable subkind.  It never retains an error display
//! string, probe ordinal, physical row/column ordinal, or proof payload.

use super::{ProbeLocalRejection, ProbeLocalStage};
use crate::foundry::completion::source_discovery::{
    CampaignError, SampledDeclaredModuleDualError, SourceDiscoveryError,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProbeLocalRejectionCategory {
    Campaign,
    SourceDiscovery,
    SampledDual,
}

impl ProbeLocalRejectionCategory {
    pub(crate) const fn stable_id(self) -> &'static str {
        match self {
            Self::Campaign => "campaign",
            Self::SourceDiscovery => "source-discovery",
            Self::SampledDual => "sampled-dual",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ProbeLocalRejectionSummary {
    stage: ProbeLocalStage,
    category: ProbeLocalRejectionCategory,
    subkind: &'static str,
}

impl ProbeLocalRejectionSummary {
    pub(crate) const fn stage(self) -> ProbeLocalStage {
        self.stage
    }

    pub(crate) const fn category(self) -> ProbeLocalRejectionCategory {
        self.category
    }

    pub(crate) const fn subkind(self) -> &'static str {
        self.subkind
    }

    pub(crate) const fn from_rejection(
        stage: ProbeLocalStage,
        rejection: &ProbeLocalRejection,
    ) -> Self {
        let (category, subkind) = match rejection {
            ProbeLocalRejection::Campaign(error) => (
                ProbeLocalRejectionCategory::Campaign,
                campaign_subkind(error),
            ),
            ProbeLocalRejection::SourceDiscovery(error) => (
                ProbeLocalRejectionCategory::SourceDiscovery,
                source_discovery_subkind(error),
            ),
            ProbeLocalRejection::SampledDual(error) => (
                ProbeLocalRejectionCategory::SampledDual,
                sampled_dual_subkind(error),
            ),
        };
        Self {
            stage,
            category,
            subkind,
        }
    }
}

const fn campaign_subkind(error: &CampaignError) -> &'static str {
    match error {
        CampaignError::EmptyRequestArity => "empty-request-arity",
        CampaignError::EmptyAccumulatedRequests => "empty-accumulated-requests",
        CampaignError::WrongRequestArity { .. } => "wrong-request-arity",
        CampaignError::WrongTargetArity { .. } => "wrong-target-arity",
        CampaignError::WrongProbeChartArity { .. } => "wrong-probe-chart-arity",
        CampaignError::WrongSourceLayout { .. } => "wrong-source-layout",
        CampaignError::FixedTaskScopeMismatch { .. } => "fixed-task-scope-mismatch",
        CampaignError::SourceChronologyMismatch => "source-chronology-mismatch",
        CampaignError::NonMonotoneGrowingRequests { .. } => "nonmonotone-growing-requests",
        CampaignError::TargetColumnAbsent => "target-column-absent",
        CampaignError::FixedStratumDoesNotCoverColumn { .. } => {
            "fixed-stratum-does-not-cover-column"
        }
        CampaignError::SampleCoordinateNotRepresentable { .. } => {
            "sample-coordinate-not-representable"
        }
        CampaignError::SampleOutsideFixedStratum { .. } => "sample-outside-fixed-stratum",
        CampaignError::ResourceCountOverflow { .. } => "resource-count-overflow",
        CampaignError::AllocationFailure { .. } => "allocation-failure",
        CampaignError::BudgetExhausted(_) => "budget-exhausted",
        CampaignError::TranslatedSources(_) => "translated-sources",
        CampaignError::PhysicalFrame(_) => "physical-frame",
        CampaignError::Stratum(_) => "stratum",
        CampaignError::Modular(_) => "modular",
        CampaignError::Invariant { .. } => "invariant",
    }
}

const fn source_discovery_subkind(error: &SourceDiscoveryError) -> &'static str {
    match error {
        SourceDiscoveryError::WrongSourceLayout { .. } => "wrong-source-layout",
        SourceDiscoveryError::ScopeMismatch { .. } => "scope-mismatch",
        SourceDiscoveryError::WrongArity { .. } => "wrong-arity",
        SourceDiscoveryError::ShiftOverflow { .. } => "shift-overflow",
        SourceDiscoveryError::ShiftConstruction(_) => "shift-construction",
        SourceDiscoveryError::SourceTranslation(_) => "source-translation",
        SourceDiscoveryError::NominationIncidenceMismatch => "nomination-incidence-mismatch",
        SourceDiscoveryError::TargetUnitNominationForObstruction => {
            "target-unit-nomination-for-obstruction"
        }
        SourceDiscoveryError::NominationObstructionMismatch => "nomination-obstruction-mismatch",
        SourceDiscoveryError::CompletedSourceChronologyMismatch => {
            "completed-source-chronology-mismatch"
        }
        SourceDiscoveryError::SelectedRequestProvenanceMismatch { .. } => {
            "selected-request-provenance-mismatch"
        }
        SourceDiscoveryError::SelectedSourceRowMismatch { .. } => "selected-source-row-mismatch",
        SourceDiscoveryError::ObstructionPlanMismatch => "obstruction-plan-mismatch",
        SourceDiscoveryError::ObstructionSampleMismatch => "obstruction-sample-mismatch",
        SourceDiscoveryError::ProposalPartitionMismatch => "proposal-partition-mismatch",
        SourceDiscoveryError::ProposalClassification(_) => "proposal-classification",
        SourceDiscoveryError::CandidateEvaluation { .. } => "candidate-evaluation",
        SourceDiscoveryError::ResourceCountOverflow { .. } => "resource-count-overflow",
        SourceDiscoveryError::ResourceLimit { .. } => "resource-limit",
        SourceDiscoveryError::AllocationFailure { .. } => "allocation-failure",
        SourceDiscoveryError::Invariant { .. } => "invariant",
    }
}

const fn sampled_dual_subkind(error: &SampledDeclaredModuleDualError) -> &'static str {
    match error {
        SampledDeclaredModuleDualError::IncidenceVerification(_) => "incidence-verification",
        SampledDeclaredModuleDualError::IncidenceTaskScopeMismatch { .. } => {
            "incidence-task-scope-mismatch"
        }
        SampledDeclaredModuleDualError::GuardedStratumRequiresSampleWitness { .. } => {
            "guarded-stratum-requires-sample-witness"
        }
        SampledDeclaredModuleDualError::QueryIsModularHit => "query-is-modular-hit",
        SampledDeclaredModuleDualError::PartitionVerification(_) => "partition-verification",
        SampledDeclaredModuleDualError::PartitionNotVerified => "partition-not-verified",
        SampledDeclaredModuleDualError::PartitionPlanMismatch => "partition-plan-mismatch",
        SampledDeclaredModuleDualError::SamplePlanMismatch => "sample-plan-mismatch",
        SampledDeclaredModuleDualError::ObstructionPlanMismatch => "obstruction-plan-mismatch",
        SampledDeclaredModuleDualError::ObstructionSampleMismatch => "obstruction-sample-mismatch",
        SampledDeclaredModuleDualError::ObstructionPartitionMismatch => {
            "obstruction-partition-mismatch"
        }
        SampledDeclaredModuleDualError::TargetColumnOutOfRange => "target-column-out-of-range",
        SampledDeclaredModuleDualError::TargetColumnMismatch => "target-column-mismatch",
        SampledDeclaredModuleDualError::TargetShiftMismatch => "target-shift-mismatch",
        SampledDeclaredModuleDualError::FixedStratumMismatch => "fixed-stratum-mismatch",
        SampledDeclaredModuleDualError::FixedOrderingMismatch => "fixed-ordering-mismatch",
        SampledDeclaredModuleDualError::FixedOwnerSnapshotMismatch => {
            "fixed-owner-snapshot-mismatch"
        }
        SampledDeclaredModuleDualError::MaterializedSourceChronologyMismatch => {
            "materialized-source-chronology-mismatch"
        }
        SampledDeclaredModuleDualError::NominationIncidenceMismatch => {
            "nomination-incidence-mismatch"
        }
        SampledDeclaredModuleDualError::NominationIsTargetUnit => "nomination-is-target-unit",
        SampledDeclaredModuleDualError::NominationObstructionMismatch => {
            "nomination-obstruction-mismatch"
        }
        SampledDeclaredModuleDualError::ResidualNominationMismatch => {
            "residual-nomination-mismatch"
        }
        SampledDeclaredModuleDualError::ResidualIncidenceMismatch => "residual-incidence-mismatch",
        SampledDeclaredModuleDualError::ResidualPlanMismatch => "residual-plan-mismatch",
        SampledDeclaredModuleDualError::ResidualObstructionMismatch => {
            "residual-obstruction-mismatch"
        }
        SampledDeclaredModuleDualError::ResidualSampleMismatch => "residual-sample-mismatch",
        SampledDeclaredModuleDualError::NominationVerification(_) => "nomination-verification",
        SampledDeclaredModuleDualError::IncompleteNominationCensus => {
            "incomplete-nomination-census"
        }
        SampledDeclaredModuleDualError::ResidualTelemetryMismatch => "residual-telemetry-mismatch",
        SampledDeclaredModuleDualError::ResidualPairingShiftOverflow { .. } => {
            "residual-pairing-shift-overflow"
        }
        SampledDeclaredModuleDualError::CuttingResiduals { .. } => "cutting-residuals",
        SampledDeclaredModuleDualError::RawObstructionMismatch => "raw-obstruction-mismatch",
        SampledDeclaredModuleDualError::RankDiagnosticsMismatch { .. } => {
            "rank-diagnostics-mismatch"
        }
        SampledDeclaredModuleDualError::Retention(_) => "retention",
    }
}
