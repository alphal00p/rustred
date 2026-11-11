//! Request-private execution of the modular jobs.
//!
//! One thread runs every job inline on the calling thread: no pool is built
//! and no thread is spawned. More threads use one private rayon pool per
//! request (never the global pool). Maps preserve the input order, so results
//! never depend on the thread count.
//!
//! Every job starts with a resource checkpoint of the calling thread's
//! budgets (captured when the executor is built, and installed around every
//! job on the pool, so that the job's own checkpoints observe them too). A
//! refused budget, e.g. a cancelled request, skips the remaining jobs and the
//! map returns its error.
use std::collections::HashSet;
use std::sync::Mutex;
use std::thread::ThreadId;

use rayon::prelude::*;
use symbolica::utils::resources::{self, ResourceBudget, ResourceLimitError};

use super::error;
use crate::reduction::ReductionError;

pub(super) struct Executor {
    /// `None` runs inline on the calling thread.
    pool: Option<rayon::ThreadPool>,
    /// Resource budgets of the calling thread, installed around every job
    /// on the pool (inline jobs run inside them already).
    budget: ResourceBudget,
    caller: ThreadId,
    /// Threads that ran jobs (statistics and the single-thread test).
    seen: Mutex<HashSet<ThreadId>>,
}

#[cfg(test)]
thread_local! {
    static POOLS_BUILT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Pools built by executors created on the current thread (tests only).
#[cfg(test)]
pub(crate) fn pools_built_on_this_thread() -> usize {
    POOLS_BUILT.with(std::cell::Cell::get)
}

impl Executor {
    pub fn new(threads: usize) -> Result<Self, ReductionError> {
        let pool = if threads <= 1 {
            None
        } else {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .thread_name(|index| format!("rustred-modular-{index}"))
                .build()
                .map_err(|e| error(format!("modular thread pool: {e}")))?;
            #[cfg(test)]
            POOLS_BUILT.with(|built| built.set(built.get() + 1));
            Some(pool)
        };
        Ok(Self {
            pool,
            budget: ResourceBudget::capture_current(),
            caller: std::thread::current().id(),
            seen: Mutex::new(HashSet::new()),
        })
    }

    /// Worker threads (1 when inline).
    pub fn threads(&self) -> usize {
        self.pool
            .as_ref()
            .map_or(1, rayon::ThreadPool::current_num_threads)
    }

    /// rr-par: the private pool (None when inline) and the caller's budgets,
    /// for jobs that spread one job over the pool (`polynomial/parallel.rs`).
    pub fn pool(&self) -> Option<&rayon::ThreadPool> {
        self.pool.as_ref()
    }

    pub fn budget(&self) -> &ResourceBudget {
        &self.budget
    }

    /// Note the calling thread as a job thread (statistics).
    pub fn note_caller(&self) {
        self.note();
    }

    /// Threads of the private pool (0 when inline).
    pub fn pool_threads(&self) -> usize {
        self.pool
            .as_ref()
            .map_or(0, rayon::ThreadPool::current_num_threads)
    }

    /// Distinct threads that ran jobs, and whether all of them were the
    /// calling thread.
    pub fn job_threads(&self) -> (usize, bool) {
        let seen = self.seen.lock().unwrap_or_else(|e| e.into_inner());
        (seen.len(), seen.iter().all(|id| *id == self.caller))
    }

    #[inline]
    fn note(&self) {
        let id = std::thread::current().id();
        self.seen
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(id);
    }

    /// Order-preserving map over owned items (see the module docs for the
    /// resource checkpoints).
    pub fn map<T, R, F>(&self, items: Vec<T>, f: F) -> Result<Vec<R>, ResourceLimitError>
    where
        T: Send,
        R: Send,
        F: Fn(T) -> R + Sync + Send,
    {
        let job = |item: T| {
            resources::checkpoint()?;
            self.note();
            Ok(f(item))
        };
        match &self.pool {
            None => items.into_iter().map(job).collect(),
            Some(pool) => pool.install(|| {
                items
                    .into_par_iter()
                    .map(|item| self.budget.run_captured(|| job(item)))
                    .collect()
            }),
        }
    }

    /// Order-preserving map over mutable items with their index (see the
    /// module docs for the resource checkpoints). A refused budget leaves
    /// the items of the skipped jobs unchanged.
    pub fn map_mut<T, R, F>(&self, items: &mut [T], f: F) -> Result<Vec<R>, ResourceLimitError>
    where
        T: Send,
        R: Send,
        F: Fn(usize, &mut T) -> R + Sync + Send,
    {
        let job = |(index, item): (usize, &mut T)| {
            resources::checkpoint()?;
            self.note();
            Ok(f(index, item))
        };
        match &self.pool {
            None => items.iter_mut().enumerate().map(job).collect(),
            Some(pool) => pool.install(|| {
                items
                    .par_iter_mut()
                    .enumerate()
                    .map(|entry| self.budget.run_captured(|| job(entry)))
                    .collect()
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::reduction::stopping::{MESSAGE, Stopping};

    fn stopped(error: &ResourceLimitError) -> bool {
        matches!(error, ResourceLimitError::MonitorUnavailable { message, .. } if message == MESSAGE)
    }

    /// Every job checks the caller's budgets first and runs inside them, on
    /// the caller and on the pool; a refused checkpoint skips every later
    /// job and is the map's error.
    #[test]
    fn jobs_run_inside_the_caller_budgets_and_stop_at_a_refused_checkpoint() {
        const JOBS: usize = 64;
        for threads in [1, 4] {
            // One checkpoint before each job, one inside it.
            let clean = Stopping::armed(None);
            let doubled = clean
                .run(|| {
                    Executor::new(threads)
                        .unwrap()
                        .map((0..JOBS).collect(), |i: usize| {
                            resources::checkpoint().unwrap();
                            2 * i
                        })
                })
                .unwrap();
            assert_eq!(doubled, (0..JOBS).map(|i| 2 * i).collect::<Vec<_>>());
            assert_eq!(clean.seen(), 2 * JOBS, "{threads}");
            // Inline jobs stop deterministically; on the pool, a refusal
            // early enough leaves later jobs to skip.
            let stops: &[usize] = if threads == 1 {
                &[1, 2, 3, JOBS + 1, 2 * JOBS - 1]
            } else {
                &[1, 2, 3, 17]
            };
            for &stop_at in stops {
                let stopping = Stopping::armed(Some(stop_at));
                let finished = AtomicUsize::new(0);
                let error = stopping
                    .run(|| {
                        Executor::new(threads)
                            .unwrap()
                            .map((0..JOBS).collect(), |_: usize| {
                                if resources::checkpoint().is_ok() {
                                    finished.fetch_add(1, Ordering::SeqCst);
                                }
                            })
                    })
                    .unwrap_err();
                assert!(stopped(&error), "{error}");
                let finished = finished.into_inner();
                assert!(finished < JOBS, "{threads} {stop_at}: {finished}");
                if threads == 1 {
                    // Inline jobs stop deterministically: job i checks at 2i+1
                    // and 2i+2.
                    assert_eq!(finished, (stop_at - 1) / 2, "{stop_at}");
                }
            }
            // map_mut: the items of skipped jobs stay unchanged.
            let stopping = Stopping::armed(Some(JOBS / 2 + 1));
            let mut items = vec![0usize; JOBS];
            let error = stopping
                .run(|| {
                    Executor::new(threads)
                        .unwrap()
                        .map_mut(&mut items, |i, item| *item = i + 1)
                })
                .unwrap_err();
            assert!(stopped(&error));
            let touched = items.iter().filter(|&&item| item != 0).count();
            assert!(touched <= JOBS / 2, "{threads}: {touched}");
            if threads == 1 {
                assert_eq!(touched, JOBS / 2);
            }
        }
    }
}
