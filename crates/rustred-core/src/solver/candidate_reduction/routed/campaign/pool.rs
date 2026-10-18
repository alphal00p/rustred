//! Optional persistent, bounded worker lifetime for repeated finite batches.
//! The per-batch FIFO, dependency deduplication and completion proof are the
//! existing native scheduler; Rayon supplies only reusable OS threads.
use super::*;
mod ordered;
use std::{
    cell::Cell,
    sync::{Mutex, MutexGuard, TryLockError},
};

thread_local! {
    static ACTIVE_POOL_WORKER: Cell<usize> = const { Cell::new(0) };
}

/// A nested Rayon operation may steal another outer worker-loop job. Running
/// that loop would block on the original, currently paused node. Skip only
/// this same pool's redundant worker job; its original driver still owns and
/// drains the shared queue. Other pool identities are never suppressed.
fn run_worker_without_reentry(pool_id: usize, job: impl FnOnce()) -> bool {
    let previous = ACTIVE_POOL_WORKER.with(Cell::get);
    if previous == pool_id {
        return false;
    }
    struct Restore(usize);
    impl Drop for Restore {
        fn drop(&mut self) {
            ACTIVE_POOL_WORKER.with(|active| active.set(self.0));
        }
    }
    ACTIVE_POOL_WORKER.with(|active| active.set(pool_id));
    let _restore = Restore(previous);
    job();
    true
}

pub struct CandidateRoutedWorkerPool {
    workers: usize,
    /// At most one live job group: otherwise a stolen outer job could belong
    /// to another Shared queue. Reentrant/concurrent use rejects, never waits.
    active: Mutex<()>,
    #[cfg(not(target_arch = "wasm32"))]
    pool: Option<rayon::ThreadPool>,
}

impl CandidateRoutedWorkerPool {
    pub fn try_new(workers: usize) -> Result<Self, super::super::CandidateRoutedError> {
        crate::campaign::ParallelExecution::preflight_requested_core_budget(workers)
            .map_err(|error| super::super::CandidateRoutedError::InvalidInput(error.to_string()))?;
        #[cfg(not(target_arch = "wasm32"))]
        let pool = if workers == 1 {
            None
        } else {
            Some(
                rayon::ThreadPoolBuilder::new()
                    .num_threads(workers)
                    .thread_name(|ordinal| format!("rustred-finite-{ordinal}"))
                    .build()
                    .map_err(|error| {
                        super::super::CandidateRoutedError::InvalidInput(format!(
                            "cannot start persistent campaign workers: {error}"
                        ))
                    })?,
            )
        };
        Ok(Self {
            workers,
            active: Mutex::new(()),
            #[cfg(not(target_arch = "wasm32"))]
            pool,
        })
    }

    pub fn workers(&self) -> usize {
        self.workers
    }

    pub(super) fn validate_caller(
        &self,
        workers: usize,
    ) -> Result<(), super::super::CandidateRoutedError> {
        if workers != self.workers {
            return Err(super::super::CandidateRoutedError::InvalidInput(
                "persistent campaign worker budget differs from trace budget".into(),
            ));
        }
        #[cfg(not(target_arch = "wasm32"))]
        if self
            .pool
            .as_ref()
            .is_some_and(|pool| pool.current_thread_index().is_some())
        {
            return Err(super::super::CandidateRoutedError::InvalidInput(
                "a persistent campaign cannot synchronously invoke its own worker pool".into(),
            ));
        }
        Ok(())
    }

