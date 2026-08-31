use crate::foundry::completion::frame::admission::ExactCircuitOwner;
use crate::foundry::completion::guard::decision::GuardDecisionDagStats;

/// Compact scalar census of the canonical semantic decision DAG retained by
/// one exact proof owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ExactProofOwnerDagCensus {
    candidates: usize,
    atoms: usize,
    candidate_atom_references: usize,
    memo_states: usize,
    nodes: usize,
    edges: usize,
    has_reachable_incomplete: bool,
}

impl ExactProofOwnerDagCensus {
    pub(crate) const fn candidates(self) -> usize {
        self.candidates
    }

    pub(crate) const fn atoms(self) -> usize {
        self.atoms
    }

    pub(crate) const fn candidate_atom_references(self) -> usize {
        self.candidate_atom_references
    }

    pub(crate) const fn memo_states(self) -> usize {
        self.memo_states
    }

    pub(crate) const fn nodes(self) -> usize {
        self.nodes
    }

    pub(crate) const fn edges(self) -> usize {
        self.edges
    }

    pub(crate) const fn has_reachable_incomplete(self) -> bool {
        self.has_reachable_incomplete
    }

    fn from_stats(stats: GuardDecisionDagStats) -> Self {
        Self {
            candidates: stats.candidates,
            atoms: stats.atoms,
            candidate_atom_references: stats.candidate_atom_references,
            memo_states: stats.memo_states,
            nodes: stats.nodes,
            edges: stats.edges,
            has_reachable_incomplete: stats.has_reachable_incomplete,
        }
    }
}

/// Borrowed, allocation-free audit view of one canonically compiled proof
/// owner. It exposes neither the semantic DAG nor executable rule payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ExactProofOwnerSummary<'a> {
    leading_lattice_point: &'a [u64],
    compiled_guard_total: bool,
    semantic_dag: ExactProofOwnerDagCensus,
}

impl<'a> ExactProofOwnerSummary<'a> {
    pub(crate) const fn leading_lattice_point(self) -> &'a [u64] {
        self.leading_lattice_point
    }

    pub(crate) const fn compiled_guard_total(self) -> bool {
        self.compiled_guard_total
    }

    pub(crate) const fn semantic_dag_census(self) -> ExactProofOwnerDagCensus {
        self.semantic_dag
    }

    pub(super) fn from_owner(owner: &'a ExactCircuitOwner) -> Self {
        Self {
            leading_lattice_point: owner.leading().coordinates(),
            compiled_guard_total: owner.is_guard_total(),
            semantic_dag: ExactProofOwnerDagCensus::from_stats(
                owner.semantic().guard_dag().stats(),
            ),
        }
    }
}
