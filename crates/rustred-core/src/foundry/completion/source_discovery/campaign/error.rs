use std::fmt;

use crate::foundry::completion::frame::PhysicalFrameError;
use crate::foundry::completion::frame::modular::ModularKernelError;
use crate::foundry::completion::stratum::StratumRegistryError;
use crate::identity::TranslatedSourceError;

/// Which bounded campaign layer exhausted its declared resource envelope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CampaignResourceStage {
    RequestAccumulation,
    SelectedTranslation,
    PhysicalFrame,
    StratumPartition,
    ModularQuery,
}

/// Typed research-budget telemetry.
///
/// This value is a resumable stop reason. It is never evidence that the
/// translated source module has no relation and cannot create a terminal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CampaignBudgetExhaustion {
    stage: CampaignResourceStage,
    resource: &'static str,
    requested: usize,
    limit: usize,
}

impl CampaignBudgetExhaustion {
    pub(crate) const fn stage(&self) -> CampaignResourceStage {
        self.stage
    }

    pub(crate) const fn resource(&self) -> &'static str {
        self.resource
    }

    pub(crate) const fn requested(&self) -> usize {
        self.requested
    }

    pub(crate) const fn limit(&self) -> usize {
        self.limit
    }

    pub(super) const fn new(
        stage: CampaignResourceStage,
        resource: &'static str,
        requested: usize,
        limit: usize,
    ) -> Self {
        Self {
            stage,
            resource,
            requested,
            limit,
        }
    }
}

/// Failures while canonicalizing requests or rebuilding one fresh task epoch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CampaignError {
    EmptyRequestArity,
    EmptyAccumulatedRequests,
    WrongRequestArity {
        request_ordinal: usize,
        expected: usize,
        actual: usize,
    },
    WrongTargetArity {
        expected: usize,
        actual: usize,
    },
    WrongProbeChartArity {
        expected: usize,
        actual: usize,
    },
    WrongSourceLayout {
        actual: &'static str,
    },
    FixedTaskScopeMismatch {
        detail: &'static str,
    },
    SourceChronologyMismatch,
    TargetColumnAbsent,
    FixedStratumDoesNotCoverColumn {
        column: usize,
    },
    SampleCoordinateNotRepresentable {
        position: usize,
        active: bool,
        coordinate: u64,
    },
    SampleOutsideFixedStratum {
        position: usize,
        index: i64,
        lower: i64,
        upper: i64,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    BudgetExhausted(CampaignBudgetExhaustion),
    TranslatedSources(TranslatedSourceError),
    PhysicalFrame(PhysicalFrameError),
    Stratum(StratumRegistryError),
    Modular(ModularKernelError),
    Invariant {
        detail: &'static str,
    },
}

impl CampaignError {
    pub(crate) const fn budget_exhaustion(&self) -> Option<&CampaignBudgetExhaustion> {
        match self {
            Self::BudgetExhausted(exhaustion) => Some(exhaustion),
            _ => None,
        }
    }
}

