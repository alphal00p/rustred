//! Authenticated, resource-bounded access to Symbolica's exact matrix algebra.
//!
//! This module is deliberately provenance-neutral.  Symbolica owns rank,
//! determinant, inversion, and multiplication; RustRed supplies only the
//! authenticated coefficient domain, admission policy, typed failure transport,
//! and replay checks needed by proof-bearing callers.
//!
//! Input and every retained native output are censused by exact clone-owned
//! capacity.  Symbolica's public scalar API does not expose a complete bound on
//! polynomial GCD, quotient, or dense-multiplication scratch, so that remaining
//! native scratch gap is explicit rather than being disguised as a byte proof.
//! Typed scalar failures cross Symbolica's infallible field traits through a
//! private unwind payload.  This boundary therefore requires Rust's
//! `panic = "unwind"`; `panic = "abort"` builds cannot recover a typed failure.

#[cfg(not(panic = "unwind"))]
compile_error!(
    "RustRed's authenticated Symbolica matrix boundary requires panic=\"unwind\" for typed failure transport"
);

use std::cell::RefCell;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::rc::Rc;

use rand::RngCore;
use symbolica::domains::SelfRing;
use symbolica::prelude::*;
use symbolica::tensors::matrix::MatrixError;

use crate::coefficient::{
    Coefficient, CoefficientContext, ExactAlgebraError, ExactAlgebraLimits,
    coefficient_clone_owned_retained_byte_bound,
};

const DEFAULT_MAX_SINGLE_MATRIX_ENTRIES: usize = 16_000_000;
const DEFAULT_MAX_LIVE_MATRIX_ENTRIES: usize = 32_000_000;
pub(crate) const DEFAULT_MAX_EXACT_OPERATIONS: usize = 100_000_000;
pub(crate) const DEFAULT_MAX_INPUT_RETAINED_BYTES: usize = 1024 * 1024 * 1024;
pub(crate) const DEFAULT_MAX_OUTPUT_RETAINED_BYTES: usize = 1024 * 1024 * 1024;

/// Admission policy for one bounded Symbolica coefficient-matrix session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SymbolicaCoefficientMatrixLimits {
    pub(crate) exact_algebra: ExactAlgebraLimits,
    /// Largest individual native matrix payload.  General inversion needs the
    /// augmented `n x 2n` matrix here.
    pub(crate) max_single_matrix_entries: usize,
    /// Largest conservative simultaneously-live native payload.
    pub(crate) max_live_matrix_entries: usize,
    /// Largest number of checked exact arithmetic operations admitted for the
    /// complete requested matrix operation. Constant construction and
    /// zero/one predicates are censused separately.
    pub(crate) max_exact_operations: usize,
    /// Aggregate clone-owned retained bytes in authenticated caller inputs.
    pub(crate) max_input_retained_bytes: usize,
    /// Aggregate clone-owned retained bytes in determinant, inverse, and
    /// verification-product outputs inspected during the native session.
    pub(crate) max_output_retained_bytes: usize,
}

impl SymbolicaCoefficientMatrixLimits {
    /// Adapt the historical family limit, which bounds the `n x 2n` augmented
    /// matrix, to this module's individual and live-payload limits.
    pub(crate) const fn for_family(
        exact_algebra: ExactAlgebraLimits,
        max_augmented_entries: usize,
        max_exact_operations: usize,
        max_input_retained_bytes: usize,
        max_output_retained_bytes: usize,
    ) -> Self {
        Self {
            exact_algebra,
            max_single_matrix_entries: max_augmented_entries,
            max_live_matrix_entries: max_augmented_entries.saturating_mul(2),
            max_exact_operations,
            max_input_retained_bytes,
            max_output_retained_bytes,
        }
    }
}

impl Default for SymbolicaCoefficientMatrixLimits {
    fn default() -> Self {
        Self {
            exact_algebra: ExactAlgebraLimits::default(),
            max_single_matrix_entries: DEFAULT_MAX_SINGLE_MATRIX_ENTRIES,
            max_live_matrix_entries: DEFAULT_MAX_LIVE_MATRIX_ENTRIES,
            max_exact_operations: DEFAULT_MAX_EXACT_OPERATIONS,
            max_input_retained_bytes: DEFAULT_MAX_INPUT_RETAINED_BYTES,
            max_output_retained_bytes: DEFAULT_MAX_OUTPUT_RETAINED_BYTES,
        }
    }
}

/// Exact census of one admitted native matrix session.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct SymbolicaCoefficientMatrixStats {
    input_entries: usize,
    output_entries: usize,
    authenticated_entries: usize,
    admitted_single_matrix_entries: usize,
    admitted_peak_live_entries: usize,
    admitted_exact_operations: usize,
    input_retained_bytes: usize,
    output_retained_bytes: usize,
    exact_operations: usize,
    additions: usize,
    subtractions: usize,
    multiplications: usize,
    divisions: usize,
    negations: usize,
    zero_constants: usize,
    one_constants: usize,
    zero_tests: usize,
    one_tests: usize,
    determinant_calls: usize,
    inverse_calls: usize,
    product_calls: usize,
    rank_calls: usize,
    non_matrix_trait_calls: usize,
}

impl SymbolicaCoefficientMatrixStats {
    pub(crate) const fn input_entries(self) -> usize {
        self.input_entries
    }

    pub(crate) const fn output_entries(self) -> usize {
        self.output_entries
    }

    pub(crate) const fn authenticated_entries(self) -> usize {
        self.authenticated_entries
    }

    pub(crate) const fn admitted_single_matrix_entries(self) -> usize {
        self.admitted_single_matrix_entries
    }

    pub(crate) const fn admitted_peak_live_entries(self) -> usize {
        self.admitted_peak_live_entries
    }

    pub(crate) const fn admitted_exact_operations(self) -> usize {
        self.admitted_exact_operations
    }

    pub(crate) const fn exact_operations(self) -> usize {
        self.exact_operations
    }

    pub(crate) const fn input_retained_bytes(self) -> usize {
        self.input_retained_bytes
    }

    pub(crate) const fn output_retained_bytes(self) -> usize {
        self.output_retained_bytes
    }

    pub(crate) const fn additions(self) -> usize {
        self.additions
    }

    pub(crate) const fn subtractions(self) -> usize {
        self.subtractions
    }

    pub(crate) const fn multiplications(self) -> usize {
        self.multiplications
    }

    pub(crate) const fn divisions(self) -> usize {
        self.divisions
    }

    pub(crate) const fn negations(self) -> usize {
        self.negations
    }

    pub(crate) const fn determinant_calls(self) -> usize {
        self.determinant_calls
    }

    pub(crate) const fn inverse_calls(self) -> usize {
        self.inverse_calls
    }

    pub(crate) const fn product_calls(self) -> usize {
        self.product_calls
    }

    pub(crate) const fn rank_calls(self) -> usize {
        self.rank_calls
    }

    pub(crate) const fn non_matrix_trait_calls(self) -> usize {
        self.non_matrix_trait_calls
    }
}

/// Which certified inverse product failed to replay to the identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SymbolicaInverseSide {
    MatrixTimesInverse,
    InverseTimesMatrix,
}

impl fmt::Display for SymbolicaInverseSide {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MatrixTimesInverse => formatter.write_str("A A^-1"),
            Self::InverseTimesMatrix => formatter.write_str("A^-1 A"),
        }
    }
}

/// Bounded classification of native Matrix errors.  It intentionally carries
/// no Matrix payload and is never created by formatting `MatrixError`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SymbolicaNativeMatrixErrorKind {
    Underdetermined,
    Inconsistent,
    NotSquare,
    Singular,
    ShapeMismatch,
    RightHandSideIsNotVector,
    ResultNotInDomain,
}

