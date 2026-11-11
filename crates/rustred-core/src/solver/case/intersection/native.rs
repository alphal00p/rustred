//! Narrow adapters to native exact equation operations, not coefficient values.

use std::cmp::Ordering;

use symbolica::domains::InternalOrdering;
use symbolica::poly::groebner::GroebnerBasis;
use symbolica::poly::{Exponent, GrevLexOrder, LexOrder, MonomialOrder, PolyVariable};
use symbolica::prelude::{Factorize, Integer, Q, Rational, Z};

use crate::algebra::CoefficientPolynomial;

use super::super::super::{GeometryError, geometry};
use super::super::{AffineGeometryError, Case};
use super::CaseIntersectionFailure;

mod lex;

pub(super) fn preflight<const N: usize>(
    parent: &Case<N>,
    equations: &[CoefficientPolynomial],
    indices: &[usize; N],
) -> Result<(), CaseIntersectionFailure> {
    super::engine::checkpoint()?;
    geometry::preflight(equations, indices).map_err(|error| match error {
        GeometryError::InvalidInput(message) => CaseIntersectionFailure::InvalidInput(message),
        other => CaseIntersectionFailure::Admission(AffineGeometryError::Coordinate(other)),
    })?;
    if let Some(affine) = parent.affine()
        && affine.index_variables() != indices
    {
        return Err(CaseIntersectionFailure::InvalidInput(
            "parent and intersection use different index-variable maps",
        ));
    }
    let Some(first) = equations.first() else {
        return Ok(());
    };
    let variables = first.variables();
    if let Some(affine) = parent.affine()
        && affine.equations()[0].variables() != variables
    {
        return Err(CaseIntersectionFailure::InvalidInput(
            "parent and equations use different coefficient variable maps",
        ));
    }
    Ok(())
}

/// Discarding a nonzero scalar is valid only because these are zero equations.
pub(super) fn primitive(polynomial: CoefficientPolynomial) -> CoefficientPolynomial {
    if polynomial.is_zero() {
        return polynomial;
    }
    positive_leading(polynomial.make_primitive())
}

fn positive_leading(polynomial: CoefficientPolynomial) -> CoefficientPolynomial {
    if !polynomial.is_zero() && polynomial.lcoeff().is_negative() {
        -polynomial
    } else {
        polynomial
    }
}

pub(super) fn canonicalize(equations: &mut Vec<CoefficientPolynomial>) {
    equations.retain(|equation| !equation.is_zero());
    equations.sort_by(InternalOrdering::internal_cmp);
    equations.dedup();
}

pub(super) fn restrict<const N: usize>(
    parent: &Case<N>,
    equations: &[CoefficientPolynomial],
    indices: &[usize; N],
) -> Result<Vec<CoefficientPolynomial>, CaseIntersectionFailure> {
    let mut restricted = Vec::with_capacity(equations.len());
    for source in equations {
        super::engine::checkpoint()?;
        let equation = if let Some(affine) = parent.affine() {
            let restricted = affine
                .restrict_equation(source)
                .map_err(CaseIntersectionFailure::Admission)?;
            if affine.has_integral_chart() {
                primitive(restricted)
            } else {
                // The rational-chart zero-locus API already removed native
                // rational content jointly; no second integer content GCD.
                positive_leading(restricted)
            }
        } else {
            let mut equation = source.clone();
            for (axis, value) in parent.fixed().iter().enumerate() {
                if let Some(value) = value {
                    equation = equation.replace(indices[axis], &Integer::from(*value));
                }
            }
            primitive(equation)
        };
        if !equation.is_zero() {
            restricted.push(equation);
        }
    }
    canonicalize(&mut restricted);
    super::engine::checkpoint()?;
    Ok(restricted)
}

pub(super) fn is_affine(polynomial: &CoefficientPolynomial) -> bool {
    polynomial.exponents_iter().all(|exponents| {
        !exponents.iter().any(|&power| power > 1)
            && exponents
                .iter()
                .filter(|&&power| power != 0)
                .take(2)
                .count()
                <= 1
    })
}

/// Preserve the entire transformed basis, including newly exposed coupled
/// affine equations. Native Q/GrevLex-F4 owns every ideal operation. The
/// returned generators use the usual Lex polynomial storage, but are not
/// asserted to be a Lex Groebner basis: consumers need the same ideal, not
/// an elimination order or Lex normal forms.
pub(super) fn normalize(
    equations: &[CoefficientPolynomial],
) -> Result<Vec<CoefficientPolynomial>, CaseIntersectionFailure> {
    normalize_in_order::<GrevLexOrder>(equations)
}

