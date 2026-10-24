//! Bounded sector-level parallelism with shared sources and compact results.

use std::fmt;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

use rayon::prelude::*;
use rayon::{ThreadPool, ThreadPoolBuilder};
use symbolica::license::LicenseManager;
use symbolica::utils::resources::ResourceBudget;

use super::{
    SectorConfig, SectorEvent, SectorSolution, SectorSolveError, SectorSolveOptions, SectorSolver,
    SectorVisitOrder, SolverError, SourceSystem,
};

mod shared;
use shared::Admission;
pub use shared::SharedSectorSnapshot;

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
    ZeroActiveSectors,
    UnsupportedTargetWorkers { requested: usize },
    NativeThreadLimit { requested: usize, allowed: usize },
    WorkerPool { requested: usize, message: String },
}

impl fmt::Display for SectorExecutorBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroWorkers => write!(f, "sector worker budget must be positive"),
            Self::ZeroActiveSectors => write!(f, "active-sector limit must be positive or omitted"),
            Self::UnsupportedTargetWorkers { requested } => write!(
                f,
                "this WebAssembly build supports exactly one inline sector worker, not {requested}"
            ),
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
    /// Cooperative stop before starting a sector; running sectors are drained.
    Cancelled { ordinal: usize, sector: [bool; N] },
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
            Self::Cancelled { ordinal, .. } => *ordinal,
            Self::Prepare { ordinal, .. }
            | Self::Solve { ordinal, .. }
            | Self::Consume { ordinal, .. } => *ordinal,
        }
    }

    pub fn sector(&self) -> &[bool; N] {
        match self {
            Self::Cancelled { sector, .. } => sector,
            Self::Prepare { sector, .. }
            | Self::Solve { sector, .. }
            | Self::Consume { sector, .. } => sector,
        }
    }
}

impl<const N: usize, E: fmt::Display> fmt::Display for SectorExecutionError<N, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled { ordinal, .. } => write!(f, "sector {ordinal} cancelled before start"),
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
        }?;
        write!(f, "\nsector_mask=")?;
        for active in self.sector() {
            f.write_str(if *active { "1" } else { "0" })?;
        }
        Ok(())
    }
}

impl<const N: usize, E: std::error::Error + 'static> std::error::Error
    for SectorExecutionError<N, E>
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Cancelled { .. } => None,
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
#[derive(Clone)]
pub struct SectorExecutor {
    workers: usize,
    scheduling: SectorScheduling,
    visit_order: Option<SectorVisitOrder>,
    pool: Option<Arc<ThreadPool>>,
    cancellation: Option<Arc<AtomicBool>>,
    shared: Option<Arc<Admission>>,
}

impl fmt::Debug for SectorExecutor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SectorExecutor")
            .field("workers", &self.workers)
            .field("scheduling", &self.scheduling)
            .field("visit_order", &self.visit_order)
            .field("owns_pool", &self.pool.is_some())
            .finish()
    }
}

impl SectorExecutor {
    pub fn new(workers: usize) -> Result<Self, SectorExecutorBuildError> {
        if workers == 0 {
            return Err(SectorExecutorBuildError::ZeroWorkers);
        }
        if cfg!(target_arch = "wasm32") && workers != 1 {
            return Err(SectorExecutorBuildError::UnsupportedTargetWorkers { requested: workers });
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
            Some(Arc::new(pool))
        };
        Ok(Self {
            workers,
            scheduling: SectorScheduling::default(),
            visit_order: None,
            pool,
            cancellation: None,
            shared: None,
        })
    }

    pub fn workers(&self) -> usize {
        self.workers
    }

    /// Share this pool between independent family coordinators. Clones retain
    /// one worker budget and one optional global admission limit. By default
    /// every sector may be queued; no extra active-sector cap is imposed.
    /// Coordinators must run outside this pool: admission waits never occupy
    /// compute workers, including when Symbolica submits nested Rayon work.
    pub fn with_shared_scheduling(
        mut self,
        max_active_sectors: Option<usize>,
    ) -> Result<Self, SectorExecutorBuildError> {
        if max_active_sectors == Some(0) {
            return Err(SectorExecutorBuildError::ZeroActiveSectors);
        }
        if self.pool.is_none() {
            let pool = ThreadPoolBuilder::new()
                .num_threads(self.workers)
                .thread_name(|ordinal| format!("rustred-sector-{ordinal}"))
                .build()
                .map_err(|error| SectorExecutorBuildError::WorkerPool {
                    requested: self.workers,
                    message: error.to_string(),
                })?;
            self.pool = Some(Arc::new(pool));
        }
        self.shared = Some(Arc::new(Admission::new(self.workers, max_active_sectors)));
        Ok(self)
    }

    pub fn shared_snapshot(&self) -> Option<SharedSectorSnapshot> {
        self.shared.as_ref().map(|shared| shared.snapshot())
    }

    /// Run preparation/assembly on the same compute pool as sector solves.
    pub fn install<R: Send>(&self, operation: impl FnOnce() -> R + Send) -> R {
        let resources = ResourceBudget::capture_current();
        match &self.pool {
            Some(pool) => pool.install(|| resources.run_captured(operation)),
            None => resources.run_captured(operation),
        }
    }

    /// Default-off cooperative cancellation. No CAS operation is interrupted;
    /// all started workers/consumers finish before the executor returns.
    pub fn with_cancellation(mut self, token: Arc<AtomicBool>) -> Self {
        self.cancellation = Some(token);
        self
    }

