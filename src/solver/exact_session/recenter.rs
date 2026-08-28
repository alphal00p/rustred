//! Authority-free GMP arithmetic for exact affine-group recentering.
//!
//! The kernel has no database/session transaction, solve plan, unresolved
//! target state, raw certificate row, or topology/loop dispatch.  Callers
//! supply borrowed exact shifts, coefficients, guards, and affine geometry.
//! The returned values are inert until an owning session authenticates and
//! publishes them.

use std::fmt;
use std::mem::{align_of, size_of};
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

#[cfg(test)]
use std::cell::Cell;

use symbolica::prelude::Integer;

use super::physical_key::GeneratedAffineResidualGroupLatticeShift;
use crate::affine_parametric_ordering::integer_magnitude_bits;
use crate::parametric_coefficient::{
    ParametricCoefficientTranslationPreflight, ParametricPolynomialTranslationPreflight,
};
use crate::{
    GuardOrigin, ParametricArithmeticLimits, ParametricCoefficient, ParametricCoefficientContext,
    ParametricCoefficientError, ParametricNonZeroCondition,
};

#[cfg(test)]
thread_local! {
    static TARGET_OFFSET_ARITHMETIC_ENTRIES_FOR_TEST: Cell<usize> = const { Cell::new(0) };
    static CENTERED_SHIFT_ARITHMETIC_OPERATIONS_FOR_TEST: Cell<usize> = const { Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_target_offset_arithmetic_entries_for_test() {
    TARGET_OFFSET_ARITHMETIC_ENTRIES_FOR_TEST.set(0);
}

#[cfg(test)]
pub(crate) fn target_offset_arithmetic_entries_for_test() -> usize {
    TARGET_OFFSET_ARITHMETIC_ENTRIES_FOR_TEST.get()
}

#[cfg(test)]
pub(crate) fn reset_centered_shift_arithmetic_operations_for_test() {
    CENTERED_SHIFT_ARITHMETIC_OPERATIONS_FOR_TEST.set(0);
}

#[cfg(test)]
pub(crate) fn centered_shift_arithmetic_operations_for_test() -> usize {
    CENTERED_SHIFT_ARITHMETIC_OPERATIONS_FOR_TEST.get()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ExactRecenterKernelLimits {
    pub(crate) arithmetic: ParametricArithmeticLimits,
    pub(crate) max_terms: usize,
    pub(crate) max_guards: usize,
    pub(crate) max_geometry_integer_operations: usize,
    pub(crate) max_geometry_integer_bit_work: usize,
    pub(crate) max_target_offset_integer_bits: usize,
    pub(crate) max_target_offset_temporary_bytes: usize,
    pub(crate) max_exact_integer_bits: usize,
    pub(crate) max_exact_shift_components: usize,
    pub(crate) max_exact_shift_integer_bits: usize,
    pub(crate) max_exact_shift_retained_bytes: usize,
    pub(crate) max_centered_shift_outer_buffer_bytes: usize,
    pub(crate) max_borrowed_reference_buffer_bytes: usize,
    pub(crate) max_coefficient_translation_integer_bits: usize,
    pub(crate) max_coefficient_translation_retained_bytes: usize,
    pub(crate) max_translation_preflight_passes: usize,
    pub(crate) max_translation_source_terms: usize,
    pub(crate) max_translation_source_exponent_entries: usize,
    pub(crate) max_translation_output_terms: usize,
    pub(crate) max_translation_output_exponent_entries: usize,
    pub(crate) max_translation_power_operations: usize,
    pub(crate) max_translation_integer_bit_work: usize,
    pub(crate) max_translation_normalized_terms: usize,
    pub(crate) max_translation_retained_output_bytes: usize,
    pub(crate) max_guard_origin_occurrences: usize,
    pub(crate) max_owner_retained_bytes: usize,
    pub(crate) max_combined_live_retained_bytes: usize,
    pub(crate) max_native_temporary_byte_envelope: usize,
}

impl Default for ExactRecenterKernelLimits {
    fn default() -> Self {
        const LARGE: usize = 64_000_000_000;
        const VERY_LARGE: usize = 4_000_000_000_000_000_000;
        const GIB: usize = 1024 * 1024 * 1024;

        Self {
            arithmetic: ParametricArithmeticLimits::default(),
            max_terms: 16_000_000,
            max_guards: 16_000_000,
            max_geometry_integer_operations: LARGE,
            max_geometry_integer_bit_work: VERY_LARGE,
            max_target_offset_integer_bits: VERY_LARGE,
            max_target_offset_temporary_bytes: 128 * GIB,
            max_exact_integer_bits: VERY_LARGE,
            max_exact_shift_components: LARGE,
            max_exact_shift_integer_bits: VERY_LARGE,
            max_exact_shift_retained_bytes: 128 * GIB,
            max_centered_shift_outer_buffer_bytes: 16 * GIB,
            max_borrowed_reference_buffer_bytes: 16 * GIB,
            max_coefficient_translation_integer_bits: VERY_LARGE,
            max_coefficient_translation_retained_bytes: 128 * GIB,
            max_translation_preflight_passes: LARGE,
            max_translation_source_terms: VERY_LARGE,
            max_translation_source_exponent_entries: VERY_LARGE,
            max_translation_output_terms: VERY_LARGE,
            max_translation_output_exponent_entries: VERY_LARGE,
            max_translation_power_operations: VERY_LARGE,
            max_translation_integer_bit_work: VERY_LARGE,
            max_translation_normalized_terms: VERY_LARGE,
            max_translation_retained_output_bytes: 128 * GIB,
            max_guard_origin_occurrences: LARGE,
            // The legacy raw-relation path admitted a 128-GiB result owner.
            // A session additionally retains its staged database and target
            // owners, so combined-live and native peaks need independent
            // headroom instead of aliasing the result-owner ceiling.
            max_owner_retained_bytes: 128 * GIB,
            max_combined_live_retained_bytes: 256 * GIB,
            max_native_temporary_byte_envelope: 512 * GIB,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ExactRecenterKernelStats {
    terms: usize,
    guards: usize,
    geometry_integer_operations: usize,
    geometry_integer_bit_work: usize,
    target_offset_integer_bits: usize,
    target_offset_prospective_retained_bytes: usize,
    target_offset_arc_retained_bytes: usize,
    target_offset_observed_retained_bytes: usize,
    target_offset_observed_arc_retained_bytes: usize,
    target_offset_temporary_bytes: usize,
    exact_shift_components: usize,
    prospective_exact_shift_integer_bits: usize,
    prospective_exact_shift_retained_bytes: usize,
    exact_shift_integer_bits: usize,
    exact_shift_retained_bytes: usize,
    centered_shift_outer_buffer_bytes: usize,
    borrowed_reference_buffer_bytes: usize,
    coefficient_translation_integer_bits: usize,
    coefficient_translation_retained_bytes: usize,
    translation_preflight_passes: usize,
    translation_source_terms: usize,
    translation_source_exponent_entries: usize,
    translation_output_terms: usize,
    translation_output_exponent_entries: usize,
    translation_power_operations: usize,
    translation_integer_bit_work: usize,
    translation_normalized_terms: usize,
    translation_retained_output_bytes: usize,
    guard_origin_occurrences: usize,
    prospective_owner_retained_bytes: usize,
    owner_retained_bytes: usize,
    combined_live_retained_bytes: usize,
    native_temporary_byte_envelope: usize,
}

macro_rules! stats_getters {
    ($($field:ident),+ $(,)?) => {$ (
        pub(crate) const fn $field(self) -> usize { self.$field }
    )+ };
}

impl ExactRecenterKernelStats {
    pub(crate) fn for_row(
        terms: usize,
        guards: usize,
        limits: ExactRecenterKernelLimits,
    ) -> Result<Self, ExactRecenterKernelError> {
        check_limit("exact recentering terms", terms, limits.max_terms)?;
        check_limit("exact recentering guards", guards, limits.max_guards)?;
        Ok(Self {
            terms,
            guards,
            ..Self::default()
        })
    }

    stats_getters!(
        terms,
        guards,
        geometry_integer_operations,
        geometry_integer_bit_work,
        target_offset_integer_bits,
        target_offset_prospective_retained_bytes,
        target_offset_arc_retained_bytes,
        target_offset_observed_retained_bytes,
        target_offset_observed_arc_retained_bytes,
        target_offset_temporary_bytes,
        exact_shift_components,
        prospective_exact_shift_integer_bits,
        prospective_exact_shift_retained_bytes,
        exact_shift_integer_bits,
        exact_shift_retained_bytes,
        centered_shift_outer_buffer_bytes,
        borrowed_reference_buffer_bytes,
        coefficient_translation_integer_bits,
        coefficient_translation_retained_bytes,
        translation_preflight_passes,
        translation_source_terms,
        translation_source_exponent_entries,
        translation_output_terms,
        translation_output_exponent_entries,
        translation_power_operations,
        translation_integer_bit_work,
        translation_normalized_terms,
        translation_retained_output_bytes,
        guard_origin_occurrences,
        prospective_owner_retained_bytes,
        owner_retained_bytes,
        combined_live_retained_bytes,
        native_temporary_byte_envelope,
    );
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExactRecenterKernelError {
    MalformedGeometry,
    CensusMismatch,
    OutputCensusMismatch,
    Coefficient,
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
}

impl ExactRecenterKernelError {
    const fn kind(self) -> &'static str {
        match self {
            Self::MalformedGeometry => "MalformedGeometry",
            Self::CensusMismatch => "CensusMismatch",
            Self::OutputCensusMismatch => "OutputCensusMismatch",
            Self::Coefficient => "Coefficient",
            Self::ResourceLimit { .. } => "ResourceLimit",
            Self::ResourceCountOverflow { .. } => "ResourceCountOverflow",
            Self::AllocationFailure { .. } => "AllocationFailure",
        }
    }
}

impl fmt::Debug for ExactRecenterKernelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExactRecenterKernelError")
            .field("kind", &self.kind())
            .field("private_payload", &"<redacted>")
            .finish()
    }
}

impl From<ParametricCoefficientError> for ExactRecenterKernelError {
    fn from(_: ParametricCoefficientError) -> Self {
        Self::Coefficient
    }
}

/// An exact physical target offset `t = r - A r_F`.
pub(crate) struct ExactTargetOffset {
    values: Vec<Integer>,
    retained_integer_bits: usize,
    retained_bytes: usize,
}

impl ExactTargetOffset {
    pub(crate) fn values(&self) -> &[Integer] {
        &self.values
    }

    pub(crate) const fn retained_integer_bits(&self) -> usize {
        self.retained_integer_bits
    }

    pub(crate) const fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }

    /// Bytes retained by an `Arc<ExactTargetOffset>`, excluding the inline
    /// `Arc` handle stored by its outer owner but including the Arc control
    /// block, alignment padding, exact offset wrapper, vector, and GMP limbs.
    pub(crate) fn arc_retained_bytes(&self) -> Result<usize, ExactRecenterKernelError> {
        Ok(self.authenticate_retained_census()?.1)
    }

    /// Recompute the ledger-owned offset census directly from the GMP payload
    /// and reject a stale or altered child scalar before an outer replay uses
    /// it for resource admission.
    pub(crate) fn authenticate_retained_census(
        &self,
    ) -> Result<(usize, usize), ExactRecenterKernelError> {
        let (retained_integer_bits, retained_vector_bytes) =
            integer_vec_owned_census(&self.values, false)?;
        let retained_bytes = checked_add(
            "exact recentering authenticated target-offset bytes",
            retained_vector_bytes,
            exact_target_offset_wrapper_overhead()?,
        )?;
        if retained_integer_bits != self.retained_integer_bits
            || retained_bytes != self.retained_bytes
        {
            return Err(ExactRecenterKernelError::CensusMismatch);
        }
        let arc_retained_bytes = checked_add(
            "exact recentering retained target-offset Arc bytes",
            retained_bytes,
            arc_payload_control_and_padding_byte_bound::<ExactTargetOffset>()?
                .checked_sub(size_of::<ExactTargetOffset>())
                .ok_or(ExactRecenterKernelError::ResourceCountOverflow {
                    resource: "exact recentering retained target-offset Arc bytes",
                })?,
        )?;
        Ok((retained_integer_bits, arc_retained_bytes))
    }

    #[cfg(test)]
    pub(crate) fn replace_retained_census_for_test(
        &mut self,
        retained_integer_bits: usize,
        retained_bytes: usize,
    ) {
        self.retained_integer_bits = retained_integer_bits;
        self.retained_bytes = retained_bytes;
    }
}

impl fmt::Debug for ExactTargetOffset {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExactTargetOffset")
            .field("arity", &self.values.len())
            .field("retained_integer_bits", &self.retained_integer_bits)
            .field("retained_bytes", &self.retained_bytes)
            .field("private_values", &"<redacted>")
            .finish()
    }
}

/// The exact substitution `delta_i = -r_i` on free index positions.
pub(crate) struct ExactCoefficientTranslation {
    values: Arc<Vec<Integer>>,
    retained_integer_bits: usize,
    retained_bytes: usize,
}

impl ExactCoefficientTranslation {
    pub(crate) fn values(&self) -> &[Integer] {
        &self.values
    }

    pub(crate) const fn retained_integer_bits(&self) -> usize {
        self.retained_integer_bits
    }

    pub(crate) const fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }

    fn into_values(self) -> Arc<Vec<Integer>> {
        self.values
    }
}

impl fmt::Debug for ExactCoefficientTranslation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExactCoefficientTranslation")
            .field("arity", &self.values.len())
            .field("retained_integer_bits", &self.retained_integer_bits)
            .field("retained_bytes", &self.retained_bytes)
            .field("private_values", &"<redacted>")
            .finish()
    }
}

