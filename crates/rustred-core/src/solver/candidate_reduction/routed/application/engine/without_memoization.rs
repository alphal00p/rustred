//! Occurrence-local Route/Apply DFS using the ordinary checked transition kernel.
//!
//! No completed node survives its parent's back-substitution. In particular,
//! observation never creates a seen-node history, dependency pins or cache census.

use super::*;
use crate::algebra::CoefficientContext;
use crate::reduction::depth_first_without_memoization::{self, DepthFirstKernel, DepthFirstStep};
use crate::reduction::{ReductionLimits, ReductionStrategy};

struct RoutedKernel<'app, 'req, 'observer, 'storage, const N: usize> {
    application: &'app mut RoutedCandidateApplication<N>,
    request: &'req mut Request<'observer, 'storage, N>,
    initial_work: ReductionStatistics,
}

impl<const N: usize> DepthFirstKernel for RoutedKernel<'_, '_, '_, '_, N> {
    type Key = Node<N>;
    type Error = CandidateRoutedError;

    fn context(&self) -> &CoefficientContext {
        self.application.context.shared.context.base()
    }

    fn limits(&self) -> ReductionLimits {
        self.application.context.limits
    }

    fn request(&mut self) -> &mut ReductionRequest {
        &mut self.request.algebra
    }

    fn check_cancel(&mut self) -> Result<(), Self::Error> {
        self.request.check_cancel()
    }

    fn cycle_error(&self, node: &Node<N>) -> Self::Error {
        CandidateRoutedError::Cycle {
            target: node.key().clone(),
        }
    }

    fn step(
        &mut self,
        node: &Node<N>,
        work: &mut ReductionStatistics,
    ) -> Result<DepthFirstStep<Node<N>>, Self::Error> {
        self.request.check_cancel()?;
        charge(
            &mut self.request.stats.operational_nodes,
            1,
            // No historical-key census is retained. A unique-node diagnostic
            // is unavailable here and must not become a visit-count limit.
            // True rule-application and active-frame limits remain checked.
            None,
            "operational occurrences",
        )?;
        if self.request.algebra.pending_frame_count() == 1 {
            self.request.set_root(node);
        } else {
            self.request.set_current(node);
        }
        self.request.observe(
            self.application,
            Some(CandidateRoutedApplicationPhase::Expansion),
            CandidateRoutedApplicationStatus::Running,
            false,
        );
        let context = self.application.context.clone();
        let shared = &context.shared;
        let base = CandidateEvaluator {
            context: &shared.context,
            root_sector: [true; N],
            ordering: &Default::default(),
            rules: &[],
            whole_piece_alternatives: &[],
            source_conditions: &shared.source_conditions,
            zero_sectors: &shared.zero_sectors,
            limits: context.limits,
        };
        let mut conditions = Vec::new();
        base.validate_target_with_conditions(node.key(), &mut |condition| {
            retain_polynomial(&mut conditions, condition)
        })?;
        if base.is_zero(node.key()) {
            return Ok(DepthFirstStep {
                nonzero_conditions: conditions,
                ..Default::default()
            });
        }

        // The native checked kernel writes work into the request. Temporarily
        // lend it this walk's counter, while retaining input/coalescing work
        // separately. No completed-result accounting is involved.
        std::mem::swap(work, &mut self.request.stats.work);
        let result = self
            .application
            .checked_expand_node(node, &mut conditions, self.request);
        std::mem::swap(work, &mut self.request.stats.work);
        let mut live = self.initial_work;
        live.merge_work(*work);
        self.request.ordered_live_work = Some(live);
        match result? {
            CheckedExpansion::Expanded(terms) => Ok(DepthFirstStep {
                // Existing DFS visits the checked map in increasing key order.
                // There is no new cross-branch coalescing or completed lookup.
                terms: terms.into_iter().collect(),
                nonzero_conditions: conditions,
                ..Default::default()
            }),
            CheckedExpansion::Terminal(_) => Ok(DepthFirstStep {
                terminals: BTreeMap::from([(node.key().clone(), shared.context.base().one())]),
                nonzero_conditions: conditions,
                ..Default::default()
            }),
        }
    }

    fn completed(&mut self, node: &Node<N>, terminal_keys: usize) -> Result<(), Self::Error> {
        self.request.set_current(node);
        if let Some(observation) = &mut self.request.observation {
            observation.metrics.completion_attempts =
                observation.metrics.completion_attempts.saturating_add(1);
            // Uniqueness is intentionally not tracked in this engine.
            observation.complexity.terms = Some(terminal_keys);
            if self.request.algebra.pending_frame_count() == 0 {
                observation.metrics.completed_roots =
                    observation.metrics.completed_roots.saturating_add(1);
                observation.stored_state.unstarted_roots = observation
                    .stored_state
                    .unstarted_roots
                    .and_then(|count| count.checked_sub(1));
            }
        }
        self.request.observe(
            self.application,
            Some(CandidateRoutedApplicationPhase::Combination),
            CandidateRoutedApplicationStatus::Running,
            false,
        );
        Ok(())
    }

