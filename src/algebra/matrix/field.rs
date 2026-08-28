//! Checked Symbolica field adapter and typed unwind transport.

use std::cell::RefCell;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::rc::Rc;

use rand::RngCore;
use symbolica::domains::SelfRing;
use symbolica::prelude::*;
use symbolica::tensors::matrix::MatrixError;

use crate::algebra::{
    Coefficient, CoefficientContext, CoefficientPolynomialPart, ExactAlgebraError,
    ExactAlgebraLimits, ExactAlgebraOperation,
};

use super::admission::{check_limit, checked_add, coefficient_retained_bytes};
use super::{
    SymbolicaCoefficientMatrixError, SymbolicaCoefficientMatrixLimits,
    SymbolicaCoefficientMatrixStats, SymbolicaNativeMatrixErrorKind,
};

#[derive(Clone, Copy, Debug)]

enum AtomicOperation {
    Add,
    Subtract,
    Multiply,
    Divide,
    Negate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PolynomialPowerAdmission {
    output_terms: usize,
    max_term_operations: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CoefficientPowerAdmission {
    numerator: PolynomialPowerAdmission,
    denominator: PolynomialPowerAdmission,
}

impl CoefficientPowerAdmission {
    fn max_term_operations(self) -> usize {
        self.numerator
            .max_term_operations
            .max(self.denominator.max_term_operations)
    }
}

fn polynomial_power_resource(part: CoefficientPolynomialPart, output: bool) -> &'static str {
    match (part, output) {
        (CoefficientPolynomialPart::Numerator, false) => {
            "exact coefficient power numerator term operations"
        }
        (CoefficientPolynomialPart::Denominator, false) => {
            "exact coefficient power denominator term operations"
        }
        (CoefficientPolynomialPart::Numerator, true) => {
            "exact coefficient power numerator output terms"
        }
        (CoefficientPolynomialPart::Denominator, true) => {
            "exact coefficient power denominator output terms"
        }
    }
}

fn polynomial_power_degree_box(
    polynomial: &MultivariatePolynomial<IntegerRing, u16>,
    exponent: u64,
    operation: ExactAlgebraOperation,
    resource: &'static str,
    limit: usize,
) -> Result<usize, ExactAlgebraError> {
    let mut terms = 1usize;
    for variable in 0..polynomial.variables.len() {
        let degree = u64::from(polynomial.degree(variable))
            .checked_mul(exponent)
            .ok_or(ExactAlgebraError::ExponentArithmeticOverflow {
                operation,
                variable,
                width: 64,
            })?;
        let width = degree
            .checked_add(1)
            .and_then(|width| usize::try_from(width).ok())
            .ok_or(ExactAlgebraError::ResourceCountOverflow { resource })?;
        terms = terms
            .checked_mul(width)
            .ok_or(ExactAlgebraError::ResourceCountOverflow { resource })?;
        if terms > limit {
            return Err(ExactAlgebraError::ResourceLimit {
                resource,
                requested: terms,
                limit,
            });
        }
    }
    if terms > limit {
        Err(ExactAlgebraError::ResourceLimit {
            resource,
            requested: terms,
            limit,
        })
    } else {
        Ok(terms)
    }
}

fn polynomial_power_admission(
    polynomial: &MultivariatePolynomial<IntegerRing, u16>,
    exponent: u64,
    part: CoefficientPolynomialPart,
    limits: ExactAlgebraLimits,
) -> Result<PolynomialPowerAdmission, ExactAlgebraError> {
    if exponent == 0 {
        return Ok(PolynomialPowerAdmission {
            output_terms: 1,
            max_term_operations: 0,
        });
    }
    if polynomial.is_zero() {
        return Ok(PolynomialPowerAdmission {
            output_terms: 0,
            max_term_operations: 0,
        });
    }

    let output_resource = polynomial_power_resource(part, true);
    let operation_resource = polynomial_power_resource(part, false);
    // Symbolica's native rational-polynomial power performs repeated
    // multiplication. Cross-GCD quotients can be denser than the sparse
    // inputs, so use the componentwise degree box rather than nterms^e.
    let output_terms = polynomial_power_degree_box(
        polynomial,
        exponent,
        ExactAlgebraOperation::Power,
        output_resource,
        limits.max_polynomial_terms,
    )?;
    let previous_terms = polynomial_power_degree_box(
        polynomial,
        exponent - 1,
        ExactAlgebraOperation::Power,
        operation_resource,
        limits.max_term_operations,
    )?;
    let base_terms = polynomial_power_degree_box(
        polynomial,
        1,
        ExactAlgebraOperation::Power,
        operation_resource,
        limits.max_term_operations,
    )?;
    let max_term_operations =
        previous_terms
            .checked_mul(base_terms)
            .ok_or(ExactAlgebraError::ResourceCountOverflow {
                resource: operation_resource,
            })?;
    if max_term_operations > limits.max_term_operations {
        return Err(ExactAlgebraError::ResourceLimit {
            resource: operation_resource,
            requested: max_term_operations,
            limit: limits.max_term_operations,
        });
    }
    Ok(PolynomialPowerAdmission {
        output_terms,
        max_term_operations,
    })
}

/// Private unwind payload for fallible trait methods.  `resume_unwind` avoids
/// invoking the process-global panic hook; the nearest matrix boundary catches
/// and downcasts this exact type immediately.
pub(super) struct CheckedFieldAbort(SymbolicaCoefficientMatrixError);

#[cold]
pub(super) fn abort_checked_field(error: ExactAlgebraError) -> ! {
    abort_checked_matrix(SymbolicaCoefficientMatrixError::ExactAlgebra(error))
}

#[cold]
pub(super) fn abort_checked_matrix(error: SymbolicaCoefficientMatrixError) -> ! {
    resume_unwind(Box::new(CheckedFieldAbort(error)))
}

#[derive(Debug, Default)]
pub(super) struct CheckedFieldState {
    pub(super) stats: SymbolicaCoefficientMatrixStats,
}

#[derive(Clone)]
pub(super) struct CheckedCoefficientField<'context> {
    pub(super) context: &'context CoefficientContext,
    inner: RationalPolynomialField<IntegerRing, u16>,
    pub(super) limits: SymbolicaCoefficientMatrixLimits,
    pub(super) state: Rc<RefCell<CheckedFieldState>>,
}

