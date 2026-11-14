//! THE sanitiser of the dashboard (audit finding C1).
//!
//! Every string that reaches the terminal, from any source (progress files, ledger records, file names, error
//! messages, the spec, the environment), is a [`Clean`]. A `Clean` can only be made by [`Clean::new`], which replaces
//! control characters (C0 incl. ESC and BEL, DEL, C1 incl. U+009B CSI) and the format characters that can spoof
//! output (bidirectional overrides and isolates, zero-width characters, line / paragraph separators, tags) by the
//! visible placeholder [`PLACEHOLDER`], and bounds the length. The renderers take `Clean` only, so the compiler
//! enforces that no ratatui `Span` / `Line` / cell is built from an unsanitised `&str`. Widths are display widths
//! (unicode-width through ratatui) of the sanitised string.

use std::fmt;

use ratatui::text::Span;
use serde::{Serialize, Serializer};

/// Replaces every refused character (visible, one cell wide).
pub const PLACEHOLDER: char = '\u{FFFD}';

/// Longest text kept (characters); longer input is cut with `~`.
pub const MAX_CHARS: usize = 1024;

/// True for a character that must never reach a terminal: control characters (C0, DEL, C1) and format characters
/// that change how surrounding text is shown (bidi embeddings / overrides / isolates / marks, zero-width characters,
/// invisible operators, BOM, interlinear annotations, tags, line and paragraph separators, soft hyphen).
pub fn refused(c: char) -> bool {
    if c.is_control() {
        return true;
    }
    matches!(
        c as u32,
        0x00AD
            | 0x0600..=0x0605
            | 0x061C
            | 0x06DD
            | 0x070F
            | 0x08E2
            | 0x180E
            | 0x200B..=0x200F
            | 0x2028..=0x202E
            | 0x2060..=0x2064
            | 0x2066..=0x206F
            | 0xFEFF
            | 0xFFF9..=0xFFFB
            | 0x110BD
            | 0x110CD
            | 0x13430..=0x1343F
            | 0x1BCA0..=0x1BCA3
            | 0x1D173..=0x1D17A
            | 0xE0001
            | 0xE0020..=0xE007F
    )
}

/// Text that is safe to put on a terminal. Only [`Clean::new`] (the sanitiser) constructs one.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Clean(String);

impl Clean {
    /// The ONE sanitising function: refused characters become [`PLACEHOLDER`], at most [`MAX_CHARS`] characters.
    pub fn new(raw: &str) -> Clean {
        Clean::bounded(raw, MAX_CHARS)
    }

