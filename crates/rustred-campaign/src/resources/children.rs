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
//! Strictness (RB-C1 a; procfix8: every read through [`procfs`]): every function here reports a failed read (EMFILE,
//! ENFILE, EACCES, EIO, a torn read, a parse failure) as an error, never as "no process". ENOENT / ESRCH of a whole
//! /proc/<pid> directory means that process no longer exists; of a task entry, that THREAD.
//!
//! The reaper: every zombie child of this process that is NOT a registered leader and NOT in the supervisor's own session
//! is reaped by exact pid through a pidfd (`launch::guard::reap_adopted`), at most `max` per tick. Children in our own
//! session are helpers the supervisor spawned itself (std::process::Command): their owner waits for them. Job processes
//! are never in the supervisor's session (every launched child calls setsid before exec). CONTRACT for the supervisor
//! process: do not spawn setsid'd children outside the launcher and expect to wait for them yourself (their status would
//! be taken by this reaper; a wait then fails with ECHILD).

use super::procfs::{self, ProcError, ProcResult};
use super::{Leaders, ProcStat};
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

fn getpid() -> u32 {
    // SAFETY: getpid cannot fail.
    unsafe { libc::getpid() as u32 }
}

/// Children listed in `/proc/<pid>/task/*/children` (all threads), read until two consecutive reads agree. Gone: the
/// process does not exist (procfix8: [`procfs`] rules).
pub(crate) fn children_of(pid: u32) -> ProcResult<BTreeSet<u32>> {
    procfs::sys().children_of(pid)
}

/// Our own direct children from the kernel's children lists, read until two consecutive reads agree (at most 4 reads;
/// the kernel documents the file as unreliable while the list changes). procfix8: lists that keep changing are an
/// error (it was the last read).
pub(crate) fn own_children() -> Result<BTreeSet<u32>, String> {
    match children_of(getpid()) {
        Ok(k) => Ok(k),
        Err(ProcError::Gone) => Err("own /proc entry missing".into()),
        Err(ProcError::Unreadable(w)) => Err(w),
    }
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

/// Every process of /proc with its stat, STRICT: a failed listing (the two-enumeration rule) or a failed stat read
/// (other than the process having vanished: ENOENT / ESRCH) is an error. Vanished processes are counted in the second
/// value.
pub(crate) fn strict_listing() -> Result<Vec<ProcStat>, String> {
    Ok(strict_listing_counted()?.0)
}

pub(crate) fn strict_listing_counted() -> Result<(Vec<ProcStat>, usize), String> {
    let root = procfs::sys();
    let mut out = Vec::new();
    let mut vanished = 0usize;
    for pid in root.pids().map_err(|e| format!("proc listing: {e}"))? {
        match root.stat(pid) {
            Ok(st) => out.push(st),
            Err(ProcError::Gone) => vanished += 1,
            Err(ProcError::Unreadable(w)) => return Err(w),
        }
    }
    Ok((out, vanished))
}

/// What the environment of a process says about the campaign marker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum MarkerFact {
    Value(String),
    Absent,
    /// The environ of a live process stayed empty (`env -i`): no marker NOW. Never cached (a later exec may change it).
    EmptyEnviron,
    /// EACCES (non-dumpable / other uid), a torn read, a timeout, the live thread read through exited, or another read
    /// error: unknown (procfix8: never Absent).
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
    /// The PROCESS is gone (ENOENT / ESRCH of its stat, task list or status; never of a thread).
    Gone,
    Facts(ProcFacts),
    Failed(String),
}

/// Bound of the environ read of [`read_proc_facts`] (an environ read takes the target's mmap lock).
const FACTS_ENVIRON_TIMEOUT: Duration = Duration::from_secs(1);

