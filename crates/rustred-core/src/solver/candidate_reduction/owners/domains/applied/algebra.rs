//! Thin bounded calls into the existing native indexed algebra/zero-locus API.
use super::super::scan::minimum_rank;
use super::{engine::Budget, model::*};
use crate::algebra::indexed::IntegerZeroLocusDomainResolution;
use crate::algebra::{IndexedAlgebraError, IndexedAlgebraLimits};
use crate::algebra::{IndexedCoefficient, IndexedCoefficientContext, IndexedPolynomial};
use crate::foundry::completion::LatticeBox;
use symbolica::prelude::Integer;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Zero {
    Yes,
    No,
    Unknown,
}
impl Zero {
    pub fn nonzero(self) -> OwnerAppliedNonzero {
        if self == Self::No {
            OwnerAppliedNonzero::Uniform
        } else {
            OwnerAppliedNonzero::Conditional
        }
    }
}

#[derive(Debug)]
pub(super) struct CoefficientClassification {
    pub zero: Zero,
    pub optional_refusal: Option<IndexedAlgebraError>,
}

pub(super) fn coefficient<const N: usize>(
    context: &IndexedCoefficientContext,
    value: &IndexedCoefficient,
    cell: &LatticeBox,
    owner: &[bool; N],
    rank: Option<u32>,
    algebra: IndexedAlgebraLimits,
    budget: &mut Budget<'_>,
) -> Result<CoefficientClassification, OwnerAppliedFailure> {
    if value.is_zero() {
        return Ok(CoefficientClassification {
            zero: Zero::Yes,
            optional_refusal: None,
        });
    }
    budget.native()?;
    let numerator = context
        .numerator_condition_with_limits(value, algebra.exact_algebra)
        .map_err(OwnerAppliedFailure::Algebra)?;
    // Numerator/input admission stays strict. Only the optional zero-locus
    // strengthening can decline the next native operation. Base splitting
    // has distinct input/equation IDs, excluded by the shared closed policy.
    match polynomial(context, &numerator, cell, owner, rank, algebra, budget) {
        Ok(zero) => Ok(CoefficientClassification {
            zero,
            optional_refusal: None,
        }),
        Err(OwnerAppliedFailure::Algebra(error))
            if crate::algebra::indexed::is_native_guard_preflight_refusal(&error) =>
        {
            Ok(CoefficientClassification {
                zero: Zero::Unknown,
                optional_refusal: Some(error),
            })
        }
        Err(error) => Err(error),
    }
}

pub(super) fn polynomial<const N: usize>(
    context: &IndexedCoefficientContext,
    value: &IndexedPolynomial,
    cell: &LatticeBox,
    owner: &[bool; N],
    rank: Option<u32>,
    algebra: IndexedAlgebraLimits,
    budget: &mut Budget<'_>,
) -> Result<Zero, OwnerAppliedFailure> {
    if value.is_zero() {
        return Ok(Zero::Yes);
    }
    if value.is_nonzero_constant() {
        return Ok(Zero::No);
    }
    budget.native()?;
    let system = context
        .base_coefficient_system(value, algebra, budget.limits.matching.guard_algebra)
        .map_err(OwnerAppliedFailure::Algebra)?;
    budget.native()?;
    let result = context
        .integer_zero_locus_domain_resolution(
            &system,
            budget.limits.matching.guard_algebra,
            |axis, root| {
                let local = if owner[axis] {
                    root - &Integer::from(1)
                } else {
                    -root.clone()
                };
                if local < Integer::from(cell.lower()[axis])
                    || cell.upper()[axis].is_some_and(|u| local > Integer::from(u))
                {
                    return false;
                }
                if !owner[axis]
                    && let Some(rank) = rank
                {
                    let others = minimum_rank(cell.lower(), owner) - u128::from(cell.lower()[axis]);
                    if others > u128::from(rank) || local > Integer::from(u128::from(rank) - others)
                    {
                        return false;
                    }
                }
                true
            },
        )
        .map_err(OwnerAppliedFailure::Algebra)?;
    Ok(match result {
        IntegerZeroLocusDomainResolution::IdenticallyZero => Zero::Yes,
        IntegerZeroLocusDomainResolution::MissesDomain => Zero::No,
        _ => Zero::Unknown,
    })
}
