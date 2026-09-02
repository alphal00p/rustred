//! Stable, detached scheduler-rejection diagnostics.
//!
//! These values identify where and in which typed subsystem a bounded probe
//! was rejected.  They contain no probe/epoch ordinal, error display text,
//! physical support coordinate, or proof authority.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FoundryCampaignSchedulerRejectionCategory {
    Campaign,
    SourceDiscovery,
    SampledDual,
}

impl FoundryCampaignSchedulerRejectionCategory {
    pub const fn stable_id(self) -> &'static str {
        match self {
            Self::Campaign => "campaign",
            Self::SourceDiscovery => "source-discovery",
            Self::SampledDual => "sampled-dual",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FoundryCampaignProbeStage {
    UnexecutedAggregateSuffix,
    BootstrapNomination,
    BootstrapAccumulation,
    EpochAdmission,
    EpochBuild,
    ModularQuery,
    ObstructionNomination,
    ResidualEvaluation,
    ObstructionBlockNomination,
    ObstructionBlockEvaluation,
    ObstructionBlockSelection,
    RequestMerge,
    SampledDualAdmission,
    ExactLift,
}

impl FoundryCampaignProbeStage {
    pub const fn stable_id(self) -> &'static str {
        match self {
            Self::UnexecutedAggregateSuffix => "unexecuted-aggregate-suffix",
            Self::BootstrapNomination => "bootstrap-nomination",
            Self::BootstrapAccumulation => "bootstrap-accumulation",
            Self::EpochAdmission => "epoch-admission",
            Self::EpochBuild => "epoch-build",
            Self::ModularQuery => "modular-query",
            Self::ObstructionNomination => "obstruction-nomination",
            Self::ResidualEvaluation => "residual-evaluation",
            Self::ObstructionBlockNomination => "obstruction-block-nomination",
            Self::ObstructionBlockEvaluation => "obstruction-block-evaluation",
            Self::ObstructionBlockSelection => "obstruction-block-selection",
            Self::RequestMerge => "request-merge",
            Self::SampledDualAdmission => "sampled-dual-admission",
            Self::ExactLift => "exact-lift",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FoundryCampaignSchedulerRejection {
    category: FoundryCampaignSchedulerRejectionCategory,
    stage: FoundryCampaignProbeStage,
    stable_subkind: &'static str,
}

impl FoundryCampaignSchedulerRejection {
    pub(crate) const fn new(
        category: FoundryCampaignSchedulerRejectionCategory,
        stage: FoundryCampaignProbeStage,
        stable_subkind: &'static str,
    ) -> Self {
        Self {
            category,
            stage,
            stable_subkind,
        }
    }

    pub const fn category(self) -> FoundryCampaignSchedulerRejectionCategory {
        self.category
    }

    pub const fn stage(self) -> FoundryCampaignProbeStage {
        self.stage
    }

    /// Stable identifier selected exhaustively from a typed Rust error
    /// variant. This is never an error's arbitrary `Display` text.
    pub const fn stable_subkind(self) -> &'static str {
        self.stable_subkind
    }
}
