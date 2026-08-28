//! Authenticated, resource-bounded access to Symbolica's exact coefficient and
//! matrix algebra.
//!
//! This module is deliberately provenance-neutral.  Symbolica owns coefficient
//! powers plus matrix rank, determinant, inversion, and multiplication;
//! RustRed supplies only the authenticated coefficient domain, admission
//! policy, typed failure transport, and replay checks needed by proof-bearing
//! callers.
//!
//! Input and every retained native output are censused by exact clone-owned
//! capacity.  Symbolica's public scalar API does not expose a complete bound on
//! polynomial GCD, quotient, or dense-multiplication scratch, so that remaining
//! native scratch gap is explicit rather than being disguised as a byte proof.
//! Typed scalar failures cross Symbolica's infallible field traits through a
//! private unwind payload.  This boundary therefore requires Rust's
//! `panic = "unwind"`; `panic = "abort"` builds cannot recover a typed failure.
//!

#[cfg(not(panic = "unwind"))]
compile_error!(
    "RustRed's authenticated Symbolica algebra boundary requires panic=\"unwind\" for typed failure transport"
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

use crate::algebra::{
    Coefficient, CoefficientContext, CoefficientPolynomialPart, ExactAlgebraError,
    ExactAlgebraLimits, ExactAlgebraOperation, coefficient_clone_owned_retained_byte_bound,
};

const DEFAULT_MAX_SINGLE_MATRIX_ENTRIES: usize = 16_000_000;
const DEFAULT_MAX_LIVE_MATRIX_ENTRIES: usize = 32_000_000;
pub(crate) const DEFAULT_MAX_EXACT_OPERATIONS: usize = 100_000_000;
pub(crate) const DEFAULT_MAX_INPUT_RETAINED_BYTES: usize = 1024 * 1024 * 1024;
pub(crate) const DEFAULT_MAX_OUTPUT_RETAINED_BYTES: usize = 1024 * 1024 * 1024;

/// Admission policy for one bounded Symbolica coefficient or matrix session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SymbolicaCoefficientMatrixLimits {
    pub(crate) exact_algebra: ExactAlgebraLimits,
    /// Largest individual native matrix payload.  General inversion needs the
    /// augmented `n x 2n` matrix here.
    pub(crate) max_single_matrix_entries: usize,
    /// Largest conservative simultaneously-live native payload.
    pub(crate) max_live_matrix_entries: usize,
    /// Largest number of checked exact arithmetic operations admitted for the
    /// complete requested native operation. Constant construction and
    /// zero/one predicates are censused separately.
    pub(crate) max_exact_operations: usize,
    /// Aggregate clone-owned retained bytes in authenticated caller inputs.
    pub(crate) max_input_retained_bytes: usize,
    /// Aggregate clone-owned retained bytes in powers, determinants, inverses,
    /// and verification-product outputs inspected during the native session.
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

/// Exact census of one admitted native coefficient or matrix session.
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
    transpose_calls: usize,
    rank_calls: usize,
    power_calls: usize,
    admitted_power_exponent: u64,
    admitted_power_term_operations: usize,
    admitted_power_numerator_terms: usize,
    admitted_power_denominator_terms: usize,
    output_power_numerator_terms: usize,
    output_power_denominator_terms: usize,
    non_matrix_trait_calls: usize,
}

impl SymbolicaCoefficientMatrixStats {
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

    pub(crate) const fn determinant_calls(self) -> usize {
        self.determinant_calls
    }

    pub(crate) const fn product_calls(self) -> usize {
        self.product_calls
    }

    pub(crate) const fn transpose_calls(self) -> usize {
        self.transpose_calls
    }

