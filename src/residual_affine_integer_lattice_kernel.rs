//! Source-independent integer-lattice restriction of ordered affine rows.
//!
//! This crate-private seam solves `c + d t = 0` directly in a compact integer
//! parameter space.  It deliberately knows nothing about families, sectors,
//! source authority, Boolean cases, or polynomial composition.  The first
//! schema retains deterministic arithmetic transcripts and exact `p/K/L`
//! geometry.  Logical-memory and payload-comparison censuses are intentionally
//! deferred until the enclosing refinement owner exists; keeping this module
//! crate-private prevents this bounded first schema from becoming public
//! certificate authority.

#[cfg(test)]
use std::cell::Cell;
use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};

use symbolica::prelude::{EuclideanDomain, Integer};

use crate::ResidualAffinePrimitiveRow;

#[cfg(test)]
thread_local! {
    static TRANSFORM_PHASE_ACTIVE: Cell<bool> = const { Cell::new(false) };
    static TRANSFORM_PHASE_STARTS: Cell<usize> = const { Cell::new(0) };
    static TRANSFORM_PHASE_WORK_UNITS: Cell<usize> = const { Cell::new(0) };
    static TRANSFORM_PHASE_MUTATIONS: Cell<usize> = const { Cell::new(0) };
    static RAW_OUTCOME_EQUAL_CALLS: Cell<usize> = const { Cell::new(0) };
}

#[cfg(test)]
fn begin_transform_phase_for_test() {
    TRANSFORM_PHASE_ACTIVE.with(|active| active.set(true));
    TRANSFORM_PHASE_STARTS.with(|starts| starts.set(starts.get() + 1));
}

#[cfg(not(test))]
fn begin_transform_phase_for_test() {}

#[cfg(test)]
fn end_transform_phase_for_test() {
    TRANSFORM_PHASE_ACTIVE.with(|active| active.set(false));
}

#[cfg(not(test))]
fn end_transform_phase_for_test() {}

#[cfg(test)]
fn note_transform_phase_work_for_test() {
    TRANSFORM_PHASE_ACTIVE.with(|active| {
        if active.get() {
            TRANSFORM_PHASE_WORK_UNITS.with(|work| work.set(work.get() + 1));
        }
    });
}

#[cfg(not(test))]
fn note_transform_phase_work_for_test() {}

#[cfg(test)]
fn note_transform_phase_mutation_for_test() {
    TRANSFORM_PHASE_ACTIVE.with(|active| {
        if active.get() {
            TRANSFORM_PHASE_MUTATIONS.with(|mutations| mutations.set(mutations.get() + 1));
        }
    });
}

#[cfg(not(test))]
fn note_transform_phase_mutation_for_test() {}

#[cfg(test)]
fn note_raw_outcome_equal_call_for_test() {
    RAW_OUTCOME_EQUAL_CALLS.with(|calls| calls.set(calls.get() + 1));
}

#[cfg(not(test))]
fn note_raw_outcome_equal_call_for_test() {}

pub(crate) const RESIDUAL_AFFINE_INTEGER_LATTICE_KERNEL_V1_SCHEMA: &str =
    "rustred-residual-affine-integer-lattice-kernel-v1";

// The implementation follows below.  Its types stay crate-private until the
// later source-bound refinement certificate supplies full memory and payload
// accounting.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ResidualAffineIntegerLatticeLimits {
    pub(crate) max_parameter_arity: usize,
    pub(crate) max_ordered_rows: usize,
    /// Cumulative `Integer` values materialized by state construction,
    /// arithmetic, transcript retention, and invariant verification.
    pub(crate) max_integer_entries: usize,
    pub(crate) max_transforms: usize,
    pub(crate) max_arithmetic_operations: usize,
    /// Euclidean quotient/remainder steps.  Proof-bearing exact divisions use
    /// the distinct counter below.
    pub(crate) max_euclidean_divisions: usize,
    pub(crate) max_exact_divisions: usize,
    pub(crate) max_verification_operations: usize,
    pub(crate) max_integer_coefficient_bits: usize,
    pub(crate) max_integer_bit_work: usize,
}

impl Default for ResidualAffineIntegerLatticeLimits {
    fn default() -> Self {
        Self {
            max_parameter_arity: 4096,
            max_ordered_rows: 1_000_000,
            max_integer_entries: 100_000_000_000,
            max_transforms: 10_000_000_000,
            max_arithmetic_operations: 1_000_000_000_000,
            max_euclidean_divisions: 100_000_000_000,
            max_exact_divisions: 100_000_000_000,
            max_verification_operations: 1_000_000_000_000,
            max_integer_coefficient_bits: 1_000_000,
            max_integer_bit_work: 1_000_000_000_000_000,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ResidualAffineIntegerLatticeStats {
    parameter_arity: usize,
    ordered_rows: usize,
    integer_entries: usize,
    transforms: usize,
    arithmetic_operations: usize,
    euclidean_divisions: usize,
    exact_divisions: usize,
    verification_operations: usize,
    largest_integer_coefficient_bits: usize,
    integer_bit_work: usize,
    independent_rows: usize,
    redundant_rows: usize,
    image_rank: usize,
}

impl ResidualAffineIntegerLatticeStats {
    pub(crate) const fn parameter_arity(self) -> usize {
        self.parameter_arity
    }

    pub(crate) const fn ordered_rows(self) -> usize {
        self.ordered_rows
    }

    pub(crate) const fn integer_entries(self) -> usize {
        self.integer_entries
    }

    pub(crate) const fn transforms(self) -> usize {
        self.transforms
    }

    pub(crate) const fn arithmetic_operations(self) -> usize {
        self.arithmetic_operations
    }

    pub(crate) const fn euclidean_divisions(self) -> usize {
        self.euclidean_divisions
    }

    pub(crate) const fn exact_divisions(self) -> usize {
        self.exact_divisions
    }

    pub(crate) const fn verification_operations(self) -> usize {
        self.verification_operations
    }

    pub(crate) const fn largest_integer_coefficient_bits(self) -> usize {
        self.largest_integer_coefficient_bits
    }

    pub(crate) const fn integer_bit_work(self) -> usize {
        self.integer_bit_work
    }

    pub(crate) const fn independent_rows(self) -> usize {
        self.independent_rows
    }

    pub(crate) const fn redundant_rows(self) -> usize {
        self.redundant_rows
    }

    pub(crate) const fn image_rank(self) -> usize {
        self.image_rank
    }
}

pub(crate) enum ResidualAffineIntegerLatticeTransform {
    Swap {
        row_ordinal: usize,
        left_live_ordinal: usize,
        right_live_ordinal: usize,
    },
    Negate {
        row_ordinal: usize,
        live_ordinal: usize,
        coefficient_before: Integer,
    },
    Bezout {
        row_ordinal: usize,
        anchor_live_ordinal: usize,
        partner_live_ordinal: usize,
        anchor_coefficient: Integer,
        partner_coefficient: Integer,
        positive_gcd: Integer,
        anchor_bezout: Integer,
        partner_bezout: Integer,
    },
    Fix {
        row_ordinal: usize,
        live_ordinal: usize,
        delta: Integer,
        positive_gcd: Integer,
        value: Integer,
    },
}

impl fmt::Debug for ResidualAffineIntegerLatticeTransform {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Swap {
                row_ordinal,
                left_live_ordinal,
                right_live_ordinal,
            } => formatter
                .debug_struct("Swap")
                .field("row_ordinal", row_ordinal)
                .field("left_live_ordinal", left_live_ordinal)
                .field("right_live_ordinal", right_live_ordinal)
                .finish(),
            Self::Negate {
                row_ordinal,
                live_ordinal,
                ..
            } => formatter
                .debug_struct("Negate")
                .field("row_ordinal", row_ordinal)
                .field("live_ordinal", live_ordinal)
                .field("coefficient_before", &"<redacted integer>")
                .finish(),
            Self::Bezout {
                row_ordinal,
                anchor_live_ordinal,
                partner_live_ordinal,
                ..
            } => formatter
                .debug_struct("Bezout")
                .field("row_ordinal", row_ordinal)
                .field("anchor_live_ordinal", anchor_live_ordinal)
                .field("partner_live_ordinal", partner_live_ordinal)
                .field("integer_payload", &"<redacted>")
                .finish(),
            Self::Fix {
                row_ordinal,
                live_ordinal,
                ..
            } => formatter
                .debug_struct("Fix")
                .field("row_ordinal", row_ordinal)
                .field("live_ordinal", live_ordinal)
                .field("integer_payload", &"<redacted>")
                .finish(),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ResidualAffineIntegerLatticeRowDisposition {
    Independent {
        row_ordinal: usize,
        live_arity_before: usize,
        live_arity_after: usize,
        transform_start: usize,
        transform_count: usize,
    },
    Redundant {
        row_ordinal: usize,
        live_arity: usize,
    },
    Inconsistent {
        row_ordinal: usize,
        live_arity: usize,
    },
}

pub(crate) enum ResidualAffineIntegerLatticeEmptyWitness {
    ZeroCoefficientsNonzeroDelta {
        row_ordinal: usize,
        delta: Integer,
    },
    DivisibilityObstruction {
        row_ordinal: usize,
        delta: Integer,
        positive_gcd: Integer,
        normalized_remainder: Integer,
    },
}

impl fmt::Debug for ResidualAffineIntegerLatticeEmptyWitness {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroCoefficientsNonzeroDelta { row_ordinal, .. } => formatter
                .debug_struct("ZeroCoefficientsNonzeroDelta")
                .field("row_ordinal", row_ordinal)
                .field("delta", &"<redacted integer>")
                .finish(),
            Self::DivisibilityObstruction { row_ordinal, .. } => formatter
                .debug_struct("DivisibilityObstruction")
                .field("row_ordinal", row_ordinal)
                .field("integer_payload", &"<redacted>")
                .finish(),
        }
    }
}

pub(crate) struct ResidualAffineIntegerLatticeSolution {
    parameter_arity: usize,
    image_rank: usize,
    p: Vec<Integer>,
    /// Row-major `parameter_arity` by `image_rank` basis matrix.
    k: Vec<Integer>,
    /// Row-major `image_rank` by `parameter_arity` integer left inverse.
    l: Vec<Integer>,
}

impl ResidualAffineIntegerLatticeSolution {
    pub(crate) const fn parameter_arity(&self) -> usize {
        self.parameter_arity
    }

    pub(crate) const fn image_rank(&self) -> usize {
        self.image_rank
    }

    pub(crate) fn p(&self) -> &[Integer] {
        &self.p
    }

    pub(crate) fn k(&self) -> &[Integer] {
        &self.k
    }

    pub(crate) fn l(&self) -> &[Integer] {
        &self.l
    }
}

impl fmt::Debug for ResidualAffineIntegerLatticeSolution {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResidualAffineIntegerLatticeSolution")
            .field("parameter_arity", &self.parameter_arity)
            .field("image_rank", &self.image_rank)
            .field("integer_payload", &"<redacted>")
            .finish()
    }
}

enum ResidualAffineIntegerLatticePayload {
    Solved(ResidualAffineIntegerLatticeSolution),
    Empty(ResidualAffineIntegerLatticeEmptyWitness),
}

/// Non-Clone retained result.  It is an arithmetic seam, not source authority.
pub(crate) struct ResidualAffineIntegerLatticeOutcome {
    schema: &'static str,
    parameter_arity: usize,
    limits: ResidualAffineIntegerLatticeLimits,
    stats: ResidualAffineIntegerLatticeStats,
    dispositions: Vec<ResidualAffineIntegerLatticeRowDisposition>,
    transforms: Vec<ResidualAffineIntegerLatticeTransform>,
    payload: ResidualAffineIntegerLatticePayload,
}

impl ResidualAffineIntegerLatticeOutcome {
    pub(crate) const fn stats(&self) -> ResidualAffineIntegerLatticeStats {
        self.stats
    }

    pub(crate) const fn limits(&self) -> ResidualAffineIntegerLatticeLimits {
        self.limits
    }

