//! Necessary affine consequences of a native coefficient factorization.
//!
//! Factor zero sets are alternatives, never a conjunction. Exactly one
//! unresolved factor may become a necessary equality after every other
//! alternative has been certified outside the actual admitted domain.

use super::*;

pub(super) enum Consequence {
    Nonzero,
    Affine(IndexedPolynomial),
    Unresolved,
}

pub(super) fn classify(
    context: &IndexedCoefficientContext,
    coefficient: &IndexedPolynomial,
    predicates: &[Vec<IndexedPolynomial>],
    domain: &conjunction::GuardDomain<'_>,
    limits: RuleCellLimits,
    work: &mut Work,
    factor_work: &mut usize,
) -> Result<Consequence, SourcePortAuditError> {
    let factors = context
        .factor_guard_coefficient_with_limits(
            coefficient,
            limits.indexed_algebra,
            limits.guard_algebra,
            factor_work,
        )
        .map_err(error)?;
    let mut remaining = None;
    for factor in factors {
        if misses_target(
            context,
            &factor,
            domain.piece,
            domain.sector,
            domain.target,
            limits,
            work,
        )? || covered_by_whole_exclusion(&factor, predicates, limits, work)?
        {
            continue;
        }
        // A second unresolved alternative, or a nonlinear one, cannot supply
        // a necessary affine equation. Returning inconclusive here never
        // claims that unvisited factors are covered.
        if remaining.is_some() || !is_index_affine(factor.raw(), context.base().variables().len()) {
            return Ok(Consequence::Unresolved);
        }
        remaining = Some(factor);
    }
    Ok(match remaining {
        Some(equation) => Consequence::Affine(equation),
        None => Consequence::Nonzero,
    })
}

fn covered_by_whole_exclusion(
    factor: &IndexedPolynomial,
    predicates: &[Vec<IndexedPolynomial>],
    limits: RuleCellLimits,
    work: &mut Work,
) -> Result<bool, SourcePortAuditError> {
    for equations in predicates {
        if equations.is_empty() {
            continue;
        }
        let mut implied = true;
        for equation in equations {
            work.charge(
                equation
                    .raw()
                    .nterms()
                    .checked_add(factor.raw().nterms())
                    .ok_or_else(|| error("affine guard implication work overflow"))?,
                limits,
            )?;
            // The fixed-face gate was checked before constructing predicates.
            // Both operands are in the same current chart. Every equation of
            // ONE exclusion must vanish on this factor; never mix exclusions.
            if !equation.is_zero()
                && std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    equation.raw().try_div(factor.raw())
                }))
                .map_err(|_| error("native affine guard implication division panicked"))?
                .is_none()
            {
                implied = false;
                break;
            }
        }
        if implied {
            return Ok(true);
        }
    }
    Ok(false)
}
