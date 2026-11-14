//! /proc scans by SESSION on a helper thread.
//!
//! Why a thread: a per-pid /proc read can block in the kernel for minutes (measured on this host, see the docstring of
//! `ops/guarded_run.py`). The control loop therefore never scans; it reads the latest published [`Snapshot`] and, when it
//! needs a fresh one (after a SIGKILL, after a child's exit), asks with [`Sampler::request_now`] and waits for a
//! snapshot whose `started_at` is later than the event, at its own pace.
//!
//! What a scan reports per tracked session: live members (zombies left out, except a zombie leader with live threads,
//! which still counts), their process groups, summed RSS and CPU ticks, the session's peak RSS since tracking began, and
//! ESCAPEES: processes whose parent is a known member of the session but whose own session differs (they called setsid).
//! Escapees are reported, never chased (see `interfaces/process-layer.md`, orphan guarantees). A process that escapes
//! and whose parent exits before any scan sees it is invisible: an accepted limit, as in guarded_run.py.

//!
//! Since the audit fixes (procfix): the sampler also
//! - reads host MemAvailable at the START and at the END of every scan (`host_before`, `host_after`): admission uses the
//!   smaller of the two with the per-job numbers of the same scan (audit A M3);
//! - reads `Pss` from `/proc/<pid>/smaps_rollup` for members of tracked sessions (shared and copy-on-write pages are
//!   no longer counted twice; `SessionUsage::mem_bytes` prefers PSS and says which kind it gave; audit A M3, B m2);
//! - when a campaign id is set ([`Sampler::set_campaign`]), lists every live process of our uid carrying a marker of
//!   that campaign (`Snapshot::marked`, the orphan scan of plan 2.7.1), with environ reads bounded and cached per
//!   (pid, start time);
//! - reaps STRAY zombie children (orphans re-parented to the supervisor as child subreaper) that are provably ours
//!   (marked by this campaign while alive, or members of a tracked session) and are not tracked leaders; reaping goes
//!   through a pidfd after a start-time check, under the leader registry lock, so a tracked leader's status is never
//!   taken;
//! - publishes its health: [`Sampler::health`] says when the latest scan is older than a threshold (audit A M2).

use super::marked::{
    CampaignMarker, ENVIRON_READ_TIMEOUT, MarkedProcess, MarkerRead, read_marker_at,
};
use super::{MemInfo, ProcStat, host_memory, live_thread_stat, read_stat};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Registry of the launcher's UNREAPED leader pids. A pid is in it from before its spawn returns until it is reaped,
/// and every reap of a leader happens while the lock is held. Holding the lock with the pid present therefore proves the
/// pid (and the session and group numbers it leads) cannot be recycled: the basis of the group-signal fast path.
#[derive(Debug, Default)]
pub(crate) struct Leaders {
    set: Mutex<BTreeSet<u32>>,
}

/// The registry is PROCESS-WIDE (reaping is process-wide: a second launcher's sampler must never reap the first's
/// leaders).
pub(crate) fn global_leaders() -> &'static Arc<Leaders> {
    static L: std::sync::OnceLock<Arc<Leaders>> = std::sync::OnceLock::new();
    L.get_or_init(|| Arc::new(Leaders::default()))
}

impl Leaders {
    pub(crate) fn lock(&self) -> MutexGuard<'_, BTreeSet<u32>> {
        self.set.lock().unwrap_or_else(|p| p.into_inner())
    }
}

/// Which memory number a usage carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum MemKind {
    /// Sum of `Pss` (smaps_rollup) over the members: shared pages split between their users.
    Pss,
    /// Sum of RSS (fallback: some member's smaps_rollup was unreadable). Counts shared / CoW pages once per process.
    Rss,
    /// `memory.current` of the job's own cgroup (delegated cgroup or systemd scope).
    CgroupCurrent,
}