/// One exact centered physical shift `q = s - r`.
#[derive(Clone)]
pub(crate) struct ExactCenteredShift {
    values: Arc<Vec<Integer>>,
    retained_integer_bits: usize,
    retained_bytes: usize,
}

impl ExactCenteredShift {
    pub(crate) fn values(&self) -> &[Integer] {
        &self.values
    }

    pub(crate) fn arity(&self) -> usize {
        self.values.len()
    }

    pub(crate) const fn retained_integer_bits(&self) -> usize {
        self.retained_integer_bits
    }

    pub(crate) const fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }
}

impl PartialEq for ExactCenteredShift {
    fn eq(&self, other: &Self) -> bool {
        self.values == other.values
    }
}
impl Eq for ExactCenteredShift {}
impl Ord for ExactCenteredShift {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.values.cmp(&other.values)
    }
}
impl PartialOrd for ExactCenteredShift {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Debug for ExactCenteredShift {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExactCenteredShift")
            .field("arity", &self.values.len())
            .field("retained_integer_bits", &self.retained_integer_bits)
            .field("retained_bytes", &self.retained_bytes)
            .field("private_values", &"<redacted>")
            .finish()
    }
}

pub(crate) type ExactBorrowedTerm<'a> = (
    &'a GeneratedAffineResidualGroupLatticeShift,
    &'a ParametricCoefficient,
);

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct ExactRecenteredTerm {
    shift: ExactCenteredShift,
    coefficient: ParametricCoefficient,
}

impl ExactRecenteredTerm {
    pub(crate) const fn shift(&self) -> &ExactCenteredShift {
        &self.shift
    }

    pub(crate) const fn coefficient(&self) -> &ParametricCoefficient {
        &self.coefficient
    }
}

impl fmt::Debug for ExactRecenteredTerm {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExactRecenteredTerm")
            .field("shift", &self.shift)
            .field("private_coefficient", &"<redacted>")
            .finish()
    }
}

/// Complete inert exact output.  Only an owning session may decide whether
/// these values become a guarded rule or merely advance the database.
pub(crate) struct ExactRecenteredRow {
    coefficient_translation: ExactCoefficientTranslation,
    terms: Arc<Vec<ExactRecenteredTerm>>,
    guards: Arc<Vec<ParametricNonZeroCondition>>,
    stats: ExactRecenterKernelStats,
}

/// Minimal already-centered relation retained by a compact application event.
///
/// Every coefficient and lattice shift has already been translated.  The
/// coefficient-translation vector, guard provenance, and construction census
/// belong only to derivation and are deliberately dropped at publication.
pub(crate) struct ExactRecenteredApplicationRow {
    terms: Arc<Vec<ExactRecenteredTerm>>,
    pivot_term_ordinal: usize,
}

impl ExactRecenteredApplicationRow {
    pub(crate) fn terms(&self) -> &[ExactRecenteredTerm] {
        &self.terms
    }

    pub(crate) const fn pivot_term_ordinal(&self) -> usize {
        self.pivot_term_ordinal
    }

    pub(crate) fn deep_owned_retained_byte_bound(&self) -> Result<usize, ExactRecenterKernelError> {
        exact_recentered_terms_deep_owned_retained_byte_bound(&self.terms)
    }
}

impl fmt::Debug for ExactRecenteredApplicationRow {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExactRecenteredApplicationRow")
            .field("term_count", &self.terms.len())
            .field("pivot_term_ordinal", &self.pivot_term_ordinal)
            .field("private_terms", &"<redacted>")
            .finish()
    }
}

impl ExactRecenteredRow {
    pub(crate) const fn coefficient_translation(&self) -> &ExactCoefficientTranslation {
        &self.coefficient_translation
    }

    pub(crate) fn terms(&self) -> &[ExactRecenteredTerm] {
        &self.terms
    }

    pub(crate) fn guards(&self) -> &[ParametricNonZeroCondition] {
        &self.guards
    }

    pub(crate) const fn stats(&self) -> ExactRecenterKernelStats {
        self.stats
    }

    /// Census the application row before consuming this derivation owner.
    pub(crate) fn application_row_deep_owned_retained_byte_bound(
        &self,
    ) -> Result<usize, ExactRecenterKernelError> {
        exact_recentered_terms_deep_owned_retained_byte_bound(&self.terms)
    }

    /// Move only the already-centered terms into compact application state.
    /// Every other field is derivation-local and is dropped by this consuming
    /// step.
    pub(crate) fn into_application_row(
        self,
        pivot_term_ordinal: usize,
    ) -> ExactRecenteredApplicationRow {
        debug_assert!(pivot_term_ordinal < self.terms.len());
        ExactRecenteredApplicationRow {
            terms: self.terms,
            pivot_term_ordinal,
        }
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        Arc<Vec<Integer>>,
        Arc<Vec<ExactRecenteredTerm>>,
        Arc<Vec<ParametricNonZeroCondition>>,
        ExactRecenterKernelStats,
    ) {
        (
            self.coefficient_translation.into_values(),
            self.terms,
            self.guards,
            self.stats,
        )
    }
}

fn exact_recentered_terms_deep_owned_retained_byte_bound(
    terms: &Arc<Vec<ExactRecenteredTerm>>,
) -> Result<usize, ExactRecenterKernelError> {
    let mut bytes = arc_vec_retained_bytes_bound::<ExactRecenteredTerm>(terms.capacity())?;
    for term in terms.iter() {
        bytes = checked_add(
            "exact recentered application-row retained bytes",
            bytes,
            term.shift.retained_bytes(),
        )?;
        bytes = checked_add(
            "exact recentered application-row retained bytes",
            bytes,
            term.coefficient
                .owned_retained_byte_bound()
                .ok_or(ExactRecenterKernelError::ResourceCountOverflow {
                    resource: "exact recentered application-row retained bytes",
                })?
                .checked_sub(size_of::<ParametricCoefficient>())
                .ok_or(ExactRecenterKernelError::ResourceCountOverflow {
                    resource: "exact recentered application-row retained bytes",
                })?,
        )?;
    }
    Ok(bytes)
}

impl fmt::Debug for ExactRecenteredRow {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExactRecenteredRow")
            .field("term_count", &self.terms.len())
            .field("guard_count", &self.guards.len())
            .field("stats", &self.stats)
            .field("private_values", &"<redacted>")
            .finish()
    }
}

fn validate_free_positions(
    pivot: &GeneratedAffineResidualGroupLatticeShift,
    free_positions: &[usize],
    arity: usize,
) -> Result<(), ExactRecenterKernelError> {
    if pivot.arity() != arity
        || free_positions.iter().any(|&position| position >= arity)
        || free_positions.windows(2).any(|pair| pair[0] >= pair[1])
    {
        Err(ExactRecenterKernelError::MalformedGeometry)
    } else {
        Ok(())
    }
}

fn validate_geometry(
    pivot: &GeneratedAffineResidualGroupLatticeShift,
    matrix: &[Integer],
    free_positions: &[usize],
    arity: usize,
) -> Result<(), ExactRecenterKernelError> {
    validate_free_positions(pivot, free_positions, arity)?;
    if matrix.len()
        != checked_mul(
            "exact recentering matrix entries",
            arity,
            free_positions.len(),
        )?
    {
        return Err(ExactRecenterKernelError::MalformedGeometry);
    }
    Ok(())
}

pub(crate) fn preflight_exact_geometry(
    pivot: &GeneratedAffineResidualGroupLatticeShift,
    matrix: &[Integer],
    free_positions: &[usize],
    limits: ExactRecenterKernelLimits,
    stats: &mut ExactRecenterKernelStats,
) -> Result<(), ExactRecenterKernelError> {
    let arity = pivot.arity();
    validate_geometry(pivot, matrix, free_positions, arity)?;
    let operations_per_row = checked_add(
        "exact recentering geometry integer operations",
        checked_mul(
            "exact recentering geometry integer operations",
            free_positions.len(),
            2,
        )?,
        1,
    )?;
    stats.geometry_integer_operations = checked_mul(
        "exact recentering geometry integer operations",
        arity,
        operations_per_row,
    )?;
    check_limit(
        "exact recentering geometry integer operations",
        stats.geometry_integer_operations,
        limits.max_geometry_integer_operations,
    )?;
    let mut bit_work = 0usize;
    let mut target_offset_bits = 0usize;
    let mut target_offset_bytes = checked_add(
        "exact recentering target-offset temporary bytes",
        vec_retained_bytes_bound::<Integer>(arity)?,
        exact_target_offset_wrapper_overhead()?,
    )?;
    // Execution materializes a product, the prior accumulator, and its sum.
    // Charge that cancellation-sensitive live peak before doing GMP work.
    let mut target_offset_live_integer_peak = 0usize;
    for row in 0..arity {
        let mut sum_bits = 0usize;
        for (free_ordinal, &free_position) in free_positions.iter().enumerate() {
            let matrix_bits = integer_bits(&matrix[row * free_positions.len() + free_ordinal])?;
            let pivot_bits = integer_bits(&pivot.values()[free_position])?;
            let product_bits =
                checked_add("exact recentering integer bits", matrix_bits, pivot_bits)?;
            check_limit(
                "exact recentering integer bits",
                product_bits,
                limits.max_exact_integer_bits,
            )?;
            let prior_sum_bits = sum_bits;
            let next_sum_bits = checked_add(
                "exact recentering integer bits",
                sum_bits.max(product_bits),
                1,
            )?;
            check_limit(
                "exact recentering integer bits",
                next_sum_bits,
                limits.max_exact_integer_bits,
            )?;
            let live_integer_bytes = checked_add(
                "exact recentering target-offset temporary bytes",
                integer_retained_bytes(product_bits)?,
                checked_add(
                    "exact recentering target-offset temporary bytes",
                    integer_retained_bytes(prior_sum_bits)?,
                    integer_retained_bytes(next_sum_bits)?,
                )?,
            )?;
            target_offset_live_integer_peak =
                target_offset_live_integer_peak.max(live_integer_bytes);
            sum_bits = next_sum_bits;
            // Integer-bit work follows the kernel-wide operand/result
            // convention independently for both GMP operations.  In
            // particular, the accumulator update is not folded into the
            // multiplication charge: cancellation can leave a small exact
            // sum even though both of its live operands were large.
            bit_work = checked_add(
                "exact recentering geometry integer-bit work",
                bit_work,
                exact_geometry_operation_bit_work(matrix_bits, pivot_bits, product_bits)?,
            )?;
            bit_work = checked_add(
                "exact recentering geometry integer-bit work",
                bit_work,
                exact_geometry_operation_bit_work(prior_sum_bits, product_bits, next_sum_bits)?,
            )?;
        }
        let row_bits = integer_bits(&pivot.values()[row])?;
        let target_bits = checked_add("exact recentering integer bits", row_bits.max(sum_bits), 1)?;
        check_limit(
            "exact recentering integer bits",
            target_bits,
            limits.max_exact_integer_bits,
        )?;
        bit_work = checked_add(
            "exact recentering geometry integer-bit work",
            bit_work,
            exact_geometry_operation_bit_work(row_bits, sum_bits, target_bits)?,
        )?;
        target_offset_bits = checked_add(
            "exact recentering target-offset integer bits",
            target_offset_bits,
            target_bits,
        )?;
        target_offset_bytes = checked_add(
            "exact recentering target-offset temporary bytes",
            target_offset_bytes,
            prospective_integer_heap_bytes(target_bits)?,
        )?;
        let subtraction_live_bytes = checked_add(
            "exact recentering target-offset temporary bytes",
            integer_retained_bytes(sum_bits)?,
            integer_retained_bytes(target_bits)?,
        )?;
        target_offset_live_integer_peak =
            target_offset_live_integer_peak.max(subtraction_live_bytes);
    }
    let target_offset_prospective_retained_bytes = target_offset_bytes;
    let target_offset_arc_retained_bytes = checked_add(
        "exact recentering retained target-offset Arc bytes",
        target_offset_prospective_retained_bytes,
        arc_payload_control_and_padding_byte_bound::<ExactTargetOffset>()?
            .checked_sub(size_of::<ExactTargetOffset>())
            .ok_or(ExactRecenterKernelError::ResourceCountOverflow {
                resource: "exact recentering retained target-offset Arc bytes",
            })?,
    )?;
    target_offset_bytes = checked_add(
        "exact recentering target-offset temporary bytes",
        target_offset_bytes,
        target_offset_live_integer_peak,
    )?;
    check_limit(
        "exact recentering geometry integer-bit work",
        bit_work,
        limits.max_geometry_integer_bit_work,
    )?;
    check_limit(
        "exact recentering target-offset integer bits",
        target_offset_bits,
        limits.max_target_offset_integer_bits,
    )?;
    check_limit(
        "exact recentering target-offset temporary bytes",
        target_offset_bytes,
        limits.max_target_offset_temporary_bytes,
    )?;
    stats.geometry_integer_bit_work = bit_work;
    stats.target_offset_integer_bits = target_offset_bits;
    stats.target_offset_prospective_retained_bytes = target_offset_prospective_retained_bytes;
    stats.target_offset_arc_retained_bytes = target_offset_arc_retained_bytes;
    stats.target_offset_temporary_bytes = target_offset_bytes;
    Ok(())
}

