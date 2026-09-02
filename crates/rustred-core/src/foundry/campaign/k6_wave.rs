//! Bottom-up K6 same-rank wave orchestration.
//!
//! Every sibling ledger in one `[2, 2, 1, 1]` wave is constructed against the
//! identical immutable predecessor. Normal incomplete stops remain
//! unpublished, and only a complete vector of closed consuming ledger seals
//! can enter the transactional sector-wave publisher. The public result keeps
//! that authority alive behind an opaque boundary while exposing only
//! detached deterministic progress values.

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};

use crate::campaign::{ParallelExecution, ParallelExecutionError};
use crate::foundry::artifact::FULL_RANK_ORBITS;
#[cfg(test)]
use crate::foundry::completion::source_discovery::CanonicalExactOwnerLedger;
#[cfg(test)]
use crate::foundry::completion::source_discovery::ProbeCoordinatorCensus;
use crate::foundry::completion::source_discovery::{
    ClosedExactExecutableOwnerCover, ClosedSectorClosureWave, ExactOwnerLedgerSealError,
    ProbeCampaignAdapter, ProbeCampaignLimits, ProbeCoordinatorConfig, ProbeCoordinatorStop,
    StagedSectorClosureError, StagedSectorClosureLimits, try_publish_sealed_sector_wave,
};
use crate::foundry::completion::stratum::ImmutableOwnerSnapshot;
use crate::sector::OrderingPolicy;

use super::k6_resource::K6CampaignResourceProfile;
use super::preset_k6::{
    K6AlgebraInputs, k6_root_predecessor_for_ordering, shared_k6_algebra_inputs,
    try_new_k6_full_rank_ledger_with_profile_and_ordering,
};
use super::run::{
    RetainedLedgerCampaignRun, detach_report, detach_scheduler_rejection,
    try_build_coordinator_config, try_drive_live_ledger_until_terminal_with_progress,
};
use super::{
    FoundryCampaignConfig, FoundryCampaignError, FoundryCampaignItinerary, FoundryCampaignReport,
    FoundryCampaignSetupStage,
};

mod progress;

use progress::{
    K6_PROGRESS_POLL_INTERVAL, K6OrbitProgressScalars, K6ProgressExecutionGuard,
    LatestK6WaveProgress,
};
pub use progress::{
    K6OrbitCampaignProgress, K6OrbitCampaignState, K6WaveCampaignProgress, K6WaveCampaignState,
};

/// Canonical bottom-up widths of the full-rank K6 orbit waves.
pub const K6_FULL_RANK_WAVE_WIDTHS: [usize; 4] = [2, 2, 1, 1];

/// One normal, unpublished sector stop retaining its exact live ledger.
#[derive(Debug)]
pub(crate) struct K6SectorCampaignStop {
    _orbit_ordinal: usize,
    _retained: RetainedLedgerCampaignRun,
}

/// Complete detached diagnostics for one sibling that prevented an atomic
/// same-rank wave from publishing.
///
/// This value has no ledger, rule-owner, or publication authority. It exists
/// so release campaign clients can inspect the exact residual census,
/// caller-bounded box coordinates, and typed stop that selected the next
/// completion slice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct K6IncompleteOrbitReport {
    orbit_ordinal: usize,
    report: FoundryCampaignReport,
}

impl K6IncompleteOrbitReport {
    pub const fn orbit_ordinal(&self) -> usize {
        self.orbit_ordinal
    }

    pub const fn report(&self) -> &FoundryCampaignReport {
        &self.report
    }
}

#[cfg(test)]
impl K6SectorCampaignStop {
    pub(crate) const fn orbit_ordinal(&self) -> usize {
        self._orbit_ordinal
    }

    pub(crate) const fn ledger(&self) -> &CanonicalExactOwnerLedger {
        self._retained.ledger()
    }

    pub(crate) const fn terminal_stop(&self) -> &ProbeCoordinatorStop {
        self._retained.terminal_stop()
    }

    pub(crate) const fn final_census(&self) -> ProbeCoordinatorCensus {
        self._retained.final_census()
    }
}

/// Atomic same-rank wave that could not yet be published.
///
/// Closed sibling seals, if any, have been dropped rather than exposed. The
/// retained stops are the exact normal bounded/refinement states that blocked
/// publication against `predecessor`.
#[derive(Debug)]
pub struct K6IncompleteSectorWave {
    wave_ordinal: usize,
    active_count: usize,
    _predecessor: ImmutableOwnerSnapshot,
    published_waves: Box<[ClosedSectorClosureWave]>,
    closed_sector_count: usize,
    _stops: Box<[K6SectorCampaignStop]>,
    incomplete_orbits: Box<[K6IncompleteOrbitReport]>,
    progress: Box<[K6WaveCampaignProgress]>,
}

