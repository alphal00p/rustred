//! Core-owned exact composition around guarded, existing single-key programs.
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use super::arithmetic::{Conditions, accumulate_with_admission};
use super::model::*;
use crate::algebra::{Coefficient, CoefficientContext};
use crate::family::{IntegralFamily, IntegralKey};
use crate::reduction::terminal_normalization::VacuumIntegralKey;
use crate::reduction::{
    CacheCensus, CacheWeight, ReductionLimits, ReductionRequest, SharedCacheBudget,
    coefficient_cache_weight,
};
use crate::sector::symmetry::integral_transport;
type Error = VacuumProductReductionError;

/// Exact finite weighted application through an existing guarded single-key
/// evaluator. This helper owns coefficient substitution and coalescing; it does
/// not mint reduction coverage. Any separate entry-domain policy remains the
/// caller's responsibility before passing inputs here. The evaluator must obey
/// the remaining limits and clear transient caches on success AND error.
pub fn reduce_vacuum_factor_combination<E: fmt::Display>(
    family: Arc<IntegralFamily>,
    terms: impl IntoIterator<Item = (IntegralKey, Coefficient)>,
    nonzero_conditions: impl IntoIterator<Item = Coefficient>,
    limits: ReductionLimits,
    mut evaluate: impl FnMut(&IntegralKey, ReductionLimits) -> Result<VacuumFactorReduction, E>,
) -> Result<VacuumFactorReduction, Error> {
    let fingerprint = family.fingerprint().to_owned();
    reduce_with_cache_census(
        family,
        terms,
        nonzero_conditions,
        limits,
        |key, remaining| {
            evaluate(key, remaining)
                .map(|result| (result, CacheCensus::default()))
                .map_err(|error| Error::Provider {
                    family: fingerprint.clone(),
                    target: key.clone(),
                    message: error.to_string(),
                })
        },
    )
}

