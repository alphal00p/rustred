//! Streaming execution of cold-compiled executor-safe product programs.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::algebra::{Coefficient, CoefficientContext};
use crate::family::{IntegralFamily, IntegralKey, ScalarProductCoordinate};
use crate::foundry::artifact::{ClosedArtifact, FactorizationRule};
use crate::reduction::{
    CacheWeight, Reducer, ReductionError, ReductionLimits, ReductionRequest, ReductionStatistics,
    SharedCacheBudget, accumulate_master_in_request, coefficient_cache_weight,
    convolve_factor_expansion_in_request,
};

use super::angular::{AngularEvaluator, cross_radial_powers};
use super::error::FactorizedProductMomentError;
use super::limits::FactorizedProductMomentLimits;
use super::model::{
    CorrelatedProductBlock, FactorizedProductMomentProgram, MomentPower, ProductBlockLayout,
    ProductMomentVariable, SingletonProductBlock,
};
use super::partial_angular::{PartialAngularEvaluator, PartialMomentKey};
use super::resources::CoefficientBudget;

type Terms = BTreeMap<IntegralKey, Coefficient>;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct NumeratorState {
    remaining: Box<[MomentPower]>,
    radial: Box<[MomentPower]>,
    cross: Box<[MomentPower]>,
}

impl NumeratorState {
    fn measure(&self) -> Result<MomentPower, ReductionError> {
        sum_moment_powers(&self.remaining, "factorized numerator descent measure")
    }
}

impl CorrelatedState {
    fn measure(&self) -> Result<MomentPower, ReductionError> {
        sum_moment_powers(&self.moment_powers, "correlated moment descent measure")
    }
}

