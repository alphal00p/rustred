//! Deterministic, explicitly bounded parallel execution for RustRed work.
//!
//! This module owns a private Rayon pool instead of changing Rayon's global
//! configuration.  Stable work ordinals are collected in ordinal order, so
//! changing the worker count cannot change a mathematical transcript.

use std::fmt;
use std::num::NonZeroUsize;

use rayon::prelude::*;
use rayon::{ThreadPool, ThreadPoolBuilder};
use symbolica::LicenseManager;

/// Failure to construct an explicitly bounded RustRed execution context.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParallelExecutionError {
    ZeroCoreBudget,
    AvailableParallelism {
        message: String,
    },
    CoreBudgetExceedsAvailable {
        requested: usize,
        available: usize,
    },
    MulticoreRequiresSymbolicaLicense {
        requested: usize,
    },
    WorkerPoolBuild {
        requested: usize,
        message: String,
    },
    OrderedResultCeilingExceeded {
        work_items: usize,
        admitted_ceiling: usize,
    },
    OrderedResultAllocation {
        admitted_results: usize,
    },
}

impl fmt::Display for ParallelExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroCoreBudget => formatter.write_str("the worker-core budget must be positive"),
            Self::AvailableParallelism { message } => write!(
                formatter,
                "cannot determine the process's available worker-core budget: {message}"
            ),
            Self::CoreBudgetExceedsAvailable {
                requested,
                available,
            } => write!(
                formatter,
                "requested n_cores {requested} exceeds the {available} logical cores available to this process"
            ),
            Self::MulticoreRequiresSymbolicaLicense { requested } => write!(
                formatter,
                "n_cores {requested} requires a Symbolica license; use n_cores = 1 or configure SYMBOLICA_LICENSE"
            ),
            Self::WorkerPoolBuild { requested, message } => write!(
                formatter,
                "cannot construct the {requested}-core RustRed worker pool: {message}"
            ),
            Self::OrderedResultCeilingExceeded {
                work_items,
                admitted_ceiling,
            } => write!(
                formatter,
                "ordered execution received {work_items} work items, exceeding its admitted result ceiling {admitted_ceiling}"
            ),
            Self::OrderedResultAllocation { admitted_results } => write!(
                formatter,
                "could not reserve the exact {admitted_results}-entry ordered result buffer"
            ),
        }
    }
}

impl std::error::Error for ParallelExecutionError {}

/// One process-local RustRed compute-core budget.
///
/// A budget of one executes inline on the calling thread and creates no Rayon
/// worker.  Larger budgets own exactly one private Rayon pool.  The pool is a
/// scheduler only: every algebraic operation remains a Symbolica operation.
pub struct ParallelExecution {
    n_cores: NonZeroUsize,
    ordered_result_ceiling: usize,
    pool: Option<ThreadPool>,
}

impl fmt::Debug for ParallelExecution {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ParallelExecution")
            .field("n_cores", &self.n_cores)
            .field("ordered_result_ceiling", &self.ordered_result_ceiling)
            .field("has_worker_pool", &self.pool.is_some())
            .finish()
    }
}

impl ParallelExecution {
    /// Validate an invocation's requested core ceiling without creating a
    /// worker. Campaign width planning remains host-independent, then calls
    /// this preflight before consuming its checked memory plan.
    pub fn preflight_requested_core_budget(n_cores: usize) -> Result<(), ParallelExecutionError> {
        let n_cores = NonZeroUsize::new(n_cores).ok_or(ParallelExecutionError::ZeroCoreBudget)?;
        if n_cores.get() == 1 {
            return Ok(());
        }
        let available = std::thread::available_parallelism().map_err(|error| {
            ParallelExecutionError::AvailableParallelism {
                message: error.to_string(),
            }
        })?;
        if n_cores > available {
            return Err(ParallelExecutionError::CoreBudgetExceedsAvailable {
                requested: n_cores.get(),
                available: available.get(),
            });
        }
        if !LicenseManager::is_licensed() {
            return Err(ParallelExecutionError::MulticoreRequiresSymbolicaLicense {
                requested: n_cores.get(),
            });
        }
        Ok(())
    }