impl K6IncompleteSectorWave {
    pub const fn wave_ordinal(&self) -> usize {
        self.wave_ordinal
    }

    pub const fn active_count(&self) -> usize {
        self.active_count
    }

    #[cfg(test)]
    pub(crate) const fn predecessor(&self) -> &ImmutableOwnerSnapshot {
        &self._predecessor
    }

    pub const fn closed_sector_count(&self) -> usize {
        self.closed_sector_count
    }

    /// Number of lower waves already published before the incomplete wave.
    pub fn published_wave_count(&self) -> usize {
        self.published_waves.len()
    }

    /// Detached progress for every published lower wave followed by the
    /// current incomplete wave.
    pub fn progress(&self) -> &[K6WaveCampaignProgress] {
        &self.progress
    }

    /// Exact detached residual reports for every sibling which blocked this
    /// wave. Reported boxes obey the caller's explicit diagnostic ceiling and
    /// carry a truncation bit when that ceiling is smaller than the partition.
    pub fn incomplete_orbits(&self) -> &[K6IncompleteOrbitReport] {
        &self.incomplete_orbits
    }

    #[cfg(test)]
    pub(crate) fn stops(&self) -> &[K6SectorCampaignStop] {
        &self._stops
    }
}

/// All four exact same-rank waves published and eligible for generic in-memory
/// artifact installation. Durable encoding remains a separate consuming app
/// boundary so no partial or diagnostic campaign state can be serialized.
#[derive(Debug)]
pub struct K6PublishedSectorWaves {
    waves: Box<[ClosedSectorClosureWave]>,
    progress: Box<[K6WaveCampaignProgress]>,
}

impl K6PublishedSectorWaves {
    pub fn published_wave_count(&self) -> usize {
        self.waves.len()
    }

    pub fn published_orbit_count(&self) -> usize {
        self.progress.iter().map(|wave| wave.orbits().len()).sum()
    }

    /// Detached progress for all four successfully published waves.
    pub fn progress(&self) -> &[K6WaveCampaignProgress] {
        &self.progress
    }

    /// Consume only the proof-bearing wave chain for closing-artifact
    /// installation. Detached search progress is dropped here by design, so
    /// autonomous and externally hinted searches with identical exact owners
    /// produce the same artifact payload.
    pub(crate) fn into_artifact_waves(self) -> Box<[ClosedSectorClosureWave]> {
        self.waves
    }

    /// Install the fully published proof chain as a standalone generic
    /// closing artifact. No campaign progress or search provenance crosses
    /// this consuming boundary.
    pub fn into_closed_artifact(
        self,
    ) -> Result<crate::foundry::artifact::ClosedArtifact, crate::foundry::artifact::ArtifactError>
    {
        crate::foundry::artifact::install_published_k6_sector_waves(self)
    }
}

/// Proof-retaining campaign outcome. `Published` still requires the consuming
/// `into_closed_artifact` installation boundary before it is a closing
/// artifact; the application must still encode and cold-reload the installed
/// value before publishing durable bytes.
#[derive(Debug)]
pub enum K6WaveCampaignOutcome {
    Published(K6PublishedSectorWaves),
    Incomplete(K6IncompleteSectorWave),
}

/// Stable public category for a hard all-wave campaign failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum K6WaveCampaignErrorKind {
    Campaign,
    ResourceLimit,
    ParallelExecution,
    ProgressAggregation,
    LedgerSeal,
    WavePublication,
    AllocationFailure,
    Invariant,
}

/// Opaque hard failure from the proof-retaining all-wave runner.
#[derive(Debug)]
pub struct K6WaveCampaignRunError(K6WaveCampaignError);

impl K6WaveCampaignRunError {
    pub const fn kind(&self) -> K6WaveCampaignErrorKind {
        match &self.0 {
            K6WaveCampaignError::Campaign(error) => match error {
                FoundryCampaignError::ResourceCountOverflow { .. }
                | FoundryCampaignError::ResourceLimit { .. } => {
                    K6WaveCampaignErrorKind::ResourceLimit
                }
                FoundryCampaignError::Invariant { .. } => K6WaveCampaignErrorKind::Invariant,
                FoundryCampaignError::Setup { .. } | FoundryCampaignError::Execution { .. } => {
                    K6WaveCampaignErrorKind::Campaign
                }
            },
            K6WaveCampaignError::ParallelExecution(_) => K6WaveCampaignErrorKind::ParallelExecution,
            K6WaveCampaignError::ProgressAggregation { .. } => {
                K6WaveCampaignErrorKind::ProgressAggregation
            }
            K6WaveCampaignError::LedgerSeal(_) => K6WaveCampaignErrorKind::LedgerSeal,
            K6WaveCampaignError::WavePublication(_) => K6WaveCampaignErrorKind::WavePublication,
            K6WaveCampaignError::AllocationFailure { .. } => {
                K6WaveCampaignErrorKind::AllocationFailure
            }
            K6WaveCampaignError::Invariant { .. } => K6WaveCampaignErrorKind::Invariant,
        }
    }
}

