//! Exact exceptional equations extracted once from a canonical solved row.

use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};

use symbolica::domains::InternalOrdering;
use symbolica::prelude::{Factorize, Integer};

use crate::algebra::CoefficientPolynomial;

use super::RuleCandidate;

/// Index-case equalities over the generic coefficient field Q(parameters).
///
/// The outer vector is OR; equations within each branch are AND. An empty
/// outer vector means false, while an empty branch would mean true. Equations
/// retain the candidate's complete variable map, with zero exponents in every
/// non-index variable. Affine and nonlinear equations remain exact equations;
/// this type makes no claim about their integer solutions or sector geometry.
///
/// These are the index exceptions of the normalized candidate RHS only.
/// Parameter-only poles are deliberately excluded: division can introduce a
/// factor such as `d - 4` even when `SourceSystem::conditions()` is empty.
/// Such poles remain visible in the candidate's exact RHS denominators and
/// must be respected when specializing parameters. Inherited source
/// conditions are additional, separate obligations; they do not cover every
/// parameter pole introduced by the solved rule.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExceptionalConditions {
    pub branches: Vec<Vec<CoefficientPolynomial>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExceptionError {
    InvalidInput(&'static str),
    NativeAlgebra,
}

impl fmt::Display for ExceptionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(detail) => write!(f, "invalid exception-extraction input: {detail}"),
            Self::NativeAlgebra => {
                write!(f, "native algebra failed while extracting rule exceptions")
            }
        }
    }
}

impl std::error::Error for ExceptionError {}

/// Extract SpIRed's denominator and inactive-index activation exceptions.
///
/// This is a cold operation on a final canonical candidate. It does not
/// authenticate or wrap every coefficient, specialize source rows, or decide
/// affine/nonlinear geometry. Native Symbolica owns factorization, coefficient
/// collection, content removal, and boundary substitution.
///
/// As for solver-produced candidates, coefficients must already have their
/// fixed-coordinate substitutions and canonical target translation applied.
/// Merely attaching a fixed `CoordinateCase` to a hand-built candidate does
/// not specialize its coefficients. Returned equations retain their meaning
/// on that case; intersection and geometric simplification belong to its owner.
pub fn extract_exceptions<const N: usize>(
    candidate: &RuleCandidate<N>,
    indices: &[usize; N],
    sector: &[bool; N],
) -> Result<ExceptionalConditions, ExceptionError> {
    preflight(candidate, indices, sector)?;
    if candidate.rhs.is_empty() {
        // No coefficient variable map exists in an empty RHS and there are
        // no denominators or terms capable of activating another sector.
        return Ok(ExceptionalConditions::default());
    }
    catch_unwind(AssertUnwindSafe(|| extract(candidate, indices, sector)))
        .map_err(|_| ExceptionError::NativeAlgebra)
}

fn preflight<const N: usize>(
    candidate: &RuleCandidate<N>,
    indices: &[usize; N],
    sector: &[bool; N],
) -> Result<(), ExceptionError> {
    if candidate.target != candidate.case.integral() || !candidate.case.is_in_sector(sector) {
        return Err(ExceptionError::InvalidInput(
            "target must be canonical for its case and sector",
        ));
    }
    for (axis, position) in indices.iter().enumerate() {
        if indices[..axis].contains(position) {
            return Err(ExceptionError::InvalidInput(
                "index-variable positions must be distinct",
            ));
        }
    }
    let Some(first) = candidate.rhs.first() else {
        return Ok(());
    };
    let variables = first.coefficient.numerator.variables();
    if indices.iter().any(|&position| position >= variables.len()) {
        return Err(ExceptionError::InvalidInput(
            "index-variable position is outside the coefficient variable map",
        ));
    }
    for term in &candidate.rhs {
        for (axis, active) in sector.iter().enumerate() {
            if term.integral[axis].is_symbolic() != candidate.target[axis].is_symbolic() {
                return Err(ExceptionError::InvalidInput(
                    "RHS and target must share their symbolic-coordinate pattern",
                ));
            }
            if !active && !term.integral[axis].is_symbolic() && term.integral[axis].value() > 0 {
                return Err(ExceptionError::InvalidInput(
                    "numeric RHS index activates an inactive sector coordinate",
                ));
            }
        }
        for polynomial in [&term.coefficient.numerator, &term.coefficient.denominator] {
            if polynomial.variables() != variables {
                return Err(ExceptionError::InvalidInput(
                    "coefficient parts must share one variable map",
                ));
            }
            if polynomial.coefficients.len().checked_mul(variables.len())
                != Some(polynomial.exponents.len())
            {
                return Err(ExceptionError::InvalidInput(
                    "native coefficient and exponent arrays have inconsistent lengths",
                ));
            }
        }
        if term.coefficient.denominator.is_zero() {
            return Err(ExceptionError::InvalidInput(
                "coefficient denominator is zero",
            ));
        }
    }
    Ok(())
}

