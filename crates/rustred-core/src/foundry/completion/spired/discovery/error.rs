use std::fmt;

use crate::foundry::completion::guard::ExactGuardProbeError;
use crate::foundry::completion::source_discovery::CampaignError;
use crate::foundry::completion::stratum::StratumRegistryError;

use super::super::{
    DirectShiftedSourceError, SpiredModularError, SpiredStructuralPreparationError,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SpiredStreamingError {
    WrongCaseFamily,
    WrongCaseContext,
    GuardedStratumRequiresSampleWitness {
        guard_count: usize,
    },
    Poisoned,
    DuplicateTargetTerm,
    PreparedPlanScopeMismatch,
    PreparedPlanChronology {
        expected_row: usize,
        actual_row: usize,
    },
    PreparedTermCountMismatch {
        row: usize,
        planned: usize,
        evaluated: usize,
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
    Probe(CampaignError),
    GuardProbe(ExactGuardProbeError),
    Evaluation(DirectShiftedSourceError),
    StructuralPreparation(SpiredStructuralPreparationError),
    Classification(StratumRegistryError),
    Modular(SpiredModularError),
    Invariant {
        detail: &'static str,
    },
}

impl fmt::Display for SpiredStreamingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongCaseFamily => formatter.write_str(
                "SpIRed streaming case and ordinary sources belong to different families",
            ),
            Self::WrongCaseContext => formatter.write_str(
                "SpIRed streaming case and coefficient evaluator use different contexts",
            ),
            Self::GuardedStratumRequiresSampleWitness { guard_count } => write!(
                formatter,
                "SpIRed streaming discovery cannot probe {guard_count} decorated-stratum guards without an exact sample-bound predicate witness"
            ),
            Self::Poisoned => {
                formatter.write_str("SpIRed streaming discovery was poisoned by a prior failure")
            }
            Self::DuplicateTargetTerm => formatter.write_str(
                "one translated ordinary source contains the target structural shift twice",
            ),
            Self::PreparedPlanScopeMismatch => formatter.write_str(
                "prepared structural rows belong to a different exact case/source/owner scope",
            ),
            Self::PreparedPlanChronology {
                expected_row,
                actual_row,
            } => write!(
                formatter,
                "prepared structural row {actual_row} is out of order; probe expects row {expected_row}"
            ),
            Self::PreparedTermCountMismatch {
                row,
                planned,
                evaluated,
            } => write!(
                formatter,
                "prepared row {row} has {planned} structural roles but coefficient evaluation returned {evaluated} residues"
            ),
            Self::ForbiddenColumnIdNotRepresentable { forbidden_columns } => write!(
                formatter,
                "SpIRed streaming discovery cannot encode forbidden column {forbidden_columns} as u32"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "SpIRed streaming {resource} overflowed usize")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "SpIRed streaming {resource} requires {requested}, exceeding the configured limit {limit}"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} entries for SpIRed streaming {resource}"
            ),
            Self::Probe(error) => write!(
                formatter,
                "SpIRed streaming modular probe does not match its exact case: {error}"
            ),
            Self::GuardProbe(error) => {
                write!(
                    formatter,
                    "SpIRed streaming guard-probe admission failed: {error}"
                )
            }
            Self::Evaluation(error) => {
                write!(formatter, "shifted-source evaluation failed: {error}")
            }
            Self::StructuralPreparation(error) => {
                write!(formatter, "shared structural preparation failed: {error}")
            }
            Self::Classification(error) => {
                write!(
                    formatter,
                    "prospective-column classification failed: {error}"
                )
            }
            Self::Modular(error) => {
                write!(formatter, "streaming modular reduction failed: {error}")
            }
            Self::Invariant { detail } => {
                write!(formatter, "SpIRed streaming invariant failed: {detail}")
            }
        }
    }
}

impl std::error::Error for SpiredStreamingError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Probe(error) => Some(error),
            Self::GuardProbe(error) => Some(error),
            Self::Evaluation(error) => Some(error),
            Self::StructuralPreparation(error) => Some(error),
            Self::Classification(error) => Some(error),
            Self::Modular(error) => Some(error),
            _ => None,
        }
    }
}

impl From<CampaignError> for SpiredStreamingError {
    fn from(error: CampaignError) -> Self {
        Self::Probe(error)
    }
}

impl From<ExactGuardProbeError> for SpiredStreamingError {
    fn from(error: ExactGuardProbeError) -> Self {
        Self::GuardProbe(error)
    }
}

impl From<DirectShiftedSourceError> for SpiredStreamingError {
    fn from(error: DirectShiftedSourceError) -> Self {
        Self::Evaluation(error)
    }
}

impl From<SpiredStructuralPreparationError> for SpiredStreamingError {
    fn from(error: SpiredStructuralPreparationError) -> Self {
        Self::StructuralPreparation(error)
    }
}

impl From<StratumRegistryError> for SpiredStreamingError {
    fn from(error: StratumRegistryError) -> Self {
        Self::Classification(error)
    }
}

impl From<SpiredModularError> for SpiredStreamingError {
    fn from(error: SpiredModularError) -> Self {
        Self::Modular(error)
    }
}
