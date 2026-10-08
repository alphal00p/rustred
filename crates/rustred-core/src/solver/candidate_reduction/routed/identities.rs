//! Finite, coefficient-carrying bridge from routed owners to exact equations.
use super::{CandidateRoutedError, CandidateRoutedIdentity, RoutedCandidateReducer};
use crate::family::IntegralKey;
use crate::reduction::{
    ReductionError, ReductionRequest, ReductionStatistics, accumulate_master_in_request,
};
use crate::sector::symmetry::integral_transport::ExpansionError;
use crate::solver::candidate_reduction::CandidateReductionError;
use std::collections::BTreeMap;

impl<const N: usize> RoutedCandidateReducer<N> {
    /// Return every directly applicable saved-owner equation, or one selected
    /// weighted routing equation plus directly applicable saved equations at
    /// its endpoints. Unresolved endpoints remain columns in the routing row.
    ///
    /// This performs at most one finite transport and no recursive reduction.
    /// Literal owner support takes precedence. It ignores terminal/deferred
    /// stopping and entry numerator-rank bounds, retaining the exact rule and
    /// transport applicability checks and configured arithmetic/expansion caps.
    /// Returned rows carry candidate authority, never a new closure claim.
    pub fn terminal_identity_equations(
        &self,
        target: &IntegralKey,
    ) -> Result<Vec<CandidateRoutedIdentity>, CandidateRoutedError> {
        let arity = self.programs.context.coefficient_context().index_count();
        if target.powers().len() != arity {
            return Err(ReductionError::WrongArity {
                expected: arity,
                actual: target.powers().len(),
            }
            .into());
        }
        let sector =
            std::array::from_fn(|axis| target.powers().get(axis).copied().unwrap_or(0) > 0);
        let mut request = ReductionRequest::default();
        let mut statistics = ReductionStatistics::default();
        let saved_rows =
            |key, request: &mut ReductionRequest, statistics: &mut ReductionStatistics| {
                self.programs
                    .applicable_identities_in_request(key, request, statistics)
                    .map(|rows| {
                        rows.into_iter().map(|row| CandidateRoutedIdentity {
                            terms: row.terms,
                            nonzero_conditions: row.nonzero_conditions,
                        })
                    })
            };
        if self.programs.owners.contains_key(&sector) {
            return Ok(saved_rows(target, &mut request, &mut statistics)?.collect());
        }
        let Some(route) = self.routes.get(&sector) else {
            return Ok(Vec::new());
        };
        if self.limits.max_transport_calls == 0 {
            return Err(CandidateRoutedError::ResourceLimit {
                resource: "transport calls",
                requested: 1,
                limit: 0,
            });
        }
        let mapped =
            route
                .transport
                .transport_with_usage(target, self.limits.expansion, |usage| {
                    for (resource, requested, limit) in [
                        (
                            "aggregate routed operations",
                            usage.operations,
                            self.limits.max_transport_operations,
                        ),
                        (
                            "aggregate routed endpoints",
                            usage.endpoints,
                            self.limits.max_transport_endpoints,
                        ),
                    ] {
                        if requested > limit {
                            return Err(ExpansionError::ResourceLimit {
                                resource,
                                requested,
                                limit,
                            });
                        }
                    }
                    Ok(())
                })?;
        let context = self.programs.context.family.coefficient_context();
        let limits = self.programs.context.limits;
        let mut terms = BTreeMap::from([(target.clone(), context.one())]);
        for endpoint in mapped.terms() {
            let coefficient = context
                .try_neg(endpoint.coefficient(), limits.exact_algebra)
                .map_err(CandidateReductionError::from)?;
            accumulate_master_in_request(
                context,
                &mut terms,
                endpoint.key(),
                coefficient,
                limits,
                &mut request,
                &mut statistics,
            )?;
        }
        // The admitted transport lane verifies unconditional unit Jacobians
        // and rational-constant affine entries. No parameter poles arise here.
        let mut rows = vec![CandidateRoutedIdentity {
            terms,
            nonzero_conditions: Vec::new(),
        }];
        for endpoint in mapped.terms() {
            rows.extend(saved_rows(endpoint.key(), &mut request, &mut statistics)?);
        }
        Ok(rows)
    }
}

#[cfg(test)]
mod tests;
