use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::algebra::{
    Coefficient, CoefficientContext, coefficient_clone_owned_retained_byte_bound,
};
use crate::family::IntegralKey;
use crate::foundry::artifact::{ClosedArtifact, CommonMassHomogeneityProof};
use crate::foundry::parametric::ParametricRule;

use super::error::ReductionError;
use super::model::{
    HomogeneousMasterCoefficient, HomogeneousMasterDecomposition, MasterDecomposition,
    ReductionLimits, ReductionStatistics,
};

/// Stateful, topology-independent memoizing applier for one sealed artifact.
pub struct Reducer<'artifact> {
    artifact: &'artifact ClosedArtifact,
    limits: ReductionLimits,
    cache: BTreeMap<IntegralKey, MasterDecomposition>,
    cache_weight: CacheWeight,
    statistics: ReductionStatistics,
    family_fingerprint: Arc<String>,
}

impl<'artifact> Reducer<'artifact> {
    pub fn new(artifact: &'artifact ClosedArtifact) -> Result<Self, ReductionError> {
        Self::with_limits(artifact, ReductionLimits::default())
    }

    pub fn with_limits(
        artifact: &'artifact ClosedArtifact,
        limits: ReductionLimits,
    ) -> Result<Self, ReductionError> {
        let mut reducer = Self {
            artifact,
            limits,
            cache: BTreeMap::new(),
            cache_weight: CacheWeight::default(),
            statistics: ReductionStatistics::default(),
            family_fingerprint: artifact.family_fingerprint_owner(),
        };
        reducer.seed_master_terminals()?;
        Ok(reducer)
    }

