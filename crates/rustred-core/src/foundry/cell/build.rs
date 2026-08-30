use std::collections::BTreeSet;

use crate::algebra::indexed::IntegerZeroSetResolution;
use crate::algebra::{
    IndexedAlgebraLimits, IndexedCoefficientContext, IndexedGuardLimits, IndexedPolynomial,
};
use crate::foundry::parametric::ParametricRule;
use crate::identity::TranslatedSourceBatch;
use crate::sector::{InteriorBounds, SectorInteriorDomain, SectorMonotoneDomain};

use super::{
    FixedIndexRestriction, RuleCell, RuleCellDomainProof, RuleCellError, RuleCellGuard,
    RuleCellLimits, RuleCellTerm, SourceViewBatch, SourceViewProvenance,
};

impl SourceViewBatch {
    pub fn try_select(
        translated: TranslatedSourceBatch,
        ordinals: &[usize],
        limits: RuleCellLimits,
    ) -> Result<Self, RuleCellError> {
        if ordinals.is_empty() {
            return Err(RuleCellError::EmptySourceSelection);
        }
        check_limit("source views", ordinals.len(), limits.max_source_views)?;
        let (family_fingerprint, context_fingerprint, sources) = translated.into_foundry_parts();
        let available = sources.len();
        let mut slots = sources.into_iter().map(Some).collect::<Vec<_>>();
        let mut relations = Vec::new();
        let mut provenance = Vec::new();
        relations.try_reserve_exact(ordinals.len()).map_err(|_| {
            RuleCellError::AllocationFailure {
                resource: "source-view relations",
                requested: ordinals.len(),
            }
        })?;
        provenance.try_reserve_exact(ordinals.len()).map_err(|_| {
            RuleCellError::AllocationFailure {
                resource: "source-view provenance",
                requested: ordinals.len(),
            }
        })?;
        for &ordinal in ordinals {
            let slot = slots
                .get_mut(ordinal)
                .ok_or(RuleCellError::SourceOrdinalOutOfRange { ordinal, available })?;
            let source = slot
                .take()
                .ok_or(RuleCellError::DuplicateSourceOrdinal { ordinal })?;
            let (relation, translated) = source.into_foundry_parts();
            relations.push(relation);
            provenance.push(SourceViewProvenance {
                translated,
                symmetry: None,
            });
        }
        Ok(Self {
            family_fingerprint,
            context_fingerprint,
            relations,
            provenance,
            construction: super::SourceViewConstruction::Direct,
        })
    }
}

impl RuleCell {
    pub fn try_tightened(
        context: &IndexedCoefficientContext,
        rule: ParametricRule,
        sources: SourceViewBatch,
        application: SectorInteriorDomain,
        limits: RuleCellLimits,
    ) -> Result<Self, RuleCellError> {
        validate_bindings(context, &rule, &sources)?;
        if application.arity() != rule.domain().arity() {
            return Err(RuleCellError::WrongApplicationArity {
                expected: rule.domain().arity(),
                actual: application.arity(),
            });
        }
        if application.sector() != rule.sector() {
            return Err(RuleCellError::ApplicationSectorMismatch);
        }
        if application
            .bounds()
            .iter()
            .zip(rule.domain().bounds())
            .any(|(inner, outer)| inner.lower() < outer.lower() || inner.upper() > outer.upper())
        {
            return Err(RuleCellError::ApplicationNotTightened);
        }
        let rhs = rule
            .right_hand_side()
            .iter()
            .map(|term| term.shift().values())
            .collect::<Vec<_>>();
        let domain = SectorMonotoneDomain::try_new_for_rule(
            application.sector().clone(),
            application.bounds().iter().copied(),
            rule.pivot().values(),
            &rhs,
        )?;
        build(
            context,
            rule,
            sources,
            domain,
            RuleCellDomainProof::TightenedOriginalInterior,
            Vec::new(),
            Vec::new(),
            limits,
        )
    }