    pub(crate) fn solution(&self) -> Option<&ResidualAffineIntegerLatticeSolution> {
        match &self.payload {
            ResidualAffineIntegerLatticePayload::Solved(solution) => Some(solution),
            ResidualAffineIntegerLatticePayload::Empty(_) => None,
        }
    }

    pub(crate) fn empty_witness(&self) -> Option<&ResidualAffineIntegerLatticeEmptyWitness> {
        match &self.payload {
            ResidualAffineIntegerLatticePayload::Solved(_) => None,
            ResidualAffineIntegerLatticePayload::Empty(witness) => Some(witness),
        }
    }

    pub(crate) fn dispositions(&self) -> &[ResidualAffineIntegerLatticeRowDisposition] {
        &self.dispositions
    }

    pub(crate) fn transforms(&self) -> &[ResidualAffineIntegerLatticeTransform] {
        &self.transforms
    }
}

impl fmt::Debug for ResidualAffineIntegerLatticeOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match &self.payload {
            ResidualAffineIntegerLatticePayload::Solved(_) => "Solved",
            ResidualAffineIntegerLatticePayload::Empty(_) => "Empty",
        };
        formatter
            .debug_struct("ResidualAffineIntegerLatticeOutcome")
            .field("schema", &self.schema)
            .field("parameter_arity", &self.parameter_arity)
            .field("kind", &kind)
            .field("stats", &self.stats)
            .field("integer_payload", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ResidualAffineIntegerLatticeError {
    SchemaMismatch,
    ReplayMismatch,
    ArityMismatch {
        row_ordinal: usize,
        expected: usize,
        actual: usize,
    },
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
    ArithmeticInvariantFailure(&'static str),
    SymbolicaPanic,
}

impl fmt::Display for ResidualAffineIntegerLatticeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaMismatch => formatter.write_str("integer-lattice kernel schema mismatch"),
            Self::ReplayMismatch => formatter.write_str("integer-lattice kernel did not replay"),
            Self::ArityMismatch {
                row_ordinal,
                expected,
                actual,
            } => write!(
                formatter,
                "integer-lattice row {row_ordinal} has arity {actual}, expected {expected}"
            ),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "integer-lattice {resource} requested {requested}, configured limit is {limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(
                    formatter,
                    "integer-lattice {resource} count overflowed usize"
                )
            }
            Self::AllocationFailure { resource } => write!(
                formatter,
                "integer-lattice {resource} allocation failed after checked preflight"
            ),
            Self::ArithmeticInvariantFailure(message) => {
                write!(formatter, "integer-lattice invariant failed: {message}")
            }
            Self::SymbolicaPanic => {
                formatter.write_str("Symbolica panicked inside the integer-lattice kernel")
            }
        }
    }
}

impl std::error::Error for ResidualAffineIntegerLatticeError {}

struct Budget {
    limits: ResidualAffineIntegerLatticeLimits,
    stats: ResidualAffineIntegerLatticeStats,
}

impl Budget {
    fn new(
        parameter_arity: usize,
        ordered_rows: usize,
        limits: ResidualAffineIntegerLatticeLimits,
    ) -> Self {
        Self {
            limits,
            stats: ResidualAffineIntegerLatticeStats {
                parameter_arity,
                ordered_rows,
                ..ResidualAffineIntegerLatticeStats::default()
            },
        }
    }

    fn add_counter(
        counter: &mut usize,
        amount: usize,
        resource: &'static str,
        limit: usize,
    ) -> Result<(), ResidualAffineIntegerLatticeError> {
        let requested = checked_add(resource, *counter, amount)?;
        check_limit(resource, requested, limit)?;
        *counter = requested;
        Ok(())
    }

    fn integer_entries(&mut self, amount: usize) -> Result<(), ResidualAffineIntegerLatticeError> {
        Self::add_counter(
            &mut self.stats.integer_entries,
            amount,
            "integer entries",
            self.limits.max_integer_entries,
        )
    }

    fn preflight_integer_entries(
        &self,
        amount: usize,
    ) -> Result<(), ResidualAffineIntegerLatticeError> {
        check_limit(
            "integer entries",
            checked_add("integer entries", self.stats.integer_entries, amount)?,
            self.limits.max_integer_entries,
        )
    }

    fn transform(&mut self) -> Result<(), ResidualAffineIntegerLatticeError> {
        Self::add_counter(
            &mut self.stats.transforms,
            1,
            "transforms",
            self.limits.max_transforms,
        )
    }

    fn arithmetic(&mut self, amount: usize) -> Result<(), ResidualAffineIntegerLatticeError> {
        Self::add_counter(
            &mut self.stats.arithmetic_operations,
            amount,
            "arithmetic operations",
            self.limits.max_arithmetic_operations,
        )
    }

    fn euclidean_division(&mut self) -> Result<(), ResidualAffineIntegerLatticeError> {
        Self::add_counter(
            &mut self.stats.euclidean_divisions,
            1,
            "Euclidean divisions",
            self.limits.max_euclidean_divisions,
        )
    }

    fn exact_division(&mut self) -> Result<(), ResidualAffineIntegerLatticeError> {
        Self::add_counter(
            &mut self.stats.exact_divisions,
            1,
            "exact divisions",
            self.limits.max_exact_divisions,
        )
    }

    fn verify(&mut self, amount: usize) -> Result<(), ResidualAffineIntegerLatticeError> {
        Self::add_counter(
            &mut self.stats.verification_operations,
            amount,
            "verification operations",
            self.limits.max_verification_operations,
        )
    }

    fn charge_bit_work(&mut self, amount: usize) -> Result<(), ResidualAffineIntegerLatticeError> {
        Self::add_counter(
            &mut self.stats.integer_bit_work,
            amount,
            "integer bit work",
            self.limits.max_integer_bit_work,
        )
    }

    fn observe_bits(&mut self, bits: usize) -> Result<(), ResidualAffineIntegerLatticeError> {
        check_limit(
            "integer coefficient bits",
            bits,
            self.limits.max_integer_coefficient_bits,
        )?;
        self.stats.largest_integer_coefficient_bits =
            self.stats.largest_integer_coefficient_bits.max(bits);
        Ok(())
    }

    fn observe_input(&mut self, value: &Integer) -> Result<(), ResidualAffineIntegerLatticeError> {
        let bits = integer_magnitude_bits(value)?;
        self.observe_bits(bits)?;
        self.charge_bit_work(bits.max(1))
    }

    fn preflight_integer_comparison(
        &mut self,
        operands: &[&Integer],
    ) -> Result<(), ResidualAffineIntegerLatticeError> {
        note_transform_phase_work_for_test();
        self.verify(1)?;
        let mut comparison_work = 0usize;
        for operand in operands {
            let bits = integer_magnitude_bits(operand)?;
            self.observe_bits(bits)?;
            comparison_work = checked_add("integer bit work", comparison_work, bits.max(1))?;
        }
        self.charge_bit_work(comparison_work)
    }

    fn integer_is_zero(
        &mut self,
        value: &Integer,
    ) -> Result<bool, ResidualAffineIntegerLatticeError> {
        self.preflight_integer_comparison(&[value])?;
        Ok(value.is_zero())
    }

    fn integer_is_negative(
        &mut self,
        value: &Integer,
    ) -> Result<bool, ResidualAffineIntegerLatticeError> {
        self.preflight_integer_comparison(&[value])?;
        Ok(value.is_negative())
    }

    fn integer_equal(
        &mut self,
        left: &Integer,
        right: &Integer,
    ) -> Result<bool, ResidualAffineIntegerLatticeError> {
        self.preflight_integer_comparison(&[left, right])?;
        Ok(left == right)
    }

    fn integer_less(
        &mut self,
        left: &Integer,
        right: &Integer,
    ) -> Result<bool, ResidualAffineIntegerLatticeError> {
        self.preflight_integer_comparison(&[left, right])?;
        Ok(left < right)
    }

    fn usize_equal(
        &mut self,
        left: usize,
        right: usize,
    ) -> Result<bool, ResidualAffineIntegerLatticeError> {
        self.verify(1)?;
        Ok(left == right)
    }

    fn usize_less(
        &mut self,
        left: usize,
        right: usize,
    ) -> Result<bool, ResidualAffineIntegerLatticeError> {
        self.verify(1)?;
        Ok(left < right)
    }

    fn usize_less_or_equal(
        &mut self,
        left: usize,
        right: usize,
    ) -> Result<bool, ResidualAffineIntegerLatticeError> {
        self.verify(1)?;
        Ok(left <= right)
    }

    fn materialized_literal(
        &mut self,
        value: i64,
    ) -> Result<Integer, ResidualAffineIntegerLatticeError> {
        note_transform_phase_work_for_test();
        let prospective = i64_magnitude_bits(value)?;
        self.observe_bits(prospective)?;
        self.integer_entries(1)?;
        self.charge_bit_work(prospective.max(1))?;
        Ok(Integer::from(value))
    }

    fn clone_integer(
        &mut self,
        value: &Integer,
    ) -> Result<Integer, ResidualAffineIntegerLatticeError> {
        note_transform_phase_work_for_test();
        let bits = integer_magnitude_bits(value)?;
        self.observe_bits(bits)?;
        self.integer_entries(1)?;
        self.charge_bit_work(bits.max(1))?;
        Ok(value.clone())
    }

    fn preflight_addition(
        &mut self,
        left: &Integer,
        right: &Integer,
    ) -> Result<usize, ResidualAffineIntegerLatticeError> {
        let bits = if self.integer_is_zero(left)? {
            integer_magnitude_bits(right)?
        } else if self.integer_is_zero(right)? {
            integer_magnitude_bits(left)?
        } else {
            checked_add(
                "integer coefficient bits",
                integer_magnitude_bits(left)?.max(integer_magnitude_bits(right)?),
                1,
            )?
        };
        self.observe_bits(bits)?;
        Ok(bits)
    }

    fn preflight_multiplication(
        &mut self,
        left: &Integer,
        right: &Integer,
    ) -> Result<usize, ResidualAffineIntegerLatticeError> {
        let bits = if self.integer_is_zero(left)? || self.integer_is_zero(right)? {
            0
        } else {
            checked_add(
                "integer coefficient bits",
                integer_magnitude_bits(left)?,
                integer_magnitude_bits(right)?,
            )?
        };
        self.observe_bits(bits)?;
        Ok(bits)
    }

    fn add(
        &mut self,
        left: &Integer,
        right: &Integer,
    ) -> Result<Integer, ResidualAffineIntegerLatticeError> {
        note_transform_phase_work_for_test();
        let output_bits = self.preflight_addition(left, right)?;
        self.arithmetic(1)?;
        self.integer_entries(1)?;
        self.charge_bit_work(output_bits.max(1))?;
        let value = left + right;
        self.observe_bits(integer_magnitude_bits(&value)?)?;
        Ok(value)
    }

    fn subtract(
        &mut self,
        left: &Integer,
        right: &Integer,
    ) -> Result<Integer, ResidualAffineIntegerLatticeError> {
        note_transform_phase_work_for_test();
        let output_bits = self.preflight_addition(left, right)?;
        self.arithmetic(1)?;
        self.integer_entries(1)?;
        self.charge_bit_work(output_bits.max(1))?;
        let value = left - right;
        self.observe_bits(integer_magnitude_bits(&value)?)?;
        Ok(value)
    }

    fn multiply(
        &mut self,
        left: &Integer,
        right: &Integer,
    ) -> Result<Integer, ResidualAffineIntegerLatticeError> {
        note_transform_phase_work_for_test();
        let output_bits = self.preflight_multiplication(left, right)?;
        let operand_work = checked_mul(
            "integer bit work",
            integer_magnitude_bits(left)?.max(1),
            integer_magnitude_bits(right)?.max(1),
        )?;
        self.arithmetic(1)?;
        self.integer_entries(1)?;
        self.charge_bit_work(checked_add(
            "integer bit work",
            operand_work,
            output_bits.max(1),
        )?)?;
        let value = left * right;
        self.observe_bits(integer_magnitude_bits(&value)?)?;
        Ok(value)
    }

