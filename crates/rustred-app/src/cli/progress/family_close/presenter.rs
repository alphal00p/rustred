//! Independent, best-effort presentation. Solver callbacks never perform I/O.

use std::io::{self, Write};
use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use super::super::terminal::TerminalSession;
use super::format::format_event;
use super::resources::Resources;
use super::state::{Counts, Snapshot, Tracker};
use crate::FamilyCloseProgress;

const TTY_INTERVAL: Duration = Duration::from_millis(100);
const PLAIN_INTERVAL: Duration = Duration::from_secs(1);
const RESOURCE_INTERVAL: Duration = Duration::from_secs(1);
const SHUTDOWN_GRACE: Duration = Duration::from_millis(500);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Outcome {
    Written,
    Failed,
    Stopped,
}

#[derive(Default)]
struct State {
    tracker: Tracker,
    latest: Option<Snapshot>,
    outcome: Option<Outcome>,
}

struct Shared {
    state: Mutex<State>,
    wake: Condvar,
    active: AtomicBool,
}

/// A bounded, coalesced progress snapshot, not a raw event journal. The only
/// solver-thread work is scalar tracking and replacing one snapshot under a
/// short lock. Formatting, resource sampling, terminal drawing and flushing
/// belong exclusively to the presenter thread.
pub(crate) struct FamilyCloseProgressMonitor<W: Write + Send + 'static> {
    shared: Option<Arc<Shared>>,
    thread: Option<JoinHandle<()>>,
    completed: Option<mpsc::Receiver<()>>,
    writer_type: PhantomData<W>,
}

impl<W: Write + Send + 'static> FamilyCloseProgressMonitor<W> {
    pub(crate) fn new(writer: W, terminal: bool, force: bool, no_color: bool) -> Self {
        Self::start(
            writer,
            terminal,
            force,
            no_color,
            if terminal {
                TTY_INTERVAL
            } else {
                PLAIN_INTERVAL
            },
            None,
        )
    }

    #[cfg(test)]
    pub(super) fn with_test_settings(
        writer: W,
        terminal: bool,
        force: bool,
        no_color: bool,
        interval: Duration,
        fixed_width: Option<u16>,
    ) -> Self {
        Self::start(writer, terminal, force, no_color, interval, fixed_width)
    }

    fn start(
        writer: W,
        terminal: bool,
        force: bool,
        no_color: bool,
        interval: Duration,
        fixed_width: Option<u16>,
    ) -> Self {
        let mut monitor = Self {
            shared: None,
            thread: None,
            completed: None,
            writer_type: PhantomData,
        };
        if !terminal && !force {
            return monitor;
        }
        let shared = Arc::new(Shared {
            state: Mutex::new(State::default()),
            wake: Condvar::new(),
            active: AtomicBool::new(true),
        });
        let state = shared.clone();
        let (sent, received) = mpsc::channel();
        let started = Instant::now();
        let spawned = thread::Builder::new()
            .name("rustred-family-progress".into())
            .spawn(move || {
                // Sending on Drop also wakes shutdown if a custom writer panics.
                let _completion = Completion {
                    shared: state.clone(),
                    sent,
                };
                let _ = present(
                    writer,
                    terminal,
                    !no_color,
                    interval.max(Duration::from_millis(1)),
                    fixed_width,
                    started,
                    &state,
                );
            });
        if let Ok(thread) = spawned {
            monitor.shared = Some(shared);
            monitor.thread = Some(thread);
            monitor.completed = Some(received);
        }
        monitor
    }

    pub(crate) fn observe(&mut self, event: FamilyCloseProgress) {
        let Some(shared) = &self.shared else {
            return;
        };
        if !shared.active.load(Ordering::Relaxed) {
            return;
        }
        // The presenter never holds this lock while allocating a display or
        // touching the writer. Aggregate counters update before coalescing.
        if let Ok(mut state) = shared.state.lock() {
            if state.outcome.is_none() {
                state.latest = Some(state.tracker.observe(event, Instant::now()));
            }
        }
    }

    pub(crate) fn finish(&mut self, success: bool) {
        self.stop(if success {
            Outcome::Written
        } else {
            Outcome::Failed
        });
    }

    fn stop(&mut self, outcome: Outcome) {
        let Some(shared) = self.shared.take() else {
            return;
        };
        if let Ok(mut state) = shared.state.lock() {
            state.outcome.get_or_insert(outcome);
        }
        shared.active.store(false, Ordering::Relaxed);
        shared.wake.notify_all();
        // A disconnected stderr/failed writer must not fail a solve. A blocked
        // custom writer must not hang it either: detach after a bounded grace.
        // Its terminal guard still restores the cursor when the write returns.
        let completed = self.completed.take().is_some_and(|done| {
            !matches!(
                done.recv_timeout(SHUTDOWN_GRACE),
                Err(mpsc::RecvTimeoutError::Timeout)
            )
        });
        if let Some(thread) = self.thread.take() {
            if completed {
                let _ = thread.join();
            }
        }
    }
}

