//! Concrete-only application of pivots derived on exceptional index loci.
//!
//! A pivot from [`crate::GeneratedPartialReeliminationCertificate`] is not a
//! global identity over `K(n)`: it is valid only after the certificate's
//! sparse equality assignment has been imposed.  This module keeps that
//! condition inseparable from the pivot.  It intentionally defines no
//! conversion to [`crate::ParametricReductionRuleCandidate`] and exposes no
//! raw centered [`crate::ParametricRelation`].

use std::collections::BTreeMap;
use std::fmt;
use std::mem::{align_of, size_of};
use std::sync::Arc;

use crate::parametric_coefficient::{
    coefficient_owned_retained_byte_bound, insert_specialized_condition,
};
use crate::{
    ConcreteIntegralKey, ConcreteRelation, GeneratedPartialReeliminationCertificate,
    GeneratedPartialReeliminationError, IndexSpace, IntegralFamily, IntegralOrderingPolicy,
    ParametricArithmeticLimits, ParametricCoefficientContext, ParametricCoefficientError,
    ParametricEliminationOrdering, ParametricRelation, ParametricRelationError,
    SectorFoundationError, SectorMask, SpecializedNonZeroCondition, StrictDescentWitness,
    algebra::Coefficient, algebra::CoefficientContext, algebra::ExactAlgebraError,
    algebra::ExactAlgebraLimits,
};

pub const CONDITIONAL_PARAMETRIC_RULE_V1_SCHEMA: &str = "rustred-conditional-parametric-rule-v1";

/// Proof budgets for one locus-bound rule and each concrete specialization.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConditionalParametricRuleLimits {
    pub arithmetic: ParametricArithmeticLimits,
    pub max_rhs_terms: usize,
    pub max_base_assumptions: usize,
    pub max_required_nonzero_conditions: usize,
    pub max_required_nonzero_origins: usize,
}

impl Default for ConditionalParametricRuleLimits {
    fn default() -> Self {
        Self {
            arithmetic: ParametricArithmeticLimits::default(),
            max_rhs_terms: 4_000_000,
            max_base_assumptions: 4_000_000,
            max_required_nonzero_conditions: 4_000_000,
            max_required_nonzero_origins: 16_000_000,
        }
    }
}

/// One exact re-elimination pivot bound permanently to its centered equality
/// locus and declared sector.
#[derive(Clone)]
pub struct ConditionalParametricRule {
    schema: &'static str,
    certificate: Arc<GeneratedPartialReeliminationCertificate>,
    family_fingerprint: Arc<str>,
    context_fingerprint: Arc<str>,
    sector: SectorMask,
    pivot_ordinal: usize,
    ordering: ParametricEliminationOrdering,
    centered_assignment: crate::PartialIndexAssignment,
    centered_relation: ParametricRelation,
    limits: ConditionalParametricRuleLimits,
}

// Do not print the private conditional row.  A Debug representation must not
// become a route for copying it into the global parametric-rule API.
impl fmt::Debug for ConditionalParametricRule {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConditionalParametricRule")
            .field("schema", &self.schema)
            .field("family_fingerprint", &self.family_fingerprint)
            .field("context_fingerprint", &self.context_fingerprint)
            .field("sector", &self.sector)
            .field("pivot_ordinal", &self.pivot_ordinal)
            .field("ordering", &self.ordering)
            .field("centered_assignment", &self.centered_assignment)
            .field("limits", &self.limits)
            .finish_non_exhaustive()
    }
}

impl ConditionalParametricRule {
    pub const SCHEMA: &'static str = CONDITIONAL_PARAMETRIC_RULE_V1_SCHEMA;

    /// Replay a generated partial re-elimination certificate and bind one of
    /// its pivots to a sector.  The centered row remains private.
    pub fn try_from_certificate_pivot(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        certificate: Arc<GeneratedPartialReeliminationCertificate>,
        pivot_ordinal: usize,
        sector: SectorMask,
        limits: ConditionalParametricRuleLimits,
    ) -> Result<Self, ConditionalParametricRuleError> {
        Self::try_from_certificate_pivot_impl(
            family,
            context,
            certificate,
            pivot_ordinal,
            sector,
            limits,
            true,
        )
    }

