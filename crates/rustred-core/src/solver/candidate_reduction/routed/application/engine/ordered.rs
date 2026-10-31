//! Saved owner/route adapter for the generic weighted-frontier engine.
use super::*;
use crate::algebra::CoefficientContext;
use crate::reduction::ordered_reduction::step_cache::codec::{NodeStage, owner_sector};
use crate::reduction::ordered_reduction::step_cache::kernel::{
    ConcurrentStepCacheKernel, OrderedStepCacheKernel, run_with_step_cache_and_options,
};
use crate::reduction::ordered_reduction::{
    self, ConcurrentOrderedKernel, OrderedKernel, OrderedKeyStage, OrderedPhaseDescriptor,
    OrderedPhaseEvent, OrderedPosition, OrderedStep, OrderedStepper,
};
use crate::reduction::{OrderedStepShard, OrderedStepStage};
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
}

fn ordering<const N: usize>(
    application: &RoutedCandidateApplication<N>,
    owner: &[bool; N],
) -> Result<OrderingPolicy, CandidateRoutedError> {
    let ordering = match &application.owners {
        super::super::ApplicationOwners::Eager(programs) => {
            programs.owners.get(owner).map(|program| &program.ordering)
        }
        super::super::ApplicationOwners::Deferred(programs) => programs.owner_ordering(owner),
    };
    ordering.cloned().ok_or_else(|| {
        CandidateRoutedError::InvalidInput("ordered owner has no authenticated ordering".into())
    })
}

fn position<const N: usize>(
    application: &RoutedCandidateApplication<N>,
    route_phases: &BTreeMap<usize, usize>,
    owner_phases: &BTreeMap<[bool; N], usize>,
    key: &Node<N>,
) -> Result<OrderedPosition<ComplexityKey>, CandidateRoutedError> {
    let (phase, ordering) = match key {
        Node::Route(target) => {
            let support = target.powers().iter().filter(|&&power| power > 0).count();
            (
                *route_phases.get(&support).ok_or_else(|| {
                    CandidateRoutedError::InvalidInput(
                        "ordered routing support is outside the family".into(),
                    )
                })?,
                OrderingPolicy::default(),
            )
        }
        Node::Apply(owner, _) => (
            *owner_phases.get(owner).ok_or_else(|| {
                CandidateRoutedError::InvalidInput(
                    "ordered owner is not in the frozen phase plan".into(),
                )
            })?,
            ordering(application, owner)?,
        ),
    };
    let priority = ordering
        .complexity_key(key.key().powers())
        .map_err(ReductionError::Ordering)?;
    Ok(OrderedPosition { phase, priority })
}

/// The evaluator that checks source conditions and zero sectors of any key.
fn base_evaluator<'a, const N: usize>(
    context: &'a crate::solver::candidate_reduction::CandidateOwnerContext<N>,
    ordering: &'a OrderingPolicy,
) -> CandidateEvaluator<'a, N> {
    let shared = &context.shared;
    CandidateEvaluator {
        context: &shared.context,
        root_sector: [true; N],
        ordering,
        rules: &[],
        whole_piece_alternatives: &[],
        source_conditions: &shared.source_conditions,
        zero_sectors: &shared.zero_sectors,
        limits: context.limits,
    }
}