    pub fn try_refined(
        context: &IndexedCoefficientContext,
        rule: ParametricRule,
        sources: SourceViewBatch,
        application_domain: SectorMonotoneDomain,
        fixed: impl IntoIterator<Item = FixedIndexRestriction>,
        pruned_rhs_ordinals: impl IntoIterator<Item = usize>,
        limits: RuleCellLimits,
    ) -> Result<Self, RuleCellError> {
        validate_bindings(context, &rule, &sources)?;
        build(
            context,
            rule,
            sources,
            application_domain,
            RuleCellDomainProof::ReprovedSectorMonotone,
            fixed.into_iter().collect(),
            pruned_rhs_ordinals.into_iter().collect(),
            limits,
        )
    }
}

fn build(
    context: &IndexedCoefficientContext,
    rule: ParametricRule,
    sources: SourceViewBatch,
    application_domain: SectorMonotoneDomain,
    domain_proof: RuleCellDomainProof,
    mut fixed: Vec<FixedIndexRestriction>,
    mut pruned: Vec<usize>,
    limits: RuleCellLimits,
) -> Result<RuleCell, RuleCellError> {
    if application_domain.arity() != rule.domain().arity() {
        return Err(RuleCellError::WrongApplicationArity {
            expected: rule.domain().arity(),
            actual: application_domain.arity(),
        });
    }
    if application_domain.sector() != rule.sector() {
        return Err(RuleCellError::ApplicationSectorMismatch);
    }
    check_limit(
        "fixed restrictions",
        fixed.len(),
        limits.max_fixed_restrictions,
    )?;
    check_limit("pruned RHS terms", pruned.len(), limits.max_pruned_terms)?;
    fixed.sort_unstable();
    for window in fixed.windows(2) {
        if window[0].position() == window[1].position() {
            return Err(RuleCellError::DuplicateFixedPosition {
                position: window[0].position(),
            });
        }
    }
    for restriction in &fixed {
        let bounds = application_domain
            .bounds()
            .get(restriction.position())
            .ok_or(RuleCellError::FixedRestrictionMismatch {
                position: restriction.position(),
            })?;
        if bounds.lower() != restriction.value() || bounds.upper() != restriction.value() {
            return Err(RuleCellError::FixedRestrictionMismatch {
                position: restriction.position(),
            });
        }
    }
    pruned.sort_unstable();
    for window in pruned.windows(2) {
        if window[0] == window[1] {
            return Err(RuleCellError::DuplicatePrunedTerm { ordinal: window[0] });
        }
    }
    let fixed_pairs = fixed
        .iter()
        .map(|item| (item.position(), item.value()))
        .collect::<Vec<_>>();
    let available = rule.right_hand_side().len();
    for &ordinal in &pruned {
        let term = rule
            .right_hand_side()
            .get(ordinal)
            .ok_or(RuleCellError::PrunedTermOutOfRange { ordinal, available })?;
        let (coefficient, _guard) = context.specialize_fixed_indices_sealed(
            term.coefficient(),
            &fixed_pairs,
            limits.indexed_algebra,
        )?;
        if !coefficient.is_zero() {
            return Err(RuleCellError::PrunedTermNotZero { ordinal });
        }
    }
    let pruned_set = pruned.iter().copied().collect::<BTreeSet<_>>();
    let retained_count =
        available
            .checked_sub(pruned.len())
            .ok_or(RuleCellError::ResourceCountOverflow {
                resource: "retained RHS terms",
            })?;
    if retained_count == 0 {
        return Err(RuleCellError::EmptyRetainedRule);
    }
    check_limit(
        "retained RHS terms",
        retained_count,
        limits.max_retained_terms,
    )?;
    let mut terms = Vec::new();
    terms
        .try_reserve_exact(retained_count)
        .map_err(|_| RuleCellError::AllocationFailure {
            resource: "retained RHS terms",
            requested: retained_count,
        })?;
    for (ordinal, term) in rule.right_hand_side().iter().enumerate() {
        if pruned_set.contains(&ordinal) {
            continue;
        }
        let descent = rule.ordering().prove_sector_monotone_shift_descent(
            &application_domain,
            rule.pivot().values(),
            term.shift().values(),
        )?;
        if !descent.verify() {
            return Err(RuleCellError::Sector(
                crate::sector::Error::NotStrictDescent,
            ));
        }
        terms.push(RuleCellTerm {
            source_rhs_ordinal: ordinal,
            descent,
        });
    }
    check_limit(
        "rule guards",
        rule.nonzero_guards().len(),
        limits.max_guards,
    )?;
    let mut guards = Vec::new();
    guards
        .try_reserve_exact(rule.nonzero_guards().len())
        .map_err(|_| RuleCellError::AllocationFailure {
            resource: "rule guards",
            requested: rule.nonzero_guards().len(),
        })?;
    for (ordinal, guard) in rule.nonzero_guards().iter().enumerate() {
        let polynomial = context
            .specialize_fixed_polynomial_sealed(
                guard.polynomial(),
                &fixed_pairs,
                limits.indexed_algebra,
            )
            .map_err(|source| RuleCellError::GuardAlgebra { ordinal, source })?;
        validate_guard_on_bounds(
            context,
            ordinal,
            &polynomial,
            application_domain.bounds(),
            limits.indexed_algebra,
            limits.guard_algebra,
        )?;
        guards.push(RuleCellGuard {
            source_guard_ordinal: ordinal,
            polynomial,
        });
    }
    Ok(RuleCell::from_parts(
        rule,
        sources,
        application_domain,
        domain_proof,
        fixed.into_boxed_slice(),
        pruned.into_boxed_slice(),
        terms.into_boxed_slice(),
        guards.into_boxed_slice(),
    ))
}

