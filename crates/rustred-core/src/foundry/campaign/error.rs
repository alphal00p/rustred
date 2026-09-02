use std::fmt;

/// Invalid deterministic input to a foundry campaign.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FoundryCampaignConfigError {
    EmptyProbeProgram,
    ZeroInteriorMargin,
    ZeroTaskReportLimit,
    WrongProbeBaseParameterArity {
        probe_ordinal: usize,
        expected: usize,
        actual: usize,
    },
    WrongProbeChartOffsetArity {
        probe_ordinal: usize,
        expected: usize,
        actual: usize,
    },
    WrongDiscoveryCoordinatePriorityArity {
        expected: usize,
        actual: usize,
    },
    WrongOrderingPolicyArity {
        expected: usize,
        actual: usize,
    },
    InvalidOrderingPolicy {
        message: String,
    },
    InvalidDiscoveryCoordinatePriority {
        message: String,
    },
    InvalidProbe {
        probe_ordinal: usize,
        message: String,
    },
}

impl fmt::Display for FoundryCampaignConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyProbeProgram => {
                formatter.write_str("foundry campaign probe program is empty")
            }
            Self::ZeroInteriorMargin => {
                formatter.write_str("foundry campaign interior margin must be positive")
            }
            Self::ZeroTaskReportLimit => {
                formatter.write_str("foundry campaign task-report limit must be positive")
            }
            Self::WrongProbeBaseParameterArity {
                probe_ordinal,
                expected,
                actual,
            } => write!(
                formatter,
                "foundry campaign probe {probe_ordinal} has {actual} base parameters, expected {expected}"
            ),
            Self::WrongProbeChartOffsetArity {
                probe_ordinal,
                expected,
                actual,
            } => write!(
                formatter,
                "foundry campaign probe {probe_ordinal} has {actual} chart offsets, expected {expected}"
            ),
            Self::WrongDiscoveryCoordinatePriorityArity { expected, actual } => write!(
                formatter,
                "foundry campaign discovery coordinate priority has arity {actual}, expected {expected}"
            ),
            Self::WrongOrderingPolicyArity { expected, actual } => write!(
                formatter,
                "foundry campaign proof ordering has arity {actual}, expected {expected}"
            ),
            Self::InvalidOrderingPolicy { message } => write!(
                formatter,
                "foundry campaign proof ordering is invalid: {message}"
            ),
            Self::InvalidDiscoveryCoordinatePriority { message } => write!(
                formatter,
                "foundry campaign discovery coordinate priority is invalid: {message}"
            ),
            Self::InvalidProbe {
                probe_ordinal,
                message,
            } => write!(
                formatter,
                "foundry campaign probe {probe_ordinal} is invalid: {message}"
            ),
        }
    }
}

impl std::error::Error for FoundryCampaignConfigError {}

/// Cold setup stage for a built-in campaign preset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FoundryCampaignSetupStage {
    Family,
    OrdinarySources,
    Sector,
    TerminalAuthority,
    PredecessorSnapshot,
    ZeroTranslation,
    IncidenceIndex,
    Ledger,
    ProbeProgram,
    Coordinator,
}

impl fmt::Display for FoundryCampaignSetupStage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Family => "family",
            Self::OrdinarySources => "ordinary sources",
            Self::Sector => "sector",
            Self::TerminalAuthority => "terminal authority",
            Self::PredecessorSnapshot => "predecessor snapshot",
            Self::ZeroTranslation => "zero-offset source translation",
            Self::IncidenceIndex => "source incidence index",
            Self::Ledger => "fresh exact ledger",
            Self::ProbeProgram => "probe program",
            Self::Coordinator => "boundary coordinator",
        })
    }
}

/// A typed failure before a deterministic diagnostic report could be
/// produced. Private proof-engine errors are rendered at this boundary rather
/// than becoming part of the public API.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FoundryCampaignError {
    Setup {
        stage: FoundryCampaignSetupStage,
        message: String,
    },
    Execution {
        message: String,
    },
    Invariant {
        detail: &'static str,
    },
}

impl FoundryCampaignError {
    pub const fn setup_stage(&self) -> Option<FoundryCampaignSetupStage> {
        match self {
            Self::Setup { stage, .. } => Some(*stage),
            Self::Execution { .. } | Self::Invariant { .. } => None,
        }
    }

    pub(crate) fn setup(stage: FoundryCampaignSetupStage, error: impl fmt::Display) -> Self {
        Self::Setup {
            stage,
            message: error.to_string(),
        }
    }
}

impl fmt::Display for FoundryCampaignError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Setup { stage, message } => {
                write!(
                    formatter,
                    "foundry campaign {stage} setup failed: {message}"
                )
            }
            Self::Execution { message } => {
                write!(formatter, "foundry campaign execution failed: {message}")
            }
            Self::Invariant { detail } => {
                write!(formatter, "foundry campaign invariant failed: {detail}")
            }
        }
    }
}

impl std::error::Error for FoundryCampaignError {}