enum NumeratorFrame {
    Expand(NumeratorState),
    Combine {
        state: NumeratorState,
        children: Vec<(NumeratorState, Coefficient)>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct RadialState {
    dependency_ordinal: usize,
    denominator_power: i64,
    radial_power: MomentPower,
}

enum RadialFrame {
    Expand(RadialState),
    Combine {
        state: RadialState,
        children: [RadialState; 2],
    },
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct CorrelatedState {
    denominator_powers: Box<[i64]>,
    moment_powers: Box<[MomentPower]>,
}

enum CorrelatedFrame {
    Expand(CorrelatedState),
    Combine {
        state: CorrelatedState,
        children: Vec<(CorrelatedState, Coefficient)>,
    },
}

struct ProductExecution<'program, 'artifact, 'request> {
    program: &'program FactorizedProductMomentProgram,
    family: &'program IntegralFamily,
    rule: &'program FactorizationRule,
    dependencies: &'program [Box<ClosedArtifact>],
    dependency_reducers: &'request mut [Reducer<'artifact>],
    request: &'request mut ReductionRequest,
    statistics: &'request mut ReductionStatistics,
    limits: ReductionLimits,
    active_powers: Box<[i64]>,
    radial_cache: BTreeMap<RadialState, Terms>,
    correlated_cache: BTreeMap<CorrelatedState, Terms>,
    cache_budget: ProductCacheBudget,
}

#[derive(Debug)]
struct ProductCacheBudget {
    shared: Arc<SharedCacheBudget>,
    limits: ReductionLimits,
    states: usize,
    weight: CacheWeight,
}

impl FactorizedProductMomentProgram {
    /// Execute this terminalizing program through the parent reducer's
    /// already-instantiated dependency reducers and shared request budget.
    pub(crate) fn reduce_parent<'artifact>(
        &self,
        family: &IntegralFamily,
        rule: &FactorizationRule,
        dependencies: &[Box<ClosedArtifact>],
        dependency_reducers: &mut [Reducer<'artifact>],
        target: &IntegralKey,
        request: &mut ReductionRequest,
        statistics: &mut ReductionStatistics,
        shared_cache: Arc<SharedCacheBudget>,
        limits: ReductionLimits,
    ) -> Result<Terms, ReductionError> {
        if self.family_fingerprint() != family.fingerprint()
            || rule.installed_family_fingerprint() != Some(family.fingerprint())
            || dependencies.len() != dependency_reducers.len()
        {
            return Err(ReductionError::ReducerInvariant {
                detail: "a factorized product program is detached from its sealed artifact",
            });
        }
        if !self.contains(target.powers())? {
            return Err(ReductionError::ReducerInvariant {
                detail: "a factorized product program received a target outside its exact dependency-root preimage",
            });
        }
        let mut active_powers = Vec::new();
        active_powers
            .try_reserve_exact(self.active_parent_positions.len())
            .map_err(|_| ReductionError::AllocationFailure {
                resource: "factorized product active powers",
                requested: self.active_parent_positions.len(),
            })?;
        for &position in &self.active_parent_positions {
            active_powers.push(*target.powers().get(position).ok_or(
                ReductionError::ReducerInvariant {
                    detail: "a factorized product active position is out of range",
                },
            )?);
        }
        let mut execution = ProductExecution {
            program: self,
            family,
            rule,
            dependencies,
            dependency_reducers,
            request,
            statistics,
            limits,
            active_powers: active_powers.into_boxed_slice(),
            radial_cache: BTreeMap::new(),
            correlated_cache: BTreeMap::new(),
            cache_budget: ProductCacheBudget::new(shared_cache, limits),
        };
        execution.reduce(target)
    }
}

impl ProductExecution<'_, '_, '_> {
    fn reduce(&mut self, target: &IntegralKey) -> Result<Terms, ReductionError> {
        let root = self.initial_state(target)?;
        let pending_baseline = self.request.pending_frame_count();
        let result = (|| {
            let mut cache = BTreeMap::new();
            let mut stack = Vec::new();
            push_numerator_frame(
                &mut stack,
                NumeratorFrame::Expand(root.clone()),
                self.request,
                self.limits,
            )?;
            while let Some(frame) = stack.pop() {
                self.request.release_pending_frame()?;
                match frame {
                    NumeratorFrame::Expand(state) => {
                        if cache.contains_key(&state) {
                            self.statistics.record_cache_hit();
                            continue;
                        }
                        let measure = state.measure()?;
                        if measure == 0 {
                            let terms = self.reduce_monomial(&state.radial, &state.cross)?;
                            insert_state(
                                &mut cache,
                                state,
                                terms,
                                &mut self.cache_budget,
                                self.limits,
                            )?;
                            continue;
                        }
                        self.record_recurrence_work(1)?;
                        let source = state.remaining.iter().position(|&power| power != 0).ok_or(
                            ReductionError::ReducerInvariant {
                                detail: "a positive numerator measure has no remaining source",
                            },
                        )?;
                        let branches = self.program.numerator_branches.get(source).ok_or(
                            ReductionError::ReducerInvariant {
                                detail: "a numerator source has no cold-compiled affine branches",
                            },
                        )?;
                        let mut children = Vec::new();
                        children.try_reserve_exact(branches.len()).map_err(|_| {
                            ReductionError::AllocationFailure {
                                resource: "factorized numerator branches",
                                requested: branches.len(),
                            }
                        })?;
                        for branch in branches {
                            let mut child = state.clone();
                            child.remaining[source] = child.remaining[source]
                                .checked_sub(1)
                                .ok_or(ReductionError::ReducerInvariant {
                                    detail: "a selected numerator source has zero remaining power",
                                })?;
                            match branch.variable {
                                ProductMomentVariable::Constant => {}
                                ProductMomentVariable::Radial(vector) => {
                                    increment_moment(&mut child.radial, vector, "radial moment")?;
                                }
                                ProductMomentVariable::Cross(edge) => {
                                    increment_moment(&mut child.cross, edge, "cross moment")?;
                                }
                            }
                            if child.measure()?.checked_add(1) != Some(measure) {
                                return Err(ReductionError::ReducerInvariant {
                                    detail: "the factorized numerator recurrence lost strict descent",
                                });
                            }
                            children.push((child, branch.coefficient.clone()));
                        }
                        push_numerator_frame(
                            &mut stack,
                            NumeratorFrame::Combine {
                                state: state.clone(),
                                children: children.clone(),
                            },
                            self.request,
                            self.limits,
                        )?;
                        for (child, _) in children.into_iter().rev() {
                            if !cache.contains_key(&child) {
                                push_numerator_frame(
                                    &mut stack,
                                    NumeratorFrame::Expand(child),
                                    self.request,
                                    self.limits,
                                )?;
                            }
                        }
                    }
                    NumeratorFrame::Combine { state, children } => {
                        let mut terms = BTreeMap::new();
                        for (child, branch_coefficient) in children {
                            let expansion =
                            cache.get(&child).ok_or(ReductionError::ReducerInvariant {
                                detail: "a factorized numerator child is absent at combine time",
                            })?;
                            add_weighted_terms(
                                self.family.coefficient_context(),
                                &mut terms,
                                expansion,
                                &branch_coefficient,
                                self.limits,
                                self.request,
                                self.statistics,
                            )?;
                        }
                        insert_state(
                            &mut cache,
                            state,
                            terms,
                            &mut self.cache_budget,
                            self.limits,
                        )?;
                    }
                }
            }
            remove_state(&mut cache, &root, &mut self.cache_budget)?.ok_or(
                ReductionError::ReducerInvariant {
                    detail: "the factorized numerator root was not evaluated",
                },
            )
        })();
        self.request.restore_pending_frames(pending_baseline);
        result
    }

    fn initial_state(&self, target: &IntegralKey) -> Result<NumeratorState, ReductionError> {
        let active = self.rule.application_domain().sector().active_bits();
        if target.powers().len() != active.len() {
            return Err(ReductionError::WrongArity {
                expected: active.len(),
                actual: target.powers().len(),
            });
        }
        let mut remaining = Vec::new();
        remaining.try_reserve_exact(active.len()).map_err(|_| {
            ReductionError::AllocationFailure {
                resource: "factorized numerator remaining powers",
                requested: active.len(),
            }
        })?;
        for (&power, &is_active) in target.powers().iter().zip(active) {
            remaining.push(if is_active {
                0
            } else {
                u128::from(power.unsigned_abs())
            });
        }
        Ok(NumeratorState {
            remaining: remaining.into_boxed_slice(),
            radial: vec![0; self.family.loop_count()].into_boxed_slice(),
            cross: vec![0; self.program.edges.len()].into_boxed_slice(),
        })
    }

