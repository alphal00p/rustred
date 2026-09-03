use std::collections::HashMap;
use std::mem::size_of;
use std::sync::Arc;

use crate::algebra::{IndexedCoefficient, IndexedCoefficientContext};

use super::super::{ForwardShift, OreOrderingAdapter};
use super::ModularGuideError;
use super::error::{check_limit, checked_add, reserve_map, reserve_vec};
use super::limits::ModularGuideLimits;
use super::model::{
    CoeffNode, CoeffNodeId, CoeffRef, DagOwner, ExactLeaf, ExactLeafId, ExactLeafKey,
    PhysicalDeltaId, RawCoeffRef,
};
use super::payload::{CoefficientPayloadWeight, try_coefficient_payload_weight};

const DAG_NODES: &str = "modular coefficient DAG nodes";
const TOTAL_DAG_NODES: &str = "total modular coefficient DAG nodes created";
const EXACT_LEAVES: &str = "modular coefficient exact leaves";
const EXACT_LEAF_TERMS: &str = "modular coefficient exact-leaf terms";
const EXACT_LEAF_EXPONENT_CELLS: &str = "modular coefficient exact-leaf exponent cells";
const EXACT_LEAF_BYTES: &str = "modular coefficient exact-leaf retained bytes";
const TOTAL_EXACT_LEAF_TERMS: &str = "total modular coefficient exact-leaf terms ingressed";
const TOTAL_EXACT_LEAF_EXPONENT_CELLS: &str =
    "total modular coefficient exact-leaf exponent cells ingressed";
const TOTAL_EXACT_LEAF_BYTES: &str = "total modular coefficient exact-leaf bytes ingressed";
const PHYSICAL_DELTAS: &str = "modular coefficient physical deltas";
const TOTAL_PHYSICAL_DELTAS: &str = "total modular coefficient physical deltas created";
const PHYSICAL_DELTA_CELLS: &str = "modular coefficient physical-delta coordinate cells";
const TOTAL_PHYSICAL_DELTA_COORDINATE_OPERATIONS: &str =
    "total modular physical-delta coordinate operations";

/// Field-independent, hash-consed coefficient-expression storage.
///
/// Exact coefficients enter once as immutable authenticated leaves. Algebraic
/// nodes contain only stable earlier-node references, while signed physical
/// translations are interned separately and compose without expanding an
/// exact polynomial.
#[derive(Debug)]
pub(super) struct ModularCoefficientDag {
    owner: DagOwner,
    context_fingerprint: Arc<String>,
    index_count: usize,
    limits: ModularGuideLimits,
    nodes: Vec<CoeffNode>,
    node_incarnations: Vec<u32>,
    next_node_incarnation: u64,
    total_nodes_created: usize,
    node_lookup: HashMap<CoeffNode, CoeffNodeId>,
    exact_leaves: Vec<ExactLeaf>,
    exact_leaf_lookup: HashMap<ExactLeafKey, ExactLeafId>,
    exact_leaf_terms: usize,
    exact_leaf_exponent_cells: usize,
    exact_leaf_retained_bytes: usize,
    total_exact_leaf_terms_ingressed: usize,
    total_exact_leaf_exponent_cells_ingressed: usize,
    total_exact_leaf_bytes_ingressed: usize,
    deltas: Vec<Arc<Vec<i64>>>,
    delta_incarnations: Vec<u32>,
    next_delta_incarnation: u64,
    total_physical_deltas_created: usize,
    total_physical_delta_coordinate_operations: usize,
    delta_lookup: HashMap<Arc<Vec<i64>>, PhysicalDeltaId>,
    delta_coordinate_cells: usize,
}