    /// Construct one bounded execution context.
    ///
    /// `ordered_result_ceiling` is the largest exact batch result count
    /// admitted by the caller's allocation-free structural preflight. Every
    /// later ordered map is constrained by this retained ceiling.
    ///
    /// Multicore execution is rejected before a RustRed worker is created if
    /// the installed Symbolica instance is not licensed for it.
    pub fn try_new(
        n_cores: usize,
        ordered_result_ceiling: usize,
    ) -> Result<Self, ParallelExecutionError> {
        Self::preflight_requested_core_budget(n_cores)?;
        let n_cores = NonZeroUsize::new(n_cores).ok_or(ParallelExecutionError::ZeroCoreBudget)?;
        if n_cores.get() == 1 {
            return Ok(Self {
                n_cores,
                ordered_result_ceiling,
                pool: None,
            });
        }
        let requested = n_cores.get();
        let pool = ThreadPoolBuilder::new()
            .num_threads(requested)
            .thread_name(|ordinal| format!("rustred-worker-{ordinal}"))
            .build()
            .map_err(|error| ParallelExecutionError::WorkerPoolBuild {
                requested,
                message: error.to_string(),
            })?;
        debug_assert_eq!(pool.current_num_threads(), requested);
        Ok(Self {
            n_cores,
            ordered_result_ceiling,
            pool: Some(pool),
        })
    }

    #[cfg(test)]
    fn n_cores(&self) -> usize {
        self.n_cores.get()
    }

    /// Number of owned Rayon worker threads. Inline width one has none.
    #[cfg(test)]
    fn worker_thread_count(&self) -> usize {
        self.pool
            .as_ref()
            .map_or(0, rayon::ThreadPool::current_num_threads)
    }

