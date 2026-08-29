//! Typed family-presentation authentication failures.

use std::fmt;

use crate::algebra::{Coefficient, ExactAlgebraError};

use super::model::PresentationConditionSource;

/// Location of caller-supplied exact data in a presentation.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PresentationCoefficientLocation {
    RoutingLoopLinear {
        row: usize,
        column: usize,
    },
    RoutingLoopExternal {
        row: usize,
        column: usize,
    },
    RoutingExternalLinear {
        row: usize,
        column: usize,
    },
    PhysicalLoopCoefficient {
        denominator: usize,
        loop_index: usize,
    },
    PhysicalExternalShift {
        denominator: usize,
        external: usize,
    },
    PhysicalMassSquared {
        denominator: usize,
    },
    CommonMassScaleSquared,
}

/// Affine component that failed exact physical-propagator replay.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PresentationDenominatorComponent {
    Constant,
    ScalarProduct { coordinate: usize },
}

/// Failure to authenticate a caller-owned family presentation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FamilyPresentationError {
    WrongDenominatorRoleCount {
        expected: usize,
        actual: usize,
    },
    EmptyDenominatorId {
        denominator: usize,
    },
    DuplicateDenominatorId {
        denominator: usize,
        id: String,
    },
    WrongRoutingOrderCount {
        momentum: &'static str,
        expected: usize,
        actual: usize,
    },
    EmptyRoutingLabel {
        momentum: &'static str,
        index: usize,
    },
    DuplicateRoutingLabel {
        momentum: &'static str,
        label: String,
    },
    RoutingLabelOverlap {
        label: String,
    },
    WrongRoutingRowCount {
        matrix: &'static str,
        expected: usize,
        actual: usize,
    },
    WrongRoutingColumnCount {
        matrix: &'static str,
        row: usize,
        expected: usize,
        actual: usize,
    },
    WrongPhysicalMomentumArity {
        denominator: usize,
        momentum: &'static str,
        expected: usize,
        actual: usize,
    },
    PhysicalMomentumHasNoLoopComponent {
        denominator: usize,
    },
    InvalidCoefficient {
        location: PresentationCoefficientLocation,
        error: ExactAlgebraError,
    },
    NonUnimodularLoopRouting {
        determinant: Coefficient,
    },
    SingularExternalRouting,
    PhysicalDenominatorMismatch {
        denominator: usize,
        component: PresentationDenominatorComponent,
    },
    ZeroCommonMassScale,
    CommonMassScaleWithoutPhysicalDenominators,
    CommonMassScaleUnused,
    PhysicalMassOutsideCommonScale {
        denominator: usize,
    },
    ZeroNonZeroCondition {
        source: PresentationConditionSource,
    },
    ExactAlgebra(ExactAlgebraError),
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    RoutingMatrixFailure {
        detail: String,
    },
}

impl fmt::Display for FamilyPresentationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongDenominatorRoleCount { expected, actual } => write!(
                formatter,
                "received {actual} denominator roles for a family with {expected} rows"
            ),
            Self::EmptyDenominatorId { denominator } => {
                write!(formatter, "denominator role {denominator} has an empty ID")
            }
            Self::DuplicateDenominatorId { denominator, id } => write!(
                formatter,
                "denominator role {denominator} repeats ID {id:?}"
            ),
            Self::WrongRoutingOrderCount {
                momentum,
                expected,
                actual,
            } => write!(
                formatter,
                "routing source {momentum} order contains {actual} labels, expected {expected}"
            ),
            Self::EmptyRoutingLabel { momentum, index } => write!(
                formatter,
                "routing source {momentum} momentum {index} has an empty label"
            ),
            Self::DuplicateRoutingLabel { momentum, label } => write!(
                formatter,
                "routing source {momentum} momentum label {label:?} is repeated"
            ),
            Self::RoutingLabelOverlap { label } => write!(
                formatter,
                "routing source label {label:?} is both loop and external"
            ),
            Self::WrongRoutingRowCount {
                matrix,
                expected,
                actual,
            } => write!(
                formatter,
                "routing matrix {matrix} has {actual} rows, expected {expected}"
            ),
            Self::WrongRoutingColumnCount {
                matrix,
                row,
                expected,
                actual,
            } => write!(
                formatter,
                "routing matrix {matrix} row {row} has {actual} columns, expected {expected}"
            ),
            Self::WrongPhysicalMomentumArity {
                denominator,
                momentum,
                expected,
                actual,
            } => write!(
                formatter,
                "physical denominator {denominator} has {actual} {momentum} coefficients, expected {expected}"
            ),
            Self::PhysicalMomentumHasNoLoopComponent { denominator } => write!(
                formatter,
                "physical denominator {denominator} has no loop-momentum component"
            ),
            Self::InvalidCoefficient { location, error } => {
                write!(
                    formatter,
                    "invalid presentation coefficient at {location:?}: {error}"
                )
            }
            Self::NonUnimodularLoopRouting { determinant } => write!(
                formatter,
                "loop routing determinant {determinant} is not +1 or -1"
            ),
            Self::SingularExternalRouting => {
                formatter.write_str("external routing matrix is identically singular")
            }
            Self::PhysicalDenominatorMismatch {
                denominator,
                component,
            } => write!(
                formatter,
                "physical denominator {denominator} does not replay at {component:?}"
            ),
            Self::ZeroCommonMassScale => {
                formatter.write_str("the claimed common mass scale is identically zero")
            }
            Self::CommonMassScaleWithoutPhysicalDenominators => formatter
                .write_str("a common mass scale was supplied without a physical denominator"),
            Self::CommonMassScaleUnused => formatter
                .write_str("the claimed common mass scale is not used by any physical denominator"),
            Self::PhysicalMassOutsideCommonScale { denominator } => write!(
                formatter,
                "physical denominator {denominator} has a mass other than zero or the claimed common scale"
            ),
            Self::ZeroNonZeroCondition { source } => write!(
                formatter,
                "presentation condition source {source:?} produced the zero polynomial"
            ),
            Self::ExactAlgebra(error) => error.fmt(formatter),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} needs {requested} units, exceeding the configured limit {limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} bounded units for {resource}"
            ),
            Self::RoutingMatrixFailure { detail } => {
                write!(
                    formatter,
                    "exact routing-matrix authentication failed: {detail}"
                )
            }
        }
    }
}

impl std::error::Error for FamilyPresentationError {}

impl From<ExactAlgebraError> for FamilyPresentationError {
    fn from(value: ExactAlgebraError) -> Self {
        Self::ExactAlgebra(value)
    }
}