    pub fn artifact(&self) -> &'artifact ClosedArtifact {
        self.artifact
    }

    pub fn limits(&self) -> ReductionLimits {
        self.limits
    }

    pub fn statistics(&self) -> ReductionStatistics {
        self.statistics
    }

    /// Drop all memoized nonterminal results while retaining the explicit
    /// artifact master terminals.
    pub fn clear_cache(&mut self) -> Result<(), ReductionError> {
        self.cache.clear();
        self.cache_weight = CacheWeight::default();
        self.refresh_cache_census();
        self.seed_master_terminals()
    }

    /// Reduce at the artifact's unit common mass.
    pub fn reduce_unit_mass(
        &mut self,
        target: &IntegralKey,
    ) -> Result<MasterDecomposition, ReductionError> {
        self.validate_target(target)?;
        if let Some(cached) = self.cache.get(target).cloned() {
            self.statistics.record_cache_hit();
            return Ok(cached);
        }

        let mut stack = Vec::new();
        self.push_frame(&mut stack, Frame::Expand(target.clone()))?;
        let mut active = BTreeSet::new();
        let mut call_rule_applications = 0usize;

        while let Some(frame) = stack.pop() {
            match frame {
                Frame::Expand(key) => {
                    if self.cache.contains_key(&key) {
                        self.statistics.record_cache_hit();
                        continue;
                    }
                    if self.artifact.is_zero_terminal(&key) {
                        self.cache_insert(
                            key.clone(),
                            MasterDecomposition::new(
                                self.family_fingerprint.clone(),
                                key,
                                BTreeMap::new(),
                            ),
                        )?;
                        continue;
                    }
                    begin_expansion(&mut active, &key)?;
                    let selected = self.select_first_rule(&key)?;
                    call_rule_applications = call_rule_applications.checked_add(1).ok_or(
                        ReductionError::RuleApplicationLimit {
                            requested: usize::MAX,
                            limit: self.limits.max_rule_applications,
                        },
                    )?;
                    if call_rule_applications > self.limits.max_rule_applications {
                        return Err(ReductionError::RuleApplicationLimit {
                            requested: call_rule_applications,
                            limit: self.limits.max_rule_applications,
                        });
                    }
                    let application = self.apply_selected_rule(&key, selected)?;
                    self.statistics.record_rule_application();
                    let mut children = Vec::new();
                    children.try_reserve_exact(application.len()).map_err(|_| {
                        ReductionError::AllocationFailure {
                            resource: "applied rule child schedule",
                            requested: application.len(),
                        }
                    })?;
                    children.extend(application.iter().map(|(child, _)| child.clone()));
                    self.push_frame(
                        &mut stack,
                        Frame::Combine {
                            target: key,
                            terms: application,
                        },
                    )?;
                    for child in children.into_iter().rev() {
                        if !self.cache.contains_key(&child) {
                            self.push_frame(&mut stack, Frame::Expand(child))?;
                        }
                    }
                }
                Frame::Combine { target, terms } => {
                    let mut masters = BTreeMap::new();
                    for (child, rule_coefficient) in terms {
                        let child_expansion =
                            self.cache
                                .get(&child)
                                .ok_or(ReductionError::ReducerInvariant {
                                    detail: "a child expansion is absent at its combine frame",
                                })?;
                        for (master, child_coefficient) in child_expansion.terms() {
                            let contribution = self.artifact.coefficient_context().try_mul(
                                &rule_coefficient,
                                child_coefficient,
                                self.limits.exact_algebra,
                            )?;
                            accumulate_master(
                                self.artifact.coefficient_context(),
                                &mut masters,
                                master,
                                contribution,
                                self.limits,
                            )?;
                        }
                    }
                    if !active.remove(&target) {
                        return Err(ReductionError::ReducerInvariant {
                            detail: "a combine frame is not present in the active dependency set",
                        });
                    }
                    self.cache_insert(
                        target.clone(),
                        MasterDecomposition::new(self.family_fingerprint.clone(), target, masters),
                    )?;
                }
            }
        }

        self.cache
            .get(target)
            .cloned()
            .ok_or(ReductionError::ReducerInvariant {
                detail: "the root expansion is absent after exhausting the work stack",
            })
    }

    /// Restore an arbitrary nonzero common mass squared inside the artifact's
    /// coefficient map.
    pub fn reduce_with_common_mass_squared(
        &mut self,
        target: &IntegralKey,
        mass_squared: &Coefficient,
    ) -> Result<MasterDecomposition, ReductionError> {
        let context = self.artifact.coefficient_context();
        context.validate_with_limits(mass_squared, self.limits.exact_algebra)?;
        if mass_squared.is_zero() {
            return Err(ReductionError::ZeroCommonMass);
        }
        let homogeneous = self.reduce_with_common_mass_homogeneity(target)?;
        let mut restored = BTreeMap::new();
        for (master, coefficient) in homogeneous.into_terms() {
            let mass_factor = pow_signed(
                context,
                mass_squared,
                coefficient.common_mass_squared_power(),
                self.limits,
            )?;
            let contribution = context.try_mul(
                coefficient.unit_mass_coefficient(),
                &mass_factor,
                self.limits.exact_algebra,
            )?;
            accumulate_master(context, &mut restored, &master, contribution, self.limits)?;
        }
        Ok(MasterDecomposition::new(
            self.family_fingerprint.clone(),
            target.clone(),
            restored,
        ))
    }

    /// Restore the common scale as explicit dimensional-homogeneity powers,
    /// allowing an adapter to materialize its own mass symbol later.
    pub fn reduce_with_common_mass_homogeneity(
        &mut self,
        target: &IntegralKey,
    ) -> Result<HomogeneousMasterDecomposition, ReductionError> {
        if self.artifact.common_mass_homogeneity()
            != Some(CommonMassHomogeneityProof::UniformVacuumMassSquared)
        {
            return Err(ReductionError::MissingCommonMassHomogeneityProof);
        }
        let unit = self.reduce_unit_mass(target)?;
        let mut terms = BTreeMap::new();
        for (master, coefficient) in unit.into_terms() {
            let exponent = homogeneity_exponent(target, &master)?;
            terms.insert(
                master,
                HomogeneousMasterCoefficient::new(coefficient, exponent),
            );
        }
        Ok(HomogeneousMasterDecomposition::new(
            self.family_fingerprint.clone(),
            target.clone(),
            terms,
        ))
    }

    fn validate_target(&self, target: &IntegralKey) -> Result<(), ReductionError> {
        if target.powers().len() == self.artifact.arity() {
            Ok(())
        } else {
            Err(ReductionError::WrongArity {
                expected: self.artifact.arity(),
                actual: target.powers().len(),
            })
        }
    }

    pub(super) fn select_first_rule(
        &self,
        target: &IntegralKey,
    ) -> Result<SelectedRule<'_>, ReductionError> {
        for rule in self.artifact.rules() {
            let mut assignment = Vec::new();
            assignment
                .try_reserve_exact(self.artifact.arity())
                .map_err(|_| ReductionError::AllocationFailure {
                    resource: "rule free-index assignment",
                    requested: self.artifact.arity(),
                })?;
            let mut representable = true;
            for (&target_power, &pivot) in target.powers().iter().zip(rule.pivot().values()) {
                let Some(value) = target_power.checked_sub(pivot) else {
                    representable = false;
                    break;
                };
                assignment.push(value);
            }
            if representable && rule.domain().contains(&assignment)? {
                return Ok(SelectedRule { rule, assignment });
            }
        }
        Err(ReductionError::UncoveredIntegral {
            target: target.clone(),
        })
    }

    fn apply_selected_rule(
        &self,
        target: &IntegralKey,
        selected: SelectedRule<'_>,
    ) -> Result<Vec<(IntegralKey, Coefficient)>, ReductionError> {
        let mut terms = Vec::new();
        terms
            .try_reserve_exact(selected.rule.right_hand_side().len())
            .map_err(|_| ReductionError::AllocationFailure {
                resource: "applied rule right-hand side",
                requested: selected.rule.right_hand_side().len(),
            })?;
        for term in selected.rule.right_hand_side() {
            let mut child_powers = Vec::new();
            child_powers
                .try_reserve_exact(self.artifact.arity())
                .map_err(|_| ReductionError::AllocationFailure {
                    resource: "applied rule child powers",
                    requested: self.artifact.arity(),
                })?;
            for (position, (&free_index, &shift)) in selected
                .assignment
                .iter()
                .zip(term.shift().values())
                .enumerate()
            {
                child_powers.push(
                    free_index
                        .checked_add(shift)
                        .ok_or(ReductionError::IndexOverflow { position })?,
                );
            }
            let child = IntegralKey::try_new(child_powers)?;
            selected
                .rule
                .ordering()
                .prove_strict_descent(target.powers(), child.powers())?;
            let (coefficient, denominator_guard) =
                self.artifact.indexed_context().specialize_sealed(
                    term.coefficient(),
                    &selected.assignment,
                    self.limits.indexed_algebra,
                )?;
            if denominator_guard.is_some() {
                return Err(ReductionError::UnexpectedCoefficientGuard);
            }
            if !coefficient.is_zero() {
                terms.push((child, coefficient));
            }
        }
        Ok(terms)
    }

    fn push_frame(&self, stack: &mut Vec<Frame>, frame: Frame) -> Result<(), ReductionError> {
        let requested = stack
            .len()
            .checked_add(1)
            .ok_or(ReductionError::AllocationFailure {
                resource: "reduction work frames",
                requested: usize::MAX,
            })?;
        if requested > self.limits.max_pending_frames {
            return Err(ReductionError::PendingFrameLimit {
                requested,
                limit: self.limits.max_pending_frames,
            });
        }
        stack
            .try_reserve(1)
            .map_err(|_| ReductionError::AllocationFailure {
                resource: "reduction work frames",
                requested,
            })?;
        stack.push(frame);
        Ok(())
    }

    fn cache_insert(
        &mut self,
        key: IntegralKey,
        value: MasterDecomposition,
    ) -> Result<(), ReductionError> {
        if !self.cache.contains_key(&key) {
            let requested = self
                .cache
                .len()
                .checked_add(1)
                .ok_or(ReductionError::CacheLimit {
                    requested: usize::MAX,
                    limit: self.limits.max_cached_integrals,
                })?;
            if requested > self.limits.max_cached_integrals {
                return Err(ReductionError::CacheLimit {
                    requested,
                    limit: self.limits.max_cached_integrals,
                });
            }
        }
        let previous_weight = self
            .cache
            .get(&key)
            .map(decomposition_cache_weight)
            .transpose()?
            .unwrap_or_default();
        let value_weight = decomposition_cache_weight(&value)?;
        let retained_without_previous = self.cache_weight.checked_sub(previous_weight)?;
        let prospective_weight = retained_without_previous.checked_add(value_weight)?;
        if prospective_weight.coefficient_terms > self.limits.max_cached_coefficient_terms {
            return Err(ReductionError::CacheCoefficientTermLimit {
                requested: prospective_weight.coefficient_terms,
                limit: self.limits.max_cached_coefficient_terms,
            });
        }
        if prospective_weight.coefficient_bytes > self.limits.max_cached_coefficient_bytes {
            return Err(ReductionError::CacheCoefficientByteLimit {
                requested: prospective_weight.coefficient_bytes,
                limit: self.limits.max_cached_coefficient_bytes,
            });
        }
        self.cache.insert(key, value);
        self.cache_weight = prospective_weight;
        self.refresh_cache_census();
        Ok(())
    }

    fn seed_master_terminals(&mut self) -> Result<(), ReductionError> {
        let mut masters = Vec::new();
        masters
            .try_reserve_exact(self.artifact.masters().len())
            .map_err(|_| ReductionError::AllocationFailure {
                resource: "master-terminal cache seeds",
                requested: self.artifact.masters().len(),
            })?;
        masters.extend(self.artifact.masters().iter().cloned());
        for master in masters {
            let mut terms = BTreeMap::new();
            terms.insert(master.clone(), self.artifact.coefficient_context().one());
            self.cache_insert(
                master.clone(),
                MasterDecomposition::new(self.family_fingerprint.clone(), master, terms),
            )?;
        }
        self.refresh_cache_census();
        Ok(())
    }

    fn refresh_cache_census(&mut self) {
        self.statistics.set_cache_census(
            self.cache.len(),
            self.cache_weight.coefficient_terms,
            self.cache_weight.coefficient_bytes,
        );
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct CacheWeight {
    coefficient_terms: usize,
    coefficient_bytes: usize,
}

impl CacheWeight {
    fn checked_add(self, other: Self) -> Result<Self, ReductionError> {
        Ok(Self {
            coefficient_terms: self
                .coefficient_terms
                .checked_add(other.coefficient_terms)
                .ok_or(ReductionError::CacheResourceCountOverflow {
                    resource: "retained coefficient terms",
                })?,
            coefficient_bytes: self
                .coefficient_bytes
                .checked_add(other.coefficient_bytes)
                .ok_or(ReductionError::CacheResourceCountOverflow {
                    resource: "retained coefficient bytes",
                })?,
        })
    }

    fn checked_sub(self, other: Self) -> Result<Self, ReductionError> {
        Ok(Self {
            coefficient_terms: self
                .coefficient_terms
                .checked_sub(other.coefficient_terms)
                .ok_or(ReductionError::ReducerInvariant {
                    detail: "cached coefficient-term census underflowed",
                })?,
            coefficient_bytes: self
                .coefficient_bytes
                .checked_sub(other.coefficient_bytes)
                .ok_or(ReductionError::ReducerInvariant {
                    detail: "cached coefficient-byte census underflowed",
                })?,
        })
    }
}

fn decomposition_cache_weight(
    decomposition: &MasterDecomposition,
) -> Result<CacheWeight, ReductionError> {
    decomposition
        .terms()
        .values()
        .try_fold(CacheWeight::default(), |weight, coefficient| {
            let coefficient_terms = coefficient
                .numerator
                .nterms()
                .checked_add(coefficient.denominator.nterms())
                .ok_or(ReductionError::CacheResourceCountOverflow {
                    resource: "retained coefficient terms",
                })?;
            let coefficient_bytes = coefficient_clone_owned_retained_byte_bound(coefficient)
                .ok_or(ReductionError::CacheResourceCountOverflow {
                    resource: "retained coefficient bytes",
                })?;
            weight.checked_add(CacheWeight {
                coefficient_terms,
                coefficient_bytes,
            })
        })
}

#[derive(Debug)]
enum Frame {
    Expand(IntegralKey),
    Combine {
        target: IntegralKey,
        terms: Vec<(IntegralKey, Coefficient)>,
    },
}

pub(super) struct SelectedRule<'rule> {
    pub(super) rule: &'rule ParametricRule,
    assignment: Vec<i64>,
}

