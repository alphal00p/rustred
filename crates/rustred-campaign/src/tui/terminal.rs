//! The live loop: alternate screen + fixed viewport sized by `TIOCGWINSZ` before every draw, raw mode for keys,
//! terminal restored on every exit path (guard + panic hook), event-driven with a 250 ms tick.
//!
//! All reading happens on the background [`Reader`] (audit M1): this loop only polls keys, takes the newest
//! snapshot, builds the view-model (pure, no I/O) and draws. The loop core [`tui_loop`] is generic over the ratatui
//! backend and the key [`Input`], so it is tested with a `TestBackend` and scripted keys.
//!
//! Terminal setup and resize handling follow rustred-app `cli/progress/preparation/viewer.rs` (b9f06de7:
//! `terminal_area` by TIOCGWINSZ on the output descriptor, `Viewport::Fixed`, alternate screen, restore in `Drop`),
//! copied and adapted (stdout, raw mode for keys, panic hook).

use std::io::{self, IsTerminal, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Once};
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyEventKind};
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{cursor, execute, style};
use ratatui::backend::{Backend, CrosstermBackend};
use ratatui::layout::Rect;
use ratatui::{Terminal, TerminalOptions, Viewport};

use super::app::{Action, App, ExitReason};
use super::controls::Controls;
use super::model::{self, JobsOrigin, ViewModel};
use super::plain::PlainPrinter;
use super::reader::{Reader, Reply};
use super::render::{Ctx, draw};
use super::source::ProgressSource;
use super::theme::Theme;
use super::{DashboardOptions, build_options, now_s};

static ACTIVE: AtomicBool = AtomicBool::new(false);
static HOOK: Once = Once::new();

/// Restores the terminal (idempotent; safe from a panic hook). Keys typed into the dashboard that it has not read
/// (a Ctrl-C pressed during the restore) are discarded, so they do not reach the shell (audit m8).
pub fn restore_terminal() {
    if ACTIVE.swap(false, Ordering::SeqCst) {
        let _ = crossterm::terminal::disable_raw_mode();
        let mut out = io::stdout();
        let _ = execute!(out, style::ResetColor, cursor::Show, LeaveAlternateScreen);
        let _ = out.flush();
        // SAFETY: tcflush on standard input with a valid queue selector; failure (not a terminal) is ignored.
        unsafe {
            libc::tcflush(0, libc::TCIFLUSH);
        }
    }
}

/// True while the TUI owns the terminal.
pub fn terminal_active() -> bool {
    ACTIVE.load(Ordering::SeqCst)
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

/// Where keys come from (the terminal, or a script in tests).
pub trait Input {
    /// Waits at most `timeout` for one event.
    fn poll(&mut self, timeout: Duration) -> io::Result<Option<Event>>;
}

/// Keys of the real terminal (crossterm), with a LOST-TERMINAL check of our own (procfix3, gate-B M2).
///
/// Why: when the terminal disappears (pty master closed: ssh drop, terminal window closed without a SIGHUP to us), the
/// descriptor reports readiness for ever (POLLHUP, reads of 0 bytes / EIO) and crossterm's `event::read` loops inside
/// without returning: the dashboard thread spun at 100 % of a CPU and the process never exited. Before any crossterm
/// call this input polls the SAME descriptor crossterm reads (stdin when it is a terminal, else `/dev/tty`) and turns
/// POLLHUP / POLLERR / POLLNVAL into an error: the loop ends (`ExitReason::Hangup`), the terminal is restored as far as
/// possible, and the supervisor continues in plain mode (its lines go to its log file).
pub struct TerminalInput {
    fd: Option<std::os::fd::OwnedFd>,
}

impl Default for TerminalInput {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminalInput {
    pub fn new() -> Self {
        use std::os::fd::{FromRawFd, OwnedFd};
        // SAFETY: isatty on a constant descriptor number.
        if unsafe { libc::isatty(libc::STDIN_FILENO) } == 1 {
            return TerminalInput { fd: None };
        }
        // SAFETY: open of a constant C string; the result is a fresh descriptor or -1.
        let fd = unsafe {
            libc::open(
                c"/dev/tty".as_ptr(),
                libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NONBLOCK | libc::O_NOCTTY,
            )
        };
        TerminalInput {
            // SAFETY: fd is a fresh descriptor owned by nobody else.
            fd: (fd >= 0).then(|| unsafe { OwnedFd::from_raw_fd(fd) }),
        }
    }

    fn raw(&self) -> i32 {
        use std::os::fd::AsRawFd;
        self.fd
            .as_ref()
            .map_or(libc::STDIN_FILENO, |f| f.as_raw_fd())
    }

    /// Ready to read (Ok(true)), nothing within `timeout` (Ok(false)), or the terminal is gone (Err).
    fn wait_readable(&self, timeout: Duration) -> io::Result<bool> {
        let mut p = libc::pollfd {
            fd: self.raw(),
            events: libc::POLLIN,
            revents: 0,
        };
        let ms = timeout.as_millis().min(i32::MAX as u128) as i32;
        // SAFETY: one valid pollfd for the duration of the call.
        let r = unsafe { libc::poll(&mut p, 1, ms) };
        if r < 0 {
            let e = io::Error::last_os_error();
            return if e.kind() == io::ErrorKind::Interrupted {
                Ok(false)
            } else {
                Err(e)
            };
        }
        if p.revents & (libc::POLLHUP | libc::POLLERR | libc::POLLNVAL) != 0 {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "the terminal hung up (POLLHUP / POLLERR)",
            ));
        }
        Ok(r > 0 && p.revents & libc::POLLIN != 0)
    }
}

