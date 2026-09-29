//! Private restored-state lockstep controller. Not called by public resume.
//! Owns all merge mutation and saves inside the scoped executor before join.
use super::super::super::{
    MergeBoundary, invalid,
    metadata::{Admission, Identity, Inputs},
    publication, stop,
};
use super::Restored;
use crate::application::routed_campaign::walking::epoch::{
    dispatch::Refill,
    inspector::{
        Context, Poll, RunError, Status, SubmitError, Work, inspect_job, with_authorized_pool,
    },
    merge::{self, Fatal, MergeConfig, RecordOut, StopReason},
    records,
};
use crate::application::routed_campaign::walking::execution::records::Sidecar;
use serde_json::Value;
use std::io;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::AtomicBool;
use std::time::Duration;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Outcome {
    Drained,
    Stopped(StopReason),
}

enum Failure {
    Engine(String),
    Save(io::Error),
}
impl From<Fatal> for Failure {
    fn from(error: Fatal) -> Self {
        Self::Engine(error.0)
    }
}

pub(super) fn initial_stop(config: MergeConfig, input_frontiers: usize) -> Option<StopReason> {
    (config.frontier_stop && input_frontiers != 0).then_some(StopReason::FrontierStop)
}

/// The executor distinguishes refusals before dispatch from failed accepted
/// protocol/work. Only the latter invalidates mathematical engine authority.
pub(super) fn pool_error<const N: usize>(restored: &mut Restored<N>, error: RunError) -> io::Error {
    match error {
        RunError::Capability(error) => io::Error::new(io::ErrorKind::PermissionDenied, error),
        RunError::Resource(error) => io::Error::other(error),
        RunError::Engine(error) => {
            if let Err(poison) = restored.publisher.poison(restored.state.k, &error) {
                io::Error::other(format!("{error}; durable poison failed: {poison}"))
            } else {
                io::Error::other(error)
            }
        }
    }
}

struct Output<'a>(&'a mut Sidecar);
impl RecordOut for Output<'_> {
    fn reserve(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn push(&mut self, record: Value) -> Result<(), String> {
        self.0.push(&record)
    }
}

fn save<const N: usize>(
    restored: &mut Restored<N>,
    identity: &Identity<'_>,
    b: usize,
    reason: Option<StopReason>,
    context: Option<stop::Stop>,
) -> Result<publication::Receipt, Failure> {
    // In particular no forced closure refresh on an interrupted/memory save.
    let boundary = MergeBoundary::borrow(&restored.state, &restored.dispatch, b)
        .map_err(|error| Failure::Engine(error.to_string()))?;
    let inputs = Inputs {
        identity,
        admission: restored.admission,
        rows: &restored.roots.rows,
        frontiers: &restored.roots.frontiers,
        stop: reason,
        operational_stop: context.as_ref(),
    };
    let receipt = restored
        .publisher
        .save(&boundary, &inputs, &mut restored.records)
        .map_err(|error| {
            if error.kind() == io::ErrorKind::InvalidData {
                Failure::Engine(error.to_string())
            } else {
                Failure::Save(error)
            }
        })?;
    restored.stop_reason = reason.map(|reason| reason.name().to_owned());
    restored.operational_stop = context;
    Ok(receipt)
}

