//! Checked specialization from the indexed field back to its base field.

use symbolica::domains::rational_polynomial::FromNumeratorAndDenominator;
use symbolica::prelude::*;

use crate::algebra::{
    Coefficient, CoefficientPolynomial, validate_coefficient_on_map, validate_polynomial_on_map,
};

use super::context::IndexedCoefficientContext;
use super::error::IndexedAlgebraError;
use super::limits::{
    IndexedAlgebraLimits, ceil_log2, check_limit, checked_indexed_add, checked_indexed_mul,
    integer_magnitude_bits, verify_polynomial_execution_envelope,
};
use super::value::{IndexedCoefficient, IndexedPolynomial};

/// Prospective mathematical bounds used immediately by one specialization.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct SpecializationPreflight {
    output_term_bound: usize,
    output_exponent_entry_bound: usize,
    largest_output_integer_bit_bound: usize,
}

impl IndexedCoefficientContext {
    /// Simultaneously specialize every index of an authenticated polynomial
    /// and project it to the exact base variable map.
    ///
    /// Unlike [`Self::specialize`], this operation performs no rational
    /// normalization: the returned polynomial is the exact mapped condition.
    /// This is the boundary used by domain owners that must retain a
    /// pre-cancellation nonzero condition at a concrete lattice point.
    pub fn specialize_polynomial(
        &self,
        value: &IndexedPolynomial,
        assignment: &[i64],
        limits: IndexedAlgebraLimits,
    ) -> Result<CoefficientPolynomial, IndexedAlgebraError> {
        self.validate_polynomial_with_limits(value, limits.exact_algebra)?;
        self.validate_index_arity(assignment)?;
        let preflight =
            self.preflight_specialize_polynomial_raw(value.raw(), assignment, limits)?;
        self.execute_specialize_polynomial_raw(value.raw(), assignment, limits, preflight)
    }

    /// Specialize a polynomial already authenticated by a sealed semantic
    /// owner. Artifact loading validates the payload once; reducer hot paths
    /// therefore check only its context seal before doing the bounded exact
    /// substitution.
    pub(crate) fn specialize_polynomial_sealed(
        &self,
        value: &IndexedPolynomial,
        assignment: &[i64],
        limits: IndexedAlgebraLimits,
    ) -> Result<CoefficientPolynomial, IndexedAlgebraError> {
        self.validate_polynomial_context(value)?;
        self.validate_index_arity(assignment)?;
        let preflight =
            self.preflight_specialize_polynomial_raw(value.raw(), assignment, limits)?;
        self.execute_specialize_polynomial_raw(value.raw(), assignment, limits, preflight)
    }

    /// Simultaneously specialize every index and project the result to the
    /// exact base variable map.
    ///
    /// The first return value is the normalized base-field coefficient. The
    /// optional polynomial is the mapped, nonconstant denominator *before*
    /// normalization and cancellation; callers that use the value outside a
    /// known generic domain must retain that polynomial as a nonzero guard.
    /// Constant mapped denominators return `None`.
    pub fn specialize(
        &self,
        value: &IndexedCoefficient,
        assignment: &[i64],
        limits: IndexedAlgebraLimits,
    ) -> Result<(Coefficient, Option<CoefficientPolynomial>), IndexedAlgebraError> {
        self.validate_with_limits(value, limits.exact_algebra)?;
        self.validate_index_arity(assignment)?;
        self.specialize_authenticated(value, assignment, limits)
    }

    /// Specialize a coefficient owned by a sealed in-process semantic owner.
    ///
    /// Artifact installation already authenticated the complete coefficient
    /// payload.  This crate-private path checks only the exact context seal,
    /// then retains the same prospective work bounds and output
    /// authentication as the public untrusted-ingress operation.
    pub(crate) fn specialize_sealed(
        &self,
        value: &IndexedCoefficient,
        assignment: &[i64],
        limits: IndexedAlgebraLimits,
    ) -> Result<(Coefficient, Option<CoefficientPolynomial>), IndexedAlgebraError> {
        self.bind_sealed(value)?;
        self.validate_index_arity(assignment)?;
        self.specialize_authenticated(value, assignment, limits)
    }