pub(crate) fn execute_target_offset(
    pivot: &GeneratedAffineResidualGroupLatticeShift,
    matrix: &[Integer],
    free_positions: &[usize],
    arity: usize,
) -> Result<ExactTargetOffset, ExactRecenterKernelError> {
    validate_geometry(pivot, matrix, free_positions, arity)?;
    let mut output = try_vec("exact recentering target offset", arity)?;
    #[cfg(test)]
    if arity > 0 {
        TARGET_OFFSET_ARITHMETIC_ENTRIES_FOR_TEST.with(|entries| {
            entries.set(entries.get().saturating_add(1));
        });
    }
    for row in 0..arity {
        let mut sum = Integer::from(0);
        for (free_ordinal, &free_position) in free_positions.iter().enumerate() {
            sum +=
                &matrix[row * free_positions.len() + free_ordinal] * &pivot.values()[free_position];
        }
        output.push(canonical_integer(&pivot.values()[row] - sum));
    }
    let (retained_integer_bits, retained_vector_bytes) = integer_vec_owned_census(&output, false)?;
    let retained_bytes = checked_add(
        "exact recentering target-offset temporary bytes",
        retained_vector_bytes,
        exact_target_offset_wrapper_overhead()?,
    )?;
    Ok(ExactTargetOffset {
        values: output,
        retained_integer_bits,
        retained_bytes,
    })
}

fn exact_geometry_operation_bit_work(
    left_bits: usize,
    right_bits: usize,
    result_bits: usize,
) -> Result<usize, ExactRecenterKernelError> {
    checked_add(
        "exact recentering geometry integer-bit work",
        left_bits.max(1),
        checked_add(
            "exact recentering geometry integer-bit work",
            right_bits.max(1),
            result_bits.max(1),
        )?,
    )
}

pub(crate) fn verify_target_offset_census(
    target_offset: &ExactTargetOffset,
    stats: &mut ExactRecenterKernelStats,
) -> Result<(), ExactRecenterKernelError> {
    let (observed_integer_bits, observed_arc_retained_bytes) =
        target_offset.authenticate_retained_census()?;
    if observed_integer_bits > stats.target_offset_integer_bits
        || target_offset.retained_bytes() > stats.target_offset_prospective_retained_bytes
        || observed_arc_retained_bytes > stats.target_offset_arc_retained_bytes
    {
        return Err(ExactRecenterKernelError::CensusMismatch);
    }
    stats.target_offset_observed_retained_bytes = target_offset.retained_bytes();
    stats.target_offset_observed_arc_retained_bytes = observed_arc_retained_bytes;
    Ok(())
}

