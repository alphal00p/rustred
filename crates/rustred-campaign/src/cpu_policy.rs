//! CPU policies (checker comment C1: site rules are a small policy trait with two implementations, not code inside
//! the supervisor). A policy hands out CPU LEASES: never the same CPU twice at once, within a core budget, released
//! explicitly or on drop. It also says when NOT to launch (pause flags).
//!
//! - [`PlainPolicy`]: a CPU list or a count from the command line, intersected with this process's affinity mask;
//!   idle CPUs first. For the user's own run on any machine.
//! - [`SitePolicy`]: this host's conventions reproduced exactly (`SCRATCHPAD/audit-closure/tools/job.sh`,
//!   `pick_free.py`, `t162-box/tools/pick.py`): a CPU is eligible when it is idle (>= `idle_min` over the last window),
//!   not excluded by a range of the config, not in a held list (JSON files named in the config), not claimed in the CPU
//!   ledger by another owner (claims minus RELEASED over the last `ledger_scan_lines` lines), not pinned by a foreign
//!   process (optional), and its `locks/cpu.<n>` flock can be taken. The flock is held for the life of the lease;
//!   a claim line is appended to the ledger before the lease is handed out and a RELEASED line when it ends, in the
//!   existing format with the campaign's tag. The exclusions and held lists are DATA of the site-policy file
//!   (`SiteConfig`), not constants here.

use crate::resources::{IdleTracker, allowed_cpus, format_cpu_list, parse_cpu_list};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::fd::{AsRawFd, RawFd};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Debug)]
pub enum PolicyError {
    Config(String),
    Io { what: String, error: io::Error },
}

impl fmt::Display for PolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PolicyError::Config(s) => write!(f, "CPU policy configuration: {s}"),
            PolicyError::Io { what, error } => write!(f, "{what}: {error}"),
        }
    }
}

impl std::error::Error for PolicyError {}

fn io_err(what: impl Into<String>) -> impl FnOnce(io::Error) -> PolicyError {
    let what = what.into();
    move |error| PolicyError::Io { what, error }
}

/// Result of an acquisition attempt.
#[derive(Debug)]
pub enum Acquire {
    Granted(CpuLease),
    /// Not now; the reason is for the dashboard ("core budget 48 reached", "PAUSE present", ...).
    NotNow(String),
}

impl Acquire {
    pub fn granted(self) -> Option<CpuLease> {
        match self {
            Acquire::Granted(l) => Some(l),
            Acquire::NotNow(_) => None,
        }
    }
}

/// A CPU policy.
pub trait CpuPolicy {
    /// Try to lease `n` CPUs for the job `label` (all or nothing).
    fn acquire(&mut self, n: usize, label: &str) -> Result<Acquire, PolicyError>;
    /// Cores the campaign may hold at once.
    fn budget(&self) -> usize;
    /// Cores leased now.
    fn in_use(&self) -> usize;
    /// Conditions under which nothing may be launched now (pause flags present), as readable reasons.
    fn launch_blockers(&self) -> Vec<String>;
    /// Take a /proc/stat sample for the idleness window (cheap; call once per control-loop tick).
    fn refresh(&mut self) -> io::Result<()>;
}

#[derive(Debug, Default)]
struct Book {
    in_use: BTreeSet<usize>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

#[derive(Debug, Clone)]
struct LedgerWriter {
    path: PathBuf,
    tag: String,
}

impl LedgerWriter {
    fn append(&self, line: &str) -> io::Result<()> {
        // One write of one line on an O_APPEND descriptor: concurrent writers do not interleave within it.
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        f.write_all(format!("{line}\n").as_bytes())
    }
    fn claim_line(&self, cpus: &[usize], label: &str) -> String {
        let (word, list) = cpu_word(cpus);
        format!(
            "{} ({}): + {word} {list} (flock) -> {}. Released at its end.",
            utc_now(),
            self.tag,
            sanitize(label)
        )
    }
    fn release_line(&self, cpus: &[usize], label: &str) -> String {
        let (_, list) = cpu_word(cpus);
        format!(
            "{} ({}): RELEASED {list} ({} ended)",
            utc_now(),
            self.tag,
            sanitize(label)
        )
    }
}

fn cpu_word(cpus: &[usize]) -> (&'static str, String) {
    let list = cpus
        .iter()
        .map(|c| c.to_string())
        .collect::<Vec<_>>()
        .join(",");
    (if cpus.len() == 1 { "CPU" } else { "CPUs" }, list)
}

/// Labels go into a prose ledger read by regexes: keep them on one line, without parentheses or semicolons.
fn sanitize(label: &str) -> String {
    label
        .chars()
        .map(|c| {
            if matches!(c, '(' | ')' | ';' | '\n' | '\r') {
                '_'
            } else {
                c
            }
        })
        .collect()
}

/// A lease of CPUs. Released on drop (errors ignored) or by [`CpuLease::release`] (errors reported).
pub struct CpuLease {
    cpus: Vec<usize>,
    label: String,
    book: Arc<Mutex<Book>>,
    locks: Vec<File>,
    ledger: Option<LedgerWriter>,
    released: bool,
}

impl fmt::Debug for CpuLease {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CpuLease")
            .field("cpus", &self.cpus)
            .field("label", &self.label)
            .finish()
    }
}