/// Budget counts inspector threads PLUS one controller. W1 remains the existing
/// inline public path; this private responsive path conservatively requires W>=2.
/// The supplied native callback must authorize CAS on the actual worker thread.
#[allow(clippy::too_many_arguments)]
pub(super) fn run_with<const N: usize>(
    restored: &mut Restored<N>,
    identity: &Identity<'_>,
    b: usize,
    budget: usize,
    config: MergeConfig,
    inspect: &(dyn Fn(&[u8], &AtomicBool) -> Vec<u8> + Sync),
    stop_requested: impl FnMut() -> Option<stop::Stop>,
    on_saved: impl FnMut(&publication::Receipt, &[Status]),
) -> io::Result<Outcome> {
    run_authorized(
        restored,
        identity,
        b,
        budget,
        config,
        &|| Ok(()),
        inspect,
        stop_requested,
        on_saved,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run_authorized<const N: usize>(
    restored: &mut Restored<N>,
    identity: &Identity<'_>,
    b: usize,
    budget: usize,
    config: MergeConfig,
    authorize: &(dyn Fn() -> Result<(), String> + Sync),
    inspect: &(dyn Fn(&[u8], &AtomicBool) -> Vec<u8> + Sync),
    mut stop_requested: impl FnMut() -> Option<stop::Stop>,
    mut on_saved: impl FnMut(&publication::Receipt, &[Status]),
) -> io::Result<Outcome> {
    if budget < 2
        || !(1..=4096).contains(&b)
        || !config.lockstep
        || config.g2
        || !matches!(restored.admission, Admission::Complete)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "private responsive epoch controller needs complete admission, lockstep, no G2 and budget >= 2",
        ));
    }
    // Shape validation occurs before workers exist or any replay payload moves.
    MergeBoundary::borrow(&restored.state, &restored.dispatch, b)?;
    let outcome = with_authorized_pool(budget - 1, authorize, inspect, |pool| {
        let step = catch_unwind(AssertUnwindSafe(|| -> Result<Outcome, Failure> {
            if let Some(reason) = initial_stop(config, restored.roots.frontiers.len()) {
                let receipt = save(restored, identity, b, Some(reason), None)?;
                on_saved(&receipt, &[]);
                return Ok(Outcome::Stopped(reason));
            }
            loop {
                if let Some(context) = stop_requested() {
                    pool.cancel().map_err(Failure::Engine)?;
                    let reason = context.kind();
                    let receipt = save(restored, identity, b, Some(reason), Some(context))?;
                    // Any prior batch has already merged; its old worker
                    // observations must not masquerade as current in-flight.
                    on_saved(&receipt, &[]);
                    return Ok(Outcome::Stopped(reason));
                }
                let jobs = if !restored.replay.is_empty() {
                    std::mem::take(&mut restored.replay)
                } else {
                    match restored.dispatch.refill(&mut restored.state, b) {
                        Refill::Jobs(jobs) => jobs,
                        Refill::Drained => {
                            let (certified, _) =
                                crate::application::routed_campaign::walking::epoch::certification(
                                    &restored.state,
                                    true,
                                    false,
                                    restored.roots.frontiers.len(),
                                );
                            let receipt = save(
                                restored,
                                identity,
                                b,
                                (!certified).then_some(StopReason::DrainedUncertified),
                                None,
                            )?;
                            on_saved(&receipt, &[]);
                            return Ok(Outcome::Drained);
                        }
                        Refill::SequenceExhausted => {
                            let receipt =
                                save(restored, identity, b, Some(StopReason::Capacity), None)?;
                            on_saved(&receipt, &[]);
                            return Ok(Outcome::Stopped(StopReason::Capacity));
                        }
                        Refill::Stalled => {
                            return Err(Failure::Engine("restored epoch dispatch stalled".into()));
                        }
                    }
                };
                let mut work = Vec::new();
                let mut results = Vec::new();
                // Both lists are B-bounded, never an arena/snapshot clone.
                if work.try_reserve_exact(jobs.len()).is_err()
                    || results.try_reserve_exact(jobs.len()).is_err()
                {
                    drop(jobs);
                    pool.cancel().map_err(Failure::Engine)?;
                    let receipt = save(restored, identity, b, Some(StopReason::RamGuard), None)?;
                    on_saved(&receipt, &[]);
                    return Ok(Outcome::Stopped(StopReason::RamGuard));
                }
                for job in jobs {
                    work.push(Work {
                        key: job.seq,
                        bytes: job.encode(),
                    });
                }
                match pool.submit(work) {
                    Ok(()) => {}
                    Err(SubmitError::Protocol(error)) => return Err(Failure::Engine(error.into())),
                    Err(SubmitError::Allocation(_)) => {
                        pool.cancel().map_err(Failure::Engine)?;
                        let receipt =
                            save(restored, identity, b, Some(StopReason::RamGuard), None)?;
                        on_saved(&receipt, &[]);
                        return Ok(Outcome::Stopped(StopReason::RamGuard));
                    }
                }
                loop {
                    if let Some(context) = stop_requested() {
                        // No partial cut reaches P1. Release both already-polled
                        // bytes and queued/late channel results BEFORE streaming save.
                        pool.cancel().map_err(Failure::Engine)?;
                        drop(results);
                        let status = pool.take_cancelled_status().map_err(Failure::Engine)?;
                        let reason = context.kind();
                        let receipt = save(restored, identity, b, Some(reason), Some(context))?;
                        on_saved(&receipt, &status);
                        return Ok(Outcome::Stopped(reason));
                    }
                    match pool
                        .poll(Duration::from_millis(50))
                        .map_err(Failure::Engine)?
                    {
                        Poll::Started(_) | Poll::Waiting => {}
                        Poll::Result { bytes, .. } => results.push(bytes),
                        Poll::Drained => break,
                    }
                }
                // P1 sorts the complete cut; first-error handling is independent
                // of worker completion order. Existing P1-P3 semantics unchanged.
                let checked = merge::p1_check(&mut restored.state, results, config)?;
                let reason = if let Some(reason) = checked.stop {
                    merge::discard_cut(&mut restored.state, &checked, &mut |id, attempts| {
                        restored.dispatch.requeue(id, attempts)
                    })?;
                    Some(reason)
                } else {
                    let plan = merge::p2(&mut restored.state, &checked)?;
                    if let Err(reason) = merge::p3_preflight(
                        &mut restored.state,
                        &checked,
                        &plan,
                        &mut Output(&mut restored.records),
                    ) {
                        merge::discard_cut(&mut restored.state, &checked, &mut |id, attempts| {
                            restored.dispatch.requeue(id, attempts)
                        })?;
                        Some(reason)
                    } else {
                        merge::p3_apply(
                            &mut restored.state,
                            checked,
                            plan,
                            config,
                            &records::Builder,
                            &mut Output(&mut restored.records),
                            &mut |id, attempts| restored.dispatch.requeue(id, attempts),
                        )?
                        .stop
                    }
                };
                if let Some(reason) = reason {
                    let receipt = save(restored, identity, b, Some(reason), None)?;
                    on_saved(&receipt, &[]);
                    return Ok(Outcome::Stopped(reason));
                }
            }
        }));
        let result = match step {
            Ok(result) => result,
            Err(_) => Err(Failure::Engine("panic in private epoch controller".into())),
        };
        if let Err(Failure::Engine(reason)) = &result {
            let _ = pool.cancel();
            if let Err(error) = restored.publisher.poison(restored.state.k, reason) {
                return Err(Failure::Save(io::Error::other(format!(
                    "{reason}; durable poison failed: {error}"
                ))));
            }
        }
        result
    });
    match outcome {
        Ok(Ok(outcome)) => Ok(outcome),
        Ok(Err(Failure::Save(error))) => Err(error),
        Ok(Err(Failure::Engine(error))) => Err(io::Error::other(error)),
        Err(error) => Err(pool_error(restored, error)),
    }
}

