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
//!
//! Crash safety and I/O (audit A M5, M6, m4-m6; audit B m6), since the procfix lane:
//! - children INHERIT the lease's flock fds (`LaunchSpec::with_lease`, the default way to use a lease): a CPU stays
//!   locked while any process of the job lives, also after a supervisor crash, so a surviving orphan and a new lease
//!   can never share a CPU;
//! - at restart [`SitePolicy::reconcile_own_claims`] appends RELEASED lines (marked "reconciled") for this tag's claims
//!   that have no RELEASED line and whose flock is free (holder gone); held ones are reported, not touched;
//! - every ledger line is ONE `write(2)` of the whole line on an O_APPEND descriptor, at most PIPE_BUF (4096) bytes,
//!   a short write is an error; labels are sanitised (whitelist, no "RELEASED", no '+', one line, <= 120 chars);
//! - a line that mentions "+ CPU" but does not parse under pick_free's rules is read leniently and its CPUs count as
//!   CLAIMED (fail closed), with a count of such lines for the dashboard;
//! - the ledger window is the true last `ledger_scan_lines` lines (read backwards; more than 64 MiB for the window
//!   refuses grants: fail closed);
//! - the site-file I/O of an acquire runs on a helper thread bounded by `io_timeout` (default 10 s; /shared can stall
//!   for seconds): on timeout the acquire returns NotNow and, if the helper later wrote a claim, it appends the matching
//!   RELEASED line and drops the flocks itself; a release writes RELEASED and drops the flocks on a helper thread, in
//!   that order, without blocking the caller beyond the bound.

use crate::resources::{
    Bounded, IdleTracker, allowed_cpus, format_cpu_list, parse_cpu_list, run_bounded,
    run_bounded_with,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::fd::{AsRawFd, RawFd};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Result of the bounded site I/O of an acquire: Err = I/O failure; Ok(Err) = not now (reason); Ok(Ok) = the CPUs
/// with their held flock files (claim line written).
type AcquireIo = Result<Result<(Vec<usize>, Vec<File>), String>, PolicyError>;

/// Default bound of the site-file I/O of one acquire / release.
pub const DEFAULT_IO_TIMEOUT: Duration = Duration::from_secs(10);

/// Longest ledger line we write (PIPE_BUF: a single write of at most this size on an O_APPEND descriptor is not
/// interleaved with other writers' lines on local file systems).
pub const MAX_LEDGER_LINE: usize = 4096;

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
        append_line(&self.path, line)
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

/// The site's files as `SitePolicy` needs them (gate3 m3): `lock_dir` an existing directory, `ledger_file` an existing
/// regular file (symlinks followed) that does not lie in `lock_dir`. Nothing is created.
fn check_site_files(c: &SiteConfig) -> Result<(), PolicyError> {
    match fs::metadata(&c.lock_dir) {
        Ok(m) if m.is_dir() => {}
        Ok(_) => {
            return Err(PolicyError::Config(format!(
                "lock_dir {} is not a directory",
                c.lock_dir.display()
            )));
        }
        Err(e) => {
            return Err(PolicyError::Config(format!(
                "lock_dir {}: {e} (the site's lock directory is never created by the campaign)",
                c.lock_dir.display()
            )));
        }
    }
    match fs::metadata(&c.ledger_file) {
        Ok(m) if m.file_type().is_file() => {}
        Ok(m) => {
            return Err(PolicyError::Config(format!(
                "ledger_file {} is not a regular file ({:?})",
                c.ledger_file.display(),
                m.file_type()
            )));
        }
        Err(e) => {
            return Err(PolicyError::Config(format!(
                "ledger_file {}: {e} (the site's CPU ledger is never created by the campaign)",
                c.ledger_file.display()
            )));
        }
    }
    let ledger = fs::canonicalize(&c.ledger_file)
        .map_err(io_err(format!("canonicalize {}", c.ledger_file.display())))?;
    let locks = fs::canonicalize(&c.lock_dir)
        .map_err(io_err(format!("canonicalize {}", c.lock_dir.display())))?;
    if ledger.starts_with(&locks) {
        return Err(PolicyError::Config(format!(
            "ledger_file {} lies inside lock_dir {}",
            c.ledger_file.display(),
            c.lock_dir.display()
        )));
    }
    Ok(())
}

/// Open an EXISTING site file (gate3 m3): never created, O_NONBLOCK (a FIFO never blocks the caller), and only a
/// regular file is accepted.
fn open_existing_regular(path: &Path, opts: &mut OpenOptions) -> io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;
    let f = opts.custom_flags(libc::O_NONBLOCK).open(path)?;
    if !f.metadata()?.file_type().is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{} is not a regular file", path.display()),
        ));
    }
    Ok(f)
}

