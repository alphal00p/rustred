use std::fmt;

/// Invalid deterministic input to a foundry campaign.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FoundryCampaignConfigError {
    EmptyProbeProgram,
    ProbeCountOverflow,
    ProbeCoordinateCountOverflow,
    TooManyProbes {
        requested: usize,
        limit: usize,
    },
    TooManyProbeCoordinates {
        requested: usize,
        limit: usize,
    },
    TooManyAggregateProbeCoordinates {
        requested: usize,
        limit: usize,
    },
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
    DomainHintArityLimit {
        actual: usize,
        limit: usize,
    },
    DomainHintAxisOutOfBounds {
        axis: usize,
        arity: usize,
    },
    DomainHintAxesNotStrictlyIncreasing {
        previous: usize,
        current: usize,
    },
    DomainHintCountOverflow,
    TooManyDomainHints {
        requested: usize,
        limit: usize,
    },
    WrongDomainHintAnchorArity {
        domain_ordinal: usize,
        expected: usize,
        actual: usize,
    },
    AutonomousDomainHints,
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
}

impl fmt::Display for FoundryCampaignConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyProbeProgram => {
                formatter.write_str("foundry campaign probe program is empty")
            }
            Self::ProbeCountOverflow => {
                formatter.write_str("foundry campaign probe count overflowed usize")
            }
            Self::ProbeCoordinateCountOverflow => formatter
                .write_str("foundry campaign retained probe-coordinate count overflowed usize"),
            Self::TooManyProbes { requested, limit } => write!(
                formatter,
                "foundry campaign declares {requested} probes, exceeding the {limit}-probe limit"
            ),
            Self::TooManyProbeCoordinates { requested, limit } => write!(
                formatter,
                "foundry campaign probe retains {requested} coordinates, exceeding the {limit}-coordinate limit"
            ),
            Self::TooManyAggregateProbeCoordinates { requested, limit } => write!(
                formatter,
                "foundry campaign probe program retains {requested} coordinates, exceeding the {limit}-coordinate aggregate limit"
            ),
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
            Self::DomainHintArityLimit { actual, limit } => write!(
                formatter,
                "foundry campaign domain-hint arity {actual} exceeds the {limit}-coordinate limit"
            ),
            Self::DomainHintAxisOutOfBounds { axis, arity } => write!(
                formatter,
                "foundry campaign domain symbolic axis {axis} is out of bounds for arity {arity}"
            ),
            Self::DomainHintAxesNotStrictlyIncreasing { previous, current } => write!(
                formatter,
                "foundry campaign domain symbolic axes are not strictly increasing at {previous}, {current}"
            ),
            Self::DomainHintCountOverflow => {
                formatter.write_str("foundry campaign domain-hint count overflowed usize")
            }
            Self::TooManyDomainHints { requested, limit } => write!(
                formatter,
                "foundry campaign declares {requested} domain hints, exceeding the {limit}-domain limit"
            ),
            Self::WrongDomainHintAnchorArity {
                domain_ordinal,
                expected,
                actual,
            } => write!(
                formatter,
                "foundry campaign domain hint {domain_ordinal} has anchor arity {actual}, expected {expected}"
            ),
            Self::AutonomousDomainHints => formatter.write_str(
                "autonomous foundry campaigns cannot contain external requested-domain hints",
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} entries for {resource}"
            ),
        }
    }
}

impl std::error::Error for FoundryCampaignConfigError {}

/// Cold setup stage for a built-in campaign preset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FoundryCampaignSetupStage {
    AutonomousSelection,
    Family,
    OrdinarySources,
    Sector,
    TerminalAuthority,
    PredecessorSnapshot,
    ZeroTranslation,
    IncidenceIndex,
    Ledger,
    ProbeProgram,
    RequestedDomains,
    Coordinator,
}

impl fmt::Display for FoundryCampaignSetupStage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::AutonomousSelection => "autonomous search-program selection",
            Self::Family => "family",
            Self::OrdinarySources => "ordinary sources",
            Self::Sector => "sector",
            Self::TerminalAuthority => "terminal authority",
            Self::PredecessorSnapshot => "predecessor snapshot",
            Self::ZeroTranslation => "zero-offset source translation",
            Self::IncidenceIndex => "source incidence index",
            Self::Ledger => "fresh exact ledger",
            Self::ProbeProgram => "probe program",
            Self::RequestedDomains => "requested-domain program",
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
    ResourceCountOverflow {
        stage: FoundryCampaignSetupStage,
        resource: &'static str,
    },
    ResourceLimit {
        stage: FoundryCampaignSetupStage,
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    Invariant {
        detail: &'static str,
    },
}

impl FoundryCampaignError {
    pub const fn setup_stage(&self) -> Option<FoundryCampaignSetupStage> {
        match self {
            Self::Setup { stage, .. }
            | Self::ResourceCountOverflow { stage, .. }
            | Self::ResourceLimit { stage, .. } => Some(*stage),
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
            Self::ResourceCountOverflow { stage, resource } => {
                write!(
                    formatter,
                    "foundry campaign {stage} {resource} count overflowed usize"
                )
            }
            Self::ResourceLimit {
                stage,
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "foundry campaign {stage} requested {requested} {resource}, configured limit is {limit}"
            ),
            Self::Invariant { detail } => {
                write!(formatter, "foundry campaign invariant failed: {detail}")
            }
        }
    }
}

impl std::error::Error for FoundryCampaignError {}
