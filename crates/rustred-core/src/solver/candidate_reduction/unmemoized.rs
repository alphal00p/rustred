//! Occurrence-local DFS over the same checked candidate rules and terminal rows.

use std::collections::BTreeMap;
use std::sync::Arc;

use super::{CandidateDecomposition, CandidateReducer, CandidateReductionError, UnresolvedPolicy};
use crate::algebra::{Coefficient, CoefficientContext};
use crate::family::IntegralKey;
use crate::reduction::depth_first_without_memoization::{self, DepthFirstKernel, DepthFirstStep};
use crate::reduction::{ReductionError, ReductionLimits, ReductionRequest, ReductionStatistics};

struct Kernel<'a, const N: usize> {
    reducer: &'a CandidateReducer<N>,
    limits: ReductionLimits,
    request: ReductionRequest,
}

impl<const N: usize> DepthFirstKernel for Kernel<'_, N> {
    type Key = IntegralKey;
    type Error = CandidateReductionError;

    fn context(&self) -> &CoefficientContext {
        self.reducer.program.context.base()
    }

    fn limits(&self) -> ReductionLimits {
        self.limits
    }

    fn request(&mut self) -> &mut ReductionRequest {
        &mut self.request
    }

    fn cycle_error(&self, key: &IntegralKey) -> Self::Error {
        ReductionError::CycleDetected {
            target: key.clone(),
        }
        .into()
    }

    fn step(
        &mut self,
        key: &IntegralKey,
        work: &mut ReductionStatistics,
    ) -> Result<DepthFirstStep<IntegralKey>, Self::Error> {
        let mut conditions = self
            .reducer
            .target_conditions_with_limits(key, self.limits)?;
        if self.reducer.is_zero(key) {
            return Ok(DepthFirstStep {
                nonzero_conditions: conditions,
                ..Default::default()
            });
        }
        if self.reducer.program.terminals.contains(key) {
            let terminals = self.reducer.terminal_terms(key)?;
            // Preserve the recorded terminal row's original poles before any
            // parent multiplication or cancellation, just as guarded DFS does.
            for coefficient in terminals.values() {
                if !coefficient.denominator.is_constant() {
                    conditions.push(coefficient.denominator.clone().into());
                }
            }
            return Ok(DepthFirstStep {
                terminals,
                nonzero_conditions: conditions,
                ..Default::default()
            });
        }
        let evaluator = match self.reducer.program.evaluator(key, self.limits) {
            Ok(evaluator) => evaluator,
            Err(CandidateReductionError::Uncovered { .. })
                if self.reducer.unresolved_policy == UnresolvedPolicy::Preserve =>
            {
                return Ok(DepthFirstStep {
                    terminals: BTreeMap::from([(key.clone(), self.context().one())]),
                    nonzero_conditions: conditions,
                    ..Default::default()
                });
            }
            Err(error) => return Err(error),
        };
        let selected = if self.reducer.unresolved_policy == UnresolvedPolicy::Preserve {
            match evaluator.selected_rule(key) {
                Ok(selected) => Some(selected),
                Err(CandidateReductionError::Uncovered { .. }) => {
                    return Ok(DepthFirstStep {
                        terminals: BTreeMap::from([(key.clone(), self.context().one())]),
                        nonzero_conditions: conditions,
                        ..Default::default()
                    });
                }
                Err(error) => return Err(error),
            }
        } else {
            None
        };
        self.request.record_rule_application(
            self.limits
                .diagnostic_limit(self.limits.max_rule_applications),
        )?;
        let (terms, local) = match selected {
            Some((baseline, rule)) => evaluator.apply_selected_with_conditions(
                key,
                baseline,
                rule,
                &mut self.request,
                work,
            )?,
            None => evaluator.apply_with_conditions(key, &mut self.request, work)?,
        };
        work.record_rule_application();
        conditions.extend(local);
        Ok(DepthFirstStep {
            // The ordinary candidate DFS visits the coalesced rule children in
            // BTreeMap order. Preserve it, without a completed-key lookup.
            terms: terms.into_iter().collect(),
            nonzero_conditions: conditions,
            ..Default::default()
        })
    }
}

impl<const N: usize> CandidateReducer<N> {
    /// Apply one guarded root without reading, creating or accounting completed
    /// results. Only the shared walker's active frames own returned child values.
    /// Existing memoized results, counters and backing/capture state are untouched.
    pub fn reduce_unit_mass_without_memoization_guarded(
        &self,
        target: &IntegralKey,
        remaining: ReductionLimits,
    ) -> Result<
        (
            CandidateDecomposition,
            Vec<Coefficient>,
            ReductionStatistics,
        ),
        CandidateReductionError,
    > {
        self.reduce_unit_mass_without_memoization_with_limits(
            target,
            self.limits.bounded_by(remaining),
        )
    }

    pub(super) fn reduce_unit_mass_without_memoization_with_limits(
        &self,
        target: &IntegralKey,
        limits: ReductionLimits,
    ) -> Result<
        (
            CandidateDecomposition,
            Vec<Coefficient>,
            ReductionStatistics,
        ),
        CandidateReductionError,
    > {
        self.validate_entry_with_limits(target, limits)?;
        let mut kernel = Kernel {
            reducer: self,
            limits,
            request: ReductionRequest::default(),
        };
        let result = depth_first_without_memoization::run(
            &mut kernel,
            vec![(target.clone(), self.program.context.base().one())],
            Vec::new(),
        )?;
        for coefficient in result.terms.values().chain(&result.nonzero_conditions) {
            self.program
                .context
                .base()
                .validate_with_limits(coefficient, limits.exact_algebra)?;
        }
        let unresolved = result
            .terms
            .keys()
            .filter(|key| !self.canonical_terminals().contains(*key))
            .cloned()
            .collect();
        Ok((
            CandidateDecomposition {
                family_fingerprint: Arc::clone(&self.program.family_fingerprint),
                target: target.clone(),
                terms: result.terms,
                unresolved,
            },
            result.nonzero_conditions,
            result.work,
        ))
    }
}
