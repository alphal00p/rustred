use std::num::NonZeroUsize;

use super::CampaignExecutionWidthError;
use super::arithmetic::{bytes_add, bytes_mul_usize};
use crate::campaign::{
    CampaignBaselineMemory, CampaignBytes, CampaignEstimatorRevision, CampaignTaskResourceEstimate,
};

/// Complete calibrated fixed-memory decomposition used before pool creation.
///
/// `per_worker_stack_tls_workspace` is charged for every possible Rayon worker
/// when the effective width is greater than one. Width one is coordinator
/// inline execution and therefore creates zero worker threads. Opaque
/// Symbolica allocations belong in the calibrated coordinator/worker or safety
/// reserve rather than being inferred from sparse-row entry counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CampaignExecutionFixedMemory {
    pub(super) process_runtime_and_shared_catalogs: CampaignBytes,
    pub(super) coordinator_stack_tls_workspace: CampaignBytes,
    pub(super) per_worker_stack_tls_workspace: CampaignBytes,
    pub(super) explicitly_admitted_inner_threads: CampaignBytes,
    pub(super) hydrated_retained_lanes: CampaignBytes,
    pub(super) staged_results: CampaignBytes,
    pub(super) checkpoint_and_output_buffers: CampaignBytes,
    pub(super) safety_reserve: CampaignBytes,
    pub(super) non_worker_total: CampaignBytes,
}

impl CampaignExecutionFixedMemory {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        process_runtime_and_shared_catalogs: CampaignBytes,
        coordinator_stack_tls_workspace: CampaignBytes,
        per_worker_stack_tls_workspace: CampaignBytes,
        explicitly_admitted_inner_threads: CampaignBytes,
        hydrated_retained_lanes: CampaignBytes,
        staged_results: CampaignBytes,
        checkpoint_and_output_buffers: CampaignBytes,
        safety_reserve: CampaignBytes,
    ) -> Result<Self, CampaignExecutionWidthError> {
        let non_worker_total = [
            process_runtime_and_shared_catalogs,
            coordinator_stack_tls_workspace,
            explicitly_admitted_inner_threads,
            hydrated_retained_lanes,
            staged_results,
            checkpoint_and_output_buffers,
            safety_reserve,
        ]
        .into_iter()
        .try_fold(CampaignBytes::ZERO, |total, component| {
            bytes_add(total, component, "fixed non-worker memory")
        })?;
        Ok(Self {
            process_runtime_and_shared_catalogs,
            coordinator_stack_tls_workspace,
            per_worker_stack_tls_workspace,
            explicitly_admitted_inner_threads,
            hydrated_retained_lanes,
            staged_results,
            checkpoint_and_output_buffers,
            safety_reserve,
            non_worker_total,
        })
    }

    pub const fn process_runtime_and_shared_catalogs(self) -> CampaignBytes {
        self.process_runtime_and_shared_catalogs
    }

    pub const fn coordinator_stack_tls_workspace(self) -> CampaignBytes {
        self.coordinator_stack_tls_workspace
    }

    pub const fn per_worker_stack_tls_workspace(self) -> CampaignBytes {
        self.per_worker_stack_tls_workspace
    }

    pub const fn explicitly_admitted_inner_threads(self) -> CampaignBytes {
        self.explicitly_admitted_inner_threads
    }

    pub const fn hydrated_retained_lanes(self) -> CampaignBytes {
        self.hydrated_retained_lanes
    }

    pub const fn staged_results(self) -> CampaignBytes {
        self.staged_results
    }

    pub const fn checkpoint_and_output_buffers(self) -> CampaignBytes {
        self.checkpoint_and_output_buffers
    }

    pub const fn safety_reserve(self) -> CampaignBytes {
        self.safety_reserve
    }

    pub const fn non_worker_total(self) -> CampaignBytes {
        self.non_worker_total
    }

    pub(super) fn total_for_worker_threads(
        self,
        worker_thread_count: usize,
    ) -> Result<CampaignBytes, CampaignExecutionWidthError> {
        let worker_memory = bytes_mul_usize(
            self.per_worker_stack_tls_workspace,
            worker_thread_count,
            "warmed worker stack/TLS/Workspace memory",
        )?;
        bytes_add(
            self.non_worker_total,
            worker_memory,
            "selected fixed execution memory",
        )
    }
}

/// Host-independent inputs to one pre-pool planning decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CampaignExecutionWidthRequest {
    pub(super) estimator_revision: CampaignEstimatorRevision,
    pub(super) requested_core_ceiling: NonZeroUsize,
    pub(super) enclosing_memory_limit: CampaignBytes,
    pub(super) operational_memory_limit: CampaignBytes,
    pub(super) fixed_memory: CampaignExecutionFixedMemory,
    pub(super) minimum_runnable_task: CampaignTaskResourceEstimate,
}

