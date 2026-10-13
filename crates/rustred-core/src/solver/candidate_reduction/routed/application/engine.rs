use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use super::cache_policy::DependencyPins;
use super::guard_union::GuardUnion;
use super::{
    CandidateRoutedApplicationStatistics, CandidateRoutedReduction, RoutedCandidateApplication,
};
use crate::algebra::{Coefficient, CoefficientPolynomial};
use crate::family::IntegralKey;
use crate::reduction::{ReductionError, ReductionRequest, accumulate_master_in_request};
use crate::sector::symmetry::integral_transport::ExpansionError;
use crate::solver::candidate_reduction::CandidateReductionError;
use crate::solver::candidate_reduction::cache::{self, CachedTerms};
use crate::solver::candidate_reduction::evaluator::CandidateEvaluator;
use crate::solver::candidate_reduction::owners::OwnerStep;
use crate::solver::candidate_reduction::routed::trace::{Node, charge, successor, support};
use crate::solver::candidate_reduction::routed::{CandidateEntryAdmission, CandidateRoutedError};

enum Frame<const N: usize> {
    Enter(Node<N>),
    Combine {
        node: Node<N>,
        terms: Arc<BTreeMap<Node<N>, Coefficient>>,
        conditions: Vec<Coefficient>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solver::RoutedCandidateReducer;
    use crate::solver::candidate_reduction::owner_test_support::*;

    #[test]
    fn active_operational_cycle_is_rejected_even_with_a_cached_result() {
        let family = Arc::new(crate::solver::tests::tadpole());
        let programs = programs(
            family.clone(),
            None,
            vec![input([true], None, vec![], &[[1]])],
            Default::default(),
        );
        let mut application = RoutedCandidateReducer::try_new(programs, [], Default::default())
            .unwrap()
            .application();
        application
            .reduce_linear_combination([(key([1]), family.coefficient_context().one())])
            .unwrap();
        let node = Node::Route(key([1]));
        let mut request = Request::default();
        request.active.insert(node.clone());
        request.push(Frame::Enter(node), usize::MAX).unwrap();
        assert!(matches!(
            application.run_stack(&mut request),
            Err(CandidateRoutedError::Cycle { .. })
        ));
    }

    #[test]
    fn incomplete_child_cache_is_an_error_at_combine() {
        let family = Arc::new(crate::solver::tests::tadpole());
        let programs = programs(
            family.clone(),
            None,
            vec![input([true], None, vec![], &[[1]])],
            Default::default(),
        );
        let mut application = RoutedCandidateReducer::try_new(programs, [], Default::default())
            .unwrap()
            .application();
        let node = Node::Route(key([1]));
        let mut request = Request::default();
        request.active.insert(node.clone());
        request
            .push(
                Frame::Combine {
                    node,
                    terms: Arc::new(BTreeMap::from([(
                        Node::Apply([true], key([1])),
                        family.coefficient_context().one(),
                    )])),
                    conditions: vec![],
                },
                usize::MAX,
            )
            .unwrap();
        assert!(matches!(
            application.run_stack(&mut request),
            Err(CandidateRoutedError::Candidate(
                CandidateReductionError::Application(ReductionError::ReducerInvariant { .. })
            ))
        ));
        assert_eq!(application.cache_statistics().cached_integrals(), 0);
    }
}

#[derive(Default)]
struct Request<const N: usize> {
    algebra: ReductionRequest,
    stats: CandidateRoutedApplicationStatistics,
    seen: BTreeSet<Node<N>>,
    active: BTreeSet<Node<N>>,
    stack: Vec<Frame<N>>,
    pins: DependencyPins<N>,
}

impl<const N: usize> Request<N> {
    fn push(&mut self, frame: Frame<N>, limit: usize) -> Result<(), CandidateRoutedError> {
        self.algebra.retain_pending_frame(limit)?;
        self.stack
            .try_reserve(1)
            .map_err(|_| ReductionError::AllocationFailure {
                resource: "routed application stack",
                requested: self.stack.len().saturating_add(1),
            })?;
        self.stack.push(frame);
        Ok(())
    }
}

fn retain_conditions(
    output: &mut Vec<Coefficient>,
    conditions: impl IntoIterator<Item = Coefficient>,
) {
    for condition in conditions {
        if !output.contains(&condition) {
            output.push(condition);
        }
    }
}

fn retain_polynomial(output: &mut Vec<Coefficient>, condition: &CoefficientPolynomial) {
    if !condition.is_constant() {
        retain_conditions(output, [condition.clone().into()]);
    }
}

impl<const N: usize> RoutedCandidateApplication<N> {
    pub fn reduce_linear_combination(
        &mut self,
        terms: impl IntoIterator<Item = (IntegralKey, Coefficient)>,
    ) -> Result<CandidateRoutedReduction, CandidateRoutedError> {
        self.reduce_linear_combination_with_entry_admission(
            terms,
            CandidateEntryAdmission::SavedGenerationScope,
        )
    }