impl fmt::Display for K6WaveCampaignRunError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl std::error::Error for K6WaveCampaignRunError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

/// Hard setup/execution/publication failure. Ordinary bounded or incomplete
/// discovery is represented by [`K6WaveCampaignOutcome::Incomplete`].
#[derive(Debug)]
pub(crate) enum K6WaveCampaignError {
    Campaign(FoundryCampaignError),
    ParallelExecution(ParallelExecutionError),
    ProgressAggregation {
        message: String,
    },
    LedgerSeal(ExactOwnerLedgerSealError),
    WavePublication(StagedSectorClosureError),
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    Invariant {
        detail: &'static str,
    },
}

impl fmt::Display for K6WaveCampaignError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Campaign(error) => error.fmt(formatter),
            Self::ParallelExecution(error) => error.fmt(formatter),
            Self::ProgressAggregation { message } => {
                write!(
                    formatter,
                    "could not start the K6 progress aggregator: {message}"
                )
            }
            Self::LedgerSeal(error) => error.fmt(formatter),
            Self::WavePublication(error) => error.fmt(formatter),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} entries for {resource}"
            ),
            Self::Invariant { detail } => write!(formatter, "K6 wave invariant failed: {detail}"),
        }
    }
}

impl std::error::Error for K6WaveCampaignError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Campaign(error) => Some(error),
            Self::ParallelExecution(error) => Some(error),
            Self::LedgerSeal(error) => Some(error),
            Self::WavePublication(error) => Some(error),
            Self::ProgressAggregation { .. }
            | Self::AllocationFailure { .. }
            | Self::Invariant { .. } => None,
        }
    }
}

impl From<FoundryCampaignError> for K6WaveCampaignError {
    fn from(value: FoundryCampaignError) -> Self {
        Self::Campaign(value)
    }
}

impl From<ParallelExecutionError> for K6WaveCampaignError {
    fn from(value: ParallelExecutionError) -> Self {
        Self::ParallelExecution(value)
    }
}

impl From<ExactOwnerLedgerSealError> for K6WaveCampaignError {
    fn from(value: ExactOwnerLedgerSealError) -> Self {
        Self::LedgerSeal(value)
    }
}

impl From<StagedSectorClosureError> for K6WaveCampaignError {
    fn from(value: StagedSectorClosureError) -> Self {
        Self::WavePublication(value)
    }
}

impl From<K6WaveCampaignError> for K6WaveCampaignRunError {
    fn from(value: K6WaveCampaignError) -> Self {
        Self(value)
    }
}

/// Run all full-rank K6 sectors in deterministic same-rank waves.
///
/// `sibling_worker_count` controls only independent siblings within a wave.
/// Every sibling reads the same immutable predecessor, results are joined in
/// canonical orbit order, and publication remains atomic. The returned typed
/// outcome keeps every published wave (and any incomplete live ledger) alive;
/// its public progress values are detached diagnostics.
pub fn run_k6_full_rank_wave_campaign(
    config: &FoundryCampaignConfig,
    sibling_worker_count: usize,
) -> Result<K6WaveCampaignOutcome, K6WaveCampaignRunError> {
    run_k6_full_rank_wave_campaign_with_progress(config, sibling_worker_count, |_| {})
}

/// Run all full-rank K6 waves while observing bounded latest-value progress.
///
/// The callback always runs on the invoking coordinator thread. Sibling
/// solvers publish only into one fixed atomic slot per orbit plus a one-entry
/// wake channel, so a slow callback coalesces intermediate revisions without
/// blocking a solver or growing an event queue. Every detached snapshot is in
/// canonical orbit order and carries its wave, orbit, and ledger-revision
/// tags; it contains no proof or publication authority.
pub fn run_k6_full_rank_wave_campaign_with_progress(
    config: &FoundryCampaignConfig,
    sibling_worker_count: usize,
    mut observe: impl FnMut(K6WaveCampaignProgress),
) -> Result<K6WaveCampaignOutcome, K6WaveCampaignRunError> {
    let resolved = config
        .try_resolve_search_program()
        .map_err(K6WaveCampaignError::from)?;
    let config = &resolved;
    if config.itinerary() != FoundryCampaignItinerary::FullRankAtomicWaves {
        return Err(K6WaveCampaignError::Invariant {
            detail: "full-rank-wave runner received a single-sector itinerary",
        }
        .into());
    }
    let root =
        k6_root_predecessor_for_ordering(config.ordering()).map_err(K6WaveCampaignError::from)?;
    try_run_k6_full_rank_waves_with_progress_against_root(
        config,
        root.clone(),
        &root,
        StagedSectorClosureLimits::default(),
        sibling_worker_count,
        &mut observe,
    )
    .map_err(Into::into)
}

