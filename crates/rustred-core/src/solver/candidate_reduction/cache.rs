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
