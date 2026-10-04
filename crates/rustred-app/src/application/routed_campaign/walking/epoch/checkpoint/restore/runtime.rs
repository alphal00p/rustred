//! Complete private restore assembly. No public resume probe calls this yet.
//! The store lock spans decode, validation, session reservation and use; no
//! runnable Dispatch escapes before every state/root/body/closure check passes.
use super::super::super::{anchors::MergedView, dispatch::Dispatch, job::Job, state::EpochState};
use super::super::{
    MergeBoundary, invalid,
    metadata::{Admission, AdmissionFailure, Identity},
    publication,
};
use super::{assembly, dispatch_state::SavedDispatch, roots::Roots};
use crate::application::routed_campaign::walking::{
    descendant_closure::Tracker, epoch::record_store::Sidecar,
};
use rustred::solver::RoutedCandidateReducer;
use std::io;
use std::path::{Path, PathBuf};

mod admission;
mod controller;
mod public;
mod rescue;
pub(in crate::application::routed_campaign::walking::epoch) use public::run;

/// Invocation-local transport observations, never persisted proof authority.
#[derive(Default, serde::Serialize)]
pub(super) struct RollingDiagnostics {
    pub started_messages: u64,
    pub returned_messages: u64,
    pub ready_poll_messages: u64,
    pub ready_drain_seconds: f64,
    pub blocking_poll_calls: u64,
    pub blocking_poll_seconds: f64,
    pub poll_timeouts: u64,
    pub prefix_wait_calls: u64,
    pub prefix_wait_seconds: f64,
    pub publication_wait_calls: u64,
    pub publication_wait_seconds: f64,
    pub refill_without_current_snapshot: u64,
    pub snapshot_refresh_calls: u64,
    pub snapshot_refresh_seconds: f64,
    pub selected_cuts: u64,
    pub selected_partial_cuts: u64,
    pub selected_nonprefix_cuts: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wait_profile: Option<super::super::super::inspector::profile::Waits>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inspection_profile: Option<super::super::super::inspector::profile::Jobs>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub escrow: Option<super::super::super::inspector::EscrowDiagnostics>,
}

pub(super) struct Restored<const N: usize> {
    /// Frozen total logical reservation bound (base plus result escrow).
    /// Inspector width may change on resume without dropping reservations.
    pub window: usize,
    pub cut_size: usize,
    pub rolling_diagnostics: RollingDiagnostics,
    pub state: EpochState<N>,
    pub dispatch: Dispatch,
    /// Original unfinished cut, in original sequence order with fresh seqs.
    /// The controller must execute this whole cut before requesting new work.
    pub replay: Vec<Job<N>>,
    pub roots: Roots,
    pub admission: Admission,
    pub stop_reason: Option<String>,
    pub operational_stop: Option<super::super::stop::Stop>,
    pub admission_failure: Option<AdmissionFailure>,
    pub publisher: publication::Store,
    pub records: Sidecar,
    pub warnings: Vec<String>,
}

fn decoded<const N: usize>(
    directory: &Path,
    pointer: &str,
    identity: &Identity<'_>,
    reducer: &RoutedCandidateReducer<N>,
    lockstep_b: usize,
) -> io::Result<(assembly::Provisional<N>, Roots, Tracker)> {
    let manifest = publication::read_manifest(&directory.join(pointer))?;
    let mut decoded = assembly::read_manifest(directory, identity, lockstep_b, manifest)?;
    let roots = decoded.read_roots(directory, identity, reducer)?;
    // No second node/edge vector: transfer owned flags and stream the already
    // authenticated immutable run log directly into the final runtime CSR.
    let available = decoded.scalars.closure.unavailable.is_none();
    let counters = std::mem::replace(&mut decoded.scalars.closure, Tracker::new(0).counters());
    let mut tracker = Tracker::from_owned_parts(
        counters,
        std::mem::take(&mut decoded.closure_flags),
        decoded.edges.pairs().filter(move |_| available),
    )
    .map_err(io::Error::other)?;
    tracker
        .restore(decoded.store.len(), decoded.scalars.p0 as usize)
        .map_err(io::Error::other)?;
    Ok((decoded, roots, tracker))
}

