//! Checked access to Symbolica's incremental sparse row reducer over `K(n)`.
//!
//! This module is a deliberately narrow algebra boundary.  Symbolica owns
//! pivot choice, forward elimination, and normalization.  RustRed validates a
//! canonical sparse input, decodes only the newest `L`/`U` rows, and transports
//! typed coefficient-limit failures across Symbolica's infallible [`Field`]
//! trait.  The returned `L` transcript is not by itself a persistent rule: a
//! database caller must replay it through the guarded provenance path before
//! committing a row.

use std::fmt;
use std::hash::{Hash, Hasher};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::sync::{Arc, Mutex, MutexGuard};

use rand::RngCore;
use symbolica::domains::{InternalOrdering, SelfRing};
use symbolica::prelude::*;
use symbolica::tensors::sparse::{LuLMode, SparseMatrix, SparseRowReducer};

use super::{ParametricCoefficient, ParametricCoefficientContext, ParametricCoefficientError};
use crate::parametric_elimination::{
    ParametricCoefficientWorkError, ParametricCoefficientWorkLedger,
    ParametricCoefficientWorkLedgerLimits, ParametricCoefficientWorkPhase,
    ParametricCoefficientWorkStats,
};

/// Resource envelope for one temporary native forward-reduction transcript.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SymbolicaParametricSparseLimits {
    pub(crate) coefficient_work: ParametricCoefficientWorkLedgerLimits,
    pub(crate) max_rows: usize,
    pub(crate) max_physical_columns: usize,
    pub(crate) max_input_entries: usize,
    /// Conservative U+L entry envelope admitted before native execution and
    /// rechecked against the observed output afterward.
    pub(crate) max_native_output_entry_envelope: usize,
    pub(crate) max_returned_trace_entries: usize,
}

impl Default for SymbolicaParametricSparseLimits {
    fn default() -> Self {
        Self {
            coefficient_work: ParametricCoefficientWorkLedgerLimits::default(),
            max_rows: 16_000_000,
            max_physical_columns: 16_000_000,
            max_input_entries: 1_000_000_000,
            max_native_output_entry_envelope: 1_000_000_000,
            max_returned_trace_entries: 16_000_000,
        }
    }
}

/// One canonical nonzero sparse-row entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SymbolicaParametricSparseEntry {
    column: usize,
    coefficient: ParametricCoefficient,
}

impl SymbolicaParametricSparseEntry {
    pub(crate) const fn new(column: usize, coefficient: ParametricCoefficient) -> Self {
        Self {
            column,
            coefficient,
        }
    }

    pub(crate) const fn column(&self) -> usize {
        self.column
    }

    pub(crate) const fn coefficient(&self) -> &ParametricCoefficient {
        &self.coefficient
    }
}

/// One row whose entries must be strictly increasing in native column order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct SymbolicaParametricSparseRow {
    entries: Vec<SymbolicaParametricSparseEntry>,
}

impl SymbolicaParametricSparseRow {
    pub(crate) const fn new(entries: Vec<SymbolicaParametricSparseEntry>) -> Self {
        Self { entries }
    }

    pub(crate) fn entries(&self) -> &[SymbolicaParametricSparseEntry] {
        &self.entries
    }

    pub(crate) const fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn try_as_input(
        &self,
    ) -> Result<SymbolicaParametricSparseInputRow<'_>, SymbolicaParametricSparseError> {
        let mut entries = Vec::new();
        entries.try_reserve_exact(self.entries.len()).map_err(|_| {
            SymbolicaParametricSparseError::AllocationFailure {
                resource: "borrowed Symbolica parametric sparse input entries",
            }
        })?;
        entries.extend(self.entries.iter().map(|entry| {
            SymbolicaParametricSparseInputEntry::new(entry.column, &entry.coefficient)
        }));
        Ok(SymbolicaParametricSparseInputRow::new(entries))
    }
}

/// Borrowed input metadata for one coefficient already retained by a caller.
/// The adapter performs the only fallible, work-charged deep coefficient copy
/// when it crosses into Symbolica's native element representation.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SymbolicaParametricSparseInputEntry<'coefficient> {
    column: usize,
    coefficient: &'coefficient ParametricCoefficient,
}

impl<'coefficient> SymbolicaParametricSparseInputEntry<'coefficient> {
    pub(crate) const fn new(
        column: usize,
        coefficient: &'coefficient ParametricCoefficient,
    ) -> Self {
        Self {
            column,
            coefficient,
        }
    }

    pub(crate) const fn column(self) -> usize {
        self.column
    }

    pub(crate) const fn coefficient(self) -> &'coefficient ParametricCoefficient {
        self.coefficient
    }
}

#[derive(Debug, Default)]
pub(crate) struct SymbolicaParametricSparseInputRow<'coefficient> {
    entries: Vec<SymbolicaParametricSparseInputEntry<'coefficient>>,
}

impl<'coefficient> SymbolicaParametricSparseInputRow<'coefficient> {
    pub(crate) const fn new(
        entries: Vec<SymbolicaParametricSparseInputEntry<'coefficient>>,
    ) -> Self {
        Self { entries }
    }

    pub(crate) fn entries(&self) -> &[SymbolicaParametricSparseInputEntry<'coefficient>] {
        &self.entries
    }

    pub(crate) const fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// One factor `c` in `row <- row - c * U[pivot_row]` selected by Symbolica.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct SymbolicaParametricSparseReduction {
    pivot_row: usize,
    factor: ParametricCoefficient,
}

impl SymbolicaParametricSparseReduction {
    pub(crate) const fn pivot_row(&self) -> usize {
        self.pivot_row
    }

    pub(crate) const fn factor(&self) -> &ParametricCoefficient {
        &self.factor
    }
}

/// Exact census of a temporary native reducer reconstruction. Coefficient work
/// includes checked input/output copies and native field callbacks; it excludes
/// pre-native structural validation and any caller-side differential replay.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct SymbolicaParametricSparseStats {
    prior_rows: usize,
    rows: usize,
    physical_columns: usize,
    input_entries: usize,
    prospective_native_output_entries: usize,
    observed_native_output_entries: usize,
    native_u_entries: usize,
    native_l_entries: usize,
    returned_trace_entries: usize,
    coefficient_work: ParametricCoefficientWorkStats,
}

impl SymbolicaParametricSparseStats {
    pub(crate) const fn prior_rows(self) -> usize {
        self.prior_rows
    }

    pub(crate) const fn rows(self) -> usize {
        self.rows
    }

    pub(crate) const fn physical_columns(self) -> usize {
        self.physical_columns
    }

    pub(crate) const fn input_entries(self) -> usize {
        self.input_entries
    }

    pub(crate) const fn prospective_native_output_entries(self) -> usize {
        self.prospective_native_output_entries
    }

    pub(crate) const fn observed_native_output_entries(self) -> usize {
        self.observed_native_output_entries
    }

    pub(crate) const fn native_u_entries(self) -> usize {
        self.native_u_entries
    }

    pub(crate) const fn native_l_entries(self) -> usize {
        self.native_l_entries
    }

    pub(crate) const fn returned_trace_entries(self) -> usize {
        self.returned_trace_entries
    }

    pub(crate) const fn coefficient_work(self) -> ParametricCoefficientWorkStats {
        self.coefficient_work
    }
}

/// Native disposition and the complete last-row transcript.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum SymbolicaParametricSparseOutcome {
    Dependent {
        reductions: Vec<SymbolicaParametricSparseReduction>,
        canonical_zero_input: bool,
        stats: SymbolicaParametricSparseStats,
    },
    Independent {
        pivot_column: usize,
        normalized_row: SymbolicaParametricSparseRow,
        reductions: Vec<SymbolicaParametricSparseReduction>,
        normalization_divisor: ParametricCoefficient,
        stats: SymbolicaParametricSparseStats,
    },
}

impl SymbolicaParametricSparseOutcome {
    pub(crate) fn reductions(&self) -> &[SymbolicaParametricSparseReduction] {
        match self {
            Self::Dependent { reductions, .. } | Self::Independent { reductions, .. } => reductions,
        }
    }

