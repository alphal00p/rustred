//! Global, phase-ordered reduction of exact weighted integral frontiers.
//!
//! This engine owns no completed-result cache. Artifact adapters retain the
//! existing checked mathematical operations and supply a certified descending
//! position; this module only schedules and combines their weighted results.
use std::cmp::Reverse;
use std::collections::BTreeMap;

pub(crate) mod arithmetic;
mod modular;

pub use arithmetic::{ModularWeightOptions, OrderedWeightArithmetic};
use modular::ModularRecorder;

use crate::algebra::{Coefficient, CoefficientContext};
use crate::family::IntegralKey;
use crate::reduction::guard_union::GuardIndex;
use crate::reduction::{ReductionError, ReductionLimits, ReductionRequest, ReductionStatistics};
use crate::solver::CandidateOrderedReductionProgress;

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

/// Weight arithmetic of one Ordered run.
///
/// The scheduler's control flow depends on weights only through the zero
/// reports of `merge` and `merge_terminal` (exact-zero erasure). Every other
/// decision (popped keys, steps, children, positions, guards) is made from
/// integer keys and the adapters' exact step coefficients.
pub(crate) trait OrderedWeights {
    type Weight;
    /// The product of the active weight and one exact step coefficient.
    type Contribution;

    /// Admit one nonzero validated input weight.
    fn admit(&mut self, coefficient: Coefficient) -> Result<Self::Weight, ReductionError>;
    /// The popped key's weight becomes the active weight.
    fn begin_step(&mut self, weight: &Self::Weight) -> Result<(), ReductionError>;
    /// The popped key's children and terminals have all been merged.
    fn end_step(&mut self, weight: Self::Weight);
    fn scale(
        &mut self,
        weight: &Self::Weight,
        coefficient: &Coefficient,
    ) -> Result<Self::Contribution, ReductionError>;
    fn contribution_is_zero(&self, contribution: &Self::Contribution) -> bool;
    /// Store a new frontier entry from a nonzero contribution.
    fn insert(&mut self, contribution: Self::Contribution) -> Result<Self::Weight, ReductionError>;
    /// Add into an existing frontier entry. `true` reports an exactly zero
    /// sum; the scheduler then erases the entry and calls `cancel`.
    fn merge(
        &mut self,
        existing: &mut Self::Weight,
        contribution: Self::Contribution,
    ) -> Result<bool, ReductionError>;
    fn cancel(&mut self, removed: Self::Weight) -> Result<(), ReductionError>;
    fn insert_terminal(
        &mut self,
        contribution: Self::Contribution,
    ) -> Result<Self::Weight, ReductionError>;
    fn merge_terminal(
        &mut self,
        existing: &mut Self::Weight,
        contribution: Self::Contribution,
    ) -> Result<bool, ReductionError>;
    fn cancel_terminal(&mut self, removed: Self::Weight) -> Result<(), ReductionError>;
    /// Census of resident coefficient size (monomials for exact weights).
    fn monomials(&self, weight: &Self::Weight) -> Result<usize, ReductionError>;
}

/// Exact normalized rational-function weights (historical arithmetic).
pub(crate) struct ExactWeights {
    context: CoefficientContext,
    limits: crate::algebra::ExactAlgebraLimits,
}

impl ExactWeights {
    pub(crate) fn new(context: &CoefficientContext, limits: ReductionLimits) -> Self {
        Self {
            context: context.clone(),
            limits: limits.exact_algebra,
        }
    }

    fn add(&self, existing: &mut Coefficient, contribution: Coefficient) -> Result<bool, ReductionError> {
        let sum = self
            .context
            .try_add(existing, &contribution, self.limits)
            .map_err(ReductionError::from)?;
        let zero = sum.is_zero();
        if !zero {
            *existing = sum;
        }
        Ok(zero)
    }
}

impl OrderedWeights for ExactWeights {
    type Weight = Coefficient;
    type Contribution = Coefficient;

