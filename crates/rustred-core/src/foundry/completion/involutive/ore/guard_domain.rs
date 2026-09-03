//! Bounded exact comparison of cold localization domains.
//!
//! A conjunction `g_1 != 0, ..., g_r != 0` is the principal open set of the
//! product.  Its canonical signature is therefore the primitive associate of
//! the square-free radical of that product.  This module constructs the same
//! object incrementally as the square-free polynomial LCM of the guards, so
//! repeated and overlapping factors do not inflate an intermediate product.
//!
//! All CAS work is delegated to Symbolica 2.2's public polynomial API:
//! [`symbolica::prelude::Factorize::square_free_factorization`],
//! [`symbolica::poly::polynomial::MultivariatePolynomial::gcd`],
//! [`symbolica::poly::polynomial::MultivariatePolynomial::try_div`], and native
//! polynomial multiplication. RustRed only provides admission accounting,
//! output authentication, and deterministic primitive-associate normalization;
//! it does not implement factorization, GCD, or exact division itself.

use std::mem::size_of;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use symbolica::prelude::{Factorize, Integer};

use crate::algebra::{
    CoefficientPolynomial, ExactAlgebraLimits, IndexedCoefficientContext, IndexedPolynomial,
};

use super::super::InvolutiveError;
use super::super::error::{check_limit, checked_add, checked_mul};

const ATTEMPTS: &str = "cold localization-domain comparison attempts";
const INPUT_GUARDS: &str = "cold localization-domain input guards";
const INPUT_TERMS: &str = "cold localization-domain input terms";
const INPUT_EXPONENT_CELLS: &str = "cold localization-domain input exponent cells";
const INPUT_BYTES: &str = "cold localization-domain input retained bytes";
const NATIVE_OPERATIONS: &str = "cold localization-domain native operations";
const SQUARE_FREE_CALLS: &str = "cold localization-domain square-free calls";
const GCD_CALLS: &str = "cold localization-domain GCD calls";
const EXACT_DIVISIONS: &str = "cold localization-domain exact divisions";
const MULTIPLICATIONS: &str = "cold localization-domain multiplications";
const PRIMITIVE_NORMALIZATIONS: &str =
    "cold localization-domain primitive-associate normalizations";
const NATIVE_TERM_PAIR_WORK: &str = "cold localization-domain native term-pair work";
const SQUARE_FREE_WORK: &str = "cold localization-domain square-free work envelope";
const FACTOR_OUTPUTS: &str = "cold localization-domain square-free factor outputs";
const NATIVE_OUTPUT_TERMS: &str = "cold localization-domain prospective native output terms";
const NATIVE_OUTPUT_EXPONENT_CELLS: &str =
    "cold localization-domain prospective native output exponent cells";
const NATIVE_OUTPUT_BYTES: &str = "cold localization-domain prospective native output bytes";
const INTERMEDIATE_TERMS: &str = "cold localization-domain intermediate terms";
const INTERMEDIATE_EXPONENT_CELLS: &str = "cold localization-domain intermediate exponent cells";
const INTERMEDIATE_BYTES: &str = "cold localization-domain intermediate retained bytes";
const OUTPUT_SIGNATURES: &str = "cold localization-domain output signatures";
const OUTPUT_TERMS: &str = "cold localization-domain output terms";
const OUTPUT_EXPONENT_CELLS: &str = "cold localization-domain output exponent cells";
const OUTPUT_BYTES: &str = "cold localization-domain output retained bytes";

/// Independent cold-path resource contract for exact principal-open checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::foundry::completion::involutive) struct LocalizationDomainLimits {
    pub(in crate::foundry::completion::involutive) max_attempts: usize,
    pub(in crate::foundry::completion::involutive) max_input_guards: usize,
    pub(in crate::foundry::completion::involutive) max_input_terms: usize,
    pub(in crate::foundry::completion::involutive) max_input_exponent_cells: usize,
    pub(in crate::foundry::completion::involutive) max_input_bytes: usize,
    pub(in crate::foundry::completion::involutive) max_native_operations: usize,
    pub(in crate::foundry::completion::involutive) max_square_free_calls: usize,
    pub(in crate::foundry::completion::involutive) max_gcd_calls: usize,
    pub(in crate::foundry::completion::involutive) max_exact_divisions: usize,
    pub(in crate::foundry::completion::involutive) max_multiplications: usize,
    pub(in crate::foundry::completion::involutive) max_primitive_normalizations: usize,
    pub(in crate::foundry::completion::involutive) max_native_term_pair_work: usize,
    pub(in crate::foundry::completion::involutive) max_square_free_work: usize,
    pub(in crate::foundry::completion::involutive) max_factor_outputs: usize,
    pub(in crate::foundry::completion::involutive) max_native_output_terms: usize,
    pub(in crate::foundry::completion::involutive) max_native_output_exponent_cells: usize,
    pub(in crate::foundry::completion::involutive) max_native_output_bytes: usize,
    pub(in crate::foundry::completion::involutive) max_intermediate_terms: usize,
    pub(in crate::foundry::completion::involutive) max_intermediate_exponent_cells: usize,
    pub(in crate::foundry::completion::involutive) max_intermediate_bytes: usize,
    pub(in crate::foundry::completion::involutive) max_output_signatures: usize,
    pub(in crate::foundry::completion::involutive) max_output_terms: usize,
    pub(in crate::foundry::completion::involutive) max_output_exponent_cells: usize,
    pub(in crate::foundry::completion::involutive) max_output_bytes: usize,
}

impl Default for LocalizationDomainLimits {
    fn default() -> Self {
        Self {
            max_attempts: 1_000_000,
            max_input_guards: 2_000_000,
            max_input_terms: 16_000_000,
            max_input_exponent_cells: 256_000_000,
            max_input_bytes: 2_147_483_648,
            max_native_operations: 64_000_000,
            max_square_free_calls: 4_000_000,
            max_gcd_calls: 16_000_000,
            max_exact_divisions: 16_000_000,
            max_multiplications: 32_000_000,
            max_primitive_normalizations: 32_000_000,
            max_native_term_pair_work: 1_000_000_000,
            max_square_free_work: 1_000_000_000,
            max_factor_outputs: 16_000_000,
            max_native_output_terms: 16_000_000,
            max_native_output_exponent_cells: 256_000_000,
            max_native_output_bytes: 2_147_483_648,
            max_intermediate_terms: 128_000_000,
            max_intermediate_exponent_cells: 2_000_000_000,
            max_intermediate_bytes: 8_589_934_592,
            max_output_signatures: 2_000_000,
            max_output_terms: 16_000_000,
            max_output_exponent_cells: 256_000_000,
            max_output_bytes: 2_147_483_648,
        }
    }
}

