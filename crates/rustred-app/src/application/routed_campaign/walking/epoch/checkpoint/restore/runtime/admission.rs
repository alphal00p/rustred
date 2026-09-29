//! Private prepared-input lifecycle. No pool exists until Ready is returned.
use super::super::super::{
    MergeBoundary, invalid,
    metadata::{Admission, AdmissionFailure, AdmissionFailureKind, Identity, Inputs},
    publication, stop,
};
use super::super::roots;
use super::{Dispatch, EpochState, Restored, Roots, Sidecar, Tracker, controller};
use crate::application::routed_campaign::{
    matching::input::Query,
    walking::{
        OwnerDomainWalkRequest,
        epoch::{AdmissionError, admission, merge::StopReason},
        initial_overlap::InitialOverlapIndex,
        queue::Phase,
    },
};
use rustred::solver::RoutedCandidateReducer;
use serde_json::Value;
use std::{
    io,
    panic::{AssertUnwindSafe, catch_unwind},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Outcome {
    Ready,
    Stopped(StopReason),
}

pub(super) fn fresh<const N: usize>(
    directory: PathBuf,
    identity: &Identity<'_>,
) -> io::Result<Restored<N>> {
    let publisher = publication::Store::fresh(directory.clone())?;
    let records = Sidecar::new(directory, publisher.next_generation());
    let (domains, events, frontiers) = identity.limits();
    Ok(Restored {
        state: EpochState::new(domains, events, frontiers),
        dispatch: Dispatch::for_admission(),
        replay: Vec::new(),
        roots: Roots {
            rows: Vec::new(),
            frontiers: Vec::new(),
        },
        admission: Admission::InProgress,
        stop_reason: None,
        operational_stop: None,
        admission_failure: None,
        publisher,
        records,
        warnings: Vec::new(),
    })
}

/// Optional overlap for the exact protected inventory, before any job. The
/// caller still rechecks its operational stop before constructing Context /
/// entering the controller; an index is not permission to ignore cancellation.
pub(super) fn initial_overlap<const N: usize>(
    restored: &Restored<N>,
    request: &OwnerDomainWalkRequest,
    cancellation: &AtomicBool,
) -> io::Result<InitialOverlapIndex<N>> {
    if !matches!(restored.admission, Admission::Complete)
        || !restored.dispatch.admission_ready()
        || restored.admission_failure.is_some()
        || !restored.replay.is_empty()
        || restored.records.total() != 0
    {
        return Err(invalid(
            "epoch initial overlap requires completed unissued admission",
        ));
    }
    Dispatch::validate_admission(&restored.state, restored.dispatch.checkpoint_snapshot())
        .map_err(io::Error::other)?;
    if !request.reuse_initial_d_bands {
        return Ok(InitialOverlapIndex::empty());
    }
    let cancelled = || {
        if cancellation.load(Ordering::Acquire) {
            Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "epoch initial overlap cancelled before jobs",
            ))
        } else {
            Ok(())
        }
    };
    cancelled()?;
    let mut prefix = Vec::new();
    prefix
        .try_reserve_exact(restored.state.p0 as usize)
        .map_err(|_| io::Error::other("epoch initial overlap prefix allocation"))?;
    for image in &restored.state.store.domains[..restored.state.p0 as usize] {
        cancelled()?;
        prefix.push(Arc::new(image.expand()));
    }
    let index = InitialOverlapIndex::from_initial(&prefix, cancellation);
    cancelled()?;
    Ok(index)
}

fn engine<const N: usize>(restored: &mut Restored<N>, reason: String) -> io::Error {
    restored.state.poisoned = true;
    match restored.publisher.poison(restored.state.k, &reason) {
        Ok(()) => io::Error::other(reason),
        Err(error) => io::Error::other(format!("{reason}; durable poison failed: {error}")),
    }
}

/// Only empty-work initial state reaches this boundary; no native graph is
/// rebuilt. Preserve genuine unavailable state without allocating node flags.
fn boundary<const N: usize>(restored: &mut Restored<N>) -> io::Result<()> {
    let state = &mut restored.state;
    state.p0 = state.watermark();
    state.counters.frontiers = restored.roots.frontiers.len() as u64;
    let unavailable = state.tracker.counters().unavailable;
    state.tracker = if let Some(reason) = unavailable {
        let mut empty = Tracker::new(0);
        empty.disable(&reason);
        let mut counters = empty.counters();
        counters.initial = state.p0 as usize;
        Tracker::from_owned_parts(counters, Vec::new(), std::iter::empty())
            .map_err(io::Error::other)?
    } else {
        Tracker::new(state.p0 as usize)
    };
    if !restored.roots.frontiers.is_empty() {
        state
            .tracker
            .disable("initial input obligations were not completely admitted");
    }
    Ok(())
}

fn save<const N: usize>(
    restored: &mut Restored<N>,
    identity: &Identity<'_>,
    b: usize,
    reason: StopReason,
    context: Option<stop::Stop>,
    on_saved: &mut impl FnMut(&publication::Receipt),
) -> io::Result<Outcome> {
    boundary(restored)?;
    match controller::save(restored, identity, b, Some(reason), context) {
        Ok(receipt) => {
            on_saved(&receipt);
            Ok(Outcome::Stopped(reason))
        }
        Err(controller::Failure::Save(error)) => Err(error),
        Err(controller::Failure::Engine(reason)) => Err(engine(restored, reason)),
    }
}