    fn admit(&mut self, coefficient: Coefficient) -> Result<Coefficient, ReductionError> {
        Ok(coefficient)
    }
    fn begin_step(&mut self, _: &Coefficient) -> Result<(), ReductionError> {
        Ok(())
    }
    fn end_step(&mut self, _: Coefficient) {}
    fn scale(
        &mut self,
        weight: &Coefficient,
        coefficient: &Coefficient,
    ) -> Result<Coefficient, ReductionError> {
        self.context
            .try_mul(weight, coefficient, self.limits)
            .map_err(ReductionError::from)
    }
    fn contribution_is_zero(&self, contribution: &Coefficient) -> bool {
        contribution.is_zero()
    }
    fn insert(&mut self, contribution: Coefficient) -> Result<Coefficient, ReductionError> {
        Ok(contribution)
    }
    fn merge(
        &mut self,
        existing: &mut Coefficient,
        contribution: Coefficient,
    ) -> Result<bool, ReductionError> {
        self.add(existing, contribution)
    }
    fn cancel(&mut self, _: Coefficient) -> Result<(), ReductionError> {
        Ok(())
    }
    fn insert_terminal(&mut self, contribution: Coefficient) -> Result<Coefficient, ReductionError> {
        Ok(contribution)
    }
    fn merge_terminal(
        &mut self,
        existing: &mut Coefficient,
        contribution: Coefficient,
    ) -> Result<bool, ReductionError> {
        self.add(existing, contribution)
    }
    fn cancel_terminal(&mut self, _: Coefficient) -> Result<(), ReductionError> {
        Ok(())
    }
    fn monomials(&self, weight: &Coefficient) -> Result<usize, ReductionError> {
        coefficient_monomials(weight)
    }
}

/// Exact and modular arithmetic in one pass: the exact zero decision is
/// authoritative and every modular decision must agree with it.
struct CrossChecked {
    exact: ExactWeights,
    modular: ModularRecorder,
}

fn disagreement() -> ReductionError {
    modular::error("modular zero decision differs from the exact decision")
}

impl OrderedWeights for CrossChecked {
    type Weight = (Coefficient, modular::ModularWeight);
    type Contribution = (Coefficient, modular::ModularContribution);

    fn admit(&mut self, coefficient: Coefficient) -> Result<Self::Weight, ReductionError> {
        Ok((
            coefficient.clone(),
            self.modular.admit(coefficient)?,
        ))
    }
    fn begin_step(&mut self, weight: &Self::Weight) -> Result<(), ReductionError> {
        self.modular.begin_step(&weight.1)
    }
    fn end_step(&mut self, weight: Self::Weight) {
        self.modular.end_step(weight.1);
    }
    fn scale(
        &mut self,
        weight: &Self::Weight,
        coefficient: &Coefficient,
    ) -> Result<Self::Contribution, ReductionError> {
        Ok((
            self.exact.scale(&weight.0, coefficient)?,
            self.modular.scale(&weight.1, coefficient)?,
        ))
    }
    fn contribution_is_zero(&self, contribution: &Self::Contribution) -> bool {
        contribution.0.is_zero()
    }
    fn insert(&mut self, contribution: Self::Contribution) -> Result<Self::Weight, ReductionError> {
        Ok((contribution.0, self.modular.insert(contribution.1)?))
    }
    fn merge(
        &mut self,
        existing: &mut Self::Weight,
        contribution: Self::Contribution,
    ) -> Result<bool, ReductionError> {
        let exact = self.exact.merge(&mut existing.0, contribution.0)?;
        if self.modular.merge(&mut existing.1, contribution.1)? != exact {
            return Err(disagreement());
        }
        Ok(exact)
    }
    fn cancel(&mut self, removed: Self::Weight) -> Result<(), ReductionError> {
        self.modular.cancel(removed.1)
    }
    fn insert_terminal(
        &mut self,
        contribution: Self::Contribution,
    ) -> Result<Self::Weight, ReductionError> {
        Ok((contribution.0, self.modular.insert_terminal(contribution.1)?))
    }
    fn merge_terminal(
        &mut self,
        existing: &mut Self::Weight,
        contribution: Self::Contribution,
    ) -> Result<bool, ReductionError> {
        let exact = self.exact.merge_terminal(&mut existing.0, contribution.0)?;
        if self.modular.merge_terminal(&mut existing.1, contribution.1)? != exact {
            return Err(disagreement());
        }
        Ok(exact)
    }
    fn cancel_terminal(&mut self, removed: Self::Weight) -> Result<(), ReductionError> {
        self.modular.cancel_terminal(removed.1)
    }
    fn monomials(&self, weight: &Self::Weight) -> Result<usize, ReductionError> {
        self.exact.monomials(&weight.0)
    }
}

