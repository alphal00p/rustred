//! Bounded exact restriction on finite faces of a genuinely unbounded box.
//!
//! This is exhaustive finite-lattice restriction, not sampling or
//! interpolation. Infinite axes remain symbolic. Numerator and denominator
//! are restricted separately, so a specialization to `0/0` is never erased
//! by rational cancellation. A nonzero symbolic denominator still carries
//! its original pole obligations: the calling owner must retain those guards.

use crate::algebra::{
    IndexedAlgebraLimits, IndexedCoefficient, IndexedCoefficientContext, IndexedPolynomial,
};
use crate::foundry::completion::{CompletionGeometryLimits, LatticeBox};

use super::super::{SourcePortAuditError, error};

pub(in crate::foundry::artifact::source_port) fn coefficient_vanishes(
    context: &IndexedCoefficientContext,
    coefficient: &IndexedCoefficient,
    cell: &LatticeBox,
    sector: &[bool],
    algebra: IndexedAlgebraLimits,
    geometry: CompletionGeometryLimits,
) -> Result<bool, SourcePortAuditError> {
    let arity = context.index_count();
    if cell.arity() != arity || sector.len() != arity {
        return Err(error(
            "bounded coefficient restriction has incompatible index geometry",
        ));
    }
    check_limit("arity", arity, geometry.max_arity)?;
    // Even an all-constant proof has one leaf. Check these before any native
    // specialization, including the no-finite-axis case.
    check_limit("finite leaves", 1, geometry.max_uncovered_boxes)?;
    check_limit(
        "finite leaf coordinates",
        arity,
        geometry.max_uncovered_box_coordinate_cells,
    )?;
    let bound = context
        .authenticate_coefficient_with_limits(coefficient, algebra.exact_algebra)
        .map_err(error)?;
    let mut numerator = context
        .numerator_condition_from_bound(bound)
        .map_err(error)?;
    let mut denominator = context
        .denominator_condition_from_bound(bound)
        .map_err(error)?;
    // Authenticated indexed contexts append the physical index variables to
    // the base map. Dependence checks do not construct or alter polynomials.
    let base_count = context.base().variables().len();

    let mut fixed = Vec::new();
    fixed.try_reserve_exact(arity).map_err(error)?;
    for axis in 0..arity {
        if cell.upper()[axis] == Some(cell.lower()[axis])
            && depends(&numerator, &denominator, base_count + axis)
        {
            // First restrict representable singleton faces together. Such a
            // face can annihilate dependence on another, unrepresentable
            // finite coordinate. That coordinate is rejected below only if
            // it is still needed after this exact native restriction.
            if let Some(value) = physical_value(cell.lower()[axis], sector[axis]) {
                fixed.push((axis, value));
            }
        }
    }
    check_limit(
        "singleton assignments",
        fixed.len(),
        geometry.max_split_operations,
    )?;
    if !fixed.is_empty() {
        numerator = specialize(context, &numerator, &fixed, algebra)?;
        denominator = specialize(context, &denominator, &fixed, algebra)?;
    }

    let mut finite = Vec::new();
    finite.try_reserve_exact(arity).map_err(error)?;
    let mut leaves = 1_usize;
    for axis in 0..arity {
        let Some(upper) = cell.upper()[axis] else {
            continue;
        };
        if !depends(&numerator, &denominator, base_count + axis) {
            continue;
        }
        let lower = cell.lower()[axis];
        if physical_value(lower, sector[axis]).is_none()
            || physical_value(upper, sector[axis]).is_none()
        {
            return Err(error(format!(
                "bounded coefficient restriction requires finite physical index {axis} outside i64"
            )));
        }
        let width = upper
            .checked_sub(lower)
            .and_then(|width| width.checked_add(1))
            .and_then(|width| usize::try_from(width).ok())
            .ok_or_else(|| {
                error("bounded coefficient restriction finite interval count overflow")
            })?;
        leaves = leaves
            .checked_mul(width)
            .ok_or_else(|| error("bounded coefficient restriction finite leaf count overflow"))?;
        check_limit("finite leaves", leaves, geometry.max_uncovered_boxes)?;
        finite.push((axis, lower, upper));
    }
    let coordinate_work = leaves
        .checked_mul(arity)
        .ok_or_else(|| error("bounded coefficient restriction finite coordinate count overflow"))?;
    check_limit(
        "finite leaf coordinates",
        coordinate_work,
        geometry.max_uncovered_box_coordinate_cells,
    )?;
    let assignment_work = leaves
        .checked_mul(finite.len())
        .and_then(|work| work.checked_add(fixed.len()))
        .ok_or_else(|| error("bounded coefficient restriction assignment count overflow"))?;
    check_limit(
        "finite assignments",
        assignment_work,
        geometry.max_split_operations,
    )?;

    if finite.is_empty() {
        return Ok(numerator.is_zero() && !denominator.is_zero());
    }
    // Preflight the complete Cartesian product above, then use a bounded
    // odometer instead of recursion on the caller-controlled number of axes.
    let mut local = Vec::new();
    local.try_reserve_exact(finite.len()).map_err(error)?;
    local.extend(finite.iter().map(|(_, lower, _)| *lower));
    fixed.clear();
    for leaf in 0..leaves {
        fixed.clear();
        for (position, &(axis, _, _)) in finite.iter().enumerate() {
            let value = physical_value(local[position], sector[axis]).ok_or_else(|| {
                error("bounded coefficient restriction escaped its finite i64 preflight")
            })?;
            fixed.push((axis, value));
        }
        let restricted_numerator = specialize(context, &numerator, &fixed, algebra)?;
        let restricted_denominator = specialize(context, &denominator, &fixed, algebra)?;
        if !restricted_numerator.is_zero() || restricted_denominator.is_zero() {
            return Ok(false);
        }
        if leaf + 1 < leaves {
            for position in (0..finite.len()).rev() {
                if local[position] < finite[position].2 {
                    local[position] += 1; // Strictly below the validated upper endpoint.
                    break;
                }
                local[position] = finite[position].1;
            }
        }
    }
    Ok(true)
}

fn depends(numerator: &IndexedPolynomial, denominator: &IndexedPolynomial, axis: usize) -> bool {
    numerator.raw().contains(axis) || denominator.raw().contains(axis)
}

fn physical_value(local: u64, positive: bool) -> Option<i64> {
    let value = if positive {
        1 + i128::from(local)
    } else {
        -i128::from(local)
    };
    i64::try_from(value).ok()
}

fn specialize(
    context: &IndexedCoefficientContext,
    polynomial: &IndexedPolynomial,
    fixed: &[(usize, i64)],
    limits: IndexedAlgebraLimits,
) -> Result<IndexedPolynomial, SourcePortAuditError> {
    context
        .specialize_fixed_polynomial_sealed(polynomial, fixed, limits)
        .map_err(error)
}

fn check_limit(resource: &str, requested: usize, limit: usize) -> Result<(), SourcePortAuditError> {
    if requested > limit {
        Err(error(format!(
            "bounded coefficient restriction {resource} exceeds geometry budget: {requested} > {limit}"
        )))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