impl fmt::Debug for CheckedCoefficientField<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CheckedCoefficientField")
            .field("variables", &self.context.parameter_names().len())
            .finish_non_exhaustive()
    }
}

impl fmt::Display for CheckedCoefficientField<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("authenticated RustRed coefficient field")
    }
}

impl PartialEq for CheckedCoefficientField<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.context.variables() == other.context.variables()
    }
}

impl Eq for CheckedCoefficientField<'_> {}

impl Hash for CheckedCoefficientField<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.context.variables().hash(state);
    }
}

impl<'context> CheckedCoefficientField<'context> {
    pub(super) fn new(
        context: &'context CoefficientContext,
        limits: SymbolicaCoefficientMatrixLimits,
        admitted_single_matrix_entries: usize,
        admitted_peak_live_entries: usize,
        admitted_exact_operations: usize,
    ) -> Self {
        let mut stats = SymbolicaCoefficientMatrixStats::default();
        stats.admitted_single_matrix_entries = admitted_single_matrix_entries;
        stats.admitted_peak_live_entries = admitted_peak_live_entries;
        stats.admitted_exact_operations = admitted_exact_operations;
        Self {
            context,
            inner: RationalPolynomialField::new(Z),
            limits,
            state: Rc::new(RefCell::new(CheckedFieldState { stats })),
        }
    }