/// The ordered step of a checked expansion. The checked step validated every
/// child and retained its source conditions. Expanding a zero-sector child
/// later would retain the same conditions and nothing else, so it is dropped
/// before its weight pays for frontier multiplications and merges.
fn ordered_step<const N: usize>(
    application: &RoutedCandidateApplication<N>,
    node: &Node<N>,
    result: CheckedExpansion<N>,
    conditions: Vec<Coefficient>,
    request: &mut Request<'_, '_, N>,
) -> OrderedStep<Node<N>> {
    let shared = &application.context.shared;
    let mut result = result;
    if let CheckedExpansion::Expanded(terms) = &mut result {
        let ordering = OrderingPolicy::default();
        let base = base_evaluator(&application.context, &ordering);
        terms.retain(|child, _| !base.is_zero(child.key()));
    }
    request.observe_activity(
        application,
        CandidateRoutedActivity::CoefficientCombination,
        match &result {
            CheckedExpansion::Expanded(terms) => Some(terms.len()),
            CheckedExpansion::Terminal(_) => Some(1),
        },
    );
    match result {
        CheckedExpansion::Expanded(terms) => OrderedStep {
            terms,
            nonzero_conditions: conditions,
            ..Default::default()
        },
        CheckedExpansion::Terminal(_) => OrderedStep {
            terminals: BTreeMap::from([(node.key().clone(), shared.context.base().one())]),
            nonzero_conditions: conditions,
            ..Default::default()
        },
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
        position(
            self.application,
            &self.route_phases,
            &self.owner_phases,
            key,
        )
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
        let ordering = OrderingPolicy::default();
        let base = base_evaluator(&context, &ordering);
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
        Ok(ordered_step(
            self.application,
            node,
            result?,
            conditions,
            self.request,
        ))
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

/// Route and Apply steps read only the immutable application: routes,
/// owner programs (prepared at most once, thread-safely) and the shared
/// context.
struct RoutedStepper<'a, const N: usize> {
    application: &'a RoutedCandidateApplication<N>,
    route_phases: &'a BTreeMap<usize, usize>,
    owner_phases: &'a BTreeMap<[bool; N], usize>,
    cancel: Option<&'a AtomicBool>,
    /// Record observation events for replay (the request is observed).
    record_events: bool,
}

/// Accounting of one detached Route/Apply step: the fresh request it ran on.
struct RoutedAccount {
    stats: CandidateRoutedApplicationStatistics,
    algebra: ReductionRequest,
    events: Vec<DetachedEvent>,
    /// The key passed validation and reached the checked expansion (only
    /// then does the sequential step publish its work to observers).
    checked: bool,
}

impl<const N: usize> OrderedStepper for RoutedStepper<'_, N> {
    type Key = Node<N>;
    type Priority = ComplexityKey;
    type Error = CandidateRoutedError;
    type Account = RoutedAccount;

    fn position(&self, key: &Node<N>) -> Result<OrderedPosition<ComplexityKey>, Self::Error> {
        position(self.application, self.route_phases, self.owner_phases, key)
    }

    /// The step of [`RoutedKernel`], on a fresh request; operational-node
    /// accounting is applied by `absorb`.
    fn step(&self, node: &Node<N>) -> (Result<OrderedStep<Node<N>>, Self::Error>, RoutedAccount) {
        let mut request = Request::<'_, '_, N> {
            cancel: self.cancel,
            detached_events: self.record_events.then(Vec::new),
            ..Default::default()
        };
        let mut checked = false;
        let result = (|| {
            request.check_cancel()?;
            let context = &self.application.context;
            let ordering = OrderingPolicy::default();
            let base = base_evaluator(&context, &ordering);
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
            checked = true;
            let result =
                self.application
                    .checked_expand_node(node, &mut conditions, &mut request)?;
            if let Some(events) = &mut request.detached_events {
                events.push(DetachedEvent::Expanded);
            }
            Ok(ordered_step(
                self.application,
                node,
                result,
                conditions,
                &mut request,
            ))
        })();
        let account = RoutedAccount {
            stats: request.stats,
            algebra: std::mem::take(&mut request.algebra),
            events: request.detached_events.take().unwrap_or_default(),
            checked,
        };
        (result, account)
    }

    /// Route steps emit Apply keys of owner phases after their Route phase
    /// and Route keys of smaller support; Apply steps emit Route keys of
    /// smaller support besides same-owner Apply keys.
    fn later_phase(&self, parent: &Node<N>, child: &Node<N>) -> bool {
        matches!(parent, Node::Route(_)) || matches!(child, Node::Route(_))
    }
}