/// One line, ONE write(2) on an O_APPEND descriptor, at most [`MAX_LEDGER_LINE`] bytes; a short write is an error. The
/// ledger must exist and be a regular file (gate3 m3: never created, never a FIFO).
pub fn append_line(path: &Path, line: &str) -> io::Result<()> {
    let buf = format!("{line}\n");
    if buf.len() > MAX_LEDGER_LINE || line.contains('\n') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "ledger line of {} bytes (max {MAX_LEDGER_LINE}, one line)",
                buf.len()
            ),
        ));
    }
    let mut f = open_existing_regular(path, OpenOptions::new().append(true))?;
    let n = f.write(buf.as_bytes())?;
    if n != buf.len() {
        return Err(io::Error::new(
            io::ErrorKind::WriteZero,
            format!("short ledger write: {n} of {} bytes", buf.len()),
        ));
    }
    Ok(())
}

/// Labels go into a prose ledger read by regexes (audit A m5): whitelist `[A-Za-z0-9 ._:/@=-]` (anything else -> '_',
/// so no '+', parentheses, semicolons or newlines), the word "RELEASED" defused, at most 120 characters.
pub fn sanitize(label: &str) -> String {
    let s: String = label
        .chars()
        .take(120)
        .map(|c| {
            if c.is_ascii_alphanumeric()
                || matches!(c, ' ' | '.' | '_' | ':' | '/' | '@' | '=' | '-')
            {
                c
            } else {
                '_'
            }
        })
        .collect();
    s.replace("RELEASED", "released")
}

/// A lease of CPUs. Released on drop (errors ignored) or by [`CpuLease::release`] (errors reported).
pub struct CpuLease {
    cpus: Vec<usize>,
    label: String,
    book: Arc<Mutex<Book>>,
    locks: Vec<File>,
    /// Paths of the lock files (site policy), parallel to `cpus`.
    lock_paths: Vec<PathBuf>,
    ledger: Option<LedgerWriter>,
    released: bool,
}

/// A CPU whose flock is still held by another process right after its lease was released (e.g. an unmarked survivor of
/// a job that inherited the lock descriptor; gate re-audit B m2). Reported, never signalled.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeldAfterRelease {
    pub cpu: usize,
    /// Processes of our uid with the lock file open (found through /proc/<pid>/fd), possibly empty.
    pub holders: Vec<u32>,
}

