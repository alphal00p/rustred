use std::cmp::Ordering;

use crate::foundry::completion::source_discovery::boundary_simplex::{
    BoundarySimplexPlan, BoundarySimplexTask,
};
use crate::sector::CoordinatePriority;

use super::ProbeCoordinatorFailure;

const TASK_INDICES: &str = "discovery-priority task indices";

/// An execution-index view over one immutable canonical task plan.
///
/// The canonical plan and every task identity remain untouched.  In
/// particular, this view cannot alter exact replay, admission, descent, or
/// artifact ordering; it only decides which already sealed task is attempted
/// next by a bounded discovery campaign.
#[derive(Debug)]
pub(super) enum DiscoveryTaskChronology {
    Canonical,
    Prioritized(Box<[usize]>),
}

impl DiscoveryTaskChronology {
    pub(super) fn try_new(
        plan: &BoundarySimplexPlan,
        priority: Option<&CoordinatePriority>,
    ) -> Result<Self, ProbeCoordinatorFailure> {
        let Some(priority) = priority else {
            return Ok(Self::Canonical);
        };
        let arity = plan
            .tasks()
            .first()
            .map(|task| task.lattice_target().len())
            .ok_or(ProbeCoordinatorFailure::Invariant {
                detail: "discovery chronology received an empty task plan",
            })?;
        if priority.arity() != arity {
            return Err(
                ProbeCoordinatorFailure::WrongDiscoveryCoordinatePriorityArity {
                    expected: arity,
                    actual: priority.arity(),
                },
            );
        }
        // Natural priority is definitionally the established canonical
        // chronology.  Avoiding even an index allocation makes that baseline
        // observationally byte-for-byte identical.
        if priority.is_natural() {
            return Ok(Self::Canonical);
        }

        let task_count = plan.tasks().len();
        let mut indices = Vec::new();
        indices.try_reserve_exact(task_count).map_err(|_| {
            ProbeCoordinatorFailure::AllocationFailure {
                resource: TASK_INDICES,
                requested: task_count,
            }
        })?;
        indices.extend(0..task_count);
        indices.sort_unstable_by(|&left, &right| {
            compare_tasks(&plan.tasks()[left], &plan.tasks()[right], priority)
        });
        debug_assert_eq!(indices.len(), task_count);
        debug_assert!(
            indices
                .iter()
                .enumerate()
                .all(|(position, &index)| index < task_count
                    && !indices[..position].contains(&index))
        );
        Ok(Self::Prioritized(indices.into_boxed_slice()))
    }

    pub(super) fn canonical_task_index(&self, execution_rank: usize) -> Option<usize> {
        match self {
            Self::Canonical => Some(execution_rank),
            Self::Prioritized(indices) => indices.get(execution_rank).copied(),
        }
    }
}

fn compare_tasks(
    left: &BoundarySimplexTask,
    right: &BoundarySimplexTask,
    priority: &CoordinatePriority,
) -> Ordering {
    // Reproduce a coordinate-lexicographic face walk in rank order.  A
    // high-priority coordinate is serviced first when it is pinned; parent
    // endpoints and the concrete lattice target then order competing tasks.
    // Canonical ordinal is the exact total-order tie breaker.
    for rank in 0..priority.arity() {
        let slot = priority
            .rank_by_slot()
            .iter()
            .position(|&candidate| candidate == rank)
            .expect("a CoordinatePriority is a checked complete permutation");
        let order = axis_signature(left, slot).cmp(&axis_signature(right, slot));
        if order != Ordering::Equal {
            return order;
        }
    }
    left.canonical_ordinal().cmp(&right.canonical_ordinal())
}

fn axis_signature(task: &BoundarySimplexTask, slot: usize) -> (bool, u64, bool, u64, u64) {
    let pinned = task.key().pinned_axes().binary_search(&slot).is_ok();
    let lower = task.key().parent_box_lower()[slot];
    let upper = task.key().parent_box_upper()[slot];
    let target = task.lattice_target()[slot];
    (
        !pinned,
        lower,
        upper.is_none(),
        upper.unwrap_or_default(),
        target,
    )
}
