use std::num::NonZeroUsize;

use super::arithmetic::{bytes_add, bytes_sub};
use super::{
    CampaignExecutionWidthError, CampaignExecutionWidthPause, CampaignExecutionWidthPlan,
    CampaignExecutionWidthPlanningOutcome, CampaignExecutionWidthRequest,
};
use crate::campaign::CampaignBaselineMemory;

pub struct CampaignExecutionWidthPlanner;

impl CampaignExecutionWidthPlanner {
    /// Select the largest feasible width without consulting the host or
    /// constructing a pool. Width is independent of task concurrency: exactly
    /// one minimum-task peak is reserved here, and later orchestration must
    /// account separately for any additional concurrent task peaks.
    pub fn try_plan(
        request: CampaignExecutionWidthRequest,
    ) -> Result<CampaignExecutionWidthPlanningOutcome, CampaignExecutionWidthError> {
        let inline_fixed_memory = request.fixed_memory.total_for_worker_threads(0)?;
        let inline_minimum_required_memory = bytes_add(
            inline_fixed_memory,
            request.minimum_runnable_task.memory().peak_additional(),
            "inline fixed memory plus minimum runnable task",
        )?;
        if inline_minimum_required_memory > request.operational_memory_limit {
            return Ok(
                CampaignExecutionWidthPlanningOutcome::PausedForMemoryCapacity(
                    CampaignExecutionWidthPause {
                        request,
                        inline_fixed_memory,
                        inline_minimum_required_memory,
                    },
                ),
            );
        }

        let requested = request.requested_core_ceiling.get();
        let remaining_after_inline_minimum = request
            .operational_memory_limit
            .get()
            .checked_sub(inline_minimum_required_memory.get())
            .expect("the inline minimum was already proven to fit");
        let per_worker = request.fixed_memory.per_worker_stack_tls_workspace.get();
        let memory_limited_parallel_width = if per_worker == 0 {
            requested
        } else {
            usize::try_from(remaining_after_inline_minimum / per_worker)
                .unwrap_or(usize::MAX)
                .min(requested)
        };
        let effective_width = if requested > 1 && memory_limited_parallel_width >= 2 {
            memory_limited_parallel_width
        } else {
            1
        };
        let worker_thread_count = if effective_width == 1 {
            0
        } else {
            effective_width
        };
        let selected_fixed_memory = request
            .fixed_memory
            .total_for_worker_threads(worker_thread_count)?;
        let minimum_required_memory = bytes_add(
            selected_fixed_memory,
            request.minimum_runnable_task.memory().peak_additional(),
            "selected fixed memory plus minimum runnable task",
        )?;
        debug_assert!(minimum_required_memory <= request.operational_memory_limit);
        let fixed_and_shared = bytes_sub(
            bytes_sub(
                selected_fixed_memory,
                request.fixed_memory.hydrated_retained_lanes,
                "selected fixed memory minus hydrated retained lanes",
            )?,
            request.fixed_memory.staged_results,
            "selected fixed memory minus staged results",
        )?;
        let baseline_memory = CampaignBaselineMemory::try_new(
            fixed_and_shared,
            request.fixed_memory.hydrated_retained_lanes,
            request.fixed_memory.staged_results,
        )
        .map_err(|_| CampaignExecutionWidthError::ByteCountOverflow {
            operation: "collapsed campaign baseline memory",
        })?;
        debug_assert_eq!(baseline_memory.total(), selected_fixed_memory);

        Ok(CampaignExecutionWidthPlanningOutcome::Ready(
            CampaignExecutionWidthPlan {
                request,
                effective_width: NonZeroUsize::new(effective_width)
                    .expect("the selected execution width is positive"),
                worker_thread_count,
                selected_fixed_memory,
                minimum_required_memory,
                baseline_memory,
            },
        ))
    }
}