/// Typed failures at the authenticated Symbolica matrix boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SymbolicaCoefficientMatrixError {
    EmptyMatrix,
    RaggedMatrix {
        row: usize,
        expected_columns: usize,
        actual_columns: usize,
    },
    NotSquare {
        rows: usize,
        columns: usize,
    },
    ShapeMismatch {
        left_rows: usize,
        left_columns: usize,
        right_rows: usize,
        right_columns: usize,
    },
    DimensionOverflow {
        rows: usize,
        columns: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    InvalidCoefficient {
        row: usize,
        column: usize,
        error: ExactAlgebraError,
    },
    ExactAlgebra(ExactAlgebraError),
    Singular,
    NativeError {
        operation: &'static str,
        kind: SymbolicaNativeMatrixErrorKind,
    },
    NativePanic {
        operation: &'static str,
    },
    InverseVerificationFailure {
        side: SymbolicaInverseSide,
        row: usize,
        column: usize,
    },
    InternalShapeFailure {
        operation: &'static str,
    },
}

impl fmt::Display for SymbolicaCoefficientMatrixError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyMatrix => formatter.write_str("a coefficient matrix cannot be empty"),
            Self::RaggedMatrix {
                row,
                expected_columns,
                actual_columns,
            } => write!(
                formatter,
                "coefficient matrix row {row} has {actual_columns} columns, expected {expected_columns}"
            ),
            Self::NotSquare { rows, columns } => {
                write!(
                    formatter,
                    "coefficient matrix is {rows}x{columns}, not square"
                )
            }
            Self::ShapeMismatch {
                left_rows,
                left_columns,
                right_rows,
                right_columns,
            } => write!(
                formatter,
                "coefficient matrix shapes {left_rows}x{left_columns} and {right_rows}x{right_columns} are incompatible"
            ),
            Self::DimensionOverflow { rows, columns } => write!(
                formatter,
                "coefficient matrix shape {rows}x{columns} exceeds Symbolica's native representation"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} needs {requested} units, exceeding the configured limit {limit}"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(formatter, "failed to reserve {requested} {resource}"),
            Self::InvalidCoefficient { row, column, error } => write!(
                formatter,
                "coefficient matrix entry ({row},{column}) is invalid: {error}"
            ),
            Self::ExactAlgebra(error) => error.fmt(formatter),
            Self::Singular => formatter.write_str("coefficient matrix is singular"),
            Self::NativeError { operation, kind } => {
                write!(
                    formatter,
                    "Symbolica matrix {operation} failed with {kind:?}"
                )
            }
            Self::NativePanic { operation } => {
                write!(
                    formatter,
                    "Symbolica panicked while computing matrix {operation}"
                )
            }
            Self::InverseVerificationFailure { side, row, column } => write!(
                formatter,
                "{side} differs from identity at ({row},{column})"
            ),
            Self::InternalShapeFailure { operation } => write!(
                formatter,
                "Symbolica returned an incompatible shape from matrix {operation}"
            ),
        }
    }
}

impl std::error::Error for SymbolicaCoefficientMatrixError {}

/// A determinant, inverse, and native two-sided replay certificate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct VerifiedSymbolicaCoefficientInverse {
    inverse: Vec<Vec<Coefficient>>,
    determinant: Coefficient,
    stats: SymbolicaCoefficientMatrixStats,
}

impl VerifiedSymbolicaCoefficientInverse {
    pub(crate) fn inverse(&self) -> &[Vec<Coefficient>] {
        &self.inverse
    }

    pub(crate) fn determinant(&self) -> &Coefficient {
        &self.determinant
    }

    pub(crate) const fn stats(&self) -> SymbolicaCoefficientMatrixStats {
        self.stats
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        Vec<Vec<Coefficient>>,
        Coefficient,
        SymbolicaCoefficientMatrixStats,
    ) {
        (self.inverse, self.determinant, self.stats)
    }
}

#[derive(Clone, Copy, Debug)]
enum AtomicOperation {
    Add,
    Subtract,
    Multiply,
    Divide,
    Negate,
}

/// Private unwind payload for fallible trait methods.  `resume_unwind` avoids
/// invoking the process-global panic hook; the nearest matrix boundary catches
/// and downcasts this exact type immediately.
struct CheckedFieldAbort(ExactAlgebraError);

#[cold]
fn abort_checked_field(error: ExactAlgebraError) -> ! {
    resume_unwind(Box::new(CheckedFieldAbort(error)))
}

#[derive(Debug, Default)]
struct CheckedFieldState {
    stats: SymbolicaCoefficientMatrixStats,
}

