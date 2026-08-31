use std::fmt;

use crate::foundry::completion::stratum::StratumRegistryError;
use crate::identity::TranslatedSourceError;

use super::super::SourceDiscoveryError;

/// Admission or shared immutable-source failures for an outer probe schedule.
/// Probe-local sampling, exact-lift, and budget outcomes are retained in the
/// successful report instead of aborting sibling probes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ProbeLocalSchedulerError {
    EmptyProbeSchedule,
    WrongSourceLayout {
        actual: &'static str,
    },
    WrongTargetArity {
        expected: usize,
        actual: usize,
    },
    WrongTaskScope {
        detail: &'static str,
    },
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
    DuplicateProbe {
        first_ordinal: usize,
        duplicate_ordinal: usize,
    },
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
    Stratum(StratumRegistryError),
    Shift(TranslatedSourceError),
    SourceTranslation(TranslatedSourceError),
    SourceModule(SourceDiscoveryError),
    Invariant {
        detail: &'static str,
    },
}

impl fmt::Display for ProbeLocalSchedulerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyProbeSchedule => {
                formatter.write_str("probe-local obstruction schedule is empty")
            }
            Self::WrongSourceLayout { actual } => write!(
                formatter,
                "probe-local scheduler requires complete ordinary IBP sources, got {actual}"
            ),
            Self::WrongTargetArity { expected, actual } => write!(
                formatter,
                "probe-local target has arity {actual}, expected {expected}"
            ),
            Self::WrongTaskScope { detail } => {
                write!(formatter, "probe-local task scope mismatch: {detail}")
            }
            Self::UnsupportedEvenModulus {
                probe_ordinal,
                modulus,
            } => write!(
                formatter,
                "probe-local probe {probe_ordinal} requires an odd prime, got even modulus {modulus}"
            ),
            Self::NonPrimeModulus {
                probe_ordinal,
                modulus,
            } => write!(
                formatter,
                "probe-local probe {probe_ordinal} requires a prime modulus, got {modulus}"
            ),
            Self::WrongBaseParameterArity {
                probe_ordinal,
                expected,
                actual,
            } => write!(
                formatter,
                "probe-local probe {probe_ordinal} has {actual} base parameters, expected {expected}"
            ),
            Self::WrongChartCoordinateArity {
                probe_ordinal,
                expected,
                actual,
            } => write!(
                formatter,
                "probe-local probe {probe_ordinal} has {actual} chart coordinates, expected {expected}"
            ),
            Self::DuplicateProbe {
                first_ordinal,
                duplicate_ordinal,
            } => write!(
                formatter,
                "probe-local probe {duplicate_ordinal} repeats the canonical finite-field point of probe {first_ordinal}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} overflowed usize")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} requires {requested}, exceeding configured limit {limit}"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} entries for {resource}"
            ),
            Self::Stratum(error) => {
                write!(formatter, "probe-local stratum admission failed: {error}")
            }
            Self::Shift(error) => write!(formatter, "probe-local zero shift failed: {error}"),
            Self::SourceTranslation(error) => write!(
                formatter,
                "probe-local complete ordinary-source translation failed: {error}"
            ),
            Self::SourceModule(error) => {
                write!(
                    formatter,
                    "probe-local source-module admission failed: {error}"
                )
            }
            Self::Invariant { detail } => {
                write!(
                    formatter,
                    "probe-local scheduler invariant failed: {detail}"
                )
            }
        }
    }
}

impl std::error::Error for ProbeLocalSchedulerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Stratum(error) => Some(error),
            Self::Shift(error) | Self::SourceTranslation(error) => Some(error),
            Self::SourceModule(error) => Some(error),
            _ => None,
        }
    }
}
