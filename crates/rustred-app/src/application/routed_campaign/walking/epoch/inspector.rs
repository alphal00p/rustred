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
//! channel is C5 on the coordinator. LC2 shares immutable prepared programs
//! with thread-owned Symbolica contexts; no per-CCX replica system is added.
//! The scoped executor exposes polling/cancellation so S3 can save before join.
use super::super::OwnerDomainWalkRequest;
use super::super::initial_overlap::InitialOverlapIndex;
use super::job::{Job, JobResult};
use super::resolve::Resolver;
use rustred::solver::RoutedCandidateReducer;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

mod finite_replay;
mod pool;
pub(super) mod profile;
pub(super) use pool::{
    Activity, EscrowDiagnostics, Poll, Pool, ResultMemory, ReturnedBytes, RunError, Status,
    SubmitError, Work, with_authorized_pool, with_polling_pool,
};

pub(super) struct Context<'a, const N: usize> {
    pub reducer: &'a RoutedCandidateReducer<N>,
    pub request: &'a OwnerDomainWalkRequest,
    pub overlap: &'a InitialOverlapIndex<N>,
    pub cancellation: &'a AtomicBool,
    pub g2: Option<&'a super::super::g2::Store<N>>,
    pub finite_account: Option<&'a super::super::finite_replay::Account>,
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
    inspect_job_inner(context, bytes, None, None)
}

/// The owned lease cannot escape into result bytes. It is dropped before this
/// callback returns, therefore before the pool sends Message::Result.
pub(super) fn inspect_job_with_snapshot<const N: usize>(
    context: &Context<'_, N>,
    bytes: &[u8],
    snapshot: super::snapshot::Snapshot<N>,
) -> Vec<u8> {
    inspect_job_inner(context, bytes, Some(&snapshot), None)
}

pub(super) fn inspect_job_profiled<const N: usize>(
    context: &Context<'_, N>,
    bytes: &[u8],
    snapshot: Option<&super::snapshot::Snapshot<N>>,
    profile: &profile::Collector,
) -> Vec<u8> {
    inspect_job_inner(context, bytes, snapshot, Some(profile))
}

fn inspect_job_inner<const N: usize>(
    context: &Context<'_, N>,
    bytes: &[u8],
    snapshot: Option<&super::snapshot::Snapshot<N>>,
    profile: Option<&profile::Collector>,
) -> Vec<u8> {
    let job = match Job::<N>::decode(bytes) {
        Ok(job) => job,
        // A job the coordinator encoded cannot fail to decode; an empty
        // result fails P1's decode and is engine-fatal there.
        Err(_) => return Vec::new(),
    };
    if let Some(view) = snapshot {
        if job.v0 != view.version
            || job.parent as usize >= view.published_len
            || view.published_len != view.len()
            || view.domains[job.parent as usize] != job.image
        {
            return Vec::new(); // P1 C5; never inspect against a different view.
        }
    }
    let mut resolver = snapshot.map_or_else(Resolver::<N>::new, Resolver::with_snapshot);
    let started = Instant::now();
    let mut observation = profile.map(|_| profile::Observation::new(started));
    let finite = finite_replay::attempt(context, &job);
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let mut emit = |event| {
            if let Some(observation) = &mut observation {
                observation.event(&event);
            }
            resolver.emit(event)
        };
        if let Some(finished) = finite
            .as_ref()
            .and_then(|outcome| finite_replay::finished(&job, outcome, &mut emit))
        {
            (finished, None)
        } else if job.flags & super::job::JOB_RESCUE_ABANDONED != 0 {
            (
                super::super::inspection::abandoned(&job.image.expand(), &mut emit),
                None,
            )
        } else {
            super::g2::inspect(context, &job, &mut emit)
        }
    }));
    let mut observed_outcome = profile::Outcome::default();
    if let Some(observation) = &mut observation {
        observation.visitor_finished();
        observed_outcome.panic = outcome.is_err();
        observed_outcome.error = outcome
            .as_ref()
            .is_ok_and(|(finished, _)| finished.error.is_some());
        observed_outcome.cancel_requested = context
            .cancellation
            .load(std::sync::atomic::Ordering::Relaxed);
    }
    let prefix = resolver.prefix();
    let seconds = || started.elapsed().as_secs_f64();
    let encoded = match outcome {
        Ok((finished, part)) => catch_unwind(AssertUnwindSafe(|| {
            let mut result: JobResult<N> = resolver.finish(&job, finished);
            if job.flags & super::job::JOB_RESCUE_ABANDONED != 0 {
                result.kind = super::job::NativeKind::Abandoned;
            }
            if let Some(part) = part {
                result.kind = super::job::NativeKind::G2Residual;
                result.g2 = Some(part);
            }
            if let Some(outcome) = &finite {
                finite_replay::annotate(&mut result, outcome);
                result.seconds = seconds();
            }
            if profile.is_some() {
                observed_outcome.error |=
                    result.error.is_some() || result.break_reason != super::job::BreakReason::None;
            }
            result.encode()
        })),
        Err(_) => catch_unwind(AssertUnwindSafe(|| {
            resolver.finish_panic(&job, seconds()).encode()
        })),
    };
    let assembly_failed = encoded.is_err();
    let bytes = encoded.unwrap_or_else(|_| assembly_panic(&job, seconds(), prefix));
    if let (Some(profile), Some(observation)) = (profile, observation) {
        observed_outcome.panic |= assembly_failed;
        profile.record(&job, observation, observed_outcome);
    }
    bytes
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
    let inspect = |bytes: &[u8], _: &AtomicBool| run_job(job, bytes);
    with_polling_pool(threads, &inspect, |pool| {
        let mut run = |jobs: Vec<Vec<u8>>| -> Result<Vec<Vec<u8>>, String> {
            let count = jobs.len();
            pool.submit(
                jobs.into_iter()
                    .enumerate()
                    .map(|(key, bytes)| Work {
                        key: key as u64,
                        bytes,
                    })
                    .collect(),
            )?;
            let mut results = Vec::new();
            results
                .try_reserve_exact(count)
                .map_err(|_| "epoch result batch allocation")?;
            loop {
                match pool.poll(Duration::from_millis(50))? {
                    Poll::Result { bytes, .. } => results.push(bytes.into_vec()),
                    Poll::Started(_) | Poll::Waiting => {}
                    Poll::Drained => return Ok(results),
                }
            }
        };
        body(&mut run)
    })
}
