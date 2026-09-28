//! Inspector threads (S2): each takes `JobBytes` from one shared queue,
//! runs the unchanged native visitor with empty initial orthants (every
//! successor arrives as an `Admit` with its full box) and the resolver sink,
//! and returns `ResultBytes`. A caught panic is a C3 result; the resolver
//! lives outside the native's unwinding frame so the emitted prefix
//! survives. Assembling and encoding the result run inside their own unwind
//! frames too (a panic there is a C3 result without stats), and the worker
//! loop catches anything left (an empty result, refused by P1 as C5), so a
//! job always yields exactly one result and the coordinator never waits for
//! a result that cannot come. A poisoned queue lock or a disconnected
//! channel is C5 on the coordinator. Per-CCX replicas, pinning, the group
//! queues and a submit/poll result channel for rolling cuts are S4.
use super::super::OwnerDomainWalkRequest;
use super::super::initial_orthants::InitialOrthants;
use super::super::initial_overlap::InitialOverlapIndex;
use super::super::inspection;
use super::job::{Job, JobResult};
use super::resolve::Resolver;
use rustred::solver::RoutedCandidateReducer;
use std::collections::VecDeque;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::AtomicBool;
use std::sync::{Condvar, Mutex, mpsc};
use std::time::Instant;

pub(super) struct Context<'a, const N: usize> {
    pub reducer: &'a RoutedCandidateReducer<N>,
    pub request: &'a OwnerDomainWalkRequest,
    pub overlap: &'a InitialOverlapIndex<N>,
    pub cancellation: &'a AtomicBool,
}

/// A C3 result without stats for a panic outside the native call (result
/// assembly or encoding); the emitted prefix counts are kept.
fn assembly_panic<const N: usize>(job: &Job<N>, seconds: f64, prefix: (u64, u64)) -> Vec<u8> {
    catch_unwind(|| {
        let mut result = Resolver::<N>::new().finish_panic(job, seconds);
        result.emitted = prefix.0;
        result.accepted = prefix.1;
        result.error = Some("panic while assembling the inspection result".into());
        result.encode()
    })
    .unwrap_or_default()
}

/// One job, bytes in, bytes out.
pub(super) fn inspect_job<const N: usize>(context: &Context<'_, N>, bytes: &[u8]) -> Vec<u8> {
    let job = match Job::<N>::decode(bytes) {
        Ok(job) => job,
        // A job the coordinator encoded cannot fail to decode; an empty
        // result fails P1's decode and is engine-fatal there.
        Err(_) => return Vec::new(),
    };
    let domain = job.image.expand();
    let mut resolver = Resolver::<N>::new();
    let started = Instant::now();
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        inspection::inspect(
            context.reducer,
            &domain,
            context.request,
            context.cancellation,
            &InitialOrthants::empty(),
            context.overlap,
            &mut |event| resolver.emit(event),
        )
    }));
    let prefix = resolver.prefix();
    let seconds = || started.elapsed().as_secs_f64();
    let encoded = match outcome {
        Ok(finished) => catch_unwind(AssertUnwindSafe(|| {
            let result: JobResult<N> = resolver.finish(&job, finished);
            result.encode()
        })),
        Err(_) => catch_unwind(AssertUnwindSafe(|| {
            resolver.finish_panic(&job, seconds()).encode()
        })),
    };
    encoded.unwrap_or_else(|_| assembly_panic(&job, seconds(), prefix))
}

struct Queue {
    jobs: VecDeque<Vec<u8>>,
    shutdown: bool,
}

/// The batch executor handed to the merge loop: one result per job, in
/// completion order (the merge sorts its cut). Err is engine-fatal (C5).
pub(super) type RunBatch<'a> = dyn FnMut(Vec<Vec<u8>>) -> Result<Vec<Vec<u8>>, String> + 'a;

/// Run one job function under a last-resort unwind frame: a panic yields an
/// empty result (P1 refuses it: C5) instead of a lost result.
fn run_job(job: &(dyn Fn(&[u8]) -> Vec<u8> + Sync), bytes: &[u8]) -> Vec<u8> {
    catch_unwind(AssertUnwindSafe(|| job(bytes))).unwrap_or_default()
}

/// Run `body` with a batch executor over `threads` inspector threads (0:
/// inline on the coordinator). `job` turns one job's bytes into its
/// result's bytes (`inspect_job` in the engine).
pub(super) fn with_pool<R>(
    threads: usize,
    job: &(dyn Fn(&[u8]) -> Vec<u8> + Sync),
    body: impl FnOnce(&mut RunBatch<'_>) -> R,
) -> Result<R, String> {
    if threads == 0 {
        let mut run = |jobs: Vec<Vec<u8>>| -> Result<Vec<Vec<u8>>, String> {
            Ok(jobs.iter().map(|bytes| run_job(job, bytes)).collect())
        };
        return Ok(body(&mut run));
    }
    let queue = Mutex::new(Queue {
        jobs: VecDeque::new(),
        shutdown: false,
    });
    let ready = Condvar::new();
    let (sender, receiver) = mpsc::channel::<Vec<u8>>();
    std::thread::scope(|scope| {
        let mut handles = Vec::with_capacity(threads);
        for slot in 0..threads {
            let queue = &queue;
            let ready = &ready;
            let sender = sender.clone();
            let spawned = std::thread::Builder::new()
                .name(format!("epoch-inspector-{slot}"))
                .spawn_scoped(scope, move || {
                    loop {
                        let next = {
                            // A poisoned lock ends the worker: its sender
                            // drops, and the coordinator sees C5.
                            let Ok(mut guard) = queue.lock() else { return };
                            loop {
                                if let Some(bytes) = guard.jobs.pop_front() {
                                    break Some(bytes);
                                }
                                if guard.shutdown {
                                    break None;
                                }
                                guard = match ready.wait(guard) {
                                    Ok(guard) => guard,
                                    Err(_) => return,
                                };
                            }
                        };
                        let Some(bytes) = next else { return };
                        if sender.send(run_job(job, &bytes)).is_err() {
                            return;
                        }
                    }
                });
            match spawned {
                Ok(handle) => handles.push(handle),
                Err(error) => {
                    if let Ok(mut guard) = queue.lock() {
                        guard.shutdown = true;
                    }
                    ready.notify_all();
                    return Err(format!("epoch inspector spawn: {error}"));
                }
            }
        }
        drop(sender);
        let mut run = |jobs: Vec<Vec<u8>>| -> Result<Vec<Vec<u8>>, String> {
            let count = jobs.len();
            queue
                .lock()
                .map_err(|_| "epoch inspector queue lock poisoned (C5)".to_string())?
                .jobs
                .extend(jobs);
            ready.notify_all();
            (0..count)
                .map(|_| {
                    receiver.recv().map_err(|_| {
                        "epoch inspector result channel disconnected with results outstanding (C5)"
                            .to_string()
                    })
                })
                .collect()
        };
        let result = catch_unwind(AssertUnwindSafe(|| body(&mut run)));
        if let Ok(mut guard) = queue.lock() {
            guard.shutdown = true;
        }
        ready.notify_all();
        for handle in handles {
            let _ = handle.join();
        }
        match result {
            Ok(result) => Ok(result),
            Err(panic) => std::panic::resume_unwind(panic),
        }
    })
}
