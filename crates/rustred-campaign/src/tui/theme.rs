//! Colour roles. Only NAMED ANSI colours (the terminal's own palette: readable on dark and light backgrounds), no
//! background colours, no RGB. With colour off every role is the default style except the selection (reverse video,
//! which is not a colour) and bold headers.

use ratatui::style::{Color, Modifier, Style};

/// What a piece of text means.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Role {
    #[default]
    Normal,
    Label,
    Title,
    Ok,
    Boxed,
    Terminal,
    Running,
    Queued,
    Warn,
    Fail,
    Estimate,
    Selected,
    BannerWarn,
    BannerFail,
}

/// Resolved colour choice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Theme {
    pub color: bool,
}

impl Theme {
    pub fn style(self, r: Role) -> Style {
        let base = Style::default();
        if !self.color {
            return match r {
                Role::Title | Role::Label => base.add_modifier(Modifier::BOLD),
                Role::Selected | Role::BannerWarn | Role::BannerFail => {
                    base.add_modifier(Modifier::REVERSED)
                }
                _ => base,
            };
        }
        match r {
            Role::Normal => base,
            Role::Label => base.add_modifier(Modifier::BOLD),
            Role::Title => base.fg(Color::Cyan).add_modifier(Modifier::BOLD),
            Role::Ok => base.fg(Color::Green),
            Role::Boxed => base.fg(Color::Blue),
            Role::Terminal => base.fg(Color::Magenta),
            Role::Running => base.fg(Color::Cyan).add_modifier(Modifier::BOLD),
            Role::Queued => base.add_modifier(Modifier::DIM),
            Role::Warn => base.fg(Color::Yellow),
            Role::Fail => base.fg(Color::Red),
            Role::Estimate => base.add_modifier(Modifier::DIM),
            Role::Selected => base.add_modifier(Modifier::REVERSED),
            Role::BannerWarn => base
                .fg(Color::Yellow)
                .add_modifier(Modifier::REVERSED | Modifier::BOLD),
            Role::BannerFail => base
                .fg(Color::Red)
                .add_modifier(Modifier::REVERSED | Modifier::BOLD),
        }
    }
}

/// Colour choice of the command line.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ColorChoice {
    #[default]
    Auto,
    Always,
    Never,
}

impl ColorChoice {
    /// `auto`: colour when the output is a terminal, `NO_COLOR` is unset or empty, and `TERM` is not `dumb`.
    pub fn resolve(self, is_tty: bool, no_color: Option<&str>, term: Option<&str>) -> bool {
        match self {
            ColorChoice::Always => true,
            ColorChoice::Never => false,
            ColorChoice::Auto => {
                is_tty && no_color.is_none_or(str::is_empty) && term != Some("dumb")
            }
        }
    }
}

/// Box drawing only with a UTF-8 locale (first set of LC_ALL, LC_CTYPE, LANG).
pub fn utf8_locale(lc_all: Option<&str>, lc_ctype: Option<&str>, lang: Option<&str>) -> bool {
    let v = [lc_all, lc_ctype, lang]
        .into_iter()
        .flatten()
        .find(|s| !s.is_empty())
        .unwrap_or("");
    let v = v.to_ascii_lowercase();
    v.contains("utf-8") || v.contains("utf8")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colour_resolution() {
        assert!(ColorChoice::Auto.resolve(true, None, Some("xterm-256color")));
        assert!(ColorChoice::Auto.resolve(true, Some(""), None));
        assert!(!ColorChoice::Auto.resolve(true, Some("1"), None));
        assert!(!ColorChoice::Auto.resolve(true, None, Some("dumb")));
        assert!(!ColorChoice::Auto.resolve(false, None, None));
        assert!(ColorChoice::Always.resolve(false, Some("1"), None));
        assert!(!ColorChoice::Never.resolve(true, None, None));
        assert!(utf8_locale(None, None, Some("en_US.UTF-8")));
        assert!(!utf8_locale(Some("C"), None, Some("en_US.UTF-8")));
        assert_eq!(Theme { color: false }.style(Role::Fail), Style::default());
        assert_eq!(Theme { color: true }.style(Role::Boxed).fg, Some(Color::Blue));
    }
}
