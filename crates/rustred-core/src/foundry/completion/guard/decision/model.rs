use std::sync::Arc;

use crate::algebra::{IndexedAlgebraError, IndexedCoefficientContext};
use crate::foundry::completion::stratum::GuardBranch;

use super::super::{CoefficientIdealGuardAtom, model::CoefficientIdealGuardPredicate};
use super::{GuardDecisionDagError, GuardDecisionDagLimits, GuardDecisionEvaluationLimits};

const PREDICATE_EVALUATIONS: &str = "semantic guard predicate evaluations";
const EVALUATION_INPUT_TERMS: &str = "semantic guard evaluation input terms";
const EVALUATION_POWER_OPERATIONS: &str = "semantic guard specialization power operations";

/// Stable discovery-candidate priority and label.
///
/// Compilation requires strictly increasing identities, so parallel discovery
/// must assign them only after a deterministic canonical sort. An identity
/// does not denote an admitted owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct GuardDecisionCandidateId(pub(crate) usize);

/// Borrowed input for one candidate conjunction. Candidate slices must be in
/// strictly increasing identity (and therefore priority) order.
#[derive(Clone, Copy, Debug)]
pub(crate) struct GuardDecisionCandidate<'a> {
    id: GuardDecisionCandidateId,
    required_nonzero: &'a [CoefficientIdealGuardAtom],
}

impl<'a> GuardDecisionCandidate<'a> {
    pub(crate) const fn new(
        id: GuardDecisionCandidateId,
        required_nonzero: &'a [CoefficientIdealGuardAtom],
    ) -> Self {
        Self {
            id,
            required_nonzero,
        }
    }

    pub(super) const fn id(self) -> GuardDecisionCandidateId {
        self.id
    }

    pub(super) const fn required_nonzero(self) -> &'a [CoefficientIdealGuardAtom] {
        self.required_nonzero
    }
}

/// Result of routing one complete assignment of semantic atom branches.
/// `Candidate` is not a proof of rule admission; `Incomplete` must restart or
/// extend discovery.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum GuardDecisionOutcome {
    Candidate(GuardDecisionCandidateId),
    Incomplete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum GuardDecisionRef {
    Leaf(GuardDecisionOutcome),
    Node(usize),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct GuardDecisionNode {
    pub(super) atom: usize,
    pub(super) zero: GuardDecisionRef,
    pub(super) nonzero: GuardDecisionRef,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CanonicalGuardCandidate {
    pub(super) id: GuardDecisionCandidateId,
    pub(super) required_atoms: Box<[usize]>,
}

/// Deterministic compilation telemetry. Counts describe retained canonical
/// structure, not the raw input before associate and literal-unit removal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GuardDecisionDagStats {
    pub(crate) candidates: usize,
    pub(crate) atoms: usize,
    pub(crate) candidate_atom_references: usize,
    pub(crate) memo_states: usize,
    pub(crate) nodes: usize,
    pub(crate) edges: usize,
    pub(crate) has_reachable_incomplete: bool,
}

/// One context-bound, reduced, ordered semantic guard decision DAG.
///
/// Nodes are hash-consed using complete structural equality. Hash values are
/// lookup accelerators only and never mathematical identities.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CoefficientIdealGuardDag {
    pub(super) context_fingerprint: Arc<String>,
    pub(super) atoms: Arc<[CoefficientIdealGuardPredicate]>,
    pub(super) candidates: Arc<[CanonicalGuardCandidate]>,
    pub(super) nodes: Arc<[GuardDecisionNode]>,
    pub(super) root: GuardDecisionRef,
    pub(super) stats: GuardDecisionDagStats,
}

impl CoefficientIdealGuardDag {
    pub(crate) const fn stats(&self) -> GuardDecisionDagStats {
        self.stats
    }

    /// Whether every abstract atom branch reaches a retained candidate.
    ///
    /// `true` is a strong, purely combinatorial totality certificate. `false`
    /// deliberately makes no claim that a reachable incomplete Boolean branch
    /// has an integer point: proving exceptional-locus coverage belongs to the
    /// guarded owner-cover layer.
    pub(crate) const fn is_abstractly_total(&self) -> bool {
        !self.stats.has_reachable_incomplete
    }

    pub(crate) fn context_fingerprint(&self) -> &str {
        self.context_fingerprint.as_str()
    }

    pub(crate) fn atom_ordinal(&self, atom: &CoefficientIdealGuardAtom) -> Option<usize> {
        (atom.context_fingerprint() == self.context_fingerprint())
            .then(|| {
                self.atoms
                    .binary_search_by(|candidate| candidate.id().cmp(atom.id()))
                    .ok()
            })
            .flatten()
    }

