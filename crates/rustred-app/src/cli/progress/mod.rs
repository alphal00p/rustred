mod model;
mod render;
mod terminal;

#[cfg(test)]
mod tests;

use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use rustred::foundry::campaign::{
    FoundryCampaignCensus, FoundryCampaignCoverageStatus, FoundryCampaignProgress,
    FoundryCampaignSnapshot, FoundryCampaignStop, FoundryCampaignTaskLocation,
    K6OrbitCampaignState, K6WaveCampaignProgress, K6WaveCampaignState,
};

use super::args::ColorPolicy;
use model::{
    CampaignPhase, DashboardState, RateSample, WaveDashboardState, defensible_cap_eta, error_count,
    owner_progress_is_one_to_one, phase_for_stop, stop_location,
};
use terminal::{TerminalSession, resident_set_bytes};

const REFRESH_INTERVAL: Duration = Duration::from_millis(100);
const RATE_SAMPLE_INTERVAL: Duration = Duration::from_millis(250);
const RSS_SAMPLE_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ProgressPresentation {
    enabled: bool,
    color: bool,
}

impl ProgressPresentation {
    pub(crate) const fn resolve(
        no_progress: bool,
        color_policy: ColorPolicy,
        stderr_is_terminal: bool,
        no_color_is_set: bool,
    ) -> Self {
        Self {
            enabled: stderr_is_terminal && !no_progress,
            color: match color_policy {
                ColorPolicy::Auto => stderr_is_terminal && !no_color_is_set,
                ColorPolicy::Always => true,
                ColorPolicy::Never => false,
            },
        }
    }
}

/// Interactive stderr presentation for one foundry campaign.
///
/// The exact callback only replaces one bounded scalar slot. A private
/// presenter thread owns clocks, RSS sampling, table allocation, terminal I/O,
/// resize-aware redraw and cleanup. Presentation failures are nonsemantic and
/// therefore never suppress the campaign's report or measurement sidecar.
pub(crate) struct CampaignProgressMonitor<W: Write + Send + 'static> {
    writer: Option<W>,
    presentation: ProgressPresentation,
    shared: Option<Arc<PresenterShared>>,
    presenter: Option<JoinHandle<()>>,
    refresh_interval: Duration,
    viewport: PresenterViewport,
    last_wave: Option<(ProgressUpdate, CampaignPhase)>,
}

#[derive(Clone, Copy)]
enum PresenterViewport {
    Inline,
    #[cfg(test)]
    Fixed(u16),
}

