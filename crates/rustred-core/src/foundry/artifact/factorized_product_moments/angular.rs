//! Iterative isotropic moments for an edge monomial of independent vectors.

use std::collections::BTreeMap;

use crate::algebra::{Coefficient, CoefficientContext};

use super::compile::admit_limit;
use super::error::FactorizedProductMomentError;
use super::limits::FactorizedProductMomentLimits;
use super::model::ProductMomentGuard;
use super::resources::{
    CoefficientBudget, admit_angular_order_key_payload, admit_guard_key_payload,
    admit_state_key_payload,
};

struct AngularBranch {
    multiplicity: u64,
    child: Box<[u64]>,
}

enum AngularPlan {
    One,
    Zero,
    Recurrence {
        vector: usize,
        rank: u64,
        branches: Box<[AngularBranch]>,
    },
}

enum AngularNode {
    Plan(AngularPlan),
    Value(Coefficient),
}

pub(super) struct AngularEvaluator<'chart> {
    context: &'chart CoefficientContext,
    dimension: &'chart Coefficient,
    loop_count: usize,
    edges: &'chart [(usize, usize)],
    nodes: BTreeMap<Box<[u64]>, AngularNode>,
    guards: BTreeMap<(usize, u64), Coefficient>,
    transitions: usize,
    limits: FactorizedProductMomentLimits,
}

impl<'chart> AngularEvaluator<'chart> {
    pub(super) fn new(
        context: &'chart CoefficientContext,
        dimension: &'chart Coefficient,
        loop_count: usize,
        edges: &'chart [(usize, usize)],
        limits: FactorizedProductMomentLimits,
    ) -> Self {
        Self {
            context,
            dimension,
            loop_count,
            edges,
            nodes: BTreeMap::new(),
            guards: BTreeMap::new(),
            transitions: 0,
            limits,
        }
    }

    pub(super) fn evaluate(
        &mut self,
        cross_powers: &[u64],
        budget: &mut CoefficientBudget,
    ) -> Result<Coefficient, FactorizedProductMomentError> {
        if cross_powers.len() != self.edges.len() {
            return Err(FactorizedProductMomentError::WrongMonomialWidth {
                component: "cross powers",
                expected: self.edges.len(),
                actual: cross_powers.len(),
            });
        }
        let root = clone_powers(cross_powers, "angular root state")?;
        if let Some(AngularNode::Value(value)) = self.nodes.get(&root) {
            let output = value.clone();
            budget.admit_temporaries([&output])?;
            return Ok(output);
        }
        self.discover(root.clone())?;
        self.evaluate_discovered(budget)?;
        let AngularNode::Value(value) =
            self.nodes
                .get(&root)
                .ok_or(FactorizedProductMomentError::Invariant {
                    detail: "the iterative angular DP did not retain its requested root",
                })?
        else {
            return Err(FactorizedProductMomentError::Invariant {
                detail: "the iterative angular root remained unevaluated",
            });
        };
        let output = value.clone();
        budget.admit_temporaries([&output])?;
        Ok(output)
    }

    fn discover(&mut self, root: Box<[u64]>) -> Result<(), FactorizedProductMomentError> {
        let mut pending = Vec::new();
        pending
            .try_reserve(1)
            .map_err(|_| FactorizedProductMomentError::AllocationFailure {
                resource: "angular pending frames",
                requested: 1,
            })?;
        pending.push(root);
        while let Some(key) = pending.pop() {
            if self.nodes.contains_key(&key) {
                continue;
            }
            let plan = self.plan(&key)?;
            let children = match &plan {
                AngularPlan::Recurrence { branches, .. } => branches.as_ref(),
                AngularPlan::One | AngularPlan::Zero => &[],
            };
            let prospective_transitions = self.transitions.checked_add(children.len()).ok_or(
                FactorizedProductMomentError::ResourceCountOverflow {
                    resource: "angular transitions",
                },
            )?;
            admit_limit(
                "angular transitions",
                prospective_transitions,
                self.limits.max_angular_transitions,
            )?;
            for branch in children {
                if !self.nodes.contains_key(&branch.child) {
                    pending.try_reserve(1).map_err(|_| {
                        FactorizedProductMomentError::AllocationFailure {
                            resource: "angular pending frames",
                            requested: pending.len().saturating_add(1),
                        }
                    })?;
                    pending.push(clone_powers(&branch.child, "angular pending child")?);
                }
            }
            admit_limit(
                "angular pending frames",
                pending.len(),
                self.limits.max_pending_frames,
            )?;
            let prospective_states = self.nodes.len().checked_add(1).ok_or(
                FactorizedProductMomentError::ResourceCountOverflow {
                    resource: "angular states",
                },
            )?;
            admit_limit(
                "angular states",
                prospective_states,
                self.limits.max_angular_states,
            )?;
            let retained_keys = prospective_states
                .checked_add(prospective_transitions)
                .and_then(|count| count.checked_add(pending.len()))
                .ok_or(FactorizedProductMomentError::ResourceCountOverflow {
                    resource: "angular retained state keys",
                })?;
            admit_state_key_payload(retained_keys, self.edges.len(), self.limits)?;
            self.transitions = prospective_transitions;
            self.nodes.insert(key, AngularNode::Plan(plan));
        }
        Ok(())
    }