    fn charge_operation(&self, operation: AtomicOperation) {
        let result = {
            let mut state = self.state.borrow_mut();
            let requested = state.stats.exact_operations.checked_add(1).ok_or(
                ExactAlgebraError::ResourceCountOverflow {
                    resource: "Symbolica coefficient matrix exact operations",
                },
            );
            match requested {
                Ok(requested) if requested <= self.limits.max_exact_operations => {
                    let operation_requested = match operation {
                        AtomicOperation::Add => state.stats.additions.checked_add(1),
                        AtomicOperation::Subtract => state.stats.subtractions.checked_add(1),
                        AtomicOperation::Multiply => state.stats.multiplications.checked_add(1),
                        AtomicOperation::Divide => state.stats.divisions.checked_add(1),
                        AtomicOperation::Negate => state.stats.negations.checked_add(1),
                    }
                    .ok_or(ExactAlgebraError::ResourceCountOverflow {
                        resource: "Symbolica coefficient matrix operation census",
                    });
                    let operation_requested =
                        operation_requested.unwrap_or_else(|error| abort_checked_field(error));
                    state.stats.exact_operations = requested;
                    match operation {
                        AtomicOperation::Add => state.stats.additions = operation_requested,
                        AtomicOperation::Subtract => state.stats.subtractions = operation_requested,
                        AtomicOperation::Multiply => {
                            state.stats.multiplications = operation_requested
                        }
                        AtomicOperation::Divide => state.stats.divisions = operation_requested,
                        AtomicOperation::Negate => state.stats.negations = operation_requested,
                    }
                    Ok(())
                }
                Ok(requested) => Err(ExactAlgebraError::ResourceLimit {
                    resource: "Symbolica coefficient matrix exact operations",
                    requested,
                    limit: self.limits.max_exact_operations,
                }),
                Err(error) => Err(error),
            }
        };
        if let Err(error) = result {
            abort_checked_field(error);
        }
    }

    fn preflight_power_admission(
        &self,
        base: &Coefficient,
        exponent: u64,
    ) -> CoefficientPowerAdmission {
        if let Err(error) =
            self.context
                .preflight_power_with_limits(base, exponent, self.limits.exact_algebra)
        {
            abort_checked_field(error);
        }
        if exponent > u64::from(u32::MAX) {
            abort_checked_matrix(SymbolicaCoefficientMatrixError::NativePowerExponentLimit {
                requested: exponent,
                limit: u32::MAX,
            });
        }
        let numerator = polynomial_power_admission(
            &base.numerator,
            exponent,
            CoefficientPolynomialPart::Numerator,
            self.limits.exact_algebra,
        )
        .unwrap_or_else(|error| abort_checked_field(error));
        let denominator = polynomial_power_admission(
            &base.denominator,
            exponent,
            CoefficientPolynomialPart::Denominator,
            self.limits.exact_algebra,
        )
        .unwrap_or_else(|error| abort_checked_field(error));
        CoefficientPowerAdmission {
            numerator,
            denominator,
        }
    }