impl<W: Write + Send + 'static> CampaignProgressMonitor<W> {
    pub(crate) fn new(writer: W, presentation: ProgressPresentation) -> Self {
        Self::with_refresh_interval(writer, presentation, REFRESH_INTERVAL)
    }

    fn with_refresh_interval(
        writer: W,
        presentation: ProgressPresentation,
        refresh_interval: Duration,
    ) -> Self {
        Self {
            writer: Some(writer),
            presentation,
            shared: None,
            presenter: None,
            refresh_interval,
            viewport: PresenterViewport::Inline,
            last_wave: None,
        }
    }

    #[cfg(test)]
    pub(super) fn with_test_refresh_interval(
        writer: W,
        presentation: ProgressPresentation,
        refresh_interval: Duration,
    ) -> Self {
        let mut monitor = Self::with_refresh_interval(writer, presentation, refresh_interval);
        monitor.viewport = PresenterViewport::Fixed(80);
        monitor
    }

    #[cfg(test)]
    pub(super) fn with_test_viewport(
        writer: W,
        presentation: ProgressPresentation,
        refresh_interval: Duration,
        width: u16,
    ) -> Self {
        let mut monitor = Self::with_refresh_interval(writer, presentation, refresh_interval);
        monitor.viewport = PresenterViewport::Fixed(width);
        monitor
    }

    pub(crate) fn start(&mut self) {
        if !self.presentation.enabled || self.presenter.is_some() {
            return;
        }
        let Some(writer) = self.writer.take() else {
            return;
        };
        let shared = Arc::new(PresenterShared::new());
        let presenter_shared = Arc::clone(&shared);
        let presentation = self.presentation;
        let refresh_interval = self.refresh_interval;
        let viewport = self.viewport;
        match thread::Builder::new()
            .name("rustred-campaign-progress".to_owned())
            .spawn(move || {
                let terminal = match viewport {
                    PresenterViewport::Inline => TerminalSession::try_new(writer),
                    #[cfg(test)]
                    PresenterViewport::Fixed(width) => {
                        TerminalSession::try_new_fixed(writer, width)
                    }
                };
                let Ok(terminal) = terminal else {
                    presenter_shared.active.store(false, Ordering::Release);
                    return;
                };
                Presenter::new(terminal, presentation, refresh_interval).run(&presenter_shared);
            }) {
            Ok(presenter) => {
                self.shared = Some(shared);
                self.presenter = Some(presenter);
            }
            Err(_) => {
                // The dashboard is optional. Thread construction must not
                // change the exact campaign or suppress durable output.
                self.presentation.enabled = false;
            }
        }
    }

    pub(crate) fn observe(&mut self, progress: FoundryCampaignProgress) {
        self.publish(ProgressUpdate::from_progress(&progress));
    }

    pub(crate) fn observe_wave(&mut self, progress: K6WaveCampaignProgress) {
        let update = ProgressUpdate::from_wave(&progress);
        self.last_wave = Some((update, terminal_phase_for_wave(&progress)));
        self.publish(update);
    }

    fn publish(&self, update: ProgressUpdate) {
        let Some(shared) = &self.shared else {
            return;
        };
        shared.publish_progress(update);
    }

    pub(crate) fn finish(
        &mut self,
        stop: FoundryCampaignStop,
        snapshot: &FoundryCampaignSnapshot,
        census: FoundryCampaignCensus,
        maximum_dimension: usize,
        task_report_ceiling: usize,
    ) {
        self.stop_presenter(PresenterTerminal::Finished(FinalUpdate {
            phase: phase_for_stop(stop),
            progress: ProgressUpdate::from_parts(
                snapshot,
                census,
                stop_location(stop),
                maximum_dimension,
                task_report_ceiling,
            ),
        }));
    }

    pub(crate) fn finish_wave(&mut self) {
        let Some((progress, phase)) = self.last_wave.take() else {
            // A valid K6 wave run always publishes at least one terminal
            // callback. Keep this defensive presentation path nonsemantic.
            self.stop_presenter(PresenterTerminal::Shutdown);
            return;
        };
        self.stop_presenter(PresenterTerminal::Finished(FinalUpdate { phase, progress }));
    }

    pub(crate) fn finish_failed(&mut self) {
        self.stop_presenter(PresenterTerminal::Failed);
    }

    fn stop_presenter(&mut self, terminal: PresenterTerminal) {
        if let Some(shared) = self.shared.take() {
            shared.publish_terminal(terminal);
        }
        if let Some(presenter) = self.presenter.take() {
            // This join occurs only after the exact campaign stopped. Terminal
            // latency can therefore never enter the mathematical hot path.
            let _ = presenter.join();
        }
    }
}

impl<W: Write + Send + 'static> Drop for CampaignProgressMonitor<W> {
    fn drop(&mut self) {
        self.stop_presenter(PresenterTerminal::Shutdown);
    }
}

#[derive(Clone, Copy, Debug)]
struct ProgressUpdate {
    phase: CampaignPhase,
    revision: u64,
    owner_count: usize,
    uncovered_box_count: usize,
    effective_dimension: Option<usize>,
    maximum_dimension: usize,
    task_report_ceiling: Option<usize>,
    strict_shrink: usize,
    no_proposal: usize,
    duplicate: usize,
    errors: usize,
    task_reports: usize,
    owner_progress_is_one_to_one: bool,
    wave: Option<WaveDashboardState>,
}