    fn negate(&mut self, value: &Integer) -> Result<Integer, ResidualAffineIntegerLatticeError> {
        note_transform_phase_work_for_test();
        let bits = integer_magnitude_bits(value)?;
        self.observe_bits(bits)?;
        self.arithmetic(1)?;
        self.integer_entries(1)?;
        self.charge_bit_work(bits.max(1))?;
        Ok(-value)
    }

    fn quot_rem_euclidean(
        &mut self,
        numerator: &Integer,
        positive_denominator: &Integer,
    ) -> Result<(Integer, Integer), ResidualAffineIntegerLatticeError> {
        note_transform_phase_work_for_test();
        if self.integer_is_zero(positive_denominator)?
            || self.integer_is_negative(positive_denominator)?
        {
            return Err(
                ResidualAffineIntegerLatticeError::ArithmeticInvariantFailure(
                    "Euclidean denominator is not positive",
                ),
            );
        }
        self.preflight_division(numerator, positive_denominator)?;
        self.euclidean_division()?;
        self.arithmetic(1)?;
        self.integer_entries(2)?;
        let (mut quotient, mut remainder) = numerator.quot_rem(positive_denominator);
        self.observe_bits(integer_magnitude_bits(&quotient)?)?;
        self.observe_bits(integer_magnitude_bits(&remainder)?)?;
        if self.integer_is_negative(&remainder)? {
            let one = self.materialized_literal(1)?;
            quotient = self.subtract(&quotient, &one)?;
            remainder = self.add(&remainder, positive_denominator)?;
        }
        if self.integer_is_negative(&remainder)?
            || !self.integer_less(&remainder, positive_denominator)?
        {
            return Err(
                ResidualAffineIntegerLatticeError::ArithmeticInvariantFailure(
                    "Euclidean remainder is not normalized",
                ),
            );
        }
        Ok((quotient, remainder))
    }

    fn quot_rem_exact(
        &mut self,
        numerator: &Integer,
        denominator: &Integer,
    ) -> Result<Integer, ResidualAffineIntegerLatticeError> {
        note_transform_phase_work_for_test();
        if self.integer_is_zero(denominator)? {
            return Err(
                ResidualAffineIntegerLatticeError::ArithmeticInvariantFailure(
                    "exact division by zero",
                ),
            );
        }
        self.preflight_division(numerator, denominator)?;
        self.exact_division()?;
        self.arithmetic(1)?;
        self.integer_entries(2)?;
        let (quotient, remainder) = numerator.quot_rem(denominator);
        self.observe_bits(integer_magnitude_bits(&quotient)?)?;
        self.observe_bits(integer_magnitude_bits(&remainder)?)?;
        if !self.integer_is_zero(&remainder)? {
            return Err(
                ResidualAffineIntegerLatticeError::ArithmeticInvariantFailure(
                    "proof-bearing division was not exact",
                ),
            );
        }
        Ok(quotient)
    }