impl ModularCoefficientDag {
    pub(super) fn try_new(
        context: &IndexedCoefficientContext,
        limits: ModularGuideLimits,
    ) -> Result<Self, ModularGuideError> {
        check_limit(DAG_NODES, 2, limits.max_nodes)?;
        check_limit(TOTAL_DAG_NODES, 2, limits.max_total_nodes_created)?;
        check_limit(PHYSICAL_DELTAS, 1, limits.max_physical_deltas)?;
        check_limit(
            TOTAL_PHYSICAL_DELTAS,
            1,
            limits.max_total_physical_deltas_created,
        )?;
        check_limit(
            PHYSICAL_DELTA_CELLS,
            context.index_count(),
            limits.max_physical_delta_coordinate_cells,
        )?;

        let mut nodes = Vec::new();
        reserve_vec(&mut nodes, 2, DAG_NODES)?;
        let mut node_lookup = HashMap::new();
        reserve_map(&mut node_lookup, 2, DAG_NODES)?;
        let zero_id = CoeffNodeId::try_new(0, 0).expect("zero is representable as a DAG node ID");
        let one_id = CoeffNodeId::try_new(1, 1).expect("one is representable as a DAG node ID");
        nodes.push(CoeffNode::Zero);
        nodes.push(CoeffNode::One);
        let mut node_incarnations = Vec::new();
        reserve_vec(&mut node_incarnations, 2, DAG_NODES)?;
        node_incarnations.extend([zero_id.incarnation(), one_id.incarnation()]);
        node_lookup.insert(CoeffNode::Zero, zero_id);
        node_lookup.insert(CoeffNode::One, one_id);

        let mut zero_delta = Vec::new();
        reserve_vec(&mut zero_delta, context.index_count(), PHYSICAL_DELTA_CELLS)?;
        zero_delta.resize(context.index_count(), 0);
        let zero_delta = Arc::new(zero_delta);
        let zero_delta_id =
            PhysicalDeltaId::try_new(0, 0).expect("zero is representable as a physical-delta ID");
        let mut deltas = Vec::new();
        reserve_vec(&mut deltas, 1, PHYSICAL_DELTAS)?;
        deltas.push(Arc::clone(&zero_delta));
        let mut delta_incarnations = Vec::new();
        reserve_vec(&mut delta_incarnations, 1, PHYSICAL_DELTAS)?;
        delta_incarnations.push(zero_delta_id.incarnation());
        let mut delta_lookup = HashMap::new();
        reserve_map(&mut delta_lookup, 1, PHYSICAL_DELTAS)?;
        delta_lookup.insert(zero_delta, zero_delta_id);

        Ok(Self {
            owner: DagOwner::fresh(),
            context_fingerprint: context.fingerprint_owner(),
            index_count: context.index_count(),
            limits,
            nodes,
            node_incarnations,
            next_node_incarnation: 2,
            total_nodes_created: 2,
            node_lookup,
            exact_leaves: Vec::new(),
            exact_leaf_lookup: HashMap::new(),
            exact_leaf_terms: 0,
            exact_leaf_exponent_cells: 0,
            exact_leaf_retained_bytes: 0,
            total_exact_leaf_terms_ingressed: 0,
            total_exact_leaf_exponent_cells_ingressed: 0,
            total_exact_leaf_bytes_ingressed: 0,
            deltas,
            delta_incarnations,
            next_delta_incarnation: 1,
            total_physical_deltas_created: 1,
            total_physical_delta_coordinate_operations: 0,
            delta_lookup,
            delta_coordinate_cells: context.index_count(),
        })
    }

    pub(super) fn zero(&self) -> CoeffRef {
        self.wrap(RawCoeffRef {
            node: CoeffNodeId::try_new(0, 0).expect("zero DAG node ID is fixed"),
            translation: self.zero_delta_id(),
        })
    }

    pub(super) fn one(&self) -> CoeffRef {
        self.wrap(RawCoeffRef {
            node: CoeffNodeId::try_new(1, 1).expect("one DAG node ID is fixed"),
            translation: self.zero_delta_id(),
        })
    }

