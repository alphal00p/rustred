use std::fmt;

use crate::foundry::completion::stratum::StratumRegistryError;

use super::super::DirectShiftedSourceError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SpiredStructuralPreparationError {
    WrongCaseFamily,
    WrongCaseContext,
    Poisoned,
    DuplicateTargetTerm {
        row_ordinal: usize,
    },
    ForbiddenColumnIdNotRepresentable {
        forbidden_columns: usize,
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
    Evaluation(DirectShiftedSourceError),
    Classification(StratumRegistryError),
    Invariant {
        detail: &'static str,
    },
}

impl fmt::Display for SpiredStructuralPreparationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongCaseFamily => formatter.write_str(
                "SpIRed structural case and ordinary sources belong to different families",
            ),
            Self::WrongCaseContext => {
                formatter.write_str("SpIRed structural case and exact source context do not match")
            }
            Self::Poisoned => formatter
                .write_str("SpIRed structural preparation was poisoned by a prior partial failure"),
            Self::DuplicateTargetTerm { row_ordinal } => write!(
                formatter,
                "prepared structural row {row_ordinal} contains the target shift more than once"
            ),
            Self::ForbiddenColumnIdNotRepresentable { forbidden_columns } => write!(
                formatter,
                "SpIRed structural preparation cannot encode forbidden column {forbidden_columns} as u32"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "SpIRed structural {resource} overflowed usize")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "SpIRed structural {resource} requires {requested}, exceeding the configured limit {limit}"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} entries for SpIRed structural {resource}"
            ),
            Self::Evaluation(error) => write!(formatter, "exact source validation failed: {error}"),
            Self::Classification(error) => {
                write!(
                    formatter,
                    "prospective structural classification failed: {error}"
                )
            }
            Self::Invariant { detail } => {
                write!(formatter, "SpIRed structural invariant failed: {detail}")
            }
        }
    }
}

impl std::error::Error for SpiredStructuralPreparationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Evaluation(error) => Some(error),
            Self::Classification(error) => Some(error),
            _ => None,
        }
    }
}

impl From<DirectShiftedSourceError> for SpiredStructuralPreparationError {
    fn from(error: DirectShiftedSourceError) -> Self {
        Self::Evaluation(error)
    }
}

impl From<StratumRegistryError> for SpiredStructuralPreparationError {
    fn from(error: StratumRegistryError) -> Self {
        Self::Classification(error)
    }
}