/// Is the sampler keeping up?
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SamplerHealth {
    /// The latest scan finished `age` ago (<= the threshold).
    Fresh { age: Duration },
    /// No scan finished within the threshold (`age` None: none ever finished); `thread_alive` false = the thread died.
    Stale {
        age: Option<Duration>,
        thread_alive: bool,
    },
}

impl SamplerHealth {
    pub fn is_fresh(&self) -> bool {
        matches!(self, SamplerHealth::Fresh { .. })
    }
}

/// A process that left a tracked session by setsid (or setsid + fork).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct EscapeeRecord {
    pub pid: u32,
    pub start_ticks: u64,
    pub ppid_when_seen: u32,
    pub session_now: u32,
    pub comm: String,
    /// Present in the latest scan.
    pub alive: bool,
}

/// Usage of one tracked session in one scan.
#[derive(Clone, Debug, Default, Serialize)]
pub struct SessionUsage {
    pub session: u32,
    /// Live members (state not Z/X, or a zombie leader with a live thread).
    pub members: Vec<ProcStat>,
    /// Process groups of the live members.
    pub groups: BTreeSet<u32>,
    /// RSS per process group of the session (children fork helpers that may change their group).
    pub group_rss_bytes: BTreeMap<u32, u64>,
    pub rss_bytes: u64,
    pub cpu_ticks: u64,
    /// Largest `rss_bytes` seen in any scan since the session was tracked.
    pub peak_rss_bytes: u64,
    pub escapees: Vec<EscapeeRecord>,
    /// Sum of `Pss` over the live members when EVERY member's smaps_rollup was read, else None.
    pub pss_bytes: Option<u64>,
}

impl SessionUsage {
    /// The memory number for caps and admission: PSS when known, else RSS. Bytes.
    pub fn mem_bytes(&self) -> (u64, MemKind) {
        match self.pss_bytes {
            Some(p) => (p, MemKind::Pss),
            None => (self.rss_bytes, MemKind::Rss),
        }
    }

    /// Live members other than `pid` (typically: other than the leader, whose zombie the launcher keeps unreaped).
    pub fn members_except(&self, pid: u32) -> impl Iterator<Item = &ProcStat> {
        self.members.iter().filter(move |m| m.pid != pid)
    }
}

/// One published scan.
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub seq: u64,
    /// When the scan began listing /proc. A process is "verified gone" only by a scan that began after the event.
    pub started_at: Instant,
    pub finished_at: Instant,
    pub sessions: BTreeMap<u32, SessionUsage>,
    /// CPUs inside a restricted affinity of some process outside the tracked sessions (only when configured).
    pub foreign_pinned: Option<BTreeSet<usize>>,
    pub processes_seen: usize,
    /// Per-pid reads that failed with something other than "gone".
    pub read_errors: u64,
    /// Host memory read just before the scan listed /proc, and just after it finished.
    pub host_before: Option<MemInfo>,
    pub host_after: Option<MemInfo>,
    /// Live processes of our uid carrying a marker of the sampler's campaign (empty when no campaign is set).
    pub marked: Vec<MarkedProcess>,
    /// Candidates whose environ could not be read in this scan (unknown: never signalled, retried).
    pub marked_unreadable: usize,
    /// Stray zombie children reaped by this scan (pid, start ticks).
    pub reaped_strays: Vec<(u32, u64)>,
}

impl Snapshot {
    /// The smaller MemAvailable of the two readings bracketing the scan (None if neither was read), in bytes.
    pub fn mem_available_min(&self) -> Option<u64> {
        match (&self.host_before, &self.host_after) {
            (Some(a), Some(b)) => Some(a.mem_available.min(b.mem_available)),
            (Some(a), None) | (None, Some(a)) => Some(a.mem_available),
            (None, None) => None,
        }
    }