    fn specialize_authenticated(
        &self,
        value: &IndexedCoefficient,
        assignment: &[i64],
        limits: IndexedAlgebraLimits,
    ) -> Result<(Coefficient, Option<CoefficientPolynomial>), IndexedAlgebraError> {
        let numerator_preflight =
            self.preflight_specialize_polynomial_raw(&value.raw.numerator, assignment, limits)?;
        let denominator_preflight =
            self.preflight_specialize_polynomial_raw(&value.raw.denominator, assignment, limits)?;
        check_coefficient_specialization_normalization_limits(
            &value.raw.numerator,
            &value.raw.denominator,
            numerator_preflight,
            denominator_preflight,
            value.raw.numerator.is_zero(),
            value.raw.denominator.is_one(),
            self.base.variables().len(),
            limits,
        )?;
        let numerator = self.execute_specialize_polynomial_raw(
            &value.raw.numerator,
            assignment,
            limits,
            numerator_preflight,
        )?;
        let denominator = self.execute_specialize_polynomial_raw(
            &value.raw.denominator,
            assignment,
            limits,
            denominator_preflight,
        )?;
        if denominator.is_zero() {
            return Err(IndexedAlgebraError::ZeroDenominator);
        }
        let denominator_nonzero = if denominator.is_constant() {
            None
        } else {
            Some(denominator.clone())
        };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            <Coefficient as FromNumeratorAndDenominator<IntegerRing, IntegerRing, u16>>::from_num_den(
                numerator,
                denominator,
                &Z,
                true,
            )
        }))
        .map_err(|_| {
            IndexedAlgebraError::Symbolica(
                "Symbolica panicked while normalizing a checked coefficient specialization"
                    .to_owned(),
            )
        })?;
        validate_coefficient_on_map(&result, self.base.variables(), limits.exact_algebra)?;
        Ok((result, denominator_nonzero))
    }

    fn preflight_specialize_polynomial_raw(
        &self,
        source: &CoefficientPolynomial,
        assignment: &[i64],
        limits: IndexedAlgebraLimits,
    ) -> Result<SpecializationPreflight, IndexedAlgebraError> {
        check_limit(
            "coefficient specialization output terms",
            source.nterms(),
            limits.exact_algebra.max_polynomial_terms,
        )?;
        let operations = source.nterms().checked_mul(self.index_count()).ok_or(
            IndexedAlgebraError::ResourceCountOverflow {
                resource: "coefficient specialization power operations",
            },
        )?;
        check_limit(
            "coefficient specialization power operations",
            operations,
            limits.max_specialization_power_operations,
        )?;

        let base_count = self.base.variables().len();
        // Preflight every arbitrary-precision power before constructing any
        // output coefficient.  Counting calls alone is insufficient:
        // `value^exponent` can allocate an integer linear in `exponent` bits
        // even when the source polynomial contains only one term.
        let mut largest_term_bits = 0usize;
        for (coefficient, exponents) in source.coefficients.iter().zip(source.exponents_iter()) {
            let requested =
                specialization_integer_bit_bound(coefficient, exponents, base_count, assignment)?;
            check_limit(
                "coefficient specialization integer bits",
                requested,
                limits.max_specialization_integer_bits,
            )?;
            largest_term_bits = largest_term_bits.max(requested);
        }
        let collision_bits = ceil_log2(source.nterms());
        let collected_bits = largest_term_bits.checked_add(collision_bits).ok_or(
            IndexedAlgebraError::ResourceCountOverflow {
                resource: "coefficient specialization integer bits",
            },
        )?;
        check_limit(
            "coefficient specialization integer bits",
            collected_bits,
            limits.max_specialization_integer_bits,
        )?;
        let output_exponent_entry_bound = checked_indexed_mul(
            "coefficient specialization output exponent entries",
            source.nterms(),
            base_count,
        )?;
        Ok(SpecializationPreflight {
            output_term_bound: source.nterms(),
            output_exponent_entry_bound,
            largest_output_integer_bit_bound: collected_bits,
        })
    }

    fn execute_specialize_polynomial_raw(
        &self,
        source: &CoefficientPolynomial,
        assignment: &[i64],
        limits: IndexedAlgebraLimits,
        preflight: SpecializationPreflight,
    ) -> Result<CoefficientPolynomial, IndexedAlgebraError> {
        let base_count = self.base.variables().len();
        let mut result = self
            .base
            .template()
            .numerator
            .zero_with_capacity(source.nterms());
        for (coefficient, exponents) in source.coefficients.iter().zip(source.exponents_iter()) {
            let mut specialized = coefficient.clone();
            for (position, value) in assignment.iter().copied().enumerate() {
                let exponent = exponents[base_count + position];
                if exponent != 0 {
                    specialized = specialized * Integer::from(value).pow(u64::from(exponent));
                }
            }
            result.append_monomial(specialized, &exponents[..base_count]);
        }
        if result.variables.as_ref() != self.base.variables().as_ref() {
            return Err(IndexedAlgebraError::WrongContext);
        }
        verify_polynomial_execution_envelope(
            &result,
            preflight.output_term_bound,
            preflight.output_exponent_entry_bound,
            preflight.largest_output_integer_bit_bound,
            "coefficient specialization",
        )?;
        validate_polynomial_on_map(
            &result,
            self.base.variables(),
            crate::algebra::CoefficientPolynomialPart::Numerator,
            limits.exact_algebra,
        )?;
        Ok(result)
    }
}