    pub(crate) const fn stats(&self) -> SymbolicaParametricSparseStats {
        match self {
            Self::Dependent { stats, .. } | Self::Independent { stats, .. } => *stats,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum SymbolicaParametricSparseError {
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    AllocationFailure {
        resource: &'static str,
    },
    DimensionOverflow,
    EmptyPriorRow {
        row: usize,
    },
    ColumnOutOfRange {
        row: usize,
        column: usize,
        physical_columns: usize,
    },
    NonIncreasingColumns {
        row: usize,
        previous: usize,
        current: usize,
    },
    ExplicitZero {
        row: usize,
        column: usize,
    },
    Coefficient(ParametricCoefficientError),
    CoefficientWork(ParametricCoefficientWorkError),
    DependentPriorRow {
        row: usize,
    },
    PriorRowReplayMismatch {
        row: usize,
    },
    NativePanic {
        operation: &'static str,
    },
    UnexpectedFieldOperation {
        operation: &'static str,
    },
    NativeTranscriptMismatch {
        operation: &'static str,
    },
}

impl fmt::Display for SymbolicaParametricSparseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} needs {requested} units, exceeding the configured limit {limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::AllocationFailure { resource } => {
                write!(formatter, "failed to reserve {resource}")
            }
            Self::DimensionOverflow => formatter.write_str(
                "parametric sparse reducer dimensions exceed Symbolica's u32 representation",
            ),
            Self::EmptyPriorRow { row } => write!(formatter, "prior sparse row {row} is empty"),
            Self::ColumnOutOfRange {
                row,
                column,
                physical_columns,
            } => write!(
                formatter,
                "sparse row {row} column {column} is outside {physical_columns} physical columns"
            ),
            Self::NonIncreasingColumns {
                row,
                previous,
                current,
            } => write!(
                formatter,
                "sparse row {row} columns are not strictly increasing at {previous}, {current}"
            ),
            Self::ExplicitZero { row, column } => {
                write!(
                    formatter,
                    "sparse row {row} stores an explicit zero at column {column}"
                )
            }
            Self::Coefficient(error) => error.fmt(formatter),
            Self::CoefficientWork(error) => error.fmt(formatter),
            Self::DependentPriorRow { row } => {
                write!(formatter, "prior sparse row {row} is linearly dependent")
            }
            Self::PriorRowReplayMismatch { row } => {
                write!(
                    formatter,
                    "prior sparse row {row} did not replay verbatim into U"
                )
            }
            Self::NativePanic { operation } => {
                write!(formatter, "Symbolica panicked during sparse {operation}")
            }
            Self::UnexpectedFieldOperation { operation } => write!(
                formatter,
                "Symbolica sparse forward reduction unexpectedly requested field {operation}"
            ),
            Self::NativeTranscriptMismatch { operation } => {
                write!(
                    formatter,
                    "Symbolica returned an invalid sparse {operation} transcript"
                )
            }
        }
    }
}

impl std::error::Error for SymbolicaParametricSparseError {}

/// Shallow native element used by Symbolica's dense scratch vector.
///
/// A plain `ParametricCoefficient` would deep-clone its sparse numerator and
/// denominator once per physical column when `SparseRowReducer` initializes
/// scratch.  The private `Arc` keeps those structural clones shallow; every
/// newly computed coefficient is still an immutable, authenticated Symbolica
/// value.
#[derive(Clone, Debug)]
struct NativeParametricElement(Arc<ParametricCoefficient>);

impl PartialEq for NativeParametricElement {
    fn eq(&self, other: &Self) -> bool {
        self.0.as_ref() == other.0.as_ref()
    }
}

impl Eq for NativeParametricElement {}

impl Hash for NativeParametricElement {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.context.hash(state);
        self.0.raw.hash(state);
    }
}

impl InternalOrdering for NativeParametricElement {
    fn internal_cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.0.context.cmp(&other.0.context) {
            std::cmp::Ordering::Equal => self.0.raw.internal_cmp(&other.0.raw),
            ordering => ordering,
        }
    }
}

enum CheckedFieldFailure {
    Work(ParametricCoefficientWorkError),
    UnexpectedOperation(&'static str),
}

struct CheckedFieldAbort(CheckedFieldFailure);

#[cold]
fn abort_checked_field(error: CheckedFieldFailure) -> ! {
    resume_unwind(Box::new(CheckedFieldAbort(error)))
}

/// Shared, panic-recoverable work controller for every clone of one native
/// field. A retained reducer and any clone-on-stage trial therefore see one
/// serialized active ledger without borrowing the caller's context.
struct CheckedFieldController {
    stage_gate: Mutex<()>,
    active_ledger: Mutex<Option<ParametricCoefficientWorkLedger>>,
}

#[derive(Clone)]
struct CheckedParametricField {
    context: Arc<ParametricCoefficientContext>,
    inner: RationalPolynomialField<IntegerRing, u16>,
    zero: NativeParametricElement,
    one: NativeParametricElement,
    controller: Arc<CheckedFieldController>,
}

/// Exclusive lifetime of one native reducer operation sequence. Dropping the
/// guard clears the active work ledger on success, typed field abort, or an
/// unrelated Symbolica panic. Poisoned standard-library mutexes are recovered
/// deliberately: poisoning records that a panic crossed a stage, while this
/// guard supplies the stronger invariant that no partial ledger survives it.
struct CheckedParametricFieldStage<'controller> {
    controller: &'controller CheckedFieldController,
    _gate: MutexGuard<'controller, ()>,
}

impl Drop for CheckedParametricFieldStage<'_> {
    fn drop(&mut self) {
        *lock_recovering_poison(&self.controller.active_ledger) = None;
    }
}

fn lock_recovering_poison<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            mutex.clear_poison();
            poisoned.into_inner()
        }
    }
}

impl fmt::Debug for CheckedParametricField {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CheckedParametricField")
            .field("context", &self.context.fingerprint())
            .finish_non_exhaustive()
    }
}

impl fmt::Display for CheckedParametricField {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("checked RustRed parametric field")
    }
}

impl PartialEq for CheckedParametricField {
    fn eq(&self, other: &Self) -> bool {
        self.context.fingerprint() == other.context.fingerprint()
    }
}

impl Eq for CheckedParametricField {}

impl Hash for CheckedParametricField {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.context.fingerprint().hash(state);
    }
}

impl CheckedParametricField {
    fn new(context: Arc<ParametricCoefficientContext>) -> Self {
        let zero = NativeParametricElement(Arc::new(context.zero()));
        let one = NativeParametricElement(Arc::new(context.one()));
        Self {
            context,
            inner: RationalPolynomialField::new(Z),
            zero,
            one,
            controller: Arc::new(CheckedFieldController {
                stage_gate: Mutex::new(()),
                active_ledger: Mutex::new(None),
            }),
        }
    }

    fn begin_stage(
        &self,
        limits: ParametricCoefficientWorkLedgerLimits,
    ) -> CheckedParametricFieldStage<'_> {
        let controller = self.controller.as_ref();
        let gate = lock_recovering_poison(&controller.stage_gate);
        let mut active = lock_recovering_poison(&controller.active_ledger);
        debug_assert!(active.is_none(), "the stage gate permits one active ledger");
        *active = Some(ParametricCoefficientWorkLedger::new(
            ParametricCoefficientWorkPhase::Construction,
            limits,
        ));
        drop(active);
        CheckedParametricFieldStage {
            controller,
            _gate: gate,
        }
    }

    fn stats(&self) -> ParametricCoefficientWorkStats {
        let active = lock_recovering_poison(&self.controller.active_ledger);
        let Some(ledger) = active.as_ref() else {
            drop(active);
            self.unexpected("work-ledger access outside an active stage")
        };
        ledger.stats()
    }

    fn with_active_ledger<T>(
        &self,
        operation: impl FnOnce(
            &mut ParametricCoefficientWorkLedger,
        ) -> Result<T, ParametricCoefficientWorkError>,
    ) -> Result<T, ParametricCoefficientWorkError> {
        let mut active = lock_recovering_poison(&self.controller.active_ledger);
        if active.is_none() {
            drop(active);
            self.unexpected("coefficient operation outside an active stage")
        }
        operation(
            active
                .as_mut()
                .expect("the active ledger was checked immediately above"),
        )
    }

    fn finish_work(
        &self,
        result: Result<ParametricCoefficient, ParametricCoefficientWorkError>,
    ) -> NativeParametricElement {
        match result {
            Ok(value) => NativeParametricElement(Arc::new(value)),
            Err(error) => abort_checked_field(CheckedFieldFailure::Work(error)),
        }
    }

    fn copy_authenticated(&self, value: &ParametricCoefficient) -> NativeParametricElement {
        let result = self.with_active_ledger(|ledger| ledger.try_copy_authenticated(value));
        self.finish_work(result)
    }

    fn add_checked(
        &self,
        left: &NativeParametricElement,
        right: &NativeParametricElement,
    ) -> NativeParametricElement {
        let result = self
            .with_active_ledger(|ledger| ledger.try_add(self.context.as_ref(), &left.0, &right.0));
        self.finish_work(result)
    }

    fn sub_checked(
        &self,
        left: &NativeParametricElement,
        right: &NativeParametricElement,
    ) -> NativeParametricElement {
        let result = self
            .with_active_ledger(|ledger| ledger.try_sub(self.context.as_ref(), &left.0, &right.0));
        self.finish_work(result)
    }

    fn mul_checked(
        &self,
        left: &NativeParametricElement,
        right: &NativeParametricElement,
    ) -> NativeParametricElement {
        let result = self
            .with_active_ledger(|ledger| ledger.try_mul(self.context.as_ref(), &left.0, &right.0));
        self.finish_work(result)
    }

    fn neg_checked(&self, value: &NativeParametricElement) -> NativeParametricElement {
        let result =
            self.with_active_ledger(|ledger| ledger.try_neg(self.context.as_ref(), &value.0));
        self.finish_work(result)
    }

    fn div_checked(
        &self,
        numerator: &NativeParametricElement,
        denominator: &NativeParametricElement,
    ) -> NativeParametricElement {
        let result = self.with_active_ledger(|ledger| {
            ledger.try_native_field_division(self.context.as_ref(), &numerator.0, &denominator.0)
        });
        self.finish_work(result)
    }

    fn unexpected(&self, operation: &'static str) -> ! {
        abort_checked_field(CheckedFieldFailure::UnexpectedOperation(operation))
    }
}