    pub(super) fn acquire(
        &self,
        workers: usize,
    ) -> Result<MutexGuard<'_, ()>, super::super::CandidateRoutedError> {
        self.validate_caller(workers)?;
        match self.active.try_lock() {
            Ok(lease) => Ok(lease),
            Err(TryLockError::Poisoned(error)) => Ok(error.into_inner()),
            Err(TryLockError::WouldBlock) => Err(super::super::CandidateRoutedError::InvalidInput(
                "persistent campaign pool already has an active batch; use a separate pool for concurrent traces".into())),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn run<const N: usize>(
        &self,
        reducer: &RoutedCandidateReducer<N>,
        shared: &Shared<'_, N>,
        mut observer: impl FnMut(&CandidateRoutedCampaignSnapshot<N>),
    ) {
        // trace_with_pool holds the exclusive lease from before entry
        // admission through every observer and scoped worker completion.
        let Some(pool) = &self.pool else {
            worker::run_with_progress(reducer, shared, || observer(&shared.snapshot()));
            observer(&shared.snapshot());
            return;
        };
        // Unlike install/scope, in_place_scope leaves the observer on the
        // calling thread. All borrowed jobs finish before it returns; no
        // unsafe lifetime extension or resident-index reference is retained.
        let remaining = std::sync::atomic::AtomicUsize::new(self.workers);
        // Identity only, never dereferenced. The enclosing immutable borrow
        // keeps this address stable until all scoped jobs have returned.
        let pool_id = std::ptr::from_ref(self) as usize;
        pool.in_place_scope(|scope| {
            for _ in 0..self.workers {
                scope.spawn(|_| {
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        run_worker_without_reentry(pool_id, || worker::run(reducer, shared));
                    }));
                    if result.is_err() {
                        shared.fail(Failure::WorkerPanicked);
                    }
                    remaining.fetch_sub(1, std::sync::atomic::Ordering::Release);
                });
            }
            let observed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                loop {
                    let (snapshot, done) = shared.wait_snapshot(Duration::from_millis(250));
                    observer(&snapshot);
                    // A panic outside an ordinary node handler may leave an
                    // active descriptor orphaned. Once every job has joined,
                    // return the typed failure instead of waiting forever.
                    if done || remaining.load(std::sync::atomic::Ordering::Acquire) == 0 {
                        break;
                    }
                }
            }));
            if let Err(panic) = observed {
                shared.fail(CandidateRoutedCampaignFailure::Cancelled);
                std::panic::resume_unwind(panic);
            }
        });
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[test]
    fn persistent_pool_rejects_nested_self_dispatch_without_blocking() {
        if !crate::test_gates::workers_or_skip("persistent pool self-dispatch", 2) {
            return;
        }
        let execution = CandidateRoutedWorkerPool::try_new(2).unwrap();
        assert!(execution.validate_caller(1).is_err());
        assert!(
            execution
                .pool
                .as_ref()
                .unwrap()
                .install(|| execution.validate_caller(2))
                .is_err()
        );
        assert!(execution.validate_caller(2).is_ok());
    }

    #[test]
    fn nested_rayon_worker_job_skips_only_its_own_pool_and_restores_identity() {
        if !crate::test_gates::workers_or_skip("nested Rayon worker guard", 2) {
            return;
        }
        let execution = CandidateRoutedWorkerPool::try_new(2).unwrap();
        let id = std::ptr::from_ref(&execution) as usize;
        execution.pool.as_ref().unwrap().install(|| {
            assert!(run_worker_without_reentry(id, || {
                let (reentered, _) = rayon::join(
                    || {
                        run_worker_without_reentry(id, || {
                            panic!("stolen worker must not block again")
                        })
                    },
                    || run_worker_without_reentry(id, || {}),
                );
                assert!(!reentered);
                assert_eq!(ACTIVE_POOL_WORKER.with(Cell::get), id);
                assert!(run_worker_without_reentry(id ^ 1, || {
                    assert_eq!(ACTIVE_POOL_WORKER.with(Cell::get), id ^ 1);
                }));
                assert_eq!(ACTIVE_POOL_WORKER.with(Cell::get), id);
                let panic = std::panic::catch_unwind(|| {
                    run_worker_without_reentry(id ^ 1, || panic!("nested failure"));
                });
                assert!(panic.is_err());
                assert_eq!(ACTIVE_POOL_WORKER.with(Cell::get), id);
            }));
            assert_eq!(ACTIVE_POOL_WORKER.with(Cell::get), 0);
        });
    }

    #[test]
    fn persistent_pool_busy_is_typed_and_does_not_prevent_later_reuse() {
        let execution = CandidateRoutedWorkerPool::try_new(1).unwrap();
        let reducer = super::super::tests::diamond(Default::default(), Default::default());
        let cancel = AtomicBool::new(false);
        let lease = execution.acquire(1).unwrap();
        assert!(execution.acquire(1).is_err());
        drop(lease);
        let _lease = execution.acquire(1).unwrap();
        let shared = Shared::new(&reducer, 1, &cancel);
        shared
            .prepare(
                &reducer,
                [crate::family::IntegralKey::try_new([6]).unwrap()],
            )
            .unwrap();
        execution.run(&reducer, &shared, |_| {});
        assert!(shared.into_result().unwrap().snapshot().finished);
    }

    #[test]
    fn queued_outer_job_stolen_during_nested_rayon_does_not_reenter_blocking_worker() {
        if !crate::test_gates::workers_or_skip("actual nested Rayon stealing", 2) {
            return;
        }
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            mpsc,
        };
        let execution = CandidateRoutedWorkerPool::try_new(2).unwrap();
        let id = std::ptr::from_ref(&execution) as usize;
        let (blocked_send, blocked_receive) = mpsc::channel();
        let (release_send, release_receive) = mpsc::channel();
        let (done_send, done_receive) = mpsc::channel();
        execution.pool.as_ref().unwrap().in_place_scope(|scope| {
            scope.spawn(move |_| {
                blocked_send.send(()).unwrap();
                release_receive
                    .recv_timeout(Duration::from_secs(10))
                    .unwrap();
            });
            blocked_receive
                .recv_timeout(Duration::from_secs(5))
                .unwrap();
            scope.spawn(move |_| {
                assert!(run_worker_without_reentry(id, || {
                    let remaining = AtomicUsize::new(1);
                    rayon::in_place_scope(|nested| {
                        nested.spawn(|_| {
                            let entered = run_worker_without_reentry(id, || {
                                panic!("stolen outer loop would wait for its paused original node");
                            });
                            assert!(!entered);
                            // Same completion accounting as the real pool's
                            // wrapper: a skipped redundant job still joins.
                            remaining.fetch_sub(1, Ordering::Release);
                        });
                        let deadline = std::time::Instant::now() + Duration::from_secs(5);
                        while remaining.load(Ordering::Acquire) != 0 {
                            assert!(std::time::Instant::now() < deadline);
                            rayon::yield_now();
                        }
                    });
                    assert_eq!(remaining.load(Ordering::Acquire), 0);
                    assert_eq!(ACTIVE_POOL_WORKER.with(Cell::get), id);
                }));
                done_send.send(()).unwrap();
            });
            let result = done_receive.recv_timeout(Duration::from_secs(6));
            // Release even on failure, so the test cannot strand a pool job.
            release_send.send(()).unwrap();
            result.unwrap();
        });
        assert!(execution.acquire(2).is_ok());
        assert_eq!(execution.pool.as_ref().unwrap().install(|| 7), 7);
    }
}
