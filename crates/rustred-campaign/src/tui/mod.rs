//! Terminal dashboard and read-only views of a campaign (package WP-I7, interface
//! `plan/auto-campaign/interfaces/dashboard.md`).
//!
//! A pure READER of the progress files (`progress/campaign.json`, `nodes/<node>.json`, `events.json`), plus an
//! optional read-only ledger reduction for drill-down. It never takes the lock, never writes, never signals; it can
//! run inside the supervisor or as a second process; a restart loses nothing.
//!
//! Entry points: [`run_dashboard`] (live; TUI on a terminal, plain lines otherwise), [`render_once`] (one text
//! rendering: `--once`, final summary, tests), [`view_model_json`] (`--json`), [`plain::status_line`] /
//! [`plain::event_line`], [`numbers::screen_numbers`] / [`numbers::campaign_json_numbers`] (website = terminal).
//!
//! Layering: `source` (files) -> `model::build` (the ONE adapter) -> `render` / `plain` (views); `app` holds the
//! interactive state and key handling; `terminal` the live loop and terminal restoration.

pub mod app;
pub mod controls;
pub mod format;
pub mod model;
pub mod numbers;
pub mod plain;
pub mod render;
pub mod source;
pub mod terminal;
pub mod theme;

use std::io::{self, IsTerminal, Write};
use std::sync::Arc;
use std::time::Duration;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;

pub use app::{App, ExitReason};
pub use controls::{Controls, SupervisorControls, WatchControls};
pub use model::ViewModel;
pub use numbers::{campaign_json_numbers, screen_numbers};
pub use source::{DirSource, MemorySource, ProgressSource};
pub use theme::ColorChoice;

use crate::clock::{Clock, SystemClock};

/// Options of the dashboard.
#[derive(Clone)]
pub struct DashboardOptions {
    /// Wall clock for ages, elapsed time and STALE (a `ManualClock` makes golden screens deterministic).
    pub clock: Arc<dyn Clock>,
    pub color: ColorChoice,
    /// Force plain mode (`--plain`).
    pub plain: bool,
    /// Box drawing: `None` = from the locale (TUI) / ASCII (`render_once`).
    pub utf8: Option<bool>,
    /// Data refresh interval (1 s).
    pub refresh: Duration,
    /// Plain mode status line interval (60 s).
    pub plain_interval: Duration,
    /// Heartbeat age above which a live campaign is STALE (60 s, plan 3.P.2).
    pub stale_after: Duration,
    /// Full redraw interval (60 s, plan 3.U.2).
    pub full_redraw: Duration,
}

impl Default for DashboardOptions {
    fn default() -> Self {
        DashboardOptions {
            clock: Arc::new(SystemClock::new()),
            color: ColorChoice::Auto,
            plain: false,
            utf8: None,
            refresh: Duration::from_secs(1),
            plain_interval: Duration::from_secs(60),
            stale_after: Duration::from_secs(60),
            full_redraw: Duration::from_secs(60),
        }
    }
}

impl std::fmt::Debug for DashboardOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DashboardOptions")
            .field("color", &self.color)
            .field("plain", &self.plain)
            .field("utf8", &self.utf8)
            .field("refresh", &self.refresh)
            .finish_non_exhaustive()
    }
}

pub(crate) fn now_s(opts: &DashboardOptions) -> f64 {
    opts.clock.now_utc().0 as f64 / 1000.0
}

pub(crate) fn build_vm(raw: &source::RawSnapshot, opts: &DashboardOptions) -> ViewModel {
    model::build(
        raw,
        model::BuildOptions {
            now_s: now_s(opts),
            stale_after_s: opts.stale_after.as_secs_f64(),
        },
    )
}

/// The view-model of a source now (adapter + clock). Fills the running-job list from the source when the progress
/// files do not carry one.
pub fn view_model(source: &dyn ProgressSource, opts: &DashboardOptions) -> ViewModel {
    let mut vm = build_vm(&source.read(), opts);
    if vm.jobs.list.is_none()
        && let Some(list) = source.running_jobs()
    {
        vm.jobs.list = Some(list);
        vm.jobs.list_origin = "ledger";
    }
    vm
}

