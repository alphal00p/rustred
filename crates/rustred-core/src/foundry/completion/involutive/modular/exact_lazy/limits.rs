use super::super::super::InvolutiveLimits;
use super::super::limits::{ExactMaterializerLimits, ModularGuideLimits};

/// Immutable support-classification and cumulative-work envelope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ExactLazySupportLimits {
    pub(super) exact_fallback: ExactMaterializerLimits,
    pub(super) max_probes_per_schedule: usize,
    pub(super) max_classification_attempts: usize,
    pub(super) max_roots_per_classification: usize,
    pub(super) max_total_classification_roots: usize,
    pub(super) max_total_scheduled_probes: usize,
    pub(super) max_total_successful_probes: usize,
    pub(super) max_total_rejected_probes: usize,
    pub(super) max_total_probe_queries: usize,
    pub(super) max_total_probe_delta_compositions: usize,
    pub(super) max_total_probe_delta_coordinate_operations: usize,
    pub(super) max_total_probe_evaluation_steps: usize,
    pub(super) max_total_probe_evaluation_frame_pushes: usize,
    pub(super) max_peak_probe_live_evaluation_frames: usize,
    pub(super) max_peak_probe_live_evaluation_values: usize,
    pub(super) max_total_probe_cache_hits: usize,
    pub(super) max_total_probe_exact_leaf_evaluations: usize,
    pub(super) max_total_probe_exact_leaf_terms_evaluated: usize,
    pub(super) max_total_probe_exact_leaf_exponent_cells_evaluated: usize,
    pub(super) max_exact_fallback_batches: usize,
    pub(super) max_exact_fallback_roots_per_batch: usize,
    pub(super) max_total_exact_fallback_roots: usize,
}

impl Default for ExactLazySupportLimits {
    fn default() -> Self {
        Self {
            exact_fallback: ExactMaterializerLimits::default(),
            max_probes_per_schedule: 64,
            max_classification_attempts: 16_000_000,
            max_roots_per_classification: 16_000_000,
            max_total_classification_roots: 1_000_000_000,
            max_total_scheduled_probes: 1_000_000_000,
            max_total_successful_probes: 1_000_000_000,
            max_total_rejected_probes: 1_000_000_000,
            max_total_probe_queries: 8_000_000_000,
            max_total_probe_delta_compositions: 8_000_000_000,
            max_total_probe_delta_coordinate_operations: 64_000_000_000,
            max_total_probe_evaluation_steps: 64_000_000_000,
            max_total_probe_evaluation_frame_pushes: 128_000_000_000,
            max_peak_probe_live_evaluation_frames: 16_000_000,
            max_peak_probe_live_evaluation_values: 16_000_000,
            max_total_probe_cache_hits: 64_000_000_000,
            max_total_probe_exact_leaf_evaluations: 8_000_000_000,
            max_total_probe_exact_leaf_terms_evaluated: 64_000_000_000,
            max_total_probe_exact_leaf_exponent_cells_evaluated: 128_000_000_000,
            max_exact_fallback_batches: 16_000_000,
            max_exact_fallback_roots_per_batch: 16_000_000,
            max_total_exact_fallback_roots: 1_000_000_000,
        }
    }
}

/// Immutable retained-shape and ingress-work contract of one ELC1 session.
///
/// The nested exact and coefficient-circuit limits are copied into the owner;
/// callers cannot silently widen either contract after proofs exist.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ExactLazyLimits {
    pub(super) exact: InvolutiveLimits,
    pub(super) coefficient: ModularGuideLimits,
    pub(super) support: ExactLazySupportLimits,
    pub(super) max_transaction_attempts: usize,
    pub(super) max_committed_transactions: usize,
    pub(super) max_imported_physical_terms: usize,
    pub(super) max_imported_provenance_terms: usize,
    pub(super) max_imported_guard_descriptors: usize,
    pub(super) max_total_imported_physical_terms: usize,
    pub(super) max_total_imported_provenance_terms: usize,
    pub(super) max_total_imported_guard_descriptors: usize,
    pub(super) max_derivation_nodes: usize,
    pub(super) max_total_derivation_nodes_created: usize,
    pub(super) max_derivation_shift_coordinate_cells: usize,
    pub(super) max_total_derivation_shift_coordinate_cells_created: usize,
    pub(super) max_guard_lineage_nodes: usize,
    pub(super) max_total_guard_lineage_nodes_created: usize,
    pub(super) max_guard_descriptor_payloads: usize,
    pub(super) max_total_guard_descriptor_payloads_created: usize,
    pub(super) max_guard_shift_coordinate_cells: usize,
    pub(super) max_total_guard_shift_coordinate_cells_created: usize,
    pub(super) max_guard_collection_node_visits: usize,
    pub(super) max_guard_collection_requirements: usize,
    pub(super) max_frozen_epoch_divisors: usize,
}

impl Default for ExactLazyLimits {
    fn default() -> Self {
        Self {
            exact: InvolutiveLimits::default(),
            coefficient: ModularGuideLimits::default(),
            support: ExactLazySupportLimits::default(),
            max_transaction_attempts: 16_000_000,
            max_committed_transactions: 16_000_000,
            max_imported_physical_terms: 16_000_000,
            max_imported_provenance_terms: 16_000_000,
            max_imported_guard_descriptors: 16_000_000,
            max_total_imported_physical_terms: 1_000_000_000,
            max_total_imported_provenance_terms: 1_000_000_000,
            max_total_imported_guard_descriptors: 1_000_000_000,
            max_derivation_nodes: 16_000_000,
            max_total_derivation_nodes_created: 1_000_000_000,
            max_derivation_shift_coordinate_cells: 256_000_000,
            max_total_derivation_shift_coordinate_cells_created: 1_000_000_000,
            max_guard_lineage_nodes: 16_000_000,
            max_total_guard_lineage_nodes_created: 1_000_000_000,
            max_guard_descriptor_payloads: 16_000_000,
            max_total_guard_descriptor_payloads_created: 1_000_000_000,
            max_guard_shift_coordinate_cells: 256_000_000,
            max_total_guard_shift_coordinate_cells_created: 1_000_000_000,
            max_guard_collection_node_visits: 64_000_000,
            max_guard_collection_requirements: 16_000_000,
            max_frozen_epoch_divisors: 16_000_000,
        }
    }
}

/// Monotone session accounting. Failed transactions keep attempted ingress
/// and coefficient-DAG churn charged; only live DAG storage rolls back.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct ExactLazyCensus {
    pub(super) transaction_attempts: usize,
    pub(super) committed_transactions: usize,
    pub(super) imported_physical_terms: usize,
    pub(super) imported_provenance_terms: usize,
    pub(super) imported_guard_descriptors: usize,
}

impl ExactLazyCensus {
    pub(super) const fn transaction_attempts(self) -> usize {
        self.transaction_attempts
    }

    pub(super) const fn committed_transactions(self) -> usize {
        self.committed_transactions
    }

    pub(super) const fn imported_physical_terms(self) -> usize {
        self.imported_physical_terms
    }

    pub(super) const fn imported_provenance_terms(self) -> usize {
        self.imported_provenance_terms
    }

    pub(super) const fn imported_guard_descriptors(self) -> usize {
        self.imported_guard_descriptors
    }
}
