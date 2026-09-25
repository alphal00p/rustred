//! One native renderer for the supervisor and the read-only status viewer.

use std::ffi::OsString;
use std::fs::File;
use std::io::{self, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

use crossterm::{cursor, execute, style};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Cell, Gauge, Paragraph, Row, Table};
use ratatui::{Frame, Terminal, TerminalOptions, Viewport};
use serde::Deserialize;
use serde_json::Value;

use crate::cli::args::ArgError;
use crate::cli::error::CliError;

const SCHEMA: &str = "rustred.independent-root-status.v1";
const MAX_STATUS_BYTES: u64 = 16 * 1024 * 1024;
const PLAIN_INTERVAL: Duration = Duration::from_secs(5);
const STALE_SECONDS: f64 = 30.0;
const INLINE_HEIGHT: u16 = 24;

#[derive(Debug, Deserialize)]
struct Status {
    schema: String,
    state: String,
    timestamp_unix_seconds: f64,
    shards_total: u64,
    shards_completed: u64,
    #[serde(default)]
    shards_running: u64,
    #[serde(default)]
    shards_queued: u64,
    #[serde(default)]
    shards_paused: u64,
    #[serde(default)]
    shards_failed: u64,
    #[serde(default)]
    roots_total: Option<u64>,
    #[serde(default)]
    roots_completed: Option<u64>,
    #[serde(default)]
    roots_running: Option<u64>,
    #[serde(default)]
    roots_queued: Option<u64>,
    #[serde(default)]
    roots_paused: Option<u64>,
    #[serde(default)]
    roots_failed: Option<u64>,
    #[serde(default)]
    workers_budget: Option<u64>,
    #[serde(default)]
    running_workers: Option<u64>,
    #[serde(default)]
    busy_workers: Option<u64>,
    #[serde(default)]
    jobs_limit: Option<u64>,
    #[serde(default)]
    cpus: Vec<u64>,
    #[serde(default)]
    cpus_available: Option<u64>,
    #[serde(default)]
    rss_bytes: Option<u64>,
    #[serde(default)]
    peak_rss_bytes: Option<u64>,
    #[serde(default)]
    max_memory_bytes: Option<u64>,
    #[serde(default)]
    measured_busy_cores: Option<f64>,
    #[serde(default)]
    cpu_seconds: Option<f64>,
    #[serde(default)]
    elapsed_seconds: Option<f64>,
    #[serde(default)]
    completed_domains: Option<u64>,
    #[serde(default)]
    pending_domains: Option<u64>,
    #[serde(default)]
    frontiers: Option<u64>,
    #[serde(default)]
    errors: Option<u64>,
    #[serde(default)]
    recent_domains_per_second: Option<f64>,
    #[serde(default)]
    latest_checkpoint_age_seconds: Option<f64>,
    #[serde(default)]
    checkpoint_saved_at_unix_seconds: Option<f64>,
    #[serde(default)]
    checkpoint_status: Option<String>,
    #[serde(default)]
    progress_updated_at_unix_seconds: Option<f64>,
    #[serde(default)]
    event_tail_backlogged: bool,
    #[serde(default)]
    artifact_state: Option<String>,
    #[serde(default)]
    artifact_path: Option<String>,
    #[serde(default)]
    last_error: Option<String>,
    #[serde(default)]
    resume_command: Option<String>,
    #[serde(default)]
    active: Vec<ShardStatus>,
}

#[derive(Debug, Deserialize)]
struct ShardStatus {
    id: Value,
    state: String,
    #[serde(default)]
    owners: Value,
    #[serde(default)]
    root_count: Option<u64>,
    #[serde(default)]
    completed_domains: Option<u64>,
    #[serde(default)]
    pending_domains: Option<u64>,
    #[serde(default)]
    rss_bytes: Option<u64>,
    #[serde(default)]
    measured_busy_cores: Option<f64>,
    #[serde(default)]
    checkpoint_age_seconds: Option<f64>,
    #[serde(default)]
    checkpoint_saved_at_unix_seconds: Option<f64>,
    #[serde(default)]
    checkpoint_status: Option<String>,
    #[serde(default)]
    progress_updated_at_unix_seconds: Option<f64>,
    #[serde(default)]
    event_tail_backlogged: bool,
    #[serde(default)]
    native_tail: Option<TailStatus>,
}

