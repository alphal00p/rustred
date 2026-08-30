use std::fmt;

use crate::algebra::{CoefficientContextError, IndexedAlgebraError};
use crate::family::{IntegralFamilyError, IntegralKeyError};
use crate::foundry::cell::RuleCellError;
use crate::foundry::parametric::ParametricRuleError;
use crate::foundry::search::SectorSearchError;
use crate::identity::{ParametricIbpError, ParametricRelationError, TranslatedSourceError};
use crate::sector;
use crate::sector::symmetry::{CanonicalizationError, permutation};

/// Typed failure while generating or sealing a closing artifact.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArtifactError {
    CoefficientContext(CoefficientContextError),
    Family(IntegralFamilyError),
    Identity(ParametricIbpError),
    ParametricRule(ParametricRuleError),
    SectorSearch(SectorSearchError),
    RuleCell(RuleCellError),
    Relation(ParametricRelationError),
    TranslatedSource(TranslatedSourceError),
    Symmetry(sector::symmetry::Error),
    SymmetryPermutation(permutation::Error),
    Canonicalization(CanonicalizationError),
    IndexedAlgebra(IndexedAlgebraError),
    IntegralKey(IntegralKeyError),
    Ordering(sector::Error),
    ZeroAnalysis(sector::zero::Error),
    UnsupportedSchema { actual: u32 },
    WrongFamily,
    WrongCoefficientContext,
    WrongArity { expected: usize, actual: usize },
    InvalidMasterManifest,
    InvalidZeroTerminal,
    InvalidFactorization { detail: &'static str },
    InvalidCanonicalizer,
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
            Self::SectorSearch(error) => error.fmt(formatter),
            Self::RuleCell(error) => error.fmt(formatter),
            Self::Relation(error) => error.fmt(formatter),
            Self::TranslatedSource(error) => error.fmt(formatter),
            Self::Symmetry(error) => error.fmt(formatter),
            Self::SymmetryPermutation(error) => error.fmt(formatter),
            Self::Canonicalization(error) => error.fmt(formatter),
            Self::IndexedAlgebra(error) => error.fmt(formatter),
            Self::IntegralKey(error) => error.fmt(formatter),
            Self::Ordering(error) => error.fmt(formatter),
            Self::ZeroAnalysis(error) => error.fmt(formatter),
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
            Self::InvalidFactorization { detail } => {
                write!(formatter, "invalid artifact factorization: {detail}")
            }
            Self::InvalidCanonicalizer => {
                formatter.write_str("artifact canonicalizer has a foreign arity or ordering")
            }
            Self::UnsupportedClosureShape => formatter
                .write_str("no installed closure verifier supports this artifact candidate shape"),
            Self::InvalidRuleShape { detail } => {
                write!(formatter, "invalid closing rule: {detail}")
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
artifact_from!(SectorSearchError, SectorSearch);
artifact_from!(RuleCellError, RuleCell);
artifact_from!(ParametricRelationError, Relation);
artifact_from!(TranslatedSourceError, TranslatedSource);
artifact_from!(sector::symmetry::Error, Symmetry);
artifact_from!(permutation::Error, SymmetryPermutation);
artifact_from!(CanonicalizationError, Canonicalization);
artifact_from!(IndexedAlgebraError, IndexedAlgebra);
artifact_from!(IntegralKeyError, IntegralKey);
artifact_from!(sector::Error, Ordering);
artifact_from!(sector::zero::Error, ZeroAnalysis);

/// Typed failure at the deterministic durable-artifact boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArtifactPersistenceError {
    InvalidMagic,
    UnsupportedSchema {
        actual: u32,
    },
    InvalidSection {
        expected: u16,
        actual: u16,
    },
    Truncated {
        offset: usize,
    },
    TrailingBytes {
        remaining: usize,
    },
    InvalidUtf8 {
        field: &'static str,
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
    InvalidCoefficient {
        field: &'static str,
    },
    NonCanonicalCoefficient {
        field: &'static str,
    },
    UnsupportedFeature {
        detail: &'static str,
    },
    SemanticMismatch {
        field: &'static str,
    },
    Artifact(ArtifactError),
}

impl fmt::Display for ArtifactPersistenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidMagic => formatter.write_str("invalid RustRed closing-artifact magic"),
            Self::UnsupportedSchema { actual } => {
                write!(
                    formatter,
                    "durable artifact schema version {actual} is unsupported"
                )
            }
            Self::InvalidSection { expected, actual } => write!(
                formatter,
                "durable artifact section {actual} occurred where section {expected} was required"
            ),
            Self::Truncated { offset } => {
                write!(formatter, "durable artifact is truncated at byte {offset}")
            }
            Self::TrailingBytes { remaining } => {
                write!(formatter, "durable artifact has {remaining} trailing bytes")
            }
            Self::InvalidUtf8 { field } => {
                write!(formatter, "durable artifact {field} is not valid UTF-8")
            }
            Self::ResourceCountOverflow { resource } => {
                write!(
                    formatter,
                    "durable artifact {resource} count overflowed usize"
                )
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "durable artifact {resource} requires {requested} units, limit is {limit}"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} units for durable artifact {resource}"
            ),
            Self::InvalidCoefficient { field } => {
                write!(
                    formatter,
                    "durable artifact {field} is not an exact coefficient"
                )
            }
            Self::NonCanonicalCoefficient { field } => write!(
                formatter,
                "durable artifact {field} is not in canonical sparse Symbolica form"
            ),
            Self::UnsupportedFeature { detail } => {
                write!(
                    formatter,
                    "durable artifact uses an unsupported feature: {detail}"
                )
            }
            Self::SemanticMismatch { field } => {
                write!(
                    formatter,
                    "durable artifact {field} failed exact replay comparison"
                )
            }
            Self::Artifact(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ArtifactPersistenceError {}

impl From<ArtifactError> for ArtifactPersistenceError {
    fn from(value: ArtifactError) -> Self {
        Self::Artifact(value)
    }
}