/// Monotone accounting for one caller-owned sequence of cold domain checks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::foundry::completion::involutive) struct LocalizationDomainCensus {
    attempts: usize,
    input_guards: usize,
    input_terms: usize,
    input_exponent_cells: usize,
    input_bytes: usize,
    native_operations: usize,
    square_free_calls: usize,
    gcd_calls: usize,
    exact_divisions: usize,
    multiplications: usize,
    primitive_normalizations: usize,
    native_term_pair_work: usize,
    square_free_work: usize,
    factor_outputs: usize,
    intermediate_terms: usize,
    intermediate_exponent_cells: usize,
    intermediate_bytes: usize,
    output_signatures: usize,
    output_terms: usize,
    output_exponent_cells: usize,
    output_bytes: usize,
}

impl LocalizationDomainCensus {
    pub(in crate::foundry::completion::involutive) const fn attempts(self) -> usize {
        self.attempts
    }

    pub(in crate::foundry::completion::involutive) const fn input_guards(self) -> usize {
        self.input_guards
    }

    pub(in crate::foundry::completion::involutive) const fn input_terms(self) -> usize {
        self.input_terms
    }

    pub(in crate::foundry::completion::involutive) const fn input_exponent_cells(self) -> usize {
        self.input_exponent_cells
    }

    pub(in crate::foundry::completion::involutive) const fn input_bytes(self) -> usize {
        self.input_bytes
    }

    pub(in crate::foundry::completion::involutive) const fn native_operations(self) -> usize {
        self.native_operations
    }

    pub(in crate::foundry::completion::involutive) const fn square_free_calls(self) -> usize {
        self.square_free_calls
    }

    pub(in crate::foundry::completion::involutive) const fn gcd_calls(self) -> usize {
        self.gcd_calls
    }

    pub(in crate::foundry::completion::involutive) const fn exact_divisions(self) -> usize {
        self.exact_divisions
    }

    pub(in crate::foundry::completion::involutive) const fn multiplications(self) -> usize {
        self.multiplications
    }

    pub(in crate::foundry::completion::involutive) const fn primitive_normalizations(
        self,
    ) -> usize {
        self.primitive_normalizations
    }

    pub(in crate::foundry::completion::involutive) const fn native_term_pair_work(self) -> usize {
        self.native_term_pair_work
    }

    pub(in crate::foundry::completion::involutive) const fn square_free_work(self) -> usize {
        self.square_free_work
    }

    pub(in crate::foundry::completion::involutive) const fn factor_outputs(self) -> usize {
        self.factor_outputs
    }

    pub(in crate::foundry::completion::involutive) const fn intermediate_terms(self) -> usize {
        self.intermediate_terms
    }

    pub(in crate::foundry::completion::involutive) const fn intermediate_exponent_cells(
        self,
    ) -> usize {
        self.intermediate_exponent_cells
    }

    pub(in crate::foundry::completion::involutive) const fn intermediate_bytes(self) -> usize {
        self.intermediate_bytes
    }

    pub(in crate::foundry::completion::involutive) const fn output_signatures(self) -> usize {
        self.output_signatures
    }

    pub(in crate::foundry::completion::involutive) const fn output_terms(self) -> usize {
        self.output_terms
    }

    pub(in crate::foundry::completion::involutive) const fn output_exponent_cells(self) -> usize {
        self.output_exponent_cells
    }

    pub(in crate::foundry::completion::involutive) const fn output_bytes(self) -> usize {
        self.output_bytes
    }
}

#[derive(Debug)]
pub(in crate::foundry::completion::involutive) struct LocalizationDomainBudget {
    limits: LocalizationDomainLimits,
    census: LocalizationDomainCensus,
}

impl LocalizationDomainBudget {
    pub(in crate::foundry::completion::involutive) const fn new(
        limits: LocalizationDomainLimits,
    ) -> Self {
        Self {
            limits,
            census: LocalizationDomainCensus {
                attempts: 0,
                input_guards: 0,
                input_terms: 0,
                input_exponent_cells: 0,
                input_bytes: 0,
                native_operations: 0,
                square_free_calls: 0,
                gcd_calls: 0,
                exact_divisions: 0,
                multiplications: 0,
                primitive_normalizations: 0,
                native_term_pair_work: 0,
                square_free_work: 0,
                factor_outputs: 0,
                intermediate_terms: 0,
                intermediate_exponent_cells: 0,
                intermediate_bytes: 0,
                output_signatures: 0,
                output_terms: 0,
                output_exponent_cells: 0,
                output_bytes: 0,
            },
        }
    }

    pub(in crate::foundry::completion::involutive) const fn census(
        &self,
    ) -> LocalizationDomainCensus {
        self.census
    }

    fn try_start(&mut self) -> Result<(), InvolutiveError> {
        charge(
            ATTEMPTS,
            &mut self.census.attempts,
            1,
            self.limits.max_attempts,
        )
    }