impl<const N: usize> ConcurrentOrderedKernel for RoutedKernel<'_, '_, '_, '_, N> {
    type Account = RoutedAccount;
    type Stepper<'s>
        = RoutedStepper<'s, N>
    where
        Self: 's;

    /// Diagnostic limits are checked inside steps against aggregate counts,
    /// which only the sequential scheduler knows at that point.
    fn stepper(&self) -> Option<RoutedStepper<'_, N>> {
        if self.application.limits.diagnostics_enabled
            || self.application.context.limits.diagnostics_enabled
        {
            return None;
        }
        Some(RoutedStepper {
            application: self.application,
            route_phases: &self.route_phases,
            owner_phases: &self.owner_phases,
            cancel: self.request.cancel,
            record_events: self.request.observation.is_some(),
        })
    }

    /// The accounting and observations of [`OrderedKernel::step`], in its
    /// order: every replayed activity notice sees the transport counters and
    /// the published work that the sequential step shows at that point.
    fn absorb(
        &mut self,
        _: &Node<N>,
        account: RoutedAccount,
        request: &mut ReductionRequest,
        work: &mut ReductionStatistics,
    ) -> Result<(), Self::Error> {
        self.request.check_cancel()?;
        let limits = self.application.limits;
        charge(
            &mut self.request.stats.operational_nodes,
            1,
            limits.diagnostic_limit(limits.max_unique_nodes),
            "operational nodes",
        )?;
        if !account.checked {
            // Rejected and zero-sector keys return before the checked
            // expansion, which is where their work would be published.
            request.absorb_detached(&account.algebra, self.application.context.limits)?;
            work.merge_work(account.stats.work);
            return Ok(());
        }
        let mut live = self.request.stats.work;
        live.merge_work(*work);
        self.request.ordered_live_work = Some(live);
        let base = [
            self.request.stats.transport_calls,
            self.request.stats.transport_operations,
            self.request.stats.transport_endpoints,
        ];
        let mut published = false;
        let publish = |kernel: &mut Self,
                       request: &mut ReductionRequest,
                       work: &mut ReductionStatistics|
         -> Result<(), CandidateRoutedError> {
            request.absorb_detached(&account.algebra, kernel.application.context.limits)?;
            work.merge_work(account.stats.work);
            let mut live = kernel.request.stats.work;
            live.merge_work(*work);
            kernel.request.ordered_live_work = Some(live);
            Ok(())
        };
        for event in account.events {
            match event {
                DetachedEvent::Activity(activity, terms, counters) => {
                    let stats = &mut self.request.stats;
                    stats.transport_calls = base[0].saturating_add(counters[0]);
                    stats.transport_operations = base[1].saturating_add(counters[1]);
                    stats.transport_endpoints = base[2].saturating_add(counters[2]);
                    self.request
                        .observe_activity(self.application, activity, terms)
                }
                DetachedEvent::LocalDescent => self.request.record_local_descent(),
                DetachedEvent::Expanded => {
                    publish(self, request, work)?;
                    published = true;
                }
            }
        }
        let stats = &mut self.request.stats;
        stats.transport_calls = base[0];
        charge(
            &mut stats.transport_calls,
            account.stats.transport_calls,
            limits.diagnostic_limit(limits.max_transport_calls),
            "transport calls",
        )?;
        stats.transport_operations = base[1].saturating_add(account.stats.transport_operations);
        stats.transport_endpoints = base[2].saturating_add(account.stats.transport_endpoints);
        if !published {
            publish(self, request, work)?;
        }
        Ok(())
    }
}

fn mask_bits<const N: usize>(mask: &[bool; N]) -> u64 {
    mask.iter()
        .enumerate()
        .filter(|&(_, &bit)| bit)
        .fold(0, |bits, (position, _)| {
            bits | 1u64.checked_shl(position as u32).unwrap_or(0)
        })
}

