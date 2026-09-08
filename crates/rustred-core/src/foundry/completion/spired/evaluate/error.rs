use std::fmt;

/// Typed failure at the direct shifted-source evaluation boundary.
///
/// Vanishing conditions and term denominators denote singular modular samples.
/// They are retry outcomes, never exact algebraic or publication evidence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DirectShiftedSourceError {
    IncompleteOrdinarySourceLayout {
        actual: &'static str,
    },
    EmptySourceRows,
    CompletedSourceContextMismatch,
    RelationFamilyMismatch {
        source_ordinal: usize,
    },
    RelationContextMismatch {
        source_ordinal: usize,
    },
    ConditionContextMismatch {
        source_ordinal: usize,
        condition_ordinal: usize,
    },
    TermContextMismatch {
        source_ordinal: usize,
        term_ordinal: usize,
    },
    EmptySourceRelation {
        source_ordinal: usize,
    },
    UnsupportedEvenModulus {
        modulus: u64,
    },
    NonPrimeModulus {
        modulus: u64,
    },
    WrongBaseParameterArity {
        expected: usize,
        actual: usize,
    },
    WrongIndexPointArity {
        expected: usize,
        actual: usize,
    },
    NonCanonicalPointResidue {
        coordinate: usize,
        residue: u64,
        modulus: u64,
    },
    SourceOrdinalOutOfRange {
        source_ordinal: usize,
        source_count: usize,
    },
    WrongOffsetArity {
        expected: usize,
        actual: usize,
    },
    StructuralShiftOverflow {
        term_ordinal: usize,
        position: usize,
        offset: i64,
        source_shift: i64,
    },
    ConditionZero {
        source_ordinal: usize,
        condition_ordinal: usize,
    },
    TermDenominatorZero {
        source_ordinal: usize,
        term_ordinal: usize,
    },
    Backend {
        source_ordinal: usize,
        detail: String,
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

impl fmt::Display for DirectShiftedSourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IncompleteOrdinarySourceLayout { actual } => write!(
                formatter,
                "direct shifted evaluation needs complete ordinary sources, got {actual}"
            ),
            Self::EmptySourceRows => {
                formatter.write_str("direct shifted evaluation needs nonempty ordinary sources")
            }
            Self::CompletedSourceContextMismatch => formatter.write_str(
                "the completed ordinary source barrier uses a different indexed context",
            ),
            Self::RelationFamilyMismatch { source_ordinal } => write!(
                formatter,
                "ordinary source {source_ordinal} uses a different family scope"
            ),
            Self::RelationContextMismatch { source_ordinal } => write!(
                formatter,
                "ordinary source {source_ordinal} uses a different indexed context"
            ),
            Self::ConditionContextMismatch {
                source_ordinal,
                condition_ordinal,
            } => write!(
                formatter,
                "ordinary source {source_ordinal} condition {condition_ordinal} uses a different indexed context"
            ),
            Self::TermContextMismatch {
                source_ordinal,
                term_ordinal,
            } => write!(
                formatter,
                "ordinary source {source_ordinal} term {term_ordinal} uses a different indexed context"
            ),
            Self::EmptySourceRelation { source_ordinal } => {
                write!(
                    formatter,
                    "ordinary source {source_ordinal} is an empty relation"
                )
            }
            Self::UnsupportedEvenModulus { modulus } => write!(
                formatter,
                "direct shifted evaluation requires an odd prime, got even modulus {modulus}"
            ),
            Self::NonPrimeModulus { modulus } => write!(
                formatter,
                "direct shifted evaluation requires a prime modulus, got {modulus}"
            ),
            Self::WrongBaseParameterArity { expected, actual } => write!(
                formatter,
                "modular base point has {actual} parameter residues, expected {expected}"
            ),
            Self::WrongIndexPointArity { expected, actual } => write!(
                formatter,
                "modular base point has {actual} integral-index residues, expected {expected}"
            ),
            Self::NonCanonicalPointResidue {
                coordinate,
                residue,
                modulus,
            } => write!(
                formatter,
                "modular point coordinate {coordinate} has residue {residue} outside [0, {modulus})"
            ),
            Self::SourceOrdinalOutOfRange {
                source_ordinal,
                source_count,
            } => write!(
                formatter,
                "ordinary source ordinal {source_ordinal} is outside 0..{source_count}"
            ),
            Self::WrongOffsetArity { expected, actual } => write!(
                formatter,
                "translated-source offset has arity {actual}, expected {expected}"
            ),
            Self::StructuralShiftOverflow {
                term_ordinal,
                position,
                offset,
                source_shift,
            } => write!(
                formatter,
                "source term {term_ordinal} structural shift overflowed at position {position}: {offset} + {source_shift}"
            ),
            Self::ConditionZero {
                source_ordinal,
                condition_ordinal,
            } => write!(
                formatter,
                "ordinary source {source_ordinal} condition {condition_ordinal} vanishes at the shifted modular point"
            ),
            Self::TermDenominatorZero {
                source_ordinal,
                term_ordinal,
            } => write!(
                formatter,
                "ordinary source {source_ordinal} term {term_ordinal} has zero denominator at the shifted modular point"
            ),
            Self::Backend {
                source_ordinal,
                detail,
            } => write!(
                formatter,
                "could not evaluate ordinary source {source_ordinal} with the modular coefficient backend: {detail}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(
                    formatter,
                    "direct shifted {resource} count overflowed usize"
                )
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "direct shifted {resource} requires {requested}, exceeding the configured limit {limit}"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} entries for direct shifted {resource}"
            ),
            Self::Invariant { detail } => {
                write!(
                    formatter,
                    "direct shifted-source invariant failed: {detail}"
                )
            }
        }
    }
}

impl std::error::Error for DirectShiftedSourceError {}