#[derive(Debug, Deserialize)]
struct TailStatus {
    #[serde(default)]
    backlogged: bool,
}

impl ShardStatus {
    fn backlogged(&self) -> bool {
        self.event_tail_backlogged
            || self
                .native_tail
                .as_ref()
                .is_some_and(|tail| tail.backlogged)
    }
}

impl Status {
    fn parse(value: &Value) -> Result<Self, CliError> {
        let status: Self = serde_json::from_value(value.clone()).map_err(|error| {
            CliError::Input(format!("invalid independent-root campaign status: {error}"))
        })?;
        if status.schema != SCHEMA {
            return Err(CliError::Input(format!(
                "unsupported campaign status schema {:?}; expected {SCHEMA}",
                status.schema
            )));
        }
        if !status.timestamp_unix_seconds.is_finite()
            || status.timestamp_unix_seconds < 0.0
            || status.shards_completed > status.shards_total
        {
            return Err(CliError::Input(
                "invalid campaign timestamp or shard completion counters".into(),
            ));
        }
        Ok(status)
    }

    fn final_state(&self) -> bool {
        matches!(
            self.state.as_str(),
            "completed" | "complete" | "failed" | "paused" | "stopped" | "cancelled"
        )
    }

    fn failed(&self) -> bool {
        matches!(self.state.as_str(), "failed" | "error")
            || self.shards_failed != 0
            || self.last_error.is_some()
    }

    fn age(&self, now: f64) -> f64 {
        (now - self.timestamp_unix_seconds).max(0.0)
    }

    fn header(&self, now: f64) -> String {
        let state = clean(&self.state);
        if self.age(now) > STALE_SECONDS && !self.final_state() {
            format!("RustRed independent roots | LAST REPORTED {state} | STALE STATUS")
        } else {
            format!("RustRed independent roots | {state}")
        }
    }

    fn shard_label(&self) -> String {
        format!(
            "Shard completion: {}/{}",
            self.shards_completed, self.shards_total
        )
    }

    fn roots_line(&self) -> String {
        format!(
            "Topology roots: {} finished / {} active / {} waiting / {} paused / {} failed ({} total)",
            count(self.roots_completed),
            count(self.roots_running),
            count(self.roots_queued),
            count(self.roots_paused),
            count(self.roots_failed),
            count(self.roots_total)
        )
    }

    fn worker_line(&self) -> String {
        let native_busy = self
            .busy_workers
            .map(|workers| format!("; {workers} native busy"))
            .unwrap_or_default();
        format!(
            "Workers: {} reserved / {} budget{native_busy}; {} measured CPU cores; CPUs {}",
            count(self.running_workers),
            count(self.workers_budget),
            decimal(self.measured_busy_cores),
            count(
                self.cpus_available
                    .or_else(|| (!self.cpus.is_empty()).then_some(self.cpus.len() as u64))
            )
        )
    }

    fn resources_line(&self) -> String {
        format!(
            "RAM {} / {} limit (peak {}); CPU {}s; elapsed {}",
            bytes(self.rss_bytes),
            bytes(self.max_memory_bytes),
            bytes(self.peak_rss_bytes),
            decimal(self.cpu_seconds),
            duration(self.elapsed_seconds)
        )
    }

    fn work_line(&self) -> String {
        // Cumulative counters may include resumed work; dividing those by this
        // invocation's elapsed time would invent a misleading throughput.
        format!(
            "Native completed {} | pending {} | {} domains/s (recent) | frontiers {} | errors {}",
            count(self.completed_domains),
            count(self.pending_domains),
            decimal(self.recent_domains_per_second),
            count(self.frontiers),
            count(self.errors)
        )
    }