/// `--json`: the view-model as JSON.
pub fn view_model_json(source: &dyn ProgressSource, opts: &DashboardOptions) -> serde_json::Value {
    serde_json::to_value(view_model(source, opts)).unwrap_or(serde_json::Value::Null)
}

/// Renders one frame into a buffer (tests: colours of cells).
pub fn render_to_buffer(vm: &ViewModel, app: &App, color: bool, utf8: bool, now_s: f64, width: u16, height: u16) -> Buffer {
    let backend = TestBackend::new(width.max(1), height.max(1));
    let mut term = Terminal::new(backend).expect("test backend");
    let ctx = render::Ctx {
        theme: theme::Theme { color },
        utf8,
        app,
        now_s,
    };
    term.draw(|f| render::draw(f, vm, &ctx)).expect("test backend draws");
    term.backend().buffer().clone()
}

/// Text of a buffer, trailing spaces of each line removed.
pub fn buffer_text(b: &Buffer) -> String {
    let a = b.area;
    let mut lines: Vec<String> = (a.y..a.bottom())
        .map(|y| {
            let s: String = (a.x..a.right()).map(|x| b[(x, y)].symbol()).collect();
            s.trim_end().to_string()
        })
        .collect();
    while lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    let mut s = lines.join("\n");
    s.push('\n');
    s
}

/// Renders the view-model with a given interactive state as plain text.
pub fn render_text(vm: &ViewModel, app: &App, opts: &DashboardOptions, width: u16, height: u16) -> String {
    buffer_text(&render_to_buffer(
        vm,
        app,
        false,
        opts.utf8.unwrap_or(false),
        now_s(opts),
        width,
        height,
    ))
}

/// One full text rendering of the SUMMARY screen (no escape codes): `--once`, the final summary, tests.
pub fn render_once(source: &dyn ProgressSource, opts: &DashboardOptions, width: u16, height: u16) -> String {
    let vm = view_model(source, opts);
    render_text(&vm, &App::new(false, 120), opts, width, height)
}

fn utf8_from_env() -> bool {
    let v = |k: &str| std::env::var(k).ok();
    theme::utf8_locale(v("LC_ALL").as_deref(), v("LC_CTYPE").as_deref(), v("LANG").as_deref())
}

/// The live dashboard: TUI when stdout is a terminal (and not `--plain` / `TERM=dumb`), plain lines otherwise.
/// Returns when the viewer quits (watcher) or the supervisor reports it is done (`Controls::campaign_done`).
/// The terminal is restored on every path; the last view is then printed to the normal screen.
pub fn run_dashboard(source: &dyn ProgressSource, controls: &dyn Controls, opts: &DashboardOptions) -> ExitReason {
    if !terminal::wants_tui(opts, controls) {
        return terminal::run_plain(source, controls, opts, &mut io::stdout());
    }
    let color = opts.color.resolve(
        io::stdout().is_terminal(),
        std::env::var("NO_COLOR").ok().as_deref(),
        std::env::var("TERM").ok().as_deref(),
    );
    let utf8 = opts.utf8.unwrap_or_else(utf8_from_env);
    match terminal::run_tui(source, controls, opts, color, utf8) {
        Ok((reason, vm, app)) => {
            if reason != ExitReason::Hangup {
                let (w, h) = terminal::terminal_area()
                    .map(|a| (a.width, a.height))
                    .unwrap_or((120, 40));
                let mut summary_app = App::new(app.hosts_campaign, app.grace_s);
                summary_app.stop_count = app.stop_count;
                summary_app.stop_first_s = app.stop_first_s;
                let text = render_text(&vm, &summary_app, opts, w, h.min(60));
                let mut out = io::stdout().lock();
                let _ = out.write_all(text.as_bytes());
                let _ = out.flush();
            }
            reason
        }
        Err(e) => {
            let _ = writeln!(io::stderr(), "dashboard: terminal unavailable ({e}); plain mode");
            terminal::run_plain(source, controls, opts, &mut io::stdout())
        }
    }
}