    /// As [`Clean::new`] with another length bound (plain-mode lines).
    pub fn bounded(raw: &str, max_chars: usize) -> Clean {
        let mut out = String::with_capacity(raw.len().min(max_chars * 4));
        for (n, c) in raw.chars().enumerate() {
            if n + 1 > max_chars {
                out.pop();
                out.push('~');
                break;
            }
            out.push(if refused(c) { PLACEHOLDER } else { c });
        }
        Clean(out)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Display width in terminal cells.
    pub fn width(&self) -> usize {
        self.0.chars().map(char_width).sum()
    }

    /// Cut to `w` cells, `~` marking a cut at the end.
    pub fn fit(&self, w: usize) -> Clean {
        if self.width() <= w {
            return self.clone();
        }
        if w == 0 {
            return Clean::default();
        }
        Clean(take_width(&self.0, w - 1) + "~")
    }

    /// Cut to `w` cells keeping both ends (`node.lo~name`-style middle cut, for names whose end matters).
    pub fn fit_mid(&self, w: usize) -> Clean {
        if self.width() <= w {
            return self.clone();
        }
        if w < 4 {
            return self.fit(w);
        }
        let tail_w = (w - 1) / 2;
        let head_w = w - 1 - tail_w;
        let head = take_width(&self.0, head_w);
        let rev: String = self.0.chars().rev().collect();
        let tail: String = take_width(&rev, tail_w).chars().rev().collect();
        Clean(format!("{head}~{tail}"))
    }

    /// Exactly `w` cells: cut (end) and padded on the right (left-aligned) or left (right-aligned).
    pub fn pad(&self, w: usize, right_align: bool) -> Clean {
        let s = self.fit(w);
        let fill = " ".repeat(w.saturating_sub(s.width()));
        Clean(if right_align {
            fill + &s.0
        } else {
            s.0 + &fill
        })
    }

    /// Appends sanitised text.
    pub fn push(&mut self, more: &str) {
        self.0.push_str(Clean::new(more).as_str());
    }
}

/// Shorthand: sanitise anything string-like.
pub fn cl(s: impl AsRef<str>) -> Clean {
    Clean::new(s.as_ref())
}

/// Optional raw text -> optional clean text.
pub fn opt(s: Option<&str>) -> Option<Clean> {
    s.map(Clean::new)
}

impl fmt::Display for Clean {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for Clean {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl PartialEq<str> for Clean {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<&str> for Clean {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl Serialize for Clean {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

/// Width of one character in cells (ratatui's unicode-width; refused characters never get here).
pub fn char_width(c: char) -> usize {
    let mut b = [0u8; 4];
    Span::raw(&*c.encode_utf8(&mut b)).width()
}

fn take_width(s: &str, w: usize) -> String {
    let mut used = 0;
    let mut out = String::new();
    for c in s.chars() {
        let cw = char_width(c);
        if used + cw > w {
            break;
        }
        used += cw;
        out.push(c);
    }
    out
}

/// Recursively sanitises every string and key of a JSON value (drill-down records read from the ledger).
pub fn clean_value(v: serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    match v {
        Value::String(s) => Value::String(Clean::new(&s).into_string()),
        Value::Array(a) => Value::Array(a.into_iter().map(clean_value).collect()),
        Value::Object(o) => Value::Object(
            o.into_iter()
                .map(|(k, x)| (Clean::new(&k).into_string(), clean_value(x)))
                .collect(),
        ),
        x => x,
    }
}

/// JSON text for a terminal or a pipe: serde_json escapes C0 controls, quotes and backslashes; this additionally
/// writes every other refused character (DEL, C1 such as U+009B, bidi / format characters) as a `\uXXXX` escape, so
/// that the text is plain printable JSON whatever the values contain. Refused characters can only occur inside
/// strings, where the escape is valid JSON.
pub fn json_text(v: &serde_json::Value, pretty: bool) -> String {
    let s = if pretty {
        serde_json::to_string_pretty(v)
    } else {
        serde_json::to_string(v)
    }
    .unwrap_or_else(|_| "null".into());
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if refused(c) && c != '\n' {
            let mut units = [0u16; 2];
            for u in c.encode_utf16(&mut units) {
                out.push_str(&format!("\\u{u:04x}"));
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_controls_bidi_and_format_characters() {
        let raw = "a\u{1b}[31mb\u{7}c\u{9b}2Jd\u{7f}e\u{202e}f\u{2066}g\u{200b}h\u{2028}i\u{feff}j\u{e0041}k\nl\tm";
        let c = Clean::new(raw);
        assert!(!c.as_str().chars().any(refused), "{c}");
        assert_eq!(c.as_str().matches(PLACEHOLDER).count(), 12);
        assert!(c.as_str().starts_with("a\u{FFFD}[31mb"));
        // idempotent
        assert_eq!(Clean::new(c.as_str()), c);
    }

    #[test]
    fn widths_cuts_and_bounds() {
        assert_eq!(Clean::new("abcdef").fit(4).as_str(), "abc~");
        assert_eq!(
            Clean::new("abcd.long-node-name").fit_mid(9).as_str(),
            "abcd~name"
        );
        assert_eq!(Clean::new("ab").pad(4, true).as_str(), "  ab");
        assert_eq!(Clean::new("日本").width(), 4);
        assert_eq!(Clean::new("日本語").fit(5).as_str(), "日本~");
        let long = "x".repeat(5000);
        assert_eq!(Clean::new(&long).as_str().chars().count(), MAX_CHARS);
    }

    #[test]
    fn json_text_escapes_c1_and_bidi() {
        let v = serde_json::json!({"a\u{9b}": "x\u{1b}]52;c;\u{7}\u{202e}y"});
        let t = json_text(&v, true);
        assert!(!t.chars().any(|c| refused(c) && c != '\n'), "{t}");
        assert!(
            t.contains("\\u009b") && t.contains("\\u001b") && t.contains("\\u202e"),
            "{t}"
        );
        let back: serde_json::Value = serde_json::from_str(&t).unwrap();
        assert_eq!(back, v);
    }
}
