//! Guarded compilation and concrete application of discovered parametric rules.
//!
//! A normalized elimination row is only a recurrence *candidate*.  LiteRed's
//! `SolvejSector` subsequently constructs a condition (`WhenBad`) excluding
//! coefficient poles and RHS integrals that leak into a harder domain.  The
//! first RustRed compiler implements the same proof boundary without relying
//! on Mathematica's condition language:
//!
//! * the exact centered `K(n)` identity and elimination trace are retained;
//! * all pre-cancellation nonzero conditions are specialized and retained;
//! * the unshifted source indices must belong to the declared sector; and
//! * every surviving RHS integral must have a checked strict-descent witness
//!   under the persisted ordering policy.
//!
//! The last check is deliberately coefficient-aware.  A shifted integral that
//! would cross an inactive boundary is harmless at a point where its exact
//! coefficient vanishes, matching the important semantics of LiteRed's
//! `WhenBad` numerator-leak test.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use crate::{
    ConcreteIntegralKey, ConcreteRelation, IndexShift, IntegralFamily, IntegralOrderingPolicy,
    ParametricArithmeticLimits, ParametricCoefficientContext, ParametricCoefficientError,
    ParametricElimination, ParametricEliminationError, ParametricEliminationOrdering,
    ParametricEliminationTrace, ParametricRelation, ParametricRelationError, SectorFoundationError,
    SectorMask, SpecializedNonZeroCondition, StrictDescentWitness, algebra::Coefficient,
    algebra::ExactAlgebraError, algebra::ExactAlgebraLimits,
};

pub const PARAMETRIC_REDUCTION_RULE_V1_SCHEMA: &str = "rustred-parametric-reduction-rule-v1";
pub const PARAMETRIC_RULE_DERIVATION_V1_SCHEMA: &str = "rustred-parametric-rule-derivation-v1";
pub const RUNTIME_DESCENT_GUARD_V1_SCHEMA: &str = "rustred-runtime-descent-guard-v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParametricRuleLimits {
    pub arithmetic: ParametricArithmeticLimits,
    pub max_rhs_terms: usize,
    pub max_source_rows_for_replay: usize,
}

impl Default for ParametricRuleLimits {
    fn default() -> Self {
        Self {
            arithmetic: ParametricArithmeticLimits::default(),
            max_rhs_terms: 4_000_000,
            max_source_rows_for_replay: 100_000,
        }
    }
}

/// The complete immutable source system and exact elimination certificate
/// from which one or more parametric rule candidates were compiled.
///
/// Keeping this object inside every emitted candidate is intentional: an
/// adaptive translated stencil must remain replayable after the transient
/// search provider and its cumulative row buffer have been dropped.  The two
/// large payloads are reference counted, so all pivots from one elimination
/// share the same retained derivation.
#[derive(Clone, Debug)]
pub struct ParametricRuleDerivation {
    source_rows: Arc<[ParametricRelation]>,
    elimination: Arc<ParametricElimination>,
}

impl ParametricRuleDerivation {
    pub const SCHEMA: &'static str = PARAMETRIC_RULE_DERIVATION_V1_SCHEMA;

    pub fn try_new(
        context: &ParametricCoefficientContext,
        source_rows: &[ParametricRelation],
        elimination: &ParametricElimination,
        limits: ParametricRuleLimits,
    ) -> Result<Self, ParametricRuleError> {
        check_rule_limit(
            "source rows retained for derivation replay",
            source_rows.len(),
            limits.max_source_rows_for_replay,
        )?;
        elimination.replay(context, source_rows)?;
        Ok(Self {
            source_rows: Arc::from(source_rows.to_vec()),
            elimination: Arc::new(elimination.clone()),
        })
    }

    pub fn source_rows(&self) -> &[ParametricRelation] {
        &self.source_rows
    }

    pub fn elimination(&self) -> &ParametricElimination {
        &self.elimination
    }

    /// Rebuild every retained pivot from the exact ordered source rows.
    pub fn replay(
        &self,
        context: &ParametricCoefficientContext,
    ) -> Result<(), ParametricRuleError> {
        self.elimination.replay(context, &self.source_rows)?;
        Ok(())
    }
}