    /// One shared bounded request, including all roots, transports, rule
    /// applications and coefficient sums. Every input crosses admission even
    /// when its coefficient is zero or another input cancels it exactly.
    pub fn reduce_linear_combination_with_entry_admission(
        &mut self,
        terms: impl IntoIterator<Item = (IntegralKey, Coefficient)>,
        admission: CandidateEntryAdmission<'_, N>,
    ) -> Result<CandidateRoutedReduction, CandidateRoutedError> {
        let mut request = Request::default();
        let result = self.reduce_request(terms, admission, &mut request, true);
        self.release_request_pins(&mut request.pins);
        // Failed attempts retain valid completed cache entries and their
        // measured work, never active or partially combined nodes.
        self.work.merge_work(request.stats.work);
        self.cache_work.merge(request.stats.cache);
        result
    }

    /// Explicitly transport initial keys using installed routes, then admit
    /// every endpoint to the selected entry union before cancellation. All
    /// transport, reduction and addition work shares one bounded request.
    pub fn reduce_routed_linear_combination_with_entry_admission(
        &mut self,
        terms: impl IntoIterator<Item = (IntegralKey, Coefficient)>,
        admission: CandidateEntryAdmission<'_, N>,
    ) -> Result<CandidateRoutedReduction, CandidateRoutedError> {
        let mut request = Request::default();
        let result = self.reduce_routed_request(terms, admission, &mut request);
        self.release_request_pins(&mut request.pins);
        self.work.merge_work(request.stats.work);
        self.cache_work.merge(request.stats.cache);
        result
    }

    fn reduce_routed_request(
        &mut self,
        terms: impl IntoIterator<Item = (IntegralKey, Coefficient)>,
        admission: CandidateEntryAdmission<'_, N>,
        request: &mut Request<N>,
    ) -> Result<CandidateRoutedReduction, CandidateRoutedError> {
        let context = self.context.clone();
        let coefficients = context.shared.context.base();
        let limits = context.limits;
        let mut entries = BTreeMap::new();
        let mut conditions = Vec::new();
        for (target, factor) in terms {
            coefficients
                .validate_with_limits(&factor, limits.exact_algebra)
                .map_err(CandidateReductionError::from)?;
            retain_polynomial(&mut conditions, &factor.denominator);
            let routed = self.route_entry_in_request(&target, &mut request.stats)?;
            retain_conditions(&mut conditions, routed.nonzero_conditions);
            for (endpoint, weight) in routed.terms {
                // Even zero caller factors cannot bypass endpoint admission.
                admission.validate_entry(&endpoint, context.scope.max_numerator_rank)?;
                let contribution = coefficients
                    .try_mul(&factor, &weight, limits.exact_algebra)
                    .map_err(CandidateReductionError::from)?;
                accumulate_master_in_request(
                    coefficients,
                    &mut entries,
                    &endpoint,
                    contribution,
                    limits,
                    &mut request.algebra,
                    &mut request.stats.work,
                )?;
            }
        }
        let mut result = self.reduce_request(entries, admission, request, false)?;
        retain_conditions(&mut conditions, result.nonzero_conditions);
        result.nonzero_conditions = conditions;
        Ok(result)
    }