impl ProgressUpdate {
    fn from_progress(progress: &FoundryCampaignProgress) -> Self {
        Self::from_parts(
            progress.snapshot(),
            progress.census(),
            progress.location(),
            progress.maximum_dimension(),
            progress.task_report_ceiling(),
        )
    }

    fn from_parts(
        snapshot: &FoundryCampaignSnapshot,
        census: FoundryCampaignCensus,
        location: Option<FoundryCampaignTaskLocation>,
        maximum_dimension: usize,
        task_report_ceiling: usize,
    ) -> Self {
        let one_to_one = owner_progress_is_one_to_one(snapshot.owner_count(), census);
        Self {
            phase: if snapshot.coverage() == FoundryCampaignCoverageStatus::Closed {
                CampaignPhase::Closing
            } else {
                CampaignPhase::Discovering
            },
            revision: snapshot.revision(),
            owner_count: snapshot.owner_count(),
            uncovered_box_count: snapshot.uncovered_box_count(),
            effective_dimension: location.map(|value| value.effective_dimension()),
            maximum_dimension,
            task_report_ceiling: Some(task_report_ceiling),
            strict_shrink: census.strict_geometric_shrink(),
            no_proposal: census.no_proposal(),
            duplicate: census.duplicate(),
            errors: error_count(census),
            task_reports: census.task_reports(),
            owner_progress_is_one_to_one: one_to_one,
            wave: None,
        }
    }