    fn artifact_line(&self) -> String {
        let artifact = self.artifact_state.as_deref().unwrap_or("pending");
        let inconsistent = artifact == "complete"
            && (self.shards_completed != self.shards_total || self.shards_failed != 0);
        if inconsistent {
            return "Artifact: ERROR: complete reported before every shard completed".into();
        }
        format!(
            "Artifact: {}{}",
            clean(artifact),
            self.artifact_path
                .as_deref()
                .map(|path| format!(" | {}", clean(path)))
                .unwrap_or_default()
        )
    }

    fn freshness_line(&self, now: f64) -> String {
        let progress = self.progress_updated_at_unix_seconds.or_else(|| {
            self.active
                .iter()
                .filter_map(|shard| shard.progress_updated_at_unix_seconds)
                .min_by(f64::total_cmp)
        });
        let native_age =
            if self.event_tail_backlogged || self.active.iter().any(ShardStatus::backlogged) {
                "catching up (age unknown)".into()
            } else {
                age(progress, now)
            };
        format!(
            "Status age {} | oldest active native update {} | checkpoint {} | no closure ETA",
            duration(Some(self.age(now))),
            native_age,
            checkpoint(
                self.checkpoint_status.as_deref(),
                self.checkpoint_saved_at_unix_seconds,
                self.latest_checkpoint_age_seconds,
                self.age(now),
                now
            )
        )
    }
}

fn clean(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_control())
        .take(1024)
        .collect()
}

fn count(value: Option<u64>) -> String {
    value.map_or_else(|| "?".into(), |value| value.to_string())
}
fn decimal(value: Option<f64>) -> String {
    value
        .filter(|value| value.is_finite() && *value >= 0.0)
        .map_or_else(|| "?".into(), |value| format!("{value:.1}"))
}

fn duration(value: Option<f64>) -> String {
    let Some(seconds) = value.filter(|value| value.is_finite() && *value >= 0.0) else {
        return "unknown".into();
    };
    let seconds = seconds as u64;
    format!(
        "{:02}:{:02}:{:02}",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60
    )
}

fn age(saved: Option<f64>, now: f64) -> String {
    duration(
        saved
            .filter(|saved| saved.is_finite() && *saved >= 0.0)
            .map(|saved| (now - saved).max(0.0)),
    )
}

fn bytes(value: Option<u64>) -> String {
    match value {
        None => "?".into(),
        Some(value) if value >= 1024 * 1024 * 1024 => {
            format!("{:.1}GiB", value as f64 / (1024.0 * 1024.0 * 1024.0))
        }
        Some(value) if value >= 1024 * 1024 => {
            format!("{:.0}MiB", value as f64 / (1024.0 * 1024.0))
        }
        Some(value) => format!("{value}B"),
    }
}

fn checkpoint(
    state: Option<&str>,
    saved: Option<f64>,
    reported_age: Option<f64>,
    status_age: f64,
    now: f64,
) -> String {
    let saved_age = saved
        .map(|saved| (now - saved).max(0.0))
        .or_else(|| reported_age.map(|reported| reported + status_age));
    let state = state.unwrap_or(if saved_age.is_some() {
        "saved"
    } else {
        "unavailable"
    });
    if state == "writing" {
        format!("WRITING; saved {}", duration(saved_age))
    } else if saved_age.is_some() {
        format!("{} {}", clean(state), duration(saved_age))
    } else {
        clean(state)
    }
}

fn shard_roots(shard: &ShardStatus) -> Option<u64> {
    shard
        .root_count
        .or_else(|| shard.owners.as_u64())
        .or_else(|| shard.owners.as_array().map(|owners| owners.len() as u64))
}

fn shard_name(shard: &ShardStatus) -> String {
    format!(
        "{} {}",
        clean(
            shard
                .id
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| shard.id.to_string())
                .as_str()
        ),
        clean(&shard.state)
    )
}

fn shard_checkpoint(shard: &ShardStatus, status: &Status, now: f64) -> String {
    checkpoint(
        shard.checkpoint_status.as_deref(),
        shard.checkpoint_saved_at_unix_seconds,
        shard.checkpoint_age_seconds,
        status.age(now),
        now,
    )
}

