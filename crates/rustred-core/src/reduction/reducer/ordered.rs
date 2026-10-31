//! Closed-artifact adapter for the separate global ordered scheduler.
//!
//! Selection, specialization, symmetry routing and product factorization are
//! deliberately the same checked operations as in the depth-first reducer.
//! This adapter never reads or populates a caller's completed-result cache.
use super::*;
use crate::reduction::ordered_reduction::{
    self, OrderedKernel, OrderedKeyStage, OrderedPhaseDescriptor, OrderedPhaseEvent,
    OrderedPhaseInspector, OrderedPosition, OrderedStep,
};
use crate::sector::{ComplexityKey, OrderingPolicy};
use crate::solver::{CandidateOrderedPhaseKind, CandidateOrderedReductionProgress};

/// One exact weighted closed-artifact reduction, without a result cache.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderedArtifactReduction {
    pub terms: BTreeMap<IntegralKey, Coefficient>,
    pub nonzero_conditions: Vec<Coefficient>,
    pub work: ReductionStatistics,
    pub progress: CandidateOrderedReductionProgress,
}

type Observer<'a> = dyn FnMut(&CandidateOrderedReductionProgress, Option<&IntegralKey>) -> Result<(), ReductionError>
    + 'a;

struct ClosedKernel<'artifact, 'observer, 'callback> {
    // Fresh state belongs exclusively to this ordered request. Lower-family
    // DFS reducers are an implementation detail of the existing complete
    // product-factorization primitive, never an attached DFS cache.
    reducer: Reducer<'artifact>,
    ordering: OrderingPolicy,
    observer: &'observer mut Observer<'callback>,
    inspector: Option<&'observer mut OrderedPhaseInspector<'callback, ReductionError>>,
}

impl<'artifact> Reducer<'artifact> {
    /// Globally combine and reduce a weighted expression at unit common mass.
    /// Every input is admitted before duplicate/zero cancellation. Existing
    /// immutable tables are shared; no completed-result cache is consulted.
    pub fn reduce_artifact_linear_combination_ordered(
        artifact: &'artifact ClosedArtifact,
        terms: impl IntoIterator<Item = (IntegralKey, Coefficient)>,
        limits: ReductionLimits,
    ) -> Result<OrderedArtifactReduction, ReductionError> {
        Self::reduce_artifact_linear_combination_ordered_with_observation(
            artifact,
            terms,
            limits,
            &mut |_, _| Ok(()),
        )
    }

    /// The observation callback is invoked between checked actions and phase
    /// transitions. Returning an error cancels this request without changing
    /// any other reducer or persistent cache.
    pub fn reduce_artifact_linear_combination_ordered_with_observation(
        artifact: &'artifact ClosedArtifact,
        terms: impl IntoIterator<Item = (IntegralKey, Coefficient)>,
        limits: ReductionLimits,
        observer: &mut Observer<'_>,
    ) -> Result<OrderedArtifactReduction, ReductionError> {
        Self::reduce_artifact_linear_combination_ordered_with_arithmetic(
            artifact,
            terms,
            limits,
            crate::reduction::OrderedWeightArithmetic::Exact,
            observer,
        )
    }

    /// As [`Self::reduce_artifact_linear_combination_ordered_with_observation`]
    /// with an explicit frontier weight arithmetic. Every arithmetic expands
    /// the same keys and returns the same exact terms and conditions.
    pub fn reduce_artifact_linear_combination_ordered_with_arithmetic(
        artifact: &'artifact ClosedArtifact,
        terms: impl IntoIterator<Item = (IntegralKey, Coefficient)>,
        limits: ReductionLimits,
        arithmetic: crate::reduction::OrderedWeightArithmetic,
        observer: &mut Observer<'_>,
    ) -> Result<OrderedArtifactReduction, ReductionError> {
        Self::reduce_artifact_ordered(artifact, terms, limits, arithmetic, observer, None)
    }

