use std::collections::BTreeSet;

use super::super::CandidateReductionError;
use super::super::evaluator::{CandidateEvaluator, validate_entry_rank};
use super::model::*;
use crate::family::IntegralKey;
use crate::reduction::{ReductionRequest, ReductionStatistics};
use crate::sector::symmetry::integral_transport::ExpansionError;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Node<const N: usize> {
    Route(IntegralKey),
    Apply([bool; N], IntegralKey),
}
impl<const N: usize> Node<N> {
    fn key(&self) -> &IntegralKey {
        match self {
            Self::Route(k) | Self::Apply(_, k) => k,
        }
    }
}
enum Frame<const N: usize> {
    Enter(Node<N>),
    Leave(Node<N>),
}

fn charge(
    value: &mut usize,
    addition: usize,
    limit: usize,
    resource: &'static str,
) -> Result<(), CandidateRoutedError> {
    let requested = value
        .checked_add(addition)
        .ok_or(CandidateRoutedError::ResourceLimit {
            resource,
            requested: usize::MAX,
            limit,
        })?;
    if requested > limit {
        return Err(CandidateRoutedError::ResourceLimit {
            resource,
            requested,
            limit,
        });
    }
    *value = requested;
    Ok(())
}

struct Traversal<const N: usize> {
    seen: BTreeSet<Node<N>>,
    active: BTreeSet<Node<N>>,
    physical: BTreeSet<IntegralKey>,
    stack: Vec<Frame<N>>,
    request: ReductionRequest,
}
impl<const N: usize> Traversal<N> {
    fn schedule(
        &mut self,
        node: Node<N>,
        nodes: usize,
        pending: usize,
    ) -> Result<(), CandidateRoutedError> {
        if self.active.contains(&node) {
            return Err(CandidateRoutedError::Cycle {
                target: node.key().clone(),
            });
        }
        if self.seen.contains(&node) {
            return Ok(());
        }
        let mut count = self.seen.len();
        charge(&mut count, 1, nodes, "operational nodes")?;
        self.request.retain_pending_frame(pending)?;
        self.physical.insert(node.key().clone());
        self.seen.insert(node.clone());
        self.stack.push(Frame::Enter(node));
        Ok(())
    }
}

fn support<const N: usize>(key: &IntegralKey) -> [bool; N] {
    std::array::from_fn(|i| key.powers()[i] > 0)
}
fn count<const N: usize>(mask: &[bool; N]) -> usize {
    mask.iter().filter(|&&x| x).count()
}