    fn reduce_request(
        &mut self,
        terms: impl IntoIterator<Item = (IntegralKey, Coefficient)>,
        admission: CandidateEntryAdmission<'_, N>,
        request: &mut Request<N>,
        count_inputs: bool,
    ) -> Result<CandidateRoutedReduction, CandidateRoutedError> {
        let context = self.context.clone();
        let shared = &context.shared;
        let limits = context.limits;
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
        let coefficients = shared.context.base();
        let mut entries = BTreeMap::new();
        let mut conditions = Vec::new();
        for (target, coefficient) in terms {
            if count_inputs {
                charge(
                    &mut request.stats.input_terms,
                    1,
                    self.limits.max_input_targets,
                    "input terms",
                )?;
            }
            base.validate_target_with_conditions(&target, &mut |condition| {
                retain_polynomial(&mut conditions, condition)
            })?;
            admission.validate_entry(&target, context.scope.max_numerator_rank)?;
            coefficients
                .validate_with_limits(&coefficient, limits.exact_algebra)
                .map_err(CandidateReductionError::from)?;
            retain_polynomial(&mut conditions, &coefficient.denominator);
            if !entries.contains_key(&target) && !coefficient.is_zero() {
                let mut size = request.stats.operational_nodes;
                charge(
                    &mut size,
                    entries.len(),
                    self.limits.max_unique_nodes,
                    "initial operational nodes",
                )?;
                charge(
                    &mut size,
                    1,
                    self.limits.max_unique_nodes,
                    "initial operational nodes",
                )?;
            }
            accumulate_master_in_request(
                coefficients,
                &mut entries,
                &target,
                coefficient,
                limits,
                &mut request.algebra,
                &mut request.stats.work,
            )?;
        }
        request.stats.requested_targets = entries.len();
        let mut output = BTreeMap::new();
        for (target, factor) in entries {
            let root = Node::Route(target);
            self.pin_dependency(&root, &mut request.pins)?;
            request.push(Frame::Enter(root.clone()), limits.max_pending_frames)?;
            self.run_stack(request)?;
            let entry = self
                .cache
                .get(&root)
                .ok_or(ReductionError::ReducerInvariant {
                    detail: "routed root is absent after application",
                })?;
            retain_conditions(&mut conditions, entry.conditions.iter().cloned());
            let terms = match &entry.terms {
                CachedTerms::Sparse(terms) => Cow::Borrowed(terms),
                _ => Cow::Owned(entry.terms.materialize(coefficients, limits)?),
            };
            for (terminal, coefficient) in terms.iter() {
                let contribution = coefficients
                    .try_mul(&factor, coefficient, limits.exact_algebra)
                    .map_err(CandidateReductionError::from)?;
                accumulate_master_in_request(
                    coefficients,
                    &mut output,
                    terminal,
                    contribution,
                    limits,
                    &mut request.algebra,
                    &mut request.stats.work,
                )?;
            }
            // The output owns its coefficients and guards now. The current
            // root may be reused or evicted while processing later roots.
            drop(terms);
            self.touch_cache(&root)?;
            self.release_dependency(&root, &mut request.pins)?;
        }
        charge(
            &mut request.stats.operational_nodes,
            request.seen.len(),
            self.limits.max_unique_nodes,
            "operational nodes",
        )?;
        Ok(CandidateRoutedReduction {
            family_fingerprint: context.family.fingerprint_owner(),
            terms: output,
            nonzero_conditions: conditions,
            statistics: request.stats,
        })
    }