impl<W: Write + Send + 'static> Drop for FamilyCloseProgressMonitor<W> {
    fn drop(&mut self) {
        self.stop(Outcome::Stopped);
    }
}

struct Completion {
    shared: Arc<Shared>,
    sent: mpsc::Sender<()>,
}
impl Drop for Completion {
    fn drop(&mut self) {
        self.shared.active.store(false, Ordering::Relaxed);
        let _ = self.sent.send(());
    }
}

enum Output<W: Write> {
    Terminal(TerminalSession<W>),
    Plain(W),
}
impl<W: Write> Output<W> {
    fn write(
        &mut self,
        display: &Display,
        color: bool,
        outcome: Option<Outcome>,
    ) -> io::Result<()> {
        match self {
            Self::Terminal(terminal) => terminal.render_family(
                [
                    &display.header,
                    &display.progress,
                    &display.counts,
                    &display.detail,
                    &display.resources,
                    &display.footer,
                ],
                color,
                outcome.is_some_and(|end| end != Outcome::Written),
            ),
            Self::Plain(writer) => {
                // One rate-limited snapshot per line, with no ANSI controls.
                writeln!(
                    writer,
                    "{} | {} | {} | {} | {} | {}",
                    display.header,
                    display.progress,
                    display.counts,
                    display.detail,
                    display.resources,
                    display.footer
                )?;
                writer.flush()
            }
        }
    }
}

fn present<W: Write>(
    writer: W,
    terminal: bool,
    color: bool,
    interval: Duration,
    fixed_width: Option<u16>,
    started: Instant,
    shared: &Shared,
) -> io::Result<()> {
    let mut output = if terminal {
        #[cfg(test)]
        let session = match fixed_width {
            Some(width) => TerminalSession::try_new_family_fixed(writer, width)?,
            None => TerminalSession::try_new_family(writer)?,
        };
        #[cfg(not(test))]
        let session = {
            let _ = fixed_width;
            TerminalSession::try_new_family(writer)?
        };
        Output::Terminal(session)
    } else {
        Output::Plain(writer)
    };
    let mut resources = Resources::default();
    let mut sampled = None;
    loop {
        let (snapshot, outcome) = {
            let Ok(state) = shared.state.lock() else {
                return Ok(());
            };
            (state.latest.clone(), state.outcome)
        };
        let now = Instant::now();
        if sampled.is_none_or(|last| now.saturating_duration_since(last) >= RESOURCE_INTERVAL) {
            resources = Resources::sample();
            sampled = Some(now);
        }
        let display = Display::new(snapshot.as_ref(), resources, started, now, outcome);
        output.write(&display, color, outcome)?;
        if outcome.is_some() {
            return Ok(());
        }
        let deadline = now + interval;
        let Ok(mut state) = shared.state.lock() else {
            return Ok(());
        };
        while state.outcome.is_none() {
            let wait = deadline.saturating_duration_since(Instant::now());
            if wait.is_zero() {
                break;
            }
            let Ok((next, _)) = shared.wake.wait_timeout(state, wait) else {
                return Ok(());
            };
            state = next;
        }
        // Only shutdown wakes early. Worker events replace the latest slot;
        // they cannot turn an event storm into unbounded output or redraws.
    }
}