    fn reduce_monomial(
        &mut self,
        radial: &[MomentPower],
        cross: &[MomentPower],
    ) -> Result<Terms, ReductionError> {
        match &self.program.layout {
            ProductBlockLayout::AllSingleton {
                singletons_by_vector,
            } => self.reduce_all_singletons(radial, cross, singletons_by_vector),
            ProductBlockLayout::OneCorrelated {
                correlated,
                singletons_by_vector,
            } => self.reduce_one_correlated(radial, cross, correlated, singletons_by_vector),
        }
    }

    fn reduce_all_singletons(
        &mut self,
        radial: &[MomentPower],
        cross: &[MomentPower],
        singletons: &[SingletonProductBlock],
    ) -> Result<Terms, ReductionError> {
        let active_powers = self.active_powers.clone();
        let context = self.family.coefficient_context();
        let moment_limits = self.remaining_moment_limits()?;
        let mut budget = CoefficientBudget::new(moment_limits);
        let mut angular = AngularEvaluator::new(
            context,
            self.family.dimension(),
            self.family.loop_count(),
            &self.program.edges,
            moment_limits,
        );
        let evaluation = angular.evaluate(cross, &mut budget);
        let attempted_transitions = angular.attempted_transition_count();
        let coalescing_additions = angular.coalescing_addition_count();
        self.record_recurrence_work(attempted_transitions)?;
        self.record_coalescing_work(coalescing_additions)?;
        let angular_coefficient = evaluation.map_err(|error| self.runtime_product_error(error))?;
        // No angular cache may coexist with nested lower-artifact reduction.
        // The returned coefficient is an owned exact value; the guards are a
        // cold replay witness and are not needed by the already-authenticated
        // hot program.
        let _ = angular.finish(&mut budget).map_err(product_error)?;
        if angular_coefficient.is_zero() {
            return Ok(BTreeMap::new());
        }
        let Some(angular_radial) =
            cross_radial_powers(self.family.loop_count(), &self.program.edges, cross)
                .map_err(product_error)?
        else {
            return Ok(BTreeMap::new());
        };
        let total_radial = add_powers(radial, &angular_radial, "total radial moment")?;
        let coefficient = context.try_mul(
            &self.program.normalization,
            &angular_coefficient,
            self.limits.exact_algebra,
        )?;
        let zero = IntegralKey::try_new(std::iter::repeat_n(0, self.family.denominator_count()))?;
        let mut products = BTreeMap::from([(zero, coefficient)]);
        for block in singletons {
            let expansion = self.reduce_radial(
                block.dependency_ordinal,
                active_powers[block.active_power_ordinal],
                total_radial[block.transformed_vector],
            )?;
            products = convolve_factor_expansion_in_request(
                context,
                &products,
                &expansion,
                std::slice::from_ref(&block.parent_position),
                self.family.denominator_count(),
                self.limits,
                self.request,
                self.statistics,
            )?;
        }
        self.route_parent_terminals(products)
    }

    fn reduce_one_correlated(
        &mut self,
        radial: &[MomentPower],
        cross: &[MomentPower],
        correlated: &CorrelatedProductBlock,
        singletons: &[SingletonProductBlock],
    ) -> Result<Terms, ReductionError> {
        let active_powers = self.active_powers.clone();
        let context = self.family.coefficient_context();
        let moment_limits = self.remaining_moment_limits()?;
        let mut budget = CoefficientBudget::new(moment_limits);
        let mut additions = 0;
        let mut angular = PartialAngularEvaluator::try_new(
            context,
            self.family.dimension(),
            self.family.loop_count(),
            &self.program.edges,
            singletons,
            moment_limits,
        )
        .map_err(product_error)?;
        let evaluation = angular.evaluate(radial, cross, &mut budget, &mut additions);
        let attempted_transitions = angular.attempted_transition_count();
        self.record_recurrence_work(attempted_transitions)?;
        self.record_coalescing_work(additions)?;
        let moments = evaluation.map_err(|error| self.runtime_product_error(error))?;
        // Release the angular guard cache before invoking any dependency.  The
        // output moment map remains live, so register its complete coefficient
        // payload in the same aggregate cache owner as the parent and all
        // dependency reducers for the duration of those nested calls.
        let _ = angular.finish().map_err(product_error)?;
        self.cache_budget
            .retain_coefficients(moments.len(), moments.values())?;
        let result = (|| {
            let mut output = BTreeMap::new();
            for (moment, angular_coefficient) in &moments {
                let normalized = context.try_mul(
                    &self.program.normalization,
                    angular_coefficient,
                    self.limits.exact_algebra,
                )?;
                let correlated_terms =
                    self.reduce_correlated_moment(correlated, &active_powers, moment)?;
                let zero =
                    IntegralKey::try_new(std::iter::repeat_n(0, self.family.denominator_count()))?;
                let mut products = BTreeMap::new();
                for (master, coefficient) in correlated_terms {
                    let mut powers = zero.powers().to_vec();
                    inject_master(&mut powers, &correlated.parent_positions, &master)?;
                    let coefficient =
                        context.try_mul(&normalized, &coefficient, self.limits.exact_algebra)?;
                    accumulate_master_in_request(
                        context,
                        &mut products,
                        &IntegralKey::try_new(powers)?,
                        coefficient,
                        self.limits,
                        self.request,
                        self.statistics,
                    )?;
                }
                for block in singletons {
                    let expansion = self.reduce_radial(
                        block.dependency_ordinal,
                        active_powers[block.active_power_ordinal],
                        moment.radial_powers(self.family.loop_count())[block.transformed_vector],
                    )?;
                    products = convolve_factor_expansion_in_request(
                        context,
                        &products,
                        &expansion,
                        std::slice::from_ref(&block.parent_position),
                        self.family.denominator_count(),
                        self.limits,
                        self.request,
                        self.statistics,
                    )?;
                }
                let routed = self.route_parent_terminals(products)?;
                add_weighted_terms(
                    context,
                    &mut output,
                    &routed,
                    &context.one(),
                    self.limits,
                    self.request,
                    self.statistics,
                )?;
            }
            Ok(output)
        })();
        let release = self
            .cache_budget
            .release_coefficients(moments.len(), moments.values());
        release?;
        result
    }

