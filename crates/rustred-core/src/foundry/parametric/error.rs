use std::fmt;

use crate::algebra::IndexedAlgebraError;
use crate::family::IntegralKeyError;
use crate::foundry::anchored::AnchoredRuleError;
use crate::sector;

/// Typed failure while preparing, eliminating, replaying, or anchoring one
/// parametric recurrence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParametricRuleError {
    EmptySourceRows,
    WrongAnchorArity {
        expected: usize,
        actual: usize,
    },
    WrongTargetShiftArity {
        expected: usize,
        actual: usize,
    },
    TargetShiftAbsent,
    TargetShiftNotPivot,
    TargetHasNoUniformlyDescendingRule,
    WrongSourceContext {
        source_ordinal: usize,
    },
    WrongSourceFamily {
        source_ordinal: usize,
    },
    IdenticallyZeroSourceCondition {
        source_ordinal: usize,
        condition_ordinal: usize,
    },
    AnchorOutsideInterior,
    DegenerateSinglePointInterior,
    ActivationLeakRequiresRefinement {
        right_hand_side_ordinal: usize,
        position: usize,
        shift: i64,
    },
    SectorMonotoneTermNotDescending {
        right_hand_side_ordinal: usize,
    },
    PointOutsideSectorMonotoneDomain,
    AnchorIndexOverflow {
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
    NativePanic {
        operation: &'static str,
    },
    ReducerRejectedChronologicalRow {
        source_ordinal: usize,
    },
    ReducerInvariant {
        detail: &'static str,
    },
    NoStrictlyDescendingRule,
    ReplayMismatch {
        shift_column: usize,
    },
    GuardVanishedAtAnchor {
        guard_ordinal: usize,
    },
    AnchorPivotMismatch,
    AnchorRightHandSideMismatch,
    AnchorSourceCombinationMismatch,
    IndexedAlgebra(IndexedAlgebraError),
    IntegralKey(IntegralKeyError),
    Ordering(sector::Error),
    Anchored(AnchoredRuleError),
}

impl fmt::Display for ParametricRuleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySourceRows => {
                formatter.write_str("parametric elimination needs at least one source row")
            }
            Self::WrongAnchorArity { expected, actual } => {
                write!(formatter, "anchor arity is {actual}, expected {expected}")
            }
            Self::WrongTargetShiftArity { expected, actual } => write!(
                formatter,
                "target shift arity is {actual}, expected {expected}"
            ),
            Self::TargetShiftAbsent => formatter.write_str(
                "the requested target shift is absent from the prepared physical columns",
            ),
            Self::TargetShiftNotPivot => formatter.write_str(
                "the requested target shift is not a pivot of the supplied source-row span",
            ),
            Self::TargetHasNoUniformlyDescendingRule => formatter.write_str(
                "the requested target pivot has no nonempty uniformly lower right-hand side after back-substitution",
            ),
            Self::WrongSourceContext { source_ordinal } => write!(
                formatter,
                "source row {source_ordinal} uses a foreign indexed coefficient context"
            ),
            Self::WrongSourceFamily { source_ordinal } => write!(
                formatter,
                "source row {source_ordinal} belongs to a different family"
            ),
            Self::IdenticallyZeroSourceCondition {
                source_ordinal,
                condition_ordinal,
            } => write!(
                formatter,
                "source row {source_ordinal} condition {condition_ordinal} is identically zero"
            ),
            Self::AnchorOutsideInterior => formatter.write_str(
                "the concrete anchor is outside the maximal representable fixed-sector interior",
            ),
            Self::DegenerateSinglePointInterior => formatter.write_str(
                "parametric derivation requires a sector interior containing more than one lattice point",
            ),
            Self::ActivationLeakRequiresRefinement {
                right_hand_side_ordinal,
                position,
                shift,
            } => write!(
                formatter,
                "sector-monotone RHS term {right_hand_side_ordinal} has positive inactive-line shift {shift} at position {position}; activation requires a refined piecewise domain"
            ),
            Self::SectorMonotoneTermNotDescending {
                right_hand_side_ordinal,
            } => write!(
                formatter,
                "sector-monotone RHS term {right_hand_side_ordinal} is not strictly lower on its nonempty same-sector cell"
            ),
            Self::PointOutsideSectorMonotoneDomain => formatter.write_str(
                "the concrete point is outside the recurrence's sector-monotone domain",
            ),
            Self::AnchorIndexOverflow { position } => write!(
                formatter,
                "the anchor plus rule shift overflows index position {position}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::ResourceLimit { resource, requested, limit } => write!(
                formatter,
                "{resource} needs {requested} units, exceeding the configured limit {limit}"
            ),
            Self::AllocationFailure { resource, requested } => {
                write!(formatter, "could not reserve {requested} units for {resource}")
            }
            Self::NativePanic { operation } => {
                write!(formatter, "Symbolica panicked while performing {operation}")
            }
            Self::ReducerRejectedChronologicalRow { source_ordinal } => write!(
                formatter,
                "Symbolica's sparse reducer rejected identity-augmented source row {source_ordinal}"
            ),
            Self::ReducerInvariant { detail } => {
                write!(formatter, "Symbolica sparse-reducer invariant failed: {detail}")
            }
            Self::NoStrictlyDescendingRule => formatter.write_str(
                "the parametric source rows contain no pivot with a nonempty uniformly lower right-hand side",
            ),
            Self::ReplayMismatch { shift_column } => write!(
                formatter,
                "exact indexed source-row replay differs at shift column {shift_column}"
            ),
            Self::GuardVanishedAtAnchor { guard_ordinal } => write!(
                formatter,
                "parametric nonzero guard {guard_ordinal} vanishes at the agreement anchor"
            ),
            Self::AnchorPivotMismatch => formatter.write_str(
                "the parametric pivot does not specialize to the independently derived anchored pivot",
            ),
            Self::AnchorRightHandSideMismatch => formatter.write_str(
                "the zero-pruned parametric right-hand side differs from the anchored rule",
            ),
            Self::AnchorSourceCombinationMismatch => formatter.write_str(
                "the parametric source combination differs from the anchored reducer chronology",
            ),
            Self::IndexedAlgebra(error) => error.fmt(formatter),
            Self::IntegralKey(error) => error.fmt(formatter),
            Self::Ordering(error) => error.fmt(formatter),
            Self::Anchored(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ParametricRuleError {}

impl From<IndexedAlgebraError> for ParametricRuleError {
    fn from(value: IndexedAlgebraError) -> Self {
        Self::IndexedAlgebra(value)
    }
}

impl From<IntegralKeyError> for ParametricRuleError {
    fn from(value: IntegralKeyError) -> Self {
        Self::IntegralKey(value)
    }
}

impl From<sector::Error> for ParametricRuleError {
    fn from(value: sector::Error) -> Self {
        Self::Ordering(value)
    }
}

impl From<AnchoredRuleError> for ParametricRuleError {
    fn from(value: AnchoredRuleError) -> Self {
        Self::Anchored(value)
    }
}