    /// Marked processes of one attempt, started at or after `not_before`.
    pub fn marked_attempt<'a>(
        &'a self,
        marker: &'a CampaignMarker,
        not_before: u64,
    ) -> impl Iterator<Item = &'a MarkedProcess> + 'a {
        self.marked
            .iter()
            .filter(move |p| p.attempt_id == marker.attempt_id && p.start_ticks >= not_before)
    }

    pub fn session(&self, sid: u32) -> Option<&SessionUsage> {
        self.sessions.get(&sid)
    }
    pub fn scan_duration(&self) -> Duration {
        self.finished_at.duration_since(self.started_at)
    }
}

#[derive(Clone, Debug)]
pub struct SamplerConfig {
    /// Pause between scans (a request cuts it short).
    pub interval: Duration,
    /// Also read /proc/<pid>/status of every non-root process to find CPUs pinned by others (pick.py's rule).
    pub collect_foreign_pinning: bool,
}

impl Default for SamplerConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(2),
            collect_foreign_pinning: false,
        }
    }
}

#[derive(Default)]
struct Track {
    /// pid -> start ticks of every member ever seen (to recognise escapees by their parent).
    known: HashMap<u32, u64>,
    peak_rss: u64,
    escapees: BTreeMap<u32, EscapeeRecord>,
}

/// Campaign settings of the sampler.
#[derive(Clone, Debug)]
struct CampaignCfg {
    id: String,
    not_before_ticks: u64,
}

/// Cached environ verdict of one process (pid, start ticks).
#[derive(Clone, Debug)]
enum CachedMarker {
    Value(String),
    NoMarker,
    /// Unreadable so far; retried up to 3 times.
    Unreadable(u8),
}

struct Shared {
    tracked: Mutex<BTreeMap<u32, Track>>,
    campaign: Mutex<Option<CampaignCfg>>,
    leaders: Arc<Leaders>,
    spawned_at: Instant,
    latest: Mutex<Option<Arc<Snapshot>>>,
    wake: Mutex<bool>,
    wake_cv: Condvar,
    published_cv: Condvar,
    stop: AtomicBool,
    config: SamplerConfig,
}

/// Background /proc scanner. Dropping it stops the thread (without waiting for a blocked scan to return).
pub struct Sampler {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl Sampler {
    pub fn spawn(config: SamplerConfig) -> io::Result<Self> {
        let shared = Arc::new(Shared {
            tracked: Mutex::new(BTreeMap::new()),
            campaign: Mutex::new(None),
            leaders: Arc::clone(global_leaders()),
            spawned_at: Instant::now(),
            latest: Mutex::new(None),
            wake: Mutex::new(false),
            wake_cv: Condvar::new(),
            published_cv: Condvar::new(),
            stop: AtomicBool::new(false),
            config,
        });
        let s = Arc::clone(&shared);
        let thread = std::thread::Builder::new()
            .name("rr-campaign-sampler".into())
            .spawn(move || run(s))?;
        Ok(Self {
            shared,
            thread: Some(thread),
        })
    }

    /// Set the campaign whose marked processes every scan lists (and whose stray zombies it may reap). Processes that
    /// started before `not_before_ticks` are not examined (0: all).
    pub fn set_campaign(&self, campaign_id: &str, not_before_ticks: u64) {
        *lock(&self.shared.campaign) = Some(CampaignCfg {
            id: campaign_id.to_string(),
            not_before_ticks,
        });
        self.request_now();
    }

    pub fn campaign_id(&self) -> Option<String> {
        lock(&self.shared.campaign).as_ref().map(|c| c.id.clone())
    }

    pub(crate) fn leaders(&self) -> &Arc<Leaders> {
        &self.shared.leaders
    }