    fn plan(&self, key: &[u64]) -> Result<AngularPlan, FactorizedProductMomentError> {
        let degree = checked_sum("angular degree", key)?;
        admit_limit("angular degree", degree, self.limits.max_angular_degree)?;
        if degree == 0 {
            return Ok(AngularPlan::One);
        }
        let incidences = incidences(self.loop_count, self.edges, key)?;
        if incidences.iter().any(|rank| rank % 2 == 1) {
            return Ok(AngularPlan::Zero);
        }
        let vector = incidences.iter().position(|&rank| rank > 0).ok_or(
            FactorizedProductMomentError::Invariant {
                detail: "a positive cross degree has no incident vector",
            },
        )?;
        let rank = incidences[vector];
        let partner_edge = key
            .iter()
            .enumerate()
            .find_map(|(edge, &power)| {
                (power > 0 && incident(self.edges[edge], vector)).then_some(edge)
            })
            .ok_or(FactorizedProductMomentError::Invariant {
                detail: "a positive vector incidence has no partner edge",
            })?;
        let partner = other_endpoint(self.edges[partner_edge], vector)?;
        let mut after_first = clone_powers(key, "angular recurrence child")?;
        decrement(&mut after_first[partner_edge], "angular edge decrement")?;
        let mut branches = Vec::new();
        branches
            .try_reserve(self.loop_count.saturating_sub(1))
            .map_err(|_| FactorizedProductMomentError::AllocationFailure {
                resource: "angular recurrence branches",
                requested: self.loop_count.saturating_sub(1),
            })?;
        for other in 0..self.loop_count {
            if other == vector {
                continue;
            }
            let edge = edge_slot(self.edges, vector, other)?;
            let multiplicity = after_first[edge];
            if multiplicity == 0 {
                continue;
            }
            let mut child = clone_powers(&after_first, "angular recurrence child")?;
            decrement(&mut child[edge], "angular edge decrement")?;
            if partner != other {
                let created = edge_slot(self.edges, partner, other)?;
                child[created] = child[created].checked_add(1).ok_or(
                    FactorizedProductMomentError::ResourceCountOverflow {
                        resource: "angular child edge power",
                    },
                )?;
            }
            let child_degree = checked_sum("angular child degree", &child)?;
            if child_degree >= degree {
                return Err(FactorizedProductMomentError::Invariant {
                    detail: "the isotropic recurrence did not strictly lower cross degree",
                });
            }
            branches.push(AngularBranch {
                multiplicity,
                child,
            });
        }
        Ok(AngularPlan::Recurrence {
            vector,
            rank,
            branches: branches.into_boxed_slice(),
        })
    }

