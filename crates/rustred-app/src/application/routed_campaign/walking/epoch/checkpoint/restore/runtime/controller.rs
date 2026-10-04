//! Durable public Epoch controller with lockstep and bounded rolling modes.
//! Owns all merge mutation and saves inside the scoped executor before join.
use super::super::super::{
    MergeBoundary, invalid,
    metadata::{Admission, Identity, Inputs},
    publication, stop,
};
use super::Restored;
use crate::application::routed_campaign::walking::epoch::record_store::Sidecar;
use crate::application::routed_campaign::walking::epoch::{
    dispatch::{Dispatch, Refill},
    inspector::{
        Activity, Context, Poll, RunError, Status, SubmitError, Work, inspect_job,
        inspect_job_profiled, inspect_job_with_snapshot, profile,
    },
    merge::{self, Fatal, MergeConfig, RecordOut, StopReason},
    records,
    snapshot::{Publication, Snapshot},
    state::EpochState,
};
use std::io;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::AtomicBool;
use std::time::Duration;

mod execution;
mod periodic;
mod rolling;

enum Refresh<const N: usize> {
    Ready(Option<Snapshot<N>>),
    Stopped(stop::Stop),
    RamGuard,
}

enum PreparationResult<const N: usize> {
    Ready(merge::preparation::Prepared<N>),
    Stopped(StopReason, Option<stop::Stop>),
}

fn prepare_cut<const N: usize>(
    engine: &merge::preparation::Engine,
    state: &EpochState<N>,
    checked: &merge::Checked<N>,
    mut tick: impl FnMut() -> Option<stop::Stop>,
) -> Result<PreparationResult<N>, Failure> {
    let mut context = None;
    let result = engine.prepare_observed(state, checked, || {
        let next = tick();
        if context.is_none() {
            context = next;
        }
        context.is_some()
    });
    if let Err(merge::preparation::Error::Fatal(error)) = result {
        return Err(error.into());
    }
    // Even if the result races cancellation, no cut is published after the
    // coordinator has accepted an operational stop.
    if let Some(context) = context {
        return Ok(PreparationResult::Stopped(context.kind(), Some(context)));
    }
    match result {
        Ok(prepared) => Ok(PreparationResult::Ready(prepared)),
        Err(merge::preparation::Error::Stopped) => {
            Ok(PreparationResult::Stopped(StopReason::Paused, None))
        }
        Err(merge::preparation::Error::RamGuard(_)) => {
            Ok(PreparationResult::Stopped(StopReason::RamGuard, None))
        }
        Err(merge::preparation::Error::Fatal(error)) => Err(error.into()),
    }
}

fn preparation_engine(identity: &Identity<'_>) -> io::Result<merge::preparation::Engine> {
    identity
        .preparation()
        .engine()
        .map_err(|error| io::Error::other(format!("epoch preparation startup: {error:?}")))
}