    /// Evaluate every requested semantic predicate at one authenticated exact
    /// index assignment and route the resulting branch path.
    pub(crate) fn try_decide_at(
        &self,
        context: &IndexedCoefficientContext,
        assignment: &[i64],
        limits: GuardDecisionEvaluationLimits,
    ) -> Result<GuardDecisionOutcome, GuardDecisionDagError> {
        if context.fingerprint() != self.context_fingerprint() {
            return Err(GuardDecisionDagError::WrongEvaluationContext);
        }
        if assignment.len() != context.index_count() {
            return Err(GuardDecisionDagError::IndexedAlgebra(
                IndexedAlgebraError::WrongIndexArity {
                    expected: context.index_count(),
                    actual: assignment.len(),
                },
            ));
        }
        let mut evaluations = 0usize;
        let mut input_terms = 0usize;
        let mut power_operations = 0usize;
        self.try_decide_with_result(|atom| {
            let predicate =
                self.atoms
                    .get(atom)
                    .ok_or(GuardDecisionDagError::InternalInvariant(
                        "atom reference is out of range",
                    ))?;
            evaluations = checked_evaluation_add(
                PREDICATE_EVALUATIONS,
                evaluations,
                1,
                limits.max_predicate_evaluations,
            )?;
            input_terms = checked_evaluation_add(
                EVALUATION_INPUT_TERMS,
                input_terms,
                predicate.input_terms(),
                limits.max_input_terms,
            )?;
            let predicate_power_operations = predicate
                .input_terms()
                .checked_mul(context.index_count())
                .ok_or(GuardDecisionDagError::ResourceCountOverflow {
                    resource: EVALUATION_POWER_OPERATIONS,
                })?;
            power_operations = checked_evaluation_add(
                EVALUATION_POWER_OPERATIONS,
                power_operations,
                predicate_power_operations,
                limits.max_specialization_power_operations,
            )?;
            predicate
                .try_branch_at(context, assignment, limits.indexed_algebra)
                .map_err(GuardDecisionDagError::IndexedAlgebra)
        })
    }

    /// Route a complete abstract Boolean assignment for compiler tests.
    ///
    /// This method does not bind branches to an indexed point or a physical
    /// parameter fibre and therefore carries no admission authority.
    pub(crate) fn try_decide(
        &self,
        branches: &[GuardBranch],
    ) -> Result<GuardDecisionOutcome, GuardDecisionDagError> {
        if branches.len() != self.atoms.len() {
            return Err(GuardDecisionDagError::BranchArity {
                expected: self.atoms.len(),
                actual: branches.len(),
            });
        }
        self.try_decide_with(|atom| branches[atom])
    }

    /// Route with an abstract, lazily queried branch oracle.
    ///
    /// Only atoms on the selected path are requested. The caller is still
    /// responsible for binding every answer to the same exact indexed point;
    /// this test-only compiler does not authenticate that binding and cannot
    /// by itself establish closure ownership.
    pub(crate) fn try_decide_with(
        &self,
        mut branch: impl FnMut(usize) -> GuardBranch,
    ) -> Result<GuardDecisionOutcome, GuardDecisionDagError> {
        self.try_decide_with_result(|atom| Ok(branch(atom)))
    }

    fn try_decide_with_result(
        &self,
        mut branch: impl FnMut(usize) -> Result<GuardBranch, GuardDecisionDagError>,
    ) -> Result<GuardDecisionOutcome, GuardDecisionDagError> {
        let mut cursor = self.root;
        for _ in 0..=self.nodes.len() {
            match cursor {
                GuardDecisionRef::Leaf(outcome) => return Ok(outcome),
                GuardDecisionRef::Node(node) => {
                    let node =
                        self.nodes
                            .get(node)
                            .ok_or(GuardDecisionDagError::InternalInvariant(
                                "node reference is out of range",
                            ))?;
                    if node.atom >= self.atoms.len() {
                        return Err(GuardDecisionDagError::InternalInvariant(
                            "atom reference is out of range",
                        ));
                    }
                    cursor = match branch(node.atom)? {
                        GuardBranch::Zero => node.zero,
                        GuardBranch::NonZero => node.nonzero,
                    };
                }
            }
        }
        Err(GuardDecisionDagError::InternalInvariant(
            "decision graph contains a cycle",
        ))
    }

    pub(crate) fn try_verify(
        &self,
        limits: GuardDecisionDagLimits,
    ) -> Result<bool, GuardDecisionDagError> {
        super::build::try_verify(self, limits)
    }
}

fn checked_evaluation_add(
    resource: &'static str,
    current: usize,
    increment: usize,
    limit: usize,
) -> Result<usize, GuardDecisionDagError> {
    let requested = current
        .checked_add(increment)
        .ok_or(GuardDecisionDagError::ResourceCountOverflow { resource })?;
    if requested > limit {
        Err(GuardDecisionDagError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(requested)
    }
}