fn extract<const N: usize>(
    candidate: &RuleCandidate<N>,
    indices: &[usize; N],
    sector: &[bool; N],
) -> ExceptionalConditions {
    let template = &candidate.rhs[0].coefficient.numerator;
    let parameter_positions: Vec<_> = (0..template.nvars())
        .filter(|position| !indices.contains(position))
        .collect();

    // Scalar content has no zero locus over Q. Removing it lets repeated
    // denominators differing by a nonzero integer share one factorization.
    let mut denominators: Vec<_> = candidate
        .rhs
        .iter()
        .filter(|term| !term.coefficient.is_zero())
        .map(|term| &term.coefficient.denominator)
        .filter(|denominator| contains_index(denominator, indices))
        .map(|denominator| primitive(denominator.clone()))
        .collect();
    sort_unique(&mut denominators);
    let mut factors = Vec::new();
    for denominator in denominators {
        for (factor, _multiplicity) in denominator.factor() {
            if contains_index(&factor, indices) {
                factors.push(primitive(factor));
            }
        }
    }
    sort_unique(&mut factors);

    let mut branches = Vec::new();
    for factor in factors {
        let coefficients = factor.to_multivariate_polynomial_list(&parameter_positions, true);
        let mut branch = Vec::with_capacity(coefficients.len());
        let mut impossible = false;
        for equation in coefficients.into_values() {
            if equation.is_zero() {
                continue;
            }
            if equation.is_constant() {
                // A nonzero coefficient of a parameter monomial prevents the
                // denominator factor from vanishing identically in parameters.
                impossible = true;
                break;
            }
            branch.push(primitive(equation));
        }
        if !impossible {
            sort_unique(&mut branch);
            branches.push(branch);
        }
    }

    for term in &candidate.rhs {
        for (axis, (&position, active)) in indices.iter().zip(sector).enumerate() {
            let power = term.integral[axis];
            if *active || !power.is_symbolic() || power.value() <= 0 {
                continue;
            }
            for boundary in 0..power.value() {
                let value = Integer::from(-boundary);
                if term
                    .coefficient
                    .numerator
                    .replace(position, &value)
                    .is_zero()
                {
                    // Even when its denominator vanishes here, denominator
                    // exceptions above still retain the corresponding pole.
                    continue;
                }
                let variable = template
                    .variable(&template.variables()[position])
                    .expect("preflight checked the variable-map position");
                let equation = &variable + &template.constant(Integer::from(boundary));
                branches.push(vec![equation]);
            }
        }
    }
    branches.sort_unstable_by(InternalOrdering::internal_cmp);
    branches.dedup();
    ExceptionalConditions { branches }
}

fn contains_index<const N: usize>(
    polynomial: &CoefficientPolynomial,
    indices: &[usize; N],
) -> bool {
    if indices.is_empty() || polynomial.nvars() == 0 {
        return false;
    }
    polynomial
        .exponents_iter()
        .any(|exponents| indices.iter().any(|&position| exponents[position] != 0))
}

fn primitive(polynomial: CoefficientPolynomial) -> CoefficientPolynomial {
    debug_assert!(!polynomial.is_zero());
    let polynomial = polynomial.make_primitive();
    if polynomial.lcoeff() < 0 {
        -polynomial
    } else {
        polynomial
    }
}

fn sort_unique(polynomials: &mut Vec<CoefficientPolynomial>) {
    polynomials.sort_unstable_by(InternalOrdering::internal_cmp);
    polynomials.dedup();
}

#[cfg(test)]
mod tests;
