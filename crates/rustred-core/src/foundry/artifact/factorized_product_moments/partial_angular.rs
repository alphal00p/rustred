//! Forward isotropic elimination of selected independent one-loop vectors.

use std::collections::BTreeMap;

use crate::algebra::{Coefficient, CoefficientContext};

use super::compile::admit_limit;
use super::error::FactorizedProductMomentError;
use super::limits::FactorizedProductMomentLimits;
use super::model::{MomentPower, ProductMomentGuard, SingletonProductBlock};
use super::resources::{CoefficientBudget, admit_guard_key_payload, admit_state_key_payload};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct PartialMomentKey {
    powers: Box<[MomentPower]>,
}

impl PartialMomentKey {
    pub(super) fn radial_powers(&self, loop_count: usize) -> &[MomentPower] {
        &self.powers[..loop_count]
    }

    pub(super) fn cross_powers(&self, loop_count: usize) -> &[MomentPower] {
        &self.powers[loop_count..]
    }
}

pub(super) struct PartialAngularEvaluator<'chart> {
    context: &'chart CoefficientContext,
    dimension: &'chart Coefficient,
    loop_count: usize,
    edges: &'chart [(usize, usize)],
    singleton_vectors: Box<[usize]>,
    guards: BTreeMap<(usize, MomentPower), Coefficient>,
    states: usize,
    transitions: usize,
    attempted_transitions: usize,
    limits: FactorizedProductMomentLimits,
}

impl<'chart> PartialAngularEvaluator<'chart> {
    pub(super) fn try_new(
        context: &'chart CoefficientContext,
        dimension: &'chart Coefficient,
        loop_count: usize,
        edges: &'chart [(usize, usize)],
        singletons: &[SingletonProductBlock],
        limits: FactorizedProductMomentLimits,
    ) -> Result<Self, FactorizedProductMomentError> {
        let mut singleton_vectors = Vec::new();
        singleton_vectors
            .try_reserve_exact(singletons.len())
            .map_err(|_| FactorizedProductMomentError::AllocationFailure {
                resource: "partial-angular singleton vectors",
                requested: singletons.len(),
            })?;
        singleton_vectors.extend(singletons.iter().map(|block| block.transformed_vector));
        singleton_vectors.sort_unstable();
        singleton_vectors.dedup();
        if singleton_vectors.len() != singletons.len() {
            return Err(FactorizedProductMomentError::Invariant {
                detail: "partial-angular singleton vectors are not disjoint",
            });
        }
        Ok(Self {
            context,
            dimension,
            loop_count,
            edges,
            singleton_vectors: singleton_vectors.into_boxed_slice(),
            guards: BTreeMap::new(),
            states: 0,
            transitions: 0,
            attempted_transitions: 0,
            limits,
        })
    }

