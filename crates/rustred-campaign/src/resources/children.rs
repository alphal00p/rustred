//! OUR CHILDREN (the kernel's children lists), strict per-process facts, and the ADOPTED-CHILDREN REAPER
//! (gate re-audit B: RB-C1, RB-C2, RB-M1; re-audit A m-4).
//!
//! Why children lists: with the supervisor as child subreaper (or as pid 1 of a namespace) every orphaned descendant of
//! a job is re-parented to the supervisor. Every live process a job ever created is therefore, at any moment, in the
//! supervisor's descendant tree, reachable through `/proc/<pid>/task/<tid>/children` from the supervisor's own threads.
//! A process that keeps re-forking into new sessions always has a live member that is a child of ours or of a member we
//! have already stopped. Kernel guarantee relied upon: a child's pid number cannot be reused until its PARENT reaps it,
//! and the only reapers of the supervisor's children are this module (exact pids, never a catch-all wait) and the
//! launcher (registered leaders, under the registry lock): a pidfd opened on a child pins that child.
//!
//! Strictness (RB-C1 a): every function here reports a failed read (EMFILE, ENFILE, EACCES, EIO, a parse failure) as an
//! error, never as "no process". ENOENT / ESRCH of a whole /proc/<pid> directory means that process no longer exists.
//!
//! The reaper: every zombie child of this process that is NOT a registered leader and NOT in the supervisor's own session
//! is reaped by exact pid through a pidfd (`launch::guard::reap_adopted`), at most `max` per tick. Children in our own
//! session are helpers the supervisor spawned itself (std::process::Command): their owner waits for them. Job processes
//! are never in the supervisor's session (every launched child calls setsid before exec). CONTRACT for the supervisor
//! process: do not spawn setsid'd children outside the launcher and expect to wait for them yourself (their status would
//! be taken by this reaper; a wait then fails with ECHILD).

use super::{Leaders, ProcStat, page_size, parse_stat};
use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

fn is_gone(e: &io::Error) -> bool {
    matches!(e.raw_os_error(), Some(libc::ENOENT) | Some(libc::ESRCH))
}

fn getpid() -> u32 {
    // SAFETY: getpid cannot fail.
    unsafe { libc::getpid() as u32 }
}

/// Children listed in `/proc/<pid>/task/*/children` (all threads). `Ok(None)`: the process does not exist.
pub(crate) fn children_of(pid: u32) -> Result<Option<BTreeSet<u32>>, String> {
    let task = format!("/proc/{pid}/task");
    let rd = match fs::read_dir(&task) {
        Ok(r) => r,
        Err(e) if is_gone(&e) => return Ok(None),
        Err(e) => return Err(format!("{task}: {e}")),
    };
    let mut out = BTreeSet::new();
    for entry in rd {
        let entry = entry.map_err(|e| format!("{task}: {e}"))?;
        let Some(tid) = entry
            .file_name()
            .to_str()
            .and_then(|s| s.parse::<u32>().ok())
        else {
            continue;
        };
        let f = format!("{task}/{tid}/children");
        match fs::read_to_string(&f) {
            Ok(t) => {
                for w in t.split_whitespace() {
                    out.insert(
                        w.parse::<u32>()
                            .map_err(|_| format!("{f}: unparsable {w:?}"))?,
                    );
                }
            }
            // A thread that exited between the listing and the read has no children to report.
            Err(e) if is_gone(&e) => {}
            Err(e) => return Err(format!("{f}: {e}")),
        }
    }
    Ok(Some(out))
}

/// Our own direct children from the kernel's children lists, read until two consecutive reads agree (at most 4 reads;
/// the kernel documents the file as unreliable while the list changes).
pub(crate) fn own_children() -> Result<BTreeSet<u32>, String> {
    let me = getpid();
    let mut prev = children_of(me)?.ok_or("own /proc entry missing")?;
    for _ in 0..3 {
        let next = children_of(me)?.ok_or("own /proc entry missing")?;
        if next == prev {
            return Ok(next);
        }
        prev = next;
    }
    Ok(prev)
}