    /// Evaluate one admitted batch of stable work ordinals.
    ///
    /// `work_items` must be an exact batch count established by the caller's
    /// allocation-free preflight and cannot exceed the ceiling retained when
    /// this executor was constructed. The result buffer is reserved fallibly
    /// before the first operation runs. The operation receives the same
    /// ordinal in serial and parallel modes, and results retain ordinal order.
    /// In particular, callers can collect `Result` values and then select the
    /// lowest-ordinal failure deterministically on the coordinator.
    pub fn map_ordered<ResultValue, Operation>(
        &self,
        work_items: usize,
        operation: Operation,
    ) -> Result<Vec<ResultValue>, ParallelExecutionError>
    where
        ResultValue: Send,
        Operation: Fn(usize) -> ResultValue + Send + Sync,
    {
        if work_items > self.ordered_result_ceiling {
            return Err(ParallelExecutionError::OrderedResultCeilingExceeded {
                work_items,
                admitted_ceiling: self.ordered_result_ceiling,
            });
        }
        let mut results = Vec::new();
        results.try_reserve_exact(work_items).map_err(|_| {
            ParallelExecutionError::OrderedResultAllocation {
                admitted_results: work_items,
            }
        })?;
        match &self.pool {
            None => {
                for ordinal in 0..work_items {
                    results.push(operation(ordinal));
                }
            }
            // Rayon reuses the capacity acquired above: its indexed
            // `collect_into_vec` reserve is therefore allocation-free here.
            Some(pool) => pool.install(|| {
                (0..work_items)
                    .into_par_iter()
                    .map(operation)
                    .collect_into_vec(&mut results);
            }),
        }
        debug_assert_eq!(results.len(), work_items);
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::sync::{Arc, Barrier, Condvar, Mutex};

    #[test]
    fn one_core_is_inline_and_ordered() {
        let execution = ParallelExecution::try_new(1, 8).unwrap();
        assert_eq!(execution.n_cores(), 1);
        assert_eq!(execution.worker_thread_count(), 0);
        assert_eq!(
            execution
                .map_ordered(8, |ordinal| ordinal * ordinal)
                .unwrap(),
            vec![0, 1, 4, 9, 16, 25, 36, 49]
        );
    }

    #[test]
    fn ordered_batch_cannot_exceed_its_retained_admission() {
        let execution = ParallelExecution::try_new(1, 1).unwrap();
        let invocations = std::sync::atomic::AtomicUsize::new(0);
        let result = execution.map_ordered(2, |ordinal| {
            invocations.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            ordinal
        });
        assert_eq!(
            result,
            Err(ParallelExecutionError::OrderedResultCeilingExceeded {
                work_items: 2,
                admitted_ceiling: 1,
            })
        );
        assert_eq!(invocations.load(std::sync::atomic::Ordering::Relaxed), 0);
    }

    #[test]
    fn ordered_result_allocation_fails_before_work_starts() {
        let execution = ParallelExecution::try_new(1, usize::MAX).unwrap();
        let invocations = std::sync::atomic::AtomicUsize::new(0);
        let result = execution.map_ordered::<u8, _>(usize::MAX, |_| {
            invocations.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            0
        });
        assert_eq!(
            result,
            Err(ParallelExecutionError::OrderedResultAllocation {
                admitted_results: usize::MAX,
            })
        );
        assert_eq!(invocations.load(std::sync::atomic::Ordering::Relaxed), 0);
    }

    #[test]
    fn zero_core_budget_is_rejected() {
        assert!(matches!(
            ParallelExecution::try_new(0, 0),
            Err(ParallelExecutionError::ZeroCoreBudget)
        ));
    }

    #[test]
    fn oversubscribed_core_budget_is_rejected_before_pool_construction() {
        let available = std::thread::available_parallelism().unwrap().get();
        let requested = available.checked_add(1).unwrap();
        assert!(matches!(
            ParallelExecution::try_new(requested, 0),
            Err(ParallelExecutionError::CoreBudgetExceedsAvailable {
                requested: actual_requested,
                available: actual_available,
            }) if actual_requested == requested && actual_available == available
        ));
    }

    #[test]
    fn licensed_parallel_map_uses_the_owned_workers_and_preserves_order() {
        if !LicenseManager::is_licensed() {
            return;
        }
        let n_cores = std::thread::available_parallelism().unwrap().get().min(4);
        if n_cores < 2 {
            return;
        }
        let execution = ParallelExecution::try_new(n_cores, n_cores).unwrap();
        assert_eq!(execution.worker_thread_count(), n_cores);
        let barrier = Arc::new(Barrier::new(n_cores));
        let worker_ids = Arc::new(Mutex::new(HashSet::new()));
        let output = execution
            .map_ordered(n_cores, {
                let barrier = Arc::clone(&barrier);
                let worker_ids = Arc::clone(&worker_ids);
                move |ordinal| {
                    worker_ids
                        .lock()
                        .unwrap()
                        .insert(std::thread::current().id());
                    barrier.wait();
                    ordinal
                }
            })
            .unwrap();
        assert_eq!(output, (0..n_cores).collect::<Vec<_>>());
        assert_eq!(worker_ids.lock().unwrap().len(), n_cores);
    }

    #[test]
    fn licensed_parallel_map_keeps_lowest_ordinal_error_first() {
        if !LicenseManager::is_licensed() || std::thread::available_parallelism().unwrap().get() < 2
        {
            return;
        }
        let execution = ParallelExecution::try_new(2, 2).unwrap();
        let later_ordinal_finished = Arc::new((Mutex::new(false), Condvar::new()));
        let results = execution
            .map_ordered(2, {
                let later_ordinal_finished = Arc::clone(&later_ordinal_finished);
                move |ordinal| -> Result<(), usize> {
                    let (finished, changed) = &*later_ordinal_finished;
                    if ordinal == 0 {
                        let mut finished = finished.lock().unwrap();
                        while !*finished {
                            finished = changed.wait(finished).unwrap();
                        }
                    } else {
                        *finished.lock().unwrap() = true;
                        changed.notify_one();
                    }
                    Err(ordinal)
                }
            })
            .unwrap();
        assert_eq!(results, vec![Err(0), Err(1)]);
        assert_eq!(results.into_iter().collect::<Result<Vec<_>, _>>(), Err(0));
    }

    #[test]
    fn execution_context_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ParallelExecution>();
    }
}
