//! Bounded rolling publication: the oldest sequence prefix is the next cut,
//! while unrelated inspections keep running. Result arrival order has no
//! authority over the order inside a cut. CP6 retains the exact issued
//! seq/v0/parent inventory and accepted records retain their publication epoch.
use super::*;
use crate::application::routed_campaign::walking::epoch::ledger6::Tag;
use std::collections::{BTreeMap, VecDeque};

#[allow(clippy::too_many_arguments)]
fn stopped<const N: usize>(
    restored: &mut Restored<N>,
    identity: &Identity<'_>,
    b: usize,
    pool: &mut dyn execution::Execution,
    snapshots: Option<&Publication<N>>,
    results: &mut BTreeMap<u64, Vec<u8>>,
    reason: StopReason,
    context: Option<stop::Stop>,
    progress: &mut impl FnMut(&EpochState<N>, &Dispatch, &'static str),
    on_saved: &mut impl FnMut(&Restored<N>, &publication::Receipt, &[Status]),
) -> Result<Outcome, Failure> {
    // Queued and returned-but-unmerged bytes are released before streaming the
    // canonical checkpoint. Running readers own independent immutable leases.
    pool.cancel().map_err(Failure::Engine)?;
    results.clear();
    if let Some(snapshots) = snapshots {
        snapshots.clear().map_err(|e| Failure::Engine(e.into()))?;
    }
    let status = pool.take_cancelled_status().map_err(Failure::Engine)?;
    let receipt = save_observed(restored, identity, b, Some(reason), context, progress)?;
    on_saved(restored, &receipt, &status);
    Ok(Outcome::Stopped(reason))
}

fn collect(
    message: Poll,
    order: &VecDeque<u64>,
    results: &mut BTreeMap<u64, Vec<u8>>,
) -> Result<bool, Failure> {
    match message {
        Poll::Result { key, bytes } => {
            if !order.contains(&key) || results.insert(key, bytes).is_some() {
                return Err(Failure::Engine(
                    "rolling result absent/duplicate sequence".into(),
                ));
            }
            Ok(false)
        }
        Poll::Started(_) | Poll::Waiting => Ok(false),
        Poll::Drained => Ok(true),
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run<const N: usize>(
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
    mut progress: impl FnMut(&EpochState<N>, &Dispatch, &'static str),
) -> io::Result<Outcome> {
    if budget == 0
        || !(1..=4096).contains(&b)
        || config.lockstep
        || !matches!(restored.admission, Admission::Complete)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "rolling epoch requires complete admission and a bounded positive window",
        ));
    }
    MergeBoundary::borrow(&restored.state, &restored.dispatch, b)?;
    let cut_size = identity.epoch_cut_size().min(b);
    let outcome = execution::with(budget, authorize, inspect, |pool| {
        let step = catch_unwind(AssertUnwindSafe(|| -> Result<Outcome, Failure> {
            let mut order = VecDeque::new();
            order
                .try_reserve(b)
                .map_err(|_| Failure::Engine("rolling sequence allocation".into()))?;
            let mut results = BTreeMap::new();
            if let Some(reason) = initial_stop(config, restored.roots.frontiers.len()) {
                return stopped(
                    restored,
                    identity,
                    b,
                    pool,
                    snapshots,
                    &mut results,
                    reason,
                    None,
                    &mut progress,
                    &mut on_saved,
                );
            }
            let mut committed = false;
            loop {
                progress(&restored.state, &restored.dispatch, "boundary");
                if let Some(context) = stop_requested() {
                    return stopped(
                        restored,
                        identity,
                        b,
                        pool,
                        snapshots,
                        &mut results,
                        context.kind(),
                        Some(context),
                        &mut progress,
                        &mut on_saved,
                    );
                }
                if std::mem::take(&mut committed)
                    && restored.state.pending_or_reserved() != 0
                    && periodic_due(restored.state.k)
                {
                    // Workers may inspect during a synchronous save: only the
                    // canonical state is borrowed, and its merge is complete.
                    let receipt = save_observed(restored, identity, b, None, None, &mut progress)?;
                    on_saved(restored, &receipt, &[]);
                    continue;
                }

                let room = b
                    .checked_sub(restored.state.in_flight.len())
                    .ok_or_else(|| Failure::Engine("rolling flight bound exceeded".into()))?;
                if room != 0 || !restored.replay.is_empty() {
                    let snapshot = if snapshots.is_some() {
                        match refresh_snapshot(restored, &mut stop_requested, &mut progress)? {
                            Refresh::Ready(snapshot) => snapshot,
                            Refresh::Stopped(context) => {
                                return stopped(
                                    restored,
                                    identity,
                                    b,
                                    pool,
                                    snapshots,
                                    &mut results,
                                    context.kind(),
                                    Some(context),
                                    &mut progress,
                                    &mut on_saved,
                                );
                            }
                            Refresh::RamGuard => {
                                return stopped(
                                    restored,
                                    identity,
                                    b,
                                    pool,
                                    snapshots,
                                    &mut results,
                                    StopReason::RamGuard,
                                    None,
                                    &mut progress,
                                    &mut on_saved,
                                );
                            }
                        }
                    } else {
                        None
                    };
                    // Both buffers can be leased by queued/running jobs. No
                    // third replica and no unbounded historical view is made.
                    if snapshots.is_none() || snapshot.is_some() {
                        let jobs = if !restored.replay.is_empty() {
                            std::mem::take(&mut restored.replay)
                        } else {
                            match restored.dispatch.refill(&mut restored.state, room) {
                                Refill::Jobs(jobs) => jobs,
                                Refill::Drained => Vec::new(),
                                Refill::Stalled if !restored.state.in_flight.is_empty() => {
                                    Vec::new()
                                }
                                Refill::Stalled => {
                                    return Err(Failure::Engine(
                                        "rolling dispatch stalled without live work".into(),
                                    ));
                                }
                                Refill::SequenceExhausted => {
                                    return stopped(
                                        restored,
                                        identity,
                                        b,
                                        pool,
                                        snapshots,
                                        &mut results,
                                        StopReason::Capacity,
                                        None,
                                        &mut progress,
                                        &mut on_saved,
                                    );
                                }
                            }
                        };
                        if !jobs.is_empty() {
                            let mut work = Vec::new();
                            let mut keys = Vec::new();
                            if work.try_reserve_exact(jobs.len()).is_err()
                                || keys.try_reserve_exact(jobs.len()).is_err()
                            {
                                return stopped(
                                    restored,
                                    identity,
                                    b,
                                    pool,
                                    snapshots,
                                    &mut results,
                                    StopReason::RamGuard,
                                    None,
                                    &mut progress,
                                    &mut on_saved,
                                );
                            }
                            for job in jobs {
                                keys.push(job.seq);
                                order.push_back(job.seq);
                                work.push(Work {
                                    key: job.seq,
                                    bytes: job.encode(),
                                });
                            }
                            if let (Some(publication), Some(snapshot)) =
                                (snapshots, snapshot.as_ref())
                            {
                                publication
                                    .bind(&keys, snapshot)
                                    .map_err(|e| Failure::Engine(e.into()))?;
                            }
                            match pool.submit_rolling(work) {
                                Ok(()) => {}
                                Err(SubmitError::Protocol(error)) => {
                                    return Err(Failure::Engine(error.into()));
                                }
                                Err(SubmitError::Allocation(_)) => {
                                    return stopped(
                                        restored,
                                        identity,
                                        b,
                                        pool,
                                        snapshots,
                                        &mut results,
                                        StopReason::RamGuard,
                                        None,
                                        &mut progress,
                                        &mut on_saved,
                                    );
                                }
                            }
                        }
                    }
                    // Drop the coordinator lease before a publication/refresh.
                    drop(snapshot);
                }

                if order.is_empty()
                    && restored.state.in_flight.is_empty()
                    && restored.state.pending_or_reserved() == 0
                {
                    let (certified, _) =
                        crate::application::routed_campaign::walking::epoch::certification(
                            &restored.state,
                            true,
                            false,
                            restored.roots.frontiers.len(),
                        );
                    let receipt = save_observed(
                        restored,
                        identity,
                        b,
                        (!certified).then_some(StopReason::DrainedUncertified),
                        None,
                        &mut progress,
                    )?;
                    on_saved(restored, &receipt, &[]);
                    return Ok(Outcome::Drained);
                }
                let count = cut_size.min(order.len());
                let no_unissued = restored.state.ledger.counts().get(Tag::Pending) == 0
                    && restored.dispatch.queued() == (0, 0);
                // A restored partial cohort must retire before Dispatch opens
                // fresh pending work. Waiting to fill this cut would deadlock
                // when the saved unfinished inventory is smaller than B.
                let replay_barrier = !restored.dispatch.admission_ready();
                let ready = count != 0
                    && (count == cut_size || no_unissued || replay_barrier)
                    && order
                        .iter()
                        .take(count)
                        .all(|key| results.contains_key(key));
                if !ready {
                    progress(&restored.state, &restored.dispatch, "inspect");
                    let drained = collect(
                        pool.poll(Duration::from_millis(50))
                            .map_err(Failure::Engine)?,
                        &order,
                        &mut results,
                    )?;
                    if drained && order.is_empty() {
                        return Err(Failure::Engine(
                            "rolling executor drained with undispatched obligations".into(),
                        ));
                    }
                    continue;
                }

                let keys: Vec<u64> = order.iter().take(count).copied().collect();
                let bytes = keys
                    .iter()
                    .map(|key| results.remove(key).expect("checked ready prefix"))
                    .collect();
                progress(&restored.state, &restored.dispatch, "p1");
                let checked = merge::p1_check(&mut restored.state, bytes, config)?;
                let reason = if let Some(reason) = checked.stop {
                    merge::discard_cut(&mut restored.state, &checked, &mut |id, attempts| {
                        restored.dispatch.requeue(id, attempts)
                    })?;
                    Some(reason)
                } else {
                    let observations = observations(identity, &checked);
                    progress(&restored.state, &restored.dispatch, "p2");
                    let plan = merge::p2(&mut restored.state, &checked)?;
                    if snapshots.is_some() {
                        let retirees = plan.survivors.iter().map(|s| s.retire.len()).sum();
                        loop {
                            // A completed callback has released its lease even
                            // when its bytes are waiting for their sequence.
                            match refresh_snapshot(restored, &mut stop_requested, &mut progress)? {
                                Refresh::Ready(view) => drop(view),
                                Refresh::Stopped(context) => {
                                    drop(plan);
                                    drop(checked);
                                    return stopped(
                                        restored,
                                        identity,
                                        b,
                                        pool,
                                        snapshots,
                                        &mut results,
                                        context.kind(),
                                        Some(context),
                                        &mut progress,
                                        &mut on_saved,
                                    );
                                }
                                Refresh::RamGuard => {
                                    drop(plan);
                                    drop(checked);
                                    return stopped(
                                        restored,
                                        identity,
                                        b,
                                        pool,
                                        snapshots,
                                        &mut results,
                                        StopReason::RamGuard,
                                        None,
                                        &mut progress,
                                        &mut on_saved,
                                    );
                                }
                            }
                            if restored.state.store.publication_room(
                                restored.state.k,
                                plan.survivors.len(),
                                retirees,
                            ) {
                                break;
                            }
                            if let Some(context) = stop_requested() {
                                // This prefix has not reached P3; retain all
                                // reservations and reissue them after restart.
                                drop(plan);
                                drop(checked);
                                return stopped(
                                    restored,
                                    identity,
                                    b,
                                    pool,
                                    snapshots,
                                    &mut results,
                                    context.kind(),
                                    Some(context),
                                    &mut progress,
                                    &mut on_saved,
                                );
                            }
                            progress(&restored.state, &restored.dispatch, "inspect");
                            // A drained pool releases the final readers; retry
                            // refresh before deciding whether publication fits.
                            collect(
                                pool.poll(Duration::from_millis(50))
                                    .map_err(Failure::Engine)?,
                                &order,
                                &mut results,
                            )?;
                        }
                    }
                    progress(&restored.state, &restored.dispatch, "p3");
                    match merge::p3_preflight(
                        &mut restored.state,
                        &checked,
                        &plan,
                        &mut Output(&mut restored.records),
                    ) {
                        Err(merge::PreflightError::Engine(error)) => {
                            return Err(Failure::Engine(error.into()));
                        }
                        Err(merge::PreflightError::Stop(reason)) => {
                            merge::discard_cut(
                                &mut restored.state,
                                &checked,
                                &mut |id, attempts| restored.dispatch.requeue(id, attempts),
                            )?;
                            Some(reason)
                        }
                        Ok(()) => {
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
                    }
                };
                pool.retire(&keys).map_err(Failure::Engine)?;
                for key in keys {
                    if order.pop_front() != Some(key) {
                        return Err(Failure::Engine("rolling publication prefix changed".into()));
                    }
                }
                if let Some(reason) = reason {
                    return stopped(
                        restored,
                        identity,
                        b,
                        pool,
                        snapshots,
                        &mut results,
                        reason,
                        None,
                        &mut progress,
                        &mut on_saved,
                    );
                }
                committed = true;
            }
        }));
        let result = match step {
            Ok(result) => result,
            Err(_) => Err(Failure::Engine("panic in rolling epoch controller".into())),
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
        progress(&restored.state, &restored.dispatch, "drain_wait");
        result
    });
    progress(&restored.state, &restored.dispatch, "joined");
    if let Some(snapshots) = snapshots {
        snapshots
            .clear()
            .map_err(|e| pool_error(restored, RunError::Engine(e.into())))?;
    }
    match outcome {
        Ok(Ok(outcome)) => Ok(outcome),
        Ok(Err(Failure::Save(error))) => Err(error),
        Ok(Err(Failure::Engine(error))) => Err(io::Error::other(error)),
        Err(error) => Err(pool_error(restored, error)),
    }
}
