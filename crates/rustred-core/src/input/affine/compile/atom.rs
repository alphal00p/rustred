use symbolica::coefficient::SerializedRational;
use symbolica::prelude::{AtomCore, AtomView, CoefficientView, Q, Z};

use crate::algebra::Coefficient;

use super::super::budget::{planned_operation_polynomial_census, signed_i64_magnitude_bits};
use super::super::construction::check_limit;
use super::super::error::SymbolicaAffineDenominatorError;
use super::super::model::SymbolicaAffineDenominatorCompiler;

impl SymbolicaAffineDenominatorCompiler {
    pub(in crate::input::affine) fn numeric_coefficient(
        &self,
        atom: AtomView<'_>,
    ) -> Result<Coefficient, SymbolicaAffineDenominatorError> {
        let AtomView::Num(number) = atom else {
            return Err(SymbolicaAffineDenominatorError::UnsupportedNumericAtom(
                atom.to_owned(),
            ));
        };
        let (numerator_bits, denominator_bits) = match number.get_coeff_view() {
            CoefficientView::Natural(real_numerator, real_denominator, imaginary, _)
                if imaginary == 0 =>
            {
                (
                    signed_i64_magnitude_bits(real_numerator),
                    signed_i64_magnitude_bits(real_denominator),
                )
            }
            CoefficientView::Large(real, imaginary) if imaginary.is_zero() => match real {
                SerializedRational::Natural(numerator, denominator) => (
                    signed_i64_magnitude_bits(numerator),
                    signed_i64_magnitude_bits(denominator),
                ),
                // The packed large-rational fields are intentionally opaque.
                // Their complete serialized Atom size is a conservative bit
                // envelope and can be inspected without cloning GMP storage.
                SerializedRational::Large(_) => {
                    let bits = atom.get_byte_size().checked_mul(8).ok_or(
                        SymbolicaAffineDenominatorError::ResourceCountOverflow {
                            resource: "numeric Atom magnitude bits",
                        },
                    )?;
                    (bits, bits)
                }
            },
            _ => {
                return Err(SymbolicaAffineDenominatorError::UnsupportedNumericAtom(
                    atom.to_owned(),
                ));
            }
        };
        let mut planned = planned_operation_polynomial_census(
            1,
            self.combined.parameter_names().len(),
            numerator_bits,
        )?;
        planned.checked_add_assign(
            planned_operation_polynomial_census(
                1,
                self.combined.parameter_names().len(),
                denominator_bits,
            )?,
            "numeric Atom allocation envelope",
        )?;
        check_limit(
            "numeric Atom integer bits",
            planned.integer_bits,
            self.limits.max_coefficient_integer_bits,
        )?;
        check_limit(
            "numeric Atom retained bytes",
            planned.retained_bytes,
            self.limits.max_combined_retained_bytes,
        )?;
        let result = atom
            .try_to_rational_polynomial(&Q, &Z, Some(self.combined.variables().clone()))
            .map_err(|_| {
                SymbolicaAffineDenominatorError::UnsupportedNumericAtom(atom.to_owned())
            })?;
        self.combined
            .validate_with_limits(&result, self.limits.exact_algebra)?;
        self.validate_retained_shape(&result)?;
        Ok(result)
    }
}
