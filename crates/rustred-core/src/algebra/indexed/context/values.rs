use std::mem::size_of;

use symbolica::domains::rational_polynomial::FromNumeratorAndDenominator;
use symbolica::prelude::*;

use crate::algebra::{
    Coefficient, CoefficientPolynomial, ExactAlgebraLimits, validate_coefficient_on_map,
    validate_polynomial_on_map,
};

use super::super::error::IndexedAlgebraError;
use super::super::limits::integer_magnitude_bits;
use super::super::value::{IndexedCoefficient, IndexedPolynomial};
use super::{BoundIndexedCoefficient, IndexedCoefficientContext};

impl IndexedCoefficientContext {
    /// Parse one exact rational expression directly into this authenticated
    /// indexed field.
    ///
    /// This is an untrusted-ingress operation intended for immutable external
    /// artifact compilers and loaders.  Symbolica performs the expression to
    /// rational-polynomial conversion on the context's exact variable map;
    /// RustRed then authenticates the complete sparse result under `limits`
    /// before attaching the context seal.  Callers must impose their own byte
    /// limit on `expression` before parsing it.
    pub fn parse_expression_with_limits(
        &self,
        expression: &str,
        limits: ExactAlgebraLimits,
    ) -> Result<IndexedCoefficient, IndexedAlgebraError> {
        let atom = try_parse!(expression, default_namespace = "rustred")
            .map_err(|error| IndexedAlgebraError::Symbolica(error.to_string()))?;
        let raw = atom
            .as_view()
            .try_to_rational_polynomial(&Q, &Z, Some(self.variables.clone()))
            .map_err(|error| IndexedAlgebraError::Symbolica(error.to_string()))?;
        self.wrap_checked_with_limits(raw, limits)
    }

    pub fn zero(&self) -> IndexedCoefficient {
        self.wrap_sealed(self.template.numerator.zero().into())
    }

    pub fn one(&self) -> IndexedCoefficient {
        self.wrap_sealed(self.template.numerator.one().into())
    }

    pub fn integer(&self, value: i64) -> IndexedCoefficient {
        self.wrap_sealed(
            self.template
                .numerator
                .constant(Integer::from(value))
                .into(),
        )
    }

    pub fn index(&self, position: usize) -> Result<IndexedCoefficient, IndexedAlgebraError> {
        let variable =
            self.index_variables
                .get(position)
                .ok_or(IndexedAlgebraError::WrongIndexArity {
                    expected: self.index_count(),
                    actual: position.saturating_add(1),
                })?;
        let polynomial = self
            .template
            .numerator
            .variable(variable)
            .map_err(IndexedAlgebraError::Symbolica)?;
        Ok(self.wrap_sealed(polynomial.into()))
    }

    pub fn lift(&self, value: &Coefficient) -> Result<IndexedCoefficient, IndexedAlgebraError> {
        self.base
            .validate_with_limits(value, ExactAlgebraLimits::default())
            .map_err(|_| IndexedAlgebraError::WrongContext)?;
        let numerator = self.extend_authenticated_base_polynomial(&value.numerator);
        let denominator = self.extend_authenticated_base_polynomial(&value.denominator);
        if denominator.is_zero() {
            return Err(IndexedAlgebraError::ZeroDenominator);
        }
        let raw = <Coefficient as FromNumeratorAndDenominator<
            IntegerRing,
            IntegerRing,
            u16,
        >>::from_num_den(numerator, denominator, &Z, true);
        self.wrap_checked(raw)
    }

    pub fn lift_base_polynomial(
        &self,
        value: &CoefficientPolynomial,
    ) -> Result<IndexedPolynomial, IndexedAlgebraError> {
        let raw = self.extend_base_polynomial(value)?;
        Ok(IndexedPolynomial {
            raw,
            context: self.fingerprint.clone(),
        })
    }

    /// Return the canonical primitive associate of one authenticated nonzero
    /// integer polynomial.
    ///
    /// Exact guard predicates are zero loci, so multiplication by a nonzero
    /// integer constant must not create another branch. Symbolica removes the
    /// integer content; RustRed fixes the remaining unit by requiring a
    /// positive leading coefficient. Callers must reject zero before entering
    /// this boundary.
    pub(crate) fn primitive_guard_associate_with_limits(
        &self,
        value: &IndexedPolynomial,
        limits: ExactAlgebraLimits,
        serialization_byte_limit: usize,
    ) -> Result<IndexedPolynomial, IndexedAlgebraError> {
        self.validate_polynomial_with_limits(value, limits)?;
        debug_assert!(!value.is_zero());
        preflight_guard_polynomial_payload(&value.raw, serialization_byte_limit)?;
        let mut raw = value.raw.clone().make_primitive();
        if raw.lcoeff().is_negative() {
            raw = raw.mul_coeff(Integer::from(-1));
        }
        validate_polynomial_on_map(
            &raw,
            &self.variables,
            crate::algebra::CoefficientPolynomialPart::Numerator,
            limits,
        )?;
        preflight_guard_polynomial_payload(&raw, serialization_byte_limit)?;
        Ok(IndexedPolynomial {
            raw,
            context: self.fingerprint.clone(),
        })
    }