/// Processes of our uid that have `path` open (a scan of /proc/<pid>/fd; unreadable entries are skipped). Diagnostics
/// only (a human decides; nothing is signalled).
pub fn lock_holders(path: &Path) -> Vec<u32> {
    let Ok(target) = fs::canonicalize(path) else {
        return Vec::new();
    };
    let me = std::process::id();
    let mut out = Vec::new();
    let Ok(rd) = fs::read_dir("/proc") else {
        return out;
    };
    for e in rd.flatten() {
        let Some(pid) = e.file_name().to_str().and_then(|s| s.parse::<u32>().ok()) else {
            continue;
        };
        if pid == me {
            continue;
        }
        let Ok(fds) = fs::read_dir(format!("/proc/{pid}/fd")) else {
            continue;
        };
        if fds
            .flatten()
            .any(|f| fs::read_link(f.path()).is_ok_and(|l| l == target))
        {
            out.push(pid);
        }
    }
    out
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

    /// Release with a CHECK of every lock (gate re-audit B m2): our flocks are dropped first, then each CPU's lock is
    /// probed. A CPU whose flock is STILL held by another process (an unmarked survivor of the job that inherited the
    /// descriptor) keeps its claim: no RELEASED line is written for it (pickers keep excluding it, and the restart's
    /// `reconcile_own_claims` lists it in `held_by` with the holders), and it is returned with the holders found. Free
    /// CPUs get the RELEASED line. Nothing is ever signalled. Bounded by [`DEFAULT_IO_TIMEOUT`].
    pub fn release_checked(mut self) -> (io::Result<()>, Vec<HeldAfterRelease>) {
        if self.released {
            return (Ok(()), Vec::new());
        }
        self.released = true;
        {
            let mut b = lock(&self.book);
            for c in &self.cpus {
                b.in_use.remove(c);
            }
        }
        let locks = std::mem::take(&mut self.locks);
        let cpus = self.cpus.clone();
        let paths = self.lock_paths.clone();
        let ledger = self.ledger.clone();
        let label = self.label.clone();
        let work = move || {
            drop(locks);
            let mut free = Vec::new();
            let mut held = Vec::new();
            if paths.is_empty() {
                free = cpus.clone();
            }
            for (&cpu, path) in cpus.iter().zip(paths.iter()) {
                // An existing lock file, never created, never truncated (shared host locks: SUBAGENT_RULES L13).
                let busy = open_existing_regular(
                    path,
                    OpenOptions::new().read(true).write(true).truncate(false),
                )
                .is_ok_and(|f| matches!(f.try_lock(), Err(std::fs::TryLockError::WouldBlock)));
                if busy {
                    held.push(HeldAfterRelease {
                        cpu,
                        holders: lock_holders(path),
                    });
                } else {
                    free.push(cpu);
                }
            }
            let r = match &ledger {
                Some(w) if !free.is_empty() => append_line(&w.path, &w.release_line(&free, &label)),
                _ => Ok(()),
            };
            (r, held)
        };
        match run_bounded(DEFAULT_IO_TIMEOUT, work).done() {
            Some(x) => x,
            None => (
                Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "lease release still in progress (background)",
                )),
                Vec::new(),
            ),
        }
    }

    /// Release now: RELEASED line, then the flocks, on a helper thread; waits at most [`DEFAULT_IO_TIMEOUT`] for the
    /// result (Err(TimedOut) = still in progress in the background, in the right order).
    pub fn release(mut self) -> io::Result<()> {
        match self.do_release(Some(DEFAULT_IO_TIMEOUT)) {
            Some(r) => r,
            None => Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "ledger write still in progress (background)",
            )),
        }
    }

    fn do_release(&mut self, wait: Option<Duration>) -> Option<io::Result<()>> {
        if self.released {
            return Some(Ok(()));
        }
        self.released = true;
        // In-process book first (a new local acquire then simply finds the flock still held until the helper drops it).
        {
            let mut b = lock(&self.book);
            for c in &self.cpus {
                b.in_use.remove(c);
            }
        }
        // RELEASED first, while the flocks are still held: a picker that sees the line and tries the flock finds it
        // either still held (skips) or free (takes it); it can never take a CPU whose claim it still believes live.
        let locks = std::mem::take(&mut self.locks);
        let line = self
            .ledger
            .as_ref()
            .map(|w| (w.path.clone(), w.release_line(&self.cpus, &self.label)));
        let work = move || {
            let r = match &line {
                Some((path, l)) => append_line(path, l),
                None => Ok(()),
            };
            drop(locks); // closing the descriptors drops the flocks (children that inherited them keep theirs)
            r
        };
        match wait {
            Some(t) => run_bounded(t, work).done(),
            None => {
                if std::thread::Builder::new()
                    .name("rr-lease-release".into())
                    .spawn(work)
                    .is_err()
                {
                    // Cannot spawn: nothing else to do; the flocks drop with this process at the latest.
                }
                None
            }
        }
    }
}

