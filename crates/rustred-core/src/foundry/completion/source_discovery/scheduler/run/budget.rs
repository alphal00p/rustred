//! Aggregate and local outer-resource accounting.

use super::super::{
    ProbeLocalBudgetCause, ProbeLocalBudgetScope, ProbeLocalRunCensus, ProbeLocalSchedulerError,
    ProbeLocalSchedulerLimits,
};
use crate::foundry::completion::source_discovery::{
    ObstructionBlockNominationUpperBound, ProbeRowEvaluationCacheTelemetry,
};

const AGGREGATE_EPOCHS: &str = "probe-local aggregate fresh epochs";
const AGGREGATE_EPOCH_REQUEST_WORK: &str = "probe-local aggregate epoch request work";
pub(super) const AGGREGATE_MATERIALIZED_SOURCE_TERMS: &str =
    "probe-local aggregate materialized source terms";
const AGGREGATE_MODULAR_ENTRY_WORK: &str = "probe-local aggregate modular entry work";
const AGGREGATE_RESIDUAL_CANDIDATE_WORK: &str = "probe-local aggregate residual candidate work";
pub(super) const AGGREGATE_RESIDUAL_SOURCE_TERM_WORK: &str =
    "probe-local aggregate residual source-term work";
const AGGREGATE_PROSPECTIVE_CLASSIFICATION_WORK: &str =
    "probe-local aggregate prospective classification work";
const AGGREGATE_BLOCK_NOMINATION_RAW_ENTRIES: &str =
    "probe-local aggregate obstruction-block nomination raw-entry reservation";
const AGGREGATE_BLOCK_NOMINATION_RAW_REQUESTS: &str =
    "probe-local aggregate obstruction-block nomination raw-request reservation";
const AGGREGATE_BLOCK_NOMINATION_COORDINATES: &str =
    "probe-local aggregate obstruction-block nomination coordinate-cell reservation";
const AGGREGATE_BLOCK_NOMINATION_COEFFICIENTS: &str =
    "probe-local aggregate obstruction-block nomination coefficient-cell reservation";
const AGGREGATE_BLOCK_NOMINATION_CANONICALIZATION: &str =
    "probe-local aggregate obstruction-block nomination canonicalization-work reservation";
const AGGREGATE_BLOCK_NOMINATION_SUBSET: &str =
    "probe-local aggregate obstruction-block nomination subset-comparison reservation";
const AGGREGATE_BLOCK_CANDIDATE_WORK: &str =
    "probe-local aggregate obstruction-block candidate work";
const AGGREGATE_BLOCK_SOURCE_TERM_WORK: &str =
    "probe-local aggregate obstruction-block source-term work";
const AGGREGATE_BLOCK_SIGNATURE_WORK: &str =
    "probe-local aggregate obstruction-block signature work";
const AGGREGATE_BLOCK_SELECTION_WORK: &str =
    "probe-local aggregate obstruction-block selection work";
const AGGREGATE_CACHE_ROWS: &str = "probe-local aggregate row-cache rows";
const AGGREGATE_CACHE_VALUE_CELLS: &str = "probe-local aggregate row-cache value cells";
const AGGREGATE_CACHE_LOGICAL_ROWS: &str = "probe-local aggregate row-cache logical rows";
const AGGREGATE_CACHE_LOGICAL_VALUE_CELLS: &str =
    "probe-local aggregate row-cache logical value cells";
const AGGREGATE_CACHE_LOOKUPS: &str = "probe-local aggregate row-cache lookup comparisons";
const AGGREGATE_CACHE_PHYSICAL: &str = "probe-local aggregate row-cache physical evaluations";
const AGGREGATE_CACHE_HITS: &str = "probe-local aggregate row-cache hits";
const AGGREGATE_CACHE_INSERTION_MOVES: &str = "probe-local aggregate row-cache insertion moves";
const AGGREGATE_MERGE_REQUEST_WORK: &str = "probe-local aggregate merge request work";
pub(super) const ITERATION_RECORDS: &str = "probe-local retained iteration records";
const EXACT_ATTEMPTS: &str = "probe-local exact-lift attempts";

