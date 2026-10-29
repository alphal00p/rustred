//! Occurrence-local depth-first execution using the ordinary checked rules.
//! Completed child values live only in their active parent's continuation.
use super::*;
use crate::reduction::depth_first_without_memoization::{self, DepthFirstKernel, DepthFirstStep};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DepthFirstArtifactReduction {
    pub terms: BTreeMap<IntegralKey, Coefficient>,
    pub nonzero_conditions: Vec<Coefficient>,
    pub work: ReductionStatistics,
}

struct ClosedKernel<'borrow, 'artifact, 'request> {
    reducer: &'borrow mut Reducer<'artifact>,
    request: &'request mut ReductionRequest,
}

impl<'artifact> Reducer<'artifact> {
    /// Execute the same depth-first rules without completed-result reuse,
    /// including lower-artifact and factorized-moment reductions. Immutable
    /// program preparation and exact coefficient arithmetic remain shared.
    pub fn reduce_artifact_linear_combination_without_memoization(
        artifact: &'artifact ClosedArtifact,
        terms: impl IntoIterator<Item = (IntegralKey, Coefficient)>,
        limits: ReductionLimits,
    ) -> Result<DepthFirstArtifactReduction, ReductionError> {
        let mut reducer = Self::build_for_execution(
            artifact,
            limits,
            Arc::new(SharedCacheBudget::default()),
            false,
        )?;
        let mut request = ReductionRequest {
            guarded_conditions: Some(Vec::new()),
            track_step_guards: true,
            without_memoization: true,
            ..Default::default()
        };
        for condition in artifact.family().domain().conditions() {
            request.retain_nonzero_guard(&condition.polynomial().clone().into(), limits)?;
        }
        let mut initial = Vec::new();
        for (target, coefficient) in terms {
            symbolica::utils::resources::checkpoint()?;
            reducer.validate_target(&target)?;
            artifact
                .coefficient_context()
                .validate_with_limits(&coefficient, limits.exact_algebra)?;
            request.retain_nonzero_guard(&coefficient.denominator.clone().into(), limits)?;
            initial
                .try_reserve(1)
                .map_err(|_| ReductionError::AllocationFailure {
                    resource: "non-memoizing depth-first input occurrences",
                    requested: initial.len().saturating_add(1),
                })?;
            initial.push((reducer.canonicalize(&target)?, coefficient));
        }
        let conditions = request.guarded_conditions.take().unwrap_or_default();
        request.guarded_conditions = Some(Vec::new());
        let output = depth_first_without_memoization::run(
            &mut ClosedKernel {
                reducer: &mut reducer,
                request: &mut request,
            },
            initial,
            conditions,
        )?;
        let mut work = ReductionStatistics::default();
        reducer.merge_work_statistics(&mut work);
        Ok(DepthFirstArtifactReduction {
            terms: output.terms,
            nonzero_conditions: output.nonzero_conditions,
            work,
        })
    }

    pub fn reduce_artifact_unit_mass_guarded_without_memoization(
        artifact: &'artifact ClosedArtifact,
        target: &IntegralKey,
        limits: ReductionLimits,
    ) -> Result<(MasterDecomposition, Vec<Coefficient>, ReductionStatistics), ReductionError> {
        let result = Self::reduce_artifact_linear_combination_without_memoization(
            artifact,
            [(target.clone(), artifact.coefficient_context().one())],
            limits,
        )?;
        Ok((
            MasterDecomposition::new(
                artifact.family_fingerprint_owner(),
                target.clone(),
                result.terms,
            ),
            result.nonzero_conditions,
            result.work,
        ))
    }

    pub(super) fn reduce_canonical_without_memoization(
        &mut self,
        target: &IntegralKey,
        request: &mut ReductionRequest,
    ) -> Result<MasterDecomposition, ReductionError> {
        let conditions = request.guarded_conditions.take().unwrap_or_default();
        request.guarded_conditions = Some(Vec::new());
        let initial = vec![(target.clone(), self.artifact.coefficient_context().one())];
        let output = depth_first_without_memoization::run(
            &mut ClosedKernel {
                reducer: self,
                request,
            },
            initial,
            conditions,
        )?;
        // A nested factorization shares the outer guard obligation buffer.
        // Move the accumulated union back without cloning or recharging it.
        request.guarded_conditions = Some(output.nonzero_conditions);
        Ok(MasterDecomposition::new(
            self.family_fingerprint.clone(),
            target.clone(),
            output.terms,
        ))
    }
}

impl DepthFirstKernel for ClosedKernel<'_, '_, '_> {
    type Key = IntegralKey;
    type Error = ReductionError;

    fn context(&self) -> &CoefficientContext {
        self.reducer.artifact.coefficient_context()
    }
    fn limits(&self) -> ReductionLimits {
        self.reducer.limits
    }
    fn request(&mut self) -> &mut ReductionRequest {
        self.request
    }
    fn cycle_error(&self, key: &IntegralKey) -> ReductionError {
        ReductionError::CycleDetected {
            target: key.clone(),
        }
    }
    fn record_work(&mut self, work: ReductionStatistics) {
        self.reducer.statistics.merge_work(work);
    }

    fn step(
        &mut self,
        key: &IntegralKey,
        work: &mut ReductionStatistics,
    ) -> Result<DepthFirstStep<IntegralKey>, ReductionError> {
        let mut step = DepthFirstStep::default();
        if self.reducer.artifact.masters().contains(key) {
            step.terminals.insert(key.clone(), self.context().one());
        } else if !self.reducer.artifact.is_zero_terminal(key) {
            self.request.record_rule_application(
                self.reducer
                    .limits
                    .diagnostic_limit(self.reducer.limits.max_rule_applications),
            )?;
            if let Some(selected) = self.reducer.select_first_factorization(key)? {
                self.reducer
                    .retain_factorization_guard(selected.ordinal, self.request)?;
                step.terminals = self
                    .reducer
                    .apply_factorization(&selected.routed_target, selected.ordinal, self.request)?
                    .into_terms();
            } else {
                let selected = self.reducer.select_first_rule(key)?;
                self.reducer
                    .retain_selected_rule_guards(&selected, self.request)?;
                // Preserve original RHS occurrence order, including duplicate
                // canonical keys: this is the old DFS without result reuse.
                step.terms = self
                    .reducer
                    .apply_selected_rule(key, selected, self.request)?;
            }
            work.record_rule_application();
        }
        step.nonzero_conditions = self.request.guarded_conditions.take().unwrap_or_default();
        self.request.guarded_conditions = Some(Vec::new());
        Ok(step)
    }
}

#[cfg(test)]
mod tests;