    fn reduce_radial(
        &mut self,
        dependency_ordinal: usize,
        denominator_power: i64,
        radial_power: MomentPower,
    ) -> Result<Terms, ReductionError> {
        let root = RadialState {
            dependency_ordinal,
            denominator_power,
            radial_power,
        };
        if let Some(cached) = self.radial_cache.get(&root) {
            self.statistics.record_cache_hit();
            return Ok(cached.clone());
        }
        let pending_baseline = self.request.pending_frame_count();
        let result = (|| {
            let mut stack = Vec::new();
            push_radial_frame(
                &mut stack,
                RadialFrame::Expand(root.clone()),
                self.request,
                self.limits,
            )?;
            while let Some(frame) = stack.pop() {
                self.request.release_pending_frame()?;
                match frame {
                    RadialFrame::Expand(state) => {
                        if self.radial_cache.contains_key(&state) {
                            self.statistics.record_cache_hit();
                            continue;
                        }
                        if state.radial_power == 0 {
                            let target = IntegralKey::try_new([state.denominator_power])?;
                            let terms = self
                            .dependency_reducers
                            .get_mut(state.dependency_ordinal)
                            .ok_or(ReductionError::ReducerInvariant {
                                detail: "a product moment references an absent dependency reducer",
                            })?
                            .reduce_unit_mass_in_request(&target, self.request)?
                            .terms()
                            .clone();
                            insert_state(
                                &mut self.radial_cache,
                                state,
                                terms,
                                &mut self.cache_budget,
                                self.limits,
                            )?;
                            continue;
                        }
                        self.record_recurrence_work(1)?;
                        let next_rank = state.radial_power - 1;
                        let shifted = state
                            .denominator_power
                            .checked_sub(1)
                            .ok_or(ReductionError::IndexOverflow { position: 0 })?;
                        let children = [
                            RadialState {
                                dependency_ordinal: state.dependency_ordinal,
                                denominator_power: state.denominator_power,
                                radial_power: next_rank,
                            },
                            RadialState {
                                dependency_ordinal: state.dependency_ordinal,
                                denominator_power: shifted,
                                radial_power: next_rank,
                            },
                        ];
                        push_radial_frame(
                            &mut stack,
                            RadialFrame::Combine {
                                state: state.clone(),
                                children: children.clone(),
                            },
                            self.request,
                            self.limits,
                        )?;
                        for child in children.into_iter().rev() {
                            if !self.radial_cache.contains_key(&child) {
                                push_radial_frame(
                                    &mut stack,
                                    RadialFrame::Expand(child),
                                    self.request,
                                    self.limits,
                                )?;
                            }
                        }
                    }
                    RadialFrame::Combine { state, children } => {
                        let mut terms = BTreeMap::new();
                        for child in children {
                            let child_terms = self.radial_cache.get(&child).ok_or(
                                ReductionError::ReducerInvariant {
                                    detail: "a radial child is absent at combine time",
                                },
                            )?;
                            add_weighted_terms(
                                self.family.coefficient_context(),
                                &mut terms,
                                child_terms,
                                &self.family.coefficient_context().one(),
                                self.limits,
                                self.request,
                                self.statistics,
                            )?;
                        }
                        insert_state(
                            &mut self.radial_cache,
                            state,
                            terms,
                            &mut self.cache_budget,
                            self.limits,
                        )?;
                    }
                }
            }
            self.radial_cache
                .get(&root)
                .cloned()
                .ok_or(ReductionError::ReducerInvariant {
                    detail: "the radial root was not evaluated",
                })
        })();
        self.request.restore_pending_frames(pending_baseline);
        result
    }