#[derive(Default)]
pub(super) struct RunBudget {
    epochs: usize,
    epoch_request_work: usize,
    materialized_source_terms: usize,
    modular_entry_work: usize,
    residual_candidate_work: usize,
    residual_source_term_work: usize,
    prospective_classification_reservation: usize,
    obstruction_block_nomination_raw_entry_reservation: usize,
    obstruction_block_nomination_raw_request_reservation: usize,
    obstruction_block_nomination_coordinate_cell_reservation: usize,
    obstruction_block_nomination_coefficient_cell_reservation: usize,
    obstruction_block_nomination_canonicalization_work_reservation: usize,
    obstruction_block_nomination_subset_comparison_reservation: usize,
    obstruction_block_candidate_work: usize,
    obstruction_block_source_term_work: usize,
    obstruction_block_signature_work: usize,
    obstruction_block_selection_work: usize,
    row_cache_rows: usize,
    row_cache_value_cells: usize,
    row_cache_logical_rows: usize,
    row_cache_logical_value_cells: usize,
    row_cache_lookup_comparisons: usize,
    row_cache_physical_evaluations: usize,
    row_cache_hits: usize,
    row_cache_insertion_moves: usize,
    merge_request_work: usize,
    iteration_records: usize,
    exact_attempts: usize,
}

impl RunBudget {
    pub(super) fn try_admit_epoch(
        &mut self,
        request_count: usize,
        source_term_work: usize,
        limits: ProbeLocalSchedulerLimits,
    ) -> Result<(), ProbeLocalBudgetCause> {
        let epochs = checked_budget_add(
            ProbeLocalBudgetScope::Aggregate,
            AGGREGATE_EPOCHS,
            self.epochs,
            1,
        )?;
        check_outer(
            ProbeLocalBudgetScope::Aggregate,
            AGGREGATE_EPOCHS,
            epochs,
            limits.max_aggregate_epochs,
        )?;
        let work = checked_budget_add(
            ProbeLocalBudgetScope::Aggregate,
            AGGREGATE_EPOCH_REQUEST_WORK,
            self.epoch_request_work,
            request_count,
        )?;
        check_outer(
            ProbeLocalBudgetScope::Aggregate,
            AGGREGATE_EPOCH_REQUEST_WORK,
            work,
            limits.max_aggregate_epoch_request_work,
        )?;
        let terms = checked_budget_add(
            ProbeLocalBudgetScope::Aggregate,
            AGGREGATE_MATERIALIZED_SOURCE_TERMS,
            self.materialized_source_terms,
            source_term_work,
        )?;
        check_outer(
            ProbeLocalBudgetScope::Aggregate,
            AGGREGATE_MATERIALIZED_SOURCE_TERMS,
            terms,
            limits.max_aggregate_materialized_source_terms,
        )?;
        self.epochs = epochs;
        self.epoch_request_work = work;
        self.materialized_source_terms = terms;
        Ok(())
    }

    pub(super) fn try_admit_modular_work(
        &mut self,
        physical_entries: usize,
        limits: ProbeLocalSchedulerLimits,
    ) -> Result<(), ProbeLocalBudgetCause> {
        let work = checked_budget_add(
            ProbeLocalBudgetScope::Aggregate,
            AGGREGATE_MODULAR_ENTRY_WORK,
            self.modular_entry_work,
            physical_entries,
        )?;
        check_outer(
            ProbeLocalBudgetScope::Aggregate,
            AGGREGATE_MODULAR_ENTRY_WORK,
            work,
            limits.max_aggregate_modular_entry_work,
        )?;
        self.modular_entry_work = work;
        Ok(())
    }