impl<const N: usize> RoutedCandidateReducer<N> {
    /// Follow exact local candidate successors with a single shared request.
    /// Only absent owners/rules become frontier entries. No back-substitution,
    /// coefficient cache, source replay, terminal inference or closure proof.
    pub fn trace_targets(
        &self,
        targets: impl IntoIterator<Item = IntegralKey>,
    ) -> Result<CandidateRoutedTraceReport<N>, CandidateRoutedError> {
        let context = &self.programs.context;
        let limits = context.limits;
        let shared = &context.shared;
        let base = CandidateEvaluator {
            context: &shared.context,
            root_sector: [true; N],
            ordering: Default::default(),
            rules: &[],
            source_conditions: &shared.source_conditions,
            zero_sectors: &shared.zero_sectors,
            limits,
        };
        let mut report = CandidateRoutedTraceReport::default();
        report.family_fingerprint = context.family.fingerprint_owner();
        let mut traversal = Traversal {
            seen: BTreeSet::new(),
            active: BTreeSet::new(),
            physical: BTreeSet::new(),
            stack: Vec::new(),
            request: ReductionRequest::default(),
        };
        // Materialize distinct entries in sorted order, independently of their
        // input iterator ordering. Every input, including duplicates, is charged.
        let mut entries = BTreeSet::new();
        for target in targets {
            charge(
                &mut report.input_targets,
                1,
                self.limits.max_input_targets,
                "input targets",
            )?;
            base.validate_target(&target)?;
            validate_entry_rank(&target, context.scope.max_numerator_rank)?;
            if !entries.contains(&target) {
                let mut count = entries.len();
                charge(
                    &mut count,
                    1,
                    self.limits.max_unique_nodes,
                    "initial operational nodes",
                )?;
                let mut pending = entries.len();
                charge(
                    &mut pending,
                    1,
                    limits.max_pending_frames,
                    "initial pending frames",
                )?;
            }
            entries.insert(target);
        }
        report.requested_targets = entries.len();
        for target in entries.into_iter().rev() {
            traversal.schedule(
                Node::Route(target),
                self.limits.max_unique_nodes,
                limits.max_pending_frames,
            )?;
        }
        let mut statistics = ReductionStatistics::default();
        while let Some(frame) = traversal.stack.pop() {
            traversal.request.release_pending_frame()?;
            let node = match frame {
                Frame::Leave(node) => {
                    if !traversal.active.remove(&node) {
                        return Err(CandidateRoutedError::InvalidInput(
                            "routed leave state was not active".into(),
                        ));
                    }
                    continue;
                }
                Frame::Enter(node) => node,
            };
            if !traversal.active.insert(node.clone()) {
                return Err(CandidateRoutedError::Cycle {
                    target: node.key().clone(),
                });
            }
            traversal
                .request
                .retain_pending_frame(limits.max_pending_frames)?;
            traversal.stack.push(Frame::Leave(node.clone()));
            let key = node.key();
            base.validate_target(key)?;
            let mut rank = 0u128;
            let mut dots = 0u128;
            for &power in key.powers() {
                let value = if power < 0 { &mut rank } else { &mut dots };
                let addition = if power < 0 {
                    u128::from(power.unsigned_abs())
                } else {
                    u128::from((power as u64).saturating_sub(1))
                };
                *value = value.checked_add(addition).ok_or_else(|| {
                    CandidateRoutedError::InvalidInput("routed degree census overflow".into())
                })?;
            }
            report.max_numerator_rank = report.max_numerator_rank.max(rank);
            report.max_dot_excess = report.max_dot_excess.max(dots);
            if base.is_zero(key) {
                report.visited_zeros.insert(key.clone());
                continue;
            }
            let parent_support = support::<N>(key);
            match &node {
                Node::Route(_) => {
                    if self.programs.owners.contains_key(&parent_support) {
                        traversal.schedule(
                            Node::Apply(parent_support, key.clone()),
                            self.limits.max_unique_nodes,
                            limits.max_pending_frames,
                        )?;
                    } else if let Some(route) = self.routes.get(&parent_support) {
                        charge(
                            &mut report.transport_calls,
                            1,
                            self.limits.max_transport_calls,
                            "transport calls",
                        )?;
                        let mapped = route.transport.transport_with_usage(
                            key,
                            self.limits.expansion,
                            |usage| {
                                let operations = report
                                    .transport_operations
                                    .checked_add(usage.operations)
                                    .ok_or(ExpansionError::ResourceCountOverflow {
                                        resource: "aggregate routed operations",
                                    })?;
                                let endpoints = report
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
                                report.transport_operations = operations;
                                report.transport_endpoints = endpoints;
                                Ok(())
                            },
                        )?;
                        let owner_support =
                            std::array::from_fn(|i| route.owner_sector.active_bits()[i]);
                        for endpoint in mapped.terms().iter().rev() {
                            let child = endpoint.key();
                            base.validate_target(child)?;
                            let child_support = support::<N>(child);
                            let child_node = if child_support == owner_support {
                                Node::Apply(owner_support, child.clone())
                            } else if child_support
                                .iter()
                                .zip(owner_support)
                                .all(|(&c, o)| !c || o)
                                && count(&child_support) < count(&parent_support)
                            {
                                Node::Route(child.clone())
                            } else {
                                return Err(CandidateRoutedError::UnsupportedSupportTransition {
                                    target: key.clone(),
                                    child: child.clone(),
                                });
                            };
                            traversal.schedule(
                                child_node,
                                self.limits.max_unique_nodes,
                                limits.max_pending_frames,
                            )?;
                        }
                    } else {
                        report.frontier.insert(CandidateRoutedFrontier {
                            target: key.clone(),
                            reason: CandidateRoutedFrontierReason::MissingOwner,
                        });
                    }
                }
                Node::Apply(owner_sector, _) => {
                    if &parent_support != owner_sector {
                        return Err(CandidateRoutedError::InvalidInput(
                            "apply node does not belong to its owner support".into(),
                        ));
                    }
                    let owner = self.programs.owners.get(owner_sector).ok_or_else(|| {
                        CandidateRoutedError::InvalidInput("admitted owner disappeared".into())
                    })?;
                    if owner.terminals.contains(key) {
                        report.declared_terminals.insert(key.clone());
                        continue;
                    }
                    traversal
                        .request
                        .record_rule_application(limits.max_rule_applications)?;
                    let evaluator = CandidateEvaluator {
                        context: &shared.context,
                        root_sector: owner.root,
                        ordering: owner.ordering,
                        rules: &owner.rules,
                        source_conditions: &shared.source_conditions,
                        zero_sectors: &shared.zero_sectors,
                        limits,
                    };
                    match evaluator.apply(key, &mut traversal.request, &mut statistics) {
                        Ok(terms) => {
                            charge(
                                &mut report.rule_applications,
                                1,
                                limits.max_rule_applications,
                                "successful rule applications",
                            )?;
                            for child in terms.into_keys().rev() {
                                let child_support = support::<N>(&child);
                                let child_node = if &child_support == owner_sector {
                                    Node::Apply(*owner_sector, child)
                                } else if child_support
                                    .iter()
                                    .zip(owner_sector)
                                    .all(|(&c, &o)| !c || o)
                                    && count(&child_support) < count(owner_sector)
                                {
                                    Node::Route(child)
                                } else {
                                    return Err(
                                        CandidateRoutedError::UnsupportedSupportTransition {
                                            target: key.clone(),
                                            child,
                                        },
                                    );
                                };
                                traversal.schedule(
                                    child_node,
                                    self.limits.max_unique_nodes,
                                    limits.max_pending_frames,
                                )?;
                            }
                        }
                        Err(CandidateReductionError::Uncovered { target }) => {
                            report.frontier.insert(CandidateRoutedFrontier {
                                target,
                                reason: CandidateRoutedFrontierReason::MissingRule {
                                    owner_sector: *owner_sector,
                                },
                            });
                        }
                        Err(error) => return Err(error.into()),
                    }
                }
            }
            // Sorted exact outputs were streamed directly into the bounded
            // scheduler; no second uncharged child-key vector is retained.
        }
        report.operational_nodes = traversal.seen.len();
        report.reachable_integrals = traversal.physical.len();
        Ok(report)
    }
}