/// Our direct children by a STRICT full /proc listing (ppid == us): an independent second enumeration. Any listing or
/// stat failure other than "gone" is an error.
pub(crate) fn own_children_by_scan() -> Result<BTreeSet<u32>, String> {
    let me = getpid();
    let mut out = BTreeSet::new();
    for st in strict_listing()? {
        if st.ppid == me {
            out.insert(st.pid);
        }
    }
    Ok(out)
}

/// Every process of /proc with its stat, STRICT: a failed listing entry or a failed stat read (other than the process
/// having vanished) is an error. Vanished processes are counted in the second value.
pub(crate) fn strict_listing() -> Result<Vec<ProcStat>, String> {
    Ok(strict_listing_counted()?.0)
}

pub(crate) fn strict_listing_counted() -> Result<(Vec<ProcStat>, usize), String> {
    let rd = fs::read_dir("/proc").map_err(|e| format!("/proc: {e}"))?;
    let mut out = Vec::new();
    let mut vanished = 0usize;
    for entry in rd {
        let entry = entry.map_err(|e| format!("/proc listing: {e}"))?;
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|s| s.parse::<u32>().ok())
        else {
            continue;
        };
        let p = format!("/proc/{pid}/stat");
        match fs::read(&p) {
            Ok(b) => match parse_stat(pid, &String::from_utf8_lossy(&b), page_size()) {
                Some(st) => out.push(st),
                None => return Err(format!("{p}: unparsable")),
            },
            Err(e) if is_gone(&e) => vanished += 1,
            Err(e) => return Err(format!("{p}: {e}")),
        }
    }
    Ok((out, vanished))
}

/// Read an environ file; an EMPTY read is repeated once after 1 ms. Why: during execve the new mm is installed before
/// its environment bounds are set (`env_start == env_end`), so a marked process caught in that window reads as empty
/// (observed: 16 of 4,000 self-ending jobs with a setsid'd `sleep` were judged unmarked that way). The window is
/// microseconds; an environment that is still empty 1 ms later is a real empty environment (`env -i`). Ok(None): empty.
fn read_environ_settled(path: &str) -> io::Result<Option<Vec<u8>>> {
    let b = fs::read(path)?;
    if !b.is_empty() {
        return Ok(Some(b));
    }
    std::thread::sleep(Duration::from_millis(1));
    let b = fs::read(path)?;
    Ok((!b.is_empty()).then_some(b))
}

/// What the environment of a process says about the campaign marker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum MarkerFact {
    Value(String),
    Absent,
    /// The environ of a live process stayed empty (`env -i`): no marker NOW. Never cached (a later exec may change it).
    EmptyEnviron,
    /// EACCES (non-dumpable / other uid), an empty environ of a live process, or another read error.
    Unreadable(String),
}

/// Strict facts about one process (read on the calling thread; callers run off the control loop).
#[derive(Clone, Debug)]
pub(crate) struct ProcFacts {
    /// Stat of the main thread, or of a live thread when the main thread is a zombie.
    pub stat: ProcStat,
    /// A zombie without live threads (no survivor).
    pub dead: bool,
    /// Real uid from /proc/<pid>/status (readable also for non-dumpable processes).
    pub real_uid: u32,
    pub marker: MarkerFact,
    /// Every live thread is stopped (state T or t).
    pub all_stopped: bool,
}

pub(crate) enum FactRead {
    Gone,
    Facts(ProcFacts),
    Failed(String),
}

fn read_status_ruid(pid: u32) -> Result<Option<u32>, String> {
    let p = format!("/proc/{pid}/status");
    match fs::read_to_string(&p) {
        Ok(t) => t
            .lines()
            .find_map(|l| l.strip_prefix("Uid:"))
            .and_then(|r| r.split_whitespace().next())
            .and_then(|v| v.parse::<u32>().ok())
            .map(Some)
            .ok_or_else(|| format!("{p}: no Uid line")),
        Err(e) if is_gone(&e) => Ok(None),
        Err(e) => Err(format!("{p}: {e}")),
    }
}

