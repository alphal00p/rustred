use super::super::FoundryCampaignSchedulerRejection;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum K6OrbitCampaignState {
    /// This canonical sibling has not yet installed its fresh ledger.
    Pending,
    /// The sibling owns a live ledger and is still discovering owners.
    Running,
    /// This sector closed and its complete same-rank wave was published.
    Published,
    /// This sector closed, but another sibling did not, so the seal was
    /// deliberately dropped without publishing a partial wave.
    ClosedUnpublished,
    NeedsRefinement,
    OperationallyBounded,
    ExhaustedAtConfig,
}

impl K6OrbitCampaignState {
    pub const fn stable_id(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Published => "published",
            Self::ClosedUnpublished => "closed-unpublished",
            Self::NeedsRefinement => "needs-refinement",
            Self::OperationallyBounded => "operationally-bounded",
            Self::ExhaustedAtConfig => "exhausted-at-config",
        }
    }
}

/// Detached deterministic progress for one canonical full-rank orbit.
///
/// This value carries no live ledger, exact owner, or publication authority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct K6OrbitCampaignProgress {
    orbit_ordinal: usize,
    representative: [i64; 6],
    active_count: usize,
    state: K6OrbitCampaignState,
    ledger_revision: u64,
    owner_count: usize,
    uncovered_box_count: usize,
    task_reports: usize,
    first_scheduler_rejection: Option<FoundryCampaignSchedulerRejection>,
    terminal_scheduler_rejection: Option<FoundryCampaignSchedulerRejection>,
}

impl K6OrbitCampaignProgress {
    pub const fn orbit_ordinal(&self) -> usize {
        self.orbit_ordinal
    }

    pub const fn representative(&self) -> &[i64; 6] {
        &self.representative
    }

    pub const fn active_count(&self) -> usize {
        self.active_count
    }

    pub const fn state(&self) -> K6OrbitCampaignState {
        self.state
    }

    pub const fn ledger_revision(&self) -> u64 {
        self.ledger_revision
    }

    pub const fn owner_count(&self) -> usize {
        self.owner_count
    }

    pub const fn uncovered_box_count(&self) -> usize {
        self.uncovered_box_count
    }

    pub const fn task_reports(&self) -> usize {
        self.task_reports
    }

    pub const fn first_scheduler_rejection(&self) -> Option<FoundryCampaignSchedulerRejection> {
        self.first_scheduler_rejection
    }

    pub const fn terminal_scheduler_rejection(&self) -> Option<FoundryCampaignSchedulerRejection> {
        self.terminal_scheduler_rejection
    }

    pub(super) fn mark_published(&mut self) {
        debug_assert_eq!(self.state, K6OrbitCampaignState::ClosedUnpublished);
        self.state = K6OrbitCampaignState::Published;
    }

    pub(super) fn attach_scheduler_rejections(
        &mut self,
        first: Option<FoundryCampaignSchedulerRejection>,
        terminal: Option<FoundryCampaignSchedulerRejection>,
    ) {
        debug_assert!(self.first_scheduler_rejection.is_none());
        debug_assert!(self.terminal_scheduler_rejection.is_none());
        self.first_scheduler_rejection = first;
        self.terminal_scheduler_rejection = terminal;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum K6WaveCampaignState {
    Running,
    Published,
    Incomplete,
}

impl K6WaveCampaignState {
    pub const fn stable_id(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Published => "published",
            Self::Incomplete => "incomplete",
        }
    }
}

/// Detached deterministic progress for one atomic same-rank wave.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct K6WaveCampaignProgress {
    wave_ordinal: usize,
    active_count: usize,
    pub(super) state: K6WaveCampaignState,
    pub(super) orbits: Box<[K6OrbitCampaignProgress]>,
}

impl K6WaveCampaignProgress {
    pub const fn wave_ordinal(&self) -> usize {
        self.wave_ordinal
    }

    pub const fn active_count(&self) -> usize {
        self.active_count
    }

    pub const fn state(&self) -> K6WaveCampaignState {
        self.state
    }

    pub fn orbits(&self) -> &[K6OrbitCampaignProgress] {
        &self.orbits
    }

    pub fn closed_orbit_count(&self) -> usize {
        self.orbits
            .iter()
            .filter(|orbit| {
                matches!(
                    orbit.state(),
                    K6OrbitCampaignState::Published | K6OrbitCampaignState::ClosedUnpublished
                )
            })
            .count()
    }
}

const K6_PROGRESS_WAKE_CAPACITY: usize = 1;
pub(super) const K6_PROGRESS_POLL_INTERVAL: Duration = Duration::from_millis(10);