impl Input for TerminalInput {
    fn poll(&mut self, timeout: Duration) -> io::Result<Option<Event>> {
        // A hung-up terminal first (crossterm would spin on it), then anything crossterm already buffered.
        self.wait_readable(Duration::ZERO)?;
        if event::poll(Duration::ZERO)? {
            return event::read().map(Some);
        }
        if self.wait_readable(timeout)? && event::poll(Duration::ZERO)? {
            return event::read().map(Some);
        }
        Ok(None)
    }
}

/// The view-model of the newest snapshot, now (or the READING screen before the first one). Pure: no I/O.
pub fn view_model_of(reader: &Reader, opts: &DashboardOptions) -> ViewModel {
    let o = build_options(opts);
    match reader.latest() {
        None => model::waiting("the campaign directory", reader.waited().as_secs_f64(), o),
        Some(snap) => {
            let mut vm = model::build(&snap.raw, o);
            if vm.jobs.list.is_none()
                && let Some(rows) = &snap.ledger_jobs
            {
                vm.jobs.list = Some(model::ledger_jobs(rows));
                vm.jobs.list_origin = JobsOrigin::LedgerPartial;
            }
            // the age of what is shown: since the read finished, or since the running read started if longer
            let age = snap
                .done
                .elapsed()
                .max(reader.reading_for().unwrap_or_default());
            vm.data_age_s = Some(age.as_secs_f64());
            vm
        }
    }
}