/// Structural admission only: native F4 itself has no cancellation callback.
pub(super) fn lex_eligible(equations: &[CoefficientPolynomial], diagnostics_enabled: bool) -> bool {
    lex::eligible(equations, diagnostics_enabled)
}

/// An exceptional representation fallback, not a different authority path.
pub(super) fn normalize_lex(
    equations: &[CoefficientPolynomial],
) -> Result<Vec<CoefficientPolynomial>, CaseIntersectionFailure> {
    #[cfg(test)]
    lex::record_call();
    normalize_in_order::<LexOrder>(equations)
}

#[cfg(test)]
pub(super) fn take_lex_calls() -> usize {
    lex::take_calls()
}

fn normalize_in_order<O: MonomialOrder>(
    equations: &[CoefficientPolynomial],
) -> Result<Vec<CoefficientPolynomial>, CaseIntersectionFailure> {
    super::engine::checkpoint()?;
    let ideal: Vec<_> = equations
        .iter()
        .map(|polynomial| {
            polynomial
                .map_coeff(|value| Rational::from(value), Q)
                .reorder::<O>()
        })
        .collect();
    super::engine::checkpoint()?;
    let basis = GroebnerBasis::new(&ideal, false);
    super::engine::checkpoint()?;
    let mut normalized = Vec::with_capacity(basis.system.len());
    for polynomial in basis.system {
        super::engine::checkpoint()?;
        let polynomial = polynomial.reorder::<LexOrder>();
        if polynomial.is_zero() {
            continue;
        }
        let reduced = polynomial.make_primitive();
        if reduced.coefficients.iter().any(|value| !value.is_integer()) {
            return Err(CaseIntersectionFailure::NativeAlgebra);
        }
        // Q make_primitive already gives primitive integral coefficients.
        let reduced = positive_leading(reduced.map_coeff(Rational::numerator, Z));
        if reduced.variables() != equations[0].variables() {
            return Err(CaseIntersectionFailure::NativeAlgebra);
        }
        normalized.push(reduced);
    }
    canonicalize(&mut normalized);
    super::engine::checkpoint()?;
    Ok(normalized)
}

#[cfg(test)]
#[path = "native/tests.rs"]
mod tests;

/// Block elimination order for the Rabinowitsch saturation: the LAST variable
/// (the auxiliary `t`) is compared first, then GrevLex on the others. Every
/// monomial containing `t` is larger than every `t`-free monomial, so the
/// `t`-free elements of a Groebner basis generate the elimination ideal.
#[derive(Clone)]
struct EliminateLast;

impl MonomialOrder for EliminateLast {
    fn cmp<E: Exponent>(a: &[E], b: &[E]) -> Ordering {
        let last = a.len() - 1;
        a[last]
            .cmp(&b[last])
            .then_with(|| GrevLexOrder::cmp(&a[..last], &b[..last]))
    }
}

/// The auxiliary Rabinowitsch variable; never part of a coefficient map.
const SATURATION_VARIABLE: PolyVariable = PolyVariable::Temporary(usize::MAX - 0x5a7);

/// Exact saturation `(equations) : (prod exclusions)^inf` over Q, returned as
/// primitive integer generators in the original variable map (the `t`-free
/// part of a Groebner basis of `equations + (1 - t prod exclusions)` in a block
/// elimination order). Its zero set is the Zariski closure of the zeros of
/// `equations` at which no exclusion vanishes: every such point is kept, and
/// only points on an excluded hypersurface can be removed.
pub(super) fn saturate(
    equations: &[CoefficientPolynomial],
    exclusions: &[CoefficientPolynomial],
) -> Result<Vec<CoefficientPolynomial>, CaseIntersectionFailure> {
    super::engine::checkpoint()?;
    let Some(first) = equations.first() else {
        return Ok(Vec::new());
    };
    if exclusions.is_empty() {
        return normalize(equations);
    }
    let variables = first.variables().clone();
    if variables.contains(&SATURATION_VARIABLE)
        || equations
            .iter()
            .chain(exclusions)
            .any(|polynomial| polynomial.variables() != &variables)
    {
        return Err(CaseIntersectionFailure::NativeAlgebra);
    }
    let lift = |polynomial: &CoefficientPolynomial| {
        let mut polynomial = polynomial.map_coeff(|value| Rational::from(value), Q);
        polynomial.add_variable(&SATURATION_VARIABLE);
        polynomial
    };
    let mut product = lift(&first.one());
    for exclusion in exclusions {
        product = &product * &lift(exclusion);
    }
    let mut t_exponents = vec![0_u16; variables.len() + 1];
    t_exponents[variables.len()] = 1;
    let t = product.monomial(Rational::from(1), t_exponents);
    let rabinowitsch = product.one() - (&t * &product);
    let mut ideal: Vec<_> = equations
        .iter()
        .map(|polynomial| lift(polynomial).reorder::<EliminateLast>())
        .collect();
    ideal.push(rabinowitsch.reorder::<EliminateLast>());
    super::engine::checkpoint()?;
    let basis = GroebnerBasis::new(&ideal, false);
    super::engine::checkpoint()?;
    let template = first.map_coeff(|value| Rational::from(value), Q);
    let nvars = variables.len();
    let mut saturated = Vec::new();
    for polynomial in basis.system {
        super::engine::checkpoint()?;
        let polynomial = polynomial.reorder::<LexOrder>();
        if polynomial.is_zero() || polynomial.degree(nvars) != 0 {
            continue;
        }
        let mut reduced = template.zero();
        for (coefficient, exponents) in polynomial
            .coefficients
            .iter()
            .zip(polynomial.exponents_iter())
        {
            reduced.append_monomial(coefficient.clone(), &exponents[..nvars]);
        }
        let reduced = reduced.make_primitive();
        if reduced.coefficients.iter().any(|value| !value.is_integer()) {
            return Err(CaseIntersectionFailure::NativeAlgebra);
        }
        let reduced = positive_leading(reduced.map_coeff(Rational::numerator, Z));
        if reduced.variables() != &variables {
            return Err(CaseIntersectionFailure::NativeAlgebra);
        }
        saturated.push(reduced);
    }
    canonicalize(&mut saturated);
    super::engine::checkpoint()?;
    Ok(saturated)
}

