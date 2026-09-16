//! Conservative integer contradictions on an affine locus intersected with a
//! box. Native Symbolica integers own the gcd, division and exact arithmetic;
//! this is domain bookkeeping, not a general integer feasibility solver.

use std::panic::{AssertUnwindSafe, catch_unwind};

use symbolica::prelude::Integer;

use crate::algebra::CoefficientPolynomial;
use crate::foundry::completion::LatticeBox;
use crate::solver::canonical_equalities;

use super::AffineApplicationDomain;

type Interval = (Option<Integer>, Option<Integer>);
const MAX_MATRIX_CELLS: usize = 65_536;
const MAX_POLYNOMIAL_CELLS: usize = 262_144;
const MAX_REFINED_FACES: usize = 8;

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
        let Some(template) = self.equations.first() else {
            return false;
        };
        if self.indices.iter().enumerate().any(|(axis, &index)| {
            index >= template.nvars() || self.indices[..axis].contains(&index)
        }) || self.equations.iter().any(|equation| {
            equation.variables() != template.variables()
                || equation.coefficients.len().checked_mul(equation.nvars())
                    != Some(equation.exponents.len())
        }) {
            return false;
        }
        let input_cells = self.equations.iter().try_fold(0usize, |total, equation| {
            total.checked_add(equation.exponents.len())
        });
        if input_cells.is_none_or(|cells| cells > MAX_POLYNOMIAL_CELLS)
            || self
                .equations
                .len()
                .checked_mul(cell.arity() + 1)
                .is_none_or(|cells| cells > MAX_MATRIX_CELLS)
        {
            return false;
        }
        // Admit the original equations before specialization: a nonlinear or
        // parameter-dependent equation must not accidentally acquire affine
        // proof authority merely because this particular box fixes an axis.
        let Some(rows) = self
            .equations
            .iter()
            .map(|equation| linear_row(equation, &self.indices))
            .collect::<Option<Vec<_>>>()
        else {
            return false;
        };
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
        self.proves_empty_without_split(&rows, &intervals)
            || self.bounded_face_refinement_proves_empty(&rows, &intervals)
    }

    fn proves_empty_without_split(&self, rows: &[Vec<Integer>], intervals: &[Interval]) -> bool {
        rows.iter().any(|row| excludes_box(row, intervals))
            || self.singleton_reduction_proves_empty(intervals)
    }

    /// Exhaust one short, equation-dependent finite axis, not a sample of
    /// the full box. Every resulting face must be proved empty while every
    /// other (possibly infinite) interval remains unchanged. No child calls
    /// this splitting entry again, so work cannot grow recursively.
    fn bounded_face_refinement_proves_empty(
        &self,
        rows: &[Vec<Integer>],
        intervals: &[Interval],
    ) -> bool {
        let selected = intervals
            .iter()
            .enumerate()
            .filter_map(|(axis, (lo, hi))| {
                let (Some(lo), Some(hi)) = (lo, hi) else {
                    return None;
                };
                if hi <= lo || !rows.iter().any(|row| !row[axis].is_zero()) {
                    return None;
                }
                // Convert only the small cardinality after exact subtraction;
                // endpoints themselves never narrow to a machine integer.
                let count = (hi - lo + Integer::one())
                    .to_i64()
                    .and_then(|count| usize::try_from(count).ok())?;
                (count <= MAX_REFINED_FACES).then_some((count, axis, lo))
            })
            .min_by_key(|&(count, axis, _)| (count, axis));
        let Some((count, axis, lower)) = selected else {
            return false;
        };
        let matrix_work = self
            .equations
            .len()
            .checked_add(self.fixed.iter().filter(|value| value.is_some()).count())
            .and_then(|rows| rows.checked_mul(self.indices.len() + 1))
            .and_then(|cells| cells.checked_mul(count));
        let polynomial_work = self
            .equations
            .iter()
            .try_fold(0usize, |total, equation| {
                total.checked_add(equation.exponents.len())
            })
            .and_then(|cells| cells.checked_mul(count));
        if matrix_work.is_none_or(|cells| cells > MAX_MATRIX_CELLS)
            || polynomial_work.is_none_or(|cells| cells > MAX_POLYNOMIAL_CELLS)
        {
            return false;
        }
        let mut face = intervals.to_vec();
        let mut value = lower.clone();
        for _ in 0..count {
            face[axis] = (Some(value.clone()), Some(value.clone()));
            if !self.proves_empty_without_split(rows, &face) {
                return false;
            }
            value += Integer::one();
        }
        true
    }

    /// A sign cell can fix an axis shared by several equations. Re-reduce
    /// those exact equations together so consequences of that fixed value
    /// reach the other rows. This is not general inequality propagation.
    fn singleton_reduction_proves_empty(&self, intervals: &[Interval]) -> bool {
        let singleton: Vec<_> = intervals
            .iter()
            .enumerate()
            .filter_map(|(axis, (lo, hi))| match (lo, hi, self.fixed[axis]) {
                (Some(lo), Some(hi), None) if lo == hi => Some((axis, lo)),
                _ => None,
            })
            .collect();
        if singleton.is_empty()
            || self
                .equations
                .len()
                .checked_add(self.fixed.iter().filter(|v| v.is_some()).count())
                .and_then(|rows| rows.checked_mul(self.indices.len() + 1))
                .is_none_or(|cells| cells > MAX_MATRIX_CELLS)
        {
            return false;
        }
        catch_unwind(AssertUnwindSafe(|| {
            let mut equations = self.equations.to_vec();
            for equation in &mut equations {
                for &(axis, value) in &singleton {
                    // Native arbitrary-size integers: neither i16 nor i64
                    // narrowing, even for the positive power u64::MAX+1.
                    *equation = equation.replace(self.indices[axis], value);
                }
            }
            match canonical_equalities(&self.fixed, &equations, &self.indices) {
                Ok(None) => true,
                Ok(Some((_, primitive))) => {
                    primitive.row_iter().any(|row| excludes_box(row, intervals))
                }
                Err(_) => false,
            }
        }))
        .unwrap_or(false)
    }
}

fn linear_row(equation: &CoefficientPolynomial, indices: &[usize]) -> Option<Vec<Integer>> {
    let mut row = vec![Integer::zero(); indices.len() + 1];
    for (term, coefficient) in equation.coefficients.iter().enumerate() {
        let mut variable = None;
        for (position, &power) in equation.exponents(term).iter().enumerate() {
            if power == 0 {
                continue;
            }
            if power != 1 || variable.is_some() {
                return None;
            }
            variable = Some(indices.iter().position(|&index| index == position)?);
        }
        match variable {
            Some(axis) => row[axis] += coefficient,
            None => row[indices.len()] -= coefficient,
        }
    }
    Some(row)
}

fn excludes_box(row: &[Integer], intervals: &[Interval]) -> bool {
    let mut rhs = row[intervals.len()].clone();
    let mut divisor = Integer::zero();
    let mut minimum = Some(Integer::zero());
    let mut maximum = Some(Integer::zero());
    for (coefficient, (lower, upper)) in row[..intervals.len()].iter().zip(intervals) {
        if coefficient.is_zero() {
            continue;
        }
        if let (Some(lo), Some(hi)) = (lower, upper)
            && lo == hi
        {
            rhs -= coefficient * lo;
            continue;
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

#[cfg(test)]
#[path = "box_bounds/joint_tests.rs"]
mod joint_tests;

#[cfg(test)]
#[path = "box_bounds/refinement_tests.rs"]
mod refinement_tests;
