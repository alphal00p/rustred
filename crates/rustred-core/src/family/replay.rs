//! Exact family replay and inverse verification.

use crate::algebra::matrix::determinant_of_coefficient_matrix;
use crate::algebra::{Coefficient, CoefficientContext, ExactAlgebraLimits};

use super::error::IntegralFamilyError;
use super::exact::{
    coefficients_are_equal, map_symbolica_matrix_error, symbolica_matrix_limits, verify_inverse,
};
use super::model::{DenominatorExpansion, IntegralFamily};

impl IntegralFamily {
    /// Recheck the inverse, every scalar-product round trip, and every cached
    /// derivative contraction in the free scalar-product module over `K`.
    pub fn verify_exact_replay(&self) -> Result<(), IntegralFamilyError> {
        let basis = self
            .denominators
            .iter()
            .map(|denominator| denominator.coefficients.clone())
            .collect::<Vec<_>>();
        let (replayed_determinant, _stats) = determinant_of_coefficient_matrix(
            &self.coefficients,
            &basis,
            symbolica_matrix_limits(self.limits),
        )
        .map_err(|error| map_symbolica_matrix_error(error, basis.len()))?;
        if &replayed_determinant != self.domain.basis_determinant() {
            return Err(IntegralFamilyError::InternalVerificationFailure {
                detail: "native determinant replay differs from the retained basis determinant"
                    .to_owned(),
            });
        }
        verify_inverse(&self.coefficients, &basis, &self.inverse_basis, self.limits)?;

        for coordinate in 0..self.coordinates.len() {
            let expansion = self.scalar_product_expansion(coordinate)?;
            let (constant, scalar_coefficients) = self.replay_denominator_expansion(&expansion)?;
            if !constant.is_zero() {
                return Err(IntegralFamilyError::InternalVerificationFailure {
                    detail: format!(
                        "scalar-product coordinate {coordinate} has nonzero replay constant"
                    ),
                });
            }
            for (candidate, coefficient) in scalar_coefficients.iter().enumerate() {
                let expected = if candidate == coordinate {
                    self.coefficients.one()
                } else {
                    self.coefficients.zero()
                };
                if !coefficients_are_equal(
                    &self.coefficients,
                    coefficient,
                    &expected,
                    self.limits.exact_algebra,
                )? {
                    return Err(IntegralFamilyError::InternalVerificationFailure {
                        detail: format!(
                            "scalar-product coordinate {coordinate} replays incorrectly at coordinate {candidate}"
                        ),
                    });
                }
            }
        }

        for denominator in 0..self.denominator_count() {
            for differentiated_loop in 0..self.loop_count() {
                for (contraction_index, &contraction) in self.contractions.iter().enumerate() {
                    let direct =
                        self.direct_derivative(denominator, differentiated_loop, contraction)?;
                    let cached = &self.derivative_contractions[denominator][differentiated_loop]
                        [contraction_index];
                    let replayed = self.replay_denominator_expansion(cached)?;
                    if !affine_forms_are_equal(
                        &self.coefficients,
                        &direct,
                        &replayed,
                        self.limits.exact_algebra,
                    )? {
                        return Err(IntegralFamilyError::InternalVerificationFailure {
                            detail: format!(
                                "derivative contraction D_{denominator}, k_{differentiated_loop}, {contraction:?} does not replay"
                            ),
                        });
                    }
                }
            }
        }
        Ok(())
    }

    fn replay_denominator_expansion(
        &self,
        expansion: &DenominatorExpansion,
    ) -> Result<(Coefficient, Vec<Coefficient>), IntegralFamilyError> {
        let mut constant = expansion.constant.clone();
        let mut scalar_coefficients = vec![self.coefficients.zero(); self.coordinates.len()];
        for (denominator_coefficient, denominator) in expansion
            .denominator_coefficients
            .iter()
            .zip(&self.denominators)
        {
            let contribution = self.coefficients.try_mul(
                denominator_coefficient,
                &denominator.constant,
                self.limits.exact_algebra,
            )?;
            constant =
                self.coefficients
                    .try_add(&constant, &contribution, self.limits.exact_algebra)?;
            for (coordinate, basis_coefficient) in denominator.coefficients.iter().enumerate() {
                let contribution = self.coefficients.try_mul(
                    denominator_coefficient,
                    basis_coefficient,
                    self.limits.exact_algebra,
                )?;
                scalar_coefficients[coordinate] = self.coefficients.try_add(
                    &scalar_coefficients[coordinate],
                    &contribution,
                    self.limits.exact_algebra,
                )?;
            }
        }
        Ok((constant, scalar_coefficients))
    }
}

fn affine_forms_are_equal(
    context: &CoefficientContext,
    left: &(Coefficient, Vec<Coefficient>),
    right: &(Coefficient, Vec<Coefficient>),
    limits: ExactAlgebraLimits,
) -> Result<bool, IntegralFamilyError> {
    if left.1.len() != right.1.len() || !coefficients_are_equal(context, &left.0, &right.0, limits)?
    {
        return Ok(false);
    }
    for (left, right) in left.1.iter().zip(&right.1) {
        if !coefficients_are_equal(context, left, right, limits)? {
            return Ok(false);
        }
    }
    Ok(true)
}