    pub(crate) const fn rank_calls(self) -> usize {
        self.rank_calls
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
        self.charge_counter(|stats| &mut stats.power_calls);
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

#[derive(Clone, Copy, Debug)]
struct PolynomialPowerBounds {
    output_terms: usize,
    max_term_operations: usize,
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

fn polynomial_degree_box(
    polynomial: &MultivariatePolynomial<IntegerRing, u16>,
    exponent: u64,
    resource: &'static str,
    limit: usize,
) -> Result<usize, SymbolicaCoefficientMatrixError> {
    let mut terms = 1usize;
    for variable in 0..polynomial.variables.len() {
        let degree = u128::from(polynomial.degree(variable))
            .checked_mul(u128::from(exponent))
            .ok_or(SymbolicaCoefficientMatrixError::ExactAlgebra(
                ExactAlgebraError::ResourceCountOverflow { resource },
            ))?;
        let width = usize::try_from(degree.checked_add(1).ok_or(
            SymbolicaCoefficientMatrixError::ExactAlgebra(
                ExactAlgebraError::ResourceCountOverflow { resource },
            ),
        )?)
        .map_err(|_| {
            SymbolicaCoefficientMatrixError::ExactAlgebra(
                ExactAlgebraError::ResourceCountOverflow { resource },
            )
        })?;
        terms = terms
            .checked_mul(width)
            .ok_or(SymbolicaCoefficientMatrixError::ExactAlgebra(
                ExactAlgebraError::ResourceCountOverflow { resource },
            ))?;
        if terms > limit {
            return Err(SymbolicaCoefficientMatrixError::ExactAlgebra(
                ExactAlgebraError::ResourceLimit {
                    resource,
                    requested: terms,
                    limit,
                },
            ));
        }
    }
    if terms > limit {
        return Err(SymbolicaCoefficientMatrixError::ExactAlgebra(
            ExactAlgebraError::ResourceLimit {
                resource,
                requested: terms,
                limit,
            },
        ));
    }
    Ok(terms)
}

fn polynomial_power_bounds(
    polynomial: &MultivariatePolynomial<IntegerRing, u16>,
    exponent: u64,
    part: CoefficientPolynomialPart,
    limits: ExactAlgebraLimits,
) -> Result<PolynomialPowerBounds, SymbolicaCoefficientMatrixError> {
    for variable in 0..polynomial.variables.len() {
        let requested = u128::from(polynomial.degree(variable))
            .checked_mul(u128::from(exponent))
            .ok_or(SymbolicaCoefficientMatrixError::ExactAlgebra(
                ExactAlgebraError::ResourceCountOverflow {
                    resource: "exact coefficient power degree",
                },
            ))?;
        if requested > limits.max_exponent {
            return Err(SymbolicaCoefficientMatrixError::ExactAlgebra(
                ExactAlgebraError::ExponentLimit {
                    operation: ExactAlgebraOperation::Multiply,
                    variable,
                    requested,
                    limit: limits.max_exponent,
                },
            ));
        }
    }

    if exponent == 0 {
        return Ok(PolynomialPowerBounds {
            output_terms: 1,
            max_term_operations: 0,
        });
    }
    if polynomial.is_zero() {
        return Ok(PolynomialPowerBounds {
            output_terms: 0,
            max_term_operations: 0,
        });
    }

    let output_resource = polynomial_power_resource(part, true);
    let operation_resource = polynomial_power_resource(part, false);
    // A componentwise degree box is deliberately used instead of `nterms^e`:
    // Symbolica's rational multiplication computes cross-GCD quotients, whose
    // sparse support can be denser than either input support.  The degree box
    // remains valid for those intermediates without reimplementing that CAS
    // algebra in RustRed.
    let output_terms = polynomial_degree_box(
        polynomial,
        exponent,
        output_resource,
        limits.max_polynomial_terms,
    )?;
    let previous_terms = polynomial_degree_box(
        polynomial,
        exponent - 1,
        operation_resource,
        limits.max_term_operations,
    )?;
    let base_terms = polynomial_degree_box(
        polynomial,
        1,
        operation_resource,
        limits.max_term_operations,
    )?;
    let max_term_operations = previous_terms.checked_mul(base_terms).ok_or(
        SymbolicaCoefficientMatrixError::ExactAlgebra(ExactAlgebraError::ResourceCountOverflow {
            resource: operation_resource,
        }),
    )?;
    if max_term_operations > limits.max_term_operations {
        return Err(SymbolicaCoefficientMatrixError::ExactAlgebra(
            ExactAlgebraError::ResourceLimit {
                resource: operation_resource,
                requested: max_term_operations,
                limit: limits.max_term_operations,
            },
        ));
    }
    Ok(PolynomialPowerBounds {
        output_terms,
        max_term_operations,
    })
}

/// Raise one authenticated coefficient through Symbolica's public exact field
/// power API.
///
/// The vendored rational-polynomial field currently implements `pow(b, e)` as
/// exactly `e` repeated rational multiplications.  RustRed admits and records
/// that native schedule, but delegates every multiplication to Symbolica.  A
/// degree-box preflight covers potentially dense cross-GCD quotients; the
/// result is then re-authenticated and charged by exact clone-owned capacity.
pub(crate) fn power_of_coefficient(
    context: &CoefficientContext,
    base: &Coefficient,
    exponent: u64,
    limits: SymbolicaCoefficientMatrixLimits,
) -> Result<(Coefficient, SymbolicaCoefficientMatrixStats), SymbolicaCoefficientMatrixError> {
    context
        .validate_with_limits(base, limits.exact_algebra)
        .map_err(SymbolicaCoefficientMatrixError::ExactAlgebra)?;

    let native_exponent_limit = u64::from(u32::MAX);
    if exponent > native_exponent_limit {
        return Err(SymbolicaCoefficientMatrixError::ResourceLimit {
            resource: "Symbolica coefficient power exponent",
            requested: usize::try_from(exponent).map_err(|_| {
                SymbolicaCoefficientMatrixError::ResourceCountOverflow {
                    resource: "Symbolica coefficient power exponent",
                }
            })?,
            limit: u32::MAX as usize,
        });
    }
    let operations = usize::try_from(exponent).map_err(|_| {
        SymbolicaCoefficientMatrixError::ResourceCountOverflow {
            resource: "Symbolica coefficient matrix exact operations",
        }
    })?;
    check_limit(
        "Symbolica coefficient matrix exact operations",
        operations,
        limits.max_exact_operations,
    )?;

    let numerator = polynomial_power_bounds(
        &base.numerator,
        exponent,
        CoefficientPolynomialPart::Numerator,
        limits.exact_algebra,
    )?;
    let denominator = polynomial_power_bounds(
        &base.denominator,
        exponent,
        CoefficientPolynomialPart::Denominator,
        limits.exact_algebra,
    )?;
    let input_retained_bytes = coefficient_retained_bytes(base)?;
    check_limit(
        "coefficient matrix input retained bytes",
        input_retained_bytes,
        limits.max_input_retained_bytes,
    )?;

    let field = CheckedCoefficientField::new(context, limits, 0, 0, operations);
    let state = field.state.clone();
    {
        let mut state = state.borrow_mut();
        state.stats.input_entries = 1;
        state.stats.input_retained_bytes = input_retained_bytes;
        state.stats.admitted_power_exponent = exponent;
        state.stats.admitted_power_term_operations = numerator
            .max_term_operations
            .max(denominator.max_term_operations);
        state.stats.admitted_power_numerator_terms = numerator.output_terms;
        state.stats.admitted_power_denominator_terms = denominator.output_terms;
        // These are the exact repeated multiplications performed by the
        // currently vendored public Symbolica RPF power implementation.
        state.stats.exact_operations = operations;
        state.stats.multiplications = operations;
    }

    let output = call_native("coefficient power", || field.pow(base, exponent))?;
    authenticate_output_coefficient(context, &output, limits, &state)?;
    {
        let mut state = state.borrow_mut();
        state.stats.output_entries = 1;
        state.stats.output_power_numerator_terms = output.numerator.nterms();
        state.stats.output_power_denominator_terms = output.denominator.nterms();
    }
    let stats = state.borrow().stats;
    Ok((output, stats))
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

/// Multiply three authenticated coefficient matrices in one native session.
/// The intermediate product is authenticated before it is consumed, while
/// RustRed owns only shape/resource policy and result transport.
pub(crate) fn multiply_three_coefficient_matrices(
    context: &CoefficientContext,
    left: &[Vec<Coefficient>],
    middle: &[Vec<Coefficient>],
    right: &[Vec<Coefficient>],
    limits: SymbolicaCoefficientMatrixLimits,
) -> Result<(Vec<Vec<Coefficient>>, SymbolicaCoefficientMatrixStats), SymbolicaCoefficientMatrixError>
{
    let left_shape = inspect_rows(left)?;
    let middle_shape = inspect_rows(middle)?;
    let right_shape = inspect_rows(right)?;
    if left_shape.columns != middle_shape.rows {
        return Err(SymbolicaCoefficientMatrixError::ShapeMismatch {
            left_rows: left_shape.rows,
            left_columns: left_shape.columns,
            right_rows: middle_shape.rows,
            right_columns: middle_shape.columns,
        });
    }
    if middle_shape.columns != right_shape.rows {
        return Err(SymbolicaCoefficientMatrixError::ShapeMismatch {
            left_rows: middle_shape.rows,
            left_columns: middle_shape.columns,
            right_rows: right_shape.rows,
            right_columns: right_shape.columns,
        });
    }

    let intermediate_shape = checked_shape(left_shape.rows, middle_shape.columns)?;
    let output_shape = checked_shape(left_shape.rows, right_shape.columns)?;
    let single_entries = left_shape
        .entries
        .max(middle_shape.entries)
        .max(right_shape.entries)
        .max(intermediate_shape.entries)
        .max(output_shape.entries);
    let first_live_entries = checked_add(
        "live Symbolica matrix entries",
        checked_add(
            "live Symbolica matrix entries",
            checked_add(
                "live Symbolica matrix entries",
                left_shape.entries,
                middle_shape.entries,
            )?,
            right_shape.entries,
        )?,
        intermediate_shape.entries,
    )?;
    let second_live_entries = checked_add(
        "live Symbolica matrix entries",
        checked_add(
            "live Symbolica matrix entries",
            right_shape.entries,
            intermediate_shape.entries,
        )?,
        output_shape.entries,
    )?;
    let live_entries = first_live_entries.max(second_live_entries);
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
    let operations = checked_add(
        "Symbolica coefficient matrix exact operations",
        product_operation_bound(left_shape.rows, left_shape.columns, middle_shape.columns)?,
        product_operation_bound(
            intermediate_shape.rows,
            intermediate_shape.columns,
            right_shape.columns,
        )?,
    )?;
    check_limit(
        "Symbolica coefficient matrix exact operations",
        operations,
        limits.max_exact_operations,
    )?;

    let field =
        CheckedCoefficientField::new(context, limits, single_entries, live_entries, operations);
    let state = field.state.clone();
    let left = matrix_from_rows(left, left_shape, field.clone())?;
    let middle = matrix_from_rows(middle, middle_shape, field.clone())?;
    let right = matrix_from_rows(right, right_shape, field)?;

    increment_session_counter(&state, "Symbolica matrix product calls", |stats| {
        &mut stats.product_calls
    })?;
    let intermediate = call_native("first three-matrix product", || &left * &middle)?;
    if intermediate.nrows() != intermediate_shape.rows
        || intermediate.ncols() != intermediate_shape.columns
    {
        return Err(SymbolicaCoefficientMatrixError::InternalShapeFailure {
            operation: "first three-matrix product",
        });
    }
    authenticate_native(context, &intermediate, limits, &state)?;
    drop(left);
    drop(middle);

    increment_session_counter(&state, "Symbolica matrix product calls", |stats| {
        &mut stats.product_calls
    })?;
    let product = call_native("second three-matrix product", || &intermediate * &right)?;
    if product.nrows() != output_shape.rows || product.ncols() != output_shape.columns {
        return Err(SymbolicaCoefficientMatrixError::InternalShapeFailure {
            operation: "second three-matrix product",
        });
    }
    authenticate_native(context, &product, limits, &state)?;
    let product = native_into_rows(product, &state)?;
    let stats = state.borrow().stats;
    Ok((product, stats))
}

/// Compute `transform * middle * transform^T` through Symbolica's native
/// transpose and matrix products in one authenticated session.
///
/// Keeping the transpose inside this boundary prevents callers from growing a
/// second, handwritten standard-matrix implementation merely to form a
/// congruence.  The two native product outputs are both authenticated and
/// charged to the output-byte census before the final matrix is returned.
pub(crate) fn congruence_of_coefficient_matrix(
    context: &CoefficientContext,
    transform: &[Vec<Coefficient>],
    middle: &[Vec<Coefficient>],
    limits: SymbolicaCoefficientMatrixLimits,
) -> Result<(Vec<Vec<Coefficient>>, SymbolicaCoefficientMatrixStats), SymbolicaCoefficientMatrixError>
{
    let transform_shape = inspect_rows(transform)?;
    let middle_shape = inspect_rows(middle)?;
    if transform_shape.columns != middle_shape.rows
        || middle_shape.columns != transform_shape.columns
    {
        return Err(SymbolicaCoefficientMatrixError::ShapeMismatch {
            left_rows: transform_shape.rows,
            left_columns: transform_shape.columns,
            right_rows: middle_shape.rows,
            right_columns: middle_shape.columns,
        });
    }

    let intermediate_shape = checked_shape(transform_shape.rows, middle_shape.columns)?;
    let output_shape = checked_shape(transform_shape.rows, transform_shape.rows)?;
    let single_entries = transform_shape
        .entries
        .max(middle_shape.entries)
        .max(intermediate_shape.entries)
        .max(output_shape.entries);
    let first_live_entries = checked_add(
        "live Symbolica matrix entries",
        checked_add(
            "live Symbolica matrix entries",
            checked_mul("live Symbolica matrix entries", 2, transform_shape.entries)?,
            middle_shape.entries,
        )?,
        intermediate_shape.entries,
    )?;
    let second_live_entries = checked_add(
        "live Symbolica matrix entries",
        checked_add(
            "live Symbolica matrix entries",
            transform_shape.entries,
            intermediate_shape.entries,
        )?,
        output_shape.entries,
    )?;
    let live_entries = first_live_entries.max(second_live_entries);
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
    let operations = checked_add(
        "Symbolica coefficient matrix exact operations",
        product_operation_bound(
            transform_shape.rows,
            transform_shape.columns,
            middle_shape.columns,
        )?,
        product_operation_bound(
            intermediate_shape.rows,
            intermediate_shape.columns,
            transform_shape.rows,
        )?,
    )?;
    check_limit(
        "Symbolica coefficient matrix exact operations",
        operations,
        limits.max_exact_operations,
    )?;

    let field =
        CheckedCoefficientField::new(context, limits, single_entries, live_entries, operations);
    let state = field.state.clone();
    let transform = matrix_from_rows(transform, transform_shape, field.clone())?;
    let middle = matrix_from_rows(middle, middle_shape, field)?;
    increment_session_counter(&state, "Symbolica matrix transpose calls", |stats| {
        &mut stats.transpose_calls
    })?;
    let transposed = call_native("congruence transpose", || transform.transpose())?;
    if transposed.nrows() != transform_shape.columns || transposed.ncols() != transform_shape.rows {
        return Err(SymbolicaCoefficientMatrixError::InternalShapeFailure {
            operation: "congruence transpose",
        });
    }
    authenticate_native(context, &transposed, limits, &state)?;

    increment_session_counter(&state, "Symbolica matrix product calls", |stats| {
        &mut stats.product_calls
    })?;
    let intermediate = call_native("left congruence product", || &transform * &middle)?;
    if intermediate.nrows() != intermediate_shape.rows
        || intermediate.ncols() != intermediate_shape.columns
    {
        return Err(SymbolicaCoefficientMatrixError::InternalShapeFailure {
            operation: "left congruence product",
        });
    }
    authenticate_native(context, &intermediate, limits, &state)?;
    drop(transform);
    drop(middle);

    increment_session_counter(&state, "Symbolica matrix product calls", |stats| {
        &mut stats.product_calls
    })?;
    let product = call_native("right congruence product", || &intermediate * &transposed)?;
    if product.nrows() != output_shape.rows || product.ncols() != output_shape.columns {
        return Err(SymbolicaCoefficientMatrixError::InternalShapeFailure {
            operation: "right congruence product",
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
    fn public_symbolica_power_is_authenticated_and_fully_censused() {
        let context = CoefficientContext::new(["x", "y"]);
        let base = context.coefficient_fixture("(x+y)/(1-x)");
        let (power, stats) = power_of_coefficient(
            &context,
            &base,
            3,
            SymbolicaCoefficientMatrixLimits::default(),
        )
        .unwrap();
        assert_eq!(power, context.coefficient_fixture("(x+y)^3/(1-x)^3"));
        context
            .validate_with_limits(&power, ExactAlgebraLimits::default())
            .unwrap();
        assert_eq!(stats.input_entries, 1);
        assert_eq!(stats.output_entries, 1);
        assert_eq!(stats.authenticated_entries, 1);
        assert_eq!(stats.power_calls, 1);
        assert_eq!(stats.non_matrix_trait_calls, 1);
        assert_eq!(stats.admitted_power_exponent, 3);
        assert_eq!(stats.admitted_exact_operations(), 3);
        assert_eq!(stats.exact_operations(), 3);
        assert_eq!(stats.multiplications, 3);
        assert_eq!(stats.admitted_power_term_operations, 36);
        assert_eq!(stats.admitted_power_numerator_terms, 16);
        assert_eq!(stats.admitted_power_denominator_terms, 4);
        assert_eq!(stats.output_power_numerator_terms, 4);
        assert_eq!(stats.output_power_denominator_terms, 4);
        assert!(stats.input_retained_bytes() > 0);
        assert!(stats.output_retained_bytes() > 0);
    }

    #[test]
    fn public_symbolica_power_handles_zero_exponent_without_multiplication() {
        let context = CoefficientContext::new(["x"]);
        let (power, stats) = power_of_coefficient(
            &context,
            &context.zero(),
            0,
            SymbolicaCoefficientMatrixLimits {
                max_exact_operations: 0,
                ..SymbolicaCoefficientMatrixLimits::default()
            },
        )
        .unwrap();
        assert_eq!(power, context.one());
        assert_eq!(stats.power_calls, 1);
        assert_eq!(stats.exact_operations(), 0);
        assert_eq!(stats.admitted_power_term_operations, 0);
        assert_eq!(stats.admitted_power_numerator_terms, 1);
        assert_eq!(stats.admitted_power_denominator_terms, 1);
    }

    #[test]
    fn public_symbolica_power_preflights_map_exponent_degree_and_terms() {
        let context = CoefficientContext::new(["x", "y"]);
        let foreign = CoefficientContext::new(["z"]);
        assert!(matches!(
            power_of_coefficient(
                &context,
                &foreign.parameter("z").unwrap(),
                2,
                SymbolicaCoefficientMatrixLimits::default(),
            ),
            Err(SymbolicaCoefficientMatrixError::ExactAlgebra(
                ExactAlgebraError::VariableMapMismatch { .. }
            ))
        ));

        assert!(matches!(
            power_of_coefficient(
                &context,
                &context.one(),
                u64::from(u32::MAX) + 1,
                SymbolicaCoefficientMatrixLimits {
                    max_exact_operations: usize::MAX,
                    ..SymbolicaCoefficientMatrixLimits::default()
                },
            ),
            Err(SymbolicaCoefficientMatrixError::ResourceLimit {
                resource: "Symbolica coefficient power exponent",
                requested,
                limit,
            }) if requested == u32::MAX as usize + 1 && limit == u32::MAX as usize
        ));

        assert!(matches!(
            power_of_coefficient(
                &context,
                &context.coefficient_fixture("x^2"),
                3,
                SymbolicaCoefficientMatrixLimits {
                    exact_algebra: ExactAlgebraLimits {
                        max_exponent: 5,
                        ..ExactAlgebraLimits::default()
                    },
                    ..SymbolicaCoefficientMatrixLimits::default()
                },
            ),
            Err(SymbolicaCoefficientMatrixError::ExactAlgebra(
                ExactAlgebraError::ExponentLimit {
                    operation: ExactAlgebraOperation::Multiply,
                    variable: 0,
                    requested: 6,
                    limit: 5,
                }
            ))
        ));

        let dense_bound = context.coefficient_fixture("x+y");
        assert!(matches!(
            power_of_coefficient(
                &context,
                &dense_bound,
                2,
                SymbolicaCoefficientMatrixLimits {
                    exact_algebra: ExactAlgebraLimits {
                        max_polynomial_terms: 8,
                        ..ExactAlgebraLimits::default()
                    },
                    ..SymbolicaCoefficientMatrixLimits::default()
                },
            ),
            Err(SymbolicaCoefficientMatrixError::ExactAlgebra(
                ExactAlgebraError::ResourceLimit {
                    resource: "exact coefficient power numerator output terms",
                    requested: 9,
                    limit: 8,
                }
            ))
        ));
        assert!(matches!(
            power_of_coefficient(
                &context,
                &dense_bound,
                2,
                SymbolicaCoefficientMatrixLimits {
                    exact_algebra: ExactAlgebraLimits {
                        max_term_operations: 15,
                        ..ExactAlgebraLimits::default()
                    },
                    ..SymbolicaCoefficientMatrixLimits::default()
                },
            ),
            Err(SymbolicaCoefficientMatrixError::ExactAlgebra(
                ExactAlgebraError::ResourceLimit {
                    resource: "exact coefficient power numerator term operations",
                    requested: 16,
                    limit: 15,
                }
            ))
        ));
        assert!(matches!(
            power_of_coefficient(
                &context,
                &dense_bound,
                3,
                SymbolicaCoefficientMatrixLimits {
                    max_exact_operations: 2,
                    ..SymbolicaCoefficientMatrixLimits::default()
                },
            ),
            Err(SymbolicaCoefficientMatrixError::ResourceLimit {
                resource: "Symbolica coefficient matrix exact operations",
                requested: 3,
                limit: 2,
            })
        ));
    }

    #[test]
    fn public_symbolica_power_retained_byte_caps_are_exact() {
        let context = CoefficientContext::new(["x"]);
        let base = context.coefficient_fixture("(x+1)/(x-1)");
        let (_, baseline) = power_of_coefficient(
            &context,
            &base,
            3,
            SymbolicaCoefficientMatrixLimits::default(),
        )
        .unwrap();
        let input_bytes = baseline.input_retained_bytes();
        let output_bytes = baseline.output_retained_bytes();
        power_of_coefficient(
            &context,
            &base,
            3,
            SymbolicaCoefficientMatrixLimits {
                max_input_retained_bytes: input_bytes,
                max_output_retained_bytes: output_bytes,
                ..SymbolicaCoefficientMatrixLimits::default()
            },
        )
        .unwrap();
        assert!(matches!(
            power_of_coefficient(
                &context,
                &base,
                3,
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
            power_of_coefficient(
                &context,
                &base,
                3,
                SymbolicaCoefficientMatrixLimits {
                    max_output_retained_bytes: output_bytes - 1,
                    ..SymbolicaCoefficientMatrixLimits::default()
                },
            ),
            Err(SymbolicaCoefficientMatrixError::ResourceLimit {
                resource: "coefficient matrix output retained bytes",
                requested,
                limit,
            }) if requested == output_bytes && limit == output_bytes - 1
        ));
    }

    #[test]
    fn rectangular_symbolic_rational_rank_is_native_and_authenticated() {
        let context = CoefficientContext::new(["a", "b", "x"]);
        let matrix = vec![
            vec![
                context.zero(),
                context.coefficient_fixture("a/x"),
                context.zero(),
                context.one(),
            ],
            vec![
                context.zero(),
                context.zero(),
                context.parameter("b").unwrap(),
                context.coefficient_fixture("1/x"),
            ],
            vec![
                context.zero(),
                context.coefficient_fixture("2*a/x"),
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
        assert_eq!(stats.input_entries, 12);
        assert_eq!(stats.output_entries, 0);
        assert_eq!(stats.authenticated_entries, 12);
        assert!(stats.input_retained_bytes() > 0);
        assert!(stats.output_retained_bytes() > 0);
        assert_eq!(stats.non_matrix_trait_calls, 0);
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
        let large = context.coefficient_fixture("340282366920938463463374607431768211507");
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
        assert_eq!(result.inverse, matrix);
        assert_eq!(result.determinant, context.one());
        for coefficient in result.inverse.iter().flatten() {
            context
                .validate_with_limits(coefficient, ExactAlgebraLimits::default())
                .unwrap();
        }
        assert_eq!(result.stats.determinant_calls(), 1);
        assert_eq!(result.stats.inverse_calls, 1);
        assert_eq!(result.stats.product_calls(), 2);
        assert_eq!(result.stats.non_matrix_trait_calls, 0);
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
        assert_eq!(field.try_inv(&x), Some(context.coefficient_fixture("1/x")));
        assert_eq!(
            field.try_div(&one, &x),
            Some(context.coefficient_fixture("1/x"))
        );
        assert_eq!(field.state.borrow().stats.exact_operations(), 2);
    }

    #[test]
    fn symbolic_nonsymmetric_inverse_and_determinant_are_exact() {
        let context = CoefficientContext::new(["a", "b", "s"]);
        let matrix = vec![
            vec![context.coefficient_fixture("a/s"), context.one()],
            vec![context.parameter("b").unwrap(), context.integer(2)],
        ];
        let result = invert_and_verify_coefficient_matrix(
            &context,
            &matrix,
            SymbolicaCoefficientMatrixLimits::default(),
        )
        .unwrap();
        assert_eq!(
            result.determinant,
            context.coefficient_fixture("(2*a-b*s)/s")
        );
        verify_coefficient_matrix_inverse(
            &context,
            &matrix,
            &result.inverse,
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
        assert_eq!(exact.stats.admitted_single_matrix_entries(), 8);
        assert_eq!(exact.stats.admitted_peak_live_entries(), 16);
        assert_eq!(exact.stats.admitted_exact_operations(), 45);

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
            assert_eq!(exact.stats.admitted_exact_operations(), expected_operations);

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
        let input_bytes = baseline.stats.input_retained_bytes();
        let output_bytes = baseline.stats.output_retained_bytes();
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

    #[test]
    fn symbolic_three_matrix_product_is_native_and_exactly_bounded() {
        let context = CoefficientContext::new(["x", "y"]);
        let left = vec![
            vec![context.parameter("x").unwrap(), context.one()],
            vec![context.zero(), context.integer(2)],
        ];
        let middle = vec![
            vec![context.one(), context.zero()],
            vec![context.parameter("y").unwrap(), context.one()],
        ];
        let right = vec![
            vec![context.coefficient_fixture("1/2"), context.zero()],
            vec![context.one(), context.one()],
        ];
        let (product, stats) = multiply_three_coefficient_matrices(
            &context,
            &left,
            &middle,
            &right,
            SymbolicaCoefficientMatrixLimits::default(),
        )
        .unwrap();
        assert_eq!(
            product,
            vec![
                vec![context.coefficient_fixture("1+(x+y)/2"), context.one(),],
                vec![context.coefficient_fixture("y+2"), context.integer(2)],
            ]
        );
        assert_eq!(stats.product_calls(), 2);
        assert_eq!(stats.transpose_calls(), 0);
        assert_eq!(stats.exact_operations(), 32);
        assert_eq!(stats.admitted_exact_operations(), 32);
        assert_eq!(stats.admitted_single_matrix_entries(), 4);
        assert_eq!(stats.admitted_peak_live_entries(), 16);
        assert!(stats.input_retained_bytes() > 0);
        assert!(stats.output_retained_bytes() > 0);

        let exact = SymbolicaCoefficientMatrixLimits {
            max_single_matrix_entries: stats.admitted_single_matrix_entries(),
            max_live_matrix_entries: stats.admitted_peak_live_entries(),
            max_exact_operations: stats.admitted_exact_operations(),
            max_input_retained_bytes: stats.input_retained_bytes(),
            max_output_retained_bytes: stats.output_retained_bytes(),
            ..SymbolicaCoefficientMatrixLimits::default()
        };
        let (_, replayed_stats) =
            multiply_three_coefficient_matrices(&context, &left, &middle, &right, exact).unwrap();
        assert_eq!(replayed_stats, stats);

        for (limits, resource) in [
            (
                SymbolicaCoefficientMatrixLimits {
                    max_single_matrix_entries: stats.admitted_single_matrix_entries() - 1,
                    ..SymbolicaCoefficientMatrixLimits::default()
                },
                "single Symbolica matrix entries",
            ),
            (
                SymbolicaCoefficientMatrixLimits {
                    max_live_matrix_entries: stats.admitted_peak_live_entries() - 1,
                    ..SymbolicaCoefficientMatrixLimits::default()
                },
                "live Symbolica matrix entries",
            ),
            (
                SymbolicaCoefficientMatrixLimits {
                    max_exact_operations: stats.admitted_exact_operations() - 1,
                    ..SymbolicaCoefficientMatrixLimits::default()
                },
                "Symbolica coefficient matrix exact operations",
            ),
            (
                SymbolicaCoefficientMatrixLimits {
                    max_input_retained_bytes: stats.input_retained_bytes() - 1,
                    ..SymbolicaCoefficientMatrixLimits::default()
                },
                "coefficient matrix input retained bytes",
            ),
            (
                SymbolicaCoefficientMatrixLimits {
                    max_output_retained_bytes: stats.output_retained_bytes() - 1,
                    ..SymbolicaCoefficientMatrixLimits::default()
                },
                "coefficient matrix output retained bytes",
            ),
        ] {
            assert!(matches!(
                multiply_three_coefficient_matrices(&context, &left, &middle, &right, limits),
                Err(SymbolicaCoefficientMatrixError::ResourceLimit {
                    resource: actual,
                    ..
                }) if actual == resource
            ));
        }

        assert!(matches!(
            multiply_three_coefficient_matrices(
                &context,
                &left,
                &[vec![context.one(), context.zero(), context.zero()]],
                &right,
                SymbolicaCoefficientMatrixLimits::default(),
            ),
            Err(SymbolicaCoefficientMatrixError::ShapeMismatch { .. })
        ));
    }

    #[test]
    fn symbolic_congruence_uses_native_transpose_and_censuses_its_output() {
        let context = CoefficientContext::new(["x", "y"]);
        let transform = vec![
            vec![context.one(), context.parameter("x").unwrap()],
            vec![context.zero(), context.one()],
        ];
        let middle = vec![
            vec![context.integer(2), context.parameter("y").unwrap()],
            vec![context.parameter("y").unwrap(), context.integer(3)],
        ];
        let (product, stats) = congruence_of_coefficient_matrix(
            &context,
            &transform,
            &middle,
            SymbolicaCoefficientMatrixLimits::default(),
        )
        .unwrap();
        assert_eq!(
            product,
            vec![
                vec![
                    context.coefficient_fixture("2+2*x*y+3*x^2"),
                    context.coefficient_fixture("y+3*x"),
                ],
                vec![context.coefficient_fixture("y+3*x"), context.integer(3)],
            ]
        );
        assert_eq!(stats.product_calls(), 2);
        assert_eq!(stats.transpose_calls(), 1);
        assert_eq!(stats.exact_operations(), 32);
        assert_eq!(stats.admitted_exact_operations(), 32);
        assert_eq!(stats.admitted_single_matrix_entries(), 4);
        assert_eq!(stats.admitted_peak_live_entries(), 16);

        let exact = SymbolicaCoefficientMatrixLimits {
            max_single_matrix_entries: stats.admitted_single_matrix_entries(),
            max_live_matrix_entries: stats.admitted_peak_live_entries(),
            max_exact_operations: stats.admitted_exact_operations(),
            max_input_retained_bytes: stats.input_retained_bytes(),
            max_output_retained_bytes: stats.output_retained_bytes(),
            ..SymbolicaCoefficientMatrixLimits::default()
        };
        let (_, replayed_stats) =
            congruence_of_coefficient_matrix(&context, &transform, &middle, exact).unwrap();
        assert_eq!(replayed_stats, stats);

        let one_below_output = SymbolicaCoefficientMatrixLimits {
            max_output_retained_bytes: stats.output_retained_bytes() - 1,
            ..SymbolicaCoefficientMatrixLimits::default()
        };
        assert!(matches!(
            congruence_of_coefficient_matrix(&context, &transform, &middle, one_below_output,),
            Err(SymbolicaCoefficientMatrixError::ResourceLimit {
                resource: "coefficient matrix output retained bytes",
                ..
            })
        ));

        assert!(matches!(
            congruence_of_coefficient_matrix(
                &context,
                &transform,
                &[vec![context.one()]],
                SymbolicaCoefficientMatrixLimits::default(),
            ),
            Err(SymbolicaCoefficientMatrixError::ShapeMismatch { .. })
        ));
    }
}
