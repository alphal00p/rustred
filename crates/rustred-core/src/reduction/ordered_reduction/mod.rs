//! Global, phase-ordered reduction of exact weighted integral frontiers.
//!
//! This engine owns no completed-result cache. Artifact adapters retain the
//! existing checked mathematical operations and supply a certified descending
//! position; this module only schedules and combines their weighted results.
use std::cmp::Reverse;
use std::collections::BTreeMap;

use crate::algebra::{Coefficient, CoefficientContext};
use crate::family::IntegralKey;
use crate::reduction::guard_union::GuardIndex;
use crate::reduction::{ReductionError, ReductionLimits, ReductionRequest, ReductionStatistics};
use crate::solver::CandidateOrderedReductionProgress;

pub(crate) mod inspection;
use inspection::OrderedStateView;
pub(crate) use inspection::{
    OrderedFrontierEntry, OrderedKeyStage, OrderedPhaseBoundary, OrderedPhaseDescriptor,
    OrderedPhaseEvent, OrderedPhaseInspector,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OrderedPosition<P> {
    /// Zero-based phase in execution order. This total is frozen before a call.
    pub phase: usize,
    /// Larger values must be reduced first within one phase.
    pub priority: P,
}

/// One mathematically checked action. A terminal output is complete for this
/// adapter; it is never placed back on the unreduced frontier.
pub(crate) struct OrderedStep<K> {
    pub terms: BTreeMap<K, Coefficient>,
    pub terminals: BTreeMap<IntegralKey, Coefficient>,
    pub nonzero_conditions: Vec<Coefficient>,
}

impl<K> Default for OrderedStep<K> {
    fn default() -> Self {
        Self {
            terms: BTreeMap::new(),
            terminals: BTreeMap::new(),
            nonzero_conditions: vec![],
        }
    }
}

pub(crate) trait OrderedKernel {
    type Key: Clone + Ord;
    type Priority: Clone + Ord;
    type Error: From<ReductionError>;

    fn context(&self) -> &CoefficientContext;
    fn limits(&self) -> ReductionLimits;
    fn phase_count(&self) -> usize;
    fn position(&mut self, key: &Self::Key)
    -> Result<OrderedPosition<Self::Priority>, Self::Error>;
    fn step(
        &mut self,
        key: &Self::Key,
        request: &mut ReductionRequest,
        work: &mut ReductionStatistics,
    ) -> Result<OrderedStep<Self::Key>, Self::Error>;
    fn observe(
        &mut self,
        _progress: &CandidateOrderedReductionProgress,
        _current: Option<&Self::Key>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Retain attempted work even when a checked action or arithmetic fails.
    fn record_work(&mut self, _work: ReductionStatistics) {}
    /// Public view of one scheduled identity for phase inspection.
    fn key_view(_key: &Self::Key) -> Option<(OrderedKeyStage<'_>, &IntegralKey)> {
        None
    }
    /// Called at both boundaries of every frozen phase, never inside a step.
    /// Adapters without a caller inspector keep this no-op.
    fn inspect_phase(&mut self, _event: OrderedPhaseEvent<'_>) -> Result<(), Self::Error> {
        Ok(())
    }
}

pub(crate) struct OrderedOutput {
    pub terms: BTreeMap<IntegralKey, Coefficient>,
    pub nonzero_conditions: Vec<Coefficient>,
    pub work: ReductionStatistics,
    pub progress: CandidateOrderedReductionProgress,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Scheduled<K, P> {
    phase: Reverse<usize>,
    priority: P,
    key: K,
}

type Frontier<K> =
    BTreeMap<Scheduled<<K as OrderedKernel>::Key, <K as OrderedKernel>::Priority>, Coefficient>;

struct SchedulerState<'a, K: OrderedKernel> {
    frontier: &'a Frontier<K>,
    terminals: &'a BTreeMap<IntegralKey, Coefficient>,
    nonzero_conditions: &'a [Coefficient],
}

impl<K: OrderedKernel> OrderedStateView for SchedulerState<'_, K> {
    fn frontier(&self) -> Box<dyn Iterator<Item = OrderedFrontierEntry<'_>> + '_> {
        Box::new(
            self.frontier
                .iter()
                .rev()
                .filter_map(|(scheduled, weight)| {
                    let (stage, integral) = K::key_view(&scheduled.key)?;
                    Some(OrderedFrontierEntry {
                        phase: scheduled.phase.0,
                        stage,
                        integral,
                        weight,
                    })
                }),
        )
    }
    fn terminals(&self) -> &BTreeMap<IntegralKey, Coefficient> {
        self.terminals
    }
    fn nonzero_conditions(&self) -> &[Coefficient] {
        self.nonzero_conditions
    }
}

fn inspect<K: OrderedKernel>(
    kernel: &mut K,
    boundary: OrderedPhaseBoundary,
    phase: usize,
    progress: &CandidateOrderedReductionProgress,
    state: SchedulerState<'_, K>,
) -> Result<(), K::Error> {
    kernel.inspect_phase(OrderedPhaseEvent::new(boundary, phase, *progress, &state))
}

fn invariant(detail: &'static str) -> ReductionError {
    ReductionError::ReducerInvariant { detail }
}

fn coefficient_monomials(coefficient: &Coefficient) -> Result<usize, ReductionError> {
    coefficient
        .numerator
        .nterms()
        .checked_add(coefficient.denominator.nterms())
        .ok_or(ReductionError::CacheResourceCountOverflow {
            resource: "ordered coefficient monomial census",
        })
}

/// Maintain the size of resident maps only: the popped active coefficient and
/// temporary arithmetic are excluded, just as they are from frontier_keys.
fn replace_monomials(total: &mut usize, old: usize, new: usize) -> Result<(), ReductionError> {
    *total = total
        .checked_sub(old)
        .ok_or_else(|| invariant("ordered coefficient monomial census underflow"))?
        .checked_add(new)
        .ok_or(ReductionError::CacheResourceCountOverflow {
            resource: "ordered coefficient monomial census",
        })?;
    Ok(())
}

/// Run one request. Adapters must validate every original input, including zero
/// weights and canceled duplicates, before constructing `initial`.
pub(crate) fn run<K: OrderedKernel>(
    kernel: &mut K,
    request: &mut ReductionRequest,
    initial: BTreeMap<K::Key, Coefficient>,
    nonzero_conditions: Vec<Coefficient>,
) -> Result<OrderedOutput, K::Error> {
    let mut work = ReductionStatistics::default();
    let result = run_inner(kernel, request, initial, nonzero_conditions, &mut work);
    kernel.record_work(work);
    result
}

fn run_inner<K: OrderedKernel>(
    kernel: &mut K,
    request: &mut ReductionRequest,
    initial: BTreeMap<K::Key, Coefficient>,
    mut nonzero_conditions: Vec<Coefficient>,
    work: &mut ReductionStatistics,
) -> Result<OrderedOutput, K::Error> {
    let limits = kernel.limits();
    let mut guards = GuardIndex::new();
    // Use the same hash/equality guard interning as the depth-first engine.
    let initial_conditions = std::mem::take(&mut nonzero_conditions);
    for condition in &initial_conditions {
        guards.intern(&mut nonzero_conditions, condition, |_| Ok(()))?;
    }
    let mut frontier: Frontier<K> = BTreeMap::new();
    let mut output = BTreeMap::new();
    let mut progress = CandidateOrderedReductionProgress {
        total_phases: kernel.phase_count(),
        input_keys: initial.len(),
        ..Default::default()
    };
    for (key, coefficient) in initial {
        if coefficient.is_zero() {
            continue;
        }
        let position = kernel.position(&key)?;
        if position.phase >= progress.total_phases {
            return Err(invariant("ordered entry phase is outside the frozen phase plan").into());
        }
        request.retain_pending_frame(limits.diagnostic_limit(limits.max_pending_frames))?;
        replace_monomials(
            &mut progress.frontier_coefficient_monomials,
            0,
            coefficient_monomials(&coefficient)?,
        )?;
        frontier.insert(
            Scheduled {
                phase: Reverse(position.phase),
                priority: position.priority,
                key,
            },
            coefficient,
        );
    }
    progress.frontier_keys = frontier.len();
    progress.peak_frontier_keys = frontier.len();
    progress.peak_frontier_coefficient_monomials = progress.frontier_coefficient_monomials;
    kernel.observe(&progress, None)?;
    for phase in 0..progress.total_phases {
        inspect(
            kernel,
            OrderedPhaseBoundary::Started,
            phase,
            &progress,
            SchedulerState {
                frontier: &frontier,
                terminals: &output,
                nonzero_conditions: &nonzero_conditions,
            },
        )?;
        while frontier
            .last_key_value()
            .is_some_and(|(node, _)| node.phase.0 == phase)
        {
            let (node, weight) = frontier.pop_last().expect("nonempty checked frontier");
            request.release_pending_frame()?;
            replace_monomials(
                &mut progress.frontier_coefficient_monomials,
                coefficient_monomials(&weight)?,
                0,
            )?;
            progress.frontier_keys = frontier.len();
            kernel.observe(&progress, Some(&node.key))?;
            let checked = kernel.step(&node.key, request, work)?;
            for condition in &checked.nonzero_conditions {
                guards.intern(&mut nonzero_conditions, condition, |_| Ok(()))?;
            }
            for (child, coefficient) in checked.terms {
                if coefficient.is_zero() {
                    continue;
                }
                progress.emitted_terms = progress.emitted_terms.saturating_add(1);
                let position = kernel.position(&child)?;
                if position.phase >= progress.total_phases
                    || position.phase < phase
                    || (position.phase == phase && position.priority >= node.priority)
                {
                    return Err(invariant(
                        "ordered checked action does not strictly decrease its certified position",
                    )
                    .into());
                }
                let contribution = kernel
                    .context()
                    .try_mul(&weight, &coefficient, limits.exact_algebra)
                    .map_err(ReductionError::from)?;
                let scheduled = Scheduled {
                    phase: Reverse(position.phase),
                    priority: position.priority,
                    key: child,
                };
                if let Some(existing) = frontier.get_mut(&scheduled) {
                    work.record_coalescing_additions(1);
                    request.record_coalescing_additions(
                        1,
                        limits.diagnostic_limit(limits.max_coalescing_additions),
                    )?;
                    let sum = kernel
                        .context()
                        .try_add(existing, &contribution, limits.exact_algebra)
                        .map_err(ReductionError::from)?;
                    replace_monomials(
                        &mut progress.frontier_coefficient_monomials,
                        coefficient_monomials(existing)?,
                        if sum.is_zero() {
                            0
                        } else {
                            coefficient_monomials(&sum)?
                        },
                    )?;
                    if sum.is_zero() {
                        frontier.remove(&scheduled);
                        request.release_pending_frame()?;
                        progress.cancelled_terms = progress.cancelled_terms.saturating_add(1);
                    } else {
                        *existing = sum;
                        progress.merged_terms = progress.merged_terms.saturating_add(1);
                    }
                } else if !contribution.is_zero() {
                    request
                        .retain_pending_frame(limits.diagnostic_limit(limits.max_pending_frames))?;
                    replace_monomials(
                        &mut progress.frontier_coefficient_monomials,
                        0,
                        coefficient_monomials(&contribution)?,
                    )?;
                    frontier.insert(scheduled, contribution);
                    progress.peak_frontier_keys = progress.peak_frontier_keys.max(frontier.len());
                }
                progress.peak_frontier_coefficient_monomials = progress
                    .peak_frontier_coefficient_monomials
                    .max(progress.frontier_coefficient_monomials);
            }
            for (terminal, coefficient) in checked.terminals {
                let contribution = kernel
                    .context()
                    .try_mul(&weight, &coefficient, limits.exact_algebra)
                    .map_err(ReductionError::from)?;
                if contribution.is_zero() {
                    continue;
                }
                progress.emitted_terms = progress.emitted_terms.saturating_add(1);
                if let Some(existing) = output.get_mut(&terminal) {
                    work.record_coalescing_additions(1);
                    request.record_coalescing_additions(
                        1,
                        limits.diagnostic_limit(limits.max_coalescing_additions),
                    )?;
                    let sum = kernel
                        .context()
                        .try_add(existing, &contribution, limits.exact_algebra)
                        .map_err(ReductionError::from)?;
                    replace_monomials(
                        &mut progress.terminal_coefficient_monomials,
                        coefficient_monomials(existing)?,
                        if sum.is_zero() {
                            0
                        } else {
                            coefficient_monomials(&sum)?
                        },
                    )?;
                    if sum.is_zero() {
                        output.remove(&terminal);
                        progress.terminal_cancellations =
                            progress.terminal_cancellations.saturating_add(1);
                    } else {
                        *existing = sum;
                        progress.terminal_merges = progress.terminal_merges.saturating_add(1);
                    }
                } else {
                    replace_monomials(
                        &mut progress.terminal_coefficient_monomials,
                        0,
                        coefficient_monomials(&contribution)?,
                    )?;
                    output.insert(terminal, contribution);
                }
                progress.peak_terminal_coefficient_monomials = progress
                    .peak_terminal_coefficient_monomials
                    .max(progress.terminal_coefficient_monomials);
            }
            progress.processed_keys = progress.processed_keys.saturating_add(1);
            progress.frontier_keys = frontier.len();
            progress.peak_frontier_keys = progress.peak_frontier_keys.max(frontier.len());
            progress.terminal_keys = output.len();
            kernel.observe(&progress, Some(&node.key))?;
        }
        if frontier
            .last_key_value()
            .is_some_and(|(node, _)| node.phase.0 < phase)
        {
            return Err(invariant("ordered phase was regenerated after completion").into());
        }
        progress.completed_phases = phase + 1;
        kernel.observe(&progress, None)?;
        inspect(
            kernel,
            OrderedPhaseBoundary::Finished,
            phase,
            &progress,
            SchedulerState {
                frontier: &frontier,
                terminals: &output,
                nonzero_conditions: &nonzero_conditions,
            },
        )?;
    }
    if !frontier.is_empty() {
        return Err(invariant("ordered frontier remains after the last phase").into());
    }
    Ok(OrderedOutput {
        terms: output,
        nonzero_conditions,
        work: *work,
        progress,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct PolynomialKernel {
        context: CoefficientContext,
        observations: Vec<CandidateOrderedReductionProgress>,
        expanded: Vec<usize>,
    }

    impl OrderedKernel for PolynomialKernel {
        type Key = usize;
        type Priority = usize;
        type Error = ReductionError;

        fn context(&self) -> &CoefficientContext {
            &self.context
        }
        fn limits(&self) -> ReductionLimits {
            ReductionLimits::default()
        }
        fn phase_count(&self) -> usize {
            1
        }
        fn position(&mut self, key: &usize) -> Result<OrderedPosition<usize>, ReductionError> {
            Ok(OrderedPosition {
                phase: 0,
                priority: *key,
            })
        }
        fn step(
            &mut self,
            key: &usize,
            _: &mut ReductionRequest,
            _: &mut ReductionStatistics,
        ) -> Result<OrderedStep<usize>, ReductionError> {
            self.expanded.push(*key);
            let terminal = IntegralKey::try_new([1]).unwrap();
            Ok(match key {
                5 | 4 => {
                    let (polynomial, constant) = if *key == 5 { ("d+1", 7) } else { ("1-d", -7) };
                    OrderedStep {
                        terms: BTreeMap::from([
                            (0, self.context.integer(constant)),
                            (1, self.context.coefficient_fixture(polynomial)),
                        ]),
                        terminals: BTreeMap::from([(
                            terminal,
                            self.context.coefficient_fixture(polynomial),
                        )]),
                        nonzero_conditions: vec![],
                    }
                }
                1 => OrderedStep {
                    terminals: BTreeMap::from([(terminal, self.context.integer(-1))]),
                    ..Default::default()
                },
                _ => panic!("the canceled integral must not be expanded"),
            })
        }
        fn observe(
            &mut self,
            progress: &CandidateOrderedReductionProgress,
            _: Option<&usize>,
        ) -> Result<(), ReductionError> {
            self.observations.push(*progress);
            Ok(())
        }
    }

    #[test]
    fn coefficient_census_tracks_polynomial_shrink_and_zero_erasure_incrementally() {
        let mut kernel = PolynomialKernel {
            context: CoefficientContext::try_new(["d"]).unwrap(),
            observations: vec![],
            expanded: vec![],
        };
        let initial = BTreeMap::from([(5, kernel.context.one()), (4, kernel.context.one())]);
        let result = run(
            &mut kernel,
            &mut ReductionRequest::default(),
            initial,
            vec![],
        )
        .unwrap();
        assert_eq!(kernel.expanded, [5, 4, 1]);
        assert!(result.terms.is_empty());
        let after_first = kernel
            .observations
            .iter()
            .find(|p| p.processed_keys == 1)
            .unwrap();
        assert_eq!(
            (
                after_first.frontier_keys,
                after_first.frontier_coefficient_monomials,
                after_first.terminal_coefficient_monomials
            ),
            (3, 7, 3)
        );
        let after_second = kernel
            .observations
            .iter()
            .find(|p| p.processed_keys == 2)
            .unwrap();
        // d+1 + (1-d) leaves the same surviving key with a smaller constant
        // coefficient; the other key cancels exactly and must lose its size.
        assert_eq!(
            (
                after_second.frontier_keys,
                after_second.frontier_coefficient_monomials,
                after_second.terminal_coefficient_monomials
            ),
            (1, 2, 2)
        );
        assert_eq!(
            (
                result.progress.cancelled_terms,
                result.progress.merged_terms,
                result.progress.terminal_cancellations
            ),
            (1, 1, 1)
        );
        assert_eq!(
            (
                result.progress.frontier_coefficient_monomials,
                result.progress.terminal_coefficient_monomials
            ),
            (0, 0)
        );
        assert_eq!(
            (
                result.progress.peak_frontier_coefficient_monomials,
                result.progress.peak_terminal_coefficient_monomials
            ),
            (7, 3)
        );
    }
}
