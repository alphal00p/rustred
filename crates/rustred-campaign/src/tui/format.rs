//! Number and text formatters of the dashboard.
//!
//! `clean` (control characters stripped: no ANSI injection from file contents) and the byte / duration helpers follow
//! `rustred-app/src/cli/shards/monitor.rs` (b9f06de7, `clean`, `count`, `decimal`, `duration`, `bytes`), copied and
//! adapted: thousands separators, binary gigabytes written `G`, compact durations (`1d04h`, `5m12s`).

/// Strips control characters (escape sequences in file contents must never reach the terminal) and bounds the length.
pub fn clean(value: &str) -> String {
    value
        .chars()
        .filter(|c| !c.is_control())
        .take(1024)
        .collect()
}

/// `1234567` -> `1,234,567`.
pub fn thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Optional count: `?` when unknown.
pub fn count(v: Option<u64>) -> String {
    v.map_or_else(|| "?".into(), thousands)
}

/// Optional count shown as `-` when unknown (table cells).
pub fn cell(v: Option<u64>) -> String {
    v.map_or_else(|| "-".into(), thousands)
}

/// Count in a table cell; zero shown as `0`, unknown `-`.
pub fn cell_u(v: u64) -> String {
    thousands(v)
}

/// Binary gigabytes with one decimal (`9.8`), `?` when unknown.
pub fn gib(v: Option<u64>) -> String {
    match v {
        None => "?".into(),
        Some(b) => format!("{:.1}", b as f64 / (1u64 << 30) as f64),
    }
}

/// Bytes, compact: `512M`, `1.5G`, `2.0T`.
pub fn bytes(v: Option<u64>) -> String {
    let Some(b) = v else { return "?".into() };
    let f = b as f64;
    const K: f64 = 1024.0;
    if f >= K * K * K * K {
        format!("{:.1}T", f / (K * K * K * K))
    } else if f >= K * K * K {
        format!("{:.1}G", f / (K * K * K))
    } else if f >= K * K {
        format!("{:.0}M", f / (K * K))
    } else if f >= K {
        format!("{:.0}K", f / K)
    } else {
        format!("{b}B")
    }
}

/// Share in percent with one decimal, `-` when unknown.
pub fn percent(v: Option<f64>) -> String {
    match v {
        Some(x) if x.is_finite() => format!("{:.1}%", x * 100.0),
        _ => "-".into(),
    }
}

/// CPU-hours from seconds with one decimal.
pub fn cpu_hours(seconds: Option<f64>) -> String {
    match seconds {
        Some(s) if s.is_finite() && s >= 0.0 => format!("{:.1}", s / 3600.0),
        _ => "?".into(),
    }
}

/// Compact duration: `42s`, `5m12s`, `3h04m`, `1d04h`; `?` when unknown or negative.
pub fn dur(seconds: Option<f64>) -> String {
    let Some(s) = seconds.filter(|s| s.is_finite() && *s >= 0.0) else {
        return "?".into();
    };
    let s = s.round() as u64;
    if s < 60 {
        format!("{s}s")
    } else if s < 3600 {
        format!("{}m{:02}s", s / 60, s % 60)
    } else if s < 86_400 {
        format!("{}h{:02}m", s / 3600, (s / 60) % 60)
    } else {
        format!("{}d{:02}h", s / 86_400, (s / 3600) % 24)
    }
}

/// Rough duration for estimates (one significant unit): `40s`, `12m`, `2.6h`, `3.1d`.
pub fn rough(seconds: f64) -> String {
    if !seconds.is_finite() || seconds < 0.0 {
        return "?".into();
    }
    if seconds < 90.0 {
        format!("{}s", seconds.round() as u64)
    } else if seconds < 90.0 * 60.0 {
        format!("{}m", (seconds / 60.0).round() as u64)
    } else if seconds < 48.0 * 3600.0 {
        format!("{:.1}h", seconds / 3600.0)
    } else {
        format!("{:.1}d", seconds / 86_400.0)
    }
}

/// `mm:ss` countdown.
pub fn countdown(seconds: f64) -> String {
    let s = seconds.max(0.0).ceil() as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

/// Clock time of an ISO stamp (`2026-11-13T23:00:19.500Z` -> `23:00:19`); the input unchanged if it is not ISO.
pub fn hms(iso: &str) -> String {
    match iso.get(11..19) {
        Some(t) if iso.len() >= 20 && iso.as_bytes().get(10) == Some(&b'T') => t.to_string(),
        _ => clean(iso),
    }
}

/// Truncates to `width` display cells (ASCII-safe; one char = one cell for the strings we produce), adding `~`
/// when cut.
pub fn fit(s: &str, width: usize) -> String {
    let n = s.chars().count();
    if n <= width {
        s.to_string()
    } else if width == 0 {
        String::new()
    } else {
        let mut t: String = s.chars().take(width - 1).collect();
        t.push('~');
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formatters() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1000), "1,000");
        assert_eq!(thousands(1234567), "1,234,567");
        assert_eq!(count(None), "?");
        assert_eq!(gib(Some(32212254720)), "30.0");
        assert_eq!(bytes(Some(3221225472)), "3.0G");
        assert_eq!(percent(Some(0.5)), "50.0%");
        assert_eq!(percent(Some(f64::NAN)), "-");
        assert_eq!(dur(Some(42.0)), "42s");
        assert_eq!(dur(Some(312.0)), "5m12s");
        assert_eq!(dur(Some(101520.0)), "1d04h");
        assert_eq!(dur(Some(-1.0)), "?");
        assert_eq!(rough(9360.0), "2.6h");
        assert_eq!(countdown(107.0), "1:47");
        assert_eq!(hms("2026-11-13T23:00:19.500Z"), "23:00:19");
        assert_eq!(hms("junk"), "junk");
        assert_eq!(fit("abcdef", 4), "abc~");
        assert_eq!(clean("a\u{1b}[31mb"), "a[31mb");
    }
}