/// `denominator^d * equation(x = numerator / denominator)` with `d` the degree
/// of `equation` in variable `x` (`numerator` and `denominator` free of `x`):
/// the exact denominator-free elimination of a linear dependent.
pub(super) fn substitute_quotient(
    equation: &CoefficientPolynomial,
    x: usize,
    numerator: &CoefficientPolynomial,
    denominator: &CoefficientPolynomial,
) -> CoefficientPolynomial {
    let degree = usize::from(equation.degree(x));
    if degree == 0 {
        return equation.clone();
    }
    let mut result = equation.zero();
    for (coefficient, power) in equation.to_univariate_polynomial_list(x) {
        let power = usize::from(power);
        let term = &coefficient * &(numerator.pow(power) * &denominator.pow(degree - power));
        result = result + term;
    }
    result
}

/// Divide out every factor of `factors` as often as it divides `polynomial`
/// exactly (valid for an equation on a region where every factor is nonzero).
pub(super) fn strip_factors(
    mut polynomial: CoefficientPolynomial,
    factors: &[CoefficientPolynomial],
) -> CoefficientPolynomial {
    if polynomial.is_zero() {
        return polynomial;
    }
    for factor in factors {
        if factor.is_constant() {
            continue;
        }
        while let Some(quotient) = polynomial.try_div(factor) {
            polynomial = quotient;
            if polynomial.is_constant() {
                break;
            }
        }
    }
    primitive(polynomial)
}

/// Distinct nonconstant zero-locus factors. A zero input must be removed by
/// the caller, and nonzero scalar content/multiplicities create no branches.
pub(super) fn factors(
    equation: &CoefficientPolynomial,
) -> Result<Vec<CoefficientPolynomial>, CaseIntersectionFailure> {
    super::engine::checkpoint()?;
    let factors = equation.factor();
    super::engine::checkpoint()?;
    if factors.is_empty()
        || factors.iter().any(|(factor, multiplicity)| {
            factor.is_zero() || *multiplicity == 0 || factor.variables() != equation.variables()
        })
    {
        return Err(CaseIntersectionFailure::NativeAlgebra);
    }
    let mut factors: Vec<_> = factors
        .into_iter()
        .filter(|(factor, _)| !factor.is_constant())
        .map(|(factor, _)| primitive(factor))
        .collect();
    canonicalize(&mut factors);
    if factors.is_empty() {
        return Err(CaseIntersectionFailure::NativeAlgebra);
    }
    super::engine::checkpoint()?;
    Ok(factors)
}

/// Apply only to the validated output of complete native `factor()`, after
/// charging all factor terms. An irreducible univariate factor of degree above
/// one has no rational root, hence no integer root. Authentication must cover
/// its entire support; parameters and coupled factors do not qualify.
pub(super) fn retain_possible_integer_factors<const N: usize>(
    factors: &mut Vec<CoefficientPolynomial>,
    indices: &[usize; N],
) {
    factors.retain(|factor| {
        let mut support = (0..factor.nvars()).filter(|&position| factor.degree(position) != 0);
        match (support.next(), support.next()) {
            (Some(position), None) if indices.contains(&position) => factor.degree(position) <= 1,
            _ => true,
        }
    });
}
