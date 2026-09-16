use symbolica::domains::rational_polynomial::FromNumeratorAndDenominator;
use symbolica::prelude::{Integer, IntegerRing, Z};

use crate::algebra::{Coefficient, CoefficientPolynomial};

use super::{
    AffineCase, ExactRow, Integral, IntegralOrder, PolynomialRow, Seed, SolverError, Term,
};

pub(crate) fn instantiate<const N: usize>(
    source: &PolynomialRow<N>,
    seed: &Seed<N>,
    indices: &[usize; N],
    fixed: &[Option<i16>; N],
    order: &IntegralOrder<N>,
    zero_sectors: &[[bool; N]],
    affine: Option<&AffineCase<N>>,
) -> Result<ExactRow<N>, SolverError> {
    // Prepared coordinates are absolute, and cannot be shifted or reopened
    // by a different case. Validate once even when this source row is empty.
    for (axis, value) in fixed.iter().enumerate() {
        if let Some(value) = value {
            if seed.integral[axis].is_symbolic()
                || seed.integral[axis].value() != *value
                || seed.shifts[axis] != 0
            {
                return Err(SolverError::InvalidInput(format!(
                    "seed is incompatible with prepared coordinate {axis} fixed to {value}"
                )));
            }
        }
    }
    let mut row = Vec::with_capacity(source.len());
    for term in source {
        let mut powers = *seed.integral.powers();
        for (axis, (power, offset)) in powers.iter_mut().zip(term.integral.powers()).enumerate() {
            *power = if fixed[axis].is_some() {
                // SourceSystem validated this numeric power before sector
                // preconditioning. In particular, fixed 1 plus seed 1 is 1.
                *offset
            } else {
                power.shifted(offset.value())?
            };
        }
        let integral = Integral::new(powers);
        if vanishes_in_subsector(&integral, order, zero_sectors) {
            continue;
        }
        let mut polynomial = term.coefficient.clone();
        for (i, variable) in indices.iter().enumerate() {
            if fixed[i].is_some() {
                continue;
            }
            if seed.integral[i].is_symbolic() {
                if seed.shifts[i] != 0 {
                    polynomial = polynomial.shift_var(*variable, &Integer::from(seed.shifts[i]));
                }
            } else {
                polynomial =
                    polynomial.replace(*variable, &Integer::from(seed.integral[i].value()));
            }
        }
        // All ordinary source translations remain admissible. Restrict the
        // shifted coefficients, never integral-key axes or the seed worklist.
        // Applying the chart before translation would erase necessary rows.
        let coefficient = if let Some(affine) = affine {
            affine.restrict_polynomial_value_validated(&polynomial)
        } else {
            polynomial.into()
        };
        if !coefficient.is_zero() {
            row.push(Term {
                integral,
                coefficient,
            });
        }
    }
    // Specializing coordinates can change both sector and tie-break ordering.
    row.sort_unstable_by(|a, b| order.compare(&a.integral, &b.integral));
    Ok(row)
}

fn vanishes_in_subsector<const N: usize>(
    integral: &Integral<N>,
    order: &IntegralOrder<N>,
    zero_sectors: &[[bool; N]],
) -> bool {
    let sector = order.sector();
    let mut actual = *sector;
    let mut symbolic = false;
    for (i, power) in integral.powers().iter().enumerate() {
        if power.is_symbolic() {
            symbolic = true;
        } else {
            actual[i] = power.value() > 0;
            // A fixed missing cut stays missing under every later symbolic
            // translation. This proof does not need the zero-sector census.
            if order.deltas()[i] && sector[i] && !actual[i] {
                return true;
            }
        }
    }
    let is_zero = |mask: &[bool; N]| {
        mask != sector
            && (zero_sectors.contains(mask)
                || mask
                    .iter()
                    .zip(order.deltas())
                    .any(|(active, cut)| *cut && !*active))
    };
    if !symbolic {
        return is_zero(&actual);
    }
    // Search subsequently recenters the winning target. A symbolic n_i can
    // therefore acquire an activating/pinching shift even when its current
    // displacement is zero. Pruning solely from the parent orthant loses an
    // identity on that boundary, and RHS guard extraction cannot see a term
    // which was already deleted. Only a sign-INDEPENDENT zero proof is safe
    // here without carrying separate source-projection obligations.
    //
    // Explicit membership, rather than monotonicity of the zero census, is
    // required: the generic source API does not promise downward closure.
    // This is a bounded optional optimization. If proving every support is
    // too expensive, retain the exact source column; never assume it zero.
    let mut remaining = 1024;
    every_symbolic_support_is_zero(integral, &mut actual, 0, &mut remaining, &is_zero)
}

fn every_symbolic_support_is_zero<const N: usize>(
    integral: &Integral<N>,
    actual: &mut [bool; N],
    from: usize,
    remaining: &mut usize,
    is_zero: &impl Fn(&[bool; N]) -> bool,
) -> bool {
    if *remaining == 0 {
        return false;
    }
    *remaining -= 1;
    let Some(axis) = (from..N).find(|&axis| integral[axis].is_symbolic()) else {
        return is_zero(actual);
    };
    if !every_symbolic_support_is_zero(integral, actual, axis + 1, remaining, is_zero) {
        return false;
    }
    actual[axis] = !actual[axis];
    let result = every_symbolic_support_is_zero(integral, actual, axis + 1, remaining, is_zero);
    actual[axis] = !actual[axis];
    result
}

#[cfg(test)]
mod projection_tests;

pub(crate) fn canonicalize<const N: usize>(
    row: ExactRow<N>,
    indices: &[usize; N],
) -> Result<(Integral<N>, ExactRow<N>), SolverError> {
    let head = row
        .first()
        .ok_or_else(|| SolverError::ExactReplay("empty winning row".into()))?;
    let shifts = std::array::from_fn(|i| {
        if head.integral[i].is_symbolic() {
            -head.integral[i].value()
        } else {
            0
        }
    });
    let target = head.integral.shifted(shifts)?;
    let pivot = translate(&head.coefficient, indices, &shifts);
    let mut rhs = Vec::with_capacity(row.len() - 1);
    for term in row.into_iter().skip(1) {
        rhs.push(Term {
            integral: term.integral.shifted(shifts)?,
            coefficient: -(&translate(&term.coefficient, indices, &shifts) / &pivot),
        });
    }
    Ok((target, rhs))
}

pub(crate) fn translate<const N: usize>(
    coefficient: &Coefficient,
    indices: &[usize; N],
    shifts: &[i16; N],
) -> Coefficient {
    fn shifted<const N: usize>(
        value: &CoefficientPolynomial,
        indices: &[usize; N],
        shifts: &[i16; N],
    ) -> CoefficientPolynomial {
        let mut result = value.clone();
        for (variable, shift) in indices.iter().zip(shifts) {
            if *shift != 0 {
                result = result.shift_var(*variable, &Integer::from(*shift));
            }
        }
        result
    }
    // An integral translation is an automorphism, so it preserves coprimality.
    <Coefficient as FromNumeratorAndDenominator<IntegerRing, IntegerRing, u16>>::from_num_den(
        shifted(&coefficient.numerator, indices, shifts),
        shifted(&coefficient.denominator, indices, shifts),
        &Z,
        false,
    )
}
