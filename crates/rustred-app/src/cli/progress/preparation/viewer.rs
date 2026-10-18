use std::ffi::OsString;
use std::fs::File;
use std::io::{self, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crossterm::{
    cursor, execute, style,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::{Terminal, TerminalOptions, Viewport};

use super::{
    model::{MAX_STATUS_BYTES, Status},
    render,
};
use crate::cli::args::ArgError;
use crate::cli::error::CliError;

struct Options {
    snapshot: PathBuf,
    once: bool,
    json: bool,
}

fn parse(arguments: Vec<OsString>) -> Result<Options, CliError> {
    let mut snapshot = None;
    let mut once = false;
    let mut json = false;
    let mut arguments = arguments.into_iter();
    while let Some(option) = arguments.next() {
        let option = option.into_string().map_err(ArgError::NonUtf8Option)?;
        match option.as_str() {
            "--snapshot" => {
                if snapshot.is_some() {
                    return Err(ArgError::DuplicateOption("--snapshot").into());
                }
                let value = arguments
                    .next()
                    .ok_or(ArgError::MissingValue("--snapshot"))?;
                if value.is_empty() {
                    return Err(ArgError::EmptyPath.into());
                }
                snapshot = Some(PathBuf::from(value));
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
        snapshot: snapshot.ok_or(ArgError::MissingRequiredOption("--snapshot"))?,
        once,
        json,
    })
}

fn read(path: &Path) -> Result<Status, CliError> {
    let file = File::open(path).map_err(|error| {
        CliError::InputIo(format!(
            "open preparation snapshot {}: {error}",
            path.display()
        ))
    })?;
    let mut bytes = Vec::new();
    file.take(MAX_STATUS_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            CliError::InputIo(format!(
                "read preparation snapshot {}: {error}",
                path.display()
            ))
        })?;
    Status::parse(&bytes).map_err(CliError::Input)
}

fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |age| age.as_secs_f64())
}

/// A write-only, full-screen consumer: no cursor-position query or stdin read.
/// The scheduler intentionally starts us in a separate session with /dev/null
/// stdin. Inline viewports require a DSR reply and cannot work reliably there.
/// NO_COLOR disables color, not the aligned table itself.
struct Dashboard {
    terminal: Terminal<CrosstermBackend<io::Stderr>>,
    color: bool,
    area: Rect,
}

fn terminal_area() -> io::Result<Rect> {
    // Crossterm's size() may inspect stdout or /dev/tty. Neither need exist for
    // this detached, stderr-only viewer. Query the actual output descriptor.
    #[cfg(unix)]
    let (width, height) = {
        use std::os::fd::AsRawFd;
        let mut size = libc::winsize {
            ws_row: 0,
            ws_col: 0,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        // SAFETY: size is writable and correctly sized for TIOCGWINSZ; the
        // borrowed stderr descriptor remains open for the whole call.
        if unsafe { libc::ioctl(io::stderr().as_raw_fd(), libc::TIOCGWINSZ, &mut size) } != 0 {
            return Err(io::Error::last_os_error());
        }
        (size.ws_col, size.ws_row)
    };
    #[cfg(not(unix))]
    let (width, height) = crossterm::terminal::size()?;
    if width == 0 || height == 0 {
        return Err(io::Error::other("terminal reports an empty viewport"));
    }
    Ok(Rect::new(0, 0, width, height))
}

impl Dashboard {
    fn new(color: bool) -> io::Result<Self> {
        let area = terminal_area()?;
        let mut terminal = Terminal::with_options(
            CrosstermBackend::new(io::stderr()),
            TerminalOptions {
                viewport: Viewport::Fixed(area),
            },
        )?;
        if let Err(error) = execute!(terminal.backend_mut(), EnterAlternateScreen, cursor::Hide) {
            let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen, cursor::Show);
            return Err(error);
        }
        if let Err(error) = terminal.clear() {
            let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen, cursor::Show);
            return Err(error);
        }
        Ok(Self {
            terminal,
            color,
            area,
        })
    }
    fn draw(&mut self, status: &Status) -> io::Result<()> {
        let area = terminal_area()?;
        if area != self.area {
            self.terminal.resize(area)?;
            self.area = area;
        }
        self.terminal
            .draw(|frame| render::render(frame, status, now(), self.color))?;
        Ok(())
    }
}
impl Drop for Dashboard {
    fn drop(&mut self) {
        let _ = self.terminal.show_cursor();
        let _ = execute!(
            self.terminal.backend_mut(),
            style::ResetColor,
            cursor::Show,
            LeaveAlternateScreen
        );
        let _ = self.terminal.backend_mut().flush();
    }
}