    /// Age of the latest finished scan against `stale_after` (audit A M2). Non-blocking.
    pub fn health(&self, stale_after: Duration) -> SamplerHealth {
        let now = Instant::now();
        let thread_alive = self.thread.as_ref().is_some_and(|t| !t.is_finished());
        let age = self
            .latest()
            .map(|s| now.saturating_duration_since(s.finished_at));
        match age {
            Some(a) if a <= stale_after && thread_alive => SamplerHealth::Fresh { age: a },
            None if thread_alive
                && now.saturating_duration_since(self.shared.spawned_at) <= stale_after =>
            {
                // Not yet a first scan, but the sampler is young: not stale yet.
                SamplerHealth::Fresh {
                    age: Duration::ZERO,
                }
            }
            _ => SamplerHealth::Stale { age, thread_alive },
        }
    }

    /// Start accounting a session (its leader's pid = the session id). Idempotent.
    pub fn track(&self, sid: u32) {
        lock(&self.shared.tracked).entry(sid).or_default();
        self.request_now();
    }

    /// Stop accounting a session (after its leader was reaped and the session was found empty).
    pub fn untrack(&self, sid: u32) {
        lock(&self.shared.tracked).remove(&sid);
    }

    /// Ask for a scan now (non-blocking).
    pub fn request_now(&self) {
        *lock(&self.shared.wake) = true;
        self.shared.wake_cv.notify_all();
    }

    /// Latest published snapshot (non-blocking; None before the first scan finished).
    pub fn latest(&self) -> Option<Arc<Snapshot>> {
        lock(&self.shared.latest).clone()
    }

