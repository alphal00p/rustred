use std::fmt;

use crate::algebra::IndexedAlgebraError;
use crate::foundry::completion::stratum::{GuardBranch, GuardPredicateAuthority};

use super::super::CoefficientIdealGuardError;

/// Typed failures while binding exact guard predicates to one raw probe.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ExactGuardProbeError {
    GuardBlindStratum,
    WrongContext,
    WrongCatalogContext,
    WrongBaseParameterArity {
        expected: usize,
        actual: usize,
    },
    WrongIndexArity {
        expected: usize,
        actual: usize,
    },
    IndexOutsideStratum {
        position: usize,
        index: i64,
        lower: i64,
        upper: i64,
    },
    UnsupportedPredicateAuthority {
        guard_ordinal: usize,
        authority: GuardPredicateAuthority,
    },
    MissingPredicatePayload {
        guard_ordinal: usize,
    },
    PredicateBranchMismatch {
        guard_ordinal: usize,
        required: GuardBranch,
        actual: GuardBranch,
    },
    BaseSampleOnNonzeroGuardWall {
        guard_ordinal: usize,
    },
    WitnessStratumMismatch,
    WitnessContextMismatch,
    WitnessBasePointMismatch,
    WitnessIndexPointMismatch,
    UnluckyGuardPrime {
        guard_ordinal: usize,
        modulus: u64,
    },
    CatalogAtom(CoefficientIdealGuardError),
    IndexedAlgebra(IndexedAlgebraError),
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
    SymbolicaPanic {
        operation: &'static str,
    },
    Invariant {
        detail: &'static str,
    },
}

impl fmt::Display for ExactGuardProbeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GuardBlindStratum => {
                formatter.write_str("an exact guard-probe witness requires a guarded stratum")
            }
            Self::WrongContext => formatter.write_str(
                "exact guard-probe witness and decorated stratum use different contexts",
            ),
            Self::WrongCatalogContext => formatter
                .write_str("exact guard predicate catalog belongs to another indexed context"),
            Self::WrongBaseParameterArity { expected, actual } => write!(
                formatter,
                "exact guard base point has arity {actual}, expected {expected}"
            ),
            Self::WrongIndexArity { expected, actual } => write!(
                formatter,
                "exact guard index point has arity {actual}, expected {expected}"
            ),
            Self::IndexOutsideStratum {
                position,
                index,
                lower,
                upper,
            } => write!(
                formatter,
                "exact guard index {index} at position {position} is outside stratum bound {lower}..={upper}"
            ),
            Self::UnsupportedPredicateAuthority {
                guard_ordinal,
                authority,
            } => write!(
                formatter,
                "decorated-stratum guard {guard_ordinal} uses unsupported predicate authority {authority:?}"
            ),
            Self::MissingPredicatePayload { guard_ordinal } => write!(
                formatter,
                "exact predicate payload for decorated-stratum guard {guard_ordinal} is absent"
            ),
            Self::PredicateBranchMismatch {
                guard_ordinal,
                required,
                actual,
            } => write!(
                formatter,
                "exact probe reaches {actual:?} for guard {guard_ordinal}, but the stratum requires {required:?}"
            ),
            Self::BaseSampleOnNonzeroGuardWall { guard_ordinal } => write!(
                formatter,
                "exact base sample lies on the numeric wall of nonzero guard {guard_ordinal}"
            ),
            Self::WitnessStratumMismatch => {
                formatter.write_str("exact guard witness belongs to another decorated stratum")
            }
            Self::WitnessContextMismatch => {
                formatter.write_str("exact guard witness belongs to another indexed context")
            }
            Self::WitnessBasePointMismatch => {
                formatter.write_str("exact guard witness belongs to another base-parameter point")
            }
            Self::WitnessIndexPointMismatch => {
                formatter.write_str("exact guard witness belongs to another integer-index point")
            }
            Self::UnluckyGuardPrime {
                guard_ordinal,
                modulus,
            } => write!(
                formatter,
                "nonzero exact guard {guard_ordinal} vanishes modulo prime {modulus}"
            ),
            Self::CatalogAtom(error) => write!(formatter, "guard catalog atom failed: {error}"),
            Self::IndexedAlgebra(error) => {
                write!(formatter, "exact guard specialization failed: {error}")
            }
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "exact guard-probe {resource} overflowed usize")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "exact guard-probe {resource} requires {requested}, exceeding the configured limit {limit}"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} entries for exact guard-probe {resource}"
            ),
            Self::SymbolicaPanic { operation } => {
                write!(formatter, "Symbolica panicked while {operation}")
            }
            Self::Invariant { detail } => {
                write!(formatter, "exact guard-probe invariant failed: {detail}")
            }
        }
    }
}

impl std::error::Error for ExactGuardProbeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CatalogAtom(error) => Some(error),
            Self::IndexedAlgebra(error) => Some(error),
            _ => None,
        }
    }
}

impl From<CoefficientIdealGuardError> for ExactGuardProbeError {
    fn from(value: CoefficientIdealGuardError) -> Self {
        Self::CatalogAtom(value)
    }
}

impl From<IndexedAlgebraError> for ExactGuardProbeError {
    fn from(value: IndexedAlgebraError) -> Self {
        Self::IndexedAlgebra(value)
    }
}
