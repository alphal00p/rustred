use std::fmt;

use crate::foundry::completion::frame::modular::ModularSourceEvaluationError;
use crate::identity::TranslatedSourceError;

/// Typed failures at the bounded inverse-incidence boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SourceDiscoveryError {
    WrongSourceLayout {
        actual: &'static str,
    },
    ScopeMismatch {
        detail: &'static str,
    },
    WrongArity {
        object: &'static str,
        expected: usize,
        actual: usize,
    },
    ShiftOverflow {
        support_ordinal: usize,
        source_ordinal: usize,
        term_ordinal: usize,
        position: usize,
        support: i64,
        source_shift: i64,
    },
    ShiftConstruction(TranslatedSourceError),
    SourceTranslation(TranslatedSourceError),
    NominationIncidenceMismatch,
    TargetUnitNominationForObstruction,
    NominationObstructionMismatch,
    CompletedSourceChronologyMismatch,
    SelectedRequestProvenanceMismatch {
        candidate_ordinal: usize,
    },
    SelectedSourceRowMismatch {
        candidate_ordinal: usize,
        source_ordinal: usize,
    },
    ObstructionPlanMismatch,
    ObstructionSampleMismatch,
    CandidateEvaluation {
        candidate_ordinal: usize,
        source_ordinal: usize,
        error: ModularSourceEvaluationError,
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
    Invariant {
        detail: &'static str,
    },
}

impl fmt::Display for SourceDiscoveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongSourceLayout { actual } => write!(
                formatter,
                "source discovery requires the complete ordinary IBP source layout, got {actual}"
            ),
            Self::ScopeMismatch { detail } => {
                write!(formatter, "source-discovery scope mismatch: {detail}")
            }
            Self::WrongArity {
                object,
                expected,
                actual,
            } => write!(
                formatter,
                "source-discovery {object} has arity {actual}, expected {expected}"
            ),
            Self::ShiftOverflow {
                support_ordinal,
                source_ordinal,
                term_ordinal,
                position,
                support,
                source_shift,
            } => write!(
                formatter,
                "inverse incidence {support}-{source_shift} overflowed at support {support_ordinal}, source {source_ordinal}, term {term_ordinal}, component {position}"
            ),
            Self::ShiftConstruction(error) => {
                write!(
                    formatter,
                    "could not retain an incident translation offset: {error}"
                )
            }
            Self::SourceTranslation(error) => {
                write!(
                    formatter,
                    "could not translate residual source candidates: {error}"
                )
            }
            Self::NominationIncidenceMismatch => formatter.write_str(
                "residual nominations were constructed by a different ordinary-source incidence index",
            ),
            Self::TargetUnitNominationForObstruction => formatter.write_str(
                "target-unit bootstrap nominations cannot be paired with a checked obstruction",
            ),
            Self::NominationObstructionMismatch => formatter.write_str(
                "residual nominations belong to a different checked obstruction query",
            ),
            Self::CompletedSourceChronologyMismatch => formatter.write_str(
                "completed source chronology differs from the sealed incidence module",
            ),
            Self::SelectedRequestProvenanceMismatch { candidate_ordinal } => write!(
                formatter,
                "selected residual candidate {candidate_ordinal} disagrees with its request provenance"
            ),
            Self::SelectedSourceRowMismatch {
                candidate_ordinal,
                source_ordinal,
            } => write!(
                formatter,
                "selected residual candidate {candidate_ordinal} has the wrong sealed row identity for ordinary source {source_ordinal}"
            ),
            Self::ObstructionPlanMismatch => formatter.write_str(
                "residual pairing frame is not the physical plan bound to its obstruction",
            ),
            Self::ObstructionSampleMismatch => formatter.write_str(
                "residual pairing frame is not the modular sample bound to its obstruction",
            ),
            Self::CandidateEvaluation {
                candidate_ordinal,
                source_ordinal,
                error,
            } => write!(
                formatter,
                "residual candidate {candidate_ordinal} (ordinary source {source_ordinal}) could not be completely evaluated: {error:?}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} needs {requested} units, exceeding the configured limit {limit}"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} units for {resource}"
            ),
            Self::Invariant { detail } => {
                write!(formatter, "source-discovery invariant failed: {detail}")
            }
        }
    }
}

impl std::error::Error for SourceDiscoveryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ShiftConstruction(error) | Self::SourceTranslation(error) => Some(error),
            _ => None,
        }
    }
}
