//! Print the same Ratatui frame in normal scrollback after leaving the live
//! alternate screen. Sequential output needs no cursor-position query, stdin,
//! absolute screen coordinates or another interactive terminal session.
use std::io::{self, Write};

use crossterm::{queue, style};
use ratatui::{
    Terminal,
    backend::TestBackend,
    style::{Color, Modifier},
    text::Span,
};

use super::{model::Status, render};

fn color(value: Color) -> style::Color {
    match value {
        Color::Reset => style::Color::Reset,
        Color::Black => style::Color::Black,
        Color::Red => style::Color::DarkRed,
        Color::Green => style::Color::DarkGreen,
        Color::Yellow => style::Color::DarkYellow,
        Color::Blue => style::Color::DarkBlue,
        Color::Magenta => style::Color::DarkMagenta,
        Color::Cyan => style::Color::DarkCyan,
        Color::Gray => style::Color::Grey,
        Color::DarkGray => style::Color::DarkGrey,
        Color::LightRed => style::Color::Red,
        Color::LightGreen => style::Color::Green,
        Color::LightYellow => style::Color::Yellow,
        Color::LightBlue => style::Color::Blue,
        Color::LightMagenta => style::Color::Magenta,
        Color::LightCyan => style::Color::Cyan,
        Color::White => style::Color::White,
        Color::Rgb(r, g, b) => style::Color::Rgb { r, g, b },
        Color::Indexed(index) => style::Color::AnsiValue(index),
    }
}

pub(super) fn write(
    output: &mut impl Write,
    status: &Status,
    now: f64,
    width: u16,
    height: u16,
    colored: bool,
) -> io::Result<()> {
    let height = height
        .min(u16::try_from(status.jobs.len().saturating_add(11)).unwrap_or(u16::MAX))
        .max(1);
    let mut terminal = Terminal::new(TestBackend::new(width.max(1), height))?;
    terminal.draw(|frame| render::render(frame, status, now, colored))?;
    let buffer = terminal.backend().buffer();
    let mut previous = None;
    for row in buffer.content.chunks(width.max(1) as usize) {
        let mut column = 0;
        while column < row.len() {
            let cell = &row[column];
            if colored {
                let identity = (cell.fg, cell.bg, cell.modifier);
                if previous != Some(identity) {
                    let mut content = style::ContentStyle::new();
                    content.foreground_color = Some(color(cell.fg));
                    content.background_color = Some(color(cell.bg));
                    for (flag, attribute) in [
                        (Modifier::BOLD, style::Attribute::Bold),
                        (Modifier::DIM, style::Attribute::Dim),
                        (Modifier::ITALIC, style::Attribute::Italic),
                        (Modifier::UNDERLINED, style::Attribute::Underlined),
                        (Modifier::REVERSED, style::Attribute::Reverse),
                        (Modifier::CROSSED_OUT, style::Attribute::CrossedOut),
                    ] {
                        if cell.modifier.contains(flag) {
                            content.attributes.set(attribute);
                        }
                    }
                    queue!(
                        output,
                        style::SetAttribute(style::Attribute::Reset),
                        style::SetStyle(content)
                    )?;
                    previous = Some(identity);
                }
            }
            output.write_all(cell.symbol().as_bytes())?;
            column += Span::raw(cell.symbol()).width().max(1);
        }
        // CR clears pending terminal autowrap at the last physical column.
        if colored {
            queue!(
                output,
                style::ResetColor,
                style::SetAttribute(style::Attribute::Reset)
            )?;
            previous = None;
        }
        output.write_all(b"\r\n")?;
    }
    output.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primary_screen_uses_the_native_table_without_cursor_queries() {
        let mut value = super::super::model::tests::fixture();
        value["state"] = "failed".into();
        value["stop_reason"] = "native_job_failed:exit_-9".into();
        let mut status = Status::parse(&serde_json::to_vec(&value).unwrap()).unwrap();
        status.finish_observations();
        for colored in [false, true] {
            let mut output = Vec::new();
            write(&mut output, &status, 200., 160, 32, colored).unwrap();
            let text = String::from_utf8(output).unwrap();
            for marker in ["╭", "╰", "PARENTS", "Parent", "native_job_failed:exit_-9"] {
                assert!(text.contains(marker), "{text}");
            }
            assert!(!text.contains("\x1b[6n"));
            assert!(!text.contains("RustRed preparation |"));
            if !colored {
                assert!(!text.contains('\x1b'));
            }
        }
    }
}
