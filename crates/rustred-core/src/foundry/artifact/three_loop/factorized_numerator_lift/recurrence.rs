use crate::algebra::Coefficient;
use crate::foundry::artifact::FactorizationRule;

use super::error::ProbeError;
use super::exact_limits;
use super::limits::{
    ProbeLimits, admit_degree, admit_new_cache_entry, checked_total, record_count,
};
use super::model::{AffineMomentKey, CornerMomentEvaluator};
use super::{ARITY, LOOP_COUNT};

impl CornerMomentEvaluator<'_> {
    /// Evaluate admitted numerator powers in a bounded work frame.
    ///
    /// This is deliberately not an arbitrary-rank API. Both recursion phases
    /// have non-raiseable structural ceilings, and all work/cache growth is
    /// checked before it occurs.
    pub(super) fn evaluate(&mut self, powers: [u64; ARITY]) -> Result<Coefficient, ProbeError> {
        self.limits.validate()?;
        let degree = checked_total("affine numerator degree", &powers)?;
        admit_degree(
            "affine numerator degree",
            degree,
            self.limits.max_affine_degree,
        )?;
        self.evaluate_state(AffineMomentKey {
            remaining_powers: powers,
            cross_powers: [0; 3],
        })
    }

    fn evaluate_state(&mut self, key: AffineMomentKey) -> Result<Coefficient, ProbeError> {
        if let Some(cached) = self.affine_cache.get(&key) {
            return Ok(cached.clone());
        }
        let parent_degree = checked_total("affine recurrence degree", &key.remaining_powers)?;
        admit_degree(
            "affine recurrence degree",
            parent_degree,
            self.limits.max_affine_degree,
        )?;
        let cross_degree = checked_total("angular cross degree", &key.cross_powers)?;
        admit_degree(
            "angular cross degree",
            cross_degree,
            self.limits.max_angular_degree,
        )?;

        let result = if parent_degree == 0 {
            self.angular_moment(key.cross_powers)?
        } else {
            let selected = (0..ARITY)
                .filter(|&slot| key.remaining_powers[slot] > 0)
                .min_by_key(|&slot| self.denominator_priority.rank_by_slot()[slot])
                .ok_or(ProbeError::Invariant {
                    detail: "a positive affine degree had no selectable coordinate",
                })?;
            let mut child_key = key.clone();
            child_key.remaining_powers[selected] = child_key.remaining_powers[selected]
                .checked_sub(1)
                .ok_or(ProbeError::Invariant {
                    detail: "the selected affine coordinate was unexpectedly zero",
                })?;
            let child_degree = checked_total(
                "affine child recurrence degree",
                &child_key.remaining_powers,
            )?;
            if child_degree.checked_add(1) != Some(parent_degree) {
                return Err(ProbeError::Invariant {
                    detail: "an affine recurrence child did not lower total degree by one",
                });
            }

            let form = self.forms[selected].clone();
            let mut result = self.context.zero();
            if !form.constant.is_zero() {
                record_count(
                    "affine recurrence transitions",
                    &mut self.affine_transition_count,
                    self.limits.max_affine_transitions,
                )?;
                let child = self.evaluate_state(child_key.clone())?;
                let contribution = self
                    .context
                    .try_mul(&form.constant, &child, exact_limits())?;
                result = self
                    .context
                    .try_add(&result, &contribution, exact_limits())?;
            }
            for (edge, coefficient) in form.cross_coefficients.iter().enumerate() {
                if coefficient.is_zero() {
                    continue;
                }
                let mut edge_child = child_key.clone();
                edge_child.cross_powers[edge] = edge_child.cross_powers[edge]
                    .checked_add(1)
                    .ok_or(ProbeError::DegreeOverflow {
                        resource: "angular cross degree",
                    })?;
                let edge_degree = checked_total("angular cross degree", &edge_child.cross_powers)?;
                admit_degree(
                    "angular cross degree",
                    edge_degree,
                    self.limits.max_angular_degree,
                )?;
                record_count(
                    "affine recurrence transitions",
                    &mut self.affine_transition_count,
                    self.limits.max_affine_transitions,
                )?;
                let child = self.evaluate_state(edge_child)?;
                let contribution = self.context.try_mul(coefficient, &child, exact_limits())?;
                result = self
                    .context
                    .try_add(&result, &contribution, exact_limits())?;
            }
            result
        };
        admit_new_cache_entry(
            "affine recurrence cache entries",
            self.affine_cache.len(),
            self.limits.max_affine_cache_entries,
        )?;
        self.affine_cache.insert(key, result.clone());
        Ok(result)
    }

    /// Exact normalized spherical moment of three independent vectors with
    /// `q_i^2 = 1`, admitted only by the corner checks in
    /// [`numerator_powers`]. The radial replacement uses the scaleless
    /// polynomial reduction of an undotted, unit-mass one-loop tadpole; it is
    /// not valid for dotted active denominators.
    ///
    /// The denominator `d+r-2` is the standard rank-`r` isotropic-tensor
    /// factor. Its use is recorded, but a future artifact owner must still
    /// persist and replay the exceptional-domain guard `d+r-2 != 0`.
    fn angular_moment(&mut self, powers: [u64; 3]) -> Result<Coefficient, ProbeError> {
        if let Some(cached) = self.angular_cache.get(&powers) {
            return Ok(cached.clone());
        }
        let parent_degree = checked_total("angular recurrence degree", &powers)?;
        admit_degree(
            "angular recurrence degree",
            parent_degree,
            self.limits.max_angular_degree,
        )?;
        let result = if parent_degree == 0 {
            self.context.one()
        } else {
            let vector = self.select_incident_vector(&powers)?;
            let rank = incidence(&powers, vector)?;
            if rank % 2 == 1 {
                self.context.zero()
            } else {
                let partner = self.select_partner(&powers, vector)?;
                let mut after_first = powers;
                decrement_edge(&mut after_first, vector, partner)?;
                let rank_offset = rank.checked_sub(2).ok_or(ProbeError::Invariant {
                    detail: "a nonzero even angular rank was below two",
                })?;
                let rank_offset = i64::try_from(rank_offset)
                    .map_err(|_| ProbeError::RankCoefficientOverflow { rank })?;
                let dimension = self
                    .context
                    .parameter("d")
                    .ok_or(ProbeError::MissingDimensionParameter)?;
                let denominator = self.context.try_add(
                    &dimension,
                    &self.context.integer(rank_offset),
                    exact_limits(),
                )?;
                self.angular_guard_ranks.insert(rank);

                let mut numerator = self.context.zero();
                for other in 0..LOOP_COUNT {
                    if other == vector {
                        continue;
                    }
                    let multiplicity = edge_power(&after_first, vector, other)?;
                    if multiplicity == 0 {
                        continue;
                    }
                    let mut child = after_first;
                    decrement_edge(&mut child, vector, other)?;
                    if partner != other {
                        increment_edge(&mut child, partner, other)?;
                    }
                    let child_degree = checked_total("angular child degree", &child)?;
                    if child_degree >= parent_degree {
                        return Err(ProbeError::Invariant {
                            detail: "an angular recurrence child did not strictly lower degree",
                        });
                    }
                    admit_degree(
                        "angular child degree",
                        child_degree,
                        self.limits.max_angular_degree,
                    )?;
                    record_count(
                        "angular recurrence transitions",
                        &mut self.angular_transition_count,
                        self.limits.max_angular_transitions,
                    )?;
                    let child_value = self.angular_moment(child)?;
                    let multiplicity = i64::try_from(multiplicity).map_err(|_| {
                        ProbeError::MultiplicityCoefficientOverflow { multiplicity }
                    })?;
                    let weighted = self.context.try_mul(
                        &self.context.integer(multiplicity),
                        &child_value,
                        exact_limits(),
                    )?;
                    numerator = self
                        .context
                        .try_add(&numerator, &weighted, exact_limits())?;
                }
                self.context
                    .try_div(&numerator, &denominator, exact_limits())?
            }
        };
        admit_new_cache_entry(
            "angular recurrence cache entries",
            self.angular_cache.len(),
            self.limits.max_angular_cache_entries,
        )?;
        self.angular_cache.insert(powers, result.clone());
        Ok(result)
    }

    fn select_incident_vector(&self, powers: &[u64; 3]) -> Result<usize, ProbeError> {
        let mut selected = None;
        for candidate in 0..LOOP_COUNT {
            if incidence(powers, candidate)? == 0 {
                continue;
            }
            if selected.is_none_or(|current| {
                self.vector_priority.rank_by_slot()[candidate]
                    < self.vector_priority.rank_by_slot()[current]
            }) {
                selected = Some(candidate);
            }
        }
        selected.ok_or(ProbeError::Invariant {
            detail: "a positive angular degree had no incident vector",
        })
    }

    fn select_partner(&self, powers: &[u64; 3], vector: usize) -> Result<usize, ProbeError> {
        let mut selected = None;
        for candidate in 0..LOOP_COUNT {
            if candidate == vector || edge_power(powers, vector, candidate)? == 0 {
                continue;
            }
            if selected.is_none_or(|current| {
                self.vector_priority.rank_by_slot()[candidate]
                    < self.vector_priority.rank_by_slot()[current]
            }) {
                selected = Some(candidate);
            }
        }
        selected.ok_or(ProbeError::Invariant {
            detail: "a vector with positive incidence had no angular partner",
        })
    }
}

