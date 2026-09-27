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

fn parse_list(s: &str) -> Vec<usize> {
    let mut v = Vec::new();
    for part in s.trim().split(',').filter(|p| !p.is_empty()) {
        if let Some((a, b)) = part.split_once('-') {
            v.extend(a.parse::<usize>().unwrap()..=b.parse::<usize>().unwrap());
        } else {
            v.push(part.parse().unwrap());
        }
    }
    v
}

/// CPUs this process may run on, and their SMT siblings outside that set.
pub fn allowed_cpus() -> (Vec<usize>, Vec<usize>) {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let own = status
        .lines()
        .find_map(|l| l.strip_prefix("Cpus_allowed_list:"))
        .map(parse_list)
        .unwrap_or_default();
    let mut sib = Vec::new();
    for &c in &own {
        if let Ok(s) = std::fs::read_to_string(format!("/sys/devices/system/cpu/cpu{c}/topology/thread_siblings_list")) {
            for x in parse_list(&s) {
                if !own.contains(&x) && !sib.contains(&x) {
                    sib.push(x);
                }
            }
        }
    }
    (own, sib)
}

/// (busy, total) jiffies summed over `cpus` from /proc/stat.
pub fn cpu_jiffies(cpus: &[usize]) -> (u64, u64) {
    let stat = std::fs::read_to_string("/proc/stat").unwrap_or_default();
    let (mut busy, mut total) = (0u64, 0u64);
    for l in stat.lines() {
        let Some(rest) = l.strip_prefix("cpu") else { continue };
        let mut it = rest.split_whitespace();
        let Some(id) = it.next().and_then(|s| s.parse::<usize>().ok()) else { continue };
        if !cpus.contains(&id) {
            continue;
        }
        let v: Vec<u64> = it.map(|x| x.parse().unwrap_or(0)).collect();
        let t: u64 = v.iter().take(8).sum();
        let idle = v.get(3).copied().unwrap_or(0) + v.get(4).copied().unwrap_or(0);
        busy += t - idle;
        total += t;
    }
    (busy, total)
}

/// utime + stime of this process in jiffies.
pub fn self_jiffies() -> u64 {
    let s = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
    let after = s.rsplit_once(')').map(|x| x.1).unwrap_or("");
    let f: Vec<&str> = after.split_whitespace().collect();
    f.get(11).and_then(|x| x.parse::<u64>().ok()).unwrap_or(0) + f.get(12).and_then(|x| x.parse::<u64>().ok()).unwrap_or(0)
}

/// Foreign load monitor: busy share of the allowed CPUs not due to this
/// process, and busy share of their SMT siblings.
pub struct LoadMon {
    own: Vec<usize>,
    sib: Vec<usize>,
    a: (u64, u64),
    s: (u64, u64),
    me: u64,
}
impl LoadMon {
    pub fn start() -> Self {
        let (own, sib) = allowed_cpus();
        let a = cpu_jiffies(&own);
        let s = cpu_jiffies(&sib);
        LoadMon { own, sib, a, s, me: self_jiffies() }
    }
    /// (foreign busy fraction on own CPUs, busy fraction on SMT siblings)
    pub fn stop(&self) -> (f64, f64) {
        let a = cpu_jiffies(&self.own);
        let s = cpu_jiffies(&self.sib);
        let me = self_jiffies() - self.me;
        let da = (a.0 - self.a.0) as f64;
        let ta = (a.1 - self.a.1).max(1) as f64;
        let ds = (s.0 - self.s.0) as f64;
        let ts = (s.1 - self.s.1).max(1) as f64;
        (((da - me as f64) / ta).max(0.0), if self.sib.is_empty() { 0.0 } else { ds / ts })
    }
}