/// Caller prepared this reducer from the owner payloads fingerprinted in
/// Identity. Preparation remains the existing native import path, not a new CAS.
pub(super) fn open<const N: usize>(
    directory: PathBuf,
    identity: &Identity<'_>,
    reducer: &RoutedCandidateReducer<N>,
    lockstep_b: usize,
) -> io::Result<Restored<N>> {
    if identity.finite_replay_enabled() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "finite replay runtime resume is unsupported; cold checkpoint verification remains available",
        ));
    }
    let mut publisher = publication::Store::open(directory)?;
    let mut warnings = Vec::new();
    let (decoded, roots, tracker) = match decoded(
        publisher.directory(),
        publication::LATEST,
        identity,
        reducer,
        lockstep_b,
    ) {
        Ok(parts) => parts,
        Err(error) if error.kind() == io::ErrorKind::InvalidInput => return Err(error),
        Err(latest) => {
            let parts = decoded(
                publisher.directory(),
                publication::PREVIOUS,
                identity,
                reducer,
                lockstep_b,
            )
            .map_err(|previous| {
                io::Error::new(
                    previous.kind(),
                    format!("epoch latest refused: {latest}; previous refused: {previous}"),
                )
            })?;
            warnings.push(format!(
                "epoch latest refused: {latest}; using validated previous generation"
            ));
            parts
        }
    };
    let assembly::Provisional {
        manifest,
        scalars,
        store,
        ledger,
        nodes,
        live,
        edges,
        anchors,
        frontier_counts,
        dispatch: saved,
        record_segments,
        rescue,
        ..
    } = decoded;
    let SavedDispatch {
        session,
        counter,
        cursor,
        requeue,
        deferred,
        in_flight,
        adaptive,
    } = saved;
    let (max_domains, max_events, max_frontiers) = identity.limits();
    let mut state = EpochState {
        store: store.into(),
        ledger,
        nodes,
        live,
        edges,
        anchors,
        merged_view: MergedView::default(),
        g2_store: None,
        rescue,
        tracker,
        k: scalars.k,
        p0: scalars.p0,
        in_flight,
        frontier_counts,
        counters: scalars.walk,
        verify: scalars.verify,
        lookup: scalars.lookup,
        inspector_lookup: Default::default(),
        preparation: Default::default(),
        max_domains,
        max_events: max_events as u64,
        max_frontiers: max_frontiers as u64,
        poisoned: false,
    };
    if scalars.g2 == "union" && matches!(scalars.initial_admission, Admission::Complete) {
        super::super::super::g2::enable(&mut state);
    }
    if matches!(scalars.initial_admission, Admission::InProgress) {
        Dispatch::validate_admission(
            &state,
            super::super::super::dispatch::DispatchSnapshot {
                session,
                counter,
                cursor,
                requeue: &requeue,
                deferred: &deferred,
                adaptive: adaptive.as_ref(),
            },
        )
        .map_err(io::Error::other)?;
        if !record_segments.is_empty() {
            return Err(invalid("epoch incomplete admission has record segments"));
        }
    }
    let window = if identity.epoch_rolling() {
        scalars.lockstep_b
    } else {
        lockstep_b
    };
    identity.epoch_base_window(window)?;
    // Last fallible authority operation before making a Dispatch. Session
    // exhaustion/failure issues no jobs; a later assembly error burns it safely.
    let session = publisher.adopt(manifest, session)?;
    let (dispatch, replay) = Dispatch::restored(
        session,
        cursor,
        requeue,
        deferred,
        adaptive,
        matches!(scalars.initial_admission, Admission::Complete),
        &mut state,
    )
    .map_err(io::Error::other)?;
    MergeBoundary::borrow(&state, &dispatch, window)?;
    if record_segments.iter().try_fold(0usize, |sum, segment| {
        usize::try_from(segment.count)
            .ok()
            .and_then(|count| sum.checked_add(count))
    }) != usize::try_from(state.edges.runs()).ok()
    {
        return Err(invalid(
            "epoch record inventory exceeds runtime address space",
        ));
    }
    let records = Sidecar::restored(
        publisher.directory().to_path_buf(),
        record_segments,
        publisher.next_generation(),
    );
    Ok(Restored {
        window,
        cut_size: identity.epoch_cut_size(),
        rolling_diagnostics: Default::default(),
        state,
        dispatch,
        replay,
        roots,
        admission: scalars.initial_admission,
        stop_reason: scalars.stop_reason,
        operational_stop: scalars.operational_stop,
        admission_failure: scalars.admission_failure,
        publisher,
        records,
        warnings,
    })
}

#[cfg(test)]
mod tests;
