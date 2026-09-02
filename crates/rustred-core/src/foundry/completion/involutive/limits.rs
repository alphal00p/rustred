use crate::algebra::IndexedAlgebraLimits;

use super::InvolutiveError;
use super::error::{check_limit, checked_add};

/// Retained-shape and cumulative-work envelope for one proposal-only Janet
/// completion calculation.
///
/// Chart lifting has its own outer [`super::OrdinaryChartLiftLimits`]. These
/// limits govern each lifted consequence admitted here and, once completion
/// starts, all normal forms, autoreductions, and prolongations in that one run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct InvolutiveLimits {
    pub(crate) max_arity: usize,
    pub(crate) max_shift_coordinate: u64,
    pub(crate) max_total_shift_degree: usize,
    pub(crate) max_row_terms: usize,
    pub(crate) max_provenance_terms: usize,
    pub(crate) max_axpy_input_terms: usize,
    pub(crate) max_consequence_coefficient_terms: usize,
    pub(crate) max_consequence_coefficient_exponent_cells: usize,
    pub(crate) max_consequence_coefficient_retained_bytes: usize,
    pub(crate) max_localization_guards: usize,
    pub(crate) max_localization_guard_terms: usize,
    pub(crate) max_localization_guard_exponent_cells: usize,
    pub(crate) max_localization_guard_retained_bytes: usize,
    pub(crate) max_basis_rows: usize,
    pub(crate) max_basis_coordinate_cells: usize,
    pub(crate) max_basis_coefficient_terms: usize,
    pub(crate) max_basis_coefficient_exponent_cells: usize,
    pub(crate) max_basis_coefficient_retained_bytes: usize,
    pub(crate) max_initial_sort_comparisons: usize,
    pub(crate) max_initial_sort_payload_visits: usize,
    pub(crate) max_initial_pivot_head_comparisons: usize,
    pub(crate) max_initial_pivot_head_coordinate_visits: usize,
    pub(crate) max_initial_pivot_insertion_moves: usize,
    pub(crate) max_mask_prefix_comparisons: usize,
    pub(crate) max_mask_sort_coordinate_comparisons: usize,
    pub(crate) max_mask_retained_bytes: usize,
    pub(crate) max_prolongations: usize,
    pub(crate) max_prolongation_coordinate_cells: usize,
    pub(crate) max_prolongation_retained_bytes: usize,
    pub(crate) max_priority_candidates: usize,
    pub(crate) max_blind_priority_intersection_cells: usize,
    pub(crate) max_blind_priority_sort_coordinate_comparisons: usize,
    pub(crate) max_blind_priority_retained_bytes: usize,
    pub(crate) max_blind_boxes_scanned: usize,
    pub(crate) max_blind_boxes_retained: usize,
    pub(crate) max_blind_coordinate_cells: usize,
    pub(crate) max_epoch: u64,
    pub(crate) max_normal_form_steps: usize,
    pub(crate) max_normal_form_divisor_visits: usize,
    pub(crate) max_normal_form_trace_bytes: usize,
    pub(crate) max_completion_iterations: usize,
    pub(crate) max_autoreduction_passes: usize,
    pub(crate) max_exact_coefficient_operations: usize,
    pub(crate) indexed_algebra: IndexedAlgebraLimits,
}

impl Default for InvolutiveLimits {
    fn default() -> Self {
        Self {
            max_arity: 4_096,
            // Ore coefficient automorphisms currently use the checked i64
            // translation API. Keep the monoid carrier symmetric rather than
            // admitting the one extra negative endpoint only on inactive axes.
            max_shift_coordinate: i64::MAX as u64,
            max_total_shift_degree: 16_777_216,
            max_row_terms: 1_000_000,
            max_provenance_terms: 1_000_000,
            max_axpy_input_terms: 2_000_000,
            max_consequence_coefficient_terms: 64_000_000,
            max_consequence_coefficient_exponent_cells: 1_000_000_000,
            max_consequence_coefficient_retained_bytes: 2_147_483_648,
            max_localization_guards: 1_000_000,
            max_localization_guard_terms: 4_000_000,
            max_localization_guard_exponent_cells: 64_000_000,
            max_localization_guard_retained_bytes: 536_870_912,
            max_basis_rows: 1_000_000,
            max_basis_coordinate_cells: 64_000_000,
            max_basis_coefficient_terms: 1_000_000_000,
            max_basis_coefficient_exponent_cells: 8_000_000_000,
            max_basis_coefficient_retained_bytes: 17_179_869_184,
            max_initial_sort_comparisons: 1_000_000_000,
            max_initial_sort_payload_visits: 8_000_000_000,
            max_initial_pivot_head_comparisons: 1_000_000_000,
            max_initial_pivot_head_coordinate_visits: 4_096_000_000_000,
            max_initial_pivot_insertion_moves: 500_000_000_000,
            max_mask_prefix_comparisons: 1_000_000_000,
            max_mask_sort_coordinate_comparisons: 1_000_000_000,
            max_mask_retained_bytes: 536_870_912,
            max_prolongations: 16_000_000,
            max_prolongation_coordinate_cells: 64_000_000,
            max_prolongation_retained_bytes: 1_073_741_824,
            max_priority_candidates: 16_000_000,
            max_blind_priority_intersection_cells: 1_000_000_000,
            max_blind_priority_sort_coordinate_comparisons: 1_000_000_000,
            max_blind_priority_retained_bytes: 1_073_741_824,
            max_blind_boxes_scanned: 262_144,
            max_blind_boxes_retained: 65_536,
            max_blind_coordinate_cells: 4_194_304,
            max_epoch: u64::MAX,
            max_normal_form_steps: 1_000_000,
            max_normal_form_divisor_visits: 1_000_000_000,
            max_normal_form_trace_bytes: 536_870_912,
            max_completion_iterations: 1_000_000,
            max_autoreduction_passes: 4_096,
            max_exact_coefficient_operations: 1_000_000_000,
            indexed_algebra: IndexedAlgebraLimits::default(),
        }
    }
}

