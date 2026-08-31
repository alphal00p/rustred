use crate::foundry::completion::frame::exact::ExactCircuitLimits;

use super::super::{CampaignLimits, SourceDiscoveryLimits};

/// Aggregate cold-path policy for one multi-probe common-epoch rebase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CanonicalReplayLimits {
    pub(crate) campaign: CampaignLimits,
    pub(crate) source_discovery: SourceDiscoveryLimits,
    pub(crate) exact_circuit: ExactCircuitLimits,
    pub(crate) max_replayed_nominations: usize,
    pub(crate) max_nomination_probe_coordinate_cells: usize,
    pub(crate) max_union_request_occurrences: usize,
    pub(crate) max_union_request_coordinate_cells: usize,
    pub(crate) max_rebase_attempts: usize,
    pub(crate) max_aggregate_modular_entry_work: usize,
    pub(crate) max_aggregate_partition_column_work: usize,
    pub(crate) max_retained_diagnostic_entries: usize,
    pub(crate) max_retained_exact_payload_cells: usize,
    pub(crate) max_retained_integer_coefficient_bits: usize,
    pub(crate) max_successful_exact_lifts: usize,
    pub(crate) max_unique_candidates: usize,
    pub(crate) max_supporting_probe_references: usize,
    pub(crate) max_anchor_coordinate_cells: usize,
    pub(crate) max_content_sort_comparisons: usize,
}

impl Default for CanonicalReplayLimits {
    fn default() -> Self {
        Self {
            campaign: CampaignLimits::default(),
            source_discovery: SourceDiscoveryLimits::default(),
            exact_circuit: ExactCircuitLimits::default(),
            max_replayed_nominations: 4_096,
            max_nomination_probe_coordinate_cells: 4_194_304,
            max_union_request_occurrences: 64_000_000,
            max_union_request_coordinate_cells: 1_000_000_000,
            max_rebase_attempts: 4_096,
            max_aggregate_modular_entry_work: 1_000_000_000,
            max_aggregate_partition_column_work: 64_000_000,
            max_retained_diagnostic_entries: 64_000_000,
            max_retained_exact_payload_cells: 256_000_000,
            max_retained_integer_coefficient_bits: 2_147_483_648,
            max_successful_exact_lifts: 4_096,
            max_unique_candidates: 4_096,
            max_supporting_probe_references: 4_096,
            max_anchor_coordinate_cells: 4_194_304,
            max_content_sort_comparisons: 16_777_216,
        }
    }
}