    /// Inspect both boundaries of every frozen phase under the selected weight
    /// arithmetic. Inspection borrows the scheduler state (weights are exact
    /// unless the arithmetic is modular without the exact cross-check); an
    /// inspector error stops this request only.
    pub fn reduce_artifact_linear_combination_ordered_with_inspector(
        artifact: &'artifact ClosedArtifact,
        terms: impl IntoIterator<Item = (IntegralKey, Coefficient)>,
        limits: ReductionLimits,
        arithmetic: crate::reduction::OrderedWeightArithmetic,
        inspector: &mut OrderedPhaseInspector<'_, ReductionError>,
    ) -> Result<OrderedArtifactReduction, ReductionError> {
        Self::reduce_artifact_ordered(
            artifact,
            terms,
            limits,
            arithmetic,
            &mut |_, _| Ok(()),
            Some(inspector),
        )
    }

    fn reduce_artifact_ordered<'callback>(
        artifact: &'artifact ClosedArtifact,
        terms: impl IntoIterator<Item = (IntegralKey, Coefficient)>,
        limits: ReductionLimits,
        arithmetic: crate::reduction::OrderedWeightArithmetic,
        observer: &mut Observer<'callback>,
        inspector: Option<&mut OrderedPhaseInspector<'callback, ReductionError>>,
    ) -> Result<OrderedArtifactReduction, ReductionError> {
        let ordering = artifact.ordering();
        // Installation already authenticates this authority. Keep the seam
        // fail-closed even for internal/test artifacts constructed in memory.
        if artifact
            .rules()
            .iter()
            .any(|rule| rule.ordering() != ordering)
            || artifact
                .rule_cells()
                .iter()
                .any(|cell| cell.rule().ordering() != ordering)
            || artifact
                .canonicalizer()
                .is_some_and(|canonicalizer| canonicalizer.ordering() != ordering)
        {
            return Err(ReductionError::ReducerInvariant {
                detail: "ordered artifact has inconsistent rule or canonicalizer ordering",
            });
        }
        let reducer = Self::build(artifact, limits, Arc::new(SharedCacheBudget::default()))?;
        let mut request = ReductionRequest {
            guarded_conditions: Some(Vec::new()),
            guarded_condition_budget: Some(GuardedConditionBudget::new(
                Arc::clone(&reducer.shared_cache),
                limits,
            )),
            track_step_guards: true,
            ..Default::default()
        };
        for condition in artifact.family().domain().conditions() {
            request.retain_nonzero_guard(&condition.polynomial().clone().into(), limits)?;
        }
        let mut initial = BTreeMap::new();
        let mut initial_work = ReductionStatistics::default();
        for (target, coefficient) in terms {
            symbolica::utils::resources::checkpoint()?;
            reducer.validate_target(&target)?;
            artifact
                .coefficient_context()
                .validate_with_limits(&coefficient, limits.exact_algebra)?;
            request.retain_nonzero_guard(&coefficient.denominator.clone().into(), limits)?;
            let canonical = reducer.canonicalize(&target)?;
            accumulate_master_in_request(
                artifact.coefficient_context(),
                &mut initial,
                &canonical,
                coefficient,
                limits,
                &mut request,
                &mut initial_work,
            )?;
        }
        let conditions = request.guarded_conditions.take().unwrap_or_default();
        request.guarded_conditions = Some(Vec::new());
        let mut kernel = ClosedKernel {
            reducer,
            ordering,
            observer,
            inspector,
        };
        let mut output = ordered_reduction::run_with_arithmetic(
            &mut kernel,
            &mut request,
            initial,
            conditions,
            arithmetic,
        )?;
        output.progress.phase_kind = CandidateOrderedPhaseKind::Reduction;
        output.work.merge_work(initial_work);
        output.work.merge_work(kernel.reducer.statistics());
        Ok(OrderedArtifactReduction {
            terms: output.terms,
            nonzero_conditions: output.nonzero_conditions,
            work: output.work,
            progress: output.progress,
        })
    }
}

