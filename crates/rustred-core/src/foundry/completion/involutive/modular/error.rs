use std::fmt;

use crate::algebra::IndexedAlgebraError;

use super::super::InvolutiveError;
use super::model::CoeffNodeId;

/// Typed failure of one proposal-only modular guide lane.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ModularGuideError {
    WrongDagOwner,
    StaleDagReference {
        resource: &'static str,
        ordinal: u32,
    },
    InvalidArenaCheckpoint {
        detail: &'static str,
    },
    WrongIndexedContext,
    WrongPointArity {
        expected: usize,
        actual: usize,
    },
    WrongTranslationArity {
        expected: usize,
        actual: usize,
    },
    UnsupportedModulus {
        modulus: u64,
    },
    InconsistentProbeIdentity,
    InconsistentBatchQueryLayout,
    StructurallyZeroInverse,
    KnownZeroCannotBeCertified,
    SingularExactLeaf {
        node: CoeffNodeId,
    },
    SingularInverse {
        node: CoeffNodeId,
    },
    ExactZeroInverse {
        node: CoeffNodeId,
    },
    InvalidExcludedDivisor {
        ordinal: usize,
        basis_rows: usize,
    },
    SampledZeroMonicLeader,
    SampledZeroLocalizationGuard,
    RejectedProbe,
    ResourceCountOverflow {
        resource: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    IdentifierNotRepresentable {
        resource: &'static str,
        value: usize,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    Algebra(IndexedAlgebraError),
    Involutive(InvolutiveError),
    Invariant {
        detail: &'static str,
    },
}

impl fmt::Display for ModularGuideError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongDagOwner => {
                formatter.write_str("coefficient reference belongs to another modular DAG")
            }
            Self::StaleDagReference { resource, ordinal } => write!(
                formatter,
                "{resource} ordinal {ordinal} names a rolled-back or absent DAG slot"
            ),
            Self::InvalidArenaCheckpoint { detail } => {
                write!(formatter, "invalid modular DAG checkpoint: {detail}")
            }
            Self::WrongIndexedContext => {
                formatter.write_str("modular DAG and probe belong to different indexed contexts")
            }
            Self::WrongPointArity { expected, actual } => write!(
                formatter,
                "modular point has arity {actual}, expected {expected}"
            ),
            Self::WrongTranslationArity { expected, actual } => write!(
                formatter,
                "physical translation has arity {actual}, expected {expected}"
            ),
            Self::UnsupportedModulus { modulus } => write!(
                formatter,
                "modular guide modulus {modulus} is not an admitted odd prime"
            ),
            Self::InconsistentProbeIdentity => formatter.write_str(
                "the supplied modular probe identity does not match its canonical point residues",
            ),
            Self::InconsistentBatchQueryLayout => formatter.write_str(
                "the consumed modular batch does not match the complete ordered tagged query layout",
            ),
            Self::StructurallyZeroInverse => {
                formatter.write_str("a structurally zero coefficient cannot be inverted")
            }
            Self::KnownZeroCannotBeCertified => {
                formatter.write_str("a known-zero coefficient cannot receive a nonzero certificate")
            }
            Self::SingularExactLeaf { node } => write!(
                formatter,
                "exact modular leaf at DAG node {} has a zero denominator",
                node.ordinal()
            ),
            Self::SingularInverse { node } => write!(
                formatter,
                "modular inverse at DAG node {} has a zero sampled operand",
                node.ordinal()
            ),
            Self::ExactZeroInverse { node } => write!(
                formatter,
                "exact materialization found a zero inverse operand at DAG node {}",
                node.ordinal()
            ),
            Self::InvalidExcludedDivisor {
                ordinal,
                basis_rows,
            } => write!(
                formatter,
                "excluded modular Janet divisor {ordinal} is outside the frozen {basis_rows}-row epoch"
            ),
            Self::SampledZeroMonicLeader => formatter
                .write_str("the greatest structural Ore term vanished in this modular probe"),
            Self::SampledZeroLocalizationGuard => formatter
                .write_str("an exact nonzero localization guard vanished in this modular probe"),
            Self::RejectedProbe => formatter.write_str("modular probe was already rejected"),
            Self::ResourceCountOverflow { resource } => {
                write!(
                    formatter,
                    "{resource} overflowed its checked integer carrier"
                )
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} needs {requested} units, exceeding the configured limit {limit}"
            ),
            Self::IdentifierNotRepresentable { resource, value } => {
                write!(
                    formatter,
                    "{resource} value {value} does not fit its stable identifier"
                )
            }
            Self::AllocationFailure {
                resource,
                requested,
            } => {
                write!(
                    formatter,
                    "could not reserve {requested} entries for {resource}"
                )
            }
            Self::Algebra(error) => error.fmt(formatter),
            Self::Involutive(error) => error.fmt(formatter),
            Self::Invariant { detail } => write!(
                formatter,
                "modular coefficient guidance reached an internal invariant failure: {detail}"
            ),
        }
    }
}

impl std::error::Error for ModularGuideError {}

impl From<IndexedAlgebraError> for ModularGuideError {
    fn from(value: IndexedAlgebraError) -> Self {
        Self::Algebra(value)
    }
}

impl From<InvolutiveError> for ModularGuideError {
    fn from(value: InvolutiveError) -> Self {
        Self::Involutive(value)
    }
}

pub(super) fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ModularGuideError> {
    left.checked_add(right)
        .ok_or(ModularGuideError::ResourceCountOverflow { resource })
}

pub(super) fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ModularGuideError> {
    left.checked_mul(right)
        .ok_or(ModularGuideError::ResourceCountOverflow { resource })
}

pub(super) fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ModularGuideError> {
    if requested > limit {
        Err(ModularGuideError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

pub(super) fn reserve_vec<T>(
    values: &mut Vec<T>,
    additional: usize,
    resource: &'static str,
) -> Result<(), ModularGuideError> {
    let requested = checked_add(resource, values.len(), additional)?;
    values
        .try_reserve_exact(additional)
        .map_err(|_| ModularGuideError::AllocationFailure {
            resource,
            requested,
        })
}

pub(super) fn reserve_map<K: Eq + std::hash::Hash, V>(
    values: &mut std::collections::HashMap<K, V>,
    additional: usize,
    resource: &'static str,
) -> Result<(), ModularGuideError> {
    let requested = checked_add(resource, values.len(), additional)?;
    values
        .try_reserve(additional)
        .map_err(|_| ModularGuideError::AllocationFailure {
            resource,
            requested,
        })
}