    fn evaluate_discovered(
        &mut self,
        budget: &mut CoefficientBudget,
    ) -> Result<(), FactorizedProductMomentError> {
        let plan_count = self
            .nodes
            .values()
            .filter(|node| matches!(node, AngularNode::Plan(_)))
            .count();
        let retained_rows = self.nodes.len().checked_add(self.transitions).ok_or(
            FactorizedProductMomentError::ResourceCountOverflow {
                resource: "angular retained state keys",
            },
        )?;
        // The evaluation order duplicates every still-planned key. Admit that
        // complete live peak before reserving or cloning the first key.
        admit_angular_order_key_payload(retained_rows, plan_count, self.edges.len(), self.limits)?;
        let mut order = Vec::new();
        order.try_reserve_exact(plan_count).map_err(|_| {
            FactorizedProductMomentError::AllocationFailure {
                resource: "angular evaluation order",
                requested: plan_count,
            }
        })?;
        for (key, node) in &self.nodes {
            if matches!(node, AngularNode::Plan(_)) {
                order.push((
                    checked_sum("angular evaluation-order degree", key)?,
                    clone_powers(key, "angular evaluation order")?,
                ));
            }
        }
        order.sort_unstable_by(|left, right| {
            left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1))
        });
        for (_, key) in order {
            let node = self
                .nodes
                .remove(&key)
                .ok_or(FactorizedProductMomentError::Invariant {
                    detail: "the angular evaluation order references an absent state",
                })?;
            let AngularNode::Plan(plan) = node else {
                return Err(FactorizedProductMomentError::Invariant {
                    detail: "the angular evaluation order contains an evaluated state",
                });
            };
            let value = self.evaluate_plan(plan, budget)?;
            budget.retain(&value)?;
            self.nodes.insert(key, AngularNode::Value(value));
        }
        Ok(())
    }

    fn evaluate_plan(
        &mut self,
        plan: AngularPlan,
        budget: &mut CoefficientBudget,
    ) -> Result<Coefficient, FactorizedProductMomentError> {
        match plan {
            AngularPlan::One => Ok(self.context.one()),
            AngularPlan::Zero => Ok(self.context.zero()),
            AngularPlan::Recurrence {
                vector,
                rank,
                branches,
            } => {
                let rank_offset =
                    rank.checked_sub(2)
                        .ok_or(FactorizedProductMomentError::Invariant {
                            detail: "a nonzero even angular rank is below two",
                        })?;
                let rank_offset = i64::try_from(rank_offset)
                    .map_err(|_| FactorizedProductMomentError::RankCoefficientOverflow { rank })?;
                let denominator = self.context.try_add(
                    self.dimension,
                    &self.context.integer(rank_offset),
                    self.limits.exact_algebra,
                )?;
                budget.retain(&denominator)?;
                self.retain_guard(vector, rank, &denominator, budget)?;
                let mut numerator = self.context.zero();
                budget.retain(&numerator)?;
                for branch in branches {
                    let child = match self.nodes.get(&branch.child) {
                        Some(AngularNode::Value(value)) => value,
                        _ => {
                            return Err(FactorizedProductMomentError::Invariant {
                                detail: "an angular child was not evaluated before its parent",
                            });
                        }
                    };
                    let multiplicity = i64::try_from(branch.multiplicity).map_err(|_| {
                        FactorizedProductMomentError::RankCoefficientOverflow {
                            rank: branch.multiplicity,
                        }
                    })?;
                    let weighted = {
                        let multiplicity = self.context.integer(multiplicity);
                        let weighted = self.context.try_mul(
                            &multiplicity,
                            child,
                            self.limits.exact_algebra,
                        )?;
                        budget.admit_temporaries([&multiplicity, &weighted])?;
                        weighted
                    };
                    let sum =
                        self.context
                            .try_add(&numerator, &weighted, self.limits.exact_algebra)?;
                    budget.admit_temporaries([&weighted, &sum])?;
                    drop(weighted);
                    budget.replace(&numerator, &sum)?;
                    numerator = sum;
                }
                let quotient =
                    self.context
                        .try_div(&numerator, &denominator, self.limits.exact_algebra)?;
                budget.admit_temporaries([&quotient])?;
                budget.release(&numerator)?;
                budget.release(&denominator)?;
                Ok(quotient)
            }
        }
    }

    fn retain_guard(
        &mut self,
        vector: usize,
        rank: u64,
        polynomial: &Coefficient,
        budget: &mut CoefficientBudget,
    ) -> Result<(), FactorizedProductMomentError> {
        if self.guards.contains_key(&(vector, rank)) {
            return Ok(());
        }
        let requested = self.guards.len().checked_add(1).ok_or(
            FactorizedProductMomentError::ResourceCountOverflow {
                resource: "product moment guards",
            },
        )?;
        admit_limit("product moment guards", requested, self.limits.max_guards)?;
        admit_guard_key_payload(requested, self.limits)?;
        let retained = polynomial.clone();
        budget.retain(&retained)?;
        self.guards.insert((vector, rank), retained);
        Ok(())
    }

    pub(super) fn state_count(&self) -> usize {
        self.nodes.len()
    }

    pub(super) fn transition_count(&self) -> usize {
        self.transitions
    }

    pub(super) fn finish(
        mut self,
        budget: &mut CoefficientBudget,
    ) -> Result<Box<[ProductMomentGuard]>, FactorizedProductMomentError> {
        for node in self.nodes.values() {
            let AngularNode::Value(coefficient) = node else {
                return Err(FactorizedProductMomentError::Invariant {
                    detail: "the angular cache retains an unevaluated plan",
                });
            };
            budget.release(coefficient)?;
        }
        self.nodes.clear();
        let mut guards = Vec::new();
        guards.try_reserve_exact(self.guards.len()).map_err(|_| {
            FactorizedProductMomentError::AllocationFailure {
                resource: "product moment guards",
                requested: self.guards.len(),
            }
        })?;
        guards.extend(
            self.guards.into_iter().map(|((vector, rank), polynomial)| {
                ProductMomentGuard::new(vector, rank, polynomial)
            }),
        );
        Ok(guards.into_boxed_slice())
    }
}