    fn try_admit_inputs<'input>(
        &mut self,
        context: &IndexedCoefficientContext,
        inputs: impl IntoIterator<Item = &'input IndexedPolynomial>,
        exact_limits: ExactAlgebraLimits,
    ) -> Result<(), InvolutiveError> {
        let mut added = PolynomialCensus::default();
        let mut count = 0usize;
        for input in inputs {
            context.validate_polynomial_with_limits(input, exact_limits)?;
            if input.is_zero() {
                return Err(InvolutiveError::Invariant {
                    detail: "a zero polynomial cannot enter a cold localization-domain proof",
                });
            }
            count = checked_add(INPUT_GUARDS, count, 1)?;
            added = added.try_add(polynomial_census(input.raw())?)?;
        }
        let input_guards = checked_add(INPUT_GUARDS, self.census.input_guards, count)?;
        let input_terms = checked_add(INPUT_TERMS, self.census.input_terms, added.terms)?;
        let input_exponent_cells = checked_add(
            INPUT_EXPONENT_CELLS,
            self.census.input_exponent_cells,
            added.exponent_cells,
        )?;
        let input_bytes = checked_add(INPUT_BYTES, self.census.input_bytes, added.bytes)?;
        check_limit(INPUT_GUARDS, input_guards, self.limits.max_input_guards)?;
        check_limit(INPUT_TERMS, input_terms, self.limits.max_input_terms)?;
        check_limit(
            INPUT_EXPONENT_CELLS,
            input_exponent_cells,
            self.limits.max_input_exponent_cells,
        )?;
        check_limit(INPUT_BYTES, input_bytes, self.limits.max_input_bytes)?;
        self.census.input_guards = input_guards;
        self.census.input_terms = input_terms;
        self.census.input_exponent_cells = input_exponent_cells;
        self.census.input_bytes = input_bytes;
        Ok(())
    }

    fn try_begin_native(
        &mut self,
        kind: NativeKind,
        term_pair_work: usize,
        square_free_work: usize,
        prospective: PolynomialCensus,
    ) -> Result<(), InvolutiveError> {
        let operations = checked_add(NATIVE_OPERATIONS, self.census.native_operations, 1)?;
        let term_pairs = checked_add(
            NATIVE_TERM_PAIR_WORK,
            self.census.native_term_pair_work,
            term_pair_work,
        )?;
        let square_free = checked_add(
            SQUARE_FREE_WORK,
            self.census.square_free_work,
            square_free_work,
        )?;
        let specialized = kind.try_next(&self.census)?;
        check_limit(
            NATIVE_OPERATIONS,
            operations,
            self.limits.max_native_operations,
        )?;
        check_limit(
            NATIVE_TERM_PAIR_WORK,
            term_pairs,
            self.limits.max_native_term_pair_work,
        )?;
        check_limit(
            SQUARE_FREE_WORK,
            square_free,
            self.limits.max_square_free_work,
        )?;
        specialized.try_check(self.limits)?;
        check_limit(
            NATIVE_OUTPUT_TERMS,
            prospective.terms,
            self.limits.max_native_output_terms,
        )?;
        check_limit(
            NATIVE_OUTPUT_EXPONENT_CELLS,
            prospective.exponent_cells,
            self.limits.max_native_output_exponent_cells,
        )?;
        check_limit(
            NATIVE_OUTPUT_BYTES,
            prospective.bytes,
            self.limits.max_native_output_bytes,
        )?;
        check_limit(
            INTERMEDIATE_TERMS,
            checked_add(
                INTERMEDIATE_TERMS,
                self.census.intermediate_terms,
                prospective.terms,
            )?,
            self.limits.max_intermediate_terms,
        )?;
        check_limit(
            INTERMEDIATE_EXPONENT_CELLS,
            checked_add(
                INTERMEDIATE_EXPONENT_CELLS,
                self.census.intermediate_exponent_cells,
                prospective.exponent_cells,
            )?,
            self.limits.max_intermediate_exponent_cells,
        )?;
        check_limit(
            INTERMEDIATE_BYTES,
            checked_add(
                INTERMEDIATE_BYTES,
                self.census.intermediate_bytes,
                prospective.bytes,
            )?,
            self.limits.max_intermediate_bytes,
        )?;
        self.census.native_operations = operations;
        self.census.native_term_pair_work = term_pairs;
        self.census.square_free_work = square_free;
        specialized.commit(&mut self.census);
        Ok(())
    }

    fn try_record_factor_outputs(&mut self, amount: usize) -> Result<(), InvolutiveError> {
        charge(
            FACTOR_OUTPUTS,
            &mut self.census.factor_outputs,
            amount,
            self.limits.max_factor_outputs,
        )
    }

    fn try_preflight_factor_outputs(&self, amount: usize) -> Result<(), InvolutiveError> {
        let requested = checked_add(FACTOR_OUTPUTS, self.census.factor_outputs, amount)?;
        check_limit(FACTOR_OUTPUTS, requested, self.limits.max_factor_outputs)
    }

    fn try_record_intermediate(
        &mut self,
        value: &IndexedPolynomial,
    ) -> Result<(), InvolutiveError> {
        let added = polynomial_census(value.raw())?;
        let terms = checked_add(
            INTERMEDIATE_TERMS,
            self.census.intermediate_terms,
            added.terms,
        )?;
        let cells = checked_add(
            INTERMEDIATE_EXPONENT_CELLS,
            self.census.intermediate_exponent_cells,
            added.exponent_cells,
        )?;
        let bytes = checked_add(
            INTERMEDIATE_BYTES,
            self.census.intermediate_bytes,
            added.bytes,
        )?;
        check_limit(
            INTERMEDIATE_TERMS,
            terms,
            self.limits.max_intermediate_terms,
        )?;
        check_limit(
            INTERMEDIATE_EXPONENT_CELLS,
            cells,
            self.limits.max_intermediate_exponent_cells,
        )?;
        check_limit(
            INTERMEDIATE_BYTES,
            bytes,
            self.limits.max_intermediate_bytes,
        )?;
        self.census.intermediate_terms = terms;
        self.census.intermediate_exponent_cells = cells;
        self.census.intermediate_bytes = bytes;
        Ok(())
    }

    fn try_record_output(&mut self, value: &IndexedPolynomial) -> Result<(), InvolutiveError> {
        let added = polynomial_census(value.raw())?;
        let signatures = checked_add(OUTPUT_SIGNATURES, self.census.output_signatures, 1)?;
        let terms = checked_add(OUTPUT_TERMS, self.census.output_terms, added.terms)?;
        let cells = checked_add(
            OUTPUT_EXPONENT_CELLS,
            self.census.output_exponent_cells,
            added.exponent_cells,
        )?;
        let bytes = checked_add(OUTPUT_BYTES, self.census.output_bytes, added.bytes)?;
        check_limit(
            OUTPUT_SIGNATURES,
            signatures,
            self.limits.max_output_signatures,
        )?;
        check_limit(OUTPUT_TERMS, terms, self.limits.max_output_terms)?;
        check_limit(
            OUTPUT_EXPONENT_CELLS,
            cells,
            self.limits.max_output_exponent_cells,
        )?;
        check_limit(OUTPUT_BYTES, bytes, self.limits.max_output_bytes)?;
        self.census.output_signatures = signatures;
        self.census.output_terms = terms;
        self.census.output_exponent_cells = cells;
        self.census.output_bytes = bytes;
        Ok(())
    }
}

