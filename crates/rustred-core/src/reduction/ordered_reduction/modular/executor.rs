//! Request-private execution of the modular jobs.
//!
//! One thread runs every job inline on the calling thread: no pool is built
//! and no thread is spawned. More threads use one private rayon pool per
//! request (never the global pool). Maps preserve the input order, so results
//! never depend on the thread count.
use std::collections::HashSet;
use std::sync::Mutex;
use std::thread::ThreadId;

use rayon::prelude::*;

use super::error;
use crate::reduction::ReductionError;

pub(super) struct Executor {
    /// `None` runs inline on the calling thread.
    pool: Option<rayon::ThreadPool>,
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

    /// Order-preserving map over owned items.
    pub fn map<T, R, F>(&self, items: Vec<T>, f: F) -> Vec<R>
    where
        T: Send,
        R: Send,
        F: Fn(T) -> R + Sync + Send,
    {
        let job = |item: T| {
            self.note();
            f(item)
        };
        match &self.pool {
            None => items.into_iter().map(job).collect(),
            Some(pool) => pool.install(|| items.into_par_iter().map(job).collect()),
        }
    }

    /// Order-preserving map over mutable items with their index.
    pub fn map_mut<T, R, F>(&self, items: &mut [T], f: F) -> Vec<R>
    where
        T: Send,
        R: Send,
        F: Fn(usize, &mut T) -> R + Sync + Send,
    {
        let job = |(index, item): (usize, &mut T)| {
            self.note();
            f(index, item)
        };
        match &self.pool {
            None => items.iter_mut().enumerate().map(job).collect(),
            Some(pool) => pool.install(|| items.par_iter_mut().enumerate().map(job).collect()),
        }
    }
}