/// The only cache-owning caller clears its native cache before entering and
/// after leaving this request. Reported census is inclusive, not a delta.
pub(super) fn reduce_with_cache_census(
    family: Arc<IntegralFamily>,
    terms: impl IntoIterator<Item = (IntegralKey, Coefficient)>,
    nonzero_conditions: impl IntoIterator<Item = Coefficient>,
    limits: ReductionLimits,
    mut evaluate: impl FnMut(
        &IntegralKey,
        ReductionLimits,
    ) -> Result<(VacuumFactorReduction, CacheCensus), Error>,
) -> Result<VacuumFactorReduction, Error> {
    let policy = VacuumProductReductionLimits {
        reduction: limits,
        ..Default::default()
    };
    let context = family.coefficient_context();
    validate_family(&family, &family)?;
    let mut conditions = Conditions::default();
    retain(
        &mut conditions,
        context,
        nonzero_conditions,
        policy,
        0,
        CacheWeight::default(),
    )?;
    retain(
        &mut conditions,
        context,
        family
            .domain()
            .conditions()
            .map(|c| c.polynomial().clone().into()),
        policy,
        0,
        CacheWeight::default(),
    )?;
    let mut algebra = ReductionRequest::default();
    let mut statistics = VacuumProductReductionStatistics::default();
    let mut inputs = BTreeMap::new();
    let mut input_count = 0usize;
    for (key, coefficient) in terms {
        super::arithmetic::charge(
            &mut input_count,
            1,
            limits.max_factorization_terms,
            "factor combination inputs",
        )?;
        validate_key(&family, &key)?;
        context.validate_with_limits(&coefficient, limits.exact_algebra)?;
        let fixed = coefficient_cache_weight(inputs.values().chain(std::iter::once(&coefficient)))?;
        retain_denominator(
            &mut conditions,
            context,
            &coefficient,
            policy,
            inputs.len().saturating_add(1),
            fixed,
        )?;
        let fixed = coefficient_cache_weight(
            inputs
                .values()
                .chain(conditions.values())
                .chain(conditions.values()),
        )?;
        let retained_keys = inputs.len();
        accumulate_with_admission(
            context,
            &mut inputs,
            key,
            coefficient,
            policy,
            &mut algebra,
            &mut statistics,
            |old, value| {
                admit(
                    retained_keys.saturating_add(usize::from(old.is_none())),
                    fixed.checked_add(coefficient_cache_weight([value])?)?,
                    limits,
                )
            },
        )?;
    }
    let mut output = BTreeMap::<VacuumIntegralKey, Coefficient>::new();
    let mut families = BTreeMap::<String, Arc<IntegralFamily>>::new();
    let mut cache_hits = 0usize;
    let mut native_cache = CacheCensus::default();
    for (key, weight) in &inputs {
        let retained = coefficient_cache_weight(
            inputs
                .values()
                .chain(output.values())
                .chain(conditions.values())
                .chain(conditions.values()),
        )?;
        let retained_keys = inputs.len().saturating_add(output.len());
        admit(
            retained_keys.saturating_add(native_cache.integrals),
            retained.checked_add(cache_weight(native_cache))?,
            limits,
        )?;
        // The native grant INCLUDES its warm cache. Subtract batch retention
        // only; subtracting the native census here would charge it twice.
        let remaining = ReductionLimits {
            max_rule_applications: algebra
                .remaining_rule_applications(limits.max_rule_applications),
            max_coalescing_additions: algebra
                .remaining_coalescing_additions(limits.max_coalescing_additions),
            max_cached_integrals: limits.max_cached_integrals.saturating_sub(retained_keys),
            max_cached_coefficient_terms: limits
                .max_cached_coefficient_terms
                .saturating_sub(retained.coefficient_terms),
            max_cached_coefficient_bytes: limits
                .max_cached_coefficient_bytes
                .saturating_sub(retained.coefficient_bytes),
            ..limits
        };
        let (result, census) = evaluate(key, remaining)?;
        native_cache = census;
        let retained = retained.checked_add(cache_weight(native_cache))?;
        let retained_keys = retained_keys.saturating_add(native_cache.integrals);
        algebra.record_rule_applications(
            result.work.rule_applications,
            limits.max_rule_applications,
        )?;
        algebra.record_coalescing_additions(
            result.work.coalescing_additions,
            limits.max_coalescing_additions,
        )?;
        statistics.rule_applications = statistics
            .rule_applications
            .saturating_add(result.work.rule_applications);
        statistics.coalescing_additions = statistics
            .coalescing_additions
            .saturating_add(result.work.coalescing_additions);
        cache_hits = cache_hits.saturating_add(result.work.cache_hits);
        if result.terms.len() > limits.max_factorization_terms {
            return Err(Error::ResourceLimit {
                resource: "factor combination result rows",
                requested: result.terms.len(),
                limit: limits.max_factorization_terms,
            });
        }
        let returned = coefficient_cache_weight(
            result
                .terms
                .iter()
                .map(|term| &term.coefficient)
                .chain(&result.nonzero_conditions),
        )?;
        admit(
            retained_keys.saturating_add(result.terms.len()),
            retained.checked_add(returned)?,
            limits,
        )?;
        let returned_keys = result.terms.len();
        let fixed = coefficient_cache_weight(inputs.values().chain(output.values()))?
            .checked_add(returned)?
            .checked_add(cache_weight(native_cache))?;
        retain(
            &mut conditions,
            context,
            result.nonzero_conditions,
            policy,
            retained_keys.saturating_add(returned_keys),
            fixed,
        )?;
        for term in result.terms {
            validate_family(&family, &term.family)?;
            validate_key(&term.family, &term.integral)?;
            let fixed = coefficient_cache_weight(inputs.values().chain(output.values()))?
                .checked_add(returned)?
                .checked_add(cache_weight(native_cache))?;
            let retained_keys = inputs
                .len()
                .saturating_add(output.len())
                .saturating_add(returned_keys)
                .saturating_add(native_cache.integrals);
            retain(
                &mut conditions,
                context,
                term.family
                    .domain()
                    .conditions()
                    .map(|c| c.polynomial().clone().into()),
                policy,
                retained_keys,
                fixed,
            )?;
            context.validate_with_limits(&term.coefficient, limits.exact_algebra)?;
            retain_denominator(
                &mut conditions,
                context,
                &term.coefficient,
                policy,
                retained_keys,
                fixed,
            )?;
            let key = VacuumIntegralKey::from_family(&term.family, term.integral);
            families
                .entry(term.family.fingerprint().to_owned())
                .or_insert(term.family);
            let coefficient = context.try_mul(weight, &term.coefficient, limits.exact_algebra)?;
            let fixed = fixed.checked_add(coefficient_cache_weight(
                conditions.values().iter().chain(conditions.values()),
            )?)?;
            accumulate_with_admission(
                context,
                &mut output,
                key,
                coefficient,
                policy,
                &mut algebra,
                &mut statistics,
                |old, value| {
                    admit(
                        retained_keys.saturating_add(usize::from(old.is_none())),
                        fixed.checked_add(coefficient_cache_weight([value])?)?,
                        limits,
                    )
                },
            )?;
        }
    }
    let retained = coefficient_cache_weight(
        inputs
            .values()
            .chain(output.values())
            .chain(conditions.values())
            .chain(conditions.values()),
    )?;
    admit(
        inputs
            .len()
            .saturating_add(output.len())
            .saturating_add(native_cache.integrals),
        retained.checked_add(cache_weight(native_cache))?,
        limits,
    )?;
    Ok(VacuumFactorReduction {
        terms: output
            .into_iter()
            .map(|(key, coefficient)| VacuumFactorTerm {
                family: families[key.family_fingerprint()].clone(),
                integral: key.integral().clone(),
                coefficient,
            })
            .collect(),
        nonzero_conditions: conditions.into_values(),
        work: VacuumFactorWork {
            rule_applications: statistics.rule_applications,
            coalescing_additions: statistics.coalescing_additions,
            cache_hits,
        },
    })
}