    pub(super) fn try_exact_leaf(
        &mut self,
        context: &IndexedCoefficientContext,
        coefficient: Arc<IndexedCoefficient>,
    ) -> Result<CoeffRef, ModularGuideError> {
        if !self.owns_context(context) {
            return Err(ModularGuideError::WrongIndexedContext);
        }
        context.bind_sealed(&coefficient)?;
        if coefficient.is_zero() {
            return Ok(self.zero());
        }
        if *coefficient == context.one() {
            return Ok(self.one());
        }
        let weight = exact_leaf_weight(&coefficient)?;
        let requested_total_leaf_terms = checked_add(
            TOTAL_EXACT_LEAF_TERMS,
            self.total_exact_leaf_terms_ingressed,
            weight.terms,
        )?;
        let requested_total_leaf_exponent_cells = checked_add(
            TOTAL_EXACT_LEAF_EXPONENT_CELLS,
            self.total_exact_leaf_exponent_cells_ingressed,
            weight.exponent_cells,
        )?;
        let requested_total_leaf_bytes = checked_add(
            TOTAL_EXACT_LEAF_BYTES,
            self.total_exact_leaf_bytes_ingressed,
            weight.bytes,
        )?;
        check_limit(
            TOTAL_EXACT_LEAF_TERMS,
            requested_total_leaf_terms,
            self.limits.max_total_exact_leaf_terms_ingressed,
        )?;
        check_limit(
            TOTAL_EXACT_LEAF_EXPONENT_CELLS,
            requested_total_leaf_exponent_cells,
            self.limits.max_total_exact_leaf_exponent_cells_ingressed,
        )?;
        check_limit(
            TOTAL_EXACT_LEAF_BYTES,
            requested_total_leaf_bytes,
            self.limits.max_total_exact_leaf_bytes_ingressed,
        )?;
        // Ingress hashing/equality scans happen below, so their conservative
        // payload charge is monotone even for an already-interned leaf or a
        // later allocation failure.
        self.total_exact_leaf_terms_ingressed = requested_total_leaf_terms;
        self.total_exact_leaf_exponent_cells_ingressed = requested_total_leaf_exponent_cells;
        self.total_exact_leaf_bytes_ingressed = requested_total_leaf_bytes;
        let key = ExactLeafKey(Arc::clone(&coefficient));
        if let Some(&leaf_id) = self.exact_leaf_lookup.get(&key) {
            let node = self
                .node_lookup
                .get(&CoeffNode::ExactLeaf(leaf_id))
                .copied()
                .ok_or(ModularGuideError::Invariant {
                    detail: "interned exact leaf is absent from the DAG node map",
                })?;
            return Ok(self.wrap(RawCoeffRef {
                node,
                translation: self.zero_delta_id(),
            }));
        }

        let requested_leaves = checked_add(EXACT_LEAVES, self.exact_leaves.len(), 1)?;
        check_limit(EXACT_LEAVES, requested_leaves, self.limits.max_exact_leaves)?;
        let requested_nodes = checked_add(DAG_NODES, self.nodes.len(), 1)?;
        check_limit(DAG_NODES, requested_nodes, self.limits.max_nodes)?;
        let requested_total_nodes = checked_add(TOTAL_DAG_NODES, self.total_nodes_created, 1)?;
        check_limit(
            TOTAL_DAG_NODES,
            requested_total_nodes,
            self.limits.max_total_nodes_created,
        )?;
        let requested_leaf_terms =
            checked_add(EXACT_LEAF_TERMS, self.exact_leaf_terms, weight.terms)?;
        let requested_leaf_exponent_cells = checked_add(
            EXACT_LEAF_EXPONENT_CELLS,
            self.exact_leaf_exponent_cells,
            weight.exponent_cells,
        )?;
        let requested_leaf_bytes = checked_add(
            EXACT_LEAF_BYTES,
            self.exact_leaf_retained_bytes,
            weight.bytes,
        )?;
        check_limit(
            EXACT_LEAF_TERMS,
            requested_leaf_terms,
            self.limits.max_exact_leaf_terms,
        )?;
        check_limit(
            EXACT_LEAF_EXPONENT_CELLS,
            requested_leaf_exponent_cells,
            self.limits.max_exact_leaf_exponent_cells,
        )?;
        check_limit(
            EXACT_LEAF_BYTES,
            requested_leaf_bytes,
            self.limits.max_exact_leaf_retained_bytes,
        )?;
        let leaf_id = ExactLeafId::try_new(self.exact_leaves.len()).ok_or(
            ModularGuideError::IdentifierNotRepresentable {
                resource: "modular coefficient exact-leaf identifier",
                value: self.exact_leaves.len(),
            },
        )?;
        let node_incarnation = self.try_issue_node_incarnation()?;
        let node_id = CoeffNodeId::try_new(self.nodes.len(), node_incarnation).ok_or(
            ModularGuideError::IdentifierNotRepresentable {
                resource: "modular coefficient DAG node identifier",
                value: self.nodes.len(),
            },
        )?;
        let node = CoeffNode::ExactLeaf(leaf_id);

        reserve_vec(&mut self.exact_leaves, 1, EXACT_LEAVES)?;
        reserve_map(&mut self.exact_leaf_lookup, 1, EXACT_LEAVES)?;
        reserve_vec(&mut self.nodes, 1, DAG_NODES)?;
        reserve_vec(&mut self.node_incarnations, 1, DAG_NODES)?;
        reserve_map(&mut self.node_lookup, 1, DAG_NODES)?;
        self.exact_leaf_lookup.insert(key, leaf_id);
        self.exact_leaves.push(coefficient);
        self.node_lookup.insert(node.clone(), node_id);
        self.nodes.push(node);
        self.node_incarnations.push(node_incarnation);
        self.total_nodes_created = requested_total_nodes;
        self.exact_leaf_terms = requested_leaf_terms;
        self.exact_leaf_exponent_cells = requested_leaf_exponent_cells;
        self.exact_leaf_retained_bytes = requested_leaf_bytes;
        Ok(self.wrap(RawCoeffRef {
            node: node_id,
            translation: self.zero_delta_id(),
        }))
    }

