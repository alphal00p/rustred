//! Explicit resource envelopes for the bounded K6 sweep.

use crate::foundry::completion::CompletionGeometryLimits;
use crate::foundry::completion::frame::PhysicalFrameLimits;
use crate::foundry::completion::frame::admission::{
    ExactCircuitOwnerCoverLimits, ExactCircuitSemanticLimits,
};
use crate::foundry::completion::frame::exact::ExactCircuitLimits;
use crate::foundry::completion::frame::modular::ModularKernelLimits;
use crate::foundry::completion::guard::decision::GuardDecisionDagLimits;
use crate::foundry::completion::stratum::StratumRegistryLimits;
use crate::identity::TranslatedSourceLimits;

pub(super) const MAX_DEGREE: usize = 2;
pub(super) const MAX_FRAME_COLUMNS: usize = 640;
const MAX_OFFSETS: usize = 28;
const MAX_FRAME_ROWS: usize = 252;
const MAX_FRAME_ENTRIES: usize = 4_096;
const MAX_OWNER_INPUTS: usize = 1_024;
const MAX_UNCOVERED_BOXES: usize = 262_144;
const K6_ROOT_OWNER_REGIONS: usize = 32;
const K6_GROUP_ORDER: usize = 24;
const K6_ROOT_ROUTE_UPPER: usize = K6_ROOT_OWNER_REGIONS * (K6_GROUP_ORDER + 1);

pub(super) fn frame_limits() -> PhysicalFrameLimits {
    PhysicalFrameLimits {
        translated_sources: TranslatedSourceLimits {
            max_requested_offsets: MAX_OFFSETS,
            max_translated_sources: MAX_FRAME_ROWS,
            max_translated_term_entries: MAX_FRAME_ENTRIES,
            max_translated_condition_entries: MAX_FRAME_ENTRIES,
            max_retained_condition_source_entries: 16_384,
            max_retained_index_coordinate_cells: 32_768,
            ..TranslatedSourceLimits::default()
        },
        max_arity: 6,
        max_degree: MAX_DEGREE,
        max_offsets: MAX_OFFSETS,
        max_offset_coordinate_cells: MAX_OFFSETS * 6,
        max_source_instances: MAX_FRAME_ROWS,
        max_physical_columns: MAX_FRAME_COLUMNS,
        max_physical_column_coordinate_cells: MAX_FRAME_COLUMNS * 6,
        max_physical_entries: MAX_FRAME_ENTRIES,
        max_csr_row_offsets: MAX_FRAME_ROWS + 1,
    }
}

pub(super) fn registry_limits() -> StratumRegistryLimits {
    StratumRegistryLimits {
        max_guard_branches: 0,
        max_guard_identity_bytes: 0,
        max_stratum_identity_bytes: 65_536,
        max_owner_regions: K6_ROOT_OWNER_REGIONS,
        max_owner_coordinate_cells: K6_ROOT_OWNER_REGIONS * 6,
        max_owner_routes: K6_ROOT_ROUTE_UPPER,
        max_owner_route_coordinate_cells: K6_ROOT_ROUTE_UPPER * 6 * 3,
        max_owner_identity_bytes: 1_048_576,
        max_physical_columns: MAX_FRAME_COLUMNS,
        max_column_coordinate_cells: MAX_FRAME_COLUMNS * 6,
        max_target_sector_cells: 262_144,
        max_owner_probes: 4_194_304,
        max_retained_owner_witnesses: MAX_FRAME_COLUMNS,
    }
}

pub(super) fn modular_limits() -> ModularKernelLimits {
    ModularKernelLimits {
        max_point_coordinates: 7,
        max_matrix_rows: MAX_FRAME_ROWS,
        max_matrix_columns: MAX_FRAME_COLUMNS,
        max_source_conditions: MAX_FRAME_ENTRIES,
        max_structural_entries: MAX_FRAME_ENTRIES,
        max_retained_entries: MAX_FRAME_ENTRIES,
        max_csr_row_offsets: MAX_FRAME_ROWS + 1,
        max_projected_columns: MAX_FRAME_COLUMNS,
        max_projected_entries: MAX_FRAME_ENTRIES,
        max_reducer_dense_cells: MAX_FRAME_ROWS * MAX_FRAME_COLUMNS,
        max_reducer_total_fill_entries: 1_000_000,
        max_reducer_fill_multiple: 20,
    }
}