#[allow(dead_code)]
/// Run one request with exact weights. Adapters must validate every original
/// input, including zero weights and canceled duplicates, before constructing
/// `initial`.
pub(crate) fn run<K: OrderedKernel>(
    kernel: &mut K,
    request: &mut ReductionRequest,
    initial: BTreeMap<K::Key, Coefficient>,
    nonzero_conditions: Vec<Coefficient>,
) -> Result<OrderedOutput, K::Error> {
    run_with_arithmetic(
        kernel,
        request,
        initial,
        nonzero_conditions,
        OrderedWeightArithmetic::Exact,
    )
}

/// Run one request with the selected weight arithmetic. Every mode expands
/// the same keys and returns the same exact terms and conditions.
pub(crate) fn run_with_arithmetic<K: OrderedKernel>(
    kernel: &mut K,
    request: &mut ReductionRequest,
    initial: BTreeMap<K::Key, Coefficient>,
    nonzero_conditions: Vec<Coefficient>,
    arithmetic: OrderedWeightArithmetic,
) -> Result<OrderedOutput, K::Error> {
    #[cfg(test)]
    let arithmetic = test_override(arithmetic, kernel.context());
    let mut work = ReductionStatistics::default();
    let result = match arithmetic {
        OrderedWeightArithmetic::Exact => {
            let mut weights = ExactWeights::new(kernel.context(), kernel.limits());
            run_inner(kernel, &mut weights, request, initial, nonzero_conditions, &mut work).map(
                |output| OrderedOutput {
                    terms: output.terms,
                    nonzero_conditions: output.nonzero_conditions,
                    work: output.work,
                    progress: output.progress,
                },
            )
        }
        OrderedWeightArithmetic::Modular(options) if options.cross_check_exact => {
            let mut weights = CrossChecked {
                exact: ExactWeights::new(kernel.context(), kernel.limits()),
                modular: ModularRecorder::try_new(kernel.context(), options)?,
            };
            run_inner(kernel, &mut weights, request, initial, nonzero_conditions, &mut work)
                .and_then(|output| {
                    let (exact, modular): (BTreeMap<_, _>, BTreeMap<_, _>) = output
                        .terms
                        .into_iter()
                        .map(|(key, (e, m))| ((key.clone(), e), (key, m)))
                        .unzip();
                    let (reconstructed, statistics) = weights.modular.finish(modular)?;
                    modular::report_statistics(&statistics);
                    if reconstructed != exact {
                        return Err(modular::error(
                            "modular reconstruction differs from the exact terminal map",
                        )
                        .into());
                    }
                    Ok(OrderedOutput {
                        terms: exact,
                        nonzero_conditions: output.nonzero_conditions,
                        work: output.work,
                        progress: output.progress,
                    })
                })
        }
        OrderedWeightArithmetic::Modular(options) => {
            let mut weights = ModularRecorder::try_new(kernel.context(), options)?;
            run_inner(kernel, &mut weights, request, initial, nonzero_conditions, &mut work)
                .and_then(|output| {
                    let (terms, statistics) = weights.finish(output.terms)?;
                    modular::report_statistics(&statistics);
                    Ok(OrderedOutput {
                        terms,
                        nonzero_conditions: output.nonzero_conditions,
                        work: output.work,
                        progress: output.progress,
                    })
                })
        }
    };
    kernel.record_work(work);
    result
}