    /// Block up to `timeout` for a snapshot that began after `after`. For tests and for blocking callers only; the
    /// control loop polls [`Self::latest`] instead.
    pub fn wait_for_scan_after(&self, after: Instant, timeout: Duration) -> Option<Arc<Snapshot>> {
        let deadline = Instant::now() + timeout;
        self.request_now();
        let mut guard = lock(&self.shared.latest);
        loop {
            if let Some(s) = guard.as_ref()
                && s.started_at > after
            {
                return Some(Arc::clone(s));
            }
            let now = Instant::now();
            if now >= deadline {
                return None;
            }
            guard = match self
                .shared
                .published_cv
                .wait_timeout(guard, (deadline - now).min(Duration::from_millis(500)))
            {
                Ok((g, _)) => g,
                Err(p) => p.into_inner().0,
            };
            drop(guard);
            self.request_now();
            guard = lock(&self.shared.latest);
        }
    }
}

impl Drop for Sampler {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::SeqCst);
        self.request_now();
        // A scan blocked in the kernel would block a join; the thread holds only Arcs, so detaching is safe.
        if let Some(t) = self.thread.take()
            && t.is_finished()
        {
            let _ = t.join();
        }
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

fn run(shared: Arc<Shared>) {
    let mut seq = 0u64;
    let mut cache: HashMap<(u32, u64), CachedMarker> = HashMap::new();
    while !shared.stop.load(Ordering::SeqCst) {
        let sids: BTreeSet<u32> = lock(&shared.tracked).keys().copied().collect();
        let campaign = lock(&shared.campaign).clone();
        let started_at = Instant::now();
        let host_before = host_memory().ok();
        let raw = scan_raw(&sids, shared.config.collect_foreign_pinning);
        let (marked, marked_unreadable) = match &campaign {
            Some(c) => marked_of(&raw, c, &mut cache),
            None => (Vec::new(), 0),
        };
        let pss = pss_of_members(&raw, &sids);
        let host_after = host_memory().ok();
        let finished_at = Instant::now();
        seq += 1;
        let sessions = {
            let mut tracked = lock(&shared.tracked);
            let mut out = fold(&raw, &mut tracked);
            for (sid, u) in out.iter_mut() {
                u.pss_bytes = pss.get(sid).copied().flatten();
            }
            out
        };
        let reaped_strays = match &campaign {
            Some(c) => reap_strays(&raw, &cache, c, &sids, &shared.leaders),
            None => Vec::new(),
        };
        let snap = Arc::new(Snapshot {
            seq,
            started_at,
            finished_at,
            sessions,
            foreign_pinned: raw.foreign_pinned,
            processes_seen: raw.all.len(),
            read_errors: raw.read_errors,
            host_before,
            host_after,
            marked,
            marked_unreadable,
            reaped_strays,
        });
        *lock(&shared.latest) = Some(snap);
        shared.published_cv.notify_all();
        // Sleep until the interval passes or someone asks.
        let mut w = lock(&shared.wake);
        let deadline = Instant::now() + shared.config.interval;
        while !*w && !shared.stop.load(Ordering::SeqCst) {
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            w = match shared.wake_cv.wait_timeout(w, deadline - now) {
                Ok((g, _)) => g,
                Err(p) => p.into_inner().0,
            };
        }
        *w = false;
    }
}

struct RawScan {
    /// Every process listed (stat of the main thread; zombie leaders with live threads replaced by the live thread).
    all: Vec<ProcStat>,
    foreign_pinned: Option<BTreeSet<usize>>,
    read_errors: u64,
}

fn scan_raw(tracked: &BTreeSet<u32>, pinning: bool) -> RawScan {
    let mut all = Vec::new();
    let mut read_errors = 0u64;
    let mut pinned: Option<BTreeSet<usize>> = if pinning { Some(BTreeSet::new()) } else { None };
    // Audit A m7: the machine's CPU count (as pick.py's os.cpu_count()), not this process's (possibly restricted) mask.
    // SAFETY: sysconf has no preconditions.
    let configured = unsafe { libc::sysconf(libc::_SC_NPROCESSORS_CONF) };
    let ncpu = (configured.max(1) as usize).max(
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1),
    );
    let Ok(dir) = fs::read_dir("/proc") else {
        return RawScan {
            all,
            foreign_pinned: pinned,
            read_errors: 1,
        };
    };
    // Streamed: each process is read close to the moment it is listed.
    for entry in dir {
        let Ok(entry) = entry else {
            read_errors += 1;
            continue;
        };
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|s| s.parse::<u32>().ok())
        else {
            continue;
        };
        let st = match read_stat(pid) {
            Ok(Some(st)) => st,
            Ok(None) => continue,
            Err(_) => {
                read_errors += 1;
                continue;
            }
        };
        let st = if st.is_dead() {
            // A dead main thread with live threads still holds memory and still runs (audit B M1: also for adopted
            // jobs, which are not tracked by session). Zombies are few: the task listing is cheap.
            match live_thread_stat(pid) {
                Ok(Some(t)) => ProcStat { pid, ..t },
                _ => st,
            }
        } else {
            st
        };
        if let Some(p) = pinned.as_mut()
            && !tracked.contains(&st.session)
            && pid != std::process::id()
            && let Some(set) = restricted_affinity(pid, ncpu)
        {
            p.extend(set);
        }
        all.push(st);
    }
    RawScan {
        all,
        foreign_pinned: pinned,
        read_errors,
    }
}

/// Affinity of a non-root process if it is restricted (fewer CPUs than the machine has), as pick.py `pinned()`.
fn restricted_affinity(pid: u32, ncpu: usize) -> Option<BTreeSet<usize>> {
    use std::os::unix::fs::MetadataExt;
    let meta = fs::metadata(format!("/proc/{pid}")).ok()?;
    if meta.uid() == 0 {
        return None;
    }
    let set = super::affinity_of(pid).ok()??;
    (set.len() < ncpu).then_some(set)
}