fn edge_slot(left: usize, right: usize) -> Result<usize, ProbeError> {
    match (left.min(right), left.max(right)) {
        (0, 1) => Ok(0),
        (0, 2) => Ok(1),
        (1, 2) => Ok(2),
        _ => Err(ProbeError::Invariant {
            detail: "a three-vector edge needs distinct in-range endpoints",
        }),
    }
}

fn edge_power(powers: &[u64; 3], left: usize, right: usize) -> Result<u64, ProbeError> {
    Ok(powers[edge_slot(left, right)?])
}

fn decrement_edge(powers: &mut [u64; 3], left: usize, right: usize) -> Result<(), ProbeError> {
    let edge = edge_slot(left, right)?;
    powers[edge] = powers[edge]
        .checked_sub(1)
        .ok_or(ProbeError::DegreeOverflow {
            resource: "angular edge decrement",
        })?;
    Ok(())
}

fn increment_edge(powers: &mut [u64; 3], left: usize, right: usize) -> Result<(), ProbeError> {
    let edge = edge_slot(left, right)?;
    powers[edge] = powers[edge]
        .checked_add(1)
        .ok_or(ProbeError::DegreeOverflow {
            resource: "angular edge increment",
        })?;
    Ok(())
}

fn incidence(powers: &[u64; 3], vector: usize) -> Result<u64, ProbeError> {
    let mut degree = 0_u64;
    for other in 0..LOOP_COUNT {
        if other == vector {
            continue;
        }
        degree = degree
            .checked_add(edge_power(powers, vector, other)?)
            .ok_or(ProbeError::DegreeOverflow {
                resource: "angular vector incidence",
            })?;
    }
    Ok(degree)
}