impl CpuLease {
    pub fn cpus(&self) -> &[usize] {
        &self.cpus
    }

    /// The flock descriptors (site policy). Pass them in `LaunchSpec::inherit_fds` to keep the claim alive for as long
    /// as the job's processes live, even if the supervisor dies (the job.sh / flock(1) behaviour).
    pub fn lock_fds(&self) -> Vec<RawFd> {
        self.locks.iter().map(AsRawFd::as_raw_fd).collect()
    }

    pub fn release(mut self) -> io::Result<()> {
        self.do_release()
    }

    fn do_release(&mut self) -> io::Result<()> {
        if self.released {
            return Ok(());
        }
        self.released = true;
        // RELEASED first, while the flocks are still held: a picker that sees the line and tries the flock finds it
        // either still held (skips) or free (takes it); it can never take a CPU whose claim it still believes live.
        let r = match &self.ledger {
            Some(w) => w.append(&w.release_line(&self.cpus, &self.label)),
            None => Ok(()),
        };
        self.locks.clear(); // closing the descriptors drops the flocks
        let mut b = lock(&self.book);
        for c in &self.cpus {
            b.in_use.remove(c);
        }
        r
    }
}

impl Drop for CpuLease {
    fn drop(&mut self) {
        let _ = self.do_release();
    }
}

fn read_pause(files: &[PathBuf]) -> Vec<String> {
    files
        .iter()
        .filter(|p| p.exists())
        .map(|p| format!("pause flag {} present", p.display()))
        .collect()
}

// ------------------------------------------------------------------------------------------------ plain policy

/// CPUs from the command line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CpuSpec {
    /// Exactly these CPUs (intersected with the affinity mask).
    List(BTreeSet<usize>),
    /// Any `n` CPUs of the affinity mask at a time.
    Count(usize),
}

impl CpuSpec {
    /// "0-7,12" or "count:16" / "16" (a bare number is a count).
    pub fn parse(s: &str) -> Result<Self, String> {
        let s = s.trim();
        if let Some(n) = s.strip_prefix("count:") {
            return n
                .trim()
                .parse()
                .map(CpuSpec::Count)
                .map_err(|_| format!("bad count {n:?}"));
        }
        if !s.contains([',', '-'])
            && let Ok(n) = s.parse::<usize>()
        {
            return Ok(CpuSpec::Count(n));
        }
        parse_cpu_list(s).map(CpuSpec::List)
    }
}

pub struct PlainPolicy {
    candidates: BTreeSet<usize>,
    budget: usize,
    book: Arc<Mutex<Book>>,
    idle: IdleTracker,
    pause_files: Vec<PathBuf>,
    /// CPUs of the list that the affinity mask removed (reported by the pre-flight).
    pub dropped: BTreeSet<usize>,
}

impl PlainPolicy {
    /// `budget`: None = the number of candidates (List) or the count (Count).
    pub fn new(spec: &CpuSpec, budget: Option<usize>) -> Result<Self, PolicyError> {
        let affinity = allowed_cpus().map_err(io_err("sched_getaffinity"))?;
        Self::with_affinity(spec, budget, &affinity)
    }

    /// As `new`, with an explicit affinity mask (tests).
    pub fn with_affinity(
        spec: &CpuSpec,
        budget: Option<usize>,
        affinity: &BTreeSet<usize>,
    ) -> Result<Self, PolicyError> {
        let (candidates, dropped, default_budget) = match spec {
            CpuSpec::List(l) => {
                let c: BTreeSet<usize> = l.intersection(affinity).copied().collect();
                let d: BTreeSet<usize> = l.difference(affinity).copied().collect();
                let n = c.len();
                (c, d, n)
            }
            CpuSpec::Count(n) => (affinity.clone(), BTreeSet::new(), *n),
        };
        if candidates.is_empty() {
            return Err(PolicyError::Config(
                "no CPU of the list is in this process's affinity mask".into(),
            ));
        }
        let budget = budget.unwrap_or(default_budget).min(candidates.len());
        if budget == 0 {
            return Err(PolicyError::Config("core budget is 0".into()));
        }
        Ok(Self {
            candidates,
            budget,
            book: Arc::default(),
            idle: IdleTracker::new(),
            pause_files: Vec::new(),
            dropped,
        })
    }

    pub fn with_pause_files(mut self, files: Vec<PathBuf>) -> Self {
        self.pause_files = files;
        self
    }

    pub fn candidates(&self) -> &BTreeSet<usize> {
        &self.candidates
    }

    /// Idleness used for ordering (tests; normally `refresh`).
    pub fn idle_tracker_mut(&mut self) -> &mut IdleTracker {
        &mut self.idle
    }
}

