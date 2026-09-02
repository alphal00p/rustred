use std::fmt;

use crate::algebra::IndexedAlgebraError;
use crate::foundry::cell::RuleCellError;
use crate::foundry::parametric::ParametricRuleError;
use crate::identity::ParametricRelationError;
use crate::sector;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ExactCircuitLoweringError {
    WrongPhysicalPlan,
    ClearedCircuitMismatch,
    WrongContext,
    WrongAnchorArity {
        expected: usize,
        actual: usize,
    },
    AnchorOutsideMonotoneAdmission,
    TargetJoin(&'static str),
    ResidualJoin {
        term: usize,
        detail: &'static str,
    },
    SourceJoin {
        row: usize,
        detail: &'static str,
    },
    PivotJoin {
        pivot: usize,
        detail: &'static str,
    },
    GuardOriginJoin {
        guard: usize,
        origin: usize,
        detail: &'static str,
    },
    EmptyRightHandSide,
    NoCommonSectorInterior,
    ReplayMismatch {
        physical_column: usize,
    },
    ReplayWitnessMismatch(&'static str),
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
    IndexedAlgebra(IndexedAlgebraError),
    Relation(ParametricRelationError),
    Parametric(ParametricRuleError),
    Cell(RuleCellError),
    Sector(sector::Error),
    Invariant(&'static str),
}

impl fmt::Display for ExactCircuitLoweringError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongPhysicalPlan => {
                formatter.write_str("exact circuit belongs to another physical plan")
            }
            Self::ClearedCircuitMismatch => {
                formatter.write_str("fraction-free certificate belongs to another exact circuit")
            }
            Self::WrongContext => {
                formatter.write_str("exact-circuit lowering uses another indexed context")
            }
            Self::WrongAnchorArity { expected, actual } => write!(
                formatter,
                "lowering anchor arity is {actual}, expected {expected}"
            ),
            Self::AnchorOutsideMonotoneAdmission => formatter.write_str(
                "lowering anchor lies outside the retained sector-monotone admission domain",
            ),
            Self::TargetJoin(detail) => write!(
                formatter,
                "exact lowering target failed its plan join: {detail}"
            ),
            Self::ResidualJoin { term, detail } => write!(
                formatter,
                "exact lowering residual {term} failed its plan join: {detail}"
            ),
            Self::SourceJoin { row, detail } => write!(
                formatter,
                "exact lowering source row {row} failed its plan join: {detail}"
            ),
            Self::PivotJoin { pivot, detail } => write!(
                formatter,
                "exact lowering pivot {pivot} failed its plan join: {detail}"
            ),
            Self::GuardOriginJoin {
                guard,
                origin,
                detail,
            } => write!(
                formatter,
                "exact lowering guard {guard} origin {origin} failed its plan join: {detail}"
            ),
            Self::EmptyRightHandSide => formatter.write_str(
                "a target-only zero circuit cannot be lowered into a descending ParametricRule",
            ),
            Self::NoCommonSectorInterior => formatter
                .write_str("the exact circuit has no nonempty common same-sector rule interior"),
            Self::ReplayMismatch { physical_column } => write!(
                formatter,
                "lowered source-span replay differs at physical column {physical_column}"
            ),
            Self::ReplayWitnessMismatch(detail) => write!(
                formatter,
                "lowered replay witness differs from the exact circuit: {detail}"
            ),
            Self::ResourceCountOverflow { resource } => write!(
                formatter,
                "exact-circuit lowering {resource} count overflowed usize"
            ),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "exact-circuit lowering {resource} needs {requested}, exceeding limit {limit}"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} entries for exact-circuit lowering {resource}"
            ),
            Self::IndexedAlgebra(error) => error.fmt(formatter),
            Self::Relation(error) => error.fmt(formatter),
            Self::Parametric(error) => error.fmt(formatter),
            Self::Cell(error) => error.fmt(formatter),
            Self::Sector(error) => error.fmt(formatter),
            Self::Invariant(detail) => write!(
                formatter,
                "exact-circuit lowering invariant failed: {detail}"
            ),
        }
    }
}

impl std::error::Error for ExactCircuitLoweringError {}

impl From<IndexedAlgebraError> for ExactCircuitLoweringError {
    fn from(value: IndexedAlgebraError) -> Self {
        Self::IndexedAlgebra(value)
    }
}
impl From<ParametricRelationError> for ExactCircuitLoweringError {
    fn from(value: ParametricRelationError) -> Self {
        Self::Relation(value)
    }
}
impl From<ParametricRuleError> for ExactCircuitLoweringError {
    fn from(value: ParametricRuleError) -> Self {
        Self::Parametric(value)
    }
}
impl From<RuleCellError> for ExactCircuitLoweringError {
    fn from(value: RuleCellError) -> Self {
        Self::Cell(value)
    }
}
impl From<sector::Error> for ExactCircuitLoweringError {
    fn from(value: sector::Error) -> Self {
        Self::Sector(value)
    }
}