impl Drop for CpuLease {
    /// Waits at most 2 s for the RELEASED line (so a process that exits right after dropping its leases still leaves a
    /// clean ledger), then lets the helper finish in the background.
    fn drop(&mut self) {
        let _ = self.do_release(Some(Duration::from_secs(2)));
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
            lock_paths: Vec::new(),
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
        // gate3 m3 (lane int5): a lock name is ONE plain file name inside lock_dir (no '/', no "." / "..": a name like
        // "../escape.{n}" opened files outside the lock directory)
        let probe = self.lock_name.replace("{n}", "0");
        if probe.contains(['/', '\0']) || probe == "." || probe == ".." || probe.starts_with("..") {
            return Err(PolicyError::Config(format!(
                "lock_name {:?} must be a plain file name inside lock_dir",
                self.lock_name
            )));
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

/// Claims per owner plus the count of lines that mention a claim but do not parse under pick_free's rules.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LedgerView {
    pub claims: BTreeMap<String, BTreeSet<usize>>,
    /// Lines with "+ CPU" that pick_free's regex does not read (their CPUs, read leniently, count as claimed).
    pub unparsable_lines: usize,
}

/// Owner placeholder for claim lines whose owner cannot be parsed (never released: fail closed within the window).
pub const UNKNOWN_OWNER: &str = "?unparsable-owner";

/// FAIL-CLOSED reading of ledger lines (audit A m4): as [`ledger_claims`], but a line that mentions "+ CPU" without a
/// pick_free-parsable claim (or without a parsable owner) still claims whatever CPU numbers follow "+ CPU", read
/// leniently, and is counted.
pub fn ledger_claims_strict<'a>(lines: impl IntoIterator<Item = &'a str>) -> LedgerView {
    let mut v = LedgerView::default();
    for line in lines {
        let owner = owner_of(line);
        let strict: BTreeSet<usize> = claims_in(line)
            .iter()
            .flat_map(|c| parse_lenient(c))
            .collect();
        let mentions = line.contains("+ CPU");
        if mentions && (strict.is_empty() || owner.is_none()) {
            v.unparsable_lines += 1;
            let lenient = lenient_claims(line);
            let who = owner.clone().unwrap_or_else(|| UNKNOWN_OWNER.to_string());
            v.claims.entry(who).or_default().extend(lenient);
        }
        let Some(who) = owner else { continue };
        if !strict.is_empty() {
            v.claims.entry(who.clone()).or_default().extend(strict);
        }
        for r in releases_in(line) {
            let set = v.claims.entry(who.clone()).or_default();
            for c in parse_lenient(&r) {
                set.remove(&c);
            }
        }
    }
    v
}

fn owner_of(line: &str) -> Option<String> {
    let sp = line.find(' ')?;
    if sp == 0 || !line[sp + 1..].starts_with('(') {
        return None;
    }
    let after = &line[sp + 2..];
    let close = after.find(')')?;
    if close == 0 {
        return None;
    }
    Some(
        after[..close]
            .split(',')
            .next()
            .unwrap_or("")
            .trim()
            .to_string(),
    )
}

/// After every "+ CPU" / "+ CPUs": the longest run of digits, ',', '-', ' ', read leniently.
fn lenient_claims(line: &str) -> BTreeSet<usize> {
    let mut out = BTreeSet::new();
    let mut rest = line;
    while let Some(i) = rest.find("+ CPU") {
        let mut t = &rest[i + 5..];
        t = t.strip_prefix('s').unwrap_or(t);
        let end = t.find(|c: char| !is_cls(c)).unwrap_or(t.len());
        out.extend(parse_lenient(&t[..end]));
        rest = &rest[i + 5..];
    }
    out
}

/// Bytes the window of the last `n` lines may span before grants are refused.
const TAIL_LIMIT: u64 = 64 << 20;

/// The true last `n` lines of a file (read backwards in chunks; audit A m6, audit B m6). More than [`TAIL_LIMIT`] bytes
/// for the window is an error (the caller fails closed).
fn tail_lines(path: &Path, n: usize) -> io::Result<Vec<String>> {
    let mut f = match open_existing_regular(path, OpenOptions::new().read(true)) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let len = f.metadata()?.len();
    let mut start = len;
    let mut buf: Vec<u8> = Vec::new();
    const CHUNK: u64 = 256 * 1024;
    loop {
        // Newlines strictly inside the buffer (a trailing newline at EOF does not start a line).
        let body = if buf.last() == Some(&b'\n') && start + (buf.len() as u64) == len {
            &buf[..buf.len() - 1]
        } else {
            &buf[..]
        };
        let newlines = body.iter().filter(|&&b| b == b'\n').count();
        if start == 0 || newlines >= n {
            break;
        }
        if len - start >= TAIL_LIMIT {
            return Err(io::Error::other(format!(
                "the last {n} lines span more than {TAIL_LIMIT} bytes"
            )));
        }
        let step = CHUNK.min(start);
        start -= step;
        f.seek(SeekFrom::Start(start))?;
        let mut chunk = vec![0u8; step as usize];
        f.read_exact(&mut chunk)?;
        chunk.extend_from_slice(&buf);
        buf = chunk;
    }
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
    io_timeout: Duration,
    /// SMT siblings per CPU when the "sibling quiet" rule is on (coord-site-policy.md rule 2c; lane I-SUP).
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
        // gate3 m3 (lane int5): the site's files are NEVER created by the campaign: the lock directory must exist (a
        // directory), the ledger must be an existing REGULAR file outside it (a FIFO blocked the main thread, a
        // directory made admission wait forever, a lock file as ledger received claims).
        check_site_files(&config)?;
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
            io_timeout: DEFAULT_IO_TIMEOUT,
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

    /// Turn on the coordinator's "SMT sibling not busy" rule (coord-site-policy.md rule 2c): a CPU is eligible only if
    /// every sibling passes the same `idle_min` bar or is leased by us. `siblings` maps a CPU to its siblings;
    /// [`read_smt_siblings`] reads them from sysfs. Additive (lane I-SUP, 2026-11-14).
    pub fn set_sibling_quiet(&mut self, siblings: BTreeMap<usize, Vec<usize>>) {
        self.siblings = Some(siblings);
    }

    pub fn config(&self) -> &SiteConfig {
        &self.config
    }

    fn lock_path(&self, cpu: usize) -> PathBuf {
        self.config
            .lock_dir
            .join(self.config.lock_name.replace("{n}", &cpu.to_string()))
    }

    /// Bound of the site-file I/O of acquire / release (default 10 s).
    pub fn set_io_timeout(&mut self, t: Duration) {
        self.io_timeout = t;
    }

    /// RESTART-TIME reconciliation (audit A M6): this tag's claims in the ledger window that have no RELEASED line and
    /// whose flock is FREE (the holder is gone; a surviving orphan job still holds its inherited flock) get one RELEASED
    /// line marked "reconciled at restart: holder gone". Blocking (site files); call before the control loop starts.
    pub fn reconcile_own_claims(&mut self) -> Result<Reconciled, PolicyError> {
        let lines = tail_lines(&self.config.ledger_file, self.config.ledger_scan_lines).map_err(
            io_err(format!("read {}", self.config.ledger_file.display())),
        )?;
        let view = ledger_claims_strict(lines.iter().map(String::as_str));
        let own = view
            .claims
            .get(&self.config.tag)
            .cloned()
            .unwrap_or_default();
        let in_use = lock(&self.book).in_use.clone();
        let mut out = Reconciled::default();
        let mut probes: Vec<File> = Vec::new();
        for c in own {
            if in_use.contains(&c) {
                out.held.push(c); // a lease of this very process
                continue;
            }
            let path = self.lock_path(c);
            // gate3 m3: an existing lock file only (never created); a missing one cannot be held: left alone
            let f = match open_existing_regular(
                &path,
                OpenOptions::new().read(true).write(true).truncate(false),
            ) {
                Ok(f) => f,
                Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
                Err(e) => {
                    return Err(PolicyError::Io {
                        what: format!("open {}", path.display()),
                        error: e,
                    });
                }
            };
            match f.try_lock() {
                Ok(()) => {
                    out.released.push(c);
                    probes.push(f);
                }
                Err(std::fs::TryLockError::WouldBlock) => {
                    out.held.push(c);
                    out.held_by.push((c, lock_holders(&path)));
                }
                Err(std::fs::TryLockError::Error(e)) => {
                    return Err(PolicyError::Io {
                        what: format!("flock {}", path.display()),
                        error: e,
                    });
                }
            }
        }
        if !out.released.is_empty() {
            let (_, list) = cpu_word(&out.released);
            let line = format!(
                "{} ({}): RELEASED {list} (reconciled at restart: holder gone)",
                utc_now(),
                self.config.tag
            );
            self.ledger.append(&line).map_err(io_err(format!(
                "append to {}",
                self.config.ledger_file.display()
            )))?;
        }
        drop(probes); // RELEASED is on record before the probe flocks drop
        Ok(out)
    }
}

/// Result of [`SitePolicy::reconcile_own_claims`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Reconciled {
    /// CPUs whose stale own claim got a RELEASED line.
    pub released: Vec<usize>,
    /// CPUs of own claims whose flock is held (a surviving job of ours, or another instance): left alone.
    pub held: Vec<usize>,
    /// For each held CPU of `held` probed here: the processes of our uid that have its lock file open (for a human,
    /// e.g. "held by unmarked survivor pid N"; never signalled). CPUs leased by this very process are not listed.
    pub held_by: Vec<(usize, Vec<u32>)>,
}