impl IndexedCoefficientContext {
    /// Substitute only the selected integral indices, retaining the same
    /// authenticated `K(n)` variable map for every unfixed index.
    ///
    /// The returned polynomial is the exact pre-cancellation denominator
    /// witness.  Foundry refinement owners retain it even when normalization
    /// cancels a common factor from the returned coefficient.
    pub fn specialize_fixed_indices(
        &self,
        value: &IndexedCoefficient,
        fixed: &[(usize, i64)],
        limits: IndexedAlgebraLimits,
    ) -> Result<(IndexedCoefficient, IndexedPolynomial), IndexedAlgebraError> {
        self.validate_with_limits(value, limits.exact_algebra)?;
        self.specialize_fixed_indices_authenticated(value, fixed, limits)
    }

    /// Sealed-owner variant of [`Self::specialize_fixed_indices`].
    pub(crate) fn specialize_fixed_indices_sealed(
        &self,
        value: &IndexedCoefficient,
        fixed: &[(usize, i64)],
        limits: IndexedAlgebraLimits,
    ) -> Result<(IndexedCoefficient, IndexedPolynomial), IndexedAlgebraError> {
        self.bind_sealed(value)?;
        self.specialize_fixed_indices_authenticated(value, fixed, limits)
    }

    /// Partially specialize one retained pre-cancellation polynomial guard.
    pub fn specialize_fixed_polynomial(
        &self,
        value: &IndexedPolynomial,
        fixed: &[(usize, i64)],
        limits: IndexedAlgebraLimits,
    ) -> Result<IndexedPolynomial, IndexedAlgebraError> {
        self.validate_polynomial_with_limits(value, limits.exact_algebra)?;
        self.specialize_fixed_polynomial_authenticated(value, fixed, limits)
    }

    pub(crate) fn specialize_fixed_polynomial_sealed(
        &self,
        value: &IndexedPolynomial,
        fixed: &[(usize, i64)],
        limits: IndexedAlgebraLimits,
    ) -> Result<IndexedPolynomial, IndexedAlgebraError> {
        self.validate_polynomial_context(value)?;
        self.specialize_fixed_polynomial_authenticated(value, fixed, limits)
    }