    pub(super) fn try_neg(&mut self, value: &CoeffRef) -> Result<CoeffRef, ModularGuideError> {
        let raw = self.require(value)?;
        if self.is_zero_raw(raw) {
            return Ok(self.zero());
        }
        if let CoeffNode::Neg(child) = self.node(raw.node)?.clone() {
            let collapsed = self.try_compose_raw(child, raw.translation)?;
            return Ok(self.wrap(collapsed));
        }
        let node = CoeffNode::Neg(raw);
        self.try_intern_node(node)
    }

    pub(super) fn try_add(
        &mut self,
        left: &CoeffRef,
        right: &CoeffRef,
    ) -> Result<CoeffRef, ModularGuideError> {
        let left = self.require(left)?;
        let right = self.require(right)?;
        if self.is_zero_raw(left) {
            return Ok(self.wrap(right));
        }
        if self.is_zero_raw(right) {
            return Ok(self.wrap(left));
        }
        if self.is_effective_negated_operand(left, right)?
            || self.is_effective_negated_operand(right, left)?
        {
            return Ok(self.zero());
        }
        let (left, right) = canonical_pair(left, right);
        self.try_intern_node(CoeffNode::Add(left, right))
    }

    pub(super) fn try_sub(
        &mut self,
        left: &CoeffRef,
        right: &CoeffRef,
    ) -> Result<CoeffRef, ModularGuideError> {
        let checkpoint = self.checkpoint();
        let result = self.try_sub_inner(left, right);
        match result {
            Ok(value) => Ok(value),
            Err(error) => {
                self.try_rollback(checkpoint)?;
                Err(error)
            }
        }
    }

    fn try_sub_inner(
        &mut self,
        left: &CoeffRef,
        right: &CoeffRef,
    ) -> Result<CoeffRef, ModularGuideError> {
        let left_raw = self.require(left)?;
        let right_raw = self.require(right)?;
        if left_raw == right_raw {
            return Ok(self.zero());
        }
        let negative = self.try_neg(right)?;
        self.try_add(left, &negative)
    }

    pub(super) fn try_mul(
        &mut self,
        left: &CoeffRef,
        right: &CoeffRef,
    ) -> Result<CoeffRef, ModularGuideError> {
        let left = self.require(left)?;
        let right = self.require(right)?;
        if self.is_zero_raw(left) || self.is_zero_raw(right) {
            return Ok(self.zero());
        }
        if self.is_one_raw(left) {
            return Ok(self.wrap(right));
        }
        if self.is_one_raw(right) {
            return Ok(self.wrap(left));
        }
        let (left, right) = canonical_pair(left, right);
        self.try_intern_node(CoeffNode::Mul(left, right))
    }

    pub(super) fn try_inv(&mut self, value: &CoeffRef) -> Result<CoeffRef, ModularGuideError> {
        let raw = self.require(value)?;
        if self.is_zero_raw(raw) {
            return Err(ModularGuideError::StructurallyZeroInverse);
        }
        if self.is_one_raw(raw) {
            return Ok(self.one());
        }
        if let CoeffNode::Inv(child) = self.node(raw.node)?.clone() {
            let collapsed = self.try_compose_raw(child, raw.translation)?;
            return Ok(self.wrap(collapsed));
        }
        self.try_intern_node(CoeffNode::Inv(raw))
    }

    pub(super) fn try_div(
        &mut self,
        numerator: &CoeffRef,
        denominator: &CoeffRef,
    ) -> Result<CoeffRef, ModularGuideError> {
        let checkpoint = self.checkpoint();
        let result = self.try_div_inner(numerator, denominator);
        match result {
            Ok(value) => Ok(value),
            Err(error) => {
                self.try_rollback(checkpoint)?;
                Err(error)
            }
        }
    }

    fn try_div_inner(
        &mut self,
        numerator: &CoeffRef,
        denominator: &CoeffRef,
    ) -> Result<CoeffRef, ModularGuideError> {
        let numerator_raw = self.require(numerator)?;
        let denominator_raw = self.require(denominator)?;
        if self.is_zero_raw(denominator_raw) {
            return Err(ModularGuideError::StructurallyZeroInverse);
        }
        if self.is_zero_raw(numerator_raw) {
            return Ok(self.zero());
        }
        let inverse = self.try_inv(denominator)?;
        self.try_mul(numerator, &inverse)
    }

