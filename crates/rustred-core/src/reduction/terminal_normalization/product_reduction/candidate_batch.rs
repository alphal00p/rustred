//! One guarded native candidate cache, scoped to a transported factor batch.
use std::sync::Arc;

use super::combination::{reduce_with_cache_census, transported_terms};
use super::model::{
    VacuumFactorReduction, VacuumFactorTerm, VacuumFactorWork, VacuumProductReductionError as Error,
};
use crate::algebra::Coefficient;
use crate::family::{IntegralFamily, IntegralKey};
use crate::reduction::{CacheCensus, ReductionLimits};
use crate::sector::symmetry::integral_transport;
use crate::solver::CandidateReducer;

/// Apply one verified factor transport using one guarded native candidate DAG
/// cache across all its distinct nonzero endpoints. Coefficients, guards and
/// shared-subproblem reuse stay in RustRed; no formulas or terminals are added.
///
/// This explicitly scoped API clears any prior candidate coefficient cache at
/// entry and clears the batch cache on both success and error. Immutable rules,
/// installed normalization, storage representation and policy are unchanged.
/// Native cache payload and retained batch coefficients/guards share the given
/// cache grant, while actual per-root work consumes one common request budget.
pub fn reduce_transported_candidate_factor<const N: usize>(
    reducer: &mut CandidateReducer<N>,
    transport: &integral_transport::Prepared,
    source: &IntegralKey,
    target_family: Arc<IntegralFamily>,
    expansion_limits: integral_transport::ExpansionLimits,
    limits: ReductionLimits,
) -> Result<VacuumFactorReduction, Error> {
    scoped(reducer, |reducer| {
        let terms = transported_terms(transport, source, &target_family, expansion_limits, limits)?;
        reduce_candidate_terms(
            reducer,
            target_family,
            terms,
            transport
                .verified_map()
                .nonzero_conditions()
                .iter()
                .map(|condition| condition.polynomial().clone().into()),
            limits,
        )
    })
}

fn scoped<const N: usize>(
    reducer: &mut CandidateReducer<N>,
    run: impl FnOnce(&mut CandidateReducer<N>) -> Result<VacuumFactorReduction, Error>,
) -> Result<VacuumFactorReduction, Error> {
    reducer.clear_cache().map_err(native_error)?;
    let result = run(reducer);
    // Do not leave a successful prefix hidden after a later endpoint failed.
    reducer.clear_cache().map_err(native_error)?;
    result
}

/// Private: inputs are the native transport's already coalesced, nonzero roots.
fn reduce_candidate_terms<const N: usize>(
    reducer: &mut CandidateReducer<N>,
    family: Arc<IntegralFamily>,
    terms: Vec<(IntegralKey, Coefficient)>,
    conditions: impl IntoIterator<Item = Coefficient>,
    limits: ReductionLimits,
) -> Result<VacuumFactorReduction, Error> {
    if reducer.family_fingerprint() != family.fingerprint() {
        return Err(Error::InvalidInput(
            "candidate factor batch has a foreign family".into(),
        ));
    }
    let family_for_rows = family.clone();
    reduce_with_cache_census(family, terms, conditions, limits, |key, remaining| {
        // Existing guarded application validates entries and the complete warm
        // cache against its inclusive remaining grant BEFORE any cache hit.
        let (decomposition, conditions, work) = reducer
            .reduce_unit_mass_guarded(key, remaining)
            .map_err(|error| {
                if let Some(resource) = crate::reduction::resource_limit_error(&error) {
                    return Error::Reduction(crate::reduction::ReductionError::Resources(
                        resource.clone(),
                    ));
                }
                match error {
                    crate::solver::CandidateReductionError::Uncovered { target }
                    | crate::solver::CandidateReductionError::OutsideNumeratorRank {
                        target, ..
                    } => Error::UnresolvedFactor {
                        family: family_for_rows.fingerprint().to_owned(),
                        target,
                    },
                    error => Error::Provider {
                        family: family_for_rows.fingerprint().to_owned(),
                        target: key.clone(),
                        message: error.to_string(),
                    },
                }
            })?;
        if let Some(target) = decomposition.unresolved().first() {
            return Err(Error::UnresolvedFactor {
                family: family_for_rows.fingerprint().to_owned(),
                target: target.clone(),
            });
        }
        let statistics = reducer.statistics();
        let result = VacuumFactorReduction {
            terms: decomposition
                .into_terms()
                .into_iter()
                .map(|(integral, coefficient)| VacuumFactorTerm {
                    family: family_for_rows.clone(),
                    integral,
                    coefficient,
                })
                .collect(),
            nonzero_conditions: conditions,
            work: VacuumFactorWork {
                rule_applications: work.rule_applications(),
                coalescing_additions: work.coalescing_additions(),
                cache_hits: work.cache_hits(),
            },
        };
        Ok((
            result,
            CacheCensus {
                integrals: statistics.cached_integrals(),
                coefficient_terms: statistics.cached_coefficient_terms(),
                coefficient_bytes: statistics.cached_coefficient_bytes(),
            },
        ))
    })
}

fn native_error(error: impl std::fmt::Display) -> Error {
    Error::InvalidInput(format!("candidate batch cache: {error}"))
}

#[cfg(test)]
mod tests;
