use std::fmt;

use crate::algebra::IndexedAlgebraError;
use crate::foundry::completion::stratum::StratumRegistryError;
use crate::sector;

/// Invalid joins or failed exact operations at the guard-case boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SpiredCoordinateGuardCaseError {
    WrongContext,
    ClearedCircuitMismatch,
    ClearedTargetMismatch,
    CircuitParentMismatch,
    RefinementParentMismatch,
    CircuitFixedDomainMismatch,
    GuardedParentCase {
        guard_branches: usize,
    },
    RefinementShapeMismatch {
        detail: &'static str,
    },
    GuardOrdinalOutOfRange {
        ordinal: usize,
        available: usize,
    },
    DuplicateGuardOrdinal {
        ordinal: usize,
    },
    UnreferencedGuardOrdinal {
        ordinal: usize,
    },
    GuardIdentityMismatch {
        required_predicate_ordinal: usize,
        guard_ordinal: usize,
    },
    RootCoordinateOutOfRange {
        position: usize,
        arity: usize,
    },
    RootNotRepresentable {
        position: usize,
    },
    IdentityCollision,
    IndexedAlgebra(IndexedAlgebraError),
    Stratum(StratumRegistryError),
    Sector(sector::Error),
    Invariant {
        detail: &'static str,
    },
}

impl fmt::Display for SpiredCoordinateGuardCaseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongContext => formatter.write_str(
                "SpIReD guard cases use a different indexed coefficient context",
            ),
            Self::ClearedCircuitMismatch => formatter
                .write_str("SpIReD guard cases received a foreign cleared exact circuit"),
            Self::ClearedTargetMismatch => formatter.write_str(
                "SpIReD guard cases received inconsistent cleared and exact target columns",
            ),
            Self::CircuitParentMismatch => formatter
                .write_str("SpIReD guard cases received a foreign exact-circuit parent stratum"),
            Self::RefinementParentMismatch => formatter
                .write_str("SpIReD guard cases received a foreign guard-refinement parent stratum"),
            Self::CircuitFixedDomainMismatch => formatter.write_str(
                "SpIReD guard cases received circuit fixed indices inconsistent with the parent stratum",
            ),
            Self::GuardedParentCase { guard_branches } => write!(
                formatter,
                "SpIReD equality-only guard discovery received a parent with {guard_branches} guard branches"
            ),
            Self::RefinementShapeMismatch { detail } => {
                write!(formatter, "invalid exact guard-refinement shape: {detail}")
            }
            Self::GuardOrdinalOutOfRange { ordinal, available } => write!(
                formatter,
                "cleared semantic guard ordinal {ordinal} is outside {available} guards"
            ),
            Self::DuplicateGuardOrdinal { ordinal } => write!(
                formatter,
                "cleared semantic guard ordinal {ordinal} occurs in multiple required predicates"
            ),
            Self::UnreferencedGuardOrdinal { ordinal } => write!(
                formatter,
                "cleared semantic guard ordinal {ordinal} occurs in no required predicate"
            ),
            Self::GuardIdentityMismatch {
                required_predicate_ordinal,
                guard_ordinal,
            } => write!(
                formatter,
                "required predicate {required_predicate_ordinal} does not match cleared semantic guard {guard_ordinal}"
            ),
            Self::RootCoordinateOutOfRange { position, arity } => write!(
                formatter,
                "exact guard root uses coordinate {position}, outside arity {arity}"
            ),
            Self::RootNotRepresentable { position } => write!(
                formatter,
                "exact guard root on coordinate {position} is outside RustRed's i64 carrier"
            ),
            Self::IdentityCollision => formatter.write_str(
                "two unequal guard-blind discovery cases carry the same exact stratum identity",
            ),
            Self::IndexedAlgebra(error) => error.fmt(formatter),
            Self::Stratum(error) => error.fmt(formatter),
            Self::Sector(error) => error.fmt(formatter),
            Self::Invariant { detail } => {
                write!(formatter, "SpIReD guard-case invariant failed: {detail}")
            }
        }
    }
}

impl std::error::Error for SpiredCoordinateGuardCaseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::IndexedAlgebra(error) => Some(error),
            Self::Stratum(error) => Some(error),
            Self::Sector(error) => Some(error),
            _ => None,
        }
    }
}