/// Thread states: (any live thread, all live threads stopped, a live thread id). Gone: the process is gone. A thread
/// that vanished between the listing and its read is left out (ENOENT / ESRCH of ITS stat).
fn thread_states(root: &procfs::ProcRoot, pid: u32) -> ProcResult<(bool, bool, Option<u32>)> {
    let (mut any_live, mut all_stopped, mut live_tid) = (false, true, None);
    for tid in root.tasks(pid)? {
        match root.task_stat(pid, tid) {
            Ok(st) => {
                if !st.is_dead() {
                    any_live = true;
                    live_tid.get_or_insert(tid);
                    if !matches!(st.state, 'T' | 't') {
                        all_stopped = false;
                    }
                }
            }
            Err(ProcError::Gone) => {}
            Err(e) => return Err(e),
        }
    }
    Ok((any_live, all_stopped, live_tid))
}

/// Read the facts of `pid` strictly. `want_marker`: also read the environ (else `marker` = Absent, unused).
/// procfix8: every read through [`procfs`]: Gone only for ENOENT / ESRCH of the process; everything else Failed (or
/// `MarkerFact::Unreadable` for the environ).
pub(crate) fn read_proc_facts(pid: u32, want_marker: bool) -> FactRead {
    let root = procfs::sys();
    let stat = match root.stat(pid) {
        Ok(s) => s,
        Err(ProcError::Gone) => return FactRead::Gone,
        Err(ProcError::Unreadable(w)) => return FactRead::Failed(w),
    };
    let (any_live, all_stopped, mut live_tid) = match thread_states(&root, pid) {
        Ok(x) => x,
        Err(ProcError::Gone) => return FactRead::Gone,
        Err(ProcError::Unreadable(w)) => return FactRead::Failed(w),
    };
    // procfix7 round 5 (gate9 G9-B2): "dead" (a zombie without live threads) ONLY by the kernel's atomic thread count
    // (`live_thread_at`), never because one task listing showed no live thread (threads replacing themselves are
    // created after the listing began). `all_stopped` stays a listing fact: it is consulted only after a SIGSTOP was
    // delivered to the GROUP, and from then on the kernel stops every thread, a thread created later included
    // (task_join_group_stop), and refuses new clones while the stop is pending (signal_pending in copy_process); with
    // no live thread listed it is false.
    let dead = if stat.is_dead() {
        match super::live_thread_at(&root, pid) {
            Ok(super::LiveThread::NoneAlive) => true,
            Ok(super::LiveThread::Found(t, _)) => {
                live_tid.get_or_insert(t);
                false
            }
            Err(ProcError::Gone) => return FactRead::Gone,
            Err(ProcError::Unreadable(w)) => return FactRead::Failed(w),
        }
    } else {
        false
    };
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
    let real_uid = match root.status(pid).and_then(|s| s.real_uid()) {
        Ok(u) => u,
        Err(ProcError::Gone) => return FactRead::Gone,
        Err(ProcError::Unreadable(w)) => return FactRead::Failed(w),
    };
    let marker = if want_marker {
        let tid = match (stat.is_dead(), live_tid) {
            (true, Some(t)) => Some(t),
            _ => None,
        };
        // The execve window (observed: 16 of 4,000 self-ending jobs with a setsid'd `sleep` read empty once) is handled
        // by evidence inside `ProcRoot::environ` (gate8 m8-ew); an EMPTY result is a real empty environment (`env -i`).
        match root.environ(pid, tid, FACTS_ENVIRON_TIMEOUT) {
            Ok(b) if b.is_empty() => MarkerFact::EmptyEnviron,
            Ok(b) => match super::marker_in_environ(&b) {
                Some(v) => MarkerFact::Value(v),
                None => MarkerFact::Absent,
            },
            Err(ProcError::Gone) if tid.is_none() => return FactRead::Gone,
            Err(ProcError::Gone) => MarkerFact::Unreadable(format!(
                "the live thread {} of {pid} exited while its environ was read",
                tid.unwrap_or(0)
            )),
            Err(ProcError::Unreadable(w)) => MarkerFact::Unreadable(w),
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
        let st = match super::read_stat(pid) {
            Ok(st) => st,
            Err(ProcError::Gone) => continue,
            // procfix8: not reaped this tick (it waits), counted
            Err(ProcError::Unreadable(_)) => {
                outstanding += 1;
                continue;
            }
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
        #[allow(clippy::let_underscore_must_use)]
        // ignored-ok (procfix7): without the reaper thread zombies wait; leaders are reaped by the launcher
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