#[derive(Clone)]
struct CheckedCoefficientField<'context> {
    context: &'context CoefficientContext,
    inner: RationalPolynomialField<IntegerRing, u16>,
    limits: SymbolicaCoefficientMatrixLimits,
    state: Rc<RefCell<CheckedFieldState>>,
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
    fn new(
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

    fn stats(&self) -> SymbolicaCoefficientMatrixStats {
        self.state.borrow().stats
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
        self.charge_counter(|stats| &mut stats.non_matrix_trait_calls);
        self.finish_raw(self.inner.pow(base, exponent))
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

#[derive(Clone, Copy, Debug)]
struct MatrixShape {
    rows: usize,
    columns: usize,
    rows_u32: u32,
    columns_u32: u32,
    entries: usize,
}

fn inspect_rows(rows: &[Vec<Coefficient>]) -> Result<MatrixShape, SymbolicaCoefficientMatrixError> {
    if rows.is_empty() {
        return Err(SymbolicaCoefficientMatrixError::EmptyMatrix);
    }
    let columns = rows[0].len();
    if columns == 0 {
        return Err(SymbolicaCoefficientMatrixError::EmptyMatrix);
    }
    if let Some((row, actual_columns)) = rows
        .iter()
        .enumerate()
        .find_map(|(row, values)| (values.len() != columns).then_some((row, values.len())))
    {
        return Err(SymbolicaCoefficientMatrixError::RaggedMatrix {
            row,
            expected_columns: columns,
            actual_columns,
        });
    }
    checked_shape(rows.len(), columns)
}

fn checked_shape(
    rows: usize,
    columns: usize,
) -> Result<MatrixShape, SymbolicaCoefficientMatrixError> {
    let rows_u32 = u32::try_from(rows)
        .map_err(|_| SymbolicaCoefficientMatrixError::DimensionOverflow { rows, columns })?;
    let columns_u32 = u32::try_from(columns)
        .map_err(|_| SymbolicaCoefficientMatrixError::DimensionOverflow { rows, columns })?;
    let entries = rows.checked_mul(columns).ok_or(
        SymbolicaCoefficientMatrixError::ResourceCountOverflow {
            resource: "coefficient matrix entries",
        },
    )?;
    rows_u32
        .checked_mul(columns_u32)
        .ok_or(SymbolicaCoefficientMatrixError::DimensionOverflow { rows, columns })?;
    Ok(MatrixShape {
        rows,
        columns,
        rows_u32,
        columns_u32,
        entries,
    })
}

fn require_square(shape: MatrixShape) -> Result<usize, SymbolicaCoefficientMatrixError> {
    if shape.rows == shape.columns {
        Ok(shape.rows)
    } else {
        Err(SymbolicaCoefficientMatrixError::NotSquare {
            rows: shape.rows,
            columns: shape.columns,
        })
    }
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, SymbolicaCoefficientMatrixError> {
    left.checked_add(right)
        .ok_or(SymbolicaCoefficientMatrixError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, SymbolicaCoefficientMatrixError> {
    left.checked_mul(right)
        .ok_or(SymbolicaCoefficientMatrixError::ResourceCountOverflow { resource })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), SymbolicaCoefficientMatrixError> {
    if requested > limit {
        Err(SymbolicaCoefficientMatrixError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn increment_session_counter(
    state: &Rc<RefCell<CheckedFieldState>>,
    resource: &'static str,
    select: impl FnOnce(&mut SymbolicaCoefficientMatrixStats) -> &mut usize,
) -> Result<(), SymbolicaCoefficientMatrixError> {
    let mut state = state.borrow_mut();
    let counter = select(&mut state.stats);
    *counter = counter
        .checked_add(1)
        .ok_or(SymbolicaCoefficientMatrixError::ResourceCountOverflow { resource })?;
    Ok(())
}

fn square_sum(bound: usize) -> Result<usize, SymbolicaCoefficientMatrixError> {
    let a = bound;
    let b = bound
        .checked_add(1)
        .ok_or(SymbolicaCoefficientMatrixError::ResourceCountOverflow {
            resource: "Symbolica determinant operation bound",
        })?;
    let c = bound
        .checked_mul(2)
        .and_then(|value| value.checked_add(1))
        .ok_or(SymbolicaCoefficientMatrixError::ResourceCountOverflow {
            resource: "Symbolica determinant operation bound",
        })?;
    // Cancel 2 and 3 before multiplying to avoid rejecting representable sums.
    let mut factors = [a, b, c];
    let even = factors.iter().position(|value| value % 2 == 0).unwrap_or(0);
    factors[even] /= 2;
    let by_three = factors.iter().position(|value| value % 3 == 0).unwrap_or(0);
    factors[by_three] /= 3;
    checked_mul(
        "Symbolica determinant operation bound",
        checked_mul(
            "Symbolica determinant operation bound",
            factors[0],
            factors[1],
        )?,
        factors[2],
    )
}

fn determinant_operation_bound(size: usize) -> Result<usize, SymbolicaCoefficientMatrixError> {
    match size {
        0 => Ok(0),
        1 => Ok(0),
        2 => Ok(3),
        3 => Ok(14),
        _ => {
            let cells = square_sum(size - 1)?;
            let divisions = square_sum(size - 2)?;
            checked_add(
                "Symbolica determinant operation bound",
                checked_add(
                    "Symbolica determinant operation bound",
                    checked_mul("Symbolica determinant operation bound", 3, cells)?,
                    divisions,
                )?,
                1,
            )
        }
    }
}

fn inverse_operation_bound(size: usize) -> Result<usize, SymbolicaCoefficientMatrixError> {
    match size {
        0 => Ok(0),
        1 => Ok(4),
        2 => Ok(10),
        3 => Ok(42),
        _ => {
            let cube = checked_mul(
                "Symbolica inverse operation bound",
                checked_mul("Symbolica inverse operation bound", size, size)?,
                size,
            )?;
            let square = checked_mul("Symbolica inverse operation bound", size, size)?;
            let positive = checked_add(
                "Symbolica inverse operation bound",
                checked_mul("Symbolica inverse operation bound", 3, cube)?,
                checked_mul("Symbolica inverse operation bound", 3, size)?,
            )?;
            positive
                .checked_sub(checked_mul("Symbolica inverse operation bound", 2, square)?)
                .ok_or(SymbolicaCoefficientMatrixError::ResourceCountOverflow {
                    resource: "Symbolica inverse operation bound",
                })
        }
    }
}

fn product_operation_bound(
    rows: usize,
    inner: usize,
    columns: usize,
) -> Result<usize, SymbolicaCoefficientMatrixError> {
    checked_mul(
        "Symbolica matrix product operation bound",
        2,
        checked_mul(
            "Symbolica matrix product operation bound",
            checked_mul("Symbolica matrix product operation bound", rows, inner)?,
            columns,
        )?,
    )
}

fn square_representation_bounds(
    size: usize,
) -> Result<(usize, usize, usize), SymbolicaCoefficientMatrixError> {
    let shape = checked_shape(size, size)?;
    let doubled =
        size.checked_mul(2)
            .ok_or(SymbolicaCoefficientMatrixError::DimensionOverflow {
                rows: size,
                columns: size,
            })?;
    let doubled_u32 =
        u32::try_from(doubled).map_err(|_| SymbolicaCoefficientMatrixError::DimensionOverflow {
            rows: size,
            columns: doubled,
        })?;
    shape.rows_u32.checked_mul(doubled_u32).ok_or(
        SymbolicaCoefficientMatrixError::DimensionOverflow {
            rows: size,
            columns: doubled,
        },
    )?;
    let augmented = checked_mul("augmented Symbolica matrix entries", shape.entries, 2)?;
    let peak_live = checked_mul("live Symbolica matrix entries", shape.entries, 4)?;
    Ok((shape.entries, augmented, peak_live))
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

fn call_native<T>(
    operation: &'static str,
    callback: impl FnOnce() -> T,
) -> Result<T, SymbolicaCoefficientMatrixError> {
    match catch_unwind(AssertUnwindSafe(callback)) {
        Ok(value) => Ok(value),
        Err(payload) => match payload.downcast::<CheckedFieldAbort>() {
            Ok(abort) => Err(SymbolicaCoefficientMatrixError::ExactAlgebra(abort.0)),
            Err(_) => Err(SymbolicaCoefficientMatrixError::NativePanic { operation }),
        },
    }
}

fn call_native_result<T, F: Ring>(
    operation: &'static str,
    callback: impl FnOnce() -> Result<T, MatrixError<F>>,
) -> Result<T, SymbolicaCoefficientMatrixError> {
    call_native(operation, callback)?.map_err(|error| map_native_error(operation, error))
}

fn validate_rows(
    context: &CoefficientContext,
    rows: &[Vec<Coefficient>],
    limits: ExactAlgebraLimits,
) -> Result<(), SymbolicaCoefficientMatrixError> {
    for (row, values) in rows.iter().enumerate() {
        for (column, coefficient) in values.iter().enumerate() {
            context
                .validate_with_limits(coefficient, limits)
                .map_err(
                    |error| SymbolicaCoefficientMatrixError::InvalidCoefficient {
                        row,
                        column,
                        error,
                    },
                )?;
        }
    }
    Ok(())
}

fn coefficient_retained_bytes(
    coefficient: &Coefficient,
) -> Result<usize, SymbolicaCoefficientMatrixError> {
    coefficient_clone_owned_retained_byte_bound(coefficient).ok_or(
        SymbolicaCoefficientMatrixError::ResourceCountOverflow {
            resource: "coefficient matrix retained bytes",
        },
    )
}

fn rows_retained_bytes(
    rows: &[Vec<Coefficient>],
) -> Result<usize, SymbolicaCoefficientMatrixError> {
    let mut bytes = 0usize;
    for coefficient in rows.iter().flatten() {
        bytes = checked_add(
            "coefficient matrix input retained bytes",
            bytes,
            coefficient_retained_bytes(coefficient)?,
        )?;
    }
    Ok(bytes)
}

fn authenticate_output_coefficient(
    context: &CoefficientContext,
    coefficient: &Coefficient,
    limits: SymbolicaCoefficientMatrixLimits,
    state: &Rc<RefCell<CheckedFieldState>>,
) -> Result<(), SymbolicaCoefficientMatrixError> {
    context
        .validate_with_limits(coefficient, limits.exact_algebra)
        .map_err(SymbolicaCoefficientMatrixError::ExactAlgebra)?;
    let bytes = coefficient_retained_bytes(coefficient)?;
    let mut state = state.borrow_mut();
    let prospective = checked_add(
        "coefficient matrix output retained bytes",
        state.stats.output_retained_bytes,
        bytes,
    )?;
    check_limit(
        "coefficient matrix output retained bytes",
        prospective,
        limits.max_output_retained_bytes,
    )?;
    state.stats.output_retained_bytes = prospective;
    state.stats.authenticated_entries = checked_add(
        "authenticated Symbolica matrix entries",
        state.stats.authenticated_entries,
        1,
    )?;
    Ok(())
}

fn matrix_from_rows<'context>(
    rows: &[Vec<Coefficient>],
    shape: MatrixShape,
    field: CheckedCoefficientField<'context>,
) -> Result<Matrix<CheckedCoefficientField<'context>>, SymbolicaCoefficientMatrixError> {
    validate_rows(field.context, rows, field.limits.exact_algebra)?;
    let retained_bytes = rows_retained_bytes(rows)?;
    {
        let mut state = field.state.borrow_mut();
        let prospective_bytes = checked_add(
            "coefficient matrix input retained bytes",
            state.stats.input_retained_bytes,
            retained_bytes,
        )?;
        check_limit(
            "coefficient matrix input retained bytes",
            prospective_bytes,
            field.limits.max_input_retained_bytes,
        )?;
        state.stats.input_retained_bytes = prospective_bytes;
        state.stats.input_entries = checked_add(
            "coefficient matrix input entries",
            state.stats.input_entries,
            shape.entries,
        )?;
    }
    let mut data = Vec::new();
    data.try_reserve_exact(shape.entries).map_err(|_| {
        SymbolicaCoefficientMatrixError::AllocationFailure {
            resource: "coefficient matrix entries",
            requested: shape.entries,
        }
    })?;
    for row in rows {
        data.extend(row.iter().cloned());
    }
    call_native("construction", || {
        Matrix::from_linear(data, shape.rows_u32, shape.columns_u32, field)
    })?
    .map_err(|_| SymbolicaCoefficientMatrixError::InternalShapeFailure {
        operation: "construction",
    })
}

fn authenticate_native<F>(
    context: &CoefficientContext,
    matrix: &Matrix<F>,
    limits: SymbolicaCoefficientMatrixLimits,
    state: &Rc<RefCell<CheckedFieldState>>,
) -> Result<(), SymbolicaCoefficientMatrixError>
where
    F: Ring<Element = Coefficient>,
{
    let mut retained_bytes = 0usize;
    for (offset, coefficient) in matrix.iter().enumerate() {
        let columns = matrix.ncols();
        context
            .validate_with_limits(coefficient, limits.exact_algebra)
            .map_err(
                |error| SymbolicaCoefficientMatrixError::InvalidCoefficient {
                    row: offset / columns,
                    column: offset % columns,
                    error,
                },
            )?;
        retained_bytes = checked_add(
            "coefficient matrix output retained bytes",
            retained_bytes,
            coefficient_retained_bytes(coefficient)?,
        )?;
    }
    let count = matrix.nrows().checked_mul(matrix.ncols()).ok_or(
        SymbolicaCoefficientMatrixError::ResourceCountOverflow {
            resource: "authenticated Symbolica matrix entries",
        },
    )?;
    let mut state = state.borrow_mut();
    let prospective_bytes = checked_add(
        "coefficient matrix output retained bytes",
        state.stats.output_retained_bytes,
        retained_bytes,
    )?;
    check_limit(
        "coefficient matrix output retained bytes",
        prospective_bytes,
        limits.max_output_retained_bytes,
    )?;
    state.stats.output_retained_bytes = prospective_bytes;
    state.stats.authenticated_entries = checked_add(
        "authenticated Symbolica matrix entries",
        state.stats.authenticated_entries,
        count,
    )?;
    Ok(())
}

fn native_into_rows<F>(
    matrix: Matrix<F>,
    state: &Rc<RefCell<CheckedFieldState>>,
) -> Result<Vec<Vec<Coefficient>>, SymbolicaCoefficientMatrixError>
where
    F: Ring<Element = Coefficient>,
{
    let rows = matrix.nrows();
    let columns = matrix.ncols();
    let entries = rows.checked_mul(columns).ok_or(
        SymbolicaCoefficientMatrixError::ResourceCountOverflow {
            resource: "coefficient matrix output entries",
        },
    )?;
    let mut output = Vec::new();
    output.try_reserve_exact(rows).map_err(|_| {
        SymbolicaCoefficientMatrixError::AllocationFailure {
            resource: "coefficient matrix output rows",
            requested: rows,
        }
    })?;
    let mut data = matrix.into_vec().into_iter();
    for _ in 0..rows {
        let mut row = Vec::new();
        row.try_reserve_exact(columns).map_err(|_| {
            SymbolicaCoefficientMatrixError::AllocationFailure {
                resource: "coefficient matrix output entries",
                requested: columns,
            }
        })?;
        row.extend(data.by_ref().take(columns));
        output.push(row);
    }
    if data.next().is_some() || output.iter().map(Vec::len).sum::<usize>() != entries {
        return Err(SymbolicaCoefficientMatrixError::InternalShapeFailure {
            operation: "output conversion",
        });
    }
    let mut state = state.borrow_mut();
    state.stats.output_entries = checked_add(
        "coefficient matrix output entries",
        state.stats.output_entries,
        entries,
    )?;
    Ok(output)
}

fn verify_identity_product(
    context: &CoefficientContext,
    product: &Matrix<CheckedCoefficientField<'_>>,
    size: usize,
    side: SymbolicaInverseSide,
    limits: SymbolicaCoefficientMatrixLimits,
    state: &Rc<RefCell<CheckedFieldState>>,
) -> Result<(), SymbolicaCoefficientMatrixError> {
    if product.nrows() != size || product.ncols() != size {
        return Err(SymbolicaCoefficientMatrixError::InternalShapeFailure {
            operation: "inverse verification",
        });
    }
    authenticate_native(context, product, limits, state)?;
    for row in 0..size {
        for column in 0..size {
            let coefficient = &product[(row as u32, column as u32)];
            let valid = if row == column {
                coefficient.is_one()
            } else {
                coefficient.is_zero()
            };
            if !valid {
                return Err(
                    SymbolicaCoefficientMatrixError::InverseVerificationFailure {
                        side,
                        row,
                        column,
                    },
                );
            }
        }
    }
    Ok(())
}

/// Compute the exact rank of a nonempty rectangular coefficient matrix through
/// Symbolica's destructive field row reduction.
///
/// Calling `Matrix::partial_row_reduce` on the owned native matrix avoids the
/// additional full clone performed by `Matrix::rank`.  RustRed does not select
/// pivots or perform elimination here: it only authenticates the input and
/// discarded echelon output, enforces the data-dependent exact-arithmetic cap,
/// and transports typed failures across Symbolica's infallible field traits.
pub(crate) fn rank_of_coefficient_matrix(
    context: &CoefficientContext,
    rows: &[Vec<Coefficient>],
    limits: SymbolicaCoefficientMatrixLimits,
) -> Result<(usize, SymbolicaCoefficientMatrixStats), SymbolicaCoefficientMatrixError> {
    let shape = inspect_rows(rows)?;
    check_limit(
        "single Symbolica matrix entries",
        shape.entries,
        limits.max_single_matrix_entries,
    )?;
    // The only native matrix is destructively reduced in place.  The borrowed
    // RustRed rows remain caller-owned and are charged independently as input.
    check_limit(
        "live Symbolica matrix entries",
        shape.entries,
        limits.max_live_matrix_entries,
    )?;

    let field = CheckedCoefficientField::new(
        context,
        limits,
        shape.entries,
        shape.entries,
        limits.max_exact_operations,
    );
    let state = field.state.clone();
    let max_column = shape.columns_u32;
    let mut matrix = matrix_from_rows(rows, shape, field)?;

    increment_session_counter(&state, "Symbolica rank calls", |stats| {
        &mut stats.rank_calls
    })?;
    let rank = call_native("rank", || matrix.partial_row_reduce(max_column))? as usize;
    if rank > shape.rows.min(shape.columns) {
        return Err(SymbolicaCoefficientMatrixError::InternalShapeFailure { operation: "rank" });
    }
    authenticate_native(context, &matrix, limits, &state)?;
    let stats = state.borrow().stats;
    Ok((rank, stats))
}

/// Compute a determinant with Symbolica after authenticating the full matrix.
pub(crate) fn determinant_of_coefficient_matrix(
    context: &CoefficientContext,
    rows: &[Vec<Coefficient>],
    limits: SymbolicaCoefficientMatrixLimits,
) -> Result<(Coefficient, SymbolicaCoefficientMatrixStats), SymbolicaCoefficientMatrixError> {
    let shape = inspect_rows(rows)?;
    let size = require_square(shape)?;
    let operations = determinant_operation_bound(size)?;
    check_limit(
        "Symbolica coefficient matrix exact operations",
        operations,
        limits.max_exact_operations,
    )?;
    let determinant_live = checked_mul("live Symbolica matrix entries", shape.entries, 2)?;
    check_limit(
        "single Symbolica matrix entries",
        shape.entries,
        limits.max_single_matrix_entries,
    )?;
    check_limit(
        "live Symbolica matrix entries",
        determinant_live,
        limits.max_live_matrix_entries,
    )?;
    let field =
        CheckedCoefficientField::new(context, limits, shape.entries, determinant_live, operations);
    let state = field.state.clone();
    let matrix = matrix_from_rows(rows, shape, field)?;
    increment_session_counter(&state, "Symbolica determinant calls", |stats| {
        &mut stats.determinant_calls
    })?;
    let determinant = call_native_result("determinant", || matrix.det())?;
    authenticate_output_coefficient(context, &determinant, limits, &state)?;
    let stats = state.borrow().stats;
    Ok((determinant, stats))
}

/// Compute and certify an exact inverse using only Symbolica matrix algebra.
pub(crate) fn invert_and_verify_coefficient_matrix(
    context: &CoefficientContext,
    rows: &[Vec<Coefficient>],
    limits: SymbolicaCoefficientMatrixLimits,
) -> Result<VerifiedSymbolicaCoefficientInverse, SymbolicaCoefficientMatrixError> {
    let shape = inspect_rows(rows)?;
    let size = require_square(shape)?;
    let (entries, augmented_entries, peak_live_entries) = square_representation_bounds(size)?;
    check_limit(
        "single Symbolica matrix entries",
        augmented_entries,
        limits.max_single_matrix_entries,
    )?;
    check_limit(
        "live Symbolica matrix entries",
        peak_live_entries,
        limits.max_live_matrix_entries,
    )?;
    let product = product_operation_bound(size, size, size)?;
    let operations = checked_add(
        "Symbolica coefficient matrix exact operations",
        checked_add(
            "Symbolica coefficient matrix exact operations",
            determinant_operation_bound(size)?,
            inverse_operation_bound(size)?,
        )?,
        checked_mul("Symbolica coefficient matrix exact operations", 2, product)?,
    )?;
    check_limit(
        "Symbolica coefficient matrix exact operations",
        operations,
        limits.max_exact_operations,
    )?;

    let field = CheckedCoefficientField::new(
        context,
        limits,
        augmented_entries,
        peak_live_entries,
        operations,
    );
    let state = field.state.clone();
    let matrix = matrix_from_rows(rows, shape, field)?;

    increment_session_counter(&state, "Symbolica determinant calls", |stats| {
        &mut stats.determinant_calls
    })?;
    let determinant = call_native_result("inverse determinant guard", || matrix.det())?;
    authenticate_output_coefficient(context, &determinant, limits, &state)?;
    if determinant.is_zero() {
        return Err(SymbolicaCoefficientMatrixError::Singular);
    }

    increment_session_counter(&state, "Symbolica inverse calls", |stats| {
        &mut stats.inverse_calls
    })?;
    let inverse = match call_native_result("inverse", || matrix.inv()) {
        Err(SymbolicaCoefficientMatrixError::NativeError {
            kind: SymbolicaNativeMatrixErrorKind::Singular,
            ..
        }) => {
            return Err(SymbolicaCoefficientMatrixError::InternalShapeFailure {
                operation: "inverse after nonzero determinant",
            });
        }
        result => result?,
    };
    if inverse.nrows() != size || inverse.ncols() != size {
        return Err(SymbolicaCoefficientMatrixError::InternalShapeFailure {
            operation: "inverse",
        });
    }
    authenticate_native(context, &inverse, limits, &state)?;

    increment_session_counter(&state, "Symbolica matrix product calls", |stats| {
        &mut stats.product_calls
    })?;
    let left = call_native("left inverse product", || &matrix * &inverse)?;
    verify_identity_product(
        context,
        &left,
        size,
        SymbolicaInverseSide::MatrixTimesInverse,
        limits,
        &state,
    )?;
    drop(left);
    increment_session_counter(&state, "Symbolica matrix product calls", |stats| {
        &mut stats.product_calls
    })?;
    let right = call_native("right inverse product", || &inverse * &matrix)?;
    verify_identity_product(
        context,
        &right,
        size,
        SymbolicaInverseSide::InverseTimesMatrix,
        limits,
        &state,
    )?;
    drop(right);

    let inverse = native_into_rows(inverse, &state)?;
    let stats = state.borrow().stats;
    debug_assert_eq!(entries, inverse.iter().map(Vec::len).sum::<usize>());
    Ok(VerifiedSymbolicaCoefficientInverse {
        inverse,
        determinant,
        stats,
    })
}

/// Verify both inverse products through Symbolica for caller-retained matrices.
pub(crate) fn verify_coefficient_matrix_inverse(
    context: &CoefficientContext,
    rows: &[Vec<Coefficient>],
    inverse: &[Vec<Coefficient>],
    limits: SymbolicaCoefficientMatrixLimits,
) -> Result<SymbolicaCoefficientMatrixStats, SymbolicaCoefficientMatrixError> {
    let shape = inspect_rows(rows)?;
    let inverse_shape = inspect_rows(inverse)?;
    let size = require_square(shape)?;
    if inverse_shape.rows != size || inverse_shape.columns != size {
        return Err(SymbolicaCoefficientMatrixError::ShapeMismatch {
            left_rows: shape.rows,
            left_columns: shape.columns,
            right_rows: inverse_shape.rows,
            right_columns: inverse_shape.columns,
        });
    }
    let product_entries = shape.entries;
    let live_entries = checked_mul("live Symbolica matrix entries", product_entries, 3)?;
    check_limit(
        "single Symbolica matrix entries",
        product_entries,
        limits.max_single_matrix_entries,
    )?;
    check_limit(
        "live Symbolica matrix entries",
        live_entries,
        limits.max_live_matrix_entries,
    )?;
    let operations = checked_mul(
        "Symbolica coefficient matrix exact operations",
        2,
        product_operation_bound(size, size, size)?,
    )?;
    check_limit(
        "Symbolica coefficient matrix exact operations",
        operations,
        limits.max_exact_operations,
    )?;
    let field =
        CheckedCoefficientField::new(context, limits, product_entries, live_entries, operations);
    let state = field.state.clone();
    let matrix = matrix_from_rows(rows, shape, field.clone())?;
    let inverse = matrix_from_rows(inverse, inverse_shape, field)?;

    increment_session_counter(&state, "Symbolica matrix product calls", |stats| {
        &mut stats.product_calls
    })?;
    let left = call_native("left inverse product", || &matrix * &inverse)?;
    verify_identity_product(
        context,
        &left,
        size,
        SymbolicaInverseSide::MatrixTimesInverse,
        limits,
        &state,
    )?;
    drop(left);
    increment_session_counter(&state, "Symbolica matrix product calls", |stats| {
        &mut stats.product_calls
    })?;
    let right = call_native("right inverse product", || &inverse * &matrix)?;
    verify_identity_product(
        context,
        &right,
        size,
        SymbolicaInverseSide::InverseTimesMatrix,
        limits,
        &state,
    )?;
    drop(right);
    let stats = state.borrow().stats;
    Ok(stats)
}

/// Multiply two authenticated matrices through Symbolica.
pub(crate) fn multiply_coefficient_matrices(
    context: &CoefficientContext,
    left: &[Vec<Coefficient>],
    right: &[Vec<Coefficient>],
    limits: SymbolicaCoefficientMatrixLimits,
) -> Result<(Vec<Vec<Coefficient>>, SymbolicaCoefficientMatrixStats), SymbolicaCoefficientMatrixError>
{
    let left_shape = inspect_rows(left)?;
    let right_shape = inspect_rows(right)?;
    if left_shape.columns != right_shape.rows {
        return Err(SymbolicaCoefficientMatrixError::ShapeMismatch {
            left_rows: left_shape.rows,
            left_columns: left_shape.columns,
            right_rows: right_shape.rows,
            right_columns: right_shape.columns,
        });
    }
    let output_shape = checked_shape(left_shape.rows, right_shape.columns)?;
    let single_entries = left_shape
        .entries
        .max(right_shape.entries)
        .max(output_shape.entries);
    let live_entries = checked_add(
        "live Symbolica matrix entries",
        checked_add(
            "live Symbolica matrix entries",
            left_shape.entries,
            right_shape.entries,
        )?,
        output_shape.entries,
    )?;
    check_limit(
        "single Symbolica matrix entries",
        single_entries,
        limits.max_single_matrix_entries,
    )?;
    check_limit(
        "live Symbolica matrix entries",
        live_entries,
        limits.max_live_matrix_entries,
    )?;
    let operations =
        product_operation_bound(left_shape.rows, left_shape.columns, right_shape.columns)?;
    check_limit(
        "Symbolica coefficient matrix exact operations",
        operations,
        limits.max_exact_operations,
    )?;
    let field =
        CheckedCoefficientField::new(context, limits, single_entries, live_entries, operations);
    let state = field.state.clone();
    let left = matrix_from_rows(left, left_shape, field.clone())?;
    let right = matrix_from_rows(right, right_shape, field)?;
    increment_session_counter(&state, "Symbolica matrix product calls", |stats| {
        &mut stats.product_calls
    })?;
    let product = call_native("product", || &left * &right)?;
    if product.nrows() != output_shape.rows || product.ncols() != output_shape.columns {
        return Err(SymbolicaCoefficientMatrixError::InternalShapeFailure {
            operation: "product",
        });
    }
    authenticate_native(context, &product, limits, &state)?;
    let product = native_into_rows(product, &state)?;
    let stats = state.borrow().stats;
    Ok((product, stats))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(context: &CoefficientContext, size: usize) -> Vec<Vec<Coefficient>> {
        (0..size)
            .map(|row| {
                (0..size)
                    .map(|column| {
                        if row == column {
                            context.one()
                        } else {
                            context.zero()
                        }
                    })
                    .collect()
            })
            .collect()
    }

    #[test]
    fn rectangular_symbolic_rational_rank_is_native_and_authenticated() {
        let context = CoefficientContext::new(["a", "b", "x"]);
        let matrix = vec![
            vec![
                context.zero(),
                context.parse("a/x").unwrap(),
                context.zero(),
                context.one(),
            ],
            vec![
                context.zero(),
                context.zero(),
                context.parameter("b").unwrap(),
                context.parse("1/x").unwrap(),
            ],
            vec![
                context.zero(),
                context.parse("2*a/x").unwrap(),
                context.zero(),
                context.integer(2),
            ],
        ];
        let (rank, stats) = rank_of_coefficient_matrix(
            &context,
            &matrix,
            SymbolicaCoefficientMatrixLimits::default(),
        )
        .unwrap();

        assert_eq!(rank, 2);
        assert_eq!(stats.rank_calls(), 1);
        assert_eq!(stats.exact_operations(), 7);
        assert_eq!(stats.input_entries(), 12);
        assert_eq!(stats.output_entries(), 0);
        assert_eq!(stats.authenticated_entries(), 12);
        assert!(stats.input_retained_bytes() > 0);
        assert!(stats.output_retained_bytes() > 0);
        assert_eq!(stats.non_matrix_trait_calls(), 0);
    }

    #[test]
    fn native_rank_handles_row_swaps_zero_leading_columns_and_deficiency() {
        let context = CoefficientContext::new(["x"]);
        let row_swap = vec![
            vec![context.zero(), context.zero(), context.one()],
            vec![
                context.parameter("x").unwrap(),
                context.zero(),
                context.zero(),
            ],
            vec![context.zero(), context.one(), context.zero()],
        ];
        let (rank, stats) = rank_of_coefficient_matrix(
            &context,
            &row_swap,
            SymbolicaCoefficientMatrixLimits::default(),
        )
        .unwrap();
        assert_eq!(rank, 3);
        assert_eq!(stats.exact_operations(), 3);

        let deficient = vec![
            vec![
                context.zero(),
                context.one(),
                context.integer(2),
                context.integer(3),
            ],
            vec![
                context.zero(),
                context.integer(2),
                context.integer(4),
                context.integer(6),
            ],
            vec![
                context.zero(),
                context.zero(),
                context.zero(),
                context.zero(),
            ],
        ];
        let (rank, _) = rank_of_coefficient_matrix(
            &context,
            &deficient,
            SymbolicaCoefficientMatrixLimits::default(),
        )
        .unwrap();
        assert_eq!(rank, 1);

        let zero = vec![vec![context.zero(); 4]; 2];
        let (rank, stats) = rank_of_coefficient_matrix(
            &context,
            &zero,
            SymbolicaCoefficientMatrixLimits {
                max_exact_operations: 0,
                ..SymbolicaCoefficientMatrixLimits::default()
            },
        )
        .unwrap();
        assert_eq!(rank, 0);
        assert_eq!(stats.exact_operations(), 0);
    }

    #[test]
    fn native_rank_covers_rectangular_shapes_one_through_six() {
        let context = CoefficientContext::new(["x"]);
        for rows in 1..=6 {
            for columns in 1..=6 {
                let expected = rows.min(columns);
                let mut matrix = vec![vec![context.zero(); columns]; rows];
                for (diagonal, row) in matrix.iter_mut().enumerate().take(expected) {
                    row[diagonal] = context.one();
                }
                let (rank, stats) = rank_of_coefficient_matrix(
                    &context,
                    &matrix,
                    SymbolicaCoefficientMatrixLimits::default(),
                )
                .unwrap();
                assert_eq!(rank, expected, "shape {rows}x{columns}");
                assert_eq!(stats.exact_operations(), expected);
                assert_eq!(stats.rank_calls(), 1);
            }
        }
    }

    #[test]
    fn native_rank_preserves_gmp_coefficients_and_rejects_foreign_maps() {
        let context = CoefficientContext::new(["x"]);
        let large = context
            .parse("340282366920938463463374607431768211507")
            .unwrap();
        let matrix = vec![
            vec![large, context.zero()],
            vec![context.zero(), context.one()],
        ];
        let (rank, _) = rank_of_coefficient_matrix(
            &context,
            &matrix,
            SymbolicaCoefficientMatrixLimits::default(),
        )
        .unwrap();
        assert_eq!(rank, 2);

        let foreign = CoefficientContext::new(["y"]);
        assert!(matches!(
            rank_of_coefficient_matrix(
                &context,
                &[vec![foreign.one()]],
                SymbolicaCoefficientMatrixLimits::default(),
            ),
            Err(SymbolicaCoefficientMatrixError::InvalidCoefficient {
                row: 0,
                column: 0,
                error: ExactAlgebraError::VariableMapMismatch { .. },
            })
        ));
    }

    #[test]
    fn native_rank_limits_cover_entries_live_bytes_and_exact_operations() {
        let context = CoefficientContext::new(["x"]);
        let matrix = vec![vec![context.one()]];
        let (_, baseline) = rank_of_coefficient_matrix(
            &context,
            &matrix,
            SymbolicaCoefficientMatrixLimits::default(),
        )
        .unwrap();
        let input_bytes = baseline.input_retained_bytes();
        let output_bytes = baseline.output_retained_bytes();
        assert!(input_bytes > 0);
        assert!(output_bytes > 0);

        let (rank, exact) = rank_of_coefficient_matrix(
            &context,
            &matrix,
            SymbolicaCoefficientMatrixLimits {
                max_single_matrix_entries: 1,
                max_live_matrix_entries: 1,
                max_exact_operations: 1,
                max_input_retained_bytes: input_bytes,
                max_output_retained_bytes: output_bytes,
                ..SymbolicaCoefficientMatrixLimits::default()
            },
        )
        .unwrap();
        assert_eq!(rank, 1);
        assert_eq!(exact.admitted_single_matrix_entries(), 1);
        assert_eq!(exact.admitted_peak_live_entries(), 1);
        assert_eq!(exact.admitted_exact_operations(), 1);
        assert_eq!(exact.exact_operations(), 1);

        for (limits, resource) in [
            (
                SymbolicaCoefficientMatrixLimits {
                    max_single_matrix_entries: 0,
                    ..SymbolicaCoefficientMatrixLimits::default()
                },
                "single Symbolica matrix entries",
            ),
            (
                SymbolicaCoefficientMatrixLimits {
                    max_live_matrix_entries: 0,
                    ..SymbolicaCoefficientMatrixLimits::default()
                },
                "live Symbolica matrix entries",
            ),
            (
                SymbolicaCoefficientMatrixLimits {
                    max_input_retained_bytes: input_bytes - 1,
                    ..SymbolicaCoefficientMatrixLimits::default()
                },
                "coefficient matrix input retained bytes",
            ),
            (
                SymbolicaCoefficientMatrixLimits {
                    max_output_retained_bytes: output_bytes - 1,
                    ..SymbolicaCoefficientMatrixLimits::default()
                },
                "coefficient matrix output retained bytes",
            ),
        ] {
            assert!(matches!(
                rank_of_coefficient_matrix(&context, &matrix, limits),
                Err(SymbolicaCoefficientMatrixError::ResourceLimit {
                    resource: actual_resource,
                    ..
                }) if actual_resource == resource
            ));
        }

        assert!(matches!(
            rank_of_coefficient_matrix(
                &context,
                &matrix,
                SymbolicaCoefficientMatrixLimits {
                    max_exact_operations: 0,
                    ..SymbolicaCoefficientMatrixLimits::default()
                },
            ),
            Err(SymbolicaCoefficientMatrixError::ExactAlgebra(
                ExactAlgebraError::ResourceLimit {
                    resource: "Symbolica coefficient matrix exact operations",
                    requested: 1,
                    limit: 0,
                }
            ))
        ));
    }

    fn check_identity_size(size: usize) {
        let context = CoefficientContext::new(["x"]);
        let matrix = identity(&context, size);
        let result = invert_and_verify_coefficient_matrix(
            &context,
            &matrix,
            SymbolicaCoefficientMatrixLimits::default(),
        )
        .unwrap();
        assert_eq!(result.inverse(), matrix);
        assert_eq!(result.determinant(), &context.one());
        for coefficient in result.inverse().iter().flatten() {
            context.validate(coefficient).unwrap();
        }
        assert_eq!(result.stats().determinant_calls(), 1);
        assert_eq!(result.stats().inverse_calls(), 1);
        assert_eq!(result.stats().product_calls(), 2);
        assert_eq!(result.stats().non_matrix_trait_calls(), 0);
    }

    macro_rules! identity_test {
        ($name:ident, $size:literal) => {
            #[test]
            fn $name() {
                check_identity_size($size);
            }
        };
    }

    identity_test!(map_aware_identity_size_1, 1);
    identity_test!(map_aware_identity_size_2, 2);
    identity_test!(map_aware_identity_size_3, 3);
    identity_test!(map_aware_identity_size_4, 4);
    identity_test!(map_aware_identity_size_5, 5);
    identity_test!(map_aware_identity_size_6, 6);

    #[test]
    fn fallible_inverse_and_division_follow_the_symbolica_ring_contract() {
        let context = CoefficientContext::new(["x"]);
        let field = CheckedCoefficientField::new(
            &context,
            SymbolicaCoefficientMatrixLimits::default(),
            1,
            1,
            2,
        );
        let zero = context.zero();
        let one = context.one();
        let x = context.parameter("x").unwrap();

        assert_eq!(field.try_inv(&zero), None);
        assert_eq!(field.try_div(&one, &zero), None);
        assert_eq!(field.try_inv(&x), Some(context.parse("1/x").unwrap()));
        assert_eq!(field.try_div(&one, &x), Some(context.parse("1/x").unwrap()));
        assert_eq!(field.stats().exact_operations(), 2);
    }

    #[test]
    fn symbolic_nonsymmetric_inverse_and_determinant_are_exact() {
        let context = CoefficientContext::new(["a", "b", "s"]);
        let matrix = vec![
            vec![context.parse("a/s").unwrap(), context.one()],
            vec![context.parameter("b").unwrap(), context.integer(2)],
        ];
        let result = invert_and_verify_coefficient_matrix(
            &context,
            &matrix,
            SymbolicaCoefficientMatrixLimits::default(),
        )
        .unwrap();
        assert_eq!(result.determinant(), &context.parse("(2*a-b*s)/s").unwrap());
        verify_coefficient_matrix_inverse(
            &context,
            &matrix,
            result.inverse(),
            SymbolicaCoefficientMatrixLimits::default(),
        )
        .unwrap();
    }

    #[test]
    fn independent_determinant_guard_rejects_general_inverse_singularity() {
        let context = CoefficientContext::new(["x"]);
        let mut matrix = identity(&context, 4);
        matrix[3] = matrix[2].clone();
        assert!(matches!(
            invert_and_verify_coefficient_matrix(
                &context,
                &matrix,
                SymbolicaCoefficientMatrixLimits::default(),
            ),
            Err(SymbolicaCoefficientMatrixError::Singular)
        ));
    }

    #[test]
    fn foreign_map_is_rejected_before_native_algebra() {
        let context = CoefficientContext::new(["x"]);
        let foreign = CoefficientContext::new(["y"]);
        let matrix = vec![vec![foreign.one()]];
        assert!(matches!(
            invert_and_verify_coefficient_matrix(
                &context,
                &matrix,
                SymbolicaCoefficientMatrixLimits::default(),
            ),
            Err(SymbolicaCoefficientMatrixError::InvalidCoefficient {
                row: 0,
                column: 0,
                error: ExactAlgebraError::VariableMapMismatch { .. },
            })
        ));
    }

    #[test]
    fn matrix_resource_limits_are_preflighted_exactly() {
        let context = CoefficientContext::new(["x"]);
        let matrix = identity(&context, 2);
        let exact = invert_and_verify_coefficient_matrix(
            &context,
            &matrix,
            SymbolicaCoefficientMatrixLimits {
                max_single_matrix_entries: 8,
                max_live_matrix_entries: 16,
                max_exact_operations: 45,
                ..SymbolicaCoefficientMatrixLimits::default()
            },
        )
        .unwrap();
        assert_eq!(exact.stats().admitted_single_matrix_entries(), 8);
        assert_eq!(exact.stats().admitted_peak_live_entries(), 16);
        assert_eq!(exact.stats().admitted_exact_operations(), 45);

        for (limits, resource, requested, limit) in [
            (
                SymbolicaCoefficientMatrixLimits {
                    max_single_matrix_entries: 7,
                    ..SymbolicaCoefficientMatrixLimits::default()
                },
                "single Symbolica matrix entries",
                8,
                7,
            ),
            (
                SymbolicaCoefficientMatrixLimits {
                    max_live_matrix_entries: 15,
                    ..SymbolicaCoefficientMatrixLimits::default()
                },
                "live Symbolica matrix entries",
                16,
                15,
            ),
            (
                SymbolicaCoefficientMatrixLimits {
                    max_exact_operations: 44,
                    ..SymbolicaCoefficientMatrixLimits::default()
                },
                "Symbolica coefficient matrix exact operations",
                45,
                44,
            ),
        ] {
            assert!(matches!(
                invert_and_verify_coefficient_matrix(&context, &matrix, limits),
                Err(SymbolicaCoefficientMatrixError::ResourceLimit {
                    resource: actual_resource,
                    requested: actual_requested,
                    limit: actual_limit,
                }) if actual_resource == resource && actual_requested == requested && actual_limit == limit
            ));
        }
    }

    #[test]
    fn exact_operation_envelopes_pin_every_native_inverse_branch() {
        let context = CoefficientContext::new(["x"]);
        // Size one and sizes four and above use Symbolica's augmented-matrix
        // inverse, while two and three use its specialized formulas.
        for (size, expected_operations) in [(1, 8), (2, 45), (3, 164), (4, 476)] {
            let matrix = identity(&context, size);
            let exact = invert_and_verify_coefficient_matrix(
                &context,
                &matrix,
                SymbolicaCoefficientMatrixLimits {
                    max_exact_operations: expected_operations,
                    ..SymbolicaCoefficientMatrixLimits::default()
                },
            )
            .unwrap();
            assert_eq!(
                exact.stats().admitted_exact_operations(),
                expected_operations
            );

            assert!(matches!(
                invert_and_verify_coefficient_matrix(
                    &context,
                    &matrix,
                    SymbolicaCoefficientMatrixLimits {
                        max_exact_operations: expected_operations - 1,
                        ..SymbolicaCoefficientMatrixLimits::default()
                    },
                ),
                Err(SymbolicaCoefficientMatrixError::ResourceLimit {
                    resource: "Symbolica coefficient matrix exact operations",
                    requested,
                    limit,
                }) if requested == expected_operations && limit == expected_operations - 1
            ));
        }
    }

    #[test]
    fn retained_byte_limits_have_exact_and_one_below_boundaries() {
        let context = CoefficientContext::new(["x"]);
        let matrix = identity(&context, 2);
        let baseline = invert_and_verify_coefficient_matrix(
            &context,
            &matrix,
            SymbolicaCoefficientMatrixLimits::default(),
        )
        .unwrap();
        let input_bytes = baseline.stats().input_retained_bytes();
        let output_bytes = baseline.stats().output_retained_bytes();
        assert!(input_bytes > 0);
        assert!(output_bytes > 0);

        invert_and_verify_coefficient_matrix(
            &context,
            &matrix,
            SymbolicaCoefficientMatrixLimits {
                max_input_retained_bytes: input_bytes,
                max_output_retained_bytes: output_bytes,
                ..SymbolicaCoefficientMatrixLimits::default()
            },
        )
        .unwrap();

        assert!(matches!(
            invert_and_verify_coefficient_matrix(
                &context,
                &matrix,
                SymbolicaCoefficientMatrixLimits {
                    max_input_retained_bytes: input_bytes - 1,
                    ..SymbolicaCoefficientMatrixLimits::default()
                },
            ),
            Err(SymbolicaCoefficientMatrixError::ResourceLimit {
                resource: "coefficient matrix input retained bytes",
                requested,
                limit,
            }) if requested == input_bytes && limit == input_bytes - 1
        ));
        assert!(matches!(
            invert_and_verify_coefficient_matrix(
                &context,
                &matrix,
                SymbolicaCoefficientMatrixLimits {
                    max_output_retained_bytes: output_bytes - 1,
                    ..SymbolicaCoefficientMatrixLimits::default()
                },
            ),
            Err(SymbolicaCoefficientMatrixError::ResourceLimit {
                resource: "coefficient matrix output retained bytes",
                limit,
                ..
            }) if limit == output_bytes - 1
        ));
    }

    #[test]
    fn checked_field_abort_recovers_the_exact_error_without_formatting_payloads() {
        let expected = ExactAlgebraError::ResourceLimit {
            resource: "sentinel-test",
            requested: 2,
            limit: 1,
        };
        let error = call_native("sentinel transport", || {
            abort_checked_field(expected.clone())
        })
        .unwrap_err();
        assert_eq!(
            error,
            SymbolicaCoefficientMatrixError::ExactAlgebra(expected)
        );
        assert!(!error.to_string().contains("matrix payload"));
    }

    #[test]
    fn unexpected_native_panic_is_redacted() {
        struct UnexpectedPanic;
        let error = call_native("panic test", || {
            resume_unwind(Box::new(UnexpectedPanic));
        })
        .unwrap_err();
        assert_eq!(
            error,
            SymbolicaCoefficientMatrixError::NativePanic {
                operation: "panic test"
            }
        );
        assert!(!error.to_string().contains("UnexpectedPanic"));
    }

    #[test]
    fn exact_algebra_failure_crosses_native_boundary_as_typed_error() {
        let context = CoefficientContext::new(["x"]);
        let matrix = vec![
            vec![context.parameter("x").unwrap(), context.zero()],
            vec![context.zero(), context.one()],
        ];
        let error = invert_and_verify_coefficient_matrix(
            &context,
            &matrix,
            SymbolicaCoefficientMatrixLimits {
                exact_algebra: ExactAlgebraLimits {
                    max_term_operations: 0,
                    ..ExactAlgebraLimits::default()
                },
                ..SymbolicaCoefficientMatrixLimits::default()
            },
        )
        .unwrap_err();
        assert!(matches!(
            error,
            SymbolicaCoefficientMatrixError::ExactAlgebra(ExactAlgebraError::ResourceLimit {
                limit: 0,
                ..
            })
        ));
    }

    #[test]
    fn checked_field_abort_is_contained_across_parallel_native_sessions() {
        let workers = (0..8)
            .map(|worker| {
                std::thread::spawn(move || {
                    let parameter = format!("x{worker}");
                    let context = CoefficientContext::new([parameter.as_str()]);
                    let matrix = vec![
                        vec![context.parameter(&parameter).unwrap(), context.zero()],
                        vec![context.zero(), context.one()],
                    ];
                    let error = invert_and_verify_coefficient_matrix(
                        &context,
                        &matrix,
                        SymbolicaCoefficientMatrixLimits {
                            exact_algebra: ExactAlgebraLimits {
                                max_term_operations: 0,
                                ..ExactAlgebraLimits::default()
                            },
                            ..SymbolicaCoefficientMatrixLimits::default()
                        },
                    )
                    .unwrap_err();
                    assert!(matches!(
                        error,
                        SymbolicaCoefficientMatrixError::ExactAlgebra(
                            ExactAlgebraError::ResourceLimit { limit: 0, .. }
                        )
                    ));

                    let expected = ExactAlgebraError::ResourceLimit {
                        resource: "parallel-sentinel-test",
                        requested: worker + 1,
                        limit: worker,
                    };
                    assert_eq!(
                        call_native("parallel sentinel transport", || {
                            abort_checked_field(expected.clone())
                        })
                        .unwrap_err(),
                        SymbolicaCoefficientMatrixError::ExactAlgebra(expected)
                    );
                })
            })
            .collect::<Vec<_>>();

        for worker in workers {
            worker.join().expect("parallel native session panicked");
        }
    }

    #[test]
    fn rectangular_product_is_symbolica_owned_and_authenticated() {
        let context = CoefficientContext::new(["x"]);
        let left = vec![
            vec![context.one(), context.integer(2), context.integer(3)],
            vec![context.integer(4), context.integer(5), context.integer(6)],
        ];
        let right = vec![
            vec![context.integer(7)],
            vec![context.integer(8)],
            vec![context.integer(9)],
        ];
        let (product, stats) = multiply_coefficient_matrices(
            &context,
            &left,
            &right,
            SymbolicaCoefficientMatrixLimits::default(),
        )
        .unwrap();
        assert_eq!(
            product,
            vec![vec![context.integer(50)], vec![context.integer(122)]]
        );
        assert_eq!(stats.exact_operations(), 12);
        assert_eq!(stats.product_calls(), 1);
    }
}
