use super::super::{CandidateRoutedError, CandidateRoutedFrontierReason};
use super::scheduler::{Failure, Shared, Work};
use super::*;
use crate::reduction::{ReductionRequest, ReductionStatistics};
use crate::solver::candidate_reduction::CandidateReductionError;
use crate::solver::candidate_reduction::evaluator::CandidateEvaluator;

pub(super) fn base<const N: usize>(
    reducer: &RoutedCandidateReducer<N>,
) -> CandidateEvaluator<'_, N> {
    let shared = &reducer.programs.context.shared;
    CandidateEvaluator {
        context: &shared.context,
        root_sector: [true; N],
        ordering: Default::default(),
        rules: &[],
        source_conditions: &shared.source_conditions,
        zero_sectors: &shared.zero_sectors,
        limits: reducer.programs.context.limits,
    }
}
fn support<const N: usize>(key: &IntegralKey) -> [bool; N] {
    std::array::from_fn(|i| key.powers()[i] > 0)
}

/// Count/support/phase descent is checked even for an already-seen child. The
/// common rule evaluator additionally checks exact same-owner integral order.
fn child<const N: usize>(
    parent: &IntegralKey,
    child: IntegralKey,
    owner: [bool; N],
) -> Result<Work<N>, Failure> {
    let mask = support::<N>(&child);
    if mask == owner {
        return Ok(Work::Apply {
            owner_sector: owner,
            target: child,
        });
    }
    if mask.iter().zip(owner).all(|(&c, o)| !c || o)
        && mask.iter().filter(|&&x| x).count() < owner.iter().filter(|&&x| x).count()
    {
        return Ok(Work::Route(child));
    }
    Err(CandidateRoutedError::UnsupportedSupportTransition {
        target: parent.clone(),
        child,
    }
    .into())
}

pub(super) fn run<const N: usize>(
    reducer: &RoutedCandidateReducer<N>,
    shared: &Shared<'_, N>,
    bounds: &BTreeMap<[bool; N], usize>,
) {
    // An unexpected native panic must leave a typed incomplete result and wake
    // peers/coordinator, not strand an active node or detach a worker.
    while let Some(node) = shared.take() {
        run_one(shared, node, |node| process(reducer, shared, bounds, node));
    }
}

pub(super) fn run_one<const N: usize>(
    shared: &Shared<'_, N>,
    node: Work<N>,
    task: impl FnOnce(&Work<N>) -> Result<(), Failure>,
) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| task(&node)))
        .unwrap_or(Err(Failure::WorkerPanicked));
    shared.finish(node, result);
}

fn process<const N: usize>(
    reducer: &RoutedCandidateReducer<N>,
    shared: &Shared<'_, N>,
    bounds: &BTreeMap<[bool; N], usize>,
    node: &Work<N>,
) -> Result<(), Failure> {
    shared.check()?;
    let key = node.target();
    let base = base(reducer);
    base.validate_target(key)?;
    shared.degrees(key)?;
    if base.is_zero(key) {
        shared.zero(key);
        return Ok(());
    }
    let parent = support::<N>(key);
    match node {
        Work::Route(_) => {
            if reducer.programs.owners.contains_key(&parent) {
                shared.schedule(Work::Apply {
                    owner_sector: parent,
                    target: key.clone(),
                })?;
            } else if let Some(route) = reducer.routes.get(&parent) {
                shared.transport_call()?;
                let mapped = route
                    .transport
                    .transport_with_usage(key, reducer.limits.expansion, |usage| {
                        shared.transport_usage(usage.operations, usage.endpoints)
                    })
                    .map_err(|error| Failure::from(CandidateRoutedError::Transport(error)))?;
                shared.check()?;
                let owner = std::array::from_fn(|i| route.owner_sector.active_bits()[i]);
                // Admission proves active-denominator bijection, hence equal
                // root support counts. Verify that scheduling premise too.
                if owner.iter().filter(|&&x| x).count() != parent.iter().filter(|&&x| x).count() {
                    return Err(CandidateRoutedError::InvalidInput(
                        "route changed active-root cardinality".into(),
                    )
                    .into());
                }
                for endpoint in mapped.terms() {
                    shared.check()?;
                    base.validate_target(endpoint.key())?;
                    shared.schedule(child(key, endpoint.key().clone(), owner)?)?;
                }
            } else {
                shared.frontier(key.clone(), CandidateRoutedFrontierReason::MissingOwner);
            }
        }
        Work::Apply { owner_sector, .. } => {
            if &parent != owner_sector {
                return Err(CandidateRoutedError::InvalidInput(
                    "apply node does not belong to owner support".into(),
                )
                .into());
            }
            let owner = reducer.programs.owners.get(owner_sector).ok_or_else(|| {
                Failure::from(CandidateRoutedError::InvalidInput(
                    "admitted owner disappeared".into(),
                ))
            })?;
            if owner.terminals.contains(key) {
                shared.terminal(key);
                return Ok(());
            }
            let bound = bounds[owner_sector];
            shared.reserve_apply(bound)?;
            let context = &reducer.programs.context.shared;
            let mut local_limits = reducer.programs.context.limits;
            local_limits.max_coalescing_additions = bound;
            let evaluator = CandidateEvaluator {
                context: &context.context,
                root_sector: owner.root,
                ordering: owner.ordering,
                rules: &owner.rules,
                source_conditions: &context.source_conditions,
                zero_sectors: &context.zero_sectors,
                limits: local_limits,
            };
            let mut request = ReductionRequest::default();
            let mut stats = ReductionStatistics::default();
            // Account/refund the reservation even if native evaluation panics.
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                evaluator.apply(key, &mut request, &mut stats)
            }));
            let successful = matches!(&result, Ok(Ok(_)));
            shared.settle_apply(bound, stats.coalescing_additions(), successful)?;
            let result = result.map_err(|_| Failure::WorkerPanicked)?;
            shared.check()?;
            match result {
                Ok(terms) => {
                    // Native exact local coalescing precedes global key dedup.
                    // Stream/drop coefficients; no persistent expression cache.
                    for target in terms.into_keys() {
                        shared.check()?;
                        shared.schedule(child(key, target, *owner_sector)?)?;
                    }
                }
                Err(CandidateReductionError::Uncovered { target }) => shared.frontier(
                    target,
                    CandidateRoutedFrontierReason::MissingRule {
                        owner_sector: *owner_sector,
                    },
                ),
                Err(error) => return Err(error.into()),
            }
        }
    }
    shared.check()
}
