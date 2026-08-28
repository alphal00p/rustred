//! Checked affine translations of indexed coefficients and polynomials.

use symbolica::domains::rational_polynomial::FromNumeratorAndDenominator;
use symbolica::prelude::*;

use crate::algebra::{Coefficient, CoefficientPolynomial, validate_polynomial_on_map};

use super::context::IndexedCoefficientContext;
use super::error::IndexedAlgebraError;
use super::limits::{
    IndexedAlgebraLimits, ceil_log2, check_limit, checked_indexed_add, checked_indexed_mul,
    integer_magnitude_bits, verify_polynomial_execution_envelope,
};
use super::value::{IndexedCoefficient, IndexedPolynomial};

/// Prospective mathematical bounds used immediately by one translation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct TranslationPreflight {
    output_term_bound: usize,
    output_exponent_entry_bound: usize,
    largest_output_integer_bit_bound: usize,
}

impl IndexedCoefficientContext {
    /// Apply `n -> n + shift` to a complete coefficient.
    pub fn translate(
        &self,
        value: &IndexedCoefficient,
        shift: &[i64],
        limits: IndexedAlgebraLimits,
    ) -> Result<IndexedCoefficient, IndexedAlgebraError> {
        self.validate_with_limits(value, limits.exact_algebra)?;
        self.validate_index_arity(shift)?;
        self.translate_coefficient_validated(value, shift, limits)
    }