/// Run the complete full-rank K6 frontier in four bottom-up same-rank waves.
///
/// The task/probe bounds in `config` apply independently to every sector. A
/// wave drives all of its siblings for deterministic diagnostics, but never
/// publishes a proper subset. Only four successive transactional publications
/// can yield `Published`; this function never builds or persists a final
/// `ClosedArtifact`.
#[cfg(test)]
pub(crate) fn try_run_k6_full_rank_waves(
    config: &FoundryCampaignConfig,
    initial_predecessor: ImmutableOwnerSnapshot,
    publication_limits: StagedSectorClosureLimits,
    sibling_worker_count: usize,
) -> Result<K6WaveCampaignOutcome, K6WaveCampaignError> {
    try_run_k6_full_rank_waves_with_progress(
        config,
        initial_predecessor,
        publication_limits,
        sibling_worker_count,
        &mut |_| {},
    )
}

#[cfg(test)]
fn try_run_k6_full_rank_waves_with_progress(
    config: &FoundryCampaignConfig,
    initial_predecessor: ImmutableOwnerSnapshot,
    publication_limits: StagedSectorClosureLimits,
    sibling_worker_count: usize,
    observe: &mut impl FnMut(K6WaveCampaignProgress),
) -> Result<K6WaveCampaignOutcome, K6WaveCampaignError> {
    let required_root = k6_root_predecessor_for_ordering(config.ordering())?;
    try_run_k6_full_rank_waves_with_progress_against_root(
        config,
        initial_predecessor,
        &required_root,
        publication_limits,
        sibling_worker_count,
        observe,
    )
}

