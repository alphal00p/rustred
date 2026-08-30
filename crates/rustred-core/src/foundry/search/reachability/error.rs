use std::fmt;

use crate::algebra::IndexedAlgebraError;
use crate::family::IntegralKeyError;
use crate::foundry::cell::RuleCellError;
use crate::sector::symmetry::CanonicalizationError;
use crate::sector::{self, OrderingPolicy};

/// Typed failure from bounded concrete reachability discovery.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReachabilityError {
    CanonicalizerArity {
        expected: usize,
        actual: usize,
    },
    CanonicalizerOrdering {
        expected: OrderingPolicy,
        actual: OrderingPolicy,
    },
    RuleCellArity {
        cell_ordinal: usize,
        expected: usize,
        actual: usize,
    },
    RuleCellOrdering {
        cell_ordinal: usize,
        expected: OrderingPolicy,
        actual: OrderingPolicy,
    },
    ForeignRuleCellContext {
        cell_ordinal: usize,
    },
    ForeignRuleCellFamily {
        cell_ordinal: usize,
    },
    RootArity {
        root_ordinal: usize,
        expected: usize,
        actual: usize,
    },
    IndexOverflow {
        position: usize,
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
    RuleCell(RuleCellError),
    IndexedAlgebra(IndexedAlgebraError),
    Ordering(sector::Error),
    Canonicalization(CanonicalizationError),
    IntegralKey(IntegralKeyError),
}

impl fmt::Display for ReachabilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CanonicalizerArity { expected, actual } => write!(
                formatter,
                "reachability canonicalizer has arity {actual}, expected {expected}"
            ),
            Self::CanonicalizerOrdering { expected, actual } => write!(
                formatter,
                "reachability canonicalizer uses ordering {}, expected {}",
                actual.stable_id(),
                expected.stable_id()
            ),
            Self::RuleCellArity {
                cell_ordinal,
                expected,
                actual,
            } => write!(
                formatter,
                "reachability rule cell {cell_ordinal} has arity {actual}, expected {expected}"
            ),
            Self::RuleCellOrdering {
                cell_ordinal,
                expected,
                actual,
            } => write!(
                formatter,
                "reachability rule cell {cell_ordinal} uses ordering {}, expected {}",
                actual.stable_id(),
                expected.stable_id()
            ),
            Self::ForeignRuleCellContext { cell_ordinal } => write!(
                formatter,
                "reachability rule cell {cell_ordinal} belongs to a different indexed coefficient context"
            ),
            Self::ForeignRuleCellFamily { cell_ordinal } => write!(
                formatter,
                "reachability rule cell {cell_ordinal} belongs to a different integral family"
            ),
            Self::RootArity {
                root_ordinal,
                expected,
                actual,
            } => write!(
                formatter,
                "reachability root {root_ordinal} has arity {actual}, expected {expected}"
            ),
            Self::IndexOverflow { position } => write!(
                formatter,
                "concrete rule-child arithmetic overflowed at index {position}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "reachability {resource} overflowed usize")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "reachability {resource} requires {requested} units, exceeding limit {limit}"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} bounded entries for reachability {resource}"
            ),
            Self::Invariant { detail } => {
                write!(formatter, "reachability invariant failed: {detail}")
            }
            Self::RuleCell(error) => error.fmt(formatter),
            Self::IndexedAlgebra(error) => error.fmt(formatter),
            Self::Ordering(error) => error.fmt(formatter),
            Self::Canonicalization(error) => error.fmt(formatter),
            Self::IntegralKey(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ReachabilityError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::RuleCell(error) => Some(error),
            Self::IndexedAlgebra(error) => Some(error),
            Self::Ordering(error) => Some(error),
            Self::Canonicalization(error) => Some(error),
            Self::IntegralKey(error) => Some(error),
            _ => None,
        }
    }
}

impl From<RuleCellError> for ReachabilityError {
    fn from(value: RuleCellError) -> Self {
        Self::RuleCell(value)
    }
}

impl From<IndexedAlgebraError> for ReachabilityError {
    fn from(value: IndexedAlgebraError) -> Self {
        Self::IndexedAlgebra(value)
    }
}

impl From<sector::Error> for ReachabilityError {
    fn from(value: sector::Error) -> Self {
        Self::Ordering(value)
    }
}

impl From<CanonicalizationError> for ReachabilityError {
    fn from(value: CanonicalizationError) -> Self {
        Self::Canonicalization(value)
    }
}

impl From<IntegralKeyError> for ReachabilityError {
    fn from(value: IntegralKeyError) -> Self {
        Self::IntegralKey(value)
    }
}
