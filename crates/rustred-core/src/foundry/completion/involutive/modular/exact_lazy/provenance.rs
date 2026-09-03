//! Compact exact source-module derivation lineage.
//!
//! The arena records algebra, not expanded source rows.  In particular
//! `LeftAxpy { target, multiplier, operator_shift, source }` means
//! `target + multiplier * E^operator_shift source`, while `Translate` applies
//! the Ore action to a complete child derivation. This keeps the canonical imported list
//! of `(source ordinal, left shift, left coefficient)` exact without copying
//! source coefficients into every consequence.

use std::collections::HashMap;

use super::super::super::ForwardShift;
use super::super::model::CoeffRef;
use super::error::{check_limit, checked_add};
use super::{ExactLazyError, ExactLazyLimits, ExactLazyOwner, LazyCoeff};

const DERIVATION_NODES: &str = "exact-lazy derivation nodes";
const TOTAL_DERIVATION_NODES: &str = "total exact-lazy derivation nodes created";
const DERIVATION_SHIFT_CELLS: &str = "exact-lazy derivation shift coordinate cells";
const TOTAL_DERIVATION_SHIFT_CELLS: &str =
    "total exact-lazy derivation shift coordinate cells created";
const DERIVATION_LOGICAL_TERMS: &str = "exact-lazy derivation logical source terms";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct DerivationNodeId(u64);

impl DerivationNodeId {
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

/// One owner- and incarnation-bound exact source-derivation root.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SourceDerivationRef {
    owner: ExactLazyOwner,
    id: DerivationNodeId,
    logical_source_terms: usize,
}

impl SourceDerivationRef {
    pub(super) fn owner(&self) -> &ExactLazyOwner {
        &self.owner
    }

    pub(super) const fn logical_source_terms(&self) -> usize {
        self.logical_source_terms
    }
}