    fn reduce_correlated_moment(
        &mut self,
        block: &CorrelatedProductBlock,
        active_powers: &[i64],
        moment: &PartialMomentKey,
    ) -> Result<Terms, ReductionError> {
        let dependency = self.dependencies.get(block.dependency_ordinal).ok_or(
            ReductionError::ReducerInvariant {
                detail: "a correlated block references an absent dependency artifact",
            },
        )?;
        let powers =
            correlated_coordinate_powers(self.program, dependency.family(), block, moment)?;
        let end = block
            .active_power_start
            .checked_add(block.parent_positions.len())
            .ok_or(ReductionError::ReducerInvariant {
                detail: "a correlated active-power range overflowed",
            })?;
        let denominator_powers = active_powers
            .get(block.active_power_start..end)
            .ok_or(ReductionError::ReducerInvariant {
                detail: "a correlated active-power range is absent",
            })?
            .to_vec()
            .into_boxed_slice();
        let root = CorrelatedState {
            denominator_powers,
            moment_powers: powers,
        };
        if let Some(cached) = self.correlated_cache.get(&root) {
            self.statistics.record_cache_hit();
            return Ok(cached.clone());
        }
        let pending_baseline = self.request.pending_frame_count();
        let result = (|| {
            let mut stack = Vec::new();
            push_correlated_frame(
                &mut stack,
                CorrelatedFrame::Expand(root.clone()),
                self.request,
                self.limits,
            )?;
            while let Some(frame) = stack.pop() {
                self.request.release_pending_frame()?;
                match frame {
                    CorrelatedFrame::Expand(state) => {
                        if self.correlated_cache.contains_key(&state) {
                            self.statistics.record_cache_hit();
                            continue;
                        }
                        let measure = state.measure()?;
                        let Some(coordinate) =
                            state.moment_powers.iter().position(|&power| power != 0)
                        else {
                            let target =
                                IntegralKey::try_new(state.denominator_powers.iter().copied())?;
                            let terms = self
                            .dependency_reducers
                            .get_mut(block.dependency_ordinal)
                            .ok_or(ReductionError::ReducerInvariant {
                                detail: "a correlated moment references an absent dependency reducer",
                            })?
                            .reduce_unit_mass_in_request(&target, self.request)?
                            .terms()
                            .clone();
                            insert_state(
                                &mut self.correlated_cache,
                                state,
                                terms,
                                &mut self.cache_budget,
                                self.limits,
                            )?;
                            continue;
                        };
                        self.record_recurrence_work(1)?;
                        let branches = block.moment_branches.get(coordinate).ok_or(
                            ReductionError::ReducerInvariant {
                                detail: "a correlated coordinate has no cold-compiled branches",
                            },
                        )?;
                        let mut children = Vec::new();
                        children.try_reserve_exact(branches.len()).map_err(|_| {
                            ReductionError::AllocationFailure {
                                resource: "correlated moment branches",
                                requested: branches.len(),
                            }
                        })?;
                        for branch in branches {
                            let mut child = state.clone();
                            child.moment_powers[coordinate] -= 1;
                            if let Some(denominator) = branch.denominator {
                                let slot = child.denominator_powers.get_mut(denominator).ok_or(
                                    ReductionError::ReducerInvariant {
                                        detail: "a correlated branch denominator is out of range",
                                    },
                                )?;
                                *slot =
                                    slot.checked_sub(1).ok_or(ReductionError::IndexOverflow {
                                        position: denominator,
                                    })?;
                            }
                            if child.measure()?.checked_add(1) != Some(measure) {
                                return Err(ReductionError::ReducerInvariant {
                                    detail: "the correlated moment recurrence lost strict descent",
                                });
                            }
                            children.push((child, branch.coefficient.clone()));
                        }
                        push_correlated_frame(
                            &mut stack,
                            CorrelatedFrame::Combine {
                                state: state.clone(),
                                children: children.clone(),
                            },
                            self.request,
                            self.limits,
                        )?;
                        for (child, _) in children.into_iter().rev() {
                            if !self.correlated_cache.contains_key(&child) {
                                push_correlated_frame(
                                    &mut stack,
                                    CorrelatedFrame::Expand(child),
                                    self.request,
                                    self.limits,
                                )?;
                            }
                        }
                    }
                    CorrelatedFrame::Combine { state, children } => {
                        let mut terms = BTreeMap::new();
                        for (child, coefficient) in children {
                            let expansion = self.correlated_cache.get(&child).ok_or(
                                ReductionError::ReducerInvariant {
                                    detail: "a correlated child is absent at combine time",
                                },
                            )?;
                            add_weighted_terms(
                                self.family.coefficient_context(),
                                &mut terms,
                                expansion,
                                &coefficient,
                                self.limits,
                                self.request,
                                self.statistics,
                            )?;
                        }
                        insert_state(
                            &mut self.correlated_cache,
                            state,
                            terms,
                            &mut self.cache_budget,
                            self.limits,
                        )?;
                    }
                }
            }
            self.correlated_cache
                .get(&root)
                .cloned()
                .ok_or(ReductionError::ReducerInvariant {
                    detail: "the correlated moment root was not evaluated",
                })
        })();
        self.request.restore_pending_frames(pending_baseline);
        result
    }

