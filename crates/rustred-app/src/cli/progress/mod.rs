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
};

use super::args::ColorPolicy;
use model::{
    CampaignPhase, DashboardState, RateSample, defensible_cap_eta, error_count,
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
        let Some(shared) = &self.shared else {
            return;
        };
        shared.publish_progress(ProgressUpdate::from_progress(&progress));
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
            stop,
            progress: ProgressUpdate::from_parts(
                snapshot,
                census,
                stop_location(stop),
                maximum_dimension,
                task_report_ceiling,
            ),
        }));
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
    coverage: FoundryCampaignCoverageStatus,
    revision: u64,
    owner_count: usize,
    uncovered_box_count: usize,
    census: FoundryCampaignCensus,
    location: Option<FoundryCampaignTaskLocation>,
    maximum_dimension: usize,
    task_report_ceiling: usize,
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
        Self {
            coverage: snapshot.coverage(),
            revision: snapshot.revision(),
            owner_count: snapshot.owner_count(),
            uncovered_box_count: snapshot.uncovered_box_count(),
            census,
            location,
            maximum_dimension,
            task_report_ceiling,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct FinalUpdate {
    stop: FoundryCampaignStop,
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
                self.state.phase = phase_for_stop(final_update.stop);
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
        self.state.phase = if progress.coverage == FoundryCampaignCoverageStatus::Closed {
            CampaignPhase::Closing
        } else {
            CampaignPhase::Discovering
        };
        self.state.revision = progress.revision;
        self.state.owner_count = progress.owner_count;
        self.state.task_report_ceiling = Some(progress.task_report_ceiling);
        self.state.uncovered_box_count = Some(progress.uncovered_box_count);
        self.state.effective_dimension = progress.location.map(|value| value.effective_dimension());
        self.state.maximum_dimension = Some(progress.maximum_dimension);
        self.state.strict_shrink = progress.census.strict_geometric_shrink();
        self.state.no_proposal = progress.census.no_proposal();
        self.state.duplicate = progress.census.duplicate();
        self.state.errors = error_count(progress.census);
        self.state.task_reports = progress.census.task_reports();
        self.state.owner_rate = self.smoothed_owner_rate;
        self.state.cap_eta = defensible_cap_eta(
            progress.owner_count,
            progress.task_report_ceiling,
            owner_progress_is_one_to_one(progress.owner_count, progress.census),
            self.smoothed_owner_rate,
        );
    }

    fn update_rate(&mut self, now: Instant, owners: usize) {
        let interval = now.saturating_duration_since(self.rate_sample.at);
        if interval < RATE_SAMPLE_INTERVAL || owners < self.rate_sample.owners {
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