    pub(super) fn try_translate_physical(
        &mut self,
        value: &CoeffRef,
        physical_delta: &[i64],
    ) -> Result<CoeffRef, ModularGuideError> {
        let raw = self.require(value)?;
        self.require_translation_arity(physical_delta)?;
        if self.is_zero_raw(raw) || self.is_one_raw(raw) || physical_delta.iter().all(|&v| v == 0) {
            return Ok(self.wrap(raw));
        }
        let translation = self.try_intern_delta_sum(raw.translation, physical_delta)?;
        Ok(self.wrap(RawCoeffRef {
            node: raw.node,
            translation,
        }))
    }

    /// Apply the sector-aware Ore automorphism for one forward operator
    /// shift. Keeping the sign map behind the ordering authority prevents a
    /// modular caller from accidentally treating inactive axes as active.
    pub(super) fn try_translate_by_operator(
        &mut self,
        value: &CoeffRef,
        operator_shift: &ForwardShift,
        ordering: &OreOrderingAdapter,
    ) -> Result<CoeffRef, ModularGuideError> {
        let physical_delta = ordering.try_physical_translation(operator_shift)?;
        self.try_translate_physical(value, &physical_delta)
    }

    pub(super) fn is_known_zero(&self, value: &CoeffRef) -> Result<bool, ModularGuideError> {
        Ok(self.is_zero_raw(self.require(value)?))
    }

    pub(super) fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub(super) fn exact_leaf_count(&self) -> usize {
        self.exact_leaves.len()
    }

    #[cfg(test)]
    pub(super) const fn exact_leaf_payload_census(&self) -> (usize, usize, usize) {
        (
            self.exact_leaf_terms,
            self.exact_leaf_exponent_cells,
            self.exact_leaf_retained_bytes,
        )
    }

    #[cfg(test)]
    pub(super) const fn cumulative_creation_census(
        &self,
    ) -> (usize, usize, usize, usize, usize, usize) {
        (
            self.total_nodes_created,
            self.total_physical_deltas_created,
            self.total_physical_delta_coordinate_operations,
            self.total_exact_leaf_terms_ingressed,
            self.total_exact_leaf_exponent_cells_ingressed,
            self.total_exact_leaf_bytes_ingressed,
        )
    }

    pub(super) fn physical_delta_count(&self) -> usize {
        self.deltas.len()
    }

    pub(super) fn index_count(&self) -> usize {
        self.index_count
    }

    pub(super) fn context_fingerprint(&self) -> &Arc<String> {
        &self.context_fingerprint
    }

    pub(super) fn owner(&self) -> &DagOwner {
        &self.owner
    }

    pub(super) fn owns_context(&self, context: &IndexedCoefficientContext) -> bool {
        context.owns_fingerprint(&self.context_fingerprint)
    }

    pub(super) fn owns_reference(&self, value: &CoeffRef) -> bool {
        self.owner.belongs_to(&value.owner)
    }

    pub(super) fn node(&self, id: CoeffNodeId) -> Result<&CoeffNode, ModularGuideError> {
        if self.node_incarnations.get(id.as_usize()).copied() != Some(id.incarnation()) {
            return Err(ModularGuideError::StaleDagReference {
                resource: "coefficient node",
                ordinal: id.ordinal(),
            });
        }
        self.nodes
            .get(id.as_usize())
            .ok_or(ModularGuideError::Invariant {
                detail: "coefficient reference names an absent DAG node",
            })
    }

    pub(super) fn exact_leaf(&self, id: ExactLeafId) -> Result<&ExactLeaf, ModularGuideError> {
        self.exact_leaves
            .get(id.as_usize())
            .ok_or(ModularGuideError::Invariant {
                detail: "coefficient DAG node names an absent exact leaf",
            })
    }

    pub(super) fn delta(&self, id: PhysicalDeltaId) -> Result<&[i64], ModularGuideError> {
        if self.delta_incarnations.get(id.as_usize()).copied() != Some(id.incarnation()) {
            return Err(ModularGuideError::StaleDagReference {
                resource: "physical translation",
                ordinal: id.ordinal(),
            });
        }
        self.deltas
            .get(id.as_usize())
            .map(|values| values.as_slice())
            .ok_or(ModularGuideError::Invariant {
                detail: "coefficient reference names an absent physical translation",
            })
    }

    pub(super) fn raw(&self, value: &CoeffRef) -> Result<RawCoeffRef, ModularGuideError> {
        self.require(value)
    }

    fn require(&self, value: &CoeffRef) -> Result<RawCoeffRef, ModularGuideError> {
        if !self.owns_reference(value) {
            return Err(ModularGuideError::WrongDagOwner);
        }
        self.node(value.raw.node)?;
        self.delta(value.raw.translation)?;
        Ok(value.raw)
    }

