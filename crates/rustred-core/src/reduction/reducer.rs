use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use crate::algebra::{
    Coefficient, CoefficientContext, coefficient_clone_owned_retained_byte_bound,
};
use crate::family::IntegralKey;
use crate::foundry::artifact::{ClosedArtifact, CommonMassHomogeneityProof};
use crate::foundry::cell::RuleCellTerm;
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
    shared_cache: Arc<SharedCacheBudget>,
    statistics: ReductionStatistics,
    family_fingerprint: Arc<String>,
    dependency_reducers: Vec<Reducer<'artifact>>,
}

impl<'artifact> Reducer<'artifact> {
    pub fn new(artifact: &'artifact ClosedArtifact) -> Result<Self, ReductionError> {
        Self::with_limits(artifact, ReductionLimits::default())
    }

    pub fn with_limits(
        artifact: &'artifact ClosedArtifact,
        limits: ReductionLimits,
    ) -> Result<Self, ReductionError> {
        Self::build(artifact, limits, Arc::new(SharedCacheBudget::default()))
    }

    #[cfg(test)]
    pub(crate) fn with_shared_cache(
        artifact: &'artifact ClosedArtifact,
        limits: ReductionLimits,
        shared_cache: Arc<SharedCacheBudget>,
    ) -> Result<Self, ReductionError> {
        Self::build(artifact, limits, shared_cache)
    }