    /// Bind a pivot from a certificate already replayed by its owning
    /// family/live-leaf transcript. Kept crate-private so standalone callers
    /// cannot bypass proof replay.
    pub(crate) fn try_from_replayed_certificate_pivot(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        certificate: Arc<GeneratedPartialReeliminationCertificate>,
        pivot_ordinal: usize,
        sector: SectorMask,
        limits: ConditionalParametricRuleLimits,
    ) -> Result<Self, ConditionalParametricRuleError> {
        Self::try_from_certificate_pivot_impl(
            family,
            context,
            certificate,
            pivot_ordinal,
            sector,
            limits,
            false,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn try_from_certificate_pivot_impl(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        certificate: Arc<GeneratedPartialReeliminationCertificate>,
        pivot_ordinal: usize,
        sector: SectorMask,
        limits: ConditionalParametricRuleLimits,
        replay_certificate: bool,
    ) -> Result<Self, ConditionalParametricRuleError> {
        if certificate.family_fingerprint() != family.fingerprint() {
            return Err(ConditionalParametricRuleError::WrongFamily);
        }
        if certificate.context_fingerprint() != context.fingerprint() {
            return Err(ConditionalParametricRuleError::WrongContext);
        }
        if sector.arity() != context.index_count() {
            return Err(ConditionalParametricRuleError::WrongArity {
                expected: context.index_count(),
                actual: sector.arity(),
            });
        }
        check_limit(
            "conditional base assumptions",
            certificate.base_assumptions().len(),
            limits.max_base_assumptions,
        )?;
        if replay_certificate {
            certificate.replay(family, context)?;
        }
        let locus = certificate.centered_pivot_loci().get(pivot_ordinal).ok_or(
            ConditionalParametricRuleError::PivotOutOfRange {
                pivot: pivot_ordinal,
                available: certificate.centered_pivot_loci().len(),
            },
        )?;
        if locus.pivot_ordinal() != pivot_ordinal {
            return Err(ConditionalParametricRuleError::CertificateReplayMismatch);
        }
        for &(position, value) in locus.centered_assignment().entries() {
            let active = sector.is_active(position)?;
            if active != (value >= 1) {
                return Err(
                    ConditionalParametricRuleError::EmptyConditionalSectorLocus {
                        position,
                        value,
                        active,
                    },
                );
            }
        }
        let centered_assignment = locus.centered_assignment().clone();
        let ordering = certificate.ordering().clone();
        let centered_relation = certificate.centered_pivot_relation_for_bound_rule(
            context,
            pivot_ordinal,
            limits.arithmetic,
        )?;
        if centered_relation.family_fingerprint() != family.fingerprint() {
            return Err(ConditionalParametricRuleError::WrongFamily);
        }
        verify_symbolic_unit_lhs(context, &centered_relation, limits.arithmetic)?;
        check_limit(
            "conditional symbolic RHS terms",
            centered_relation.terms().len().saturating_sub(1),
            limits.max_rhs_terms,
        )?;

        Ok(Self {
            schema: CONDITIONAL_PARAMETRIC_RULE_V1_SCHEMA,
            certificate,
            family_fingerprint: Arc::from(family.fingerprint()),
            context_fingerprint: Arc::from(context.fingerprint()),
            sector,
            pivot_ordinal,
            ordering,
            centered_assignment,
            centered_relation,
            limits,
        })
    }

    pub const fn schema(&self) -> &'static str {
        self.schema
    }

    pub const fn certificate(&self) -> &Arc<GeneratedPartialReeliminationCertificate> {
        &self.certificate
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

    pub const fn pivot_ordinal(&self) -> usize {
        self.pivot_ordinal
    }

    pub const fn ordering(&self) -> &ParametricEliminationOrdering {
        &self.ordering
    }

    pub const fn centered_assignment(&self) -> &crate::PartialIndexAssignment {
        &self.centered_assignment
    }

    pub const fn limits(&self) -> ConditionalParametricRuleLimits {
        self.limits
    }

    /// Regenerate the complete conditional system and compare this bound
    /// pivot, including the private centered relation and its guard history.
    pub fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), ConditionalParametricRuleError> {
        if self.schema != CONDITIONAL_PARAMETRIC_RULE_V1_SCHEMA {
            return Err(ConditionalParametricRuleError::SchemaMismatch);
        }
        let replayed = Self::try_from_certificate_pivot(
            family,
            context,
            self.certificate.clone(),
            self.pivot_ordinal,
            self.sector.clone(),
            self.limits,
        )?;
        if self.payload_eq(&replayed) {
            Ok(())
        } else {
            Err(ConditionalParametricRuleError::CertificateReplayMismatch)
        }
    }

    pub(crate) fn payload_eq(&self, other: &Self) -> bool {
        self.schema == other.schema
            && self.family_fingerprint == other.family_fingerprint
            && self.context_fingerprint == other.context_fingerprint
            && self.sector == other.sector
            && self.pivot_ordinal == other.pivot_ordinal
            && self.ordering == other.ordering
            && self.centered_assignment == other.centered_assignment
            && self.limits == other.limits
            && self
                .centered_relation
                .has_identical_guard_provenance(&other.centered_relation)
    }

    /// Apply only at a concrete point satisfying both the declared sector and
    /// every coordinate equality in the centered locus.
    pub fn apply(
        &self,
        context: &ParametricCoefficientContext,
        indices: &[i64],
    ) -> Result<ConditionalParametricRuleApplication, ConditionalParametricRuleError> {
        if context.fingerprint() != self.context_fingerprint.as_ref() {
            return Err(ConditionalParametricRuleError::WrongContext);
        }
        if indices.len() != self.sector.arity() {
            return Err(ConditionalParametricRuleError::WrongArity {
                expected: self.sector.arity(),
                actual: indices.len(),
            });
        }
        if !self.sector.contains_indices(indices)? {
            return Ok(ConditionalParametricRuleApplication::Inapplicable(
                ConditionalParametricRuleInapplicability::OutsideSector,
            ));
        }
        for &(position, expected) in self.centered_assignment.entries() {
            let actual = indices[position];
            if actual != expected {
                return Ok(ConditionalParametricRuleApplication::Inapplicable(
                    ConditionalParametricRuleInapplicability::OutsideEqualityLocus {
                        position,
                        expected,
                        actual,
                    },
                ));
            }
        }

        let concrete =
            match self
                .centered_relation
                .specialize(context, indices, self.limits.arithmetic)
            {
                Ok(concrete) => concrete,
                Err(ParametricRelationError::UnsatisfiableDomain) => {
                    return Ok(ConditionalParametricRuleApplication::Inapplicable(
                        ConditionalParametricRuleInapplicability::NonzeroGuardVanished,
                    ));
                }
                Err(error) => return Err(error.into()),
            };
        let (source_key, source_coefficient) = concrete
            .terms()
            .iter()
            .find(|(key, _)| key.powers() == indices)
            .ok_or(ConditionalParametricRuleError::MissingConcreteLhs)?;
        let unit_delta = context.base().try_sub(
            source_coefficient,
            &context.base().one(),
            self.limits.arithmetic.exact_algebra,
        )?;
        if !unit_delta.is_zero() {
            return Err(ConditionalParametricRuleError::NonUnitConcreteLhs);
        }

        // Preflight the already-materialized concrete guards before cloning
        // them into the durable reduction proof.  Lower-level specialization
        // has its own algebra budgets; these are the condition-bound rule's
        // aggregate retention budgets.
        check_specialized_guard_limits(concrete.guarded_nonzero_conditions(), self.limits)?;
        let mut required_nonzero = concrete.guarded_nonzero_conditions().to_vec();
        for assumption in self.certificate.base_assumptions() {
            let specialized = context.specialize_nonzero_condition(
                assumption.condition(),
                indices,
                self.limits.arithmetic,
            )?;
            if specialized.polynomial().is_zero() {
                return Ok(ConditionalParametricRuleApplication::Inapplicable(
                    ConditionalParametricRuleInapplicability::NonzeroGuardVanished,
                ));
            }
            if specialized.polynomial().is_nonzero_constant() {
                continue;
            }
            preflight_specialized_condition_insert(&required_nonzero, &specialized, self.limits)?;
            insert_specialized_condition(
                &mut required_nonzero,
                specialized,
                self.limits.arithmetic.max_guard_origins,
            )?;
        }
        check_specialized_guard_limits(&required_nonzero, self.limits)?;

        check_limit(
            "conditional specialized RHS terms",
            concrete.terms().len().saturating_sub(1),
            self.limits.max_rhs_terms,
        )?;
        let mut rhs = BTreeMap::new();
        let mut descent = BTreeMap::new();
        for (target, coefficient) in concrete.terms() {
            if target == source_key {
                continue;
            }
            let target_sector = SectorMask::try_from_indices(target.powers())?;
            if !target_sector.is_subsector_of(&self.sector)? {
                return Ok(ConditionalParametricRuleApplication::Inapplicable(
                    ConditionalParametricRuleInapplicability::RhsSectorLeak {
                        target: target.clone(),
                        target_sector,
                    },
                ));
            }
            let witness = match self
                .ordering
                .policy()
                .prove_strict_descent(indices, target.powers())
            {
                Ok(witness) => witness,
                Err(SectorFoundationError::NotStrictDescent) => {
                    return Ok(ConditionalParametricRuleApplication::Inapplicable(
                        ConditionalParametricRuleInapplicability::NonDescendingRhs {
                            target: target.clone(),
                        },
                    ));
                }
                Err(error) => return Err(error.into()),
            };
            let solved = context
                .base()
                .try_neg(coefficient, self.limits.arithmetic.exact_algebra)?;
            rhs.insert(target.clone(), solved);
            descent.insert(target.clone(), witness);
        }

        Ok(ConditionalParametricRuleApplication::Applicable(
            ConditionalConcreteReduction {
                coordinate_rule: Arc::new(self.clone()),
                parametric_context: context.clone(),
                family_fingerprint: self.family_fingerprint.clone(),
                pivot_ordinal: self.pivot_ordinal,
                source: source_key.clone(),
                rhs,
                required_nonzero,
                descent,
                specialized_relation: concrete,
            },
        ))
    }
}

/// Concrete result of a condition-bound pivot.  It retains the complete rule
/// certificate and can independently replay the exact specialization.
#[derive(Clone)]
pub struct ConditionalConcreteReduction {
    coordinate_rule: Arc<ConditionalParametricRule>,
    parametric_context: ParametricCoefficientContext,
    family_fingerprint: Arc<str>,
    pivot_ordinal: usize,
    source: ConcreteIntegralKey,
    rhs: BTreeMap<ConcreteIntegralKey, Coefficient>,
    required_nonzero: Vec<SpecializedNonZeroCondition>,
    descent: BTreeMap<ConcreteIntegralKey, StrictDescentWitness>,
    specialized_relation: ConcreteRelation,
}

impl fmt::Debug for ConditionalConcreteReduction {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConditionalConcreteReduction")
            .field("coordinate_rule", &"<redacted>")
            .field("family_fingerprint", &self.family_fingerprint)
            .field("pivot_ordinal", &self.pivot_ordinal)
            .field("source", &self.source)
            .field("rhs_terms", &self.rhs.len())
            .field("required_nonzero", &self.required_nonzero.len())
            .field("descent_witnesses", &self.descent.len())
            .field("specialized_relation", &"<redacted>")
            .finish()
    }
}

