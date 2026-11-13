//! Time source of the campaign: wall-clock UTC (for records and files) and a monotonic clock (for durations,
//! deadlines, cadences). Every time-dependent behaviour takes a `&dyn Clock`, so tests drive it with
//! [`ManualClock`] and get byte-identical ledgers and progress files.
//!
//! Wall time is carried as integer milliseconds since the Unix epoch ([`UtcMillis`]); it is formatted as
//! ISO-8601 with a `Z` suffix and millisecond precision (`2026-11-13T23:05:01.250Z`) without any date crate.

use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Milliseconds since 1970-01-01T00:00:00Z (may be negative for dates before the epoch; never produced here).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct UtcMillis(pub i64);

impl UtcMillis {
    /// ISO-8601 UTC, millisecond precision, `Z` suffix.
    pub fn to_iso(self) -> String {
        let ms = self.0.rem_euclid(1000);
        let secs = self.0.div_euclid(1000);
        let days = secs.div_euclid(86_400);
        let sod = secs.rem_euclid(86_400);
        let (y, m, d) = civil_from_days(days);
        format!(
            "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{ms:03}Z",
            sod / 3600,
            (sod / 60) % 60,
            sod % 60
        )
    }

    /// Parses the format written by [`UtcMillis::to_iso`] (also accepts no fraction and 1-9 fraction digits).
    pub fn parse_iso(text: &str) -> Option<UtcMillis> {
        let b = text.as_bytes();
        if b.len() < 20 || *b.last()? != b'Z' {
            return None;
        }
        let num = |r: std::ops::Range<usize>| -> Option<i64> { text.get(r)?.parse::<i64>().ok() };
        if b[4] != b'-' || b[7] != b'-' || b[10] != b'T' || b[13] != b':' || b[16] != b':' {
            return None;
        }
        let (y, mo, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
        let (h, mi, s) = (num(11..13)?, num(14..16)?, num(17..19)?);
        if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || s > 60 {
            return None;
        }
        let mut ms = 0i64;
        let rest = &text[19..text.len() - 1];
        if !rest.is_empty() {
            let frac = rest.strip_prefix('.')?;
            if frac.is_empty() || frac.len() > 9 || !frac.bytes().all(|c| c.is_ascii_digit()) {
                return None;
            }
            let mut digits = frac.to_string();
            digits.truncate(3);
            while digits.len() < 3 {
                digits.push('0');
            }
            ms = digits.parse().ok()?;
        }
        let days = days_from_civil(y, mo as u32, d as u32);
        Some(UtcMillis(((days * 86_400) + h * 3600 + mi * 60 + s) * 1000 + ms))
    }
}

impl std::fmt::Display for UtcMillis {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_iso())
    }
}

/// Howard Hinnant's `civil_from_days` (public domain algorithm).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = if m > 2 { m - 3 } else { m + 9 } as i64;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The campaign's time source.
pub trait Clock: Send + Sync {
    /// Wall-clock time (UTC) for records and file stamps. May jump (NTP); never used for durations.
    fn now_utc(&self) -> UtcMillis;
    /// Monotonic time since an arbitrary origin fixed for the clock's lifetime; used for durations and deadlines.
    fn monotonic(&self) -> Duration;
}

/// The real clock: `SystemTime` for UTC, `Instant` for monotonic time.
#[derive(Debug)]
pub struct SystemClock {
    origin: Instant,
}

impl SystemClock {
    pub fn new() -> Self {
        SystemClock { origin: Instant::now() }
    }
}

impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for SystemClock {
    fn now_utc(&self) -> UtcMillis {
        let d = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
        UtcMillis(d.as_millis() as i64)
    }
    fn monotonic(&self) -> Duration {
        self.origin.elapsed()
    }
}

/// A clock moved only by the test: `advance` moves both wall and monotonic time; `set_utc` moves the wall
/// clock alone (to model an NTP jump); `tick_ms` > 0 makes every `now_utc` call advance by that step
/// (records then get distinct, deterministic stamps without explicit `advance` calls).
#[derive(Debug)]
pub struct ManualClock {
    inner: Mutex<ManualInner>,
}

#[derive(Debug)]
struct ManualInner {
    utc: i64,
    mono: Duration,
    tick_ms: i64,
}

impl ManualClock {
    pub fn new(start: UtcMillis) -> Self {
        ManualClock { inner: Mutex::new(ManualInner { utc: start.0, mono: Duration::ZERO, tick_ms: 0 }) }
    }
    /// Starts at `iso` (panics on a malformed literal: test helper).
    pub fn at(iso: &str) -> Self {
        Self::new(UtcMillis::parse_iso(iso).expect("ManualClock::at: malformed ISO time"))
    }
    pub fn with_tick(self, tick_ms: i64) -> Self {
        self.inner.lock().unwrap().tick_ms = tick_ms;
        self
    }
    pub fn advance(&self, d: Duration) {
        let mut g = self.inner.lock().unwrap();
        g.utc += d.as_millis() as i64;
        g.mono += d;
    }
    pub fn set_utc(&self, t: UtcMillis) {
        self.inner.lock().unwrap().utc = t.0;
    }
}

impl Clock for ManualClock {
    fn now_utc(&self) -> UtcMillis {
        let mut g = self.inner.lock().unwrap();
        let t = g.utc;
        if g.tick_ms > 0 {
            let step = g.tick_ms;
            g.utc += step;
            g.mono += Duration::from_millis(step as u64);
        }
        UtcMillis(t)
    }
    fn monotonic(&self) -> Duration {
        self.inner.lock().unwrap().mono
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_round_trip_and_known_values() {
        assert_eq!(UtcMillis(0).to_iso(), "1970-01-01T00:00:00.000Z");
        let t = UtcMillis::parse_iso("2026-11-13T23:05:01.250Z").unwrap();
        assert_eq!(t.to_iso(), "2026-11-13T23:05:01.250Z");
        assert_eq!(UtcMillis::parse_iso("2000-02-29T12:00:00Z").unwrap().to_iso(), "2000-02-29T12:00:00.000Z");
        assert_eq!(UtcMillis(951_825_600_000).to_iso(), "2000-02-29T12:00:00.000Z");
        for x in [0i64, 1, 86_399_999, 1_700_000_000_123, 4_102_444_800_000, 1_234_567_890_999] {
            assert_eq!(UtcMillis::parse_iso(&UtcMillis(x).to_iso()), Some(UtcMillis(x)));
        }
        assert!(UtcMillis::parse_iso("2026-13-01T00:00:00Z").is_none());
        assert!(UtcMillis::parse_iso("garbage").is_none());
    }

    #[test]
    fn manual_clock_is_deterministic() {
        let c = ManualClock::at("2026-11-13T00:00:00Z").with_tick(5);
        assert_eq!(c.now_utc().to_iso(), "2026-11-13T00:00:00.000Z");
        assert_eq!(c.now_utc().to_iso(), "2026-11-13T00:00:00.005Z");
        c.advance(Duration::from_secs(60));
        assert_eq!(c.now_utc().to_iso(), "2026-11-13T00:01:00.010Z");
        assert_eq!(c.monotonic(), Duration::from_millis(60_015));
        c.set_utc(UtcMillis(0));
        assert_eq!(c.monotonic(), Duration::from_millis(60_015));
    }

    #[test]
    fn system_clock_is_sane() {
        let c = SystemClock::new();
        assert!(c.now_utc().0 > 1_700_000_000_000);
        let a = c.monotonic();
        assert!(c.monotonic() >= a);
    }
}