pub(super) struct Display {
    pub(super) header: String,
    pub(super) progress: String,
    pub(super) counts: String,
    pub(super) detail: String,
    pub(super) resources: String,
    pub(super) footer: String,
}

impl Display {
    pub(super) fn new(
        snapshot: Option<&Snapshot>,
        resources: Resources,
        started: Instant,
        now: Instant,
        outcome: Option<Outcome>,
    ) -> Self {
        let elapsed = now.saturating_duration_since(started);
        let quiet = snapshot.map_or(elapsed, |s| now.saturating_duration_since(s.at));
        let phase = snapshot.map_or("starting", |s| s.phase);
        let phase_age = snapshot.map_or(elapsed, |s| now.saturating_duration_since(s.phase_since));
        let counts = snapshot.map_or(Counts::default(), |s| s.counts);
        let status = match outcome {
            None => "running",
            Some(Outcome::Written) => "output written",
            Some(Outcome::Failed) => "failed (see error)",
            Some(Outcome::Stopped) => "stopped (output unconfirmed)",
        };
        let header = format!(
            "RustRed | {status} | elapsed={:.1}s quiet={:.1}s",
            elapsed.as_secs_f64(),
            quiet.as_secs_f64()
        );
        let progress = progress_bar(counts.fraction(), elapsed);
        let detail = snapshot.map_or_else(
            || "No solver observation yet".into(),
            |s| {
                // The formatter's clock is the observation clock, not the heartbeat
                // clock. Explicit labeling prevents a stale event looking live.
                format!("last event: {}", format_event(s.event.clone(), s.frame))
            },
        );
        let last_failure = snapshot.and_then(|s| s.last_failure.as_ref());
        let footer = format!(
            "phase={phase} age={:.1}s case={}{}",
            phase_age.as_secs_f64(),
            snapshot
                .and_then(|s| s.case)
                .map_or_else(|| "unknown".into(), |v| v.to_string()),
            last_failure.map_or_else(String::new, |failure| {
                let safe: String = failure
                    .message
                    .chars()
                    .flat_map(|c| {
                        if c.is_control() {
                            c.escape_default().collect::<Vec<_>>()
                        } else {
                            vec![c]
                        }
                    })
                    .collect();
                format!(
                    " | last failure: sector={} ordinal={} message={safe}",
                    failure.sector, failure.ordinal
                )
            })
        );
        Self {
            header,
            progress,
            detail,
            footer,
            resources: resources.line(),
            counts: if counts.overflow {
                "aggregate counters overflowed; totals unknown".into()
            } else {
                format!(
                    "completed-sector totals: rules={} residuals={} | observed rule hits={} | reused={} failures={} checkpoints={}",
                    counts.rules,
                    counts.residuals,
                    counts.observed_rules,
                    counts.reused,
                    counts.failures,
                    counts.checkpointed
                )
            },
        }
    }
}

pub(super) fn progress_bar(fraction: Option<(usize, usize)>, elapsed: Duration) -> String {
    const WIDTH: usize = 20;
    match fraction {
        Some((0, 0)) => "sector generation: no sectors scheduled (not a closure claim)".into(),
        Some((done, total)) if total != 0 && done <= total => {
            let fill = ((done as u128 * WIDTH as u128) / total as u128) as usize;
            format!(
                "sector generation [{}{}] {done}/{total} (not closure)",
                "#".repeat(fill),
                "-".repeat(WIDTH - fill)
            )
        }
        _ => {
            let position = ((elapsed.as_millis() / 100) % WIDTH as u128) as usize;
            format!(
                "sector generation [{}>{}] total unknown (no ETA)",
                " ".repeat(position),
                " ".repeat(WIDTH - position - 1)
            )
        }
    }
}