pub(super) fn cross_radial_powers(
    loop_count: usize,
    edges: &[(usize, usize)],
    powers: &[u64],
) -> Result<Option<Box<[u64]>>, FactorizedProductMomentError> {
    let incidences = incidences(loop_count, edges, powers)?;
    if incidences.iter().any(|rank| rank % 2 == 1) {
        return Ok(None);
    }
    let mut radial = Vec::new();
    radial.try_reserve_exact(loop_count).map_err(|_| {
        FactorizedProductMomentError::AllocationFailure {
            resource: "angular radial powers",
            requested: loop_count,
        }
    })?;
    radial.extend(incidences.into_iter().map(|rank| rank / 2));
    Ok(Some(radial.into_boxed_slice()))
}

fn incidences(
    loop_count: usize,
    edges: &[(usize, usize)],
    powers: &[u64],
) -> Result<Vec<u64>, FactorizedProductMomentError> {
    if powers.len() != edges.len() {
        return Err(FactorizedProductMomentError::WrongMonomialWidth {
            component: "cross powers",
            expected: edges.len(),
            actual: powers.len(),
        });
    }
    let mut output = Vec::new();
    output.try_reserve_exact(loop_count).map_err(|_| {
        FactorizedProductMomentError::AllocationFailure {
            resource: "angular vector incidences",
            requested: loop_count,
        }
    })?;
    output.resize(loop_count, 0_u64);
    for (&(left, right), &power) in edges.iter().zip(powers) {
        output[left] = output[left].checked_add(power).ok_or(
            FactorizedProductMomentError::ResourceCountOverflow {
                resource: "angular vector incidence",
            },
        )?;
        output[right] = output[right].checked_add(power).ok_or(
            FactorizedProductMomentError::ResourceCountOverflow {
                resource: "angular vector incidence",
            },
        )?;
    }
    Ok(output)
}

fn edge_slot(
    edges: &[(usize, usize)],
    left: usize,
    right: usize,
) -> Result<usize, FactorizedProductMomentError> {
    let pair = (left.min(right), left.max(right));
    edges
        .iter()
        .position(|&edge| edge == pair)
        .ok_or(FactorizedProductMomentError::Invariant {
            detail: "the complete product chart lost one cross edge",
        })
}

fn incident(edge: (usize, usize), vector: usize) -> bool {
    edge.0 == vector || edge.1 == vector
}

fn other_endpoint(
    edge: (usize, usize),
    vector: usize,
) -> Result<usize, FactorizedProductMomentError> {
    if edge.0 == vector {
        Ok(edge.1)
    } else if edge.1 == vector {
        Ok(edge.0)
    } else {
        Err(FactorizedProductMomentError::Invariant {
            detail: "an angular partner edge is not incident on the selected vector",
        })
    }
}

fn decrement(power: &mut u64, detail: &'static str) -> Result<(), FactorizedProductMomentError> {
    *power = power
        .checked_sub(1)
        .ok_or(FactorizedProductMomentError::Invariant { detail })?;
    Ok(())
}

fn checked_sum(
    resource: &'static str,
    values: &[u64],
) -> Result<usize, FactorizedProductMomentError> {
    let total = values.iter().try_fold(0_u64, |total, &value| {
        total
            .checked_add(value)
            .ok_or(FactorizedProductMomentError::ResourceCountOverflow { resource })
    })?;
    usize::try_from(total)
        .map_err(|_| FactorizedProductMomentError::ResourceCountOverflow { resource })
}

fn clone_powers(
    powers: &[u64],
    resource: &'static str,
) -> Result<Box<[u64]>, FactorizedProductMomentError> {
    let mut output = Vec::new();
    output.try_reserve_exact(powers.len()).map_err(|_| {
        FactorizedProductMomentError::AllocationFailure {
            resource,
            requested: powers.len(),
        }
    })?;
    output.extend_from_slice(powers);
    Ok(output.into_boxed_slice())
}
