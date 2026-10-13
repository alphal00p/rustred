//! Explicit entry transport, separate from entry admission and IBP application.
use super::{
    CandidateRoutedApplicationStatistics, CandidateRoutedReduction, RoutedCandidateApplication,
};
use crate::algebra::Coefficient;
use crate::family::IntegralKey;
use crate::sector::symmetry::integral_transport::ExpansionError;
use crate::solver::candidate_reduction::evaluator::CandidateEvaluator;
use crate::solver::candidate_reduction::routed::CandidateRoutedError;
use crate::solver::candidate_reduction::routed::trace::{charge, successor, support};
use std::collections::BTreeMap;

impl<const N: usize> RoutedCandidateApplication<N> {
    /// Return only the installed verified entry transport. Literal owners take
    /// precedence and return the identity. No rule, cache, terminal substitution
    /// or entry admission is applied. The caller must separately admit every
    /// nonzero endpoint against its original entry union before reducing it.
    /// Endpoints covered by installed zero certificates are omitted after
    /// their source guards and resource charges have been retained.
    pub fn route_entry_to_owner(
        &self,
        target: &IntegralKey,
    ) -> Result<CandidateRoutedReduction, CandidateRoutedError> {
        self.route_entry_in_request(target, &mut CandidateRoutedApplicationStatistics::default())
    }

    pub(super) fn route_entry_in_request(
        &self,
        target: &IntegralKey,
        statistics: &mut CandidateRoutedApplicationStatistics,
    ) -> Result<CandidateRoutedReduction, CandidateRoutedError> {
        let context = &self.reducer.programs.context;
        let shared = &context.shared;
        let evaluator = CandidateEvaluator {
            context: &shared.context,
            root_sector: [true; N],
            ordering: &Default::default(),
            rules: &[],
            whole_piece_alternatives: &[],
            source_conditions: &shared.source_conditions,
            zero_sectors: &shared.zero_sectors,
            limits: context.limits,
        };
        let mut conditions = Vec::new();
        let mut retain = |condition: &crate::algebra::CoefficientPolynomial| {
            if !condition.is_constant() {
                let condition: Coefficient = condition.clone().into();
                if !conditions.contains(&condition) {
                    conditions.push(condition);
                }
            }
        };
        evaluator.validate_target_with_conditions(target, &mut retain)?;
        charge(
            &mut statistics.input_terms,
            1,
            self.reducer.limits.max_input_targets,
            "input targets",
        )?;
        charge(
            &mut statistics.operational_nodes,
            1,
            self.reducer.limits.max_unique_nodes,
            "operational nodes",
        )?;
        statistics.requested_targets = 1;
        let support = support::<N>(target);
        let terms = if self.reducer.programs.owners.contains_key(&support) {
            BTreeMap::from([(target.clone(), shared.context.base().one())])
        } else {
            let route = self.reducer.routes.get(&support).ok_or_else(|| {
                CandidateRoutedError::MissingOwner {
                    target: target.clone(),
                }
            })?;
            charge(
                &mut statistics.transport_calls,
                1,
                self.reducer.limits.max_transport_calls,
                "transport calls",
            )?;
            let mapped = route.transport.transport_with_usage(
                target,
                self.reducer.limits.expansion,
                |usage| {
                    let operations = statistics
                        .transport_operations
                        .checked_add(usage.operations)
                        .ok_or(ExpansionError::ResourceCountOverflow {
                            resource: "aggregate routed operations",
                        })?;
                    let endpoints = statistics
                        .transport_endpoints
                        .checked_add(usage.endpoints)
                        .ok_or(ExpansionError::ResourceCountOverflow {
                            resource: "aggregate routed endpoints",
                        })?;
                    for (resource, requested, limit) in [
                        (
                            "aggregate routed operations",
                            operations,
                            self.reducer.limits.max_transport_operations,
                        ),
                        (
                            "aggregate routed endpoints",
                            endpoints,
                            self.reducer.limits.max_transport_endpoints,
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
                    statistics.transport_operations = operations;
                    statistics.transport_endpoints = endpoints;
                    Ok(())
                },
            )?;
            let owner: [bool; N] = std::array::from_fn(|i| {
                route
                    .owner_sector
                    .active_bits()
                    .get(i)
                    .copied()
                    .unwrap_or(false)
            });
            let mut terms = BTreeMap::new();
            for endpoint in mapped.terms() {
                evaluator.validate_target_with_conditions(endpoint.key(), &mut retain)?;
                successor(target, endpoint.key().clone(), owner)?;
                charge(
                    &mut statistics.operational_nodes,
                    1,
                    self.reducer.limits.max_unique_nodes,
                    "operational nodes",
                )?;
                // Only installed exact zero certificates justify omission.
                // Keep source guards, transition validation and all expansion
                // work charged even when the endpoint subsequently vanishes.
                if evaluator.is_zero(endpoint.key()) {
                    continue;
                }
                terms.insert(endpoint.key().clone(), endpoint.coefficient().clone());
            }
            terms
        };
        Ok(CandidateRoutedReduction {
            family_fingerprint: context.family.fingerprint_owner(),
            terms,
            nonzero_conditions: conditions,
            statistics: *statistics,
        })
    }
}