fn try_run_k6_full_rank_waves_with_progress_against_root(
    config: &FoundryCampaignConfig,
    initial_predecessor: ImmutableOwnerSnapshot,
    required_root: &ImmutableOwnerSnapshot,
    publication_limits: StagedSectorClosureLimits,
    sibling_worker_count: usize,
    observe: &mut impl FnMut(K6WaveCampaignProgress),
) -> Result<K6WaveCampaignOutcome, K6WaveCampaignError> {
    validate_wave_manifest()?;
    let maximum_wave_width =
        K6_FULL_RANK_WAVE_WIDTHS
            .iter()
            .copied()
            .max()
            .ok_or(K6WaveCampaignError::Invariant {
                detail: "K6 wave manifest is empty",
            })?;
    // One private pool is reused across all four waves. Its admitted result
    // ceiling is the manifest width, so no worker starts before the exact
    // orbit-ordered collection buffer has been reserved.
    let execution = ParallelExecution::try_new(sibling_worker_count, maximum_wave_width)?;
    if !initial_predecessor.same_authority_as(required_root) {
        return Err(K6WaveCampaignError::Invariant {
            detail: "K6 full-rank waves did not start from the exact installed root authority",
        });
    }
    let inputs = shared_k6_algebra_inputs()?;
    let resource_profile = K6CampaignResourceProfile::try_for_task_report_ceiling(
        config.max_task_reports(),
    )
    .map_err(|error| FoundryCampaignError::setup(FoundryCampaignSetupStage::Ledger, error))?;
    let publication_limits = resource_profile
        .try_raise_publication_limits(publication_limits, maximum_wave_width)
        .map_err(|error| FoundryCampaignError::setup(FoundryCampaignSetupStage::Ledger, error))?;
    let campaign_limits = resource_profile.probe_campaign_limits();
    let coordinator_config = try_build_coordinator_config(config, campaign_limits)?;
    let mut predecessor = initial_predecessor;
    let mut published = Vec::new();
    published
        .try_reserve_exact(K6_FULL_RANK_WAVE_WIDTHS.len())
        .map_err(|_| K6WaveCampaignError::AllocationFailure {
            resource: "published K6 sector waves",
            requested: K6_FULL_RANK_WAVE_WIDTHS.len(),
        })?;

    let mut orbit_start = 0usize;
    let mut progress = Vec::new();
    progress
        .try_reserve_exact(K6_FULL_RANK_WAVE_WIDTHS.len())
        .map_err(|_| K6WaveCampaignError::AllocationFailure {
            resource: "K6 wave progress",
            requested: K6_FULL_RANK_WAVE_WIDTHS.len(),
        })?;
    for (wave_ordinal, &wave_width) in K6_FULL_RANK_WAVE_WIDTHS.iter().enumerate() {
        let wave_predecessor = predecessor.clone();
        let active_count = FULL_RANK_ORBITS[orbit_start]
            .representative
            .iter()
            .filter(|&&power| power > 0)
            .count();
        let mut closed = Vec::new();
        closed.try_reserve_exact(wave_width).map_err(|_| {
            K6WaveCampaignError::AllocationFailure {
                resource: "closed K6 sibling covers",
                requested: wave_width,
            }
        })?;
        let mut stops = Vec::new();
        stops.try_reserve_exact(wave_width).map_err(|_| {
            K6WaveCampaignError::AllocationFailure {
                resource: "incomplete K6 sibling stops",
                requested: wave_width,
            }
        })?;

        // Each operation builds and owns exactly one mutable ledger. Workers
        // update only fixed latest-value slots; the invoking coordinator
        // drains and detaches canonically ordered snapshots while the ordered
        // map runs on a scoped execution thread.
        let (sibling_results, mut live_progress) = try_drive_k6_wave_with_progress(
            &execution,
            inputs,
            wave_ordinal,
            active_count,
            orbit_start,
            wave_width,
            &wave_predecessor,
            resource_profile,
            campaign_limits,
            config.ordering(),
            &coordinator_config,
            config,
            observe,
        )?;
        for result in sibling_results {
            match result? {
                K6DrivenSibling::Closed(cover) => closed.push(cover),
                K6DrivenSibling::Stopped(stop) => stops.push(stop),
            }
        }
        attach_stopped_sibling_diagnostics(&mut live_progress, &stops)?;

        if !stops.is_empty() {
            let mut incomplete_orbits = Vec::new();
            incomplete_orbits
                .try_reserve_exact(stops.len())
                .map_err(|_| K6WaveCampaignError::AllocationFailure {
                    resource: "detached incomplete K6 orbit reports",
                    requested: stops.len(),
                })?;
            for stop in &stops {
                incomplete_orbits.push(K6IncompleteOrbitReport {
                    orbit_ordinal: stop._orbit_ordinal,
                    report: detach_report(
                        config,
                        stop._retained.ledger(),
                        stop._retained.terminal_stop(),
                        stop._retained.final_census(),
                    )?,
                });
            }
            let final_progress =
                finalize_wave_progress(live_progress, K6WaveCampaignState::Incomplete)?;
            observe(final_progress.clone());
            progress.push(final_progress);
            return Ok(K6WaveCampaignOutcome::Incomplete(K6IncompleteSectorWave {
                wave_ordinal,
                active_count,
                _predecessor: wave_predecessor,
                published_waves: published.into_boxed_slice(),
                closed_sector_count: closed.len(),
                _stops: stops.into_boxed_slice(),
                incomplete_orbits: incomplete_orbits.into_boxed_slice(),
                progress: progress.into_boxed_slice(),
            }));
        }
        if closed.len() != wave_width {
            return Err(K6WaveCampaignError::Invariant {
                detail: "closed K6 sibling count differs from the wave width",
            });
        }
        let wave =
            try_publish_sealed_sector_wave(wave_predecessor.clone(), closed, publication_limits)?;
        if !wave.predecessor().same_authority_as(&wave_predecessor)
            || wave.layers().len() != wave_width
        {
            return Err(K6WaveCampaignError::Invariant {
                detail: "published K6 wave differs from its retained predecessor or width",
            });
        }
        predecessor = wave.successor().clone();
        published.push(wave);
        let final_progress = finalize_wave_progress(live_progress, K6WaveCampaignState::Published)?;
        observe(final_progress.clone());
        progress.push(final_progress);
        orbit_start += wave_width;
    }
    if orbit_start != FULL_RANK_ORBITS.len() {
        return Err(K6WaveCampaignError::Invariant {
            detail: "K6 wave widths do not consume the full-rank orbit manifest",
        });
    }
    Ok(K6WaveCampaignOutcome::Published(K6PublishedSectorWaves {
        waves: published.into_boxed_slice(),
        progress: progress.into_boxed_slice(),
    }))
}

