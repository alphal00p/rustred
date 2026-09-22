//! Thin bounded calls into the existing native indexed algebra/zero-locus API.
use super::super::scan::minimum_rank;
use super::{engine::Budget, model::*};
use crate::algebra::IndexedAlgebraLimits;
use crate::algebra::indexed::IntegerZeroLocusDomainResolution;
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

pub(super) fn coefficient<const N: usize>(
    context: &IndexedCoefficientContext,
    value: &IndexedCoefficient,
    cell: &LatticeBox,
    owner: &[bool; N],
    rank: Option<u32>,
    algebra: IndexedAlgebraLimits,
    budget: &mut Budget<'_>,
) -> Result<Zero, OwnerAppliedFailure> {
    if value.is_zero() {
        return Ok(Zero::Yes);
    }
    budget.native()?;
    let numerator = context
        .numerator_condition_with_limits(value, algebra.exact_algebra)
        .map_err(OwnerAppliedFailure::Algebra)?;
    polynomial(context, &numerator, cell, owner, rank, algebra, budget)
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
