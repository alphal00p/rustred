use std::sync::Arc;

use crate::algebra::{IndexedCoefficientContext, IndexedPolynomial};
use crate::family::IntegralKey;
use crate::foundry::completion::frame::exact::ExactCircuitLoweringSeal;
use crate::foundry::parametric::ParametricRule;
use crate::identity::{ParametricRelation, TranslatedSourceProvenance};
use crate::sector::{
    Mask, SectorInteriorDomain, SectorMonotoneDomain, SectorMonotoneShiftDescentWitness,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FixedIndexRestriction {
    position: usize,
    value: i64,
}

impl FixedIndexRestriction {
    pub const fn new(position: usize, value: i64) -> Self {
        Self { position, value }
    }
    pub const fn position(self) -> usize {
        self.position
    }
    pub const fn value(self) -> i64 {
        self.value
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SymmetrySourceProvenance {
    group_element: usize,
}

impl SymmetrySourceProvenance {
    pub const fn new(group_element: usize) -> Self {
        Self { group_element }
    }
    pub const fn group_element(self) -> usize {
        self.group_element
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceViewProvenance {
    pub(super) translated: TranslatedSourceProvenance,
    pub(super) symmetry: Option<SymmetrySourceProvenance>,
}

impl SourceViewProvenance {
    pub(crate) const fn from_exact_translation(
        _seal: &ExactCircuitLoweringSeal,
        translated: TranslatedSourceProvenance,
    ) -> Self {
        Self {
            translated,
            symmetry: None,
        }
    }

    pub fn translated(&self) -> &TranslatedSourceProvenance {
        &self.translated
    }
    pub const fn symmetry(&self) -> Option<SymmetrySourceProvenance> {
        self.symmetry
    }
}

#[derive(Debug)]
pub struct SourceViewBatch {
    pub(super) family_fingerprint: Arc<String>,
    pub(super) context_fingerprint: Arc<String>,
    pub(super) relations: Vec<ParametricRelation>,
    pub(super) provenance: Vec<SourceViewProvenance>,
    pub(super) construction: SourceViewConstruction,
}

/// How an immutable source span entered the rule-cell foundry.
#[derive(Debug)]
pub enum SourceViewConstruction {
    Direct,
    ResidualProjection(ResidualProjectionEvidence),
    /// The immutable translated rows remain unmodified, but every algebraic
    /// replay is evaluated in this exact singleton-coordinate quotient.
    FixedIndexSpecialization(FixedIndexSpecializationEvidence),
}

/// Exact, topology-neutral quotient attached to an otherwise immutable source
/// span.  No raw source term or condition is deleted or renumbered: consumers
/// specialize it with Symbolica at the proof boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FixedIndexSpecializationEvidence {
    pub(super) fixed: Box<[FixedIndexRestriction]>,
}

impl FixedIndexSpecializationEvidence {
    pub fn fixed_restrictions(&self) -> &[FixedIndexRestriction] {
        &self.fixed
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResidualTermDisposition {
    CoefficientZero,
    ProvedZero {
        zero_sector: Mask,
    },
    Routed {
        group_element: usize,
        projected_shift: Box<[i64]>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResidualTermProjection {
    pub(super) source_shift: Box<[i64]>,
    pub(super) disposition: ResidualTermDisposition,
}

impl ResidualTermProjection {
    pub fn source_shift(&self) -> &[i64] {
        &self.source_shift
    }

    pub fn disposition(&self) -> &ResidualTermDisposition {
        &self.disposition
    }
}

/// One-time proof payload for exact zero/symmetry feedback on a restricted
/// source span. Original translated relations remain owned here; the batch's
/// active relations are their exact projected replay.
#[derive(Debug)]
pub struct ResidualProjectionEvidence {
    pub(super) domain: SectorInteriorDomain,
    pub(super) fixed: Box<[FixedIndexRestriction]>,
    pub(super) original_relations: Vec<ParametricRelation>,
    pub(super) terms: Vec<Box<[ResidualTermProjection]>>,
    pub(super) stabilizer_group_elements: Box<[usize]>,
}

impl ResidualProjectionEvidence {
    pub fn domain(&self) -> &SectorInteriorDomain {
        &self.domain
    }

    pub fn fixed_restrictions(&self) -> &[FixedIndexRestriction] {
        &self.fixed
    }

    pub fn original_relations(&self) -> &[ParametricRelation] {
        &self.original_relations
    }

    pub fn term_projections(&self) -> &[Box<[ResidualTermProjection]>] {
        &self.terms
    }

    pub fn stabilizer_group_elements(&self) -> &[usize] {
        &self.stabilizer_group_elements
    }
}

impl SourceViewBatch {
    pub(crate) fn try_from_exact_lowered_parts(
        _seal: &ExactCircuitLoweringSeal,
        family_fingerprint: Arc<String>,
        context_fingerprint: Arc<String>,
        relations: Vec<ParametricRelation>,
        provenance: Vec<SourceViewProvenance>,
    ) -> Result<Self, super::RuleCellError> {
        if relations.len() != provenance.len() {
            return Err(super::RuleCellError::SourceProvenanceCountMismatch {
                relations: relations.len(),
                provenance: provenance.len(),
            });
        }
        Ok(Self {
            family_fingerprint,
            context_fingerprint,
            relations,
            provenance,
            construction: SourceViewConstruction::Direct,
        })
    }

    pub(crate) fn try_from_exact_fixed_specialization_parts(
        _seal: &ExactCircuitLoweringSeal,
        family_fingerprint: Arc<String>,
        context_fingerprint: Arc<String>,
        relations: Vec<ParametricRelation>,
        provenance: Vec<SourceViewProvenance>,
        fixed: Box<[FixedIndexRestriction]>,
    ) -> Result<Self, super::RuleCellError> {
        if relations.len() != provenance.len() {
            return Err(super::RuleCellError::SourceProvenanceCountMismatch {
                relations: relations.len(),
                provenance: provenance.len(),
            });
        }
        if fixed.is_empty() {
            return Err(super::RuleCellError::EmptyFixedIndexSpecialization);
        }
        for window in fixed.windows(2) {
            if window[0].position() >= window[1].position() {
                return Err(super::RuleCellError::DuplicateFixedPosition {
                    position: window[1].position(),
                });
            }
        }
        Ok(Self {
            family_fingerprint,
            context_fingerprint,
            relations,
            provenance,
            construction: SourceViewConstruction::FixedIndexSpecialization(
                FixedIndexSpecializationEvidence { fixed },
            ),
        })
    }

    pub fn family_fingerprint(&self) -> &str {
        self.family_fingerprint.as_str()
    }
    pub fn context_fingerprint(&self) -> &str {
        self.context_fingerprint.as_str()
    }
    pub fn len(&self) -> usize {
        self.relations.len()
    }
    pub fn is_empty(&self) -> bool {
        self.relations.is_empty()
    }
    pub fn provenance(&self) -> &[SourceViewProvenance] {
        &self.provenance
    }
    pub fn construction(&self) -> &SourceViewConstruction {
        &self.construction
    }
    pub(crate) fn relations(&self) -> &[ParametricRelation] {
        &self.relations
    }

    #[cfg(test)]
    fn replace_translated_source_ordinal_for_artifact_test(
        &mut self,
        ordinal: usize,
        source_ordinal: usize,
    ) {
        self.provenance[ordinal]
            .translated
            .replace_source_ordinal_for_artifact_test(source_ordinal);
    }

    #[cfg(test)]
    fn replace_translated_source_row_for_artifact_test(
        &mut self,
        ordinal: usize,
        source_row: crate::identity::RowId,
    ) {
        self.provenance[ordinal]
            .translated
            .replace_source_row_for_artifact_test(source_row);
    }

    #[cfg(test)]
    fn replace_translated_source_offset_for_artifact_test(
        &mut self,
        ordinal: usize,
        offset: crate::identity::IntegralShift,
    ) {
        self.provenance[ordinal]
            .translated
            .replace_offset_for_artifact_test(offset);
    }

    #[cfg(test)]
    fn replace_source_relation_for_artifact_test(
        &mut self,
        ordinal: usize,
        relation: ParametricRelation,
    ) {
        self.relations[ordinal] = relation;
    }

    #[cfg(test)]
    fn replace_residual_original_relation_for_artifact_test(
        &mut self,
        ordinal: usize,
        relation: ParametricRelation,
    ) {
        let SourceViewConstruction::ResidualProjection(evidence) = &mut self.construction else {
            panic!("test-only residual mutation requires a residual source projection");
        };
        evidence.original_relations[ordinal] = relation;
    }

    #[cfg(test)]
    fn attach_unregistered_symmetry_for_artifact_test(
        &mut self,
        ordinal: usize,
        group_element: usize,
    ) {
        self.provenance[ordinal].symmetry = Some(SymmetrySourceProvenance::new(group_element));
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RuleCellDomainProof {
    TightenedOriginalInterior,
    ReprovedSectorMonotone,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleCellTerm {
    pub(super) source_rhs_ordinal: usize,
    pub(super) descent: SectorMonotoneShiftDescentWitness,
}

impl RuleCellTerm {
    pub const fn source_rhs_ordinal(&self) -> usize {
        self.source_rhs_ordinal
    }
    pub fn descent(&self) -> &SectorMonotoneShiftDescentWitness {
        &self.descent
    }
}

/// A retained source guard whose exceptional integer locus was proved disjoint
/// from the owning cell's complete application domain during construction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleCellGuard {
    pub(super) source_guard_ordinal: usize,
    pub(super) polynomial: IndexedPolynomial,
}

impl RuleCellGuard {
    pub const fn source_guard_ordinal(&self) -> usize {
        self.source_guard_ordinal
    }
    pub fn polynomial(&self) -> &IndexedPolynomial {
        &self.polynomial
    }
}

/// Exact rectangular decomposition around one separable integer guard root.
///
/// The admitted component is the unique guard-free component containing the
/// replay anchor. The singleton root is mandatory alternate-support work. The
/// optional deferred component is reserved for a future bounded multi-cell
/// extension; the current builder admits only endpoint roots and leaves an
/// interior root fail-closed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RuleCellGuardDomainSplit {
    guard_ordinal: usize,
    position: usize,
    value: i64,
    admitted: SectorMonotoneDomain,
    exceptional: SectorMonotoneDomain,
    deferred_guard_free: Option<SectorMonotoneDomain>,
}

impl RuleCellGuardDomainSplit {
    #[cfg(test)]
    pub(crate) const fn guard_ordinal(&self) -> usize {
        self.guard_ordinal
    }

    #[cfg(test)]
    pub(crate) const fn position(&self) -> usize {
        self.position
    }

    #[cfg(test)]
    pub(crate) const fn value(&self) -> i64 {
        self.value
    }

    pub(crate) const fn admitted_domain(&self) -> &SectorMonotoneDomain {
        &self.admitted
    }

    #[cfg(test)]
    pub(crate) const fn exceptional_domain(&self) -> &SectorMonotoneDomain {
        &self.exceptional
    }

    #[cfg(test)]
    pub(crate) const fn deferred_guard_free_domain(&self) -> Option<&SectorMonotoneDomain> {
        self.deferred_guard_free.as_ref()
    }

    pub(super) fn from_parts(
        guard_ordinal: usize,
        position: usize,
        value: i64,
        admitted: SectorMonotoneDomain,
        exceptional: SectorMonotoneDomain,
        deferred_guard_free: Option<SectorMonotoneDomain>,
    ) -> Self {
        Self {
            guard_ordinal,
            position,
            value,
            admitted,
            exceptional,
            deferred_guard_free,
        }
    }
}

#[derive(Debug)]
pub struct RuleCell {
    rule: ParametricRule,
    sources: SourceViewBatch,
    proof_domain: SectorInteriorDomain,
    application_domain: SectorMonotoneDomain,
    domain_proof: RuleCellDomainProof,
    fixed: Box<[FixedIndexRestriction]>,
    pruned_rhs_ordinals: Box<[usize]>,
    terms: Box<[RuleCellTerm]>,
    guards: Box<[RuleCellGuard]>,
}

impl RuleCell {
    pub fn rule(&self) -> &ParametricRule {
        &self.rule
    }
    pub fn sources(&self) -> &SourceViewBatch {
        &self.sources
    }
    pub fn proof_domain(&self) -> &SectorInteriorDomain {
        &self.proof_domain
    }
    pub fn application_domain(&self) -> &SectorMonotoneDomain {
        &self.application_domain
    }
    pub const fn domain_proof(&self) -> RuleCellDomainProof {
        self.domain_proof
    }
    pub fn fixed_restrictions(&self) -> &[FixedIndexRestriction] {
        &self.fixed
    }
    pub fn pruned_rhs_ordinals(&self) -> &[usize] {
        &self.pruned_rhs_ordinals
    }
    pub fn terms(&self) -> &[RuleCellTerm] {
        &self.terms
    }
    pub fn guards(&self) -> &[RuleCellGuard] {
        &self.guards
    }
    pub(crate) fn indexed_context_matches(&self, context: &IndexedCoefficientContext) -> bool {
        self.rule.context_fingerprint() == context.fingerprint()
    }

    #[cfg(test)]
    pub(crate) fn replace_first_guard_polynomial_for_test(
        &mut self,
        polynomial: IndexedPolynomial,
    ) {
        self.guards[0].polynomial = polynomial;
    }

    #[cfg(test)]
    pub(crate) fn replace_rule_ordering_for_artifact_test(
        &mut self,
        ordering: crate::sector::OrderingPolicy,
    ) {
        self.rule.replace_ordering_for_artifact_test(ordering);
    }

    #[cfg(test)]
    pub(crate) fn replace_translated_source_ordinal_for_artifact_test(
        &mut self,
        ordinal: usize,
        source_ordinal: usize,
    ) {
        self.sources
            .replace_translated_source_ordinal_for_artifact_test(ordinal, source_ordinal);
    }

    #[cfg(test)]
    pub(crate) fn replace_translated_source_row_for_artifact_test(
        &mut self,
        ordinal: usize,
        source_row: crate::identity::RowId,
    ) {
        self.sources
            .replace_translated_source_row_for_artifact_test(ordinal, source_row);
    }

    #[cfg(test)]
    pub(crate) fn replace_translated_source_offset_for_artifact_test(
        &mut self,
        ordinal: usize,
        offset: crate::identity::IntegralShift,
    ) {
        self.sources
            .replace_translated_source_offset_for_artifact_test(ordinal, offset);
    }

    #[cfg(test)]
    pub(crate) fn replace_source_relation_for_artifact_test(
        &mut self,
        ordinal: usize,
        relation: ParametricRelation,
    ) {
        self.sources
            .replace_source_relation_for_artifact_test(ordinal, relation);
    }

    #[cfg(test)]
    pub(crate) fn replace_residual_original_relation_for_artifact_test(
        &mut self,
        ordinal: usize,
        relation: ParametricRelation,
    ) {
        self.sources
            .replace_residual_original_relation_for_artifact_test(ordinal, relation);
    }

    #[cfg(test)]
    pub(crate) fn attach_unregistered_source_symmetry_for_artifact_test(
        &mut self,
        ordinal: usize,
        group_element: usize,
    ) {
        self.sources
            .attach_unregistered_symmetry_for_artifact_test(ordinal, group_element);
    }
    pub fn assignment_for_target(
        &self,
        target: &IntegralKey,
    ) -> Result<Option<Vec<i64>>, super::RuleCellError> {
        if target.powers().len() != self.application_domain.arity() {
            return Err(super::RuleCellError::WrongApplicationArity {
                expected: self.application_domain.arity(),
                actual: target.powers().len(),
            });
        }
        let mut assignment = Vec::new();
        assignment
            .try_reserve_exact(target.powers().len())
            .map_err(|_| super::RuleCellError::AllocationFailure {
                resource: "free-index assignment",
                requested: target.powers().len(),
            })?;
        for (&power, &pivot) in target.powers().iter().zip(self.rule.pivot().values()) {
            let Some(value) = power.checked_sub(pivot) else {
                // A cell whose free-index assignment cannot be represented
                // is simply inapplicable. Another cell may own the target;
                // if none does, the reducer reports the genuine coverage
                // failure rather than leaking an internal arithmetic error.
                return Ok(None);
            };
            assignment.push(value);
        }
        if self.application_domain.contains(&assignment)? {
            Ok(Some(assignment))
        } else {
            Ok(None)
        }
    }

    pub(super) fn from_parts(
        rule: ParametricRule,
        sources: SourceViewBatch,
        application_domain: SectorMonotoneDomain,
        domain_proof: RuleCellDomainProof,
        fixed: Box<[FixedIndexRestriction]>,
        pruned_rhs_ordinals: Box<[usize]>,
        terms: Box<[RuleCellTerm]>,
        guards: Box<[RuleCellGuard]>,
    ) -> Self {
        let proof_domain = rule.domain().clone();
        Self {
            rule,
            sources,
            proof_domain,
            application_domain,
            domain_proof,
            fixed,
            pruned_rhs_ordinals,
            terms,
            guards,
        }
    }
}