    fn wrap(&self, raw: RawCoeffRef) -> CoeffRef {
        CoeffRef {
            owner: self.owner.clone(),
            raw,
        }
    }

    fn zero_delta_id(&self) -> PhysicalDeltaId {
        PhysicalDeltaId::try_new(0, 0).expect("zero physical-delta ID is fixed")
    }

    fn is_zero_raw(&self, value: RawCoeffRef) -> bool {
        value.node.ordinal() == 0
    }

    fn is_one_raw(&self, value: RawCoeffRef) -> bool {
        value.node.ordinal() == 1
    }

    fn try_intern_node(&mut self, node: CoeffNode) -> Result<CoeffRef, ModularGuideError> {
        if let Some(&id) = self.node_lookup.get(&node) {
            return Ok(self.wrap(RawCoeffRef {
                node: id,
                translation: self.zero_delta_id(),
            }));
        }
        let requested = checked_add(DAG_NODES, self.nodes.len(), 1)?;
        check_limit(DAG_NODES, requested, self.limits.max_nodes)?;
        let requested_total = checked_add(TOTAL_DAG_NODES, self.total_nodes_created, 1)?;
        check_limit(
            TOTAL_DAG_NODES,
            requested_total,
            self.limits.max_total_nodes_created,
        )?;
        let incarnation = self.try_issue_node_incarnation()?;
        let id = CoeffNodeId::try_new(self.nodes.len(), incarnation).ok_or(
            ModularGuideError::IdentifierNotRepresentable {
                resource: "modular coefficient DAG node identifier",
                value: self.nodes.len(),
            },
        )?;
        reserve_vec(&mut self.nodes, 1, DAG_NODES)?;
        reserve_vec(&mut self.node_incarnations, 1, DAG_NODES)?;
        reserve_map(&mut self.node_lookup, 1, DAG_NODES)?;
        self.node_lookup.insert(node.clone(), id);
        self.nodes.push(node);
        self.node_incarnations.push(incarnation);
        self.total_nodes_created = requested_total;
        Ok(self.wrap(RawCoeffRef {
            node: id,
            translation: self.zero_delta_id(),
        }))
    }

    fn is_effective_negated_operand(
        &self,
        value: RawCoeffRef,
        expected: RawCoeffRef,
    ) -> Result<bool, ModularGuideError> {
        let CoeffNode::Neg(child) = self.node(value.node)?.clone() else {
            return Ok(false);
        };
        if child.node != expected.node {
            return Ok(false);
        }
        let child_delta = self.delta(child.translation)?;
        let outer_delta = self.delta(value.translation)?;
        let expected_delta = self.delta(expected.translation)?;
        Ok(child_delta
            .iter()
            .zip(outer_delta)
            .zip(expected_delta)
            .all(|((&child, &outer), &expected)| child.checked_add(outer) == Some(expected)))
    }

    fn try_compose_raw(
        &mut self,
        value: RawCoeffRef,
        outer: PhysicalDeltaId,
    ) -> Result<RawCoeffRef, ModularGuideError> {
        if outer == self.zero_delta_id() {
            return Ok(value);
        }
        let mut outer_values = Vec::new();
        reserve_vec(&mut outer_values, self.index_count, PHYSICAL_DELTA_CELLS)?;
        outer_values.extend_from_slice(self.delta(outer)?);
        let translation = self.try_intern_delta_sum(value.translation, &outer_values)?;
        Ok(RawCoeffRef {
            node: value.node,
            translation,
        })
    }