fn attach_stopped_sibling_diagnostics(
    progress: &mut K6WaveCampaignProgress,
    stops: &[K6SectorCampaignStop],
) -> Result<(), K6WaveCampaignError> {
    for stop in stops {
        let orbit = progress
            .orbits
            .iter_mut()
            .find(|orbit| orbit.orbit_ordinal() == stop._orbit_ordinal)
            .ok_or(K6WaveCampaignError::Invariant {
                detail: "stopped K6 sibling is absent from final wave progress",
            })?;
        let terminal_stop = stop._retained.terminal_stop();
        let first = stop
            ._retained
            .final_census()
            .first_scheduler_rejection()
            .map(detach_scheduler_rejection);
        let terminal = match terminal_stop {
            ProbeCoordinatorStop::OperationallyBounded(bounded) => match bounded.reason() {
                crate::foundry::completion::source_discovery::ProbeCoordinatorOperationalReason::IncompleteProbeExecution {
                    terminal_scheduler_rejection,
                    ..
                } => terminal_scheduler_rejection.map(detach_scheduler_rejection),
                crate::foundry::completion::source_discovery::ProbeCoordinatorOperationalReason::EpochLimit { .. }
                | crate::foundry::completion::source_discovery::ProbeCoordinatorOperationalReason::PlanLimit { .. }
                | crate::foundry::completion::source_discovery::ProbeCoordinatorOperationalReason::TaskReportLimit { .. } => None,
            },
            ProbeCoordinatorStop::NeedsRefinement(_)
            | ProbeCoordinatorStop::ExhaustedAtConfig { .. } => None,
            ProbeCoordinatorStop::CompilerClosed { .. }
            | ProbeCoordinatorStop::OwnerSetChanged(_)
            | ProbeCoordinatorStop::Failed(_) => {
                return Err(K6WaveCampaignError::Invariant {
                    detail: "stopped K6 sibling retained a non-stopping coordinator state",
                });
            }
        };
        orbit.attach_scheduler_rejections(first, terminal);
    }
    Ok(())
}

/// One worker-local result. Neither variant can publish by itself; the wave
/// coordinator consumes the complete orbit-ordered vector first.
enum K6DrivenSibling {
    Closed(ClosedExactExecutableOwnerCover),
    Stopped(K6SectorCampaignStop),
}

#[allow(clippy::too_many_arguments)]
fn try_drive_k6_wave_with_progress(
    execution: &ParallelExecution,
    inputs: &K6AlgebraInputs,
    wave_ordinal: usize,
    active_count: usize,
    orbit_start: usize,
    wave_width: usize,
    predecessor: &ImmutableOwnerSnapshot,
    resource_profile: K6CampaignResourceProfile,
    campaign_limits: ProbeCampaignLimits,
    ordering: OrderingPolicy,
    coordinator_config: &ProbeCoordinatorConfig,
    campaign_config: &FoundryCampaignConfig,
    observe: &mut impl FnMut(K6WaveCampaignProgress),
) -> Result<
    (
        Vec<Result<K6DrivenSibling, K6WaveCampaignError>>,
        K6WaveCampaignProgress,
    ),
    K6WaveCampaignError,
> {
    let (latest, receiver) =
        LatestK6WaveProgress::try_new(wave_ordinal, active_count, orbit_start, wave_width)?;
    let finished = AtomicBool::new(false);
    let mut last_emitted = None;

    let results = std::thread::scope(|scope| {
        let runner = std::thread::Builder::new()
            .name(format!("rustred-k6-wave-{wave_ordinal}"))
            .spawn_scoped(scope, || {
                let _finished = K6ProgressExecutionGuard {
                    finished: &finished,
                    wake: &latest.wake,
                };
                execution.map_ordered(wave_width, |local_ordinal| {
                    try_drive_k6_sibling(
                        inputs,
                        orbit_start + local_ordinal,
                        local_ordinal,
                        predecessor,
                        resource_profile,
                        campaign_limits,
                        ordering,
                        coordinator_config,
                        campaign_config,
                        &latest,
                    )
                })
            })
            .map_err(|error| K6WaveCampaignError::ProgressAggregation {
                message: error.to_string(),
            })?;

        loop {
            emit_latest_wave_progress(&latest, observe, &mut last_emitted)?;
            if finished.load(AtomicOrdering::Acquire) {
                break;
            }
            match receiver.recv_timeout(K6_PROGRESS_POLL_INTERVAL) {
                Ok(()) | Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(K6WaveCampaignError::Invariant {
                        detail: "K6 progress wake channel disconnected before execution finished",
                    });
                }
            }
        }
        while let Ok(()) = receiver.try_recv() {}
        emit_latest_wave_progress(&latest, observe, &mut last_emitted)?;

        match runner.join() {
            Ok(results) => results.map_err(Into::into),
            Err(payload) => std::panic::resume_unwind(payload),
        }
    })?;
    let final_live = latest.try_snapshot(K6WaveCampaignState::Running)?;
    Ok((results, final_live))
}

