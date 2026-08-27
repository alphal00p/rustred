//! Memory-admitted execution-width planning before any worker pool exists.
//!
//! This module is deliberately algebra-free and host-independent.  It turns
//! the invocation-wide `--n-cores` ceiling and a calibrated fixed-memory
//! breakdown into the largest feasible effective width.  It does not inspect
//! a topology, count the first ready wave, construct a reducer, or create a
//! thread pool.  A checked plan must be consumed separately before
//! [`ParallelExecution`](crate::ParallelExecution) is constructed.

use std::fmt;
use std::num::NonZeroUsize;

use crate::{
    CampaignBaselineMemory, CampaignBytes, CampaignEstimatorRevision, CampaignTaskResourceEstimate,
    ParallelExecution, ParallelExecutionError,
};

pub const CAMPAIGN_EXECUTION_WIDTH_PLAN_V1_SCHEMA: &str =
    "rustred.campaign-execution-width-plan.v1";

/// Complete calibrated fixed-memory decomposition used before pool creation.
///
/// `per_worker_stack_tls_workspace` is charged for every possible Rayon worker
/// when the effective width is greater than one.  Width one is coordinator
/// inline execution and therefore creates zero worker threads.  The remaining
/// fields are charged once.  Opaque Symbolica allocations belong in the
/// corresponding calibrated coordinator/worker or safety reserve rather than
/// being inferred from sparse-row entry counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CampaignExecutionFixedMemory {
    process_runtime_and_shared_catalogs: CampaignBytes,
    coordinator_stack_tls_workspace: CampaignBytes,
    per_worker_stack_tls_workspace: CampaignBytes,
    explicitly_admitted_inner_threads: CampaignBytes,
    hydrated_retained_lanes: CampaignBytes,
    staged_results: CampaignBytes,
    checkpoint_and_output_buffers: CampaignBytes,
    safety_reserve: CampaignBytes,
    non_worker_total: CampaignBytes,
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

    fn total_for_worker_threads(
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
    estimator_revision: CampaignEstimatorRevision,
    requested_core_ceiling: NonZeroUsize,
    enclosing_memory_limit: CampaignBytes,
    operational_memory_limit: CampaignBytes,
    fixed_memory: CampaignExecutionFixedMemory,
    minimum_runnable_task: CampaignTaskResourceEstimate,
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

/// Successful physical execution metadata.  It intentionally implements no
/// hashing contract: width and memory policy must never enter mathematical
/// family, rule, or bundle identities.
#[derive(Debug, PartialEq, Eq)]
pub struct CampaignExecutionWidthPlan {
    schema: &'static str,
    request: CampaignExecutionWidthRequest,
    effective_width: NonZeroUsize,
    worker_thread_count: usize,
    selected_fixed_memory: CampaignBytes,
    minimum_required_memory: CampaignBytes,
    admission_baseline: CampaignBaselineMemory,
}

impl CampaignExecutionWidthPlan {
    pub const fn schema(&self) -> &'static str {
        self.schema
    }

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

    /// Collapse the detailed physical breakdown into the existing runtime
    /// admission ledger categories without losing or duplicating a byte.
    pub const fn admission_baseline(&self) -> CampaignBaselineMemory {
        self.admission_baseline
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

    /// Campaign admission keeps the physical plan for diagnostics and derives
    /// every runtime capacity from it. Construction still consumes the plan;
    /// returning it alongside the executor prevents an independently supplied
    /// policy from replacing its checked metadata.
    pub(crate) fn try_into_plan_and_parallel_execution(
        self,
    ) -> Result<(Self, ParallelExecution), ParallelExecutionError> {
        ParallelExecution::validate_requested_core_budget(self.requested_core_ceiling())?;
        let execution = self.try_construct_execution_with_factories(
            || ParallelExecution::try_new(1),
            ParallelExecution::try_new,
        )?;
        Ok((self, execution))
    }

    /// Shared construction branch used by production and by the counting
    /// acceptance test. The pool factory is never called for inline width one.
    fn try_construct_execution_with_factories<Execution, Error, Inline, PoolFactory>(
        &self,
        inline: Inline,
        pool_factory: PoolFactory,
    ) -> Result<Execution, Error>
    where
        Inline: FnOnce() -> Result<Execution, Error>,
        PoolFactory: FnOnce(usize) -> Result<Execution, Error>,
    {
        if self.worker_thread_count == 0 {
            inline()
        } else {
            pool_factory(self.effective_width.get())
        }
    }
}

/// Typed no-fit result.  Since no [`CampaignExecutionWidthPlan`] exists, this
/// value has no execution-construction surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CampaignExecutionWidthPause {
    schema: &'static str,
    request: CampaignExecutionWidthRequest,
    inline_fixed_memory: CampaignBytes,
    inline_minimum_required_memory: CampaignBytes,
}