pub(super) fn exact_limits() -> ExactCircuitLimits {
    ExactCircuitLimits {
        max_physical_columns: MAX_FRAME_COLUMNS,
        max_selected_rows: MAX_FRAME_ROWS,
        max_projected_physical_columns: MAX_FRAME_COLUMNS,
        max_augmented_columns: MAX_FRAME_COLUMNS + MAX_FRAME_ROWS + 1,
        max_projected_input_nonzero_entries: MAX_FRAME_ENTRIES,
        max_native_decomposition_nonzero_entries: 4_000_000,
        max_pivot_dependency_entries: 4_000_000,
        max_source_combination_terms: MAX_FRAME_ROWS,
        max_replay_source_terms: MAX_FRAME_ENTRIES,
        max_replay_exact_operations: 10_000_000,
        max_circuit_terms: MAX_FRAME_COLUMNS,
        max_dependency_owner_witnesses: MAX_FRAME_COLUMNS,
        max_guards: 4_096,
        max_guard_origins: 32_768,
        max_condition_source_entries: 32_768,
        ..ExactCircuitLimits::default()
    }
}

pub(super) fn semantic_limits(max_candidates: usize) -> ExactCircuitSemanticLimits {
    ExactCircuitSemanticLimits {
        max_candidates,
        max_residual_terms: MAX_FRAME_COLUMNS * max_candidates,
        max_source_contributions: MAX_FRAME_ROWS * max_candidates,
        max_pivot_guards: MAX_FRAME_ROWS * max_candidates,
        max_nonzero_guards: 4_096 * max_candidates,
        max_guard_origins: 32_768 * max_candidates,
        max_condition_sources: 32_768 * max_candidates,
        max_condition_source_coordinate_cells: 196_608 * max_candidates,
        max_dependency_owners: MAX_FRAME_COLUMNS * max_candidates,
        max_guard_coefficient_equations: 32_768 * max_candidates,
        max_guard_base_monomial_exponents: 196_608 * max_candidates,
        max_guard_generators: 32_768 * max_candidates,
        max_guard_identity_bytes: 4_194_304 * max_candidates,
        max_modular_sample_point_entries: 7 * max_candidates,
        max_modular_diagnostic_entries: 4_096 * max_candidates,
        max_exact_polynomials: 65_536 * max_candidates,
        max_polynomial_terms: 524_288 * max_candidates,
        max_exponent_entries: 4_194_304 * max_candidates,
        max_integer_coefficient_bits: 4_194_304 * max_candidates,
        guard_dag: GuardDecisionDagLimits {
            max_context_identity_bytes: 1_048_576,
            max_candidates,
            max_unique_atoms: 4_096 * max_candidates,
            max_candidate_atom_references: 4_096 * max_candidates,
            max_atom_identity_bytes: 4_194_304 * max_candidates,
            max_states: 8_192 * max_candidates,
            max_state_words: 8_192 * max_candidates,
            max_candidate_scans: 8_192 * max_candidates,
            max_nodes: 8_192 * max_candidates,
            max_edges: 16_384 * max_candidates,
            max_pending_work_items: 8_192 * max_candidates,
        },
        ..ExactCircuitSemanticLimits::default()
    }
}

pub(super) fn owner_cover_limits() -> ExactCircuitOwnerCoverLimits {
    ExactCircuitOwnerCoverLimits {
        max_owner_inputs: MAX_OWNER_INPUTS,
        max_owner_coordinate_cells: MAX_OWNER_INPUTS * 6,
        max_explicit_terminals: 1,
        max_terminal_coordinate_cells: 6,
        max_finite_complement_points: 65_536,
        max_finite_complement_coordinate_cells: 65_536 * 6,
        max_point_owner_probes: 4_194_304,
        geometry: CompletionGeometryLimits {
            max_arity: 6,
            max_requested_generators: MAX_OWNER_INPUTS,
            max_requested_generator_coordinate_cells: MAX_OWNER_INPUTS * 6,
            max_minimal_generators: MAX_OWNER_INPUTS,
            max_requested_boxes: 0,
            max_requested_box_coordinate_cells: 0,
            max_uncovered_boxes: MAX_UNCOVERED_BOXES,
            max_uncovered_box_coordinate_cells: MAX_UNCOVERED_BOXES * 6,
            max_split_operations: 4_194_304,
        },
        ..ExactCircuitOwnerCoverLimits::default()
    }
}