fn plain_line(status: &Status, now: f64) -> String {
    format!(
        "{} | {} | shards {} active / {} waiting / {} paused / {} failed | {} | {} | {} | {} | {} | {}{}",
        status.header(now),
        status.shard_label(),
        status.shards_running,
        status.shards_queued,
        status.shards_paused,
        status.shards_failed,
        status.roots_line(),
        status.worker_line(),
        status.resources_line(),
        status.work_line(),
        status.freshness_line(now),
        status.artifact_line(),
        status
            .last_error
            .as_deref()
            .map(|error| format!(" | ERROR {}", clean(error)))
            .unwrap_or_default()
    )
}

fn render(frame: &mut Frame<'_>, status: &Status, now: f64, color: bool) {
    let areas = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(2),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(2),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(frame.area());
    let header_style = if color {
        Style::default()
            .fg(if status.failed() {
                Color::Red
            } else if status.age(now) > STALE_SECONDS && !status.final_state() {
                Color::Yellow
            } else {
                Color::Cyan
            })
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    frame.render_widget(
        Paragraph::new(status.header(now)).style(header_style),
        areas[0],
    );
    let ratio = if status.shards_total == 0 {
        0.0
    } else {
        status.shards_completed as f64 / status.shards_total as f64
    };
    let gauge_style = if color {
        Style::default().fg(if status.failed() {
            Color::Red
        } else {
            Color::Green
        })
    } else {
        Style::default()
    };
    frame.render_widget(
        Gauge::default()
            .ratio(ratio.clamp(0.0, 1.0))
            .label(status.shard_label())
            .gauge_style(gauge_style),
        areas[1],
    );
    frame.render_widget(
        Paragraph::new(format!(
            "{}\n{}",
            status.worker_line(),
            status.resources_line()
        )),
        areas[2],
    );
    frame.render_widget(Paragraph::new(status.work_line()), areas[3]);
    frame.render_widget(Paragraph::new(status.roots_line()), areas[4]);
    render_shards(frame, areas[5], status, now, color);
    frame.render_widget(Paragraph::new(status.artifact_line()), areas[6]);
    frame.render_widget(Paragraph::new(status.freshness_line(now)), areas[7]);
    let footer = if let Some(error) = &status.last_error {
        format!("ERROR: {}", clean(error))
    } else {
        format!(
            "Shards {} active / {} waiting / {} paused / {} failed; job limit {}; shard completion is not tuple or time coverage",
            status.shards_running,
            status.shards_queued,
            status.shards_paused,
            status.shards_failed,
            count(status.jobs_limit)
        )
    };
    frame.render_widget(
        Paragraph::new(footer).style(if status.failed() {
            header_style
        } else {
            Style::default()
        }),
        areas[8],
    );
}

fn render_shards(frame: &mut Frame<'_>, area: Rect, status: &Status, now: f64, color: bool) {
    if area.height == 0 {
        return;
    }
    let capacity = area.height.saturating_sub(2) as usize;
    // Filtering before taking the viewport limit avoids an early finished shard
    // hiding later active work. Hidden rows are always counted in the footer.
    let active = status
        .active
        .iter()
        .filter(|shard| {
            matches!(
                shard.state.as_str(),
                "running" | "starting" | "stopping" | "checkpointing" | "draining"
            )
        })
        .collect::<Vec<_>>();
    let wide = area.width >= 115;
    let headings = if wide {
        vec![
            "Shard / state",
            "Roots",
            "Native done",
            "Pending",
            "RSS",
            "Busy CPU",
            "Checkpoint / saved age",
            "Native age",
        ]
    } else {
        vec![
            "Shard/state",
            "Roots",
            "Done",
            "Pending",
            "RSS",
            "CPU",
            "CP / saved age",
        ]
    };
    let mut rows = Vec::with_capacity(capacity.min(active.len()));
    for shard in active.iter().take(capacity) {
        let mut cells = vec![
            shard_name(shard),
            count(shard_roots(shard)),
            count(shard.completed_domains),
            count(shard.pending_domains),
            bytes(shard.rss_bytes),
            decimal(shard.measured_busy_cores),
            shard_checkpoint(shard, status, now),
        ];
        if wide {
            cells.push(if shard.backlogged() {
                "catching up".into()
            } else {
                age(shard.progress_updated_at_unix_seconds, now)
            });
        }
        rows.push(Row::new(cells.into_iter().map(Cell::from)));
    }
    let mut widths = vec![
        Constraint::Min(13),
        Constraint::Length(5),
        Constraint::Length(if wide { 11 } else { 6 }),
        Constraint::Length(7),
        Constraint::Length(7),
        Constraint::Length(if wide { 8 } else { 4 }),
        Constraint::Length(if wide { 26 } else { 17 }),
    ];
    if wide {
        widths.push(Constraint::Length(12));
    }
    let header = Row::new(headings).style(if color {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    });
    let table_area = Rect {
        height: area.height.saturating_sub(1),
        ..area
    };
    frame.render_widget(
        Table::new(rows, widths).header(header).column_spacing(1),
        table_area,
    );
    let hidden = active.len().saturating_sub(capacity);
    let message = if hidden > 0 {
        format!("+ {hidden} active shards not shown; --once / --json includes every shard")
    } else if active.is_empty() {
        "No active shards".into()
    } else {
        format!(
            "{} active shard rows; completed domains are native counters",
            active.len()
        )
    };
    frame.render_widget(
        Paragraph::new(message),
        Rect::new(area.x, area.y + area.height - 1, area.width, 1),
    );
}

struct InlineTerminal {
    terminal: Terminal<CrosstermBackend<io::Stderr>>,
}

impl InlineTerminal {
    fn new() -> io::Result<Self> {
        let height = crossterm::terminal::size()
            .map(|(_, height)| height)
            .unwrap_or(INLINE_HEIGHT)
            .min(INLINE_HEIGHT)
            .max(1);
        let mut terminal = Terminal::with_options(
            CrosstermBackend::new(io::stderr()),
            TerminalOptions {
                viewport: Viewport::Inline(height),
            },
        )?;
        terminal.hide_cursor()?;
        Ok(Self { terminal })
    }

    fn draw(&mut self, status: &Status, now: f64) -> io::Result<()> {
        self.terminal
            .draw(|frame| render(frame, status, now, true))?;
        Ok(())
    }
}

impl Drop for InlineTerminal {
    fn drop(&mut self) {
        let _ = self.terminal.show_cursor();
        let _ = execute!(
            self.terminal.backend_mut(),
            style::ResetColor,
            cursor::Show,
            cursor::MoveToNextLine(1)
        );
        let _ = self.terminal.backend_mut().flush();
    }
}

fn use_terminal(tty: bool, no_color: bool, term: Option<&str>) -> bool {
    tty && !no_color && term != Some("dumb")
}

pub(super) struct Presenter {
    terminal: Option<InlineTerminal>,
    last_line: Option<Instant>,
    last_state: Option<String>,
}

impl Presenter {
    pub(super) fn new() -> Self {
        let interactive = use_terminal(
            io::stderr().is_terminal(),
            std::env::var_os("NO_COLOR").is_some(),
            std::env::var("TERM").ok().as_deref(),
        );
        Self {
            terminal: interactive.then(InlineTerminal::new).and_then(Result::ok),
            last_line: None,
            last_state: None,
        }
    }

    pub(super) fn update(&mut self, value: &Value) -> Result<(), CliError> {
        let status = Status::parse(value)?;
        let now = super::now();
        if let Some(terminal) = &mut self.terminal {
            if terminal.draw(&status, now).is_ok() {
                return Ok(());
            }
            self.terminal = None;
        }
        let changed = self.last_state.as_deref() != Some(&status.state);
        if changed
            || self
                .last_line
                .is_none_or(|last| last.elapsed() >= PLAIN_INTERVAL)
        {
            let mut stderr = io::stderr().lock();
            writeln!(stderr, "{}", plain_line(&status, now))
                .and_then(|()| stderr.flush())
                .map_err(|error| CliError::OutputIo(format!("write campaign monitor: {error}")))?;
            self.last_line = Some(Instant::now());
            self.last_state = Some(status.state);
        }
        Ok(())
    }
}

#[derive(Debug)]
struct Options {
    directory: PathBuf,
    once: bool,
    json: bool,
}

fn parse_options(arguments: Vec<OsString>) -> Result<Options, CliError> {
    let mut directory = None;
    let mut once = false;
    let mut json = false;
    let mut arguments = arguments.into_iter();
    while let Some(option) = arguments.next() {
        let option = option.into_string().map_err(ArgError::NonUtf8Option)?;
        match option.as_str() {
            "--directory" => {
                if directory.is_some() {
                    return Err(ArgError::DuplicateOption("--directory").into());
                }
                let path = arguments
                    .next()
                    .ok_or(ArgError::MissingValue("--directory"))?;
                if path.is_empty() {
                    return Err(ArgError::EmptyPath.into());
                }
                directory = Some(PathBuf::from(path));
            }
            "--once" if !once => once = true,
            "--json" if !json => json = true,
            "--once" => return Err(ArgError::DuplicateOption("--once").into()),
            "--json" => return Err(ArgError::DuplicateOption("--json").into()),
            _ if option.starts_with('-') => return Err(ArgError::UnknownOption(option).into()),
            _ => return Err(ArgError::UnexpectedArgument(option).into()),
        }
    }
    if once && json {
        return Err(ArgError::InvalidCombination("choose --once or --json, not both").into());
    }
    Ok(Options {
        directory: directory.ok_or(ArgError::MissingRequiredOption("--directory"))?,
        once,
        json,
    })
}

fn read_status(directory: &Path) -> Result<Value, CliError> {
    let path = directory.join("status.json");
    let file = File::open(&path).map_err(|error| {
        CliError::InputIo(format!("open campaign status {}: {error}", path.display()))
    })?;
    let mut bytes = Vec::new();
    file.take(MAX_STATUS_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            CliError::InputIo(format!("read campaign status {}: {error}", path.display()))
        })?;
    if bytes.len() as u64 > MAX_STATUS_BYTES {
        return Err(CliError::Input(
            "campaign status exceeds 16 MiB viewer limit".into(),
        ));
    }
    let value = serde_json::from_slice(&bytes).map_err(|error| {
        CliError::Input(format!("parse campaign status {}: {error}", path.display()))
    })?;
    Status::parse(&value)?;
    Ok(value)
}

