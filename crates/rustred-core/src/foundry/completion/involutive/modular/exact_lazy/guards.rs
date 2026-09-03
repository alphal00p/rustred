//! Typed, compact guard lineage for exact-lazy cancellation.
//!
//! Guard kinds are deliberately not flattened to coefficient roots.  A
//! denominator guard asks whether a rational expression is *defined* at a
//! probe and may legitimately evaluate to zero; polynomial and numerator
//! guards ask for a nonzero value.  This distinction is part of the retained
//! descriptor type, not caller convention.

use std::collections::{HashMap, HashSet};

use crate::algebra::{IndexedCoefficientContext, IndexedPolynomial};

use super::super::super::ForwardShift;
use super::super::model::CoeffRef;
use super::arena::GuardedLeaderInverse;
use super::error::{check_limit, checked_add};
use super::{
    ExactIngressNonzero, ExactLazyError, ExactLazyLimits, ExactLazyOwner, ExactLazyTransaction,
    ExactNonzeroProof, LazyCoeff,
};

const GUARD_NODES: &str = "exact-lazy guard-lineage nodes";
const TOTAL_GUARD_NODES: &str = "total exact-lazy guard-lineage nodes created";
const GUARD_DESCRIPTOR_PAYLOADS: &str = "exact-lazy guard descriptor payloads";
const TOTAL_GUARD_DESCRIPTOR_PAYLOADS: &str = "total exact-lazy guard descriptor payloads created";
const GUARD_SHIFT_CELLS: &str = "exact-lazy guard shift coordinate cells";
const TOTAL_GUARD_SHIFT_CELLS: &str = "total exact-lazy guard shift coordinate cells created";
const GUARD_COLLECTION_VISITS: &str = "exact-lazy guard collection node visits";
const GUARD_COLLECTION_REQUIREMENTS: &str = "exact-lazy guard collection requirements";
const GUARD_LOGICAL_DESCRIPTORS: &str = "exact-lazy guard logical descriptors";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct GuardNodeId(u64);

impl GuardNodeId {
    fn try_new(ordinal: usize, incarnation: u32) -> Option<Self> {
        let ordinal = u32::try_from(ordinal).ok()?;
        Some(Self((u64::from(incarnation) << 32) | u64::from(ordinal)))
    }

    const fn ordinal(self) -> u32 {
        self.0 as u32
    }

    const fn as_usize(self) -> usize {
        self.ordinal() as usize
    }

    const fn incarnation(self) -> u32 {
        (self.0 >> 32) as u32
    }
}

/// One owner- and incarnation-bound root of a guard-lineage DAG.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct GuardLineageRef {
    owner: ExactLazyOwner,
    id: GuardNodeId,
    logical_descriptors: usize,
}

impl GuardLineageRef {
    pub(super) fn owner(&self) -> &ExactLazyOwner {
        &self.owner
    }

    pub(super) const fn logical_descriptors(&self) -> usize {
        self.logical_descriptors
    }
}

/// Opaque evidence that a guard coefficient entered through the indexed
/// polynomial seam and therefore has denominator exactly one.
#[derive(Debug)]
struct DenominatorOneEvidence {
    owner: ExactLazyOwner,
    coefficient: CoeffRef,
}

/// An already authenticated denominator-one polynomial guard.
#[derive(Debug)]
pub(super) struct PolynomialGuardDescriptor {
    coefficient: CoeffRef,
    denominator_one: DenominatorOneEvidence,
}

impl PolynomialGuardDescriptor {
    pub(super) fn try_ingress(
        transaction: &mut ExactLazyTransaction<'_, '_>,
        context: &IndexedCoefficientContext,
        polynomial: &IndexedPolynomial,
    ) -> Result<Self, ExactLazyError> {
        // This crate-private Symbolica-backed conversion is the sole minting
        // path for denominator-one evidence. A generic lazy rational root and
        // a nonzero proof cannot call this constructor.
        let coefficient = context.coefficient_from_polynomial_sealed(polynomial)?;
        let (coefficient, proof) = ExactIngressNonzero::try_ingress(
            transaction,
            context,
            std::sync::Arc::new(coefficient),
        )?;
        let proof = ExactNonzeroProof::ExactIngress(proof);
        if !proof.owns_live(transaction, &coefficient) {
            return Err(ExactLazyError::InvalidProof {
                detail: "polynomial guard ingress proof does not authenticate its root",
            });
        }
        let root = coefficient.root().clone();
        Ok(Self {
            coefficient: root.clone(),
            denominator_one: DenominatorOneEvidence {
                owner: transaction.owner().clone(),
                coefficient: root,
            },
        })
    }
}