impl Set for CheckedParametricField {
    type Element = NativeParametricElement;

    fn size(&self) -> Option<Integer> {
        None
    }
}

impl RingOps<NativeParametricElement> for CheckedParametricField {
    fn add(
        &self,
        left: NativeParametricElement,
        right: NativeParametricElement,
    ) -> NativeParametricElement {
        self.add_checked(&left, &right)
    }

    fn sub(
        &self,
        left: NativeParametricElement,
        right: NativeParametricElement,
    ) -> NativeParametricElement {
        self.sub_checked(&left, &right)
    }

    fn mul(
        &self,
        left: NativeParametricElement,
        right: NativeParametricElement,
    ) -> NativeParametricElement {
        self.mul_checked(&left, &right)
    }

    fn neg(&self, value: NativeParametricElement) -> NativeParametricElement {
        self.neg_checked(&value)
    }

    fn add_assign(&self, left: &mut NativeParametricElement, right: NativeParametricElement) {
        *left = self.add_checked(left, &right);
    }

    fn sub_assign(&self, left: &mut NativeParametricElement, right: NativeParametricElement) {
        *left = self.sub_checked(left, &right);
    }

    fn mul_assign(&self, left: &mut NativeParametricElement, right: NativeParametricElement) {
        *left = self.mul_checked(left, &right);
    }

    fn add_mul_assign(
        &self,
        accumulator: &mut NativeParametricElement,
        left: NativeParametricElement,
        right: NativeParametricElement,
    ) {
        let product = self.mul_checked(&left, &right);
        *accumulator = self.add_checked(accumulator, &product);
    }

    fn sub_mul_assign(
        &self,
        accumulator: &mut NativeParametricElement,
        left: NativeParametricElement,
        right: NativeParametricElement,
    ) {
        let product = self.mul_checked(&left, &right);
        *accumulator = self.sub_checked(accumulator, &product);
    }
}

impl RingOps<&NativeParametricElement> for CheckedParametricField {
    fn add(
        &self,
        left: &NativeParametricElement,
        right: &NativeParametricElement,
    ) -> NativeParametricElement {
        self.add_checked(left, right)
    }

    fn sub(
        &self,
        left: &NativeParametricElement,
        right: &NativeParametricElement,
    ) -> NativeParametricElement {
        self.sub_checked(left, right)
    }

    fn mul(
        &self,
        left: &NativeParametricElement,
        right: &NativeParametricElement,
    ) -> NativeParametricElement {
        self.mul_checked(left, right)
    }

    fn neg(&self, value: &NativeParametricElement) -> NativeParametricElement {
        self.neg_checked(value)
    }

    fn add_assign(&self, left: &mut NativeParametricElement, right: &NativeParametricElement) {
        *left = self.add_checked(left, right);
    }

    fn sub_assign(&self, left: &mut NativeParametricElement, right: &NativeParametricElement) {
        *left = self.sub_checked(left, right);
    }

    fn mul_assign(&self, left: &mut NativeParametricElement, right: &NativeParametricElement) {
        *left = self.mul_checked(left, right);
    }

    fn add_mul_assign(
        &self,
        accumulator: &mut NativeParametricElement,
        left: &NativeParametricElement,
        right: &NativeParametricElement,
    ) {
        let product = self.mul_checked(left, right);
        *accumulator = self.add_checked(accumulator, &product);
    }

    fn sub_mul_assign(
        &self,
        accumulator: &mut NativeParametricElement,
        left: &NativeParametricElement,
        right: &NativeParametricElement,
    ) {
        let product = self.mul_checked(left, right);
        *accumulator = self.sub_checked(accumulator, &product);
    }
}

impl Ring for CheckedParametricField {
    fn zero(&self) -> NativeParametricElement {
        self.zero.clone()
    }

    fn one(&self) -> NativeParametricElement {
        self.one.clone()
    }

    fn nth(&self, _value: Integer) -> NativeParametricElement {
        self.unexpected("nth")
    }

    fn pow(&self, _base: &NativeParametricElement, _exponent: u64) -> NativeParametricElement {
        self.unexpected("pow")
    }

    fn is_zero(&self, value: &NativeParametricElement) -> bool {
        value.0.is_zero()
    }

    fn is_one(&self, value: &NativeParametricElement) -> bool {
        value.0.raw.is_one()
    }

    fn one_is_gcd_unit() -> bool {
        <RationalPolynomialField<IntegerRing, u16> as Ring>::one_is_gcd_unit()
    }

    fn characteristic(&self) -> Integer {
        self.inner.characteristic()
    }

    fn try_inv(&self, value: &NativeParametricElement) -> Option<NativeParametricElement> {
        if self.is_zero(value) {
            None
        } else {
            Some(self.div_checked(&self.one, value))
        }
    }

    fn try_div(
        &self,
        numerator: &NativeParametricElement,
        denominator: &NativeParametricElement,
    ) -> Option<NativeParametricElement> {
        if self.is_zero(denominator) {
            None
        } else {
            Some(self.div_checked(numerator, denominator))
        }
    }

    fn sample(&self, _rng: &mut impl RngCore, _range: (i64, i64)) -> NativeParametricElement {
        self.unexpected("sample")
    }

    fn format<W: fmt::Write>(
        &self,
        element: &NativeParametricElement,
        options: &PrintOptions,
        state: PrintState,
        formatter: &mut W,
    ) -> Result<bool, fmt::Error> {
        self.inner.format(&element.0.raw, options, state, formatter)
    }

    fn has_independent_elements(&self) -> bool {
        true
    }
}

impl EuclideanDomain for CheckedParametricField {
    fn rem(
        &self,
        _left: &NativeParametricElement,
        _right: &NativeParametricElement,
    ) -> NativeParametricElement {
        self.unexpected("rem")
    }

    fn quot_rem(
        &self,
        _numerator: &NativeParametricElement,
        _denominator: &NativeParametricElement,
    ) -> (NativeParametricElement, NativeParametricElement) {
        self.unexpected("quot_rem")
    }

    fn gcd(
        &self,
        _left: &NativeParametricElement,
        _right: &NativeParametricElement,
    ) -> NativeParametricElement {
        self.unexpected("gcd")
    }
}

impl Field for CheckedParametricField {
    fn div(
        &self,
        numerator: &NativeParametricElement,
        denominator: &NativeParametricElement,
    ) -> NativeParametricElement {
        self.div_checked(numerator, denominator)
    }

    fn div_assign(
        &self,
        numerator: &mut NativeParametricElement,
        denominator: &NativeParametricElement,
    ) {
        *numerator = self.div_checked(numerator, denominator);
    }

    fn inv(&self, value: &NativeParametricElement) -> NativeParametricElement {
        self.div_checked(&self.one, value)
    }
}