fn refresh_snapshot<const N: usize>(
    restored: &Restored<N>,
    stop_requested: &mut impl FnMut() -> Option<stop::Stop>,
    progress: &mut impl FnMut(&EpochState<N>, &Dispatch, &'static str),
) -> Result<Refresh<N>, Failure> {
    let mut stopped = None;
    let result = restored
        .state
        .store
        .try_snapshot_with(restored.state.k, &mut || {
            progress(&restored.state, &restored.dispatch, "boundary");
            if let Some(context) = stop_requested() {
                stopped = Some(context);
                Err("lookup refresh interrupted")
            } else {
                Ok(())
            }
        });
    if let Some(context) = stopped {
        return Ok(Refresh::Stopped(context));
    }
    match result {
        Ok(snapshot) => Ok(Refresh::Ready(snapshot)),
        Err(error) if error.contains("allocation") => Ok(Refresh::RamGuard),
        Err(error) => Err(Failure::Engine(error.into())),
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Outcome {
    Drained,
    Stopped(StopReason),
}

pub(super) enum Failure {
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
            restored.state.poisoned = true;
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
    fn push(&mut self, record: records::typed::Record) -> Result<(), String> {
        self.0.push(&record)
    }
}

/// P1 stores parent order for deterministic merge mechanics. Adaptive cost
/// attribution instead uses logical issuance order, independent of arrival and
/// of the scheduler's parent choices. Only committed merging results count.
fn observations<const N: usize>(
    identity: &Identity<'_>,
    checked: &merge::Checked<N>,
) -> Vec<(u32, f64, u64)> {
    if !identity.adaptive_dispatch() {
        return Vec::new();
    }
    let mut entries: Vec<_> = checked
        .entries
        .iter()
        .filter(|entry| entry.class.merges() && !entry.recurring_panic)
        .map(|entry| &entry.result)
        .collect();
    entries.sort_unstable_by_key(|result| result.seq);
    entries
        .into_iter()
        .map(|result| (result.parent, result.seconds, result.known_reuse))
        .collect()
}

pub(super) fn save<const N: usize>(
    restored: &mut Restored<N>,
    identity: &Identity<'_>,
    b: usize,
    reason: Option<StopReason>,
    context: Option<stop::Stop>,
) -> Result<publication::Receipt, Failure> {
    save_observed(restored, identity, b, reason, context, &mut |_, _, _| {})
}

pub(super) fn save_observed<const N: usize>(
    restored: &mut Restored<N>,
    identity: &Identity<'_>,
    b: usize,
    reason: Option<StopReason>,
    context: Option<stop::Stop>,
    progress: &mut impl FnMut(&EpochState<N>, &Dispatch, &'static str),
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
        admission_failure: restored.admission_failure.as_ref(),
    };
    let state = &restored.state;
    let dispatch = &restored.dispatch;
    let receipt = restored
        .publisher
        .save_observed(&boundary, &inputs, &mut restored.records, &mut || {
            progress(state, dispatch, "checkpoint")
        })
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
    stop_requested: impl FnMut() -> Option<stop::Stop>,
    on_saved: impl FnMut(&publication::Receipt, &[Status]),
) -> io::Result<Outcome> {
    run_authorized_periodic(
        restored,
        identity,
        b,
        budget,
        config,
        authorize,
        inspect,
        stop_requested,
        |_| false,
        on_saved,
    )
}

/// A due callback observes only a successful, nonfinal committed merge. It
/// cannot change the cut, session or dispatch order; save remains synchronous.
#[allow(clippy::too_many_arguments)]
pub(super) fn run_authorized_periodic<const N: usize>(
    restored: &mut Restored<N>,
    identity: &Identity<'_>,
    b: usize,
    budget: usize,
    config: MergeConfig,
    authorize: &(dyn Fn() -> Result<(), String> + Sync),
    inspect: &(dyn Fn(&[u8], &AtomicBool) -> Vec<u8> + Sync),
    mut stop_requested: impl FnMut() -> Option<stop::Stop>,
    mut periodic_due: impl FnMut(u64) -> bool,
    mut on_saved: impl FnMut(&publication::Receipt, &[Status]),
) -> io::Result<Outcome> {
    run_authorized_lookup_periodic(
        restored,
        identity,
        b,
        budget,
        config,
        authorize,
        inspect,
        &mut stop_requested,
        &mut periodic_due,
        &mut on_saved,
        None,
    )
}

/// The private real-native path publishes one immutable lookup view per cut.
/// Synthetic/all-miss callers retain the previous wrapper above.
#[allow(clippy::too_many_arguments)]
pub(super) fn run_authorized_lookup_periodic<const N: usize>(
    restored: &mut Restored<N>,
    identity: &Identity<'_>,
    b: usize,
    budget: usize,
    config: MergeConfig,
    authorize: &(dyn Fn() -> Result<(), String> + Sync),
    inspect: &(dyn Fn(&[u8], &AtomicBool) -> Vec<u8> + Sync),
    mut stop_requested: impl FnMut() -> Option<stop::Stop>,
    mut periodic_due: impl FnMut(u64) -> bool,
    mut on_saved: impl FnMut(&publication::Receipt, &[Status]),
    snapshots: Option<&Publication<N>>,
) -> io::Result<Outcome> {
    if budget < 2 {
        return Err(invalid(
            "private responsive controller requires budget >= 2",
        ));
    }
    run_observed(
        restored,
        identity,
        b,
        budget,
        config,
        authorize,
        inspect,
        &mut stop_requested,
        &mut periodic_due,
        |_, receipt, status| on_saved(receipt, status),
        snapshots,
        |_, _, _, _| {},
        |_| {},
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run_observed<const N: usize>(
    restored: &mut Restored<N>,
    identity: &Identity<'_>,
    b: usize,
    budget: usize,
    config: MergeConfig,
    authorize: &(dyn Fn() -> Result<(), String> + Sync),
    inspect: &(dyn Fn(&[u8], &AtomicBool) -> Vec<u8> + Sync),
    mut stop_requested: impl FnMut() -> Option<stop::Stop>,
    mut periodic_due: impl FnMut(u64) -> bool,
    mut on_saved: impl FnMut(&Restored<N>, &publication::Receipt, &[Status]),
    snapshots: Option<&Publication<N>>,
    mut progress: impl FnMut(&EpochState<N>, &Dispatch, &'static str, &dyn Fn() -> Option<Activity>),
    mut maintenance: impl FnMut(&mut EpochState<N>),
) -> io::Result<Outcome> {
    if !config.lockstep {
        return rolling::run(
            restored,
            identity,
            b,
            budget,
            config,
            authorize,
            inspect,
            stop_requested,
            periodic_due,
            on_saved,
            snapshots,
            progress,
            maintenance,
        );
    }
    if budget == 0
        || !(1..=4096).contains(&b)
        || !config.lockstep
        || !matches!(restored.admission, Admission::Complete)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "epoch controller needs complete admission, lockstep and budget >= 1",
        ));
    }
    // Shape validation occurs before workers exist or any replay payload moves.
    MergeBoundary::borrow(&restored.state, &restored.dispatch, b)?;
    let preparation = preparation_engine(identity)?;
    let outcome = execution::with(budget, authorize, inspect, |pool| {
        let step = catch_unwind(AssertUnwindSafe(|| -> Result<Outcome, Failure> {
            if let Some(reason) = initial_stop(config, restored.roots.frontiers.len()) {
                progress(&restored.state, &restored.dispatch, "checkpoint", &|| {
                    pool.activity()
                });
                let receipt = save_observed(
                    restored,
                    identity,
                    b,
                    Some(reason),
                    None,
                    &mut |state, dispatch, phase| {
                        progress(state, dispatch, phase, &|| pool.activity())
                    },
                )?;
                on_saved(restored, &receipt, &[]);
                return Ok(Outcome::Stopped(reason));
            }
            let mut committed_boundary = false;
            loop {
                progress(&restored.state, &restored.dispatch, "boundary", &|| {
                    pool.activity()
                });
                if let Some(context) = stop_requested() {
                    pool.cancel().map_err(Failure::Engine)?;
                    let reason = context.kind();
                    progress(&restored.state, &restored.dispatch, "checkpoint", &|| {
                        pool.activity()
                    });
                    let receipt = save_observed(
                        restored,
                        identity,
                        b,
                        Some(reason),
                        Some(context),
                        &mut |state, dispatch, phase| {
                            progress(state, dispatch, phase, &|| pool.activity())
                        },
                    )?;
                    // Any prior batch has already merged; its old worker
                    // observations must not masquerade as current in-flight.
                    on_saved(restored, &receipt, &[]);
                    return Ok(Outcome::Stopped(reason));
                }
                if committed_boundary {
                    // Only complete, non-stopped merge boundaries may run
                    // optional monitoring; a checkpoint itself never asks.
                    maintenance(&mut restored.state);
                }
                if std::mem::take(&mut committed_boundary)
                    && restored.state.pending_or_reserved() != 0
                    && periodic_due(restored.state.k)
                {
                    if !restored.state.in_flight.is_empty() {
                        return Err(Failure::Engine("periodic save has an active cut".into()));
                    }
                    // A newly requested stop wins even if it arrived while
                    // checking the timer. There is no inspection to cancel.
                    if let Some(context) = stop_requested() {
                        pool.cancel().map_err(Failure::Engine)?;
                        let reason = context.kind();
                        progress(&restored.state, &restored.dispatch, "checkpoint", &|| {
                            pool.activity()
                        });
                        let receipt = save_observed(
                            restored,
                            identity,
                            b,
                            Some(reason),
                            Some(context),
                            &mut |state, dispatch, phase| {
                                progress(state, dispatch, phase, &|| pool.activity())
                            },
                        )?;
                        on_saved(restored, &receipt, &[]);
                        return Ok(Outcome::Stopped(reason));
                    }
                    progress(&restored.state, &restored.dispatch, "checkpoint", &|| {
                        pool.activity()
                    });
                    let receipt = save_observed(
                        restored,
                        identity,
                        b,
                        None,
                        None,
                        &mut |state, dispatch, phase| {
                            progress(state, dispatch, phase, &|| pool.activity())
                        },
                    )?;
                    on_saved(restored, &receipt, &[]);
                    // Stop can arrive during the synchronous write/callback.
                    // Recheck before refill without repeating the periodic save.
                    continue;
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
                            progress(&restored.state, &restored.dispatch, "checkpoint", &|| {
                                pool.activity()
                            });
                            let receipt = save_observed(
                                restored,
                                identity,
                                b,
                                (!certified).then_some(StopReason::DrainedUncertified),
                                None,
                                &mut |state, dispatch, phase| {
                                    progress(state, dispatch, phase, &|| pool.activity())
                                },
                            )?;
                            on_saved(restored, &receipt, &[]);
                            return Ok(Outcome::Drained);
                        }
                        Refill::SequenceExhausted => {
                            progress(&restored.state, &restored.dispatch, "checkpoint", &|| {
                                pool.activity()
                            });
                            let receipt = save_observed(
                                restored,
                                identity,
                                b,
                                Some(StopReason::Capacity),
                                None,
                                &mut |state, dispatch, phase| {
                                    progress(state, dispatch, phase, &|| pool.activity())
                                },
                            )?;
                            on_saved(restored, &receipt, &[]);
                            return Ok(Outcome::Stopped(StopReason::Capacity));
                        }
                        Refill::Stalled => {
                            return Err(Failure::Engine("restored epoch dispatch stalled".into()));
                        }
                    }
                };
                let mut work = Vec::new();
                let mut results = Vec::new();
                if let Some(snapshots) = snapshots {
                    restored
                        .state
                        .store
                        .ensure_unique()
                        .map_err(|e| Failure::Engine(e.into()))?;
                    let snapshot = match refresh_snapshot(
                        restored,
                        &mut stop_requested,
                        &mut |state, dispatch, phase| {
                            progress(state, dispatch, phase, &|| pool.activity())
                        },
                    )? {
                        Refresh::Ready(Some(snapshot)) => snapshot,
                        Refresh::Ready(None) => {
                            return Err(Failure::Engine(
                                "drained lockstep lookup buffers remain leased".into(),
                            ));
                        }
                        Refresh::Stopped(context) => {
                            pool.cancel().map_err(Failure::Engine)?;
                            let reason = context.kind();
                            let receipt = save_observed(
                                restored,
                                identity,
                                b,
                                Some(reason),
                                Some(context),
                                &mut |state, dispatch, phase| {
                                    progress(state, dispatch, phase, &|| pool.activity())
                                },
                            )?;
                            on_saved(restored, &receipt, &[]);
                            return Ok(Outcome::Stopped(reason));
                        }
                        Refresh::RamGuard => {
                            pool.cancel().map_err(Failure::Engine)?;
                            let receipt = save_observed(
                                restored,
                                identity,
                                b,
                                Some(StopReason::RamGuard),
                                None,
                                &mut |state, dispatch, phase| {
                                    progress(state, dispatch, phase, &|| pool.activity())
                                },
                            )?;
                            on_saved(restored, &receipt, &[]);
                            return Ok(Outcome::Stopped(StopReason::RamGuard));
                        }
                    };
                    snapshots
                        .publish(snapshot)
                        .map_err(|e| Failure::Engine(e.into()))?;
                }
                // Both lists are B-bounded, never an arena/snapshot clone.
                if work.try_reserve_exact(jobs.len()).is_err()
                    || results.try_reserve_exact(jobs.len()).is_err()
                {
                    drop(jobs);
                    pool.cancel().map_err(Failure::Engine)?;
                    progress(&restored.state, &restored.dispatch, "checkpoint", &|| {
                        pool.activity()
                    });
                    let receipt = save_observed(
                        restored,
                        identity,
                        b,
                        Some(StopReason::RamGuard),
                        None,
                        &mut |state, dispatch, phase| {
                            progress(state, dispatch, phase, &|| pool.activity())
                        },
                    )?;
                    on_saved(restored, &receipt, &[]);
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
                        progress(&restored.state, &restored.dispatch, "checkpoint", &|| {
                            pool.activity()
                        });
                        let receipt = save_observed(
                            restored,
                            identity,
                            b,
                            Some(StopReason::RamGuard),
                            None,
                            &mut |state, dispatch, phase| {
                                progress(state, dispatch, phase, &|| pool.activity())
                            },
                        )?;
                        on_saved(restored, &receipt, &[]);
                        return Ok(Outcome::Stopped(StopReason::RamGuard));
                    }
                }
                loop {
                    progress(&restored.state, &restored.dispatch, "inspect", &|| {
                        pool.activity()
                    });
                    if let Some(context) = stop_requested() {
                        // No partial cut reaches P1. Release both already-polled
                        // bytes and queued/late channel results BEFORE streaming save.
                        pool.cancel().map_err(Failure::Engine)?;
                        drop(results);
                        let status = pool.take_cancelled_status().map_err(Failure::Engine)?;
                        let reason = context.kind();
                        progress(&restored.state, &restored.dispatch, "checkpoint", &|| {
                            pool.activity()
                        });
                        let receipt = save_observed(
                            restored,
                            identity,
                            b,
                            Some(reason),
                            Some(context),
                            &mut |state, dispatch, phase| {
                                progress(state, dispatch, phase, &|| pool.activity())
                            },
                        )?;
                        on_saved(restored, &receipt, &status);
                        return Ok(Outcome::Stopped(reason));
                    }
                    match pool
                        .poll(Duration::from_millis(50))
                        .map_err(Failure::Engine)?
                    {
                        Poll::Started(_) | Poll::Waiting => {}
                        Poll::Result { bytes, .. } => results.push(bytes.into_vec()),
                        Poll::Drained => break,
                    }
                }
                if let Some(snapshots) = snapshots {
                    snapshots.clear().map_err(|e| Failure::Engine(e.into()))?;
                    restored
                        .state
                        .store
                        .ensure_unique()
                        .map_err(|e| Failure::Engine(e.into()))?;
                }
                // P1 sorts the complete cut; first-error handling is independent
                // of worker completion order. Existing P1-P3 semantics unchanged.
                progress(&restored.state, &restored.dispatch, "p1", &|| {
                    pool.activity()
                });
                let checked = merge::p1_check_bound(
                    &mut restored.state,
                    results,
                    config,
                    snapshots.and_then(Publication::native_session),
                )?;
                let reason = if let Some(reason) = checked.stop {
                    merge::discard_cut(&mut restored.state, &checked, &mut |id, attempts| {
                        restored.dispatch.requeue(id, attempts)
                    })?;
                    Some(reason)
                } else {
                    let observations = observations(identity, &checked);
                    progress(&restored.state, &restored.dispatch, "p2", &|| {
                        pool.activity()
                    });
                    let prepared =
                        match prepare_cut(&preparation, &restored.state, &checked, || {
                            progress(&restored.state, &restored.dispatch, "p2", &|| {
                                pool.activity()
                            });
                            stop_requested()
                        })? {
                            PreparationResult::Ready(prepared) => prepared,
                            PreparationResult::Stopped(reason, context) => {
                                merge::discard_cut(
                                    &mut restored.state,
                                    &checked,
                                    &mut |id, attempts| restored.dispatch.requeue(id, attempts),
                                )?;
                                pool.retire_all_returned().map_err(Failure::Engine)?;
                                let receipt = save_observed(
                                    restored,
                                    identity,
                                    b,
                                    Some(reason),
                                    context,
                                    &mut |state, dispatch, phase| {
                                        progress(state, dispatch, phase, &|| pool.activity())
                                    },
                                )?;
                                on_saved(restored, &receipt, &[]);
                                return Ok(Outcome::Stopped(reason));
                            }
                        };
                    restored.state.preparation.add(&prepared.metrics);
                    let plan = prepared.plan;
                    plan.counters.apply_to(&mut restored.state);
                    progress(&restored.state, &restored.dispatch, "p3", &|| {
                        pool.activity()
                    });
                    if let Err(reason) = merge::p3_preflight(
                        &mut restored.state,
                        &checked,
                        &plan,
                        &mut Output(&mut restored.records),
                    ) {
                        let reason = match reason {
                            merge::PreflightError::Stop(reason) => reason,
                            merge::PreflightError::Engine(error) => {
                                return Err(Failure::Engine(error.into()));
                            }
                        };
                        merge::discard_cut(&mut restored.state, &checked, &mut |id, attempts| {
                            restored.dispatch.requeue(id, attempts)
                        })?;
                        Some(reason)
                    } else {
                        let applied = merge::p3_apply(
                            &mut restored.state,
                            checked,
                            plan,
                            config,
                            &records::Builder,
                            &mut Output(&mut restored.records),
                            &mut |id, attempts| restored.dispatch.requeue(id, attempts),
                        )?;
                        restored.dispatch.observe_completed(
                            &restored.state,
                            &observations,
                            applied.new_ids,
                        );
                        applied.stop
                    }
                };
                // The complete cut was committed or explicitly discarded.
                // Returned callbacks are no longer awaiting publication.
                pool.retire_all_returned().map_err(Failure::Engine)?;
                if let Some(reason) = reason {
                    progress(&restored.state, &restored.dispatch, "checkpoint", &|| {
                        pool.activity()
                    });
                    let receipt = save_observed(
                        restored,
                        identity,
                        b,
                        Some(reason),
                        None,
                        &mut |state, dispatch, phase| {
                            progress(state, dispatch, phase, &|| pool.activity())
                        },
                    )?;
                    on_saved(restored, &receipt, &[]);
                    return Ok(Outcome::Stopped(reason));
                }
                committed_boundary = true;
            }
        }));
        let result = match step {
            Ok(result) => result,
            Err(_) => Err(Failure::Engine("panic in private epoch controller".into())),
        };
        if let Err(Failure::Engine(reason)) = &result {
            restored.state.poisoned = true;
            let _ = pool.cancel();
            if let Err(error) = restored.publisher.poison(restored.state.k, reason) {
                return Err(Failure::Save(io::Error::other(format!(
                    "{reason}; durable poison failed: {error}"
                ))));
            }
        }
        progress(&restored.state, &restored.dispatch, "drain_wait", &|| {
            pool.activity()
        });
        result
    });
    progress(&restored.state, &restored.dispatch, "joined", &|| {
        Some(Activity::default())
    });
    // with_authorized_pool has joined every worker on all paths. Cancellation
    // therefore may release publication here, but never before its durable save.
    if let Some(snapshots) = snapshots {
        snapshots
            .clear()
            .map_err(|e| pool_error(restored, RunError::Engine(e.into())))?;
        restored
            .state
            .store
            .ensure_unique()
            .map_err(|e| pool_error(restored, RunError::Engine(e.into())))?;
    }
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
    run_native_lookup(
        restored,
        identity,
        b,
        context,
        LookupMode::AllMiss,
        on_saved,
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum LookupMode {
    AllMiss,
    Snapshot,
}

/// Explicit private comparison switch. No environment/public request default
/// changes, and the old entry point remains all-miss.
pub(super) fn run_native_lookup<const N: usize>(
    restored: &mut Restored<N>,
    identity: &Identity<'_>,
    b: usize,
    context: &Context<'_, N>,
    mode: LookupMode,
    mut on_saved: impl FnMut(&publication::Receipt, &[Status]),
) -> io::Result<Outcome> {
    run_native_observed(
        restored,
        identity,
        b,
        context,
        mode,
        |_, receipt, status| on_saved(receipt, status),
        |_, _, _, _| {},
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run_native_observed<const N: usize>(
    restored: &mut Restored<N>,
    identity: &Identity<'_>,
    b: usize,
    context: &Context<'_, N>,
    mode: LookupMode,
    mut on_saved: impl FnMut(&Restored<N>, &publication::Receipt, &[Status]),
    progress: impl FnMut(&EpochState<N>, &Dispatch, &'static str, &dyn Fn() -> Option<Activity>),
) -> io::Result<Outcome> {
    let schedule = periodic::Schedule::new(
        context
            .request
            .checkpoint
            .as_ref()
            .map(|options| Duration::from_secs(options.interval_seconds)),
    )?;
    let authorize = || {
        if !symbolica::license::LicenseManager::is_licensed() {
            return Err("responsive epoch worker requires actual Symbolica authorization".into());
        }
        Ok(())
    };
    let snapshots = if mode == LookupMode::Snapshot {
        Publication::native(restored.dispatch.checkpoint_snapshot().session)
    } else {
        Publication::new()
    };
    let profiling = profile::Collector::from_environment();
    restored.rolling_diagnostics.wait_profile = profiling
        .as_ref()
        .map(|profile| profile::Waits::new(profile.origin()));
    let inspect = |bytes: &[u8], stop: &AtomicBool| {
        let stop = if context.request.workers == 1 {
            context.cancellation
        } else {
            stop
        };
        let context = Context {
            reducer: context.reducer,
            request: context.request,
            overlap: context.overlap,
            cancellation: stop,
            g2: context.g2,
            finite_account: context.finite_account,
        };
        if mode == LookupMode::AllMiss {
            return match &profiling {
                Some(profile) => inspect_job_profiled(&context, bytes, None, profile),
                None => inspect_job(&context, bytes),
            };
        }
        let snapshot = match if context.request.epoch_rolling {
            crate::application::routed_campaign::walking::epoch::job::Job::<N>::decode(bytes)
                .map_err(|_| "epoch rolling job decode")
                .and_then(|job| snapshots.acquire_job(job.seq))
        } else {
            snapshots.acquire()
        } {
            Ok(snapshot) => snapshot,
            Err(_) => return Vec::new(),
        };
        match &profiling {
            Some(profile) => inspect_job_profiled(&context, bytes, Some(&snapshot), profile),
            None => inspect_job_with_snapshot(&context, bytes, snapshot),
        }
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
    let outcome = run_observed(
        restored,
        identity,
        b,
        budget.inspection + budget.coordinator,
        MergeConfig {
            frontier_stop: context.request.frontier_policy
                == crate::OwnerDomainWalkFrontierPolicy::Stop,
            lockstep: !context.request.epoch_rolling,
            g2: context.g2.is_some(),
            finite_replay: context.request.finite_replay,
        },
        &authorize,
        &inspect,
        || {
            stop::requested(
                context.cancellation,
                context.request.epoch_stop_file.as_deref(),
            )
        },
        |_| schedule.due(),
        |restored, receipt, status| {
            schedule.saved();
            on_saved(restored, receipt, status);
        },
        (mode == LookupMode::Snapshot).then_some(&snapshots),
        progress,
        |state| state.tracker.refresh_periodic_monitor(context.cancellation),
    );
    // All scoped inspectors have joined, including error/cancel returns.
    // Observations never enter JobResult, merge decisions or checkpoint bytes.
    if let Some(waits) = &mut restored.rolling_diagnostics.wait_profile {
        waits.finish();
    }
    restored.rolling_diagnostics.inspection_profile = profiling.map(|profile| profile.snapshot());
    outcome
}