fn cache_weight(census: CacheCensus) -> CacheWeight {
    CacheWeight {
        coefficient_terms: census.coefficient_terms,
        coefficient_bytes: census.coefficient_bytes,
    }
}

fn retain(
    conditions: &mut Conditions,
    context: &CoefficientContext,
    values: impl IntoIterator<Item = Coefficient>,
    policy: VacuumProductReductionLimits,
    fixed_keys: usize,
    fixed: CacheWeight,
) -> Result<(), Error> {
    conditions.extend_with_admission(context, values, policy, |previous, next| {
        let guards = coefficient_cache_weight(previous.iter().chain(std::iter::once(next)))?;
        // Conditions owns both an ordered Vec and a collision-safe HashSet.
        // Admit BOTH coefficient copies before either allocation is retained.
        admit(
            fixed_keys,
            fixed.checked_add(guards)?.checked_add(guards)?,
            policy.reduction,
        )
    })
}

fn retain_denominator(
    conditions: &mut Conditions,
    context: &CoefficientContext,
    value: &Coefficient,
    policy: VacuumProductReductionLimits,
    fixed_keys: usize,
    fixed: CacheWeight,
) -> Result<(), Error> {
    if !value.denominator.is_constant() {
        retain(
            conditions,
            context,
            [value.denominator.clone().into()],
            policy,
            fixed_keys,
            fixed,
        )?;
    }
    Ok(())
}

/// Apply a previously verified unit-Jacobian family transport and compose its
/// finite weighted endpoints with existing lower programs. Every negative row
/// uses native integral transport; no numerator mapping lives in the adapter.
pub fn reduce_transported_vacuum_factor<E: fmt::Display>(
    transport: &integral_transport::Prepared,
    source: &IntegralKey,
    target_family: Arc<IntegralFamily>,
    expansion_limits: integral_transport::ExpansionLimits,
    limits: ReductionLimits,
    evaluate: impl FnMut(&IntegralKey, ReductionLimits) -> Result<VacuumFactorReduction, E>,
) -> Result<VacuumFactorReduction, Error> {
    let terms = transported_terms(transport, source, &target_family, expansion_limits, limits)?;
    reduce_vacuum_factor_combination(
        target_family,
        terms,
        transport
            .verified_map()
            .nonzero_conditions()
            .iter()
            .map(|condition| condition.polynomial().clone().into()),
        limits,
        evaluate,
    )
}

pub(super) fn transported_terms(
    transport: &integral_transport::Prepared,
    source: &IntegralKey,
    target_family: &IntegralFamily,
    expansion_limits: integral_transport::ExpansionLimits,
    limits: ReductionLimits,
) -> Result<Vec<(IntegralKey, Coefficient)>, Error> {
    if target_family.fingerprint() != transport.target_family().fingerprint() {
        return Err(Error::InvalidInput(
            "factor transport target family differs".into(),
        ));
    }
    let mapped = transport
        .transport(source, expansion_limits)
        .map_err(|error| Error::InvalidInput(format!("factor transport failed: {error}")))?;
    let mapped_weight =
        coefficient_cache_weight(mapped.terms().iter().map(|term| term.coefficient()))?;
    // The native transport currently exposes borrowed endpoints. Admit its
    // short-lived copy and drop the source before granting callback budgets.
    admit(
        mapped.terms().len().saturating_mul(2),
        mapped_weight.checked_add(mapped_weight)?,
        limits,
    )?;
    let terms = mapped
        .terms()
        .iter()
        .map(|term| (term.key().clone(), term.coefficient().clone()))
        .collect::<Vec<_>>();
    drop(mapped);
    Ok(terms)
}

fn validate_key(family: &IntegralFamily, key: &IntegralKey) -> Result<(), Error> {
    if key.powers().len() != family.denominator_count() {
        return Err(Error::InvalidInput(
            "factor combination key has foreign arity".into(),
        ));
    }
    Ok(())
}
fn validate_family(anchor: &IntegralFamily, family: &IntegralFamily) -> Result<(), Error> {
    if anchor.loop_count() != family.loop_count()
        || anchor.dimension() != family.dimension()
        || !anchor
            .coefficient_context()
            .has_same_variable_map(family.coefficient_context())
        || family.external_count() != 0
        || family.power_shifts().iter().any(|p| !p.is_zero())
    {
        return Err(Error::InvalidInput("factor combination family differs in dimension, coefficient map, loop count, vacuum kinematics or shifts".into()));
    }
    Ok(())
}
fn admit(keys: usize, weight: CacheWeight, limits: ReductionLimits) -> Result<(), Error> {
    SharedCacheBudget::default().replace(0, CacheWeight::default(), keys, weight, limits)?;
    Ok(())
}
