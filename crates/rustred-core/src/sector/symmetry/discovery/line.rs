//! Exact line-momentum proposals, independent of line masses.

use std::sync::Arc;

use symbolica::poly::factor::Factorize;
use symbolica::prelude::{Integer, MultivariatePolynomial, PolyVariable, Rational, Z};

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

/// Propose the momentum row `r` of `D = r.r + constant`, with `r` over the
/// loop momenta followed by the external momenta.
///
/// The loop part is the integer square root of the loop-loop quadratic form;
/// the external part then follows uniquely from the loop-external
/// coefficients, `2 r_loop[i] r_external[e]`, and must reproduce every one of
/// them. The constant (a mass, a parameter-dependent deformation, and for
/// external momenta the numeric `r_external.G.r_external` term) is
/// deliberately ignored: exact family-map verification retains and compares
/// the complete denominator, so mass equality is proved there, not here.
/// Symbolica factors the quadratic form; the integer root is replayed by
/// exact polynomial multiplication rather than trusted by shape.
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
    let loops = family.loop_count();
    let mut quadratic = Quadratic::new(&Z, None, variables.clone());
    let mut mixed = Vec::new();
    for (coordinate, value) in family.coordinates().iter().zip(denominator.coefficients()) {
        let Some(value) = integer_constant(value, context)? else {
            return Ok(Err(LineGeometrySkip::NonIntegerQuadratic));
        };
        match *coordinate {
            ScalarProductCoordinate::LoopLoop { left, right } => {
                let mut powers = vec![0_u16; loops];
                powers[left] += 1;
                powers[right] += 1;
                quadratic.append_monomial(value, &powers);
            }
            ScalarProductCoordinate::LoopExternal {
                loop_index,
                external_index,
            } => mixed.push((loop_index, external_index, value)),
        }
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
    let mut loop_row = vec![Integer::from(0); loops];
    for term in 0..proposed.nterms() {
        let slot = proposed
            .exponents(term)
            .iter()
            .position(|&power| power == 1)
            .ok_or(DiscoveryError::Invalid(
                "factored line momentum has a nonlinear term",
            ))?;
        loop_row[slot] = proposed.coefficients[term].clone();
    }
    // r_external[e] = coefficient(k_i.p_e) / (2 r_loop[i]) at any nonzero
    // r_loop[i]; every loop-external coefficient must then agree.
    let pivot = loop_row
        .iter()
        .position(|value| !value.is_zero())
        .ok_or(DiscoveryError::Invalid("line momentum has no loop part"))?;
    let mut external_row = vec![Rational::zero(); family.external_count()];
    for &(loop_index, external_index, ref value) in &mixed {
        if loop_index == pivot {
            external_row[external_index] =
                Rational::from((value.clone(), Integer::from(2) * &loop_row[pivot]));
        }
    }
    for &(loop_index, external_index, ref value) in &mixed {
        let implied = &Rational::from(Integer::from(2) * &loop_row[loop_index])
            * &external_row[external_index];
        if implied != *value {
            return Ok(Err(LineGeometrySkip::ExternalMomenta));
        }
    }
    let mut row = Vec::with_capacity(loops + family.external_count());
    row.extend(
        loop_row
            .into_iter()
            .map(|value| Coefficient::from(context.zero().numerator.constant(value))),
    );
    for value in external_row {
        row.push(rational_coefficient(&value, context)?);
    }
    Ok(Ok(row))
}

fn rational_coefficient(
    value: &Rational,
    context: &CoefficientContext,
) -> Result<Coefficient, DiscoveryError> {
    let numerator: Coefficient = context
        .zero()
        .numerator
        .constant(value.numerator_ref().clone())
        .into();
    let denominator: Coefficient = context
        .zero()
        .numerator
        .constant(value.denominator_ref().clone())
        .into();
    context
        .try_div(&numerator, &denominator, Default::default())
        .map_err(super::exact)
}