impl CpuPolicy for PlainPolicy {
    fn acquire(&mut self, n: usize, label: &str) -> Result<Acquire, PolicyError> {
        if n == 0 {
            return Err(PolicyError::Config("acquire of 0 CPUs".into()));
        }
        let blockers = self.launch_blockers();
        if !blockers.is_empty() {
            return Ok(Acquire::NotNow(blockers.join("; ")));
        }
        let mut book = lock(&self.book);
        if book.in_use.len() + n > self.budget {
            return Ok(Acquire::NotNow(format!(
                "core budget {} reached ({} in use)",
                self.budget,
                book.in_use.len()
            )));
        }
        let mut free: Vec<usize> = self.candidates.difference(&book.in_use).copied().collect();
        if free.len() < n {
            return Ok(Acquire::NotNow(format!(
                "only {} of {n} CPUs free",
                free.len()
            )));
        }
        // Idle first (unknown idleness ranks last), then by number.
        free.sort_by(|a, b| {
            let ia = self.idle.idle(*a).unwrap_or(-1.0);
            let ib = self.idle.idle(*b).unwrap_or(-1.0);
            ib.total_cmp(&ia).then(a.cmp(b))
        });
        let cpus: Vec<usize> = free.into_iter().take(n).collect();
        book.in_use.extend(cpus.iter().copied());
        drop(book);
        Ok(Acquire::Granted(CpuLease {
            cpus,
            label: label.to_string(),
            book: Arc::clone(&self.book),
            locks: Vec::new(),
            ledger: None,
            released: false,
        }))
    }

    fn budget(&self) -> usize {
        self.budget
    }

    fn in_use(&self) -> usize {
        lock(&self.book).in_use.len()
    }

    fn launch_blockers(&self) -> Vec<String> {
        read_pause(&self.pause_files)
    }

    fn refresh(&mut self) -> io::Result<()> {
        self.idle.refresh(Duration::from_millis(900))
    }
}

// ------------------------------------------------------------------------------------------------ site policy

pub const SITE_SCHEMA: &str = "rustred.campaign.site-policy.v1";

/// A JSON file holding CPU numbers at `key`: a dotted path where a segment `name[]` maps over an array, e.g. `held`,
/// `all_cpus_listed`, `entries[].cpus`. Leaves may be integers, CPU-list strings ("128-151,160"), or arrays of these.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeldFile {
    pub path: PathBuf,
    pub key: String,
    /// Missing file: ignored when false (default, as pick_free.py), refusal when true. A file that exists but does not
    /// parse is ALWAYS a refusal (fail closed).
    #[serde(default)]
    pub required: bool,
}

/// The site-policy file (JSON). `interfaces/process-layer.md` shows this host's values.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SiteConfig {
    pub schema: String,
    /// Owner tag of the ledger lines, e.g. "rr-campaign". No parentheses or commas.
    pub tag: String,
    pub lock_dir: PathBuf,
    /// Lock file name with `{n}` for the CPU number.
    #[serde(default = "default_lock_name")]
    pub lock_name: String,
    pub ledger_file: PathBuf,
    #[serde(default = "default_scan_lines")]
    pub ledger_scan_lines: usize,
    /// CPU list to choose from (default: this process's affinity mask); always intersected with the mask.
    #[serde(default)]
    pub candidates: Option<String>,
    /// CPU lists never used.
    #[serde(default)]
    pub exclude: Vec<String>,
    #[serde(default)]
    pub held_files: Vec<HeldFile>,
    /// Existence of any of these = do not launch.
    #[serde(default)]
    pub pause_files: Vec<PathBuf>,
    /// Minimum idle fraction over the last window (pick.py: 0.85). 0 accepts CPUs whose idleness is not measured yet.
    #[serde(default = "default_idle_min")]
    pub idle_min: f64,
    /// Core budget (the allotment, e.g. 48).
    pub max_cores: usize,
    /// Exclude CPUs inside a restricted affinity of a foreign process (pick.py `pinned()`); the control loop feeds
    /// them from the sampler (`set_foreign_pinned`). Until it has, nothing is granted.
    #[serde(default)]
    pub respect_foreign_pinning: bool,
    /// Ledger claims under our own tag are not exclusions (pick_free.py ignores its own tag; the flock decides).
    #[serde(default = "default_true")]
    pub ignore_own_tag_claims: bool,
}

fn default_lock_name() -> String {
    "cpu.{n}".into()
}
fn default_scan_lines() -> usize {
    1500
}
fn default_idle_min() -> f64 {
    0.85
}
fn default_true() -> bool {
    true
}

impl SiteConfig {
    pub fn read(path: &Path) -> Result<Self, PolicyError> {
        let text = fs::read_to_string(path).map_err(io_err(format!("read {}", path.display())))?;
        let c: SiteConfig = serde_json::from_str(&text)
            .map_err(|e| PolicyError::Config(format!("{}: {e}", path.display())))?;
        c.validate()?;
        Ok(c)
    }

    pub fn validate(&self) -> Result<(), PolicyError> {
        if self.schema != SITE_SCHEMA {
            return Err(PolicyError::Config(format!(
                "schema {:?}, expected {SITE_SCHEMA}",
                self.schema
            )));
        }
        if self.tag.is_empty() || self.tag.contains(['(', ')', ',', '\n']) {
            return Err(PolicyError::Config(format!("bad tag {:?}", self.tag)));
        }
        if !self.lock_name.contains("{n}") {
            return Err(PolicyError::Config("lock_name must contain {n}".into()));
        }
        if self.max_cores == 0 {
            return Err(PolicyError::Config("max_cores is 0".into()));
        }
        if !(0.0..=1.0).contains(&self.idle_min) {
            return Err(PolicyError::Config("idle_min outside [0, 1]".into()));
        }
        for e in &self.exclude {
            parse_cpu_list(e).map_err(|m| PolicyError::Config(format!("exclude: {m}")))?;
        }
        if let Some(c) = &self.candidates {
            parse_cpu_list(c).map_err(|m| PolicyError::Config(format!("candidates: {m}")))?;
        }
        Ok(())
    }
}