/// A request to retain the denominator of one rational DAG expression.
#[derive(Debug)]
pub(super) struct DenominatorOfGuardDescriptor {
    rational: CoeffRef,
}

/// The numerator-nonzero condition introduced by a proof-bound actual-leader
/// inverse. Its constructor accepts the sealed authority, never an arbitrary
/// rational root.
#[derive(Debug)]
pub(super) struct NumeratorOfGuardDescriptor {
    rational: CoeffRef,
}

/// Probe semantics of one leaf descriptor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum GuardProbeRequirement {
    /// The rational image must exist and be nonzero.
    Nonzero(LazyCoeff),
    /// The rational image must exist; a zero value is admissible.
    Defined(LazyCoeff),
}

/// One typed descriptor plus the accumulated Ore translation that must be
/// applied before probing it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct GuardProbeLeaf {
    requirement: GuardProbeRequirement,
    translation: ForwardShift,
}

impl GuardProbeLeaf {
    pub(super) fn into_parts(self) -> (GuardProbeRequirement, ForwardShift) {
        (self.requirement, self.translation)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum GuardNodeKey {
    Empty,
    Polynomial(CoeffRef),
    DenominatorOf(CoeffRef),
    NumeratorOf(CoeffRef),
    Translate {
        shift: ForwardShift,
        child: GuardNodeId,
    },
    Union(GuardNodeId, GuardNodeId),
}

#[derive(Debug)]
enum GuardNode {
    Empty,
    Polynomial(PolynomialGuardDescriptor),
    DenominatorOf(DenominatorOfGuardDescriptor),
    NumeratorOf(NumeratorOfGuardDescriptor),
    Translate {
        shift: ForwardShift,
        child: GuardNodeId,
    },
    Union(GuardNodeId, GuardNodeId),
}

impl GuardNode {
    fn key(&self) -> GuardNodeKey {
        match self {
            Self::Empty => GuardNodeKey::Empty,
            Self::Polynomial(descriptor) => {
                GuardNodeKey::Polynomial(descriptor.coefficient.clone())
            }
            Self::DenominatorOf(descriptor) => {
                GuardNodeKey::DenominatorOf(descriptor.rational.clone())
            }
            Self::NumeratorOf(descriptor) => GuardNodeKey::NumeratorOf(descriptor.rational.clone()),
            Self::Translate { shift, child } => GuardNodeKey::Translate {
                shift: shift.clone(),
                child: *child,
            },
            Self::Union(left, right) => GuardNodeKey::Union(*left, *right),
        }
    }

    fn descriptor_payloads(&self) -> usize {
        usize::from(matches!(
            self,
            Self::Polynomial(_) | Self::DenominatorOf(_) | Self::NumeratorOf(_)
        ))
    }

    fn retained_shift_cells(&self) -> usize {
        match self {
            Self::Translate { shift, .. } => shift.arity(),
            _ => 0,
        }
    }
}

#[derive(Debug)]
pub(super) struct GuardArena {
    owner: ExactLazyOwner,
    limits: ExactLazyLimits,
    nodes: Vec<GuardNode>,
    incarnations: Vec<u32>,
    logical_descriptors: Vec<usize>,
    lookup: HashMap<GuardNodeKey, GuardNodeId>,
    next_incarnation: u64,
    retained_descriptor_payloads: usize,
    retained_shift_cells: usize,
    total_nodes_created: usize,
    total_descriptor_payloads_created: usize,
    total_shift_cells_created: usize,
}

impl GuardArena {
    pub(super) fn try_new(
        owner: ExactLazyOwner,
        limits: ExactLazyLimits,
    ) -> Result<Self, ExactLazyError> {
        check_limit(GUARD_NODES, 1, limits.max_guard_lineage_nodes)?;
        check_limit(
            TOTAL_GUARD_NODES,
            1,
            limits.max_total_guard_lineage_nodes_created,
        )?;
        let empty = GuardNode::Empty;
        let empty_id = GuardNodeId::try_new(0, 0).expect("empty guard ID is representable");
        let mut nodes = Vec::new();
        try_reserve_vec(&mut nodes, 1, GUARD_NODES)?;
        nodes.push(empty);
        let mut incarnations = Vec::new();
        try_reserve_vec(&mut incarnations, 1, GUARD_NODES)?;
        incarnations.push(0);
        let mut logical_descriptors = Vec::new();
        try_reserve_vec(&mut logical_descriptors, 1, GUARD_NODES)?;
        logical_descriptors.push(0);
        let mut lookup = HashMap::new();
        try_reserve_map(&mut lookup, 1, GUARD_NODES)?;
        lookup.insert(GuardNodeKey::Empty, empty_id);
        Ok(Self {
            owner,
            limits,
            nodes,
            incarnations,
            logical_descriptors,
            lookup,
            next_incarnation: 1,
            retained_descriptor_payloads: 0,
            retained_shift_cells: 0,
            total_nodes_created: 1,
            total_descriptor_payloads_created: 0,
            total_shift_cells_created: 0,
        })
    }

    pub(super) fn empty(&self) -> GuardLineageRef {
        self.wrap(self.empty_id())
    }

    pub(super) fn try_polynomial(
        &mut self,
        descriptor: PolynomialGuardDescriptor,
    ) -> Result<GuardLineageRef, ExactLazyError> {
        if !descriptor.denominator_one.owner.belongs_to(&self.owner)
            || descriptor.denominator_one.coefficient != descriptor.coefficient
        {
            return Err(ExactLazyError::InvalidProof {
                detail: "polynomial guard lacks matching denominator-one evidence",
            });
        }
        self.try_intern(GuardNode::Polynomial(descriptor), 1)
    }

    pub(super) fn try_denominator_of(
        &mut self,
        rational: &LazyCoeff,
    ) -> Result<GuardLineageRef, ExactLazyError> {
        self.try_intern(
            GuardNode::DenominatorOf(DenominatorOfGuardDescriptor {
                rational: rational.root().clone(),
            }),
            1,
        )
    }

    pub(super) fn try_numerator_of(
        &mut self,
        seal: &GuardedLeaderInverse,
    ) -> Result<GuardLineageRef, ExactLazyError> {
        self.try_intern(
            GuardNode::NumeratorOf(NumeratorOfGuardDescriptor {
                rational: seal.leader().root().clone(),
            }),
            1,
        )
    }

    pub(super) fn try_union(
        &mut self,
        left: &GuardLineageRef,
        right: &GuardLineageRef,
    ) -> Result<GuardLineageRef, ExactLazyError> {
        let left = self.require_id(left)?;
        let right = self.require_id(right)?;
        if left == self.empty_id() {
            return Ok(self.wrap(right));
        }
        if right == self.empty_id() {
            return Ok(self.wrap(left));
        }
        let logical_descriptors = self.try_sum_logical_counts(left, right)?;
        let (left, right) = if left <= right {
            (left, right)
        } else {
            (right, left)
        };
        self.try_intern(GuardNode::Union(left, right), logical_descriptors)
    }

    pub(super) fn try_translate(
        &mut self,
        guards: &GuardLineageRef,
        shift: &ForwardShift,
    ) -> Result<GuardLineageRef, ExactLazyError> {
        let child = self.require_id(guards)?;
        if shift.arity() != self.owner.arity() {
            return Err(ExactLazyError::WrongArity {
                object: "exact-lazy guard translation",
                expected: self.owner.arity(),
                actual: shift.arity(),
            });
        }
        ForwardShift::try_new(shift.values().iter().copied(), self.limits.exact)?;
        if shift.is_zero() || child == self.empty_id() {
            return Ok(guards.clone());
        }
        let logical_descriptors = self.logical_count(child)?;
        self.try_intern(
            GuardNode::Translate {
                shift: shift.clone(),
                child,
            },
            logical_descriptors,
        )
    }

    pub(super) fn try_probe_requirement(
        &self,
        descriptor: &GuardLineageRef,
    ) -> Result<GuardProbeRequirement, ExactLazyError> {
        let id = self.require_id(descriptor)?;
        match self.node(id)? {
            GuardNode::Polynomial(descriptor) => Ok(GuardProbeRequirement::Nonzero(
                self.wrap_coefficient(&descriptor.coefficient),
            )),
            GuardNode::DenominatorOf(descriptor) => Ok(GuardProbeRequirement::Defined(
                self.wrap_coefficient(&descriptor.rational),
            )),
            GuardNode::NumeratorOf(descriptor) => Ok(GuardProbeRequirement::Nonzero(
                self.wrap_coefficient(&descriptor.rational),
            )),
            GuardNode::Empty | GuardNode::Translate { .. } | GuardNode::Union(_, _) => {
                Err(ExactLazyError::InvalidSupport {
                    detail: "probe requirement requested for a non-descriptor guard node",
                })
            }
        }
    }

    /// Collect typed leaf requirements in deterministic DAG order while
    /// retaining the complete accumulated Ore translation of each leaf.
    pub(super) fn try_collect_probe_leaves(
        &self,
        lineage: &GuardLineageRef,
    ) -> Result<Vec<GuardProbeLeaf>, ExactLazyError> {
        let root = self.require_id(lineage)?;
        let zero = ForwardShift::try_zero(self.owner.arity(), self.limits.exact)?;
        let mut stack = Vec::new();
        try_reserve_vec(&mut stack, 1, GUARD_COLLECTION_VISITS)?;
        stack.push((root, zero));
        let mut visited = HashSet::new();
        let mut leaves = Vec::new();
        let mut visits = 0usize;
        while let Some((node_id, translation)) = stack.pop() {
            visits = checked_add(GUARD_COLLECTION_VISITS, visits, 1)?;
            check_limit(
                GUARD_COLLECTION_VISITS,
                visits,
                self.limits.max_guard_collection_node_visits,
            )?;
            try_reserve_set(&mut visited, 1, GUARD_COLLECTION_VISITS)?;
            if !visited.insert((node_id, translation.clone())) {
                continue;
            }
            match self.node(node_id)? {
                GuardNode::Empty => {}
                GuardNode::Polynomial(descriptor) => self.try_push_probe_leaf(
                    &mut leaves,
                    GuardProbeRequirement::Nonzero(self.wrap_coefficient(&descriptor.coefficient)),
                    translation,
                )?,
                GuardNode::DenominatorOf(descriptor) => self.try_push_probe_leaf(
                    &mut leaves,
                    GuardProbeRequirement::Defined(self.wrap_coefficient(&descriptor.rational)),
                    translation,
                )?,
                GuardNode::NumeratorOf(descriptor) => self.try_push_probe_leaf(
                    &mut leaves,
                    GuardProbeRequirement::Nonzero(self.wrap_coefficient(&descriptor.rational)),
                    translation,
                )?,
                GuardNode::Translate { shift, child } => {
                    let translated = translation.try_checked_add(shift, self.limits.exact)?;
                    try_push_stack(
                        &mut stack,
                        (*child, translated),
                        self.limits.max_guard_collection_node_visits,
                    )?;
                }
                GuardNode::Union(left, right) => {
                    // Push larger/right first so the canonical smaller/left
                    // branch is visited first under LIFO traversal.
                    try_push_stack(
                        &mut stack,
                        (*right, translation.clone()),
                        self.limits.max_guard_collection_node_visits,
                    )?;
                    try_push_stack(
                        &mut stack,
                        (*left, translation),
                        self.limits.max_guard_collection_node_visits,
                    )?;
                }
            }
        }
        Ok(leaves)
    }

    pub(super) fn require_ref(&self, value: &GuardLineageRef) -> Result<(), ExactLazyError> {
        self.require_id(value).map(|_| ())
    }

    fn require_id(&self, value: &GuardLineageRef) -> Result<GuardNodeId, ExactLazyError> {
        if !self.owner.belongs_to(value.owner()) {
            return Err(ExactLazyError::WrongSessionOwner);
        }
        if self.incarnations.get(value.id.as_usize()).copied() != Some(value.id.incarnation()) {
            return Err(ExactLazyError::InvalidSupport {
                detail: "guard-lineage reference is stale",
            });
        }
        if self.logical_count(value.id)? != value.logical_descriptors {
            return Err(ExactLazyError::InvalidSupport {
                detail: "guard-lineage reference has a forged logical-descriptor count",
            });
        }
        Ok(value.id)
    }

    pub(super) fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub(super) const fn retained_descriptor_payloads(&self) -> usize {
        self.retained_descriptor_payloads
    }

    pub(super) const fn retained_shift_cells(&self) -> usize {
        self.retained_shift_cells
    }

    #[cfg(test)]
    pub(super) const fn cumulative_census(&self) -> (usize, usize, usize) {
        (
            self.total_nodes_created,
            self.total_descriptor_payloads_created,
            self.total_shift_cells_created,
        )
    }

    pub(super) fn checkpoint(&self) -> GuardCheckpoint {
        GuardCheckpoint {
            owner: self.owner.clone(),
            node_count: self.nodes.len(),
            boundary: self
                .live_id(self.nodes.len() - 1)
                .expect("guard arena always retains empty"),
            retained_descriptor_payloads: self.retained_descriptor_payloads,
            retained_shift_cells: self.retained_shift_cells,
        }
    }

    pub(super) fn try_rollback(
        &mut self,
        checkpoint: GuardCheckpoint,
    ) -> Result<(), ExactLazyError> {
        if !self.owner.belongs_to(&checkpoint.owner) {
            return Err(ExactLazyError::WrongSessionOwner);
        }
        if checkpoint.node_count == 0
            || checkpoint.node_count > self.nodes.len()
            || checkpoint.retained_descriptor_payloads > self.retained_descriptor_payloads
            || checkpoint.retained_shift_cells > self.retained_shift_cells
            || self.live_id(checkpoint.node_count - 1) != Some(checkpoint.boundary)
        {
            return Err(ExactLazyError::TransactionRollback {
                detail: "invalid guard-arena checkpoint",
            });
        }
        while self.nodes.len() > checkpoint.node_count {
            let node = self.nodes.pop().expect("checkpoint bounds guard rollback");
            self.lookup.remove(&node.key());
        }
        self.incarnations.truncate(checkpoint.node_count);
        self.logical_descriptors.truncate(checkpoint.node_count);
        self.retained_descriptor_payloads = checkpoint.retained_descriptor_payloads;
        self.retained_shift_cells = checkpoint.retained_shift_cells;
        Ok(())
    }

    fn try_intern(
        &mut self,
        node: GuardNode,
        logical_descriptors: usize,
    ) -> Result<GuardLineageRef, ExactLazyError> {
        check_limit(
            GUARD_LOGICAL_DESCRIPTORS,
            logical_descriptors,
            self.limits.exact.max_localization_guards,
        )?;
        let key = node.key();
        if let Some(&id) = self.lookup.get(&key) {
            if self.logical_count(id)? != logical_descriptors {
                return Err(ExactLazyError::InvalidSupport {
                    detail: "hash-consed guard lineage disagrees on its logical-descriptor count",
                });
            }
            return Ok(self.wrap(id));
        }
        let requested_nodes = checked_add(GUARD_NODES, self.nodes.len(), 1)?;
        check_limit(
            GUARD_NODES,
            requested_nodes,
            self.limits.max_guard_lineage_nodes,
        )?;
        let requested_total_nodes = checked_add(TOTAL_GUARD_NODES, self.total_nodes_created, 1)?;
        check_limit(
            TOTAL_GUARD_NODES,
            requested_total_nodes,
            self.limits.max_total_guard_lineage_nodes_created,
        )?;
        let descriptor_payloads = node.descriptor_payloads();
        let requested_descriptor_payloads = checked_add(
            GUARD_DESCRIPTOR_PAYLOADS,
            self.retained_descriptor_payloads,
            descriptor_payloads,
        )?;
        check_limit(
            GUARD_DESCRIPTOR_PAYLOADS,
            requested_descriptor_payloads,
            self.limits.max_guard_descriptor_payloads,
        )?;
        let requested_total_descriptor_payloads = checked_add(
            TOTAL_GUARD_DESCRIPTOR_PAYLOADS,
            self.total_descriptor_payloads_created,
            descriptor_payloads,
        )?;
        check_limit(
            TOTAL_GUARD_DESCRIPTOR_PAYLOADS,
            requested_total_descriptor_payloads,
            self.limits.max_total_guard_descriptor_payloads_created,
        )?;
        let shift_cells = node.retained_shift_cells();
        let requested_shift_cells =
            checked_add(GUARD_SHIFT_CELLS, self.retained_shift_cells, shift_cells)?;
        check_limit(
            GUARD_SHIFT_CELLS,
            requested_shift_cells,
            self.limits.max_guard_shift_coordinate_cells,
        )?;
        let requested_total_shift_cells = checked_add(
            TOTAL_GUARD_SHIFT_CELLS,
            self.total_shift_cells_created,
            shift_cells,
        )?;
        check_limit(
            TOTAL_GUARD_SHIFT_CELLS,
            requested_total_shift_cells,
            self.limits.max_total_guard_shift_coordinate_cells_created,
        )?;
        let incarnation = self.try_issue_incarnation()?;
        let id = GuardNodeId::try_new(self.nodes.len(), incarnation).ok_or(
            ExactLazyError::ResourceCountOverflow {
                resource: "exact-lazy guard-lineage node identifier",
            },
        )?;
        try_reserve_vec(&mut self.nodes, 1, GUARD_NODES)?;
        try_reserve_vec(&mut self.incarnations, 1, GUARD_NODES)?;
        try_reserve_vec(&mut self.logical_descriptors, 1, GUARD_NODES)?;
        try_reserve_map(&mut self.lookup, 1, GUARD_NODES)?;
        self.lookup.insert(key, id);
        self.nodes.push(node);
        self.incarnations.push(incarnation);
        self.logical_descriptors.push(logical_descriptors);
        self.retained_descriptor_payloads = requested_descriptor_payloads;
        self.retained_shift_cells = requested_shift_cells;
        self.total_nodes_created = requested_total_nodes;
        self.total_descriptor_payloads_created = requested_total_descriptor_payloads;
        self.total_shift_cells_created = requested_total_shift_cells;
        Ok(self.wrap(id))
    }

    fn try_push_probe_leaf(
        &self,
        leaves: &mut Vec<GuardProbeLeaf>,
        requirement: GuardProbeRequirement,
        translation: ForwardShift,
    ) -> Result<(), ExactLazyError> {
        let requested = checked_add(GUARD_COLLECTION_REQUIREMENTS, leaves.len(), 1)?;
        check_limit(
            GUARD_COLLECTION_REQUIREMENTS,
            requested,
            self.limits.max_guard_collection_requirements,
        )?;
        try_reserve_vec(leaves, 1, GUARD_COLLECTION_REQUIREMENTS)?;
        leaves.push(GuardProbeLeaf {
            requirement,
            translation,
        });
        Ok(())
    }

    fn node(&self, id: GuardNodeId) -> Result<&GuardNode, ExactLazyError> {
        if self.incarnations.get(id.as_usize()).copied() != Some(id.incarnation()) {
            return Err(ExactLazyError::InvalidSupport {
                detail: "guard-lineage node identifier is stale",
            });
        }
        self.nodes
            .get(id.as_usize())
            .ok_or(ExactLazyError::InvalidSupport {
                detail: "guard-lineage node identifier is absent",
            })
    }

    fn try_issue_incarnation(&mut self) -> Result<u32, ExactLazyError> {
        let incarnation = u32::try_from(self.next_incarnation).map_err(|_| {
            ExactLazyError::ResourceCountOverflow {
                resource: "exact-lazy guard-lineage node incarnation",
            }
        })?;
        self.next_incarnation =
            self.next_incarnation
                .checked_add(1)
                .ok_or(ExactLazyError::ResourceCountOverflow {
                    resource: "exact-lazy guard-lineage node incarnation",
                })?;
        Ok(incarnation)
    }

    fn empty_id(&self) -> GuardNodeId {
        GuardNodeId::try_new(0, 0).expect("empty guard ID is fixed")
    }

    fn live_id(&self, ordinal: usize) -> Option<GuardNodeId> {
        GuardNodeId::try_new(ordinal, *self.incarnations.get(ordinal)?)
    }

    fn logical_count(&self, id: GuardNodeId) -> Result<usize, ExactLazyError> {
        self.logical_descriptors
            .get(id.as_usize())
            .copied()
            .ok_or(ExactLazyError::InvalidSupport {
                detail: "guard-lineage node lacks its arena-minted logical-descriptor count",
            })
    }

    fn try_sum_logical_counts(
        &self,
        left: GuardNodeId,
        right: GuardNodeId,
    ) -> Result<usize, ExactLazyError> {
        let count = checked_add(
            GUARD_LOGICAL_DESCRIPTORS,
            self.logical_count(left)?,
            self.logical_count(right)?,
        )?;
        check_limit(
            GUARD_LOGICAL_DESCRIPTORS,
            count,
            self.limits.exact.max_localization_guards,
        )?;
        Ok(count)
    }

    fn wrap(&self, id: GuardNodeId) -> GuardLineageRef {
        GuardLineageRef {
            owner: self.owner.clone(),
            id,
            logical_descriptors: self
                .logical_descriptors
                .get(id.as_usize())
                .copied()
                .expect("private live guard ID has a logical-descriptor count"),
        }
    }

    fn wrap_coefficient(&self, root: &CoeffRef) -> LazyCoeff {
        LazyCoeff::from_owned_root(self.owner.clone(), root.clone())
    }
}

#[derive(Debug)]
pub(super) struct GuardCheckpoint {
    owner: ExactLazyOwner,
    node_count: usize,
    boundary: GuardNodeId,
    retained_descriptor_payloads: usize,
    retained_shift_cells: usize,
}

fn try_reserve_vec<T>(
    values: &mut Vec<T>,
    additional: usize,
    resource: &'static str,
) -> Result<(), ExactLazyError> {
    values
        .try_reserve(additional)
        .map_err(|_| ExactLazyError::AllocationFailure {
            resource,
            requested: additional,
        })
}

fn try_reserve_map<K, V>(
    values: &mut HashMap<K, V>,
    additional: usize,
    resource: &'static str,
) -> Result<(), ExactLazyError>
where
    K: Eq + std::hash::Hash,
{
    values
        .try_reserve(additional)
        .map_err(|_| ExactLazyError::AllocationFailure {
            resource,
            requested: additional,
        })
}

fn try_reserve_set<T>(
    values: &mut HashSet<T>,
    additional: usize,
    resource: &'static str,
) -> Result<(), ExactLazyError>
where
    T: Eq + std::hash::Hash,
{
    values
        .try_reserve(additional)
        .map_err(|_| ExactLazyError::AllocationFailure {
            resource,
            requested: additional,
        })
}

fn try_push_stack<T>(values: &mut Vec<T>, value: T, limit: usize) -> Result<(), ExactLazyError> {
    let requested = checked_add(GUARD_COLLECTION_VISITS, values.len(), 1)?;
    check_limit(GUARD_COLLECTION_VISITS, requested, limit)?;
    try_reserve_vec(values, 1, GUARD_COLLECTION_VISITS)?;
    values.push(value);
    Ok(())
}