struct Signals(Vec<signal_hook::SigId>);
impl Signals {
    fn register(cancel: &Arc<AtomicBool>) -> Result<Self, CliError> {
        let mut owned = Self(Vec::new());
        for signal in [signal_hook::consts::SIGINT, signal_hook::consts::SIGTERM] {
            owned.0.push(
                signal_hook::flag::register(signal, Arc::clone(cancel)).map_err(|error| {
                    CliError::InputIo(format!("install preparation viewer handler: {error}"))
                })?,
            );
        }
        Ok(owned)
    }
}
impl Drop for Signals {
    fn drop(&mut self) {
        for id in &self.0 {
            signal_hook::low_level::unregister(*id);
        }
    }
}

pub(crate) fn run(arguments: Vec<OsString>) -> Result<(), CliError> {
    let options = parse(arguments)?;
    let mut status = read(&options.snapshot)?;
    status.expire_native(now());
    if options.once || options.json {
        let mut output = io::stdout().lock();
        if options.json {
            serde_json::to_writer(&mut output, &status)
                .map_err(|error| CliError::OutputIo(error.to_string()))?;
            writeln!(output).map_err(|error| CliError::OutputIo(error.to_string()))?;
        } else {
            writeln!(output, "{}", render::plain(&status, now(), true))
                .map_err(|error| CliError::OutputIo(error.to_string()))?;
        }
        return output
            .flush()
            .map_err(|error| CliError::OutputIo(error.to_string()));
    }
    let cancel = Arc::new(AtomicBool::new(false));
    let _signals = Signals::register(&cancel)?;
    let tty = io::stderr().is_terminal() && std::env::var("TERM").ok().as_deref() != Some("dumb");
    let mut dashboard = if tty {
        match Dashboard::new(std::env::var_os("NO_COLOR").is_none()) {
            Ok(dashboard) => Some(dashboard),
            Err(error) => {
                eprintln!("Preparation TUI unavailable ({error}); using plain progress.");
                None
            }
        }
    } else {
        None
    };
    let mut printed = None;
    let mut previous_state = String::new();
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Ok(());
        }
        if let Some(terminal) = &mut dashboard {
            if let Err(error) = terminal.draw(&status) {
                dashboard = None;
                eprintln!("Preparation TUI stopped ({error}); using plain progress.");
            }
        }
        if dashboard.is_none()
            && (previous_state != status.state
                || printed.is_none_or(|at: Instant| at.elapsed() >= Duration::from_secs(5)))
        {
            writeln!(
                io::stderr().lock(),
                "{}",
                render::plain(&status, now(), false)
            )
            .map_err(|error| CliError::OutputIo(error.to_string()))?;
            printed = Some(Instant::now());
        }
        previous_state.clone_from(&status.state);
        if status.terminal() {
            // Leave one useful final record in normal scrollback after restoring
            // the caller's terminal. The alternate screen itself is transient.
            if dashboard.take().is_some() {
                writeln!(
                    io::stderr().lock(),
                    "{}",
                    render::plain(&status, now(), false)
                )
                .map_err(|error| CliError::OutputIo(error.to_string()))?;
            }
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(250));
        // Atomic publication makes partial JSON unlikely, but a failed read is
        // still a viewer error, never a scheduler/campaign state transition.
        status = read(&options.snapshot)?;
        status.expire_native(now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn viewer_options_require_exact_read_only_source() {
        assert!(parse(vec![]).is_err());
        assert!(
            parse(
                ["--snapshot", "x", "--once", "--json"]
                    .map(OsString::from)
                    .to_vec()
            )
            .is_err()
        );
        assert!(parse(["--snapshot", "x", "--force"].map(OsString::from).to_vec()).is_err());
        let options = parse(["--snapshot", "x", "--once"].map(OsString::from).to_vec()).unwrap();
        assert!(options.once);
        assert_eq!(options.snapshot, PathBuf::from("x"));
    }
}
