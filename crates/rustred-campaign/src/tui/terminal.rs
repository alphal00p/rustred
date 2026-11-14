//! The live loop: alternate screen + fixed viewport sized by `TIOCGWINSZ` before every draw, raw mode for keys,
//! terminal restored on every exit path (guard + panic hook), event-driven with a 250 ms tick.
//!
//! Terminal setup and resize handling follow rustred-app `cli/progress/preparation/viewer.rs` (b9f06de7:
//! `terminal_area` by TIOCGWINSZ on the output descriptor, `Viewport::Fixed`, alternate screen, restore in `Drop`),
//! copied and adapted (stdout, raw mode for keys, panic hook).

use std::io::{self, IsTerminal, Write};
use std::sync::Once;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyEventKind};
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{cursor, execute, style};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::{Terminal, TerminalOptions, Viewport};

use super::app::{Action, App, ExitReason};
use super::controls::Controls;
use super::model::ViewModel;
use super::plain::PlainPrinter;
use super::render::{Ctx, draw};
use super::source::ProgressSource;
use super::theme::Theme;
use super::{DashboardOptions, now_s, view_model};

static ACTIVE: AtomicBool = AtomicBool::new(false);
static HOOK: Once = Once::new();

/// Restores the terminal (idempotent; safe from a panic hook).
pub fn restore_terminal() {
    if ACTIVE.swap(false, Ordering::SeqCst) {
        let _ = crossterm::terminal::disable_raw_mode();
        let mut out = io::stdout();
        let _ = execute!(out, style::ResetColor, cursor::Show, LeaveAlternateScreen);
        let _ = out.flush();
    }
}

fn install_panic_hook() {
    HOOK.call_once(|| {
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            restore_terminal();
            prev(info);
        }));
    });
}

/// Restores the terminal when dropped.
pub struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore_terminal();
    }
}