/// Extract the inactive numerator degrees admitted by the corner fixture.
///
/// The target's active mask must match the authenticated factorization sector,
/// and every active denominator must be undotted (`power == 1`) before the
/// `q_i^2 = 1` radial simplification is allowed. Negative powers are carried
/// exactly in `u64` and rejected by the configured/hard degree ceilings before
/// recursive evaluation begins.
pub(super) fn numerator_powers(
    target: [i64; ARITY],
    rule: &FactorizationRule,
    limits: ProbeLimits,
) -> Result<[u64; ARITY], ProbeError> {
    let limits = limits.validate()?;
    let expected_active = rule.application_domain().sector().active_bits();
    if expected_active.len() != ARITY {
        return Err(ProbeError::WrongSectorArity {
            expected: ARITY,
            actual: expected_active.len(),
        });
    }
    let mut output = [0_u64; ARITY];
    for (slot, (power, &active)) in target.into_iter().zip(expected_active).enumerate() {
        if active {
            if power != 1 {
                return Err(ProbeError::NonCornerActivePower { slot, power });
            }
        } else if power > 0 {
            return Err(ProbeError::ForeignActivePower { slot, power });
        } else if power < 0 {
            output[slot] = power.unsigned_abs();
        }
    }
    let degree = checked_total("corner numerator degree", &output)?;
    admit_degree("corner numerator degree", degree, limits.max_affine_degree)?;
    Ok(output)
}
