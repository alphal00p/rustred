//! Sufficient affine sign exclusion on an actual local-coordinate box.
//!
//! Like solver/case/affine/bounds.rs, this is domain endpoint bookkeeping
//! over Symbolica Integer coefficients, not an equation/inequality solver.
//! The enclosing caller has already admitted/authenticated native input.
use crate::algebra::indexed::{BaseCoefficientSystem, ceil_log2, integer_magnitude_bits};
use crate::algebra::{IndexedAlgebraError, IndexedAlgebraLimits, IndexedGuardLimits};
use crate::foundry::completion::LatticeBox;
use symbolica::prelude::Integer;

use super::super::geometry::minimum_rank;

/// One nonvanishing coefficient equation suffices for the simultaneous system.
/// The rank simplex is only used to tighten individual inactive upper bounds;
/// the resulting box is an overcover, never an exact feasibility assertion.
pub(super) fn misses_zero<const N: usize>(
    system: &BaseCoefficientSystem,
    base_count: usize,
    cell: &LatticeBox,
    owner: &[bool; N],
    rank: Option<u32>,
    algebra: IndexedAlgebraLimits,
    limits: IndexedGuardLimits,
) -> Result<bool, IndexedAlgebraError> {
    let minimum_rank = minimum_rank(cell, owner);
    if rank.is_some_and(|r| minimum_rank > u128::from(r)) {
        return Ok(false); // the dispatch layer owns empty-domain elimination
    }
    let mut work = 0usize;
    for equation in system.equations() {
        let p = equation.index_polynomial().raw();
        // Two exponent traversals (recognition and endpoint selection) plus
        // one coefficient-bit scan. Charge before even inspecting a skipped
        // nonlinear row. Native input admission separately bounds the payload.
        let Some(scan_work) = p
            .nvars()
            .checked_mul(2)
            .and_then(|v| v.checked_add(1))
            .and_then(|v| v.checked_mul(p.nterms()))
        else {
            return Ok(false);
        };
        let Some(next_work) = work.checked_add(scan_work) else {
            return Ok(false);
        };
        if next_work > limits.max_gcd_factor_work {
            return Ok(false);
        }
        work = next_work;
        // Check TOTAL degree, including mixed monomials. No temporary native
        // polynomial, expression, matrix or coefficient copy is constructed.
        if p.nvars() != base_count + N
            || !p.exponents_iter().all(|powers| {
                powers[..base_count].iter().all(|&v| v == 0)
                    && powers[base_count..]
                        .iter()
                        .try_fold(0u16, |total, &v| {
                            if v > 1 || total + v > 1 {
                                None
                            } else {
                                Some(total + v)
                            }
                        })
                        .is_some()
            })
        {
            continue;
        }
        let max_bits = p
            .coefficients
            .iter()
            .map(integer_magnitude_bits)
            .max()
            .unwrap_or(0);
        let Some(bits) = usize::try_from(max_bits)
            .ok()
            .and_then(|b| b.checked_add(65)) // u64 endpoint plus active +1
            .and_then(|b| b.checked_add(ceil_log2(p.nterms())))
            .and_then(|b| b.checked_add(2))
        else {
            return Ok(false);
        };
        // Magnitude/work envelopes, not hidden GMP capacities or an RSS cap.
        // This optional precheck has its own bounded allowance; it neither
        // spends nor relaxes the unchanged native factor/replay allowance.
        // The optional optimization falls through if its extra arithmetic
        // cannot be admitted; no native operation has occurred for this row.
        let Some(payload) = bits.checked_mul(8) else {
            return Ok(false);
        };
        let Some(limbs) = bits
            .checked_add(usize::BITS as usize - 1)
            .map(|b| b / usize::BITS as usize)
        else {
            return Ok(false);
        };
        let Some(row_work) = limbs
            .checked_mul(limbs)
            .and_then(|v| v.checked_mul(8))
            .and_then(|v| v.checked_mul(p.nterms()))
        else {
            return Ok(false);
        };
        let Some(next_work) = work.checked_add(row_work) else {
            return Ok(false);
        };
        if bits > algebra.max_specialization_integer_bits
            || payload > limits.max_total_integer_bits
            || next_work > limits.max_gcd_factor_work
        {
            return Ok(false);
        }
        work = next_work;
        let excluded = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut lower = Some(Integer::zero());
            let mut upper = Some(Integer::zero());
            for (powers, coefficient) in p.exponents_iter().zip(&p.coefficients) {
                let Some(axis) = powers[base_count..].iter().position(|&v| v != 0) else {
                    if let Some(bound) = &mut lower {
                        *bound += coefficient;
                    }
                    if let Some(bound) = &mut upper {
                        *bound += coefficient;
                    }
                    continue;
                };
                let local_lower = cell.lower()[axis];
                let mut local_upper = cell.upper()[axis];
                if !owner[axis]
                    && let Some(rank) = rank
                {
                    // minimum_rank <= rank was checked above. The residual
                    // bound includes this coordinate's existing lower bound.
                    let cap =
                        u64::try_from(u128::from(rank) - (minimum_rank - u128::from(local_lower)))
                            .expect("rank fits u32");
                    local_upper = Some(local_upper.map_or(cap, |u| u.min(cap)));
                }
                // These exact physical endpoints fit i128 even at u64::MAX.
                // None remains mathematical infinity, never an i64 proxy.
                let (a, b) = if owner[axis] {
                    (
                        Some(i128::from(local_lower) + 1),
                        local_upper.map(|u| i128::from(u) + 1),
                    )
                } else {
                    (
                        local_upper.map(|u| -i128::from(u)),
                        Some(-i128::from(local_lower)),
                    )
                };
                let (a, b) = if coefficient.is_negative() {
                    (b, a)
                } else {
                    (a, b)
                };
                lower = lower
                    .zip(a)
                    .map(|(v, a)| v + coefficient * Integer::from(a));
                upper = upper
                    .zip(b)
                    .map(|(v, b)| v + coefficient * Integer::from(b));
            }
            lower.is_some_and(|v| v > Integer::zero()) || upper.is_some_and(|v| v < Integer::zero())
        }))
        .map_err(|_| {
            IndexedAlgebraError::Symbolica(
                "Symbolica panicked during affine guard box endpoint arithmetic".to_owned(),
            )
        })?;
        if excluded {
            return Ok(true);
        }
    }
    Ok(false)
}