/// Cumulative logical work charged by one completion calculation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct InvolutiveWorkCensus {
    normal_form_steps: usize,
    normal_form_divisor_visits: usize,
    normal_form_trace_bytes: usize,
    autoreduction_passes: usize,
    completion_iterations: usize,
    exact_coefficient_operations: usize,
}

impl InvolutiveWorkCensus {
    pub(crate) const fn normal_form_steps(self) -> usize {
        self.normal_form_steps
    }

    pub(crate) const fn normal_form_divisor_visits(self) -> usize {
        self.normal_form_divisor_visits
    }

    pub(crate) const fn normal_form_trace_bytes(self) -> usize {
        self.normal_form_trace_bytes
    }

    pub(crate) const fn autoreduction_passes(self) -> usize {
        self.autoreduction_passes
    }

    pub(crate) const fn completion_iterations(self) -> usize {
        self.completion_iterations
    }

    pub(crate) const fn exact_coefficient_operations(self) -> usize {
        self.exact_coefficient_operations
    }
}

/// Mutable admission ledger shared by every nested operation in one proposal.
#[derive(Debug, Default)]
pub(super) struct InvolutiveWorkBudget {
    census: InvolutiveWorkCensus,
}

impl InvolutiveWorkBudget {
    pub(super) fn census(&self) -> InvolutiveWorkCensus {
        self.census
    }

    pub(super) fn charge_normal_form_step(
        &mut self,
        limits: InvolutiveLimits,
    ) -> Result<(), InvolutiveError> {
        let result = charge(
            "Janet normal-form steps",
            &mut self.census.normal_form_steps,
            1,
            limits.max_normal_form_steps,
        );
        self.record_typed_stop(&result);
        result
    }

    pub(super) fn charge_divisor_visit(
        &mut self,
        limits: InvolutiveLimits,
    ) -> Result<(), InvolutiveError> {
        let result = charge(
            "Janet normal-form divisor visits",
            &mut self.census.normal_form_divisor_visits,
            1,
            limits.max_normal_form_divisor_visits,
        );
        self.record_typed_stop(&result);
        result
    }

    pub(super) fn charge_trace_bytes(
        &mut self,
        amount: usize,
        limits: InvolutiveLimits,
    ) -> Result<(), InvolutiveError> {
        let result = charge(
            "Janet normal-form trace bytes",
            &mut self.census.normal_form_trace_bytes,
            amount,
            limits.max_normal_form_trace_bytes,
        );
        self.record_typed_stop(&result);
        result
    }

    pub(super) fn charge_autoreduction_pass(
        &mut self,
        limits: InvolutiveLimits,
    ) -> Result<(), InvolutiveError> {
        let result = charge(
            "Janet autoreduction passes",
            &mut self.census.autoreduction_passes,
            1,
            limits.max_autoreduction_passes,
        );
        self.record_typed_stop(&result);
        result
    }

    pub(super) fn charge_completion_iteration(
        &mut self,
        limits: InvolutiveLimits,
    ) -> Result<(), InvolutiveError> {
        let result = charge(
            "Janet completion iterations",
            &mut self.census.completion_iterations,
            1,
            limits.max_completion_iterations,
        );
        self.record_typed_stop(&result);
        result
    }

    pub(super) fn charge_exact_coefficient_operations(
        &mut self,
        amount: usize,
        limits: InvolutiveLimits,
    ) -> Result<(), InvolutiveError> {
        let result = charge(
            "Janet exact coefficient operations",
            &mut self.census.exact_coefficient_operations,
            amount,
            limits.max_exact_coefficient_operations,
        );
        self.record_typed_stop(&result);
        result
    }

    fn record_typed_stop(&self, result: &Result<(), InvolutiveError>) {
        #[cfg(test)]
        if result.is_err() {
            super::diagnostics::record_work_at_typed_stop(self.census);
        }
        #[cfg(not(test))]
        let _ = result;
    }
}

fn charge(
    resource: &'static str,
    consumed: &mut usize,
    amount: usize,
    limit: usize,
) -> Result<(), InvolutiveError> {
    let requested = checked_add(resource, *consumed, amount)?;
    check_limit(resource, requested, limit)?;
    *consumed = requested;
    Ok(())
}