/// Equality with charged GMP comparison work; target ordering/state stays in
/// the caller and is never accepted by this kernel.
pub(crate) fn exact_offsets_equal(
    left: &[Integer],
    right: &[Integer],
    limits: ExactRecenterKernelLimits,
    stats: &mut ExactRecenterKernelStats,
) -> Result<bool, ExactRecenterKernelError> {
    if left.len() != right.len() {
        return Ok(false);
    }
    for (left, right) in left.iter().zip(right) {
        stats.geometry_integer_operations = bounded_add(
            "exact recentering geometry integer operations",
            stats.geometry_integer_operations,
            1,
            limits.max_geometry_integer_operations,
        )?;
        let comparison_work = integer_bits(left)?.max(integer_bits(right)?).max(1);
        stats.geometry_integer_bit_work = bounded_add(
            "exact recentering geometry integer-bit work",
            stats.geometry_integer_bit_work,
            comparison_work,
            limits.max_geometry_integer_bit_work,
        )?;
        if left != right {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(crate) fn preflight_coefficient_translation(
    pivot: &GeneratedAffineResidualGroupLatticeShift,
    free_positions: &[usize],
    arity: usize,
    limits: ExactRecenterKernelLimits,
    stats: &mut ExactRecenterKernelStats,
) -> Result<(), ExactRecenterKernelError> {
    validate_free_positions(pivot, free_positions, arity)?;
    let mut total_bits = 0usize;
    let mut retained_bytes = arc_vec_retained_bytes_bound::<Integer>(arity)?;
    let mut free_cursor = 0usize;
    for position in 0..arity {
        let is_free = free_positions.get(free_cursor).copied() == Some(position);
        free_cursor += usize::from(is_free);
        if !is_free {
            continue;
        }
        let bits = integer_bits(&pivot.values()[position])?;
        check_limit(
            "exact recentering coefficient-translation integer bits",
            bits,
            limits.max_exact_integer_bits,
        )?;
        total_bits = checked_add(
            "exact recentering coefficient-translation integer bits",
            total_bits,
            bits,
        )?;
        retained_bytes = checked_add(
            "exact recentering coefficient-translation retained bytes",
            retained_bytes,
            prospective_integer_heap_bytes(bits)?,
        )?;
        stats.geometry_integer_operations = bounded_add(
            "exact recentering geometry integer operations",
            stats.geometry_integer_operations,
            1,
            limits.max_geometry_integer_operations,
        )?;
        stats.geometry_integer_bit_work = bounded_add(
            "exact recentering geometry integer-bit work",
            stats.geometry_integer_bit_work,
            bits.max(1),
            limits.max_geometry_integer_bit_work,
        )?;
    }
    check_limit(
        "exact recentering coefficient-translation integer bits",
        total_bits,
        limits.max_coefficient_translation_integer_bits,
    )?;
    check_limit(
        "exact recentering coefficient-translation retained bytes",
        retained_bytes,
        limits.max_coefficient_translation_retained_bytes,
    )?;
    stats.coefficient_translation_integer_bits = total_bits;
    stats.coefficient_translation_retained_bytes = retained_bytes;
    Ok(())
}

pub(crate) fn execute_coefficient_translation(
    pivot: &GeneratedAffineResidualGroupLatticeShift,
    free_positions: &[usize],
    arity: usize,
) -> Result<ExactCoefficientTranslation, ExactRecenterKernelError> {
    validate_free_positions(pivot, free_positions, arity)?;
    let mut output = try_vec("exact recentering coefficient translation", arity)?;
    let mut free_cursor = 0usize;
    for position in 0..arity {
        let is_free = free_positions.get(free_cursor).copied() == Some(position);
        free_cursor += usize::from(is_free);
        output.push(if is_free {
            canonical_integer(-&pivot.values()[position])
        } else {
            Integer::from(0)
        });
    }
    let (retained_integer_bits, retained_bytes) = integer_vec_owned_census(&output, true)?;
    Ok(ExactCoefficientTranslation {
        values: Arc::new(output),
        retained_integer_bits,
        retained_bytes,
    })
}

pub(crate) fn verify_coefficient_translation_census(
    translation: &ExactCoefficientTranslation,
    stats: &ExactRecenterKernelStats,
) -> Result<(), ExactRecenterKernelError> {
    if translation.retained_integer_bits() > stats.coefficient_translation_integer_bits
        || translation.retained_bytes() > stats.coefficient_translation_retained_bytes
    {
        return Err(ExactRecenterKernelError::CensusMismatch);
    }
    Ok(())
}

/// Sealed all-row admission produced before any centered GMP value exists.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ExactCenteredShiftAdmission {
    shift_count: usize,
    components: usize,
    subtraction_operations: usize,
    subtraction_integer_bit_work: usize,
    post_geometry_integer_operations: usize,
    post_geometry_integer_bit_work: usize,
    centered_shift_outer_buffer_bytes: usize,
    prospective_integer_bits: usize,
    prospective_retained_bytes: usize,
}

impl ExactCenteredShiftAdmission {
    pub(crate) const fn shift_count(&self) -> usize {
        self.shift_count
    }

    pub(crate) const fn components(&self) -> usize {
        self.components
    }

    pub(crate) const fn prospective_integer_bits(&self) -> usize {
        self.prospective_integer_bits
    }

    pub(crate) const fn prospective_retained_bytes(&self) -> usize {
        self.prospective_retained_bytes
    }

    fn authenticates_caller_stats(&self, stats: &ExactRecenterKernelStats) -> bool {
        stats.geometry_integer_operations == self.post_geometry_integer_operations
            && stats.geometry_integer_bit_work == self.post_geometry_integer_bit_work
            && stats.exact_shift_components == self.components
            && stats.centered_shift_outer_buffer_bytes == self.centered_shift_outer_buffer_bytes
            && stats.prospective_exact_shift_integer_bits == self.prospective_integer_bits
            && stats.prospective_exact_shift_retained_bytes == self.prospective_retained_bytes
    }
}

/// Admit every prospective component of `q = s - r` without allocating it.
pub(crate) fn preflight_centered_shifts(
    terms: &[ExactBorrowedTerm<'_>],
    pivot: &GeneratedAffineResidualGroupLatticeShift,
    limits: ExactRecenterKernelLimits,
    stats: &mut ExactRecenterKernelStats,
) -> Result<ExactCenteredShiftAdmission, ExactRecenterKernelError> {
    let shift_count = terms.len();
    let components = checked_mul(
        "exact recentering exact-shift components",
        shift_count,
        pivot.arity(),
    )?;
    check_limit(
        "exact recentering exact-shift components",
        components,
        limits.max_exact_shift_components,
    )?;
    let centered_shift_outer_buffer_bytes =
        vec_retained_bytes_bound::<ExactCenteredShift>(shift_count)?;
    check_limit(
        "exact recentering centered-shift outer buffer bytes",
        centered_shift_outer_buffer_bytes,
        limits.max_centered_shift_outer_buffer_bytes,
    )?;
    let mut prospective_integer_bits = 0usize;
    let mut prospective_retained_bytes = 0usize;
    let mut subtraction_operations = 0usize;
    let mut subtraction_integer_bit_work = 0usize;
    for &(shift, _) in terms {
        if shift.arity() != pivot.arity() {
            return Err(ExactRecenterKernelError::MalformedGeometry);
        }
        prospective_retained_bytes = checked_add(
            "exact recentering exact-shift retained bytes",
            prospective_retained_bytes,
            arc_vec_retained_bytes_bound::<Integer>(pivot.arity())?,
        )?;
        for (value, center) in shift.values().iter().zip(pivot.values()) {
            let value_bits = integer_bits(value)?;
            let center_bits = integer_bits(center)?;
            let bits = checked_add(
                "exact recentering exact-shift integer bits",
                value_bits.max(center_bits),
                1,
            )?;
            check_limit(
                "exact recentering exact-shift integer bits",
                bits,
                limits.max_exact_integer_bits,
            )?;
            prospective_integer_bits = checked_add(
                "exact recentering exact-shift integer bits",
                prospective_integer_bits,
                bits,
            )?;
            prospective_retained_bytes = checked_add(
                "exact recentering exact-shift retained bytes",
                prospective_retained_bytes,
                prospective_integer_heap_bytes(bits)?,
            )?;
            subtraction_operations = checked_add(
                "exact recentering geometry integer operations",
                subtraction_operations,
                1,
            )?;
            subtraction_integer_bit_work = checked_add(
                "exact recentering geometry integer-bit work",
                subtraction_integer_bit_work,
                exact_geometry_operation_bit_work(value_bits, center_bits, bits)?,
            )?;
        }
    }
    check_limit(
        "exact recentering exact-shift integer bits",
        prospective_integer_bits,
        limits.max_exact_shift_integer_bits,
    )?;
    check_limit(
        "exact recentering exact-shift retained bytes",
        prospective_retained_bytes,
        limits.max_exact_shift_retained_bytes,
    )?;
    let geometry_integer_operations = bounded_add(
        "exact recentering geometry integer operations",
        stats.geometry_integer_operations,
        subtraction_operations,
        limits.max_geometry_integer_operations,
    )?;
    let geometry_integer_bit_work = bounded_add(
        "exact recentering geometry integer-bit work",
        stats.geometry_integer_bit_work,
        subtraction_integer_bit_work,
        limits.max_geometry_integer_bit_work,
    )?;
    stats.geometry_integer_operations = geometry_integer_operations;
    stats.geometry_integer_bit_work = geometry_integer_bit_work;
    stats.exact_shift_components = components;
    stats.centered_shift_outer_buffer_bytes = centered_shift_outer_buffer_bytes;
    stats.prospective_exact_shift_integer_bits = prospective_integer_bits;
    stats.prospective_exact_shift_retained_bytes = prospective_retained_bytes;
    Ok(ExactCenteredShiftAdmission {
        shift_count,
        components,
        subtraction_operations,
        subtraction_integer_bit_work,
        post_geometry_integer_operations: geometry_integer_operations,
        post_geometry_integer_bit_work: geometry_integer_bit_work,
        centered_shift_outer_buffer_bytes,
        prospective_integer_bits,
        prospective_retained_bytes,
    })
}

/// Execute a sealed centered-shift admission and census the retained GMP data.
pub(crate) fn execute_centered_shifts(
    terms: &[ExactBorrowedTerm<'_>],
    pivot: &GeneratedAffineResidualGroupLatticeShift,
    admission: ExactCenteredShiftAdmission,
    limits: ExactRecenterKernelLimits,
    stats: &mut ExactRecenterKernelStats,
) -> Result<Vec<ExactCenteredShift>, ExactRecenterKernelError> {
    if !admission.authenticates_caller_stats(stats) {
        return Err(ExactRecenterKernelError::CensusMismatch);
    }
    // Replay into isolated statistics: it authenticates the sealed input
    // census without charging the owning caller a second time.
    let mut replay_stats = ExactRecenterKernelStats {
        geometry_integer_operations: admission
            .post_geometry_integer_operations
            .checked_sub(admission.subtraction_operations)
            .ok_or(ExactRecenterKernelError::CensusMismatch)?,
        geometry_integer_bit_work: admission
            .post_geometry_integer_bit_work
            .checked_sub(admission.subtraction_integer_bit_work)
            .ok_or(ExactRecenterKernelError::CensusMismatch)?,
        ..ExactRecenterKernelStats::default()
    };
    let replay = preflight_centered_shifts(terms, pivot, limits, &mut replay_stats)?;
    if replay != admission {
        return Err(ExactRecenterKernelError::CensusMismatch);
    }
    let mut output = try_vec("exact recentering centered shifts", admission.shift_count)?;
    let observed_outer_buffer_bytes =
        vec_retained_bytes_bound::<ExactCenteredShift>(output.capacity())?;
    if observed_outer_buffer_bytes > stats.centered_shift_outer_buffer_bytes {
        return Err(ExactRecenterKernelError::CensusMismatch);
    }
    let mut observed_bits = 0usize;
    let mut observed_bytes = 0usize;
    for &(shift, _) in terms {
        if shift.arity() != pivot.arity() {
            return Err(ExactRecenterKernelError::MalformedGeometry);
        }
        let mut values = try_vec("exact recentering centered-shift values", pivot.arity())?;
        let mut retained_bits = 0usize;
        for (value, center) in shift.values().iter().zip(pivot.values()) {
            #[cfg(test)]
            CENTERED_SHIFT_ARITHMETIC_OPERATIONS_FOR_TEST.with(|operations| {
                operations.set(operations.get().saturating_add(1));
            });
            let centered = canonical_integer(value - center);
            let bits = integer_bits(&centered)?;
            retained_bits = checked_add(
                "exact recentering exact-shift integer bits",
                retained_bits,
                bits,
            )?;
            values.push(centered);
        }
        let (censused_bits, retained_bytes) = integer_vec_owned_census(&values, true)?;
        if censused_bits != retained_bits {
            return Err(ExactRecenterKernelError::CensusMismatch);
        }
        observed_bits = checked_add(
            "exact recentering exact-shift integer bits",
            observed_bits,
            retained_bits,
        )?;
        observed_bytes = checked_add(
            "exact recentering exact-shift retained bytes",
            observed_bytes,
            retained_bytes,
        )?;
        output.push(ExactCenteredShift {
            values: Arc::new(values),
            retained_integer_bits: retained_bits,
            retained_bytes,
        });
    }
    if observed_bits > admission.prospective_integer_bits
        || observed_bytes > admission.prospective_retained_bytes
    {
        return Err(ExactRecenterKernelError::CensusMismatch);
    }
    stats.exact_shift_integer_bits = observed_bits;
    stats.exact_shift_retained_bytes = observed_bytes;
    Ok(output)
}

fn preflight_borrowed_reference_buffers(
    term_count: usize,
    guard_count: usize,
    limits: ExactRecenterKernelLimits,
    stats: &mut ExactRecenterKernelStats,
) -> Result<(), ExactRecenterKernelError> {
    let bytes = checked_add(
        "exact recentering borrowed-reference buffer bytes",
        vec_retained_bytes_bound::<ExactBorrowedTerm<'_>>(term_count)?,
        vec_retained_bytes_bound::<&ParametricNonZeroCondition>(guard_count)?,
    )?;
    check_limit(
        "exact recentering borrowed-reference buffer bytes",
        bytes,
        limits.max_borrowed_reference_buffer_bytes,
    )?;
    stats.borrowed_reference_buffer_bytes = bytes;
    Ok(())
}

fn materialize_borrowed_references<'a, I, G>(
    terms: I,
    guards: G,
    stats: &ExactRecenterKernelStats,
) -> Result<
    (
        Vec<ExactBorrowedTerm<'a>>,
        Vec<&'a ParametricNonZeroCondition>,
    ),
    ExactRecenterKernelError,
>
where
    I: Iterator<Item = ExactBorrowedTerm<'a>>,
    G: Iterator<Item = &'a ParametricNonZeroCondition>,
{
    let mut retained_terms = try_vec("exact recentering borrowed term references", stats.terms)?;
    for term in terms {
        if retained_terms.len() == stats.terms {
            return Err(ExactRecenterKernelError::CensusMismatch);
        }
        retained_terms.push(term);
    }
    let mut retained_guards = try_vec("exact recentering borrowed guard references", stats.guards)?;
    for guard in guards {
        if retained_guards.len() == stats.guards {
            return Err(ExactRecenterKernelError::CensusMismatch);
        }
        retained_guards.push(guard);
    }
    if retained_terms.len() != stats.terms || retained_guards.len() != stats.guards {
        return Err(ExactRecenterKernelError::CensusMismatch);
    }
    let observed_bytes = checked_add(
        "exact recentering borrowed-reference buffer bytes",
        vec_retained_bytes_bound::<ExactBorrowedTerm<'_>>(retained_terms.capacity())?,
        vec_retained_bytes_bound::<&ParametricNonZeroCondition>(retained_guards.capacity())?,
    )?;
    if observed_bytes > stats.borrowed_reference_buffer_bytes {
        return Err(ExactRecenterKernelError::CensusMismatch);
    }
    Ok((retained_terms, retained_guards))
}

/// Translate a borrowed centered row without knowing its source or owner.
///
/// `owner_size` is the fixed retained size of the caller's eventual wrapper.
/// The prospective/observed additional-output values count payload retained
/// outside the translated vectors.  Keeping them separate prevents temporary
/// GMP construction headroom from being reported as durable output bytes.
/// Existing live owner bytes and native scratch are admitted separately before
/// output allocation.
#[allow(clippy::too_many_arguments)]
pub(crate) fn translate_centered_row<'a, I, G>(
    context: &ParametricCoefficientContext,
    terms: I,
    guards: G,
    pivot: &GeneratedAffineResidualGroupLatticeShift,
    free_positions: &[usize],
    locator_origin: &GuardOrigin,
    owner_size: usize,
    additional_output_prospective_retained_bytes: usize,
    additional_output_observed_retained_bytes: usize,
    target_offset_retained_in_output: bool,
    external_live_retained_bytes: usize,
    external_native_scratch_bytes: usize,
    limits: ExactRecenterKernelLimits,
    stats: &mut ExactRecenterKernelStats,
) -> Result<ExactRecenteredRow, ExactRecenterKernelError>
where
    I: Iterator<Item = ExactBorrowedTerm<'a>> + ExactSizeIterator,
    G: Iterator<Item = &'a ParametricNonZeroCondition> + ExactSizeIterator,
{
    if terms.len() != stats.terms || guards.len() != stats.guards {
        return Err(ExactRecenterKernelError::CensusMismatch);
    }
    preflight_borrowed_reference_buffers(stats.terms, stats.guards, limits, stats)?;
    let reference_phase_output_bytes = checked_add(
        "exact recentering reference phase retained bytes",
        owner_size,
        additional_output_prospective_retained_bytes,
    )?;
    admit_combined_live(
        reference_phase_output_bytes,
        external_live_retained_bytes,
        limits,
        stats,
    )?;
    admit_native_phase(
        external_native_scratch_bytes,
        target_offset_retained_in_output,
        limits,
        stats,
    )?;
    let (retained_terms, retained_guards) = materialize_borrowed_references(terms, guards, stats)?;
    preflight_coefficient_translation(pivot, free_positions, pivot.arity(), limits, stats)?;
    let coefficient_phase_output_bytes = checked_add(
        "exact recentering coefficient phase retained bytes",
        checked_add(
            "exact recentering coefficient phase retained bytes",
            owner_size,
            additional_output_prospective_retained_bytes,
        )?,
        stats.coefficient_translation_retained_bytes,
    )?;
    admit_combined_live(
        coefficient_phase_output_bytes,
        external_live_retained_bytes,
        limits,
        stats,
    )?;
    admit_native_phase(
        external_native_scratch_bytes,
        target_offset_retained_in_output,
        limits,
        stats,
    )?;
    let coefficient_translation =
        execute_coefficient_translation(pivot, free_positions, pivot.arity())?;
    verify_coefficient_translation_census(&coefficient_translation, stats)?;
    let centered_admission = preflight_centered_shifts(&retained_terms, pivot, limits, stats)?;
    let admission = preflight_symbolica_translations::<ExactRecenteredTerm>(
        context,
        &retained_terms,
        &retained_guards,
        coefficient_translation.values(),
        additional_output_prospective_retained_bytes,
        locator_origin,
        owner_size,
        external_live_retained_bytes,
        external_native_scratch_bytes,
        target_offset_retained_in_output,
        limits,
        stats,
    )?;
    let centered_shifts =
        execute_centered_shifts(&retained_terms, pivot, centered_admission, limits, stats)?;

    let mut output_terms = try_vec("exact recentering output terms", stats.terms)?;
    let mut output_guards = try_vec("exact recentering output guards", stats.guards)?;
    for ((_, coefficient), shift) in retained_terms.iter().copied().zip(centered_shifts) {
        output_terms.push(ExactRecenteredTerm {
            shift,
            coefficient: context.translate_exact(
                coefficient,
                coefficient_translation.values(),
                limits.arithmetic,
            )?,
        });
    }
    for &condition in &retained_guards {
        let polynomial = context.translate_polynomial_exact(
            condition.polynomial(),
            coefficient_translation.values(),
            limits.arithmetic,
        )?;
        let origins = condition
            .origins()
            .iter()
            .cloned()
            .chain(std::iter::once(locator_origin.clone()));
        output_guards.push(context.nonzero_condition_with_origins_and_origin_limit(
            polynomial,
            origins,
            limits.arithmetic.exact_algebra,
            limits.arithmetic.max_guard_origins,
        )?);
    }
    let owner_retained_bytes = observed_output_bytes(
        owner_size,
        &output_terms,
        &output_guards,
        &coefficient_translation,
        additional_output_observed_retained_bytes,
    )?;
    check_limit(
        "exact recentering owner retained bytes",
        owner_retained_bytes,
        limits.max_owner_retained_bytes,
    )?;
    if owner_retained_bytes > admission.final_retained_output_bytes {
        return Err(ExactRecenterKernelError::OutputCensusMismatch);
    }
    let observed_combined_live_bytes = checked_add(
        "exact recentering combined live retained bytes",
        external_live_retained_bytes,
        owner_retained_bytes,
    )?;
    if observed_combined_live_bytes > stats.combined_live_retained_bytes {
        return Err(ExactRecenterKernelError::OutputCensusMismatch);
    }
    stats.owner_retained_bytes = owner_retained_bytes;
    Ok(ExactRecenteredRow {
        coefficient_translation,
        terms: Arc::new(output_terms),
        guards: Arc::new(output_guards),
        stats: *stats,
    })
}

#[allow(clippy::too_many_arguments)]
fn preflight_symbolica_translations<T>(
    context: &ParametricCoefficientContext,
    terms: &[ExactBorrowedTerm<'_>],
    guards: &[&ParametricNonZeroCondition],
    shift: &[Integer],
    additional_output_retained_bytes: usize,
    locator_origin: &GuardOrigin,
    owner_size: usize,
    external_live_retained_bytes: usize,
    external_native_scratch_bytes: usize,
    target_offset_retained_in_output: bool,
    limits: ExactRecenterKernelLimits,
    stats: &mut ExactRecenterKernelStats,
) -> Result<ExactTranslationAdmission, ExactRecenterKernelError> {
    let mut maximum_polynomial_bytes = 0usize;
    let mut final_bytes = owner_size;
    final_bytes = checked_add(
        "exact recentering final retained output bytes",
        final_bytes,
        arc_vec_retained_bytes_bound::<T>(terms.len())?,
    )?;
    final_bytes = checked_add(
        "exact recentering final retained output bytes",
        final_bytes,
        arc_vec_retained_bytes_bound::<ParametricNonZeroCondition>(guards.len())?,
    )?;
    for increment in [
        stats.prospective_exact_shift_retained_bytes,
        stats.coefficient_translation_retained_bytes,
        additional_output_retained_bytes,
    ] {
        final_bytes = checked_add(
            "exact recentering final retained output bytes",
            final_bytes,
            increment,
        )?;
    }
    for &(_, coefficient) in terms {
        let preflight =
            context.preflight_translate_coefficient_exact(coefficient, shift, limits.arithmetic)?;
        accumulate_coefficient_preflight(stats, preflight, limits)?;
        maximum_polynomial_bytes = maximum_polynomial_bytes
            .max(preflight.numerator().retained_output_byte_bound())
            .max(preflight.denominator().retained_output_byte_bound());
        final_bytes = checked_add(
            "exact recentering final retained output bytes",
            final_bytes,
            preflight.normalized_coefficient_byte_bound(),
        )?;
    }
    for &guard in guards {
        let prospective_origins = checked_add(
            "exact recentering guard-origin occurrences",
            guard.origins().len(),
            usize::from(!guard.origins().contains(locator_origin)),
        )?;
        check_limit(
            "exact recentering guard origins per condition",
            prospective_origins,
            limits.arithmetic.max_guard_origins,
        )?;
        stats.guard_origin_occurrences = bounded_add(
            "exact recentering guard-origin occurrences",
            stats.guard_origin_occurrences,
            prospective_origins,
            limits.max_guard_origin_occurrences,
        )?;
        let preflight = context.preflight_translate_polynomial_exact(
            guard.polynomial(),
            shift,
            limits.arithmetic,
        )?;
        accumulate_polynomial_preflight(stats, preflight, limits)?;
        maximum_polynomial_bytes =
            maximum_polynomial_bytes.max(preflight.retained_output_byte_bound());
        final_bytes = checked_add(
            "exact recentering final retained output bytes",
            final_bytes,
            preflight.retained_output_byte_bound(),
        )?;
        for origin in guard.origins() {
            final_bytes = checked_add(
                "exact recentering final retained output bytes",
                final_bytes,
                origin.retained_byte_bound().ok_or(
                    ExactRecenterKernelError::ResourceCountOverflow {
                        resource: "exact recentering final retained output bytes",
                    },
                )?,
            )?;
        }
        final_bytes = checked_add(
            "exact recentering final retained output bytes",
            final_bytes,
            locator_origin.retained_byte_bound().ok_or(
                ExactRecenterKernelError::ResourceCountOverflow {
                    resource: "exact recentering final retained output bytes",
                },
            )?,
        )?;
    }
    check_limit(
        "exact recentering translation retained output bytes",
        stats.translation_retained_output_bytes,
        limits.max_translation_retained_output_bytes,
    )?;
    check_limit(
        "exact recentering owner retained bytes",
        final_bytes,
        limits.max_owner_retained_bytes,
    )?;
    stats.prospective_owner_retained_bytes = final_bytes;
    admit_combined_live(final_bytes, external_live_retained_bytes, limits, stats)?;
    let native_temporary = checked_add(
        "exact recentering native temporary byte envelope",
        stats.combined_live_retained_bytes,
        checked_add(
            "exact recentering native temporary byte envelope",
            checked_mul(
                "exact recentering native temporary byte envelope",
                maximum_polynomial_bytes,
                3,
            )?,
            native_exact_scratch_bytes(
                stats,
                external_native_scratch_bytes,
                target_offset_retained_in_output,
            )?,
        )?,
    )?;
    check_limit(
        "exact recentering native temporary byte envelope",
        native_temporary,
        limits.max_native_temporary_byte_envelope,
    )?;
    stats.native_temporary_byte_envelope =
        stats.native_temporary_byte_envelope.max(native_temporary);
    Ok(ExactTranslationAdmission {
        final_retained_output_bytes: final_bytes,
    })
}

#[derive(Clone, Copy)]
struct ExactTranslationAdmission {
    final_retained_output_bytes: usize,
}

fn accumulate_coefficient_preflight(
    stats: &mut ExactRecenterKernelStats,
    preflight: ParametricCoefficientTranslationPreflight,
    limits: ExactRecenterKernelLimits,
) -> Result<(), ExactRecenterKernelError> {
    accumulate_translation_counts(
        stats,
        preflight.source_terms(),
        checked_add(
            "exact recentering source exponent entries",
            preflight.numerator().source_exponent_entries(),
            preflight.denominator().source_exponent_entries(),
        )?,
        preflight.output_term_bound(),
        checked_add(
            "exact recentering output exponent entries",
            preflight.numerator().output_exponent_entry_bound(),
            preflight.denominator().output_exponent_entry_bound(),
        )?,
        preflight.power_operation_bound(),
        preflight.integer_bit_work_bound(),
        preflight.normalized_coefficient_term_bound(),
        preflight.normalized_coefficient_byte_bound(),
        limits,
    )
}

fn accumulate_polynomial_preflight(
    stats: &mut ExactRecenterKernelStats,
    preflight: ParametricPolynomialTranslationPreflight,
    limits: ExactRecenterKernelLimits,
) -> Result<(), ExactRecenterKernelError> {
    accumulate_translation_counts(
        stats,
        preflight.source_terms(),
        preflight.source_exponent_entries(),
        preflight.output_term_bound(),
        preflight.output_exponent_entry_bound(),
        preflight.power_operation_bound(),
        preflight.integer_bit_work_bound(),
        preflight.retained_output_term_bound(),
        preflight.retained_output_byte_bound(),
        limits,
    )
}

#[allow(clippy::too_many_arguments)]
fn accumulate_translation_counts(
    stats: &mut ExactRecenterKernelStats,
    source_terms: usize,
    source_exponent_entries: usize,
    output_terms: usize,
    output_exponent_entries: usize,
    power_operations: usize,
    integer_bit_work: usize,
    normalized_terms: usize,
    retained_bytes: usize,
    limits: ExactRecenterKernelLimits,
) -> Result<(), ExactRecenterKernelError> {
    // Symbolica's exact execution repeats its allocation-free preflight.
    stats.translation_preflight_passes = bounded_add(
        "exact recentering translation preflight passes",
        stats.translation_preflight_passes,
        2,
        limits.max_translation_preflight_passes,
    )?;
    for (resource, field, increment, limit) in [
        (
            "exact recentering translation source terms",
            &mut stats.translation_source_terms,
            source_terms,
            limits.max_translation_source_terms,
        ),
        (
            "exact recentering translation source exponent entries",
            &mut stats.translation_source_exponent_entries,
            source_exponent_entries,
            limits.max_translation_source_exponent_entries,
        ),
        (
            "exact recentering translation output terms",
            &mut stats.translation_output_terms,
            output_terms,
            limits.max_translation_output_terms,
        ),
        (
            "exact recentering translation output exponent entries",
            &mut stats.translation_output_exponent_entries,
            output_exponent_entries,
            limits.max_translation_output_exponent_entries,
        ),
        (
            "exact recentering translation power operations",
            &mut stats.translation_power_operations,
            power_operations,
            limits.max_translation_power_operations,
        ),
        (
            "exact recentering translation integer-bit work",
            &mut stats.translation_integer_bit_work,
            integer_bit_work,
            limits.max_translation_integer_bit_work,
        ),
    ] {
        let doubled = checked_mul(resource, increment, 2)?;
        *field = bounded_add(resource, *field, doubled, limit)?;
    }
    stats.translation_normalized_terms = bounded_add(
        "exact recentering translation normalized terms",
        stats.translation_normalized_terms,
        normalized_terms,
        limits.max_translation_normalized_terms,
    )?;
    stats.translation_retained_output_bytes = bounded_add(
        "exact recentering translation retained output bytes",
        stats.translation_retained_output_bytes,
        retained_bytes,
        limits.max_translation_retained_output_bytes,
    )?;
    Ok(())
}

fn admit_combined_live(
    prospective_output_retained_bytes: usize,
    external_live_retained_bytes: usize,
    limits: ExactRecenterKernelLimits,
    stats: &mut ExactRecenterKernelStats,
) -> Result<(), ExactRecenterKernelError> {
    let combined = checked_add(
        "exact recentering combined live retained bytes",
        external_live_retained_bytes,
        prospective_output_retained_bytes,
    )?;
    check_limit(
        "exact recentering combined live retained bytes",
        combined,
        limits.max_combined_live_retained_bytes,
    )?;
    stats.combined_live_retained_bytes = stats.combined_live_retained_bytes.max(combined);
    Ok(())
}

fn admit_native_phase(
    external_native_scratch_bytes: usize,
    target_offset_retained_in_output: bool,
    limits: ExactRecenterKernelLimits,
    stats: &mut ExactRecenterKernelStats,
) -> Result<(), ExactRecenterKernelError> {
    let native = checked_add(
        "exact recentering native temporary byte envelope",
        stats.combined_live_retained_bytes,
        native_exact_scratch_bytes(
            stats,
            external_native_scratch_bytes,
            target_offset_retained_in_output,
        )?,
    )?;
    check_limit(
        "exact recentering native temporary byte envelope",
        native,
        limits.max_native_temporary_byte_envelope,
    )?;
    stats.native_temporary_byte_envelope = stats.native_temporary_byte_envelope.max(native);
    Ok(())
}

pub(crate) fn admit_inert_owner(
    owner_retained_bytes: usize,
    external_live_retained_bytes: usize,
    external_native_scratch_bytes: usize,
    target_offset_retained_in_output: bool,
    limits: ExactRecenterKernelLimits,
    stats: &mut ExactRecenterKernelStats,
) -> Result<(), ExactRecenterKernelError> {
    check_limit(
        "exact recentering owner retained bytes",
        owner_retained_bytes,
        limits.max_owner_retained_bytes,
    )?;
    stats.prospective_owner_retained_bytes = owner_retained_bytes;
    admit_combined_live(
        owner_retained_bytes,
        external_live_retained_bytes,
        limits,
        stats,
    )?;
    let native_temporary_byte_envelope = checked_add(
        "exact recentering native temporary byte envelope",
        stats.combined_live_retained_bytes,
        native_exact_scratch_bytes(
            stats,
            external_native_scratch_bytes,
            target_offset_retained_in_output,
        )?,
    )?;
    check_limit(
        "exact recentering native temporary byte envelope",
        native_temporary_byte_envelope,
        limits.max_native_temporary_byte_envelope,
    )?;
    stats.owner_retained_bytes = owner_retained_bytes;
    stats.native_temporary_byte_envelope = native_temporary_byte_envelope;
    Ok(())
}

/// Replace the prospective inert-owner census with the allocator-observed
/// retained size after construction, without changing the already-admitted
/// prospective or combined-live envelopes.
pub(crate) fn observe_inert_owner(
    observed_owner_retained_bytes: usize,
    external_live_retained_bytes: usize,
    stats: &mut ExactRecenterKernelStats,
) -> Result<(), ExactRecenterKernelError> {
    if observed_owner_retained_bytes > stats.prospective_owner_retained_bytes
        || checked_add(
            "exact recentering observed combined live retained bytes",
            external_live_retained_bytes,
            observed_owner_retained_bytes,
        )? > stats.combined_live_retained_bytes
    {
        return Err(ExactRecenterKernelError::OutputCensusMismatch);
    }
    stats.owner_retained_bytes = observed_owner_retained_bytes;
    Ok(())
}

pub(crate) fn native_exact_scratch_bytes(
    stats: &ExactRecenterKernelStats,
    external_native_scratch_bytes: usize,
    target_offset_retained_in_output: bool,
) -> Result<usize, ExactRecenterKernelError> {
    let resource = "exact recentering native temporary byte envelope";
    let mut bytes = external_native_scratch_bytes;
    let target_offset_temporary_bytes = if target_offset_retained_in_output {
        stats
            .target_offset_temporary_bytes
            .checked_sub(stats.target_offset_prospective_retained_bytes)
            .ok_or(ExactRecenterKernelError::ResourceCountOverflow { resource })?
    } else {
        stats.target_offset_temporary_bytes
    };
    for increment in [
        target_offset_temporary_bytes,
        stats.centered_shift_outer_buffer_bytes,
        stats.borrowed_reference_buffer_bytes,
    ] {
        bytes = checked_add(resource, bytes, increment)?;
    }
    Ok(bytes)
}

fn observed_output_bytes(
    owner_size: usize,
    terms: &Vec<ExactRecenteredTerm>,
    guards: &Vec<ParametricNonZeroCondition>,
    coefficient_translation: &ExactCoefficientTranslation,
    additional_output_retained_bytes: usize,
) -> Result<usize, ExactRecenterKernelError> {
    let mut bytes = owner_size;
    bytes = checked_add(
        "exact recentering observed output bytes",
        bytes,
        arc_vec_retained_bytes_bound::<ExactRecenteredTerm>(terms.capacity())?,
    )?;
    for term in terms {
        bytes = checked_add(
            "exact recentering observed output bytes",
            bytes,
            term.shift.retained_bytes(),
        )?;
        bytes = checked_add(
            "exact recentering observed output bytes",
            bytes,
            term.coefficient.owned_retained_byte_bound().ok_or(
                ExactRecenterKernelError::ResourceCountOverflow {
                    resource: "exact recentering observed output bytes",
                },
            )?,
        )?;
    }
    bytes = checked_add(
        "exact recentering observed output bytes",
        bytes,
        arc_vec_retained_bytes_bound::<ParametricNonZeroCondition>(guards.capacity())?,
    )?;
    for guard in guards {
        bytes = checked_add(
            "exact recentering observed output bytes",
            bytes,
            guard.owned_retained_byte_bound().ok_or(
                ExactRecenterKernelError::ResourceCountOverflow {
                    resource: "exact recentering observed output bytes",
                },
            )?,
        )?;
    }
    bytes = checked_add(
        "exact recentering observed output bytes",
        bytes,
        coefficient_translation.retained_bytes(),
    )?;
    checked_add(
        "exact recentering observed output bytes",
        bytes,
        additional_output_retained_bytes,
    )
}

fn canonical_integer(value: Integer) -> Integer {
    match value {
        Integer::Single(value) => Integer::from(value),
        Integer::Double(value) => Integer::from(value),
        Integer::Large(value) => Integer::from(value),
    }
}

fn exact_target_offset_wrapper_overhead() -> Result<usize, ExactRecenterKernelError> {
    size_of::<ExactTargetOffset>()
        .checked_sub(size_of::<Vec<Integer>>())
        .ok_or(ExactRecenterKernelError::ResourceCountOverflow {
            resource: "exact recentering target-offset wrapper bytes",
        })
}

pub(crate) fn integer_bits(value: &Integer) -> Result<usize, ExactRecenterKernelError> {
    integer_magnitude_bits(value).map_err(|_| ExactRecenterKernelError::ResourceCountOverflow {
        resource: "exact recentering integer bits",
    })
}

fn integer_owned_heap_bytes(value: &Integer) -> Result<usize, ExactRecenterKernelError> {
    match value {
        Integer::Single(_) | Integer::Double(_) => Ok(0),
        Integer::Large(value) => value.capacity().checked_add(7).map(|bits| bits / 8).ok_or(
            ExactRecenterKernelError::ResourceCountOverflow {
                resource: "exact recentering integer owned heap bytes",
            },
        ),
    }
}

pub(crate) fn prospective_integer_heap_bytes(
    bits: usize,
) -> Result<usize, ExactRecenterKernelError> {
    if bits <= i128::BITS as usize - 1 {
        Ok(0)
    } else {
        let limbs = checked_add("exact recentering integer retained bytes", bits, 191)? / 64;
        checked_mul(
            "exact recentering integer retained bytes",
            limbs,
            size_of::<u64>(),
        )
    }
}

fn integer_retained_bytes(bits: usize) -> Result<usize, ExactRecenterKernelError> {
    checked_add(
        "exact recentering integer retained bytes",
        size_of::<Integer>(),
        prospective_integer_heap_bytes(bits)?,
    )
}

fn arc_payload_control_and_padding_byte_bound<T>() -> Result<usize, ExactRecenterKernelError> {
    checked_add(
        "exact recentering retained bytes",
        checked_mul(
            "exact recentering retained bytes",
            2,
            size_of::<AtomicUsize>(),
        )?,
        checked_add(
            "exact recentering retained bytes",
            align_of::<T>().saturating_sub(1),
            size_of::<T>(),
        )?,
    )
}

pub(crate) fn vec_retained_bytes_bound<T>(
    capacity: usize,
) -> Result<usize, ExactRecenterKernelError> {
    checked_add(
        "exact recentering retained bytes",
        size_of::<Vec<T>>(),
        checked_mul("exact recentering retained bytes", capacity, size_of::<T>())?,
    )
}

pub(crate) fn arc_vec_retained_bytes_bound<T>(
    capacity: usize,
) -> Result<usize, ExactRecenterKernelError> {
    checked_add(
        "exact recentering retained bytes",
        arc_payload_control_and_padding_byte_bound::<Vec<T>>()?,
        checked_mul("exact recentering retained bytes", capacity, size_of::<T>())?,
    )
}

pub(crate) fn integer_vec_owned_census(
    values: &Vec<Integer>,
    retained_in_arc: bool,
) -> Result<(usize, usize), ExactRecenterKernelError> {
    let mut bits = 0usize;
    let mut bytes = if retained_in_arc {
        arc_vec_retained_bytes_bound::<Integer>(values.capacity())?
    } else {
        vec_retained_bytes_bound::<Integer>(values.capacity())?
    };
    for value in values {
        bits = checked_add(
            "exact recentering integer-vector bits",
            bits,
            integer_bits(value)?,
        )?;
        bytes = checked_add(
            "exact recentering integer-vector retained bytes",
            bytes,
            integer_owned_heap_bytes(value)?,
        )?;
    }
    Ok((bits, bytes))
}

pub(crate) fn try_vec<T>(
    resource: &'static str,
    capacity: usize,
) -> Result<Vec<T>, ExactRecenterKernelError> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(capacity)
        .map_err(|_| ExactRecenterKernelError::AllocationFailure { resource })?;
    Ok(output)
}

pub(crate) fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ExactRecenterKernelError> {
    left.checked_add(right)
        .ok_or(ExactRecenterKernelError::ResourceCountOverflow { resource })
}

pub(crate) fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ExactRecenterKernelError> {
    left.checked_mul(right)
        .ok_or(ExactRecenterKernelError::ResourceCountOverflow { resource })
}

pub(crate) fn bounded_add(
    resource: &'static str,
    current: usize,
    increment: usize,
    limit: usize,
) -> Result<usize, ExactRecenterKernelError> {
    let requested = checked_add(resource, current, increment)?;
    check_limit(resource, requested, limit)?;
    Ok(requested)
}

pub(crate) fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ExactRecenterKernelError> {
    if requested > limit {
        Err(ExactRecenterKernelError::ResourceLimit {
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
    use std::sync::Arc;

    use super::super::physical_key::{
        GeneratedAffineResidualGroupPhysicalFrame, GeneratedAffineResidualGroupPhysicalKeyLimits,
    };
    use super::*;
    use crate::generated_affine_residual_boolean_cover::{
        GeneratedAffineResidualBooleanCoverCompiler, GeneratedAffineResidualBooleanCoverLimits,
    };
    use crate::generated_affine_residual_source_authority::GeneratedAffineResidualSourceAuthority;
    use crate::solver::closure::case_inventory::{
        GeneratedAffineResidualCaseAuthority, GeneratedAffineResidualCaseAuthorityLimits,
        GeneratedAffineResidualCaseInventoryCompiler, GeneratedAffineResidualCaseInventoryLimits,
    };
    use crate::{
        AffineDenominator, CoefficientContext, GeneratedSectorDiscoveryCompiler,
        GeneratedSectorDiscoveryLimits, GeneratedSectorLiveLeafQueueCompiler,
        GeneratedSectorLiveLeafQueueLimits, IntegralFamily, IntegralOrderingPolicy,
        ParametricIbpGenerator, SectorMask,
    };

    fn equal_mass_two_loop_family(name: &str) -> IntegralFamily {
        let coefficients = CoefficientContext::new(["d", "m2"]);
        let zero = coefficients.zero();
        let one = coefficients.one();
        let minus_m2 = coefficients.parse("-m2").unwrap();
        IntegralFamily::new(
            name,
            vec!["k1".into(), "k2".into()],
            Vec::new(),
            coefficients.clone(),
            coefficients.parameter("d").unwrap(),
            vec![
                AffineDenominator::new(
                    minus_m2.clone(),
                    vec![one.clone(), zero.clone(), zero.clone()],
                ),
                AffineDenominator::new(
                    minus_m2.clone(),
                    vec![zero.clone(), zero.clone(), one.clone()],
                ),
                AffineDenominator::new(minus_m2, vec![one.clone(), coefficients.integer(2), one]),
            ],
            Vec::new(),
            vec![zero.clone(), zero.clone(), zero],
        )
        .unwrap()
    }

    fn exact_kernel_fixture(
        name: &str,
    ) -> (
        ParametricCoefficientContext,
        Arc<GeneratedAffineResidualGroupPhysicalFrame>,
        Vec<Integer>,
        Vec<usize>,
    ) {
        let family = equal_mass_two_loop_family(name);
        let context = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .context()
            .clone();
        let mut discovery_limits = GeneratedSectorDiscoveryLimits::default();
        discovery_limits.adaptive.max_search_depth = 0;
        let discovery = GeneratedSectorDiscoveryCompiler::compile(
            &family,
            &context,
            SectorMask::try_from_bit_string("011").unwrap(),
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            discovery_limits,
        )
        .unwrap();
        let mut queue_limits = GeneratedSectorLiveLeafQueueLimits::default();
        queue_limits.translation_radius = 0;
        queue_limits.max_translation_points = 1;
        let queue = Arc::new(
            GeneratedSectorLiveLeafQueueCompiler::compile(
                &family,
                &context,
                &discovery,
                queue_limits,
            )
            .unwrap(),
        );
        let boolean = Arc::new(
            GeneratedAffineResidualBooleanCoverCompiler::compile(
                &family,
                &context,
                GeneratedAffineResidualSourceAuthority::initial_global(queue),
                GeneratedAffineResidualBooleanCoverLimits::default(),
            )
            .unwrap(),
        );
        let inventory = Arc::new(
            GeneratedAffineResidualCaseInventoryCompiler::compile(
                &family,
                &context,
                boolean,
                GeneratedAffineResidualCaseInventoryLimits::default(),
            )
            .unwrap(),
        );
        let group_ordinal = (0..inventory.group_count())
            .max_by_key(|&ordinal| {
                inventory
                    .authenticated_group_view(&context, ordinal)
                    .unwrap()
                    .case_ordinals()
                    .len()
            })
            .unwrap();
        let group = inventory
            .authenticated_group_view(&context, group_ordinal)
            .unwrap();
        let matrix = group.compact_linear_coefficients().to_vec();
        let free_positions = group.free_positions().to_vec();
        let authority = Arc::new(
            GeneratedAffineResidualCaseAuthority::try_new(
                &family,
                &context,
                Arc::clone(&inventory),
                group.anchor_case_ordinal(),
                GeneratedAffineResidualCaseAuthorityLimits::default(),
            )
            .unwrap(),
        );
        let frame = Arc::new(
            GeneratedAffineResidualGroupPhysicalFrame::try_new(
                &family,
                &context,
                authority,
                GeneratedAffineResidualGroupPhysicalKeyLimits::default(),
            )
            .unwrap(),
        );
        (context, frame, matrix, free_positions)
    }

    fn lattice_shift(
        frame: &Arc<GeneratedAffineResidualGroupPhysicalFrame>,
        values: &[Integer],
    ) -> GeneratedAffineResidualGroupLatticeShift {
        frame
            .test_key_for_borrowed_physical_values(values)
            .unwrap()
            .shift()
            .clone()
    }

    fn cancellation_geometry() -> (
        GeneratedAffineResidualGroupLatticeShift,
        Vec<Integer>,
        Vec<usize>,
    ) {
        let (_, frame, matrix, free_positions) =
            exact_kernel_fixture("exact-kernel-target-offset-cancellation");
        let huge = Integer::from(1) << 4096_u32;
        let pivot = lattice_shift(&frame, &[huge, Integer::from(0), Integer::from(0)]);
        (pivot, matrix, free_positions)
    }

    #[test]
    fn defaults_preserve_legacy_gmp_limits_and_session_memory_headroom() {
        const LARGE: usize = 64_000_000_000;
        const VERY_LARGE: usize = 4_000_000_000_000_000_000;
        const GIB: usize = 1024 * 1024 * 1024;

        let limits = ExactRecenterKernelLimits::default();

        assert_eq!(limits.arithmetic, ParametricArithmeticLimits::default());
        assert_eq!(limits.max_terms, 16_000_000);
        assert_eq!(limits.max_guards, 16_000_000);
        assert_eq!(limits.max_geometry_integer_operations, LARGE);
        assert_eq!(limits.max_geometry_integer_bit_work, VERY_LARGE);
        assert_eq!(limits.max_target_offset_integer_bits, VERY_LARGE);
        assert_eq!(limits.max_target_offset_temporary_bytes, 128 * GIB);
        assert_eq!(limits.max_exact_integer_bits, VERY_LARGE);
        assert_eq!(limits.max_exact_shift_components, LARGE);
        assert_eq!(limits.max_exact_shift_integer_bits, VERY_LARGE);
        assert_eq!(limits.max_exact_shift_retained_bytes, 128 * GIB);
        assert_eq!(limits.max_centered_shift_outer_buffer_bytes, 16 * GIB);
        assert_eq!(limits.max_borrowed_reference_buffer_bytes, 16 * GIB);
        assert_eq!(limits.max_coefficient_translation_integer_bits, VERY_LARGE);
        assert_eq!(limits.max_coefficient_translation_retained_bytes, 128 * GIB);
        assert_eq!(limits.max_translation_preflight_passes, LARGE);
        assert_eq!(limits.max_translation_source_terms, VERY_LARGE);
        assert_eq!(limits.max_translation_source_exponent_entries, VERY_LARGE);
        assert_eq!(limits.max_translation_output_terms, VERY_LARGE);
        assert_eq!(limits.max_translation_output_exponent_entries, VERY_LARGE);
        assert_eq!(limits.max_translation_power_operations, VERY_LARGE);
        assert_eq!(limits.max_translation_integer_bit_work, VERY_LARGE);
        assert_eq!(limits.max_translation_normalized_terms, VERY_LARGE);
        assert_eq!(limits.max_translation_retained_output_bytes, 128 * GIB);
        assert_eq!(limits.max_guard_origin_occurrences, LARGE);

        assert_eq!(limits.max_owner_retained_bytes, 128 * GIB);
        assert_eq!(limits.max_combined_live_retained_bytes, 256 * GIB);
        assert_eq!(limits.max_native_temporary_byte_envelope, 512 * GIB);
        assert!(limits.max_owner_retained_bytes < limits.max_combined_live_retained_bytes);
        assert!(
            limits.max_combined_live_retained_bytes < limits.max_native_temporary_byte_envelope
        );
    }

    #[test]
    fn zero_arity_target_offset_census_includes_wrapper_fields() {
        let values = Vec::<Integer>::new();
        let (_, vector_bytes) = integer_vec_owned_census(&values, false).unwrap();
        let retained_bytes = checked_add(
            "test target-offset retained bytes",
            vector_bytes,
            exact_target_offset_wrapper_overhead().unwrap(),
        )
        .unwrap();
        assert_eq!(retained_bytes, size_of::<ExactTargetOffset>());
        assert_eq!(
            exact_target_offset_wrapper_overhead().unwrap(),
            size_of::<ExactTargetOffset>() - size_of::<Vec<Integer>>()
        );
    }

    #[test]
    fn target_offset_arc_census_excludes_the_outer_arc_handle() {
        let values = vec![Integer::from(1) << 512_u32, Integer::from(0)];
        let (retained_integer_bits, vector_bytes) =
            integer_vec_owned_census(&values, false).unwrap();
        let retained_bytes = checked_add(
            "test target-offset retained bytes",
            vector_bytes,
            exact_target_offset_wrapper_overhead().unwrap(),
        )
        .unwrap();
        let offset = ExactTargetOffset {
            values,
            retained_integer_bits,
            retained_bytes,
        };
        let arc_control_and_padding =
            arc_payload_control_and_padding_byte_bound::<ExactTargetOffset>().unwrap()
                - size_of::<ExactTargetOffset>();
        let deep_arc_bytes = offset.arc_retained_bytes().unwrap();

        assert_eq!(
            deep_arc_bytes,
            offset.retained_bytes() + arc_control_and_padding
        );
        let outer_owner_bytes = checked_add(
            "test target-offset outer owner bytes",
            size_of::<Arc<ExactTargetOffset>>(),
            deep_arc_bytes,
        )
        .unwrap();
        assert_eq!(
            outer_owner_bytes.checked_sub(deep_arc_bytes),
            Some(size_of::<Arc<ExactTargetOffset>>())
        );
        assert_ne!(
            deep_arc_bytes, outer_owner_bytes,
            "the Arc-backed payload census must not include its outer owner's inline Arc handle"
        );

        let mut stats = ExactRecenterKernelStats {
            target_offset_integer_bits: retained_integer_bits,
            target_offset_prospective_retained_bytes: retained_bytes,
            target_offset_arc_retained_bytes: deep_arc_bytes,
            ..ExactRecenterKernelStats::default()
        };
        verify_target_offset_census(&offset, &mut stats).unwrap();
        assert_eq!(
            stats.target_offset_observed_retained_bytes(),
            retained_bytes
        );
        assert_eq!(
            stats.target_offset_observed_arc_retained_bytes(),
            deep_arc_bytes
        );
    }

    #[test]
    fn retained_target_offset_enters_native_envelope_once_with_exact_limits() {
        let template = ExactRecenterKernelStats {
            target_offset_prospective_retained_bytes: 40,
            target_offset_temporary_bytes: 64,
            centered_shift_outer_buffer_bytes: 7,
            borrowed_reference_buffer_bytes: 11,
            ..ExactRecenterKernelStats::default()
        };
        let owner_retained_bytes = 101;
        let external_live_retained_bytes = 13;
        let external_native_scratch_bytes = 17;
        let combined_live = owner_retained_bytes + external_live_retained_bytes;
        let native_scratch = external_native_scratch_bytes
            + (template.target_offset_temporary_bytes()
                - template.target_offset_prospective_retained_bytes())
            + template.centered_shift_outer_buffer_bytes()
            + template.borrowed_reference_buffer_bytes();
        let native_envelope = combined_live + native_scratch;

        assert_eq!(
            native_exact_scratch_bytes(&template, external_native_scratch_bytes, true).unwrap(),
            native_scratch
        );
        assert_eq!(
            native_exact_scratch_bytes(&template, external_native_scratch_bytes, false).unwrap()
                - native_scratch,
            template.target_offset_prospective_retained_bytes(),
            "an owner that already retains the offset must add only its construction scratch"
        );

        let exact_limits = ExactRecenterKernelLimits {
            max_owner_retained_bytes: owner_retained_bytes,
            max_combined_live_retained_bytes: combined_live,
            max_native_temporary_byte_envelope: native_envelope,
            ..ExactRecenterKernelLimits::default()
        };
        let mut exact = template;
        admit_inert_owner(
            owner_retained_bytes,
            external_live_retained_bytes,
            external_native_scratch_bytes,
            true,
            exact_limits,
            &mut exact,
        )
        .unwrap();
        assert_eq!(
            exact.prospective_owner_retained_bytes(),
            owner_retained_bytes
        );
        assert_eq!(exact.owner_retained_bytes(), owner_retained_bytes);
        assert_eq!(exact.combined_live_retained_bytes(), combined_live);
        assert_eq!(exact.native_temporary_byte_envelope(), native_envelope);

        for (resource, limits) in [
            (
                "exact recentering owner retained bytes",
                ExactRecenterKernelLimits {
                    max_owner_retained_bytes: owner_retained_bytes - 1,
                    ..exact_limits
                },
            ),
            (
                "exact recentering combined live retained bytes",
                ExactRecenterKernelLimits {
                    max_combined_live_retained_bytes: combined_live - 1,
                    ..exact_limits
                },
            ),
            (
                "exact recentering native temporary byte envelope",
                ExactRecenterKernelLimits {
                    max_native_temporary_byte_envelope: native_envelope - 1,
                    ..exact_limits
                },
            ),
        ] {
            let mut rejected = template;
            assert!(matches!(
                admit_inert_owner(
                    owner_retained_bytes,
                    external_live_retained_bytes,
                    external_native_scratch_bytes,
                    true,
                    limits,
                    &mut rejected,
                ),
                Err(ExactRecenterKernelError::ResourceLimit {
                    resource: actual,
                    requested,
                    limit,
                }) if actual == resource && requested == limit + 1
            ));
        }
    }

    #[test]
    fn large_bit_cancellation_separates_temporary_and_observed_offset_bytes() {
        let (pivot, matrix, free_positions) = cancellation_geometry();
        let limits = ExactRecenterKernelLimits::default();
        let mut stats = ExactRecenterKernelStats::for_row(0, 0, limits).unwrap();
        preflight_exact_geometry(&pivot, &matrix, &free_positions, limits, &mut stats).unwrap();

        let huge_bits = integer_bits(&pivot.values()[0]).unwrap();
        let exact_bit_work = huge_bits
            .checked_mul(19)
            .and_then(|work| work.checked_add(25))
            .unwrap();
        assert_eq!(stats.geometry_integer_operations(), 9);
        assert_eq!(stats.geometry_integer_bit_work(), exact_bit_work);
        let exact_bits = stats.target_offset_integer_bits();
        let prospective_bytes = stats.target_offset_prospective_retained_bytes();
        let prospective_arc_bytes = stats.target_offset_arc_retained_bytes();
        let temporary_bytes = stats.target_offset_temporary_bytes();
        assert!(exact_bits > 4096);
        assert!(prospective_arc_bytes > prospective_bytes);
        assert!(temporary_bytes > prospective_bytes);

        let offset =
            execute_target_offset(&pivot, &matrix, &free_positions, pivot.arity()).unwrap();
        assert!(
            offset
                .values()
                .iter()
                .all(|value| value == &Integer::from(0))
        );
        verify_target_offset_census(&offset, &mut stats).unwrap();
        assert!(
            prospective_bytes > stats.target_offset_observed_retained_bytes(),
            "cancelling large GMP intermediates must not become retained zero limbs"
        );
        assert!(prospective_arc_bytes > stats.target_offset_observed_arc_retained_bytes());
        assert!(temporary_bytes > stats.target_offset_observed_retained_bytes());
        assert_eq!(
            stats.target_offset_observed_arc_retained_bytes(),
            offset.arc_retained_bytes().unwrap()
        );

        let exact_limits = ExactRecenterKernelLimits {
            max_target_offset_integer_bits: exact_bits,
            max_target_offset_temporary_bytes: temporary_bytes,
            max_geometry_integer_bit_work: exact_bit_work,
            ..limits
        };
        let mut exact_stats = ExactRecenterKernelStats::for_row(0, 0, exact_limits).unwrap();
        preflight_exact_geometry(
            &pivot,
            &matrix,
            &free_positions,
            exact_limits,
            &mut exact_stats,
        )
        .unwrap();
        assert_eq!(exact_stats.target_offset_integer_bits(), exact_bits);
        assert_eq!(exact_stats.target_offset_temporary_bytes(), temporary_bytes);
        assert_eq!(exact_stats.geometry_integer_bit_work(), exact_bit_work);

        for (resource, one_below, proves_no_arithmetic) in [
            (
                "exact recentering target-offset integer bits",
                ExactRecenterKernelLimits {
                    max_target_offset_integer_bits: exact_bits - 1,
                    ..limits
                },
                false,
            ),
            (
                "exact recentering target-offset temporary bytes",
                ExactRecenterKernelLimits {
                    max_target_offset_temporary_bytes: temporary_bytes - 1,
                    ..limits
                },
                false,
            ),
            (
                "exact recentering geometry integer-bit work",
                ExactRecenterKernelLimits {
                    max_geometry_integer_bit_work: exact_bit_work - 1,
                    ..limits
                },
                true,
            ),
        ] {
            let mut rejected = ExactRecenterKernelStats::for_row(0, 0, one_below).unwrap();
            reset_target_offset_arithmetic_entries_for_test();
            assert!(matches!(
                preflight_exact_geometry(
                    &pivot,
                    &matrix,
                    &free_positions,
                    one_below,
                    &mut rejected,
                ),
                Err(ExactRecenterKernelError::ResourceLimit {
                    resource: actual,
                    requested,
                    limit,
                }) if actual == resource && requested == limit + 1
            ));
            if proves_no_arithmetic {
                assert_eq!(
                    target_offset_arithmetic_entries_for_test(),
                    0,
                    "geometry work must reject before target-offset GMP arithmetic",
                );
            }
        }
    }

    #[test]
    fn centered_subtractions_have_exact_limits_and_execution_does_not_double_charge() {
        let (context, frame, _, _) = exact_kernel_fixture("exact-kernel-centered-work");
        let max = i64::MAX;
        let pivot = lattice_shift(
            &frame,
            &[
                Integer::from(7),
                Integer::from(max - 1),
                Integer::from(max - 1),
            ],
        );
        let second = lattice_shift(
            &frame,
            &[
                Integer::from(7),
                Integer::from(max - 2),
                Integer::from(max - 1),
            ],
        );
        let coefficient = context.one();
        let terms = [(&pivot, &coefficient), (&second, &coefficient)];
        let defaults = ExactRecenterKernelLimits::default();
        const PRIOR_OPERATIONS: usize = 15;
        const PRIOR_INTEGER_BIT_WORK: usize = 518;
        const CENTERED_OPERATIONS: usize = 6;
        const CENTERED_INTEGER_BIT_WORK: usize = 780;
        const EXPECTED_OPERATIONS: usize = PRIOR_OPERATIONS + CENTERED_OPERATIONS;
        const EXPECTED_INTEGER_BIT_WORK: usize = PRIOR_INTEGER_BIT_WORK + CENTERED_INTEGER_BIT_WORK;
        let initial_stats = ExactRecenterKernelStats {
            geometry_integer_operations: PRIOR_OPERATIONS,
            geometry_integer_bit_work: PRIOR_INTEGER_BIT_WORK,
            ..ExactRecenterKernelStats::default()
        };
        let mut baseline = initial_stats;
        let admission = preflight_centered_shifts(&terms, &pivot, defaults, &mut baseline).unwrap();

        assert_eq!(baseline.geometry_integer_operations(), EXPECTED_OPERATIONS);
        assert_eq!(
            baseline.geometry_integer_bit_work(),
            EXPECTED_INTEGER_BIT_WORK
        );

        let exact_limits = ExactRecenterKernelLimits {
            max_geometry_integer_operations: EXPECTED_OPERATIONS,
            max_geometry_integer_bit_work: EXPECTED_INTEGER_BIT_WORK,
            ..defaults
        };
        let mut exact_stats = initial_stats;
        let exact_admission =
            preflight_centered_shifts(&terms, &pivot, exact_limits, &mut exact_stats).unwrap();
        reset_centered_shift_arithmetic_operations_for_test();
        let centered = execute_centered_shifts(
            &terms,
            &pivot,
            exact_admission,
            exact_limits,
            &mut exact_stats,
        )
        .unwrap();
        assert_eq!(centered.len(), terms.len());
        assert_eq!(
            centered_shift_arithmetic_operations_for_test(),
            CENTERED_OPERATIONS
        );
        assert_eq!(
            exact_stats.geometry_integer_operations(),
            EXPECTED_OPERATIONS,
            "execution replay must not charge the caller twice",
        );
        assert_eq!(
            exact_stats.geometry_integer_bit_work(),
            EXPECTED_INTEGER_BIT_WORK,
            "execution replay must not charge the caller twice",
        );
        drop(admission);

        for (resource, one_below) in [
            (
                "exact recentering geometry integer operations",
                ExactRecenterKernelLimits {
                    max_geometry_integer_operations: EXPECTED_OPERATIONS - 1,
                    ..defaults
                },
            ),
            (
                "exact recentering geometry integer-bit work",
                ExactRecenterKernelLimits {
                    max_geometry_integer_bit_work: EXPECTED_INTEGER_BIT_WORK - 1,
                    ..defaults
                },
            ),
        ] {
            let mut rejected = initial_stats;
            reset_centered_shift_arithmetic_operations_for_test();
            assert!(matches!(
                preflight_centered_shifts(&terms, &pivot, one_below, &mut rejected),
                Err(ExactRecenterKernelError::ResourceLimit {
                    resource: actual,
                    requested,
                    limit,
                }) if actual == resource && requested == limit + 1
            ));
            assert_eq!(
                centered_shift_arithmetic_operations_for_test(),
                0,
                "one-below admission must reject before centered GMP subtraction",
            );
        }
    }

    #[test]
    fn centered_admission_rejects_same_shape_low_bit_census_before_arithmetic() {
        let (context, frame, _, _) = exact_kernel_fixture("exact-kernel-centered-binding");
        let huge = Integer::from(1) << 4096_u32;
        let high_pivot = lattice_shift(&frame, &[huge.clone(), Integer::from(0), Integer::from(0)]);
        let high_term = lattice_shift(
            &frame,
            &[&huge + Integer::from(1), Integer::from(1), Integer::from(0)],
        );
        let low_pivot = lattice_shift(
            &frame,
            &[Integer::from(1), Integer::from(0), Integer::from(0)],
        );
        let low_term = lattice_shift(
            &frame,
            &[Integer::from(2), Integer::from(1), Integer::from(0)],
        );
        let coefficient = context.one();
        let high_terms = [(&high_term, &coefficient)];
        let low_terms = [(&low_term, &coefficient)];
        let limits = ExactRecenterKernelLimits::default();
        let mut high_stats = ExactRecenterKernelStats::default();
        let high_admission =
            preflight_centered_shifts(&high_terms, &high_pivot, limits, &mut high_stats).unwrap();
        let mut low_stats = ExactRecenterKernelStats::default();
        let low_admission =
            preflight_centered_shifts(&low_terms, &low_pivot, limits, &mut low_stats).unwrap();

        assert_eq!(high_admission.shift_count(), low_admission.shift_count());
        assert_eq!(high_admission.components(), low_admission.components());
        assert_eq!(
            high_stats.centered_shift_outer_buffer_bytes(),
            low_stats.centered_shift_outer_buffer_bytes()
        );
        assert!(
            high_admission.prospective_integer_bits() > low_admission.prospective_integer_bits()
        );
        assert!(
            high_admission.prospective_retained_bytes()
                > low_admission.prospective_retained_bytes()
        );
        assert!(high_stats.geometry_integer_bit_work() > low_stats.geometry_integer_bit_work());

        let high_pivot_before = high_pivot.values().to_vec();
        let high_term_before = high_term.values().to_vec();
        let low_stats_before = low_stats;
        reset_centered_shift_arithmetic_operations_for_test();
        assert_eq!(
            execute_centered_shifts(
                &high_terms,
                &high_pivot,
                high_admission,
                limits,
                &mut low_stats,
            ),
            Err(ExactRecenterKernelError::CensusMismatch)
        );
        assert_eq!(centered_shift_arithmetic_operations_for_test(), 0);
        assert_eq!(low_stats, low_stats_before);
        assert_eq!(high_pivot.values(), high_pivot_before);
        assert_eq!(high_term.values(), high_term_before);
    }

    #[test]
    fn centered_and_borrowed_reference_buffers_have_exact_one_below_limits() {
        let (context, frame, _, _) = exact_kernel_fixture("exact-kernel-buffer-envelopes");
        let max = i64::MAX;
        let pivot = lattice_shift(
            &frame,
            &[
                Integer::from(7),
                Integer::from(max - 1),
                Integer::from(max - 1),
            ],
        );
        let second = lattice_shift(
            &frame,
            &[
                Integer::from(7),
                Integer::from(max - 2),
                Integer::from(max - 1),
            ],
        );
        let coefficient = context.one();
        let terms = [(&pivot, &coefficient), (&second, &coefficient)];
        let defaults = ExactRecenterKernelLimits::default();
        let mut baseline = ExactRecenterKernelStats::for_row(terms.len(), 1, defaults).unwrap();
        preflight_borrowed_reference_buffers(terms.len(), 1, defaults, &mut baseline).unwrap();
        preflight_centered_shifts(&terms, &pivot, defaults, &mut baseline).unwrap();
        let centered_demand = baseline.centered_shift_outer_buffer_bytes();
        let reference_demand = baseline.borrowed_reference_buffer_bytes();
        assert!(centered_demand > size_of::<Vec<ExactCenteredShift>>());
        assert!(reference_demand > size_of::<Vec<ExactBorrowedTerm<'_>>>());

        let exact_limits = ExactRecenterKernelLimits {
            max_centered_shift_outer_buffer_bytes: centered_demand,
            max_borrowed_reference_buffer_bytes: reference_demand,
            ..defaults
        };
        let mut exact = ExactRecenterKernelStats::for_row(terms.len(), 1, exact_limits).unwrap();
        preflight_borrowed_reference_buffers(terms.len(), 1, exact_limits, &mut exact).unwrap();
        preflight_centered_shifts(&terms, &pivot, exact_limits, &mut exact).unwrap();
        assert_eq!(exact.centered_shift_outer_buffer_bytes(), centered_demand);
        assert_eq!(exact.borrowed_reference_buffer_bytes(), reference_demand);

        let centered_one_below = ExactRecenterKernelLimits {
            max_centered_shift_outer_buffer_bytes: centered_demand - 1,
            ..exact_limits
        };
        let mut rejected =
            ExactRecenterKernelStats::for_row(terms.len(), 1, centered_one_below).unwrap();
        assert!(matches!(
            preflight_centered_shifts(&terms, &pivot, centered_one_below, &mut rejected),
            Err(ExactRecenterKernelError::ResourceLimit {
                resource: "exact recentering centered-shift outer buffer bytes",
                requested,
                limit,
            }) if requested == centered_demand && limit + 1 == centered_demand
        ));

        let reference_one_below = ExactRecenterKernelLimits {
            max_borrowed_reference_buffer_bytes: reference_demand - 1,
            ..exact_limits
        };
        let mut rejected =
            ExactRecenterKernelStats::for_row(terms.len(), 1, reference_one_below).unwrap();
        assert!(matches!(
            preflight_borrowed_reference_buffers(
                terms.len(),
                1,
                reference_one_below,
                &mut rejected,
            ),
            Err(ExactRecenterKernelError::ResourceLimit {
                resource: "exact recentering borrowed-reference buffer bytes",
                requested,
                limit,
            }) if requested == reference_demand && limit + 1 == reference_demand
        ));
    }

    #[test]
    fn native_scratch_categories_exclude_retained_output_payloads() {
        let stats = ExactRecenterKernelStats {
            target_offset_prospective_retained_bytes: 7,
            target_offset_temporary_bytes: 11,
            centered_shift_outer_buffer_bytes: 13,
            borrowed_reference_buffer_bytes: 17,
            prospective_exact_shift_retained_bytes: 19,
            exact_shift_retained_bytes: 23,
            coefficient_translation_retained_bytes: 29,
            ..ExactRecenterKernelStats::default()
        };
        assert_eq!(native_exact_scratch_bytes(&stats, 5, false).unwrap(), 46);
        assert_eq!(native_exact_scratch_bytes(&stats, 5, true).unwrap(), 39);
    }
}
