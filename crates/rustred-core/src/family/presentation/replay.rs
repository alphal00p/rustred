//! Exact replay of physical propagator rows.

use crate::algebra::{Coefficient, CoefficientContext};
use crate::family::{IntegralFamily, ScalarProductCoordinate};

use super::authentication::coefficients_equal;
use super::error::{FamilyPresentationError, PresentationDenominatorComponent};
use super::model::{AlgebraicSign, FamilyConventions, PhysicalPropagator};

pub(super) fn verify_physical_denominator(
    family: &IntegralFamily,
    denominator: usize,
    physical: &PhysicalPropagator,
    conventions: FamilyConventions,
) -> Result<(), FamilyPresentationError> {
    let context = family.coefficient_context();
    let limits = family.limits.exact_algebra;
    for (coordinate, kind) in family.coordinates().iter().copied().enumerate() {
        let unsigned = match kind {
            ScalarProductCoordinate::LoopLoop { left, right } => {
                let mut coefficient = context.try_mul(
                    &physical.momentum().loop_coefficients()[left],
                    &physical.momentum().loop_coefficients()[right],
                    limits,
                )?;
                if left != right {
                    coefficient = context.try_mul(&coefficient, &context.integer(2), limits)?;
                }
                coefficient
            }
            ScalarProductCoordinate::LoopExternal {
                loop_index,
                external_index,
            } => {
                let coefficient = context.try_mul(
                    &physical.momentum().loop_coefficients()[loop_index],
                    &physical.momentum().external_shift()[external_index],
                    limits,
                )?;
                context.try_mul(&coefficient, &context.integer(2), limits)?
            }
        };
        let expected = apply_sign(
            context,
            &unsigned,
            conventions.propagator().momentum_squared_sign(),
            family,
        )?;
        if !coefficients_equal(
            family,
            &expected,
            &family.denominators()[denominator].coefficients()[coordinate],
        )? {
            return Err(FamilyPresentationError::PhysicalDenominatorMismatch {
                denominator,
                component: PresentationDenominatorComponent::ScalarProduct { coordinate },
            });
        }
    }

    let momentum_constant = external_square(family, physical)?;
    let signed_momentum = apply_sign(
        context,
        &momentum_constant,
        conventions.propagator().momentum_squared_sign(),
        family,
    )?;
    let signed_mass = apply_sign(
        context,
        physical.mass_squared(),
        conventions.propagator().mass_squared_sign(),
        family,
    )?;
    let expected_constant = context.try_add(&signed_momentum, &signed_mass, limits)?;
    if !coefficients_equal(
        family,
        &expected_constant,
        family.denominators()[denominator].constant(),
    )? {
        return Err(FamilyPresentationError::PhysicalDenominatorMismatch {
            denominator,
            component: PresentationDenominatorComponent::Constant,
        });
    }
    Ok(())
}

fn external_square(
    family: &IntegralFamily,
    physical: &PhysicalPropagator,
) -> Result<Coefficient, FamilyPresentationError> {
    let context = family.coefficient_context();
    let limits = family.limits.exact_algebra;
    let shift = physical.momentum().external_shift();
    let mut total = context.zero();
    for (left, left_coefficient) in shift.iter().enumerate() {
        if left_coefficient.is_zero() {
            continue;
        }
        for (right, right_coefficient) in shift.iter().enumerate() {
            if right_coefficient.is_zero() || family.external_gram()[left][right].is_zero() {
                continue;
            }
            let pair = context.try_mul(left_coefficient, right_coefficient, limits)?;
            let contribution =
                context.try_mul(&pair, &family.external_gram()[left][right], limits)?;
            total = context.try_add(&total, &contribution, limits)?;
        }
    }
    Ok(total)
}

fn apply_sign(
    context: &CoefficientContext,
    value: &Coefficient,
    sign: AlgebraicSign,
    family: &IntegralFamily,
) -> Result<Coefficient, FamilyPresentationError> {
    match sign {
        AlgebraicSign::Positive => Ok(value.clone()),
        AlgebraicSign::Negative => Ok(context.try_neg(value, family.limits.exact_algebra)?),
    }
}
