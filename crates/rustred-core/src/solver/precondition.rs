//! SpIRed's source-row preconditioning over native integer polynomials.
//!
//! This is the GCD-scaled forward/backward cancellation in
//! `vendor/spired/src/solver.tpp::rowReduce`, including its asymmetric forward
//! subtraction sign. It preserves the row span over the fraction field, but
//! need not preserve the polynomial row module or the rank at exceptional
//! parameter values where an elimination multiplier vanishes. Keep the source
//! equations when such specializations must be recovered.
//!
//! Unlike the reference's unchecked `getLeading()` on an empty row, both
//! passes explicitly skip zero rows, including dependencies found in elimination.

use std::cmp::Ordering;

use crate::algebra::CoefficientPolynomial;

use super::{IntegralOrder, PolynomialRow, Term};

/// Precondition canonical source rows without introducing rational functions.
///
/// Rows must have distinct nonzero terms sorted by `order`; coefficients must
/// share one ordered variable map. Empty/dependent rows are retained at the
/// end. Coefficient ties use SpIRed's degree-lexicographic comparison in the
/// native variable-map order. For the reference family order (indices, then
/// dimension, then scalar parameters), use [`precondition_with_variable_order`]
/// if the native map uses a different order.
///
/// This preserves the generic fraction-field span, not every specialized rank.
/// It deliberately does not divide row content or make pivots monic.
///
/// # Panics
/// Panics if coefficient variable maps differ.
pub fn precondition<const N: usize>(
    rows: Vec<PolynomialRow<N>>,
    order: &IntegralOrder<N>,
) -> Vec<PolynomialRow<N>> {
    let nvars = rows
        .iter()
        .flatten()
        .next()
        .map_or(0, |term| term.coefficient.nvars());
    precondition_with_variable_order(rows, order, &(0..nvars).collect::<Vec<_>>())
}

/// Precondition with a degree-lexicographic variable priority permutation.
///
/// `variables` lists native variable positions from highest to lowest priority.
/// It changes comparison and GCD sign conventions without changing coefficient
/// variables or their native order. Allocation and sharing of variable maps
/// remain under Symbolica's control. SpIRed uses `[n1, ..., nN, d, scalars...]`.
/// The canonical-row and generic-span contract is the same as [`precondition`].
///
/// # Panics
/// Panics if coefficient variable maps differ or `variables` is not a
/// permutation of their positions. With only empty rows, the result is returned
/// unchanged because no coefficient map is available to validate.
pub fn precondition_with_variable_order<const N: usize>(
    mut rows: Vec<PolynomialRow<N>>,
    order: &IntegralOrder<N>,
    variables: &[usize],
) -> Vec<PolynomialRow<N>> {
    let Some(first) = rows.iter().flatten().next() else {
        return rows;
    };
    let map = first.coefficient.variables();
    let mut sorted_variables = variables.to_vec();
    sorted_variables.sort_unstable();
    assert!(
        sorted_variables.iter().copied().eq(0..map.len()),
        "precondition variable order must be a permutation of the coefficient map"
    );
    for row in &rows {
        for term in row {
            assert_eq!(
                term.coefficient.variables(),
                map,
                "precondition coefficients must share one ordered variable map"
            );
            debug_assert!(!term.coefficient.is_zero());
        }
        debug_assert!(
            row.windows(2).all(|pair| {
                order.compare(&pair[0].integral, &pair[1].integral) == Ordering::Less
            })
        );
    }
    let polynomials = PolynomialOrder { variables };

    // nth_element(current, current, end) selects the smallest remaining row.
    // Equal rows are interchangeable, so only that selected minimum matters.
    for current in 0..rows.len() {
        rows[current..].select_nth_unstable_by(0, |left, right| {
            compare_rows(left, right, order, &polynomials)
        });
        let (earlier, later) = rows.split_at_mut(current + 1);
        let pivot = &earlier[current];
        let Some(head) = pivot.first() else {
            break;
        };
        for row in later {
            let Some(target) = row.first() else {
                continue;
            };
            if head.integral == target.integral {
                let (pivot_scale, row_scale) =
                    polynomials.cancellation_scales(&head.coefficient, &target.coefficient);
                // Reference forward sign: pivot * target/g - target * pivot/g.
                *row = scaled_subtract(pivot, &pivot_scale, row, &row_scale, order);
            }
        }
    }

    for current in (0..rows.len()).rev() {
        let (earlier, later) = rows.split_at_mut(current);
        let pivot = &later[0];
        let Some(head) = pivot.first() else {
            continue;
        };
        for row in earlier.iter_mut().rev() {
            if let Ok(position) =
                row.binary_search_by(|term| order.compare(&term.integral, &head.integral))
            {
                let (pivot_scale, row_scale) =
                    polynomials.cancellation_scales(&head.coefficient, &row[position].coefficient);
                // Reference backward sign: target * pivot/g - pivot * target/g.
                *row = scaled_subtract(row, &row_scale, pivot, &pivot_scale, order);
            }
        }
    }
    rows
}