impl ConditionalConcreteReduction {
    /// Return the coordinate-equality rule that owns this concrete reduction.
    pub const fn coordinate_rule(&self) -> &Arc<ConditionalParametricRule> {
        &self.coordinate_rule
    }

    /// Authenticated sector of the owning coordinate-equality rule.
    pub fn sector(&self) -> &SectorMask {
        self.coordinate_rule.sector()
    }

    /// Authenticated strict-descent policy of the owning rule.
    pub fn ordering_policy(&self) -> IntegralOrderingPolicy {
        self.coordinate_rule.ordering().policy()
    }
    pub const fn parametric_context(&self) -> &ParametricCoefficientContext {
        &self.parametric_context
    }
    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }
    pub const fn pivot_ordinal(&self) -> usize {
        self.pivot_ordinal
    }
    pub const fn source(&self) -> &ConcreteIntegralKey {
        &self.source
    }
    pub const fn rhs(&self) -> &BTreeMap<ConcreteIntegralKey, Coefficient> {
        &self.rhs
    }
    pub fn required_nonzero(&self) -> &[SpecializedNonZeroCondition] {
        &self.required_nonzero
    }
    pub fn descent_witnesses(&self) -> &BTreeMap<ConcreteIntegralKey, StrictDescentWitness> {
        &self.descent
    }
    pub const fn specialized_relation(&self) -> &ConcreteRelation {
        &self.specialized_relation
    }

    /// Conservative bytes owned by a deep clone of this concrete proof.
    /// Shared owner/certificate graphs and shared context maps remain behind
    /// `Arc`; the newly allocated family fingerprint and both Symbolica
    /// context templates are charged.
    pub(crate) fn owned_retained_byte_bound(&self) -> Option<usize> {
        let mut bytes = size_of::<Self>();
        bytes = bytes.checked_add(self.parametric_context.clone_owned_retained_byte_bound()?)?;
        bytes = bytes.checked_add(arc_str_allocation_byte_bound(
            self.family_fingerprint.len(),
        )?)?;
        bytes = bytes.checked_add(self.source.owned_retained_byte_bound()?)?;
        bytes = bytes.checked_add(self.specialized_relation.owned_retained_byte_bound()?)?;

        let rhs_node = conservative_btree_node_byte_bound::<ConcreteIntegralKey, Coefficient>()?;
        for (key, coefficient) in &self.rhs {
            bytes = bytes.checked_add(rhs_node)?;
            bytes = bytes.checked_add(key.owned_retained_byte_bound()?)?;
            bytes = bytes.checked_add(coefficient_owned_retained_byte_bound(coefficient)?)?;
        }

        bytes = bytes.checked_add(
            self.required_nonzero
                .capacity()
                .checked_mul(size_of::<SpecializedNonZeroCondition>())?,
        )?;
        for condition in &self.required_nonzero {
            bytes = bytes.checked_add(condition.owned_retained_byte_bound()?)?;
        }

        let descent_node =
            conservative_btree_node_byte_bound::<ConcreteIntegralKey, StrictDescentWitness>()?;
        for (key, witness) in &self.descent {
            bytes = bytes.checked_add(descent_node)?;
            bytes = bytes.checked_add(key.owned_retained_byte_bound()?)?;
            bytes = bytes.checked_add(witness.owned_retained_byte_bound()?)?;
        }
        Some(bytes)
    }

    pub fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), ConditionalParametricRuleError> {
        self.coordinate_rule.replay(family, context)?;
        let ConditionalParametricRuleApplication::Applicable(replayed) =
            self.coordinate_rule.apply(context, self.source.powers())?
        else {
            return Err(ConditionalParametricRuleError::CertificateReplayMismatch);
        };
        if self.family_fingerprint == replayed.family_fingerprint
            && self.parametric_context.fingerprint() == replayed.parametric_context.fingerprint()
            && self.pivot_ordinal == replayed.pivot_ordinal
            && self.source == replayed.source
            && self.rhs == replayed.rhs
            && self.required_nonzero == replayed.required_nonzero
            && self.descent == replayed.descent
            && self
                .specialized_relation
                .has_identical_guard_provenance(&replayed.specialized_relation)
            && self.coordinate_rule.payload_eq(&replayed.coordinate_rule)
        {
            Ok(())
        } else {
            Err(ConditionalParametricRuleError::CertificateReplayMismatch)
        }
    }

    /// Cheap demand-engine-facing check.  Full provenance replay is provided
    /// by [`Self::replay`].
    pub fn verify_application(
        &self,
        context: &CoefficientContext,
        policy: IntegralOrderingPolicy,
        limits: ExactAlgebraLimits,
    ) -> Result<bool, ExactAlgebraError> {
        if self.specialized_relation.family_fingerprint() != self.family_fingerprint.as_ref()
            || self.coordinate_rule.family_fingerprint() != self.family_fingerprint.as_ref()
            || self.parametric_context.fingerprint() != self.coordinate_rule.context_fingerprint()
            || policy != self.ordering_policy()
            || !self
                .parametric_context
                .base()
                .has_same_variable_map(context)
            || !self
                .sector()
                .contains_indices(self.source.powers())
                .unwrap_or(false)
            || self.coordinate_rule.pivot_ordinal() != self.pivot_ordinal
            || !assignment_satisfied(
                self.coordinate_rule.centered_assignment(),
                self.source.powers(),
            )
            || self.rhs.keys().ne(self.descent.keys())
        {
            return Ok(false);
        }
        let Some(lhs) = self.specialized_relation.terms().get(&self.source) else {
            return Ok(false);
        };
        if !context.try_sub(lhs, &context.one(), limits)?.is_zero()
            || self.specialized_relation.terms().len() != self.rhs.len() + 1
        {
            return Ok(false);
        }
        for condition in &self.required_nonzero {
            context.validate_with_limits(&condition.polynomial().raw().clone().into(), limits)?;
        }
        for (target, solved) in &self.rhs {
            let Some(witness) = self.descent.get(target) else {
                return Ok(false);
            };
            let target_sector = match SectorMask::try_from_indices(target.powers()) {
                Ok(sector) => sector,
                Err(_) => return Ok(false),
            };
            if !target_sector
                .is_subsector_of(self.sector())
                .unwrap_or(false)
                || witness.policy() != policy
                || !witness.verify()
                || !policy
                    .complexity_key(self.source.powers())
                    .is_ok_and(|key| &key == witness.source())
                || !policy
                    .complexity_key(target.powers())
                    .is_ok_and(|key| &key == witness.target())
            {
                return Ok(false);
            }
            context.validate_with_limits(solved, limits)?;
            let Some(equation_coefficient) = self.specialized_relation.terms().get(target) else {
                return Ok(false);
            };
            if !context
                .try_add(equation_coefficient, solved, limits)?
                .is_zero()
            {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

fn conservative_btree_node_byte_bound<Key, Value>() -> Option<usize> {
    size_of::<(Key, Value)>()
        .checked_mul(16)?
        .checked_add(32usize.checked_mul(size_of::<usize>())?)
}

fn arc_str_allocation_byte_bound(length: usize) -> Option<usize> {
    size_of::<usize>()
        .checked_mul(2)?
        .checked_add(align_of::<usize>().saturating_sub(1))?
        .checked_add(length)
}

#[derive(Clone, Debug)]
pub enum ConditionalParametricRuleApplication {
    Applicable(ConditionalConcreteReduction),
    Inapplicable(ConditionalParametricRuleInapplicability),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConditionalParametricRuleInapplicability {
    OutsideSector,
    OutsideEqualityLocus {
        position: usize,
        expected: i64,
        actual: i64,
    },
    NonzeroGuardVanished,
    RhsSectorLeak {
        target: ConcreteIntegralKey,
        target_sector: SectorMask,
    },
    NonDescendingRhs {
        target: ConcreteIntegralKey,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConditionalParametricRuleError {
    WrongFamily,
    WrongContext,
    WrongArity {
        expected: usize,
        actual: usize,
    },
    PivotOutOfRange {
        pivot: usize,
        available: usize,
    },
    EmptyConditionalSectorLocus {
        position: usize,
        value: i64,
        active: bool,
    },
    MissingSymbolicLhs,
    NonUnitSymbolicLhs,
    MissingConcreteLhs,
    NonUnitConcreteLhs,
    SchemaMismatch,
    CertificateReplayMismatch,
    ResourceCountOverflow {
        resource: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    Generated(GeneratedPartialReeliminationError),
    Relation(ParametricRelationError),
    Coefficient(ParametricCoefficientError),
    ExactAlgebra(ExactAlgebraError),
    Sector(SectorFoundationError),
}

impl fmt::Display for ConditionalParametricRuleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongFamily => formatter.write_str("conditional rule family mismatch"),
            Self::WrongContext => formatter.write_str("conditional rule context mismatch"),
            Self::WrongArity { expected, actual } => write!(
                formatter,
                "conditional rule arity is {actual}, expected {expected}"
            ),
            Self::PivotOutOfRange { pivot, available } => write!(
                formatter,
                "conditional pivot {pivot} is outside {available} available pivots"
            ),
            Self::EmptyConditionalSectorLocus {
                position,
                value,
                active,
            } => write!(
                formatter,
                "conditional equality n[{position}]={value} contradicts the declared {} slot",
                if *active { "active" } else { "inactive" }
            ),
            Self::MissingSymbolicLhs => {
                formatter.write_str("conditional centered pivot has no zero-shift LHS")
            }
            Self::NonUnitSymbolicLhs => {
                formatter.write_str("conditional centered pivot LHS is not exactly one")
            }
            Self::MissingConcreteLhs => {
                formatter.write_str("conditional specialization has no source integral")
            }
            Self::NonUnitConcreteLhs => {
                formatter.write_str("conditional specialization source coefficient is not one")
            }
            Self::SchemaMismatch => formatter.write_str("conditional rule schema mismatch"),
            Self::CertificateReplayMismatch => {
                formatter.write_str("conditional rule differs after complete certificate replay")
            }
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "conditional {resource} count overflowed usize")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "conditional {resource} requested {requested}, configured limit is {limit}"
            ),
            Self::Generated(error) => error.fmt(formatter),
            Self::Relation(error) => error.fmt(formatter),
            Self::Coefficient(error) => error.fmt(formatter),
            Self::ExactAlgebra(error) => error.fmt(formatter),
            Self::Sector(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ConditionalParametricRuleError {}

impl From<GeneratedPartialReeliminationError> for ConditionalParametricRuleError {
    fn from(value: GeneratedPartialReeliminationError) -> Self {
        Self::Generated(value)
    }
}
impl From<ParametricRelationError> for ConditionalParametricRuleError {
    fn from(value: ParametricRelationError) -> Self {
        Self::Relation(value)
    }
}
impl From<ParametricCoefficientError> for ConditionalParametricRuleError {
    fn from(value: ParametricCoefficientError) -> Self {
        Self::Coefficient(value)
    }
}
impl From<ExactAlgebraError> for ConditionalParametricRuleError {
    fn from(value: ExactAlgebraError) -> Self {
        Self::ExactAlgebra(value)
    }
}
impl From<SectorFoundationError> for ConditionalParametricRuleError {
    fn from(value: SectorFoundationError) -> Self {
        Self::Sector(value)
    }
}

fn verify_symbolic_unit_lhs(
    context: &ParametricCoefficientContext,
    relation: &ParametricRelation,
    limits: ParametricArithmeticLimits,
) -> Result<(), ConditionalParametricRuleError> {
    let zero = IndexSpace::try_new(context.index_count())?.zero();
    let coefficient = relation
        .terms()
        .get(&zero)
        .ok_or(ConditionalParametricRuleError::MissingSymbolicLhs)?;
    let delta = context.sub_with_limits(coefficient, &context.one(), limits.exact_algebra)?;
    if delta.is_zero() {
        Ok(())
    } else {
        Err(ConditionalParametricRuleError::NonUnitSymbolicLhs)
    }
}

fn check_specialized_guard_limits(
    conditions: &[SpecializedNonZeroCondition],
    limits: ConditionalParametricRuleLimits,
) -> Result<(), ConditionalParametricRuleError> {
    check_limit(
        "required nonzero conditions",
        conditions.len(),
        limits.max_required_nonzero_conditions,
    )?;
    let origins = conditions.iter().try_fold(0usize, |total, condition| {
        total.checked_add(condition.origins().len()).ok_or(
            ConditionalParametricRuleError::ResourceCountOverflow {
                resource: "required nonzero origins",
            },
        )
    })?;
    check_limit(
        "required nonzero origins",
        origins,
        limits.max_required_nonzero_origins,
    )
}

fn preflight_specialized_condition_insert(
    conditions: &[SpecializedNonZeroCondition],
    condition: &SpecializedNonZeroCondition,
    limits: ConditionalParametricRuleLimits,
) -> Result<(), ConditionalParametricRuleError> {
    let current_origins = conditions.iter().try_fold(0usize, |total, existing| {
        total.checked_add(existing.origins().len()).ok_or(
            ConditionalParametricRuleError::ResourceCountOverflow {
                resource: "required nonzero origins",
            },
        )
    })?;
    let existing = conditions
        .iter()
        .find(|existing| existing.polynomial() == condition.polynomial());
    let (condition_count, additional_origins) = if let Some(existing) = existing {
        (
            conditions.len(),
            condition
                .origins()
                .iter()
                .filter(|origin| !existing.origins().contains(*origin))
                .count(),
        )
    } else {
        (
            conditions.len().checked_add(1).ok_or(
                ConditionalParametricRuleError::ResourceCountOverflow {
                    resource: "required nonzero conditions",
                },
            )?,
            condition.origins().len(),
        )
    };
    check_limit(
        "required nonzero conditions",
        condition_count,
        limits.max_required_nonzero_conditions,
    )?;
    let total_origins = current_origins.checked_add(additional_origins).ok_or(
        ConditionalParametricRuleError::ResourceCountOverflow {
            resource: "required nonzero origins",
        },
    )?;
    check_limit(
        "required nonzero origins",
        total_origins,
        limits.max_required_nonzero_origins,
    )
}

fn assignment_satisfied(assignment: &crate::PartialIndexAssignment, indices: &[i64]) -> bool {
    indices.len() == assignment.arity()
        && assignment
            .entries()
            .iter()
            .all(|&(position, expected)| indices[position] == expected)
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ConditionalParametricRuleError> {
    if requested <= limit {
        Ok(())
    } else {
        Err(ConditionalParametricRuleError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    }
}