/// A proof-bearing parametric rule candidate emitted from generic sparse
/// elimination.  Fields are private: construction always verifies the unit
/// LHS and binds the candidate to an elimination pivot and sector.
#[derive(Clone, Debug)]
pub struct ParametricReductionRuleCandidate {
    derivation: ParametricRuleDerivation,
    family_fingerprint: Arc<str>,
    context_fingerprint: Arc<str>,
    sector: SectorMask,
    ordering: ParametricEliminationOrdering,
    source_row_count: usize,
    source_manifest: Arc<str>,
    pivot_ordinal: usize,
    original_pivot: IndexShift,
    trace: ParametricEliminationTrace,
    centered_relation: ParametricRelation,
    discovery_anchor: Box<[i64]>,
    limits: ParametricRuleLimits,
}

/// Compatibility spelling.  This remains a candidate until a separate
/// coverage proof establishes a complete integer-domain partition.
pub type ParametricReductionRule = ParametricReductionRuleCandidate;

impl ParametricReductionRuleCandidate {
    pub const SCHEMA: &'static str = PARAMETRIC_REDUCTION_RULE_V1_SCHEMA;
    pub const GUARD_SCHEMA: &'static str = RUNTIME_DESCENT_GUARD_V1_SCHEMA;

    pub fn try_from_elimination_pivot(
        context: &ParametricCoefficientContext,
        source_rows: &[ParametricRelation],
        elimination: &ParametricElimination,
        pivot_ordinal: usize,
        sector: SectorMask,
        limits: ParametricRuleLimits,
    ) -> Result<Self, ParametricRuleError> {
        let derivation =
            ParametricRuleDerivation::try_new(context, source_rows, elimination, limits)?;
        Self::try_from_derivation_pivot(context, &derivation, pivot_ordinal, sector, limits)
    }