    fn route_parent_terminals(&mut self, raw: Terms) -> Result<Terms, ReductionError> {
        let mut output = BTreeMap::new();
        for (raw_master, coefficient) in raw {
            let terminal = self.rule.parent_terminal_for(&raw_master).ok_or(
                ReductionError::ReducerInvariant {
                    detail: "a product moment emitted an unauthenticated master product",
                },
            )?;
            accumulate_master_in_request(
                self.family.coefficient_context(),
                &mut output,
                terminal,
                coefficient,
                self.limits,
                self.request,
                self.statistics,
            )?;
        }
        Ok(output)
    }

    fn record_recurrence_work(&mut self, count: usize) -> Result<(), ReductionError> {
        if count == 0 {
            return Ok(());
        }
        self.statistics.record_rule_applications(count);
        self.request
            .record_rule_applications(count, self.limits.max_rule_applications)?;
        Ok(())
    }

    fn record_coalescing_work(&mut self, count: usize) -> Result<(), ReductionError> {
        if count == 0 {
            return Ok(());
        }
        self.statistics.record_coalescing_additions(count);
        self.request
            .record_coalescing_additions(count, self.limits.max_coalescing_additions)
    }

    fn remaining_moment_limits(&self) -> Result<FactorizedProductMomentLimits, ReductionError> {
        let cache = self.cache_budget.remaining()?;
        let mut limits = runtime_moment_limits(self.limits);
        limits.max_angular_states = limits.max_angular_states.min(cache.integrals);
        limits.max_retained_coefficient_terms = limits
            .max_retained_coefficient_terms
            .min(cache.coefficient_terms);
        limits.max_retained_coefficient_clone_owned_bytes = limits
            .max_retained_coefficient_clone_owned_bytes
            .min(cache.coefficient_bytes);
        limits.max_pending_frames = limits.max_pending_frames.min(
            self.request
                .remaining_pending_frames(self.limits.max_pending_frames),
        );
        limits.max_angular_transitions = limits.max_angular_transitions.min(
            self.request
                .remaining_rule_applications(self.limits.max_rule_applications),
        );
        limits.max_coalescing_additions = limits.max_coalescing_additions.min(
            self.request
                .remaining_coalescing_additions(self.limits.max_coalescing_additions),
        );
        Ok(limits)
    }

    fn runtime_product_error(&self, error: FactorizedProductMomentError) -> ReductionError {
        let FactorizedProductMomentError::ResourceLimit {
            resource,
            requested,
            limit,
        } = error
        else {
            return product_error(error);
        };
        let cache = self.cache_budget.shared.snapshot();
        let aggregate = |retained: usize| retained.checked_add(requested);
        match resource {
            "angular states" => aggregate(cache.integrals).map_or(
                ReductionError::CacheResourceCountOverflow { resource },
                |requested| ReductionError::CacheLimit {
                    requested,
                    limit: self.limits.max_cached_integrals,
                },
            ),
            "product retained coefficient terms" => aggregate(cache.coefficient_terms).map_or(
                ReductionError::CacheResourceCountOverflow { resource },
                |requested| ReductionError::CacheCoefficientTermLimit {
                    requested,
                    limit: self.limits.max_cached_coefficient_terms,
                },
            ),
            "product retained coefficient clone-owned bytes" => aggregate(cache.coefficient_bytes)
                .map_or(
                    ReductionError::CacheResourceCountOverflow { resource },
                    |requested| ReductionError::CacheCoefficientByteLimit {
                        requested,
                        limit: self.limits.max_cached_coefficient_bytes,
                    },
                ),
            "angular pending frames" => self
                .request
                .pending_frame_count()
                .checked_add(requested)
                .map_or_else(
                    || ReductionError::PendingFrameLimit {
                        requested: usize::MAX,
                        limit: self.limits.max_pending_frames,
                    },
                    |requested| ReductionError::PendingFrameLimit {
                        requested,
                        limit: self.limits.max_pending_frames,
                    },
                ),
            _ => ReductionError::FactorizedProductMoment {
                detail: FactorizedProductMomentError::ResourceLimit {
                    resource,
                    requested,
                    limit,
                }
                .to_string(),
            },
        }
    }
}

fn correlated_coordinate_powers(
    program: &FactorizedProductMomentProgram,
    dependency_family: &IntegralFamily,
    block: &CorrelatedProductBlock,
    moment: &PartialMomentKey,
) -> Result<Box<[MomentPower]>, ReductionError> {
    let loop_count = program.loop_factor_count();
    let radial = moment.radial_powers(loop_count);
    let cross = moment.cross_powers(loop_count);
    let mut output = Vec::new();
    output
        .try_reserve_exact(block.moment_branches.len())
        .map_err(|_| ReductionError::AllocationFailure {
            resource: "correlated coordinate powers",
            requested: block.moment_branches.len(),
        })?;
    if dependency_family.coordinates().len() != block.moment_branches.len() {
        return Err(ReductionError::ReducerInvariant {
            detail: "the correlated branch table no longer matches its dependency coordinates",
        });
    }
    for coordinate in dependency_family.coordinates() {
        let ScalarProductCoordinate::LoopLoop { left, right } = *coordinate else {
            return Err(ReductionError::ReducerInvariant {
                detail: "a correlated dependency contains an external scalar product",
            });
        };
        let global_left =
            *block
                .transformed_vectors
                .get(left)
                .ok_or(ReductionError::ReducerInvariant {
                    detail: "a correlated dependency loop position is out of range",
                })?;
        let global_right =
            *block
                .transformed_vectors
                .get(right)
                .ok_or(ReductionError::ReducerInvariant {
                    detail: "a correlated dependency loop position is out of range",
                })?;
        output.push(if left == right {
            radial[global_left]
        } else {
            let pair = (global_left.min(global_right), global_left.max(global_right));
            let edge = program
                .edges
                .iter()
                .position(|&candidate| candidate == pair)
                .ok_or(ReductionError::ReducerInvariant {
                    detail: "a correlated cross coordinate is absent",
                })?;
            cross[edge]
        });
    }
    Ok(output.into_boxed_slice())
}