    fn charge_power_operations(&self, exponent: u64, admission: CoefficientPowerAdmission) {
        if exponent > u64::from(u32::MAX) {
            abort_checked_matrix(SymbolicaCoefficientMatrixError::NativePowerExponentLimit {
                requested: exponent,
                limit: u32::MAX,
            });
        }
        let operations = usize::try_from(exponent).unwrap_or_else(|_| {
            abort_checked_field(ExactAlgebraError::ResourceCountOverflow {
                resource: "Symbolica coefficient power operations",
            })
        });

        let result = {
            let mut state = self.state.borrow_mut();
            let exact_operations = state.stats.exact_operations.checked_add(operations).ok_or(
                ExactAlgebraError::ResourceCountOverflow {
                    resource: "Symbolica coefficient matrix exact operations",
                },
            );
            let multiplications = state.stats.multiplications.checked_add(operations).ok_or(
                ExactAlgebraError::ResourceCountOverflow {
                    resource: "Symbolica coefficient matrix operation census",
                },
            );
            match (exact_operations, multiplications) {
                (Ok(exact_operations), Ok(multiplications))
                    if exact_operations <= self.limits.max_exact_operations =>
                {
                    state.stats.exact_operations = exact_operations;
                    state.stats.multiplications = multiplications;
                    state.stats.admitted_power_exponent =
                        state.stats.admitted_power_exponent.max(exponent);
                    state.stats.admitted_power_term_operations = state
                        .stats
                        .admitted_power_term_operations
                        .max(admission.max_term_operations());
                    state.stats.admitted_power_numerator_terms = state
                        .stats
                        .admitted_power_numerator_terms
                        .max(admission.numerator.output_terms);
                    state.stats.admitted_power_denominator_terms = state
                        .stats
                        .admitted_power_denominator_terms
                        .max(admission.denominator.output_terms);
                    Ok(())
                }
                (Ok(exact_operations), Ok(_)) => Err(ExactAlgebraError::ResourceLimit {
                    resource: "Symbolica coefficient matrix exact operations",
                    requested: exact_operations,
                    limit: self.limits.max_exact_operations,
                }),
                (Err(error), _) | (_, Err(error)) => Err(error),
            }
        };
        if let Err(error) = result {
            abort_checked_field(error);
        }
    }

    fn charge_counter(
        &self,
        select: impl FnOnce(&mut SymbolicaCoefficientMatrixStats) -> &mut usize,
    ) {
        let error = {
            let mut state = self.state.borrow_mut();
            let counter = select(&mut state.stats);
            match counter.checked_add(1) {
                Some(next) => {
                    *counter = next;
                    None
                }
                None => Some(ExactAlgebraError::ResourceCountOverflow {
                    resource: "Symbolica coefficient matrix field calls",
                }),
            }
        };
        if let Some(error) = error {
            abort_checked_field(error);
        }
    }

    fn finish(&self, result: Result<Coefficient, ExactAlgebraError>) -> Coefficient {
        match result {
            Ok(value) => value,
            Err(error) => abort_checked_field(error),
        }
    }

    fn finish_raw(&self, value: Coefficient) -> Coefficient {
        if let Err(error) = self
            .context
            .validate_with_limits(&value, self.limits.exact_algebra)
        {
            abort_checked_field(error);
        }
        value
    }

    fn finish_power_raw(
        &self,
        value: Coefficient,
        admission: CoefficientPowerAdmission,
    ) -> Coefficient {
        let result = (|| {
            self.context
                .validate_with_limits(&value, self.limits.exact_algebra)
                .map_err(SymbolicaCoefficientMatrixError::ExactAlgebra)?;
            let numerator_terms = value.numerator.nterms();
            let denominator_terms = value.denominator.nterms();
            check_limit(
                polynomial_power_resource(CoefficientPolynomialPart::Numerator, true),
                numerator_terms,
                admission.numerator.output_terms,
            )?;
            check_limit(
                polynomial_power_resource(CoefficientPolynomialPart::Denominator, true),
                denominator_terms,
                admission.denominator.output_terms,
            )?;
            let retained_bytes = coefficient_retained_bytes(&value)?;
            let mut state = self.state.borrow_mut();
            let output_retained_bytes = checked_add(
                "coefficient matrix output retained bytes",
                state.stats.output_retained_bytes,
                retained_bytes,
            )?;
            check_limit(
                "coefficient matrix output retained bytes",
                output_retained_bytes,
                self.limits.max_output_retained_bytes,
            )?;
            let authenticated_entries = checked_add(
                "authenticated Symbolica matrix entries",
                state.stats.authenticated_entries,
                1,
            )?;
            let output_entries = checked_add(
                "coefficient matrix output entries",
                state.stats.output_entries,
                1,
            )?;
            state.stats.output_retained_bytes = output_retained_bytes;
            state.stats.authenticated_entries = authenticated_entries;
            state.stats.output_entries = output_entries;
            state.stats.output_power_numerator_terms = state
                .stats
                .output_power_numerator_terms
                .max(numerator_terms);
            state.stats.output_power_denominator_terms = state
                .stats
                .output_power_denominator_terms
                .max(denominator_terms);
            Ok::<(), SymbolicaCoefficientMatrixError>(())
        })();
        if let Err(error) = result {
            abort_checked_matrix(error);
        }
        value
    }