    fn translate_coefficient_validated(
        &self,
        value: &IndexedCoefficient,
        shift: &[i64],
        limits: IndexedAlgebraLimits,
    ) -> Result<IndexedCoefficient, IndexedAlgebraError> {
        let numerator_preflight =
            self.preflight_translate_polynomial_raw(&value.raw.numerator, shift, limits)?;
        let denominator_preflight =
            self.preflight_translate_polynomial_raw(&value.raw.denominator, shift, limits)?;
        let numerator = self.execute_translate_polynomial_raw(
            &value.raw.numerator,
            shift,
            limits,
            numerator_preflight,
        )?;
        let denominator = self.execute_translate_polynomial_raw(
            &value.raw.denominator,
            shift,
            limits,
            denominator_preflight,
        )?;
        if denominator.is_zero() {
            return Err(IndexedAlgebraError::ZeroDenominator);
        }
        let raw = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            // Translation is a polynomial-ring automorphism. The validated
            // source numerator and denominator are coprime, hence their
            // translated images are coprime too. Avoid a redundant native
            // GCD and its otherwise unbounded transient workspace.
            <Coefficient as FromNumeratorAndDenominator<IntegerRing, IntegerRing, u16>>::from_num_den(
                numerator,
                denominator,
                &Z,
                false,
            )
        }))
        .map_err(|_| {
            IndexedAlgebraError::Symbolica(
                "Symbolica panicked while normalizing a checked parametric translation".to_owned(),
            )
        })?;
        self.wrap_checked_with_limits(raw, limits.exact_algebra)
    }

    pub fn translate_polynomial(
        &self,
        value: &IndexedPolynomial,
        shift: &[i64],
        limits: IndexedAlgebraLimits,
    ) -> Result<IndexedPolynomial, IndexedAlgebraError> {
        self.validate_polynomial_with_limits(value, limits.exact_algebra)?;
        self.validate_index_arity(shift)?;
        Ok(IndexedPolynomial {
            raw: self.translate_polynomial_raw(&value.raw, shift, limits)?,
            context: self.fingerprint.clone(),
        })
    }

    fn translate_polynomial_raw(
        &self,
        source: &CoefficientPolynomial,
        shift: &[i64],
        limits: IndexedAlgebraLimits,
    ) -> Result<CoefficientPolynomial, IndexedAlgebraError> {
        let preflight = self.preflight_translate_polynomial_raw(source, shift, limits)?;
        self.execute_translate_polynomial_raw(source, shift, limits, preflight)
    }

    fn preflight_translate_polynomial_raw(
        &self,
        source: &CoefficientPolynomial,
        shift: &[i64],
        limits: IndexedAlgebraLimits,
    ) -> Result<TranslationPreflight, IndexedAlgebraError> {
        validate_polynomial_on_map(
            source,
            &self.variables,
            crate::algebra::CoefficientPolynomialPart::Numerator,
            limits.exact_algebra,
        )?;
        let base_count = self.base.variables().len();
        let mut output_term_bound = 0_usize;
        let mut power_operation_bound = 0_usize;
        let mut largest_contribution_bits = 0usize;
        for (coefficient, exponents) in source.coefficients.iter().zip(source.exponents_iter()) {
            let mut term_bound = 1_usize;
            for (position, offset) in shift.iter().enumerate() {
                if *offset == 0 {
                    continue;
                }
                let exponent = usize::from(exponents[base_count + position]);
                if exponent != 0 {
                    power_operation_bound = checked_indexed_add(
                        "parametric translation power operations",
                        power_operation_bound,
                        term_bound,
                    )?;
                }
                term_bound = checked_indexed_mul(
                    "parametric translation output terms",
                    term_bound,
                    exponent + 1,
                )?;
            }
            output_term_bound = checked_indexed_add(
                "parametric translation output terms",
                output_term_bound,
                term_bound,
            )?;
            let mut requested = integer_magnitude_bits(coefficient);
            for (position, offset) in shift.iter().enumerate() {
                if *offset == 0 {
                    continue;
                }
                let exponent = u128::from(exponents[base_count + position]);
                if exponent == 0 {
                    continue;
                }
                requested = requested.checked_add(exponent).ok_or(
                    IndexedAlgebraError::ResourceCountOverflow {
                        resource: "parametric translation integer bits",
                    },
                )?;
                let offset_bits = u128::from(i64::BITS - offset.unsigned_abs().leading_zeros());
                if offset_bits > 1 {
                    requested = requested
                        .checked_add(offset_bits.checked_mul(exponent).ok_or(
                            IndexedAlgebraError::ResourceCountOverflow {
                                resource: "parametric translation integer bits",
                            },
                        )?)
                        .ok_or(IndexedAlgebraError::ResourceCountOverflow {
                            resource: "parametric translation integer bits",
                        })?;
                }
            }
            let requested = usize::try_from(requested).map_err(|_| {
                IndexedAlgebraError::ResourceCountOverflow {
                    resource: "parametric translation integer bits",
                }
            })?;
            check_limit(
                "parametric translation integer bits",
                requested,
                limits.max_specialization_integer_bits,
            )?;
            largest_contribution_bits = largest_contribution_bits.max(requested);
        }
        check_limit(
            "parametric translation output terms",
            output_term_bound,
            limits.exact_algebra.max_polynomial_terms,
        )?;
        check_limit(
            "parametric translation power operations",
            power_operation_bound,
            limits.max_specialization_power_operations,
        )?;

        // Expanding (n+a)^e produces coefficients containing binomial(e,k)
        // and powers of `a`. For each contribution use binomial(e,k) <= 2^e,
        // then charge ceil(log2(output_term_bound)) for worst-case collection
        // of equal monomials.
        let collision_bits = ceil_log2(output_term_bound);
        let collected_bits = largest_contribution_bits
            .checked_add(collision_bits)
            .ok_or(IndexedAlgebraError::ResourceCountOverflow {
                resource: "parametric translation integer bits",
            })?;
        check_limit(
            "parametric translation integer bits",
            collected_bits,
            limits.max_specialization_integer_bits,
        )?;
        let output_exponent_entry_bound = checked_indexed_mul(
            "parametric translation output exponent entries",
            output_term_bound,
            self.variables.len(),
        )?;
        Ok(TranslationPreflight {
            output_term_bound,
            output_exponent_entry_bound,
            largest_output_integer_bit_bound: collected_bits,
        })
    }

    fn execute_translate_polynomial_raw(
        &self,
        source: &CoefficientPolynomial,
        shift: &[i64],
        limits: IndexedAlgebraLimits,
        preflight: TranslationPreflight,
    ) -> Result<CoefficientPolynomial, IndexedAlgebraError> {
        let mut result = source.clone();
        let base_count = self.base.variables().len();
        for (position, offset) in shift.iter().enumerate() {
            if *offset == 0 {
                continue;
            }
            let variable_position = base_count + position;
            if !source
                .exponents_iter()
                .any(|exponents| exponents[variable_position] != 0)
            {
                // The preflight correctly charges no offset bits when this
                // index is absent.
                continue;
            }
            let variable = self
                .template
                .numerator
                .variable(&self.index_variables[position])
                .map_err(IndexedAlgebraError::Symbolica)?;
            result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let replacement =
                    &variable + &self.template.numerator.constant(Integer::from(*offset));
                result.replace_with_poly(variable_position, &replacement)
            }))
            .map_err(|_| {
                IndexedAlgebraError::Symbolica(
                    "Symbolica panicked during checked parametric translation".to_owned(),
                )
            })?;
        }
        if result.variables.as_ref() != self.variables.as_ref() {
            return Err(IndexedAlgebraError::WrongContext);
        }
        verify_polynomial_execution_envelope(
            &result,
            preflight.output_term_bound,
            preflight.output_exponent_entry_bound,
            preflight.largest_output_integer_bit_bound,
            "parametric translation",
        )?;
        validate_polynomial_on_map(
            &result,
            &self.variables,
            crate::algebra::CoefficientPolynomialPart::Numerator,
            limits.exact_algebra,
        )?;
        Ok(result)
    }
}