fn write_json(writer: &mut impl Write, value: &Value) -> io::Result<()> {
    serde_json::to_writer(&mut *writer, value).map_err(io::Error::other)?;
    writer.write_all(b"\n")?;
    writer.flush()
}

fn write_once(writer: &mut impl Write, status: &Status, now: f64) -> io::Result<()> {
    writeln!(writer, "{}", plain_line(status, now))?;
    for shard in &status.active {
        writeln!(
            writer,
            "  shard {} | roots {} | native completed {} | pending {} | RSS {} | busy CPU {} | checkpoint {} | native age {}{}",
            shard_name(shard),
            count(shard_roots(shard)),
            count(shard.completed_domains),
            count(shard.pending_domains),
            bytes(shard.rss_bytes),
            decimal(shard.measured_busy_cores),
            shard_checkpoint(shard, status, now),
            if shard.backlogged() {
                "unknown".into()
            } else {
                age(shard.progress_updated_at_unix_seconds, now)
            },
            if shard.backlogged() {
                " (event reader catching up)"
            } else {
                ""
            }
        )?;
    }
    if let Some(command) = &status.resume_command {
        writeln!(writer, "Resume: {}", clean(command))?;
    }
    writer.flush()
}

struct ViewerSignals(Vec<signal_hook::SigId>);

