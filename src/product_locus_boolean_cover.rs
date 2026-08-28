//! Bounded Boolean expansion of product loci on one generated residual leaf.
//!
//! Coverage V4 exposes product structure in either of two exact forms: a
//! checked concrete product with its retained factor witness, or already-
//! disjoint canonical factor predicates selected before product
//! materialization.  This module consumes both forms directly and never asks
//! Symbolica to factor a polynomial after the fact.  Witnessed product
//! equalities become positive CNF clauses, product nonzero predicates become
//! factor-wise nonzero facts, and factor-only coverage leaves remain ordinary
//! singleton clauses and path facts.  A deterministic Shannon/DPLL transcript
//! partitions each source leaf into disjoint Boolean paths.  Affine recognition
//! and simultaneous integer row solving intentionally belong to the next proof
//! layer.

use std::fmt;
use std::mem::{align_of, size_of};
use std::sync::Arc;

use crate::{
    CoordinateEqualityLocusCertificate, CoordinateEqualityLocusError,
    GeneratedSectorLiveLeafQueueCertificate, GeneratedSectorLiveLeafQueueError, IntegralFamily,
    ParametricCoefficientContext, ParametricCoefficientError, SectorFoundationError, SectorMask,
    SectorOrthantSide, SymbolicPolynomialPredicateKind, SymbolicSectorCaseError,
    SymbolicSectorCaseId,
};

/// Stable schema for construction-provenance product expansion and disjoint
/// Boolean residual branching.
pub const RESIDUAL_PRODUCT_LOCUS_BOOLEAN_COVER_V1_SCHEMA: &str =
    "rustred-residual-product-locus-boolean-cover-v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResidualProductLocusBooleanCoverLimits {
    pub max_family_fingerprint_bytes: usize,
    pub max_context_fingerprint_bytes: usize,
    pub max_source_identity_bytes_referenced: usize,
    pub max_sector_arity: usize,
    pub max_source_predicates: usize,
    pub max_base_unit_checks: usize,
    pub max_structural_locus_lookup_comparisons: usize,
    pub max_product_witness_lookup_comparisons: usize,
    pub max_atoms: usize,
    pub max_atom_lookup_comparisons: usize,
    pub max_referenced_locus_terms: usize,
    pub max_root_clauses: usize,
    pub max_root_literals: usize,
    pub max_root_nonzero_facts: usize,
    pub max_factor_references_expanded: usize,
    pub max_subsumption_comparisons: usize,
    pub max_coordinate_recognition_checks: usize,
    pub max_coordinate_contradiction_comparisons: usize,
    pub max_dpll_nodes: usize,
    pub max_dpll_branches: usize,
    pub max_dpll_depth: usize,
    pub max_frontier_states: usize,
    pub max_path_facts: usize,
    pub max_propagations: usize,
    pub max_retained_propagation_entries: usize,
    pub max_clauses_per_state: usize,
    pub max_literals_per_state: usize,
    pub max_retained_node_clauses: usize,
    pub max_retained_node_literals: usize,
    /// Aggregate atom ordinals owned by exhausted-clause empty witnesses.
    pub max_retained_empty_reason_ordinals: usize,
    pub max_retained_state_bytes: usize,
    pub max_peak_frontier_bytes: usize,
    pub max_peak_simplification_scratch_bytes: usize,
    pub max_terminals: usize,
    /// Fresh, no-allocation comparison phase for cover-owned payload only.
    /// Independently allocated nested queue/extraction proofs are rejected at
    /// the explicit V1 persistence boundary rather than compared unchecked.
    pub max_payload_comparison_units: usize,
    pub max_payload_comparison_bytes: usize,
}