/// The loop core: keys, snapshots, drawing. `size` gives the terminal size before each draw (None: keep the old
/// one); `owns_terminal` makes the loop stop when someone else restored the terminal (a panic hook of another
/// thread, audit m8).
#[allow(clippy::too_many_arguments)]
pub fn tui_loop<B: Backend>(
    term: &mut Terminal<B>,
    input: &mut dyn Input,
    size: &dyn Fn() -> Option<Rect>,
    reader: &Reader,
    controls: &dyn Controls,
    opts: &DashboardOptions,
    theme: Theme,
    utf8: bool,
    owns_terminal: bool,
) -> (ExitReason, ViewModel, App) {
    let mut app = App::new(controls.hosts_campaign(), controls.grace_seconds());
    let mut vm = view_model_of(reader, opts);
    let mut area = size().unwrap_or_else(|| term.get_frame().area());
    let mut last_full = Instant::now();
    let mut last_draw: Option<Instant> = None;
    let mut last_snap = reader.latest();
    let mut dirty = true;
    let reason = loop {
        let now = now_s(opts);
        if owns_terminal && !terminal_active() {
            break ExitReason::TerminalError(
                "the terminal was restored by a panic elsewhere in the process".into(),
            );
        }
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
        for r in reader.replies() {
            match r {
                Reply::Items(n, v) => app.apply_items(&n, v),
                Reply::Detail(i, v) => app.apply_detail(&i, v),
            }
            dirty = true;
        }
        let snap = reader.latest();
        let new_snap = match (&snap, &last_snap) {
            (Some(a), Some(b)) => !Arc::ptr_eq(a, b),
            (a, b) => a.is_some() != b.is_some(),
        };
        if new_snap {
            last_snap = snap;
            dirty = true;
        }
        if controls.take_winch() || last_full.elapsed() >= opts.full_redraw {
            if term.clear().is_err() {
                break ExitReason::Hangup;
            }
            last_full = Instant::now();
            dirty = true;
        }
        if let Some(a) = size()
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
            vm = view_model_of(reader, opts);
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
        match input.poll(Duration::from_millis(250)) {
            Ok(Some(Event::Key(k)))
                if matches!(k.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
            {
                match app.on_key(k, &vm, reader, controls, now_s(opts)) {
                    Action::Quit(r) => break r,
                    Action::Redraw => dirty = true,
                    Action::None => {}
                }
            }
            Ok(Some(Event::Resize(_, _))) => dirty = true,
            Ok(_) => {}
            Err(_) => break ExitReason::Hangup,
        }
    };
    (reason, vm, app)
}

/// The TUI on the process's terminal. Returns the exit reason and the last view-model (printed to the normal screen
/// by the caller). The reader thread is told to stop on return (never joined: a hung read cannot hold the exit).
pub fn run_tui(
    source: Arc<dyn ProgressSource>,
    controls: &dyn Controls,
    opts: &DashboardOptions,
    color: bool,
    utf8: bool,
) -> Result<(ExitReason, ViewModel, App), io::Error> {
    let (mut term, guard, _) = setup()?;
    let reader = Reader::spawn(source, opts.refresh);
    let size = || terminal_area().ok();
    let r = tui_loop(
        &mut term,
        &mut TerminalInput::new(),
        &size,
        &reader,
        controls,
        opts,
        Theme { color },
        utf8,
        true,
    );
    reader.stop();
    drop(term);
    drop(guard);
    Ok(r)
}

/// Plain mode loop: lines to `out` until the viewer quits or the campaign ends. Reading happens on the background
/// reader; this loop checks the controls every 100 ms whatever the file system does.
pub fn run_plain(
    source: Arc<dyn ProgressSource>,
    controls: &dyn Controls,
    opts: &DashboardOptions,
    out: &mut dyn Write,
) -> ExitReason {
    let reader = Reader::spawn(source, opts.refresh);
    let mut app = App::new(controls.hosts_campaign(), controls.grace_seconds());
    let mut printer = PlainPrinter::new();
    let mut last = None;
    let reason = loop {
        let now = now_s(opts);
        app.observe_stops(controls.stop_count(), now);
        if controls.campaign_done() {
            if reader.latest().is_some() {
                let vm = view_model_of(&reader, opts);
                let _ = printer.print(out, &vm, &app, now, 0.0);
            }
            break finish(controls, &app);
        }
        if !controls.hosts_campaign() && app.stop_count > 0 {
            break ExitReason::ViewerQuit;
        }
        if app.stop_count >= 3 {
            break ExitReason::ForcedExit;
        }
        let snap = reader.latest();
        let new_snap = match (&snap, &last) {
            (Some(a), Some(b)) => !Arc::ptr_eq(a, b),
            (a, b) => a.is_some() != b.is_some(),
        };
        if new_snap {
            last = snap;
            let vm = view_model_of(&reader, opts);
            if printer
                .print(out, &vm, &app, now, opts.plain_interval.as_secs_f64())
                .is_err()
            {
                break ExitReason::Hangup;
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    reader.stop();
    reason
}

/// True when the dashboard should draw (stdout is a terminal, TERM is not dumb, no hang-up, not `--plain`).
pub fn wants_tui(opts: &DashboardOptions, controls: &dyn Controls) -> bool {
    !opts.plain
        && io::stdout().is_terminal()
        && std::env::var("TERM").ok().as_deref() != Some("dumb")
        && !controls.hangup_seen()
}