impl CampaignExecutionWidthPause {
    pub const fn schema(self) -> &'static str {
        self.schema
    }

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

pub struct CampaignExecutionWidthPlanner;

impl CampaignExecutionWidthPlanner {
    /// Select the largest feasible width without consulting the host or
    /// constructing a pool.  Width is not the number of tasks in the first
    /// wave: exactly one minimum-task peak is reserved here, while the separate
    /// wave admission layer decides how many heavyweight tasks coexist.
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
                        schema: CAMPAIGN_EXECUTION_WIDTH_PLAN_V1_SCHEMA,
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
        let admission_baseline = CampaignBaselineMemory::try_new(
            fixed_and_shared,
            request.fixed_memory.hydrated_retained_lanes,
            request.fixed_memory.staged_results,
        )
        .map_err(|_| CampaignExecutionWidthError::ByteCountOverflow {
            operation: "collapsed campaign admission baseline",
        })?;
        debug_assert_eq!(admission_baseline.total(), selected_fixed_memory);

        Ok(CampaignExecutionWidthPlanningOutcome::Ready(
            CampaignExecutionWidthPlan {
                schema: CAMPAIGN_EXECUTION_WIDTH_PLAN_V1_SCHEMA,
                request,
                effective_width: NonZeroUsize::new(effective_width)
                    .expect("the selected execution width is positive"),
                worker_thread_count,
                selected_fixed_memory,
                minimum_required_memory,
                admission_baseline,
            },
        ))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CampaignExecutionWidthError {
    ZeroRequestedCoreCeiling,
    ZeroEnclosingMemoryLimit,
    ZeroOperationalMemoryLimit,
    OperationalMemoryNotBelowEnclosing {
        operational: CampaignBytes,
        enclosing: CampaignBytes,
    },
    MinimumTaskEstimatorRevisionMismatch {
        expected: CampaignEstimatorRevision,
        actual: CampaignEstimatorRevision,
    },
    MinimumTaskMustUseOneCore {
        actual: usize,
    },
    ByteCountOverflow {
        operation: &'static str,
    },
    CoreCountDoesNotFitByteArithmetic {
        operation: &'static str,
        count: usize,
    },
}

impl fmt::Display for CampaignExecutionWidthError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroRequestedCoreCeiling => {
                formatter.write_str("campaign requested core ceiling must be positive")
            }
            Self::ZeroEnclosingMemoryLimit => {
                formatter.write_str("campaign enclosing memory limit must be positive")
            }
            Self::ZeroOperationalMemoryLimit => {
                formatter.write_str("campaign operational memory limit must be positive")
            }
            Self::OperationalMemoryNotBelowEnclosing {
                operational,
                enclosing,
            } => write!(
                formatter,
                "campaign operational memory limit {operational} must be strictly below enclosing memory limit {enclosing}"
            ),
            Self::MinimumTaskEstimatorRevisionMismatch { expected, actual } => write!(
                formatter,
                "minimum runnable task uses estimator revision {}, expected {}",
                actual.get(),
                expected.get()
            ),
            Self::MinimumTaskMustUseOneCore { actual } => write!(
                formatter,
                "minimum runnable task requests {actual} cores; pre-pool V1 requires exactly one"
            ),
            Self::ByteCountOverflow { operation } => {
                write!(formatter, "{operation} overflowed u64")
            }
            Self::CoreCountDoesNotFitByteArithmetic { operation, count } => write!(
                formatter,
                "{operation} cannot represent core/thread count {count} as u64"
            ),
        }
    }
}

impl std::error::Error for CampaignExecutionWidthError {}