/// CPU numbers at a dotted key of a JSON value.
pub fn cpus_at_key(v: &serde_json::Value, key: &str) -> Result<BTreeSet<usize>, String> {
    let mut nodes = vec![v];
    for seg in key.split('.').filter(|s| !s.is_empty()) {
        let (name, map) = match seg.strip_suffix("[]") {
            Some(n) => (n, true),
            None => (seg, false),
        };
        let mut next = Vec::new();
        for n in nodes {
            let child = n.get(name).ok_or_else(|| format!("key {name:?} missing"))?;
            if map {
                next.extend(
                    child
                        .as_array()
                        .ok_or_else(|| format!("{name:?} is not an array"))?
                        .iter(),
                );
            } else {
                next.push(child);
            }
        }
        nodes = next;
    }
    let mut out = BTreeSet::new();
    fn leaf(v: &serde_json::Value, out: &mut BTreeSet<usize>) -> Result<(), String> {
        match v {
            serde_json::Value::Number(n) => {
                out.insert(n.as_u64().ok_or_else(|| format!("bad CPU number {n}"))? as usize);
            }
            serde_json::Value::String(s) => out.extend(parse_cpu_list(s)?),
            serde_json::Value::Array(a) => {
                for x in a {
                    leaf(x, out)?;
                }
            }
            other => return Err(format!("unexpected value {other}")),
        }
        Ok(())
    }
    for n in nodes {
        leaf(n, &mut out)?;
    }
    Ok(out)
}

/// pick_free.py's `parse`: tokens split on spaces and commas; "a-b" ranges and numbers; anything else ignored.
fn parse_lenient(s: &str) -> BTreeSet<usize> {
    let mut out = BTreeSet::new();
    for part in s.split([' ', ',']).filter(|p| !p.is_empty()) {
        if let Some((a, b)) = part.split_once('-') {
            if let (Ok(a), Ok(b)) = (a.parse::<usize>(), b.parse::<usize>())
                && a <= b
                && b - a < 4096
            {
                out.extend(a..=b);
            }
        } else if let Ok(n) = part.parse::<usize>() {
            out.insert(n);
        }
    }
    out
}

fn is_cls(c: char) -> bool {
    c.is_ascii_digit() || c == ',' || c == '-' || c == ' '
}

/// `\+ CPUs? ([\d,\- ]+?)(?:\(| ->|;|$)` of pick_free.py, every match in the line.
fn claims_in(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = line;
    while let Some(i) = rest.find("+ CPU") {
        let mut s = &rest[i + 5..];
        if let Some(t) = s.strip_prefix('s') {
            s = t;
        }
        let Some(t) = s.strip_prefix(' ') else {
            rest = &rest[i + 5..];
            continue;
        };
        // Lazy run of class characters up to the first terminator: '(' / ';' / end, or " ->".
        let mut found = None;
        for (j, ch) in t.char_indices() {
            if t[j..].starts_with(" ->") && j > 0 {
                found = Some(j);
                break;
            }
            if ch == '(' || ch == ';' {
                if j > 0 {
                    found = Some(j);
                }
                break;
            }
            if !is_cls(ch) {
                break;
            }
            if j + ch.len_utf8() == t.len() {
                found = Some(t.len());
            }
        }
        if let Some(j) = found {
            out.push(t[..j].to_string());
        }
        rest = &rest[i + 5..];
    }
    out
}

/// `RELEASED ([\d,\- ]+)` of pick_free.py, every match in the line.
fn releases_in(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = line;
    while let Some(i) = rest.find("RELEASED ") {
        let t = &rest[i + 9..];
        let end = t.find(|c: char| !is_cls(c)).unwrap_or(t.len());
        if end > 0 {
            out.push(t[..end].to_string());
        }
        rest = &rest[i + 9..];
    }
    out
}

/// Live claims per owner from ledger lines (pick_free.py semantics: `(\S+) \(([^)]+)\)` at the start, owner = text up
/// to the first comma; claims add, RELEASED removes, in line order).
pub fn ledger_claims<'a>(
    lines: impl IntoIterator<Item = &'a str>,
) -> BTreeMap<String, BTreeSet<usize>> {
    let mut held: BTreeMap<String, BTreeSet<usize>> = BTreeMap::new();
    for line in lines {
        let Some(sp) = line.find(' ') else { continue };
        if sp == 0 || !line[sp + 1..].starts_with('(') {
            continue;
        }
        let after = &line[sp + 2..];
        let Some(close) = after.find(')') else {
            continue;
        };
        if close == 0 {
            continue;
        }
        let who = after[..close]
            .split(',')
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        for c in claims_in(line) {
            held.entry(who.clone())
                .or_default()
                .extend(parse_lenient(&c));
        }
        for r in releases_in(line) {
            let set = held.entry(who.clone()).or_default();
            for c in parse_lenient(&r) {
                set.remove(&c);
            }
        }
    }
    held
}