fn fold(raw: &RawScan, tracked: &mut BTreeMap<u32, Track>) -> BTreeMap<u32, SessionUsage> {
    let by_pid: HashMap<u32, &ProcStat> = raw.all.iter().map(|p| (p.pid, p)).collect();
    let mut out: BTreeMap<u32, SessionUsage> = BTreeMap::new();
    for (&sid, track) in tracked.iter_mut() {
        let mut u = SessionUsage {
            session: sid,
            ..Default::default()
        };
        for p in raw.all.iter().filter(|p| p.session == sid) {
            track.known.insert(p.pid, p.start_ticks);
            if p.is_dead() {
                continue;
            }
            u.groups.insert(p.pgrp);
            *u.group_rss_bytes.entry(p.pgrp).or_default() += p.rss_bytes;
            u.rss_bytes = u.rss_bytes.saturating_add(p.rss_bytes);
            u.cpu_ticks = u.cpu_ticks.saturating_add(p.cpu_ticks);
            u.members.push(p.clone());
        }
        track.peak_rss = track.peak_rss.max(u.rss_bytes);
        u.peak_rss_bytes = track.peak_rss;
        out.insert(sid, u);
    }
    // Audit A m11: `known` only needs processes that can still be a parent (present in this scan).
    for track in tracked.values_mut() {
        track
            .known
            .retain(|pid, start| by_pid.get(pid).is_some_and(|q| q.start_ticks == *start));
    }
    // Escapees: parent is a known member (same start time, still present), own session is not tracked.
    for p in &raw.all {
        if tracked.contains_key(&p.session) || p.is_dead() {
            continue;
        }
        for track in tracked.values_mut() {
            let Some(&parent_start) = track.known.get(&p.ppid) else {
                continue;
            };
            let parent_ok = by_pid
                .get(&p.ppid)
                .is_some_and(|q| q.start_ticks == parent_start);
            if parent_ok {
                // Only the process that left is listed, not its own descendants (they live in its new session).
                track
                    .escapees
                    .entry(p.pid)
                    .or_insert_with(|| EscapeeRecord {
                        pid: p.pid,
                        start_ticks: p.start_ticks,
                        ppid_when_seen: p.ppid,
                        session_now: p.session,
                        comm: p.comm.clone(),
                        alive: true,
                    });
            }
        }
    }
    for (&sid, track) in tracked.iter_mut() {
        for e in track.escapees.values_mut() {
            e.alive = by_pid
                .get(&e.pid)
                .is_some_and(|q| q.start_ticks == e.start_ticks && !q.is_dead());
        }
        // Audit A m11: keep every live escapee and at most 64 dead ones.
        let dead: Vec<u32> = track
            .escapees
            .values()
            .filter(|e| !e.alive)
            .map(|e| e.pid)
            .collect();
        for pid in dead.iter().take(dead.len().saturating_sub(64)) {
            track.escapees.remove(pid);
        }
        if let Some(u) = out.get_mut(&sid) {
            u.escapees = track.escapees.values().cloned().collect();
        }
    }
    out
}

/// Marked processes of the campaign in a raw scan (environ reads bounded and cached by (pid, start)).
fn marked_of(
    raw: &RawScan,
    c: &CampaignCfg,
    cache: &mut HashMap<(u32, u64), CachedMarker>,
) -> (Vec<MarkedProcess>, usize) {
    let uid = super::our_uid();
    let me = std::process::id();
    let root = Path::new("/proc");
    let present: BTreeSet<(u32, u64)> = raw.all.iter().map(|p| (p.pid, p.start_ticks)).collect();
    cache.retain(|k, _| present.contains(k));
    let mut out = Vec::new();
    let mut unreadable = 0usize;
    for p in &raw.all {
        if p.pid <= 1 || p.pid == me || p.start_ticks < c.not_before_ticks {
            continue;
        }
        let key = (p.pid, p.start_ticks);
        let retry = match cache.get(&key) {
            Some(CachedMarker::Value(_)) | Some(CachedMarker::NoMarker) => None,
            Some(CachedMarker::Unreadable(n)) if *n >= 3 => None,
            Some(CachedMarker::Unreadable(n)) => Some(*n),
            None => Some(0),
        };
        if let Some(n) = retry {
            if p.is_dead() || super::owner_uid_at(root, p.pid) != Some(uid) {
                // Not ours, or a zombie (no environ): remember as unmarked.
                if !p.is_dead() {
                    cache.insert(key, CachedMarker::NoMarker);
                }
                continue;
            }
            let mut r = read_marker_at(root, p.pid, None, ENVIRON_READ_TIMEOUT);
            if r == MarkerRead::Unreadable
                && let Ok(Some((tid, _))) = super::live_thread(p.pid)
            {
                // Main thread exited, a thread lives: its environ (same mm) is readable through the task entry.
                r = read_marker_at(root, p.pid, Some(tid), ENVIRON_READ_TIMEOUT);
            }
            let v = match r {
                MarkerRead::Marker(v) => CachedMarker::Value(v),
                MarkerRead::NoMarker | MarkerRead::Gone => CachedMarker::NoMarker,
                MarkerRead::Unreadable => CachedMarker::Unreadable(n + 1),
            };
            cache.insert(key, v);
        }
        match cache.get(&key) {
            Some(CachedMarker::Value(v)) => {
                if let Some(m) = CampaignMarker::parse(v)
                    && m.campaign_id == c.id
                    && !p.is_dead()
                {
                    out.push(MarkedProcess::from_stat(p, m.attempt_id));
                }
            }
            Some(CachedMarker::Unreadable(_)) => unreadable += 1,
            _ => {}
        }
    }
    (out, unreadable)
}