/// One owned, read-only view of a live derivation node.
///
/// Children and coefficient roots are rewrapped with the arena owner before
/// they cross this seam.  The lowering interpreter therefore never observes
/// or constructs raw arena ordinals and still has to pass ordinary liveness
/// checks before using a returned value.
#[derive(Clone, Debug)]
pub(super) enum SourceDerivationNodeView {
    Zero,
    Source {
        source_ordinal: usize,
    },
    Translate {
        shift: ForwardShift,
        child: SourceDerivationRef,
    },
    Axpy {
        target: SourceDerivationRef,
        multiplier: LazyCoeff,
        source: SourceDerivationRef,
    },
    LeftAxpy {
        target: SourceDerivationRef,
        multiplier: LazyCoeff,
        operator_shift: ForwardShift,
        source: SourceDerivationRef,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum DerivationNode {
    Zero,
    Source {
        source_ordinal: usize,
    },
    Translate {
        shift: ForwardShift,
        child: DerivationNodeId,
    },
    Axpy {
        target: DerivationNodeId,
        multiplier: CoeffRef,
        source: DerivationNodeId,
    },
    /// `target + multiplier * E^operator_shift source` as one logical
    /// cancellation/import node.  The shift is not separately applied to the
    /// multiplier.
    LeftAxpy {
        target: DerivationNodeId,
        multiplier: CoeffRef,
        operator_shift: ForwardShift,
        source: DerivationNodeId,
    },
}

impl DerivationNode {
    fn retained_shift_cells(&self) -> usize {
        match self {
            Self::Translate { shift, .. }
            | Self::LeftAxpy {
                operator_shift: shift,
                ..
            } => shift.arity(),
            Self::Zero | Self::Source { .. } | Self::Axpy { .. } => 0,
        }
    }
}

#[derive(Debug)]
pub(super) struct ProvenanceArena {
    owner: ExactLazyOwner,
    limits: ExactLazyLimits,
    nodes: Vec<DerivationNode>,
    incarnations: Vec<u32>,
    logical_source_terms: Vec<usize>,
    lookup: HashMap<DerivationNode, DerivationNodeId>,
    next_incarnation: u64,
    retained_shift_cells: usize,
    total_nodes_created: usize,
    total_shift_cells_created: usize,
}

impl ProvenanceArena {
    pub(super) fn try_new(
        owner: ExactLazyOwner,
        limits: ExactLazyLimits,
    ) -> Result<Self, ExactLazyError> {
        check_limit(DERIVATION_NODES, 1, limits.max_derivation_nodes)?;
        check_limit(
            TOTAL_DERIVATION_NODES,
            1,
            limits.max_total_derivation_nodes_created,
        )?;
        let zero = DerivationNode::Zero;
        let zero_id = DerivationNodeId::try_new(0, 0).expect("zero derivation ID is representable");
        let mut nodes = Vec::new();
        try_reserve_vec(&mut nodes, 1, DERIVATION_NODES)?;
        nodes.push(zero.clone());
        let mut incarnations = Vec::new();
        try_reserve_vec(&mut incarnations, 1, DERIVATION_NODES)?;
        incarnations.push(0);
        let mut logical_source_terms = Vec::new();
        try_reserve_vec(&mut logical_source_terms, 1, DERIVATION_NODES)?;
        logical_source_terms.push(0);
        let mut lookup = HashMap::new();
        try_reserve_map(&mut lookup, 1, DERIVATION_NODES)?;
        lookup.insert(zero, zero_id);
        Ok(Self {
            owner,
            limits,
            nodes,
            incarnations,
            logical_source_terms,
            lookup,
            next_incarnation: 1,
            retained_shift_cells: 0,
            total_nodes_created: 1,
            total_shift_cells_created: 0,
        })
    }

    pub(super) fn zero(&self) -> SourceDerivationRef {
        self.wrap(DerivationNodeId::try_new(0, 0).expect("zero derivation ID is fixed"))
    }

    pub(super) fn try_source(
        &mut self,
        source_ordinal: usize,
    ) -> Result<SourceDerivationRef, ExactLazyError> {
        self.try_intern(DerivationNode::Source { source_ordinal }, 1)
    }

    pub(super) fn try_translate(
        &mut self,
        child: &SourceDerivationRef,
        shift: &ForwardShift,
    ) -> Result<SourceDerivationRef, ExactLazyError> {
        let child_id = self.require_id(child)?;
        if shift.arity() != self.owner.arity() {
            return Err(ExactLazyError::WrongArity {
                object: "exact-lazy derivation translation",
                expected: self.owner.arity(),
                actual: shift.arity(),
            });
        }
        // A value constructed under a wider foreign limit contract must not
        // bypass this session's immutable shift ceilings.
        ForwardShift::try_new(shift.values().iter().copied(), self.limits.exact)?;
        if shift.is_zero() || child_id == self.zero_id() {
            return Ok(child.clone());
        }
        let logical_source_terms = self.logical_count(child_id)?;
        self.try_intern(
            DerivationNode::Translate {
                shift: shift.clone(),
                child: child_id,
            },
            logical_source_terms,
        )
    }

    pub(super) fn try_axpy(
        &mut self,
        target: &SourceDerivationRef,
        multiplier: &LazyCoeff,
        source: &SourceDerivationRef,
    ) -> Result<SourceDerivationRef, ExactLazyError> {
        let target = self.require_id(target)?;
        let source = self.require_id(source)?;
        if source == self.zero_id() {
            return Ok(self.wrap(target));
        }
        let logical_source_terms = self.try_sum_logical_counts(target, source)?;
        self.try_intern(
            DerivationNode::Axpy {
                target,
                multiplier: multiplier.root().clone(),
                source,
            },
            logical_source_terms,
        )
    }

    pub(super) fn try_left_axpy(
        &mut self,
        target: &SourceDerivationRef,
        multiplier: &LazyCoeff,
        operator_shift: &ForwardShift,
        source: &SourceDerivationRef,
    ) -> Result<SourceDerivationRef, ExactLazyError> {
        let target = self.require_id(target)?;
        let source = self.require_id(source)?;
        if operator_shift.arity() != self.owner.arity() {
            return Err(ExactLazyError::WrongArity {
                object: "exact-lazy left-AXPY derivation translation",
                expected: self.owner.arity(),
                actual: operator_shift.arity(),
            });
        }
        ForwardShift::try_new(operator_shift.values().iter().copied(), self.limits.exact)?;
        if source == self.zero_id() {
            return Ok(self.wrap(target));
        }
        let logical_source_terms = self.try_sum_logical_counts(target, source)?;
        self.try_intern(
            DerivationNode::LeftAxpy {
                target,
                multiplier: multiplier.root().clone(),
                operator_shift: operator_shift.clone(),
                source,
            },
            logical_source_terms,
        )
    }

    pub(super) fn require_ref(&self, value: &SourceDerivationRef) -> Result<(), ExactLazyError> {
        self.require_id(value).map(|_| ())
    }

    /// Return a fully owner-bound view for one step of the bounded cold
    /// derivation interpreter.
    pub(super) fn try_node_view(
        &self,
        value: &SourceDerivationRef,
    ) -> Result<SourceDerivationNodeView, ExactLazyError> {
        let id = self.require_id(value)?;
        let node = self
            .nodes
            .get(id.as_usize())
            .ok_or(ExactLazyError::InvalidSupport {
                detail: "source derivation node is absent",
            })?;
        Ok(match node {
            DerivationNode::Zero => SourceDerivationNodeView::Zero,
            DerivationNode::Source { source_ordinal } => SourceDerivationNodeView::Source {
                source_ordinal: *source_ordinal,
            },
            DerivationNode::Translate { shift, child } => SourceDerivationNodeView::Translate {
                shift: shift.clone(),
                child: self.wrap(*child),
            },
            DerivationNode::Axpy {
                target,
                multiplier,
                source,
            } => SourceDerivationNodeView::Axpy {
                target: self.wrap(*target),
                multiplier: LazyCoeff::from_owned_root(self.owner.clone(), multiplier.clone()),
                source: self.wrap(*source),
            },
            DerivationNode::LeftAxpy {
                target,
                multiplier,
                operator_shift,
                source,
            } => SourceDerivationNodeView::LeftAxpy {
                target: self.wrap(*target),
                multiplier: LazyCoeff::from_owned_root(self.owner.clone(), multiplier.clone()),
                operator_shift: operator_shift.clone(),
                source: self.wrap(*source),
            },
        })
    }

    fn require_id(&self, value: &SourceDerivationRef) -> Result<DerivationNodeId, ExactLazyError> {
        if !self.owner.belongs_to(value.owner()) {
            return Err(ExactLazyError::WrongSessionOwner);
        }
        if self.incarnations.get(value.id.as_usize()).copied() != Some(value.id.incarnation()) {
            return Err(ExactLazyError::InvalidSupport {
                detail: "source derivation reference is stale",
            });
        }
        if self.logical_count(value.id)? != value.logical_source_terms {
            return Err(ExactLazyError::InvalidSupport {
                detail: "source derivation reference has a forged logical-term count",
            });
        }
        Ok(value.id)
    }

    pub(super) fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub(super) const fn retained_shift_cells(&self) -> usize {
        self.retained_shift_cells
    }

    #[cfg(test)]
    pub(super) const fn cumulative_census(&self) -> (usize, usize) {
        (self.total_nodes_created, self.total_shift_cells_created)
    }

    pub(super) fn checkpoint(&self) -> ProvenanceCheckpoint {
        ProvenanceCheckpoint {
            owner: self.owner.clone(),
            node_count: self.nodes.len(),
            boundary: self
                .live_id(self.nodes.len() - 1)
                .expect("provenance arena always retains zero"),
            retained_shift_cells: self.retained_shift_cells,
        }
    }

    pub(super) fn try_rollback(
        &mut self,
        checkpoint: ProvenanceCheckpoint,
    ) -> Result<(), ExactLazyError> {
        if !self.owner.belongs_to(&checkpoint.owner) {
            return Err(ExactLazyError::WrongSessionOwner);
        }
        if checkpoint.node_count == 0
            || checkpoint.node_count > self.nodes.len()
            || checkpoint.retained_shift_cells > self.retained_shift_cells
            || self.live_id(checkpoint.node_count - 1) != Some(checkpoint.boundary)
        {
            return Err(ExactLazyError::TransactionRollback {
                detail: "invalid provenance-arena checkpoint",
            });
        }
        while self.nodes.len() > checkpoint.node_count {
            let node = self
                .nodes
                .pop()
                .expect("checkpoint bounds provenance rollback");
            self.lookup.remove(&node);
        }
        self.incarnations.truncate(checkpoint.node_count);
        self.logical_source_terms.truncate(checkpoint.node_count);
        self.retained_shift_cells = checkpoint.retained_shift_cells;
        Ok(())
    }

    fn try_intern(
        &mut self,
        node: DerivationNode,
        logical_source_terms: usize,
    ) -> Result<SourceDerivationRef, ExactLazyError> {
        check_limit(
            DERIVATION_LOGICAL_TERMS,
            logical_source_terms,
            self.limits.exact.max_provenance_terms,
        )?;
        if let Some(&id) = self.lookup.get(&node) {
            if self.logical_count(id)? != logical_source_terms {
                return Err(ExactLazyError::InvalidSupport {
                    detail: "hash-consed derivation disagrees on its logical-term count",
                });
            }
            return Ok(self.wrap(id));
        }
        let requested_nodes = checked_add(DERIVATION_NODES, self.nodes.len(), 1)?;
        check_limit(
            DERIVATION_NODES,
            requested_nodes,
            self.limits.max_derivation_nodes,
        )?;
        let requested_total_nodes =
            checked_add(TOTAL_DERIVATION_NODES, self.total_nodes_created, 1)?;
        check_limit(
            TOTAL_DERIVATION_NODES,
            requested_total_nodes,
            self.limits.max_total_derivation_nodes_created,
        )?;
        let shift_cells = node.retained_shift_cells();
        let requested_shift_cells = checked_add(
            DERIVATION_SHIFT_CELLS,
            self.retained_shift_cells,
            shift_cells,
        )?;
        check_limit(
            DERIVATION_SHIFT_CELLS,
            requested_shift_cells,
            self.limits.max_derivation_shift_coordinate_cells,
        )?;
        let requested_total_shift_cells = checked_add(
            TOTAL_DERIVATION_SHIFT_CELLS,
            self.total_shift_cells_created,
            shift_cells,
        )?;
        check_limit(
            TOTAL_DERIVATION_SHIFT_CELLS,
            requested_total_shift_cells,
            self.limits
                .max_total_derivation_shift_coordinate_cells_created,
        )?;
        let incarnation = self.try_issue_incarnation()?;
        let id = DerivationNodeId::try_new(self.nodes.len(), incarnation).ok_or(
            ExactLazyError::ResourceCountOverflow {
                resource: "exact-lazy derivation node identifier",
            },
        )?;
        try_reserve_vec(&mut self.nodes, 1, DERIVATION_NODES)?;
        try_reserve_vec(&mut self.incarnations, 1, DERIVATION_NODES)?;
        try_reserve_vec(&mut self.logical_source_terms, 1, DERIVATION_NODES)?;
        try_reserve_map(&mut self.lookup, 1, DERIVATION_NODES)?;
        self.lookup.insert(node.clone(), id);
        self.nodes.push(node);
        self.incarnations.push(incarnation);
        self.logical_source_terms.push(logical_source_terms);
        self.retained_shift_cells = requested_shift_cells;
        self.total_nodes_created = requested_total_nodes;
        self.total_shift_cells_created = requested_total_shift_cells;
        Ok(self.wrap(id))
    }

    fn try_issue_incarnation(&mut self) -> Result<u32, ExactLazyError> {
        let incarnation = u32::try_from(self.next_incarnation).map_err(|_| {
            ExactLazyError::ResourceCountOverflow {
                resource: "exact-lazy derivation node incarnation",
            }
        })?;
        self.next_incarnation =
            self.next_incarnation
                .checked_add(1)
                .ok_or(ExactLazyError::ResourceCountOverflow {
                    resource: "exact-lazy derivation node incarnation",
                })?;
        Ok(incarnation)
    }

    fn zero_id(&self) -> DerivationNodeId {
        DerivationNodeId::try_new(0, 0).expect("zero derivation ID is fixed")
    }

    fn logical_count(&self, id: DerivationNodeId) -> Result<usize, ExactLazyError> {
        self.logical_source_terms.get(id.as_usize()).copied().ok_or(
            ExactLazyError::InvalidSupport {
                detail: "derivation node lacks its arena-minted logical-term count",
            },
        )
    }

    fn try_sum_logical_counts(
        &self,
        left: DerivationNodeId,
        right: DerivationNodeId,
    ) -> Result<usize, ExactLazyError> {
        let count = checked_add(
            DERIVATION_LOGICAL_TERMS,
            self.logical_count(left)?,
            self.logical_count(right)?,
        )?;
        check_limit(
            DERIVATION_LOGICAL_TERMS,
            count,
            self.limits.exact.max_provenance_terms,
        )?;
        Ok(count)
    }

    fn live_id(&self, ordinal: usize) -> Option<DerivationNodeId> {
        DerivationNodeId::try_new(ordinal, *self.incarnations.get(ordinal)?)
    }

    fn wrap(&self, id: DerivationNodeId) -> SourceDerivationRef {
        SourceDerivationRef {
            owner: self.owner.clone(),
            id,
            logical_source_terms: self
                .logical_source_terms
                .get(id.as_usize())
                .copied()
                .expect("private live derivation ID has a logical-term count"),
        }
    }
}

#[derive(Debug)]
pub(super) struct ProvenanceCheckpoint {
    owner: ExactLazyOwner,
    node_count: usize,
    boundary: DerivationNodeId,
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