fn emit_latest_wave_progress(
    latest: &LatestK6WaveProgress,
    observe: &mut impl FnMut(K6WaveCampaignProgress),
    last_emitted: &mut Option<K6WaveCampaignProgress>,
) -> Result<(), K6WaveCampaignError> {
    let snapshot = latest.try_snapshot(K6WaveCampaignState::Running)?;
    if last_emitted.as_ref() != Some(&snapshot) {
        observe(snapshot.clone());
        *last_emitted = Some(snapshot);
    }
    Ok(())
}

fn finalize_wave_progress(
    mut progress: K6WaveCampaignProgress,
    state: K6WaveCampaignState,
) -> Result<K6WaveCampaignProgress, K6WaveCampaignError> {
    if progress.state != K6WaveCampaignState::Running {
        return Err(K6WaveCampaignError::Invariant {
            detail: "K6 wave progress was finalized more than once",
        });
    }
    match state {
        K6WaveCampaignState::Running => {
            return Err(K6WaveCampaignError::Invariant {
                detail: "K6 final progress cannot retain the running state",
            });
        }
        K6WaveCampaignState::Published => {
            if progress
                .orbits
                .iter()
                .any(|orbit| orbit.state() != K6OrbitCampaignState::ClosedUnpublished)
            {
                return Err(K6WaveCampaignError::Invariant {
                    detail: "published K6 wave progress contains a nonclosed sibling",
                });
            }
            for orbit in &mut progress.orbits {
                orbit.mark_published();
            }
        }
        K6WaveCampaignState::Incomplete => {
            if progress.orbits.iter().any(|orbit| {
                matches!(
                    orbit.state(),
                    K6OrbitCampaignState::Pending
                        | K6OrbitCampaignState::Running
                        | K6OrbitCampaignState::Published
                )
            }) || progress
                .orbits
                .iter()
                .all(|orbit| orbit.state() == K6OrbitCampaignState::ClosedUnpublished)
            {
                return Err(K6WaveCampaignError::Invariant {
                    detail: "incomplete K6 wave progress has no terminal blocking sibling",
                });
            }
        }
    }
    progress.state = state;
    Ok(progress)
}

fn try_drive_k6_sibling(
    inputs: &K6AlgebraInputs,
    orbit_ordinal: usize,
    local_ordinal: usize,
    predecessor: &ImmutableOwnerSnapshot,
    resource_profile: K6CampaignResourceProfile,
    campaign_limits: ProbeCampaignLimits,
    ordering: OrderingPolicy,
    coordinator_config: &ProbeCoordinatorConfig,
    campaign_config: &FoundryCampaignConfig,
    progress: &LatestK6WaveProgress,
) -> Result<K6DrivenSibling, K6WaveCampaignError> {
    let orbit = FULL_RANK_ORBITS
        .get(orbit_ordinal)
        .ok_or(K6WaveCampaignError::Invariant {
            detail: "K6 sibling orbit ordinal exceeds the full-rank manifest",
        })?;
    let ledger = try_new_k6_full_rank_ledger_with_profile_and_ordering(
        inputs,
        orbit.representative,
        predecessor.clone(),
        ordering,
        resource_profile,
        campaign_limits,
    )?;
    if !ledger.predecessor_snapshot().same_authority_as(predecessor) {
        return Err(K6WaveCampaignError::Invariant {
            detail: "same-rank sibling ledger used a different predecessor authority",
        });
    }
    progress.publish_scalars(
        local_ordinal,
        K6OrbitProgressScalars {
            state: K6OrbitCampaignState::Running,
            ledger_revision: ledger.revision().get(),
            owner_count: ledger.snapshot().owner_count(),
            uncovered_box_count: ledger.snapshot().uncovered_box_count(),
            task_reports: 0,
        },
    );
    let adapter = ProbeCampaignAdapter::try_new(
        inputs.generator(),
        inputs.completed(),
        inputs.zero_sources(),
        campaign_limits,
    )
    .map_err(|error| FoundryCampaignError::setup(FoundryCampaignSetupStage::Coordinator, error))?;
    let retained = try_drive_live_ledger_until_terminal_with_progress(
        coordinator_config.clone(),
        campaign_config,
        adapter,
        ledger,
        |exact, census, _| {
            progress.publish_snapshot(local_ordinal, K6OrbitCampaignState::Running, exact, census);
        },
    )?;
    let terminal_state = match retained.terminal_stop() {
        ProbeCoordinatorStop::CompilerClosed { .. } => K6OrbitCampaignState::ClosedUnpublished,
        ProbeCoordinatorStop::NeedsRefinement(_) => K6OrbitCampaignState::NeedsRefinement,
        ProbeCoordinatorStop::OperationallyBounded(_) => K6OrbitCampaignState::OperationallyBounded,
        ProbeCoordinatorStop::ExhaustedAtConfig { .. } => K6OrbitCampaignState::ExhaustedAtConfig,
        ProbeCoordinatorStop::OwnerSetChanged(_) | ProbeCoordinatorStop::Failed(_) => {
            return Err(K6WaveCampaignError::Invariant {
                detail: "nonterminal coordinator stop escaped the live-ledger driver",
            });
        }
    };
    progress.publish_snapshot(
        local_ordinal,
        terminal_state,
        retained.ledger().snapshot(),
        retained.final_census(),
    );
    match retained.terminal_stop() {
        ProbeCoordinatorStop::CompilerClosed { .. } => {
            let (ledger, _) = retained.into_parts();
            Ok(K6DrivenSibling::Closed(ledger.try_into_closed_cover()?))
        }
        ProbeCoordinatorStop::NeedsRefinement(_)
        | ProbeCoordinatorStop::OperationallyBounded(_)
        | ProbeCoordinatorStop::ExhaustedAtConfig { .. } => {
            Ok(K6DrivenSibling::Stopped(K6SectorCampaignStop {
                _orbit_ordinal: orbit_ordinal,
                _retained: retained,
            }))
        }
        ProbeCoordinatorStop::OwnerSetChanged(_) | ProbeCoordinatorStop::Failed(_) => {
            unreachable!("terminal state mapping rejected nonterminal coordinator stops")
        }
    }
}

