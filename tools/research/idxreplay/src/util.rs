//! Small std-only helpers: thread CPU clock, RNG, statistics, JSON output.
use std::fmt::Write as _;

#[repr(C)]
struct Timespec {
    tv_sec: i64,
    tv_nsec: i64,
}
unsafe extern "C" {
    fn clock_gettime(clk: i32, tp: *mut Timespec) -> i32;
}
const CLOCK_THREAD_CPUTIME_ID: i32 = 3;

/// CPU nanoseconds consumed by the calling thread.
pub fn thread_cpu_ns() -> u64 {
    let mut ts = Timespec { tv_sec: 0, tv_nsec: 0 };
    // SAFETY: valid pointer to a properly laid out timespec.
    let rc = unsafe { clock_gettime(CLOCK_THREAD_CPUTIME_ID, &mut ts) };
    assert_eq!(rc, 0);
    ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64
}

pub struct Rng(pub u64);
impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    pub fn below(&mut self, m: usize) -> usize {
        (self.next() % m as u64) as usize
    }
}

/// Deterministic thinning: keep `id` iff its hash falls below `permille/1024`.
#[inline]
pub fn kept(id: u32, per1024: u64) -> bool {
    per1024 >= 1024 || (id as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 54 < per1024
}

pub fn quantiles(mut v: Vec<u64>) -> (u64, u64, u64, u64) {
    if v.is_empty() {
        return (0, 0, 0, 0);
    }
    v.sort_unstable();
    let k = v.len();
    (v[k / 2], v[k * 9 / 10], v[(k * 99 / 100).min(k - 1)], v[k - 1])
}

/// Least-squares slope and intercept of y on x with weights.
pub fn fit(xs: &[f64], ys: &[f64], ws: &[f64]) -> (f64, f64, f64) {
    let sw: f64 = ws.iter().sum();
    if sw == 0.0 || xs.len() < 2 {
        return (f64::NAN, f64::NAN, f64::NAN);
    }
    let mx = xs.iter().zip(ws).map(|(x, w)| x * w).sum::<f64>() / sw;
    let my = ys.iter().zip(ws).map(|(y, w)| y * w).sum::<f64>() / sw;
    let sxx: f64 = xs.iter().zip(ws).map(|(x, w)| w * (x - mx) * (x - mx)).sum();
    let sxy: f64 = xs.iter().zip(ys).zip(ws).map(|((x, y), w)| w * (x - mx) * (y - my)).sum();
    let syy: f64 = ys.iter().zip(ws).map(|(y, w)| w * (y - my) * (y - my)).sum();
    let b = sxy / sxx;
    let r2 = if syy > 0.0 { sxy * sxy / (sxx * syy) } else { f64::NAN };
    (b, my - b * mx, r2)
}

/// Minimal JSON object builder (flat key/value pairs).
pub struct Json(String);
impl Json {
    pub fn new() -> Self {
        Json(String::from("{"))
    }
    fn sep(&mut self) {
        if self.0.len() > 1 {
            self.0.push(',');
        }
    }
    pub fn s(mut self, k: &str, v: &str) -> Self {
        self.sep();
        let _ = write!(self.0, "\"{k}\":\"{}\"", v.replace('"', "'"));
        self
    }
    pub fn u(mut self, k: &str, v: u64) -> Self {
        self.sep();
        let _ = write!(self.0, "\"{k}\":{v}");
        self
    }
    pub fn f(mut self, k: &str, v: f64) -> Self {
        self.sep();
        if v.is_finite() {
            let _ = write!(self.0, "\"{k}\":{v:.6}");
        } else {
            let _ = write!(self.0, "\"{k}\":null");
        }
        self
    }
    pub fn raw(mut self, k: &str, v: &str) -> Self {
        self.sep();
        let _ = write!(self.0, "\"{k}\":{v}");
        self
    }
    pub fn done(mut self) -> String {
        self.0.push('}');
        self.0
    }
}

pub fn now() -> String {
    // Seconds since the epoch; wall-clock labels only.
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap();
    format!("{}", t.as_secs())
}
