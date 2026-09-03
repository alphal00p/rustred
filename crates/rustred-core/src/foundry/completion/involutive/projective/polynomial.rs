//! Resource-admitted calls into Symbolica's native integer-polynomial API.

use std::mem::size_of;
use std::panic::{AssertUnwindSafe, catch_unwind};

use symbolica::prelude::Integer;

use crate::algebra::{
    CoefficientPolynomial, ExactAlgebraError, ExactAlgebraOperation, IndexedAlgebraError,
    IndexedCoefficient, IndexedCoefficientContext, IndexedPolynomial,
};

use super::error::{ProjectiveError, check_limit, checked_add, checked_mul, try_vec};
use super::limits::{ProjectiveLimits, ProjectiveWorkBudget, ProjectiveWorkCensus};
use super::model::ProjectivePayloadCensus;

const POLYNOMIAL_OPERATIONS: &str = "projective polynomial operations";
const SUM_INPUT_TERMS: &str = "projective polynomial sum input terms";
const MULTIPLICATION_TERM_PAIRS: &str = "projective polynomial multiplication term pairs";
const GCD_CALLS: &str = "projective polynomial GCD calls";
const GCD_TERM_PAIRS: &str = "projective polynomial GCD term-pair envelope";
const GCD_MULTIPLE_INPUTS: &str = "projective polynomial multiple-GCD inputs";
const LCM_STEPS: &str = "projective polynomial LCM steps";
const EXACT_DIVISIONS: &str = "projective exact polynomial divisions";
const TRANSLATIONS: &str = "projective polynomial translations";
const GENERATED_TERMS: &str = "projective generated polynomial terms";
const RETAINED_TERMS: &str = "projective retained polynomial terms";
const RETAINED_EXPONENT_CELLS: &str = "projective retained polynomial exponent cells";
const RETAINED_BYTES: &str = "projective retained polynomial bytes";

pub(super) struct PolynomialWork<'context, 'budget> {
    context: &'context IndexedCoefficientContext,
    limits: ProjectiveLimits,
    budget: &'budget mut ProjectiveWorkBudget,
}

impl<'context, 'budget> PolynomialWork<'context, 'budget> {
    pub(super) fn try_new(
        context: &'context IndexedCoefficientContext,
        limits: ProjectiveLimits,
        budget: &'budget mut ProjectiveWorkBudget,
    ) -> Result<Self, ProjectiveError> {
        budget.require_limits(limits)?;
        Ok(Self {
            context,
            limits,
            budget,
        })
    }

    pub(super) const fn census(&self) -> ProjectiveWorkCensus {
        self.budget.census
    }

    pub(super) fn record_content_normalization(&mut self) -> Result<(), ProjectiveError> {
        self.budget.census.content_normalizations = checked_add(
            "projective augmented-content normalizations",
            self.budget.census.content_normalizations,
            1,
        )?;
        Ok(())
    }

    pub(super) fn one(&mut self) -> Result<IndexedPolynomial, ProjectiveError> {
        self.charge_operation()?;
        let value = self.context.numerator_condition_with_limits(
            &self.context.one(),
            self.limits.involutive.indexed_algebra.exact_algebra,
        )?;
        self.record_generated(&value)?;
        Ok(value)
    }

    pub(super) fn numerator(
        &mut self,
        value: &IndexedCoefficient,
    ) -> Result<IndexedPolynomial, ProjectiveError> {
        self.charge_operation()?;
        let value = self.context.numerator_condition_with_limits(
            value,
            self.limits.involutive.indexed_algebra.exact_algebra,
        )?;
        self.record_generated(&value)?;
        Ok(value)
    }

    pub(super) fn denominator(
        &mut self,
        value: &IndexedCoefficient,
    ) -> Result<IndexedPolynomial, ProjectiveError> {
        self.charge_operation()?;
        let value = self.context.denominator_condition_with_limits(
            value,
            self.limits.involutive.indexed_algebra.exact_algebra,
        )?;
        self.record_generated(&value)?;
        Ok(value)
    }

    pub(super) fn add(
        &mut self,
        left: &IndexedPolynomial,
        right: &IndexedPolynomial,
    ) -> Result<IndexedPolynomial, ProjectiveError> {
        self.validate(left)?;
        self.validate(right)?;
        self.charge_operation()?;
        let inputs = checked_add(SUM_INPUT_TERMS, left.raw().nterms(), right.raw().nterms())?;
        self.budget.census.sum_input_terms =
            checked_add(SUM_INPUT_TERMS, self.budget.census.sum_input_terms, inputs)?;
        check_limit(
            SUM_INPUT_TERMS,
            self.budget.census.sum_input_terms,
            self.limits.max_sum_input_terms,
        )?;
        let raw = catch_unwind(AssertUnwindSafe(|| left.raw() + right.raw())).map_err(|_| {
            ProjectiveError::NativePanic {
                operation: "adding projective integer polynomials",
            }
        })?;
        self.admit(raw)
    }