/// Size of the terminal on stdout (TIOCGWINSZ), not crossterm's guess.
pub fn terminal_area() -> io::Result<Rect> {
    use std::os::fd::AsRawFd;
    let mut size = libc::winsize {
        ws_row: 0,
        ws_col: 0,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    // SAFETY: `size` is a writable, correctly sized winsize for TIOCGWINSZ; stdout's descriptor stays open for the
    // whole call.
    if unsafe { libc::ioctl(io::stdout().as_raw_fd(), libc::TIOCGWINSZ, &mut size) } != 0 {
        return Err(io::Error::last_os_error());
    }
    if size.ws_col == 0 || size.ws_row == 0 {
        return Err(io::Error::other("terminal reports an empty viewport"));
    }
    Ok(Rect::new(0, 0, size.ws_col, size.ws_row))
}

type Term = Terminal<CrosstermBackend<io::Stdout>>;

fn setup() -> io::Result<(Term, TerminalGuard, Rect)> {
    let area = terminal_area()?;
    install_panic_hook();
    crossterm::terminal::enable_raw_mode()?;
    ACTIVE.store(true, Ordering::SeqCst);
    let guard = TerminalGuard;
    execute!(io::stdout(), EnterAlternateScreen, cursor::Hide)?;
    let mut t = Terminal::with_options(
        CrosstermBackend::new(io::stdout()),
        TerminalOptions {
            viewport: Viewport::Fixed(area),
        },
    )?;
    t.clear()?;
    Ok((t, guard, area))
}

fn finish(controls: &dyn Controls, app: &App) -> ExitReason {
    let n = controls.stop_count().max(app.stop_count);
    if n == 0 {
        ExitReason::CampaignEnded
    } else {
        ExitReason::StopRequested(crate::signals::StopLevel::from_count(n))
    }
}

/// The TUI loop. Returns the exit reason and the last view-model (printed to the normal screen by the caller).
pub fn run_tui(
    source: &dyn ProgressSource,
    controls: &dyn Controls,
    opts: &DashboardOptions,
    color: bool,
    utf8: bool,
) -> Result<(ExitReason, ViewModel, App), io::Error> {
    let (mut term, guard, mut area) = setup()?;
    let mut app = App::new(controls.hosts_campaign(), controls.grace_seconds());
    let mut vm = view_model(source, opts);
    let mut last_read = Instant::now();
    let mut last_full = Instant::now();
    let mut last_draw: Option<Instant> = None;
    let mut dirty = true;
    let theme = Theme { color };
    let reason = loop {
        let now = now_s(opts);
        if controls.hangup_seen() {
            break ExitReason::Hangup;
        }
        if controls.campaign_done() {
            break finish(controls, &app);
        }
        let before = app.stop_count;
        app.observe_stops(controls.stop_count(), now);
        if app.stop_count != before {
            dirty = true;
            if !controls.hosts_campaign() {
                break ExitReason::ViewerQuit;
            }
            if app.stop_count >= 3 {
                break ExitReason::ForcedExit;
            }
        }
        if last_read.elapsed() >= opts.refresh {
            vm = view_model(source, opts);
            last_read = Instant::now();
            dirty = true;
        }
        if controls.take_winch() || last_full.elapsed() >= opts.full_redraw {
            if term.clear().is_err() {
                break ExitReason::Hangup;
            }
            last_full = Instant::now();
            dirty = true;
        }
        if let Ok(a) = terminal_area()
            && a != area
        {
            if term.resize(a).is_err() {
                break ExitReason::Hangup;
            }
            area = a;
            dirty = true;
        }
        // ages and the countdown move every second
        if last_draw.is_none_or(|d| d.elapsed() >= Duration::from_secs(1)) {
            dirty = true;
        }
        if dirty {
            let ctx = Ctx {
                theme,
                utf8,
                app: &app,
                now_s: now,
            };
            if term.draw(|f| draw(f, &vm, &ctx)).is_err() {
                break ExitReason::Hangup;
            }
            last_draw = Some(Instant::now());
            dirty = false;
        }
        match event::poll(Duration::from_millis(250)) {
            Ok(true) => match event::read() {
                Ok(Event::Key(k))
                    if matches!(k.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
                {
                    match app.on_key(k, &vm, source, controls, now_s(opts)) {
                        Action::Quit(r) => break r,
                        Action::Redraw => dirty = true,
                        Action::None => {}
                    }
                }
                Ok(Event::Resize(_, _)) => dirty = true,
                Ok(_) => {}
                Err(_) => break ExitReason::Hangup,
            },
            Ok(false) => {}
            Err(_) => break ExitReason::Hangup,
        }
    };
    drop(term);
    drop(guard);
    Ok((reason, vm, app))
}

/// Plain mode loop: lines to `out` until the viewer quits or the campaign ends.
pub fn run_plain(
    source: &dyn ProgressSource,
    controls: &dyn Controls,
    opts: &DashboardOptions,
    out: &mut dyn Write,
) -> ExitReason {
    let mut app = App::new(controls.hosts_campaign(), controls.grace_seconds());
    let mut printer = PlainPrinter::new();
    let mut last_read: Option<Instant> = None;
    loop {
        let now = now_s(opts);
        app.observe_stops(controls.stop_count(), now);
        if controls.campaign_done() {
            let vm = view_model(source, opts);
            let _ = printer.print(out, &vm, &app, now, 0.0);
            return finish(controls, &app);
        }
        if !controls.hosts_campaign() && app.stop_count > 0 {
            return ExitReason::ViewerQuit;
        }
        if app.stop_count >= 3 {
            return ExitReason::ForcedExit;
        }
        if last_read.is_none_or(|t| t.elapsed() >= opts.refresh) {
            let vm = view_model(source, opts);
            if printer
                .print(out, &vm, &app, now, opts.plain_interval.as_secs_f64())
                .is_err()
            {
                return ExitReason::Hangup;
            }
            last_read = Some(Instant::now());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// True when the dashboard should draw (stdout is a terminal, TERM is not dumb, no hang-up, not `--plain`).
pub fn wants_tui(opts: &DashboardOptions, controls: &dyn Controls) -> bool {
    !opts.plain
        && io::stdout().is_terminal()
        && std::env::var("TERM").ok().as_deref() != Some("dumb")
        && !controls.hangup_seen()
}