/// Last `n` lines of a file, reading only its tail (the ledger grows without bound).
fn tail_lines(path: &Path, n: usize) -> io::Result<Vec<String>> {
    let mut f = match File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let len = f.metadata()?.len();
    let want = (n as u64).saturating_mul(512).max(64 * 1024);
    let start = len.saturating_sub(want);
    f.seek(SeekFrom::Start(start))?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf)?;
    let text = String::from_utf8_lossy(&buf);
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    if start > 0 && !lines.is_empty() {
        lines.remove(0); // partial first line
    }
    let skip = lines.len().saturating_sub(n);
    Ok(lines.split_off(skip))
}

pub struct SitePolicy {
    config: SiteConfig,
    candidates: BTreeSet<usize>,
    exclude: BTreeSet<usize>,
    book: Arc<Mutex<Book>>,
    idle: IdleTracker,
    foreign_pinned: Option<BTreeSet<usize>>,
    extra_pause: Vec<PathBuf>,
    ledger: LedgerWriter,
    /// SMT siblings per CPU when the "sibling quiet" rule is on (coord-site-policy.md rule 2c).
    siblings: Option<BTreeMap<usize, Vec<usize>>>,
}

impl SitePolicy {
    pub fn new(config: SiteConfig) -> Result<Self, PolicyError> {
        let affinity = allowed_cpus().map_err(io_err("sched_getaffinity"))?;
        Self::with_affinity(config, &affinity)
    }

    pub fn with_affinity(
        config: SiteConfig,
        affinity: &BTreeSet<usize>,
    ) -> Result<Self, PolicyError> {
        config.validate()?;
        let candidates: BTreeSet<usize> = match &config.candidates {
            Some(c) => parse_cpu_list(c)
                .map_err(PolicyError::Config)?
                .intersection(affinity)
                .copied()
                .collect(),
            None => affinity.clone(),
        };
        let mut exclude = BTreeSet::new();
        for e in &config.exclude {
            exclude.extend(parse_cpu_list(e).map_err(PolicyError::Config)?);
        }
        if candidates.is_empty() {
            return Err(PolicyError::Config(
                "no candidate CPU in this process's affinity mask".into(),
            ));
        }
        fs::create_dir_all(&config.lock_dir)
            .map_err(io_err(format!("create {}", config.lock_dir.display())))?;
        let ledger = LedgerWriter {
            path: config.ledger_file.clone(),
            tag: config.tag.clone(),
        };
        Ok(Self {
            config,
            candidates,
            exclude,
            book: Arc::default(),
            idle: IdleTracker::new(),
            foreign_pinned: None,
            extra_pause: Vec::new(),
            ledger,
            siblings: None,
        })
    }

    /// An optional second pause flag (e.g. the campaign's own), besides the config's.
    pub fn add_pause_file(&mut self, p: PathBuf) {
        self.extra_pause.push(p);
    }

    /// CPUs pinned by foreign processes (from `Snapshot::foreign_pinned`).
    pub fn set_foreign_pinned(&mut self, cpus: BTreeSet<usize>) {
        self.foreign_pinned = Some(cpus);
    }

    pub fn idle_tracker_mut(&mut self) -> &mut IdleTracker {
        &mut self.idle
    }

    pub fn config(&self) -> &SiteConfig {
        &self.config
    }

    /// Turn on the coordinator's "SMT sibling not busy" rule (coord-site-policy.md rule 2c): a CPU is eligible only if
    /// every sibling is idle by the same `idle_min` measure (or itself leased by us). `siblings` maps a CPU to its
    /// siblings; [`read_smt_siblings`] reads them from sysfs. Additive (lane I-SUP, 2026-11-14).
    pub fn set_sibling_quiet(&mut self, siblings: BTreeMap<usize, Vec<usize>>) {
        self.siblings = Some(siblings);
    }

    /// Restart reconciliation (audit-proc-A M6; coord-site-policy.md test 3): for every CPU our tag still claims in
    /// the CPU ledger whose flock can be taken now (no live holder: the claimant died), append the missing RELEASED
    /// line while holding the flock, then drop it. Claims whose flock is held (a live job of ours or a sibling
    /// supervisor with the same tag) are left alone. Returns the CPUs released. Additive (lane I-SUP, 2026-11-14).
    pub fn reconcile_own_claims(&mut self) -> Result<Vec<usize>, PolicyError> {
        let lines = tail_lines(&self.config.ledger_file, self.config.ledger_scan_lines).map_err(
            io_err(format!("read {}", self.config.ledger_file.display())),
        )?;
        let claims = ledger_claims(lines.iter().map(String::as_str));
        let Some(own) = claims.get(&self.config.tag) else {
            return Ok(Vec::new());
        };
        let in_use = lock(&self.book).in_use.clone();
        let mut held: Vec<(usize, File)> = Vec::new();
        for &c in own {
            if in_use.contains(&c) {
                continue;
            }
            let path = self.lock_path(c);
            let f = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(&path)
                .map_err(io_err(format!("open {}", path.display())))?;
            if f.try_lock().is_ok() {
                held.push((c, f));
            }
        }
        let cpus: Vec<usize> = held.iter().map(|x| x.0).collect();
        if !cpus.is_empty() {
            self.ledger
                .append(
                    &self
                        .ledger
                        .release_line(&cpus, "stale claims of a dead supervisor"),
                )
                .map_err(io_err(format!(
                    "append to {}",
                    self.config.ledger_file.display()
                )))?;
        }
        drop(held);
        Ok(cpus)
    }