    pub(super) fn sub(
        &mut self,
        left: &IndexedPolynomial,
        right: &IndexedPolynomial,
    ) -> Result<IndexedPolynomial, ProjectiveError> {
        self.validate(left)?;
        self.validate(right)?;
        self.charge_operation()?;
        let inputs = checked_add(SUM_INPUT_TERMS, left.raw().nterms(), right.raw().nterms())?;
        self.budget.census.sum_input_terms =
            checked_add(SUM_INPUT_TERMS, self.budget.census.sum_input_terms, inputs)?;
        check_limit(
            SUM_INPUT_TERMS,
            self.budget.census.sum_input_terms,
            self.limits.max_sum_input_terms,
        )?;
        let raw = catch_unwind(AssertUnwindSafe(|| left.raw() - right.raw())).map_err(|_| {
            ProjectiveError::NativePanic {
                operation: "subtracting projective integer polynomials",
            }
        })?;
        self.admit(raw)
    }

    pub(super) fn neg(
        &mut self,
        value: &IndexedPolynomial,
    ) -> Result<IndexedPolynomial, ProjectiveError> {
        self.validate(value)?;
        self.charge_operation()?;
        let raw = catch_unwind(AssertUnwindSafe(|| -value.raw().clone())).map_err(|_| {
            ProjectiveError::NativePanic {
                operation: "negating a projective integer polynomial",
            }
        })?;
        self.admit(raw)
    }

    pub(super) fn mul(
        &mut self,
        left: &IndexedPolynomial,
        right: &IndexedPolynomial,
    ) -> Result<IndexedPolynomial, ProjectiveError> {
        self.validate(left)?;
        self.validate(right)?;
        self.preflight_product_exponents(left, right)?;
        self.charge_operation()?;
        let pairs = checked_mul(
            MULTIPLICATION_TERM_PAIRS,
            left.raw().nterms(),
            right.raw().nterms(),
        )?;
        self.budget.census.multiplication_term_pairs = checked_add(
            MULTIPLICATION_TERM_PAIRS,
            self.budget.census.multiplication_term_pairs,
            pairs,
        )?;
        check_limit(
            MULTIPLICATION_TERM_PAIRS,
            self.budget.census.multiplication_term_pairs,
            self.limits.max_multiplication_term_pairs,
        )?;
        let raw = catch_unwind(AssertUnwindSafe(|| left.raw() * right.raw())).map_err(|_| {
            ProjectiveError::NativePanic {
                operation: "multiplying projective integer polynomials",
            }
        })?;
        self.admit(raw)
    }

    pub(super) fn gcd(
        &mut self,
        left: &IndexedPolynomial,
        right: &IndexedPolynomial,
    ) -> Result<IndexedPolynomial, ProjectiveError> {
        self.validate(left)?;
        self.validate(right)?;
        self.charge_gcd(checked_mul(
            GCD_TERM_PAIRS,
            left.raw().nterms(),
            right.raw().nterms(),
        )?)?;
        self.charge_operation()?;
        let raw = catch_unwind(AssertUnwindSafe(|| left.raw().gcd(right.raw()))).map_err(|_| {
            ProjectiveError::NativePanic {
                operation: "computing a projective polynomial GCD",
            }
        })?;
        self.admit(raw)
    }

    pub(super) fn gcd_multiple(
        &mut self,
        values: &[&IndexedPolynomial],
    ) -> Result<IndexedPolynomial, ProjectiveError> {
        if values.is_empty() {
            return Err(ProjectiveError::Invariant {
                detail: "multiple polynomial GCD received no augmented entry",
            });
        }
        check_limit(
            GCD_MULTIPLE_INPUTS,
            values.len(),
            self.limits.max_gcd_multiple_inputs,
        )?;
        let cumulative_inputs = checked_add(
            GCD_MULTIPLE_INPUTS,
            self.budget.census.gcd_multiple_inputs,
            values.len(),
        )?;
        check_limit(
            GCD_MULTIPLE_INPUTS,
            cumulative_inputs,
            self.limits.max_gcd_multiple_inputs,
        )?;
        let mut total_terms = 0usize;
        for value in values {
            self.validate(value)?;
            total_terms = checked_add(GCD_TERM_PAIRS, total_terms, value.raw().nterms())?;
        }
        // Symbolica's multiple-GCD strategy is adaptive and exposes no
        // scratch census.  The square of aggregate input support is a stable
        // conservative admission envelope, not a claim about native work.
        self.charge_gcd(checked_mul(GCD_TERM_PAIRS, total_terms, total_terms)?)?;
        self.budget.census.gcd_multiple_inputs = cumulative_inputs;
        self.charge_operation()?;
        let mut raw = try_vec(GCD_MULTIPLE_INPUTS, values.len())?;
        raw.extend(values.iter().map(|value| value.raw().clone()));
        let result = catch_unwind(AssertUnwindSafe(|| {
            CoefficientPolynomial::gcd_multiple(raw)
        }))
        .map_err(|_| ProjectiveError::NativePanic {
            operation: "computing projective augmented polynomial content",
        })?;
        self.admit(result)
    }

