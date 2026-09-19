//! Private coefficient representation within the existing application DAG.

use std::collections::{BTreeMap, btree_map::Entry};

use crate::algebra::{Coefficient, CoefficientContext, FactorizedCoefficient};
use crate::family::IntegralKey;
use crate::reduction::{
    CacheWeight, ReductionError, ReductionLimits, ReductionRequest, ReductionStatistics,
    accumulate_master_in_request, coefficient_cache_weight,
};

use super::{CandidateCacheRepresentation, CandidateReductionError};

#[derive(Debug)]
pub(super) enum CachedTerms {
    Sparse(BTreeMap<IntegralKey, Coefficient>),
    Factorized(BTreeMap<IntegralKey, FactorizedCoefficient>),
}

#[derive(Debug)]
pub(super) struct CacheEntry {
    pub terms: CachedTerms,
    pub weight: CacheWeight,
}

impl CachedTerms {
    pub fn from_sparse(
        terms: BTreeMap<IntegralKey, Coefficient>,
        representation: CandidateCacheRepresentation,
        context: &CoefficientContext,
        limits: ReductionLimits,
    ) -> Result<Self, CandidateReductionError> {
        match representation {
            CandidateCacheRepresentation::Sparse => Ok(Self::Sparse(terms)),
            CandidateCacheRepresentation::Factorized => Ok(Self::Factorized(
                terms
                    .into_iter()
                    .map(|(key, coefficient)| {
                        Ok((
                            key,
                            FactorizedCoefficient::from_coefficient(
                                context,
                                coefficient,
                                limits.exact_algebra,
                            )?,
                        ))
                    })
                    .collect::<Result<_, CandidateReductionError>>()?,
            )),
        }
    }

    pub fn materialize(
        &self,
        context: &CoefficientContext,
        limits: ReductionLimits,
    ) -> Result<BTreeMap<IntegralKey, Coefficient>, CandidateReductionError> {
        match self {
            Self::Sparse(terms) => Ok(terms.clone()),
            Self::Factorized(terms) => terms
                .iter()
                .map(|(key, coefficient)| {
                    Ok((
                        key.clone(),
                        coefficient.materialize(context, limits.exact_algebra)?,
                    ))
                })
                .collect(),
        }
    }

    pub fn terminal_keys(&self) -> impl Iterator<Item = &IntegralKey> {
        let sparse = if let Self::Sparse(terms) = self {
            Some(terms)
        } else {
            None
        };
        let factorized = if let Self::Factorized(terms) = self {
            Some(terms)
        } else {
            None
        };
        sparse
            .into_iter()
            .flat_map(|terms| terms.keys())
            .chain(factorized.into_iter().flat_map(|terms| terms.keys()))
    }

    pub fn weight(&self) -> Result<CacheWeight, CandidateReductionError> {
        match self {
            Self::Sparse(terms) => Ok(coefficient_cache_weight(terms.values())?),
            Self::Factorized(terms) => {
                terms
                    .values()
                    .try_fold(CacheWeight::default(), |sum, value| {
                        let (coefficient_terms, coefficient_bytes) = value.cache_weight()?;
                        Ok(sum.checked_add(CacheWeight {
                            coefficient_terms,
                            coefficient_bytes,
                        })?)
                    })
            }
        }
    }
}

pub(super) fn combine(
    context: &CoefficientContext,
    cache: &BTreeMap<IntegralKey, CacheEntry>,
    representation: CandidateCacheRepresentation,
    terms: BTreeMap<IntegralKey, Coefficient>,
    limits: ReductionLimits,
    request: &mut ReductionRequest,
    statistics: &mut ReductionStatistics,
) -> Result<CachedTerms, CandidateReductionError> {
    match representation {
        CandidateCacheRepresentation::Sparse => {
            let mut output = BTreeMap::new();
            for (child, factor) in terms {
                let CachedTerms::Sparse(child) = &child_entry(cache, &child)?.terms else {
                    return Err(mixed_representation());
                };
                for (terminal, coefficient) in child {
                    let contribution =
                        context.try_mul(&factor, coefficient, limits.exact_algebra)?;
                    accumulate_master_in_request(
                        context,
                        &mut output,
                        terminal,
                        contribution,
                        limits,
                        request,
                        statistics,
                    )?;
                }
            }
            Ok(CachedTerms::Sparse(output))
        }
        CandidateCacheRepresentation::Factorized => {
            let mut output = BTreeMap::new();
            for (child, factor) in terms {
                let CachedTerms::Factorized(child) = &child_entry(cache, &child)?.terms else {
                    return Err(mixed_representation());
                };
                // Rule conditions, original poles and descent were checked by
                // apply_candidate before this frame. An empty child requires
                // no coefficient algebra, just as in the ordinary branch.
                if child.is_empty() {
                    continue;
                }
                // Only the specialized edge factor is converted. Descendant
                // decompositions have never been expanded into ordinary RPs.
                let factor =
                    FactorizedCoefficient::from_coefficient(context, factor, limits.exact_algebra)?;
                for (terminal, coefficient) in child {
                    let contribution =
                        factor.try_mul(coefficient, context, limits.exact_algebra)?;
                    if contribution.is_zero() {
                        continue;
                    }
                    match output.entry(terminal.clone()) {
                        Entry::Vacant(entry) => {
                            entry.insert(contribution);
                        }
                        Entry::Occupied(mut entry) => {
                            // Match the ordinary applier's rejected-attempt work
                            // accounting and request-wide coalescing boundary.
                            statistics.record_coalescing_additions(1);
                            request
                                .record_coalescing_additions(1, limits.max_coalescing_additions)?;
                            let sum = entry.get().try_add(
                                &contribution,
                                context,
                                limits.exact_algebra,
                            )?;
                            if sum.is_zero() {
                                entry.remove();
                            } else {
                                *entry.get_mut() = sum;
                            }
                        }
                    }
                }
            }
            Ok(CachedTerms::Factorized(output))
        }
    }
}