    fn add_checked(&self, left: &Coefficient, right: &Coefficient) -> Coefficient {
        self.charge_operation(AtomicOperation::Add);
        self.finish(self.context.try_add(left, right, self.limits.exact_algebra))
    }

    fn sub_checked(&self, left: &Coefficient, right: &Coefficient) -> Coefficient {
        self.charge_operation(AtomicOperation::Subtract);
        self.finish(self.context.try_sub(left, right, self.limits.exact_algebra))
    }

    fn mul_checked(&self, left: &Coefficient, right: &Coefficient) -> Coefficient {
        self.charge_operation(AtomicOperation::Multiply);
        self.finish(self.context.try_mul(left, right, self.limits.exact_algebra))
    }

    fn div_checked(&self, numerator: &Coefficient, denominator: &Coefficient) -> Coefficient {
        self.charge_operation(AtomicOperation::Divide);
        self.finish(
            self.context
                .try_div(numerator, denominator, self.limits.exact_algebra),
        )
    }

    fn neg_checked(&self, value: &Coefficient) -> Coefficient {
        self.charge_operation(AtomicOperation::Negate);
        self.finish(self.context.try_neg(value, self.limits.exact_algebra))
    }

    fn contextual_integer(&self, value: Integer) -> Coefficient {
        self.finish_raw(self.context.template().numerator.constant(value).into())
    }
}

impl Set for CheckedCoefficientField<'_> {
    type Element = Coefficient;

    fn size(&self) -> Option<Integer> {
        None
    }
}

impl RingOps<Coefficient> for CheckedCoefficientField<'_> {
    fn add(&self, left: Coefficient, right: Coefficient) -> Coefficient {
        self.charge_counter(|stats| &mut stats.non_matrix_trait_calls);
        self.add_checked(&left, &right)
    }

    fn sub(&self, left: Coefficient, right: Coefficient) -> Coefficient {
        self.charge_counter(|stats| &mut stats.non_matrix_trait_calls);
        self.sub_checked(&left, &right)
    }

    fn mul(&self, left: Coefficient, right: Coefficient) -> Coefficient {
        self.charge_counter(|stats| &mut stats.non_matrix_trait_calls);
        self.mul_checked(&left, &right)
    }

    fn neg(&self, value: Coefficient) -> Coefficient {
        self.charge_counter(|stats| &mut stats.non_matrix_trait_calls);
        self.neg_checked(&value)
    }

    fn add_assign(&self, left: &mut Coefficient, right: Coefficient) {
        self.charge_counter(|stats| &mut stats.non_matrix_trait_calls);
        *left = self.add_checked(left, &right);
    }

    fn sub_assign(&self, left: &mut Coefficient, right: Coefficient) {
        self.charge_counter(|stats| &mut stats.non_matrix_trait_calls);
        *left = self.sub_checked(left, &right);
    }

    fn mul_assign(&self, left: &mut Coefficient, right: Coefficient) {
        self.charge_counter(|stats| &mut stats.non_matrix_trait_calls);
        *left = self.mul_checked(left, &right);
    }

    fn add_mul_assign(&self, accumulator: &mut Coefficient, left: Coefficient, right: Coefficient) {
        self.charge_counter(|stats| &mut stats.non_matrix_trait_calls);
        let product = self.mul_checked(&left, &right);
        *accumulator = self.add_checked(accumulator, &product);
    }

    fn sub_mul_assign(&self, accumulator: &mut Coefficient, left: Coefficient, right: Coefficient) {
        self.charge_counter(|stats| &mut stats.non_matrix_trait_calls);
        let product = self.mul_checked(&left, &right);
        *accumulator = self.sub_checked(accumulator, &product);
    }
}