/// Thread states: (any live thread, all live threads stopped, a live thread id).
fn thread_states(pid: u32) -> Result<Option<(bool, bool, Option<u32>)>, String> {
    let task = format!("/proc/{pid}/task");
    let rd = match fs::read_dir(&task) {
        Ok(r) => r,
        Err(e) if is_gone(&e) => return Ok(None),
        Err(e) => return Err(format!("{task}: {e}")),
    };
    let (mut any_live, mut all_stopped, mut live_tid) = (false, true, None);
    for entry in rd {
        let entry = entry.map_err(|e| format!("{task}: {e}"))?;
        let Some(tid) = entry
            .file_name()
            .to_str()
            .and_then(|s| s.parse::<u32>().ok())
        else {
            continue;
        };
        let f = format!("{task}/{tid}/stat");
        match fs::read(&f) {
            // A task being released reads as empty: it is gone.
            Ok(b) if b.is_empty() => {}
            Ok(b) => {
                let st = parse_stat(pid, &String::from_utf8_lossy(&b), page_size())
                    .ok_or_else(|| format!("{f}: unparsable"))?;
                if !st.is_dead() {
                    any_live = true;
                    live_tid.get_or_insert(tid);
                    if !matches!(st.state, 'T' | 't') {
                        all_stopped = false;
                    }
                }
            }
            Err(e) if is_gone(&e) => {}
            Err(e) => return Err(format!("{f}: {e}")),
        }
    }
    Ok(Some((any_live, all_stopped, live_tid)))
}

/// Read the facts of `pid` strictly. `want_marker`: also read the environ (else `marker` = Absent, unused).
pub(crate) fn read_proc_facts(pid: u32, want_marker: bool) -> FactRead {
    let sp = format!("/proc/{pid}/stat");
    let stat = match fs::read(&sp) {
        Ok(b) if b.is_empty() => return FactRead::Gone,
        Ok(b) => match parse_stat(pid, &String::from_utf8_lossy(&b), page_size()) {
            Some(s) => s,
            None => return FactRead::Failed(format!("{sp}: unparsable")),
        },
        Err(e) if is_gone(&e) => return FactRead::Gone,
        Err(e) => return FactRead::Failed(format!("{sp}: {e}")),
    };
    let (any_live, all_stopped, live_tid) = match thread_states(pid) {
        Ok(Some(x)) => x,
        Ok(None) => return FactRead::Gone,
        Err(e) => return FactRead::Failed(e),
    };
    let dead = stat.is_dead() && !any_live;
    if dead {
        // A zombie without live threads: no survivor; nothing more to read (fast path for re-parented zombies).
        return FactRead::Facts(ProcFacts {
            stat,
            dead,
            real_uid: u32::MAX,
            marker: MarkerFact::Absent,
            all_stopped: false,
        });
    }
    let real_uid = match read_status_ruid(pid) {
        Ok(Some(u)) => u,
        Ok(None) => return FactRead::Gone,
        Err(e) => return FactRead::Failed(e),
    };
    let marker = if want_marker && !dead {
        let path = match (stat.is_dead(), live_tid) {
            (true, Some(t)) => format!("/proc/{pid}/task/{t}/environ"),
            _ => format!("/proc/{pid}/environ"),
        };
        match read_environ_settled(&path) {
            Ok(Some(b)) => match super::marker_in_environ(&b) {
                Some(v) => MarkerFact::Value(v),
                None => MarkerFact::Absent,
            },
            // Empty twice, 1 ms apart: a live process without environment (`env -i`).
            Ok(None) => MarkerFact::EmptyEnviron,
            Err(e) if is_gone(&e) => return FactRead::Gone,
            Err(e) => MarkerFact::Unreadable(format!("{path}: {e}")),
        }
    } else {
        MarkerFact::Absent
    };
    FactRead::Facts(ProcFacts {
        stat,
        dead,
        real_uid,
        marker,
        all_stopped: any_live && all_stopped,
    })
}

// ------------------------------------------------------------------------------------------------ the reaper

/// Counters of the adopted-children reaper (process-wide).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct ReaperStats {
    /// Zombies reaped since the process started.
    pub reaped_total: u64,
    /// Zombie children seen in the latest tick that were NOT reaped (registered leaders, own-session helpers, or a
    /// reap that failed).
    pub outstanding: u64,
    /// Ticks run.
    pub ticks: u64,
    /// The latest tick could not enumerate our children (e.g. EMFILE).
    pub last_tick_failed: bool,
}