    fn try_intern_delta_sum(
        &mut self,
        left: PhysicalDeltaId,
        right: &[i64],
    ) -> Result<PhysicalDeltaId, ModularGuideError> {
        self.require_translation_arity(right)?;
        let requested_total_coordinate_operations = checked_add(
            TOTAL_PHYSICAL_DELTA_COORDINATE_OPERATIONS,
            self.total_physical_delta_coordinate_operations,
            self.index_count,
        )?;
        check_limit(
            TOTAL_PHYSICAL_DELTA_COORDINATE_OPERATIONS,
            requested_total_coordinate_operations,
            self.limits.max_total_physical_delta_coordinate_operations,
        )?;
        // This charge is monotone even if coordinate arithmetic or a later
        // allocation fails: the attempted composition work was consumed.
        self.total_physical_delta_coordinate_operations = requested_total_coordinate_operations;
        let left = self.delta(left)?;
        let mut result = Vec::new();
        reserve_vec(&mut result, self.index_count, PHYSICAL_DELTA_CELLS)?;
        for (&left, &right) in left.iter().zip(right) {
            let value =
                left.checked_add(right)
                    .ok_or(ModularGuideError::ResourceCountOverflow {
                        resource: "modular physical translation coordinate",
                    })?;
            if value.unsigned_abs() > self.limits.max_absolute_physical_translation {
                return Err(ModularGuideError::ResourceLimit {
                    resource: "modular absolute physical translation",
                    requested: usize::try_from(value.unsigned_abs()).unwrap_or(usize::MAX),
                    limit: usize::try_from(self.limits.max_absolute_physical_translation)
                        .unwrap_or(usize::MAX),
                });
            }
            result.push(value);
        }
        if let Some(&id) = self.delta_lookup.get(&result) {
            return Ok(id);
        }
        let requested_deltas = checked_add(PHYSICAL_DELTAS, self.deltas.len(), 1)?;
        check_limit(
            PHYSICAL_DELTAS,
            requested_deltas,
            self.limits.max_physical_deltas,
        )?;
        let requested_total_deltas =
            checked_add(TOTAL_PHYSICAL_DELTAS, self.total_physical_deltas_created, 1)?;
        check_limit(
            TOTAL_PHYSICAL_DELTAS,
            requested_total_deltas,
            self.limits.max_total_physical_deltas_created,
        )?;
        let requested_cells = checked_add(
            PHYSICAL_DELTA_CELLS,
            self.delta_coordinate_cells,
            self.index_count,
        )?;
        check_limit(
            PHYSICAL_DELTA_CELLS,
            requested_cells,
            self.limits.max_physical_delta_coordinate_cells,
        )?;
        let incarnation = self.try_issue_delta_incarnation()?;
        let id = PhysicalDeltaId::try_new(self.deltas.len(), incarnation).ok_or(
            ModularGuideError::IdentifierNotRepresentable {
                resource: "modular physical-delta identifier",
                value: self.deltas.len(),
            },
        )?;
        reserve_vec(&mut self.deltas, 1, PHYSICAL_DELTAS)?;
        reserve_vec(&mut self.delta_incarnations, 1, PHYSICAL_DELTAS)?;
        reserve_map(&mut self.delta_lookup, 1, PHYSICAL_DELTAS)?;
        let result = Arc::new(result);
        self.delta_lookup.insert(Arc::clone(&result), id);
        self.deltas.push(result);
        self.delta_incarnations.push(incarnation);
        self.total_physical_deltas_created = requested_total_deltas;
        self.delta_coordinate_cells = requested_cells;
        Ok(id)
    }

    fn require_translation_arity(&self, values: &[i64]) -> Result<(), ModularGuideError> {
        if values.len() == self.index_count {
            Ok(())
        } else {
            Err(ModularGuideError::WrongTranslationArity {
                expected: self.index_count,
                actual: values.len(),
            })
        }
    }

    pub(super) fn checkpoint(&self) -> ArenaCheckpoint {
        ArenaCheckpoint {
            owner: self.owner.clone(),
            node_count: self.nodes.len(),
            node_boundary: self
                .live_node_id(self.nodes.len() - 1)
                .expect("a modular DAG always retains zero and one"),
            exact_leaf_count: self.exact_leaves.len(),
            exact_leaf_terms: self.exact_leaf_terms,
            exact_leaf_exponent_cells: self.exact_leaf_exponent_cells,
            exact_leaf_retained_bytes: self.exact_leaf_retained_bytes,
            delta_count: self.deltas.len(),
            delta_boundary: self
                .live_delta_id(self.deltas.len() - 1)
                .expect("a modular DAG always retains the zero translation"),
            delta_coordinate_cells: self.delta_coordinate_cells,
        }
    }