    pub(super) fn evaluate(
        &mut self,
        radial_powers: &[MomentPower],
        cross_powers: &[MomentPower],
        budget: &mut CoefficientBudget,
        coalescing_additions: &mut usize,
    ) -> Result<BTreeMap<PartialMomentKey, Coefficient>, FactorizedProductMomentError> {
        if radial_powers.len() != self.loop_count || cross_powers.len() != self.edges.len() {
            return Err(FactorizedProductMomentError::WrongMonomialWidth {
                component: "partial-angular powers",
                expected: self.loop_count + self.edges.len(),
                actual: radial_powers.len() + cross_powers.len(),
            });
        }
        let key_width = self.loop_count.checked_add(self.edges.len()).ok_or(
            FactorizedProductMomentError::ResourceCountOverflow {
                resource: "partial-angular key width",
            },
        )?;
        admit_live_state_keys(0, 0, 1, key_width, self.limits)?;
        admit_limit("angular pending frames", 1, self.limits.max_pending_frames)?;
        let root = self.key(radial_powers, cross_powers, "partial-angular root")?;
        let root_measure = self.measure(root.cross_powers(self.loop_count))?;
        admit_limit(
            "angular degree",
            usize::try_from(root_measure).map_err(|_| {
                FactorizedProductMomentError::ResourceCountOverflow {
                    resource: "partial-angular degree",
                }
            })?,
            self.limits.max_angular_degree,
        )?;
        let mut pending = BTreeMap::new();
        let one = self.context.one();
        budget.retain(&one)?;
        pending.insert((root_measure, root), one);
        let mut output = BTreeMap::new();

        while let Some(((measure, key), coefficient)) = pending.pop_last() {
            let prospective_states = self.states.checked_add(1).ok_or(
                FactorizedProductMomentError::ResourceCountOverflow {
                    resource: "partial-angular states",
                },
            )?;
            admit_limit(
                "angular states",
                prospective_states,
                self.limits.max_angular_states,
            )?;
            self.states = prospective_states;
            if measure == 0 {
                budget.release(&coefficient)?;
                accumulate_state(
                    self.context,
                    &mut output,
                    key,
                    coefficient,
                    budget,
                    self.limits,
                    coalescing_additions,
                )?;
                continue;
            }

            let cross = key.cross_powers(self.loop_count);
            let (vector, rank) =
                self.select_singleton(cross)?
                    .ok_or(FactorizedProductMomentError::Invariant {
                        detail: "a positive partial-angular measure has no singleton incidence",
                    })?;
            if rank % 2 == 1 {
                budget.release(&coefficient)?;
                continue;
            }
            let rank_offset =
                rank.checked_sub(2)
                    .ok_or(FactorizedProductMomentError::Invariant {
                        detail: "a nonzero even partial-angular rank is below two",
                    })?;
            let denominator = self.context.try_add(
                self.dimension,
                &self.context.unsigned_integer(rank_offset),
                self.limits.exact_algebra,
            )?;
            budget.retain(&denominator)?;
            self.retain_guard(vector, rank, &denominator, budget)?;

            let fixed_edge = cross
                .iter()
                .enumerate()
                .find_map(|(edge, &power)| {
                    (power > 0 && incident(self.edges[edge], vector)).then_some(edge)
                })
                .ok_or(FactorizedProductMomentError::Invariant {
                    detail: "a singleton incidence has no incident edge",
                })?;
            let partner = other_endpoint(self.edges[fixed_edge], vector)?;
            admit_live_state_keys(pending.len(), output.len(), 2, key_width, self.limits)?;
            let mut after_first = clone_key(&key, "partial-angular first contraction")?;
            decrement(
                &mut after_first.powers[self.loop_count + fixed_edge],
                "partial-angular fixed edge",
            )?;

            for edge in 0..self.edges.len() {
                if !incident(self.edges[edge], vector) {
                    continue;
                }
                let slot = self.loop_count + edge;
                let multiplicity = after_first.powers[slot];
                if multiplicity == 0 {
                    continue;
                }
                let prospective_transitions = self.transitions.checked_add(1).ok_or(
                    FactorizedProductMomentError::ResourceCountOverflow {
                        resource: "partial-angular transitions",
                    },
                )?;
                // Record and admit the transition before allocating its child or
                // performing exact coefficient algebra.  The caller can thereby
                // charge even a rejected attempt to the aggregate request.
                self.attempted_transitions = prospective_transitions;
                admit_limit(
                    "angular transitions",
                    prospective_transitions,
                    self.limits.max_angular_transitions,
                )?;
                let other = other_endpoint(self.edges[edge], vector)?;
                let prospective_pending = pending.len().checked_add(1).ok_or(
                    FactorizedProductMomentError::ResourceCountOverflow {
                        resource: "partial-angular pending frames",
                    },
                )?;
                admit_limit(
                    "angular pending frames",
                    prospective_pending,
                    self.limits.max_pending_frames,
                )?;
                admit_live_state_keys(pending.len(), output.len(), 3, key_width, self.limits)?;
                let mut child = clone_key(&after_first, "partial-angular child")?;
                decrement(&mut child.powers[slot], "partial-angular paired edge")?;
                child.powers[vector] = child.powers[vector].checked_add(1).ok_or(
                    FactorizedProductMomentError::ResourceCountOverflow {
                        resource: "partial-angular singleton radial power",
                    },
                )?;
                if partner == other {
                    child.powers[partner] = child.powers[partner].checked_add(1).ok_or(
                        FactorizedProductMomentError::ResourceCountOverflow {
                            resource: "partial-angular partner radial power",
                        },
                    )?;
                } else {
                    let created = edge_slot(self.edges, partner, other)?;
                    let created = self.loop_count + created;
                    child.powers[created] = child.powers[created].checked_add(1).ok_or(
                        FactorizedProductMomentError::ResourceCountOverflow {
                            resource: "partial-angular created cross power",
                        },
                    )?;
                }
                let child_measure = self.measure(child.cross_powers(self.loop_count))?;
                if child_measure >= measure {
                    return Err(FactorizedProductMomentError::Invariant {
                        detail: "partial isotropy did not strictly lower singleton incidence",
                    });
                }
                let weighted = self.context.try_mul(
                    &coefficient,
                    &self.context.unsigned_integer(multiplicity),
                    self.limits.exact_algebra,
                )?;
                let contribution =
                    self.context
                        .try_div(&weighted, &denominator, self.limits.exact_algebra)?;
                budget.admit_temporaries([&weighted, &contribution])?;
                self.transitions = prospective_transitions;
                accumulate_pending(
                    self.context,
                    &mut pending,
                    child_measure,
                    child,
                    contribution,
                    budget,
                    self.limits,
                    coalescing_additions,
                    Some(&weighted),
                )?;
            }
            budget.release(&denominator)?;
            budget.release(&coefficient)?;
            admit_live_state_keys(pending.len(), output.len(), 2, key_width, self.limits)?;
            admit_limit(
                "angular pending frames",
                pending.len(),
                self.limits.max_pending_frames,
            )?;
        }
        Ok(output)
    }