/// `RUSTRED_TEST_ORDERED_MODULAR=1` runs every exact Ordered unit test with
/// cross-checked modular weights (univariate contexts only).
#[cfg(test)]
fn test_override(
    arithmetic: OrderedWeightArithmetic,
    context: &CoefficientContext,
) -> OrderedWeightArithmetic {
    if arithmetic == OrderedWeightArithmetic::Exact
        && context.variables().len() <= 1
        && std::env::var_os("RUSTRED_TEST_ORDERED_MODULAR").is_some_and(|v| v == "1")
    {
        OrderedWeightArithmetic::Modular(ModularWeightOptions {
            batch_points: 8,
            threads: 2,
            cross_check_exact: true,
            ..Default::default()
        })
    } else {
        arithmetic
    }
}

struct RawOutput<W> {
    terms: BTreeMap<IntegralKey, W>,
    nonzero_conditions: Vec<Coefficient>,
    work: ReductionStatistics,
    progress: CandidateOrderedReductionProgress,
}

fn run_inner<K: OrderedKernel, A: OrderedWeights>(
    kernel: &mut K,
    weights: &mut A,
    request: &mut ReductionRequest,
    initial: BTreeMap<K::Key, Coefficient>,
    mut nonzero_conditions: Vec<Coefficient>,
    work: &mut ReductionStatistics,
) -> Result<RawOutput<A::Weight>, K::Error> {
    let limits = kernel.limits();
    let mut guards = GuardIndex::new();
    // Use the same hash/equality guard interning as the depth-first engine.
    let initial_conditions = std::mem::take(&mut nonzero_conditions);
    for condition in &initial_conditions {
        guards.intern(&mut nonzero_conditions, condition, |_| Ok(()))?;
    }
    let mut frontier = BTreeMap::new();
    let mut output: BTreeMap<IntegralKey, A::Weight> = BTreeMap::new();
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
        let weight = weights.admit(coefficient)?;
        replace_monomials(
            &mut progress.frontier_coefficient_monomials,
            0,
            weights.monomials(&weight)?,
        )?;
        frontier.insert(
            Scheduled {
                phase: Reverse(position.phase),
                priority: position.priority,
                key,
            },
            weight,
        );
    }
    progress.frontier_keys = frontier.len();
    progress.peak_frontier_keys = frontier.len();
    progress.peak_frontier_coefficient_monomials = progress.frontier_coefficient_monomials;
    kernel.observe(&progress, None)?;
    for phase in 0..progress.total_phases {
        while frontier
            .last_key_value()
            .is_some_and(|(node, _)| node.phase.0 == phase)
        {
            let (node, weight) = frontier.pop_last().expect("nonempty checked frontier");
            request.release_pending_frame()?;
            replace_monomials(
                &mut progress.frontier_coefficient_monomials,
                weights.monomials(&weight)?,
                0,
            )?;
            progress.frontier_keys = frontier.len();
            kernel.observe(&progress, Some(&node.key))?;
            let checked = kernel.step(&node.key, request, work)?;
            for condition in &checked.nonzero_conditions {
                guards.intern(&mut nonzero_conditions, condition, |_| Ok(()))?;
            }
            weights.begin_step(&weight)?;
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
                let contribution = weights.scale(&weight, &coefficient)?;
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
                    let old = weights.monomials(existing)?;
                    let zero = weights.merge(existing, contribution)?;
                    let new = if zero { 0 } else { weights.monomials(existing)? };
                    replace_monomials(&mut progress.frontier_coefficient_monomials, old, new)?;
                    if zero {
                        let removed = frontier.remove(&scheduled).expect("existing frontier key");
                        weights.cancel(removed)?;
                        request.release_pending_frame()?;
                        progress.cancelled_terms = progress.cancelled_terms.saturating_add(1);
                    } else {
                        progress.merged_terms = progress.merged_terms.saturating_add(1);
                    }
                } else if !weights.contribution_is_zero(&contribution) {
                    request
                        .retain_pending_frame(limits.diagnostic_limit(limits.max_pending_frames))?;
                    let inserted = weights.insert(contribution)?;
                    replace_monomials(
                        &mut progress.frontier_coefficient_monomials,
                        0,
                        weights.monomials(&inserted)?,
                    )?;
                    frontier.insert(scheduled, inserted);
                    progress.peak_frontier_keys = progress.peak_frontier_keys.max(frontier.len());
                }
                progress.peak_frontier_coefficient_monomials = progress
                    .peak_frontier_coefficient_monomials
                    .max(progress.frontier_coefficient_monomials);
            }
            for (terminal, coefficient) in checked.terminals {
                let contribution = weights.scale(&weight, &coefficient)?;
                if weights.contribution_is_zero(&contribution) {
                    continue;
                }
                progress.emitted_terms = progress.emitted_terms.saturating_add(1);
                if let Some(existing) = output.get_mut(&terminal) {
                    work.record_coalescing_additions(1);
                    request.record_coalescing_additions(
                        1,
                        limits.diagnostic_limit(limits.max_coalescing_additions),
                    )?;
                    let old = weights.monomials(existing)?;
                    let zero = weights.merge_terminal(existing, contribution)?;
                    let new = if zero { 0 } else { weights.monomials(existing)? };
                    replace_monomials(&mut progress.terminal_coefficient_monomials, old, new)?;
                    if zero {
                        let removed = output.remove(&terminal).expect("existing terminal");
                        weights.cancel_terminal(removed)?;
                        progress.terminal_cancellations =
                            progress.terminal_cancellations.saturating_add(1);
                    } else {
                        progress.terminal_merges = progress.terminal_merges.saturating_add(1);
                    }
                } else {
                    let inserted = weights.insert_terminal(contribution)?;
                    replace_monomials(
                        &mut progress.terminal_coefficient_monomials,
                        0,
                        weights.monomials(&inserted)?,
                    )?;
                    output.insert(terminal, inserted);
                }
                progress.peak_terminal_coefficient_monomials = progress
                    .peak_terminal_coefficient_monomials
                    .max(progress.terminal_coefficient_monomials);
            }
            weights.end_step(weight);
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
    }
    if !frontier.is_empty() {
        return Err(invariant("ordered frontier remains after the last phase").into());
    }
    Ok(RawOutput {
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

    fn modular(cross_check_exact: bool) -> OrderedWeightArithmetic {
        OrderedWeightArithmetic::Modular(ModularWeightOptions {
            batch_points: 4,
            threads: 3,
            cross_check_exact,
            ..Default::default()
        })
    }

    #[test]
    fn modular_weights_erase_the_same_canceled_keys() {
        for cross_check in [false, true] {
            let mut kernel = PolynomialKernel {
                context: CoefficientContext::try_new(["d"]).unwrap(),
                observations: vec![],
                expanded: vec![],
            };
            let initial = BTreeMap::from([(5, kernel.context.one()), (4, kernel.context.one())]);
            let result = run_with_arithmetic(
                &mut kernel,
                &mut ReductionRequest::default(),
                initial,
                vec![],
                modular(cross_check),
            )
            .unwrap();
            assert_eq!(kernel.expanded, [5, 4, 1]);
            assert!(result.terms.is_empty());
            assert_eq!(
                (
                    result.progress.cancelled_terms,
                    result.progress.merged_terms,
                    result.progress.terminal_cancellations
                ),
                (1, 1, 1)
            );
        }
    }

    /// A ladder with large integers, poles and repeated merges.
    struct LadderKernel {
        context: CoefficientContext,
        length: usize,
    }

    impl OrderedKernel for LadderKernel {
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
            2
        }
        fn position(&mut self, key: &usize) -> Result<OrderedPosition<usize>, ReductionError> {
            Ok(OrderedPosition {
                phase: usize::from(*key < self.length / 2),
                priority: *key,
            })
        }
        fn step(
            &mut self,
            key: &usize,
            _: &mut ReductionRequest,
            _: &mut ReductionStatistics,
        ) -> Result<OrderedStep<usize>, ReductionError> {
            let k = *key as i64;
            let c = |text: String| self.context.coefficient_fixture(&text);
            let terminal = |i: i64| IntegralKey::try_new([i]).unwrap();
            if *key < 3 {
                return Ok(OrderedStep {
                    terminals: BTreeMap::from([
                        (terminal(k), c(format!("(d-{k})/(2*d+{k}+1)"))),
                        (terminal(7), c(format!("{}", 3 - k))),
                    ]),
                    nonzero_conditions: vec![c(format!("2*d+{k}+1"))],
                    ..Default::default()
                });
            }
            Ok(OrderedStep {
                terms: BTreeMap::from([
                    (
                        key - 1,
                        c(format!("(123456789012345678901*d^2-{k}*d+5)/(3*d+{k})")),
                    ),
                    (key - 2, c(format!("(d+{k})/(7*d-{})", 2 * k + 1))),
                    (key - 3, c(format!("-{k}/98765432109876543211"))),
                ]),
                terminals: BTreeMap::from([(terminal(9), c(format!("d^{}", k % 3)))]),
                nonzero_conditions: vec![c(format!("3*d+{k}"))],
            })
        }
    }

    #[test]
    fn modular_weights_reconstruct_large_exact_ladder_results() {
        let context = CoefficientContext::try_new(["d"]).unwrap();
        let initial = BTreeMap::from([
            (13, context.coefficient_fixture("(d^3-2)/(d-11)")),
            (11, context.coefficient_fixture("1/3")),
        ]);
        let exact = run(
            &mut LadderKernel {
                context: context.clone(),
                length: 13,
            },
            &mut ReductionRequest::default(),
            initial.clone(),
            vec![context.coefficient_fixture("d-11")],
        )
        .unwrap();
        for cross_check in [false, true] {
            let result = run_with_arithmetic(
                &mut LadderKernel {
                    context: context.clone(),
                    length: 13,
                },
                &mut ReductionRequest::default(),
                initial.clone(),
                vec![context.coefficient_fixture("d-11")],
                modular(cross_check),
            )
            .unwrap();
            assert_eq!(result.terms, exact.terms);
            assert_eq!(result.nonzero_conditions, exact.nonzero_conditions);
            assert_eq!(result.progress.processed_keys, exact.progress.processed_keys);
        }
        assert_eq!(exact.terms.len(), 5);
    }

    #[test]
    fn modular_weights_support_constant_contexts_and_reject_multivariate_ones() {
        let constant = CoefficientContext::try_new(Vec::<String>::new()).unwrap();
        struct Constant(CoefficientContext);
        impl OrderedKernel for Constant {
            type Key = usize;
            type Priority = usize;
            type Error = ReductionError;
            fn context(&self) -> &CoefficientContext {
                &self.0
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
                let terminal = IntegralKey::try_new([0]).unwrap();
                Ok(if *key == 0 {
                    OrderedStep {
                        terminals: BTreeMap::from([(terminal, self.0.integer(1))]),
                        ..Default::default()
                    }
                } else {
                    OrderedStep {
                        terms: BTreeMap::from([(key - 1, self.0.coefficient_fixture("-7/3"))]),
                        ..Default::default()
                    }
                })
            }
        }
        let initial = BTreeMap::from([(30, constant.coefficient_fixture("5/11"))]);
        let exact = run(
            &mut Constant(constant.clone()),
            &mut ReductionRequest::default(),
            initial.clone(),
            vec![],
        )
        .unwrap();
        let modular_result = run_with_arithmetic(
            &mut Constant(constant.clone()),
            &mut ReductionRequest::default(),
            initial,
            vec![],
            modular(false),
        )
        .unwrap();
        assert_eq!(modular_result.terms, exact.terms);
        let multivariate = CoefficientContext::try_new(["d", "m"]).unwrap();
        assert!(matches!(
            run_with_arithmetic(
                &mut Constant(multivariate.clone()),
                &mut ReductionRequest::default(),
                BTreeMap::from([(1, multivariate.one())]),
                vec![],
                modular(false),
            ),
            Err(ReductionError::ModularWeights { .. })
        ));
    }
}
