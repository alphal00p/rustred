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

use super::{ProcStat, live_thread_stat, read_stat};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

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
}

impl SessionUsage {
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
}

impl Snapshot {
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

struct Shared {
    tracked: Mutex<BTreeMap<u32, Track>>,
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
    while !shared.stop.load(Ordering::SeqCst) {
        let sids: BTreeSet<u32> = lock(&shared.tracked).keys().copied().collect();
        let started_at = Instant::now();
        let raw = scan_raw(&sids, shared.config.collect_foreign_pinning);
        let finished_at = Instant::now();
        seq += 1;
        let sessions = {
            let mut tracked = lock(&shared.tracked);
            fold(&raw, &mut tracked)
        };
        let snap = Arc::new(Snapshot {
            seq,
            started_at,
            finished_at,
            sessions,
            foreign_pinned: raw.foreign_pinned,
            processes_seen: raw.all.len(),
            read_errors: raw.read_errors,
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
    let ncpu = super::allowed_cpus().map(|s| s.len()).unwrap_or(0).max(
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
        let st = if st.is_dead() && tracked.contains(&st.session) {
            // A dead main thread with live threads still holds memory.
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
        if let Some(u) = out.get_mut(&sid) {
            u.escapees = track.escapees.values().cloned().collect();
        }
    }
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