impl OrderedKernel for ClosedKernel<'_, '_, '_> {
    type Key = IntegralKey;
    type Priority = ComplexityKey;
    type Error = ReductionError;

    fn context(&self) -> &CoefficientContext {
        self.reducer.artifact.coefficient_context()
    }

    fn limits(&self) -> ReductionLimits {
        self.reducer.limits
    }

    fn phase_count(&self) -> usize {
        if self.ordering.is_support_primary() {
            self.reducer.artifact.arity() + 1
        } else {
            1
        }
    }

    fn position(
        &mut self,
        key: &IntegralKey,
    ) -> Result<OrderedPosition<ComplexityKey>, ReductionError> {
        let priority = self.ordering.complexity_key(key.powers())?;
        // A programmed physical-degree prefix need not preserve support
        // phases. Its exact saved ordering still certifies one global phase.
        let phase = if self.ordering.is_support_primary() {
            self.reducer.artifact.arity() - priority.propagators()
        } else {
            0
        };
        Ok(OrderedPosition { phase, priority })
    }

    fn step(
        &mut self,
        key: &IntegralKey,
        request: &mut ReductionRequest,
        work: &mut ReductionStatistics,
    ) -> Result<OrderedStep<IntegralKey>, ReductionError> {
        let mut result = OrderedStep::default();
        if self.reducer.artifact.masters().contains(key) {
            result.terminals.insert(key.clone(), self.context().one());
        } else if !self.reducer.artifact.is_zero_terminal(key) {
            request.record_rule_application(
                self.reducer
                    .limits
                    .diagnostic_limit(self.reducer.limits.max_rule_applications),
            )?;
            if let Some(selected) = self.reducer.select_first_factorization(key)? {
                self.reducer
                    .retain_factorization_guard(selected.ordinal, request)?;
                result.terminals = self
                    .reducer
                    .apply_factorization(&selected.routed_target, selected.ordinal, request)?
                    .into_terms();
                if result
                    .terminals
                    .keys()
                    .any(|terminal| !self.reducer.artifact.masters().contains(terminal))
                {
                    return Err(ReductionError::ReducerInvariant {
                        detail: "ordered factorization produced an undeclared terminal",
                    });
                }
            } else {
                let selected = self.reducer.select_first_rule(key)?;
                self.reducer
                    .retain_selected_rule_guards(&selected, request)?;
                for (child, coefficient) in
                    self.reducer.apply_selected_rule(key, selected, request)?
                {
                    accumulate_master_in_request(
                        self.context(),
                        &mut result.terms,
                        &child,
                        coefficient,
                        self.reducer.limits,
                        request,
                        work,
                    )?;
                }
            }
            self.reducer.statistics.record_rule_application();
        }
        result.nonzero_conditions = request.guarded_conditions.take().unwrap_or_default();
        request.guarded_conditions = Some(Vec::new());
        Ok(result)
    }

    fn observe(
        &mut self,
        progress: &CandidateOrderedReductionProgress,
        current: Option<&IntegralKey>,
    ) -> Result<(), ReductionError> {
        let mut progress = *progress;
        progress.phase_kind = CandidateOrderedPhaseKind::Reduction;
        progress.active_support = current
            .map(|key| key.powers().iter().filter(|&&power| power > 0).count())
            .or_else(|| {
                (self.ordering.is_support_primary()
                    && progress.completed_phases < progress.total_phases)
                    .then(|| self.reducer.artifact.arity() - progress.completed_phases)
            });
        (self.observer)(&progress, current)
    }

    fn key_view(key: &IntegralKey) -> Option<(OrderedKeyStage<'_>, &IntegralKey)> {
        Some((OrderedKeyStage::Integral, key))
    }

    fn inspect_phase(&mut self, event: OrderedPhaseEvent<'_>) -> Result<(), ReductionError> {
        let Some(inspector) = self.inspector.as_deref_mut() else {
            return Ok(());
        };
        inspector(
            &event.with_descriptor(OrderedPhaseDescriptor {
                kind: CandidateOrderedPhaseKind::Reduction,
                support: self
                    .ordering
                    .is_support_primary()
                    .then(|| self.reducer.artifact.arity() - event.phase),
                ..Default::default()
            }),
        )
    }
}

#[cfg(test)]
mod tests;