/// Prove that every replay-required irreducible factor is present in the
/// authenticated lazy domain. Historic lazy guards may make that domain
/// strictly smaller, which is safe and intentionally accepted.
pub(super) fn try_require_principal_open_coverage(
    context: &IndexedCoefficientContext,
    authenticated_lazy: &[Arc<IndexedPolynomial>],
    replay_required: &[Arc<IndexedPolynomial>],
    exact_limits: ExactAlgebraLimits,
    budget: &mut LocalizationDomainBudget,
) -> Result<(), InvolutiveError> {
    budget.try_start()?;
    // Authenticate and preflight the complete two-sided input batch before
    // the first native operation or cumulative input-payload charge.
    budget.try_admit_inputs(
        context,
        authenticated_lazy
            .iter()
            .chain(replay_required)
            .map(Arc::as_ref),
        exact_limits,
    )?;

    let mut work = PolynomialWork {
        context,
        exact_limits,
        budget,
    };
    let lazy_signature = work.try_signature(authenticated_lazy)?;
    let replay_signature = work.try_signature(replay_required)?;
    if replay_signature.raw().is_one() {
        return Ok(());
    }
    if lazy_signature.raw().is_one() {
        return Err(InvolutiveError::LocalizationDomainMismatch);
    }
    let common = work.try_gcd(&lazy_signature, &replay_signature)?;
    let common = work.try_primitive_associate(&common)?;
    if common != replay_signature {
        return Err(InvolutiveError::LocalizationDomainMismatch);
    }
    Ok(())
}

struct PolynomialWork<'context, 'budget> {
    context: &'context IndexedCoefficientContext,
    exact_limits: ExactAlgebraLimits,
    budget: &'budget mut LocalizationDomainBudget,
}

impl PolynomialWork<'_, '_> {
    fn try_signature(
        &mut self,
        guards: &[Arc<IndexedPolynomial>],
    ) -> Result<IndexedPolynomial, InvolutiveError> {
        let mut signature = self.one()?;
        for guard in guards {
            if guard.is_nonzero_constant() {
                continue;
            }
            let radical = self.try_square_free_radical(guard)?;
            if radical.raw().is_one() {
                continue;
            }
            if signature.raw().is_one() {
                signature = radical;
                continue;
            }
            let common = self.try_gcd(&signature, &radical)?;
            let quotient = self.try_exact_div(&radical, &common)?;
            signature = self.try_mul(&signature, &quotient)?;
            signature = self.try_primitive_associate(&signature)?;
        }
        signature = self.try_primitive_associate(&signature)?;
        self.budget.try_record_output(&signature)?;
        Ok(signature)
    }

    fn one(&self) -> Result<IndexedPolynomial, InvolutiveError> {
        Ok(self
            .context
            .numerator_condition_with_limits(&self.context.one(), self.exact_limits)?)
    }

    fn try_square_free_radical(
        &mut self,
        value: &IndexedPolynomial,
    ) -> Result<IndexedPolynomial, InvolutiveError> {
        let value = self.try_primitive_associate(value)?;
        if value.is_nonzero_constant() {
            return self.one();
        }
        let envelope = square_free_envelope(value.raw())?;
        self.budget
            .try_preflight_factor_outputs(envelope.factor_outputs)?;
        self.budget
            .try_begin_native(NativeKind::SquareFree, 0, envelope.work, envelope.output)?;
        let factors = catch_unwind(AssertUnwindSafe(|| value.raw().square_free_factorization()))
            .map_err(|_| InvolutiveError::NativePolynomialPanic {
                operation: "computing a cold localization square-free factorization",
            })?;
        if factors.is_empty() || factors.len() > envelope.factor_outputs {
            return Err(InvolutiveError::Invariant {
                detail: "Symbolica square-free output escaped its admitted factor-count envelope",
            });
        }
        self.budget.try_record_factor_outputs(factors.len())?;
        let mut radical = self.one()?;
        for (raw, multiplicity) in factors {
            if multiplicity == 0 {
                return Err(InvolutiveError::Invariant {
                    detail: "Symbolica returned a zero-multiplicity square-free factor",
                });
            }
            let factor = self
                .context
                .admit_native_polynomial_result_with_limits(raw, self.exact_limits)?;
            self.budget.try_record_intermediate(&factor)?;
            if factor.is_zero() {
                return Err(InvolutiveError::Invariant {
                    detail: "Symbolica returned a zero square-free factor",
                });
            }
            if factor.is_nonzero_constant() {
                continue;
            }
            let factor = self.try_primitive_associate(&factor)?;
            radical = self.try_mul(&radical, &factor)?;
        }
        self.try_primitive_associate(&radical)
    }

    fn try_primitive_associate(
        &mut self,
        value: &IndexedPolynomial,
    ) -> Result<IndexedPolynomial, InvolutiveError> {
        self.context
            .validate_polynomial_with_limits(value, self.exact_limits)?;
        if value.is_zero() {
            return Err(InvolutiveError::Invariant {
                detail: "a zero polynomial cannot define a localization-domain signature",
            });
        }
        let prospective = polynomial_census(value.raw())?;
        self.budget
            .try_begin_native(NativeKind::Primitive, 0, 0, prospective)?;
        let normalized = catch_unwind(AssertUnwindSafe(|| {
            self.context.primitive_guard_associate_with_limits(
                value,
                self.exact_limits,
                self.budget.limits.max_native_output_bytes,
            )
        }))
        .map_err(|_| InvolutiveError::NativePolynomialPanic {
            operation: "normalizing a cold localization primitive associate",
        })??;
        self.budget.try_record_intermediate(&normalized)?;
        Ok(normalized)
    }

    fn try_gcd(
        &mut self,
        left: &IndexedPolynomial,
        right: &IndexedPolynomial,
    ) -> Result<IndexedPolynomial, InvolutiveError> {
        self.validate_pair(left, right)?;
        let pair_work = checked_mul(
            NATIVE_TERM_PAIR_WORK,
            left.raw().nterms(),
            right.raw().nterms(),
        )?;
        check_limit(
            NATIVE_TERM_PAIR_WORK,
            pair_work,
            self.exact_limits.max_term_operations,
        )?;
        let prospective = gcd_output_envelope(left.raw(), right.raw())?;
        self.budget
            .try_begin_native(NativeKind::Gcd, pair_work, 0, prospective)?;
        let raw = catch_unwind(AssertUnwindSafe(|| left.raw().gcd(right.raw()))).map_err(|_| {
            InvolutiveError::NativePolynomialPanic {
                operation: "computing a cold localization polynomial GCD",
            }
        })?;
        let gcd = self
            .context
            .admit_native_polynomial_result_with_limits(raw, self.exact_limits)?;
        self.budget.try_record_intermediate(&gcd)?;
        if gcd.is_zero() {
            return Err(InvolutiveError::Invariant {
                detail: "a nonzero localization GCD unexpectedly vanished",
            });
        }
        Ok(gcd)
    }

