use super::geometry::{Budget, charge, minimum_rank};
use super::model::OwnerDomainMatchFailure;
use crate::algebra::indexed::IntegerZeroLocusDomainResolution;
use crate::algebra::{IndexedAlgebraLimits, IndexedCoefficientContext, IndexedPolynomial};
use crate::foundry::completion::LatticeBox;
use symbolica::prelude::Integer;

/// Closed native resource identifiers audited in indexed/base_coefficients.rs.
/// These are admission checks before the NEXT GCD/factor operation, not proof
/// that no native work has happened: a prior GCD/equation may already have run.
/// Output/input/replay caps and all non-ResourceLimit faults remain hard errors.
pub(super) fn permits_bounded_refinement(failure: &OwnerDomainMatchFailure) -> bool {
    matches!(
        failure,
        OwnerDomainMatchFailure::Algebra(crate::algebra::IndexedAlgebraError::ResourceLimit {
            resource: "guard univariate degree"
                | "guard gcd/factor work"
                | "guard factor variables"
                | "guard factor per-variable degree"
                | "guard factor total degree"
                | "guard factor dense slots"
                | "guard factor recombination subsets"
                | "guard prospective factor terms"
                | "guard prospective factor integer bits"
                | "guard separable factor work",
            ..
        })
    )
}

pub(super) enum Resolution {
    Zero,
    Nonzero,
    Unknown,
    Planes {
        roots: Vec<(usize, u64)>,
        exact: bool,
    },
}

pub(super) fn resolve<const N: usize>(
    context: &IndexedCoefficientContext,
    polynomial: &IndexedPolynomial,
    cell: &LatticeBox,
    owner: &[bool; N],
    rank: Option<u32>,
    algebra: IndexedAlgebraLimits,
    budget: &mut Budget,
) -> Result<Resolution, OwnerDomainMatchFailure> {
    charge(
        &mut budget.stats.predicates,
        1,
        budget.limits.max_predicates,
        "predicates",
    )?;
    budget.cells::<N>(1)?; // bounded fixed-index scratch
    let mut fixed = Vec::new();
    fixed
        .try_reserve_exact(N)
        .map_err(|_| OwnerDomainMatchFailure::AllocationFailure {
            resource: "fixed index scratch",
        })?;
    for axis in 0..N {
        if cell.upper()[axis] == Some(cell.lower()[axis]) {
            let value = if owner[axis] {
                i128::from(cell.lower()[axis]) + 1
            } else {
                -i128::from(cell.lower()[axis])
            };
            // The public partial-specialization API has an i64 carrier. Never
            // truncate a larger mathematical fixed coordinate or call it empty.
            let Ok(value) = i64::try_from(value) else {
                return Ok(Resolution::Unknown);
            };
            fixed.push((axis, value));
        }
    }
    let restricted = context
        .specialize_fixed_polynomial_sealed(polynomial, &fixed, algebra)
        .map_err(OwnerDomainMatchFailure::Algebra)?;
    let system = context
        .base_coefficient_system(&restricted, algebra, budget.limits.guard_algebra)
        .map_err(OwnerDomainMatchFailure::Algebra)?;
    let resolution = context
        .integer_zero_locus_domain_resolution(&system, budget.limits.guard_algebra, |axis, root| {
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
                let other = minimum_rank(cell, owner) - u128::from(cell.lower()[axis]);
                if other > u128::from(rank) || local > Integer::from(u128::from(rank) - other) {
                    return false;
                }
            }
            true
        })
        .map_err(OwnerDomainMatchFailure::Algebra)?;
    let (planes, exact) = match resolution {
        IntegerZeroLocusDomainResolution::IdenticallyZero => return Ok(Resolution::Zero),
        IntegerZeroLocusDomainResolution::MissesDomain => return Ok(Resolution::Nonzero),
        IntegerZeroLocusDomainResolution::UnsupportedCoupled => return Ok(Resolution::Unknown),
        IntegerZeroLocusDomainResolution::IntersectsExactHyperplanes(p) => (p, true),
        IntegerZeroLocusDomainResolution::IntersectsConservativeCover(p) => (p, false),
    };
    budget.cells::<N>(planes.len())?; // conservative root-list allowance
    let mut roots = Vec::new();
    roots.try_reserve_exact(planes.len()).map_err(|_| {
        OwnerDomainMatchFailure::AllocationFailure {
            resource: "guard root cuts",
        }
    })?;
    for plane in &planes {
        let axis = plane.index_position();
        let local = if owner[axis] {
            plane.root() - &Integer::from(1)
        } else {
            -plane.root().clone()
        };
        let Ok(local) = u64::try_from(local) else {
            return Ok(Resolution::Unknown);
        };
        // An unbounded interval above u64::MAX has no representable lower
        // endpoint in BoxCover; refuse the entire split rather than lose it.
        if local == u64::MAX && cell.upper()[axis].is_none() {
            return Ok(Resolution::Unknown);
        }
        roots.push((axis, local));
    }
    roots.sort_unstable();
    roots.dedup();
    Ok(Resolution::Planes { roots, exact })
}