    fn select_singleton(
        &self,
        cross: &[MomentPower],
    ) -> Result<Option<(usize, MomentPower)>, FactorizedProductMomentError> {
        for &vector in &self.singleton_vectors {
            let mut rank = 0_u128;
            for (edge, &power) in self.edges.iter().zip(cross) {
                if incident(*edge, vector) {
                    rank = rank.checked_add(power).ok_or(
                        FactorizedProductMomentError::ResourceCountOverflow {
                            resource: "partial-angular singleton rank",
                        },
                    )?;
                }
            }
            if rank > 0 {
                return Ok(Some((vector, rank)));
            }
        }
        Ok(None)
    }

    fn measure(&self, cross: &[MomentPower]) -> Result<MomentPower, FactorizedProductMomentError> {
        let mut measure = 0_u128;
        for &vector in &self.singleton_vectors {
            for (edge, &power) in self.edges.iter().zip(cross) {
                if incident(*edge, vector) {
                    measure = measure.checked_add(power).ok_or(
                        FactorizedProductMomentError::ResourceCountOverflow {
                            resource: "partial-angular incidence measure",
                        },
                    )?;
                }
            }
        }
        Ok(measure)
    }

    fn key(
        &self,
        radial: &[MomentPower],
        cross: &[MomentPower],
        resource: &'static str,
    ) -> Result<PartialMomentKey, FactorizedProductMomentError> {
        let count = radial
            .len()
            .checked_add(cross.len())
            .ok_or(FactorizedProductMomentError::ResourceCountOverflow { resource })?;
        let mut powers = Vec::new();
        powers.try_reserve_exact(count).map_err(|_| {
            FactorizedProductMomentError::AllocationFailure {
                resource,
                requested: count,
            }
        })?;
        powers.extend_from_slice(radial);
        powers.extend_from_slice(cross);
        Ok(PartialMomentKey {
            powers: powers.into_boxed_slice(),
        })
    }

    fn retain_guard(
        &mut self,
        vector: usize,
        rank: MomentPower,
        denominator: &Coefficient,
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
        let retained = denominator.clone();
        budget.retain(&retained)?;
        self.guards.insert((vector, rank), retained);
        Ok(())
    }

    pub(super) fn attempted_transition_count(&self) -> usize {
        self.attempted_transitions
    }

