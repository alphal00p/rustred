//! Bounded rolling publication: the default commits the oldest sequence prefix;
//! opt-in oldest-ready commits the lowest completed sequences instead. Arrival
//! may change the selected cut, never its exact P1/P2/P3 authority. CP6 retains the exact issued
//! seq/v0/parent inventory and accepted records retain their publication epoch.
use super::super::RollingDiagnostics;
use super::*;
use crate::OwnerDomainWalkEpochPublicationOrder;
use crate::application::routed_campaign::walking::epoch::inspector::{
    EscrowDiagnostics, ReturnedBytes,
};
use crate::application::routed_campaign::walking::epoch::ledger6::Tag;
use std::collections::{BTreeMap, VecDeque};
use std::time::Instant;

#[allow(clippy::too_many_arguments)]
fn stopped<const N: usize>(
    restored: &mut Restored<N>,
    identity: &Identity<'_>,
    b: usize,
    pool: &mut dyn execution::Execution,
    snapshots: Option<&Publication<N>>,
    results: &mut BTreeMap<u64, ReturnedBytes>,
    reason: StopReason,
    context: Option<stop::Stop>,
    progress: &mut impl FnMut(&EpochState<N>, &Dispatch, &'static str, &dyn Fn() -> Option<Activity>),
    on_saved: &mut impl FnMut(&Restored<N>, &publication::Receipt, &[Status]),
) -> Result<Outcome, Failure> {
    // Queued and returned-but-unmerged bytes are released before streaming the
    // canonical checkpoint. Running readers own independent immutable leases.
    pool.cancel().map_err(Failure::Engine)?;
    let status = pool.take_cancelled_status().map_err(Failure::Engine)?;
    results.clear();
    if let Some(snapshots) = snapshots {
        snapshots.clear().map_err(|e| Failure::Engine(e.into()))?;
    }
    let receipt = save_observed(
        restored,
        identity,
        b,
        Some(reason),
        context,
        &mut |state, dispatch, phase| progress(state, dispatch, phase, &|| None),
    )?;
    on_saved(restored, &receipt, &status);
    Ok(Outcome::Stopped(reason))
}

fn collect(
    message: Poll,
    order: &VecDeque<u64>,
    results: &mut BTreeMap<u64, ReturnedBytes>,
    diagnostics: &mut RollingDiagnostics,
    pool: &mut dyn execution::Execution,
    escrow: bool,
) -> Result<bool, Failure> {
    match message {
        Poll::Result { key, bytes } => {
            diagnostics.returned_messages += 1;
            if !order.contains(&key) || results.insert(key, bytes).is_some() {
                return Err(Failure::Engine(
                    "rolling result absent/duplicate sequence".into(),
                ));
            }
            if escrow {
                pool.recycle_result(key).map_err(Failure::Engine)?;
            }
            Ok(false)
        }
        Poll::Started(_) => {
            diagnostics.started_messages += 1;
            Ok(false)
        }
        Poll::Waiting => {
            diagnostics.poll_timeouts += 1;
            Ok(false)
        }
        Poll::Drained => Ok(true),
    }
}

/// Select only completed jobs and prefer a full cut. Partial cuts are reserved
/// for the existing tail/replay drain conditions, never a wall-clock heuristic.
fn select_cut<T>(
    order: &VecDeque<u64>,
    results: &BTreeMap<u64, T>,
    cut_size: usize,
    partial_allowed: bool,
    policy: OwnerDomainWalkEpochPublicationOrder,
) -> Vec<u64> {
    let count = cut_size.min(order.len());
    if count == 0 || (count < cut_size && !partial_allowed) {
        return Vec::new();
    }
    match policy {
        OwnerDomainWalkEpochPublicationOrder::OldestPrefix => {
            if order
                .iter()
                .take(count)
                .all(|key| results.contains_key(key))
            {
                order.iter().take(count).copied().collect()
            } else {
                Vec::new()
            }
        }
        OwnerDomainWalkEpochPublicationOrder::OldestReady => {
            if results.len() >= count {
                results.keys().take(count).copied().collect()
            } else {
                Vec::new()
            }
        }
    }
}

#[cfg(test)]
mod selection_tests {
    use super::*;

    #[test]
    fn ready_bypasses_a_hole_but_never_turns_a_full_window_into_singleton_cuts() {
        use OwnerDomainWalkEpochPublicationOrder::{OldestPrefix, OldestReady};
        let order = (0..32).collect();
        let mut results: BTreeMap<u64, Vec<u8>> = BTreeMap::new();
        for key in 1..16 {
            results.insert(key, Vec::new());
        }
        for tail in [false, true] {
            assert!(select_cut(&order, &results, 16, tail, OldestReady).is_empty());
        }
        results.insert(16, Vec::new());
        assert!(select_cut(&order, &results, 16, true, OldestPrefix).is_empty());
        assert_eq!(
            select_cut(&order, &results, 16, false, OldestReady),
            (1..=16).collect::<Vec<_>>()
        );
        results.insert(0, Vec::new());
        assert_eq!(
            select_cut(&order, &results, 16, false, OldestReady),
            (0..16).collect::<Vec<_>>()
        );
        assert_eq!(
            select_cut(&order, &results, 16, false, OldestPrefix),
            (0..16).collect::<Vec<_>>()
        );
    }

    #[test]
    fn partial_tail_and_replay_wait_for_their_entire_remaining_inventory() {
        let order = VecDeque::from([3, 7, 9]);
        let mut results: BTreeMap<u64, Vec<u8>> =
            BTreeMap::from([(7, Vec::new()), (9, Vec::new())]);
        for policy in [
            OwnerDomainWalkEpochPublicationOrder::OldestPrefix,
            OwnerDomainWalkEpochPublicationOrder::OldestReady,
        ] {
            assert!(select_cut(&order, &results, 16, true, policy).is_empty());
        }
        results.insert(3, Vec::new());
        for policy in [
            OwnerDomainWalkEpochPublicationOrder::OldestPrefix,
            OwnerDomainWalkEpochPublicationOrder::OldestReady,
        ] {
            assert!(select_cut(&order, &results, 16, false, policy).is_empty());
            assert_eq!(select_cut(&order, &results, 16, true, policy), [3, 7, 9]);
            assert!(
                select_cut(
                    &VecDeque::new(),
                    &BTreeMap::<u64, Vec<u8>>::new(),
                    16,
                    true,
                    policy
                )
                .is_empty()
            );
        }
    }
}

/// Consume only already available threaded-pool messages. Inline W1 must not
/// call this: its poll executes a native rather than performing a channel read.
/// The fixed cap preserves cancellation/heartbeat opportunities at wide bounds.
fn collect_available(
    pool: &mut dyn execution::Execution,
    order: &VecDeque<u64>,
    results: &mut BTreeMap<u64, ReturnedBytes>,
    diagnostics: &mut RollingDiagnostics,
    window: usize,
    escrow: bool,
) -> Result<(), Failure> {
    for _ in 0..window.saturating_mul(2).min(256) {
        let message = pool.poll(Duration::ZERO).map_err(Failure::Engine)?;
        if matches!(message, Poll::Waiting | Poll::Drained) {
            break;
        }
        diagnostics.ready_poll_messages += 1;
        collect(message, order, results, diagnostics, pool, escrow)?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn wait_for_receipt(
    pool: &mut dyn execution::Execution,
    order: &VecDeque<u64>,
    results: &mut BTreeMap<u64, ReturnedBytes>,
    diagnostics: &mut RollingDiagnostics,
    publication_wait: bool,
    cut_size: usize,
    window: usize,
    pending: usize,
    inspector_capacity: usize,
    escrow: bool,
) -> Result<bool, Failure> {
    let sample = diagnostics.wait_profile.as_ref().map(|_| {
        let missing = profile::earliest_missing(order, results, cut_size);
        profile::WaitSample::capture_with_inventory(
            publication_wait,
            !order.is_empty(),
            missing,
            window,
            inspector_capacity,
            pending,
            pool.profiled_activity(missing),
            escrow.then_some((order.len(), results.len())),
        )
    });
    let started = Instant::now();
    let message = pool.poll(Duration::from_millis(50));
    let seconds = started.elapsed().as_secs_f64();
    diagnostics.blocking_poll_calls += 1;
    diagnostics.blocking_poll_seconds += seconds;
    if let (Some(profile), Some(sample)) = (&mut diagnostics.wait_profile, sample) {
        profile.record(sample, started, seconds);
    }
    if publication_wait {
        diagnostics.publication_wait_calls += 1;
        diagnostics.publication_wait_seconds += seconds;
    } else {
        diagnostics.prefix_wait_calls += 1;
        diagnostics.prefix_wait_seconds += seconds;
    }
    collect(
        message.map_err(Failure::Engine)?,
        order,
        results,
        diagnostics,
        pool,
        escrow,
    )
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
    mut progress: impl FnMut(&EpochState<N>, &Dispatch, &'static str, &dyn Fn() -> Option<Activity>),
    mut maintenance: impl FnMut(&mut EpochState<N>),
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
    let preparation = preparation_engine(identity)?;
    let base_window = identity.epoch_base_window(b)?;
    let cut_size = identity.epoch_cut_size().min(base_window);
    let escrow = identity.epoch_result_escrow_jobs() != 0 && budget > 1;
    let byte_limit = identity.epoch_result_escrow_bytes().unwrap_or(0);
    if identity.epoch_result_escrow_jobs() != 0 {
        restored.rolling_diagnostics.escrow = Some(EscrowDiagnostics {
            enabled: escrow,
            version: 1,
            base_window,
            total_logical_limit: b,
            extra_job_allowance: identity.epoch_result_escrow_jobs(),
            result_byte_admission_limit: byte_limit,
            ..Default::default()
        });
    }
    let publication_order = identity.epoch_publication_order();
    let outcome = execution::with(budget, authorize, inspect, |pool| {
        let step = catch_unwind(AssertUnwindSafe(|| -> Result<Outcome, Failure> {
            if escrow {
                pool.enable_result_escrow(b).map_err(Failure::Engine)?;
            }
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
                progress(&restored.state, &restored.dispatch, "boundary", &|| {
                    pool.activity()
                });
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
                if committed {
                    // P3 is complete; workers hold immutable lookup leases,
                    // not the observational dependency tracker.
                    maintenance(&mut restored.state);
                }
                if std::mem::take(&mut committed)
                    && restored.state.pending_or_reserved() != 0
                    && periodic_due(restored.state.k)
                {
                    // Workers may inspect during a synchronous save: only the
                    // canonical state is borrowed, and its merge is complete.
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
                    continue;
                }

                let logical_room = b
                    .checked_sub(restored.state.in_flight.len())
                    .ok_or_else(|| Failure::Engine("rolling flight bound exceeded".into()))?;
                if escrow {
                    collect_available(
                        pool,
                        &order,
                        &mut results,
                        &mut restored.rolling_diagnostics,
                        b,
                        true,
                    )?;
                }
                let mut room = base_window.saturating_sub(restored.state.in_flight.len());
                let mut extra_dispatch = false;
                if escrow
                    && room == 0
                    && logical_room != 0
                    && restored.state.ledger.counts().get(Tag::Pending) != 0
                    && profile::earliest_missing(&order, &results, cut_size).is_some()
                {
                    let memory = pool.result_memory();
                    if memory.bytes < byte_limit {
                        if let Some(activity) = pool.activity() {
                            if activity.queued == 0
                                && activity.computing < budget - 1
                                && !results.is_empty()
                            {
                                room = logical_room.min(budget - 1 - activity.computing);
                                extra_dispatch = room != 0;
                            }
                        }
                    }
                }
                if let Some(d) = &mut restored.rolling_diagnostics.escrow {
                    d.logical_reserved = restored.state.in_flight.len();
                    d.peak_logical_reserved = d.peak_logical_reserved.max(d.logical_reserved);
                    d.coordinator_completed = results.len();
                    d.returned_results = pool.result_memory();
                    d.result_byte_overshoot =
                        d.returned_results.peak_bytes.saturating_sub(byte_limit);
                    if let Some(a) = pool.activity() {
                        d.physical_queued = a.queued;
                        d.physical_computing = a.computing;
                        d.physical_returned = a.returned;
                    }
                }
                if room != 0 || !restored.replay.is_empty() {
                    let snapshot = if snapshots.is_some() {
                        let refresh_started = Instant::now();
                        let refreshed = refresh_snapshot(
                            restored,
                            &mut stop_requested,
                            &mut |state, dispatch, phase| {
                                progress(state, dispatch, phase, &|| pool.activity())
                            },
                        )?;
                        restored.rolling_diagnostics.snapshot_refresh_calls += 1;
                        restored.rolling_diagnostics.snapshot_refresh_seconds +=
                            refresh_started.elapsed().as_secs_f64();
                        match refreshed {
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
                    if snapshots.is_some() && snapshot.is_none() {
                        restored.rolling_diagnostics.refill_without_current_snapshot += 1;
                    }
                    // Queued/running jobs pin immutable roots. Publication
                    // respects bounded historical retention and delta charge;
                    // it never clones an unbounded series of whole stores.
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
                            if let Some(d) = &mut restored.rolling_diagnostics.escrow {
                                d.logical_reserved = restored.state.in_flight.len();
                                d.peak_logical_reserved =
                                    d.peak_logical_reserved.max(d.logical_reserved);
                            }
                            if extra_dispatch {
                                restored
                                    .rolling_diagnostics
                                    .escrow
                                    .as_mut()
                                    .expect("enabled escrow")
                                    .extra_jobs_dispatched += jobs.len() as u64;
                            }
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
                        &mut |state, dispatch, phase| {
                            progress(state, dispatch, phase, &|| pool.activity())
                        },
                    )?;
                    on_saved(restored, &receipt, &[]);
                    return Ok(Outcome::Drained);
                }
                if budget > 1 {
                    let drain_started = Instant::now();
                    collect_available(
                        pool,
                        &order,
                        &mut results,
                        &mut restored.rolling_diagnostics,
                        b,
                        escrow,
                    )?;
                    restored.rolling_diagnostics.ready_drain_seconds +=
                        drain_started.elapsed().as_secs_f64();
                }
                let no_unissued = restored.state.ledger.counts().get(Tag::Pending) == 0
                    && restored.dispatch.queued() == (0, 0);
                // A restored partial cohort must retire before Dispatch opens
                // fresh pending work. Waiting to fill this cut would deadlock
                // when the saved unfinished inventory is smaller than B.
                let replay_barrier = !restored.dispatch.admission_ready();
                let keys = select_cut(
                    &order,
                    &results,
                    cut_size,
                    no_unissued || replay_barrier,
                    publication_order,
                );
                if keys.is_empty() {
                    progress(&restored.state, &restored.dispatch, "inspect", &|| {
                        pool.activity()
                    });
                    let drained = wait_for_receipt(
                        pool,
                        &order,
                        &mut results,
                        &mut restored.rolling_diagnostics,
                        false,
                        cut_size,
                        b,
                        restored.state.ledger.counts().get(Tag::Pending) as usize,
                        budget.saturating_sub(1),
                        escrow,
                    )?;
                    if drained && order.is_empty() {
                        return Err(Failure::Engine(
                            "rolling executor drained with undispatched obligations".into(),
                        ));
                    }
                    continue;
                }

                restored.rolling_diagnostics.selected_cuts += 1;
                restored.rolling_diagnostics.selected_partial_cuts +=
                    u64::from(keys.len() < cut_size);
                restored.rolling_diagnostics.selected_nonprefix_cuts += u64::from(
                    !order
                        .iter()
                        .take(keys.len())
                        .copied()
                        .eq(keys.iter().copied()),
                );
                let bytes = keys
                    .iter()
                    .map(|key| results.remove(key).expect("checked ready cut").into_vec())
                    .collect();
                progress(&restored.state, &restored.dispatch, "p1", &|| {
                    pool.activity()
                });
                let checked = merge::p1_check_bound(
                    &mut restored.state,
                    bytes,
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
                                // Keep the original Reserved cut in the saved
                                // ledger so restore replays every discarded byte.
                                drop(checked);
                                return stopped(
                                    restored,
                                    identity,
                                    b,
                                    pool,
                                    snapshots,
                                    &mut results,
                                    reason,
                                    context,
                                    &mut progress,
                                    &mut on_saved,
                                );
                            }
                        };
                    restored.state.preparation.add(&prepared.metrics);
                    let plan = prepared.plan;
                    plan.counters.apply_to(&mut restored.state);
                    if snapshots.is_some() {
                        let retirees = plan.survivors.iter().map(|s| s.retire.len()).sum();
                        loop {
                            // A completed callback has released its lease even
                            // when its bytes are waiting for their sequence.
                            let refresh_started = Instant::now();
                            let refreshed = refresh_snapshot(
                                restored,
                                &mut stop_requested,
                                &mut |state, dispatch, phase| {
                                    progress(state, dispatch, phase, &|| pool.activity())
                                },
                            )?;
                            restored.rolling_diagnostics.snapshot_refresh_calls += 1;
                            restored.rolling_diagnostics.snapshot_refresh_seconds +=
                                refresh_started.elapsed().as_secs_f64();
                            match refreshed {
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
                            progress(&restored.state, &restored.dispatch, "inspect", &|| {
                                pool.activity()
                            });
                            // A drained pool releases the final readers; retry
                            // refresh before deciding whether publication fits.
                            wait_for_receipt(
                                pool,
                                &order,
                                &mut results,
                                &mut restored.rolling_diagnostics,
                                true,
                                cut_size,
                                b,
                                restored.state.ledger.counts().get(Tag::Pending) as usize,
                                budget.saturating_sub(1),
                                escrow,
                            )?;
                        }
                    }
                    progress(&restored.state, &restored.dispatch, "p3", &|| {
                        pool.activity()
                    });
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
                if publication_order == OwnerDomainWalkEpochPublicationOrder::OldestPrefix {
                    for key in keys {
                        if order.pop_front() != Some(key) {
                            return Err(Failure::Engine(
                                "rolling publication prefix changed".into(),
                            ));
                        }
                    }
                } else {
                    // Both inventories are sequence-sorted, but `order` may
                    // contain older unfinished holes. Retire only this cut.
                    let before = order.len();
                    order.retain(|key| keys.binary_search(key).is_err());
                    if before - order.len() != keys.len() {
                        return Err(Failure::Engine(
                            "rolling ready cut inventory changed".into(),
                        ));
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
        progress(&restored.state, &restored.dispatch, "drain_wait", &|| {
            pool.activity()
        });
        result
    });
    progress(&restored.state, &restored.dispatch, "joined", &|| {
        Some(Activity::default())
    });
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