    /// Reserve every term in the nominated residual source batch before the
    /// exhaustive census begins. The same exact term count is a conservative
    /// upper bound for the more expensive prospective classifier, which is
    /// actually run only for retained nonzero rows.
    pub(super) fn try_admit_residual_work(
        &mut self,
        candidate_work: usize,
        source_term_work: usize,
        limits: ProbeLocalSchedulerLimits,
    ) -> Result<(), ProbeLocalBudgetCause> {
        let candidates = checked_budget_add(
            ProbeLocalBudgetScope::Aggregate,
            AGGREGATE_RESIDUAL_CANDIDATE_WORK,
            self.residual_candidate_work,
            candidate_work,
        )?;
        check_outer(
            ProbeLocalBudgetScope::Aggregate,
            AGGREGATE_RESIDUAL_CANDIDATE_WORK,
            candidates,
            limits.max_aggregate_residual_candidate_work,
        )?;
        let source_terms = checked_budget_add(
            ProbeLocalBudgetScope::Aggregate,
            AGGREGATE_RESIDUAL_SOURCE_TERM_WORK,
            self.residual_source_term_work,
            source_term_work,
        )?;
        check_outer(
            ProbeLocalBudgetScope::Aggregate,
            AGGREGATE_RESIDUAL_SOURCE_TERM_WORK,
            source_terms,
            limits.max_aggregate_residual_source_term_work,
        )?;
        let classifications = checked_budget_add(
            ProbeLocalBudgetScope::Aggregate,
            AGGREGATE_PROSPECTIVE_CLASSIFICATION_WORK,
            self.prospective_classification_reservation,
            source_term_work,
        )?;
        check_outer(
            ProbeLocalBudgetScope::Aggregate,
            AGGREGATE_PROSPECTIVE_CLASSIFICATION_WORK,
            classifications,
            limits.max_aggregate_prospective_classification_work,
        )?;
        let cache_rows = checked_budget_add(
            ProbeLocalBudgetScope::Aggregate,
            AGGREGATE_CACHE_LOGICAL_ROWS,
            self.row_cache_logical_rows,
            candidate_work,
        )?;
        check_outer(
            ProbeLocalBudgetScope::Aggregate,
            AGGREGATE_CACHE_LOGICAL_ROWS,
            cache_rows,
            limits.max_aggregate_row_cache_logical_rows,
        )?;
        let cache_values = checked_budget_add(
            ProbeLocalBudgetScope::Aggregate,
            AGGREGATE_CACHE_LOGICAL_VALUE_CELLS,
            self.row_cache_logical_value_cells,
            source_term_work,
        )?;
        check_outer(
            ProbeLocalBudgetScope::Aggregate,
            AGGREGATE_CACHE_LOGICAL_VALUE_CELLS,
            cache_values,
            limits.max_aggregate_row_cache_logical_value_cells,
        )?;
        self.residual_candidate_work = candidates;
        self.residual_source_term_work = source_terms;
        self.prospective_classification_reservation = classifications;
        self.row_cache_logical_rows = cache_rows;
        self.row_cache_logical_value_cells = cache_values;
        Ok(())
    }

    pub(super) fn try_admit_merge_work(
        &mut self,
        existing_requests: usize,
        candidate_requests: usize,
        limits: ProbeLocalSchedulerLimits,
    ) -> Result<(), ProbeLocalBudgetCause> {
        let local = checked_budget_add(
            ProbeLocalBudgetScope::Aggregate,
            AGGREGATE_MERGE_REQUEST_WORK,
            existing_requests,
            candidate_requests,
        )?;
        let work = checked_budget_add(
            ProbeLocalBudgetScope::Aggregate,
            AGGREGATE_MERGE_REQUEST_WORK,
            self.merge_request_work,
            local,
        )?;
        check_outer(
            ProbeLocalBudgetScope::Aggregate,
            AGGREGATE_MERGE_REQUEST_WORK,
            work,
            limits.max_aggregate_merge_request_work,
        )?;
        self.merge_request_work = work;
        Ok(())
    }

