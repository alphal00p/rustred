use std::fmt;

use crate::foundry::completion::stratum::StratumRegistryError;

/// Typed failures while admitting or scheduling bounded target evidence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TargetEvidenceError {
    WrongFrameContext,
    WrongContextIndexArity {
        expected: usize,
        actual: usize,
    },
    ForeignFramePartition,
    PartitionVerification(StratumRegistryError),
    UnsupportedEvenModulus {
        probe_ordinal: usize,
        modulus: u64,
    },
    NonPrimeModulus {
        probe_ordinal: usize,
        modulus: u64,
    },
    WrongBaseParameterArity {
        probe_ordinal: usize,
        expected: usize,
        actual: usize,
    },
    WrongChartCoordinateArity {
        probe_ordinal: usize,
        expected: usize,
        actual: usize,
    },
    DuplicateProbeTask {
        first_ordinal: usize,
        duplicate_ordinal: usize,
    },
    MissingDiscoveryProbe,
    ResourceCountOverflow {
        resource: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    Invariant {
        detail: &'static str,
    },
}

impl fmt::Display for TargetEvidenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongFrameContext => write!(
                formatter,
                "target-evidence context fingerprint differs from its physical frame"
            ),
            Self::WrongContextIndexArity { expected, actual } => write!(
                formatter,
                "target-evidence context has {actual} indices, expected frame arity {expected}"
            ),
            Self::ForeignFramePartition => write!(
                formatter,
                "target-evidence probe plan and target partition borrow different physical frames"
            ),
            Self::PartitionVerification(error) => {
                write!(
                    formatter,
                    "target-evidence partition verification failed: {error}"
                )
            }
            Self::UnsupportedEvenModulus {
                probe_ordinal,
                modulus,
            } => write!(
                formatter,
                "target-evidence probe {probe_ordinal} requires an odd prime, got even modulus {modulus}"
            ),
            Self::NonPrimeModulus {
                probe_ordinal,
                modulus,
            } => write!(
                formatter,
                "target-evidence probe {probe_ordinal} requires a prime modulus, got {modulus}"
            ),
            Self::WrongBaseParameterArity {
                probe_ordinal,
                expected,
                actual,
            } => write!(
                formatter,
                "target-evidence probe {probe_ordinal} has {actual} base parameters, expected {expected}"
            ),
            Self::WrongChartCoordinateArity {
                probe_ordinal,
                expected,
                actual,
            } => write!(
                formatter,
                "target-evidence probe {probe_ordinal} has {actual} chart coordinates, expected {expected}"
            ),
            Self::DuplicateProbeTask {
                first_ordinal,
                duplicate_ordinal,
            } => write!(
                formatter,
                "target-evidence probe {duplicate_ordinal} repeats the canonical finite-field task key of probe {first_ordinal}"
            ),
            Self::MissingDiscoveryProbe => {
                write!(formatter, "target-evidence plan has no Discovery probe")
            }
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} overflowed usize")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} requires {requested}, exceeding the configured limit {limit}"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} entries for {resource}"
            ),
            Self::Invariant { detail } => {
                write!(formatter, "target-evidence invariant failed: {detail}")
            }
        }
    }
}

impl std::error::Error for TargetEvidenceError {}

impl From<StratumRegistryError> for TargetEvidenceError {
    fn from(value: StratumRegistryError) -> Self {
        Self::PartitionVerification(value)
    }
}