pub(super) fn continue_admission<const N: usize>(
    restored: &mut Restored<N>,
    identity: &Identity<'_>,
    reducer: &RoutedCandidateReducer<N>,
    b: usize,
    stop_requested: impl FnMut() -> Option<stop::Stop>,
    on_saved: impl FnMut(&publication::Receipt),
) -> io::Result<Outcome> {
    continue_with(
        restored,
        identity,
        reducer,
        b,
        stop_requested,
        on_saved,
        admission::one,
    )
}

// The one-row seam only injects existing precommit allocation failures in
// tests; production always calls the shared legacy admission implementation.
#[allow(clippy::too_many_arguments)]
pub(super) fn continue_with<const N: usize>(
    restored: &mut Restored<N>,
    identity: &Identity<'_>,
    reducer: &RoutedCandidateReducer<N>,
    b: usize,
    mut stop_requested: impl FnMut() -> Option<stop::Stop>,
    mut on_saved: impl FnMut(&publication::Receipt),
    mut one: impl FnMut(
        &mut EpochState<N>,
        &Query,
        Option<Phase>,
        &mut Vec<Value>,
        &mut Vec<Value>,
    ) -> Result<(), AdmissionError>,
) -> io::Result<Outcome> {
    if !matches!(restored.admission, Admission::InProgress)
        || !restored.replay.is_empty()
        || restored.records.total() != 0
    {
        return Err(invalid(
            "private epoch continuation requires an unfinished initial prefix",
        ));
    }
    restored
        .dispatch
        .check_admission(&restored.state)
        .map_err(io::Error::other)?;
    MergeBoundary::borrow(&restored.state, &restored.dispatch, b)?;
    if restored.operational_stop.as_ref().is_some_and(|context| {
        !context.valid() || restored.stop_reason.as_deref() != Some(context.kind().name())
    }) {
        return Err(invalid("epoch admission operational context differs"));
    }
    super::super::super::metadata::validate_failure(
        restored.admission_failure.as_ref(),
        restored.admission,
        restored.roots.rows.len(),
        identity.queries().len(),
        restored.stop_reason.as_deref(),
        restored.operational_stop.is_some(),
    )?;
    roots::validate(
        &restored.roots,
        identity,
        reducer,
        &restored.state.store,
        restored.state.p0,
    )?;
    let result = catch_unwind(AssertUnwindSafe(|| -> io::Result<Outcome> {
        loop {
            if let Some(context) = stop_requested() {
                return save(
                    restored,
                    identity,
                    b,
                    context.kind(),
                    Some(context),
                    &mut on_saved,
                );
            }
            if restored
                .admission_failure
                .as_ref()
                .is_some_and(|failure| failure.kind == AdmissionFailureKind::Refused)
            {
                return save(
                    restored,
                    identity,
                    b,
                    StopReason::ErrorStop,
                    None,
                    &mut on_saved,
                );
            }
            let index = restored.roots.rows.len();
            let Some(query) = identity.queries().get(index) else {
                boundary(restored)?;
                roots::validate(
                    &restored.roots,
                    identity,
                    reducer,
                    &restored.state.store,
                    restored.state.p0,
                )
                .map_err(|error| engine(restored, error.to_string()))?;
                let inputs = Inputs {
                    identity,
                    admission: Admission::Complete,
                    rows: &restored.roots.rows,
                    frontiers: &restored.roots.frontiers,
                    stop: None,
                    operational_stop: None,
                    admission_failure: None,
                };
                inputs.validate(&MergeBoundary::borrow(
                    &restored.state,
                    &restored.dispatch,
                    b,
                )?)?;
                if restored.admission_failure.is_some() {
                    return Err(engine(
                        restored,
                        "complete admission retains failed query".into(),
                    ));
                }
                restored
                    .dispatch
                    .complete_admission(&restored.state)
                    .map_err(|reason| engine(restored, reason))?;
                restored.admission = Admission::Complete;
                restored.stop_reason = None;
                restored.operational_stop = None;
                if let Some(context) = stop_requested() {
                    return save(
                        restored,
                        identity,
                        b,
                        context.kind(),
                        Some(context),
                        &mut on_saved,
                    );
                }
                return Ok(Outcome::Ready);
            };
            let phase = admission::query_phase(reducer, identity.route_domain_overcover(), query);
            match one(
                &mut restored.state,
                query,
                phase,
                &mut restored.roots.rows,
                &mut restored.roots.frontiers,
            ) {
                Ok(()) => {
                    restored.admission_failure = None;
                }
                Err(error) => {
                    let failure = match AdmissionFailure::from_error(index, error) {
                        Ok(failure) => failure,
                        Err(error) => return Err(engine(restored, error.to_string())),
                    };
                    let reason = failure.reason();
                    restored.admission_failure = Some(failure);
                    return save(restored, identity, b, reason, None, &mut on_saved);
                }
            }
        }
    }));
    match result {
        Ok(result) => result,
        Err(_) => Err(engine(
            restored,
            "panic in private epoch input continuation".into(),
        )),
    }
}