    /// Admit the complete conservative two-phase nomination envelope before
    /// union support/request allocation, enumeration, or sorting begins.
    pub(super) fn try_admit_obstruction_block_nomination(
        &mut self,
        upper: ObstructionBlockNominationUpperBound,
        limits: ProbeLocalSchedulerLimits,
    ) -> Result<(), ProbeLocalBudgetCause> {
        let resources = [
            (
                AGGREGATE_BLOCK_NOMINATION_RAW_ENTRIES,
                self.obstruction_block_nomination_raw_entry_reservation,
                upper.raw_block_entries(),
                limits.max_aggregate_obstruction_block_nomination_raw_entry_reservation,
            ),
            (
                AGGREGATE_BLOCK_NOMINATION_RAW_REQUESTS,
                self.obstruction_block_nomination_raw_request_reservation,
                upper.raw_request_visits(),
                limits.max_aggregate_obstruction_block_nomination_raw_request_reservation,
            ),
            (
                AGGREGATE_BLOCK_NOMINATION_COORDINATES,
                self.obstruction_block_nomination_coordinate_cell_reservation,
                upper.coordinate_cells(),
                limits.max_aggregate_obstruction_block_nomination_coordinate_cell_reservation,
            ),
            (
                AGGREGATE_BLOCK_NOMINATION_COEFFICIENTS,
                self.obstruction_block_nomination_coefficient_cell_reservation,
                upper.dense_coefficient_cells(),
                limits.max_aggregate_obstruction_block_nomination_coefficient_cell_reservation,
            ),
            (
                AGGREGATE_BLOCK_NOMINATION_CANONICALIZATION,
                self.obstruction_block_nomination_canonicalization_work_reservation,
                upper.canonicalization_logical_work_reservation(),
                limits.max_aggregate_obstruction_block_nomination_canonicalization_work_reservation,
            ),
            (
                AGGREGATE_BLOCK_NOMINATION_SUBSET,
                self.obstruction_block_nomination_subset_comparison_reservation,
                upper.subset_comparisons(),
                limits.max_aggregate_obstruction_block_nomination_subset_comparison_reservation,
            ),
        ];
        let mut admitted = [0usize; 6];
        for (ordinal, (resource, aggregate, addition, limit)) in
            resources.iter().copied().enumerate()
        {
            admitted[ordinal] = checked_budget_add(
                ProbeLocalBudgetScope::Aggregate,
                resource,
                aggregate,
                addition,
            )?;
            check_outer(
                ProbeLocalBudgetScope::Aggregate,
                resource,
                admitted[ordinal],
                limit,
            )?;
        }
        self.obstruction_block_nomination_raw_entry_reservation = admitted[0];
        self.obstruction_block_nomination_raw_request_reservation = admitted[1];
        self.obstruction_block_nomination_coordinate_cell_reservation = admitted[2];
        self.obstruction_block_nomination_coefficient_cell_reservation = admitted[3];
        self.obstruction_block_nomination_canonicalization_work_reservation = admitted[4];
        self.obstruction_block_nomination_subset_comparison_reservation = admitted[5];
        Ok(())
    }

    pub(super) fn try_admit_obstruction_block_work(
        &mut self,
        candidate_count: usize,
        source_term_work: usize,
        width: usize,
        selection_width: usize,
        limits: ProbeLocalSchedulerLimits,
    ) -> Result<(), ProbeLocalBudgetCause> {
        let candidates = checked_budget_add(
            ProbeLocalBudgetScope::Aggregate,
            AGGREGATE_BLOCK_CANDIDATE_WORK,
            self.obstruction_block_candidate_work,
            candidate_count,
        )?;
        check_outer(
            ProbeLocalBudgetScope::Aggregate,
            AGGREGATE_BLOCK_CANDIDATE_WORK,
            candidates,
            limits.max_aggregate_obstruction_block_candidate_work,
        )?;
        let source_terms = checked_budget_add(
            ProbeLocalBudgetScope::Aggregate,
            AGGREGATE_BLOCK_SOURCE_TERM_WORK,
            self.obstruction_block_source_term_work,
            source_term_work,
        )?;
        check_outer(
            ProbeLocalBudgetScope::Aggregate,
            AGGREGATE_BLOCK_SOURCE_TERM_WORK,
            source_terms,
            limits.max_aggregate_obstruction_block_source_term_work,
        )?;
        let signature_local =
            source_term_work
                .checked_mul(width)
                .ok_or(ProbeLocalBudgetCause::CountOverflow {
                    scope: ProbeLocalBudgetScope::Aggregate,
                    resource: AGGREGATE_BLOCK_SIGNATURE_WORK,
                })?;
        let signature = checked_budget_add(
            ProbeLocalBudgetScope::Aggregate,
            AGGREGATE_BLOCK_SIGNATURE_WORK,
            self.obstruction_block_signature_work,
            signature_local,
        )?;
        check_outer(
            ProbeLocalBudgetScope::Aggregate,
            AGGREGATE_BLOCK_SIGNATURE_WORK,
            signature,
            limits.max_aggregate_obstruction_block_signature_work,
        )?;
        let selection_local = candidate_count
            .checked_mul(selection_width.min(candidate_count))
            .ok_or(ProbeLocalBudgetCause::CountOverflow {
                scope: ProbeLocalBudgetScope::Aggregate,
                resource: AGGREGATE_BLOCK_SELECTION_WORK,
            })?;
        let selection = checked_budget_add(
            ProbeLocalBudgetScope::Aggregate,
            AGGREGATE_BLOCK_SELECTION_WORK,
            self.obstruction_block_selection_work,
            selection_local,
        )?;
        check_outer(
            ProbeLocalBudgetScope::Aggregate,
            AGGREGATE_BLOCK_SELECTION_WORK,
            selection,
            limits.max_aggregate_obstruction_block_selection_work,
        )?;
        let cache_rows = checked_budget_add(
            ProbeLocalBudgetScope::Aggregate,
            AGGREGATE_CACHE_LOGICAL_ROWS,
            self.row_cache_logical_rows,
            candidate_count,
        )?;
        check_outer(
            ProbeLocalBudgetScope::Aggregate,
            AGGREGATE_CACHE_LOGICAL_ROWS,
            cache_rows,
            limits.max_aggregate_row_cache_logical_rows,
        )?;
        let cache_values = checked_budget_add(
            ProbeLocalBudgetScope::Aggregate,
            AGGREGATE_CACHE_LOGICAL_VALUE_CELLS,
            self.row_cache_logical_value_cells,
            source_term_work,
        )?;
        check_outer(
            ProbeLocalBudgetScope::Aggregate,
            AGGREGATE_CACHE_LOGICAL_VALUE_CELLS,
            cache_values,
            limits.max_aggregate_row_cache_logical_value_cells,
        )?;
        self.obstruction_block_candidate_work = candidates;
        self.obstruction_block_source_term_work = source_terms;
        self.obstruction_block_signature_work = signature;
        self.obstruction_block_selection_work = selection;
        self.row_cache_logical_rows = cache_rows;
        self.row_cache_logical_value_cells = cache_values;
        Ok(())
    }

