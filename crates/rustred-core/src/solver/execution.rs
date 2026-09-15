//! Bounded sector-level parallelism with shared sources and compact results.

use std::fmt;
use std::time::{Duration, Instant};

use rayon::prelude::*;
use rayon::{ThreadPool, ThreadPoolBuilder};
use symbolica::license::LicenseManager;

use super::{
    SectorConfig, SectorEvent, SectorSolution, SectorSolveError, SectorSolveOptions, SectorSolver,
    SolverError, SourceSystem,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SectorScheduling {
    InputOrder,
    /// Prioritize larger sectors in the worklist without stored timing hints.
    /// Rayon still determines the actual parallel start order.
    #[default]
    ActiveFirst,
}

#[derive(Debug)]
pub enum SectorExecutorBuildError {
    ZeroWorkers,
    NativeThreadLimit { requested: usize, allowed: usize },
    WorkerPool { requested: usize, message: String },
}

impl fmt::Display for SectorExecutorBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroWorkers => write!(f, "sector worker budget must be positive"),
            Self::NativeThreadLimit { requested, allowed } => write!(
                f,
                "requested {requested} sector workers, but Symbolica permits {allowed} on the execution threads"
            ),
            Self::WorkerPool { requested, message } => {
                write!(f, "cannot build {requested}-worker sector pool: {message}")
            }
        }
    }
}

impl std::error::Error for SectorExecutorBuildError {}

/// An owned sector result delivered on the worker that produced it.
/// Consumers can write separate outputs and retain only a small summary.
#[derive(Debug)]
pub struct SectorCompleted<const N: usize> {
    pub ordinal: usize,
    pub sector: [bool; N],
    pub preconditioning: Duration,
    pub solution: SectorSolution<N>,
}

#[derive(Debug)]
pub enum SectorExecutionError<const N: usize, E> {
    Prepare {
        ordinal: usize,
        sector: [bool; N],
        source: SolverError,
    },
    Solve {
        ordinal: usize,
        sector: [bool; N],
        source: SectorSolveError<N>,
    },
    Consume {
        ordinal: usize,
        sector: [bool; N],
        source: E,
    },
}

impl<const N: usize, E> SectorExecutionError<N, E> {
    pub fn ordinal(&self) -> usize {
        match self {
            Self::Prepare { ordinal, .. }
            | Self::Solve { ordinal, .. }
            | Self::Consume { ordinal, .. } => *ordinal,
        }
    }

    pub fn sector(&self) -> &[bool; N] {
        match self {
            Self::Prepare { sector, .. }
            | Self::Solve { sector, .. }
            | Self::Consume { sector, .. } => sector,
        }
    }
}

impl<const N: usize, E: fmt::Display> fmt::Display for SectorExecutionError<N, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Prepare {
                ordinal, source, ..
            } => {
                write!(f, "sector {ordinal} preparation: {source}")
            }
            Self::Solve {
                ordinal, source, ..
            } => write!(f, "sector {ordinal} search: {source}"),
            Self::Consume {
                ordinal, source, ..
            } => {
                write!(f, "sector {ordinal} output: {source}")
            }
        }
    }
}

impl<const N: usize, E: std::error::Error + 'static> std::error::Error
    for SectorExecutionError<N, E>
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Prepare { source, .. } => Some(source),
            Self::Solve { source, .. } => Some(source),
            Self::Consume { source, .. } => Some(source),
        }
    }
}

/// A reusable, private Rayon pool for independent sector solves.
///
/// Width one executes inline, important for restricted Symbolica's
/// thread-local permit. Wider executors create one pool, never mutate the
/// global Rayon pool or process environment, and share immutable source
/// definitions. Sector bases and sparse reducers remain worker-local.
///
/// The budget covers this executor, including native Rayon work submitted
/// inside its pool. It cannot constrain independent pools or external workers
/// created by caller callbacks. Concurrent inline invocations are caller work
/// and are not a process-global one-thread reservation.
/// Nested Rayon callbacks can suspend one sector frame while executing another;
/// the thread budget is not a universal bound on live callback allocations.
pub struct SectorExecutor {
    workers: usize,
    scheduling: SectorScheduling,
    pool: Option<ThreadPool>,
}

impl fmt::Debug for SectorExecutor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SectorExecutor")
            .field("workers", &self.workers)
            .field("scheduling", &self.scheduling)
            .field("owns_pool", &self.pool.is_some())
            .finish()
    }
}

impl SectorExecutor {
    pub fn new(workers: usize) -> Result<Self, SectorExecutorBuildError> {
        if workers == 0 {
            return Err(SectorExecutorBuildError::ZeroWorkers);
        }
        let pool = if workers == 1 {
            None
        } else {
            let allowed = LicenseManager::max_threads(workers);
            if allowed < workers {
                return Err(SectorExecutorBuildError::NativeThreadLimit {
                    requested: workers,
                    allowed,
                });
            }
            let pool = ThreadPoolBuilder::new()
                .num_threads(workers)
                .thread_name(|ordinal| format!("rustred-sector-{ordinal}"))
                .build()
                .map_err(|error| SectorExecutorBuildError::WorkerPool {
                    requested: workers,
                    message: error.to_string(),
                })?;
            // A caller-local LibraryUnlock is not inherited by new workers.
            // Check native capabilities once on each execution thread, before
            // any worker algebra can request an unavailable restricted permit.
            let allowed = pool
                .broadcast(|_| LicenseManager::max_threads(workers))
                .into_iter()
                .min()
                .unwrap_or(0);
            if allowed < workers {
                return Err(SectorExecutorBuildError::NativeThreadLimit {
                    requested: workers,
                    allowed,
                });
            }
            Some(pool)
        };
        Ok(Self {
            workers,
            scheduling: SectorScheduling::default(),
            pool,
        })
    }