    pub(super) fn exact_div(
        &mut self,
        numerator: &IndexedPolynomial,
        denominator: &IndexedPolynomial,
    ) -> Result<IndexedPolynomial, ProjectiveError> {
        self.validate(numerator)?;
        self.validate(denominator)?;
        if denominator.is_zero() {
            return Err(ProjectiveError::IndexedAlgebra(
                IndexedAlgebraError::ExactAlgebra(ExactAlgebraError::DivisionByZero),
            ));
        }
        self.budget.census.exact_divisions =
            checked_add(EXACT_DIVISIONS, self.budget.census.exact_divisions, 1)?;
        check_limit(
            EXACT_DIVISIONS,
            self.budget.census.exact_divisions,
            self.limits.max_exact_divisions,
        )?;
        self.charge_operation()?;
        let raw = catch_unwind(AssertUnwindSafe(|| {
            numerator.raw().try_div(denominator.raw())
        }))
        .map_err(|_| ProjectiveError::NativePanic {
            operation: "performing projective exact polynomial division",
        })?
        .ok_or(ProjectiveError::NonExactPolynomialDivision)?;
        self.admit(raw)
    }

    pub(super) fn lcm(
        &mut self,
        left: &IndexedPolynomial,
        right: &IndexedPolynomial,
    ) -> Result<IndexedPolynomial, ProjectiveError> {
        self.validate(left)?;
        self.validate(right)?;
        if left.is_zero() || right.is_zero() {
            return Err(ProjectiveError::Invariant {
                detail: "a rational coefficient exposed a zero denominator",
            });
        }
        self.budget.census.lcm_steps = checked_add(LCM_STEPS, self.budget.census.lcm_steps, 1)?;
        check_limit(
            LCM_STEPS,
            self.budget.census.lcm_steps,
            self.limits.max_lcm_steps,
        )?;
        if left.raw().is_one() || right.raw().is_one() {
            // Do not hide a potentially allocating deep polynomial clone in
            // a fast path. Symbolica's multiplication by one is admitted and
            // charged through the same native boundary as every other copy.
            return self.mul(left, right);
        }
        let gcd = self.gcd(left, right)?;
        let quotient = self.exact_div(left, &gcd)?;
        self.mul(&quotient, right)
    }

    pub(super) fn translate(
        &mut self,
        value: &IndexedPolynomial,
        physical_translation: &[i64],
    ) -> Result<IndexedPolynomial, ProjectiveError> {
        self.validate(value)?;
        self.budget.census.translations =
            checked_add(TRANSLATIONS, self.budget.census.translations, 1)?;
        check_limit(
            TRANSLATIONS,
            self.budget.census.translations,
            self.limits.max_translations,
        )?;
        self.charge_operation()?;
        let result = self.context.translate_polynomial_sealed(
            value,
            physical_translation,
            self.limits.involutive.indexed_algebra,
        )?;
        self.record_generated(&result)?;
        Ok(result)
    }

    fn validate(&self, value: &IndexedPolynomial) -> Result<(), ProjectiveError> {
        // Every projective input crosses one complete validation boundary,
        // and every native result is completely admitted before sealing.
        // The hot replay lane therefore needs only the immutable context
        // identity check; rescanning sparse payload and exponent bounds here
        // would repeat O(size(poly)) work for every arithmetic operand.
        self.context.validate_polynomial_context(value)?;
        Ok(())
    }

    fn charge_operation(&mut self) -> Result<(), ProjectiveError> {
        self.budget.census.polynomial_operations = checked_add(
            POLYNOMIAL_OPERATIONS,
            self.budget.census.polynomial_operations,
            1,
        )?;
        check_limit(
            POLYNOMIAL_OPERATIONS,
            self.budget.census.polynomial_operations,
            self.limits.max_polynomial_operations,
        )
    }