    /// Check a conservative upper envelope for one cache-evaluation batch
    /// before any lookup or finite-field evaluation mutates probe telemetry.
    ///
    /// Every candidate can miss (one retained row, all of its value cells,
    /// and one physical evaluation) or hit.  A binary search over at most
    /// `current rows + candidates` entries performs no more than
    /// `ceil(log2(entries + 1))` comparisons.  The hit and miss bounds are
    /// intentionally checked independently because their aggregate policies
    /// are independent.  This method is check-only; exact deltas are admitted
    /// after the batch, including on a fallible evaluation exit.
    pub(super) fn try_preflight_row_cache_batch(
        &self,
        current: ProbeRowEvaluationCacheTelemetry,
        candidate_count: usize,
        source_term_work: usize,
        limits: ProbeLocalSchedulerLimits,
    ) -> Result<(), ProbeLocalBudgetCause> {
        let maximum_rows = checked_budget_add(
            ProbeLocalBudgetScope::Aggregate,
            AGGREGATE_CACHE_ROWS,
            current.rows(),
            candidate_count,
        )?;
        let comparisons_per_lookup = if maximum_rows == 0 {
            0
        } else {
            usize::BITS as usize - maximum_rows.leading_zeros() as usize
        };
        let lookup_comparisons = candidate_count.checked_mul(comparisons_per_lookup).ok_or(
            ProbeLocalBudgetCause::CountOverflow {
                scope: ProbeLocalBudgetScope::Aggregate,
                resource: AGGREGATE_CACHE_LOOKUPS,
            },
        )?;
        let existing_moves = candidate_count.checked_mul(current.rows()).ok_or(
            ProbeLocalBudgetCause::CountOverflow {
                scope: ProbeLocalBudgetScope::Aggregate,
                resource: AGGREGATE_CACHE_INSERTION_MOVES,
            },
        )?;
        let (triangular_left, triangular_right) = if candidate_count % 2 == 0 {
            (candidate_count / 2, candidate_count.saturating_sub(1))
        } else {
            (candidate_count, candidate_count.saturating_sub(1) / 2)
        };
        let within_batch_moves = triangular_left.checked_mul(triangular_right).ok_or(
            ProbeLocalBudgetCause::CountOverflow {
                scope: ProbeLocalBudgetScope::Aggregate,
                resource: AGGREGATE_CACHE_INSERTION_MOVES,
            },
        )?;
        let insertion_moves = checked_budget_add(
            ProbeLocalBudgetScope::Aggregate,
            AGGREGATE_CACHE_INSERTION_MOVES,
            existing_moves,
            within_batch_moves,
        )?;
        for (resource, aggregate, upper_delta, limit) in [
            (
                AGGREGATE_CACHE_ROWS,
                self.row_cache_rows,
                candidate_count,
                limits.max_aggregate_row_cache_rows,
            ),
            (
                AGGREGATE_CACHE_VALUE_CELLS,
                self.row_cache_value_cells,
                source_term_work,
                limits.max_aggregate_row_cache_value_cells,
            ),
            (
                AGGREGATE_CACHE_LOOKUPS,
                self.row_cache_lookup_comparisons,
                lookup_comparisons,
                limits.max_aggregate_row_cache_lookup_comparisons,
            ),
            (
                AGGREGATE_CACHE_PHYSICAL,
                self.row_cache_physical_evaluations,
                candidate_count,
                limits.max_aggregate_row_cache_physical_evaluations,
            ),
            (
                AGGREGATE_CACHE_HITS,
                self.row_cache_hits,
                candidate_count,
                limits.max_aggregate_row_cache_hits,
            ),
            (
                AGGREGATE_CACHE_INSERTION_MOVES,
                self.row_cache_insertion_moves,
                insertion_moves,
                limits.max_aggregate_row_cache_insertion_moves,
            ),
        ] {
            let requested = checked_budget_add(
                ProbeLocalBudgetScope::Aggregate,
                resource,
                aggregate,
                upper_delta,
            )?;
            check_outer(ProbeLocalBudgetScope::Aggregate, resource, requested, limit)?;
        }
        Ok(())
    }