fn runtime_moment_limits(limits: ReductionLimits) -> FactorizedProductMomentLimits {
    let work = limits.max_rule_applications;
    FactorizedProductMomentLimits {
        exact_algebra: limits.exact_algebra,
        max_angular_degree: work,
        max_angular_states: limits.max_cached_integrals,
        max_angular_transitions: work,
        max_pending_frames: limits.max_pending_frames,
        max_guards: work,
        max_coalescing_additions: limits.max_coalescing_additions,
        max_retained_coefficient_terms: limits.max_cached_coefficient_terms,
        max_retained_coefficient_clone_owned_bytes: limits.max_cached_coefficient_bytes,
        ..FactorizedProductMomentLimits::default()
    }
}

impl ProductCacheBudget {
    fn new(shared: Arc<SharedCacheBudget>, limits: ReductionLimits) -> Self {
        Self {
            shared,
            limits,
            states: 0,
            weight: CacheWeight::default(),
        }
    }

    fn retain(&mut self, terms: &Terms, _limits: ReductionLimits) -> Result<(), ReductionError> {
        self.retain_coefficients(1, terms.values())
    }

    fn retain_coefficients<'a>(
        &mut self,
        states: usize,
        coefficients: impl IntoIterator<Item = &'a Coefficient>,
    ) -> Result<(), ReductionError> {
        let prospective_states =
            self.states
                .checked_add(states)
                .ok_or(ReductionError::CacheResourceCountOverflow {
                    resource: "factorized product states",
                })?;
        let prospective_weight = self
            .weight
            .checked_add(coefficient_cache_weight(coefficients)?)?;
        self.shared.replace(
            self.states,
            self.weight,
            prospective_states,
            prospective_weight,
            self.limits,
        )?;
        self.states = prospective_states;
        self.weight = prospective_weight;
        Ok(())
    }

    fn release(&mut self, terms: &Terms) -> Result<(), ReductionError> {
        self.release_coefficients(1, terms.values())
    }

    fn release_coefficients<'a>(
        &mut self,
        states: usize,
        coefficients: impl IntoIterator<Item = &'a Coefficient>,
    ) -> Result<(), ReductionError> {
        let prospective_states =
            self.states
                .checked_sub(states)
                .ok_or(ReductionError::ReducerInvariant {
                    detail: "the factorized product state census underflowed",
                })?;
        let prospective_weight = self
            .weight
            .checked_sub(coefficient_cache_weight(coefficients)?)?;
        self.shared.replace(
            self.states,
            self.weight,
            prospective_states,
            prospective_weight,
            self.limits,
        )?;
        self.states = prospective_states;
        self.weight = prospective_weight;
        Ok(())
    }

    fn remaining(&self) -> Result<crate::reduction::CacheCensus, ReductionError> {
        let census = self.shared.snapshot();
        Ok(crate::reduction::CacheCensus {
            integrals: self
                .limits
                .max_cached_integrals
                .checked_sub(census.integrals)
                .ok_or(ReductionError::ReducerInvariant {
                    detail: "the shared integral cache exceeds its installed limit",
                })?,
            coefficient_terms: self
                .limits
                .max_cached_coefficient_terms
                .checked_sub(census.coefficient_terms)
                .ok_or(ReductionError::ReducerInvariant {
                    detail: "the shared coefficient-term cache exceeds its installed limit",
                })?,
            coefficient_bytes: self
                .limits
                .max_cached_coefficient_bytes
                .checked_sub(census.coefficient_bytes)
                .ok_or(ReductionError::ReducerInvariant {
                    detail: "the shared coefficient-byte cache exceeds its installed limit",
                })?,
        })
    }
}

impl Drop for ProductCacheBudget {
    fn drop(&mut self) {
        if self.states == 0 && self.weight == CacheWeight::default() {
            return;
        }
        let released = self.shared.replace(
            self.states,
            self.weight,
            0,
            CacheWeight::default(),
            self.limits,
        );
        debug_assert!(released.is_ok());
    }
}

fn insert_state<K: Ord>(
    cache: &mut BTreeMap<K, Terms>,
    key: K,
    terms: Terms,
    budget: &mut ProductCacheBudget,
    limits: ReductionLimits,
) -> Result<(), ReductionError> {
    if cache.contains_key(&key) {
        return Err(ReductionError::ReducerInvariant {
            detail: "a factorized product state was retained twice",
        });
    }
    budget.retain(&terms, limits)?;
    cache.insert(key, terms);
    Ok(())
}

