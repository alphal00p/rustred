use crate::algebra::{Coefficient, CoefficientPolynomial};

use super::{Integral, SolverError};

/// One integral and its native Symbolica coefficient. No family or provenance
/// ownership is duplicated in individual terms.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Term<const N: usize, C> {
    pub integral: Integral<N>,
    pub coefficient: C,
}

/// Terms in harder-first integral order, with distinct keys and no zero terms.
/// The constructor/preparation boundary establishes these invariants once.
pub type Row<const N: usize, C> = Vec<Term<N, C>>;
pub type PolynomialRow<const N: usize> = Row<N, CoefficientPolynomial>;
pub type ExactRow<const N: usize> = Row<N, Coefficient>;

/// Clear one row's rational-polynomial denominators with native Symbolica
/// arithmetic. Integral keys, term order, signs, and variable context remain
/// unchanged; empty rows remain empty. This does not collect applicability
/// conditions: the caller retains those before clearing denominators.
pub(super) fn clear_denominators<const N: usize>(
    row: ExactRow<N>,
) -> Result<PolynomialRow<N>, SolverError> {
    let Some(first) = row.first() else {
        return Ok(Vec::new());
    };
    let variables = first.coefficient.numerator.variables();
    let mut common = first.coefficient.denominator.one();
    for term in &row {
        let coefficient = &term.coefficient;
        if coefficient.numerator.variables() != variables
            || coefficient.denominator.variables() != variables
        {
            return Err(SolverError::InvalidInput(
                "row coefficients must share a native variable map before denominator clearing"
                    .into(),
            ));
        }
        let denominator = &coefficient.denominator;
        if denominator.is_zero() {
            return Err(SolverError::InvalidInput(
                "cannot clear a zero coefficient denominator".into(),
            ));
        }
        let gcd = common.gcd(denominator);
        let quotient = denominator.try_div_exact(&gcd).ok_or_else(|| {
            SolverError::ExactReplay("native polynomial GCD does not divide a denominator".into())
        })?;
        common = &common * &quotient;
    }
    row.into_iter()
        .map(|term| {
            let scale = common
                .try_div_exact(&term.coefficient.denominator)
                .ok_or_else(|| {
                    SolverError::ExactReplay(
                        "native common denominator is not exactly divisible".into(),
                    )
                })?;
            Ok(Term {
                integral: term.integral,
                coefficient: &term.coefficient.numerator * &scale,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::algebra::CoefficientContext;

    use super::*;

    #[test]
    fn denominator_clearing_uses_native_lcm_and_preserves_signs_keys_and_context() {
        let context = CoefficientContext::new(["a", "b", "d"]);
        let inputs = ["(a+1)/(2*b)", "-1/(3*b)", "d/(2*b^2)"];
        let expected = ["3*b*(a+1)", "-2*b", "3*d"];
        let row = inputs
            .iter()
            .enumerate()
            .map(|(index, coefficient)| Term {
                // Deliberately retain caller order instead of sorting keys.
                integral: Integral::numeric([2 - index as i16]).unwrap(),
                coefficient: context.coefficient_fixture(coefficient),
            })
            .collect();
        let cleared = clear_denominators(row).unwrap();
        for (index, (term, expected)) in cleared.iter().zip(expected).enumerate() {
            assert_eq!(
                term.integral,
                Integral::numeric([2 - index as i16]).unwrap()
            );
            assert_eq!(
                term.coefficient,
                context.coefficient_fixture(expected).numerator
            );
            assert_eq!(
                term.coefficient.variables(),
                context.coefficient_fixture("1").numerator.variables()
            );
        }
    }

    #[test]
    fn denominator_clearing_preserves_empty_and_polynomial_rows() {
        assert!(clear_denominators::<1>(Vec::new()).unwrap().is_empty());
        let context = CoefficientContext::new(["a"]);
        let row = ["0", "-3*a", "2"]
            .into_iter()
            .enumerate()
            .map(|(index, coefficient)| Term {
                integral: Integral::symbolic([index as i16]).unwrap(),
                coefficient: context.coefficient_fixture(coefficient),
            })
            .collect::<Vec<_>>();
        let expected = row
            .iter()
            .map(|term| Term {
                integral: term.integral,
                coefficient: term.coefficient.numerator.clone(),
            })
            .collect::<Vec<_>>();
        assert_eq!(clear_denominators(row).unwrap(), expected);
    }

    #[test]
    fn denominator_clearing_rejects_incompatible_maps_and_zero_denominators() {
        let context = CoefficientContext::new(["a"]);
        let other = CoefficientContext::new(["b"]);
        let term = |coefficient| Term {
            integral: Integral::symbolic([0]).unwrap(),
            coefficient,
        };
        assert!(matches!(
            clear_denominators(vec![
                term(context.coefficient_fixture("1/a")),
                term(other.coefficient_fixture("1/b")),
            ]),
            Err(SolverError::InvalidInput(_))
        ));
        let mut malformed = context.coefficient_fixture("1");
        malformed.denominator = malformed.denominator.zero();
        assert!(matches!(
            clear_denominators(vec![term(malformed)]),
            Err(SolverError::InvalidInput(_))
        ));
    }
}