    fn try_exact_div(
        &mut self,
        numerator: &IndexedPolynomial,
        denominator: &IndexedPolynomial,
    ) -> Result<IndexedPolynomial, InvolutiveError> {
        self.validate_pair(numerator, denominator)?;
        if denominator.is_zero() {
            return Err(InvolutiveError::Invariant {
                detail: "a zero polynomial reached cold localization exact division",
            });
        }
        let pair_work = checked_mul(
            NATIVE_TERM_PAIR_WORK,
            numerator.raw().nterms(),
            denominator.raw().nterms(),
        )?;
        check_limit(
            NATIVE_TERM_PAIR_WORK,
            pair_work,
            self.exact_limits.max_term_operations,
        )?;
        let prospective = dense_output_envelope(numerator.raw())?;
        self.budget
            .try_begin_native(NativeKind::ExactDivision, pair_work, 0, prospective)?;
        let raw = catch_unwind(AssertUnwindSafe(|| {
            numerator.raw().try_div(denominator.raw())
        }))
        .map_err(|_| InvolutiveError::NativePolynomialPanic {
            operation: "performing a cold localization exact polynomial division",
        })?
        .ok_or(InvolutiveError::NonExactPolynomialDivision {
            operation: "cold localization radical LCM",
        })?;
        let quotient = self
            .context
            .admit_native_polynomial_result_with_limits(raw, self.exact_limits)?;
        self.budget.try_record_intermediate(&quotient)?;
        Ok(quotient)
    }

    fn try_mul(
        &mut self,
        left: &IndexedPolynomial,
        right: &IndexedPolynomial,
    ) -> Result<IndexedPolynomial, InvolutiveError> {
        self.validate_pair(left, right)?;
        let pair_work = checked_mul(
            NATIVE_TERM_PAIR_WORK,
            left.raw().nterms(),
            right.raw().nterms(),
        )?;
        check_limit(
            NATIVE_TERM_PAIR_WORK,
            pair_work,
            self.exact_limits.max_term_operations,
        )?;
        let prospective = product_output_envelope(
            left.raw(),
            right.raw(),
            pair_work,
            self.exact_limits.max_exponent,
        )?;
        self.budget
            .try_begin_native(NativeKind::Multiplication, pair_work, 0, prospective)?;
        let raw = catch_unwind(AssertUnwindSafe(|| left.raw() * right.raw())).map_err(|_| {
            InvolutiveError::NativePolynomialPanic {
                operation: "multiplying cold localization polynomials",
            }
        })?;
        let product = self
            .context
            .admit_native_polynomial_result_with_limits(raw, self.exact_limits)?;
        self.budget.try_record_intermediate(&product)?;
        if product.is_zero() {
            return Err(InvolutiveError::Invariant {
                detail: "nonzero localization factors produced a zero product",
            });
        }
        Ok(product)
    }