    fn charge_gcd(&mut self, term_pairs: usize) -> Result<(), ProjectiveError> {
        self.budget.census.gcd_calls = checked_add(GCD_CALLS, self.budget.census.gcd_calls, 1)?;
        check_limit(
            GCD_CALLS,
            self.budget.census.gcd_calls,
            self.limits.max_gcd_calls,
        )?;
        self.budget.census.gcd_term_pairs = checked_add(
            GCD_TERM_PAIRS,
            self.budget.census.gcd_term_pairs,
            term_pairs,
        )?;
        check_limit(
            GCD_TERM_PAIRS,
            self.budget.census.gcd_term_pairs,
            self.limits.max_gcd_term_pairs,
        )
    }

    fn preflight_product_exponents(
        &self,
        left: &IndexedPolynomial,
        right: &IndexedPolynomial,
    ) -> Result<(), ProjectiveError> {
        let exact = self.limits.involutive.indexed_algebra.exact_algebra;
        for variable in 0..left.raw().nvars() {
            let requested = u64::from(left.raw().degree(variable))
                .checked_add(u64::from(right.raw().degree(variable)))
                .ok_or(ProjectiveError::ResourceCountOverflow {
                    resource: "projective polynomial product exponent",
                })?;
            if requested > u64::from(exact.max_exponent) {
                return Err(ProjectiveError::IndexedAlgebra(
                    IndexedAlgebraError::ExactAlgebra(ExactAlgebraError::ExponentLimit {
                        operation: ExactAlgebraOperation::Multiply,
                        variable,
                        requested,
                        limit: exact.max_exponent,
                    }),
                ));
            }
        }
        Ok(())
    }

    fn admit(&mut self, raw: CoefficientPolynomial) -> Result<IndexedPolynomial, ProjectiveError> {
        let result = self.context.admit_native_polynomial_result_with_limits(
            raw,
            self.limits.involutive.indexed_algebra.exact_algebra,
        )?;
        self.record_generated(&result)?;
        Ok(result)
    }

    fn record_generated(&mut self, value: &IndexedPolynomial) -> Result<(), ProjectiveError> {
        self.budget.census.generated_polynomial_terms = checked_add(
            GENERATED_TERMS,
            self.budget.census.generated_polynomial_terms,
            value.raw().nterms(),
        )?;
        check_limit(
            GENERATED_TERMS,
            self.budget.census.generated_polynomial_terms,
            self.limits.max_generated_polynomial_terms,
        )
    }
}

pub(super) fn payload_census<'a>(
    values: impl IntoIterator<Item = &'a IndexedPolynomial>,
) -> Result<ProjectivePayloadCensus, ProjectiveError> {
    let mut result = ProjectivePayloadCensus::default();
    for value in values {
        let raw = value.raw();
        result.polynomial_terms = checked_add(
            RETAINED_TERMS,
            result.polynomial_terms,
            raw.coefficients.len(),
        )?;
        result.exponent_cells = checked_add(
            RETAINED_EXPONENT_CELLS,
            result.exponent_cells,
            raw.exponents.len(),
        )?;
        let coefficient_bytes =
            checked_mul(RETAINED_BYTES, raw.coefficients.len(), size_of::<Integer>())?;
        let exponent_bytes = checked_mul(RETAINED_BYTES, raw.exponents.len(), size_of::<u16>())?;
        let mut bytes = checked_add(
            RETAINED_BYTES,
            size_of::<IndexedPolynomial>(),
            checked_add(RETAINED_BYTES, coefficient_bytes, exponent_bytes)?,
        )?;
        for coefficient in &raw.coefficients {
            if let Integer::Large(value) = coefficient {
                let bits = usize::try_from(value.significant_bits()).map_err(|_| {
                    ProjectiveError::ResourceCountOverflow {
                        resource: RETAINED_BYTES,
                    }
                })?;
                bytes = checked_add(RETAINED_BYTES, bytes, bits.div_ceil(8))?;
            }
        }
        result.retained_bytes = checked_add(RETAINED_BYTES, result.retained_bytes, bytes)?;
    }
    Ok(result)
}

pub(super) fn admit_payload(
    payload: ProjectivePayloadCensus,
    limits: ProjectiveLimits,
) -> Result<(), ProjectiveError> {
    check_limit(
        RETAINED_TERMS,
        payload.polynomial_terms,
        limits.max_retained_polynomial_terms,
    )?;
    check_limit(
        RETAINED_EXPONENT_CELLS,
        payload.exponent_cells,
        limits.max_retained_polynomial_exponent_cells,
    )?;
    check_limit(
        RETAINED_BYTES,
        payload.retained_bytes,
        limits.max_retained_polynomial_bytes,
    )
}