/// Route records are sharded by the owner their transport reaches (the
/// support itself when it is an owner), Apply records by their owner.
fn step_cache_shard<const N: usize>(
    application: &RoutedCandidateApplication<N>,
    node: &Node<N>,
) -> Option<OrderedStepShard> {
    if N > 64 {
        return None;
    }
    let (stage, class) = match node {
        Node::Route(key) => {
            let sector = support::<N>(key);
            let class = if application.owners.contains(&sector) {
                mask_bits(&sector)
            } else {
                application.routes.get(&sector).map_or(0, |route| {
                    route
                        .owner_sector
                        .active_bits()
                        .iter()
                        .enumerate()
                        .filter(|&(_, &bit)| bit)
                        .fold(0, |bits, (position, _)| bits | 1u64 << position)
                })
            };
            (OrderedStepStage::Route, class)
        }
        Node::Apply(owner, _) => (OrderedStepStage::Apply, mask_bits(owner)),
    };
    Some(OrderedStepShard {
        stage,
        support: support::<N>(node.key()).iter().filter(|&&bit| bit).count() as u16,
        class,
    })
}

/// Rebuild a recorded Route/Apply identity of this application.
fn step_cache_key<const N: usize>(
    application: &RoutedCandidateApplication<N>,
    stage: NodeStage,
    integral: IntegralKey,
) -> Result<Node<N>, CandidateRoutedError> {
    if integral.powers().len() != application.family().denominator_count() {
        return Err(ordered_reduction::step_cache::failure(
            "a recorded identity has the wrong arity",
        )
        .into());
    }
    match stage {
        NodeStage::Route => Ok(Node::Route(integral)),
        NodeStage::Apply(mask) => owner_sector::<N>(mask)
            .filter(|owner| application.owners.contains(owner))
            .map(|owner| Node::Apply(owner, integral))
            .ok_or_else(|| {
                ordered_reduction::step_cache::failure(
                    "a recorded Apply identity names an absent owner",
                )
                .into()
            }),
        NodeStage::Integral => Err(ordered_reduction::step_cache::failure(
            "a recorded saved-application child is not a Route or Apply identity",
        )
        .into()),
    }
}

impl<const N: usize> OrderedStepCacheKernel for RoutedKernel<'_, '_, '_, '_, N> {
    fn step_cache_shard(&mut self, node: &Node<N>) -> Option<OrderedStepShard> {
        step_cache_shard(self.application, node)
    }

    fn step_cache_key(
        &self,
        stage: NodeStage,
        integral: IntegralKey,
    ) -> Result<Node<N>, Self::Error> {
        step_cache_key(self.application, stage, integral)
    }

    /// The cancellation check and operational-node charge of a checked step.
    fn step_cache_hit(&mut self, _node: &Node<N>) -> Result<(), Self::Error> {
        self.request.check_cancel()?;
        charge(
            &mut self.request.stats.operational_nodes,
            1,
            self.application
                .limits
                .diagnostic_limit(self.application.limits.max_unique_nodes),
            "operational nodes",
        )
    }
}

/// The worker-side cache hooks read the same immutable application.
impl<const N: usize> ConcurrentStepCacheKernel for RoutedKernel<'_, '_, '_, '_, N> {
    fn stepper_cache_shard(
        stepper: &RoutedStepper<'_, N>,
        node: &Node<N>,
    ) -> Option<OrderedStepShard> {
        step_cache_shard(stepper.application, node)
    }

    fn stepper_cache_key(
        stepper: &RoutedStepper<'_, N>,
        stage: NodeStage,
        integral: IntegralKey,
    ) -> Result<Node<N>, CandidateRoutedError> {
        step_cache_key(stepper.application, stage, integral)
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
        let arithmetic = self.ordered_weight_arithmetic;
        let scheduler = self.ordered_scheduler;
        let step_cache = self.ordered_step_cache.clone();
        if let Some(cache) = &step_cache {
            self.check_ordered_step_cache(cache)?;
        }
        let result = run_with_step_cache_and_options(
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
            arithmetic,
            scheduler,
            step_cache.as_deref(),
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