    fn from_wave(progress: &K6WaveCampaignProgress) -> Self {
        let mut revision = 0_u64;
        let mut owner_count = 0_usize;
        let mut uncovered_box_count = 0_usize;
        let mut task_reports = 0_usize;
        let mut closed_orbit_count = 0_usize;
        let mut running_orbit_count = 0_usize;
        let mut terminal_orbit_count = 0_usize;
        for orbit in progress.orbits() {
            revision = revision.max(orbit.ledger_revision());
            owner_count = owner_count.saturating_add(orbit.owner_count());
            uncovered_box_count = uncovered_box_count.saturating_add(orbit.uncovered_box_count());
            task_reports = task_reports.saturating_add(orbit.task_reports());
            match orbit.state() {
                K6OrbitCampaignState::Published | K6OrbitCampaignState::ClosedUnpublished => {
                    closed_orbit_count = closed_orbit_count.saturating_add(1);
                }
                K6OrbitCampaignState::Running => {
                    running_orbit_count = running_orbit_count.saturating_add(1);
                }
                K6OrbitCampaignState::NeedsRefinement
                | K6OrbitCampaignState::OperationallyBounded
                | K6OrbitCampaignState::ExhaustedAtConfig => {
                    terminal_orbit_count = terminal_orbit_count.saturating_add(1);
                }
                K6OrbitCampaignState::Pending => {}
            }
        }
        let maximum_dimension = progress
            .orbits()
            .first()
            .map_or(progress.active_count(), |orbit| {
                orbit.representative().len()
            });
        Self {
            phase: live_phase_for_wave(progress),
            revision,
            owner_count,
            uncovered_box_count,
            effective_dimension: Some(progress.active_count()),
            maximum_dimension,
            task_report_ceiling: None,
            strict_shrink: 0,
            no_proposal: 0,
            duplicate: 0,
            errors: 0,
            task_reports,
            owner_progress_is_one_to_one: false,
            wave: Some(WaveDashboardState {
                ordinal: progress.wave_ordinal(),
                active_count: progress.active_count(),
                orbit_count: progress.orbits().len(),
                closed_orbit_count,
                running_orbit_count,
                terminal_orbit_count,
            }),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct FinalUpdate {
    phase: CampaignPhase,
    progress: ProgressUpdate,
}

#[derive(Clone, Copy, Debug)]
enum PresenterTerminal {
    Finished(FinalUpdate),
    Failed,
    Shutdown,
}

#[derive(Default)]
struct PresenterSharedState {
    latest: Option<ProgressUpdate>,
    terminal: Option<PresenterTerminal>,
}

struct PresenterShared {
    state: Mutex<PresenterSharedState>,
    wake: Condvar,
    active: AtomicBool,
}

impl PresenterShared {
    fn new() -> Self {
        Self {
            state: Mutex::new(PresenterSharedState::default()),
            wake: Condvar::new(),
            active: AtomicBool::new(true),
        }
    }

    fn publish_progress(&self, progress: ProgressUpdate) {
        if !self.active.load(Ordering::Acquire) {
            return;
        }
        let Ok(mut state) = self.state.try_lock() else {
            return;
        };
        if state.terminal.is_none() {
            // Exactly one latest-value slot: intermediate presentation frames
            // may be coalesced, semantic core callbacks are not.
            state.latest = Some(progress);
            self.wake.notify_one();
        }
    }

    fn publish_terminal(&self, terminal: PresenterTerminal) {
        if !self.active.load(Ordering::Acquire) {
            return;
        }
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        state.terminal = Some(terminal);
        self.wake.notify_one();
    }
}

struct Presenter<W: Write> {
    terminal: TerminalSession<W>,
    presentation: ProgressPresentation,
    started: Instant,
    last_render: Option<Instant>,
    state: DashboardState,
    rate_sample: RateSample,
    smoothed_owner_rate: Option<f64>,
    last_rss_sample: Option<Instant>,
    refresh_interval: Duration,
}

impl<W: Write> Presenter<W> {
    fn new(
        terminal: TerminalSession<W>,
        presentation: ProgressPresentation,
        refresh_interval: Duration,
    ) -> Self {
        let now = Instant::now();
        Self {
            terminal,
            presentation,
            started: now,
            last_render: None,
            state: DashboardState::default(),
            rate_sample: RateSample { at: now, owners: 0 },
            smoothed_owner_rate: None,
            last_rss_sample: None,
            refresh_interval,
        }
    }

    fn run(mut self, shared: &PresenterShared) {
        let now = Instant::now();
        self.sample_rss(now);
        if self.render_at(now).is_err() {
            shared.active.store(false, Ordering::Release);
            return;
        }

        loop {
            let deadline = self
                .last_render
                .unwrap_or_else(Instant::now)
                .checked_add(self.refresh_interval)
                .unwrap_or_else(Instant::now);
            let (latest, terminal) = match wait_for_update(shared, deadline) {
                Some(update) => update,
                None => break,
            };
            let now = Instant::now();
            if let Some(progress) = latest {
                self.apply_progress(now, progress);
            }
            if let Some(terminal) = terminal {
                self.finish_terminal(now, terminal);
                break;
            }
            if now >= deadline {
                self.sample_rss(now);
                if self.render_at(now).is_err() {
                    break;
                }
            }
        }
        shared.active.store(false, Ordering::Release);
    }

    fn finish_terminal(&mut self, now: Instant, terminal: PresenterTerminal) {
        match terminal {
            PresenterTerminal::Finished(final_update) => {
                self.apply_progress(now, final_update.progress);
                self.state.phase = final_update.phase;
            }
            PresenterTerminal::Failed => {
                self.state.elapsed = now.saturating_duration_since(self.started);
                self.state.phase = CampaignPhase::Failed;
            }
            PresenterTerminal::Shutdown => {
                self.terminal.close();
                return;
            }
        }
        self.sample_rss(now);
        let _ = self.render_at(now);
        self.terminal.close();
    }

    fn apply_progress(&mut self, now: Instant, progress: ProgressUpdate) {
        self.update_rate(now, progress.owner_count);
        self.state.elapsed = now.saturating_duration_since(self.started);
        self.state.phase = progress.phase;
        self.state.revision = progress.revision;
        self.state.owner_count = progress.owner_count;
        self.state.task_report_ceiling = progress.task_report_ceiling;
        self.state.uncovered_box_count = Some(progress.uncovered_box_count);
        self.state.effective_dimension = progress.effective_dimension;
        self.state.maximum_dimension = Some(progress.maximum_dimension);
        self.state.strict_shrink = progress.strict_shrink;
        self.state.no_proposal = progress.no_proposal;
        self.state.duplicate = progress.duplicate;
        self.state.errors = progress.errors;
        self.state.task_reports = progress.task_reports;
        self.state.owner_rate = self.smoothed_owner_rate;
        self.state.cap_eta = defensible_cap_eta(
            progress.owner_count,
            progress.task_report_ceiling.unwrap_or(progress.owner_count),
            progress.task_report_ceiling.is_some() && progress.owner_progress_is_one_to_one,
            self.smoothed_owner_rate,
        );
        self.state.wave = progress.wave;
    }

    fn update_rate(&mut self, now: Instant, owners: usize) {
        let interval = now.saturating_duration_since(self.rate_sample.at);
        if owners < self.rate_sample.owners {
            self.rate_sample = RateSample { at: now, owners };
            self.smoothed_owner_rate = None;
            return;
        }
        if interval < RATE_SAMPLE_INTERVAL {
            return;
        }
        let added = owners - self.rate_sample.owners;
        if added != 0 {
            let rate = added as f64 / interval.as_secs_f64();
            self.smoothed_owner_rate = Some(match self.smoothed_owner_rate {
                None => rate,
                Some(previous) => previous.mul_add(0.75, rate * 0.25),
            });
        }
        self.rate_sample = RateSample { at: now, owners };
    }

    fn sample_rss(&mut self, now: Instant) {
        if self
            .last_rss_sample
            .is_some_and(|last| now.saturating_duration_since(last) < RSS_SAMPLE_INTERVAL)
        {
            return;
        }
        self.state.rss_bytes = resident_set_bytes();
        self.last_rss_sample = Some(now);
    }

    fn render_at(&mut self, now: Instant) -> std::io::Result<()> {
        self.state.elapsed = now.saturating_duration_since(self.started);
        self.terminal.render(&self.state, self.presentation.color)?;
        self.last_render = Some(now);
        Ok(())
    }
}

fn live_phase_for_wave(progress: &K6WaveCampaignProgress) -> CampaignPhase {
    match progress.state() {
        K6WaveCampaignState::Running => {
            if progress
                .orbits()
                .iter()
                .all(|orbit| orbit.state() == K6OrbitCampaignState::Pending)
            {
                CampaignPhase::Starting
            } else {
                CampaignPhase::Discovering
            }
        }
        K6WaveCampaignState::Published => CampaignPhase::Closing,
        K6WaveCampaignState::Incomplete => incomplete_wave_phase(progress),
    }
}

fn terminal_phase_for_wave(progress: &K6WaveCampaignProgress) -> CampaignPhase {
    match progress.state() {
        K6WaveCampaignState::Published => CampaignPhase::Closed,
        K6WaveCampaignState::Incomplete => incomplete_wave_phase(progress),
        K6WaveCampaignState::Running => CampaignPhase::Failed,
    }
}

fn incomplete_wave_phase(progress: &K6WaveCampaignProgress) -> CampaignPhase {
    let mut states = progress.orbits().iter().map(|orbit| orbit.state());
    if states
        .clone()
        .any(|state| state == K6OrbitCampaignState::NeedsRefinement)
    {
        CampaignPhase::Refinement
    } else if states
        .clone()
        .any(|state| state == K6OrbitCampaignState::OperationallyBounded)
    {
        CampaignPhase::Bounded
    } else if states.any(|state| state == K6OrbitCampaignState::ExhaustedAtConfig) {
        CampaignPhase::Exhausted
    } else {
        CampaignPhase::Failed
    }
}

fn wait_for_update(
    shared: &PresenterShared,
    deadline: Instant,
) -> Option<(Option<ProgressUpdate>, Option<PresenterTerminal>)> {
    let mut state = shared.state.lock().ok()?;
    while state.latest.is_none() && state.terminal.is_none() {
        let now = Instant::now();
        if now >= deadline {
            break;
        }
        let timeout = deadline.saturating_duration_since(now);
        let waited = shared.wake.wait_timeout(state, timeout).ok()?;
        state = waited.0;
        if waited.1.timed_out() {
            break;
        }
    }
    Some((state.latest.take(), state.terminal.take()))
}
