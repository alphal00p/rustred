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
    AvailableParallelism { message: String },
    CoreBudgetExceedsAvailable { requested: usize, available: usize },
    MulticoreRequiresSymbolicaLicense { requested: usize },
    WorkerPoolBuild { requested: usize, message: String },
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
    pool: Option<ThreadPool>,
}

impl fmt::Debug for ParallelExecution {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ParallelExecution")
            .field("n_cores", &self.n_cores)
            .field("has_worker_pool", &self.pool.is_some())
            .finish()
    }
}

impl ParallelExecution {
    /// Validate an invocation's requested core ceiling without creating a
    /// worker. Campaign width planning remains host-independent, then calls
    /// this preflight before consuming its checked memory plan.
    pub fn validate_requested_core_budget(n_cores: usize) -> Result<(), ParallelExecutionError> {
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
    /// Multicore execution is rejected before a RustRed worker is created if
    /// the installed Symbolica instance is not licensed for it.
    pub fn try_new(n_cores: usize) -> Result<Self, ParallelExecutionError> {
        Self::validate_requested_core_budget(n_cores)?;
        let n_cores = NonZeroUsize::new(n_cores).ok_or(ParallelExecutionError::ZeroCoreBudget)?;
        if n_cores.get() == 1 {
            return Ok(Self {
                n_cores,
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
            pool: Some(pool),
        })
    }

    pub fn n_cores(&self) -> usize {
        self.n_cores.get()
    }

    pub fn is_parallel(&self) -> bool {
        self.pool.is_some()
    }

    /// Number of owned Rayon worker threads. Inline width one has none.
    pub fn worker_thread_count(&self) -> usize {
        self.pool
            .as_ref()
            .map_or(0, rayon::ThreadPool::current_num_threads)
    }

    /// Evaluate stable work ordinals, returning results in ordinal order.
    ///
    /// The operation receives the same ordinal in serial and parallel modes.
    /// In particular, callers can collect `Result` values and then select the
    /// lowest-ordinal failure deterministically on the coordinator.
    pub fn map_ordered<ResultValue, Operation>(
        &self,
        work_items: usize,
        operation: Operation,
    ) -> Vec<ResultValue>
    where
        ResultValue: Send,
        Operation: Fn(usize) -> ResultValue + Send + Sync,
    {
        match &self.pool {
            None => (0..work_items).map(operation).collect(),
            Some(pool) => pool.install(|| (0..work_items).into_par_iter().map(operation).collect()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::sync::{Arc, Barrier, Condvar, Mutex};

    #[test]
    fn one_core_is_inline_and_ordered() {
        let execution = ParallelExecution::try_new(1).unwrap();
        assert_eq!(execution.n_cores(), 1);
        assert_eq!(execution.worker_thread_count(), 0);
        assert!(!execution.is_parallel());
        assert_eq!(
            execution.map_ordered(8, |ordinal| ordinal * ordinal),
            vec![0, 1, 4, 9, 16, 25, 36, 49]
        );
    }

    #[test]
    fn zero_core_budget_is_rejected() {
        assert!(matches!(
            ParallelExecution::try_new(0),
            Err(ParallelExecutionError::ZeroCoreBudget)
        ));
    }

    #[test]
    fn oversubscribed_core_budget_is_rejected_before_pool_construction() {
        let available = std::thread::available_parallelism().unwrap().get();
        let requested = available.checked_add(1).unwrap();
        assert!(matches!(
            ParallelExecution::try_new(requested),
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
        let execution = ParallelExecution::try_new(n_cores).unwrap();
        assert_eq!(execution.worker_thread_count(), n_cores);
        let barrier = Arc::new(Barrier::new(n_cores));
        let worker_ids = Arc::new(Mutex::new(HashSet::new()));
        let output = execution.map_ordered(n_cores, {
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
        });
        assert_eq!(output, (0..n_cores).collect::<Vec<_>>());
        assert_eq!(worker_ids.lock().unwrap().len(), n_cores);
    }

    #[test]
    fn licensed_parallel_map_keeps_lowest_ordinal_error_first() {
        if !LicenseManager::is_licensed() || std::thread::available_parallelism().unwrap().get() < 2
        {
            return;
        }
        let execution = ParallelExecution::try_new(2).unwrap();
        let later_ordinal_finished = Arc::new((Mutex::new(false), Condvar::new()));
        let results = execution.map_ordered(2, {
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
        });
        assert_eq!(results, vec![Err(0), Err(1)]);
        assert_eq!(results.into_iter().collect::<Result<Vec<_>, _>>(), Err(0));
    }

    #[test]
    fn execution_context_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ParallelExecution>();
    }
}