    pub(super) fn finish(self) -> Result<Box<[ProductMomentGuard]>, FactorizedProductMomentError> {
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

fn admit_live_state_keys(
    pending: usize,
    output: usize,
    temporaries: usize,
    width: usize,
    limits: FactorizedProductMomentLimits,
) -> Result<(), FactorizedProductMomentError> {
    let rows = pending
        .checked_add(output)
        .and_then(|value| value.checked_add(temporaries))
        .ok_or(FactorizedProductMomentError::ResourceCountOverflow {
            resource: "partial-angular retained keys",
        })?;
    admit_state_key_payload(rows, width, limits)
}

fn accumulate_pending(
    context: &CoefficientContext,
    map: &mut BTreeMap<(MomentPower, PartialMomentKey), Coefficient>,
    measure: MomentPower,
    key: PartialMomentKey,
    coefficient: Coefficient,
    budget: &mut CoefficientBudget,
    limits: FactorizedProductMomentLimits,
    additions: &mut usize,
    live_weighted: Option<&Coefficient>,
) -> Result<(), FactorizedProductMomentError> {
    accumulate_generic(
        context,
        map,
        (measure, key),
        coefficient,
        budget,
        limits,
        additions,
        live_weighted,
    )
}

fn accumulate_state(
    context: &CoefficientContext,
    map: &mut BTreeMap<PartialMomentKey, Coefficient>,
    key: PartialMomentKey,
    coefficient: Coefficient,
    budget: &mut CoefficientBudget,
    limits: FactorizedProductMomentLimits,
    additions: &mut usize,
) -> Result<(), FactorizedProductMomentError> {
    accumulate_generic(
        context,
        map,
        key,
        coefficient,
        budget,
        limits,
        additions,
        None,
    )
}

fn accumulate_generic<K: Ord>(
    context: &CoefficientContext,
    map: &mut BTreeMap<K, Coefficient>,
    key: K,
    coefficient: Coefficient,
    budget: &mut CoefficientBudget,
    limits: FactorizedProductMomentLimits,
    additions: &mut usize,
    live_weighted: Option<&Coefficient>,
) -> Result<(), FactorizedProductMomentError> {
    if coefficient.is_zero() {
        return Ok(());
    }
    let retained_states = map.len();
    match map.entry(key) {
        std::collections::btree_map::Entry::Vacant(entry) => {
            let prospective_states = retained_states.checked_add(1).ok_or(
                FactorizedProductMomentError::ResourceCountOverflow {
                    resource: "partial-angular map terms",
                },
            )?;
            admit_limit(
                "angular states",
                prospective_states,
                limits.max_angular_states,
            )?;
            budget.retain(&coefficient)?;
            entry.insert(coefficient);
        }
        std::collections::btree_map::Entry::Occupied(mut entry) => {
            let prospective = additions.checked_add(1).ok_or(
                FactorizedProductMomentError::ResourceCountOverflow {
                    resource: "product coefficient coalescing additions",
                },
            )?;
            // Count the attempted merge before admission so a rejected
            // one-beyond-limit operation is still visible to the aggregate
            // request and telemetry owner.
            *additions = prospective;
            admit_limit(
                "product coefficient coalescing additions",
                prospective,
                limits.max_coalescing_additions,
            )?;
            let sum = context.try_add(entry.get(), &coefficient, limits.exact_algebra)?;
            if let Some(weighted) = live_weighted {
                // `weighted` remains live while division has produced
                // `coefficient` and coalescing has produced `sum`.
                budget.admit_temporaries([weighted, &coefficient, &sum])?;
            } else {
                budget.admit_temporaries([&coefficient, &sum])?;
            }
            if sum.is_zero() {
                budget.release(entry.get())?;
                entry.remove();
            } else {
                budget.replace(entry.get(), &sum)?;
                *entry.get_mut() = sum;
            }
        }
    }
    Ok(())
}

fn clone_key(
    key: &PartialMomentKey,
    resource: &'static str,
) -> Result<PartialMomentKey, FactorizedProductMomentError> {
    let mut powers = Vec::new();
    powers.try_reserve_exact(key.powers.len()).map_err(|_| {
        FactorizedProductMomentError::AllocationFailure {
            resource,
            requested: key.powers.len(),
        }
    })?;
    powers.extend_from_slice(&key.powers);
    Ok(PartialMomentKey {
        powers: powers.into_boxed_slice(),
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
            detail: "an angular edge is not incident to the selected vector",
        })
    }
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
            detail: "a complete cross edge is absent",
        })
}

fn decrement(
    value: &mut MomentPower,
    resource: &'static str,
) -> Result<(), FactorizedProductMomentError> {
    *value = value
        .checked_sub(1)
        .ok_or(FactorizedProductMomentError::ResourceCountOverflow { resource })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn occupied_partial_merge_admits_weighted_contribution_and_sum_peak() {
        let context = CoefficientContext::try_new(["d"]).unwrap();
        let limits = FactorizedProductMomentLimits {
            // Four constant rational coefficients are simultaneously live:
            // retained entry, weighted numerator, divided contribution, sum.
            // Each has one numerator and one denominator term.
            max_retained_coefficient_terms: 7,
            ..FactorizedProductMomentLimits::default()
        };
        let mut budget = CoefficientBudget::new(limits);
        let retained = context.one();
        budget.retain(&retained).unwrap();
        let mut map = BTreeMap::from([(0_u8, retained)]);
        let weighted = context.one();
        let contribution = context.one();
        budget
            .admit_temporaries([&weighted, &contribution])
            .unwrap();
        let mut additions = 0;
        assert_eq!(
            accumulate_generic(
                &context,
                &mut map,
                0,
                contribution,
                &mut budget,
                limits,
                &mut additions,
                Some(&weighted),
            ),
            Err(FactorizedProductMomentError::ResourceLimit {
                resource: "product retained coefficient terms",
                requested: 8,
                limit: 7,
            })
        );
        assert_eq!(additions, 1, "a rejected coalescing attempt is observable");
    }

    #[test]
    fn occupied_partial_merge_does_not_consume_a_second_state_slot() {
        let context = CoefficientContext::try_new(["d"]).unwrap();
        let limits = FactorizedProductMomentLimits {
            max_angular_states: 1,
            ..FactorizedProductMomentLimits::default()
        };
        let mut budget = CoefficientBudget::new(limits);
        let retained = context.one();
        budget.retain(&retained).unwrap();
        let mut map = BTreeMap::from([(0_u8, retained)]);
        let mut additions = 0;

        accumulate_generic(
            &context,
            &mut map,
            0,
            context.one(),
            &mut budget,
            limits,
            &mut additions,
            None,
        )
        .unwrap();

        assert_eq!(map.len(), 1);
        assert_eq!(map[&0], context.integer(2));
        assert_eq!(additions, 1);
    }
}