impl Default for ResidualProductLocusBooleanCoverLimits {
    fn default() -> Self {
        Self {
            max_family_fingerprint_bytes: 1024 * 1024,
            max_context_fingerprint_bytes: 1024 * 1024,
            max_source_identity_bytes_referenced: 1024 * 1024 * 1024,
            max_sector_arity: 16_000_000,
            max_source_predicates: 4_000_000,
            max_base_unit_checks: 4_000_000,
            max_structural_locus_lookup_comparisons: 256_000_000,
            max_product_witness_lookup_comparisons: 256_000_000,
            max_atoms: 16_000_000,
            max_atom_lookup_comparisons: 256_000_000,
            max_referenced_locus_terms: 256_000_000,
            max_root_clauses: 16_000_000,
            max_root_literals: 64_000_000,
            max_root_nonzero_facts: 64_000_000,
            max_factor_references_expanded: 64_000_000,
            max_subsumption_comparisons: 256_000_000,
            max_coordinate_recognition_checks: 16_000_000,
            max_coordinate_contradiction_comparisons: 256_000_000,
            max_dpll_nodes: 16_000_000,
            max_dpll_branches: 16_000_000,
            max_dpll_depth: 16_000_000,
            max_frontier_states: 16_000_000,
            max_path_facts: 256_000_000,
            max_propagations: 256_000_000,
            max_retained_propagation_entries: 256_000_000,
            max_clauses_per_state: 16_000_000,
            max_literals_per_state: 64_000_000,
            max_retained_node_clauses: 256_000_000,
            max_retained_node_literals: 1024 * 1024 * 1024,
            max_retained_empty_reason_ordinals: 1024 * 1024 * 1024,
            max_retained_state_bytes: 8 * 1024 * 1024 * 1024,
            max_peak_frontier_bytes: 4 * 1024 * 1024 * 1024,
            max_peak_simplification_scratch_bytes: 4 * 1024 * 1024 * 1024,
            max_terminals: 16_000_000,
            max_payload_comparison_units: 64_000_000_000,
            max_payload_comparison_bytes: 64 * 1024 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ResidualProductLocusBooleanCoverStats {
    source_predicates: usize,
    base_unit_checks: usize,
    structural_locus_lookup_comparisons: usize,
    product_witness_lookup_comparisons: usize,
    atoms: usize,
    atom_lookup_comparisons: usize,
    referenced_locus_terms: usize,
    root_clauses: usize,
    root_literals: usize,
    root_nonzero_facts: usize,
    product_equalities_expanded: usize,
    product_nonzero_predicates_expanded: usize,
    factor_references_expanded: usize,
    duplicate_literals_removed: usize,
    duplicate_or_subsumed_clauses_removed: usize,
    subsumption_comparisons: usize,
    coordinate_recognition_checks: usize,
    coordinate_contradiction_comparisons: usize,
    dpll_nodes: usize,
    dpll_branches: usize,
    propagations: usize,
    retained_propagation_entries: usize,
    path_facts: usize,
    retained_node_clauses: usize,
    retained_node_literals: usize,
    retained_empty_reason_ordinals: usize,
    retained_state_bytes: usize,
    peak_frontier_states: usize,
    peak_frontier_bytes: usize,
    peak_simplification_scratch_bytes: usize,
    ready_terminals: usize,
    proved_empty_terminals: usize,
    payload_comparison_units: usize,
    payload_comparison_bytes: usize,
}

macro_rules! stats_getters {
    ($($field:ident),+ $(,)?) => {$(
        pub const fn $field(self) -> usize { self.$field }
    )+};
}

impl ResidualProductLocusBooleanCoverStats {
    stats_getters!(
        source_predicates,
        base_unit_checks,
        structural_locus_lookup_comparisons,
        product_witness_lookup_comparisons,
        atoms,
        atom_lookup_comparisons,
        referenced_locus_terms,
        root_clauses,
        root_literals,
        root_nonzero_facts,
        product_equalities_expanded,
        product_nonzero_predicates_expanded,
        factor_references_expanded,
        duplicate_literals_removed,
        duplicate_or_subsumed_clauses_removed,
        subsumption_comparisons,
        coordinate_recognition_checks,
        coordinate_contradiction_comparisons,
        dpll_nodes,
        dpll_branches,
        propagations,
        retained_propagation_entries,
        path_facts,
        retained_node_clauses,
        retained_node_literals,
        retained_empty_reason_ordinals,
        retained_state_bytes,
        peak_frontier_states,
        peak_frontier_bytes,
        peak_simplification_scratch_bytes,
        ready_terminals,
        proved_empty_terminals,
        payload_comparison_units,
        payload_comparison_bytes,
    );
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualProductLocusBooleanPolarity {
    EqualZero,
    NonZero,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResidualProductLocusBooleanDecision {
    atom_locus_ordinal: usize,
    polarity: ResidualProductLocusBooleanPolarity,
}

impl ResidualProductLocusBooleanDecision {
    pub const fn atom_locus_ordinal(self) -> usize {
        self.atom_locus_ordinal
    }
    pub const fn polarity(self) -> ResidualProductLocusBooleanPolarity {
        self.polarity
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResidualProductLocusBooleanEmptyReason {
    OppositeFacts {
        atom_locus_ordinal: usize,
    },
    ExhaustedClause {
        atom_locus_ordinals: Box<[usize]>,
    },
    OrthantViolation {
        atom_locus_ordinal: usize,
        index: usize,
        value: i64,
        side: SectorOrthantSide,
    },
    ConflictingCoordinateEqualities {
        first_atom_locus_ordinal: usize,
        second_atom_locus_ordinal: usize,
        index: usize,
        first_value: i64,
        second_value: i64,
    },
    CoordinateEqualityNonzero {
        equality_atom_locus_ordinal: usize,
        nonzero_atom_locus_ordinal: usize,
        index: usize,
        value: i64,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResidualProductLocusBooleanNodeOutcome {
    /// The path is a satisfiable Boolean component. Its equal-zero atoms are
    /// the rows to recognize, while its nonzero atoms are retained guards.
    ReadyForAffineRecognition,
    ProvedEmpty(ResidualProductLocusBooleanEmptyReason),
    Branched {
        atom_locus_ordinal: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResidualProductLocusBooleanNode {
    ordinal: usize,
    parent_ordinal: Option<usize>,
    decision_from_parent: Option<ResidualProductLocusBooleanDecision>,
    depth: usize,
    equal_zero_atoms: Box<[usize]>,
    nonzero_atoms: Box<[usize]>,
    propagated_zero_atoms: Box<[usize]>,
    remaining_clauses: Box<[Box<[usize]>]>,
    outcome: ResidualProductLocusBooleanNodeOutcome,
}

impl ResidualProductLocusBooleanNode {
    pub const fn ordinal(&self) -> usize {
        self.ordinal
    }
    pub const fn parent_ordinal(&self) -> Option<usize> {
        self.parent_ordinal
    }
    pub const fn decision_from_parent(&self) -> Option<ResidualProductLocusBooleanDecision> {
        self.decision_from_parent
    }
    pub const fn depth(&self) -> usize {
        self.depth
    }
    pub fn equal_zero_atoms(&self) -> &[usize] {
        &self.equal_zero_atoms
    }
    pub fn nonzero_atoms(&self) -> &[usize] {
        &self.nonzero_atoms
    }
    /// Deterministic unit-propagation order at this node. Decisions inherited
    /// from ancestors remain in `equal_zero_atoms`; this slice records only
    /// implications derived while simplifying this node.
    pub fn propagated_zero_atoms(&self) -> &[usize] {
        &self.propagated_zero_atoms
    }
    pub fn remaining_clauses(&self) -> &[Box<[usize]>] {
        &self.remaining_clauses
    }
    pub const fn outcome(&self) -> &ResidualProductLocusBooleanNodeOutcome {
        &self.outcome
    }
    pub fn is_terminal(&self) -> bool {
        !matches!(
            self.outcome,
            ResidualProductLocusBooleanNodeOutcome::Branched { .. }
        )
    }
}

#[derive(Clone, Debug)]
pub struct ResidualProductLocusBooleanCoverCertificate {
    schema: &'static str,
    family_fingerprint: Arc<str>,
    context_fingerprint: Arc<str>,
    sector: SectorMask,
    source_queue: Arc<GeneratedSectorLiveLeafQueueCertificate>,
    source_work_item_ordinal: usize,
    source_case: SymbolicSectorCaseId,
    source_extraction: Arc<CoordinateEqualityLocusCertificate>,
    atoms: Box<[usize]>,
    root_clauses: Box<[Box<[usize]>]>,
    root_nonzero_atoms: Box<[usize]>,
    nodes: Box<[ResidualProductLocusBooleanNode]>,
    limits: ResidualProductLocusBooleanCoverLimits,
    stats: ResidualProductLocusBooleanCoverStats,
}

impl ResidualProductLocusBooleanCoverCertificate {
    pub const fn schema(&self) -> &'static str {
        self.schema
    }
    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }
    pub fn context_fingerprint(&self) -> &str {
        &self.context_fingerprint
    }
    pub const fn sector(&self) -> &SectorMask {
        &self.sector
    }
    pub const fn source_queue(&self) -> &Arc<GeneratedSectorLiveLeafQueueCertificate> {
        &self.source_queue
    }
    pub const fn source_work_item_ordinal(&self) -> usize {
        self.source_work_item_ordinal
    }
    pub const fn source_case(&self) -> SymbolicSectorCaseId {
        self.source_case
    }
    pub const fn source_extraction(&self) -> &Arc<CoordinateEqualityLocusCertificate> {
        &self.source_extraction
    }
    pub fn atom_locus_ordinals(&self) -> &[usize] {
        &self.atoms
    }
    pub fn root_clauses(&self) -> &[Box<[usize]>] {
        &self.root_clauses
    }
    pub fn root_nonzero_atoms(&self) -> &[usize] {
        &self.root_nonzero_atoms
    }
    pub fn nodes(&self) -> &[ResidualProductLocusBooleanNode] {
        &self.nodes
    }
    pub fn terminals(&self) -> impl Iterator<Item = &ResidualProductLocusBooleanNode> {
        self.nodes.iter().filter(|node| node.is_terminal())
    }
    pub const fn limits(&self) -> ResidualProductLocusBooleanCoverLimits {
        self.limits
    }
    pub const fn stats(&self) -> ResidualProductLocusBooleanCoverStats {
        self.stats
    }

    pub fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), ResidualProductLocusBooleanCoverError> {
        self.replay_with_queue(family, context, self.source_queue.clone())
    }

    /// V1 replay accepts the exact retained queue allocation.  A future
    /// persistence schema may add checked comparators to the recursively
    /// retained queue/discovery/coverage proof graph; until then a distinct
    /// nested allocation fails with a typed boundary instead of entering an
    /// unchecked deep equality operation.
    pub fn replay_with_queue(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        queue: Arc<GeneratedSectorLiveLeafQueueCertificate>,
    ) -> Result<(), ResidualProductLocusBooleanCoverError> {
        validate_scope(
            self.schema,
            &self.family_fingerprint,
            &self.context_fingerprint,
            family,
            context,
            self.limits,
        )?;
        if !Arc::ptr_eq(&self.source_queue, &queue) {
            return Err(
                ResidualProductLocusBooleanCoverError::IndependentNestedPayloadComparisonUnsupported {
                    component: "source queue",
                },
            );
        }
        queue.replay(family, context)?;
        let rebuilt = compile_replayed(
            family,
            context,
            queue,
            self.source_work_item_ordinal,
            self.limits,
        )?;
        if self.payload_eq_checked(&rebuilt)? {
            Ok(())
        } else {
            Err(ResidualProductLocusBooleanCoverError::ReplayMismatch)
        }
    }

    /// Find the unique nonempty Boolean terminal containing an exact integer
    /// point. `None` means the point is outside this source leaf.
    pub fn ready_terminal_for_indices(
        &self,
        context: &ParametricCoefficientContext,
        indices: &[i64],
    ) -> Result<Option<&ResidualProductLocusBooleanNode>, ResidualProductLocusBooleanCoverError>
    {
        if self.context_fingerprint.as_ref() != context.fingerprint() {
            return Err(ResidualProductLocusBooleanCoverError::WrongContext);
        }
        if !self.sector.contains_indices(indices)? {
            return Ok(None);
        }
        let coverage = self.source_queue.discovery().coverage();
        let arithmetic = self
            .source_queue
            .discovery()
            .limits()
            .coverage
            .generated_when_bad
            .when_bad
            .arithmetic;
        let mut found = None;
        for node in self.terminals() {
            if !matches!(
                node.outcome,
                ResidualProductLocusBooleanNodeOutcome::ReadyForAffineRecognition
            ) {
                continue;
            }
            let mut zero_match = true;
            for &ordinal in &node.equal_zero_atoms {
                let polynomial = coverage.structural_locus(ordinal).ok_or(
                    ResidualProductLocusBooleanCoverError::StructuralLocusOrdinalOutOfRange {
                        ordinal,
                    },
                )?;
                zero_match &= context
                    .specialize_polynomial(polynomial, indices, arithmetic)?
                    .is_zero();
            }
            let mut nonzero_match = true;
            for &ordinal in &node.nonzero_atoms {
                let polynomial = coverage.structural_locus(ordinal).ok_or(
                    ResidualProductLocusBooleanCoverError::StructuralLocusOrdinalOutOfRange {
                        ordinal,
                    },
                )?;
                nonzero_match &= !context
                    .specialize_polynomial(polynomial, indices, arithmetic)?
                    .is_zero();
            }
            if zero_match && nonzero_match {
                if found.is_some() {
                    return Err(ResidualProductLocusBooleanCoverError::TerminalEvaluationMismatch);
                }
                found = Some(node);
            }
        }
        Ok(found)
    }

    /// Fallible exact equality for the cover-owned V1 payload.
    ///
    /// The comparison first charges a fresh, no-allocation phase from the
    /// actual retained shape of both operands. Top-level allocation identity
    /// is irrelevant. Nested queue and extraction proofs use allocation
    /// identity as the current V1 authentication boundary: independently
    /// allocated nested proofs are rejected explicitly and are never handed
    /// to their legacy unchecked recursive comparators.
    pub(crate) fn payload_eq_checked(
        &self,
        other: &Self,
    ) -> Result<bool, ResidualProductLocusBooleanCoverError> {
        if std::ptr::eq(self, other) {
            return Ok(true);
        }
        preflight_cover_owned_payload_comparison(self, other)?;

        let local_equal = self.schema == other.schema
            && self.family_fingerprint == other.family_fingerprint
            && self.context_fingerprint == other.context_fingerprint
            && self.sector == other.sector
            && self.source_work_item_ordinal == other.source_work_item_ordinal
            && self.source_case == other.source_case
            && self.atoms == other.atoms
            && self.root_clauses == other.root_clauses
            && self.root_nonzero_atoms == other.root_nonzero_atoms
            && self.nodes == other.nodes
            && self.limits == other.limits
            && self.stats == other.stats;
        if !local_equal {
            return Ok(false);
        }

        if !Arc::ptr_eq(&self.source_queue, &other.source_queue) {
            return Err(
                ResidualProductLocusBooleanCoverError::IndependentNestedPayloadComparisonUnsupported {
                    component: "source queue",
                },
            );
        }
        if !Arc::ptr_eq(&self.source_extraction, &other.source_extraction) {
            // Cheap scalar discrimination preserves ordinary mismatch
            // behavior without attempting the extraction's deep partition
            // equality. Equal scalar bindings still require the future
            // checked nested comparator.
            if self.source_extraction.schema() != other.source_extraction.schema()
                || self.source_extraction.source_case() != other.source_extraction.source_case()
            {
                return Ok(false);
            }
            return Err(
                ResidualProductLocusBooleanCoverError::IndependentNestedPayloadComparisonUnsupported {
                    component: "source extraction",
                },
            );
        }
        Ok(true)
    }

    /// Recompute the cover-owned retained logical envelope from the sealed
    /// raw V1 shape. This does not expose source identity or payload fields to
    /// source-neutral callers.
    pub(crate) fn recompute_retained_owned_logical_bytes_upper_bound(
        &self,
    ) -> Result<usize, ResidualProductLocusBooleanCoverError> {
        cover_retained_owned_logical_bytes_upper_bound(self)
    }

    /// Recompute the cover-local checked-comparison census from the sealed V1
    /// shape without exposing any source payload.
    pub(crate) fn recompute_payload_comparison_census(
        &self,
    ) -> Result<(usize, usize), ResidualProductLocusBooleanCoverError> {
        let census = cover_owned_payload_comparison_census(self, self.limits)?;
        Ok((census.units, census.bytes))
    }

    /// Legacy fail-closed Boolean surface for callers which cannot propagate
    /// comparison-budget or V1 nested-persistence errors.
    pub(crate) fn payload_eq(&self, other: &Self) -> bool {
        self.payload_eq_checked(other).unwrap_or(false)
    }
}

#[cfg(test)]
impl ResidualProductLocusBooleanCoverCertificate {
    pub(crate) fn tamper_first_root_literal_for_test(&mut self) {
        if let Some(first) = self
            .root_clauses
            .first_mut()
            .and_then(|clause| clause.first_mut())
        {
            *first = usize::MAX;
        }
    }

    pub(crate) fn tamper_first_node_fact_for_test(&mut self) {
        if let Some(first) = self.nodes.iter_mut().find_map(|node| {
            node.equal_zero_atoms
                .first_mut()
                .or_else(|| node.nonzero_atoms.first_mut())
        }) {
            *first = usize::MAX;
        }
    }

    pub(crate) fn tamper_stats_for_test(&mut self) {
        self.stats.dpll_nodes = self.stats.dpll_nodes.saturating_add(1);
    }

    pub(crate) fn tamper_payload_comparison_stats_for_test(&mut self) {
        self.stats.payload_comparison_units = self.stats.payload_comparison_units.saturating_add(1);
    }

    pub(crate) fn tamper_source_work_item_for_test(&mut self) {
        self.source_work_item_ordinal = usize::MAX;
    }

    pub(crate) fn tamper_source_extraction_for_test(
        &mut self,
        extraction: Arc<CoordinateEqualityLocusCertificate>,
    ) {
        self.source_extraction = extraction;
    }
}

pub struct ResidualProductLocusBooleanCoverCompiler;

impl ResidualProductLocusBooleanCoverCompiler {
    pub fn compile(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        queue: Arc<GeneratedSectorLiveLeafQueueCertificate>,
        source_work_item_ordinal: usize,
        limits: ResidualProductLocusBooleanCoverLimits,
    ) -> Result<ResidualProductLocusBooleanCoverCertificate, ResidualProductLocusBooleanCoverError>
    {
        queue.replay(family, context)?;
        let certificate =
            compile_replayed(family, context, queue, source_work_item_ordinal, limits)?;
        let item = certificate
            .source_queue
            .work_items()
            .get(source_work_item_ordinal)
            .ok_or(ResidualProductLocusBooleanCoverError::WorkItemNotFound {
                ordinal: source_work_item_ordinal,
            })?;
        if !Arc::ptr_eq(&certificate.source_extraction, item.extraction_arc()) {
            return Err(ResidualProductLocusBooleanCoverError::FreshExtractionAllocationMismatch);
        }
        Ok(certificate)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResidualProductLocusBooleanCoverError {
    SchemaMismatch,
    WrongFamily,
    WrongContext,
    SourceQueueMismatch,
    IndependentNestedPayloadComparisonUnsupported {
        component: &'static str,
    },
    FreshExtractionAllocationMismatch,
    WorkItemNotFound {
        ordinal: usize,
    },
    SourceCaseNotFound {
        case: SymbolicSectorCaseId,
    },
    SourceBindingMismatch,
    StructuralLocusNotFound {
        predicate_ordinal: usize,
    },
    StructuralLocusOrdinalOutOfRange {
        ordinal: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    AllocationFailure {
        resource: &'static str,
    },
    ReplayMismatch,
    TerminalEvaluationMismatch,
    Queue(GeneratedSectorLiveLeafQueueError),
    Coordinate(CoordinateEqualityLocusError),
    Coefficient(ParametricCoefficientError),
    Sector(SectorFoundationError),
    SymbolicCase(SymbolicSectorCaseError),
}

impl fmt::Display for ResidualProductLocusBooleanCoverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaMismatch => f.write_str("product-locus Boolean cover schema mismatch"),
            Self::WrongFamily => f.write_str("product-locus Boolean cover family mismatch"),
            Self::WrongContext => f.write_str("product-locus Boolean cover context mismatch"),
            Self::SourceQueueMismatch => f.write_str("product-locus Boolean cover source queue mismatch"),
            Self::IndependentNestedPayloadComparisonUnsupported { component } => write!(f, "product-locus Boolean cover V1 cannot compare independently allocated nested {component} payloads without a checked recursive comparator"),
            Self::FreshExtractionAllocationMismatch => f.write_str("fresh product-locus Boolean cover did not retain the work item's extraction allocation"),
            Self::WorkItemNotFound { ordinal } => write!(f, "live-leaf work item {ordinal} was not found"),
            Self::SourceCaseNotFound { case } => write!(f, "source residual case {case} was not found"),
            Self::SourceBindingMismatch => f.write_str("product-locus source work item, extraction, partition, or sector differs"),
            Self::StructuralLocusNotFound { predicate_ordinal } => write!(f, "source predicate {predicate_ordinal} has no exact retained structural locus"),
            Self::StructuralLocusOrdinalOutOfRange { ordinal } => write!(f, "structural locus ordinal {ordinal} is outside the retained table"),
            Self::ResourceCountOverflow { resource } => write!(f, "{resource} count overflowed usize"),
            Self::ResourceLimit { resource, requested, limit } => write!(f, "{resource} requested {requested}, configured limit is {limit}"),
            Self::AllocationFailure { resource } => write!(f, "could not reserve bounded storage for {resource}"),
            Self::ReplayMismatch => f.write_str("product-locus Boolean cover did not replay"),
            Self::TerminalEvaluationMismatch => f.write_str("Boolean terminals did not evaluate disjointly"),
            Self::Queue(error) => error.fmt(f),
            Self::Coordinate(error) => error.fmt(f),
            Self::Coefficient(error) => error.fmt(f),
            Self::Sector(error) => error.fmt(f),
            Self::SymbolicCase(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for ResidualProductLocusBooleanCoverError {}

impl From<GeneratedSectorLiveLeafQueueError> for ResidualProductLocusBooleanCoverError {
    fn from(value: GeneratedSectorLiveLeafQueueError) -> Self {
        Self::Queue(value)
    }
}
impl From<CoordinateEqualityLocusError> for ResidualProductLocusBooleanCoverError {
    fn from(value: CoordinateEqualityLocusError) -> Self {
        Self::Coordinate(value)
    }
}
impl From<ParametricCoefficientError> for ResidualProductLocusBooleanCoverError {
    fn from(value: ParametricCoefficientError) -> Self {
        Self::Coefficient(value)
    }
}
impl From<SectorFoundationError> for ResidualProductLocusBooleanCoverError {
    fn from(value: SectorFoundationError) -> Self {
        Self::Sector(value)
    }
}
impl From<SymbolicSectorCaseError> for ResidualProductLocusBooleanCoverError {
    fn from(value: SymbolicSectorCaseError) -> Self {
        Self::SymbolicCase(value)
    }
}

#[derive(Clone)]
struct BooleanState {
    parent_ordinal: Option<usize>,
    decision_from_parent: Option<ResidualProductLocusBooleanDecision>,
    depth: usize,
    zero: Vec<usize>,
    nonzero: Vec<usize>,
    clauses: Vec<Vec<usize>>,
}

/// Raw construction coordinates which canonical V1 statistics intentionally
/// discard. Merely recording these scalar lengths is infallible; conversion to
/// a byte envelope is performed only by the V2 positional-with-census seam.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ResidualProductLocusBooleanRawTransientCensus {
    initial_clause_reserve_entries: usize,
    initial_nonzero_reserve_entries: usize,
    raw_root_clauses: usize,
    raw_root_literals: usize,
    raw_root_nonzero_facts: usize,
}

struct ResidualProductLocusBooleanInternalCompilation {
    certificate: ResidualProductLocusBooleanCoverCertificate,
    raw_transient_census: ResidualProductLocusBooleanRawTransientCensus,
}

/// V2-only result from the authenticated positional path. The V1 certificate
/// remains byte-for-byte schema-compatible; both memory scalars live beside
/// it and cross the authority seam without exposing raw construction shape.
pub(crate) struct ResidualProductLocusBooleanV2Compilation {
    certificate: ResidualProductLocusBooleanCoverCertificate,
    retained_owned_logical_bytes_upper_bound: usize,
    compilation_owned_logical_peak_upper_bound: usize,
}

impl ResidualProductLocusBooleanV2Compilation {
    pub(crate) fn into_parts(self) -> (ResidualProductLocusBooleanCoverCertificate, usize, usize) {
        (
            self.certificate,
            self.retained_owned_logical_bytes_upper_bound,
            self.compilation_owned_logical_peak_upper_bound,
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BaseFieldPredicateResolution {
    Discharge,
    Contradiction,
}

fn resolve_base_field_predicate(
    kind: SymbolicPolynomialPredicateKind,
    polynomial_is_zero: bool,
) -> BaseFieldPredicateResolution {
    match (kind, polynomial_is_zero) {
        (SymbolicPolynomialPredicateKind::EqualZero, true)
        | (SymbolicPolynomialPredicateKind::NonZero, false) => {
            BaseFieldPredicateResolution::Discharge
        }
        (SymbolicPolynomialPredicateKind::EqualZero, false)
        | (SymbolicPolynomialPredicateKind::NonZero, true) => {
            BaseFieldPredicateResolution::Contradiction
        }
    }
}

fn product_predicate_atoms<'a>(
    product_locus_ordinal: &'a usize,
    factor_locus_ordinals: Option<&'a [usize]>,
) -> &'a [usize] {
    factor_locus_ordinals.unwrap_or_else(|| std::slice::from_ref(product_locus_ordinal))
}

/// Compile one V1 Boolean cover after the caller has replayed the exact
/// retained live-leaf queue.
///
/// This crate-private seam exists so an authority-wide V2 compiler can replay
/// its source allocation exactly once before visiting every work item.  The
/// caller must pass the same queue allocation it replayed; this function still
/// authenticates the complete local source binding and all ordinary V1
/// resource limits, but deliberately does not replay the queue itself.
pub(crate) fn compile_replayed(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    queue: Arc<GeneratedSectorLiveLeafQueueCertificate>,
    source_work_item_ordinal: usize,
    limits: ResidualProductLocusBooleanCoverLimits,
) -> Result<ResidualProductLocusBooleanCoverCertificate, ResidualProductLocusBooleanCoverError> {
    compile_replayed_inner(
        family,
        context,
        queue,
        source_work_item_ordinal,
        None,
        limits,
    )
    .map(|compiled| compiled.certificate)
}

/// Unforgeable lifetime-bound proof that one exact V1 queue allocation has
/// replayed successfully in the supplied family and coefficient context.
///
/// Fields are private and construction always performs the queue replay.  The
/// positional no-replay child seam is therefore unreachable without this
/// proof.  The session borrows all three authorities and allocates nothing.
pub(crate) struct ResidualProductLocusBooleanReplaySession<'scope> {
    family: &'scope IntegralFamily,
    context: &'scope ParametricCoefficientContext,
    queue: &'scope Arc<GeneratedSectorLiveLeafQueueCertificate>,
}

impl<'scope> ResidualProductLocusBooleanReplaySession<'scope> {
    pub(crate) fn replay(
        family: &'scope IntegralFamily,
        context: &'scope ParametricCoefficientContext,
        queue: &'scope Arc<GeneratedSectorLiveLeafQueueCertificate>,
    ) -> Result<Self, GeneratedSectorLiveLeafQueueError> {
        queue.replay(family, context)?;
        Ok(Self {
            family,
            context,
            queue,
        })
    }

    pub(crate) fn authenticates_queue(
        &self,
        queue: &Arc<GeneratedSectorLiveLeafQueueCertificate>,
    ) -> bool {
        Arc::ptr_eq(self.queue, queue)
    }

    pub(crate) fn family_fingerprint(&self) -> &str {
        self.family.fingerprint_ref()
    }

    pub(crate) fn context_fingerprint(&self) -> &str {
        self.context.fingerprint()
    }

    /// Compile at the binary-resolved case position authenticated by the
    /// replayed queue owner.  The retained case at that slot must still agree
    /// with the work item's sealed case identifier.
    pub(crate) fn compile_replayed_at_case_position(
        &self,
        source_work_item_ordinal: usize,
        source_case_position: usize,
        limits: ResidualProductLocusBooleanCoverLimits,
    ) -> Result<ResidualProductLocusBooleanCoverCertificate, ResidualProductLocusBooleanCoverError>
    {
        compile_replayed_inner(
            self.family,
            self.context,
            Arc::clone(self.queue),
            source_work_item_ordinal,
            Some(source_case_position),
            limits,
        )
        .map(|compiled| compiled.certificate)
    }

    /// V2-only positional compilation with allocation censes adjacent to, but
    /// never embedded in, the frozen V1 certificate payload.
    pub(crate) fn compile_replayed_at_case_position_with_census(
        &self,
        source_work_item_ordinal: usize,
        source_case_position: usize,
        limits: ResidualProductLocusBooleanCoverLimits,
    ) -> Result<ResidualProductLocusBooleanV2Compilation, ResidualProductLocusBooleanCoverError>
    {
        let compiled = compile_replayed_inner(
            self.family,
            self.context,
            Arc::clone(self.queue),
            source_work_item_ordinal,
            Some(source_case_position),
            limits,
        )?;
        let retained_owned_logical_bytes_upper_bound =
            cover_retained_owned_logical_bytes_upper_bound(&compiled.certificate)?;
        let compilation_owned_logical_peak_upper_bound =
            cover_compilation_owned_logical_peak_upper_bound(
                &compiled.certificate,
                compiled.raw_transient_census,
            )?;
        Ok(ResidualProductLocusBooleanV2Compilation {
            certificate: compiled.certificate,
            retained_owned_logical_bytes_upper_bound,
            compilation_owned_logical_peak_upper_bound,
        })
    }
}

impl fmt::Debug for ResidualProductLocusBooleanReplaySession<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResidualProductLocusBooleanReplaySession")
            .field("private_replayed_scope", &"<redacted>")
            .finish()
    }
}

fn compile_replayed_inner(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    queue: Arc<GeneratedSectorLiveLeafQueueCertificate>,
    source_work_item_ordinal: usize,
    authenticated_source_case_position: Option<usize>,
    limits: ResidualProductLocusBooleanCoverLimits,
) -> Result<ResidualProductLocusBooleanInternalCompilation, ResidualProductLocusBooleanCoverError> {
    validate_scope(
        RESIDUAL_PRODUCT_LOCUS_BOOLEAN_COVER_V1_SCHEMA,
        queue.family_fingerprint(),
        queue.context_fingerprint(),
        family,
        context,
        limits,
    )?;
    check_limit(
        "family fingerprint bytes",
        family.fingerprint_ref().len(),
        limits.max_family_fingerprint_bytes,
    )?;
    check_limit(
        "context fingerprint bytes",
        context.fingerprint().len(),
        limits.max_context_fingerprint_bytes,
    )?;
    check_limit(
        "Boolean cover sector arity",
        queue.sector().arity(),
        limits.max_sector_arity,
    )?;
    let item = queue.work_items().get(source_work_item_ordinal).ok_or(
        ResidualProductLocusBooleanCoverError::WorkItemNotFound {
            ordinal: source_work_item_ordinal,
        },
    )?;
    let coverage_partition = queue.discovery().coverage().partition();
    let extraction_partition = item.extraction().source_partition();
    let source_partition_matches = if authenticated_source_case_position.is_some() {
        // The positional V2 seam is reached only after replay of this exact
        // immutable queue allocation.  Its extraction and coverage partitions
        // share the canonical source-identity Arc by construction.  Checking
        // that allocation plus the cheap scope scalars authenticates the same
        // partition without repeating derived equality over every split,
        // leaf predicate, exponent vector, and GMP coefficient.
        extraction_partition.schema() == coverage_partition.schema()
            && extraction_partition.context_fingerprint()
                == coverage_partition.context_fingerprint()
            && extraction_partition.orthant().sector() == coverage_partition.orthant().sector()
            && Arc::ptr_eq(
                extraction_partition.source_identity(),
                coverage_partition.source_identity(),
            )
    } else {
        // Preserve the original V1 compilation and replay semantics exactly.
        extraction_partition == coverage_partition
    };
    if item.ordinal() != source_work_item_ordinal
        || item.extraction().source_case() != item.source_case()
        || !source_partition_matches
        || queue.sector() != queue.discovery().sector()
    {
        return Err(ResidualProductLocusBooleanCoverError::SourceBindingMismatch);
    }
    let retained_source_case = item.source_case();
    let retained_source_extraction = item.extraction_arc().clone();
    let source_identity_bytes = item.extraction().source_partition().source_identity().len();
    check_limit(
        "source partition identity bytes referenced",
        source_identity_bytes,
        limits.max_source_identity_bytes_referenced,
    )?;
    let partition = coverage_partition;
    let source_case = if let Some(position) = authenticated_source_case_position {
        partition
            .cases()
            .get(position)
            .filter(|source_case| source_case.id() == item.source_case())
    } else {
        partition.case(item.source_case())
    }
    .ok_or(ResidualProductLocusBooleanCoverError::SourceCaseNotFound {
        case: item.source_case(),
    })?;
    check_limit(
        "source predicates",
        source_case.predicates().len(),
        limits.max_source_predicates,
    )?;

    let coverage = queue.discovery().coverage();
    let mut stats = ResidualProductLocusBooleanCoverStats {
        source_predicates: source_case.predicates().len(),
        ..ResidualProductLocusBooleanCoverStats::default()
    };
    let initial_clause_reserve_entries =
        source_case.predicates().len().min(limits.max_root_clauses);
    let mut clauses = Vec::new();
    clauses
        .try_reserve_exact(initial_clause_reserve_entries)
        .map_err(
            |_| ResidualProductLocusBooleanCoverError::AllocationFailure {
                resource: "root clauses",
            },
        )?;
    let initial_nonzero_reserve_entries = source_case
        .predicates()
        .len()
        .min(limits.max_root_nonzero_facts);
    let mut nonzero = Vec::new();
    nonzero
        .try_reserve_exact(initial_nonzero_reserve_entries)
        .map_err(
            |_| ResidualProductLocusBooleanCoverError::AllocationFailure {
                resource: "root nonzero facts",
            },
        )?;
    let mut raw_root_literals = 0usize;

    for (predicate_ordinal, predicate) in source_case.predicates().iter().enumerate() {
        stats.structural_locus_lookup_comparisons = bounded_add(
            "structural-locus lookup comparisons",
            stats.structural_locus_lookup_comparisons,
            coverage.structural_loci().len(),
            limits.max_structural_locus_lookup_comparisons,
        )?;
        let locus = coverage
            .structural_loci()
            .iter()
            .position(|retained| retained == predicate.polynomial())
            .ok_or(
                ResidualProductLocusBooleanCoverError::StructuralLocusNotFound {
                    predicate_ordinal,
                },
            )?;
        stats.base_unit_checks = bounded_add(
            "base-field unit checks",
            stats.base_unit_checks,
            1,
            limits.max_base_unit_checks,
        )?;
        let depends_on_indices = context.polynomial_depends_on_indices_with_limits(
            predicate.polynomial(),
            queue
                .discovery()
                .limits()
                .coverage
                .sector_cases
                .exact_algebra,
        )?;
        if !depends_on_indices {
            match resolve_base_field_predicate(predicate.kind(), predicate.polynomial().is_zero()) {
                BaseFieldPredicateResolution::Discharge => continue,
                BaseFieldPredicateResolution::Contradiction => {
                    // An empty positive clause is the canonical Boolean
                    // contradiction. Nonzero elements of K are units in K[n].
                    check_limit(
                        "root clauses",
                        checked_add("root clauses", clauses.len(), 1)?,
                        limits.max_root_clauses,
                    )?;
                    clauses.try_reserve_exact(1).map_err(|_| {
                        ResidualProductLocusBooleanCoverError::AllocationFailure {
                            resource: "root clauses",
                        }
                    })?;
                    clauses.push(Vec::new());
                    continue;
                }
            }
        }
        stats.product_witness_lookup_comparisons = bounded_add(
            "product-witness lookup comparisons",
            stats.product_witness_lookup_comparisons,
            coverage.product_zero_decompositions().len(),
            limits.max_product_witness_lookup_comparisons,
        )?;
        let decomposition = coverage
            .product_zero_decompositions()
            .iter()
            .find(|witness| witness.product_locus_ordinal() == locus);
        match predicate.kind() {
            SymbolicPolynomialPredicateKind::EqualZero => {
                if let Some(witness) = decomposition {
                    stats.product_equalities_expanded = checked_add(
                        "expanded product equalities",
                        stats.product_equalities_expanded,
                        1,
                    )?;
                    charge_factor_references(
                        &mut stats,
                        witness.factor_locus_ordinals().len(),
                        limits,
                    )?;
                }
                let values = product_predicate_atoms(
                    &locus,
                    decomposition.map(|witness| witness.factor_locus_ordinals()),
                );
                let mut clause = Vec::new();
                let requested_clauses = checked_add("root clauses", clauses.len(), 1)?;
                check_limit("root clauses", requested_clauses, limits.max_root_clauses)?;
                let requested_literals =
                    checked_add("root literals", raw_root_literals, values.len())?;
                check_limit(
                    "root literals",
                    requested_literals,
                    limits.max_root_literals,
                )?;
                clause.try_reserve_exact(values.len()).map_err(|_| {
                    ResidualProductLocusBooleanCoverError::AllocationFailure {
                        resource: "root clause literals",
                    }
                })?;
                clause.extend_from_slice(values);
                clauses.try_reserve_exact(1).map_err(|_| {
                    ResidualProductLocusBooleanCoverError::AllocationFailure {
                        resource: "root clauses",
                    }
                })?;
                clauses.push(clause);
                raw_root_literals = requested_literals;
            }
            SymbolicPolynomialPredicateKind::NonZero => {
                if let Some(witness) = decomposition {
                    stats.product_nonzero_predicates_expanded = checked_add(
                        "expanded product nonzero predicates",
                        stats.product_nonzero_predicates_expanded,
                        1,
                    )?;
                    charge_factor_references(
                        &mut stats,
                        witness.factor_locus_ordinals().len(),
                        limits,
                    )?;
                }
                let values = product_predicate_atoms(
                    &locus,
                    decomposition.map(|witness| witness.factor_locus_ordinals()),
                );
                let requested = checked_add("root nonzero facts", nonzero.len(), values.len())?;
                check_limit(
                    "root nonzero facts",
                    requested,
                    limits.max_root_nonzero_facts,
                )?;
                nonzero.try_reserve_exact(values.len()).map_err(|_| {
                    ResidualProductLocusBooleanCoverError::AllocationFailure {
                        resource: "root nonzero facts",
                    }
                })?;
                nonzero.extend_from_slice(values);
            }
        }
    }
    let raw_transient_census = ResidualProductLocusBooleanRawTransientCensus {
        initial_clause_reserve_entries,
        initial_nonzero_reserve_entries,
        raw_root_clauses: clauses.len(),
        raw_root_literals,
        raw_root_nonzero_facts: nonzero.len(),
    };
    clauses = canonicalize_clauses(clauses, &mut stats, limits)?;
    nonzero.sort_unstable();
    let old_nonzero = nonzero.len();
    nonzero.dedup();
    stats.duplicate_literals_removed = checked_add(
        "duplicate literals removed",
        stats.duplicate_literals_removed,
        old_nonzero - nonzero.len(),
    )?;
    let root_literals = clauses.iter().try_fold(0usize, |sum, clause| {
        checked_add("root literals", sum, clause.len())
    })?;
    check_limit("root clauses", clauses.len(), limits.max_root_clauses)?;
    check_limit("root literals", root_literals, limits.max_root_literals)?;
    stats.root_clauses = clauses.len();
    stats.root_literals = root_literals;
    stats.root_nonzero_facts = nonzero.len();

    // Build the canonical atom set incrementally. Do not first allocate all
    // root literal references: a highly duplicated CNF may have a small atom
    // budget even when its retained literal budget is much larger.
    let mut atoms = Vec::new();
    for &ordinal in &nonzero {
        insert_unique_atom(&mut atoms, ordinal, &mut stats, limits)?;
    }
    for &ordinal in clauses.iter().flat_map(|clause| clause.iter()) {
        insert_unique_atom(&mut atoms, ordinal, &mut stats, limits)?;
    }
    for &ordinal in &atoms {
        let polynomial = coverage.structural_locus(ordinal).ok_or(
            ResidualProductLocusBooleanCoverError::StructuralLocusOrdinalOutOfRange { ordinal },
        )?;
        stats.referenced_locus_terms = bounded_add(
            "referenced structural-locus terms",
            stats.referenced_locus_terms,
            polynomial.term_count(),
            limits.max_referenced_locus_terms,
        )?;
    }
    stats.atoms = atoms.len();

    let requested_coordinate_checks = checked_add(
        "coordinate recognition checks",
        stats.coordinate_recognition_checks,
        atoms.len(),
    )?;
    check_limit(
        "coordinate recognition checks",
        requested_coordinate_checks,
        limits.max_coordinate_recognition_checks,
    )?;
    let mut coordinates = Vec::new();
    coordinates.try_reserve_exact(atoms.len()).map_err(|_| {
        ResidualProductLocusBooleanCoverError::AllocationFailure {
            resource: "coordinate atom metadata",
        }
    })?;
    let coordinate_limits = queue.discovery().limits().coverage.coordinate_loci;
    for &ordinal in &atoms {
        let coordinate = crate::coordinate_equality_loci::recognize_coordinate_locus_for_pruning(
            context,
            coverage.structural_locus(ordinal).ok_or(
                ResidualProductLocusBooleanCoverError::StructuralLocusOrdinalOutOfRange { ordinal },
            )?,
            coordinate_limits,
        )?;
        coordinates.push((ordinal, coordinate));
    }
    stats.coordinate_recognition_checks = requested_coordinate_checks;

    // The initial DPLL state duplicates the canonical root CNF. Enforce the
    // tighter per-state budgets before cloning any clause or literal storage;
    // root-retention limits alone need not imply these bounds.
    check_limit(
        "clauses per DPLL state",
        clauses.len(),
        limits.max_clauses_per_state,
    )?;
    let initial_literals = clauses.iter().try_fold(0usize, |sum, clause| {
        checked_add("literals per DPLL state", sum, clause.len())
    })?;
    check_limit(
        "literals per DPLL state",
        initial_literals,
        limits.max_literals_per_state,
    )?;
    check_limit("DPLL frontier states", 1, limits.max_frontier_states)?;
    check_limit("DPLL nodes", 1, limits.max_dpll_nodes)?;
    check_limit("retained path facts", nonzero.len(), limits.max_path_facts)?;
    check_limit(
        "retained node clauses",
        clauses.len(),
        limits.max_retained_node_clauses,
    )?;
    check_limit(
        "retained node literals",
        initial_literals,
        limits.max_retained_node_literals,
    )?;
    let initial_state_bytes =
        state_bytes_from_shape(0, nonzero.len(), clauses.len(), initial_literals)?;
    check_limit(
        "retained DPLL state bytes",
        initial_state_bytes,
        limits.max_retained_state_bytes,
    )?;
    check_limit(
        "peak DPLL frontier bytes",
        initial_state_bytes,
        limits.max_peak_frontier_bytes,
    )?;
    let initial = BooleanState {
        parent_ordinal: None,
        decision_from_parent: None,
        depth: 0,
        zero: Vec::new(),
        nonzero: try_clone_vec(&nonzero, "root nonzero facts for DPLL")?,
        clauses: try_clone_clauses(&clauses, "root clauses for DPLL")?,
    };
    let nodes = build_dpll_cover(initial, queue.sector(), &coordinates, &mut stats, limits)?;

    let mut certificate = ResidualProductLocusBooleanCoverCertificate {
        schema: RESIDUAL_PRODUCT_LOCUS_BOOLEAN_COVER_V1_SCHEMA,
        family_fingerprint: Arc::from(family.fingerprint_ref()),
        context_fingerprint: context.fingerprint().into(),
        sector: queue.sector().clone(),
        source_queue: queue,
        source_work_item_ordinal,
        source_case: retained_source_case,
        source_extraction: retained_source_extraction,
        atoms: atoms.into_boxed_slice(),
        root_clauses: box_clauses(clauses)?,
        root_nonzero_atoms: nonzero.into_boxed_slice(),
        nodes: nodes.into_boxed_slice(),
        limits,
        stats,
    };
    let comparison = cover_owned_payload_comparison_census(&certificate, limits)?;
    certificate.stats.payload_comparison_units = comparison.units;
    certificate.stats.payload_comparison_bytes = comparison.bytes;
    Ok(ResidualProductLocusBooleanInternalCompilation {
        certificate,
        raw_transient_census,
    })
}

fn charge_factor_references(
    stats: &mut ResidualProductLocusBooleanCoverStats,
    count: usize,
    limits: ResidualProductLocusBooleanCoverLimits,
) -> Result<(), ResidualProductLocusBooleanCoverError> {
    stats.factor_references_expanded = bounded_add(
        "expanded product factor references",
        stats.factor_references_expanded,
        count,
        limits.max_factor_references_expanded,
    )?;
    Ok(())
}

fn canonicalize_clauses(
    mut clauses: Vec<Vec<usize>>,
    stats: &mut ResidualProductLocusBooleanCoverStats,
    limits: ResidualProductLocusBooleanCoverLimits,
) -> Result<Vec<Vec<usize>>, ResidualProductLocusBooleanCoverError> {
    for clause in &mut clauses {
        clause.sort_unstable();
        let before = clause.len();
        clause.dedup();
        stats.duplicate_literals_removed = checked_add(
            "duplicate literals removed",
            stats.duplicate_literals_removed,
            before - clause.len(),
        )?;
    }
    clauses.sort_by(|left, right| left.len().cmp(&right.len()).then_with(|| left.cmp(right)));
    let mut retained: Vec<Vec<usize>> = Vec::new();
    retained.try_reserve_exact(clauses.len()).map_err(|_| {
        ResidualProductLocusBooleanCoverError::AllocationFailure {
            resource: "canonical clauses",
        }
    })?;
    'candidate: for clause in clauses {
        for earlier in &retained {
            stats.subsumption_comparisons = bounded_add(
                "clause subsumption comparisons",
                stats.subsumption_comparisons,
                1,
                limits.max_subsumption_comparisons,
            )?;
            if sorted_subset(earlier, &clause) {
                stats.duplicate_or_subsumed_clauses_removed = checked_add(
                    "duplicate or subsumed clauses removed",
                    stats.duplicate_or_subsumed_clauses_removed,
                    1,
                )?;
                continue 'candidate;
            }
        }
        retained.push(clause);
    }
    Ok(retained)
}

fn sorted_subset(left: &[usize], right: &[usize]) -> bool {
    let (mut i, mut j) = (0, 0);
    while i < left.len() && j < right.len() {
        if left[i] == right[j] {
            i += 1;
            j += 1;
        } else if left[i] > right[j] {
            j += 1;
        } else {
            return false;
        }
    }
    i == left.len()
}

fn insert_unique_atom(
    atoms: &mut Vec<usize>,
    ordinal: usize,
    stats: &mut ResidualProductLocusBooleanCoverStats,
    limits: ResidualProductLocusBooleanCoverLimits,
) -> Result<(), ResidualProductLocusBooleanCoverError> {
    let (mut low, mut high) = (0usize, atoms.len());
    while low < high {
        stats.atom_lookup_comparisons = bounded_add(
            "Boolean atom lookup comparisons",
            stats.atom_lookup_comparisons,
            1,
            limits.max_atom_lookup_comparisons,
        )?;
        let middle = low + (high - low) / 2;
        match atoms[middle].cmp(&ordinal) {
            std::cmp::Ordering::Less => low = middle + 1,
            std::cmp::Ordering::Greater => high = middle,
            std::cmp::Ordering::Equal => return Ok(()),
        }
    }
    check_limit(
        "Boolean atoms",
        checked_add("Boolean atoms", atoms.len(), 1)?,
        limits.max_atoms,
    )?;
    atoms.try_reserve_exact(1).map_err(|_| {
        ResidualProductLocusBooleanCoverError::AllocationFailure {
            resource: "Boolean atoms",
        }
    })?;
    atoms.insert(low, ordinal);
    Ok(())
}

fn build_dpll_cover(
    initial: BooleanState,
    sector: &SectorMask,
    coordinates: &[(usize, Option<(usize, i64)>)],
    stats: &mut ResidualProductLocusBooleanCoverStats,
    limits: ResidualProductLocusBooleanCoverLimits,
) -> Result<Vec<ResidualProductLocusBooleanNode>, ResidualProductLocusBooleanCoverError> {
    // The initial state is itself one frontier entry. Reject a zero frontier
    // budget before reserving or pushing that entry. Recheck per-state shape
    // here as well so every internal caller receives the same hard boundary.
    check_limit("DPLL frontier states", 1, limits.max_frontier_states)?;
    check_limit(
        "clauses per DPLL state",
        initial.clauses.len(),
        limits.max_clauses_per_state,
    )?;
    let initial_literals = initial.clauses.iter().try_fold(0usize, |sum, clause| {
        checked_add("literals per DPLL state", sum, clause.len())
    })?;
    check_limit(
        "literals per DPLL state",
        initial_literals,
        limits.max_literals_per_state,
    )?;
    let initial_bytes = state_bytes(&initial)?;
    check_limit(
        "peak DPLL frontier bytes",
        initial_bytes,
        limits.max_peak_frontier_bytes,
    )?;
    let mut frontier = Vec::new();
    frontier.try_reserve_exact(1).map_err(|_| {
        ResidualProductLocusBooleanCoverError::AllocationFailure {
            resource: "DPLL frontier",
        }
    })?;
    frontier.push(initial);
    let mut frontier_bytes = initial_bytes;
    stats.peak_frontier_states = 1;
    stats.peak_frontier_bytes = initial_bytes;
    let mut nodes = Vec::new();

    while let Some(mut state) = frontier.pop() {
        let current_state_bytes = state_bytes(&state)?;
        frontier_bytes = frontier_bytes.checked_sub(current_state_bytes).ok_or(
            ResidualProductLocusBooleanCoverError::ResourceCountOverflow {
                resource: "DPLL frontier bytes",
            },
        )?;
        let node_ordinal = nodes.len();
        check_limit(
            "DPLL nodes",
            checked_add("DPLL nodes", node_ordinal, 1)?,
            limits.max_dpll_nodes,
        )?;
        check_limit("DPLL depth", state.depth, limits.max_dpll_depth)?;

        // Simplification can only remove clauses/literals, but it may force
        // additional zero facts and retain their propagation transcript. Use
        // the complete atom census as a conservative growth cap and reject
        // cumulative retention limits before simplification allocates or does
        // coordinate/subsumption work.
        let current_clauses = state.clauses.len();
        let current_literals = state.clauses.iter().try_fold(0usize, |sum, clause| {
            checked_add("retained node literals", sum, clause.len())
        })?;
        let current_path_facts =
            checked_add("retained path facts", state.zero.len(), state.nonzero.len())?;
        let conservative_path_facts =
            checked_add("retained path facts", current_path_facts, coordinates.len())?;
        check_limit(
            "retained path facts",
            checked_add(
                "retained path facts",
                stats.path_facts,
                conservative_path_facts,
            )?,
            limits.max_path_facts,
        )?;
        check_limit(
            "retained node clauses",
            checked_add(
                "retained node clauses",
                stats.retained_node_clauses,
                current_clauses,
            )?,
            limits.max_retained_node_clauses,
        )?;
        check_limit(
            "retained node literals",
            checked_add(
                "retained node literals",
                stats.retained_node_literals,
                current_literals,
            )?,
            limits.max_retained_node_literals,
        )?;
        let conservative_growth_bytes = coordinates
            .len()
            .checked_mul(size_of::<usize>())
            .and_then(|bytes| bytes.checked_mul(2))
            .ok_or(
                ResidualProductLocusBooleanCoverError::ResourceCountOverflow {
                    resource: "retained DPLL state bytes",
                },
            )?;
        let conservative_state_bytes = checked_add(
            "retained DPLL state bytes",
            current_state_bytes,
            conservative_growth_bytes,
        )?;
        check_limit(
            "retained DPLL state bytes",
            checked_add(
                "retained DPLL state bytes",
                stats.retained_state_bytes,
                conservative_state_bytes,
            )?,
            limits.max_retained_state_bytes,
        )?;
        let scratch_bytes = simplification_scratch_bytes(&state, coordinates.len())?;
        check_limit(
            "peak DPLL simplification scratch bytes",
            scratch_bytes,
            limits.max_peak_simplification_scratch_bytes,
        )?;
        stats.peak_simplification_scratch_bytes =
            stats.peak_simplification_scratch_bytes.max(scratch_bytes);

        let mut node_propagations = Vec::new();
        let contradiction = simplify_state(
            &mut state,
            sector,
            coordinates,
            &mut node_propagations,
            stats,
            limits,
        )?;
        let node_clauses = state.clauses.len();
        let node_literals = state.clauses.iter().try_fold(0usize, |sum, c| {
            checked_add("retained node literals", sum, c.len())
        })?;
        let path_facts = checked_add("path facts", state.zero.len(), state.nonzero.len())?;
        let retained_bytes = checked_add(
            "retained DPLL state bytes",
            state_bytes(&state)?,
            node_propagations
                .len()
                .checked_mul(size_of::<usize>())
                .ok_or(
                    ResidualProductLocusBooleanCoverError::ResourceCountOverflow {
                        resource: "retained DPLL state bytes",
                    },
                )?,
        )?;
        let next_path_facts = checked_add("retained path facts", stats.path_facts, path_facts)?;
        check_limit(
            "retained path facts",
            next_path_facts,
            limits.max_path_facts,
        )?;
        let next_node_clauses = checked_add(
            "retained node clauses",
            stats.retained_node_clauses,
            node_clauses,
        )?;
        check_limit(
            "retained node clauses",
            next_node_clauses,
            limits.max_retained_node_clauses,
        )?;
        let next_node_literals = checked_add(
            "retained node literals",
            stats.retained_node_literals,
            node_literals,
        )?;
        check_limit(
            "retained node literals",
            next_node_literals,
            limits.max_retained_node_literals,
        )?;
        let next_state_bytes = checked_add(
            "retained DPLL state bytes",
            stats.retained_state_bytes,
            retained_bytes,
        )?;
        check_limit(
            "retained DPLL state bytes",
            next_state_bytes,
            limits.max_retained_state_bytes,
        )?;
        let next_retained_propagations = checked_add(
            "retained DPLL propagation entries",
            stats.retained_propagation_entries,
            node_propagations.len(),
        )?;
        check_limit(
            "retained DPLL propagation entries",
            next_retained_propagations,
            limits.max_retained_propagation_entries,
        )?;
        let outcome = if let Some(reason) = contradiction {
            let total = checked_add(
                "Boolean terminals",
                stats.ready_terminals,
                stats.proved_empty_terminals,
            )?;
            check_limit(
                "Boolean terminals",
                checked_add("Boolean terminals", total, 1)?,
                limits.max_terminals,
            )?;
            stats.proved_empty_terminals = checked_add(
                "proved-empty Boolean terminals",
                stats.proved_empty_terminals,
                1,
            )?;
            ResidualProductLocusBooleanNodeOutcome::ProvedEmpty(reason)
        } else if state.clauses.is_empty() {
            let total = checked_add(
                "Boolean terminals",
                stats.ready_terminals,
                stats.proved_empty_terminals,
            )?;
            check_limit(
                "Boolean terminals",
                checked_add("Boolean terminals", total, 1)?,
                limits.max_terminals,
            )?;
            stats.ready_terminals =
                checked_add("ready Boolean terminals", stats.ready_terminals, 1)?;
            ResidualProductLocusBooleanNodeOutcome::ReadyForAffineRecognition
        } else {
            // Unit propagation is complete here. Branch on the canonical
            // smallest atom of the shortest remaining clause.
            let atom = state.clauses[0][0];
            stats.dpll_branches = bounded_add(
                "DPLL branches",
                stats.dpll_branches,
                1,
                limits.max_dpll_branches,
            )?;
            let child_depth = checked_add("DPLL depth", state.depth, 1)?;
            check_limit("DPLL depth", child_depth, limits.max_dpll_depth)?;
            let requested_frontier = checked_add("DPLL frontier states", frontier.len(), 2)?;
            check_limit(
                "DPLL frontier states",
                requested_frontier,
                limits.max_frontier_states,
            )?;
            let guaranteed_pending_nodes = checked_add(
                "DPLL nodes",
                checked_add("DPLL nodes", frontier.len(), 2)?,
                1,
            )?;
            let guaranteed_node_demand =
                checked_add("DPLL nodes", nodes.len(), guaranteed_pending_nodes)?;
            check_limit("DPLL nodes", guaranteed_node_demand, limits.max_dpll_nodes)?;
            let frontier_path_facts = frontier.iter().try_fold(0usize, |sum, pending| {
                checked_add(
                    "retained path facts",
                    sum,
                    checked_add(
                        "retained path facts",
                        pending.zero.len(),
                        pending.nonzero.len(),
                    )?,
                )
            })?;
            let child_path_facts = checked_add("retained path facts", path_facts, 1)?;
            let two_child_path_facts = child_path_facts.checked_mul(2).ok_or(
                ResidualProductLocusBooleanCoverError::ResourceCountOverflow {
                    resource: "retained path facts",
                },
            )?;
            let guaranteed_path_fact_demand = [
                stats.path_facts,
                path_facts,
                frontier_path_facts,
                two_child_path_facts,
            ]
            .into_iter()
            .try_fold(0usize, |sum, value| {
                checked_add("retained path facts", sum, value)
            })?;
            check_limit(
                "retained path facts",
                guaranteed_path_fact_demand,
                limits.max_path_facts,
            )?;
            // Both children clone this exact state and add at most one fact.
            // Conservatively charge two complete current-state copies plus
            // two words before any child allocation occurs.
            let conservative_child_bytes = checked_add(
                "DPLL frontier bytes",
                retained_bytes.checked_mul(2).ok_or(
                    ResidualProductLocusBooleanCoverError::ResourceCountOverflow {
                        resource: "DPLL frontier bytes",
                    },
                )?,
                size_of::<usize>().checked_mul(2).ok_or(
                    ResidualProductLocusBooleanCoverError::ResourceCountOverflow {
                        resource: "DPLL frontier bytes",
                    },
                )?,
            )?;
            let conservative_frontier = checked_add(
                "DPLL frontier bytes",
                frontier_bytes,
                conservative_child_bytes,
            )?;
            check_limit(
                "peak DPLL frontier bytes",
                conservative_frontier,
                limits.max_peak_frontier_bytes,
            )?;
            frontier.try_reserve_exact(2).map_err(|_| {
                ResidualProductLocusBooleanCoverError::AllocationFailure {
                    resource: "DPLL frontier",
                }
            })?;
            let mut nonzero_child = clone_state(&state)?;
            nonzero_child.parent_ordinal = Some(node_ordinal);
            nonzero_child.decision_from_parent = Some(ResidualProductLocusBooleanDecision {
                atom_locus_ordinal: atom,
                polarity: ResidualProductLocusBooleanPolarity::NonZero,
            });
            nonzero_child.depth = child_depth;
            insert_fact(&mut nonzero_child.nonzero, atom)?;
            let mut zero_child = clone_state(&state)?;
            zero_child.parent_ordinal = Some(node_ordinal);
            zero_child.decision_from_parent = Some(ResidualProductLocusBooleanDecision {
                atom_locus_ordinal: atom,
                polarity: ResidualProductLocusBooleanPolarity::EqualZero,
            });
            zero_child.depth = child_depth;
            insert_fact(&mut zero_child.zero, atom)?;
            let nonzero_bytes = state_bytes(&nonzero_child)?;
            let zero_bytes = state_bytes(&zero_child)?;
            let requested_bytes = checked_add(
                "DPLL frontier bytes",
                checked_add("DPLL frontier bytes", frontier_bytes, nonzero_bytes)?,
                zero_bytes,
            )?;
            check_limit(
                "peak DPLL frontier bytes",
                requested_bytes,
                limits.max_peak_frontier_bytes,
            )?;
            // LIFO: push nonzero first so deterministic zero-first traversal is retained.
            frontier.push(nonzero_child);
            frontier.push(zero_child);
            frontier_bytes = requested_bytes;
            stats.peak_frontier_states = stats.peak_frontier_states.max(requested_frontier);
            stats.peak_frontier_bytes = stats.peak_frontier_bytes.max(requested_bytes);
            ResidualProductLocusBooleanNodeOutcome::Branched {
                atom_locus_ordinal: atom,
            }
        };

        stats.path_facts = next_path_facts;
        stats.retained_node_clauses = next_node_clauses;
        stats.retained_node_literals = next_node_literals;
        stats.retained_state_bytes = next_state_bytes;
        stats.retained_propagation_entries = next_retained_propagations;
        stats.dpll_nodes = checked_add("DPLL nodes", stats.dpll_nodes, 1)?;
        nodes.try_reserve_exact(1).map_err(|_| {
            ResidualProductLocusBooleanCoverError::AllocationFailure {
                resource: "DPLL nodes",
            }
        })?;
        nodes.push(ResidualProductLocusBooleanNode {
            ordinal: node_ordinal,
            parent_ordinal: state.parent_ordinal,
            decision_from_parent: state.decision_from_parent,
            depth: state.depth,
            equal_zero_atoms: state.zero.into_boxed_slice(),
            nonzero_atoms: state.nonzero.into_boxed_slice(),
            propagated_zero_atoms: node_propagations.into_boxed_slice(),
            remaining_clauses: box_clauses(state.clauses)?,
            outcome,
        });
    }
    Ok(nodes)
}

fn simplify_state(
    state: &mut BooleanState,
    sector: &SectorMask,
    coordinates: &[(usize, Option<(usize, i64)>)],
    node_propagations: &mut Vec<usize>,
    stats: &mut ResidualProductLocusBooleanCoverStats,
    limits: ResidualProductLocusBooleanCoverLimits,
) -> Result<Option<ResidualProductLocusBooleanEmptyReason>, ResidualProductLocusBooleanCoverError> {
    loop {
        if let Some(atom) = first_intersection(&state.zero, &state.nonzero) {
            return Ok(Some(
                ResidualProductLocusBooleanEmptyReason::OppositeFacts {
                    atom_locus_ordinal: atom,
                },
            ));
        }
        if let Some(reason) = coordinate_contradiction(state, sector, coordinates, stats, limits)? {
            return Ok(Some(reason));
        }
        let mut simplified = Vec::new();
        simplified
            .try_reserve_exact(state.clauses.len())
            .map_err(
                |_| ResidualProductLocusBooleanCoverError::AllocationFailure {
                    resource: "simplified DPLL clauses",
                },
            )?;
        for clause in &state.clauses {
            if clause
                .iter()
                .any(|atom| state.zero.binary_search(atom).is_ok())
            {
                continue;
            }
            let mut remaining = Vec::new();
            remaining.try_reserve_exact(clause.len()).map_err(|_| {
                ResidualProductLocusBooleanCoverError::AllocationFailure {
                    resource: "simplified DPLL literals",
                }
            })?;
            remaining.extend(
                clause
                    .iter()
                    .copied()
                    .filter(|atom| state.nonzero.binary_search(atom).is_err()),
            );
            if remaining.is_empty() {
                let retained_empty_reason_ordinals = bounded_add(
                    "retained empty-reason ordinals",
                    stats.retained_empty_reason_ordinals,
                    clause.len(),
                    limits.max_retained_empty_reason_ordinals,
                )?;
                let exhausted = try_clone_vec(clause, "exhausted-clause witness")?;
                stats.retained_empty_reason_ordinals = retained_empty_reason_ordinals;
                return Ok(Some(
                    ResidualProductLocusBooleanEmptyReason::ExhaustedClause {
                        atom_locus_ordinals: exhausted.into_boxed_slice(),
                    },
                ));
            }
            simplified.push(remaining);
        }
        simplified = canonicalize_clauses(simplified, stats, limits)?;
        check_limit(
            "clauses per DPLL state",
            simplified.len(),
            limits.max_clauses_per_state,
        )?;
        let literals = simplified.iter().try_fold(0usize, |sum, c| {
            checked_add("literals per DPLL state", sum, c.len())
        })?;
        check_limit(
            "literals per DPLL state",
            literals,
            limits.max_literals_per_state,
        )?;
        state.clauses = simplified;
        let Some(unit) = state
            .clauses
            .iter()
            .find(|clause| clause.len() == 1)
            .map(|clause| clause[0])
        else {
            return Ok(None);
        };
        if state.zero.binary_search(&unit).is_err() {
            let next_propagations = checked_add("DPLL propagations", stats.propagations, 1)?;
            check_limit(
                "DPLL propagations",
                next_propagations,
                limits.max_propagations,
            )?;
            let next_retained = checked_add(
                "retained DPLL propagation entries",
                checked_add(
                    "retained DPLL propagation entries",
                    stats.retained_propagation_entries,
                    node_propagations.len(),
                )?,
                1,
            )?;
            check_limit(
                "retained DPLL propagation entries",
                next_retained,
                limits.max_retained_propagation_entries,
            )?;
            node_propagations.try_reserve_exact(1).map_err(|_| {
                ResidualProductLocusBooleanCoverError::AllocationFailure {
                    resource: "DPLL propagation transcript",
                }
            })?;
            insert_fact(&mut state.zero, unit)?;
            stats.propagations = next_propagations;
            node_propagations.push(unit);
        }
        // Loop again so the newly forced zero fact removes every satisfied
        // clause before the state is classified or branched.
    }
}

fn coordinate_contradiction(
    state: &BooleanState,
    sector: &SectorMask,
    coordinates: &[(usize, Option<(usize, i64)>)],
    stats: &mut ResidualProductLocusBooleanCoverStats,
    limits: ResidualProductLocusBooleanCoverLimits,
) -> Result<Option<ResidualProductLocusBooleanEmptyReason>, ResidualProductLocusBooleanCoverError> {
    let coordinate_payload_bytes = state
        .zero
        .len()
        .checked_mul(size_of::<(usize, usize, i64)>())
        .ok_or(
            ResidualProductLocusBooleanCoverError::ResourceCountOverflow {
                resource: "DPLL simplification scratch bytes",
            },
        )?;
    let coordinate_scratch_bytes = checked_add(
        "DPLL simplification scratch bytes",
        size_of::<Vec<(usize, usize, i64)>>(),
        coordinate_payload_bytes,
    )?;
    check_limit(
        "peak DPLL simplification scratch bytes",
        coordinate_scratch_bytes,
        limits.max_peak_simplification_scratch_bytes,
    )?;
    stats.peak_simplification_scratch_bytes = stats
        .peak_simplification_scratch_bytes
        .max(coordinate_scratch_bytes);
    let zero_pairs = state
        .zero
        .len()
        .saturating_sub(1)
        .checked_mul(state.zero.len())
        .and_then(|value| value.checked_div(2))
        .ok_or(
            ResidualProductLocusBooleanCoverError::ResourceCountOverflow {
                resource: "coordinate contradiction comparisons",
            },
        )?;
    let zero_nonzero_pairs = state.zero.len().checked_mul(state.nonzero.len()).ok_or(
        ResidualProductLocusBooleanCoverError::ResourceCountOverflow {
            resource: "coordinate contradiction comparisons",
        },
    )?;
    let maximum_comparisons = checked_add(
        "coordinate contradiction comparisons",
        zero_pairs,
        zero_nonzero_pairs,
    )?;
    check_limit(
        "coordinate contradiction comparisons",
        checked_add(
            "coordinate contradiction comparisons",
            stats.coordinate_contradiction_comparisons,
            maximum_comparisons,
        )?,
        limits.max_coordinate_contradiction_comparisons,
    )?;
    let mut zero_coordinates = Vec::new();
    zero_coordinates
        .try_reserve_exact(state.zero.len())
        .map_err(
            |_| ResidualProductLocusBooleanCoverError::AllocationFailure {
                resource: "coordinate zero facts",
            },
        )?;
    for &atom in &state.zero {
        if let Some((index, value)) = coordinate_for(coordinates, atom) {
            let side = if sector.is_active(index)? {
                SectorOrthantSide::AtLeastOne
            } else {
                SectorOrthantSide::AtMostZero
            };
            let accepts = match side {
                SectorOrthantSide::AtLeastOne => value >= 1,
                SectorOrthantSide::AtMostZero => value <= 0,
            };
            if !accepts {
                return Ok(Some(
                    ResidualProductLocusBooleanEmptyReason::OrthantViolation {
                        atom_locus_ordinal: atom,
                        index,
                        value,
                        side,
                    },
                ));
            }
            for &(other_atom, other_index, other_value) in &zero_coordinates {
                stats.coordinate_contradiction_comparisons = bounded_add(
                    "coordinate contradiction comparisons",
                    stats.coordinate_contradiction_comparisons,
                    1,
                    limits.max_coordinate_contradiction_comparisons,
                )?;
                if index == other_index && value != other_value {
                    return Ok(Some(
                        ResidualProductLocusBooleanEmptyReason::ConflictingCoordinateEqualities {
                            first_atom_locus_ordinal: other_atom,
                            second_atom_locus_ordinal: atom,
                            index,
                            first_value: other_value,
                            second_value: value,
                        },
                    ));
                }
            }
            zero_coordinates.push((atom, index, value));
        }
    }
    for &nonzero_atom in &state.nonzero {
        if let Some((index, value)) = coordinate_for(coordinates, nonzero_atom) {
            for &(zero_atom, zero_index, zero_value) in &zero_coordinates {
                stats.coordinate_contradiction_comparisons = bounded_add(
                    "coordinate contradiction comparisons",
                    stats.coordinate_contradiction_comparisons,
                    1,
                    limits.max_coordinate_contradiction_comparisons,
                )?;
                if zero_index == index && zero_value == value {
                    return Ok(Some(
                        ResidualProductLocusBooleanEmptyReason::CoordinateEqualityNonzero {
                            equality_atom_locus_ordinal: zero_atom,
                            nonzero_atom_locus_ordinal: nonzero_atom,
                            index,
                            value,
                        },
                    ));
                }
            }
        }
    }
    Ok(None)
}

fn coordinate_for(
    coordinates: &[(usize, Option<(usize, i64)>)],
    atom: usize,
) -> Option<(usize, i64)> {
    coordinates
        .binary_search_by_key(&atom, |entry| entry.0)
        .ok()
        .and_then(|index| coordinates[index].1)
}

fn first_intersection(left: &[usize], right: &[usize]) -> Option<usize> {
    let (mut i, mut j) = (0, 0);
    while i < left.len() && j < right.len() {
        if left[i] == right[j] {
            return Some(left[i]);
        }
        if left[i] < right[j] {
            i += 1;
        } else {
            j += 1;
        }
    }
    None
}

fn insert_fact(
    facts: &mut Vec<usize>,
    atom: usize,
) -> Result<(), ResidualProductLocusBooleanCoverError> {
    if let Err(position) = facts.binary_search(&atom) {
        facts.try_reserve_exact(1).map_err(|_| {
            ResidualProductLocusBooleanCoverError::AllocationFailure {
                resource: "DPLL path facts",
            }
        })?;
        facts.insert(position, atom);
    }
    Ok(())
}

fn clone_state(
    state: &BooleanState,
) -> Result<BooleanState, ResidualProductLocusBooleanCoverError> {
    Ok(BooleanState {
        parent_ordinal: state.parent_ordinal,
        decision_from_parent: state.decision_from_parent,
        depth: state.depth,
        zero: try_clone_vec(&state.zero, "DPLL zero facts")?,
        nonzero: try_clone_vec(&state.nonzero, "DPLL nonzero facts")?,
        clauses: {
            let mut clauses = Vec::new();
            clauses
                .try_reserve_exact(state.clauses.len())
                .map_err(
                    |_| ResidualProductLocusBooleanCoverError::AllocationFailure {
                        resource: "DPLL clauses",
                    },
                )?;
            for clause in &state.clauses {
                clauses.push(try_clone_vec(clause, "DPLL clause literals")?);
            }
            clauses
        },
    })
}

fn try_clone_vec<T: Clone>(
    source: &[T],
    resource: &'static str,
) -> Result<Vec<T>, ResidualProductLocusBooleanCoverError> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(source.len())
        .map_err(|_| ResidualProductLocusBooleanCoverError::AllocationFailure { resource })?;
    result.extend_from_slice(source);
    Ok(result)
}

fn try_clone_clauses(
    source: &[Vec<usize>],
    resource: &'static str,
) -> Result<Vec<Vec<usize>>, ResidualProductLocusBooleanCoverError> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(source.len())
        .map_err(|_| ResidualProductLocusBooleanCoverError::AllocationFailure { resource })?;
    for clause in source {
        result.push(try_clone_vec(clause, resource)?);
    }
    Ok(result)
}

fn box_clauses(
    clauses: Vec<Vec<usize>>,
) -> Result<Box<[Box<[usize]>]>, ResidualProductLocusBooleanCoverError> {
    let mut boxed = Vec::new();
    boxed.try_reserve_exact(clauses.len()).map_err(|_| {
        ResidualProductLocusBooleanCoverError::AllocationFailure {
            resource: "boxed Boolean clauses",
        }
    })?;
    for clause in clauses {
        boxed.push(clause.into_boxed_slice());
    }
    Ok(boxed.into_boxed_slice())
}

fn state_bytes(state: &BooleanState) -> Result<usize, ResidualProductLocusBooleanCoverError> {
    let literals = state.clauses.iter().try_fold(0usize, |sum, c| {
        checked_add("DPLL state bytes", sum, c.len())
    })?;
    state_bytes_from_shape(
        state.zero.len(),
        state.nonzero.len(),
        state.clauses.len(),
        literals,
    )
}

/// Logical owned bytes requested by a Boolean state.
///
/// `size_of::<BooleanState>()` accounts for the three outer `Vec` headers and
/// scalar metadata. The heap charge accounts for one `Vec<usize>` header per
/// clause and every initialized fact/literal payload element. Rust allocators
/// may round `try_reserve_exact` capacities upward; such allocator-private
/// padding is not semantically retained and is deliberately not charged.
/// Every producer nevertheless performs a fallible exact reserve before it
/// writes those logically charged elements.
fn state_bytes_from_shape(
    zero_facts: usize,
    nonzero_facts: usize,
    clauses: usize,
    literals: usize,
) -> Result<usize, ResidualProductLocusBooleanCoverError> {
    let fact_entries = checked_add("DPLL state bytes", zero_facts, nonzero_facts)?;
    let payload_entries = checked_add("DPLL state bytes", fact_entries, literals)?;
    let payload_bytes = payload_entries.checked_mul(size_of::<usize>()).ok_or(
        ResidualProductLocusBooleanCoverError::ResourceCountOverflow {
            resource: "DPLL state bytes",
        },
    )?;
    let clause_header_bytes = clauses.checked_mul(size_of::<Vec<usize>>()).ok_or(
        ResidualProductLocusBooleanCoverError::ResourceCountOverflow {
            resource: "DPLL state bytes",
        },
    )?;
    checked_add(
        "DPLL state bytes",
        size_of::<BooleanState>(),
        checked_add("DPLL state bytes", clause_header_bytes, payload_bytes)?,
    )
}

fn simplification_scratch_bytes(
    state: &BooleanState,
    maximum_new_propagations: usize,
) -> Result<usize, ResidualProductLocusBooleanCoverError> {
    let literals = state.clauses.iter().try_fold(0usize, |sum, clause| {
        checked_add("DPLL simplification scratch bytes", sum, clause.len())
    })?;
    // While canonicalizing, the original state, one complete simplified copy,
    // and a retained outer clause vector may coexist. This function charges
    // *additional scratch* beyond the original state, whose bytes are checked
    // separately as retained/frontier storage. Thus the two additional outer
    // arrays need six `Vec<usize>` header words per clause. Exhausted-clause
    // witnesses can additionally clone one clause, hence two literal payload
    // copies are conservative. Coordinate facts
    // use three machine words each; every atom may become one zero fact and
    // one propagation transcript entry.
    let clause_words = state.clauses.len().checked_mul(6).ok_or(
        ResidualProductLocusBooleanCoverError::ResourceCountOverflow {
            resource: "DPLL simplification scratch bytes",
        },
    )?;
    let literal_words = literals.checked_mul(2).ok_or(
        ResidualProductLocusBooleanCoverError::ResourceCountOverflow {
            resource: "DPLL simplification scratch bytes",
        },
    )?;
    let coordinate_facts = checked_add(
        "DPLL simplification scratch bytes",
        state.zero.len(),
        maximum_new_propagations,
    )?;
    let coordinate_words = coordinate_facts.checked_mul(3).ok_or(
        ResidualProductLocusBooleanCoverError::ResourceCountOverflow {
            resource: "DPLL simplification scratch bytes",
        },
    )?;
    let words = [
        clause_words,
        literal_words,
        coordinate_words,
        maximum_new_propagations,
        16,
    ]
    .into_iter()
    .try_fold(0usize, |sum, value| {
        checked_add("DPLL simplification scratch bytes", sum, value)
    })?;
    words.checked_mul(size_of::<usize>()).ok_or(
        ResidualProductLocusBooleanCoverError::ResourceCountOverflow {
            resource: "DPLL simplification scratch bytes",
        },
    )
}

/// Allocator-independent control/alignment envelope for the `Arc` which seals
/// one fixed-size cover certificate.  The certificate payload itself is
/// charged separately by the caller.
fn fixed_arc_control_and_padding_bytes<T>() -> Result<usize, ResidualProductLocusBooleanCoverError>
{
    checked_add(
        "Boolean cover retained Arc bytes",
        checked_mul("Boolean cover retained Arc bytes", 2, size_of::<usize>())?,
        align_of::<T>().saturating_sub(1),
    )
}

/// Conservative allocation envelope for one dynamically sized `Arc<str>`.
/// Two reference-count words and one word of header/alignment slack are added
/// to the exact initialized string payload.
fn arc_str_allocation_byte_upper_bound(
    length: usize,
) -> Result<usize, ResidualProductLocusBooleanCoverError> {
    checked_add(
        "Boolean cover retained fingerprint bytes",
        length,
        checked_mul(
            "Boolean cover retained fingerprint bytes",
            3,
            size_of::<usize>(),
        )?,
    )
}

fn cover_retained_owned_logical_bytes_upper_bound(
    certificate: &ResidualProductLocusBooleanCoverCertificate,
) -> Result<usize, ResidualProductLocusBooleanCoverError> {
    let limits = certificate.limits;
    // Bound every outer shape before walking nested clause/node payloads. The
    // certificate is currently sealed and non-deserializable, but replay uses
    // this as an authentication primitive and must remain safe under future
    // persistence or in-module tampering.
    check_limit(
        "family fingerprint bytes",
        certificate.family_fingerprint.len(),
        limits.max_family_fingerprint_bytes,
    )?;
    check_limit(
        "context fingerprint bytes",
        certificate.context_fingerprint.len(),
        limits.max_context_fingerprint_bytes,
    )?;
    check_limit(
        "Boolean cover sector arity",
        certificate.sector.arity(),
        limits.max_sector_arity,
    )?;
    check_limit("Boolean atoms", certificate.atoms.len(), limits.max_atoms)?;
    check_limit(
        "root clauses",
        certificate.root_clauses.len(),
        limits.max_root_clauses,
    )?;
    check_limit(
        "root nonzero facts",
        certificate.root_nonzero_atoms.len(),
        limits.max_root_nonzero_facts,
    )?;
    check_limit("DPLL nodes", certificate.nodes.len(), limits.max_dpll_nodes)?;
    let root_literals = certificate
        .root_clauses
        .iter()
        .try_fold(0usize, |sum, clause| {
            let next = checked_add("Boolean cover retained root literals", sum, clause.len())?;
            check_limit("root literals", next, limits.max_root_literals)?;
            Ok::<usize, ResidualProductLocusBooleanCoverError>(next)
        })?;
    let mut path_facts = 0usize;
    let mut propagations = 0usize;
    let mut node_clauses = 0usize;
    let mut node_literals = 0usize;
    let mut empty_reason_ordinals = 0usize;
    for node in &certificate.nodes {
        path_facts = checked_add(
            "Boolean cover retained path facts",
            path_facts,
            checked_add(
                "Boolean cover retained path facts",
                node.equal_zero_atoms.len(),
                node.nonzero_atoms.len(),
            )?,
        )?;
        check_limit("path facts", path_facts, limits.max_path_facts)?;
        propagations = checked_add(
            "Boolean cover retained propagations",
            propagations,
            node.propagated_zero_atoms.len(),
        )?;
        check_limit(
            "retained propagation entries",
            propagations,
            limits.max_retained_propagation_entries,
        )?;
        node_clauses = checked_add(
            "Boolean cover retained node clauses",
            node_clauses,
            node.remaining_clauses.len(),
        )?;
        check_limit(
            "retained node clauses",
            node_clauses,
            limits.max_retained_node_clauses,
        )?;
        for clause in &node.remaining_clauses {
            node_literals = checked_add(
                "Boolean cover retained node literals",
                node_literals,
                clause.len(),
            )?;
            check_limit(
                "retained node literals",
                node_literals,
                limits.max_retained_node_literals,
            )?;
        }
        if let ResidualProductLocusBooleanNodeOutcome::ProvedEmpty(
            ResidualProductLocusBooleanEmptyReason::ExhaustedClause {
                atom_locus_ordinals,
            },
        ) = &node.outcome
        {
            empty_reason_ordinals = checked_add(
                "Boolean cover retained empty-reason ordinals",
                empty_reason_ordinals,
                atom_locus_ordinals.len(),
            )?;
            check_limit(
                "retained empty-reason ordinals",
                empty_reason_ordinals,
                limits.max_retained_empty_reason_ordinals,
            )?;
        }
    }

    cover_retained_owned_logical_bytes_upper_bound_from_shape(
        certificate.family_fingerprint.len(),
        certificate.context_fingerprint.len(),
        certificate.sector.owned_retained_byte_bound().ok_or(
            ResidualProductLocusBooleanCoverError::ResourceCountOverflow {
                resource: "Boolean cover retained sector bytes",
            },
        )?,
        certificate.atoms.len(),
        certificate.root_clauses.len(),
        root_literals,
        certificate.root_nonzero_atoms.len(),
        certificate.nodes.len(),
        path_facts,
        propagations,
        node_clauses,
        node_literals,
        empty_reason_ordinals,
    )
}

#[allow(clippy::too_many_arguments)]
fn cover_retained_owned_logical_bytes_upper_bound_from_shape(
    family_fingerprint_bytes: usize,
    context_fingerprint_bytes: usize,
    sector_owned_bytes: usize,
    atoms: usize,
    root_clauses: usize,
    root_literals: usize,
    root_nonzero_atoms: usize,
    nodes: usize,
    path_facts: usize,
    propagations: usize,
    node_clauses: usize,
    node_literals: usize,
    empty_reason_ordinals: usize,
) -> Result<usize, ResidualProductLocusBooleanCoverError> {
    // `Vec<usize>` is deliberately used as the clause-header envelope.  The
    // retained representation stores `Box<[usize]>`, but compilation may have
    // the larger `Vec` header arrays alive while boxed outer arrays are built.
    let clause_header_bytes = size_of::<Vec<usize>>().max(size_of::<Box<[usize]>>());
    [
        fixed_arc_control_and_padding_bytes::<ResidualProductLocusBooleanCoverCertificate>()?,
        size_of::<ResidualProductLocusBooleanCoverCertificate>(),
        arc_str_allocation_byte_upper_bound(family_fingerprint_bytes)?,
        arc_str_allocation_byte_upper_bound(context_fingerprint_bytes)?,
        sector_owned_bytes,
        checked_mul("Boolean cover retained atoms", atoms, size_of::<usize>())?,
        checked_mul(
            "Boolean cover retained root clause headers",
            root_clauses,
            clause_header_bytes,
        )?,
        checked_mul(
            "Boolean cover retained root literals",
            root_literals,
            size_of::<usize>(),
        )?,
        checked_mul(
            "Boolean cover retained root nonzero atoms",
            root_nonzero_atoms,
            size_of::<usize>(),
        )?,
        checked_mul(
            "Boolean cover retained nodes",
            nodes,
            size_of::<ResidualProductLocusBooleanNode>(),
        )?,
        checked_mul(
            "Boolean cover retained path facts",
            path_facts,
            size_of::<usize>(),
        )?,
        checked_mul(
            "Boolean cover retained propagations",
            propagations,
            size_of::<usize>(),
        )?,
        checked_mul(
            "Boolean cover retained node clause headers",
            node_clauses,
            clause_header_bytes,
        )?,
        checked_mul(
            "Boolean cover retained node literals",
            node_literals,
            size_of::<usize>(),
        )?,
        checked_mul(
            "Boolean cover retained empty-reason ordinals",
            empty_reason_ordinals,
            size_of::<usize>(),
        )?,
    ]
    .into_iter()
    .try_fold(0usize, |sum, value| {
        checked_add("Boolean cover retained owned bytes", sum, value)
    })
}

fn cover_compilation_owned_logical_peak_upper_bound(
    certificate: &ResidualProductLocusBooleanCoverCertificate,
    transient: ResidualProductLocusBooleanRawTransientCensus,
) -> Result<usize, ResidualProductLocusBooleanCoverError> {
    let root_build_bytes = root_build_owned_logical_bytes_upper_bound(
        transient.initial_clause_reserve_entries,
        transient.initial_nonzero_reserve_entries,
        transient.raw_root_clauses,
        transient.raw_root_literals,
        transient.raw_root_nonzero_facts,
    )?;
    compilation_owned_logical_peak_upper_bound_from_shape(
        cover_retained_owned_logical_bytes_upper_bound(certificate)?,
        root_build_bytes,
        certificate.atoms.len(),
        certificate.stats.peak_frontier_bytes,
        certificate.stats.peak_simplification_scratch_bytes,
    )
}

fn root_build_owned_logical_bytes_upper_bound(
    initial_clause_reserve_entries: usize,
    initial_nonzero_reserve_entries: usize,
    raw_root_clauses: usize,
    raw_root_literals: usize,
    raw_root_nonzero_facts: usize,
) -> Result<usize, ResidualProductLocusBooleanCoverError> {
    // Canonicalization reserves a second outer clause array before consuming
    // the raw one. Literal buffers move between those arrays, so one raw
    // literal payload suffices. Initial reserves can exceed initialized raw
    // lengths and remain live until the root vectors are consumed.
    let raw_clause_outer_entries = initial_clause_reserve_entries.max(raw_root_clauses);
    let raw_nonzero_entries = initial_nonzero_reserve_entries.max(raw_root_nonzero_facts);
    [
        checked_mul(
            "Boolean cover raw root clause headers",
            raw_clause_outer_entries,
            size_of::<Vec<usize>>(),
        )?,
        checked_mul(
            "Boolean cover canonical root clause headers",
            raw_root_clauses,
            size_of::<Vec<usize>>(),
        )?,
        checked_mul(
            "Boolean cover raw root literals",
            raw_root_literals,
            size_of::<usize>(),
        )?,
        checked_mul(
            "Boolean cover raw root nonzero facts",
            raw_nonzero_entries,
            size_of::<usize>(),
        )?,
    ]
    .into_iter()
    .try_fold(0usize, |sum, value| {
        checked_add("Boolean cover raw root build bytes", sum, value)
    })
}

fn compilation_owned_logical_peak_upper_bound_from_shape(
    retained_owned_bytes: usize,
    root_build_bytes: usize,
    atoms: usize,
    peak_frontier_bytes: usize,
    peak_simplification_scratch_bytes: usize,
) -> Result<usize, ResidualProductLocusBooleanCoverError> {
    // Two complete retained-shape envelopes conservatively cover the retained
    // output and node/root boxing overlap. The separately instrumented raw
    // root build is essential: canonicalization can discard arbitrarily many
    // duplicate/subsumed entries, so final V1 root stats alone are not a peak
    // bound. Coordinates are compile-only. Frontier and simplification scratch
    // can coexist with the retained prefix and are charged independently.
    [
        checked_mul(
            "Boolean cover compilation peak upper bound",
            retained_owned_bytes,
            2,
        )?,
        root_build_bytes,
        checked_mul(
            "Boolean cover compilation coordinate metadata",
            atoms,
            size_of::<(usize, Option<(usize, i64)>)>(),
        )?,
        peak_frontier_bytes,
        peak_simplification_scratch_bytes,
    ]
    .into_iter()
    .try_fold(0usize, |sum, value| {
        checked_add("Boolean cover compilation peak upper bound", sum, value)
    })
}

/// Preflight envelope for a child whose concrete retained shape has not yet
/// been built. Every shape coordinate comes from the existing V1 hard limits,
/// so a source-neutral parent can reject its own aggregate memory bound before
/// invoking child compilation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ResidualProductLocusBooleanMemoryEnvelope {
    retained_owned_logical_bytes_upper_bound: usize,
    compilation_owned_logical_peak_upper_bound: usize,
}

impl ResidualProductLocusBooleanMemoryEnvelope {
    pub(crate) const fn retained_owned_logical_bytes_upper_bound(self) -> usize {
        self.retained_owned_logical_bytes_upper_bound
    }

    pub(crate) const fn compilation_owned_logical_peak_upper_bound(self) -> usize {
        self.compilation_owned_logical_peak_upper_bound
    }
}

pub(crate) fn residual_product_locus_boolean_memory_envelope_from_limits(
    limits: ResidualProductLocusBooleanCoverLimits,
) -> Result<ResidualProductLocusBooleanMemoryEnvelope, ResidualProductLocusBooleanCoverError> {
    let sector_owned_bytes = checked_mul(
        "Boolean cover retained sector bytes",
        limits.max_sector_arity,
        size_of::<bool>(),
    )?;
    let retained = cover_retained_owned_logical_bytes_upper_bound_from_shape(
        limits.max_family_fingerprint_bytes,
        limits.max_context_fingerprint_bytes,
        sector_owned_bytes,
        limits.max_atoms,
        limits.max_root_clauses,
        limits.max_root_literals,
        limits.max_root_nonzero_facts,
        limits.max_dpll_nodes,
        limits.max_path_facts,
        limits.max_retained_propagation_entries,
        limits.max_retained_node_clauses,
        limits.max_retained_node_literals,
        limits.max_retained_empty_reason_ordinals,
    )?;
    let root_build = root_build_owned_logical_bytes_upper_bound(
        limits.max_source_predicates.min(limits.max_root_clauses),
        limits
            .max_source_predicates
            .min(limits.max_root_nonzero_facts),
        limits.max_root_clauses,
        limits.max_root_literals,
        limits.max_root_nonzero_facts,
    )?;
    let peak = compilation_owned_logical_peak_upper_bound_from_shape(
        retained,
        root_build,
        limits.max_atoms,
        limits.max_peak_frontier_bytes,
        limits.max_peak_simplification_scratch_bytes,
    )?;
    Ok(ResidualProductLocusBooleanMemoryEnvelope {
        retained_owned_logical_bytes_upper_bound: retained,
        compilation_owned_logical_peak_upper_bound: peak,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CoverOwnedPayloadComparisonCensus {
    /// Prospective scalar/container comparison work for two independently
    /// allocated equal cover payloads.
    units: usize,
    /// Prospective logical bytes read from those two cover-owned payloads.
    bytes: usize,
}

/// Fixed scalar/discriminant allowance for one cover. This includes the
/// schema, source ordinals, limits, statistics, node scalar metadata, and Arc
/// pointer checks. Variable-size payloads are added exactly below.
const COVER_PAYLOAD_FIXED_COMPARISON_UNITS: usize = 128;
const BOOLEAN_NODE_FIXED_COMPARISON_UNITS: usize = 16;

fn preflight_cover_owned_payload_comparison(
    retained: &ResidualProductLocusBooleanCoverCertificate,
    supplied: &ResidualProductLocusBooleanCoverCertificate,
) -> Result<(), ResidualProductLocusBooleanCoverError> {
    let limits = retained.limits;
    let retained_census = cover_owned_payload_comparison_census(retained, limits)?;
    let supplied_census = cover_owned_payload_comparison_census(supplied, limits)?;
    check_limit(
        "payload comparison units",
        retained_census.units.max(supplied_census.units),
        limits.max_payload_comparison_units,
    )?;
    check_limit(
        "payload comparison bytes",
        retained_census.bytes.max(supplied_census.bytes),
        limits.max_payload_comparison_bytes,
    )
}

/// Compute the complete cover-local comparison shape without allocating and
/// without reading any nested queue/extraction proof payload. Before walking
/// clause headers or nodes, the corresponding outer counts are checked against
/// both their construction bounds and a minimum comparison-phase charge.
fn cover_owned_payload_comparison_census(
    certificate: &ResidualProductLocusBooleanCoverCertificate,
    limits: ResidualProductLocusBooleanCoverLimits,
) -> Result<CoverOwnedPayloadComparisonCensus, ResidualProductLocusBooleanCoverError> {
    check_limit(
        "family fingerprint bytes",
        certificate.family_fingerprint.len(),
        limits.max_family_fingerprint_bytes,
    )?;
    check_limit(
        "context fingerprint bytes",
        certificate.context_fingerprint.len(),
        limits.max_context_fingerprint_bytes,
    )?;
    check_limit(
        "Boolean cover sector arity",
        certificate.sector.arity(),
        limits.max_sector_arity,
    )?;
    check_limit("Boolean atoms", certificate.atoms.len(), limits.max_atoms)?;
    check_limit(
        "root clauses",
        certificate.root_clauses.len(),
        limits.max_root_clauses,
    )?;
    check_limit(
        "root nonzero facts",
        certificate.root_nonzero_atoms.len(),
        limits.max_root_nonzero_facts,
    )?;
    check_limit("DPLL nodes", certificate.nodes.len(), limits.max_dpll_nodes)?;

    let minimum_one_side_units = [
        COVER_PAYLOAD_FIXED_COMPARISON_UNITS,
        certificate.sector.arity(),
        certificate.atoms.len(),
        certificate.root_clauses.len(),
        certificate.root_nonzero_atoms.len(),
        checked_mul(
            "payload comparison units",
            certificate.nodes.len(),
            BOOLEAN_NODE_FIXED_COMPARISON_UNITS,
        )?,
    ]
    .into_iter()
    .try_fold(0usize, |sum, value| {
        checked_add("payload comparison units", sum, value)
    })?;
    check_limit(
        "payload comparison units",
        checked_mul("payload comparison units", minimum_one_side_units, 2)?,
        limits.max_payload_comparison_units,
    )?;

    let minimum_one_side_bytes = [
        size_of::<ResidualProductLocusBooleanCoverCertificate>(),
        certificate.schema.len(),
        certificate.family_fingerprint.len(),
        certificate.context_fingerprint.len(),
        checked_mul(
            "payload comparison bytes",
            certificate.sector.arity(),
            size_of::<bool>(),
        )?,
        checked_mul(
            "payload comparison bytes",
            certificate.atoms.len(),
            size_of::<usize>(),
        )?,
        checked_mul(
            "payload comparison bytes",
            certificate.root_clauses.len(),
            size_of::<Box<[usize]>>(),
        )?,
        checked_mul(
            "payload comparison bytes",
            certificate.root_nonzero_atoms.len(),
            size_of::<usize>(),
        )?,
        checked_mul(
            "payload comparison bytes",
            certificate.nodes.len(),
            size_of::<ResidualProductLocusBooleanNode>(),
        )?,
    ]
    .into_iter()
    .try_fold(0usize, |sum, value| {
        checked_add("payload comparison bytes", sum, value)
    })?;
    check_limit(
        "payload comparison bytes",
        checked_mul("payload comparison bytes", minimum_one_side_bytes, 2)?,
        limits.max_payload_comparison_bytes,
    )?;

    let root_literals = certificate
        .root_clauses
        .iter()
        .try_fold(0usize, |sum, clause| {
            bounded_add("root literals", sum, clause.len(), limits.max_root_literals)
        })?;
    let mut path_facts = 0usize;
    let mut propagation_entries = 0usize;
    let mut node_clauses = 0usize;
    let mut node_literals = 0usize;
    let mut empty_reason_ordinals = 0usize;
    let mut terminals = 0usize;
    for node in &certificate.nodes {
        path_facts = bounded_add(
            "retained path facts",
            path_facts,
            checked_add(
                "retained path facts",
                node.equal_zero_atoms.len(),
                node.nonzero_atoms.len(),
            )?,
            limits.max_path_facts,
        )?;
        propagation_entries = bounded_add(
            "retained DPLL propagation entries",
            propagation_entries,
            node.propagated_zero_atoms.len(),
            limits.max_retained_propagation_entries,
        )?;
        node_clauses = bounded_add(
            "retained node clauses",
            node_clauses,
            node.remaining_clauses.len(),
            limits.max_retained_node_clauses,
        )?;
        check_limit(
            "payload comparison units",
            checked_mul(
                "payload comparison units",
                checked_add(
                    "payload comparison units",
                    minimum_one_side_units,
                    node_clauses,
                )?,
                2,
            )?,
            limits.max_payload_comparison_units,
        )?;
        check_limit(
            "payload comparison bytes",
            checked_mul(
                "payload comparison bytes",
                checked_add(
                    "payload comparison bytes",
                    minimum_one_side_bytes,
                    checked_mul(
                        "payload comparison bytes",
                        node_clauses,
                        size_of::<Box<[usize]>>(),
                    )?,
                )?,
                2,
            )?,
            limits.max_payload_comparison_bytes,
        )?;
        for clause in &node.remaining_clauses {
            node_literals = bounded_add(
                "retained node literals",
                node_literals,
                clause.len(),
                limits.max_retained_node_literals,
            )?;
        }
        if let ResidualProductLocusBooleanNodeOutcome::ProvedEmpty(
            ResidualProductLocusBooleanEmptyReason::ExhaustedClause {
                atom_locus_ordinals,
            },
        ) = &node.outcome
        {
            empty_reason_ordinals = bounded_add(
                "retained empty-reason ordinals",
                empty_reason_ordinals,
                atom_locus_ordinals.len(),
                limits.max_retained_empty_reason_ordinals,
            )?;
        }
        if node.is_terminal() {
            terminals = bounded_add("Boolean terminals", terminals, 1, limits.max_terminals)?;
        }
    }

    let one_side_units = [
        minimum_one_side_units,
        root_literals,
        path_facts,
        propagation_entries,
        node_clauses,
        node_literals,
        empty_reason_ordinals,
        terminals,
    ]
    .into_iter()
    .try_fold(0usize, |sum, value| {
        checked_add("payload comparison units", sum, value)
    })?;
    let units = checked_mul("payload comparison units", one_side_units, 2)?;
    check_limit(
        "payload comparison units",
        units,
        limits.max_payload_comparison_units,
    )?;

    let variable_usize_entries = [
        root_literals,
        path_facts,
        propagation_entries,
        node_literals,
        empty_reason_ordinals,
    ]
    .into_iter()
    .try_fold(0usize, |sum, value| {
        checked_add("payload comparison bytes", sum, value)
    })?;
    let one_side_bytes = checked_add(
        "payload comparison bytes",
        minimum_one_side_bytes,
        checked_add(
            "payload comparison bytes",
            checked_mul(
                "payload comparison bytes",
                variable_usize_entries,
                size_of::<usize>(),
            )?,
            checked_mul(
                "payload comparison bytes",
                node_clauses,
                size_of::<Box<[usize]>>(),
            )?,
        )?,
    )?;
    let bytes = checked_mul("payload comparison bytes", one_side_bytes, 2)?;
    check_limit(
        "payload comparison bytes",
        bytes,
        limits.max_payload_comparison_bytes,
    )?;

    Ok(CoverOwnedPayloadComparisonCensus { units, bytes })
}

fn validate_scope(
    schema: &'static str,
    family_fingerprint: &str,
    context_fingerprint: &str,
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    limits: ResidualProductLocusBooleanCoverLimits,
) -> Result<(), ResidualProductLocusBooleanCoverError> {
    if schema != RESIDUAL_PRODUCT_LOCUS_BOOLEAN_COVER_V1_SCHEMA {
        return Err(ResidualProductLocusBooleanCoverError::SchemaMismatch);
    }
    check_limit(
        "family fingerprint bytes",
        family_fingerprint.len(),
        limits.max_family_fingerprint_bytes,
    )?;
    check_limit(
        "family fingerprint bytes",
        family.fingerprint_ref().len(),
        limits.max_family_fingerprint_bytes,
    )?;
    check_limit(
        "context fingerprint bytes",
        context_fingerprint.len(),
        limits.max_context_fingerprint_bytes,
    )?;
    check_limit(
        "context fingerprint bytes",
        context.fingerprint().len(),
        limits.max_context_fingerprint_bytes,
    )?;
    if family_fingerprint != family.fingerprint_ref() {
        return Err(ResidualProductLocusBooleanCoverError::WrongFamily);
    }
    if context_fingerprint != context.fingerprint() {
        return Err(ResidualProductLocusBooleanCoverError::WrongContext);
    }
    Ok(())
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ResidualProductLocusBooleanCoverError> {
    left.checked_add(right)
        .ok_or(ResidualProductLocusBooleanCoverError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ResidualProductLocusBooleanCoverError> {
    left.checked_mul(right)
        .ok_or(ResidualProductLocusBooleanCoverError::ResourceCountOverflow { resource })
}

fn bounded_add(
    resource: &'static str,
    current: usize,
    additional: usize,
    limit: usize,
) -> Result<usize, ResidualProductLocusBooleanCoverError> {
    let requested = checked_add(resource, current, additional)?;
    check_limit(resource, requested, limit)?;
    Ok(requested)
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ResidualProductLocusBooleanCoverError> {
    if requested > limit {
        Err(ResidualProductLocusBooleanCoverError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AffineDenominator, CoordinateEqualityLocusExtractor, GeneratedSectorDiscoveryCompiler,
        GeneratedSectorDiscoveryLimits, GeneratedSectorLiveLeafQueueCompiler,
        GeneratedSectorLiveLeafQueueLimits, IntegralOrderingPolicy, ParametricIbpGenerator,
        algebra::CoefficientContext,
    };

    fn generated_sunset_cover() -> (
        IntegralFamily,
        ParametricCoefficientContext,
        ResidualProductLocusBooleanCoverCertificate,
    ) {
        let coefficients = CoefficientContext::new(["d", "m2"]);
        let zero = coefficients.zero();
        let one = coefficients.one();
        let minus_m2 = coefficients.parse("-m2").unwrap();
        let family = IntegralFamily::new(
            "boolean-product-locus-unit-tamper",
            vec!["k1".into(), "k2".into()],
            Vec::new(),
            coefficients.clone(),
            coefficients.parameter("d").unwrap(),
            vec![
                AffineDenominator::new(
                    minus_m2.clone(),
                    vec![one.clone(), zero.clone(), zero.clone()],
                ),
                AffineDenominator::new(
                    minus_m2.clone(),
                    vec![zero.clone(), zero.clone(), one.clone()],
                ),
                AffineDenominator::new(minus_m2, vec![one.clone(), coefficients.integer(2), one]),
            ],
            Vec::new(),
            vec![zero.clone(), zero.clone(), zero],
        )
        .unwrap();
        let context = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .context()
            .clone();
        let mut discovery_limits = GeneratedSectorDiscoveryLimits::default();
        discovery_limits.adaptive.max_search_depth = 0;
        let discovery = GeneratedSectorDiscoveryCompiler::compile(
            &family,
            &context,
            SectorMask::try_from_bit_string("111").unwrap(),
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            discovery_limits,
        )
        .unwrap();
        let mut queue_limits = GeneratedSectorLiveLeafQueueLimits::default();
        queue_limits.translation_radius = 0;
        queue_limits.max_translation_points = 1;
        let queue = Arc::new(
            GeneratedSectorLiveLeafQueueCompiler::compile(
                &family,
                &context,
                &discovery,
                queue_limits,
            )
            .unwrap(),
        );
        let cover = ResidualProductLocusBooleanCoverCompiler::compile(
            &family,
            &context,
            queue,
            0,
            ResidualProductLocusBooleanCoverLimits::default(),
        )
        .unwrap();
        (family, context, cover)
    }

    fn synthetic(
        clauses: &[&[usize]],
        nonzero: &[usize],
        coordinates: &[(usize, Option<(usize, i64)>)],
    ) -> (
        Vec<ResidualProductLocusBooleanNode>,
        ResidualProductLocusBooleanCoverStats,
    ) {
        let mut stats = ResidualProductLocusBooleanCoverStats::default();
        let clauses = canonicalize_clauses(
            clauses.iter().map(|clause| clause.to_vec()).collect(),
            &mut stats,
            ResidualProductLocusBooleanCoverLimits::default(),
        )
        .unwrap();
        let mut nonzero = nonzero.to_vec();
        nonzero.sort_unstable();
        nonzero.dedup();
        let state = BooleanState {
            parent_ordinal: None,
            decision_from_parent: None,
            depth: 0,
            zero: Vec::new(),
            nonzero,
            clauses,
        };
        let sector = SectorMask::try_from_bit_string("11").unwrap();
        let nodes = build_dpll_cover(
            state,
            &sector,
            coordinates,
            &mut stats,
            ResidualProductLocusBooleanCoverLimits::default(),
        )
        .unwrap();
        (nodes, stats)
    }

    #[test]
    fn canonical_cnf_and_zero_first_dpll_are_disjoint_and_complete() {
        // (a or b) and (b or c); duplicate/superset clauses are redundant.
        let (nodes, stats) = synthetic(
            &[&[2, 1], &[1, 2], &[1, 2, 3], &[2, 3]],
            &[],
            &[(1, None), (2, None), (3, None)],
        );
        assert!(stats.duplicate_literals_removed() == 0);
        assert!(stats.duplicate_or_subsumed_clauses_removed() >= 2);
        let terminals: Vec<_> = nodes
            .iter()
            .filter(|node| {
                matches!(
                    node.outcome(),
                    ResidualProductLocusBooleanNodeOutcome::ReadyForAffineRecognition
                )
            })
            .collect();
        for mask in 0u8..8 {
            let formula =
                ((mask & 1 != 0) || (mask & 2 != 0)) && ((mask & 2 != 0) || (mask & 4 != 0));
            let matches = terminals
                .iter()
                .filter(|node| {
                    node.equal_zero_atoms()
                        .iter()
                        .all(|&a| mask & (1 << (a - 1)) != 0)
                        && node
                            .nonzero_atoms()
                            .iter()
                            .all(|&a| mask & (1 << (a - 1)) == 0)
                })
                .count();
            assert_eq!(matches, usize::from(formula), "assignment {mask:03b}");
        }
        let first_branch = nodes
            .iter()
            .find_map(|node| match node.outcome() {
                ResidualProductLocusBooleanNodeOutcome::Branched { atom_locus_ordinal } => {
                    Some(*atom_locus_ordinal)
                }
                _ => None,
            })
            .unwrap();
        assert_eq!(first_branch, 1);
    }

    #[test]
    fn base_units_and_product_nonzero_atoms_have_exact_truth_semantics() {
        assert_eq!(
            resolve_base_field_predicate(SymbolicPolynomialPredicateKind::EqualZero, true),
            BaseFieldPredicateResolution::Discharge
        );
        assert_eq!(
            resolve_base_field_predicate(SymbolicPolynomialPredicateKind::NonZero, false),
            BaseFieldPredicateResolution::Discharge
        );
        assert_eq!(
            resolve_base_field_predicate(SymbolicPolynomialPredicateKind::EqualZero, false),
            BaseFieldPredicateResolution::Contradiction
        );
        assert_eq!(
            resolve_base_field_predicate(SymbolicPolynomialPredicateKind::NonZero, true),
            BaseFieldPredicateResolution::Contradiction
        );

        let product = 9usize;
        let factors = [2usize, 5usize];
        assert_eq!(product_predicate_atoms(&product, Some(&factors)), factors);
        assert_eq!(product_predicate_atoms(&product, None), [product]);
        let (nodes, _) = synthetic(&[], &factors, &[(2, None), (5, None)]);
        let ready: Vec<_> = nodes
            .iter()
            .filter(|node| {
                matches!(
                    node.outcome(),
                    ResidualProductLocusBooleanNodeOutcome::ReadyForAffineRecognition
                )
            })
            .collect();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].nonzero_atoms(), factors);
        assert!(ready[0].equal_zero_atoms().is_empty());
    }

    #[test]
    fn dpll_nodes_retain_complete_ancestry_decisions_and_propagations() {
        let (nodes, _) = synthetic(&[&[1, 2], &[2, 3]], &[], &[(1, None), (2, None), (3, None)]);
        assert!(!nodes.is_empty());
        for (ordinal, node) in nodes.iter().enumerate() {
            assert_eq!(node.ordinal(), ordinal);
            match (node.parent_ordinal(), node.decision_from_parent()) {
                (None, None) => assert_eq!(ordinal, 0),
                (Some(parent), Some(decision)) => {
                    assert!(parent < ordinal);
                    assert_eq!(node.depth(), nodes[parent].depth() + 1);
                    match decision.polarity() {
                        ResidualProductLocusBooleanPolarity::EqualZero => assert!(
                            node.equal_zero_atoms()
                                .binary_search(&decision.atom_locus_ordinal())
                                .is_ok()
                        ),
                        ResidualProductLocusBooleanPolarity::NonZero => assert!(
                            node.nonzero_atoms()
                                .binary_search(&decision.atom_locus_ordinal())
                                .is_ok()
                        ),
                    }
                }
                _ => panic!("parent and incoming decision must be retained together"),
            }
            assert!(
                node.propagated_zero_atoms()
                    .windows(2)
                    .all(|pair| pair[0] != pair[1])
            );
            assert!(
                node.propagated_zero_atoms()
                    .iter()
                    .all(|atom| { node.equal_zero_atoms().binary_search(atom).is_ok() })
            );
        }
        assert!(
            nodes
                .iter()
                .any(|node| !node.propagated_zero_atoms().is_empty())
        );
        for node in &nodes {
            if let ResidualProductLocusBooleanNodeOutcome::Branched { atom_locus_ordinal } =
                node.outcome()
            {
                let children: Vec<_> = nodes
                    .iter()
                    .filter(|candidate| candidate.parent_ordinal() == Some(node.ordinal()))
                    .collect();
                assert_eq!(children.len(), 2);
                let mut polarities: Vec<_> = children
                    .iter()
                    .map(|child| child.decision_from_parent().unwrap())
                    .collect();
                polarities.sort_by_key(|decision| match decision.polarity() {
                    ResidualProductLocusBooleanPolarity::EqualZero => 0,
                    ResidualProductLocusBooleanPolarity::NonZero => 1,
                });
                assert_eq!(polarities[0].atom_locus_ordinal(), *atom_locus_ordinal);
                assert_eq!(polarities[1].atom_locus_ordinal(), *atom_locus_ordinal);
                assert_eq!(
                    polarities[0].polarity(),
                    ResidualProductLocusBooleanPolarity::EqualZero
                );
                assert_eq!(
                    polarities[1].polarity(),
                    ResidualProductLocusBooleanPolarity::NonZero
                );
            }
        }
    }

    #[test]
    fn propagation_and_coordinate_pruning_have_typed_empty_outcomes() {
        let (nodes, _) = synthetic(&[&[1], &[2]], &[], &[(1, Some((0, 1))), (2, Some((0, 2)))]);
        assert!(matches!(
            nodes.last().unwrap().outcome(),
            ResidualProductLocusBooleanNodeOutcome::ProvedEmpty(
                ResidualProductLocusBooleanEmptyReason::ConflictingCoordinateEqualities { .. }
            )
        ));

        let (nodes, _) = synthetic(&[&[1]], &[], &[(1, Some((0, 0)))]);
        assert!(matches!(
            nodes[0].outcome(),
            ResidualProductLocusBooleanNodeOutcome::ProvedEmpty(
                ResidualProductLocusBooleanEmptyReason::OrthantViolation { .. }
            )
        ));
    }

    #[test]
    fn prospective_node_budget_fails_before_child_clones() {
        let mut limits = ResidualProductLocusBooleanCoverLimits::default();
        limits.max_dpll_nodes = 2;
        let mut stats = ResidualProductLocusBooleanCoverStats::default();
        let state = BooleanState {
            parent_ordinal: None,
            decision_from_parent: None,
            depth: 0,
            zero: Vec::new(),
            nonzero: Vec::new(),
            clauses: vec![vec![1, 2]],
        };
        assert!(matches!(
            build_dpll_cover(
                state,
                &SectorMask::try_from_bit_string("11").unwrap(),
                &[(1, None), (2, None)],
                &mut stats,
                limits,
            ),
            Err(ResidualProductLocusBooleanCoverError::ResourceLimit {
                resource: "DPLL nodes",
                requested: 3,
                limit: 2,
            })
        ));
    }

    #[test]
    fn repaired_atom_retention_and_simplification_limits_fail_one_below() {
        let mut atoms = Vec::new();
        let mut stats = ResidualProductLocusBooleanCoverStats::default();
        let mut atom_limits = ResidualProductLocusBooleanCoverLimits::default();
        atom_limits.max_atoms = 0;
        assert!(matches!(
            insert_unique_atom(&mut atoms, 7, &mut stats, atom_limits),
            Err(ResidualProductLocusBooleanCoverError::ResourceLimit {
                resource: "Boolean atoms",
                requested: 1,
                limit: 0,
            })
        ));
        assert!(atoms.is_empty());

        insert_unique_atom(
            &mut atoms,
            7,
            &mut stats,
            ResidualProductLocusBooleanCoverLimits::default(),
        )
        .unwrap();
        let mut comparison_limits = ResidualProductLocusBooleanCoverLimits::default();
        comparison_limits.max_atom_lookup_comparisons = 0;
        assert!(matches!(
            insert_unique_atom(&mut atoms, 7, &mut stats, comparison_limits),
            Err(ResidualProductLocusBooleanCoverError::ResourceLimit {
                resource: "Boolean atom lookup comparisons",
                requested: 1,
                limit: 0,
            })
        ));

        let state = || BooleanState {
            parent_ordinal: None,
            decision_from_parent: None,
            depth: 0,
            zero: Vec::new(),
            nonzero: Vec::new(),
            clauses: vec![vec![1]],
        };
        let sector = SectorMask::try_from_bit_string("1").unwrap();

        let retained_cases: [(
            &'static str,
            fn(&mut ResidualProductLocusBooleanCoverLimits),
        ); 3] = [
            (
                "retained path facts",
                |limits: &mut ResidualProductLocusBooleanCoverLimits| limits.max_path_facts = 0,
            ),
            (
                "retained node clauses",
                |limits: &mut ResidualProductLocusBooleanCoverLimits| {
                    limits.max_retained_node_clauses = 0
                },
            ),
            (
                "retained node literals",
                |limits: &mut ResidualProductLocusBooleanCoverLimits| {
                    limits.max_retained_node_literals = 0
                },
            ),
        ];
        for (resource, mutate) in retained_cases {
            let mut limits = ResidualProductLocusBooleanCoverLimits::default();
            mutate(&mut limits);
            let mut stats = ResidualProductLocusBooleanCoverStats::default();
            assert!(matches!(
                build_dpll_cover(state(), &sector, &[(1, None)], &mut stats, limits),
                Err(ResidualProductLocusBooleanCoverError::ResourceLimit {
                    resource: actual,
                    ..
                }) if actual == resource
            ));
            assert_eq!(stats.propagations(), 0);
        }

        let bytes = state_bytes(&state()).unwrap();
        let mut byte_limits = ResidualProductLocusBooleanCoverLimits::default();
        byte_limits.max_retained_state_bytes = bytes - 1;
        let mut stats = ResidualProductLocusBooleanCoverStats::default();
        assert!(matches!(
            build_dpll_cover(state(), &sector, &[], &mut stats, byte_limits),
            Err(ResidualProductLocusBooleanCoverError::ResourceLimit {
                resource: "retained DPLL state bytes",
                ..
            })
        ));

        let scratch = simplification_scratch_bytes(&state(), 1).unwrap();
        let mut scratch_limits = ResidualProductLocusBooleanCoverLimits::default();
        scratch_limits.max_peak_simplification_scratch_bytes = scratch - 1;
        let mut stats = ResidualProductLocusBooleanCoverStats::default();
        assert!(matches!(
            build_dpll_cover(
                state(),
                &sector,
                &[(1, None)],
                &mut stats,
                scratch_limits,
            ),
            Err(ResidualProductLocusBooleanCoverError::ResourceLimit {
                resource: "peak DPLL simplification scratch bytes",
                requested,
                limit,
            }) if requested == scratch && limit + 1 == scratch
        ));

        let mut propagation_limits = ResidualProductLocusBooleanCoverLimits::default();
        propagation_limits.max_retained_propagation_entries = 0;
        let mut stats = ResidualProductLocusBooleanCoverStats::default();
        assert!(matches!(
            build_dpll_cover(
                state(),
                &sector,
                &[(1, None)],
                &mut stats,
                propagation_limits,
            ),
            Err(ResidualProductLocusBooleanCoverError::ResourceLimit {
                resource: "retained DPLL propagation entries",
                requested: 1,
                limit: 0,
            })
        ));
    }

    #[test]
    fn initial_frontier_and_state_shape_are_rejected_at_zero_before_push_or_clone() {
        let state = || BooleanState {
            parent_ordinal: None,
            decision_from_parent: None,
            depth: 0,
            zero: Vec::new(),
            nonzero: Vec::new(),
            clauses: vec![vec![1]],
        };
        let sector = SectorMask::try_from_bit_string("1").unwrap();

        let mut frontier_limits = ResidualProductLocusBooleanCoverLimits::default();
        frontier_limits.max_frontier_states = 0;
        let mut stats = ResidualProductLocusBooleanCoverStats::default();
        assert!(matches!(
            build_dpll_cover(state(), &sector, &[(1, None)], &mut stats, frontier_limits,),
            Err(ResidualProductLocusBooleanCoverError::ResourceLimit {
                resource: "DPLL frontier states",
                requested: 1,
                limit: 0,
            })
        ));
        assert_eq!(stats, ResidualProductLocusBooleanCoverStats::default());

        let mut clause_limits = ResidualProductLocusBooleanCoverLimits::default();
        clause_limits.max_clauses_per_state = 0;
        let mut stats = ResidualProductLocusBooleanCoverStats::default();
        assert!(matches!(
            build_dpll_cover(state(), &sector, &[(1, None)], &mut stats, clause_limits,),
            Err(ResidualProductLocusBooleanCoverError::ResourceLimit {
                resource: "clauses per DPLL state",
                requested: 1,
                limit: 0,
            })
        ));
        assert_eq!(stats, ResidualProductLocusBooleanCoverStats::default());

        let mut literal_limits = ResidualProductLocusBooleanCoverLimits::default();
        literal_limits.max_literals_per_state = 0;
        let mut stats = ResidualProductLocusBooleanCoverStats::default();
        assert!(matches!(
            build_dpll_cover(state(), &sector, &[(1, None)], &mut stats, literal_limits,),
            Err(ResidualProductLocusBooleanCoverError::ResourceLimit {
                resource: "literals per DPLL state",
                requested: 1,
                limit: 0,
            })
        ));
        assert_eq!(stats, ResidualProductLocusBooleanCoverStats::default());
    }

    #[test]
    fn certificate_replay_rejects_root_node_stats_and_source_tampering() {
        let (family, context, cover) = generated_sunset_cover();
        cover.replay(&family, &context).unwrap();
        assert!(cover.payload_eq(&cover.clone()));

        let mut coordinate_limits = ResidualProductLocusBooleanCoverLimits::default();
        coordinate_limits.max_coordinate_recognition_checks = 0;
        assert!(matches!(
            compile_replayed(
                &family,
                &context,
                cover.source_queue().clone(),
                0,
                coordinate_limits,
            ),
            Err(ResidualProductLocusBooleanCoverError::ResourceLimit {
                resource: "coordinate recognition checks",
                ..
            })
        ));

        let mut initial_bytes_limits = ResidualProductLocusBooleanCoverLimits::default();
        initial_bytes_limits.max_retained_state_bytes = 0;
        assert!(matches!(
            compile_replayed(
                &family,
                &context,
                cover.source_queue().clone(),
                0,
                initial_bytes_limits,
            ),
            Err(ResidualProductLocusBooleanCoverError::ResourceLimit {
                resource: "retained DPLL state bytes",
                ..
            })
        ));

        let mut root = cover.clone();
        root.tamper_first_root_literal_for_test();
        assert!(matches!(
            root.replay(&family, &context),
            Err(ResidualProductLocusBooleanCoverError::ReplayMismatch)
        ));

        let mut node = cover.clone();
        node.tamper_first_node_fact_for_test();
        assert!(matches!(
            node.replay(&family, &context),
            Err(ResidualProductLocusBooleanCoverError::ReplayMismatch)
        ));

        let mut stats = cover.clone();
        stats.tamper_stats_for_test();
        assert!(matches!(
            stats.replay(&family, &context),
            Err(ResidualProductLocusBooleanCoverError::ReplayMismatch)
        ));

        let mut source_ordinal = cover.clone();
        let mut source = cover;
        let alternative_case = source
            .source_queue()
            .discovery()
            .coverage()
            .partition()
            .cases()
            .iter()
            .find(|case| case.id() != source.source_case())
            .unwrap()
            .id();
        let alternative_extraction = Arc::new(
            CoordinateEqualityLocusExtractor::extract(
                &context,
                source.source_queue().discovery().coverage().partition(),
                alternative_case,
                source.source_queue().limits().coordinate_loci,
            )
            .unwrap(),
        );
        source.tamper_source_extraction_for_test(alternative_extraction);
        assert!(matches!(
            source.replay(&family, &context),
            Err(ResidualProductLocusBooleanCoverError::ReplayMismatch)
        ));

        source_ordinal.tamper_source_work_item_for_test();
        assert!(matches!(
            source_ordinal.replay(&family, &context),
            Err(ResidualProductLocusBooleanCoverError::WorkItemNotFound { .. })
        ));
    }

    #[test]
    fn replay_session_positional_compilation_matches_v1_and_rejects_wrong_binding_and_limits() {
        let (family, context, baseline) = generated_sunset_cover();
        let queue = baseline.source_queue().clone();
        let position = queue
            .discovery()
            .coverage()
            .partition()
            .cases()
            .iter()
            .position(|case| case.id() == baseline.source_case())
            .expect("retained source case has an exact partition position");
        let session =
            ResidualProductLocusBooleanReplaySession::replay(&family, &context, &queue).unwrap();
        assert!(session.authenticates_queue(&queue));

        let positional = session
            .compile_replayed_at_case_position(
                baseline.source_work_item_ordinal(),
                position,
                baseline.limits(),
            )
            .unwrap();
        let ordinary = compile_replayed(
            &family,
            &context,
            queue.clone(),
            baseline.source_work_item_ordinal(),
            baseline.limits(),
        )
        .unwrap();
        assert!(positional.payload_eq_checked(&ordinary).unwrap());
        let with_census = session
            .compile_replayed_at_case_position_with_census(
                baseline.source_work_item_ordinal(),
                position,
                baseline.limits(),
            )
            .unwrap();
        let (censused, retained_bytes, peak_bytes) = with_census.into_parts();
        assert!(censused.payload_eq_checked(&ordinary).unwrap());
        assert!(retained_bytes > 0);
        assert!(peak_bytes >= retained_bytes);
        assert!(Arc::ptr_eq(positional.source_queue(), &queue));
        assert!(Arc::ptr_eq(
            positional.source_extraction(),
            queue.work_items()[baseline.source_work_item_ordinal()].extraction_arc(),
        ));

        let wrong_position = queue
            .discovery()
            .coverage()
            .partition()
            .cases()
            .iter()
            .position(|case| case.id() != baseline.source_case())
            .expect("fixture has another partition case");
        assert!(matches!(
            session.compile_replayed_at_case_position(
                baseline.source_work_item_ordinal(),
                wrong_position,
                baseline.limits(),
            ),
            Err(ResidualProductLocusBooleanCoverError::SourceCaseNotFound { .. })
        ));

        let source_identity_bytes = queue
            .discovery()
            .coverage()
            .partition()
            .source_identity()
            .len();
        assert!(source_identity_bytes > 0);
        let mut exact = baseline.limits();
        exact.max_source_identity_bytes_referenced = source_identity_bytes;
        session
            .compile_replayed_at_case_position(baseline.source_work_item_ordinal(), position, exact)
            .unwrap();
        exact.max_source_identity_bytes_referenced = source_identity_bytes - 1;
        assert!(matches!(
            session.compile_replayed_at_case_position(
                baseline.source_work_item_ordinal(),
                position,
                exact,
            ),
            Err(ResidualProductLocusBooleanCoverError::ResourceLimit {
                resource: "source partition identity bytes referenced",
                requested,
                limit,
            }) if requested == source_identity_bytes && limit + 1 == requested
        ));

        let wrong_context = ParametricCoefficientContext::try_new(
            context.base(),
            "boolean-replay-session-wrong-context-private",
            context.index_count(),
        )
        .unwrap();
        assert!(
            ResidualProductLocusBooleanReplaySession::replay(&family, &wrong_context, &queue,)
                .is_err()
        );

        let independently_allocated = Arc::new(queue.as_ref().clone());
        assert!(!session.authenticates_queue(&independently_allocated));
    }

    #[test]
    fn checked_payload_comparison_is_exact_bounded_and_detects_late_local_difference() {
        let (family, context, baseline) = generated_sunset_cover();
        let observed = baseline.stats();
        assert!(observed.payload_comparison_units() > 0);
        assert!(observed.payload_comparison_bytes() > 0);

        let mut exact_limits = baseline.limits();
        exact_limits.max_payload_comparison_units = observed.payload_comparison_units();
        exact_limits.max_payload_comparison_bytes = observed.payload_comparison_bytes();
        let exact = compile_replayed(
            &family,
            &context,
            baseline.source_queue().clone(),
            baseline.source_work_item_ordinal(),
            exact_limits,
        )
        .unwrap();
        let independently_compiled_top_level = compile_replayed(
            &family,
            &context,
            baseline.source_queue().clone(),
            baseline.source_work_item_ordinal(),
            exact_limits,
        )
        .unwrap();
        assert!(!std::ptr::eq(&exact, &independently_compiled_top_level));
        assert!(Arc::ptr_eq(
            exact.source_queue(),
            independently_compiled_top_level.source_queue()
        ));
        assert!(Arc::ptr_eq(
            exact.source_extraction(),
            independently_compiled_top_level.source_extraction()
        ));
        assert!(
            exact
                .payload_eq_checked(&independently_compiled_top_level)
                .unwrap()
        );

        let mut late_difference = independently_compiled_top_level.clone();
        late_difference.tamper_stats_for_test();
        assert!(!exact.payload_eq_checked(&late_difference).unwrap());

        let mut unit_limited = exact.clone();
        unit_limited.limits.max_payload_comparison_units =
            exact.stats().payload_comparison_units() - 1;
        assert!(matches!(
            unit_limited.payload_eq_checked(&independently_compiled_top_level),
            Err(ResidualProductLocusBooleanCoverError::ResourceLimit {
                resource: "payload comparison units",
                requested,
                limit,
            }) if requested == exact.stats().payload_comparison_units()
                && limit + 1 == requested
        ));

        let mut byte_limited = exact.clone();
        byte_limited.limits.max_payload_comparison_bytes =
            exact.stats().payload_comparison_bytes() - 1;
        assert!(matches!(
            byte_limited.payload_eq_checked(&independently_compiled_top_level),
            Err(ResidualProductLocusBooleanCoverError::ResourceLimit {
                resource: "payload comparison bytes",
                requested,
                limit,
            }) if requested == exact.stats().payload_comparison_bytes()
                && limit + 1 == requested
        ));
    }

    #[test]
    fn checked_payload_comparison_fails_typed_at_distinct_nested_v1_allocations() {
        let (_, _, cover) = generated_sunset_cover();

        let mut distinct_queue = cover.clone();
        distinct_queue.source_queue = Arc::new(cover.source_queue.as_ref().clone());
        assert!(matches!(
            cover.payload_eq_checked(&distinct_queue),
            Err(
                ResidualProductLocusBooleanCoverError::IndependentNestedPayloadComparisonUnsupported {
                    component: "source queue",
                }
            )
        ));

        let mut distinct_extraction = cover.clone();
        distinct_extraction.source_extraction = Arc::new(cover.source_extraction.as_ref().clone());
        assert!(matches!(
            cover.payload_eq_checked(&distinct_extraction),
            Err(
                ResidualProductLocusBooleanCoverError::IndependentNestedPayloadComparisonUnsupported {
                    component: "source extraction",
                }
            )
        ));
    }

    #[test]
    fn exhausted_clause_witness_retention_is_aggregately_bounded() {
        let state = BooleanState {
            parent_ordinal: None,
            decision_from_parent: None,
            depth: 0,
            zero: Vec::new(),
            nonzero: vec![1],
            clauses: vec![vec![1]],
        };
        let sector = SectorMask::try_from_bit_string("1").unwrap();
        let mut limits = ResidualProductLocusBooleanCoverLimits::default();
        limits.max_retained_empty_reason_ordinals = 0;
        let mut stats = ResidualProductLocusBooleanCoverStats::default();
        assert!(matches!(
            build_dpll_cover(state, &sector, &[(1, None)], &mut stats, limits),
            Err(ResidualProductLocusBooleanCoverError::ResourceLimit {
                resource: "retained empty-reason ordinals",
                requested: 1,
                limit: 0,
            })
        ));
        assert_eq!(stats.retained_empty_reason_ordinals(), 0);
    }
}