pub(super) fn validate_guard_on_bounds(
    context: &IndexedCoefficientContext,
    ordinal: usize,
    polynomial: &IndexedPolynomial,
    bounds: &[InteriorBounds],
    limits: IndexedAlgebraLimits,
    guard_limits: IndexedGuardLimits,
) -> Result<(), RuleCellError> {
    if bounds.len() != context.index_count() {
        return Err(RuleCellError::WrongApplicationArity {
            expected: context.index_count(),
            actual: bounds.len(),
        });
    }
    let coefficient_system = context
        .base_coefficient_system(polynomial, limits, guard_limits)
        .map_err(|source| RuleCellError::GuardAlgebra { ordinal, source })?;
    let integer_locus = match context
        .univariate_integer_zero_set(&coefficient_system, guard_limits)
        .map_err(|source| RuleCellError::GuardAlgebra { ordinal, source })?
    {
        IntegerZeroSetResolution::IdenticallyZero => {
            return Err(RuleCellError::GuardIdenticallyZero { ordinal });
        }
        IntegerZeroSetResolution::UnsupportedMultivariate => {
            return Err(RuleCellError::UnsupportedMultivariateGuardLocus { ordinal });
        }
        IntegerZeroSetResolution::Exact(locus) => locus,
    };
    if let Some(position) = integer_locus.index_position() {
        let bounds = bounds[position];
        for root in integer_locus.roots() {
            if let Some(value) = root.to_i64()
                && bounds.contains(value)
            {
                return Err(RuleCellError::GuardVanishesInApplicationDomain {
                    ordinal,
                    position,
                    value,
                });
            }
        }
    }
    Ok(())
}

fn validate_bindings(
    context: &IndexedCoefficientContext,
    rule: &ParametricRule,
    sources: &SourceViewBatch,
) -> Result<(), RuleCellError> {
    if rule.family_fingerprint() != sources.family_fingerprint() {
        return Err(RuleCellError::ForeignFamily);
    }
    if rule.context_fingerprint() != context.fingerprint()
        || sources.context_fingerprint() != context.fingerprint()
    {
        return Err(RuleCellError::ForeignContext);
    }
    for (contribution, source) in rule.source_combination().iter().enumerate() {
        let relation = sources
            .relations()
            .get(source.source_ordinal())
            .ok_or(RuleCellError::SourceReplayMismatch { contribution })?;
        if relation.row_id() != source.row_id() {
            return Err(RuleCellError::SourceReplayMismatch { contribution });
        }
    }
    Ok(())
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), RuleCellError> {
    if requested > limit {
        Err(RuleCellError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

#[allow(dead_code)]
fn singleton(value: i64) -> InteriorBounds {
    InteriorBounds::new(value, value)
}