/// One worker-written, coordinator-read latest-value slot.
///
/// Each orbit has exactly one writer. The sequence word is a compact seqlock:
/// an odd value denotes an in-progress update and an unchanged even value
/// brackets a coherent scalar read. Solver workers perform only atomics and a
/// nonblocking `try_send`; a slow observer can therefore coalesce updates but
/// can never stall discovery or grow a queue.
#[derive(Debug)]
struct LatestK6OrbitProgress {
    sequence: AtomicU64,
    state: AtomicU8,
    ledger_revision: AtomicU64,
    owner_count: AtomicUsize,
    uncovered_box_count: AtomicUsize,
    task_reports: AtomicUsize,
}

impl LatestK6OrbitProgress {
    fn pending() -> Self {
        Self {
            sequence: AtomicU64::new(0),
            state: AtomicU8::new(K6OrbitCampaignState::Pending as u8),
            ledger_revision: AtomicU64::new(0),
            owner_count: AtomicUsize::new(0),
            uncovered_box_count: AtomicUsize::new(0),
            task_reports: AtomicUsize::new(0),
        }
    }

    fn publish(
        &self,
        state: K6OrbitCampaignState,
        ledger_revision: u64,
        owner_count: usize,
        uncovered_box_count: usize,
        task_reports: usize,
    ) {
        let previous = self.sequence.fetch_add(1, AtomicOrdering::SeqCst);
        debug_assert_eq!(previous % 2, 0, "one K6 orbit slot acquired two writers");
        self.state.store(state as u8, AtomicOrdering::SeqCst);
        self.ledger_revision
            .store(ledger_revision, AtomicOrdering::SeqCst);
        self.owner_count.store(owner_count, AtomicOrdering::SeqCst);
        self.uncovered_box_count
            .store(uncovered_box_count, AtomicOrdering::SeqCst);
        self.task_reports
            .store(task_reports, AtomicOrdering::SeqCst);
        self.sequence.fetch_add(1, AtomicOrdering::SeqCst);
    }