    pub fn workers(&self) -> usize {
        self.workers
    }

    pub fn scheduling(&self) -> SectorScheduling {
        self.scheduling
    }

    pub fn with_scheduling(mut self, scheduling: SectorScheduling) -> Self {
        self.scheduling = scheduling;
        self
    }

    /// Solve a manifest and consume each completed solution on its worker.
    /// Only the consumer's `T` is retained until ordered collection finishes.
    pub fn map<const N: usize, T, E, F>(
        &self,
        sources: &SourceSystem<N>,
        sectors: &[[bool; N]],
        config: &SectorConfig<N>,
        options: SectorSolveOptions,
        consume: F,
    ) -> Result<Vec<T>, SectorExecutionError<N, E>>
    where
        T: Send,
        E: Send,
        F: Fn(SectorCompleted<N>) -> Result<T, E> + Send + Sync,
    {
        self.map_with_observer(sources, sectors, config, options, |_, _, _| {}, consume)
    }

    /// Like [`Self::map`], with borrowed, ordinal-labelled worker progress.
    ///
    /// Returned summaries and selected failures use manifest order, regardless
    /// of scheduling or worker count. All submitted jobs finish before the
    /// first manifest-ordinal error is returned; output already written by
    /// successful callbacks is not rolled back. Progress and callback arrival
    /// order are intentionally live and nondeterministic. Callbacks must use
    /// distinct output destinations (the ordinal disambiguates repeated masks).
    pub fn map_with_observer<const N: usize, T, E, O, F>(
        &self,
        sources: &SourceSystem<N>,
        sectors: &[[bool; N]],
        config: &SectorConfig<N>,
        options: SectorSolveOptions,
        observe: O,
        consume: F,
    ) -> Result<Vec<T>, SectorExecutionError<N, E>>
    where
        T: Send,
        E: Send,
        O: Fn(usize, [bool; N], SectorEvent<'_, N>) + Send + Sync,
        F: Fn(SectorCompleted<N>) -> Result<T, E> + Send + Sync,
    {
        self.map_configured_with_observer(
            sources,
            sectors,
            |_, _| config.clone(),
            options,
            observe,
            consume,
        )
    }

    /// Solve jobs with caller-supplied per-sector configuration. This supports
    /// ordering portfolios and repeated sector masks without mutating shared
    /// sources or a global ordering table. `configure` runs once on each job's
    /// worker; its ordinal is the original manifest ordinal, not start order.
    ///
    /// The caller may share the zero-sector census through `Arc` as usual.
    /// Configuration errors and result ordering obey [`Self::map_with_observer`].
    pub fn map_configured_with_observer<const N: usize, T, E, C, O, F>(
        &self,
        sources: &SourceSystem<N>,
        sectors: &[[bool; N]],
        configure: C,
        options: SectorSolveOptions,
        observe: O,
        consume: F,
    ) -> Result<Vec<T>, SectorExecutionError<N, E>>
    where
        T: Send,
        E: Send,
        C: Fn(usize, [bool; N]) -> SectorConfig<N> + Send + Sync,
        O: Fn(usize, [bool; N], SectorEvent<'_, N>) + Send + Sync,
        F: Fn(SectorCompleted<N>) -> Result<T, E> + Send + Sync,
    {
        let mut jobs: Vec<_> = (0..sectors.len()).collect();
        if self.scheduling == SectorScheduling::ActiveFirst {
            jobs.sort_unstable_by_key(|&ordinal| {
                (
                    std::cmp::Reverse(sectors[ordinal].iter().filter(|&&active| active).count()),
                    sectors[ordinal],
                    ordinal,
                )
            });
        }
        let operation = |ordinal: usize| {
            let sector = sectors[ordinal];
            let result = (|| {
                let start = Instant::now();
                let solver = SectorSolver::new(sources, sector, configure(ordinal, sector))
                    .map_err(|source| SectorExecutionError::Prepare {
                        ordinal,
                        sector,
                        source,
                    })?;
                let preconditioning = start.elapsed();
                let solution = solver
                    .solve_sector_with_observer(options, |event| observe(ordinal, sector, event))
                    .map_err(|source| SectorExecutionError::Solve {
                        ordinal,
                        sector,
                        source,
                    })?;
                drop(solver);
                consume(SectorCompleted {
                    ordinal,
                    sector,
                    preconditioning,
                    solution,
                })
                .map_err(|source| SectorExecutionError::Consume {
                    ordinal,
                    sector,
                    source,
                })
            })();
            (ordinal, result)
        };
        let mut results: Vec<_> = match &self.pool {
            None => jobs.into_iter().map(operation).collect(),
            Some(pool) => pool.install(|| jobs.into_par_iter().map(operation).collect()),
        };
        results.sort_unstable_by_key(|(ordinal, _)| *ordinal);
        results.into_iter().map(|(_, result)| result).collect()
    }
}

#[cfg(test)]
mod tests;
