use std::fmt;

/// Typed failures at the private streaming modular boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SpiredModularError {
    UnsupportedEvenModulus {
        modulus: u64,
    },
    NonPrimeModulus {
        modulus: u64,
    },
    NonCanonicalResidue {
        row: usize,
        term: Option<usize>,
        residue: u64,
        modulus: u64,
    },
    DuplicateForbiddenTerm {
        row: usize,
        first_term: usize,
        second_term: usize,
    },
    NonCanonicalForbiddenColumnBatch {
        first_column: usize,
        second_column: usize,
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
    U32NotRepresentable {
        resource: &'static str,
        value: usize,
    },
    NativePanic {
        operation: &'static str,
    },
    Poisoned,
    AlreadyHit,
    Invariant {
        detail: &'static str,
    },
}

impl fmt::Display for SpiredModularError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedEvenModulus { modulus } => write!(
                formatter,
                "streaming modular reduction requires an odd prime, got even modulus {modulus}"
            ),
            Self::NonPrimeModulus { modulus } => write!(
                formatter,
                "streaming modular reduction requires a prime modulus, got {modulus}"
            ),
            Self::NonCanonicalResidue {
                row,
                term,
                residue,
                modulus,
            } => match term {
                Some(term) => write!(
                    formatter,
                    "streamed row {row} forbidden term {term} has residue {residue} outside [0, {modulus})"
                ),
                None => write!(
                    formatter,
                    "streamed row {row} target has residue {residue} outside [0, {modulus})"
                ),
            },
            Self::DuplicateForbiddenTerm {
                row,
                first_term,
                second_term,
            } => write!(
                formatter,
                "streamed row {row} repeats one forbidden column at term positions {first_term} and {second_term}"
            ),
            Self::NonCanonicalForbiddenColumnBatch {
                first_column,
                second_column,
            } => write!(
                formatter,
                "pre-registered forbidden-column batch is not strictly ordered at positions {first_column} and {second_column}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(
                    formatter,
                    "streaming modular {resource} count overflowed usize"
                )
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "streaming modular {resource} requires {requested}, exceeding the configured limit {limit}"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} entries for streaming modular {resource}"
            ),
            Self::U32NotRepresentable { resource, value } => write!(
                formatter,
                "streaming modular {resource} value {value} does not fit Symbolica's u32 carrier"
            ),
            Self::NativePanic { operation } => {
                write!(formatter, "Symbolica panicked while {operation}")
            }
            Self::Poisoned => formatter.write_str(
                "streaming modular lane is poisoned by a prior partial native operation",
            ),
            Self::AlreadyHit => formatter.write_str(
                "streaming modular lane already reached its first checked target-rank gain",
            ),
            Self::Invariant { detail } => {
                write!(formatter, "streaming modular invariant failed: {detail}")
            }
        }
    }
}

impl std::error::Error for SpiredModularError {}