    fn build(
        artifact: &'artifact ClosedArtifact,
        limits: ReductionLimits,
        shared_cache: Arc<SharedCacheBudget>,
    ) -> Result<Self, ReductionError> {
        let mut dependency_reducers = Vec::new();
        dependency_reducers
            .try_reserve_exact(artifact.dependencies().len())
            .map_err(|_| ReductionError::AllocationFailure {
                resource: "lower-artifact reducers",
                requested: artifact.dependencies().len(),
            })?;
        for dependency in artifact.dependencies() {
            dependency_reducers.push(Self::build(
                dependency.as_ref(),
                limits,
                shared_cache.clone(),
            )?);
        }
        let mut reducer = Self {
            artifact,
            limits,
            cache: BTreeMap::new(),
            cache_weight: CacheWeight::default(),
            shared_cache,
            statistics: ReductionStatistics::default(),
            family_fingerprint: artifact.family_fingerprint_owner(),
            dependency_reducers,
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
        let mut aggregate = ReductionStatistics::default();
        self.merge_work_statistics(&mut aggregate);
        let census = self.shared_cache.snapshot();
        aggregate.set_cache_census(
            census.integrals,
            census.coefficient_terms,
            census.coefficient_bytes,
        );
        aggregate
    }

    /// Drop all memoized nonterminal results while retaining the explicit
    /// artifact master terminals.
    pub fn clear_cache(&mut self) -> Result<(), ReductionError> {
        for dependency in &mut self.dependency_reducers {
            dependency.clear_cache()?;
        }
        self.shared_cache.replace(
            self.cache.len(),
            self.cache_weight,
            0,
            CacheWeight::default(),
            self.limits,
        )?;
        self.cache.clear();
        self.cache_weight = CacheWeight::default();
        self.seed_master_terminals()
    }

    /// Reduce at the artifact's unit common mass.
    pub fn reduce_unit_mass(
        &mut self,
        target: &IntegralKey,
    ) -> Result<MasterDecomposition, ReductionError> {
        let mut request = ReductionRequest::default();
        self.reduce_unit_mass_in_request(target, &mut request)
    }

    pub(crate) fn reduce_unit_mass_in_request(
        &mut self,
        target: &IntegralKey,
        request: &mut ReductionRequest,
    ) -> Result<MasterDecomposition, ReductionError> {
        self.validate_target(target)?;
        let canonical_target = self.canonicalize(target)?;
        let canonical = self.reduce_canonical_unit_mass(&canonical_target, request)?;
        if canonical_target == *target {
            Ok(canonical)
        } else {
            Ok(MasterDecomposition::new(
                self.family_fingerprint.clone(),
                target.clone(),
                canonical.into_terms(),
            ))
        }
    }

    fn reduce_canonical_unit_mass(
        &mut self,
        target: &IntegralKey,
        request: &mut ReductionRequest,
    ) -> Result<MasterDecomposition, ReductionError> {
        if let Some(cached) = self.cache.get(target).cloned() {
            self.statistics.record_cache_hit();
            return Ok(cached);
        }

        let pending_baseline = request.pending_frame_count();
        let result = (|| {
            let mut stack = Vec::new();
            self.push_frame(&mut stack, Frame::Expand(target.clone()), request)?;
            let mut active = BTreeSet::new();
            while let Some(frame) = stack.pop() {
                request.release_pending_frame()?;
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
                        if let Some(factorization) = self.select_first_factorization(&key)? {
                            request.record_rule_application(self.limits.max_rule_applications)?;
                            let routed = self.apply_factorization(
                                &factorization.routed_target,
                                factorization.ordinal,
                                request,
                            )?;
                            let expansion = MasterDecomposition::new(
                                self.family_fingerprint.clone(),
                                key.clone(),
                                routed.into_terms(),
                            );
                            self.statistics.record_rule_application();
                            self.cache_insert(key, expansion)?;
                            continue;
                        }
                        let selected = self.select_first_rule(&key)?;
                        begin_expansion(&mut active, &key)?;
                        request.record_rule_application(self.limits.max_rule_applications)?;
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
                            request,
                        )?;
                        for child in children.into_iter().rev() {
                            if !self.cache.contains_key(&child) {
                                self.push_frame(&mut stack, Frame::Expand(child), request)?;
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
                                accumulate_master_in_request(
                                    self.artifact.coefficient_context(),
                                    &mut masters,
                                    master,
                                    contribution,
                                    self.limits,
                                    request,
                                    &mut self.statistics,
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
                            MasterDecomposition::new(
                                self.family_fingerprint.clone(),
                                target,
                                masters,
                            ),
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
        })();
        request.restore_pending_frames(pending_baseline);
        result
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
        if target.powers().len() != self.artifact.arity() {
            return Err(ReductionError::WrongArity {
                expected: self.artifact.arity(),
                actual: target.powers().len(),
            });
        }
        for (position, (&value, bounds)) in target
            .powers()
            .iter()
            .zip(self.artifact.supported_root_power_bounds())
            .enumerate()
        {
            if !bounds.contains(value) {
                return Err(ReductionError::OutsideCertifiedRootDomain {
                    position,
                    value,
                    lower: bounds.lower(),
                    upper: bounds.upper(),
                });
            }
        }
        Ok(())
    }

    pub(super) fn select_first_rule(
        &self,
        target: &IntegralKey,
    ) -> Result<SelectedRule<'_>, ReductionError> {
        for cell_owner in self.artifact.rule_cells() {
            let cell = cell_owner.as_ref();
            let Some(assignment) = cell.assignment_for_target(target)? else {
                continue;
            };
            let mut applicable = true;
            for guard in cell.guards() {
                let specialized = self
                    .artifact
                    .indexed_context()
                    .specialize_polynomial_sealed(
                        guard.polynomial(),
                        &assignment,
                        self.limits.indexed_algebra,
                    )?;
                if specialized.is_zero() {
                    applicable = false;
                    break;
                }
            }
            if applicable {
                return Ok(SelectedRule {
                    rule: cell.rule(),
                    assignment,
                    retained_terms: Some(cell.terms()),
                });
            }
        }
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
            if !representable || !rule.domain().contains(&assignment)? {
                continue;
            }
            let mut applicable = true;
            for guard in rule.nonzero_guards() {
                let specialized = self
                    .artifact
                    .indexed_context()
                    .specialize_polynomial_sealed(
                        guard.polynomial(),
                        &assignment,
                        self.limits.indexed_algebra,
                    )?;
                if specialized.is_zero() {
                    applicable = false;
                    break;
                }
            }
            if applicable {
                return Ok(SelectedRule {
                    rule,
                    assignment,
                    retained_terms: None,
                });
            }
        }
        Err(ReductionError::UncoveredIntegral {
            target: target.clone(),
        })
    }

    fn select_first_factorization(
        &self,
        target: &IntegralKey,
    ) -> Result<Option<SelectedFactorization>, ReductionError> {
        let orbit = self
            .artifact
            .canonicalizer()
            .map(|canonicalizer| canonicalizer.orbit(target))
            .transpose()?;
        for ordinal in 0..self.artifact.factorization_rules().len() {
            let rule = self.artifact.factorization_rules().get(ordinal).ok_or(
                ReductionError::ReducerInvariant {
                    detail: "a sealed factorization has no executable domain",
                },
            )?;
            let program = self
                .artifact
                .factorized_product_programs()
                .get(ordinal)
                .and_then(Option::as_ref);
            let test = |candidate: &IntegralKey| -> Result<bool, ReductionError> {
                match program {
                    Some(program) => Ok(program.contains(candidate.powers())?),
                    None => Ok(rule.application_domain().contains(candidate.powers())?),
                }
            };
            if let Some(orbit) = &orbit {
                // Images are already sorted by the installed exact ordering.
                // Selecting the first safe image is deterministic for the
                // entire orbit, while the outer ordinal loop preserves sealed
                // factorization-owner precedence.
                for image in orbit.images() {
                    if test(image.integral())? {
                        return Ok(Some(SelectedFactorization {
                            ordinal,
                            routed_target: image.integral().clone(),
                        }));
                    }
                }
            } else if test(target)? {
                return Ok(Some(SelectedFactorization {
                    ordinal,
                    routed_target: target.clone(),
                }));
            }
        }
        Ok(None)
    }

    fn apply_factorization(
        &mut self,
        target: &IntegralKey,
        factorization_ordinal: usize,
        request: &mut ReductionRequest,
    ) -> Result<MasterDecomposition, ReductionError> {
        let rule = self
            .artifact
            .factorization_rules()
            .get(factorization_ordinal)
            .ok_or(ReductionError::ReducerInvariant {
                detail: "a selected factorization ordinal is absent",
            })?;
        let masters = if let Some(program) = self
            .artifact
            .factorized_product_programs()
            .get(factorization_ordinal)
            .and_then(Option::as_ref)
        {
            program.reduce_parent(
                self.artifact.family(),
                rule,
                self.artifact.dependencies(),
                &mut self.dependency_reducers,
                target,
                request,
                &mut self.statistics,
                Arc::clone(&self.shared_cache),
                self.limits,
            )?
        } else {
            self.apply_corner_factorization(target, rule, request)?
        };
        Ok(MasterDecomposition::new(
            self.family_fingerprint.clone(),
            target.clone(),
            masters,
        ))
    }

    /// Generic zero-inactive-power product lane retained for authenticated
    /// factorization shapes that do not admit the complete numerator-moment
    /// compiler yet.
    fn apply_corner_factorization(
        &mut self,
        target: &IntegralKey,
        rule: &crate::foundry::artifact::FactorizationRule,
        request: &mut ReductionRequest,
    ) -> Result<BTreeMap<IntegralKey, Coefficient>, ReductionError> {
        let context = self.artifact.coefficient_context();
        let mut products = BTreeMap::new();
        products.insert(
            IntegralKey::try_new(std::iter::repeat_n(0_i64, self.artifact.arity()))?,
            rule.normalization().clone(),
        );
        for factor in rule.factors() {
            let mut powers = Vec::new();
            powers
                .try_reserve_exact(factor.parent_positions().len())
                .map_err(|_| ReductionError::AllocationFailure {
                    resource: "factorized lower-integral powers",
                    requested: factor.parent_positions().len(),
                })?;
            for &position in factor.parent_positions() {
                powers.push(*target.powers().get(position).ok_or(
                    ReductionError::ReducerInvariant {
                        detail: "a sealed factorization projection is out of range",
                    },
                )?);
            }
            let dependency_target = IntegralKey::try_new(powers)?;
            let dependency = self
                .dependency_reducers
                .get_mut(factor.dependency_ordinal())
                .ok_or(ReductionError::ReducerInvariant {
                    detail: "a sealed factorization dependency is absent",
                })?;
            let expansion = dependency.reduce_unit_mass_in_request(&dependency_target, request)?;
            products = convolve_factor_expansion_in_request(
                context,
                &products,
                expansion.terms(),
                factor.parent_positions(),
                self.artifact.arity(),
                self.limits,
                request,
                &mut self.statistics,
            )?;
        }
        let mut masters = BTreeMap::new();
        for (raw_master, coefficient) in products {
            let master = rule.parent_terminal_for(&raw_master).ok_or(
                ReductionError::ReducerInvariant {
                    detail: "a sealed factorization produced an unauthenticated parent-master product",
                },
            )?;
            accumulate_master_in_request(
                context,
                &mut masters,
                master,
                coefficient,
                self.limits,
                request,
                &mut self.statistics,
            )?;
        }
        Ok(masters)
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
        let retained_ordinals = selected.retained_terms.map(|terms| {
            terms
                .iter()
                .map(RuleCellTerm::source_rhs_ordinal)
                .collect::<BTreeSet<_>>()
        });
        for (ordinal, term) in selected.rule.right_hand_side().iter().enumerate() {
            if retained_ordinals
                .as_ref()
                .is_some_and(|retained| !retained.contains(&ordinal))
            {
                continue;
            }
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
            let raw_child = IntegralKey::try_new(child_powers)?;
            selected
                .rule
                .ordering()
                .prove_strict_descent(target.powers(), raw_child.powers())?;
            let child = if let Some(canonicalizer) = self.artifact.canonicalizer() {
                canonicalizer
                    .canonicalize_descending_child(target, &raw_child)?
                    .into_child()
                    .canonical()
                    .clone()
            } else {
                raw_child
            };
            let (coefficient, _formal_parameter_denominator) =
                self.artifact.indexed_context().specialize_sealed(
                    term.coefficient(),
                    &selected.assignment,
                    self.limits.indexed_algebra,
                )?;
            if !coefficient.is_zero() {
                terms.push((child, coefficient));
            }
        }
        Ok(terms)
    }

    fn canonicalize(&self, target: &IntegralKey) -> Result<IntegralKey, ReductionError> {
        match self.artifact.canonicalizer() {
            Some(canonicalizer) => Ok(canonicalizer.canonicalize(target)?.canonical().clone()),
            None => Ok(target.clone()),
        }
    }

    fn push_frame(
        &self,
        stack: &mut Vec<Frame>,
        frame: Frame,
        request: &mut ReductionRequest,
    ) -> Result<(), ReductionError> {
        let requested = stack
            .len()
            .checked_add(1)
            .ok_or(ReductionError::AllocationFailure {
                resource: "reduction work frames",
                requested: usize::MAX,
            })?;
        stack
            .try_reserve(1)
            .map_err(|_| ReductionError::AllocationFailure {
                resource: "reduction work frames",
                requested,
            })?;
        request.retain_pending_frame(self.limits.max_pending_frames)?;
        stack.push(frame);
        Ok(())
    }

    fn cache_insert(
        &mut self,
        key: IntegralKey,
        value: MasterDecomposition,
    ) -> Result<(), ReductionError> {
        let previous_count = usize::from(self.cache.contains_key(&key));
        let previous_weight = self
            .cache
            .get(&key)
            .map(decomposition_cache_weight)
            .transpose()?
            .unwrap_or_default();
        let value_weight = decomposition_cache_weight(&value)?;
        let retained_without_previous = self.cache_weight.checked_sub(previous_weight)?;
        let prospective_weight = retained_without_previous.checked_add(value_weight)?;
        self.shared_cache.replace(
            previous_count,
            previous_weight,
            1,
            value_weight,
            self.limits,
        )?;
        self.cache.insert(key, value);
        self.cache_weight = prospective_weight;
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
        Ok(())
    }

    fn merge_work_statistics(&self, aggregate: &mut ReductionStatistics) {
        aggregate.merge_work(self.statistics);
        for dependency in &self.dependency_reducers {
            dependency.merge_work_statistics(aggregate);
        }
    }
}

/// Deterministically convolve one complete dependency-master expansion into
/// disjoint parent positions. Kept separate from dependency scheduling so the
/// generic multi-master product algebra has direct tests.
#[cfg(test)]
pub(crate) fn convolve_factor_expansion(
    context: &CoefficientContext,
    products: &BTreeMap<IntegralKey, Coefficient>,
    dependency_terms: &BTreeMap<IntegralKey, Coefficient>,
    parent_positions: &[usize],
    parent_arity: usize,
    limits: ReductionLimits,
) -> Result<BTreeMap<IntegralKey, Coefficient>, ReductionError> {
    let mut request = ReductionRequest::default();
    let mut statistics = ReductionStatistics::default();
    convolve_factor_expansion_in_request(
        context,
        products,
        dependency_terms,
        parent_positions,
        parent_arity,
        limits,
        &mut request,
        &mut statistics,
    )
}

/// Request-accounted variant used by every live factorized reduction. An
/// occupied output entry is charged before exact addition, including the
/// rejected one-beyond-limit attempt.
#[allow(clippy::too_many_arguments)]
pub(crate) fn convolve_factor_expansion_in_request(
    context: &CoefficientContext,
    products: &BTreeMap<IntegralKey, Coefficient>,
    dependency_terms: &BTreeMap<IntegralKey, Coefficient>,
    parent_positions: &[usize],
    parent_arity: usize,
    limits: ReductionLimits,
    request: &mut ReductionRequest,
    statistics: &mut ReductionStatistics,
) -> Result<BTreeMap<IntegralKey, Coefficient>, ReductionError> {
    let requested = products.len().checked_mul(dependency_terms.len()).ok_or(
        ReductionError::FactorizationTermLimit {
            requested: usize::MAX,
            limit: limits.max_factorization_terms,
        },
    )?;
    if requested > limits.max_factorization_terms {
        return Err(ReductionError::FactorizationTermLimit {
            requested,
            limit: limits.max_factorization_terms,
        });
    }
    let mut next_products = BTreeMap::new();
    for (partial_master, partial_coefficient) in products {
        if partial_master.powers().len() != parent_arity {
            return Err(ReductionError::ReducerInvariant {
                detail: "a partial factorization master has foreign parent arity",
            });
        }
        for (dependency_master, dependency_coefficient) in dependency_terms {
            if dependency_master.powers().len() != parent_positions.len() {
                return Err(ReductionError::ReducerInvariant {
                    detail: "a dependency master has foreign factorization arity",
                });
            }
            let mut parent_powers = partial_master.powers().to_vec();
            for (&parent_position, &power) in
                parent_positions.iter().zip(dependency_master.powers())
            {
                *parent_powers.get_mut(parent_position).ok_or(
                    ReductionError::ReducerInvariant {
                        detail: "a sealed factorization master embedding is out of range",
                    },
                )? = power;
            }
            let parent_master = IntegralKey::try_new(parent_powers)?;
            let coefficient = context.try_mul(
                partial_coefficient,
                dependency_coefficient,
                limits.exact_algebra,
            )?;
            accumulate_master_in_request(
                context,
                &mut next_products,
                &parent_master,
                coefficient,
                limits,
                request,
                statistics,
            )?;
        }
    }
    Ok(next_products)
}

#[derive(Default)]
pub(crate) struct ReductionRequest {
    rule_applications: usize,
    coalescing_additions: usize,
    pending_frames: usize,
}

impl ReductionRequest {
    pub(crate) fn record_rule_application(&mut self, limit: usize) -> Result<(), ReductionError> {
        self.record_rule_applications(1, limit)
    }

    pub(crate) fn record_rule_applications(
        &mut self,
        count: usize,
        limit: usize,
    ) -> Result<(), ReductionError> {
        self.rule_applications = self.rule_applications.checked_add(count).ok_or(
            ReductionError::RuleApplicationLimit {
                requested: usize::MAX,
                limit,
            },
        )?;
        if self.rule_applications > limit {
            return Err(ReductionError::RuleApplicationLimit {
                requested: self.rule_applications,
                limit,
            });
        }
        Ok(())
    }

    pub(crate) fn remaining_rule_applications(&self, limit: usize) -> usize {
        limit.saturating_sub(self.rule_applications)
    }

    pub(crate) fn record_coalescing_additions(
        &mut self,
        count: usize,
        limit: usize,
    ) -> Result<(), ReductionError> {
        self.coalescing_additions = self.coalescing_additions.checked_add(count).ok_or(
            ReductionError::CoalescingAdditionLimit {
                requested: usize::MAX,
                limit,
            },
        )?;
        if self.coalescing_additions > limit {
            return Err(ReductionError::CoalescingAdditionLimit {
                requested: self.coalescing_additions,
                limit,
            });
        }
        Ok(())
    }

    pub(crate) fn remaining_coalescing_additions(&self, limit: usize) -> usize {
        limit.saturating_sub(self.coalescing_additions)
    }

    pub(crate) const fn pending_frame_count(&self) -> usize {
        self.pending_frames
    }

    pub(crate) fn remaining_pending_frames(&self, limit: usize) -> usize {
        limit.saturating_sub(self.pending_frames)
    }

    pub(crate) fn retain_pending_frame(&mut self, limit: usize) -> Result<(), ReductionError> {
        self.pending_frames =
            self.pending_frames
                .checked_add(1)
                .ok_or(ReductionError::PendingFrameLimit {
                    requested: usize::MAX,
                    limit,
                })?;
        if self.pending_frames > limit {
            return Err(ReductionError::PendingFrameLimit {
                requested: self.pending_frames,
                limit,
            });
        }
        Ok(())
    }

    pub(crate) fn release_pending_frame(&mut self) -> Result<(), ReductionError> {
        self.pending_frames =
            self.pending_frames
                .checked_sub(1)
                .ok_or(ReductionError::ReducerInvariant {
                    detail: "the shared pending-frame census underflowed",
                })?;
        Ok(())
    }

    pub(crate) fn restore_pending_frames(&mut self, baseline: usize) {
        debug_assert!(self.pending_frames >= baseline);
        self.pending_frames = baseline;
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct CacheWeight {
    pub(crate) coefficient_terms: usize,
    pub(crate) coefficient_bytes: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct CacheCensus {
    pub(crate) integrals: usize,
    pub(crate) coefficient_terms: usize,
    pub(crate) coefficient_bytes: usize,
}

#[derive(Debug, Default)]
pub(crate) struct SharedCacheBudget {
    census: Mutex<CacheCensus>,
}

impl SharedCacheBudget {
    pub(crate) fn snapshot(&self) -> CacheCensus {
        *self
            .census
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub(crate) fn replace(
        &self,
        old_integrals: usize,
        old_weight: CacheWeight,
        new_integrals: usize,
        new_weight: CacheWeight,
        limits: ReductionLimits,
    ) -> Result<(), ReductionError> {
        let mut census = self
            .census
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let integrals = census
            .integrals
            .checked_sub(old_integrals)
            .and_then(|value| value.checked_add(new_integrals))
            .ok_or(ReductionError::CacheResourceCountOverflow {
                resource: "retained integrals",
            })?;
        let coefficient_terms = census
            .coefficient_terms
            .checked_sub(old_weight.coefficient_terms)
            .and_then(|value| value.checked_add(new_weight.coefficient_terms))
            .ok_or(ReductionError::CacheResourceCountOverflow {
                resource: "retained coefficient terms",
            })?;
        let coefficient_bytes = census
            .coefficient_bytes
            .checked_sub(old_weight.coefficient_bytes)
            .and_then(|value| value.checked_add(new_weight.coefficient_bytes))
            .ok_or(ReductionError::CacheResourceCountOverflow {
                resource: "retained coefficient bytes",
            })?;
        if integrals > limits.max_cached_integrals {
            return Err(ReductionError::CacheLimit {
                requested: integrals,
                limit: limits.max_cached_integrals,
            });
        }
        if coefficient_terms > limits.max_cached_coefficient_terms {
            return Err(ReductionError::CacheCoefficientTermLimit {
                requested: coefficient_terms,
                limit: limits.max_cached_coefficient_terms,
            });
        }
        if coefficient_bytes > limits.max_cached_coefficient_bytes {
            return Err(ReductionError::CacheCoefficientByteLimit {
                requested: coefficient_bytes,
                limit: limits.max_cached_coefficient_bytes,
            });
        }
        *census = CacheCensus {
            integrals,
            coefficient_terms,
            coefficient_bytes,
        };
        Ok(())
    }
}

impl CacheWeight {
    pub(crate) fn checked_add(self, other: Self) -> Result<Self, ReductionError> {
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

    pub(crate) fn checked_sub(self, other: Self) -> Result<Self, ReductionError> {
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
    coefficient_cache_weight(decomposition.terms().values())
}

pub(crate) fn coefficient_cache_weight<'a>(
    coefficients: impl IntoIterator<Item = &'a Coefficient>,
) -> Result<CacheWeight, ReductionError> {
    coefficients
        .into_iter()
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

struct SelectedFactorization {
    ordinal: usize,
    /// Exact symmetry image admitted by the selected executor-safe product
    /// domain. The cache key remains the reducer's ordinary canonical target.
    routed_target: IntegralKey,
}

pub(super) struct SelectedRule<'rule> {
    pub(super) rule: &'rule ParametricRule,
    assignment: Vec<i64>,
    retained_terms: Option<&'rule [RuleCellTerm]>,
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

pub(crate) fn accumulate_master(
    context: &CoefficientContext,
    terms: &mut BTreeMap<IntegralKey, Coefficient>,
    master: &IntegralKey,
    contribution: Coefficient,
    limits: ReductionLimits,
) -> Result<(), ReductionError> {
    accumulate_master_with(context, terms, master, contribution, limits, || Ok(()))
}

/// Merge one exact master coefficient while charging the aggregate request
/// before any occupied-entry algebra. Statistics intentionally record a
/// rejected attempt as work performed, matching rule and angular accounting.
pub(crate) fn accumulate_master_in_request(
    context: &CoefficientContext,
    terms: &mut BTreeMap<IntegralKey, Coefficient>,
    master: &IntegralKey,
    contribution: Coefficient,
    limits: ReductionLimits,
    request: &mut ReductionRequest,
    statistics: &mut ReductionStatistics,
) -> Result<(), ReductionError> {
    accumulate_master_with(context, terms, master, contribution, limits, || {
        statistics.record_coalescing_additions(1);
        request.record_coalescing_additions(1, limits.max_coalescing_additions)
    })
}

fn accumulate_master_with(
    context: &CoefficientContext,
    terms: &mut BTreeMap<IntegralKey, Coefficient>,
    master: &IntegralKey,
    contribution: Coefficient,
    limits: ReductionLimits,
    mut before_coalescing: impl FnMut() -> Result<(), ReductionError>,
) -> Result<(), ReductionError> {
    if contribution.is_zero() {
        return Ok(());
    }
    if let std::collections::btree_map::Entry::Occupied(mut entry) = terms.entry(master.clone()) {
        before_coalescing()?;
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