pub(super) fn begin_expansion(
    active: &mut BTreeSet<IntegralKey>,
    target: &IntegralKey,
) -> Result<(), ReductionError> {
    if active.insert(target.clone()) {
        Ok(())
    } else {
        Err(ReductionError::CycleDetected {
            target: target.clone(),
        })
    }
}

pub(super) fn accumulate_master(
    context: &CoefficientContext,
    terms: &mut BTreeMap<IntegralKey, Coefficient>,
    master: &IntegralKey,
    contribution: Coefficient,
    limits: ReductionLimits,
) -> Result<(), ReductionError> {
    if contribution.is_zero() {
        return Ok(());
    }
    if let std::collections::btree_map::Entry::Occupied(mut entry) = terms.entry(master.clone()) {
        let sum = context.try_add(entry.get(), &contribution, limits.exact_algebra)?;
        if sum.is_zero() {
            entry.remove();
        } else {
            *entry.get_mut() = sum;
        }
    } else {
        terms.insert(master.clone(), contribution);
    }
    Ok(())
}

fn homogeneity_exponent(
    target: &IntegralKey,
    master: &IntegralKey,
) -> Result<i128, ReductionError> {
    if target.powers().len() != master.powers().len() {
        return Err(ReductionError::WrongArity {
            expected: target.powers().len(),
            actual: master.powers().len(),
        });
    }
    let target_sum = target.powers().iter().try_fold(0_i128, |sum, &power| {
        sum.checked_add(i128::from(power))
            .ok_or(ReductionError::CommonMassPowerOverflow)
    })?;
    let master_sum = master.powers().iter().try_fold(0_i128, |sum, &power| {
        sum.checked_add(i128::from(power))
            .ok_or(ReductionError::CommonMassPowerOverflow)
    })?;
    master_sum
        .checked_sub(target_sum)
        .ok_or(ReductionError::CommonMassPowerOverflow)
}

fn pow_signed(
    context: &CoefficientContext,
    value: &Coefficient,
    exponent: i128,
    limits: ReductionLimits,
) -> Result<Coefficient, ReductionError> {
    let magnitude = exponent.unsigned_abs();
    let mut remaining =
        u64::try_from(magnitude).map_err(|_| ReductionError::CommonMassPowerOverflow)?;
    let mut result = context.one();
    let mut base = value.clone();
    while remaining != 0 {
        if remaining & 1 == 1 {
            result = context.try_mul(&result, &base, limits.exact_algebra)?;
        }
        remaining >>= 1;
        if remaining != 0 {
            base = context.try_mul(&base, &base, limits.exact_algebra)?;
        }
    }
    if exponent < 0 {
        Ok(context.try_div(&context.one(), &result, limits.exact_algebra)?)
    } else {
        Ok(result)
    }
}