    pub(super) fn try_admit_row_cache_delta(
        &mut self,
        previous: ProbeRowEvaluationCacheTelemetry,
        current: ProbeRowEvaluationCacheTelemetry,
        limits: ProbeLocalSchedulerLimits,
    ) -> Result<(), ProbeLocalBudgetCause> {
        let deltas = [
            (
                AGGREGATE_CACHE_ROWS,
                current.rows(),
                previous.rows(),
                self.row_cache_rows,
                limits.max_aggregate_row_cache_rows,
            ),
            (
                AGGREGATE_CACHE_VALUE_CELLS,
                current.value_cells(),
                previous.value_cells(),
                self.row_cache_value_cells,
                limits.max_aggregate_row_cache_value_cells,
            ),
            (
                AGGREGATE_CACHE_LOOKUPS,
                current.lookup_comparisons(),
                previous.lookup_comparisons(),
                self.row_cache_lookup_comparisons,
                limits.max_aggregate_row_cache_lookup_comparisons,
            ),
            (
                AGGREGATE_CACHE_PHYSICAL,
                current.physical_evaluations(),
                previous.physical_evaluations(),
                self.row_cache_physical_evaluations,
                limits.max_aggregate_row_cache_physical_evaluations,
            ),
            (
                AGGREGATE_CACHE_HITS,
                current.cache_hits(),
                previous.cache_hits(),
                self.row_cache_hits,
                limits.max_aggregate_row_cache_hits,
            ),
            (
                AGGREGATE_CACHE_INSERTION_MOVES,
                current.insertion_moves(),
                previous.insertion_moves(),
                self.row_cache_insertion_moves,
                limits.max_aggregate_row_cache_insertion_moves,
            ),
        ];
        let mut admitted = [0usize; 6];
        for (ordinal, (resource, current, previous, aggregate, _limit)) in
            deltas.iter().copied().enumerate()
        {
            let delta =
                current
                    .checked_sub(previous)
                    .ok_or(ProbeLocalBudgetCause::CountOverflow {
                        scope: ProbeLocalBudgetScope::Probe,
                        resource,
                    })?;
            admitted[ordinal] =
                checked_budget_add(ProbeLocalBudgetScope::Aggregate, resource, aggregate, delta)?;
        }
        // Commit exact performed work before enforcing the postcondition.  A
        // preflight above should make a limit failure unreachable, but if an
        // invariant changes, the terminal census must still not erase work
        // already performed by the cache.
        self.row_cache_rows = admitted[0];
        self.row_cache_value_cells = admitted[1];
        self.row_cache_lookup_comparisons = admitted[2];
        self.row_cache_physical_evaluations = admitted[3];
        self.row_cache_hits = admitted[4];
        self.row_cache_insertion_moves = admitted[5];
        for (ordinal, (resource, _, _, _, limit)) in deltas.iter().copied().enumerate() {
            check_outer(
                ProbeLocalBudgetScope::Aggregate,
                resource,
                admitted[ordinal],
                limit,
            )?;
        }
        Ok(())
    }

