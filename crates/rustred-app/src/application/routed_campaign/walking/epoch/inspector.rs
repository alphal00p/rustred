//! Inspector threads (S2): each takes `JobBytes` from one shared queue,
//! runs the unchanged native visitor with empty initial orthants (every
//! successor arrives as an `Admit` with its full box) and the resolver sink,
//! and returns `ResultBytes`. A caught panic is a C3 result; the resolver
//! lives outside the unwinding frame so the emitted prefix survives. Per-CCX
//! replicas, pinning and the group queues are S4.
use super::super::OwnerDomainWalkRequest;
use super::super::initial_orthants::InitialOrthants;
use super::super::initial_overlap::InitialOverlapIndex;
use super::super::inspection;
use super::job::{Job, JobResult};
use super::resolve::Resolver;
use rustred::solver::RoutedCandidateReducer;
use std::collections::VecDeque;
use std::sync::atomic::AtomicBool;
use std::sync::{Condvar, Mutex, mpsc};
use std::time::Instant;

pub(super) struct Context<'a, const N: usize> {
    pub reducer: &'a RoutedCandidateReducer<N>,
    pub request: &'a OwnerDomainWalkRequest,
    pub overlap: &'a InitialOverlapIndex<N>,
    pub cancellation: &'a AtomicBool,
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
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
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
    let result: JobResult<N> = match outcome {
        Ok(finished) => resolver.finish(&job, finished),
        Err(_) => resolver.finish_panic(&job, started.elapsed().as_secs_f64()),
    };
    result.encode()
}

struct Queue {
    jobs: VecDeque<Vec<u8>>,
    shutdown: bool,
}

/// Run `body` with a batch executor over `threads` inspector threads (0:
/// inline on the coordinator). The executor returns one result per job, in
/// completion order; the merge sorts its cut.
pub(super) fn with_pool<const N: usize, R>(
    threads: usize,
    context: &Context<'_, N>,
    body: impl FnOnce(&mut dyn FnMut(Vec<Vec<u8>>) -> Vec<Vec<u8>>) -> R,
) -> Result<R, String> {
    if threads == 0 {
        let mut run = |jobs: Vec<Vec<u8>>| {
            jobs.iter()
                .map(|job| inspect_job(context, job))
                .collect::<Vec<_>>()
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
                        let job = {
                            let mut guard = queue.lock().expect("inspector queue");
                            loop {
                                if let Some(job) = guard.jobs.pop_front() {
                                    break Some(job);
                                }
                                if guard.shutdown {
                                    break None;
                                }
                                guard = ready.wait(guard).expect("inspector queue");
                            }
                        };
                        let Some(job) = job else { return };
                        if sender.send(inspect_job(context, &job)).is_err() {
                            return;
                        }
                    }
                });
            match spawned {
                Ok(handle) => handles.push(handle),
                Err(error) => {
                    queue.lock().expect("inspector queue").shutdown = true;
                    ready.notify_all();
                    return Err(format!("epoch inspector spawn: {error}"));
                }
            }
        }
        drop(sender);
        let mut run = |jobs: Vec<Vec<u8>>| {
            let count = jobs.len();
            {
                let mut guard = queue.lock().expect("inspector queue");
                guard.jobs.extend(jobs);
            }
            ready.notify_all();
            (0..count)
                .map(|_| receiver.recv().unwrap_or_default())
                .collect::<Vec<_>>()
        };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| body(&mut run)));
        queue.lock().expect("inspector queue").shutdown = true;
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
