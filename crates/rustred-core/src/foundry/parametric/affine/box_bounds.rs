//! Conservative integer contradictions on an affine locus intersected with a
//! box. Native Symbolica integers own the gcd, division and exact arithmetic;
//! this is domain bookkeeping, not a general integer feasibility solver.

use symbolica::prelude::Integer;

use crate::foundry::completion::LatticeBox;

use super::AffineApplicationDomain;

impl AffineApplicationDomain {
    /// `true` proves the intersection empty. `false` is inconclusive.
    ///
    /// Read the defining polynomials, not the persisted cached matrix: a cold
    /// witness must not gain authority from an unchecked redundant matrix.
    /// Original powers remain integers; rational charts never relax that.
    pub(crate) fn is_proved_empty_in_box(&self, cell: &LatticeBox) -> bool {
        if cell.arity() != self.sector.len()
            || self.indices.len() != cell.arity()
            || self.fixed.len() != cell.arity()
        {
            return false;
        }
        let mut intervals = Vec::with_capacity(cell.arity());
        for axis in 0..cell.arity() {
            let lo = Integer::from(cell.lower()[axis]);
            let hi = cell.upper()[axis].map(Integer::from);
            let (mut lower, mut upper) = if self.sector[axis] {
                (Some(lo + Integer::one()), hi.map(|v| v + Integer::one()))
            } else {
                (hi.map(|v| -v), Some(-lo))
            };
            if let Some(value) = self.fixed[axis] {
                let value = Integer::from(value);
                if lower.as_ref().is_some_and(|lo| value < *lo)
                    || upper.as_ref().is_some_and(|hi| value > *hi)
                {
                    return true;
                }
                lower = Some(value.clone());
                upper = Some(value);
            }
            intervals.push((lower, upper));
        }
        self.equations.iter().any(|equation| {
            if equation.nvars() == 0
                || equation.coefficients.len().checked_mul(equation.nvars())
                    != Some(equation.exponents.len())
            {
                return false;
            }
            let mut rhs = Integer::zero();
            let mut coefficients = vec![Integer::zero(); cell.arity()];
            for (term, coefficient) in equation.coefficients.iter().enumerate() {
                let mut variable = None;
                for (position, &power) in equation.exponents(term).iter().enumerate() {
                    if power == 0 {
                        continue;
                    }
                    if power != 1 || variable.is_some() {
                        return false;
                    }
                    let Some(axis) = self.indices.iter().position(|v| *v == position) else {
                        // Generic parameters or nonlinear terms are unsupported.
                        return false;
                    };
                    variable = Some(axis);
                }
                match variable {
                    Some(axis) => coefficients[axis] += coefficient,
                    None => rhs -= coefficient,
                }
            }
            let mut divisor = Integer::zero();
            let mut minimum = Some(Integer::zero());
            let mut maximum = Some(Integer::zero());
            for (coefficient, (lower, upper)) in coefficients.iter().zip(&intervals) {
                if coefficient.is_zero() {
                    continue;
                }
                if let (Some(lo), Some(hi)) = (lower, upper) {
                    if lo == hi {
                        rhs -= coefficient * lo;
                        continue;
                    }
                }
                divisor = divisor.gcd(coefficient);
                let (lo, hi) = if coefficient.is_negative() {
                    (upper, lower)
                } else {
                    (lower, upper)
                };
                minimum = minimum.zip(lo.as_ref()).map(|(v, lo)| v + coefficient * lo);
                maximum = maximum.zip(hi.as_ref()).map(|(v, hi)| v + coefficient * hi);
            }
            if divisor.is_zero() {
                return !rhs.is_zero();
            }
            !rhs.quot_rem(&divisor).1.is_zero()
                || minimum.as_ref().is_some_and(|lo| rhs < *lo)
                || maximum.as_ref().is_some_and(|hi| rhs > *hi)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algebra::CoefficientContext;
    use crate::solver::{AffineCase, AffineIntersection, CoordinateCase};

    fn domain(equation: &str, sector: [bool; 2]) -> AffineApplicationDomain {
        // Include a parameter first to catch accidental axis/map conflation.
        let context = CoefficientContext::new(["d", "n0", "n1"]);
        let AffineIntersection::Affine(case) = AffineCase::from_coordinate(
            &CoordinateCase::generic(),
            &[context.coefficient_fixture(equation).numerator],
            &[1, 2],
            &sector,
        )
        .unwrap() else {
            panic!("expected coupled case")
        };
        AffineApplicationDomain::from_case(&case, &sector).unwrap()
    }

    #[test]
    fn excludes_half_integer_boundary_but_not_the_remaining_infinite_ray() {
        let domain = domain("1+n0-2*n1", [false, false]);
        let boundary = LatticeBox::try_new([0, 0], [Some(0), None]).unwrap();
        assert!(domain.is_proved_empty_in_box(&boundary));
        let ray = LatticeBox::try_new([1, 0], [None, None]).unwrap();
        assert!(!domain.is_proved_empty_in_box(&ray));
        assert!(domain.contains_powers(&[-1, 0]));
        assert!(domain.contains_powers(&[-2001, -1000]));
    }

    #[test]
    fn interval_bounds_and_integer_congruences_are_both_conservative() {
        let domain = domain("n0-n1", [true, true]);
        let separated = LatticeBox::try_new([0, 4], [Some(2), None]).unwrap();
        assert!(domain.is_proved_empty_in_box(&separated));
        let touching = LatticeBox::try_new([0, 2], [Some(2), None]).unwrap();
        assert!(!domain.is_proved_empty_in_box(&touching));
        // True infinity, and endpoints above signed machine range.
        let large = LatticeBox::try_new([u64::MAX, u64::MAX], [None, None]).unwrap();
        assert!(!domain.is_proved_empty_in_box(&large));
    }

    #[test]
    fn rationally_feasible_interval_still_needs_integer_divisibility() {
        let domain = domain("2*n0-n1", [false, false]);
        // n1=-1 gives n0=-1/2: within the real interval but not an index.
        assert!(
            domain.is_proved_empty_in_box(&LatticeBox::try_new([0, 1], [None, Some(1)]).unwrap())
        );
        assert!(
            !domain.is_proved_empty_in_box(&LatticeBox::try_new([0, 2], [None, Some(2)]).unwrap())
        );
    }

    #[test]
    fn contradictions_use_equations_not_mutated_cached_matrix() {
        let mut domain = domain("1+n0-2*n1", [false, false]);
        domain.primitive_matrix = None;
        assert!(
            domain.is_proved_empty_in_box(&LatticeBox::try_new([0, 0], [Some(0), None]).unwrap())
        );
        domain.equations = Box::new([]);
        assert!(
            !domain.is_proved_empty_in_box(&LatticeBox::try_new([0, 0], [Some(0), None]).unwrap())
        );
    }

    #[test]
    fn unsupported_equations_never_supply_contradiction_evidence() {
        let context = CoefficientContext::new(["d", "n0", "n1"]);
        let mut domain = domain("1+n0-2*n1", [false, false]);
        let boundary = LatticeBox::try_new([0, 0], [Some(0), None]).unwrap();
        for equation in ["1+n0-2*d*n1", "1+n0-n1^2"] {
            domain.equations = vec![context.coefficient_fixture(equation).numerator].into();
            assert!(!domain.is_proved_empty_in_box(&boundary));
        }
    }
}