    pub(super) fn try_admit_iteration_record(
        &mut self,
        limits: ProbeLocalSchedulerLimits,
    ) -> Result<(), ProbeLocalBudgetCause> {
        let records = checked_budget_add(
            ProbeLocalBudgetScope::Aggregate,
            ITERATION_RECORDS,
            self.iteration_records,
            1,
        )?;
        check_outer(
            ProbeLocalBudgetScope::Aggregate,
            ITERATION_RECORDS,
            records,
            limits.max_retained_iteration_records,
        )?;
        self.iteration_records = records;
        Ok(())
    }

    pub(super) fn try_admit_exact(
        &mut self,
        limits: ProbeLocalSchedulerLimits,
    ) -> Result<(), ProbeLocalBudgetCause> {
        let attempts = checked_budget_add(
            ProbeLocalBudgetScope::Aggregate,
            EXACT_ATTEMPTS,
            self.exact_attempts,
            1,
        )?;
        check_outer(
            ProbeLocalBudgetScope::Aggregate,
            EXACT_ATTEMPTS,
            attempts,
            limits.max_exact_lift_attempts,
        )?;
        self.exact_attempts = attempts;
        Ok(())
    }

    pub(super) const fn census(&self) -> ProbeLocalRunCensus {
        ProbeLocalRunCensus::new(
            self.epochs,
            self.epoch_request_work,
            self.materialized_source_terms,
            self.modular_entry_work,
            self.residual_candidate_work,
            self.residual_source_term_work,
            self.prospective_classification_reservation,
            self.obstruction_block_nomination_raw_entry_reservation,
            self.obstruction_block_nomination_raw_request_reservation,
            self.obstruction_block_nomination_coordinate_cell_reservation,
            self.obstruction_block_nomination_coefficient_cell_reservation,
            self.obstruction_block_nomination_canonicalization_work_reservation,
            self.obstruction_block_nomination_subset_comparison_reservation,
            self.obstruction_block_candidate_work,
            self.obstruction_block_source_term_work,
            self.obstruction_block_signature_work,
            self.obstruction_block_selection_work,
            self.row_cache_rows,
            self.row_cache_value_cells,
            self.row_cache_lookup_comparisons,
            self.row_cache_physical_evaluations,
            self.row_cache_hits,
            self.row_cache_insertion_moves,
            self.merge_request_work,
            self.iteration_records,
            self.exact_attempts,
        )
    }
}

pub(super) fn checked_budget_add(
    scope: ProbeLocalBudgetScope,
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ProbeLocalBudgetCause> {
    left.checked_add(right)
        .ok_or(ProbeLocalBudgetCause::CountOverflow { scope, resource })
}

pub(super) fn check_outer(
    scope: ProbeLocalBudgetScope,
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ProbeLocalBudgetCause> {
    if requested > limit {
        Err(ProbeLocalBudgetCause::Outer {
            scope,
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

pub(super) fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ProbeLocalSchedulerError> {
    left.checked_add(right)
        .ok_or(ProbeLocalSchedulerError::ResourceCountOverflow { resource })
}

pub(super) fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ProbeLocalSchedulerError> {
    if requested > limit {
        Err(ProbeLocalSchedulerError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

pub(super) fn try_vec<T>(
    resource: &'static str,
    capacity: usize,
) -> Result<Vec<T>, ProbeLocalSchedulerError> {
    let mut values = Vec::new();
    values.try_reserve_exact(capacity).map_err(|_| {
        ProbeLocalSchedulerError::AllocationFailure {
            resource,
            requested: capacity,
        }
    })?;
    Ok(values)
}