impl CampaignExecutionWidthRequest {
    pub fn try_new(
        estimator_revision: CampaignEstimatorRevision,
        requested_core_ceiling: usize,
        enclosing_memory_limit: CampaignBytes,
        operational_memory_limit: CampaignBytes,
        fixed_memory: CampaignExecutionFixedMemory,
        minimum_runnable_task: CampaignTaskResourceEstimate,
    ) -> Result<Self, CampaignExecutionWidthError> {
        let requested_core_ceiling = NonZeroUsize::new(requested_core_ceiling)
            .ok_or(CampaignExecutionWidthError::ZeroRequestedCoreCeiling)?;
        if enclosing_memory_limit == CampaignBytes::ZERO {
            return Err(CampaignExecutionWidthError::ZeroEnclosingMemoryLimit);
        }
        if operational_memory_limit == CampaignBytes::ZERO {
            return Err(CampaignExecutionWidthError::ZeroOperationalMemoryLimit);
        }
        if operational_memory_limit >= enclosing_memory_limit {
            return Err(
                CampaignExecutionWidthError::OperationalMemoryNotBelowEnclosing {
                    operational: operational_memory_limit,
                    enclosing: enclosing_memory_limit,
                },
            );
        }
        if minimum_runnable_task.estimator_revision() != estimator_revision {
            return Err(
                CampaignExecutionWidthError::MinimumTaskEstimatorRevisionMismatch {
                    expected: estimator_revision,
                    actual: minimum_runnable_task.estimator_revision(),
                },
            );
        }
        if minimum_runnable_task.cores() != 1 {
            return Err(CampaignExecutionWidthError::MinimumTaskMustUseOneCore {
                actual: minimum_runnable_task.cores(),
            });
        }
        Ok(Self {
            estimator_revision,
            requested_core_ceiling,
            enclosing_memory_limit,
            operational_memory_limit,
            fixed_memory,
            minimum_runnable_task,
        })
    }

    pub const fn estimator_revision(self) -> CampaignEstimatorRevision {
        self.estimator_revision
    }

    pub const fn requested_core_ceiling(self) -> usize {
        self.requested_core_ceiling.get()
    }

    pub const fn enclosing_memory_limit(self) -> CampaignBytes {
        self.enclosing_memory_limit
    }

    pub const fn operational_memory_limit(self) -> CampaignBytes {
        self.operational_memory_limit
    }

    pub const fn fixed_memory(self) -> CampaignExecutionFixedMemory {
        self.fixed_memory
    }

    pub const fn minimum_runnable_task(self) -> CampaignTaskResourceEstimate {
        self.minimum_runnable_task
    }
}

/// Successful physical execution metadata. It intentionally implements no
/// hashing contract: width and memory policy must never enter mathematical
/// family, rule, or bundle identities.
#[derive(Debug, PartialEq, Eq)]
pub struct CampaignExecutionWidthPlan {
    pub(super) request: CampaignExecutionWidthRequest,
    pub(super) effective_width: NonZeroUsize,
    pub(super) worker_thread_count: usize,
    pub(super) selected_fixed_memory: CampaignBytes,
    pub(super) minimum_required_memory: CampaignBytes,
    pub(super) baseline_memory: CampaignBaselineMemory,
}

impl CampaignExecutionWidthPlan {
    pub const fn estimator_revision(&self) -> CampaignEstimatorRevision {
        self.request.estimator_revision
    }

    pub const fn requested_core_ceiling(&self) -> usize {
        self.request.requested_core_ceiling.get()
    }

    pub const fn effective_width(&self) -> usize {
        self.effective_width.get()
    }

    pub const fn worker_thread_count(&self) -> usize {
        self.worker_thread_count
    }

    pub const fn enclosing_memory_limit(&self) -> CampaignBytes {
        self.request.enclosing_memory_limit
    }

    pub const fn operational_memory_limit(&self) -> CampaignBytes {
        self.request.operational_memory_limit
    }

    pub const fn fixed_memory(&self) -> CampaignExecutionFixedMemory {
        self.request.fixed_memory
    }

    pub const fn minimum_runnable_task(&self) -> CampaignTaskResourceEstimate {
        self.request.minimum_runnable_task
    }

    pub const fn selected_fixed_memory(&self) -> CampaignBytes {
        self.selected_fixed_memory
    }

    pub const fn minimum_required_memory(&self) -> CampaignBytes {
        self.minimum_required_memory
    }

    /// Collapse the detailed physical breakdown into the public campaign
    /// baseline categories without losing or duplicating a byte.
    pub const fn baseline_memory(&self) -> CampaignBaselineMemory {
        self.baseline_memory
    }

    pub const fn operational_headroom_after_minimum(&self) -> CampaignBytes {
        CampaignBytes::new(
            self.request.operational_memory_limit.get() - self.minimum_required_memory.get(),
        )
    }

    pub const fn enclosing_headroom(&self) -> CampaignBytes {
        CampaignBytes::new(
            self.request.enclosing_memory_limit.get() - self.request.operational_memory_limit.get(),
        )
    }
}

/// Typed no-fit result. Since no [`CampaignExecutionWidthPlan`] exists, this
/// value has no execution-construction surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CampaignExecutionWidthPause {
    pub(super) request: CampaignExecutionWidthRequest,
    pub(super) inline_fixed_memory: CampaignBytes,
    pub(super) inline_minimum_required_memory: CampaignBytes,
}

impl CampaignExecutionWidthPause {
    pub const fn request(self) -> CampaignExecutionWidthRequest {
        self.request
    }

    pub const fn inline_fixed_memory(self) -> CampaignBytes {
        self.inline_fixed_memory
    }

    pub const fn inline_minimum_required_memory(self) -> CampaignBytes {
        self.inline_minimum_required_memory
    }

    pub const fn memory_shortfall(self) -> CampaignBytes {
        CampaignBytes::new(
            self.inline_minimum_required_memory.get() - self.request.operational_memory_limit.get(),
        )
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum CampaignExecutionWidthPlanningOutcome {
    Ready(CampaignExecutionWidthPlan),
    PausedForMemoryCapacity(CampaignExecutionWidthPause),
}