    fn preflight_division(
        &mut self,
        numerator: &Integer,
        denominator: &Integer,
    ) -> Result<(), ResidualAffineIntegerLatticeError> {
        let numerator_bits = integer_magnitude_bits(numerator)?.max(1);
        let denominator_bits = integer_magnitude_bits(denominator)?.max(1);
        self.observe_bits(numerator_bits)?;
        self.observe_bits(denominator_bits)?;
        self.charge_bit_work(checked_add(
            "integer bit work",
            checked_mul("integer bit work", numerator_bits, denominator_bits)?,
            checked_add("integer bit work", numerator_bits, denominator_bits)?,
        )?)
    }
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ResidualAffineIntegerLatticeError> {
    left.checked_add(right)
        .ok_or(ResidualAffineIntegerLatticeError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ResidualAffineIntegerLatticeError> {
    left.checked_mul(right)
        .ok_or(ResidualAffineIntegerLatticeError::ResourceCountOverflow { resource })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ResidualAffineIntegerLatticeError> {
    if requested > limit {
        Err(ResidualAffineIntegerLatticeError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn integer_magnitude_bits(value: &Integer) -> Result<usize, ResidualAffineIntegerLatticeError> {
    let bits = match value {
        Integer::Single(value) => u128::from(i64::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Double(value) => u128::from(i128::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Large(value) => u128::from(value.significant_bits()),
    };
    usize::try_from(bits).map_err(
        |_| ResidualAffineIntegerLatticeError::ResourceCountOverflow {
            resource: "integer coefficient bits",
        },
    )
}

fn i64_magnitude_bits(value: i64) -> Result<usize, ResidualAffineIntegerLatticeError> {
    usize::try_from(i64::BITS - value.unsigned_abs().leading_zeros()).map_err(|_| {
        ResidualAffineIntegerLatticeError::ResourceCountOverflow {
            resource: "integer coefficient bits",
        }
    })
}

struct WorkState {
    parameter_arity: usize,
    live_arity: usize,
    p: Vec<Integer>,
    k: Vec<Integer>,
    l: Vec<Integer>,
}

pub(crate) fn solve_residual_affine_integer_lattice(
    parameter_arity: usize,
    ordered_rows: &[ResidualAffinePrimitiveRow],
    limits: ResidualAffineIntegerLatticeLimits,
) -> Result<ResidualAffineIntegerLatticeOutcome, ResidualAffineIntegerLatticeError> {
    catch_unwind(AssertUnwindSafe(|| {
        solve_residual_affine_integer_lattice_inner(parameter_arity, ordered_rows, limits)
    }))
    .map_err(|_| ResidualAffineIntegerLatticeError::SymbolicaPanic)?
}

pub(crate) fn verify_residual_affine_integer_lattice(
    parameter_arity: usize,
    ordered_rows: &[ResidualAffinePrimitiveRow],
    outcome: &ResidualAffineIntegerLatticeOutcome,
    limits: ResidualAffineIntegerLatticeLimits,
) -> Result<(), ResidualAffineIntegerLatticeError> {
    catch_unwind(AssertUnwindSafe(|| {
        verify_residual_affine_integer_lattice_inner(parameter_arity, ordered_rows, outcome, limits)
    }))
    .map_err(|_| ResidualAffineIntegerLatticeError::SymbolicaPanic)?
}

fn solve_residual_affine_integer_lattice_inner(
    parameter_arity: usize,
    ordered_rows: &[ResidualAffinePrimitiveRow],
    limits: ResidualAffineIntegerLatticeLimits,
) -> Result<ResidualAffineIntegerLatticeOutcome, ResidualAffineIntegerLatticeError> {
    check_limit(
        "parameter arity",
        parameter_arity,
        limits.max_parameter_arity,
    )?;
    check_limit("ordered rows", ordered_rows.len(), limits.max_ordered_rows)?;

    let mut budget = Budget::new(parameter_arity, ordered_rows.len(), limits);
    validate_ordered_rows(parameter_arity, ordered_rows, &mut budget)?;
    let mut state = initial_state(parameter_arity, &mut budget)?;

    let mut dispositions = Vec::new();
    dispositions
        .try_reserve_exact(ordered_rows.len())
        .map_err(|_| ResidualAffineIntegerLatticeError::AllocationFailure {
            resource: "row dispositions",
        })?;
    let mut transforms = Vec::new();

    for (row_ordinal, row) in ordered_rows.iter().enumerate() {
        let live_before = state.live_arity;
        let transform_start = transforms.len();
        let (mut alpha, delta) = evaluate_row(row, &state, &mut budget)?;
        let mut anchor = None;
        for (ordinal, coefficient) in alpha.iter().enumerate() {
            if !budget.integer_is_zero(coefficient)? {
                anchor = Some(ordinal);
                break;
            }
        }

        let Some(anchor) = anchor else {
            budget.verify(1)?;
            if budget.integer_is_zero(&delta)? {
                budget.stats.redundant_rows =
                    checked_add("redundant rows", budget.stats.redundant_rows, 1)?;
                dispositions.push(ResidualAffineIntegerLatticeRowDisposition::Redundant {
                    row_ordinal,
                    live_arity: live_before,
                });
                continue;
            }

            let retained_delta = budget.clone_integer(&delta)?;
            dispositions.push(ResidualAffineIntegerLatticeRowDisposition::Inconsistent {
                row_ordinal,
                live_arity: live_before,
            });
            budget.stats.image_rank = 0;
            return Ok(ResidualAffineIntegerLatticeOutcome {
                schema: RESIDUAL_AFFINE_INTEGER_LATTICE_KERNEL_V1_SCHEMA,
                parameter_arity,
                limits,
                stats: budget.stats,
                dispositions,
                transforms,
                payload: ResidualAffineIntegerLatticePayload::Empty(
                    ResidualAffineIntegerLatticeEmptyWitness::ZeroCoefficientsNonzeroDelta {
                        row_ordinal,
                        delta: retained_delta,
                    },
                ),
            });
        };

        let positive_gcd = positive_gcd_of_nonzero_entries(&alpha, &mut budget)?;
        let (fixed_value, normalized_remainder) =
            budget.quot_rem_euclidean(&delta, &positive_gcd)?;
        if !budget.integer_is_zero(&normalized_remainder)? {
            budget.verify(3)?;
            if budget.integer_is_negative(&positive_gcd)?
                || budget.integer_is_zero(&positive_gcd)?
                || budget.integer_is_negative(&normalized_remainder)?
                || !budget.integer_less(&normalized_remainder, &positive_gcd)?
            {
                return Err(
                    ResidualAffineIntegerLatticeError::ArithmeticInvariantFailure(
                        "divisibility witness is not normalized",
                    ),
                );
            }
            let witness = ResidualAffineIntegerLatticeEmptyWitness::DivisibilityObstruction {
                row_ordinal,
                delta: budget.clone_integer(&delta)?,
                positive_gcd: budget.clone_integer(&positive_gcd)?,
                normalized_remainder: budget.clone_integer(&normalized_remainder)?,
            };
            dispositions.push(ResidualAffineIntegerLatticeRowDisposition::Inconsistent {
                row_ordinal,
                live_arity: live_before,
            });
            budget.stats.image_rank = 0;
            return Ok(ResidualAffineIntegerLatticeOutcome {
                schema: RESIDUAL_AFFINE_INTEGER_LATTICE_KERNEL_V1_SCHEMA,
                parameter_arity,
                limits,
                stats: budget.stats,
                dispositions,
                transforms,
                payload: ResidualAffineIntegerLatticePayload::Empty(witness),
            });
        }

        if !budget.usize_equal(anchor, 0)? {
            admit_transform(&mut transforms, &mut budget)?;
            begin_transform_phase_for_test();
            commit_transform(
                &mut transforms,
                ResidualAffineIntegerLatticeTransform::Swap {
                    row_ordinal,
                    left_live_ordinal: 0,
                    right_live_ordinal: anchor,
                },
            );
            swap_live_coordinates(&mut state, &mut alpha, 0, anchor, &mut budget)?;
            end_transform_phase_for_test();
        }

        if budget.integer_is_negative(&alpha[0])? {
            admit_transform(&mut transforms, &mut budget)?;
            begin_transform_phase_for_test();
            let coefficient_before = budget.clone_integer(&alpha[0])?;
            commit_transform(
                &mut transforms,
                ResidualAffineIntegerLatticeTransform::Negate {
                    row_ordinal,
                    live_ordinal: 0,
                    coefficient_before,
                },
            );
            negate_live_coordinate(&mut state, &mut alpha, 0, &mut budget)?;
            end_transform_phase_for_test();
        }
        if budget.integer_is_negative(&alpha[0])? || budget.integer_is_zero(&alpha[0])? {
            return Err(
                ResidualAffineIntegerLatticeError::ArithmeticInvariantFailure(
                    "anchor sign normalization failed",
                ),
            );
        }

        for partner in 1..state.live_arity {
            if budget.integer_is_zero(&alpha[partner])? {
                continue;
            }
            apply_bezout_pair(
                row_ordinal,
                partner,
                &mut state,
                &mut alpha,
                &mut transforms,
                &mut budget,
            )?;
        }

        let mut reduced = budget.integer_equal(&alpha[0], &positive_gcd)?;
        for coefficient in alpha.iter().skip(1) {
            reduced &= budget.integer_is_zero(coefficient)?;
        }
        if !reduced {
            return Err(
                ResidualAffineIntegerLatticeError::ArithmeticInvariantFailure(
                    "unimodular reduction did not produce the positive gcd anchor",
                ),
            );
        }

        admit_transform(&mut transforms, &mut budget)?;
        begin_transform_phase_for_test();
        let retained_delta = budget.clone_integer(&delta)?;
        let retained_gcd = budget.clone_integer(&positive_gcd)?;
        let retained_value = budget.clone_integer(&fixed_value)?;
        commit_transform(
            &mut transforms,
            ResidualAffineIntegerLatticeTransform::Fix {
                row_ordinal,
                live_ordinal: 0,
                delta: retained_delta,
                positive_gcd: retained_gcd,
                value: retained_value,
            },
        );
        fix_and_delete_anchor(&mut state, &fixed_value, &mut budget)?;
        end_transform_phase_for_test();

        budget.stats.independent_rows =
            checked_add("independent rows", budget.stats.independent_rows, 1)?;
        dispositions.push(ResidualAffineIntegerLatticeRowDisposition::Independent {
            row_ordinal,
            live_arity_before: live_before,
            live_arity_after: state.live_arity,
            transform_start,
            transform_count: transforms.len() - transform_start,
        });
    }

    verify_solution_state(ordered_rows, &state, &mut budget)?;
    budget.stats.image_rank = state.live_arity;
    let stats = budget.stats;
    Ok(ResidualAffineIntegerLatticeOutcome {
        schema: RESIDUAL_AFFINE_INTEGER_LATTICE_KERNEL_V1_SCHEMA,
        parameter_arity,
        limits,
        stats,
        dispositions,
        transforms,
        payload: ResidualAffineIntegerLatticePayload::Solved(
            ResidualAffineIntegerLatticeSolution {
                parameter_arity,
                image_rank: state.live_arity,
                p: state.p,
                k: state.k,
                l: state.l,
            },
        ),
    })
}

fn validate_ordered_rows(
    parameter_arity: usize,
    ordered_rows: &[ResidualAffinePrimitiveRow],
    budget: &mut Budget,
) -> Result<(), ResidualAffineIntegerLatticeError> {
    for (row_ordinal, row) in ordered_rows.iter().enumerate() {
        if !budget.usize_equal(row.arity(), parameter_arity)? {
            return Err(ResidualAffineIntegerLatticeError::ArityMismatch {
                row_ordinal,
                expected: parameter_arity,
                actual: row.arity(),
            });
        }
        for component in row.components() {
            budget.observe_input(component)?;
        }
    }
    Ok(())
}

fn initial_state(
    parameter_arity: usize,
    budget: &mut Budget,
) -> Result<WorkState, ResidualAffineIntegerLatticeError> {
    let square = checked_mul("initial lattice entries", parameter_arity, parameter_arity)?;
    let (mut p, mut k, mut l) = try_three_integer_vecs(
        ("initial p", parameter_arity),
        ("initial K", square),
        ("initial L", square),
        budget,
    )?;
    for _ in 0..parameter_arity {
        p.push(budget.materialized_literal(0)?);
    }
    for row in 0..parameter_arity {
        for column in 0..parameter_arity {
            let value = if budget.usize_equal(row, column)? {
                1
            } else {
                0
            };
            k.push(budget.materialized_literal(value)?);
            l.push(budget.materialized_literal(value)?);
        }
    }
    Ok(WorkState {
        parameter_arity,
        live_arity: parameter_arity,
        p,
        k,
        l,
    })
}

#[cfg(test)]
thread_local! {
    static INTEGER_VECTOR_RESERVE_ATTEMPTS: Cell<usize> = const { Cell::new(0) };
}

#[cfg(test)]
fn note_integer_vector_reserve_attempt() {
    INTEGER_VECTOR_RESERVE_ATTEMPTS.with(|attempts| attempts.set(attempts.get() + 1));
}

#[cfg(not(test))]
fn note_integer_vector_reserve_attempt() {}

fn try_integer_vec(
    resource: &'static str,
    capacity: usize,
    budget: &Budget,
) -> Result<Vec<Integer>, ResidualAffineIntegerLatticeError> {
    budget.preflight_integer_entries(capacity)?;
    reserve_integer_vec(resource, capacity)
}

fn reserve_integer_vec(
    resource: &'static str,
    capacity: usize,
) -> Result<Vec<Integer>, ResidualAffineIntegerLatticeError> {
    note_integer_vector_reserve_attempt();
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| ResidualAffineIntegerLatticeError::AllocationFailure { resource })?;
    Ok(values)
}

fn try_two_integer_vecs(
    left: (&'static str, usize),
    right: (&'static str, usize),
    budget: &Budget,
) -> Result<(Vec<Integer>, Vec<Integer>), ResidualAffineIntegerLatticeError> {
    budget.preflight_integer_entries(checked_add("integer entries", left.1, right.1)?)?;
    let left_values = reserve_integer_vec(left.0, left.1)?;
    let right_values = reserve_integer_vec(right.0, right.1)?;
    Ok((left_values, right_values))
}

fn try_three_integer_vecs(
    first: (&'static str, usize),
    second: (&'static str, usize),
    third: (&'static str, usize),
    budget: &Budget,
) -> Result<(Vec<Integer>, Vec<Integer>, Vec<Integer>), ResidualAffineIntegerLatticeError> {
    let requested = checked_add(
        "integer entries",
        first.1,
        checked_add("integer entries", second.1, third.1)?,
    )?;
    budget.preflight_integer_entries(requested)?;
    let first_values = reserve_integer_vec(first.0, first.1)?;
    let second_values = reserve_integer_vec(second.0, second.1)?;
    let third_values = reserve_integer_vec(third.0, third.1)?;
    Ok((first_values, second_values, third_values))
}

fn admit_transform(
    transforms: &mut Vec<ResidualAffineIntegerLatticeTransform>,
    budget: &mut Budget,
) -> Result<(), ResidualAffineIntegerLatticeError> {
    budget.transform()?;
    transforms.try_reserve_exact(1).map_err(|_| {
        ResidualAffineIntegerLatticeError::AllocationFailure {
            resource: "lattice transforms",
        }
    })?;
    Ok(())
}

fn commit_transform(
    transforms: &mut Vec<ResidualAffineIntegerLatticeTransform>,
    transform: ResidualAffineIntegerLatticeTransform,
) {
    note_transform_phase_mutation_for_test();
    transforms.push(transform);
}

fn evaluate_row(
    row: &ResidualAffinePrimitiveRow,
    state: &WorkState,
    budget: &mut Budget,
) -> Result<(Vec<Integer>, Integer), ResidualAffineIntegerLatticeError> {
    let mut alpha = try_integer_vec("residual alpha", state.live_arity, budget)?;
    for live_column in 0..state.live_arity {
        let mut accumulator = budget.materialized_literal(0)?;
        for parameter in 0..state.parameter_arity {
            let product = budget.multiply(
                &row.coefficients()[parameter],
                &state.k[parameter * state.live_arity + live_column],
            )?;
            accumulator = budget.add(&accumulator, &product)?;
        }
        alpha.push(accumulator);
    }

    let mut d_dot_p = budget.materialized_literal(0)?;
    for parameter in 0..state.parameter_arity {
        let product = budget.multiply(&row.coefficients()[parameter], &state.p[parameter])?;
        d_dot_p = budget.add(&d_dot_p, &product)?;
    }
    let negative_constant = budget.negate(row.constant())?;
    let delta = budget.subtract(&negative_constant, &d_dot_p)?;
    Ok((alpha, delta))
}

fn positive_gcd_of_nonzero_entries(
    entries: &[Integer],
    budget: &mut Budget,
) -> Result<Integer, ResidualAffineIntegerLatticeError> {
    let mut first = None;
    for entry in entries {
        if !budget.integer_is_zero(entry)? {
            first = Some(entry);
            break;
        }
    }
    let first = first.ok_or(
        ResidualAffineIntegerLatticeError::ArithmeticInvariantFailure(
            "positive gcd requested for an all-zero vector",
        ),
    )?;
    let mut gcd = absolute_integer(first, budget)?;
    let mut skipped_first_nonzero = false;
    for entry in entries {
        if budget.integer_is_zero(entry)? {
            continue;
        }
        if !skipped_first_nonzero {
            skipped_first_nonzero = true;
            continue;
        }
        let mut remainder = absolute_integer(entry, budget)?;
        while !budget.integer_is_zero(&remainder)? {
            let (_, next) = budget.quot_rem_euclidean(&gcd, &remainder)?;
            gcd = remainder;
            remainder = next;
        }
    }
    if budget.integer_is_zero(&gcd)? || budget.integer_is_negative(&gcd)? {
        return Err(
            ResidualAffineIntegerLatticeError::ArithmeticInvariantFailure(
                "computed gcd is not positive",
            ),
        );
    }
    Ok(gcd)
}

fn deterministic_extended_gcd(
    left: &Integer,
    right: &Integer,
    budget: &mut Budget,
) -> Result<(Integer, Integer, Integer), ResidualAffineIntegerLatticeError> {
    let mut old_remainder = absolute_integer(left, budget)?;
    let mut remainder = absolute_integer(right, budget)?;
    let mut old_left = budget.materialized_literal(1)?;
    let mut current_left = budget.materialized_literal(0)?;
    let mut old_right = budget.materialized_literal(0)?;
    let mut current_right = budget.materialized_literal(1)?;

    while !budget.integer_is_zero(&remainder)? {
        let (quotient, next_remainder) = budget.quot_rem_euclidean(&old_remainder, &remainder)?;
        old_remainder = remainder;
        remainder = next_remainder;

        let q_left = budget.multiply(&quotient, &current_left)?;
        let next_left = budget.subtract(&old_left, &q_left)?;
        old_left = current_left;
        current_left = next_left;

        let q_right = budget.multiply(&quotient, &current_right)?;
        let next_right = budget.subtract(&old_right, &q_right)?;
        old_right = current_right;
        current_right = next_right;
    }
    if budget.integer_is_negative(left)? {
        old_left = budget.negate(&old_left)?;
    }
    if budget.integer_is_negative(right)? {
        old_right = budget.negate(&old_right)?;
    }

    let left_product = budget.multiply(left, &old_left)?;
    let right_product = budget.multiply(right, &old_right)?;
    let reconstructed = budget.add(&left_product, &right_product)?;
    if !budget.integer_equal(&reconstructed, &old_remainder)?
        || budget.integer_is_negative(&old_remainder)?
    {
        return Err(
            ResidualAffineIntegerLatticeError::ArithmeticInvariantFailure(
                "deterministic extended-gcd identity failed",
            ),
        );
    }
    Ok((old_remainder, old_left, old_right))
}

fn absolute_integer(
    value: &Integer,
    budget: &mut Budget,
) -> Result<Integer, ResidualAffineIntegerLatticeError> {
    if budget.integer_is_negative(value)? {
        budget.negate(value)
    } else {
        budget.clone_integer(value)
    }
}

fn swap_live_coordinates(
    state: &mut WorkState,
    alpha: &mut [Integer],
    left: usize,
    right: usize,
    budget: &mut Budget,
) -> Result<(), ResidualAffineIntegerLatticeError> {
    if !budget.usize_less(left, state.live_arity)?
        || !budget.usize_less(right, state.live_arity)?
        || !budget.usize_equal(alpha.len(), state.live_arity)?
    {
        return Err(
            ResidualAffineIntegerLatticeError::ArithmeticInvariantFailure(
                "coordinate swap is outside the live lattice",
            ),
        );
    }
    alpha.swap(left, right);
    for row in 0..state.parameter_arity {
        state.k.swap(
            row * state.live_arity + left,
            row * state.live_arity + right,
        );
    }
    for column in 0..state.parameter_arity {
        state.l.swap(
            left * state.parameter_arity + column,
            right * state.parameter_arity + column,
        );
    }
    Ok(())
}

fn negate_live_coordinate(
    state: &mut WorkState,
    alpha: &mut [Integer],
    coordinate: usize,
    budget: &mut Budget,
) -> Result<(), ResidualAffineIntegerLatticeError> {
    if !budget.usize_less(coordinate, state.live_arity)?
        || !budget.usize_equal(alpha.len(), state.live_arity)?
    {
        return Err(
            ResidualAffineIntegerLatticeError::ArithmeticInvariantFailure(
                "coordinate negation is outside the live lattice",
            ),
        );
    }
    alpha[coordinate] = budget.negate(&alpha[coordinate])?;
    for row in 0..state.parameter_arity {
        let position = row * state.live_arity + coordinate;
        state.k[position] = budget.negate(&state.k[position])?;
    }
    for column in 0..state.parameter_arity {
        let position = coordinate * state.parameter_arity + column;
        state.l[position] = budget.negate(&state.l[position])?;
    }
    Ok(())
}

fn apply_bezout_pair(
    row_ordinal: usize,
    partner: usize,
    state: &mut WorkState,
    alpha: &mut [Integer],
    transforms: &mut Vec<ResidualAffineIntegerLatticeTransform>,
    budget: &mut Budget,
) -> Result<(), ResidualAffineIntegerLatticeError> {
    // Admit and reserve this transcript record before any payload clone,
    // comparison, Euclidean step, exact division, or matrix mutation in the
    // transform phase.
    admit_transform(transforms, budget)?;
    begin_transform_phase_for_test();
    if budget.usize_equal(partner, 0)?
        || !budget.usize_less(partner, state.live_arity)?
        || !budget.usize_equal(alpha.len(), state.live_arity)?
    {
        return Err(
            ResidualAffineIntegerLatticeError::ArithmeticInvariantFailure(
                "Bezout partner is outside the live lattice",
            ),
        );
    }
    let anchor = budget.clone_integer(&alpha[0])?;
    let other = budget.clone_integer(&alpha[partner])?;
    let (positive_gcd, anchor_bezout, other_bezout) =
        deterministic_extended_gcd(&anchor, &other, budget)?;
    if budget.integer_is_zero(&positive_gcd)? || budget.integer_is_negative(&positive_gcd)? {
        return Err(
            ResidualAffineIntegerLatticeError::ArithmeticInvariantFailure(
                "pair gcd is not positive",
            ),
        );
    }

    let other_over_gcd = budget.quot_rem_exact(&other, &positive_gcd)?;
    let anchor_over_gcd = budget.quot_rem_exact(&anchor, &positive_gcd)?;
    let negative_other_over_gcd = budget.negate(&other_over_gcd)?;
    let negative_other_bezout = budget.negate(&other_bezout)?;

    let transcript = ResidualAffineIntegerLatticeTransform::Bezout {
        row_ordinal,
        anchor_live_ordinal: 0,
        partner_live_ordinal: partner,
        anchor_coefficient: budget.clone_integer(&anchor)?,
        partner_coefficient: budget.clone_integer(&other)?,
        positive_gcd: budget.clone_integer(&positive_gcd)?,
        anchor_bezout: budget.clone_integer(&anchor_bezout)?,
        partner_bezout: budget.clone_integer(&other_bezout)?,
    };
    commit_transform(transforms, transcript);

    for row in 0..state.parameter_arity {
        let anchor_position = row * state.live_arity;
        let partner_position = anchor_position + partner;
        let new_anchor = linear_combination(
            &state.k[anchor_position],
            &anchor_bezout,
            &state.k[partner_position],
            &other_bezout,
            budget,
        )?;
        let new_partner = linear_combination(
            &state.k[anchor_position],
            &negative_other_over_gcd,
            &state.k[partner_position],
            &anchor_over_gcd,
            budget,
        )?;
        state.k[anchor_position] = new_anchor;
        state.k[partner_position] = new_partner;
    }

    for column in 0..state.parameter_arity {
        let anchor_position = column;
        let partner_position = partner * state.parameter_arity + column;
        let new_anchor = linear_combination(
            &state.l[anchor_position],
            &anchor_over_gcd,
            &state.l[partner_position],
            &other_over_gcd,
            budget,
        )?;
        let new_partner = linear_combination(
            &state.l[anchor_position],
            &negative_other_bezout,
            &state.l[partner_position],
            &anchor_bezout,
            budget,
        )?;
        state.l[anchor_position] = new_anchor;
        state.l[partner_position] = new_partner;
    }

    let transformed_anchor =
        linear_combination(&anchor, &anchor_bezout, &other, &other_bezout, budget)?;
    let transformed_partner = linear_combination(
        &anchor,
        &negative_other_over_gcd,
        &other,
        &anchor_over_gcd,
        budget,
    )?;
    if !budget.integer_equal(&transformed_anchor, &positive_gcd)?
        || !budget.integer_is_zero(&transformed_partner)?
    {
        return Err(
            ResidualAffineIntegerLatticeError::ArithmeticInvariantFailure(
                "Bezout column transform failed",
            ),
        );
    }
    alpha[0] = transformed_anchor;
    alpha[partner] = transformed_partner;
    end_transform_phase_for_test();
    Ok(())
}

fn linear_combination(
    left_value: &Integer,
    left_scalar: &Integer,
    right_value: &Integer,
    right_scalar: &Integer,
    budget: &mut Budget,
) -> Result<Integer, ResidualAffineIntegerLatticeError> {
    let left = budget.multiply(left_value, left_scalar)?;
    let right = budget.multiply(right_value, right_scalar)?;
    budget.add(&left, &right)
}

fn fix_and_delete_anchor(
    state: &mut WorkState,
    fixed_value: &Integer,
    budget: &mut Budget,
) -> Result<(), ResidualAffineIntegerLatticeError> {
    if budget.usize_equal(state.live_arity, 0)? {
        return Err(
            ResidualAffineIntegerLatticeError::ArithmeticInvariantFailure(
                "cannot fix a coordinate in a rank-zero lattice",
            ),
        );
    }
    for parameter in 0..state.parameter_arity {
        let contribution = budget.multiply(&state.k[parameter * state.live_arity], fixed_value)?;
        state.p[parameter] = budget.add(&state.p[parameter], &contribution)?;
    }

    let new_live_arity = state.live_arity - 1;
    let k_entries = checked_mul(
        "restricted K entries",
        state.parameter_arity,
        new_live_arity,
    )?;
    let l_entries = checked_mul(
        "restricted L entries",
        new_live_arity,
        state.parameter_arity,
    )?;
    let (mut new_k, mut new_l) = try_two_integer_vecs(
        ("restricted K", k_entries),
        ("restricted L", l_entries),
        budget,
    )?;
    for parameter in 0..state.parameter_arity {
        for live in 1..state.live_arity {
            new_k.push(budget.clone_integer(&state.k[parameter * state.live_arity + live])?);
        }
    }
    for live in 1..state.live_arity {
        for parameter in 0..state.parameter_arity {
            new_l.push(budget.clone_integer(&state.l[live * state.parameter_arity + parameter])?);
        }
    }
    state.k = new_k;
    state.l = new_l;
    state.live_arity = new_live_arity;
    Ok(())
}

fn verify_solution_state(
    ordered_rows: &[ResidualAffinePrimitiveRow],
    state: &WorkState,
    budget: &mut Budget,
) -> Result<(), ResidualAffineIntegerLatticeError> {
    verify_solution_slices(
        state.parameter_arity,
        state.live_arity,
        &state.p,
        &state.k,
        &state.l,
        ordered_rows,
        budget,
    )
}

fn verify_solution_slices(
    parameter_arity: usize,
    image_rank: usize,
    p: &[Integer],
    k: &[Integer],
    l: &[Integer],
    ordered_rows: &[ResidualAffinePrimitiveRow],
    budget: &mut Budget,
) -> Result<(), ResidualAffineIntegerLatticeError> {
    let expected_k = checked_mul("K entries", parameter_arity, image_rank)?;
    let expected_l = checked_mul("L entries", image_rank, parameter_arity)?;
    if !budget.usize_equal(p.len(), parameter_arity)?
        || !budget.usize_equal(k.len(), expected_k)?
        || !budget.usize_equal(l.len(), expected_l)?
    {
        return Err(
            ResidualAffineIntegerLatticeError::ArithmeticInvariantFailure(
                "p/K/L dimensions are inconsistent",
            ),
        );
    }

    // L K = I_s.
    for left_row in 0..image_rank {
        for right_column in 0..image_rank {
            let mut value = budget.materialized_literal(0)?;
            for parameter in 0..parameter_arity {
                let product = budget.multiply(
                    &l[left_row * parameter_arity + parameter],
                    &k[parameter * image_rank + right_column],
                )?;
                value = budget.add(&value, &product)?;
            }
            let diagonal = budget.usize_equal(left_row, right_column)?;
            let expected = budget.materialized_literal(if diagonal { 1 } else { 0 })?;
            if !budget.integer_equal(&value, &expected)? {
                return Err(
                    ResidualAffineIntegerLatticeError::ArithmeticInvariantFailure(
                        "L K is not the identity",
                    ),
                );
            }
        }
    }

    // L p = 0.
    for live_row in 0..image_rank {
        let mut value = budget.materialized_literal(0)?;
        for parameter in 0..parameter_arity {
            let product =
                budget.multiply(&l[live_row * parameter_arity + parameter], &p[parameter])?;
            value = budget.add(&value, &product)?;
        }
        if !budget.integer_is_zero(&value)? {
            return Err(
                ResidualAffineIntegerLatticeError::ArithmeticInvariantFailure("L p is not zero"),
            );
        }
    }

    // Every consumed row vanishes on p + K u.
    for row in ordered_rows {
        let mut constant_value = budget.clone_integer(row.constant())?;
        for parameter in 0..parameter_arity {
            let product = budget.multiply(&row.coefficients()[parameter], &p[parameter])?;
            constant_value = budget.add(&constant_value, &product)?;
        }
        if !budget.integer_is_zero(&constant_value)? {
            return Err(
                ResidualAffineIntegerLatticeError::ArithmeticInvariantFailure(
                    "an input row does not vanish at p",
                ),
            );
        }
        for live_column in 0..image_rank {
            let mut linear_value = budget.materialized_literal(0)?;
            for parameter in 0..parameter_arity {
                let product = budget.multiply(
                    &row.coefficients()[parameter],
                    &k[parameter * image_rank + live_column],
                )?;
                linear_value = budget.add(&linear_value, &product)?;
            }
            if !budget.integer_is_zero(&linear_value)? {
                return Err(
                    ResidualAffineIntegerLatticeError::ArithmeticInvariantFailure(
                        "an input row does not annihilate K",
                    ),
                );
            }
        }
    }
    Ok(())
}

fn verify_residual_affine_integer_lattice_inner(
    parameter_arity: usize,
    ordered_rows: &[ResidualAffinePrimitiveRow],
    outcome: &ResidualAffineIntegerLatticeOutcome,
    limits: ResidualAffineIntegerLatticeLimits,
) -> Result<(), ResidualAffineIntegerLatticeError> {
    check_limit(
        "parameter arity",
        parameter_arity,
        limits.max_parameter_arity,
    )?;
    check_limit("ordered rows", ordered_rows.len(), limits.max_ordered_rows)?;
    let mut direct_budget = Budget::new(parameter_arity, ordered_rows.len(), limits);
    direct_budget.verify(1)?;
    if outcome.schema != RESIDUAL_AFFINE_INTEGER_LATTICE_KERNEL_V1_SCHEMA {
        return Err(ResidualAffineIntegerLatticeError::SchemaMismatch);
    }
    if !direct_budget.usize_equal(outcome.parameter_arity, parameter_arity)? {
        return Err(ResidualAffineIntegerLatticeError::ReplayMismatch);
    }
    direct_budget.verify(1)?;
    if outcome.limits != limits {
        return Err(ResidualAffineIntegerLatticeError::ReplayMismatch);
    }
    validate_ordered_rows(parameter_arity, ordered_rows, &mut direct_budget)?;

    match &outcome.payload {
        ResidualAffineIntegerLatticePayload::Solved(solution) => {
            if !direct_budget.usize_equal(solution.parameter_arity, parameter_arity)?
                || !direct_budget.usize_less_or_equal(solution.image_rank, parameter_arity)?
                || !direct_budget.usize_equal(outcome.dispositions.len(), ordered_rows.len())?
            {
                return Err(ResidualAffineIntegerLatticeError::ReplayMismatch);
            }
            verify_solution_slices(
                parameter_arity,
                solution.image_rank,
                &solution.p,
                &solution.k,
                &solution.l,
                ordered_rows,
                &mut direct_budget,
            )?;
        }
        ResidualAffineIntegerLatticePayload::Empty(witness) => {
            verify_empty_witness(witness, ordered_rows.len(), &mut direct_budget)?;
        }
    }

    // Replay starts from the ordered rows, reruns every deterministic gcd and
    // transform, rechecks p/K/L, and then authenticates the exact payload.
    let replayed =
        solve_residual_affine_integer_lattice_inner(parameter_arity, ordered_rows, limits)?;
    if !bounded_outcome_equal(&replayed, outcome, &mut direct_budget)? {
        return Err(ResidualAffineIntegerLatticeError::ReplayMismatch);
    }
    Ok(())
}

fn verify_empty_witness(
    witness: &ResidualAffineIntegerLatticeEmptyWitness,
    row_count: usize,
    budget: &mut Budget,
) -> Result<(), ResidualAffineIntegerLatticeError> {
    match witness {
        ResidualAffineIntegerLatticeEmptyWitness::ZeroCoefficientsNonzeroDelta {
            row_ordinal,
            delta,
        } => {
            if !budget.usize_less(*row_ordinal, row_count)? || budget.integer_is_zero(delta)? {
                return Err(ResidualAffineIntegerLatticeError::ReplayMismatch);
            }
        }
        ResidualAffineIntegerLatticeEmptyWitness::DivisibilityObstruction {
            row_ordinal,
            delta,
            positive_gcd,
            normalized_remainder,
        } => {
            if !budget.usize_less(*row_ordinal, row_count)?
                || budget.integer_is_zero(positive_gcd)?
                || budget.integer_is_negative(positive_gcd)?
                || budget.integer_is_zero(normalized_remainder)?
                || budget.integer_is_negative(normalized_remainder)?
                || !budget.integer_less(normalized_remainder, positive_gcd)?
            {
                return Err(ResidualAffineIntegerLatticeError::ReplayMismatch);
            }
            let (_, replayed_remainder) = budget.quot_rem_euclidean(delta, positive_gcd)?;
            if !budget.integer_equal(&replayed_remainder, normalized_remainder)? {
                return Err(ResidualAffineIntegerLatticeError::ReplayMismatch);
            }
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct OutcomeComparisonCensus {
    units: usize,
    integer_bits: usize,
    largest_integer_bits: usize,
}

impl OutcomeComparisonCensus {
    fn units(&mut self, amount: usize) -> Result<(), ResidualAffineIntegerLatticeError> {
        self.units = checked_add("payload comparison units", self.units, amount)?;
        Ok(())
    }

    fn integer(&mut self, value: &Integer) -> Result<(), ResidualAffineIntegerLatticeError> {
        self.units(1)?;
        let bits = integer_magnitude_bits(value)?;
        self.integer_bits = checked_add(
            "payload comparison integer bit work",
            self.integer_bits,
            bits.max(1),
        )?;
        self.largest_integer_bits = self.largest_integer_bits.max(bits);
        Ok(())
    }
}

fn outcome_operand_comparison_census(
    outcome: &ResidualAffineIntegerLatticeOutcome,
    census: &mut OutcomeComparisonCensus,
) -> Result<(), ResidualAffineIntegerLatticeError> {
    // Schema, arity, ten limit fields, thirteen statistic fields, two outer
    // vector headers, and the payload discriminant.
    census.units(1 + 1 + 10 + 13 + 2 + 1)?;
    for disposition in &outcome.dispositions {
        match disposition {
            ResidualAffineIntegerLatticeRowDisposition::Independent { .. } => census.units(6)?,
            ResidualAffineIntegerLatticeRowDisposition::Redundant { .. }
            | ResidualAffineIntegerLatticeRowDisposition::Inconsistent { .. } => census.units(3)?,
        }
    }
    for transform in &outcome.transforms {
        match transform {
            ResidualAffineIntegerLatticeTransform::Swap { .. } => census.units(4)?,
            ResidualAffineIntegerLatticeTransform::Negate {
                coefficient_before, ..
            } => {
                census.units(3)?;
                census.integer(coefficient_before)?;
            }
            ResidualAffineIntegerLatticeTransform::Bezout {
                anchor_coefficient,
                partner_coefficient,
                positive_gcd,
                anchor_bezout,
                partner_bezout,
                ..
            } => {
                census.units(4)?;
                for integer in [
                    anchor_coefficient,
                    partner_coefficient,
                    positive_gcd,
                    anchor_bezout,
                    partner_bezout,
                ] {
                    census.integer(integer)?;
                }
            }
            ResidualAffineIntegerLatticeTransform::Fix {
                delta,
                positive_gcd,
                value,
                ..
            } => {
                census.units(3)?;
                census.integer(delta)?;
                census.integer(positive_gcd)?;
                census.integer(value)?;
            }
        }
    }
    match &outcome.payload {
        ResidualAffineIntegerLatticePayload::Solved(solution) => {
            census.units(2 + 3)?;
            for integer in solution.p.iter().chain(&solution.k).chain(&solution.l) {
                census.integer(integer)?;
            }
        }
        ResidualAffineIntegerLatticePayload::Empty(witness) => match witness {
            ResidualAffineIntegerLatticeEmptyWitness::ZeroCoefficientsNonzeroDelta {
                delta,
                ..
            } => {
                census.units(2)?;
                census.integer(delta)?;
            }
            ResidualAffineIntegerLatticeEmptyWitness::DivisibilityObstruction {
                delta,
                positive_gcd,
                normalized_remainder,
                ..
            } => {
                census.units(2)?;
                census.integer(delta)?;
                census.integer(positive_gcd)?;
                census.integer(normalized_remainder)?;
            }
        },
    }
    Ok(())
}

fn bounded_outcome_equal(
    left: &ResidualAffineIntegerLatticeOutcome,
    right: &ResidualAffineIntegerLatticeOutcome,
    budget: &mut Budget,
) -> Result<bool, ResidualAffineIntegerLatticeError> {
    let mut census = OutcomeComparisonCensus::default();
    outcome_operand_comparison_census(left, &mut census)?;
    outcome_operand_comparison_census(right, &mut census)?;
    // The complete census is admitted before derive-generated equality can
    // inspect a discriminant, scalar, container length, or GMP integer.
    budget.verify(census.units)?;
    budget.observe_bits(census.largest_integer_bits)?;
    budget.charge_bit_work(census.integer_bits)?;
    Ok(raw_outcome_equal_after_complete_census(left, right))
}

/// This is deliberately private and must only be called after the complete
/// two-operand census above has been admitted.  Retained GMP-bearing types do
/// not implement `PartialEq`, preventing an unbudgeted crate-wide bypass.
fn raw_outcome_equal_after_complete_census(
    left: &ResidualAffineIntegerLatticeOutcome,
    right: &ResidualAffineIntegerLatticeOutcome,
) -> bool {
    note_raw_outcome_equal_call_for_test();
    left.schema == right.schema
        && left.parameter_arity == right.parameter_arity
        && left.limits == right.limits
        && left.stats == right.stats
        && left.dispositions == right.dispositions
        && left.transforms.len() == right.transforms.len()
        && left
            .transforms
            .iter()
            .zip(&right.transforms)
            .all(|(left, right)| raw_transform_equal(left, right))
        && raw_payload_equal(&left.payload, &right.payload)
}

fn raw_transform_equal(
    left: &ResidualAffineIntegerLatticeTransform,
    right: &ResidualAffineIntegerLatticeTransform,
) -> bool {
    match (left, right) {
        (
            ResidualAffineIntegerLatticeTransform::Swap {
                row_ordinal: left_row,
                left_live_ordinal: left_left,
                right_live_ordinal: left_right,
            },
            ResidualAffineIntegerLatticeTransform::Swap {
                row_ordinal: right_row,
                left_live_ordinal: right_left,
                right_live_ordinal: right_right,
            },
        ) => left_row == right_row && left_left == right_left && left_right == right_right,
        (
            ResidualAffineIntegerLatticeTransform::Negate {
                row_ordinal: left_row,
                live_ordinal: left_live,
                coefficient_before: left_coefficient,
            },
            ResidualAffineIntegerLatticeTransform::Negate {
                row_ordinal: right_row,
                live_ordinal: right_live,
                coefficient_before: right_coefficient,
            },
        ) => {
            left_row == right_row
                && left_live == right_live
                && left_coefficient == right_coefficient
        }
        (
            ResidualAffineIntegerLatticeTransform::Bezout {
                row_ordinal: left_row,
                anchor_live_ordinal: left_anchor_live,
                partner_live_ordinal: left_partner_live,
                anchor_coefficient: left_anchor,
                partner_coefficient: left_partner,
                positive_gcd: left_gcd,
                anchor_bezout: left_anchor_bezout,
                partner_bezout: left_partner_bezout,
            },
            ResidualAffineIntegerLatticeTransform::Bezout {
                row_ordinal: right_row,
                anchor_live_ordinal: right_anchor_live,
                partner_live_ordinal: right_partner_live,
                anchor_coefficient: right_anchor,
                partner_coefficient: right_partner,
                positive_gcd: right_gcd,
                anchor_bezout: right_anchor_bezout,
                partner_bezout: right_partner_bezout,
            },
        ) => {
            left_row == right_row
                && left_anchor_live == right_anchor_live
                && left_partner_live == right_partner_live
                && left_anchor == right_anchor
                && left_partner == right_partner
                && left_gcd == right_gcd
                && left_anchor_bezout == right_anchor_bezout
                && left_partner_bezout == right_partner_bezout
        }
        (
            ResidualAffineIntegerLatticeTransform::Fix {
                row_ordinal: left_row,
                live_ordinal: left_live,
                delta: left_delta,
                positive_gcd: left_gcd,
                value: left_value,
            },
            ResidualAffineIntegerLatticeTransform::Fix {
                row_ordinal: right_row,
                live_ordinal: right_live,
                delta: right_delta,
                positive_gcd: right_gcd,
                value: right_value,
            },
        ) => {
            left_row == right_row
                && left_live == right_live
                && left_delta == right_delta
                && left_gcd == right_gcd
                && left_value == right_value
        }
        _ => false,
    }
}

fn raw_payload_equal(
    left: &ResidualAffineIntegerLatticePayload,
    right: &ResidualAffineIntegerLatticePayload,
) -> bool {
    match (left, right) {
        (
            ResidualAffineIntegerLatticePayload::Solved(left),
            ResidualAffineIntegerLatticePayload::Solved(right),
        ) => {
            left.parameter_arity == right.parameter_arity
                && left.image_rank == right.image_rank
                && left.p == right.p
                && left.k == right.k
                && left.l == right.l
        }
        (
            ResidualAffineIntegerLatticePayload::Empty(left),
            ResidualAffineIntegerLatticePayload::Empty(right),
        ) => raw_empty_witness_equal(left, right),
        _ => false,
    }
}

fn raw_empty_witness_equal(
    left: &ResidualAffineIntegerLatticeEmptyWitness,
    right: &ResidualAffineIntegerLatticeEmptyWitness,
) -> bool {
    match (left, right) {
        (
            ResidualAffineIntegerLatticeEmptyWitness::ZeroCoefficientsNonzeroDelta {
                row_ordinal: left_row,
                delta: left_delta,
            },
            ResidualAffineIntegerLatticeEmptyWitness::ZeroCoefficientsNonzeroDelta {
                row_ordinal: right_row,
                delta: right_delta,
            },
        ) => left_row == right_row && left_delta == right_delta,
        (
            ResidualAffineIntegerLatticeEmptyWitness::DivisibilityObstruction {
                row_ordinal: left_row,
                delta: left_delta,
                positive_gcd: left_gcd,
                normalized_remainder: left_remainder,
            },
            ResidualAffineIntegerLatticeEmptyWitness::DivisibilityObstruction {
                row_ordinal: right_row,
                delta: right_delta,
                positive_gcd: right_gcd,
                normalized_remainder: right_remainder,
            },
        ) => {
            left_row == right_row
                && left_delta == right_delta
                && left_gcd == right_gcd
                && left_remainder == right_remainder
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(components: &[i64]) -> ResidualAffinePrimitiveRow {
        ResidualAffinePrimitiveRow::try_from_canonical_components_with_limits(
            components.iter().copied().map(Integer::from).collect(),
            64,
            100_000,
            10_000_000,
        )
        .expect("test input is a canonical primitive row")
    }

    fn integers(values: &[i64]) -> Vec<Integer> {
        values.iter().copied().map(Integer::from).collect()
    }

    fn exact_limits(
        stats: ResidualAffineIntegerLatticeStats,
    ) -> ResidualAffineIntegerLatticeLimits {
        ResidualAffineIntegerLatticeLimits {
            max_parameter_arity: stats.parameter_arity(),
            max_ordered_rows: stats.ordered_rows(),
            max_integer_entries: stats.integer_entries(),
            max_transforms: stats.transforms(),
            max_arithmetic_operations: stats.arithmetic_operations(),
            max_euclidean_divisions: stats.euclidean_divisions(),
            max_exact_divisions: stats.exact_divisions(),
            max_verification_operations: stats.verification_operations(),
            max_integer_coefficient_bits: stats.largest_integer_coefficient_bits(),
            max_integer_bit_work: stats.integer_bit_work(),
        }
    }

    fn expect_resource(
        result: Result<ResidualAffineIntegerLatticeOutcome, ResidualAffineIntegerLatticeError>,
        expected: &'static str,
    ) {
        assert!(matches!(
            result,
            Err(ResidualAffineIntegerLatticeError::ResourceLimit { resource, .. })
                if resource == expected
        ));
    }

    fn reset_integer_vector_reserve_attempts() {
        INTEGER_VECTOR_RESERVE_ATTEMPTS.with(|attempts| attempts.set(0));
    }

    fn integer_vector_reserve_attempts() -> usize {
        INTEGER_VECTOR_RESERVE_ATTEMPTS.with(Cell::get)
    }

    fn reset_transform_phase_hooks() {
        TRANSFORM_PHASE_ACTIVE.with(|active| active.set(false));
        TRANSFORM_PHASE_STARTS.with(|value| value.set(0));
        TRANSFORM_PHASE_WORK_UNITS.with(|value| value.set(0));
        TRANSFORM_PHASE_MUTATIONS.with(|value| value.set(0));
    }

    fn transform_phase_hook_counts() -> (usize, usize, usize) {
        (
            TRANSFORM_PHASE_STARTS.with(Cell::get),
            TRANSFORM_PHASE_WORK_UNITS.with(Cell::get),
            TRANSFORM_PHASE_MUTATIONS.with(Cell::get),
        )
    }

    fn reset_raw_outcome_equal_calls() {
        RAW_OUTCOME_EQUAL_CALLS.with(|calls| calls.set(0));
    }

    fn raw_outcome_equal_calls() -> usize {
        RAW_OUTCOME_EQUAL_CALLS.with(Cell::get)
    }

    #[test]
    fn non_unit_congruence_produces_an_integral_rank_one_basis() {
        let rows = [row(&[0, 2, 3])];
        let limits = ResidualAffineIntegerLatticeLimits::default();
        let outcome = solve_residual_affine_integer_lattice(2, &rows, limits)
            .expect("2*n0+3*n1 must have an integral lattice solution");
        let solution = outcome.solution().expect("branch must be inhabited");

        assert_eq!(solution.parameter_arity(), 2);
        assert_eq!(solution.image_rank(), 1);
        assert_eq!(solution.p(), integers(&[0, 0]));
        assert_eq!(solution.k(), integers(&[-3, 2]));
        assert_eq!(solution.l(), integers(&[-1, -1]));
        assert!(matches!(
            outcome.transforms(),
            [
                ResidualAffineIntegerLatticeTransform::Bezout { .. },
                ResidualAffineIntegerLatticeTransform::Fix { .. }
            ]
        ));
        verify_residual_affine_integer_lattice(2, &rows, &outcome, limits)
            .expect("non-unit solution must replay");
    }

    #[test]
    fn divisibility_obstruction_retains_positive_gcd_and_normalized_remainder() {
        // Canonical representative of 2*n0+4*n1-3=0.
        let rows = [row(&[3, -2, -4])];
        let limits = ResidualAffineIntegerLatticeLimits::default();
        let outcome = solve_residual_affine_integer_lattice(2, &rows, limits)
            .expect("inconsistency is a successful, witnessed outcome");

        match outcome.empty_witness() {
            Some(ResidualAffineIntegerLatticeEmptyWitness::DivisibilityObstruction {
                row_ordinal,
                delta,
                positive_gcd,
                normalized_remainder,
            }) => {
                assert_eq!(*row_ordinal, 0);
                assert_eq!(delta, &Integer::from(-3));
                assert_eq!(positive_gcd, &Integer::from(2));
                assert_eq!(normalized_remainder, &Integer::from(1));
            }
            other => panic!("unexpected empty witness: {other:?}"),
        }
        assert!(outcome.transforms().is_empty());
        verify_residual_affine_integer_lattice(2, &rows, &outcome, limits)
            .expect("divisibility witness must replay");
    }

    #[test]
    fn simultaneous_two_equation_three_variable_coset_is_exact() {
        let rows = [
            row(&[0, 2, 3, 0]),
            // Canonical representative of x+y+z-5=0.
            row(&[5, -1, -1, -1]),
        ];
        let limits = ResidualAffineIntegerLatticeLimits::default();
        let outcome = solve_residual_affine_integer_lattice(3, &rows, limits)
            .expect("simultaneous affine system must compile");
        let solution = outcome.solution().expect("system must be inhabited");

        assert_eq!(solution.image_rank(), 1);
        assert_eq!(solution.p(), integers(&[0, 0, 5]));
        assert_eq!(solution.k(), integers(&[-3, 2, 1]));
        assert_eq!(solution.l(), integers(&[-1, -1, 0]));
        assert_eq!(outcome.stats().independent_rows(), 2);
        assert_eq!(outcome.stats().image_rank(), 1);
        verify_residual_affine_integer_lattice(3, &rows, &outcome, limits)
            .expect("multi-equation solution must replay");
    }

    #[test]
    fn negative_anchor_is_negated_before_the_fixed_value_is_absorbed() {
        let rows = [row(&[2, -1])];
        let limits = ResidualAffineIntegerLatticeLimits::default();
        let outcome =
            solve_residual_affine_integer_lattice(1, &rows, limits).expect("2-t0=0 must compile");
        let solution = outcome.solution().expect("equation has one point");

        assert_eq!(solution.image_rank(), 0);
        assert_eq!(solution.p(), integers(&[2]));
        assert!(solution.k().is_empty());
        assert!(solution.l().is_empty());
        assert!(matches!(
            outcome.transforms(),
            [ResidualAffineIntegerLatticeTransform::Negate {
                coefficient_before,
                ..
            }, ResidualAffineIntegerLatticeTransform::Fix { value, .. }]
                if coefficient_before == &Integer::from(-1)
                    && value == &Integer::from(-2)
        ));
        verify_residual_affine_integer_lattice(1, &rows, &outcome, limits)
            .expect("anchor-sign transcript must replay");
    }

    #[test]
    fn every_exercised_positive_compile_counter_has_an_exact_and_one_below_boundary() {
        let rows = [row(&[0, 2, 3, 0]), row(&[5, -1, -1, -1])];
        let baseline = solve_residual_affine_integer_lattice(
            3,
            &rows,
            ResidualAffineIntegerLatticeLimits::default(),
        )
        .expect("baseline counter fixture must compile");
        let stats = baseline.stats();
        let exact = exact_limits(stats);
        assert!(stats.parameter_arity() > 0);
        assert!(stats.ordered_rows() > 0);
        assert!(stats.integer_entries() > 0);
        assert!(stats.transforms() > 0);
        assert!(stats.arithmetic_operations() > 0);
        assert!(stats.euclidean_divisions() > 0);
        assert!(stats.exact_divisions() > 0);
        assert!(stats.verification_operations() > 0);
        assert!(stats.largest_integer_coefficient_bits() > 0);
        assert!(stats.integer_bit_work() > 0);
        solve_residual_affine_integer_lattice(3, &rows, exact)
            .expect("all exact compile counters must pass");

        let mut below = exact;
        below.max_parameter_arity -= 1;
        expect_resource(
            solve_residual_affine_integer_lattice(3, &rows, below),
            "parameter arity",
        );
        let mut below = exact;
        below.max_ordered_rows -= 1;
        expect_resource(
            solve_residual_affine_integer_lattice(3, &rows, below),
            "ordered rows",
        );
        let mut below = exact;
        below.max_integer_entries -= 1;
        expect_resource(
            solve_residual_affine_integer_lattice(3, &rows, below),
            "integer entries",
        );
        let mut below = exact;
        below.max_transforms -= 1;
        expect_resource(
            solve_residual_affine_integer_lattice(3, &rows, below),
            "transforms",
        );
        let mut below = exact;
        below.max_arithmetic_operations -= 1;
        expect_resource(
            solve_residual_affine_integer_lattice(3, &rows, below),
            "arithmetic operations",
        );
        let mut below = exact;
        below.max_euclidean_divisions -= 1;
        expect_resource(
            solve_residual_affine_integer_lattice(3, &rows, below),
            "Euclidean divisions",
        );
        let mut below = exact;
        below.max_exact_divisions -= 1;
        expect_resource(
            solve_residual_affine_integer_lattice(3, &rows, below),
            "exact divisions",
        );
        let mut below = exact;
        below.max_verification_operations -= 1;
        expect_resource(
            solve_residual_affine_integer_lattice(3, &rows, below),
            "verification operations",
        );
        let mut below = exact;
        below.max_integer_coefficient_bits -= 1;
        expect_resource(
            solve_residual_affine_integer_lattice(3, &rows, below),
            "integer coefficient bits",
        );
        let mut below = exact;
        below.max_integer_bit_work -= 1;
        expect_resource(
            solve_residual_affine_integer_lattice(3, &rows, below),
            "integer bit work",
        );
    }

    #[test]
    fn integer_vector_capacity_is_rejected_before_the_denied_reserve() {
        reset_integer_vector_reserve_attempts();
        let mut limits = ResidualAffineIntegerLatticeLimits::default();
        // p + K + L for arity two needs 2 + 4 + 4 integer slots.
        limits.max_integer_entries = 9;
        expect_resource(
            solve_residual_affine_integer_lattice(2, &[], limits),
            "integer entries",
        );
        assert_eq!(integer_vector_reserve_attempts(), 0);

        reset_integer_vector_reserve_attempts();
        let rows = [row(&[0, 1, 1])];
        // The initial three allocations are admitted exactly; alpha needs two
        // more slots and must fail before its reserve attempt.
        limits.max_integer_entries = 11;
        expect_resource(
            solve_residual_affine_integer_lattice(2, &rows, limits),
            "integer entries",
        );
        assert_eq!(integer_vector_reserve_attempts(), 3);
    }

    #[test]
    fn arbitrary_precision_coprime_coefficients_are_not_narrowed() {
        let huge = Integer::from(u128::MAX) * Integer::from(8) + Integer::from(1);
        let neighbor = &huge + &Integer::from(2);
        let input = ResidualAffinePrimitiveRow::try_from_canonical_components_with_limits(
            vec![Integer::from(0), huge, neighbor],
            3,
            1024,
            10_000_000,
        )
        .expect("large row is canonical and primitive");
        let rows = [input];
        let limits = ResidualAffineIntegerLatticeLimits::default();
        let outcome = solve_residual_affine_integer_lattice(2, &rows, limits)
            .expect("large coprime coefficients must compile");
        assert_eq!(outcome.solution().unwrap().image_rank(), 1);
        assert!(outcome.stats().largest_integer_coefficient_bits() > 128);
        verify_residual_affine_integer_lattice(2, &rows, &outcome, limits)
            .expect("large-integer transcript must replay");
    }

    #[test]
    fn zero_alpha_nonzero_delta_is_a_witnessed_empty_branch() {
        let rows = [row(&[0, 1]), row(&[1, -1])];
        let limits = ResidualAffineIntegerLatticeLimits::default();
        let outcome = solve_residual_affine_integer_lattice(1, &rows, limits)
            .expect("inconsistent restriction is a successful outcome");
        assert!(matches!(
            outcome.empty_witness(),
            Some(ResidualAffineIntegerLatticeEmptyWitness::ZeroCoefficientsNonzeroDelta {
                row_ordinal: 1,
                delta,
            }) if delta == &Integer::from(-1)
        ));
        verify_residual_affine_integer_lattice(1, &rows, &outcome, limits)
            .expect("zero-alpha witness must replay");
    }

    #[test]
    fn redundant_row_and_zero_dimensional_solution_are_distinguished() {
        let rows = [row(&[0, 1, 0]), row(&[0, 1, 0]), row(&[0, 0, 1])];
        let limits = ResidualAffineIntegerLatticeLimits::default();
        let outcome = solve_residual_affine_integer_lattice(2, &rows, limits)
            .expect("zero-dimensional consistent system must compile");
        let solution = outcome.solution().expect("system is inhabited");
        assert_eq!(solution.image_rank(), 0);
        assert_eq!(solution.p(), integers(&[0, 0]));
        assert!(solution.k().is_empty());
        assert!(solution.l().is_empty());
        assert_eq!(outcome.stats().independent_rows(), 2);
        assert_eq!(outcome.stats().redundant_rows(), 1);
        assert!(matches!(
            outcome.dispositions(),
            [
                ResidualAffineIntegerLatticeRowDisposition::Independent { .. },
                ResidualAffineIntegerLatticeRowDisposition::Redundant { row_ordinal: 1, .. },
                ResidualAffineIntegerLatticeRowDisposition::Independent { .. }
            ]
        ));
        verify_residual_affine_integer_lattice(2, &rows, &outcome, limits)
            .expect("zero-dimensional transcript must replay");
    }

    #[test]
    fn transcript_tamper_is_rejected_by_bounded_deep_replay_comparison() {
        let rows = [row(&[0, 2, 3])];
        let limits = ResidualAffineIntegerLatticeLimits::default();
        let mut outcome = solve_residual_affine_integer_lattice(2, &rows, limits)
            .expect("tamper fixture must compile");
        let fix = outcome
            .transforms
            .iter_mut()
            .find_map(|transform| match transform {
                ResidualAffineIntegerLatticeTransform::Fix { value, .. } => Some(value),
                _ => None,
            })
            .expect("fixture has a fixed-coordinate record");
        *fix = Integer::from(17);
        assert!(matches!(
            verify_residual_affine_integer_lattice(2, &rows, &outcome, limits),
            Err(ResidualAffineIntegerLatticeError::ReplayMismatch)
        ));
    }

    #[test]
    fn transform_denial_precedes_every_phase_start_payload_work_and_mutation() {
        let fixtures = [
            (2, row(&[0, 0, 1])), // Swap is first.
            (1, row(&[2, -1])),   // Negate is first.
            (2, row(&[0, 2, 3])), // Bezout is first.
            (1, row(&[0, 1])),    // Fix is first.
        ];
        for (arity, fixture) in fixtures {
            reset_transform_phase_hooks();
            let mut limits = ResidualAffineIntegerLatticeLimits::default();
            limits.max_transforms = 0;
            expect_resource(
                solve_residual_affine_integer_lattice(arity, &[fixture], limits),
                "transforms",
            );
            assert_eq!(transform_phase_hook_counts(), (0, 0, 0));
        }
    }

    #[test]
    fn deep_comparison_census_is_exact_and_one_below_precedes_raw_equality() {
        let rows = [row(&[0, 2, 3])];
        let limits = ResidualAffineIntegerLatticeLimits::default();
        let left = solve_residual_affine_integer_lattice(2, &rows, limits).unwrap();
        let right = solve_residual_affine_integer_lattice(2, &rows, limits).unwrap();
        let mut census = OutcomeComparisonCensus::default();
        outcome_operand_comparison_census(&left, &mut census).unwrap();
        outcome_operand_comparison_census(&right, &mut census).unwrap();
        assert!(census.units > 0);
        assert!(census.integer_bits > 0);

        let mut exact = ResidualAffineIntegerLatticeLimits::default();
        exact.max_verification_operations = census.units;
        exact.max_integer_bit_work = census.integer_bits;
        exact.max_integer_coefficient_bits = census.largest_integer_bits;
        let mut exact_budget = Budget::new(2, rows.len(), exact);
        reset_raw_outcome_equal_calls();
        assert!(bounded_outcome_equal(&left, &right, &mut exact_budget).unwrap());
        assert_eq!(raw_outcome_equal_calls(), 1);
        assert_eq!(exact_budget.stats.verification_operations(), census.units);
        assert_eq!(exact_budget.stats.integer_bit_work(), census.integer_bits);

        let mut below_verification = exact;
        below_verification.max_verification_operations = census.units - 1;
        let mut budget = Budget::new(2, rows.len(), below_verification);
        reset_raw_outcome_equal_calls();
        assert!(matches!(
            bounded_outcome_equal(&left, &right, &mut budget),
            Err(ResidualAffineIntegerLatticeError::ResourceLimit {
                resource: "verification operations",
                ..
            })
        ));
        assert_eq!(raw_outcome_equal_calls(), 0);

        let mut below_bits = exact;
        below_bits.max_integer_bit_work = census.integer_bits - 1;
        let mut budget = Budget::new(2, rows.len(), below_bits);
        reset_raw_outcome_equal_calls();
        assert!(matches!(
            bounded_outcome_equal(&left, &right, &mut budget),
            Err(ResidualAffineIntegerLatticeError::ResourceLimit {
                resource: "integer bit work",
                ..
            })
        ));
        assert_eq!(raw_outcome_equal_calls(), 0);
    }

    #[test]
    fn explicit_swap_transcript_replays_exactly() {
        let rows = [row(&[0, 0, 1])];
        let limits = ResidualAffineIntegerLatticeLimits::default();
        let outcome = solve_residual_affine_integer_lattice(2, &rows, limits).unwrap();
        let solution = outcome.solution().unwrap();
        assert_eq!(solution.p(), integers(&[0, 0]));
        assert_eq!(solution.k(), integers(&[1, 0]));
        assert_eq!(solution.l(), integers(&[1, 0]));
        assert!(matches!(
            outcome.transforms(),
            [
                ResidualAffineIntegerLatticeTransform::Swap {
                    left_live_ordinal: 0,
                    right_live_ordinal: 1,
                    ..
                },
                ResidualAffineIntegerLatticeTransform::Fix { .. }
            ]
        ));
        verify_residual_affine_integer_lattice(2, &rows, &outcome, limits).unwrap();
    }

    #[test]
    fn geometry_bezout_negate_and_container_tampering_are_rejected() {
        let nonunit_rows = [row(&[0, 2, 3])];
        let limits = ResidualAffineIntegerLatticeLimits::default();

        for target in ["p", "K", "L"] {
            let mut outcome =
                solve_residual_affine_integer_lattice(2, &nonunit_rows, limits).unwrap();
            let solution = match &mut outcome.payload {
                ResidualAffineIntegerLatticePayload::Solved(solution) => solution,
                ResidualAffineIntegerLatticePayload::Empty(_) => unreachable!(),
            };
            match target {
                "p" => solution.p[0] = Integer::from(1),
                "K" => solution.k[0] = Integer::from(17),
                "L" => solution.l[0] = Integer::from(17),
                _ => unreachable!(),
            }
            assert!(
                verify_residual_affine_integer_lattice(2, &nonunit_rows, &outcome, limits).is_err()
            );
        }

        let mut bezout = solve_residual_affine_integer_lattice(2, &nonunit_rows, limits).unwrap();
        let coefficient = bezout
            .transforms
            .iter_mut()
            .find_map(|transform| match transform {
                ResidualAffineIntegerLatticeTransform::Bezout { anchor_bezout, .. } => {
                    Some(anchor_bezout)
                }
                _ => None,
            })
            .unwrap();
        *coefficient = Integer::from(19);
        assert!(matches!(
            verify_residual_affine_integer_lattice(2, &nonunit_rows, &bezout, limits),
            Err(ResidualAffineIntegerLatticeError::ReplayMismatch)
        ));

        let sign_rows = [row(&[2, -1])];
        let mut negate = solve_residual_affine_integer_lattice(1, &sign_rows, limits).unwrap();
        let coefficient = negate
            .transforms
            .iter_mut()
            .find_map(|transform| match transform {
                ResidualAffineIntegerLatticeTransform::Negate {
                    coefficient_before, ..
                } => Some(coefficient_before),
                _ => None,
            })
            .unwrap();
        *coefficient = Integer::from(23);
        assert!(matches!(
            verify_residual_affine_integer_lattice(1, &sign_rows, &negate, limits),
            Err(ResidualAffineIntegerLatticeError::ReplayMismatch)
        ));

        let mut container =
            solve_residual_affine_integer_lattice(2, &nonunit_rows, limits).unwrap();
        container.dispositions.clear();
        assert!(matches!(
            verify_residual_affine_integer_lattice(2, &nonunit_rows, &container, limits),
            Err(ResidualAffineIntegerLatticeError::ReplayMismatch)
        ));

        let swap_rows = [row(&[0, 0, 1])];
        let mut swap = solve_residual_affine_integer_lattice(2, &swap_rows, limits).unwrap();
        let right_live_ordinal = swap
            .transforms
            .iter_mut()
            .find_map(|transform| match transform {
                ResidualAffineIntegerLatticeTransform::Swap {
                    right_live_ordinal, ..
                } => Some(right_live_ordinal),
                _ => None,
            })
            .unwrap();
        *right_live_ordinal = 0;
        assert!(matches!(
            verify_residual_affine_integer_lattice(2, &swap_rows, &swap, limits),
            Err(ResidualAffineIntegerLatticeError::ReplayMismatch)
        ));
    }

    #[test]
    fn immutable_outcome_replays_concurrently_on_four_threads() {
        use std::sync::Arc;

        let rows = Arc::new(vec![row(&[0, 2, 3, 0]), row(&[5, -1, -1, -1])]);
        let limits = ResidualAffineIntegerLatticeLimits::default();
        let outcome =
            Arc::new(solve_residual_affine_integer_lattice(3, rows.as_slice(), limits).unwrap());
        let mut workers = Vec::new();
        for _ in 0..4 {
            let rows = Arc::clone(&rows);
            let outcome = Arc::clone(&outcome);
            workers.push(std::thread::spawn(move || {
                verify_residual_affine_integer_lattice(3, rows.as_slice(), outcome.as_ref(), limits)
            }));
        }
        for worker in workers {
            worker.join().unwrap().unwrap();
        }
    }
}