#[cfg(test)]
fn try_build_wave_ledgers(
    inputs: &K6AlgebraInputs,
    orbit_start: usize,
    wave_width: usize,
    predecessor: &ImmutableOwnerSnapshot,
    resource_profile: K6CampaignResourceProfile,
) -> Result<Vec<CanonicalExactOwnerLedger>, K6WaveCampaignError> {
    let orbit_end = orbit_start
        .checked_add(wave_width)
        .ok_or(K6WaveCampaignError::Invariant {
            detail: "K6 wave orbit range overflowed",
        })?;
    let orbits =
        FULL_RANK_ORBITS
            .get(orbit_start..orbit_end)
            .ok_or(K6WaveCampaignError::Invariant {
                detail: "K6 wave orbit range exceeds the manifest",
            })?;
    let mut ledgers = Vec::new();
    ledgers
        .try_reserve_exact(wave_width)
        .map_err(|_| K6WaveCampaignError::AllocationFailure {
            resource: "fresh K6 sibling ledgers",
            requested: wave_width,
        })?;
    for orbit in orbits {
        ledgers.push(try_new_k6_full_rank_ledger_with_profile_and_ordering(
            inputs,
            orbit.representative,
            predecessor.clone(),
            OrderingPolicy::default(),
            resource_profile,
            ProbeCampaignLimits::default(),
        )?);
    }
    Ok(ledgers)
}

fn validate_wave_manifest() -> Result<(), K6WaveCampaignError> {
    let mut orbit_start = 0usize;
    for (wave_ordinal, &wave_width) in K6_FULL_RANK_WAVE_WIDTHS.iter().enumerate() {
        let orbit_end =
            orbit_start
                .checked_add(wave_width)
                .ok_or(K6WaveCampaignError::Invariant {
                    detail: "K6 wave width sum overflowed",
                })?;
        let orbits =
            FULL_RANK_ORBITS
                .get(orbit_start..orbit_end)
                .ok_or(K6WaveCampaignError::Invariant {
                    detail: "K6 wave width exceeds the full-rank orbit manifest",
                })?;
        let expected_active_count = wave_ordinal + 3;
        if orbits.iter().any(|orbit| {
            orbit
                .representative
                .iter()
                .filter(|&&power| power > 0)
                .count()
                != expected_active_count
        }) {
            return Err(K6WaveCampaignError::Invariant {
                detail: "K6 full-rank orbit manifest violates the [2,2,1,1] rank waves",
            });
        }
        orbit_start = orbit_end;
    }
    if orbit_start != FULL_RANK_ORBITS.len() {
        return Err(K6WaveCampaignError::Invariant {
            detail: "K6 wave widths do not cover the full-rank orbit manifest",
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests;