    fn read(&self) -> Result<K6OrbitProgressScalars, K6WaveCampaignError> {
        loop {
            let before = self.sequence.load(AtomicOrdering::SeqCst);
            if before % 2 != 0 {
                std::hint::spin_loop();
                continue;
            }
            let state = decode_orbit_state(self.state.load(AtomicOrdering::SeqCst))?;
            let snapshot = K6OrbitProgressScalars {
                state,
                ledger_revision: self.ledger_revision.load(AtomicOrdering::SeqCst),
                owner_count: self.owner_count.load(AtomicOrdering::SeqCst),
                uncovered_box_count: self.uncovered_box_count.load(AtomicOrdering::SeqCst),
                task_reports: self.task_reports.load(AtomicOrdering::SeqCst),
            };
            let after = self.sequence.load(AtomicOrdering::SeqCst);
            if before == after {
                return Ok(snapshot);
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct K6OrbitProgressScalars {
    pub(super) state: K6OrbitCampaignState,
    pub(super) ledger_revision: u64,
    pub(super) owner_count: usize,
    pub(super) uncovered_box_count: usize,
    pub(super) task_reports: usize,
}

fn decode_orbit_state(encoded: u8) -> Result<K6OrbitCampaignState, K6WaveCampaignError> {
    match encoded {
        value if value == K6OrbitCampaignState::Pending as u8 => Ok(K6OrbitCampaignState::Pending),
        value if value == K6OrbitCampaignState::Running as u8 => Ok(K6OrbitCampaignState::Running),
        value if value == K6OrbitCampaignState::Published as u8 => {
            Ok(K6OrbitCampaignState::Published)
        }
        value if value == K6OrbitCampaignState::ClosedUnpublished as u8 => {
            Ok(K6OrbitCampaignState::ClosedUnpublished)
        }
        value if value == K6OrbitCampaignState::NeedsRefinement as u8 => {
            Ok(K6OrbitCampaignState::NeedsRefinement)
        }
        value if value == K6OrbitCampaignState::OperationallyBounded as u8 => {
            Ok(K6OrbitCampaignState::OperationallyBounded)
        }
        value if value == K6OrbitCampaignState::ExhaustedAtConfig as u8 => {
            Ok(K6OrbitCampaignState::ExhaustedAtConfig)
        }
        _ => Err(K6WaveCampaignError::Invariant {
            detail: "K6 latest-progress slot contains an unknown orbit state",
        }),
    }
}

#[derive(Debug)]
pub(super) struct LatestK6WaveProgress {
    wave_ordinal: usize,
    active_count: usize,
    orbit_start: usize,
    orbits: Box<[LatestK6OrbitProgress]>,
    pub(super) wake: SyncSender<()>,
}

impl LatestK6WaveProgress {
    pub(super) fn try_new(
        wave_ordinal: usize,
        active_count: usize,
        orbit_start: usize,
        wave_width: usize,
    ) -> Result<(Self, Receiver<()>), K6WaveCampaignError> {
        let orbit_end =
            orbit_start
                .checked_add(wave_width)
                .ok_or(K6WaveCampaignError::Invariant {
                    detail: "K6 progress orbit range overflowed",
                })?;
        if FULL_RANK_ORBITS.get(orbit_start..orbit_end).is_none() {
            return Err(K6WaveCampaignError::Invariant {
                detail: "K6 progress orbit range exceeds the canonical manifest",
            });
        }
        let mut slots = Vec::new();
        slots.try_reserve_exact(wave_width).map_err(|_| {
            K6WaveCampaignError::AllocationFailure {
                resource: "K6 latest-progress orbit slots",
                requested: wave_width,
            }
        })?;
        slots.resize_with(wave_width, LatestK6OrbitProgress::pending);
        let (wake, receiver) = sync_channel(K6_PROGRESS_WAKE_CAPACITY);
        Ok((
            Self {
                wave_ordinal,
                active_count,
                orbit_start,
                orbits: slots.into_boxed_slice(),
                wake,
            },
            receiver,
        ))
    }

    pub(super) fn publish_snapshot(
        &self,
        local_ordinal: usize,
        state: K6OrbitCampaignState,
        exact: ExactOwnerCoverSnapshot,
        census: ProbeCoordinatorCensus,
    ) {
        self.publish_scalars(
            local_ordinal,
            K6OrbitProgressScalars {
                state,
                ledger_revision: exact.revision().get(),
                owner_count: exact.owner_count(),
                uncovered_box_count: exact.uncovered_box_count(),
                task_reports: census.task_reports(),
            },
        );
    }

    pub(super) fn publish_scalars(&self, local_ordinal: usize, progress: K6OrbitProgressScalars) {
        let slot = self
            .orbits
            .get(local_ordinal)
            .expect("map_ordered yielded a K6 sibling outside the admitted wave");
        slot.publish(
            progress.state,
            progress.ledger_revision,
            progress.owner_count,
            progress.uncovered_box_count,
            progress.task_reports,
        );
        match self.wake.try_send(()) {
            Ok(()) | Err(TrySendError::Full(())) | Err(TrySendError::Disconnected(())) => {}
        }
    }

    pub(super) fn try_snapshot(
        &self,
        state: K6WaveCampaignState,
    ) -> Result<K6WaveCampaignProgress, K6WaveCampaignError> {
        let mut orbits = Vec::new();
        orbits.try_reserve_exact(self.orbits.len()).map_err(|_| {
            K6WaveCampaignError::AllocationFailure {
                resource: "detached K6 orbit progress",
                requested: self.orbits.len(),
            }
        })?;
        for (local_ordinal, slot) in self.orbits.iter().enumerate() {
            let orbit_ordinal = self.orbit_start + local_ordinal;
            let orbit =
                FULL_RANK_ORBITS
                    .get(orbit_ordinal)
                    .ok_or(K6WaveCampaignError::Invariant {
                        detail: "K6 progress snapshot exceeds the canonical orbit manifest",
                    })?;
            let scalar = slot.read()?;
            let active_count = orbit
                .representative
                .iter()
                .filter(|&&power| power > 0)
                .count();
            if active_count != self.active_count {
                return Err(K6WaveCampaignError::Invariant {
                    detail: "K6 progress snapshot crosses a same-rank wave boundary",
                });
            }
            orbits.push(K6OrbitCampaignProgress {
                orbit_ordinal,
                representative: orbit.representative,
                active_count,
                state: scalar.state,
                ledger_revision: scalar.ledger_revision,
                owner_count: scalar.owner_count,
                uncovered_box_count: scalar.uncovered_box_count,
                task_reports: scalar.task_reports,
                first_scheduler_rejection: None,
                terminal_scheduler_rejection: None,
            });
        }
        Ok(K6WaveCampaignProgress {
            wave_ordinal: self.wave_ordinal,
            active_count: self.active_count,
            state,
            orbits: orbits.into_boxed_slice(),
        })
    }
}

pub(super) struct K6ProgressExecutionGuard<'progress> {
    pub(super) finished: &'progress AtomicBool,
    pub(super) wake: &'progress SyncSender<()>,
}

impl Drop for K6ProgressExecutionGuard<'_> {
    fn drop(&mut self) {
        self.finished.store(true, AtomicOrdering::Release);
        match self.wake.try_send(()) {
            Ok(()) | Err(TrySendError::Full(())) | Err(TrySendError::Disconnected(())) => {}
        }
    }
}
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, AtomicUsize, Ordering as AtomicOrdering};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::time::Duration;

use crate::foundry::artifact::FULL_RANK_ORBITS;
use crate::foundry::completion::source_discovery::{
    ExactOwnerCoverSnapshot, ProbeCoordinatorCensus,
};

use super::K6WaveCampaignError;