    pub fn scheduling(&self) -> SectorScheduling {
        self.scheduling
    }

    pub fn with_scheduling(mut self, scheduling: SectorScheduling) -> Self {
        self.scheduling = scheduling;
        self.visit_order = None;
        self
    }

    /// Override scheduling with a validated finite plan. It must match the
    /// next call's full input inventory, including any repeated sector masks.
    /// Results/source IDs remain in original order; parallel start order is
    /// not a serial dependency barrier. No callback runs in the worker engine.
    /// An empty input remains a no-op, as with the ordinary executor.
    pub fn with_sector_visit_order(mut self, plan: SectorVisitOrder) -> Self {
        self.visit_order = Some(plan);
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
        self.map_configured_with_error_observer(
            sources,
            sectors,
            configure,
            options,
            observe,
            |_| {},
            consume,
        )
    }

    /// Like [`Self::map_configured_with_observer`], reporting each preparation,
    /// search or consumer failure on its worker before that job returns.
    ///
    /// The borrowed error is the same error retained for ordered collection;
    /// observing it does not cancel other jobs or change the final error's
    /// manifest ordering. Failure callbacks may overlap. Panics, including
    /// observer panics, propagate normally and are not reported as job errors.
    #[allow(clippy::too_many_arguments)]
    pub fn map_configured_with_error_observer<const N: usize, T, E, C, O, H, F>(
        &self,
        sources: &SourceSystem<N>,
        sectors: &[[bool; N]],
        configure: C,
        options: SectorSolveOptions,
        observe: O,
        observe_error: H,
        consume: F,
    ) -> Result<Vec<T>, SectorExecutionError<N, E>>
    where
        T: Send,
        E: Send,
        C: Fn(usize, [bool; N]) -> SectorConfig<N> + Send + Sync,
        O: Fn(usize, [bool; N], SectorEvent<'_, N>) + Send + Sync,
        H: Fn(&SectorExecutionError<N, E>) + Send + Sync,
        F: Fn(SectorCompleted<N>) -> Result<T, E> + Send + Sync,
    {
        let mut jobs: Vec<_> = if let Some(plan) = &self.visit_order {
            if let Err(source) =
                super::discovery_strategy::validate_visit_order(plan.ordinals(), sectors.len())
            {
                // An empty inventory executes no work. Any nonempty inventory
                // rejects the mismatch before configure/prepare/observe.
                if let Some(&sector) = sectors.first() {
                    let error = SectorExecutionError::Prepare {
                        ordinal: 0,
                        sector,
                        source,
                    };
                    observe_error(&error);
                    return Err(error);
                }
                return Ok(Vec::new());
            }
            plan.ordinals().to_vec()
        } else {
            (0..sectors.len()).collect()
        };
        if self.visit_order.is_none() && self.scheduling == SectorScheduling::ActiveFirst {
            jobs.sort_unstable_by_key(|&ordinal| {
                (
                    std::cmp::Reverse(sectors[ordinal].iter().filter(|&&active| active).count()),
                    sectors[ordinal],
                    ordinal,
                )
            });
        }
        let resources = ResourceBudget::capture_current();
        let operation = |ordinal: usize| {
            resources.run_captured(|| {
                let _running = self.shared.as_ref().map(|shared| shared.running());
                let sector = sectors[ordinal];
                let result = (|| {
                    if self
                        .cancellation
                        .as_ref()
                        .is_some_and(|token| token.load(Ordering::Acquire))
                    {
                        return Err(SectorExecutionError::Cancelled { ordinal, sector });
                    }
                    let start = Instant::now();
                    let solver = SectorSolver::new(sources, sector, configure(ordinal, sector))
                        .map_err(|source| SectorExecutionError::Prepare {
                            ordinal,
                            sector,
                            source,
                        })?;
                    let preconditioning = start.elapsed();
                    let solution = solver
                        .solve_sector_with_observer(options, |event| {
                            observe(ordinal, sector, event)
                        })
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
                if let Err(error) = &result {
                    observe_error(error);
                }
                (ordinal, result)
            })
        };
        let mut results: Vec<_> = match (&self.pool, &self.shared) {
            (Some(pool), Some(shared)) => {
                // in_place_scope keeps this producer on its external family
                // coordinator. Blocking admission cannot deadlock Rayon workers.
                assert!(
                    pool.current_thread_index().is_none(),
                    "shared family coordinators must run outside the worker pool"
                );
                let results = Mutex::new(Vec::with_capacity(jobs.len()));
                pool.in_place_scope_fifo(|scope| {
                    for ordinal in jobs {
                        let Some(permit) = shared.acquire(self.cancellation.as_deref()) else {
                            let error = SectorExecutionError::Cancelled {
                                ordinal,
                                sector: sectors[ordinal],
                            };
                            observe_error(&error);
                            results.lock().unwrap().push((ordinal, Err(error)));
                            continue;
                        };
                        let operation = &operation;
                        let results = &results;
                        scope.spawn_fifo(move |_| {
                            let _permit = permit;
                            let result = operation(ordinal);
                            results.lock().unwrap().push(result);
                        });
                    }
                });
                results.into_inner().unwrap()
            }
            (None, _) => jobs.into_iter().map(operation).collect(),
            (Some(pool), None) => pool.install(|| jobs.into_par_iter().map(operation).collect()),
        };
        results.sort_unstable_by_key(|(ordinal, _)| *ordinal);
        results.into_iter().map(|(_, result)| result).collect()
    }
}

#[cfg(test)]
mod tests;