fn compare_rows<const N: usize>(
    left: &PolynomialRow<N>,
    right: &PolynomialRow<N>,
    order: &IntegralOrder<N>,
    polynomials: &PolynomialOrder<'_>,
) -> Ordering {
    match (left.first(), right.first()) {
        (None, None) => return Ordering::Equal,
        (None, Some(_)) => return Ordering::Greater,
        (Some(_), None) => return Ordering::Less,
        (Some(left), Some(right)) => {
            let comparison = order.compare(&left.integral, &right.integral);
            if comparison != Ordering::Equal {
                return comparison;
            }
        }
    }
    left.len()
        .cmp(&right.len())
        .then_with(|| {
            left.iter()
                .zip(right)
                .map(|(left, right)| order.compare(&left.integral, &right.integral))
                .find(|comparison| *comparison != Ordering::Equal)
                .unwrap_or(Ordering::Equal)
        })
        .then_with(|| {
            left.iter()
                .zip(right)
                .map(|(left, right)| polynomials.compare(&left.coefficient, &right.coefficient))
                .find(|comparison| *comparison != Ordering::Equal)
                .unwrap_or(Ordering::Equal)
        })
}

// SpIRed calls FLINT fmpz_mpoly_cmp: number of monomials, all descending
// DEGLEX exponent vectors, then all integer coefficients in that same order.
// Symbolica's internal_cmp uses its native ascending Lex storage instead.
struct PolynomialOrder<'a> {
    variables: &'a [usize],
}

impl PolynomialOrder<'_> {
    fn monomial(&self, left: &[u16], right: &[u16]) -> Ordering {
        let degree = |powers: &[u16]| powers.iter().map(|power| u64::from(*power)).sum::<u64>();
        degree(left).cmp(&degree(right)).then_with(|| {
            self.variables
                .iter()
                .map(|&position| left[position].cmp(&right[position]))
                .find(|comparison| *comparison != Ordering::Equal)
                .unwrap_or(Ordering::Equal)
        })
    }

    fn descending_monomials(&self, polynomial: &CoefficientPolynomial) -> Vec<usize> {
        let mut positions: Vec<_> = (0..polynomial.nterms()).collect();
        positions.sort_unstable_by(|&left, &right| {
            self.monomial(polynomial.exponents(right), polynomial.exponents(left))
        });
        positions
    }

    fn compare(&self, left: &CoefficientPolynomial, right: &CoefficientPolynomial) -> Ordering {
        left.nterms().cmp(&right.nterms()).then_with(|| {
            let left_positions = self.descending_monomials(left);
            let right_positions = self.descending_monomials(right);
            left_positions
                .iter()
                .zip(&right_positions)
                .map(|(&left_position, &right_position)| {
                    self.monomial(
                        left.exponents(left_position),
                        right.exponents(right_position),
                    )
                })
                .find(|comparison| *comparison != Ordering::Equal)
                .unwrap_or_else(|| {
                    left_positions
                        .iter()
                        .zip(&right_positions)
                        .map(|(&left_position, &right_position)| {
                            left.coefficients[left_position]
                                .cmp(&right.coefficients[right_position])
                        })
                        .find(|comparison| *comparison != Ordering::Equal)
                        .unwrap_or(Ordering::Equal)
                })
        })
    }

    fn cancellation_scales(
        &self,
        pivot: &CoefficientPolynomial,
        target: &CoefficientPolynomial,
    ) -> (CoefficientPolynomial, CoefficientPolynomial) {
        let mut gcd = pivot.gcd(target);
        // FLINT normalizes the DEGLEX-leading coefficient positive. Native
        // gcd uses Lex; the unit can differ for inhomogeneous polynomials.
        let leading = (0..gcd.nterms())
            .max_by(|&left, &right| self.monomial(gcd.exponents(left), gcd.exponents(right)))
            .expect("the GCD of nonzero pivot coefficients is nonzero");
        if gcd.coefficients[leading].is_negative() {
            gcd = -gcd;
        }
        (
            target
                .try_div_exact(&gcd)
                .expect("a polynomial GCD divides its target exactly"),
            pivot
                .try_div_exact(&gcd)
                .expect("a polynomial GCD divides its pivot exactly"),
        )
    }
}

fn scaled_subtract<const N: usize>(
    left: &PolynomialRow<N>,
    left_scale: &CoefficientPolynomial,
    right: &PolynomialRow<N>,
    right_scale: &CoefficientPolynomial,
    order: &IntegralOrder<N>,
) -> PolynomialRow<N> {
    let mut output = Vec::with_capacity(left.len() + right.len());
    let mut left = left.iter().peekable();
    let mut right = right.iter().peekable();
    while left.peek().is_some() || right.peek().is_some() {
        let comparison = match (left.peek(), right.peek()) {
            (Some(left), Some(right)) => order.compare(&left.integral, &right.integral),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => unreachable!(),
        };
        let (integral, coefficient) = match comparison {
            Ordering::Less => {
                let term = left.next().unwrap();
                (term.integral, &term.coefficient * left_scale)
            }
            Ordering::Greater => {
                let term = right.next().unwrap();
                (term.integral, -(&term.coefficient * right_scale))
            }
            Ordering::Equal => {
                let left = left.next().unwrap();
                let right = right.next().unwrap();
                (
                    left.integral,
                    &left.coefficient * left_scale - &right.coefficient * right_scale,
                )
            }
        };
        if !coefficient.is_zero() {
            output.push(Term {
                integral,
                coefficient,
            });
        }
    }
    output
}

#[cfg(test)]
mod tests;