/// Real native wrapper, still private. Never infer worker capability merely
/// from an environment variable or a caller-thread LibraryUnlock token.
pub(super) fn run_native<const N: usize>(
    restored: &mut Restored<N>,
    identity: &Identity<'_>,
    b: usize,
    context: &Context<'_, N>,
    on_saved: impl FnMut(&publication::Receipt, &[Status]),
) -> io::Result<Outcome> {
    if context.request.workers < 2 {
        return Err(invalid(
            "responsive epoch checkpointing cannot move unlicensed inline W1 CAS",
        ));
    }
    let authorize = || {
        if !symbolica::license::LicenseManager::is_licensed() {
            return Err("responsive epoch worker requires actual Symbolica authorization".into());
        }
        Ok(())
    };
    let inspect = |bytes: &[u8], stop: &AtomicBool| {
        inspect_job(
            &Context {
                reducer: context.reducer,
                request: context.request,
                overlap: context.overlap,
                cancellation: stop,
            },
            bytes,
        )
    };
    use crate::application::routed_campaign::walking::worker_budget::{self, WorkerBudget};
    worker_budget::validate(
        context.request.workers,
        context.request.inspection_workers,
        context.request.max_containment_checks,
    )
    .map_err(invalid)?;
    let budget = WorkerBudget::for_request(context.request);
    // Reserved helper capacity is not silently converted into extra inspectors.
    run_authorized(
        restored,
        identity,
        b,
        budget.inspection + budget.coordinator,
        MergeConfig {
            frontier_stop: context.request.frontier_policy
                == crate::OwnerDomainWalkFrontierPolicy::Stop,
            lockstep: true,
            g2: false,
        },
        &authorize,
        &inspect,
        || {
            stop::requested(
                context.cancellation,
                context.request.epoch_stop_file.as_deref(),
            )
        },
        on_saved,
    )
}