    pub(super) fn try_rollback(
        &mut self,
        checkpoint: ArenaCheckpoint,
    ) -> Result<(), ModularGuideError> {
        if !checkpoint.owner.belongs_to(&self.owner) {
            return Err(ModularGuideError::WrongDagOwner);
        }
        if checkpoint.node_count < 2
            || checkpoint.node_count > self.nodes.len()
            || checkpoint.exact_leaf_count > self.exact_leaves.len()
            || checkpoint.delta_count == 0
            || checkpoint.delta_count > self.deltas.len()
        {
            return Err(ModularGuideError::InvalidArenaCheckpoint {
                detail: "retained counts are outside the current arena prefix",
            });
        }
        if self.live_node_id(checkpoint.node_count - 1) != Some(checkpoint.node_boundary) {
            return Err(ModularGuideError::InvalidArenaCheckpoint {
                detail: "coefficient-node prefix incarnation has changed",
            });
        }
        if self.live_delta_id(checkpoint.delta_count - 1) != Some(checkpoint.delta_boundary) {
            return Err(ModularGuideError::InvalidArenaCheckpoint {
                detail: "physical-delta prefix incarnation has changed",
            });
        }
        if checkpoint.exact_leaf_terms > self.exact_leaf_terms
            || checkpoint.exact_leaf_exponent_cells > self.exact_leaf_exponent_cells
            || checkpoint.exact_leaf_retained_bytes > self.exact_leaf_retained_bytes
            || checkpoint.delta_coordinate_cells > self.delta_coordinate_cells
        {
            return Err(ModularGuideError::InvalidArenaCheckpoint {
                detail: "retained payload census is not a current-prefix lower bound",
            });
        }
        while self.nodes.len() > checkpoint.node_count {
            let node = self.nodes.pop().expect("checkpoint bounds node rollback");
            self.node_lookup.remove(&node);
        }
        self.node_incarnations.truncate(checkpoint.node_count);
        while self.exact_leaves.len() > checkpoint.exact_leaf_count {
            let leaf = self
                .exact_leaves
                .pop()
                .expect("checkpoint bounds exact-leaf rollback");
            self.exact_leaf_lookup.remove(&ExactLeafKey(leaf));
        }
        self.exact_leaf_terms = checkpoint.exact_leaf_terms;
        self.exact_leaf_exponent_cells = checkpoint.exact_leaf_exponent_cells;
        self.exact_leaf_retained_bytes = checkpoint.exact_leaf_retained_bytes;
        while self.deltas.len() > checkpoint.delta_count {
            let delta = self.deltas.pop().expect("checkpoint bounds delta rollback");
            self.delta_lookup.remove(&delta);
        }
        self.delta_incarnations.truncate(checkpoint.delta_count);
        self.delta_coordinate_cells = checkpoint.delta_coordinate_cells;
        Ok(())
    }

    fn live_node_id(&self, ordinal: usize) -> Option<CoeffNodeId> {
        CoeffNodeId::try_new(ordinal, *self.node_incarnations.get(ordinal)?)
    }

    fn live_delta_id(&self, ordinal: usize) -> Option<PhysicalDeltaId> {
        PhysicalDeltaId::try_new(ordinal, *self.delta_incarnations.get(ordinal)?)
    }

    fn try_issue_node_incarnation(&mut self) -> Result<u32, ModularGuideError> {
        let incarnation = u32::try_from(self.next_node_incarnation).map_err(|_| {
            ModularGuideError::ResourceCountOverflow {
                resource: "modular coefficient node incarnation",
            }
        })?;
        self.next_node_incarnation = self.next_node_incarnation.checked_add(1).ok_or(
            ModularGuideError::ResourceCountOverflow {
                resource: "modular coefficient node incarnation",
            },
        )?;
        Ok(incarnation)
    }

    fn try_issue_delta_incarnation(&mut self) -> Result<u32, ModularGuideError> {
        let incarnation = u32::try_from(self.next_delta_incarnation).map_err(|_| {
            ModularGuideError::ResourceCountOverflow {
                resource: "modular physical-translation incarnation",
            }
        })?;
        self.next_delta_incarnation = self.next_delta_incarnation.checked_add(1).ok_or(
            ModularGuideError::ResourceCountOverflow {
                resource: "modular physical-translation incarnation",
            },
        )?;
        Ok(incarnation)
    }
}

#[derive(Debug)]
pub(super) struct ArenaCheckpoint {
    owner: DagOwner,
    node_count: usize,
    node_boundary: CoeffNodeId,
    exact_leaf_count: usize,
    exact_leaf_terms: usize,
    exact_leaf_exponent_cells: usize,
    exact_leaf_retained_bytes: usize,
    delta_count: usize,
    delta_boundary: PhysicalDeltaId,
    delta_coordinate_cells: usize,
}

fn canonical_pair(left: RawCoeffRef, right: RawCoeffRef) -> (RawCoeffRef, RawCoeffRef) {
    if left <= right {
        (left, right)
    } else {
        (right, left)
    }
}

fn exact_leaf_weight(
    coefficient: &IndexedCoefficient,
) -> Result<CoefficientPayloadWeight, ModularGuideError> {
    let payload = try_coefficient_payload_weight(coefficient)?;
    let bytes = [
        payload.bytes,
        size_of::<ExactLeaf>(),
        size_of::<ExactLeafKey>(),
        size_of::<ExactLeafId>(),
    ]
    .into_iter()
    .try_fold(0usize, |total, value| {
        checked_add(EXACT_LEAF_BYTES, total, value)
    })?;
    Ok(CoefficientPayloadWeight {
        terms: payload.terms,
        exponent_cells: payload.exponent_cells,
        bytes,
    })
}