    fn validate_pair(
        &self,
        left: &IndexedPolynomial,
        right: &IndexedPolynomial,
    ) -> Result<(), InvolutiveError> {
        self.context
            .validate_polynomial_with_limits(left, self.exact_limits)?;
        self.context
            .validate_polynomial_with_limits(right, self.exact_limits)?;
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum NativeKind {
    SquareFree,
    Gcd,
    ExactDivision,
    Multiplication,
    Primitive,
}

#[derive(Clone, Copy)]
struct SpecializedNativeCount {
    kind: NativeKind,
    value: usize,
}

impl NativeKind {
    fn try_next(
        self,
        census: &LocalizationDomainCensus,
    ) -> Result<SpecializedNativeCount, InvolutiveError> {
        let (resource, current) = match self {
            Self::SquareFree => (SQUARE_FREE_CALLS, census.square_free_calls),
            Self::Gcd => (GCD_CALLS, census.gcd_calls),
            Self::ExactDivision => (EXACT_DIVISIONS, census.exact_divisions),
            Self::Multiplication => (MULTIPLICATIONS, census.multiplications),
            Self::Primitive => (PRIMITIVE_NORMALIZATIONS, census.primitive_normalizations),
        };
        Ok(SpecializedNativeCount {
            kind: self,
            value: checked_add(resource, current, 1)?,
        })
    }
}

impl SpecializedNativeCount {
    fn try_check(self, limits: LocalizationDomainLimits) -> Result<(), InvolutiveError> {
        let (resource, limit) = match self.kind {
            NativeKind::SquareFree => (SQUARE_FREE_CALLS, limits.max_square_free_calls),
            NativeKind::Gcd => (GCD_CALLS, limits.max_gcd_calls),
            NativeKind::ExactDivision => (EXACT_DIVISIONS, limits.max_exact_divisions),
            NativeKind::Multiplication => (MULTIPLICATIONS, limits.max_multiplications),
            NativeKind::Primitive => (
                PRIMITIVE_NORMALIZATIONS,
                limits.max_primitive_normalizations,
            ),
        };
        check_limit(resource, self.value, limit)
    }

    fn commit(self, census: &mut LocalizationDomainCensus) {
        match self.kind {
            NativeKind::SquareFree => census.square_free_calls = self.value,
            NativeKind::Gcd => census.gcd_calls = self.value,
            NativeKind::ExactDivision => census.exact_divisions = self.value,
            NativeKind::Multiplication => census.multiplications = self.value,
            NativeKind::Primitive => census.primitive_normalizations = self.value,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct PolynomialCensus {
    terms: usize,
    exponent_cells: usize,
    bytes: usize,
}

impl PolynomialCensus {
    fn try_add(self, right: Self) -> Result<Self, InvolutiveError> {
        Ok(Self {
            terms: checked_add(INPUT_TERMS, self.terms, right.terms)?,
            exponent_cells: checked_add(
                INPUT_EXPONENT_CELLS,
                self.exponent_cells,
                right.exponent_cells,
            )?,
            bytes: checked_add(INPUT_BYTES, self.bytes, right.bytes)?,
        })
    }
}

struct SquareFreeEnvelope {
    factor_outputs: usize,
    work: usize,
    output: PolynomialCensus,
}

fn square_free_envelope(
    polynomial: &CoefficientPolynomial,
) -> Result<SquareFreeEnvelope, InvolutiveError> {
    let (dense_slots, total_degree, active_variables) = degree_envelope(polynomial)?;
    let factor_outputs = checked_add(FACTOR_OUTPUTS, total_degree, 1)?;
    let prospective_terms = checked_mul(NATIVE_OUTPUT_TERMS, dense_slots, total_degree.max(1))?;
    let coefficient_bits = factor_coefficient_bit_envelope(polynomial)?;
    let output = prospective_census(
        prospective_terms,
        polynomial.nvars(),
        coefficient_bits,
        factor_outputs,
    )?;
    let work = checked_mul(
        SQUARE_FREE_WORK,
        checked_mul(SQUARE_FREE_WORK, dense_slots, dense_slots)?,
        active_variables.max(1),
    )?;
    Ok(SquareFreeEnvelope {
        factor_outputs,
        work,
        output,
    })
}

fn gcd_output_envelope(
    left: &CoefficientPolynomial,
    right: &CoefficientPolynomial,
) -> Result<PolynomialCensus, InvolutiveError> {
    let mut terms = 1usize;
    for position in 0..left.nvars() {
        let degree = usize::from(left.degree(position).min(right.degree(position)));
        terms = checked_mul(
            NATIVE_OUTPUT_TERMS,
            terms,
            checked_add(NATIVE_OUTPUT_TERMS, degree, 1)?,
        )?;
    }
    prospective_census(
        terms,
        left.nvars(),
        factor_coefficient_bit_envelope(left)?.min(factor_coefficient_bit_envelope(right)?),
        1,
    )
}

fn dense_output_envelope(
    polynomial: &CoefficientPolynomial,
) -> Result<PolynomialCensus, InvolutiveError> {
    let (terms, _, _) = degree_envelope(polynomial)?;
    prospective_census(
        terms,
        polynomial.nvars(),
        factor_coefficient_bit_envelope(polynomial)?,
        1,
    )
}

/// Conservative coefficient-height bound for every integer-polynomial
/// factor of `polynomial`.
///
/// Substitute the variables by mixed-radix powers of one indeterminate, with
/// radix `degree_i + 1`.  Every monomial in the input and in either factor has
/// a unique image, multiplication introduces no radix carry because factor
/// degrees add to the input degree, and coefficients are unchanged.  The
/// resulting univariate polynomial has degree at most `dense_slots - 1`.
/// The univariate Mignotte bound then gives
///
/// `bits(factor) <= bits(input) + degree + log2(number of input terms)`.
///
/// The final logarithm is deliberately rounded up rather than halved.  This
/// is looser than the Euclidean-norm form of the bound but keeps the native
/// byte preflight simple and unambiguously conservative.  GCDs and exact
/// quotients are factors too; using only their input coefficient height is
/// unsound because cancellation can make a factor taller than its product.
fn factor_coefficient_bit_envelope(
    polynomial: &CoefficientPolynomial,
) -> Result<usize, InvolutiveError> {
    let (dense_slots, _, _) = degree_envelope(polynomial)?;
    checked_add(
        NATIVE_OUTPUT_BYTES,
        max_integer_bits(polynomial)?,
        checked_add(
            NATIVE_OUTPUT_BYTES,
            dense_slots.saturating_sub(1),
            ceil_log2(polynomial.nterms().max(1)),
        )?,
    )
}

fn product_output_envelope(
    left: &CoefficientPolynomial,
    right: &CoefficientPolynomial,
    term_pairs: usize,
    max_exponent: u16,
) -> Result<PolynomialCensus, InvolutiveError> {
    let mut dense_slots = 1usize;
    for position in 0..left.nvars() {
        let degree = usize::from(left.degree(position))
            .checked_add(usize::from(right.degree(position)))
            .ok_or(InvolutiveError::ResourceCountOverflow {
                resource: NATIVE_OUTPUT_EXPONENT_CELLS,
            })?;
        if degree > usize::from(max_exponent) {
            return Err(InvolutiveError::ResourceLimit {
                resource: "cold localization-domain product exponent",
                requested: degree,
                limit: usize::from(max_exponent),
            });
        }
        dense_slots = checked_mul(
            NATIVE_OUTPUT_TERMS,
            dense_slots,
            checked_add(NATIVE_OUTPUT_TERMS, degree, 1)?,
        )?;
    }
    let terms = dense_slots.min(term_pairs);
    let coefficient_bits = checked_add(
        NATIVE_OUTPUT_BYTES,
        checked_add(
            NATIVE_OUTPUT_BYTES,
            max_integer_bits(left)?,
            max_integer_bits(right)?,
        )?,
        ceil_log2(left.nterms().min(right.nterms()).max(1)),
    )?;
    prospective_census(terms, left.nvars(), coefficient_bits, 1)
}

fn degree_envelope(
    polynomial: &CoefficientPolynomial,
) -> Result<(usize, usize, usize), InvolutiveError> {
    let mut dense_slots = 1usize;
    let mut total_degree = 0usize;
    let mut active_variables = 0usize;
    for position in 0..polynomial.nvars() {
        let degree = usize::from(polynomial.degree(position));
        if degree != 0 {
            active_variables = checked_add(SQUARE_FREE_WORK, active_variables, 1)?;
        }
        total_degree = checked_add(SQUARE_FREE_WORK, total_degree, degree)?;
        dense_slots = checked_mul(
            SQUARE_FREE_WORK,
            dense_slots,
            checked_add(SQUARE_FREE_WORK, degree, 1)?,
        )?;
    }
    Ok((dense_slots, total_degree, active_variables))
}

fn prospective_census(
    terms: usize,
    variables: usize,
    coefficient_bits: usize,
    polynomial_count: usize,
) -> Result<PolynomialCensus, InvolutiveError> {
    let exponent_cells = checked_mul(NATIVE_OUTPUT_EXPONENT_CELLS, terms, variables)?;
    let coefficient_bytes = checked_add(NATIVE_OUTPUT_BYTES, coefficient_bits, 7)? / 8;
    let per_term = checked_add(
        NATIVE_OUTPUT_BYTES,
        checked_add(
            NATIVE_OUTPUT_BYTES,
            size_of::<Integer>(),
            checked_mul(NATIVE_OUTPUT_BYTES, variables, size_of::<u16>())?,
        )?,
        coefficient_bytes,
    )?;
    let bytes = checked_add(
        NATIVE_OUTPUT_BYTES,
        checked_mul(
            NATIVE_OUTPUT_BYTES,
            polynomial_count,
            size_of::<IndexedPolynomial>(),
        )?,
        checked_mul(NATIVE_OUTPUT_BYTES, terms, per_term)?,
    )?;
    Ok(PolynomialCensus {
        terms,
        exponent_cells,
        bytes,
    })
}

fn polynomial_census(
    polynomial: &CoefficientPolynomial,
) -> Result<PolynomialCensus, InvolutiveError> {
    let terms = polynomial.coefficients.len();
    let exponent_cells = polynomial.exponents.len();
    let mut bytes = checked_add(
        INPUT_BYTES,
        size_of::<IndexedPolynomial>(),
        checked_add(
            INPUT_BYTES,
            checked_mul(INPUT_BYTES, terms, size_of::<Integer>())?,
            checked_mul(INPUT_BYTES, exponent_cells, size_of::<u16>())?,
        )?,
    )?;
    for coefficient in &polynomial.coefficients {
        if let Integer::Large(value) = coefficient {
            let bits = usize::try_from(value.significant_bits()).map_err(|_| {
                InvolutiveError::ResourceCountOverflow {
                    resource: INPUT_BYTES,
                }
            })?;
            bytes = checked_add(INPUT_BYTES, bytes, checked_add(INPUT_BYTES, bits, 7)? / 8)?;
        }
    }
    Ok(PolynomialCensus {
        terms,
        exponent_cells,
        bytes,
    })
}

fn max_integer_bits(polynomial: &CoefficientPolynomial) -> Result<usize, InvolutiveError> {
    polynomial
        .coefficients
        .iter()
        .map(|coefficient| match coefficient {
            Integer::Single(value) => {
                Ok((i64::BITS - value.unsigned_abs().leading_zeros()) as usize)
            }
            Integer::Double(value) => {
                Ok((i128::BITS - value.unsigned_abs().leading_zeros()) as usize)
            }
            Integer::Large(value) => usize::try_from(value.significant_bits()).map_err(|_| {
                InvolutiveError::ResourceCountOverflow {
                    resource: NATIVE_OUTPUT_BYTES,
                }
            }),
        })
        .try_fold(0usize, |maximum, bits| bits.map(|bits| maximum.max(bits)))
}

fn ceil_log2(value: usize) -> usize {
    if value <= 1 {
        0
    } else {
        usize::BITS as usize - (value - 1).leading_zeros() as usize
    }
}

fn charge(
    resource: &'static str,
    current: &mut usize,
    amount: usize,
    limit: usize,
) -> Result<(), InvolutiveError> {
    let requested = checked_add(resource, *current, amount)?;
    check_limit(resource, requested, limit)?;
    *current = requested;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algebra::{CoefficientContext, IndexedCoefficient};

    fn context() -> IndexedCoefficientContext {
        let base = CoefficientContext::new(std::iter::empty::<&str>());
        IndexedCoefficientContext::try_new(&base, "cold-localization-domain-tests", 3).unwrap()
    }

    fn guard(
        context: &IndexedCoefficientContext,
        value: &IndexedCoefficient,
    ) -> Arc<IndexedPolynomial> {
        Arc::new(
            context
                .numerator_condition_with_limits(value, ExactAlgebraLimits::default())
                .unwrap(),
        )
    }

    fn univariate(context: &IndexedCoefficientContext, coefficients: &[i64]) -> IndexedCoefficient {
        let x = context.index(0).unwrap();
        let mut value = context.zero();
        let mut power = context.one();
        for &coefficient in coefficients {
            let term = context.mul(&context.integer(coefficient), &power).unwrap();
            value = context.add(&value, &term).unwrap();
            power = context.mul(&power, &x).unwrap();
        }
        value
    }

    fn covered(
        context: &IndexedCoefficientContext,
        authenticated_lazy: &[Arc<IndexedPolynomial>],
        replay_required: &[Arc<IndexedPolynomial>],
    ) -> Result<LocalizationDomainCensus, InvolutiveError> {
        let mut budget = LocalizationDomainBudget::new(LocalizationDomainLimits::default());
        try_require_principal_open_coverage(
            context,
            authenticated_lazy,
            replay_required,
            ExactAlgebraLimits::default(),
            &mut budget,
        )?;
        Ok(budget.census())
    }

    #[test]
    fn powers_associates_and_nonzero_constants_have_one_principal_open_signature() {
        let context = context();
        let a = context
            .add(&context.index(0).unwrap(), &context.one())
            .unwrap();
        let a_squared = context.mul(&a, &a).unwrap();
        let a_fourth = context.mul(&a_squared, &a_squared).unwrap();
        let minus_two_a = context.mul(&context.integer(-2), &a).unwrap();
        let seven = context.integer(7);

        let lazy = [guard(&context, &a_fourth), guard(&context, &seven)];
        let replay = [guard(&context, &minus_two_a), guard(&context, &a_squared)];
        let census = covered(&context, &lazy, &replay).unwrap();
        assert_eq!(census.attempts(), 1);
        assert_eq!(census.input_guards(), 4);
        assert!(census.square_free_calls() >= 3);
        assert!(census.native_operations() > census.square_free_calls());
        assert_eq!(census.output_signatures(), 2);
    }

    #[test]
    fn overlapping_products_and_multivariate_reducibles_compare_by_radical() {
        let context = context();
        let a = context
            .add(&context.index(0).unwrap(), &context.one())
            .unwrap();
        let b = context
            .add(&context.index(1).unwrap(), &context.integer(2))
            .unwrap();
        let c = context
            .add(&context.index(2).unwrap(), &context.integer(3))
            .unwrap();
        let ab = context.mul(&a, &b).unwrap();
        let bc = context.mul(&b, &c).unwrap();
        let abc = context.mul(&ab, &c).unwrap();

        covered(
            &context,
            &[guard(&context, &ab), guard(&context, &bc)],
            &[guard(&context, &abc)],
        )
        .unwrap();

        let sum = context
            .add(&context.index(0).unwrap(), &context.index(1).unwrap())
            .unwrap();
        let difference = context
            .sub(&context.index(0).unwrap(), &context.index(1).unwrap())
            .unwrap();
        let reducible = context.mul(&sum, &difference).unwrap();
        covered(
            &context,
            &[guard(&context, &reducible)],
            &[guard(&context, &sum), guard(&context, &difference)],
        )
        .unwrap();
    }

    #[test]
    fn missing_or_unrelated_replay_factor_fails_closed() {
        let context = context();
        let a = context
            .add(&context.index(0).unwrap(), &context.one())
            .unwrap();
        let b = context
            .add(&context.index(1).unwrap(), &context.one())
            .unwrap();
        let c = context
            .add(&context.index(2).unwrap(), &context.one())
            .unwrap();
        let ab = context.mul(&a, &b).unwrap();

        // Historic lazy guards may conservatively restrict the domain beyond
        // the conditions reconstructed by replay.
        covered(
            &context,
            &[guard(&context, &a), guard(&context, &b)],
            &[guard(&context, &a)],
        )
        .unwrap();

        assert_eq!(
            covered(&context, &[guard(&context, &a)], &[guard(&context, &ab)],),
            Err(InvolutiveError::LocalizationDomainMismatch)
        );
        assert_eq!(
            covered(
                &context,
                &[guard(&context, &a), guard(&context, &b)],
                &[guard(&context, &c)],
            ),
            Err(InvolutiveError::LocalizationDomainMismatch)
        );
    }

    #[test]
    fn zero_guard_is_rejected_before_any_native_operation() {
        let context = context();
        let zero = guard(&context, &context.zero());
        let mut budget = LocalizationDomainBudget::new(LocalizationDomainLimits::default());
        assert_eq!(
            try_require_principal_open_coverage(
                &context,
                &[zero],
                &[],
                ExactAlgebraLimits::default(),
                &mut budget,
            ),
            Err(InvolutiveError::Invariant {
                detail: "a zero polynomial cannot enter a cold localization-domain proof",
            })
        );
        assert_eq!(budget.census().attempts(), 1);
        assert_eq!(budget.census().input_guards(), 0);
        assert_eq!(budget.census().native_operations(), 0);
    }

    #[test]
    fn one_below_input_and_output_limits_are_monotone_and_fail_closed() {
        let context = context();
        let a = context
            .add(&context.index(0).unwrap(), &context.one())
            .unwrap();
        let b = context
            .add(&context.index(1).unwrap(), &context.one())
            .unwrap();
        let ab = context.mul(&a, &b).unwrap();
        let lazy = [guard(&context, &a), guard(&context, &b)];
        let replay = [guard(&context, &ab)];

        let mut input_limits = LocalizationDomainLimits::default();
        input_limits.max_input_guards = lazy.len() + replay.len() - 1;
        let mut input_budget = LocalizationDomainBudget::new(input_limits);
        assert_eq!(
            try_require_principal_open_coverage(
                &context,
                &lazy,
                &replay,
                ExactAlgebraLimits::default(),
                &mut input_budget,
            ),
            Err(InvolutiveError::ResourceLimit {
                resource: INPUT_GUARDS,
                requested: 3,
                limit: 2,
            })
        );
        assert_eq!(input_budget.census().attempts(), 1);
        assert_eq!(input_budget.census().input_guards(), 0);
        assert_eq!(input_budget.census().native_operations(), 0);

        let baseline = covered(&context, &lazy, &replay).unwrap();
        assert_eq!(baseline.output_signatures(), 2);
        let mut output_limits = LocalizationDomainLimits::default();
        output_limits.max_output_signatures = baseline.output_signatures() - 1;
        let mut output_budget = LocalizationDomainBudget::new(output_limits);
        assert_eq!(
            try_require_principal_open_coverage(
                &context,
                &lazy,
                &replay,
                ExactAlgebraLimits::default(),
                &mut output_budget,
            ),
            Err(InvolutiveError::ResourceLimit {
                resource: OUTPUT_SIGNATURES,
                requested: 2,
                limit: 1,
            })
        );
        assert_eq!(output_budget.census().attempts(), 1);
        assert_eq!(output_budget.census().output_signatures(), 1);
        assert!(output_budget.census().native_operations() > 0);
    }

    #[test]
    fn factor_height_envelopes_cover_taller_exact_quotients_and_gcds() {
        let context = context();

        // (7 + 3x - 3x^2 - 7x^3) / (x - 1) = -7 - 10x - 7x^2.
        // All factors are square-free and pairwise coprime, so this exact
        // quotient can occur while incrementally forming a radical LCM.
        let numerator = guard(&context, &univariate(&context, &[7, 3, -3, -7]));
        let divisor = guard(&context, &univariate(&context, &[-1, 1]));
        let quotient = context
            .admit_native_polynomial_result_with_limits(
                numerator.raw().try_div(divisor.raw()).unwrap(),
                ExactAlgebraLimits::default(),
            )
            .unwrap();
        assert!(
            max_integer_bits(quotient.raw()).unwrap() > max_integer_bits(numerator.raw()).unwrap()
        );
        assert!(
            factor_coefficient_bit_envelope(numerator.raw()).unwrap()
                >= max_integer_bits(quotient.raw()).unwrap()
        );

        // Both products have height 15, but their primitive GCD
        // 15 + 29x + 15x^2 has height 29.  The two cofactors x-1 and
        // -(x^2-x+1) are coprime, so this is the complete polynomial GCD.
        let left = guard(&context, &univariate(&context, &[15, 14, -14, -15]));
        let right = guard(&context, &univariate(&context, &[15, 14, 1, 14, 15]));
        let gcd = context
            .admit_native_polynomial_result_with_limits(
                left.raw().gcd(right.raw()),
                ExactAlgebraLimits::default(),
            )
            .unwrap();
        let input_bits = max_integer_bits(left.raw())
            .unwrap()
            .max(max_integer_bits(right.raw()).unwrap());
        assert!(max_integer_bits(gcd.raw()).unwrap() > input_bits);
        assert!(
            factor_coefficient_bit_envelope(left.raw())
                .unwrap()
                .min(factor_coefficient_bit_envelope(right.raw()).unwrap())
                >= max_integer_bits(gcd.raw()).unwrap()
        );
    }

    #[test]
    fn one_below_exact_quotient_envelope_rejects_before_native_entry() {
        let context = context();
        let numerator = guard(&context, &univariate(&context, &[7, 3, -3, -7]));
        let divisor = guard(&context, &univariate(&context, &[-1, 1]));
        let envelope = dense_output_envelope(numerator.raw()).unwrap();
        assert!(envelope.bytes > 0);
        let mut limits = LocalizationDomainLimits::default();
        limits.max_native_output_bytes = envelope.bytes - 1;
        let mut budget = LocalizationDomainBudget::new(limits);
        let mut work = PolynomialWork {
            context: &context,
            exact_limits: ExactAlgebraLimits::default(),
            budget: &mut budget,
        };

        assert_eq!(
            work.try_exact_div(&numerator, &divisor),
            Err(InvolutiveError::ResourceLimit {
                resource: NATIVE_OUTPUT_BYTES,
                requested: envelope.bytes,
                limit: envelope.bytes - 1,
            })
        );
        assert_eq!(budget.census().native_operations(), 0);
        assert_eq!(budget.census().exact_divisions(), 0);
    }

    #[test]
    fn localization_domain_errors_have_stable_typed_diagnostics() {
        assert_eq!(
            InvolutiveError::LocalizationDomainMismatch.to_string(),
            "the authenticated lazy localization does not imply every replay-required nonzero condition"
        );
        assert_eq!(
            InvolutiveError::NativePolynomialPanic {
                operation: "testing a native operation",
            }
            .to_string(),
            "Symbolica panicked while testing a native operation"
        );
        assert_eq!(
            InvolutiveError::NonExactPolynomialDivision {
                operation: "testing exact division",
            }
            .to_string(),
            "testing exact division was not an exact polynomial division"
        );
    }
}