impl fmt::Display for CampaignError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyRequestArity => {
                write!(formatter, "source-discovery campaign request arity is zero")
            }
            Self::EmptyAccumulatedRequests => write!(
                formatter,
                "a fresh selected-source campaign epoch requires at least one accumulated request"
            ),
            Self::WrongRequestArity {
                request_ordinal,
                expected,
                actual,
            } => write!(
                formatter,
                "campaign request {request_ordinal} has arity {actual}, expected {expected}"
            ),
            Self::WrongTargetArity { expected, actual } => write!(
                formatter,
                "campaign target shift has arity {actual}, expected {expected}"
            ),
            Self::WrongProbeChartArity { expected, actual } => write!(
                formatter,
                "campaign probe has {actual} chart coordinates, expected {expected}"
            ),
            Self::WrongSourceLayout { actual } => write!(
                formatter,
                "campaign epochs require the complete ordinary IBP source layout, got {actual}"
            ),
            Self::FixedTaskScopeMismatch { detail } => {
                write!(formatter, "fixed campaign task scope mismatch: {detail}")
            }
            Self::SourceChronologyMismatch => write!(
                formatter,
                "selected translation changed the accumulated ordinary-source chronology"
            ),
            Self::TargetColumnAbsent => write!(
                formatter,
                "the raw campaign target is absent from the freshly rebuilt physical frame"
            ),
            Self::FixedStratumDoesNotCoverColumn { column } => write!(
                formatter,
                "the fixed campaign stratum does not keep fresh physical column {column} representable"
            ),
            Self::SampleCoordinateNotRepresentable {
                position,
                active,
                coordinate,
            } => write!(
                formatter,
                "chart coordinate {coordinate} at position {position} cannot be mapped to an i64 index in the {} sector",
                if *active { "active" } else { "inactive" }
            ),
            Self::SampleOutsideFixedStratum {
                position,
                index,
                lower,
                upper,
            } => write!(
                formatter,
                "sample index {index} at position {position} lies outside the fixed stratum bound {lower}..={upper}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} entries for {resource}"
            ),
            Self::BudgetExhausted(exhaustion) => write!(
                formatter,
                "campaign {:?} budget exhausted: {} needs {}, limit {}",
                exhaustion.stage, exhaustion.resource, exhaustion.requested, exhaustion.limit
            ),
            Self::TranslatedSources(error) => write!(
                formatter,
                "fresh campaign selected-source translation failed: {error}"
            ),
            Self::PhysicalFrame(error) => {
                write!(formatter, "fresh campaign physical frame failed: {error}")
            }
            Self::Stratum(error) => {
                write!(formatter, "fresh campaign target partition failed: {error}")
            }
            Self::Modular(error) => {
                write!(formatter, "fresh campaign modular query failed: {error}")
            }
            Self::Invariant { detail } => {
                write!(formatter, "fresh campaign invariant failed: {detail}")
            }
        }
    }
}

impl std::error::Error for CampaignError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::TranslatedSources(error) => Some(error),
            Self::PhysicalFrame(error) => Some(error),
            Self::Stratum(error) => Some(error),
            Self::Modular(error) => Some(error),
            _ => None,
        }
    }
}

pub(super) fn translated_error(error: TranslatedSourceError) -> CampaignError {
    match error {
        TranslatedSourceError::ResourceLimit {
            resource,
            requested,
            limit,
        } => CampaignError::BudgetExhausted(CampaignBudgetExhaustion::new(
            CampaignResourceStage::SelectedTranslation,
            resource,
            requested,
            limit,
        )),
        error => CampaignError::TranslatedSources(error),
    }
}

pub(super) fn frame_error(error: PhysicalFrameError) -> CampaignError {
    match error {
        PhysicalFrameError::ResourceLimit {
            resource,
            requested,
            limit,
        } => CampaignError::BudgetExhausted(CampaignBudgetExhaustion::new(
            CampaignResourceStage::PhysicalFrame,
            resource,
            requested,
            limit,
        )),
        error => CampaignError::PhysicalFrame(error),
    }
}

pub(super) fn stratum_error(error: StratumRegistryError) -> CampaignError {
    match error {
        StratumRegistryError::UncoveredPhysicalShift { column } => {
            CampaignError::FixedStratumDoesNotCoverColumn { column }
        }
        StratumRegistryError::ResourceLimit {
            resource,
            requested,
            limit,
        } => CampaignError::BudgetExhausted(CampaignBudgetExhaustion::new(
            CampaignResourceStage::StratumPartition,
            resource,
            requested,
            limit,
        )),
        error => CampaignError::Stratum(error),
    }
}

pub(super) fn modular_error(error: ModularKernelError) -> CampaignError {
    match error {
        ModularKernelError::ResourceLimit {
            resource,
            requested,
            limit,
        } => CampaignError::BudgetExhausted(CampaignBudgetExhaustion::new(
            CampaignResourceStage::ModularQuery,
            resource,
            requested,
            limit,
        )),
        error => CampaignError::Modular(error),
    }
}