/// `Pss` (bytes) of one process from smaps_rollup, bounded.
fn pss_of(pid: u32) -> Option<u64> {
    let path = Path::new("/proc").join(format!("{pid}/smaps_rollup"));
    let super::ReadOutcome::Data(b) = super::read_bounded(&path, Duration::from_secs(1)) else {
        return None;
    };
    String::from_utf8_lossy(&b).lines().find_map(|l| {
        l.strip_prefix("Pss:")
            .and_then(|r| r.split_whitespace().next())
            .and_then(|kb| kb.parse::<u64>().ok())
            .and_then(|kb| kb.checked_mul(1024))
    })
}

/// Per tracked session: sum of Pss over its live members, None when any member's Pss could not be read.
fn pss_of_members(raw: &RawScan, sids: &BTreeSet<u32>) -> BTreeMap<u32, Option<u64>> {
    let mut out: BTreeMap<u32, Option<u64>> = BTreeMap::new();
    for p in raw
        .all
        .iter()
        .filter(|p| sids.contains(&p.session) && !p.is_dead())
    {
        let e = out.entry(p.session).or_insert(Some(0));
        *e = match (*e, pss_of(p.pid)) {
            (Some(a), Some(b)) => Some(a.saturating_add(b)),
            _ => None,
        };
    }
    out
}

/// Reap zombie children of this process that are provably ours and not tracked leaders (subreaper strays).
/// Ours = a member of a tracked session (job descendants keep their session unless they call setsid), or a process
/// whose environ carried this campaign's marker while it was alive (the cache entry survives while the zombie is
/// listed). Anything else (e.g. a helper the supervisor spawned with std::process::Command) is left to its owner.
fn reap_strays(
    raw: &RawScan,
    cache: &HashMap<(u32, u64), CachedMarker>,
    c: &CampaignCfg,
    sids: &BTreeSet<u32>,
    leaders: &Leaders,
) -> Vec<(u32, u64)> {
    let me = std::process::id();
    let mut out = Vec::new();
    let guard = leaders.lock();
    for p in raw.all.iter().filter(|p| p.ppid == me && p.is_dead()) {
        if guard.contains(&p.pid) {
            continue;
        }
        let marked = matches!(
            cache.get(&(p.pid, p.start_ticks)),
            Some(CachedMarker::Value(v)) if CampaignMarker::parse(v).is_some_and(|m| m.campaign_id == c.id)
        );
        if !sids.contains(&p.session) && !marked {
            continue;
        }
        if crate::launch::reap_exact(p.pid, p.start_ticks) {
            out.push((p.pid, p.start_ticks));
        }
    }
    drop(guard);
    out
}