    fn specialize_fixed_indices_authenticated(
        &self,
        value: &IndexedCoefficient,
        fixed: &[(usize, i64)],
        limits: IndexedAlgebraLimits,
    ) -> Result<(IndexedCoefficient, IndexedPolynomial), IndexedAlgebraError> {
        let fixed = self.canonical_fixed_indices(fixed)?;
        let numerator_preflight =
            self.preflight_fixed_polynomial(&value.raw.numerator, &fixed, limits)?;
        let denominator_preflight =
            self.preflight_fixed_polynomial(&value.raw.denominator, &fixed, limits)?;
        check_coefficient_specialization_normalization_limits(
            &value.raw.numerator,
            &value.raw.denominator,
            numerator_preflight,
            denominator_preflight,
            value.raw.numerator.is_zero(),
            value.raw.denominator.is_one(),
            self.variables.len(),
            limits,
        )?;
        let numerator = self.execute_fixed_polynomial(
            &value.raw.numerator,
            &fixed,
            limits,
            numerator_preflight,
        )?;
        let denominator = self.execute_fixed_polynomial(
            &value.raw.denominator,
            &fixed,
            limits,
            denominator_preflight,
        )?;
        if denominator.is_zero() {
            return Err(IndexedAlgebraError::ZeroDenominator);
        }
        let denominator_guard = IndexedPolynomial {
            raw: denominator.clone(),
            context: self.fingerprint.clone(),
        };
        let raw = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            <Coefficient as FromNumeratorAndDenominator<IntegerRing, IntegerRing, u16>>::from_num_den(
                numerator,
                denominator,
                &Z,
                true,
            )
        }))
        .map_err(|_| {
            IndexedAlgebraError::Symbolica(
                "Symbolica panicked while normalizing a checked fixed-index specialization"
                    .to_owned(),
            )
        })?;
        Ok((
            self.wrap_checked_with_limits(raw, limits.exact_algebra)?,
            denominator_guard,
        ))
    }

    fn specialize_fixed_polynomial_authenticated(
        &self,
        value: &IndexedPolynomial,
        fixed: &[(usize, i64)],
        limits: IndexedAlgebraLimits,
    ) -> Result<IndexedPolynomial, IndexedAlgebraError> {
        let fixed = self.canonical_fixed_indices(fixed)?;
        let preflight = self.preflight_fixed_polynomial(&value.raw, &fixed, limits)?;
        Ok(IndexedPolynomial {
            raw: self.execute_fixed_polynomial(&value.raw, &fixed, limits, preflight)?,
            context: self.fingerprint.clone(),
        })
    }

    fn canonical_fixed_indices(
        &self,
        fixed: &[(usize, i64)],
    ) -> Result<Vec<(usize, i64)>, IndexedAlgebraError> {
        let mut canonical = Vec::new();
        canonical.try_reserve_exact(fixed.len()).map_err(|_| {
            IndexedAlgebraError::AllocationFailure {
                resource: "fixed-index assignments",
                requested: fixed.len(),
            }
        })?;
        canonical.extend_from_slice(fixed);
        canonical.sort_unstable_by_key(|(position, _)| *position);
        for window in canonical.windows(2) {
            if window[0].0 == window[1].0 {
                return Err(IndexedAlgebraError::DuplicateFixedIndex {
                    position: window[0].0,
                });
            }
        }
        if let Some(&(position, _)) = canonical
            .iter()
            .find(|(position, _)| *position >= self.index_count())
        {
            return Err(IndexedAlgebraError::FixedIndexOutOfRange {
                position,
                index_count: self.index_count(),
            });
        }
        Ok(canonical)
    }

    fn preflight_fixed_polynomial(
        &self,
        source: &CoefficientPolynomial,
        fixed: &[(usize, i64)],
        limits: IndexedAlgebraLimits,
    ) -> Result<SpecializationPreflight, IndexedAlgebraError> {
        validate_polynomial_on_map(
            source,
            &self.variables,
            crate::algebra::CoefficientPolynomialPart::Numerator,
            limits.exact_algebra,
        )?;
        check_limit(
            "fixed-index specialization output terms",
            source.nterms(),
            limits.exact_algebra.max_polynomial_terms,
        )?;
        let operations = source.nterms().checked_mul(fixed.len()).ok_or(
            IndexedAlgebraError::ResourceCountOverflow {
                resource: "fixed-index specialization power operations",
            },
        )?;
        check_limit(
            "fixed-index specialization power operations",
            operations,
            limits.max_specialization_power_operations,
        )?;
        let mut assignment = vec![1_i64; self.index_count()];
        for &(position, value) in fixed {
            assignment[position] = value;
        }
        let base_count = self.base.variables().len();
        let mut largest = 0usize;
        for (coefficient, exponents) in source.coefficients.iter().zip(source.exponents_iter()) {
            largest = largest.max(specialization_integer_bit_bound(
                coefficient,
                exponents,
                base_count,
                &assignment,
            )?);
        }
        largest = checked_indexed_add(
            "fixed-index specialization integer bits",
            largest,
            ceil_log2(source.nterms()),
        )?;
        check_limit(
            "fixed-index specialization integer bits",
            largest,
            limits.max_specialization_integer_bits,
        )?;
        Ok(SpecializationPreflight {
            output_term_bound: source.nterms(),
            output_exponent_entry_bound: checked_indexed_mul(
                "fixed-index specialization output exponent entries",
                source.nterms(),
                self.variables.len(),
            )?,
            largest_output_integer_bit_bound: largest,
        })
    }

    fn execute_fixed_polynomial(
        &self,
        source: &CoefficientPolynomial,
        fixed: &[(usize, i64)],
        limits: IndexedAlgebraLimits,
        preflight: SpecializationPreflight,
    ) -> Result<CoefficientPolynomial, IndexedAlgebraError> {
        let base_count = self.base.variables().len();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut result = source.clone();
            for &(position, value) in fixed {
                result = result.replace(base_count + position, &Integer::from(value));
            }
            result
        }))
        .map_err(|_| {
            IndexedAlgebraError::Symbolica(
                "Symbolica panicked during a checked fixed-index polynomial substitution"
                    .to_owned(),
            )
        })?;
        verify_polynomial_execution_envelope(
            &result,
            preflight.output_term_bound,
            preflight.output_exponent_entry_bound,
            preflight.largest_output_integer_bit_bound,
            "fixed-index specialization",
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

fn check_coefficient_specialization_normalization_limits(
    numerator_source: &CoefficientPolynomial,
    denominator_source: &CoefficientPolynomial,
    numerator: SpecializationPreflight,
    denominator: SpecializationPreflight,
    numerator_is_zero: bool,
    denominator_is_one: bool,
    variable_count: usize,
    limits: IndexedAlgebraLimits,
) -> Result<(), IndexedAlgebraError> {
    let normalization_input_term_pairs = checked_indexed_mul(
        "coefficient specialization normalization input term pairs",
        numerator.output_term_bound.max(1),
        denominator.output_term_bound,
    )?;
    check_limit(
        "coefficient specialization normalization input term pairs",
        normalization_input_term_pairs,
        limits.exact_algebra.max_term_operations,
    )?;

    let (numerator_bits, denominator_bits) = if numerator_is_zero || denominator_is_one {
        (
            numerator.largest_output_integer_bit_bound,
            denominator.largest_output_integer_bit_bound,
        )
    } else {
        let term_cap = limits.exact_algebra.max_polynomial_terms;
        (
            normalized_factor_envelope_from_source(
                numerator_source,
                0,
                variable_count,
                numerator.output_term_bound,
                numerator.largest_output_integer_bit_bound,
                term_cap,
                "coefficient specialization normalized numerator support",
            )?
            .1,
            normalized_factor_envelope_from_source(
                denominator_source,
                0,
                variable_count,
                denominator.output_term_bound,
                denominator.largest_output_integer_bit_bound,
                term_cap,
                "coefficient specialization normalized denominator support",
            )?
            .1,
        )
    };
    check_limit(
        "coefficient specialization normalized integer bits",
        numerator_bits.max(denominator_bits),
        limits.max_specialization_integer_bits,
    )
}

fn normalized_factor_envelope_from_source(
    source: &CoefficientPolynomial,
    first_variable: usize,
    variable_count: usize,
    mapped_term_bound: usize,
    mapped_integer_bit_bound: usize,
    successful_term_cap: usize,
    resource: &'static str,
) -> Result<(usize, usize), IndexedAlgebraError> {
    if source.is_zero() {
        return Ok((0, 0));
    }
    // A mixed-radix Kronecker image with radices degree_i+1 is injective on
    // every possible factor. Its degree is support_size-1. The univariate
    // Landau-Mignotte factor-height bound then gives
    //   bits(factor) <= bits(input) + degree + ceil(log2(input terms)).
    // This is intentionally coarse, but it remains finite, allocation-free,
    // and sound even when exact GCD division turns a sparse input into a dense
    // quotient such as (x^n-1)/(x-1).
    let mut support_size = 1usize;
    let variable_end = first_variable
        .checked_add(variable_count)
        .ok_or(IndexedAlgebraError::ResourceCountOverflow { resource })?;
    if variable_end > source.variables.len() {
        return Err(IndexedAlgebraError::WrongContext);
    }
    for variable in first_variable..variable_end {
        let mut degree = 0usize;
        for exponents in source.exponents_iter() {
            degree = degree.max(usize::from(exponents[variable]));
        }
        support_size = checked_indexed_mul(resource, support_size, degree + 1)?;
    }
    // Exact division may materialize every monomial in this support before
    // the post-normalization authenticator sees the result. Reject the dense
    // support prospectively; `min(successful_term_cap)` would only turn the
    // configured term cap into a post-allocation publication gate.
    check_limit(resource, support_size, successful_term_cap)?;
    let term_bound = support_size;
    let integer_bit_bound = checked_indexed_add(
        resource,
        mapped_integer_bit_bound.max(1),
        checked_indexed_add(
            resource,
            support_size.saturating_sub(1),
            ceil_log2(mapped_term_bound),
        )?,
    )?;
    Ok((term_bound, integer_bit_bound))
}

fn specialization_integer_bit_bound(
    coefficient: &Integer,
    exponents: &[u16],
    base_count: usize,
    assignment: &[i64],
) -> Result<usize, IndexedAlgebraError> {
    let mut requested = integer_magnitude_bits(coefficient);
    if requested == 0 {
        return Ok(0);
    }
    for (position, value) in assignment.iter().copied().enumerate() {
        let exponent = exponents[base_count + position];
        if exponent == 0 {
            continue;
        }
        let magnitude = value.unsigned_abs();
        if magnitude == 0 {
            return Ok(0);
        }
        // Multiplication by (+/-1)^e does not grow the coefficient.  For all
        // other bases, e*bit_length(base) is a conservative bit bound for the
        // power and hence for its contribution to the product.
        if magnitude != 1 {
            let value_bits = u64::from(u64::BITS - magnitude.leading_zeros());
            let power_bits = value_bits.checked_mul(u64::from(exponent)).ok_or(
                IndexedAlgebraError::ResourceCountOverflow {
                    resource: "coefficient specialization integer bits",
                },
            )?;
            requested = requested.checked_add(power_bits).ok_or(
                IndexedAlgebraError::ResourceCountOverflow {
                    resource: "coefficient specialization integer bits",
                },
            )?;
        }
    }
    usize::try_from(requested).map_err(|_| IndexedAlgebraError::ResourceCountOverflow {
        resource: "coefficient specialization integer bits",
    })
}