    fn lock_path(&self, cpu: usize) -> PathBuf {
        self.config
            .lock_dir
            .join(self.config.lock_name.replace("{n}", &cpu.to_string()))
    }

    /// Everything that excludes a CPU now, or Err(reason) when a source cannot be trusted (fail closed).
    fn exclusions(&self) -> Result<BTreeSet<usize>, String> {
        let mut ex = self.exclude.clone();
        for h in &self.config.held_files {
            match fs::read_to_string(&h.path) {
                Ok(text) => {
                    let v: serde_json::Value = serde_json::from_str(&text)
                        .map_err(|e| format!("held file {} unreadable: {e}", h.path.display()))?;
                    ex.extend(cpus_at_key(&v, &h.key).map_err(|e| {
                        format!("held file {} key {}: {e}", h.path.display(), h.key)
                    })?);
                }
                Err(e) if e.kind() == io::ErrorKind::NotFound && !h.required => {}
                Err(e) => return Err(format!("held file {}: {e}", h.path.display())),
            }
        }
        let lines = tail_lines(&self.config.ledger_file, self.config.ledger_scan_lines)
            .map_err(|e| format!("ledger {}: {e}", self.config.ledger_file.display()))?;
        for (who, set) in ledger_claims(lines.iter().map(String::as_str)) {
            if !(self.config.ignore_own_tag_claims && who == self.config.tag) {
                ex.extend(set);
            }
        }
        if self.config.respect_foreign_pinning {
            match &self.foreign_pinned {
                Some(p) => ex.extend(p.iter().copied()),
                None => return Err("foreign CPU pinning not scanned yet".into()),
            }
        }
        Ok(ex)
    }
}

impl CpuPolicy for SitePolicy {
    fn acquire(&mut self, n: usize, label: &str) -> Result<Acquire, PolicyError> {
        if n == 0 {
            return Err(PolicyError::Config("acquire of 0 CPUs".into()));
        }
        let blockers = self.launch_blockers();
        if !blockers.is_empty() {
            return Ok(Acquire::NotNow(blockers.join("; ")));
        }
        let in_use = lock(&self.book).in_use.clone();
        if in_use.len() + n > self.config.max_cores {
            return Ok(Acquire::NotNow(format!(
                "core budget {} reached ({} in use)",
                self.config.max_cores,
                in_use.len()
            )));
        }
        let ex = match self.exclusions() {
            Ok(e) => e,
            Err(reason) => return Ok(Acquire::NotNow(reason)),
        };
        let mut eligible: Vec<usize> = self
            .candidates
            .iter()
            .copied()
            .filter(|c| !ex.contains(c) && !in_use.contains(c))
            .filter(|c| match self.idle.idle(*c) {
                Some(i) => i >= self.config.idle_min,
                None => self.config.idle_min <= 0.0,
            })
            .filter(|c| match &self.siblings {
                None => true,
                Some(map) => map.get(c).is_none_or(|sib| {
                    sib.iter().all(|s| {
                        in_use.contains(s)
                            || match self.idle.idle(*s) {
                                Some(i) => i >= self.config.idle_min,
                                None => self.config.idle_min <= 0.0,
                            }
                    })
                }),
            })
            .collect();
        eligible.sort_by(|a, b| {
            let ia = self.idle.idle(*a).unwrap_or(0.0);
            let ib = self.idle.idle(*b).unwrap_or(0.0);
            ib.total_cmp(&ia).then(a.cmp(b))
        });
        let mut got: Vec<(usize, File)> = Vec::new();
        for c in eligible {
            if got.len() == n {
                break;
            }
            let path = self.lock_path(c);
            let f = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(&path)
                .map_err(io_err(format!("open {}", path.display())))?;
            match f.try_lock() {
                Ok(()) => got.push((c, f)),
                Err(std::fs::TryLockError::WouldBlock) => {}
                Err(std::fs::TryLockError::Error(e)) => {
                    return Err(PolicyError::Io {
                        what: format!("flock {}", path.display()),
                        error: e,
                    });
                }
            }
        }
        if got.len() < n {
            return Ok(Acquire::NotNow(format!(
                "only {} of {n} CPUs free under the site rules",
                got.len()
            )));
        }
        let cpus: Vec<usize> = got.iter().map(|x| x.0).collect();
        // The claim is on record before the lease exists; no record, no lease (the flocks drop with `got`).
        self.ledger
            .append(&self.ledger.claim_line(&cpus, label))
            .map_err(io_err(format!(
                "append to {}",
                self.config.ledger_file.display()
            )))?;
        lock(&self.book).in_use.extend(cpus.iter().copied());
        Ok(Acquire::Granted(CpuLease {
            cpus,
            label: label.to_string(),
            book: Arc::clone(&self.book),
            locks: got.into_iter().map(|x| x.1).collect(),
            ledger: Some(self.ledger.clone()),
            released: false,
        }))
    }

    fn budget(&self) -> usize {
        self.config.max_cores
    }

    fn in_use(&self) -> usize {
        lock(&self.book).in_use.len()
    }

    fn launch_blockers(&self) -> Vec<String> {
        let mut files = self.config.pause_files.clone();
        files.extend(self.extra_pause.iter().cloned());
        read_pause(&files)
    }

    fn refresh(&mut self) -> io::Result<()> {
        self.idle.refresh(Duration::from_millis(900))
    }
}

