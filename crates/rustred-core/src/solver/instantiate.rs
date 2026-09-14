use symbolica::domains::rational_polynomial::FromNumeratorAndDenominator;
use symbolica::prelude::{Integer, IntegerRing, Z};

use crate::algebra::{Coefficient, CoefficientPolynomial};

use super::{ExactRow, Integral, IntegralOrder, PolynomialRow, Seed, SolverError, Term};

pub(super) fn instantiate<const N: usize>(
    source: &PolynomialRow<N>,
    seed: &Seed<N>,
    indices: &[usize; N],
    fixed: &[Option<i16>; N],
    order: &IntegralOrder<N>,
    zero_sectors: &[[bool; N]],
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
        if !polynomial.is_zero() {
            row.push(Term {
                integral,
                coefficient: polynomial.into(),
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
    for (i, power) in integral.powers().iter().enumerate() {
        if !power.is_symbolic() {
            actual[i] = power.value() > 0;
        }
    }
    actual != *sector
        && (zero_sectors.contains(&actual)
            || actual
                .iter()
                .zip(order.deltas())
                .any(|(active, cut)| *cut && !*active))
}

pub(super) fn canonicalize<const N: usize>(
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

pub(super) fn translate<const N: usize>(
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
