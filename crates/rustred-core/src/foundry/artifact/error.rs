use std::fmt;

use crate::algebra::{CoefficientContextError, IndexedAlgebraError};
use crate::family::{IntegralFamilyError, IntegralKeyError};
use crate::foundry::parametric::ParametricRuleError;
use crate::identity::{ParametricIbpError, ParametricRelationError};
use crate::sector;

use super::model::ArtifactSchemaVersion;

/// Typed failure while generating or sealing a closing artifact.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArtifactError {
    CoefficientContext(CoefficientContextError),
    Family(IntegralFamilyError),
    Identity(ParametricIbpError),
    ParametricRule(ParametricRuleError),
    Relation(ParametricRelationError),
    IndexedAlgebra(IndexedAlgebraError),
    IntegralKey(IntegralKeyError),
    Ordering(sector::Error),
    UnsupportedSchema { actual: u32 },
    WrongFamily,
    WrongCoefficientContext,
    WrongArity { expected: usize, actual: usize },
    InvalidMasterManifest,
    InvalidZeroTerminal,
    UnsupportedClosureShape,
    InvalidRuleShape { detail: &'static str },
    InvalidDescentWitness { right_hand_side_ordinal: usize },
    UnprovedGuardApplicability { guard_ordinal: usize },
    InvalidReplayEvidence { detail: &'static str },
}

impl fmt::Display for ArtifactError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CoefficientContext(error) => error.fmt(formatter),
            Self::Family(error) => error.fmt(formatter),
            Self::Identity(error) => error.fmt(formatter),
            Self::ParametricRule(error) => error.fmt(formatter),
            Self::Relation(error) => error.fmt(formatter),
            Self::IndexedAlgebra(error) => error.fmt(formatter),
            Self::IntegralKey(error) => error.fmt(formatter),
            Self::Ordering(error) => error.fmt(formatter),
            Self::UnsupportedSchema { actual } => {
                write!(formatter, "artifact schema version {actual} is unsupported")
            }
            Self::WrongFamily => {
                formatter.write_str("artifact evidence belongs to a different integral family")
            }
            Self::WrongCoefficientContext => {
                formatter.write_str("artifact evidence belongs to a different coefficient context")
            }
            Self::WrongArity { expected, actual } => {
                write!(formatter, "artifact arity is {actual}, expected {expected}")
            }
            Self::InvalidMasterManifest => {
                formatter.write_str("artifact master-terminal manifest is invalid")
            }
            Self::InvalidZeroTerminal => {
                formatter.write_str("artifact zero-sector terminal is invalid or unproved")
            }
            Self::UnsupportedClosureShape => formatter
                .write_str("no installed closure verifier supports this artifact candidate shape"),
            Self::InvalidRuleShape { detail } => {
                write!(formatter, "invalid one-loop closing rule: {detail}")
            }
            Self::InvalidDescentWitness {
                right_hand_side_ordinal,
            } => write!(
                formatter,
                "artifact rule RHS term {right_hand_side_ordinal} has invalid strict-descent evidence"
            ),
            Self::UnprovedGuardApplicability { guard_ordinal } => write!(
                formatter,
                "artifact rule guard {guard_ordinal} is not proved nonzero on the complete rule domain"
            ),
            Self::InvalidReplayEvidence { detail } => {
                write!(formatter, "invalid exact source replay evidence: {detail}")
            }
        }
    }
}

impl std::error::Error for ArtifactError {}

macro_rules! artifact_from {
    ($source:ty, $variant:ident) => {
        impl From<$source> for ArtifactError {
            fn from(value: $source) -> Self {
                Self::$variant(value)
            }
        }
    };
}

artifact_from!(CoefficientContextError, CoefficientContext);
artifact_from!(IntegralFamilyError, Family);
artifact_from!(ParametricIbpError, Identity);
artifact_from!(ParametricRuleError, ParametricRule);
artifact_from!(ParametricRelationError, Relation);
artifact_from!(IndexedAlgebraError, IndexedAlgebra);
artifact_from!(IntegralKeyError, IntegralKey);
artifact_from!(sector::Error, Ordering);

/// Durable encoding is intentionally not claimed by the first in-process
/// artifact slice.  Callers receive this typed boundary instead of bytes that
/// could not yet be authenticated on a subsequent load.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArtifactPersistenceError {
    DurableEncodingUnavailable { schema: ArtifactSchemaVersion },
}

impl fmt::Display for ArtifactPersistenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DurableEncodingUnavailable { schema } => write!(
                formatter,
                "durable encoding is not yet available for closing artifact schema {}",
                schema.as_u32()
            ),
        }
    }
}

impl std::error::Error for ArtifactPersistenceError {}