impl RingOps<&Coefficient> for CheckedCoefficientField<'_> {
    fn add(&self, left: &Coefficient, right: &Coefficient) -> Coefficient {
        self.add_checked(left, right)
    }

    fn sub(&self, left: &Coefficient, right: &Coefficient) -> Coefficient {
        self.sub_checked(left, right)
    }

    fn mul(&self, left: &Coefficient, right: &Coefficient) -> Coefficient {
        self.mul_checked(left, right)
    }

    fn neg(&self, value: &Coefficient) -> Coefficient {
        self.neg_checked(value)
    }

    fn add_assign(&self, left: &mut Coefficient, right: &Coefficient) {
        *left = self.add_checked(left, right);
    }

    fn sub_assign(&self, left: &mut Coefficient, right: &Coefficient) {
        *left = self.sub_checked(left, right);
    }

    fn mul_assign(&self, left: &mut Coefficient, right: &Coefficient) {
        *left = self.mul_checked(left, right);
    }

    fn add_mul_assign(
        &self,
        accumulator: &mut Coefficient,
        left: &Coefficient,
        right: &Coefficient,
    ) {
        let product = self.mul_checked(left, right);
        *accumulator = self.add_checked(accumulator, &product);
    }

    fn sub_mul_assign(
        &self,
        accumulator: &mut Coefficient,
        left: &Coefficient,
        right: &Coefficient,
    ) {
        let product = self.mul_checked(left, right);
        *accumulator = self.sub_checked(accumulator, &product);
    }
}

impl Ring for CheckedCoefficientField<'_> {
    fn zero(&self) -> Coefficient {
        self.charge_counter(|stats| &mut stats.zero_constants);
        self.context.zero()
    }

    fn one(&self) -> Coefficient {
        self.charge_counter(|stats| &mut stats.one_constants);
        self.context.one()
    }

    fn nth(&self, value: Integer) -> Coefficient {
        self.charge_counter(|stats| &mut stats.non_matrix_trait_calls);
        self.contextual_integer(value)
    }

    fn pow(&self, base: &Coefficient, exponent: u64) -> Coefficient {
        self.charge_counter(|stats| &mut stats.power_calls);
        self.charge_counter(|stats| &mut stats.non_matrix_trait_calls);
        let admission = self.preflight_power_admission(base, exponent);
        self.charge_power_operations(exponent, admission);
        self.finish_power_raw(self.inner.pow(base, exponent), admission)
    }

    fn is_zero(&self, value: &Coefficient) -> bool {
        self.charge_counter(|stats| &mut stats.zero_tests);
        value.is_zero()
    }

    fn is_one(&self, value: &Coefficient) -> bool {
        self.charge_counter(|stats| &mut stats.one_tests);
        value.is_one()
    }

    fn one_is_gcd_unit() -> bool {
        <RationalPolynomialField<IntegerRing, u16> as Ring>::one_is_gcd_unit()
    }

    fn characteristic(&self) -> Integer {
        self.inner.characteristic()
    }

    fn try_inv(&self, value: &Coefficient) -> Option<Coefficient> {
        self.charge_counter(|stats| &mut stats.non_matrix_trait_calls);
        if value.is_zero() {
            None
        } else {
            Some(self.div_checked(&self.context.one(), value))
        }
    }

    fn try_div(&self, numerator: &Coefficient, denominator: &Coefficient) -> Option<Coefficient> {
        if denominator.is_zero() {
            None
        } else {
            Some(self.div_checked(numerator, denominator))
        }
    }

    fn sample(&self, rng: &mut impl RngCore, range: (i64, i64)) -> Coefficient {
        self.charge_counter(|stats| &mut stats.non_matrix_trait_calls);
        self.contextual_integer(Z.sample(rng, range))
    }

    fn format<W: fmt::Write>(
        &self,
        element: &Coefficient,
        options: &PrintOptions,
        state: PrintState,
        formatter: &mut W,
    ) -> Result<bool, fmt::Error> {
        self.inner.format(element, options, state, formatter)
    }

    fn has_independent_elements(&self) -> bool {
        true
    }
}