    pub fn denominator_condition_with_limits(
        &self,
        value: &IndexedCoefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<IndexedPolynomial, IndexedAlgebraError> {
        let value = self.authenticate_coefficient_with_limits(value, limits)?;
        self.denominator_condition_from_bound(value)
    }

    pub fn numerator_condition_with_limits(
        &self,
        value: &IndexedCoefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<IndexedPolynomial, IndexedAlgebraError> {
        let value = self.authenticate_coefficient_with_limits(value, limits)?;
        self.numerator_condition_from_bound(value)
    }

    pub(crate) fn numerator_condition_from_bound(
        &self,
        value: BoundIndexedCoefficient<'_, '_>,
    ) -> Result<IndexedPolynomial, IndexedAlgebraError> {
        self.validate_bound(value)?;
        Ok(IndexedPolynomial {
            raw: value.value.raw.numerator.clone(),
            context: self.fingerprint.clone(),
        })
    }

    /// Extract a denominator from a coefficient already authenticated or
    /// sealed to this exact context.
    pub(crate) fn denominator_condition_from_bound(
        &self,
        value: BoundIndexedCoefficient<'_, '_>,
    ) -> Result<IndexedPolynomial, IndexedAlgebraError> {
        self.validate_bound(value)?;
        Ok(IndexedPolynomial {
            raw: value.value.raw.denominator.clone(),
            context: self.fingerprint.clone(),
        })
    }

    pub(super) fn wrap_sealed(&self, raw: Coefficient) -> IndexedCoefficient {
        debug_assert!(self.raw_has_sealed_shape(&raw));
        IndexedCoefficient {
            raw,
            context: self.fingerprint.clone(),
        }
    }

    fn raw_has_sealed_shape(&self, raw: &Coefficient) -> bool {
        let polynomial_has_shape = |polynomial: &CoefficientPolynomial| {
            polynomial.variables.as_ref() == self.variables.as_ref()
                && polynomial
                    .coefficients
                    .len()
                    .checked_mul(self.variables.len())
                    == Some(polynomial.exponents.len())
        };
        polynomial_has_shape(&raw.numerator)
            && polynomial_has_shape(&raw.denominator)
            && !raw.denominator.coefficients.is_empty()
    }

    fn wrap_checked(&self, raw: Coefficient) -> Result<IndexedCoefficient, IndexedAlgebraError> {
        self.wrap_checked_with_limits(raw, ExactAlgebraLimits::default())
    }

    pub(in crate::algebra::indexed) fn wrap_checked_with_limits(
        &self,
        raw: Coefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<IndexedCoefficient, IndexedAlgebraError> {
        validate_coefficient_on_map(&raw, &self.variables, limits)?;
        self.record_authenticated_native_result();
        Ok(self.wrap_sealed(raw))
    }

    /// Admit one raw rational function returned by a native Symbolica
    /// algorithm which consumed values from this exact indexed context.
    ///
    /// Native coefficient fields do not carry RustRed's variable-map
    /// identity. This crate-private seam performs the complete map, layout,
    /// exponent, and resource authentication exactly once before sealing the
    /// result for trusted downstream arithmetic.
    pub(crate) fn admit_native_result_with_limits(
        &self,
        raw: Coefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<IndexedCoefficient, IndexedAlgebraError> {
        self.wrap_checked_with_limits(raw, limits)
    }

    /// Admit one raw polynomial returned by a native Symbolica algorithm
    /// which consumed values from this exact indexed context.
    ///
    /// This is the polynomial counterpart of
    /// [`Self::admit_native_result_with_limits`].  It authenticates the exact
    /// variable map, sparse layout, exponent ceiling, and retained-term limit
    /// before sealing the result with this context identity.  Callers must
    /// still authenticate every native input before invoking Symbolica.
    pub(crate) fn admit_native_polynomial_result_with_limits(
        &self,
        raw: CoefficientPolynomial,
        limits: ExactAlgebraLimits,
    ) -> Result<IndexedPolynomial, IndexedAlgebraError> {
        validate_polynomial_on_map(
            &raw,
            &self.variables,
            crate::algebra::CoefficientPolynomialPart::Numerator,
            limits,
        )?;
        self.record_authenticated_native_result();
        Ok(IndexedPolynomial {
            raw,
            context: self.fingerprint.clone(),
        })
    }

    fn extend_base_polynomial(
        &self,
        source: &CoefficientPolynomial,
    ) -> Result<CoefficientPolynomial, IndexedAlgebraError> {
        validate_polynomial_on_map(
            source,
            self.base.variables(),
            crate::algebra::CoefficientPolynomialPart::Numerator,
            ExactAlgebraLimits::default(),
        )?;
        Ok(self.extend_authenticated_base_polynomial(source))
    }

    fn extend_authenticated_base_polynomial(
        &self,
        source: &CoefficientPolynomial,
    ) -> CoefficientPolynomial {
        let mut result = self
            .template
            .numerator
            .zero_with_capacity(source.coefficients.len());
        let mut exponents = vec![0_u16; self.variables.len()];
        for (coefficient, source_exponents) in
            source.coefficients.iter().zip(source.exponents_iter())
        {
            exponents.fill(0);
            exponents[..self.base.variables().len()].copy_from_slice(source_exponents);
            result.append_monomial(coefficient.clone(), &exponents);
        }
        result
    }
}

/// Conservatively bound the complete sparse payload before cloning or
/// formatting a guard polynomial. The rational upper approximation
/// `30103 / 100000 > log10(2)` bounds decimal coefficient strings without
/// allocating them; the second envelope includes the cloned integer and
/// exponent buffers.
fn preflight_guard_polynomial_payload(
    polynomial: &CoefficientPolynomial,
    byte_limit: usize,
) -> Result<(), IndexedAlgebraError> {
    const LOG10_2_NUMERATOR_UPPER: usize = 30_103;
    const LOG10_2_DENOMINATOR: usize = 100_000;
    let mut serialized = 0usize;
    let mut cloned = polynomial
        .coefficients
        .len()
        .checked_mul(size_of::<Integer>())
        .and_then(|bytes| {
            polynomial
                .exponents
                .len()
                .checked_mul(size_of::<u16>())
                .and_then(|exponents| bytes.checked_add(exponents))
        })
        .ok_or(IndexedAlgebraError::ResourceCountOverflow {
            resource: "guard polynomial cloned payload bytes",
        })?;
    for (coefficient, exponents) in polynomial
        .coefficients
        .iter()
        .zip(polynomial.exponents_iter())
    {
        let bits = usize::try_from(integer_magnitude_bits(coefficient)).map_err(|_| {
            IndexedAlgebraError::ResourceCountOverflow {
                resource: "guard polynomial serialized payload bytes",
            }
        })?;
        let digits = bits
            .checked_mul(LOG10_2_NUMERATOR_UPPER)
            .and_then(|scaled| scaled.checked_add(LOG10_2_DENOMINATOR - 1))
            .map(|rounded| rounded / LOG10_2_DENOMINATOR)
            .ok_or(IndexedAlgebraError::ResourceCountOverflow {
                resource: "guard polynomial serialized payload bytes",
            })?
            .max(1);
        let coefficient_bytes = digits
            .checked_add(usize::from(coefficient.is_negative()))
            .ok_or(IndexedAlgebraError::ResourceCountOverflow {
                resource: "guard polynomial serialized payload bytes",
            })?;
        let mut term = decimal_digits(coefficient_bytes)
            .checked_add(1)
            .and_then(|bytes| bytes.checked_add(coefficient_bytes))
            .and_then(|bytes| bytes.checked_add(2))
            .ok_or(IndexedAlgebraError::ResourceCountOverflow {
                resource: "guard polynomial serialized payload bytes",
            })?;
        for (position, &exponent) in exponents.iter().enumerate() {
            if position != 0 {
                term = term
                    .checked_add(1)
                    .ok_or(IndexedAlgebraError::ResourceCountOverflow {
                        resource: "guard polynomial serialized payload bytes",
                    })?;
            }
            term = term
                .checked_add(decimal_digits(usize::from(exponent)))
                .ok_or(IndexedAlgebraError::ResourceCountOverflow {
                    resource: "guard polynomial serialized payload bytes",
                })?;
        }
        term = term
            .checked_add(2)
            .ok_or(IndexedAlgebraError::ResourceCountOverflow {
                resource: "guard polynomial serialized payload bytes",
            })?;
        serialized =
            serialized
                .checked_add(term)
                .ok_or(IndexedAlgebraError::ResourceCountOverflow {
                    resource: "guard polynomial serialized payload bytes",
                })?;
        if let Integer::Large(value) = coefficient {
            let capacity_bits = usize::try_from(value.capacity()).map_err(|_| {
                IndexedAlgebraError::ResourceCountOverflow {
                    resource: "guard polynomial cloned payload bytes",
                }
            })?;
            let capacity_bytes = capacity_bits
                .checked_add(7)
                .map(|rounded| rounded / 8)
                .ok_or(IndexedAlgebraError::ResourceCountOverflow {
                    resource: "guard polynomial cloned payload bytes",
                })?;
            cloned = cloned.checked_add(capacity_bytes).ok_or(
                IndexedAlgebraError::ResourceCountOverflow {
                    resource: "guard polynomial cloned payload bytes",
                },
            )?;
        }
        if serialized > byte_limit {
            return Err(IndexedAlgebraError::ResourceLimit {
                resource: "guard polynomial serialized payload bytes",
                requested: serialized,
                limit: byte_limit,
            });
        }
    }
    if cloned > byte_limit {
        return Err(IndexedAlgebraError::ResourceLimit {
            resource: "guard polynomial cloned payload bytes",
            requested: cloned,
            limit: byte_limit,
        });
    }
    Ok(())
}

fn decimal_digits(mut value: usize) -> usize {
    let mut digits = 1usize;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}