    fn run_stack(&mut self, request: &mut Request<N>) -> Result<(), CandidateRoutedError> {
        let context = self.context.clone();
        let shared = &context.shared;
        let limits = context.limits;
        let coefficients = shared.context.base();
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
        while let Some(frame) = request.stack.pop() {
            request.algebra.release_pending_frame()?;
            let node = match frame {
                Frame::Combine {
                    node,
                    terms,
                    mut conditions,
                } => {
                    // One shallow membership index for the complete frame,
                    // including every child. Cache guards are borrowed until
                    // a genuinely new condition is retained in first-seen order.
                    let mut guards = GuardUnion::try_new(&mut conditions)?;
                    let output = cache::combine_values(
                        coefficients,
                        self.cache_representation,
                        terms.iter().map(|(child, factor)| {
                            let entry =
                                self.cache
                                    .get(child)
                                    .ok_or(ReductionError::ReducerInvariant {
                                        detail: "routed child is absent at combine frame",
                                    })?;
                            guards.extend(&entry.conditions)?;
                            Ok((&entry.terms, factor.clone()))
                        }),
                        limits,
                        &mut request.algebra,
                        &mut request.stats.work,
                    )?;
                    drop(guards);
                    // Every queued parent retains its own reference. Release
                    // this frame's children only after all arithmetic and
                    // guards have been copied, before admitting the parent.
                    for child in terms.keys() {
                        self.touch_cache(child)?;
                        self.release_dependency(child, &mut request.pins)?;
                    }
                    if !request.active.remove(&node) {
                        return Err(ReductionError::ReducerInvariant {
                            detail: "routed combine node is not active",
                        }
                        .into());
                    }
                    self.cache_insert(
                        node,
                        output,
                        conditions,
                        &request.pins,
                        &mut request.stats.cache,
                    )?;
                    continue;
                }
                Frame::Enter(node) => node,
            };
            if request.active.contains(&node) {
                return Err(CandidateRoutedError::Cycle {
                    target: node.key().clone(),
                });
            }
            let seen_before = request.seen.contains(&node);
            if !seen_before {
                let mut size = request.stats.operational_nodes;
                charge(
                    &mut size,
                    request.seen.len(),
                    self.limits.max_unique_nodes,
                    "operational nodes",
                )?;
                charge(
                    &mut size,
                    1,
                    self.limits.max_unique_nodes,
                    "operational nodes",
                )?;
                request.seen.insert(node.clone());
            }
            let key = node.key();
            let mut conditions = Vec::new();
            base.validate_target_with_conditions(key, &mut |condition| {
                retain_polynomial(&mut conditions, condition)
            })?;
            if self.cache.contains_key(&node) {
                self.touch_cache(&node)?;
                request.stats.work.record_cache_hit();
                continue;
            }
            if seen_before {
                request.stats.cache.recomputed_nodes =
                    request.stats.cache.recomputed_nodes.saturating_add(1);
            }
            if base.is_zero(key) {
                self.cache_insert_sparse(node, BTreeMap::new(), conditions, request)?;
                continue;
            }
            request.active.insert(node.clone());
            let parent_support = support::<N>(key);
            let terms = match &node {
                Node::Route(_) => {
                    if self.owners.contains(&parent_support) {
                        BTreeMap::from([(
                            Node::Apply(parent_support, key.clone()),
                            coefficients.one(),
                        )])
                    } else if let Some(route) = self.routes.get(&parent_support) {
                        charge(
                            &mut request.stats.transport_calls,
                            1,
                            self.limits.max_transport_calls,
                            "transport calls",
                        )?;
                        let mapped = route.transport.transport_with_usage(
                            key,
                            self.limits.expansion,
                            |usage| {
                                let operations = request
                                    .stats
                                    .transport_operations
                                    .checked_add(usage.operations)
                                    .ok_or(ExpansionError::ResourceCountOverflow {
                                        resource: "aggregate routed operations",
                                    })?;
                                let endpoints = request
                                    .stats
                                    .transport_endpoints
                                    .checked_add(usage.endpoints)
                                    .ok_or(ExpansionError::ResourceCountOverflow {
                                        resource: "aggregate routed endpoints",
                                    })?;
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
                                    if requested > limit {
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
                        let owner_support = std::array::from_fn(|i| {
                            route
                                .owner_sector
                                .active_bits()
                                .get(i)
                                .copied()
                                .unwrap_or(false)
                        });
                        let mut terms = BTreeMap::new();
                        for endpoint in mapped.terms() {
                            let child = endpoint.key();
                            base.validate_target(child)?;
                            let child_node = successor(key, child.clone(), owner_support)?;
                            terms.insert(child_node, endpoint.coefficient().clone());
                        }
                        terms
                    } else {
                        return Err(CandidateRoutedError::MissingOwner {
                            target: key.clone(),
                        });
                    }
                }
                Node::Apply(owner_sector, _) => {
                    if &parent_support != owner_sector {
                        return Err(CandidateRoutedError::InvalidInput(
                            "apply node does not belong to its owner support".into(),
                        ));
                    }
                    let owner = self.owners.owner(owner_sector)?.ok_or_else(|| {
                        CandidateRoutedError::InvalidInput("admitted owner disappeared".into())
                    })?;
                    let result = if owner.batches[0].terminals.contains(key) {
                        OwnerStep::Terminal
                    } else {
                        request
                            .algebra
                            .record_rule_application(limits.max_rule_applications)?;
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
                                    retain_conditions(&mut conditions, local);
                                    terms
                                }))
                        })?
                    };
                    match result {
                        OwnerStep::Terminal => {
                            request.active.remove(&node);
                            self.cache_insert_sparse(
                                node.clone(),
                                BTreeMap::from([(key.clone(), coefficients.one())]),
                                conditions,
                                request,
                            )?;
                            continue;
                        }
                        OwnerStep::Uncovered => {
                            return Err(CandidateReductionError::Uncovered {
                                target: key.clone(),
                            }
                            .into());
                        }
                        OwnerStep::Applied(terms) => {
                            request.stats.work.record_rule_application();
                            let mut children = BTreeMap::new();
                            for (child, coefficient) in terms {
                                let child_node = successor(key, child, *owner_sector)?;
                                children.insert(child_node, coefficient);
                            }
                            children
                        }
                    }
                }
            };
            // Schedule in sorted order; Route and Apply(owner, key) are distinct
            // memoized states even when they refer to the same physical key.
            let terms = Arc::new(terms);
            request.push(
                Frame::Combine {
                    node,
                    terms: terms.clone(),
                    conditions,
                },
                limits.max_pending_frames,
            )?;
            for child in terms.keys().rev() {
                self.pin_dependency(child, &mut request.pins)?;
                request.push(Frame::Enter(child.clone()), limits.max_pending_frames)?;
            }
        }
        Ok(())
    }

    fn cache_insert_sparse(
        &mut self,
        node: Node<N>,
        terms: BTreeMap<IntegralKey, Coefficient>,
        conditions: Vec<Coefficient>,
        request: &mut Request<N>,
    ) -> Result<(), CandidateRoutedError> {
        let context = &self.context;
        let terms = CachedTerms::from_sparse(
            terms,
            self.cache_representation,
            context.shared.context.base(),
            context.limits,
        )?;
        self.cache_insert(
            node,
            terms,
            conditions,
            &request.pins,
            &mut request.stats.cache,
        )
    }
}