    /// Compile one pivot from an already checked, shareable derivation.
    pub fn try_from_derivation_pivot(
        context: &ParametricCoefficientContext,
        derivation: &ParametricRuleDerivation,
        pivot_ordinal: usize,
        sector: SectorMask,
        limits: ParametricRuleLimits,
    ) -> Result<Self, ParametricRuleError> {
        let elimination = derivation.elimination();
        if context.fingerprint() != elimination.context_fingerprint() {
            return Err(ParametricRuleError::WrongContext);
        }
        if sector.arity() != context.index_count() {
            return Err(ParametricRuleError::WrongArity {
                expected: context.index_count(),
                actual: sector.arity(),
            });
        }
        let pivot = elimination.pivots().get(pivot_ordinal).ok_or(
            ParametricRuleError::PivotOutOfRange {
                pivot: pivot_ordinal,
                available: elimination.pivots().len(),
            },
        )?;
        let centered_relation = pivot.centered_relation(context, limits.arithmetic)?;
        if centered_relation.family_fingerprint() != elimination.family_fingerprint() {
            return Err(ParametricRuleError::WrongFamily);
        }
        verify_symbolic_unit_lhs(context, &centered_relation, limits.arithmetic)?;
        let rhs_terms = centered_relation.terms().len().saturating_sub(1);
        check_rule_limit("RHS terms", rhs_terms, limits.max_rhs_terms)?;
        check_rule_limit(
            "source rows retained for replay",
            elimination.stats().source_rows(),
            limits.max_source_rows_for_replay,
        )?;
        let discovery_anchor = elimination
            .ordering()
            .anchor()
            .iter()
            .zip(pivot.pivot().values())
            .enumerate()
            .map(|(position, (&anchor, &shift))| {
                anchor
                    .checked_add(shift)
                    .ok_or(ParametricRuleError::IndexOverflow { position })
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_boxed_slice();

        Ok(Self {
            derivation: derivation.clone(),
            family_fingerprint: Arc::from(elimination.family_fingerprint()),
            context_fingerprint: Arc::from(elimination.context_fingerprint()),
            sector,
            ordering: elimination.ordering().clone(),
            source_row_count: elimination.stats().source_rows(),
            source_manifest: Arc::from(elimination.source_manifest()),
            pivot_ordinal,
            original_pivot: pivot.pivot().clone(),
            trace: pivot.trace().clone(),
            centered_relation,
            discovery_anchor,
            limits,
        })
    }

    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }

    pub const fn derivation(&self) -> &ParametricRuleDerivation {
        &self.derivation
    }

    pub fn context_fingerprint(&self) -> &str {
        &self.context_fingerprint
    }

    pub const fn sector(&self) -> &SectorMask {
        &self.sector
    }

    pub const fn ordering(&self) -> &ParametricEliminationOrdering {
        &self.ordering
    }

    pub const fn source_row_count(&self) -> usize {
        self.source_row_count
    }

    pub fn source_manifest(&self) -> &str {
        &self.source_manifest
    }

    pub const fn pivot_ordinal(&self) -> usize {
        self.pivot_ordinal
    }

    pub const fn original_pivot(&self) -> &IndexShift {
        &self.original_pivot
    }

    pub const fn trace(&self) -> &ParametricEliminationTrace {
        &self.trace
    }

    pub const fn centered_relation(&self) -> &ParametricRelation {
        &self.centered_relation
    }

    pub fn discovery_anchor(&self) -> &[i64] {
        &self.discovery_anchor
    }

    pub const fn limits(&self) -> ParametricRuleLimits {
        self.limits
    }

    /// Replay the complete retained source system and reconstruct this pivot.
    /// This requires no external search buffer and is therefore the durable
    /// persistence boundary for an adaptively discovered rule.
    pub fn replay_retained(
        &self,
        context: &ParametricCoefficientContext,
    ) -> Result<(), ParametricRuleError> {
        self.derivation.replay(context)?;
        let replayed = Self::try_from_derivation_pivot(
            context,
            &self.derivation,
            self.pivot_ordinal,
            self.sector.clone(),
            self.limits,
        )?;
        self.verify_replayed_candidate(&replayed)
    }

    /// Independently replay the source elimination and reconstruct this rule.
    pub fn replay(
        &self,
        context: &ParametricCoefficientContext,
        elimination: &ParametricElimination,
        source_rows: &[ParametricRelation],
    ) -> Result<(), ParametricRuleError> {
        check_rule_limit(
            "source rows supplied for replay",
            source_rows.len(),
            self.limits.max_source_rows_for_replay,
        )?;
        elimination.replay(context, source_rows)?;
        let replayed = Self::try_from_elimination_pivot(
            context,
            source_rows,
            elimination,
            self.pivot_ordinal,
            self.sector.clone(),
            self.limits,
        )?;
        self.verify_replayed_candidate(&replayed)
    }

    fn verify_replayed_candidate(&self, replayed: &Self) -> Result<(), ParametricRuleError> {
        if replayed.family_fingerprint != self.family_fingerprint
            || replayed.context_fingerprint != self.context_fingerprint
            || replayed.ordering != self.ordering
            || replayed.source_row_count != self.source_row_count
            || replayed.source_manifest != self.source_manifest
            || replayed.original_pivot != self.original_pivot
            || replayed.trace != self.trace
            || replayed.discovery_anchor != self.discovery_anchor
            || !replayed
                .centered_relation
                .has_identical_guard_provenance(&self.centered_relation)
        {
            return Err(ParametricRuleError::ReplayMismatch);
        }
        Ok(())
    }

    /// Specialize and apply the rule at exact unshifted integer indices.
    ///
    /// Nonconstant kinematic guards are returned with the reduction; callers
    /// must preserve them as assumptions.  A zero specialized guard or a
    /// surviving non-descending RHS term makes the rule inapplicable.
    pub fn apply(
        &self,
        context: &ParametricCoefficientContext,
        indices: &[i64],
    ) -> Result<ParametricRuleApplication, ParametricRuleError> {
        if context.fingerprint() != self.context_fingerprint.as_ref() {
            return Err(ParametricRuleError::WrongContext);
        }
        if indices.len() != self.sector.arity() {
            return Err(ParametricRuleError::WrongArity {
                expected: self.sector.arity(),
                actual: indices.len(),
            });
        }
        if !self.sector.contains_indices(indices)? {
            return Ok(ParametricRuleApplication::Inapplicable(
                ParametricRuleInapplicability::OutsideSector,
            ));
        }

        let concrete =
            match self
                .centered_relation
                .specialize(context, indices, self.limits.arithmetic)
            {
                Ok(concrete) => concrete,
                Err(ParametricRelationError::UnsatisfiableDomain) => {
                    return Ok(ParametricRuleApplication::Inapplicable(
                        ParametricRuleInapplicability::NonzeroGuardVanished,
                    ));
                }
                Err(error) => return Err(error.into()),
            };
        let candidate = Arc::new(self.clone());
        match build_concrete_reduction(
            candidate,
            context,
            indices,
            concrete,
            self.limits.arithmetic.exact_algebra,
            self.limits.max_rhs_terms,
        )? {
            ConcreteReductionBuildOutcome::Applicable(reduction) => {
                Ok(ParametricRuleApplication::Applicable(reduction))
            }
            ConcreteReductionBuildOutcome::RhsSectorLeak {
                target,
                target_sector,
            } => Ok(ParametricRuleApplication::Inapplicable(
                ParametricRuleInapplicability::RhsSectorLeak {
                    target,
                    target_sector,
                },
            )),
            ConcreteReductionBuildOutcome::NonDescendingRhs { target } => {
                Ok(ParametricRuleApplication::Inapplicable(
                    ParametricRuleInapplicability::NonDescendingRhs { target },
                ))
            }
        }
    }

    /// Symbolic application without an integer assignment cannot decide
    /// sector boundaries or the coefficient-aware RHS descent predicate.
    pub const fn symbolic_applicability(&self) -> ParametricRuleApplication {
        ParametricRuleApplication::Undecidable(
            ParametricRuleUndecidability::ConcreteIndicesRequired,
        )
    }

    /// Specialize the exact unit-pivot equation without applying sector-leak
    /// or descent policy. LiteRed-style quotient providers use this checked
    /// surface to remove proved-zero terms and transport verified symmetry
    /// images before compiling a separate certified concrete rewrite.
    pub fn specialize_raw(
        &self,
        context: &ParametricCoefficientContext,
        indices: &[i64],
    ) -> Result<ConcreteRelation, ParametricRuleError> {
        if context.fingerprint() != self.context_fingerprint.as_ref() {
            return Err(ParametricRuleError::WrongContext);
        }
        if indices.len() != self.sector.arity() {
            return Err(ParametricRuleError::WrongArity {
                expected: self.sector.arity(),
                actual: indices.len(),
            });
        }
        if !self.sector.contains_indices(indices)? {
            return Err(ParametricRuleError::OutsideCandidateSector);
        }
        Ok(self
            .centered_relation
            .specialize(context, indices, self.limits.arithmetic)?)
    }
}

#[derive(Clone, Debug)]
pub struct ConcreteReduction {
    candidate: Arc<ParametricReductionRuleCandidate>,
    parametric_context: ParametricCoefficientContext,
    family_fingerprint: Arc<str>,
    pivot_ordinal: usize,
    source: ConcreteIntegralKey,
    rhs: BTreeMap<ConcreteIntegralKey, Coefficient>,
    descent: BTreeMap<ConcreteIntegralKey, StrictDescentWitness>,
    specialized_relation: ConcreteRelation,
}

impl ConcreteReduction {
    /// The exact parametric candidate specialized by this reduction.
    pub fn candidate(&self) -> &ParametricReductionRuleCandidate {
        &self.candidate
    }

