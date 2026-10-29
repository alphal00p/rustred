//! Shared checked operations; execution engines own scheduling and mutable result state.
use super::*;

impl<const N: usize> RoutedCandidateApplication<N> {
    /// Common exact Route/Apply kernel. Scheduling, completed-cache ownership,
    /// and back-substitution stay in their respective execution engines.
    pub(super) fn checked_expand_node(
        &mut self,
        node: &Node<N>,
        conditions: &mut Vec<Coefficient>,
        request: &mut Request<'_, '_, N>,
    ) -> Result<CheckedExpansion<N>, CandidateRoutedError> {
        let context = self.context.clone();
        let shared = &context.shared;
        let limits = context.limits;
        let key = node.key();
        let parent_support = support::<N>(key);
        match node {
            Node::Route(_) => {
                if self.owners.contains(&parent_support) {
                    return Ok(CheckedExpansion::Expanded(BTreeMap::from([(
                        Node::Apply(parent_support, key.clone()),
                        shared.context.base().one(),
                    )])));
                }
                if let Some(route) = self.routes.get(&parent_support) {
                    charge(
                        &mut request.stats.transport_calls,
                        1,
                        self.limits
                            .diagnostic_limit(self.limits.max_transport_calls),
                        "transport calls",
                    )?;
                    request.observe_activity(self, CandidateRoutedActivity::Transport, None);
                    let mapped = route.transport.transport_with_usage(
                        key,
                        self.limits.expansion,
                        |usage| {
                            let operations = request
                                .stats
                                .transport_operations
                                .saturating_add(usage.operations);
                            let endpoints = request
                                .stats
                                .transport_endpoints
                                .saturating_add(usage.endpoints);
                            for (resource, requested, limit) in [
                                (
                                    "aggregate routed operations",
                                    operations,
                                    self.limits.max_transport_operations,
                                ),
                                (
                                    "aggregate routed endpoints",
                                    endpoints,
                                    self.limits.max_transport_endpoints,
                                ),
                            ] {
                                if self.limits.diagnostics_enabled && requested > limit {
                                    return Err(ExpansionError::ResourceLimit {
                                        resource,
                                        requested,
                                        limit,
                                    });
                                }
                            }
                            request.stats.transport_operations = operations;
                            request.stats.transport_endpoints = endpoints;
                            Ok(())
                        },
                    )?;
                    let owner = std::array::from_fn(|i| {
                        route
                            .owner_sector
                            .active_bits()
                            .get(i)
                            .copied()
                            .unwrap_or(false)
                    });
                    let base = CandidateEvaluator {
                        context: &shared.context,
                        root_sector: [true; N],
                        ordering: &Default::default(),
                        rules: &[],
                        whole_piece_alternatives: &[],
                        source_conditions: &shared.source_conditions,
                        zero_sectors: &shared.zero_sectors,
                        limits,
                    };
                    let mut terms = BTreeMap::new();
                    for endpoint in mapped.terms() {
                        // Transported endpoints are checked before any global
                        // cancellation, so retain the conditions incurred by
                        // that check even if the endpoint never expands.
                        base.validate_target_with_conditions(endpoint.key(), &mut |condition| {
                            retain_polynomial(conditions, condition)
                        })?;
                        terms.insert(
                            successor(key, endpoint.key().clone(), owner)?,
                            endpoint.coefficient().clone(),
                        );
                    }
                    return Ok(CheckedExpansion::Expanded(terms));
                }
                if self.unresolved_policy == UnresolvedPolicy::Preserve {
                    return Ok(CheckedExpansion::Terminal(
                        CandidateRoutedCompletionSource::UnresolvedPreserved,
                    ));
                }
                Err(CandidateRoutedError::MissingOwner {
                    target: key.clone(),
                })
            }
            Node::Apply(owner_sector, _) => {
                if parent_support != *owner_sector {
                    return Err(CandidateRoutedError::InvalidInput(
                        "apply node does not belong to its owner support".into(),
                    ));
                }
                request.observe_activity(self, CandidateRoutedActivity::OwnerPreparation, None);
                let owner = self.owners.owner(owner_sector)?.ok_or_else(|| {
                    CandidateRoutedError::InvalidInput("admitted owner disappeared".into())
                })?;
                let result = if owner.batches[0].terminals.contains(key) {
                    OwnerStep::Terminal
                } else {
                    request.algebra.record_rule_application(
                        limits.diagnostic_limit(limits.max_rule_applications),
                    )?;
                    request.observe_activity(self, CandidateRoutedActivity::RuleEvaluation, None);
                    owner.evaluate_step::<CandidateReductionError>(key, |batch| {
                        let evaluator = CandidateEvaluator {
                            context: &shared.context,
                            root_sector: owner.root,
                            ordering: &owner.ordering,
                            rules: &batch.rules,
                            whole_piece_alternatives: &batch.whole_piece_alternatives,
                            source_conditions: &shared.source_conditions,
                            zero_sectors: &shared.zero_sectors,
                            limits,
                        };
                        Ok(evaluator
                            .apply_with_conditions(
                                key,
                                &mut request.algebra,
                                &mut request.stats.work,
                            )
                            .map(|(terms, local)| {
                                retain_conditions(conditions, local);
                                terms
                            }))
                    })?
                };
                match result {
                    OwnerStep::Terminal => Ok(CheckedExpansion::Terminal(
                        CandidateRoutedCompletionSource::Terminal,
                    )),
                    OwnerStep::Uncovered
                        if self.unresolved_policy == UnresolvedPolicy::Preserve =>
                    {
                        Ok(CheckedExpansion::Terminal(
                            CandidateRoutedCompletionSource::UnresolvedPreserved,
                        ))
                    }
                    OwnerStep::Uncovered => Err(CandidateReductionError::Uncovered {
                        target: key.clone(),
                    }
                    .into()),
                    OwnerStep::Applied(terms) => {
                        request.stats.work.record_rule_application();
                        let mut children = BTreeMap::new();
                        for (child, coefficient) in terms {
                            children.insert(successor(key, child, *owner_sector)?, coefficient);
                            request.record_local_descent();
                        }
                        Ok(CheckedExpansion::Expanded(children))
                    }
                }
            }
        }
    }
}
