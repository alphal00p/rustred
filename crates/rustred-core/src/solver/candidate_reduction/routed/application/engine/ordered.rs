//! Saved owner/route adapter for the generic weighted-frontier engine.
use super::*;
use crate::algebra::CoefficientContext;
use crate::reduction::ordered_reduction::{
    self, OrderedKernel, OrderedKeyStage, OrderedPhaseDescriptor, OrderedPhaseEvent,
    OrderedPosition, OrderedStep,
};
use crate::reduction::{ReductionLimits, ReductionStrategy};
use crate::sector::{ComplexityKey, OrderingPolicy};
use crate::solver::{CandidateOrderedPhaseKind, CandidateOrderedReductionProgress};

#[derive(Clone, Copy)]
struct Phase<const N: usize> {
    support: usize,
    owner: Option<[bool; N]>,
    owner_index: Option<usize>,
}

struct RoutedKernel<'app, 'req, 'observer, 'storage, const N: usize> {
    application: &'app mut RoutedCandidateApplication<N>,
    request: &'req mut Request<'observer, 'storage, N>,
    phases: Vec<Phase<N>>,
    route_phases: BTreeMap<usize, usize>,
    owner_phases: BTreeMap<[bool; N], usize>,
}

impl<const N: usize> RoutedKernel<'_, '_, '_, '_, N> {
    fn phases(application: &RoutedCandidateApplication<N>) -> Vec<Phase<N>> {
        let owners: Vec<_> = match &application.owners {
            super::super::ApplicationOwners::Eager(programs) => {
                programs.owner_sectors().copied().collect()
            }
            super::super::ApplicationOwners::Deferred(programs) => {
                programs.owner_sectors().copied().collect()
            }
        };
        let mut phases = Vec::new();
        for support in (0..=application.family().denominator_count()).rev() {
            phases.push(Phase {
                support,
                owner: None,
                owner_index: None,
            });
            for (owner_index, &owner) in owners.iter().enumerate() {
                if owner.iter().filter(|&&bit| bit).count() == support {
                    phases.push(Phase {
                        support,
                        owner: Some(owner),
                        owner_index: Some(owner_index),
                    });
                }
            }
        }
        phases
    }

    fn ordering(&self, owner: &[bool; N]) -> Result<OrderingPolicy, CandidateRoutedError> {
        let ordering = match &self.application.owners {
            super::super::ApplicationOwners::Eager(programs) => {
                programs.owners.get(owner).map(|program| &program.ordering)
            }
            super::super::ApplicationOwners::Deferred(programs) => programs.owner_ordering(owner),
        };
        ordering.cloned().ok_or_else(|| {
            CandidateRoutedError::InvalidInput("ordered owner has no authenticated ordering".into())
        })
    }
}

impl<const N: usize> OrderedKernel for RoutedKernel<'_, '_, '_, '_, N> {
    type Key = Node<N>;
    type Priority = ComplexityKey;
    type Error = CandidateRoutedError;

    fn context(&self) -> &CoefficientContext {
        self.application.context.shared.context.base()
    }
    fn limits(&self) -> ReductionLimits {
        self.application.context.limits
    }
    fn phase_count(&self) -> usize {
        self.phases.len()
    }
    fn position(&mut self, key: &Node<N>) -> Result<OrderedPosition<ComplexityKey>, Self::Error> {
        let (phase, ordering) = match key {
            Node::Route(target) => {
                let support = target.powers().iter().filter(|&&power| power > 0).count();
                (
                    *self.route_phases.get(&support).ok_or_else(|| {
                        CandidateRoutedError::InvalidInput(
                            "ordered routing support is outside the family".into(),
                        )
                    })?,
                    OrderingPolicy::default(),
                )
            }
            Node::Apply(owner, _) => (
                *self.owner_phases.get(owner).ok_or_else(|| {
                    CandidateRoutedError::InvalidInput(
                        "ordered owner is not in the frozen phase plan".into(),
                    )
                })?,
                self.ordering(owner)?,
            ),
        };
        let priority = ordering
            .complexity_key(key.key().powers())
            .map_err(ReductionError::Ordering)?;
        Ok(OrderedPosition { phase, priority })
    }

