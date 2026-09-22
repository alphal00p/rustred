//! Checked adapter for the existing saved native chart; no new algebra kernel.
use std::panic::{AssertUnwindSafe, catch_unwind};

use super::{Budget, OwnerAppliedFailure};
use crate::algebra::indexed::{ceil_log2, integer_magnitude_bits};
use crate::algebra::{
    CoefficientPolynomial, IndexedAlgebraLimits, IndexedCoefficient, IndexedCoefficientContext,
    IndexedPolynomial,
};
use crate::solver::{AffineCase, AffineGeometryError};

fn overflow() -> OwnerAppliedFailure {
    OwnerAppliedFailure::CountOverflow {
        resource: "affine restriction envelope",
    }
}

/// The support accounting is shared with AffineDomainRestriction. The bit
/// envelope adapts the Hadamard/common-denominator calculation in
/// foundry/artifact/source_port/lower/domain/guard.rs::restrict_prepared,
/// doubled for the two common-denominator scales of a rational coefficient.
/// These are substitution envelopes, NOT bounds on native GCD/quotient transient
/// growth or normalized quotient support (a sparse quotient can become dense).
/// The existing native-result admission independently validates final output.
fn preflight<const N: usize>(
    case: &AffineCase<N>,
    raw: &CoefficientPolynomial,
    limits: IndexedAlgebraLimits,
    budget: &Budget<'_>,
) -> Result<(usize, usize), OwnerAppliedFailure> {
    let (terms, _) = case
        .restriction_term_bound(raw)
        .map_err(OwnerAppliedFailure::AffineRestriction)?;
    let degree = raw.exponents_iter().try_fold(0usize, |maximum, powers| {
        powers
            .iter()
            .try_fold(0usize, |n, &p| {
                n.checked_add(usize::from(p)).ok_or_else(overflow)
            })
            .map(|degree| maximum.max(degree))
    })?;
    budget.check(
        degree,
        usize::from(limits.exact_algebra.max_exponent),
        "affine total degree",
    )?;
    budget.check(
        terms,
        limits.exact_algebra.max_polynomial_terms,
        "affine prospective terms",
    )?;
    let cells = terms.checked_mul(raw.nvars()).ok_or_else(overflow)?;
    let work = terms
        .checked_mul(terms.max(1))
        .and_then(|n| n.checked_mul(degree + 1))
        .and_then(|n| n.checked_add(raw.exponents.len()))
        .and_then(|n| n.checked_add(cells))
        .and_then(|n| n.checked_add(case.primitive_matrix().iter().len()))
        .ok_or_else(overflow)?;
    budget.check(
        work,
        limits.exact_algebra.max_term_operations,
        "affine prospective term operations",
    )?;
    let matrix_bits = case
        .primitive_matrix()
        .iter()
        .map(integer_magnitude_bits)
        .max()
        .unwrap_or(0);
    let chart_bits = usize::try_from(matrix_bits)
        .ok()
        .and_then(|n| n.checked_add(ceil_log2(N.max(1)) + 2))
        .and_then(|n| n.checked_mul(N.max(1)))
        .ok_or_else(overflow)?;
    let input_bits = usize::try_from(
        raw.coefficients
            .iter()
            .map(integer_magnitude_bits)
            .max()
            .unwrap_or(0),
    )
    .map_err(|_| overflow())?;
    let bits = chart_bits
        .checked_add(17 + ceil_log2(N + 1))
        .and_then(|n| n.checked_mul(degree))
        .and_then(|n| n.checked_add(input_bits))
        .and_then(|n| n.checked_add(ceil_log2(raw.nterms().max(1)) + 2))
        .and_then(|n| n.checked_mul(2))
        .ok_or_else(overflow)?;
    budget.check(
        bits,
        limits.max_specialization_integer_bits,
        "affine prospective integer bits",
    )?;
    budget.check(
        terms.checked_mul(bits).ok_or_else(overflow)?,
        budget.limits.matching.guard_algebra.max_total_integer_bits,
        "affine prospective total bits",
    )?;
    Ok((terms, bits))
}

pub(in crate::solver::candidate_reduction::owners::domains) fn coefficient<const N: usize>(
    context: &IndexedCoefficientContext,
    value: &IndexedCoefficient,
    fixed: &[(usize, i64)],
    affine: Option<&AffineCase<N>>,
    limits: IndexedAlgebraLimits,
    budget: &mut Budget<'_>,
) -> Result<IndexedCoefficient, OwnerAppliedFailure> {
    let restricted;
    let value = if let Some(case) = affine {
        let (nt, nb) = preflight(case, &value.raw().numerator, limits, budget)?;
        let (dt, db) = preflight(case, &value.raw().denominator, limits, budget)?;
        budget.check(
            nt.checked_add(dt)
                .and_then(|n| n.checked_mul(nb.max(db)))
                .ok_or_else(overflow)?,
            budget.limits.matching.guard_algebra.max_total_integer_bits,
            "affine joint prospective total bits",
        )?;
        budget.native()?;
        let raw = catch_unwind(AssertUnwindSafe(|| case.restrict_coefficient(value.raw())))
            .map_err(|_| {
                OwnerAppliedFailure::AffineRestriction(AffineGeometryError::NativeAlgebra)
            })?
            .map_err(OwnerAppliedFailure::AffineRestriction)?;
        budget.cancelled()?;
        restricted = context
            .admit_native_result_with_limits(raw, limits.exact_algebra)
            .map_err(OwnerAppliedFailure::Algebra)?;
        &restricted
    } else {
        value
    };
    context
        .specialize_fixed_indices_sealed(value, fixed, limits)
        .map(|(value, _)| value)
        .map_err(OwnerAppliedFailure::Algebra)
}

/// Only zero-locus/source-condition predicates use this scalar-normalizing API.
/// A child source condition must already have been translated by +shift.
pub(in crate::solver::candidate_reduction::owners::domains) fn polynomial<const N: usize>(
    context: &IndexedCoefficientContext,
    value: &IndexedPolynomial,
    fixed: &[(usize, i64)],
    affine: Option<&AffineCase<N>>,
    limits: IndexedAlgebraLimits,
    budget: &mut Budget<'_>,
) -> Result<IndexedPolynomial, OwnerAppliedFailure> {
    let restricted;
    let value = if let Some(case) = affine {
        preflight(case, value.raw(), limits, budget)?;
        budget.native()?;
        let raw = catch_unwind(AssertUnwindSafe(|| case.restrict_equation(value.raw())))
            .map_err(|_| {
                OwnerAppliedFailure::AffineRestriction(AffineGeometryError::NativeAlgebra)
            })?
            .map_err(OwnerAppliedFailure::AffineRestriction)?;
        budget.cancelled()?;
        restricted = context
            .admit_native_polynomial_result_with_limits(raw, limits.exact_algebra)
            .map_err(OwnerAppliedFailure::Algebra)?;
        &restricted
    } else {
        value
    };
    context
        .specialize_fixed_polynomial_sealed(value, fixed, limits)
        .map_err(OwnerAppliedFailure::Algebra)
}