/// Synchronous scan of the given sessions (blocks the caller; for tests and one-off checks, never the control loop).
pub fn scan_sessions_now(sids: &BTreeSet<u32>) -> BTreeMap<u32, SessionUsage> {
    let raw = scan_raw(sids, false);
    let mut tracked: BTreeMap<u32, Track> = sids.iter().map(|&s| (s, Track::default())).collect();
    fold(&raw, &mut tracked)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(pid: u32, ppid: u32, sid: u32, start: u64, state: char, rss: u64) -> ProcStat {
        ProcStat {
            pid,
            comm: format!("p{pid}"),
            state,
            ppid,
            pgrp: pid,
            session: sid,
            cpu_ticks: 1,
            start_ticks: start,
            rss_bytes: rss,
        }
    }

    #[test]
    fn fold_sessions_escapees_and_peak() {
        let mut tracked: BTreeMap<u32, Track> = BTreeMap::new();
        tracked.insert(100, Track::default());
        let raw = RawScan {
            all: vec![
                p(100, 1, 100, 10, 'S', 1000),
                p(101, 100, 100, 11, 'S', 500),
                p(102, 100, 100, 12, 'Z', 0),
                p(200, 101, 200, 13, 'S', 7), // escaped by setsid
                p(300, 999, 300, 14, 'S', 7), // unrelated
            ],
            foreign_pinned: None,
            read_errors: 0,
        };
        let out = fold(&raw, &mut tracked);
        let u = &out[&100];
        assert_eq!(u.members.len(), 2);
        assert_eq!(u.rss_bytes, 1500);
        assert_eq!(u.group_rss_bytes[&100], 1000);
        assert_eq!(u.group_rss_bytes[&101], 500);
        assert_eq!(u.peak_rss_bytes, 1500);
        assert_eq!(u.escapees.len(), 1);
        assert_eq!(u.escapees[0].pid, 200);
        // Second scan: less memory, escapee gone; peak stays, escapee listed as dead.
        let raw2 = RawScan {
            all: vec![p(100, 1, 100, 10, 'S', 10)],
            foreign_pinned: None,
            read_errors: 0,
        };
        let out2 = fold(&raw2, &mut tracked);
        assert_eq!(out2[&100].rss_bytes, 10);
        assert_eq!(out2[&100].peak_rss_bytes, 1500);
        assert!(!out2[&100].escapees[0].alive);
    }

    #[test]
    fn recycled_parent_pid_is_not_an_escape() {
        let mut tracked: BTreeMap<u32, Track> = BTreeMap::new();
        tracked.insert(100, Track::default());
        let raw = RawScan {
            all: vec![p(100, 1, 100, 10, 'S', 1), p(101, 100, 100, 11, 'S', 1)],
            foreign_pinned: None,
            read_errors: 0,
        };
        fold(&raw, &mut tracked);
        // 101 is now a different process (start 99) with a child in another session.
        let raw2 = RawScan {
            all: vec![
                p(100, 1, 100, 10, 'S', 1),
                p(101, 1, 500, 99, 'S', 1),
                p(501, 101, 501, 100, 'S', 1),
            ],
            foreign_pinned: None,
            read_errors: 0,
        };
        let out = fold(&raw2, &mut tracked);
        assert!(out[&100].escapees.is_empty());
    }

    #[test]
    fn sampler_publishes_own_session() {
        let sid = super::super::read_stat(std::process::id())
            .unwrap()
            .unwrap()
            .session;
        let s = Sampler::spawn(SamplerConfig {
            interval: Duration::from_millis(200),
            collect_foreign_pinning: false,
        })
        .unwrap();
        let t0 = Instant::now();
        s.track(sid);
        let snap = s
            .wait_for_scan_after(t0, Duration::from_secs(120))
            .expect("a scan within 120 s");
        let u = snap.session(sid).unwrap();
        assert!(u.members.iter().any(|m| m.pid == std::process::id()));
        assert!(u.rss_bytes > 0);
        s.untrack(sid);
    }
}