impl ViewerSignals {
    fn register(cancel: &Arc<AtomicBool>) -> Result<Self, CliError> {
        let mut signals = Self(Vec::new());
        for signal in [signal_hook::consts::SIGINT, signal_hook::consts::SIGTERM] {
            let id = signal_hook::flag::register(signal, Arc::clone(cancel)).map_err(|error| {
                CliError::InputIo(format!(
                    "install campaign viewer interrupt handler: {error}"
                ))
            })?;
            signals.0.push(id);
        }
        Ok(signals)
    }
}

impl Drop for ViewerSignals {
    fn drop(&mut self) {
        for id in &self.0 {
            signal_hook::low_level::unregister(*id);
        }
    }
}

/// Reads only status.json. This command never acquires campaign locks, signals
/// processes, rewrites status, or invokes native algebra.
pub(crate) fn run(arguments: Vec<OsString>) -> Result<(), CliError> {
    let options = parse_options(arguments)?;
    if options.once || options.json {
        let value = read_status(&options.directory)?;
        let mut stdout = io::stdout().lock();
        if options.json {
            write_json(&mut stdout, &value)
        } else {
            write_once(&mut stdout, &Status::parse(&value)?, super::now())
        }
        .map_err(|error| CliError::OutputIo(format!("write campaign status: {error}")))?;
        return Ok(());
    }
    let cancel = Arc::new(AtomicBool::new(false));
    // Keep this guard alive until after the terminal is restored. Interrupting
    // a viewer only closes the view; the campaign receives no stop request.
    let _signals = ViewerSignals::register(&cancel)?;
    let mut presenter = Presenter::new();
    while !cancel.load(Ordering::Relaxed) {
        let value = read_status(&options.directory)?;
        let status = Status::parse(&value)?;
        presenter.update(&value)?;
        if status.final_state() {
            break;
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use serde_json::json;

    fn fixture() -> Value {
        json!({"schema":SCHEMA,"state":"running","timestamp_unix_seconds":100.0,"shards_total":20,"shards_completed":3,
            "shards_running":2,"shards_queued":15,"roots_total":100,"roots_completed":15,"roots_running":10,"roots_queued":75,
            "workers_budget":50,"running_workers":8,"busy_workers":2,"measured_busy_cores":1.5,"cpus_available":50,
            "completed_domains":100,"pending_domains":900,"frontiers":0,"errors":0,"elapsed_seconds":20.0,
            "active":[{"id":1,"state":"running","owners":["01","10"],"completed_domains":40,"pending_domains":5,
                "checkpoint_status":"writing","checkpoint_saved_at_unix_seconds":80.0,"progress_updated_at_unix_seconds":95.0}]})
    }

    fn screen(terminal: &Terminal<TestBackend>) -> String {
        let buffer = terminal.backend().buffer();
        let area = buffer.area;
        (area.y..area.bottom())
            .map(|y| {
                (area.x..area.right())
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn gauge_labels_shards_and_header_colors_follow_error_state() {
        let mut terminal = Terminal::new(TestBackend::new(150, 20)).unwrap();
        let mut value = fixture();
        let status = Status::parse(&value).unwrap();
        terminal
            .draw(|frame| render(frame, &status, 105.0, true))
            .unwrap();
        let text = screen(&terminal);
        assert!(text.contains("Shard completion: 3/20"));
        assert!(text.contains("8 reserved / 50 budget; 2 native busy; 1.5 measured CPU cores"));
        assert!(text.contains("WRITING; saved 00:00:25"));
        assert!(text.contains("no closure ETA"));
        assert_eq!(terminal.backend().buffer()[(0, 0)].fg, Color::Cyan);
        value["state"] = json!("failed");
        value["last_error"] = json!("disk unavailable");
        let failed = Status::parse(&value).unwrap();
        terminal
            .draw(|frame| render(frame, &failed, 105.0, true))
            .unwrap();
        assert_eq!(terminal.backend().buffer()[(0, 0)].fg, Color::Red);
        assert!(screen(&terminal).contains("ERROR: disk unavailable"));
        terminal
            .draw(|frame| render(frame, &failed, 105.0, false))
            .unwrap();
        assert_eq!(terminal.backend().buffer()[(0, 0)].fg, Color::Reset);
    }

    #[test]
    fn resize_filters_before_viewport_limit_and_counts_hidden_rows() {
        let mut value = fixture();
        let mut rows = vec![json!({"id":"finished-early","state":"completed"})];
        rows.extend((0..30).map(|id| json!({"id":id,"state":"running","owners":1})));
        value["active"] = json!(rows);
        let status = Status::parse(&value).unwrap();
        let mut terminal = Terminal::new(TestBackend::new(150, 14)).unwrap();
        terminal
            .draw(|frame| render(frame, &status, 105.0, true))
            .unwrap();
        let text = screen(&terminal);
        assert!(!text.contains("finished-early"));
        assert!(text.contains("0 running"));
        assert!(text.contains("active shards not shown"));
        for (width, height) in [(60, 12), (18, 3), (1, 1), (150, 25)] {
            terminal.backend_mut().resize(width, height);
            terminal.resize(Rect::new(0, 0, width, height)).unwrap();
            terminal
                .draw(|frame| render(frame, &status, 105.0, true))
                .unwrap();
        }
        assert!(screen(&terminal).contains("Shard completion: 3/20"));
    }

    #[test]
    fn json_and_plain_outputs_are_ansi_free_and_once_keeps_all_rows() {
        let mut value = fixture();
        value["last_error"] = json!("bad\n\u{1b}[31m");
        value["active"] = json!(
            (0..50)
                .map(|id| json!({"id":id,"state":"running","owners":1}))
                .collect::<Vec<_>>()
        );
        let status = Status::parse(&value).unwrap();
        let mut plain = Vec::new();
        write_once(&mut plain, &status, 105.0).unwrap();
        assert!(!plain.contains(&0x1b));
        assert_eq!(String::from_utf8(plain).unwrap().lines().count(), 51);
        let mut output = Vec::new();
        write_json(&mut output, &value).unwrap();
        assert!(!output.contains(&0x1b));
        assert_eq!(serde_json::from_slice::<Value>(&output).unwrap(), value);
        assert!(!use_terminal(false, false, Some("xterm")));
        assert!(!use_terminal(true, true, Some("xterm")));
        assert!(!use_terminal(true, false, Some("dumb")));
        assert!(use_terminal(true, false, Some("xterm")));
    }

    #[test]
    fn stale_snapshot_does_not_reset_native_or_checkpoint_ages() {
        let status = Status::parse(&fixture()).unwrap();
        assert!(
            status
                .header(150.0)
                .contains("LAST REPORTED running | STALE STATUS")
        );
        assert!(
            status
                .freshness_line(150.0)
                .contains("native update 00:00:55")
        );
        assert!(shard_checkpoint(&status.active[0], &status, 150.0).contains("saved 00:01:10"));
        assert!(status.work_line().contains("? domains/s (recent)"));
        let mut value = fixture();
        value.as_object_mut().unwrap().remove("busy_workers");
        let status = Status::parse(&value).unwrap();
        assert!(!status.worker_line().contains("native busy"));
        assert!(status.worker_line().contains("1.5 measured CPU cores"));
        value["active"][0]["native_tail"] = json!({"backlogged":true});
        let status = Status::parse(&value).unwrap();
        assert!(
            status
                .freshness_line(105.0)
                .contains("catching up (age unknown)")
        );
        let mut plain = Vec::new();
        write_once(&mut plain, &status, 105.0).unwrap();
        assert!(
            String::from_utf8(plain)
                .unwrap()
                .contains("native age unknown (event reader catching up)")
        );
        value["active"] = json!([]);
        value["event_tail_backlogged"] = json!(true);
        assert!(
            Status::parse(&value)
                .unwrap()
                .freshness_line(105.0)
                .contains("catching up (age unknown)")
        );
    }

    #[test]
    fn rejects_schema_counter_errors_and_misleading_artifact_complete() {
        let mut value = fixture();
        value["artifact_state"] = json!("complete");
        assert!(
            Status::parse(&value)
                .unwrap()
                .artifact_line()
                .contains("ERROR")
        );
        value["shards_completed"] = json!(21);
        assert!(Status::parse(&value).is_err());
        value["shards_completed"] = json!(3);
        value["schema"] = json!("different");
        assert!(Status::parse(&value).is_err());
        assert!(
            parse_options(vec![
                "--directory".into(),
                "x".into(),
                "--json".into(),
                "--once".into()
            ])
            .is_err()
        );
    }
}