/// Everything that excludes a CPU now (fail closed), with the count of unparsable claim lines.
fn exclusions_of(
    config: &SiteConfig,
    exclude: &BTreeSet<usize>,
    foreign_pinned: Option<&BTreeSet<usize>>,
) -> Result<(BTreeSet<usize>, usize), String> {
    {
        let mut ex = exclude.clone();
        for h in &config.held_files {
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
        let lines = tail_lines(&config.ledger_file, config.ledger_scan_lines)
            .map_err(|e| format!("ledger {}: {e}", config.ledger_file.display()))?;
        let view = ledger_claims_strict(lines.iter().map(String::as_str));
        for (who, set) in view.claims {
            if !(config.ignore_own_tag_claims && who == config.tag) {
                ex.extend(set);
            }
        }
        if config.respect_foreign_pinning {
            match foreign_pinned {
                Some(p) => ex.extend(p.iter().copied()),
                None => return Err("foreign CPU pinning not scanned yet".into()),
            }
        }
        Ok((ex, view.unparsable_lines))
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
        // Site-file I/O (held files, ledger, lock files, claim line) on a helper thread, bounded (audit A M5).
        let config = self.config.clone();
        let exclude = self.exclude.clone();
        let foreign = self.foreign_pinned.clone();
        let candidates = self.candidates.clone();
        let idle: BTreeMap<usize, f64> = candidates
            .iter()
            .filter_map(|&c| self.idle.idle(c).map(|i| (c, i)))
            .collect();
        // Sibling rule (cheap, in memory): candidates with a busy SMT sibling are not eligible now.
        let sibling_busy: BTreeSet<usize> = match &self.siblings {
            None => BTreeSet::new(),
            Some(map) => candidates
                .iter()
                .copied()
                .filter(|c| {
                    map.get(c).is_some_and(|sib| {
                        sib.iter().any(|s| {
                            !in_use.contains(s)
                                && !match self.idle.idle(*s) {
                                    Some(i) => i >= self.config.idle_min,
                                    None => self.config.idle_min <= 0.0,
                                }
                        })
                    })
                })
                .collect(),
        };
        let ledger = self.ledger.clone();
        let label_owned = label.to_string();
        let work = move || -> AcquireIo {
            let ex = match exclusions_of(&config, &exclude, foreign.as_ref()) {
                Ok((e, _)) => e,
                Err(reason) => return Ok(Err(reason)),
            };
            let mut eligible: Vec<usize> = candidates
                .iter()
                .copied()
                .filter(|c| !ex.contains(c) && !in_use.contains(c) && !sibling_busy.contains(c))
                .filter(|c| match idle.get(c) {
                    Some(&i) => i >= config.idle_min,
                    None => config.idle_min <= 0.0,
                })
                .collect();
            eligible.sort_by(|a, b| {
                let ia = idle.get(a).copied().unwrap_or(0.0);
                let ib = idle.get(b).copied().unwrap_or(0.0);
                ib.total_cmp(&ia).then(a.cmp(b))
            });
            let mut got: Vec<(usize, File)> = Vec::new();
            for c in eligible {
                if got.len() == n {
                    break;
                }
                let path = config
                    .lock_dir
                    .join(config.lock_name.replace("{n}", &c.to_string()));
                // gate3 m3: an existing lock file only (never created): a CPU without one is not available
                let f = match open_existing_regular(
                    &path,
                    OpenOptions::new().read(true).write(true).truncate(false),
                ) {
                    Ok(f) => f,
                    Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
                    Err(e) => {
                        return Err(PolicyError::Io {
                            what: format!("open {}", path.display()),
                            error: e,
                        });
                    }
                };
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
                return Ok(Err(format!(
                    "only {} of {n} CPUs free under the site rules",
                    got.len()
                )));
            }
            let cpus: Vec<usize> = got.iter().map(|x| x.0).collect();
            // The claim is on record before the lease exists; no record, no lease (the flocks drop with `got`).
            ledger
                .append(&ledger.claim_line(&cpus, &label_owned))
                .map_err(io_err(format!(
                    "append to {}",
                    config.ledger_file.display()
                )))?;
            Ok(Ok((cpus, got.into_iter().map(|x| x.1).collect())))
        };
        let undo_ledger = self.ledger.clone();
        let undo_label = label.to_string();
        let on_abandon = move |r: AcquireIo| {
            // The caller gave up: a claim written late is released again, then its flocks drop.
            if let Ok(Ok((cpus, files))) = r {
                let _ = undo_ledger.append(&undo_ledger.release_line(&cpus, &undo_label));
                drop(files);
            }
        };
        let (cpus, locks) = match run_bounded_with(self.io_timeout, work, on_abandon) {
            Bounded::TimedOut => {
                return Ok(Acquire::NotNow(format!(
                    "site files did not answer within {:?}",
                    self.io_timeout
                )));
            }
            // gate3 M3: no helper thread (EAGAIN / ENOMEM): nothing was written; try again later
            Bounded::SpawnFailed(e) => {
                return Ok(Acquire::NotNow(format!("site-file helper thread: {e}")));
            }
            Bounded::Done(Err(e)) => return Err(e),
            Bounded::Done(Ok(Err(reason))) => return Ok(Acquire::NotNow(reason)),
            Bounded::Done(Ok(Ok(x))) => x,
        };
        lock(&self.book).in_use.extend(cpus.iter().copied());
        let lock_paths = cpus.iter().map(|&c| self.lock_path(c)).collect();
        Ok(Acquire::Granted(CpuLease {
            cpus,
            label: label.to_string(),
            book: Arc::clone(&self.book),
            locks,
            lock_paths,
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

/// SMT siblings of every allowed CPU from `/sys/devices/system/cpu/cpu<n>/topology/thread_siblings_list` (the CPU
/// itself removed). CPUs without the file are absent from the map. (lane I-SUP)
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
    match exclusions_of(&p.config, &p.exclude, p.foreign_pinned.as_ref()) {
        Ok((ex, bad)) => format!(
            "{} CPUs excluded: {}; {bad} claim line(s) unparsable under pick_free's rules (counted as claims)",
            ex.len(),
            format_cpu_list(&ex)
        ),
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
    fn sibling_quiet_rule() {
        let root = std::env::temp_dir().join(format!("rr-site-sibling-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("locks")).unwrap();
        // gate3 m3: the site's files exist beforehand (the campaign never creates them)
        fs::write(root.join("CPU_LEDGER.md"), b"").unwrap();
        for n in 0..4 {
            fs::write(root.join("locks").join(format!("cpu.{n}")), b"").unwrap();
        }
        let mut c: SiteConfig = serde_json::from_str(SITE_EXAMPLE).unwrap();
        c.lock_dir = root.join("locks");
        c.ledger_file = root.join("CPU_LEDGER.md");
        c.held_files.clear();
        c.pause_files.clear();
        c.exclude.clear();
        c.candidates = Some("0-3".into());
        c.respect_foreign_pinning = false;
        c.idle_min = 0.5;
        let aff: BTreeSet<usize> = (0..4).collect();
        let mut p = SitePolicy::with_affinity(c, &aff).unwrap();
        // 2 and 3 are siblings and 3 is busy -> 2 not eligible; 0 and 1 (siblings, both idle) eligible.
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
