//! Exact integer line-momentum proposals, independent of line masses.

use std::sync::Arc;

use symbolica::poly::factor::Factorize;
use symbolica::prelude::{Integer, MultivariatePolynomial, PolyVariable, Z};

use crate::algebra::{Coefficient, CoefficientContext};
use crate::family::{IntegralFamily, ScalarProductCoordinate};

use super::{DiscoveryError, LineGeometrySkip};

type Quadratic = MultivariatePolynomial<symbolica::prelude::IntegerRing, u16>;

/// Integer constant value of an authenticated coefficient, if it is one.
fn integer_constant(
    value: &Coefficient,
    context: &CoefficientContext,
) -> Result<Option<Integer>, DiscoveryError> {
    if !context.contains(value) {
        return Err(DiscoveryError::ForeignCoefficient);
    }
    Ok(
        (value.denominator.is_one() && value.numerator.is_constant())
            .then(|| value.numerator.get_constant()),
    )
}

/// Propose the integer momentum row `q` of `D = q.q + constant`.
///
/// The constant (mass, or any parameter-dependent deformation of it) is
/// deliberately ignored: exact family-map verification retains and compares
/// the complete denominator, so mass equality is proved there, not here.
/// Symbolica factors the quadratic form; the integer root is replayed by exact
/// polynomial multiplication rather than trusted by shape.
pub(crate) fn line_momentum(
    family: &IntegralFamily,
    slot: usize,
    variables: &Arc<Vec<PolyVariable>>,
) -> Result<Result<Vec<Coefficient>, LineGeometrySkip>, DiscoveryError> {
    let context = family.coefficient_context();
    let denominator = family
        .denominators()
        .get(slot)
        .ok_or(DiscoveryError::Invalid("line momentum slot out of range"))?;
    let mut quadratic = Quadratic::new(&Z, None, variables.clone());
    for (coordinate, value) in family.coordinates().iter().zip(denominator.coefficients()) {
        let Some(value) = integer_constant(value, context)? else {
            return Ok(Err(LineGeometrySkip::NonIntegerQuadratic));
        };
        let ScalarProductCoordinate::LoopLoop { left, right } = *coordinate else {
            return Ok(Err(LineGeometrySkip::ExternalMomenta));
        };
        let mut powers = vec![0_u16; family.loop_count()];
        powers[left] += 1;
        powers[right] += 1;
        quadratic.append_monomial(value, &powers);
    }
    if quadratic.is_zero() {
        return Ok(Err(LineGeometrySkip::NotLinearSquare));
    }
    let mut scalar = Integer::from(1);
    let mut linear = None;
    for (factor, multiplicity) in quadratic.factor() {
        if factor.is_constant() {
            scalar *= factor.get_constant().pow(multiplicity as u64);
        } else if multiplicity == 2
            && linear.is_none()
            && (0..factor.nterms())
                .all(|term| factor.exponents(term).iter().copied().sum::<u16>() == 1)
        {
            linear = Some(factor);
        } else {
            return Ok(Err(LineGeometrySkip::NotLinearSquare));
        }
    }
    let Some(linear) = linear else {
        return Ok(Err(LineGeometrySkip::NotLinearSquare));
    };
    if scalar <= Integer::from(0) {
        return Ok(Err(LineGeometrySkip::NotLinearSquare));
    }
    // Symbolica owns integer root extraction; the proposal is independently
    // checked by exact polynomial multiplication, not trusted by shape alone.
    let root = scalar.root(2);
    let proposed = &linear * &linear.constant(root);
    if &proposed * &proposed != quadratic {
        return Ok(Err(LineGeometrySkip::NotLinearSquare));
    }
    let mut row = vec![context.zero(); family.loop_count()];
    for term in 0..proposed.nterms() {
        let slot = proposed
            .exponents(term)
            .iter()
            .position(|&power| power == 1)
            .ok_or(DiscoveryError::Invalid(
                "factored line momentum has a nonlinear term",
            ))?;
        row[slot] = context
            .zero()
            .numerator
            .constant(proposed.coefficients[term].clone())
            .into();
    }
    Ok(Ok(row))
}
