use std::sync::Arc;

use crate::algebra::{IndexedCoefficientContext, IndexedPolynomial};
use crate::family::IntegralKey;
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