fn child_entry<'a>(
    cache: &'a BTreeMap<IntegralKey, CacheEntry>,
    child: &IntegralKey,
) -> Result<&'a CacheEntry, CandidateReductionError> {
    cache.get(child).ok_or_else(|| {
        ReductionError::ReducerInvariant {
            detail: "candidate child is absent at combine frame",
        }
        .into()
    })
}

fn mixed_representation() -> CandidateReductionError {
    ReductionError::ReducerInvariant {
        detail: "candidate cache contains mixed coefficient representations",
    }
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn factorized_empty_child_skips_unneeded_conversion_but_not_cache_checks() {
        let context = CoefficientContext::new(["x"]);
        let key = IntegralKey::try_new([1]).unwrap();
        let factor = context.coefficient_fixture("1/(x^8-1)");
        let limits = ReductionLimits {
            exact_algebra: crate::algebra::ExactAlgebraLimits {
                max_polynomial_terms: 2,
                ..Default::default()
            },
            max_coalescing_additions: 0,
            ..Default::default()
        };
        context
            .validate_with_limits(&factor, limits.exact_algebra)
            .unwrap();
        // The ordinary coefficient is admitted, but factoring its denominator
        // needs a larger conservative expanded-support envelope. An empty
        // descendant makes that unused conversion unnecessary in both modes.
        assert!(
            FactorizedCoefficient::from_coefficient(&context, factor.clone(), limits.exact_algebra)
                .is_err()
        );
        for representation in [
            CandidateCacheRepresentation::Sparse,
            CandidateCacheRepresentation::Factorized,
        ] {
            let mut cache = BTreeMap::from([(
                key.clone(),
                CacheEntry {
                    terms: CachedTerms::from_sparse(
                        BTreeMap::new(),
                        representation,
                        &context,
                        limits,
                    )
                    .unwrap(),
                    weight: CacheWeight::default(),
                },
            )]);
            let mut request = ReductionRequest::default();
            let mut statistics = ReductionStatistics::default();
            let output = combine(
                &context,
                &cache,
                representation,
                BTreeMap::from([(key.clone(), factor.clone())]),
                limits,
                &mut request,
                &mut statistics,
            )
            .unwrap();
            assert_eq!(output.terminal_keys().count(), 0);
            assert_eq!(statistics, ReductionStatistics::default());
            cache.clear();
            assert!(matches!(
                combine(
                    &context,
                    &cache,
                    representation,
                    BTreeMap::from([(key.clone(), factor.clone())]),
                    limits,
                    &mut request,
                    &mut statistics
                ),
                Err(CandidateReductionError::Application(
                    ReductionError::ReducerInvariant { .. }
                ))
            ));
        }
        let cache = BTreeMap::from([(
            key.clone(),
            CacheEntry {
                terms: CachedTerms::Sparse(BTreeMap::new()),
                weight: CacheWeight::default(),
            },
        )]);
        assert!(matches!(
            combine(
                &context,
                &cache,
                CandidateCacheRepresentation::Factorized,
                BTreeMap::from([(key, factor)]),
                limits,
                &mut ReductionRequest::default(),
                &mut ReductionStatistics::default()
            ),
            Err(CandidateReductionError::Application(
                ReductionError::ReducerInvariant { .. }
            ))
        ));
    }
}
