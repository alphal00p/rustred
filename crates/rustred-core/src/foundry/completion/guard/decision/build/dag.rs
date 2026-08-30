use std::collections::HashMap;
use std::sync::Arc;

use super::super::super::model::CoefficientIdealGuardAtomId;
use super::super::model::{CanonicalGuardCandidate, GuardDecisionNode, GuardDecisionRef};
use super::super::{GuardDecisionDagError, GuardDecisionDagLimits, GuardDecisionOutcome};
use super::resource::{
    CANDIDATE_SCANS, EDGES, MEMO_STATE_WORDS, MEMO_STATES, NODES, PENDING_WORK, UNIQUE_ATOMS,
    check_limit, checked_add, try_clone_vec, try_hash_map, try_vec,
};

pub(super) struct DagBuild {
    pub(super) root: GuardDecisionRef,
    pub(super) nodes: Vec<GuardDecisionNode>,
    pub(super) memo_states: usize,
}

pub(super) fn try_build(
    atoms: &[CoefficientIdealGuardAtomId],
    candidates: &[CanonicalGuardCandidate],
    limits: GuardDecisionDagLimits,
) -> Result<DagBuild, GuardDecisionDagError> {
    let mut builder = DagBuilder::new(atoms, candidates, limits)?;
    let root = builder.build()?;
    Ok(DagBuild {
        root,
        memo_states: builder.states.len(),
        nodes: builder.nodes,
    })
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct StateKey {
    next_atom: usize,
    active: Arc<[u64]>,
}

#[derive(Clone, Copy, Debug)]
enum StateStatus {
    Pending,
    Ready(GuardDecisionRef),
}

#[derive(Clone, Debug)]
enum Work {
    Evaluate(StateKey),
    Finish {
        state: StateKey,
        atom: usize,
        zero: StateKey,
        nonzero: StateKey,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct NodeKey {
    atom: usize,
    zero: GuardDecisionRef,
    nonzero: GuardDecisionRef,
}

struct DagBuilder<'a> {
    atoms: &'a [CoefficientIdealGuardAtomId],
    candidates: &'a [CanonicalGuardCandidate],
    limits: GuardDecisionDagLimits,
    nodes: Vec<GuardDecisionNode>,
    node_interner: HashMap<NodeKey, GuardDecisionRef>,
    states: HashMap<StateKey, StateStatus>,
    state_words: usize,
    candidate_scans: usize,
    work: Vec<Work>,
}

impl<'a> DagBuilder<'a> {
    fn new(
        atoms: &'a [CoefficientIdealGuardAtomId],
        candidates: &'a [CanonicalGuardCandidate],
        limits: GuardDecisionDagLimits,
    ) -> Result<Self, GuardDecisionDagError> {
        Ok(Self {
            atoms,
            candidates,
            limits,
            nodes: try_vec(0, NODES)?,
            node_interner: try_hash_map(0, NODES)?,
            states: try_hash_map(0, MEMO_STATES)?,
            state_words: 0,
            candidate_scans: 0,
            work: try_vec(0, PENDING_WORK)?,
        })
    }

    fn build(&mut self) -> Result<GuardDecisionRef, GuardDecisionDagError> {
        let word_count = self.candidates.len().div_ceil(u64::BITS as usize);
        let mut active = try_vec(word_count, MEMO_STATE_WORDS)?;
        active.resize(word_count, u64::MAX);
        if let Some(last) = active.last_mut() {
            let retained = self.candidates.len() % u64::BITS as usize;
            if retained != 0 {
                *last = (1_u64 << retained) - 1;
            }
        }
        let root = StateKey {
            next_atom: 0,
            active: Arc::from(active),
        };
        self.push_work(Work::Evaluate(root.clone()))?;

        while let Some(work) = self.work.pop() {
            match work {
                Work::Evaluate(state) => self.evaluate_state(state)?,
                Work::Finish {
                    state,
                    atom,
                    zero,
                    nonzero,
                } => self.finish_state(state, atom, &zero, &nonzero)?,
            }
        }
        self.ready_state(&root)
    }

    fn evaluate_state(&mut self, state: StateKey) -> Result<(), GuardDecisionDagError> {
        if self.states.contains_key(&state) {
            return Ok(());
        }
        self.insert_pending(state.clone())?;
        let Some(first) = first_active(&state.active, self.candidates.len()) else {
            return self.set_ready(
                &state,
                GuardDecisionRef::Leaf(GuardDecisionOutcome::Incomplete),
            );
        };
        if candidate_is_satisfied(&self.candidates[first], state.next_atom) {
            return self.set_ready(
                &state,
                GuardDecisionRef::Leaf(GuardDecisionOutcome::Candidate(self.candidates[first].id)),
            );
        }

        let atom = self.next_required_atom(&state)?;
        let zero = StateKey {
            next_atom: atom
                .checked_add(1)
                .ok_or(GuardDecisionDagError::ResourceCountOverflow {
                    resource: UNIQUE_ATOMS,
                })?,
            active: self.zero_branch_active(&state, atom)?,
        };
        let nonzero = StateKey {
            next_atom: zero.next_atom,
            active: state.active.clone(),
        };
        self.push_work(Work::Finish {
            state,
            atom,
            zero: zero.clone(),
            nonzero: nonzero.clone(),
        })?;
        self.push_work(Work::Evaluate(nonzero))?;
        self.push_work(Work::Evaluate(zero))
    }

    fn finish_state(
        &mut self,
        state: StateKey,
        atom: usize,
        zero: &StateKey,
        nonzero: &StateKey,
    ) -> Result<(), GuardDecisionDagError> {
        let zero = self.ready_state(zero)?;
        let nonzero = self.ready_state(nonzero)?;
        let decision = self.intern_node(atom, zero, nonzero)?;
        self.set_ready(&state, decision)
    }

    fn next_required_atom(&mut self, state: &StateKey) -> Result<usize, GuardDecisionDagError> {
        let mut next = None;
        for candidate in active_candidates(&state.active, self.candidates.len()) {
            self.charge_candidate_scan()?;
            let required = &self.candidates[candidate].required_atoms;
            let offset = required.partition_point(|&atom| atom < state.next_atom);
            if let Some(&atom) = required.get(offset) {
                next = Some(next.map_or(atom, |current: usize| current.min(atom)));
            }
        }
        next.filter(|&atom| atom < self.atoms.len()).ok_or(
            GuardDecisionDagError::InternalInvariant(
                "active unsatisfied candidates have no remaining guard atom",
            ),
        )
    }

    fn zero_branch_active(
        &mut self,
        state: &StateKey,
        atom: usize,
    ) -> Result<Arc<[u64]>, GuardDecisionDagError> {
        let mut active = try_clone_vec(&state.active, MEMO_STATE_WORDS)?;
        for candidate in active_candidates(&state.active, self.candidates.len()) {
            self.charge_candidate_scan()?;
            if self.candidates[candidate]
                .required_atoms
                .binary_search(&atom)
                .is_ok()
            {
                clear_active(&mut active, candidate);
            }
        }
        Ok(Arc::from(active))
    }

    fn charge_candidate_scan(&mut self) -> Result<(), GuardDecisionDagError> {
        self.candidate_scans = checked_add(CANDIDATE_SCANS, self.candidate_scans, 1)?;
        check_limit(
            CANDIDATE_SCANS,
            self.candidate_scans,
            self.limits.max_candidate_scans,
        )
    }

    fn insert_pending(&mut self, state: StateKey) -> Result<(), GuardDecisionDagError> {
        let requested = checked_add(MEMO_STATES, self.states.len(), 1)?;
        check_limit(MEMO_STATES, requested, self.limits.max_states)?;
        self.state_words = checked_add(MEMO_STATE_WORDS, self.state_words, state.active.len())?;
        check_limit(
            MEMO_STATE_WORDS,
            self.state_words,
            self.limits.max_state_words,
        )?;
        self.states
            .try_reserve(1)
            .map_err(|_| GuardDecisionDagError::AllocationFailure {
                resource: MEMO_STATES,
                requested,
            })?;
        self.states.insert(state, StateStatus::Pending);
        Ok(())
    }

    fn set_ready(
        &mut self,
        state: &StateKey,
        decision: GuardDecisionRef,
    ) -> Result<(), GuardDecisionDagError> {
        let status = self
            .states
            .get_mut(state)
            .ok_or(GuardDecisionDagError::InternalInvariant(
                "memo state was not registered",
            ))?;
        *status = StateStatus::Ready(decision);
        Ok(())
    }

    fn ready_state(&self, state: &StateKey) -> Result<GuardDecisionRef, GuardDecisionDagError> {
        match self.states.get(state) {
            Some(StateStatus::Ready(decision)) => Ok(*decision),
            Some(StateStatus::Pending) => Err(GuardDecisionDagError::InternalInvariant(
                "memo dependency remained pending",
            )),
            None => Err(GuardDecisionDagError::InternalInvariant(
                "memo dependency was not evaluated",
            )),
        }
    }

    fn intern_node(
        &mut self,
        atom: usize,
        zero: GuardDecisionRef,
        nonzero: GuardDecisionRef,
    ) -> Result<GuardDecisionRef, GuardDecisionDagError> {
        if zero == nonzero {
            return Ok(zero);
        }
        self.require_child_order(atom, zero)?;
        self.require_child_order(atom, nonzero)?;
        let key = NodeKey {
            atom,
            zero,
            nonzero,
        };
        if let Some(existing) = self.node_interner.get(&key) {
            return Ok(*existing);
        }
        let requested = checked_add(NODES, self.nodes.len(), 1)?;
        check_limit(NODES, requested, self.limits.max_nodes)?;
        let edges = requested
            .checked_mul(2)
            .ok_or(GuardDecisionDagError::ResourceCountOverflow { resource: EDGES })?;
        check_limit(EDGES, edges, self.limits.max_edges)?;
        self.nodes
            .try_reserve_exact(1)
            .map_err(|_| GuardDecisionDagError::AllocationFailure {
                resource: NODES,
                requested,
            })?;
        self.node_interner.try_reserve(1).map_err(|_| {
            GuardDecisionDagError::AllocationFailure {
                resource: NODES,
                requested,
            }
        })?;
        let decision = GuardDecisionRef::Node(self.nodes.len());
        self.nodes.push(GuardDecisionNode {
            atom,
            zero,
            nonzero,
        });
        self.node_interner.insert(key, decision);
        Ok(decision)
    }

    fn require_child_order(
        &self,
        atom: usize,
        child: GuardDecisionRef,
    ) -> Result<(), GuardDecisionDagError> {
        if let GuardDecisionRef::Node(child) = child {
            let child = self
                .nodes
                .get(child)
                .ok_or(GuardDecisionDagError::InternalInvariant(
                    "child node is out of range",
                ))?;
            if child.atom <= atom {
                return Err(GuardDecisionDagError::InternalInvariant(
                    "guard atoms are not strictly increasing along a path",
                ));
            }
        }
        Ok(())
    }

    fn push_work(&mut self, work: Work) -> Result<(), GuardDecisionDagError> {
        let requested = checked_add(PENDING_WORK, self.work.len(), 1)?;
        check_limit(PENDING_WORK, requested, self.limits.max_pending_work_items)?;
        self.work
            .try_reserve_exact(1)
            .map_err(|_| GuardDecisionDagError::AllocationFailure {
                resource: PENDING_WORK,
                requested,
            })?;
        self.work.push(work);
        Ok(())
    }
}

fn candidate_is_satisfied(candidate: &CanonicalGuardCandidate, next_atom: usize) -> bool {
    candidate
        .required_atoms
        .last()
        .is_none_or(|&atom| atom < next_atom)
}

fn first_active(active: &[u64], candidate_count: usize) -> Option<usize> {
    active_candidates(active, candidate_count).next()
}

fn active_candidates(active: &[u64], candidate_count: usize) -> impl Iterator<Item = usize> + '_ {
    active.iter().enumerate().flat_map(move |(word, &bits)| {
        let base = word * u64::BITS as usize;
        (0..u64::BITS as usize).filter_map(move |bit| {
            let candidate = base + bit;
            (candidate < candidate_count && bits & (1_u64 << bit) != 0).then_some(candidate)
        })
    })
}

fn clear_active(active: &mut [u64], candidate: usize) {
    active[candidate / u64::BITS as usize] &= !(1_u64 << (candidate % u64::BITS as usize));
}