/// "YYYY-MM-DDTHH:MM:SSZ" of the system clock (the ledger's time-stamp format; `date -u +%FT%TZ`).
pub fn utc_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    utc_string(secs)
}

/// Civil date from Unix seconds (H. Hinnant's days-from-civil inverse).
pub fn utc_string(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// SMT siblings of every CPU from `/sys/devices/system/cpu/cpu<n>/topology/thread_siblings_list` (the CPU itself
/// removed). CPUs without the file are absent from the map.
pub fn read_smt_siblings() -> BTreeMap<usize, Vec<usize>> {
    let mut out = BTreeMap::new();
    let Ok(cpus) = allowed_cpus() else {
        return out;
    };
    for c in cpus {
        let p = format!("/sys/devices/system/cpu/cpu{c}/topology/thread_siblings_list");
        if let Ok(t) = fs::read_to_string(&p)
            && let Ok(set) = parse_cpu_list(t.trim())
        {
            out.insert(c, set.into_iter().filter(|&s| s != c).collect());
        }
    }
    out
}

/// For the pre-flight: a readable summary of what a site policy would exclude now.
pub fn describe_exclusions(p: &SitePolicy) -> String {
    match p.exclusions() {
        Ok(ex) => format!("{} CPUs excluded: {}", ex.len(), format_cpu_list(&ex)),
        Err(e) => format!("exclusions unknown: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The example of `plan/auto-campaign/interfaces/process-layer.md` section 7 (this host's values) parses and
    /// validates; keep the two in sync.
    #[test]
    fn documented_site_example_parses() {
        let c: SiteConfig = serde_json::from_str(SITE_EXAMPLE).unwrap();
        c.validate().unwrap();
        let mut ex = BTreeSet::new();
        for e in &c.exclude {
            ex.extend(parse_cpu_list(e).unwrap());
        }
        for cpu in [168, 175, 288, 319, 128, 162, 166, 192, 335, 378] {
            assert!(ex.contains(&cpu), "{cpu}");
        }
        assert!(!ex.contains(&163) && !ex.contains(&0));
        assert_eq!(c.max_cores, 48);
        assert_eq!(c.held_files.len(), 4);
    }

    const SITE_EXAMPLE: &str = r#"{
  "schema": "rustred.campaign.site-policy.v1",
  "tag": "rr-campaign",
  "lock_dir": "/shared/vhirschi/t3code/vakint_dev/p_integral_dev/ops/locks",
  "lock_name": "cpu.{n}",
  "ledger_file": "/shared/vhirschi/t3code/vakint_dev/p_integral_dev/ops/CPU_LEDGER.md",
  "ledger_scan_lines": 1500,
  "candidates": null,
  "exclude": ["168-175", "288-319", "128-162,164-166", "192-215,258-260,262,271,335", "342,362,363,378"],
  "held_files": [
    {"path": "/shared/vhirschi/t3code/vakint_dev/p_integral_dev/ops/FOREIGN_5L_LIFT.held", "key": "held"},
    {"path": "/shared/vhirschi/t3code/vakint_dev/p_integral_dev/ops/FOREIGN_5L_LIFT.json", "key": "all_cpus_listed"},
    {"path": "/shared/vhirschi/t3code/vakint_dev/p_integral_dev/ops/OUR_PINS.json", "key": "entries[].cpus"},
    {"path": "/shared/vhirschi/t3code/vakint/scratchpad/5l-pilot/coordinator/admission/LIFT16359_CPUS.json", "key": "cpus"}
  ],
  "pause_files": ["/shared/vhirschi/t3code/vakint_dev/p_integral_dev/ops/PAUSE"],
  "idle_min": 0.85,
  "max_cores": 48,
  "respect_foreign_pinning": true,
  "ignore_own_tag_claims": true
}"#;

    #[test]
    fn ledger_parsing_matches_pick_free() {
        let lines = [
            "2026-11-13T23:00:37Z (t162-cert): + CPU 44 (flock) -> certification worker. Released at its end.",
            "2026-11-13T23:00:37Z (t162-cert): + CPU 10 (flock) -> worker e-c124.",
            "2026-11-13T23:00:37Z (t162-cert): RELEASED 44 (worker e-c126 ended)",
            "2026-11-13T23:00:41Z (t162-box, lane 2): + CPUs 28,30-31 (flock) -> certifier",
            "2026-11-13 09:20 (x): + CPU 5 for something",
            "2026-11-13T23:00:41Z (rr-campaign): + CPU 7 -> job; other",
            "no owner here + CPU 9 (flock)",
            "2026-11-03 18:55 resume run wf: m1 released 128-159",
        ];
        let held = ledger_claims(lines.iter().copied());
        assert_eq!(held["t162-cert"], BTreeSet::from([10]));
        assert_eq!(held["t162-box"], BTreeSet::from([28, 30, 31]));
        assert_eq!(held.get("x").cloned().unwrap_or_default(), BTreeSet::new());
        assert_eq!(held["rr-campaign"], BTreeSet::from([7]));
        assert!(!held.values().any(|s| s.contains(&9)));
        // Our own lines parse back.
        let w = LedgerWriter {
            path: PathBuf::from("/nonexistent"),
            tag: "rr-campaign".into(),
        };
        let c = w.claim_line(&[3, 5], "job a1 (x)");
        let r = w.release_line(&[3], "job a1");
        let h = ledger_claims([c.as_str()]);
        assert_eq!(h["rr-campaign"], BTreeSet::from([3, 5]));
        let h = ledger_claims([c.as_str(), r.as_str()]);
        assert_eq!(h["rr-campaign"], BTreeSet::from([5]));
        assert!(c.contains("+ CPUs 3,5 (flock) -> job a1 _x_."), "{c}");
    }

    #[test]
    fn reconcile_and_sibling_quiet() {
        let root = std::env::temp_dir().join(format!("rr-site-reconcile-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("locks")).unwrap();
        let mut c: SiteConfig = serde_json::from_str(SITE_EXAMPLE).unwrap();
        c.lock_dir = root.join("locks");
        c.ledger_file = root.join("CPU_LEDGER.md");
        c.held_files.clear();
        c.pause_files.clear();
        c.exclude.clear();
        c.candidates = Some("0-3".into());
        c.respect_foreign_pinning = false;
        c.idle_min = 0.5;
        // Our dead supervisor claimed 0 and 1; a live holder still has the flock of 1.
        fs::write(
            &c.ledger_file,
            "2026-11-14T00:00:00Z (rr-campaign): + CPUs 0,1 (flock) -> old job. Released at its end.\n",
        )
        .unwrap();
        let live = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(root.join("locks/cpu.1"))
            .unwrap();
        live.try_lock().unwrap();
        let aff: BTreeSet<usize> = (0..4).collect();
        let mut p = SitePolicy::with_affinity(c.clone(), &aff).unwrap();
        assert_eq!(p.reconcile_own_claims().unwrap(), vec![0]);
        let text = fs::read_to_string(&c.ledger_file).unwrap();
        assert_eq!(
            ledger_claims(text.lines())["rr-campaign"],
            BTreeSet::from([1])
        );
        assert!(p.reconcile_own_claims().unwrap().is_empty());
        drop(live);
        // Sibling rule: 2 and 3 are siblings; 3 busy -> 2 not eligible, 0 (sibling 1 idle) eligible.
        p.idle_tracker_mut()
            .set_idle([(0, 0.9), (1, 0.9), (2, 0.9), (3, 0.1)].into());
        p.set_sibling_quiet([(0, vec![1]), (1, vec![0]), (2, vec![3]), (3, vec![2])].into());
        let a = p.acquire(2, "x").unwrap().granted().unwrap();
        let mut got = a.cpus().to_vec();
        got.sort();
        assert_eq!(got, vec![0, 1]);
        assert!(matches!(p.acquire(1, "y").unwrap(), Acquire::NotNow(_)));
        drop(a);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn keys_and_lists() {
        let v: serde_json::Value = serde_json::from_str(
            r#"{"held":[8,13],"entries":[{"cpus":"128-130"},{"cpus":"192,200"}],"x":"1-2"}"#,
        )
        .unwrap();
        assert_eq!(cpus_at_key(&v, "held").unwrap(), BTreeSet::from([8, 13]));
        assert_eq!(
            cpus_at_key(&v, "entries[].cpus").unwrap(),
            BTreeSet::from([128, 129, 130, 192, 200])
        );
        assert_eq!(cpus_at_key(&v, "x").unwrap(), BTreeSet::from([1, 2]));
        assert!(cpus_at_key(&v, "nope").is_err());
        assert_eq!(CpuSpec::parse("16").unwrap(), CpuSpec::Count(16));
        assert_eq!(CpuSpec::parse("count:4").unwrap(), CpuSpec::Count(4));
        assert_eq!(
            CpuSpec::parse("0-2,5").unwrap(),
            CpuSpec::List(BTreeSet::from([0, 1, 2, 5]))
        );
        assert_eq!(utc_string(0), "1970-01-01T00:00:00Z");
        assert_eq!(utc_string(1_794_610_837), "2026-11-13T23:00:37Z");
    }

    #[test]
    fn plain_policy_budget_affinity_idle_first_and_no_double_grant() {
        let affinity: BTreeSet<usize> = (0..8).collect();
        let spec = CpuSpec::List([2, 3, 4, 5, 100].into());
        let mut p = PlainPolicy::with_affinity(&spec, Some(3), &affinity).unwrap();
        assert_eq!(p.dropped, BTreeSet::from([100]));
        p.idle_tracker_mut()
            .set_idle([(2, 0.1), (3, 0.9), (4, 0.95), (5, 0.5)].into());
        let a = p.acquire(2, "a").unwrap().granted().unwrap();
        assert_eq!(a.cpus(), &[4, 3]);
        assert!(matches!(p.acquire(2, "b").unwrap(), Acquire::NotNow(_)));
        let b = p.acquire(1, "b").unwrap().granted().unwrap();
        assert_eq!(b.cpus(), &[5]);
        assert_eq!(p.in_use(), 3);
        drop(a);
        assert_eq!(p.in_use(), 1);
        let c = p.acquire(2, "c").unwrap().granted().unwrap();
        assert!(!c.cpus().contains(&5));
        b.release().unwrap();
        assert!(PlainPolicy::with_affinity(&CpuSpec::List([100].into()), None, &affinity).is_err());
        let q = PlainPolicy::with_affinity(&CpuSpec::Count(4), None, &affinity).unwrap();
        assert_eq!(q.budget(), 4);
    }
}