fn bytes_add(
    left: CampaignBytes,
    right: CampaignBytes,
    operation: &'static str,
) -> Result<CampaignBytes, CampaignExecutionWidthError> {
    left.get()
        .checked_add(right.get())
        .map(CampaignBytes::new)
        .ok_or(CampaignExecutionWidthError::ByteCountOverflow { operation })
}

fn bytes_sub(
    left: CampaignBytes,
    right: CampaignBytes,
    operation: &'static str,
) -> Result<CampaignBytes, CampaignExecutionWidthError> {
    left.get()
        .checked_sub(right.get())
        .map(CampaignBytes::new)
        .ok_or(CampaignExecutionWidthError::ByteCountOverflow { operation })
}

fn bytes_mul_usize(
    bytes: CampaignBytes,
    count: usize,
    operation: &'static str,
) -> Result<CampaignBytes, CampaignExecutionWidthError> {
    let count = u64::try_from(count).map_err(|_| {
        CampaignExecutionWidthError::CoreCountDoesNotFitByteArithmetic { operation, count }
    })?;
    bytes
        .get()
        .checked_mul(count)
        .map(CampaignBytes::new)
        .ok_or(CampaignExecutionWidthError::ByteCountOverflow { operation })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CampaignMemoryEstimate, CampaignTaskMemoryEnvelope};
    use std::cell::Cell;

    fn memory(revision: CampaignEstimatorRevision, bytes: u64) -> CampaignTaskResourceEstimate {
        let envelope = CampaignTaskMemoryEnvelope::try_new(
            CampaignMemoryEstimate::try_new(CampaignBytes::new(bytes), CampaignBytes::ZERO)
                .unwrap(),
            CampaignMemoryEstimate::zero(),
        )
        .unwrap();
        CampaignTaskResourceEstimate::try_new(revision, 1, envelope).unwrap()
    }

    fn fixed(non_worker: u64, per_worker: u64) -> CampaignExecutionFixedMemory {
        CampaignExecutionFixedMemory::try_new(
            CampaignBytes::new(non_worker),
            CampaignBytes::ZERO,
            CampaignBytes::new(per_worker),
            CampaignBytes::ZERO,
            CampaignBytes::ZERO,
            CampaignBytes::ZERO,
            CampaignBytes::ZERO,
            CampaignBytes::ZERO,
        )
        .unwrap()
    }

    fn request(
        requested: usize,
        enclosing: u64,
        operational: u64,
        non_worker: u64,
        per_worker: u64,
        minimum_task: u64,
    ) -> CampaignExecutionWidthRequest {
        let revision = CampaignEstimatorRevision::try_new(7).unwrap();
        CampaignExecutionWidthRequest::try_new(
            revision,
            requested,
            CampaignBytes::new(enclosing),
            CampaignBytes::new(operational),
            fixed(non_worker, per_worker),
            memory(revision, minimum_task),
        )
        .unwrap()
    }

    #[test]
    fn selects_largest_width_and_records_complete_physical_metadata() {
        let outcome =
            CampaignExecutionWidthPlanner::try_plan(request(100, 1_024, 900, 100, 10, 100))
                .unwrap();
        let CampaignExecutionWidthPlanningOutcome::Ready(plan) = outcome else {
            panic!("the calibrated request must fit")
        };
        assert_eq!(plan.schema(), CAMPAIGN_EXECUTION_WIDTH_PLAN_V1_SCHEMA);
        assert_eq!(plan.estimator_revision().get(), 7);
        assert_eq!(plan.requested_core_ceiling(), 100);
        assert_eq!(plan.effective_width(), 70);
        assert_eq!(plan.worker_thread_count(), 70);
        assert_eq!(plan.enclosing_memory_limit(), CampaignBytes::new(1_024));
        assert_eq!(plan.operational_memory_limit(), CampaignBytes::new(900));
        assert_eq!(plan.selected_fixed_memory(), CampaignBytes::new(800));
        assert_eq!(plan.minimum_required_memory(), CampaignBytes::new(900));
        assert_eq!(
            plan.operational_headroom_after_minimum(),
            CampaignBytes::ZERO
        );
        assert_eq!(plan.enclosing_headroom(), CampaignBytes::new(124));
        assert_eq!(plan.admission_baseline().total(), CampaignBytes::new(800));
        assert_eq!(
            plan.fixed_memory().per_worker_stack_tls_workspace(),
            CampaignBytes::new(10)
        );
    }

    #[test]
    fn one_worker_reserve_does_not_create_a_two_worker_pool() {
        let outcome =
            CampaignExecutionWidthPlanner::try_plan(request(100, 500, 250, 100, 100, 50)).unwrap();
        let CampaignExecutionWidthPlanningOutcome::Ready(plan) = outcome else {
            panic!("inline execution must fit")
        };
        assert_eq!(plan.effective_width(), 1);
        assert_eq!(plan.worker_thread_count(), 0);
        assert_eq!(plan.selected_fixed_memory(), CampaignBytes::new(100));
        assert_eq!(plan.minimum_required_memory(), CampaignBytes::new(150));
    }

    #[test]
    fn requested_one_and_zero_worker_reserve_have_exact_semantics() {
        let CampaignExecutionWidthPlanningOutcome::Ready(inline) =
            CampaignExecutionWidthPlanner::try_plan(request(1, 500, 400, 100, 0, 50)).unwrap()
        else {
            panic!("inline request must fit")
        };
        assert_eq!(inline.effective_width(), 1);
        assert_eq!(inline.worker_thread_count(), 0);

        let CampaignExecutionWidthPlanningOutcome::Ready(wide) =
            CampaignExecutionWidthPlanner::try_plan(request(100, 500, 400, 100, 0, 50)).unwrap()
        else {
            panic!("zero worker reserve must admit the requested width")
        };
        assert_eq!(wide.effective_width(), 100);
        assert_eq!(wide.worker_thread_count(), 100);
    }

    #[test]
    fn inline_exact_boundary_fits_and_one_below_returns_typed_pause() {
        let CampaignExecutionWidthPlanningOutcome::Ready(exact) =
            CampaignExecutionWidthPlanner::try_plan(request(8, 301, 300, 200, 80, 100)).unwrap()
        else {
            panic!("the exact inline boundary must fit")
        };
        assert_eq!(exact.effective_width(), 1);
        assert_eq!(exact.minimum_required_memory(), CampaignBytes::new(300));

        let CampaignExecutionWidthPlanningOutcome::PausedForMemoryCapacity(pause) =
            CampaignExecutionWidthPlanner::try_plan(request(8, 301, 299, 200, 80, 100)).unwrap()
        else {
            panic!("one byte below the inline minimum must pause")
        };
        assert_eq!(pause.inline_fixed_memory(), CampaignBytes::new(200));
        assert_eq!(
            pause.inline_minimum_required_memory(),
            CampaignBytes::new(300)
        );
        assert_eq!(pause.memory_shortfall(), CampaignBytes::new(1));
    }

    #[test]
    fn invalid_limits_and_fixed_sum_overflow_are_rejected() {
        let revision = CampaignEstimatorRevision::try_new(1).unwrap();
        let fixed = fixed(1, 1);
        let task = memory(revision, 1);
        assert!(matches!(
            CampaignExecutionWidthRequest::try_new(
                revision,
                0,
                CampaignBytes::new(10),
                CampaignBytes::new(9),
                fixed,
                task,
            ),
            Err(CampaignExecutionWidthError::ZeroRequestedCoreCeiling)
        ));

        let envelope = CampaignTaskMemoryEnvelope::try_new(
            CampaignMemoryEstimate::zero(),
            CampaignMemoryEstimate::zero(),
        )
        .unwrap();
        let wrong_revision = CampaignTaskResourceEstimate::try_new(
            CampaignEstimatorRevision::try_new(2).unwrap(),
            1,
            envelope,
        )
        .unwrap();
        assert!(matches!(
            CampaignExecutionWidthRequest::try_new(
                revision,
                1,
                CampaignBytes::new(10),
                CampaignBytes::new(9),
                fixed,
                wrong_revision,
            ),
            Err(CampaignExecutionWidthError::MinimumTaskEstimatorRevisionMismatch { .. })
        ));
        let wide_task = CampaignTaskResourceEstimate::try_new(revision, 2, envelope).unwrap();
        assert!(matches!(
            CampaignExecutionWidthRequest::try_new(
                revision,
                1,
                CampaignBytes::new(10),
                CampaignBytes::new(9),
                fixed,
                wide_task,
            ),
            Err(CampaignExecutionWidthError::MinimumTaskMustUseOneCore { actual: 2 })
        ));
        for (enclosing, operational) in [(0, 0), (10, 0), (10, 10), (10, 11)] {
            assert!(
                CampaignExecutionWidthRequest::try_new(
                    revision,
                    1,
                    CampaignBytes::new(enclosing),
                    CampaignBytes::new(operational),
                    fixed,
                    task,
                )
                .is_err()
            );
        }
        assert!(matches!(
            CampaignExecutionFixedMemory::try_new(
                CampaignBytes::new(u64::MAX),
                CampaignBytes::new(1),
                CampaignBytes::ZERO,
                CampaignBytes::ZERO,
                CampaignBytes::ZERO,
                CampaignBytes::ZERO,
                CampaignBytes::ZERO,
                CampaignBytes::ZERO,
            ),
            Err(CampaignExecutionWidthError::ByteCountOverflow { .. })
        ));
    }

    #[test]
    fn selected_worker_multiplication_cannot_overflow_or_overadmit() {
        let outcome = CampaignExecutionWidthPlanner::try_plan(request(
            usize::MAX,
            u64::MAX,
            u64::MAX - 1,
            1,
            u64::MAX / 2,
            1,
        ))
        .unwrap();
        let CampaignExecutionWidthPlanningOutcome::Ready(plan) = outcome else {
            panic!("inline execution remains feasible")
        };
        assert_eq!(plan.effective_width(), 1);
        assert_eq!(plan.worker_thread_count(), 0);
        assert!(plan.minimum_required_memory() <= plan.operational_memory_limit());
    }

    #[test]
    fn counting_factory_observes_zero_e_or_exact_e_worker_threads() {
        let inline_calls = Cell::new(0usize);
        let pool_calls = Cell::new(0usize);
        let requested_workers = Cell::new(usize::MAX);

        let CampaignExecutionWidthPlanningOutcome::PausedForMemoryCapacity(_pause) =
            CampaignExecutionWidthPlanner::try_plan(request(4, 20, 10, 8, 1, 3)).unwrap()
        else {
            panic!("request must pause before a plan exists")
        };
        assert_eq!(inline_calls.get(), 0);
        assert_eq!(pool_calls.get(), 0);

        let CampaignExecutionWidthPlanningOutcome::Ready(inline) =
            CampaignExecutionWidthPlanner::try_plan(request(4, 40, 20, 8, 7, 3)).unwrap()
        else {
            panic!("only inline execution should fit")
        };
        inline
            .try_construct_execution_with_factories(
                || -> Result<(), ()> {
                    inline_calls.set(inline_calls.get() + 1);
                    Ok(())
                },
                |worker_count| -> Result<(), ()> {
                    pool_calls.set(pool_calls.get() + 1);
                    requested_workers.set(worker_count);
                    Ok(())
                },
            )
            .unwrap();
        assert_eq!(inline_calls.get(), 1);
        assert_eq!(pool_calls.get(), 0);

        let CampaignExecutionWidthPlanningOutcome::Ready(parallel) =
            CampaignExecutionWidthPlanner::try_plan(request(4, 100, 90, 8, 7, 3)).unwrap()
        else {
            panic!("parallel execution should fit")
        };
        parallel
            .try_construct_execution_with_factories(
                || -> Result<(), ()> {
                    inline_calls.set(inline_calls.get() + 1);
                    Ok(())
                },
                |worker_count| -> Result<(), ()> {
                    pool_calls.set(pool_calls.get() + 1);
                    requested_workers.set(worker_count);
                    Ok(())
                },
            )
            .unwrap();
        assert_eq!(inline_calls.get(), 1);
        assert_eq!(pool_calls.get(), 1);
        assert_eq!(requested_workers.get(), 4);
    }

    #[test]
    fn accepted_inline_plan_consumes_into_an_executor_without_a_worker_pool() {
        let CampaignExecutionWidthPlanningOutcome::Ready(plan) =
            CampaignExecutionWidthPlanner::try_plan(request(1, 100, 90, 10, 20, 10)).unwrap()
        else {
            panic!("inline execution must fit")
        };
        let (_plan, execution) = plan.try_into_plan_and_parallel_execution().unwrap();
        assert_eq!(execution.n_cores(), 1);
        assert_eq!(execution.worker_thread_count(), 0);
        assert!(!execution.is_parallel());
    }
}
