use crate::foundry::completion::frame::exact::ExactCircuitLimits;

use super::super::{CampaignLimits, SourceDiscoveryLimits};

/// Aggregate and per-probe policy for one ordered outer obstruction run.
///
/// Nested campaign, source-discovery, and exact-circuit policies remain in
/// force. These additional limits bound work retained or accumulated across
/// otherwise independent probe-local campaigns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ProbeLocalSchedulerLimits {
    pub(crate) campaign: CampaignLimits,
    pub(crate) source_discovery: SourceDiscoveryLimits,
    pub(crate) exact_circuit: ExactCircuitLimits,
    pub(crate) max_probes: usize,
    pub(crate) max_retained_probe_coordinate_cells: usize,
    pub(crate) max_retained_outcomes: usize,
    pub(crate) max_iterations_per_probe: usize,
    pub(crate) max_requests_per_probe: usize,
    pub(crate) max_request_coordinate_cells_per_probe: usize,
    /// Maximum deterministic frontier-ranked batch from an exhaustive
    /// nonzero-residual census promoted into the next fresh epoch.
    ///
    /// This bounds frame growth only.  Residual classification remains
    /// exhaustive, an empty census remains the sole sampled-dual authority,
    /// and unselected requests may be nominated again by the next checked
    /// obstruction.  Consequently this policy can delay discovery but cannot
    /// manufacture a hit or a no-relation certificate.
    pub(crate) max_residual_proposals_per_iteration: usize,
    pub(crate) max_aggregate_epochs: usize,
    /// Sum of request counts materialized by all fresh epoch attempts.
    pub(crate) max_aggregate_epoch_request_work: usize,
    /// Exact sum of translated source-term occurrences requested by epochs.
    pub(crate) max_aggregate_materialized_source_terms: usize,
    /// Exact physical-entry census charged before each modular query.
    pub(crate) max_aggregate_modular_entry_work: usize,
    /// Sum of structurally nominated residual candidates evaluated across all
    /// probes and epochs in one scheduler run.
    pub(crate) max_aggregate_residual_candidate_work: usize,
    /// Sum of exact translated-source terms evaluated by residual censuses.
    pub(crate) max_aggregate_residual_source_term_work: usize,
    /// Conservative translated-term reservation for prospective semantic
    /// classification. Actual scoring only visits nonzero-residual rows.
    pub(crate) max_aggregate_prospective_classification_work: usize,
    /// Conservative aggregate envelopes admitted before union nomination
    /// materializes raw supports, requests, coordinates, or sorting work.
    pub(crate) max_aggregate_obstruction_block_nomination_raw_entry_reservation: usize,
    pub(crate) max_aggregate_obstruction_block_nomination_raw_request_reservation: usize,
    pub(crate) max_aggregate_obstruction_block_nomination_coordinate_cell_reservation: usize,
    pub(crate) max_aggregate_obstruction_block_nomination_coefficient_cell_reservation: usize,
    pub(crate) max_aggregate_obstruction_block_nomination_canonicalization_work_reservation: usize,
    pub(crate) max_aggregate_obstruction_block_nomination_subset_comparison_reservation: usize,
    pub(crate) max_aggregate_obstruction_block_candidate_work: usize,
    pub(crate) max_aggregate_obstruction_block_source_term_work: usize,
    pub(crate) max_aggregate_obstruction_block_signature_work: usize,
    pub(crate) max_aggregate_obstruction_block_selection_work: usize,
    pub(crate) max_aggregate_row_cache_rows: usize,
    pub(crate) max_aggregate_row_cache_value_cells: usize,
    /// Conservative combined primary+block logical cache reservation, charged
    /// before evaluation even when a row is expected to hit.
    pub(crate) max_aggregate_row_cache_logical_rows: usize,
    pub(crate) max_aggregate_row_cache_logical_value_cells: usize,
    pub(crate) max_aggregate_row_cache_lookup_comparisons: usize,
    pub(crate) max_aggregate_row_cache_physical_evaluations: usize,
    pub(crate) max_aggregate_row_cache_hits: usize,
    pub(crate) max_aggregate_row_cache_insertion_moves: usize,
    /// Conservative `existing requests + residual candidates` merge work.
    pub(crate) max_aggregate_merge_request_work: usize,
    pub(crate) max_retained_iteration_records: usize,
    pub(crate) max_exact_lift_attempts: usize,
    /// Opt-in ceiling for a second exact Symbolica reduction over every row
    /// of an already materialized frame when the modularly selected minor is
    /// specialization-only. Zero disables the fallback. This never expands
    /// the request universe and remains independently bounded by
    /// [`ExactCircuitLimits`].
    pub(crate) max_complete_frame_exact_fallback_rows: usize,
}

impl Default for ProbeLocalSchedulerLimits {
    fn default() -> Self {
        Self {
            campaign: CampaignLimits::default(),
            source_discovery: SourceDiscoveryLimits::default(),
            exact_circuit: ExactCircuitLimits::default(),
            max_probes: 4_096,
            max_retained_probe_coordinate_cells: 4_194_304,
            max_retained_outcomes: 4_096,
            max_iterations_per_probe: 4_096,
            max_requests_per_probe: 1_000_000,
            max_request_coordinate_cells_per_probe: 64_000_000,
            max_residual_proposals_per_iteration: 32,
            max_aggregate_epochs: 16_384,
            max_aggregate_epoch_request_work: 64_000_000,
            max_aggregate_materialized_source_terms: 1_000_000_000,
            max_aggregate_modular_entry_work: 1_000_000_000,
            max_aggregate_residual_candidate_work: 100_000,
            max_aggregate_residual_source_term_work: 1_000_000,
            max_aggregate_prospective_classification_work: 1_000_000,
            max_aggregate_obstruction_block_nomination_raw_entry_reservation: 100_000_000,
            max_aggregate_obstruction_block_nomination_raw_request_reservation: 1_000_000_000,
            max_aggregate_obstruction_block_nomination_coordinate_cell_reservation: 2_000_000_000,
            max_aggregate_obstruction_block_nomination_coefficient_cell_reservation: 1_000_000_000,
            max_aggregate_obstruction_block_nomination_canonicalization_work_reservation:
                2_000_000_000,
            max_aggregate_obstruction_block_nomination_subset_comparison_reservation: 1_000_000_000,
            max_aggregate_obstruction_block_candidate_work: 100_000,
            max_aggregate_obstruction_block_source_term_work: 1_000_000,
            max_aggregate_obstruction_block_signature_work: 4_000_000,
            max_aggregate_obstruction_block_selection_work: 3_200_000,
            max_aggregate_row_cache_rows: 16_000_000,
            max_aggregate_row_cache_value_cells: 256_000_000,
            max_aggregate_row_cache_logical_rows: 16_000_000,
            max_aggregate_row_cache_logical_value_cells: 256_000_000,
            max_aggregate_row_cache_lookup_comparisons: 256_000_000,
            max_aggregate_row_cache_physical_evaluations: 16_000_000,
            max_aggregate_row_cache_hits: 16_000_000,
            max_aggregate_row_cache_insertion_moves: 512_000_000,
            max_aggregate_merge_request_work: 128_000_000,
            max_retained_iteration_records: 16_384,
            max_exact_lift_attempts: 4_096,
            max_complete_frame_exact_fallback_rows: 0,
        }
    }
}
