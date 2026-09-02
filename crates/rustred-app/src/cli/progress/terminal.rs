use std::io::{self, Write};

#[cfg(target_os = "linux")]
use std::fs;

use crossterm::{cursor, execute, style};
use ratatui::backend::CrosstermBackend;
#[cfg(test)]
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::Paragraph;
use ratatui::{Terminal, TerminalOptions, Viewport};

use super::model::{CampaignPhase, DashboardState};
use super::render::{DASHBOARD_HEIGHT, render_dashboard_for_width};

/// Resize-aware inline Ratatui viewport with idempotent best-effort cleanup.
///
/// The alternate screen is deliberately not used: the final status remains
/// visible in scrollback. Ratatui owns buffer diffs and cursor placement, so
/// terminal wrapping never feeds manual cursor-up arithmetic.
pub(super) struct TerminalSession<W: Write> {
    terminal: Terminal<CrosstermBackend<W>>,
    closed: bool,
}

impl<W: Write> TerminalSession<W> {
    pub(super) fn try_new(writer: W) -> io::Result<Self> {
        Self::try_with_viewport(writer, Viewport::Inline(DASHBOARD_HEIGHT))
    }

    #[cfg(test)]
    pub(super) fn try_new_fixed(writer: W, width: u16) -> io::Result<Self> {
        Self::try_with_viewport(
            writer,
            Viewport::Fixed(Rect::new(0, 0, width, DASHBOARD_HEIGHT)),
        )
    }

    fn try_with_viewport(writer: W, viewport: Viewport) -> io::Result<Self> {
        let backend = CrosstermBackend::new(writer);
        let terminal = Terminal::with_options(backend, TerminalOptions { viewport })?;
        let mut session = Self {
            terminal,
            closed: false,
        };
        session.terminal.hide_cursor()?;
        Ok(session)
    }

    pub(super) fn render(&mut self, state: &DashboardState, color: bool) -> io::Result<()> {
        self.terminal.draw(|frame| {
            let area = frame.area();
            let content = render_dashboard_for_width(state, area.width);
            let style = if color {
                Style::default()
                    .fg(phase_color(state.phase))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            frame.render_widget(Paragraph::new(content).style(style), area);
        })?;
        Ok(())
    }

    pub(super) fn close(&mut self) {
        if self.closed {
            return;
        }
        self.closed = true;
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

impl<W: Write> Drop for TerminalSession<W> {
    fn drop(&mut self) {
        self.close();
    }
}

fn phase_color(phase: CampaignPhase) -> Color {
    match phase {
        CampaignPhase::Starting => Color::Blue,
        CampaignPhase::Discovering => Color::Cyan,
        CampaignPhase::Closing | CampaignPhase::Closed => Color::Green,
        CampaignPhase::Bounded | CampaignPhase::Exhausted => Color::Yellow,
        CampaignPhase::Refinement => Color::Magenta,
        CampaignPhase::Failed => Color::Red,
    }
}

#[cfg(target_os = "linux")]
pub(super) fn resident_set_bytes() -> Option<u64> {
    let status = fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|line| line.starts_with("VmRSS:"))?;
    let mut fields = line.split_ascii_whitespace();
    (fields.next()? == "VmRSS:").then_some(())?;
    let kibibytes = fields.next()?.parse::<u64>().ok()?;
    (fields.next()? == "kB").then_some(())?;
    kibibytes.checked_mul(1_024)
}

#[cfg(not(target_os = "linux"))]
pub(super) fn resident_set_bytes() -> Option<u64> {
    None
}