struct ReaperState {
    reaped: AtomicU64,
    outstanding: AtomicU64,
    ticks: AtomicU64,
    failed: AtomicBool,
    wake: Mutex<bool>,
    cv: Condvar,
}

fn state() -> &'static ReaperState {
    static S: OnceLock<ReaperState> = OnceLock::new();
    S.get_or_init(|| ReaperState {
        reaped: AtomicU64::new(0),
        outstanding: AtomicU64::new(0),
        ticks: AtomicU64::new(0),
        failed: AtomicBool::new(false),
        wake: Mutex::new(false),
        cv: Condvar::new(),
    })
}

/// The reaper's counters.
pub fn reaper_stats() -> ReaperStats {
    let s = state();
    ReaperStats {
        reaped_total: s.reaped.load(Ordering::SeqCst),
        outstanding: s.outstanding.load(Ordering::SeqCst),
        ticks: s.ticks.load(Ordering::SeqCst),
        last_tick_failed: s.failed.load(Ordering::SeqCst),
    }
}

/// Ask the reaper thread for a tick now (non-blocking).
pub fn request_reap() {
    let s = state();
    *s.wake.lock().unwrap_or_else(|p| p.into_inner()) = true;
    s.cv.notify_all();
}

/// One reaper tick: reap at most `max` zombie children that are not registered leaders and not in our own session.
/// Returns how many were reaped.
pub(crate) fn reap_tick(leaders: &Leaders, max: usize) -> usize {
    let s = state();
    s.ticks.fetch_add(1, Ordering::SeqCst);
    let kids = match own_children() {
        Ok(k) => k,
        Err(_) => {
            s.failed.store(true, Ordering::SeqCst);
            return 0;
        }
    };
    s.failed.store(false, Ordering::SeqCst);
    // SAFETY: getsid(0) reads the caller's session id.
    let own_session = unsafe { libc::getsid(0) }.max(0) as u32;
    let me = getpid();
    let mut reaped = 0usize;
    let mut outstanding = 0u64;
    for pid in kids {
        let Ok(Some(st)) = super::read_stat(pid) else {
            continue;
        };
        if !st.is_dead() || st.ppid != me {
            continue;
        }
        if st.session == own_session || reaped >= max {
            outstanding += 1;
            continue;
        }
        let guard = leaders.lock();
        if guard.contains(&pid) {
            // A registered leader: its zombie pins the job's session until the launcher releases it. Never reaped here.
            drop(guard);
            outstanding += 1;
            continue;
        }
        let ok = crate::launch::reap_adopted(pid, st.start_ticks, own_session);
        drop(guard);
        if ok {
            reaped += 1;
        } else {
            outstanding += 1;
        }
    }
    s.reaped.fetch_add(reaped as u64, Ordering::SeqCst);
    s.outstanding.store(outstanding, Ordering::SeqCst);
    reaped
}

/// Start the process-wide reaper thread once (idempotent): a tick every `period` or on [`request_reap`].
pub(crate) fn start_reaper(leaders: Arc<Leaders>, period: Duration, max_per_tick: usize) {
    static STARTED: OnceLock<()> = OnceLock::new();
    STARTED.get_or_init(|| {
        let _ = std::thread::Builder::new()
            .name("rr-adopted-reaper".into())
            .spawn(move || {
                let s = state();
                loop {
                    let n = reap_tick(&leaders, max_per_tick);
                    let mut w = s.wake.lock().unwrap_or_else(|p| p.into_inner());
                    // A full tick: go again at once (a storm of zombies is drained without waiting a period).
                    if n >= max_per_tick {
                        *w = false;
                        continue;
                    }
                    let deadline = Instant::now() + period;
                    while !*w {
                        let now = Instant::now();
                        if now >= deadline {
                            break;
                        }
                        w = match s.cv.wait_timeout(w, deadline - now) {
                            Ok((g, _)) => g,
                            Err(p) => p.into_inner().0,
                        };
                    }
                    *w = false;
                }
            });
    });
}