    /// The candidate sector authenticated by this concrete reduction.
    pub fn sector(&self) -> &SectorMask {
        self.candidate.sector()
    }

    /// The candidate's authenticated strict-descent policy.
    pub fn ordering_policy(&self) -> IntegralOrderingPolicy {
        self.candidate.ordering().policy()
    }

    /// Exact `K(n)` identity used to specialize this persisted application.
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
        self.specialized_relation.guarded_nonzero_conditions()
    }

    pub fn descent_witnesses(&self) -> &BTreeMap<ConcreteIntegralKey, StrictDescentWitness> {
        &self.descent
    }

    pub const fn specialized_relation(&self) -> &ConcreteRelation {
        &self.specialized_relation
    }

    pub fn verify_descent(&self, policy: IntegralOrderingPolicy) -> bool {
        self.rhs.keys().eq(self.descent.keys())
            && self.descent.iter().all(|(target, witness)| {
                witness.policy() == policy
                    && witness.verify()
                    && policy
                        .complexity_key(self.source.powers())
                        .is_ok_and(|key| &key == witness.source())
                    && policy
                        .complexity_key(target.powers())
                        .is_ok_and(|key| &key == witness.target())
            })
    }

    /// Rebuild the retained candidate, re-specialize it at the
    /// authenticated integer assignment, and compare the entire application
    /// payload.
    /// The cheaper
    /// [`Self::verify_application`] checks only the already-specialized
    /// equation and is used by the demand engine, which has a base-field
    /// context but not the extended `K(n)` context.
    pub fn replay_application(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<bool, ParametricRuleError> {
        if family.fingerprint_ref() != self.family_fingerprint.as_ref()
            || self.parametric_context.fingerprint() != context.fingerprint()
            || self.candidate.family_fingerprint() != self.family_fingerprint.as_ref()
            || self.candidate.pivot_ordinal() != self.pivot_ordinal
            || !self.sector().contains_indices(self.source.powers())?
        {
            return Ok(false);
        }
        if self.candidate.context_fingerprint() != context.fingerprint() {
            return Ok(false);
        }
        self.candidate.replay_retained(context)?;
        let ParametricRuleApplication::Applicable(replayed) =
            self.candidate.apply(context, self.source.powers())?
        else {
            return Ok(false);
        };
        Ok(self.has_identical_application(&replayed))
    }

    fn has_identical_application(&self, other: &Self) -> bool {
        self.family_fingerprint == other.family_fingerprint
            && self.parametric_context.fingerprint() == other.parametric_context.fingerprint()
            && self.pivot_ordinal == other.pivot_ordinal
            && self.source == other.source
            && self.rhs == other.rhs
            && self.descent == other.descent
            && self
                .specialized_relation
                .has_identical_guard_provenance(&other.specialized_relation)
            && candidates_have_identical_payload(&self.candidate, &other.candidate)
    }

    /// Replay the solved equation and its complete specialized guard payload.
    pub fn verify_application(
        &self,
        context: &crate::algebra::CoefficientContext,
        policy: IntegralOrderingPolicy,
        limits: ExactAlgebraLimits,
    ) -> Result<bool, ExactAlgebraError> {
        if self.specialized_relation.family_fingerprint() != self.family_fingerprint.as_ref()
            || self.candidate.family_fingerprint() != self.family_fingerprint.as_ref()
            || self.candidate.pivot_ordinal() != self.pivot_ordinal
            || self.ordering_policy() != policy
            || !self
                .sector()
                .contains_indices(self.source.powers())
                .unwrap_or(false)
            || self.rhs.len() != self.descent.len()
            || !self.rhs.keys().eq(self.descent.keys())
            || !self.verify_descent(policy)
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
        for (target, solved) in &self.rhs {
            let target_sector = match SectorMask::try_from_indices(target.powers()) {
                Ok(sector) => sector,
                Err(_) => return Ok(false),
            };
            if !target_sector
                .is_subsector_of(self.sector())
                .unwrap_or(false)
            {
                return Ok(false);
            }
            context.validate_with_limits(solved, limits)?;
            let Some(equation_coefficient) = self.specialized_relation.terms().get(target) else {
                return Ok(false);
            };
            let expected = context.try_neg(solved, limits)?;
            if !context
                .try_sub(equation_coefficient, &expected, limits)?
                .is_zero()
            {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

enum ConcreteReductionBuildOutcome {
    Applicable(ConcreteReduction),
    RhsSectorLeak {
        target: ConcreteIntegralKey,
        target_sector: SectorMask,
    },
    NonDescendingRhs {
        target: ConcreteIntegralKey,
    },
}

fn candidates_have_identical_payload(
    left: &ParametricReductionRuleCandidate,
    right: &ParametricReductionRuleCandidate,
) -> bool {
    left.family_fingerprint == right.family_fingerprint
        && left.context_fingerprint == right.context_fingerprint
        && left.sector == right.sector
        && left.ordering == right.ordering
        && left.source_row_count == right.source_row_count
        && left.source_manifest == right.source_manifest
        && left.pivot_ordinal == right.pivot_ordinal
        && left.original_pivot == right.original_pivot
        && left.trace == right.trace
        && left.discovery_anchor == right.discovery_anchor
        && left
            .centered_relation
            .has_identical_guard_provenance(&right.centered_relation)
}

/// Build a concrete solved equation after the candidate has performed its
/// admissibility check and exact specialization.
fn build_concrete_reduction(
    candidate: Arc<ParametricReductionRuleCandidate>,
    context: &ParametricCoefficientContext,
    indices: &[i64],
    concrete: ConcreteRelation,
    exact_algebra: ExactAlgebraLimits,
    max_rhs_terms: usize,
) -> Result<ConcreteReductionBuildOutcome, ParametricRuleError> {
    if concrete.family_fingerprint() != candidate.family_fingerprint() {
        return Err(ParametricRuleError::WrongFamily);
    }
    let source = concrete
        .terms()
        .keys()
        .find(|key| key.powers() == indices)
        .cloned()
        .ok_or(ParametricRuleError::MissingConcreteLhs)?;
    let source_coefficient = concrete
        .terms()
        .get(&source)
        .ok_or(ParametricRuleError::MissingConcreteLhs)?;
    let unit_delta =
        context
            .base()
            .try_sub(source_coefficient, &context.base().one(), exact_algebra)?;
    if !unit_delta.is_zero() {
        return Err(ParametricRuleError::NonUnitConcreteLhs);
    }

    let rhs_count = concrete.terms().len().saturating_sub(1);
    check_rule_limit("specialized RHS terms", rhs_count, max_rhs_terms)?;
    let policy = candidate.ordering().policy();
    let mut rhs = BTreeMap::new();
    let mut descent = BTreeMap::new();
    for (target, coefficient) in concrete.terms() {
        if target == &source {
            continue;
        }
        let target_sector = SectorMask::try_from_indices(target.powers())?;
        if !target_sector.is_subsector_of(candidate.sector())? {
            return Ok(ConcreteReductionBuildOutcome::RhsSectorLeak {
                target: target.clone(),
                target_sector,
            });
        }
        let witness = match policy.prove_strict_descent(indices, target.powers()) {
            Ok(witness) => witness,
            Err(SectorFoundationError::NotStrictDescent) => {
                return Ok(ConcreteReductionBuildOutcome::NonDescendingRhs {
                    target: target.clone(),
                });
            }
            Err(error) => return Err(error.into()),
        };
        let solved_coefficient = context.base().try_neg(coefficient, exact_algebra)?;
        rhs.insert(target.clone(), solved_coefficient);
        descent.insert(target.clone(), witness);
    }

    let family_fingerprint = Arc::clone(&candidate.family_fingerprint);
    let pivot_ordinal = candidate.pivot_ordinal();
    Ok(ConcreteReductionBuildOutcome::Applicable(
        ConcreteReduction {
            candidate,
            parametric_context: context.clone(),
            family_fingerprint,
            pivot_ordinal,
            source,
            rhs,
            descent,
            specialized_relation: concrete,
        },
    ))
}

/// Three-valued rule guard outcome.  Concrete exact application produces
/// `Applicable` or `Inapplicable`; symbolic application is `Undecidable`.
#[derive(Clone, Debug)]
pub enum ParametricRuleApplication {
    Applicable(ConcreteReduction),
    Inapplicable(ParametricRuleInapplicability),
    Undecidable(ParametricRuleUndecidability),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParametricRuleInapplicability {
    OutsideSector,
    NonzeroGuardVanished,
    RhsSectorLeak {
        target: ConcreteIntegralKey,
        target_sector: SectorMask,
    },
    NonDescendingRhs {
        target: ConcreteIntegralKey,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParametricRuleUndecidability {
    ConcreteIndicesRequired,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParametricRuleError {
    WrongContext,
    WrongFamily,
    WrongArity {
        expected: usize,
        actual: usize,
    },
    PivotOutOfRange {
        pivot: usize,
        available: usize,
    },
    IndexOverflow {
        position: usize,
    },
    MissingSymbolicLhs,
    NonUnitSymbolicLhs,
    MissingConcreteLhs,
    NonUnitConcreteLhs,
    OutsideCandidateSector,
    ReplayMismatch,
    ResourceCountOverflow {
        resource: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ExactAlgebra(ExactAlgebraError),
    ParametricCoefficient(ParametricCoefficientError),
    Relation(ParametricRelationError),
    Elimination(ParametricEliminationError),
    Sector(SectorFoundationError),
}

impl fmt::Display for ParametricRuleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongContext => formatter.write_str("parametric rule context mismatch"),
            Self::WrongFamily => formatter.write_str("parametric rule family mismatch"),
            Self::WrongArity { expected, actual } => {
                write!(
                    formatter,
                    "parametric rule arity is {actual}, expected {expected}"
                )
            }
            Self::PivotOutOfRange { pivot, available } => write!(
                formatter,
                "parametric pivot {pivot} is outside {available} available pivots"
            ),
            Self::IndexOverflow { position } => {
                write!(
                    formatter,
                    "parametric rule index overflow at position {position}"
                )
            }
            Self::MissingSymbolicLhs => {
                formatter.write_str("centered parametric rule has no zero-shift LHS")
            }
            Self::NonUnitSymbolicLhs => {
                formatter.write_str("centered parametric rule LHS is not exactly one")
            }
            Self::MissingConcreteLhs => {
                formatter.write_str("specialized parametric rule has no source integral")
            }
            Self::NonUnitConcreteLhs => {
                formatter.write_str("specialized parametric rule source coefficient is not one")
            }
            Self::OutsideCandidateSector => {
                formatter.write_str("raw specialization lies outside the candidate sector")
            }
            Self::ReplayMismatch => {
                formatter.write_str("parametric rule differs after elimination replay")
            }
            Self::ResourceCountOverflow { resource } => {
                write!(
                    formatter,
                    "parametric rule {resource} count overflowed usize"
                )
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "parametric rule {resource} requested {requested}, configured limit is {limit}"
            ),
            Self::ExactAlgebra(error) => error.fmt(formatter),
            Self::ParametricCoefficient(error) => error.fmt(formatter),
            Self::Relation(error) => error.fmt(formatter),
            Self::Elimination(error) => error.fmt(formatter),
            Self::Sector(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ParametricRuleError {}

impl From<ExactAlgebraError> for ParametricRuleError {
    fn from(value: ExactAlgebraError) -> Self {
        Self::ExactAlgebra(value)
    }
}

impl From<ParametricCoefficientError> for ParametricRuleError {
    fn from(value: ParametricCoefficientError) -> Self {
        Self::ParametricCoefficient(value)
    }
}

impl From<ParametricRelationError> for ParametricRuleError {
    fn from(value: ParametricRelationError) -> Self {
        Self::Relation(value)
    }
}

impl From<ParametricEliminationError> for ParametricRuleError {
    fn from(value: ParametricEliminationError) -> Self {
        Self::Elimination(value)
    }
}

impl From<SectorFoundationError> for ParametricRuleError {
    fn from(value: SectorFoundationError) -> Self {
        Self::Sector(value)
    }
}

fn verify_symbolic_unit_lhs(
    context: &ParametricCoefficientContext,
    relation: &ParametricRelation,
    limits: ParametricArithmeticLimits,
) -> Result<(), ParametricRuleError> {
    let zero = crate::IndexSpace::try_new(relation.arity())?.zero();
    let coefficient = relation
        .terms()
        .get(&zero)
        .ok_or(ParametricRuleError::MissingSymbolicLhs)?;
    let delta = context.sub_with_limits(coefficient, &context.one(), limits.exact_algebra)?;
    if delta.is_zero() {
        Ok(())
    } else {
        Err(ParametricRuleError::NonUnitSymbolicLhs)
    }
}

fn check_rule_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ParametricRuleError> {
    if requested > limit {
        Err(ParametricRuleError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}