    fn record_work(&mut self, work: ReductionStatistics) {
        self.request.stats.work = self.initial_work;
        self.request.stats.work.merge_work(work);
        self.request.ordered_live_work = Some(self.request.stats.work);
    }
}

impl<const N: usize> RoutedCandidateApplication<N> {
    /// Applies to both direct and initially-routed entry paths, including the
    /// legacy persistent API that enters without the observed-request wrapper.
    pub(super) fn validate_request_cache_strategy(
        &self,
        request: &Request<'_, '_, N>,
    ) -> Result<(), CandidateRoutedError> {
        if !self.reduction_strategy.uses_completed_cache()
            && (request.persistent.backing.is_some() || request.persistent.capture.is_some())
        {
            return Err(CandidateRoutedError::InvalidInput(
                "selected reduction strategy does not support depth-first result caches or capture"
                    .into(),
            ));
        }
        Ok(())
    }

    pub(super) fn reduce_without_memoization_request(
        &mut self,
        terms: impl IntoIterator<Item = (IntegralKey, Coefficient)>,
        admission: CandidateEntryAdmission<'_, N>,
        request: &mut Request<'_, '_, N>,
        count_inputs: bool,
    ) -> Result<CandidateRoutedReduction, CandidateRoutedError> {
        debug_assert_eq!(
            self.reduction_strategy,
            ReductionStrategy::DepthFirstWithoutMemoization
        );
        let context = self.context.clone();
        let shared = &context.shared;
        let limits = context.limits;
        let base = CandidateEvaluator {
            context: &shared.context,
            root_sector: [true; N],
            ordering: &Default::default(),
            rules: &[],
            whole_piece_alternatives: &[],
            source_conditions: &shared.source_conditions,
            zero_sectors: &shared.zero_sectors,
            limits,
        };
        let mut entries = BTreeMap::new();
        let mut conditions = Vec::new();
        for (target, coefficient) in terms {
            request.set_entry_root(&target);
            if count_inputs {
                charge(
                    &mut request.stats.input_terms,
                    1,
                    self.limits.diagnostic_limit(self.limits.max_input_targets),
                    "input terms",
                )?;
            }
            base.validate_target_with_conditions(&target, &mut |condition| {
                retain_polynomial(&mut conditions, condition)
            })?;
            self.admit_entry(admission, &target)?;
            shared
                .context
                .base()
                .validate_with_limits(&coefficient, limits.exact_algebra)
                .map_err(CandidateReductionError::from)?;
            retain_polynomial(&mut conditions, &coefficient.denominator);
            // Preserve ordinary DFS's input coalescing and validation order.
            accumulate_master_in_request(
                shared.context.base(),
                &mut entries,
                &target,
                coefficient,
                limits,
                &mut request.algebra,
                &mut request.stats.work,
            )?;
        }
        request.check_cancel()?;
        request.stats.requested_targets = entries.len();
        if let Some(observation) = &mut request.observation {
            observation.metrics.requested_roots = Some(entries.len());
            observation.stored_state.unstarted_roots = Some(entries.len());
            observation.stored_state.collected_output_integrals = Some(0);
            observation.metrics.output_terms = Some(0);
        }
        let initial = entries
            .into_iter()
            .map(|(key, weight)| (Node::Route(key), weight))
            .collect();
        let initial_work = request.stats.work;
        let output = depth_first_without_memoization::run(
            &mut RoutedKernel {
                application: self,
                request,
                initial_work,
            },
            initial,
            conditions,
        )?;
        let mut unresolved = BTreeSet::new();
        for key in output.terms.keys() {
            if !self.terminal_is_declared(key)? {
                unresolved.insert(key.clone());
            }
        }
        if let Some(observation) = &mut request.observation {
            observation.metrics.output_terms = Some(output.terms.len());
            observation.stored_state.collected_output_integrals = Some(output.terms.len());
        }
        Ok(CandidateRoutedReduction {
            family_fingerprint: context.family.fingerprint_owner(),
            terms: output.terms,
            unresolved,
            nonzero_conditions: output.nonzero_conditions,
            statistics: request.stats,
        })
    }
}

#[cfg(test)]
mod tests;