    fn step(
        &mut self,
        node: &Node<N>,
        request: &mut ReductionRequest,
        work: &mut ReductionStatistics,
    ) -> Result<OrderedStep<Node<N>>, Self::Error> {
        self.request.check_cancel()?;
        charge(
            &mut self.request.stats.operational_nodes,
            1,
            self.application
                .limits
                .diagnostic_limit(self.application.limits.max_unique_nodes),
            "operational nodes",
        )?;
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
            return Ok(OrderedStep {
                nonzero_conditions: conditions,
                ..Default::default()
            });
        }
        // The shared step retains exactly the same rule/transport accounting as
        // DFS. The ordered request does not read or create completed records.
        let mut live = self.request.stats.work;
        live.merge_work(*work);
        self.request.ordered_live_work = Some(live);
        std::mem::swap(request, &mut self.request.algebra);
        std::mem::swap(work, &mut self.request.stats.work);
        let result = self
            .application
            .checked_expand_node(node, &mut conditions, self.request);
        std::mem::swap(request, &mut self.request.algebra);
        std::mem::swap(work, &mut self.request.stats.work);
        let mut live = self.request.stats.work;
        live.merge_work(*work);
        self.request.ordered_live_work = Some(live);
        let result = result?;
        self.request.observe_activity(
            self.application,
            CandidateRoutedActivity::CoefficientCombination,
            match &result {
                CheckedExpansion::Expanded(terms) => Some(terms.len()),
                CheckedExpansion::Terminal(_) => Some(1),
            },
        );
        match result {
            CheckedExpansion::Expanded(terms) => Ok(OrderedStep {
                terms,
                nonzero_conditions: conditions,
                ..Default::default()
            }),
            CheckedExpansion::Terminal(_) => Ok(OrderedStep {
                terminals: BTreeMap::from([(node.key().clone(), shared.context.base().one())]),
                nonzero_conditions: conditions,
                ..Default::default()
            }),
        }
    }

    fn observe(
        &mut self,
        progress: &CandidateOrderedReductionProgress,
        current: Option<&Node<N>>,
    ) -> Result<(), Self::Error> {
        self.request.check_cancel()?;
        let mut progress = *progress;
        let phase = self.phases.get(progress.completed_phases);
        progress.active_support = phase.map(|phase| phase.support);
        progress.phase_kind = if phase.is_some_and(|phase| phase.owner.is_some()) {
            CandidateOrderedPhaseKind::Reduction
        } else {
            CandidateOrderedPhaseKind::Routing
        };
        progress.owner_index = phase.and_then(|phase| phase.owner_index);
        let changed_phase = self
            .request
            .ordered
            .is_none_or(|previous| previous.completed_phases != progress.completed_phases);
        self.request.ordered = Some(progress);
        if let Some(node) = current {
            self.request.set_current(node);
        } else if let Some(observation) = &mut self.request.observation {
            observation.current = None;
        }
        if let Some(observation) = &mut self.request.observation {
            observation.metrics.completed_unique_operational_nodes = progress.processed_keys;
            observation.metrics.completion_attempts = progress.processed_keys;
            observation.metrics.output_terms = Some(progress.terminal_keys);
            observation.stored_state.unstarted_roots = None;
            observation.stored_state.collected_output_integrals = Some(progress.terminal_keys);
        }
        self.request.observe(
            self.application,
            Some(CandidateRoutedApplicationPhase::Expansion),
            CandidateRoutedApplicationStatus::Running,
            changed_phase,
        );
        Ok(())
    }

    fn record_work(&mut self, work: ReductionStatistics) {
        let mut live = self.request.stats.work;
        live.merge_work(work);
        self.request.ordered_live_work = Some(live);
    }

    fn key_view(node: &Node<N>) -> Option<(OrderedKeyStage<'_>, &IntegralKey)> {
        Some(match node {
            Node::Route(key) => (OrderedKeyStage::Route, key),
            Node::Apply(owner, key) => (
                OrderedKeyStage::Apply {
                    owner_sector: owner,
                },
                key,
            ),
        })
    }

    fn inspect_phase(&mut self, event: OrderedPhaseEvent<'_>) -> Result<(), Self::Error> {
        let Some(inspector) = self.request.ordered_inspector.as_deref_mut() else {
            return Ok(());
        };
        let phase = &self.phases[event.phase];
        inspector(&event.with_descriptor(OrderedPhaseDescriptor {
            kind: if phase.owner.is_some() {
                CandidateOrderedPhaseKind::Reduction
            } else {
                CandidateOrderedPhaseKind::Routing
            },
            support: Some(phase.support),
            owner_index: phase.owner_index,
            owner_sector: phase.owner.as_ref().map(|owner| owner.as_slice()),
        }))
    }
}

impl<const N: usize> RoutedCandidateApplication<N> {
    pub(super) fn reduce_ordered_request(
        &mut self,
        terms: impl IntoIterator<Item = (IntegralKey, Coefficient)>,
        admission: CandidateEntryAdmission<'_, N>,
        request: &mut Request<'_, '_, N>,
        count_inputs: bool,
    ) -> Result<CandidateRoutedReduction, CandidateRoutedError> {
        debug_assert_eq!(self.reduction_strategy, ReductionStrategy::Ordered);
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
        request.stats.requested_targets = entries.len();
        let initial = entries
            .into_iter()
            .map(|(key, coefficient)| (Node::Route(key), coefficient))
            .collect();
        let phases = RoutedKernel::phases(self);
        let route_phases = phases
            .iter()
            .enumerate()
            .filter_map(|(i, phase)| phase.owner.is_none().then_some((phase.support, i)))
            .collect();
        let owner_phases = phases
            .iter()
            .enumerate()
            .filter_map(|(i, phase)| phase.owner.map(|owner| (owner, i)))
            .collect();
        let mut algebra = std::mem::take(&mut request.algebra);
        let result = ordered_reduction::run(
            &mut RoutedKernel {
                application: self,
                request,
                phases,
                route_phases,
                owner_phases,
            },
            &mut algebra,
            initial,
            conditions,
        );
        request.algebra = algebra;
        let output = match result {
            Ok(output) => output,
            Err(error) => {
                if let Some(work) = request.ordered_live_work {
                    request.stats.work = work;
                }
                return Err(error);
            }
        };
        request.stats.work.merge_work(output.work);
        request.ordered_live_work = Some(request.stats.work);
        let mut unresolved = BTreeSet::new();
        for key in output.terms.keys() {
            if !self.terminal_is_declared(key)? {
                unresolved.insert(key.clone());
            }
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