/// Rebuild prior normalized rows and forward-reduce one candidate through
/// Symbolica.  `physical_columns` excludes the permanently unused sentinel
/// column added internally by this function.
pub(crate) fn forward_reduce_last_row(
    context: &ParametricCoefficientContext,
    physical_columns: usize,
    prior_rows: &[SymbolicaParametricSparseInputRow<'_>],
    candidate: &SymbolicaParametricSparseInputRow<'_>,
    limits: SymbolicaParametricSparseLimits,
) -> Result<SymbolicaParametricSparseOutcome, SymbolicaParametricSparseError> {
    check_limit(
        "Symbolica parametric sparse physical columns",
        physical_columns,
        limits.max_physical_columns,
    )?;
    let native_columns = physical_columns.checked_add(1).ok_or(
        SymbolicaParametricSparseError::ResourceCountOverflow {
            resource: "Symbolica parametric sparse columns including sentinel",
        },
    )?;
    let native_columns_u32 = u32::try_from(native_columns)
        .map_err(|_| SymbolicaParametricSparseError::DimensionOverflow)?;
    let rows = prior_rows.len().checked_add(1).ok_or(
        SymbolicaParametricSparseError::ResourceCountOverflow {
            resource: "Symbolica parametric sparse rows",
        },
    )?;
    u32::try_from(rows).map_err(|_| SymbolicaParametricSparseError::DimensionOverflow)?;
    check_limit("Symbolica parametric sparse rows", rows, limits.max_rows)?;

    let mut input_entries = candidate.entries.len();
    for row in prior_rows {
        input_entries = input_entries.checked_add(row.entries.len()).ok_or(
            SymbolicaParametricSparseError::ResourceCountOverflow {
                resource: "Symbolica parametric sparse input entries",
            },
        )?;
    }
    check_limit(
        "Symbolica parametric sparse input entries",
        input_entries,
        limits.max_input_entries,
    )?;
    // Accepted prior rows replay verbatim: their U payload is exactly their
    // input support and each contributes one L diagonal. The candidate can
    // add at most one dense physical U row plus one factor per prior row and
    // one diagonal. Admit this complete accepted-stage envelope before native
    // allocation. A malformed prior row is rejected at its first mismatch,
    // still within the same envelope.
    let prior_input_entries = input_entries.checked_sub(candidate.entries.len()).ok_or(
        SymbolicaParametricSparseError::ResourceCountOverflow {
            resource: "prospective Symbolica parametric sparse native output entries",
        },
    )?;
    let prospective_native_output_entries = prior_input_entries
        .checked_add(physical_columns)
        .and_then(|value| value.checked_add(prior_rows.len().checked_mul(2)?))
        .and_then(|value| value.checked_add(1))
        .ok_or(SymbolicaParametricSparseError::ResourceCountOverflow {
            resource: "prospective Symbolica parametric sparse native output entries",
        })?;
    check_limit(
        "prospective Symbolica parametric sparse native output entries",
        prospective_native_output_entries,
        limits.max_native_output_entry_envelope,
    )?;
    for (row_ordinal, row) in prior_rows.iter().enumerate() {
        if row.is_empty() {
            return Err(SymbolicaParametricSparseError::EmptyPriorRow { row: row_ordinal });
        }
        validate_row(
            context,
            physical_columns,
            row_ordinal,
            row,
            limits.coefficient_work.arithmetic.exact_algebra,
        )?;
    }
    validate_row(
        context,
        physical_columns,
        prior_rows.len(),
        candidate,
        limits.coefficient_work.arithmetic.exact_algebra,
    )?;

    let field = CheckedParametricField::new(Arc::new(context.clone()));
    let _field_stage = field.begin_stage(limits.coefficient_work);
    let mut reducer = call_native("reducer construction", || {
        SparseRowReducer::new(native_columns_u32, field.clone(), LuLMode::Full)
    })?;

    for (row_ordinal, row) in prior_rows.iter().enumerate() {
        let (values, columns) =
            call_native("prior-row input copy", || copy_input_row(&field, row))??;
        let u_rows_before = reducer.u().nrows();
        let pivot = call_native("prior-row forward reduction", || {
            reducer.add_row(&values, &columns)
        })?;
        let Some(_) = pivot else {
            return Err(SymbolicaParametricSparseError::DependentPriorRow { row: row_ordinal });
        };
        if reducer.u().nrows() != u_rows_before + 1
            || !native_row_matches(reducer.u(), u_rows_before as usize, row)
        {
            return Err(SymbolicaParametricSparseError::PriorRowReplayMismatch {
                row: row_ordinal,
            });
        }
    }

    let u_rows_before = reducer.u().nrows() as usize;
    let l_rows_before = reducer.l().nrows() as usize;
    if u_rows_before != prior_rows.len() || l_rows_before != prior_rows.len() {
        return Err(SymbolicaParametricSparseError::NativeTranscriptMismatch {
            operation: "prior-row dimensions",
        });
    }

    let (candidate_values, candidate_columns) =
        call_native("candidate input copy", || copy_input_row(&field, candidate))??;
    let pivot_column = call_native("candidate forward reduction", || {
        reducer.add_row(&candidate_values, &candidate_columns)
    })?;

    if reducer
        .pivots()
        .get(physical_columns)
        .copied()
        .flatten()
        .is_some()
    {
        return Err(SymbolicaParametricSparseError::NativeTranscriptMismatch {
            operation: "sentinel-column preservation",
        });
    }

    let native_output_entries = reducer
        .u()
        .nvalues()
        .checked_add(reducer.l().nvalues())
        .ok_or(SymbolicaParametricSparseError::ResourceCountOverflow {
            resource: "Symbolica parametric sparse native output entries",
        })?;
    check_limit(
        "Symbolica parametric sparse native output entries",
        native_output_entries,
        limits.max_native_output_entry_envelope,
    )?;

    if candidate.is_empty() {
        if pivot_column.is_some()
            || reducer.u().nrows() as usize != u_rows_before
            || reducer.l().nrows() as usize != l_rows_before
        {
            return Err(SymbolicaParametricSparseError::NativeTranscriptMismatch {
                operation: "canonical-zero candidate",
            });
        }
        let stats = sparse_stats(
            &field,
            prior_rows.len(),
            rows,
            physical_columns,
            input_entries,
            prospective_native_output_entries,
            native_output_entries,
            &reducer,
            0,
        );
        return Ok(SymbolicaParametricSparseOutcome::Dependent {
            reductions: Vec::new(),
            canonical_zero_input: true,
            stats,
        });
    }

    if reducer.l().nrows() as usize != l_rows_before + 1 {
        return Err(SymbolicaParametricSparseError::NativeTranscriptMismatch {
            operation: "candidate L row",
        });
    }
    match pivot_column {
        None => {
            if reducer.u().nrows() as usize != u_rows_before {
                return Err(SymbolicaParametricSparseError::NativeTranscriptMismatch {
                    operation: "dependent candidate U dimensions",
                });
            }
            let returned_trace_entries = native_row_len(reducer.l(), l_rows_before)?;
            check_limit(
                "Symbolica parametric sparse returned trace entries",
                returned_trace_entries,
                limits.max_returned_trace_entries,
            )?;
            let l_row = call_native("candidate L extraction", || {
                copy_native_row(&field, reducer.l(), l_rows_before)
            })??;
            let reductions = decode_reductions(l_row, u_rows_before)?;
            if reductions.len() != returned_trace_entries {
                return Err(SymbolicaParametricSparseError::NativeTranscriptMismatch {
                    operation: "dependent returned trace length",
                });
            }
            let stats = sparse_stats(
                &field,
                prior_rows.len(),
                rows,
                physical_columns,
                input_entries,
                prospective_native_output_entries,
                native_output_entries,
                &reducer,
                returned_trace_entries,
            );
            Ok(SymbolicaParametricSparseOutcome::Dependent {
                reductions,
                canonical_zero_input: false,
                stats,
            })
        }
        Some(pivot_column) => {
            let pivot_column = pivot_column as usize;
            if pivot_column >= physical_columns
                || reducer.u().nrows() as usize != u_rows_before + 1
                || reducer.pivots().get(pivot_column).copied().flatten()
                    != Some(u_rows_before as u32)
            {
                return Err(SymbolicaParametricSparseError::NativeTranscriptMismatch {
                    operation: "independent candidate pivot",
                });
            }
            let l_trace_entries = native_row_len(reducer.l(), l_rows_before)?;
            let u_trace_entries = native_row_len(reducer.u(), u_rows_before)?;
            let returned_trace_entries = l_trace_entries.checked_add(u_trace_entries).ok_or(
                SymbolicaParametricSparseError::ResourceCountOverflow {
                    resource: "Symbolica parametric sparse returned trace entries",
                },
            )?;
            check_limit(
                "Symbolica parametric sparse returned trace entries",
                returned_trace_entries,
                limits.max_returned_trace_entries,
            )?;
            let mut l_row = call_native("candidate L extraction", || {
                copy_native_row(&field, reducer.l(), l_rows_before)
            })??;
            let Some(diagonal) = l_row.pop() else {
                return Err(SymbolicaParametricSparseError::NativeTranscriptMismatch {
                    operation: "independent candidate L diagonal",
                });
            };
            if diagonal.column != u_rows_before || diagonal.coefficient.is_zero() {
                return Err(SymbolicaParametricSparseError::NativeTranscriptMismatch {
                    operation: "independent candidate L diagonal",
                });
            }
            let normalization_divisor = diagonal.coefficient;
            let reductions = decode_reductions(l_row, u_rows_before)?;
            let normalized_row =
                SymbolicaParametricSparseRow::new(call_native("candidate U extraction", || {
                    copy_native_row(&field, reducer.u(), u_rows_before)
                })??);
            if normalized_row.entries.first().map(|entry| entry.column) != Some(pivot_column)
                || normalized_row
                    .entries
                    .first()
                    .is_none_or(|entry| !entry.coefficient.raw.is_one())
                || normalized_row
                    .entries
                    .iter()
                    .any(|entry| entry.column >= physical_columns)
            {
                return Err(SymbolicaParametricSparseError::NativeTranscriptMismatch {
                    operation: "independent normalized U row",
                });
            }
            if reductions
                .len()
                .checked_add(normalized_row.entries.len())
                .and_then(|value| value.checked_add(1))
                != Some(returned_trace_entries)
            {
                return Err(SymbolicaParametricSparseError::NativeTranscriptMismatch {
                    operation: "independent returned trace length",
                });
            }
            let stats = sparse_stats(
                &field,
                prior_rows.len(),
                rows,
                physical_columns,
                input_entries,
                prospective_native_output_entries,
                native_output_entries,
                &reducer,
                returned_trace_entries,
            );
            Ok(SymbolicaParametricSparseOutcome::Independent {
                pivot_column,
                normalized_row,
                reductions,
                normalization_divisor,
                stats,
            })
        }
    }
}

fn validate_row(
    context: &ParametricCoefficientContext,
    physical_columns: usize,
    row_ordinal: usize,
    row: &SymbolicaParametricSparseInputRow<'_>,
    exact_algebra: crate::ExactAlgebraLimits,
) -> Result<(), SymbolicaParametricSparseError> {
    let mut previous = None;
    for entry in &row.entries {
        if entry.column >= physical_columns {
            return Err(SymbolicaParametricSparseError::ColumnOutOfRange {
                row: row_ordinal,
                column: entry.column,
                physical_columns,
            });
        }
        if let Some(previous) = previous {
            if entry.column <= previous {
                return Err(SymbolicaParametricSparseError::NonIncreasingColumns {
                    row: row_ordinal,
                    previous,
                    current: entry.column,
                });
            }
        }
        context
            .validate_with_limits(entry.coefficient, exact_algebra)
            .map_err(SymbolicaParametricSparseError::Coefficient)?;
        if entry.coefficient.is_zero() {
            return Err(SymbolicaParametricSparseError::ExplicitZero {
                row: row_ordinal,
                column: entry.column,
            });
        }
        previous = Some(entry.column);
    }
    Ok(())
}

fn copy_input_row(
    field: &CheckedParametricField,
    row: &SymbolicaParametricSparseInputRow<'_>,
) -> Result<(Vec<NativeParametricElement>, Vec<u32>), SymbolicaParametricSparseError> {
    let mut values = Vec::new();
    let mut columns = Vec::new();
    values.try_reserve_exact(row.entries.len()).map_err(|_| {
        SymbolicaParametricSparseError::AllocationFailure {
            resource: "Symbolica parametric sparse input values",
        }
    })?;
    columns.try_reserve_exact(row.entries.len()).map_err(|_| {
        SymbolicaParametricSparseError::AllocationFailure {
            resource: "Symbolica parametric sparse input columns",
        }
    })?;
    for entry in &row.entries {
        values.push(field.copy_authenticated(entry.coefficient));
        columns.push(
            u32::try_from(entry.column)
                .map_err(|_| SymbolicaParametricSparseError::DimensionOverflow)?,
        );
    }
    Ok((values, columns))
}

fn native_row_len(
    matrix: &SparseMatrix<CheckedParametricField>,
    row_ordinal: usize,
) -> Result<usize, SymbolicaParametricSparseError> {
    let Some((&start, &end)) = matrix
        .row_ptrs()
        .get(row_ordinal)
        .zip(matrix.row_ptrs().get(row_ordinal + 1))
    else {
        return Err(SymbolicaParametricSparseError::NativeTranscriptMismatch {
            operation: "native row bounds",
        });
    };
    end.checked_sub(start)
        .ok_or(SymbolicaParametricSparseError::NativeTranscriptMismatch {
            operation: "native row pointer order",
        })
}

fn native_row_matches(
    matrix: &SparseMatrix<CheckedParametricField>,
    row_ordinal: usize,
    expected: &SymbolicaParametricSparseInputRow<'_>,
) -> bool {
    let Some((&start, &end)) = matrix
        .row_ptrs()
        .get(row_ordinal)
        .zip(matrix.row_ptrs().get(row_ordinal + 1))
    else {
        return false;
    };
    let Some(columns) = matrix.col_idcs().get(start..end) else {
        return false;
    };
    let Some(values) = matrix.values().get(start..end) else {
        return false;
    };
    columns.len() == expected.entries.len()
        && columns
            .iter()
            .zip(values)
            .zip(&expected.entries)
            .all(|((&column, value), expected)| {
                column as usize == expected.column && value.0.as_ref() == expected.coefficient
            })
}

fn copy_native_row(
    field: &CheckedParametricField,
    matrix: &SparseMatrix<CheckedParametricField>,
    row_ordinal: usize,
) -> Result<Vec<SymbolicaParametricSparseEntry>, SymbolicaParametricSparseError> {
    let Some((&start, &end)) = matrix
        .row_ptrs()
        .get(row_ordinal)
        .zip(matrix.row_ptrs().get(row_ordinal + 1))
    else {
        return Err(SymbolicaParametricSparseError::NativeTranscriptMismatch {
            operation: "native row bounds",
        });
    };
    let mut output = Vec::new();
    output
        .try_reserve_exact(end.saturating_sub(start))
        .map_err(|_| SymbolicaParametricSparseError::AllocationFailure {
            resource: "Symbolica parametric sparse returned row",
        })?;
    for (&column, value) in matrix.col_idcs()[start..end]
        .iter()
        .zip(&matrix.values()[start..end])
    {
        let copy = field.copy_authenticated(&value.0);
        let coefficient = Arc::try_unwrap(copy.0).map_err(|_| {
            SymbolicaParametricSparseError::NativeTranscriptMismatch {
                operation: "returned coefficient ownership",
            }
        })?;
        output.push(SymbolicaParametricSparseEntry::new(
            column as usize,
            coefficient,
        ));
    }
    Ok(output)
}

fn decode_reductions(
    entries: Vec<SymbolicaParametricSparseEntry>,
    prior_rows: usize,
) -> Result<Vec<SymbolicaParametricSparseReduction>, SymbolicaParametricSparseError> {
    let mut reductions = Vec::new();
    reductions.try_reserve_exact(entries.len()).map_err(|_| {
        SymbolicaParametricSparseError::AllocationFailure {
            resource: "Symbolica parametric sparse reduction transcript",
        }
    })?;
    for entry in entries {
        if entry.column >= prior_rows || entry.coefficient.is_zero() {
            return Err(SymbolicaParametricSparseError::NativeTranscriptMismatch {
                operation: "candidate reduction factors",
            });
        }
        reductions.push(SymbolicaParametricSparseReduction {
            pivot_row: entry.column,
            factor: entry.coefficient,
        });
    }
    Ok(reductions)
}

fn sparse_stats(
    field: &CheckedParametricField,
    prior_rows: usize,
    rows: usize,
    physical_columns: usize,
    input_entries: usize,
    prospective_native_output_entries: usize,
    observed_native_output_entries: usize,
    reducer: &SparseRowReducer<CheckedParametricField>,
    returned_trace_entries: usize,
) -> SymbolicaParametricSparseStats {
    debug_assert_eq!(rows, prior_rows.saturating_add(1));
    debug_assert_eq!(
        reducer.u().nvalues().checked_add(reducer.l().nvalues()),
        Some(observed_native_output_entries)
    );
    SymbolicaParametricSparseStats {
        prior_rows,
        rows,
        physical_columns,
        input_entries,
        prospective_native_output_entries,
        observed_native_output_entries,
        native_u_entries: reducer.u().nvalues(),
        native_l_entries: reducer.l().nvalues(),
        returned_trace_entries,
        coefficient_work: field.stats(),
    }
}

fn call_native<T>(
    operation: &'static str,
    call: impl FnOnce() -> T,
) -> Result<T, SymbolicaParametricSparseError> {
    match catch_unwind(AssertUnwindSafe(call)) {
        Ok(value) => Ok(value),
        Err(payload) => match payload.downcast::<CheckedFieldAbort>() {
            Ok(abort) => match abort.0 {
                CheckedFieldFailure::Work(error) => {
                    Err(SymbolicaParametricSparseError::CoefficientWork(error))
                }
                CheckedFieldFailure::UnexpectedOperation(operation) => {
                    Err(SymbolicaParametricSparseError::UnexpectedFieldOperation { operation })
                }
            },
            Err(_) => Err(SymbolicaParametricSparseError::NativePanic { operation }),
        },
    }
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), SymbolicaParametricSparseError> {
    if requested > limit {
        Err(SymbolicaParametricSparseError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CoefficientContext;

    fn context(scope: &str) -> ParametricCoefficientContext {
        ParametricCoefficientContext::try_new(&CoefficientContext::new(["d"]), scope, 1).unwrap()
    }

    fn entry(column: usize, coefficient: ParametricCoefficient) -> SymbolicaParametricSparseEntry {
        SymbolicaParametricSparseEntry::new(column, coefficient)
    }

    fn row(entries: Vec<SymbolicaParametricSparseEntry>) -> SymbolicaParametricSparseRow {
        SymbolicaParametricSparseRow::new(entries)
    }

    fn forward_owned(
        context: &ParametricCoefficientContext,
        physical_columns: usize,
        prior_rows: &[SymbolicaParametricSparseRow],
        candidate: &SymbolicaParametricSparseRow,
        limits: SymbolicaParametricSparseLimits,
    ) -> Result<SymbolicaParametricSparseOutcome, SymbolicaParametricSparseError> {
        let prior_inputs = prior_rows
            .iter()
            .map(SymbolicaParametricSparseRow::try_as_input)
            .collect::<Result<Vec<_>, _>>()?;
        let candidate_input = candidate.try_as_input()?;
        forward_reduce_last_row(
            context,
            physical_columns,
            &prior_inputs,
            &candidate_input,
            limits,
        )
    }

    #[test]
    fn checked_parametric_field_clones_share_an_owned_send_sync_controller() {
        fn assert_send_sync<T: Send + Sync>() {}

        assert_send_sync::<CheckedParametricField>();
        assert_send_sync::<SparseRowReducer<CheckedParametricField>>();

        let context = Arc::new(context("symbolica-sparse-owned-field-context"));
        let field = CheckedParametricField::new(context.clone());
        let clone = field.clone();
        let mut reducer = SparseRowReducer::new(2, field.clone(), LuLMode::Full);
        let mut reducer_clone = reducer.clone();
        assert!(Arc::ptr_eq(&field.context, &context));
        assert!(Arc::ptr_eq(&field.context, &clone.context));
        assert!(Arc::ptr_eq(&field.controller, &clone.controller));
        assert!(Arc::ptr_eq(
            &reducer.u().field().controller,
            &reducer_clone.u().field().controller,
        ));
        let retained_coefficient = context.index(0).unwrap();
        drop(context);

        let stage = field.begin_stage(ParametricCoefficientWorkLedgerLimits::default());
        assert_eq!(field.stats(), ParametricCoefficientWorkStats::default());
        let input = clone.copy_authenticated(&retained_coefficient);
        assert_eq!(reducer_clone.add_row(&[input], &[0]), Some(0));
        assert_eq!(field.stats(), clone.stats());
        assert!(field.stats().algebra_work() > 0);
        let cloned_reducer_stats = field.stats();
        assert_eq!(reducer.u().nrows(), 0);
        assert_eq!(reducer_clone.u().nrows(), 1);
        drop(stage);
        assert!(
            lock_recovering_poison(&field.controller.active_ledger).is_none(),
            "dropping a successful stage must clear its shared ledger"
        );

        let retry_stage = field.begin_stage(ParametricCoefficientWorkLedgerLimits::default());
        assert_eq!(field.stats(), ParametricCoefficientWorkStats::default());
        let input = reducer
            .u()
            .field()
            .copy_authenticated(&retained_coefficient);
        assert_eq!(reducer.add_row(&[input], &[0]), Some(0));
        assert_eq!(field.stats(), cloned_reducer_stats);
        drop(retry_stage);
        assert_eq!(reducer.u(), reducer_clone.u());
    }

    #[test]
    fn checked_parametric_field_rejects_callbacks_without_an_active_stage() {
        let context = Arc::new(context("symbolica-sparse-field-inactive-callback"));
        let one = context.one();
        let field = CheckedParametricField::new(context);

        assert!(matches!(
            call_native("inactive field callback test", || {
                drop(field.copy_authenticated(&one));
            }),
            Err(SymbolicaParametricSparseError::UnexpectedFieldOperation {
                operation: "coefficient operation outside an active stage",
            })
        ));
        assert!(lock_recovering_poison(&field.controller.active_ledger).is_none());

        let stats = call_native("field callback after inactive rejection", || {
            let _stage = field.begin_stage(ParametricCoefficientWorkLedgerLimits::default());
            drop(field.copy_authenticated(&one));
            field.stats()
        })
        .unwrap();
        assert!(stats.algebra_work() > 0);
        assert!(lock_recovering_poison(&field.controller.active_ledger).is_none());
    }

    #[test]
    fn checked_parametric_field_unknown_panic_cleans_and_recovers_its_stage() {
        let context = Arc::new(context("symbolica-sparse-field-native-panic"));
        let one = context.one();
        let field = CheckedParametricField::new(context);

        assert!(matches!(
            call_native("unknown panic cleanup test", || {
                let _stage = field.begin_stage(ParametricCoefficientWorkLedgerLimits::default());
                drop(field.copy_authenticated(&one));
                let _: Result<(), ParametricCoefficientWorkError> =
                    field.with_active_ledger(|_| panic!("synthetic unknown native panic"));
            }),
            Err(SymbolicaParametricSparseError::NativePanic {
                operation: "unknown panic cleanup test",
            })
        ));
        assert!(lock_recovering_poison(&field.controller.active_ledger).is_none());

        let recovered = call_native("field retry after unknown panic", || {
            let _stage = field.begin_stage(ParametricCoefficientWorkLedgerLimits::default());
            assert_eq!(field.stats(), ParametricCoefficientWorkStats::default());
            drop(field.copy_authenticated(&one));
            field.stats()
        })
        .unwrap();
        assert!(recovered.algebra_work() > 0);
        assert!(lock_recovering_poison(&field.controller.active_ledger).is_none());
    }

    #[test]
    fn checked_parametric_field_sibling_clones_serialize_fresh_stage_ledgers() {
        use std::sync::TryLockError;
        use std::sync::mpsc::sync_channel;
        use std::thread;
        use std::time::Duration;

        let context = Arc::new(context("symbolica-sparse-field-sibling-stages"));
        let one = context.one();
        let field = CheckedParametricField::new(context);
        let first_field = field.clone();
        let first_one = one.clone();
        let (first_entered_sender, first_entered_receiver) = sync_channel(0);
        let (release_first_sender, release_first_receiver) = sync_channel(0);
        let first = thread::spawn(move || {
            let stage = first_field.begin_stage(ParametricCoefficientWorkLedgerLimits::default());
            drop(first_field.copy_authenticated(&first_one));
            first_entered_sender.send(first_field.stats()).unwrap();
            release_first_receiver.recv().unwrap();
            drop(stage);
        });
        let first_stats = first_entered_receiver.recv().unwrap();

        let second_field = field.clone();
        let (second_attempt_sender, second_attempt_receiver) = sync_channel(0);
        let (second_entered_sender, second_entered_receiver) = sync_channel(0);
        let second = thread::spawn(move || {
            second_attempt_sender.send(()).unwrap();
            let stage = second_field.begin_stage(ParametricCoefficientWorkLedgerLimits::default());
            assert_eq!(
                second_field.stats(),
                ParametricCoefficientWorkStats::default(),
                "a sibling stage must not inherit the prior clone's work"
            );
            drop(second_field.copy_authenticated(&one));
            second_entered_sender.send(second_field.stats()).unwrap();
            drop(stage);
        });
        second_attempt_receiver.recv().unwrap();
        assert!(matches!(
            field.controller.stage_gate.try_lock(),
            Err(TryLockError::WouldBlock)
        ));

        release_first_sender.send(()).unwrap();
        let second_stats = second_entered_receiver
            .recv_timeout(Duration::from_secs(10))
            .unwrap();
        first.join().unwrap();
        second.join().unwrap();
        assert_eq!(second_stats, first_stats);
        assert!(lock_recovering_poison(&field.controller.active_ledger).is_none());
    }

    #[test]
    fn checked_parametric_field_controlled_abort_cleans_and_resets_for_retry() {
        let context = Arc::new(context("symbolica-sparse-field-stage-retry"));
        let one = context.one();

        let pilot = CheckedParametricField::new(context.clone());
        let pilot_stage = pilot.begin_stage(ParametricCoefficientWorkLedgerLimits::default());
        drop(pilot.copy_authenticated(&one));
        let one_copy_stats = pilot.stats();
        drop(pilot_stage);
        assert!(one_copy_stats.algebra_work() > 0);

        let mut limits = ParametricCoefficientWorkLedgerLimits::default();
        limits.max_algebra_work = one_copy_stats.algebra_work();
        limits.max_exponent_entry_work = one_copy_stats.exponent_entry_work();
        limits.max_integer_bit_work = one_copy_stats.integer_bit_work();
        let field = CheckedParametricField::new(context);

        let fail_after_one_copy = || {
            call_native("controlled field-limit test", || {
                let _stage = field.begin_stage(limits);
                drop(field.copy_authenticated(&one));
                drop(field.copy_authenticated(&one));
            })
        };
        let first_failure = fail_after_one_copy();
        assert!(matches!(
            &first_failure,
            Err(SymbolicaParametricSparseError::CoefficientWork(_))
        ));
        assert!(
            lock_recovering_poison(&field.controller.active_ledger).is_none(),
            "a typed field abort must not retain partial work"
        );

        let retry_stats = call_native("field retry after controlled abort", || {
            let _stage = field.begin_stage(limits);
            assert_eq!(field.stats(), ParametricCoefficientWorkStats::default());
            drop(field.copy_authenticated(&one));
            field.stats()
        })
        .unwrap();
        assert_eq!(retry_stats, one_copy_stats);
        assert!(
            lock_recovering_poison(&field.controller.active_ledger).is_none(),
            "a successful retry must also clear its ledger"
        );

        assert_eq!(fail_after_one_copy(), first_failure);
        assert!(
            lock_recovering_poison(&field.controller.active_ledger).is_none(),
            "repeated aborts must leave the controller reusable"
        );

        let relaxed_stats = call_native("relaxed field retry", || {
            let _stage = field.begin_stage(ParametricCoefficientWorkLedgerLimits::default());
            drop(field.copy_authenticated(&one));
            drop(field.copy_authenticated(&one));
            field.stats()
        })
        .unwrap();
        assert!(relaxed_stats.algebra_work() > one_copy_stats.algebra_work());
        assert!(
            lock_recovering_poison(&field.controller.active_ledger).is_none(),
            "per-stage relaxed limits must not change cleanup semantics"
        );
    }

    #[test]
    fn symbolica_sparse_exact_bridge_decodes_nonmonotone_independent_l_trace() {
        let context = context("symbolica-sparse-independent");
        let one = context.one();
        let n = context.index(0).unwrap();
        let d = context.lift(&context.base().parameter_at(0)).unwrap();
        let prior = vec![
            row(vec![entry(1, one.clone()), entry(3, one.clone())]),
            row(vec![entry(0, one.clone()), entry(2, one.clone())]),
        ];
        let candidate = row(vec![
            entry(0, context.integer(2)),
            entry(1, context.integer(3)),
            entry(2, context.add(&n, &context.integer(2)).unwrap()),
            entry(3, context.add(&d, &context.integer(3)).unwrap()),
        ]);

        let outcome = forward_owned(
            &context,
            4,
            &prior,
            &candidate,
            SymbolicaParametricSparseLimits::default(),
        )
        .unwrap();
        let SymbolicaParametricSparseOutcome::Independent {
            pivot_column,
            normalized_row,
            reductions,
            normalization_divisor,
            stats,
        } = outcome
        else {
            panic!("candidate must be independent")
        };
        assert_eq!(pivot_column, 2);
        assert_eq!(
            reductions
                .iter()
                .map(SymbolicaParametricSparseReduction::pivot_row)
                .collect::<Vec<_>>(),
            vec![1, 0]
        );
        assert_eq!(reductions[0].factor(), &context.integer(2));
        assert_eq!(reductions[1].factor(), &context.integer(3));
        assert_eq!(normalization_divisor, n);
        assert_eq!(normalized_row.entries()[0].column(), 2);
        assert_eq!(normalized_row.entries()[0].coefficient(), &one);
        assert_eq!(normalized_row.entries()[1].column(), 3);
        assert_eq!(
            normalized_row.entries()[1].coefficient(),
            &context.checked_div(&d, &normalization_divisor).unwrap()
        );
        assert_eq!(stats.prior_rows(), 2);
        assert_eq!(stats.rows(), 3);
        assert_eq!(stats.physical_columns(), 4);
        assert_eq!(stats.input_entries(), 8);
        assert_eq!(stats.prospective_native_output_entries(), 13);
        assert_eq!(stats.native_u_entries(), 6);
        assert_eq!(stats.native_l_entries(), 5);
        assert_eq!(stats.observed_native_output_entries(), 11);
        assert_eq!(
            stats.observed_native_output_entries(),
            stats.native_u_entries() + stats.native_l_entries()
        );
        assert_eq!(
            stats.returned_trace_entries(),
            reductions.len() + normalized_row.entries().len() + 1
        );
        assert!(stats.coefficient_work().algebra_work() > 0);
    }

    #[test]
    fn symbolica_sparse_exact_bridge_keeps_dependent_trace_at_full_physical_rank() {
        let context = context("symbolica-sparse-dependent-sentinel");
        let prior = vec![
            row(vec![entry(1, context.one())]),
            row(vec![entry(0, context.one())]),
        ];
        let candidate = row(vec![
            entry(0, context.integer(2)),
            entry(1, context.integer(3)),
        ]);
        let outcome = forward_owned(
            &context,
            2,
            &prior,
            &candidate,
            SymbolicaParametricSparseLimits::default(),
        )
        .unwrap();
        let SymbolicaParametricSparseOutcome::Dependent {
            reductions,
            canonical_zero_input,
            stats,
        } = outcome
        else {
            panic!("candidate must be dependent")
        };
        assert!(!canonical_zero_input);
        assert_eq!(
            reductions
                .iter()
                .map(SymbolicaParametricSparseReduction::pivot_row)
                .collect::<Vec<_>>(),
            vec![1, 0]
        );
        assert_eq!(reductions[0].factor(), &context.integer(2));
        assert_eq!(reductions[1].factor(), &context.integer(3));
        assert_eq!(stats.prior_rows(), 2);
        assert_eq!(stats.rows(), 3);
        assert_eq!(stats.physical_columns(), 2);
        assert_eq!(stats.input_entries(), 4);
        assert_eq!(stats.prospective_native_output_entries(), 9);
        assert_eq!(stats.native_u_entries(), 2);
        assert_eq!(stats.native_l_entries(), 4);
        assert_eq!(stats.observed_native_output_entries(), 6);
        assert_eq!(stats.returned_trace_entries(), 2);
    }

    #[test]
    fn symbolica_sparse_exact_bridge_classifies_empty_candidate_without_forging_l_row() {
        let context = context("symbolica-sparse-empty");
        let prior = vec![row(vec![entry(0, context.one())])];
        let outcome = forward_owned(
            &context,
            1,
            &prior,
            &SymbolicaParametricSparseRow::default(),
            SymbolicaParametricSparseLimits::default(),
        )
        .unwrap();
        let SymbolicaParametricSparseOutcome::Dependent {
            canonical_zero_input,
            reductions,
            stats,
        } = outcome
        else {
            panic!("empty candidate must be dependent")
        };
        assert!(canonical_zero_input);
        assert!(reductions.is_empty());
        assert_eq!(stats.prior_rows(), 1);
        assert_eq!(stats.rows(), 2);
        assert_eq!(stats.physical_columns(), 1);
        assert_eq!(stats.input_entries(), 1);
        assert_eq!(stats.prospective_native_output_entries(), 5);
        assert_eq!(stats.native_u_entries(), 1);
        assert_eq!(stats.native_l_entries(), 1);
        assert_eq!(stats.observed_native_output_entries(), 2);
        assert_eq!(stats.returned_trace_entries(), 0);
    }

    #[test]
    fn symbolica_sparse_exact_bridge_rejects_noncanonical_rows_before_native_entry() {
        let context = context("symbolica-sparse-invalid");
        let unordered = row(vec![entry(1, context.one()), entry(0, context.integer(2))]);
        assert!(matches!(
            forward_owned(
                &context,
                2,
                &[],
                &unordered,
                SymbolicaParametricSparseLimits::default(),
            ),
            Err(SymbolicaParametricSparseError::NonIncreasingColumns { .. })
        ));
        let explicit_zero = row(vec![entry(0, context.zero())]);
        assert!(matches!(
            forward_owned(
                &context,
                1,
                &[],
                &explicit_zero,
                SymbolicaParametricSparseLimits::default(),
            ),
            Err(SymbolicaParametricSparseError::ExplicitZero { .. })
        ));
        let sentinel = row(vec![entry(1, context.one())]);
        assert!(matches!(
            forward_owned(
                &context,
                1,
                &[],
                &sentinel,
                SymbolicaParametricSparseLimits::default(),
            ),
            Err(SymbolicaParametricSparseError::ColumnOutOfRange { .. })
        ));

        let foreign = ParametricCoefficientContext::try_new(
            &CoefficientContext::new(["d"]),
            "symbolica-sparse-foreign",
            1,
        )
        .unwrap();
        let foreign_row = row(vec![entry(0, foreign.one())]);
        assert!(matches!(
            forward_owned(
                &context,
                1,
                &[],
                &foreign_row,
                SymbolicaParametricSparseLimits::default(),
            ),
            Err(SymbolicaParametricSparseError::Coefficient(
                ParametricCoefficientError::WrongContext
            ))
        ));
    }

    #[test]
    fn symbolica_sparse_exact_bridge_preserves_rational_normalization_divisor() {
        let context = context("symbolica-sparse-rational-divisor");
        let one = context.one();
        let three = context.integer(3);
        let n = context.index(0).unwrap();
        let d = context.lift(&context.base().parameter_at(0)).unwrap();
        let two_n = context.mul(&context.integer(2), &n).unwrap();
        let numerator = context.sub(&d, &two_n).unwrap();
        let denominator = context.add(&n, &one).unwrap();
        let divisor = context.checked_div(&numerator, &denominator).unwrap();
        let three_divisor = context.mul(&three, &divisor).unwrap();
        let candidate = row(vec![entry(0, divisor.clone()), entry(1, three_divisor)]);

        let outcome = forward_owned(
            &context,
            2,
            &[],
            &candidate,
            SymbolicaParametricSparseLimits::default(),
        )
        .unwrap();
        let SymbolicaParametricSparseOutcome::Independent {
            pivot_column,
            normalized_row,
            reductions,
            normalization_divisor,
            ..
        } = outcome
        else {
            panic!("a nonzero rational row must be independent")
        };
        assert_eq!(pivot_column, 0);
        assert!(reductions.is_empty());
        assert_eq!(normalization_divisor, divisor);
        assert_eq!(normalized_row.entries().len(), 2);
        assert_eq!(normalized_row.entries()[0].column(), 0);
        assert_eq!(normalized_row.entries()[0].coefficient(), &one);
        assert_eq!(normalized_row.entries()[1].column(), 1);
        assert_eq!(normalized_row.entries()[1].coefficient(), &three);
    }

    #[test]
    fn symbolica_sparse_exact_bridge_returns_owned_output_after_sources_are_dropped() {
        let context = context("symbolica-sparse-owned-output");
        let candidate = row(vec![
            entry(0, context.index(0).unwrap()),
            entry(1, context.index(0).unwrap()),
        ]);
        let expected_divisor = candidate.entries()[0]
            .coefficient()
            .to_expression()
            .to_canonical_string();
        let expected_normalized = context.one().to_expression().to_canonical_string();
        let outcome = forward_owned(
            &context,
            2,
            &[],
            &candidate,
            SymbolicaParametricSparseLimits::default(),
        )
        .unwrap();

        drop(candidate);
        drop(context);

        let SymbolicaParametricSparseOutcome::Independent {
            normalized_row,
            normalization_divisor,
            ..
        } = outcome
        else {
            panic!("the owned source row must be independent")
        };
        assert_eq!(
            normalization_divisor.to_expression().to_canonical_string(),
            expected_divisor
        );
        assert_eq!(normalized_row.entries().len(), 2);
        for entry in normalized_row.entries() {
            assert_eq!(
                entry.coefficient().to_expression().to_canonical_string(),
                expected_normalized
            );
        }
    }

    #[test]
    fn symbolica_sparse_exact_bridge_rejects_foreign_context_prior_row() {
        let local = context("symbolica-sparse-local-prior");
        let foreign = context("symbolica-sparse-foreign-prior");
        let prior = vec![row(vec![entry(0, foreign.one())])];
        let candidate = row(vec![entry(1, local.one())]);

        assert!(matches!(
            forward_owned(
                &local,
                2,
                &prior,
                &candidate,
                SymbolicaParametricSparseLimits::default(),
            ),
            Err(SymbolicaParametricSparseError::Coefficient(
                ParametricCoefficientError::WrongContext
            ))
        ));
    }

    #[test]
    fn symbolica_sparse_exact_bridge_enforces_exact_prospective_output_envelope() {
        let context = context("symbolica-sparse-native-output-limit");
        let candidate = row(vec![entry(0, context.one()), entry(1, context.one())]);
        let pilot = forward_owned(
            &context,
            2,
            &[],
            &candidate,
            SymbolicaParametricSparseLimits::default(),
        )
        .unwrap();
        let exact_native_entries = pilot.stats().prospective_native_output_entries();
        assert_eq!(exact_native_entries, 3);
        assert_eq!(pilot.stats().observed_native_output_entries(), 3);

        let mut exact = SymbolicaParametricSparseLimits::default();
        exact.max_native_output_entry_envelope = exact_native_entries;
        assert_eq!(
            forward_owned(&context, 2, &[], &candidate, exact),
            Ok(pilot)
        );

        let mut one_below = exact;
        one_below.max_native_output_entry_envelope = exact_native_entries - 1;
        assert!(matches!(
            forward_owned(&context, 2, &[], &candidate, one_below),
            Err(SymbolicaParametricSparseError::ResourceLimit {
                resource: "prospective Symbolica parametric sparse native output entries",
                requested,
                limit,
            }) if requested == exact_native_entries && limit == exact_native_entries - 1
        ));
    }

    #[test]
    fn symbolica_sparse_exact_bridge_distinguishes_output_envelope_from_observed_fill() {
        let context = context("symbolica-sparse-native-output-envelope");
        let prior = vec![row(vec![entry(0, context.one())])];
        let candidate = row(vec![entry(0, context.integer(2))]);
        let pilot = forward_owned(
            &context,
            1,
            &prior,
            &candidate,
            SymbolicaParametricSparseLimits::default(),
        )
        .unwrap();
        assert_eq!(pilot.stats().prospective_native_output_entries(), 5);
        assert_eq!(pilot.stats().observed_native_output_entries(), 3);

        let mut observed_only = SymbolicaParametricSparseLimits::default();
        observed_only.max_native_output_entry_envelope = 3;
        assert!(matches!(
            forward_owned(&context, 1, &prior, &candidate, observed_only),
            Err(SymbolicaParametricSparseError::ResourceLimit {
                resource: "prospective Symbolica parametric sparse native output entries",
                requested: 5,
                limit: 3,
            })
        ));

        let mut exact_envelope = observed_only;
        exact_envelope.max_native_output_entry_envelope = 5;
        assert_eq!(
            forward_owned(&context, 1, &prior, &candidate, exact_envelope),
            Ok(pilot)
        );
    }

    #[test]
    fn symbolica_sparse_exact_bridge_enforces_exact_returned_trace_limit() {
        let context = context("symbolica-sparse-returned-trace-limit");
        let candidate = row(vec![entry(0, context.one()), entry(1, context.one())]);
        let pilot = forward_owned(
            &context,
            2,
            &[],
            &candidate,
            SymbolicaParametricSparseLimits::default(),
        )
        .unwrap();
        let exact_trace_entries = pilot.stats().returned_trace_entries();
        assert_eq!(exact_trace_entries, 3);

        let mut exact = SymbolicaParametricSparseLimits::default();
        exact.max_returned_trace_entries = exact_trace_entries;
        assert_eq!(
            forward_owned(&context, 2, &[], &candidate, exact),
            Ok(pilot)
        );

        let mut one_below = exact;
        one_below.max_returned_trace_entries = exact_trace_entries - 1;
        assert!(matches!(
            forward_owned(&context, 2, &[], &candidate, one_below),
            Err(SymbolicaParametricSparseError::ResourceLimit {
                resource: "Symbolica parametric sparse returned trace entries",
                requested,
                limit,
            }) if requested == exact_trace_entries && limit == exact_trace_entries - 1
        ));
    }

    #[test]
    fn symbolica_sparse_exact_bridge_rejects_dependent_duplicate_prior_transcript() {
        let context = context("symbolica-sparse-dependent-prior");
        let prior = vec![
            row(vec![entry(0, context.one())]),
            row(vec![entry(0, context.one())]),
        ];

        assert!(matches!(
            forward_owned(
                &context,
                1,
                &prior,
                &SymbolicaParametricSparseRow::default(),
                SymbolicaParametricSparseLimits::default(),
            ),
            Err(SymbolicaParametricSparseError::DependentPriorRow { row: 1 })
        ));
    }

    #[test]
    fn symbolica_sparse_exact_bridge_rejects_nonunit_prior_replay_transcript() {
        let context = context("symbolica-sparse-nonunit-prior");
        let prior = vec![row(vec![entry(0, context.integer(2))])];

        assert!(matches!(
            forward_owned(
                &context,
                1,
                &prior,
                &SymbolicaParametricSparseRow::default(),
                SymbolicaParametricSparseLimits::default(),
            ),
            Err(SymbolicaParametricSparseError::PriorRowReplayMismatch { row: 0 })
        ));
    }

    #[test]
    fn symbolica_sparse_exact_bridge_exact_work_limit_and_retry_are_deterministic() {
        let context = context("symbolica-sparse-work-limit");
        let n = context.index(0).unwrap();
        let prior = vec![row(vec![entry(0, context.one()), entry(2, context.one())])];
        let candidate = row(vec![
            entry(0, context.integer(2)),
            entry(1, n),
            entry(2, context.integer(2)),
        ]);
        let pilot = forward_owned(
            &context,
            3,
            &prior,
            &candidate,
            SymbolicaParametricSparseLimits::default(),
        )
        .unwrap();
        let exact_work = pilot.stats().coefficient_work();
        assert!(exact_work.algebra_work() > 0);
        assert!(exact_work.exponent_entry_work() > 0);
        assert!(exact_work.integer_bit_work() > 0);

        let mut exact = SymbolicaParametricSparseLimits::default();
        exact.coefficient_work.max_algebra_work = exact_work.algebra_work();
        exact.coefficient_work.max_exponent_entry_work = exact_work.exponent_entry_work();
        exact.coefficient_work.max_integer_bit_work = exact_work.integer_bit_work();
        let exact_outcome = forward_owned(&context, 3, &prior, &candidate, exact).unwrap();
        assert_eq!(exact_outcome, pilot);

        let mut one_below = exact;
        one_below.coefficient_work.max_algebra_work = exact_work.algebra_work() - 1;
        assert!(matches!(
            forward_owned(&context, 3, &prior, &candidate, one_below),
            Err(SymbolicaParametricSparseError::CoefficientWork(
                ParametricCoefficientWorkError::Elimination(
                    crate::parametric_elimination::ParametricEliminationError::ResourceLimit {
                        resource: "construction coefficient algebra work",
                        requested,
                        limit,
                    }
                )
            )) if requested == exact_work.algebra_work() && limit == exact_work.algebra_work() - 1
        ));

        let mut exponent_one_below = exact;
        exponent_one_below.coefficient_work.max_exponent_entry_work =
            exact_work.exponent_entry_work() - 1;
        assert!(matches!(
            forward_owned(&context, 3, &prior, &candidate, exponent_one_below),
            Err(SymbolicaParametricSparseError::CoefficientWork(
                ParametricCoefficientWorkError::Elimination(
                    crate::parametric_elimination::ParametricEliminationError::ResourceLimit {
                        resource: "construction coefficient exponent-entry work",
                        requested,
                        limit,
                    }
                )
            )) if requested == exact_work.exponent_entry_work()
                && limit == exact_work.exponent_entry_work() - 1
        ));

        let mut integer_one_below = exact;
        integer_one_below.coefficient_work.max_integer_bit_work = exact_work.integer_bit_work() - 1;
        assert!(matches!(
            forward_owned(&context, 3, &prior, &candidate, integer_one_below),
            Err(SymbolicaParametricSparseError::CoefficientWork(
                ParametricCoefficientWorkError::Elimination(
                    crate::parametric_elimination::ParametricEliminationError::ResourceLimit {
                        resource: "construction coefficient integer-bit work",
                        requested,
                        limit,
                    }
                )
            )) if requested == exact_work.integer_bit_work()
                && limit == exact_work.integer_bit_work() - 1
        ));

        assert_eq!(
            forward_owned(&context, 3, &prior, &candidate, exact).unwrap(),
            pilot
        );
    }
}