fn remove_state<K: Ord>(
    cache: &mut BTreeMap<K, Terms>,
    key: &K,
    budget: &mut ProductCacheBudget,
) -> Result<Option<Terms>, ReductionError> {
    let Some(terms) = cache.remove(key) else {
        return Ok(None);
    };
    budget.release(&terms)?;
    Ok(Some(terms))
}

pub(super) fn add_weighted_terms(
    context: &CoefficientContext,
    output: &mut Terms,
    input: &Terms,
    weight: &Coefficient,
    limits: ReductionLimits,
    request: &mut ReductionRequest,
    statistics: &mut ReductionStatistics,
) -> Result<(), ReductionError> {
    for (master, coefficient) in input {
        let contribution = context.try_mul(weight, coefficient, limits.exact_algebra)?;
        accumulate_master_in_request(
            context,
            output,
            master,
            contribution,
            limits,
            request,
            statistics,
        )?;
    }
    if output.len() > limits.max_factorization_terms {
        return Err(ReductionError::FactorizationTermLimit {
            requested: output.len(),
            limit: limits.max_factorization_terms,
        });
    }
    Ok(())
}

fn add_powers(
    left: &[MomentPower],
    right: &[MomentPower],
    resource: &'static str,
) -> Result<Box<[MomentPower]>, ReductionError> {
    if left.len() != right.len() {
        return Err(ReductionError::ReducerInvariant {
            detail: "moment power vectors have different widths",
        });
    }
    left.iter()
        .zip(right)
        .map(|(&left, &right)| {
            left.checked_add(right)
                .ok_or(ReductionError::CacheResourceCountOverflow { resource })
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Vec::into_boxed_slice)
}

fn increment_moment(
    values: &mut [MomentPower],
    position: usize,
    resource: &'static str,
) -> Result<(), ReductionError> {
    let slot = values
        .get_mut(position)
        .ok_or(ReductionError::ReducerInvariant {
            detail: "a moment branch position is out of range",
        })?;
    *slot = slot
        .checked_add(1)
        .ok_or(ReductionError::CacheResourceCountOverflow { resource })?;
    Ok(())
}

fn sum_moment_powers(
    values: &[MomentPower],
    resource: &'static str,
) -> Result<MomentPower, ReductionError> {
    values.iter().try_fold(0_u128, |total, &value| {
        total
            .checked_add(value)
            .ok_or(ReductionError::CacheResourceCountOverflow { resource })
    })
}

fn inject_master(
    parent: &mut [i64],
    positions: &[usize],
    master: &IntegralKey,
) -> Result<(), ReductionError> {
    if positions.len() != master.powers().len() {
        return Err(ReductionError::ReducerInvariant {
            detail: "a dependency master has the wrong embedding arity",
        });
    }
    for (&position, &power) in positions.iter().zip(master.powers()) {
        let slot = parent
            .get_mut(position)
            .ok_or(ReductionError::ReducerInvariant {
                detail: "a dependency master embedding is out of range",
            })?;
        if *slot != 0 {
            return Err(ReductionError::ReducerInvariant {
                detail: "dependency master embeddings overlap",
            });
        }
        *slot = power;
    }
    Ok(())
}

fn push_numerator_frame(
    stack: &mut Vec<NumeratorFrame>,
    frame: NumeratorFrame,
    request: &mut ReductionRequest,
    limits: ReductionLimits,
) -> Result<(), ReductionError> {
    push_frame(stack, frame, request, limits, "factorized numerator frames")
}

fn push_radial_frame(
    stack: &mut Vec<RadialFrame>,
    frame: RadialFrame,
    request: &mut ReductionRequest,
    limits: ReductionLimits,
) -> Result<(), ReductionError> {
    push_frame(stack, frame, request, limits, "factorized radial frames")
}

fn push_correlated_frame(
    stack: &mut Vec<CorrelatedFrame>,
    frame: CorrelatedFrame,
    request: &mut ReductionRequest,
    limits: ReductionLimits,
) -> Result<(), ReductionError> {
    push_frame(
        stack,
        frame,
        request,
        limits,
        "factorized correlated frames",
    )
}

fn push_frame<T>(
    stack: &mut Vec<T>,
    frame: T,
    request: &mut ReductionRequest,
    limits: ReductionLimits,
    resource: &'static str,
) -> Result<(), ReductionError> {
    let requested = stack
        .len()
        .checked_add(1)
        .ok_or(ReductionError::PendingFrameLimit {
            requested: usize::MAX,
            limit: limits.max_pending_frames,
        })?;
    stack
        .try_reserve(1)
        .map_err(|_| ReductionError::AllocationFailure {
            resource,
            requested,
        })?;
    request.retain_pending_frame(limits.max_pending_frames)?;
    stack.push(frame);
    Ok(())
}

fn product_error(error: FactorizedProductMomentError) -> ReductionError {
    match error {
        FactorizedProductMomentError::ExactAlgebra(error) => error.into(),
        FactorizedProductMomentError::IntegralKey(error) => error.into(),
        other => ReductionError::FactorizedProductMoment {
            detail: other.to_string(),
        },
    }
}