impl EuclideanDomain for CheckedCoefficientField<'_> {
    fn rem(&self, left: &Coefficient, right: &Coefficient) -> Coefficient {
        self.charge_counter(|stats| &mut stats.non_matrix_trait_calls);
        self.finish_raw(self.inner.rem(left, right))
    }

    fn quot_rem(
        &self,
        numerator: &Coefficient,
        denominator: &Coefficient,
    ) -> (Coefficient, Coefficient) {
        self.charge_counter(|stats| &mut stats.non_matrix_trait_calls);
        (
            self.div_checked(numerator, denominator),
            self.context.zero(),
        )
    }

    fn gcd(&self, left: &Coefficient, right: &Coefficient) -> Coefficient {
        self.charge_counter(|stats| &mut stats.non_matrix_trait_calls);
        self.finish_raw(self.inner.gcd(left, right))
    }
}

impl Field for CheckedCoefficientField<'_> {
    fn div(&self, numerator: &Coefficient, denominator: &Coefficient) -> Coefficient {
        self.charge_counter(|stats| &mut stats.non_matrix_trait_calls);
        self.div_checked(numerator, denominator)
    }

    fn div_assign(&self, numerator: &mut Coefficient, denominator: &Coefficient) {
        self.charge_counter(|stats| &mut stats.non_matrix_trait_calls);
        *numerator = self.div_checked(numerator, denominator);
    }

    fn inv(&self, value: &Coefficient) -> Coefficient {
        self.div_checked(&self.context.one(), value)
    }
}

fn map_native_error<F: Ring>(
    operation: &'static str,
    error: MatrixError<F>,
) -> SymbolicaCoefficientMatrixError {
    let kind = match error {
        MatrixError::Underdetermined { .. } => SymbolicaNativeMatrixErrorKind::Underdetermined,
        MatrixError::Inconsistent => SymbolicaNativeMatrixErrorKind::Inconsistent,
        MatrixError::NotSquare => SymbolicaNativeMatrixErrorKind::NotSquare,
        MatrixError::Singular => SymbolicaNativeMatrixErrorKind::Singular,
        MatrixError::ShapeMismatch => SymbolicaNativeMatrixErrorKind::ShapeMismatch,
        MatrixError::RightHandSideIsNotVector => {
            SymbolicaNativeMatrixErrorKind::RightHandSideIsNotVector
        }
        MatrixError::ResultNotInDomain => SymbolicaNativeMatrixErrorKind::ResultNotInDomain,
    };
    SymbolicaCoefficientMatrixError::NativeError { operation, kind }
}

pub(super) fn call_native<T>(
    operation: &'static str,
    callback: impl FnOnce() -> T,
) -> Result<T, SymbolicaCoefficientMatrixError> {
    match catch_unwind(AssertUnwindSafe(callback)) {
        Ok(value) => Ok(value),
        Err(payload) => match payload.downcast::<CheckedFieldAbort>() {
            Ok(abort) => Err(abort.0),
            Err(_) => Err(SymbolicaCoefficientMatrixError::NativePanic { operation }),
        },
    }
}

pub(super) fn call_native_result<T, F: Ring>(
    operation: &'static str,
    callback: impl FnOnce() -> Result<T, MatrixError<F>>,
) -> Result<T, SymbolicaCoefficientMatrixError> {
    call_native(operation, callback)?.map_err(|error| map_native_error(operation, error))
}
