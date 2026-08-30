use crate::algebra::IndexedAlgebraLimits;

/// Resource policy for compiling one semantic guard decision DAG.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GuardDecisionDagLimits {
    pub(crate) max_context_identity_bytes: usize,
    pub(crate) max_candidates: usize,
    pub(crate) max_unique_atoms: usize,
    /// Raw candidate-to-atom references before duplicate and unit removal.
    pub(crate) max_candidate_atom_references: usize,
    /// Cumulative generator and exact representative identity bytes over
    /// unique retained atoms.
    pub(crate) max_atom_identity_bytes: usize,
    pub(crate) max_states: usize,
    /// Cumulative active-candidate bitset words retained by memo states.
    pub(crate) max_state_words: usize,
    /// Candidate inspections while finding and applying branch atoms.
    pub(crate) max_candidate_scans: usize,
    pub(crate) max_nodes: usize,
    pub(crate) max_edges: usize,
    pub(crate) max_pending_work_items: usize,
}

/// Selected aggregate counters and the per-predicate algebra policy for one
/// exact point traversal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GuardDecisionEvaluationLimits {
    pub(crate) max_predicate_evaluations: usize,
    pub(crate) max_input_terms: usize,
    pub(crate) max_specialization_power_operations: usize,
    pub(crate) indexed_algebra: IndexedAlgebraLimits,
}

impl Default for GuardDecisionEvaluationLimits {
    fn default() -> Self {
        Self {
            max_predicate_evaluations: 16_384,
            max_input_terms: 4_194_304,
            max_specialization_power_operations: 67_108_864,
            indexed_algebra: IndexedAlgebraLimits::default(),
        }
    }
}

impl Default for GuardDecisionDagLimits {
    fn default() -> Self {
        Self {
            max_context_identity_bytes: 1_048_576,
            max_candidates: 4_096,
            max_unique_atoms: 16_384,
            max_candidate_atom_references: 262_144,
            max_atom_identity_bytes: 67_108_864,
            max_states: 1_048_576,
            max_state_words: 67_108_864,
            max_candidate_scans: 1_000_000_000,
            max_nodes: 1_048_576,
            max_edges: 2_097_152,
            max_pending_work_items: 65_536,
        }
    }
}
