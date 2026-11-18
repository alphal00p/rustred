//! Linux process identities, host memory, CPU sets and idleness, and measured resource samples of child SESSIONS.
//!
//! Provenance: the stat parser, `sample`, `allowed_cpus`, `ticks_per_second` and the cgroup-v2 headroom walk of
//! `host_available` are copied and adapted from `crates/rustred-app/src/cli/shards/resources.rs` at commit b9f06de7
//! (RustRed batch 6), as ruled in `plan/auto-campaign/interfaces/shards-extraction.md` section 5. Changes against the
//! source: the identity gains the boot id; samples carry ppid, session and state; accounting is per SESSION and per
//! process GROUP (children fork helpers, some of which change their group); scans run on a helper thread
//! ([`Sampler`]) so that a `/proc/<pid>` read that blocks in the kernel never stalls the control loop (the lesson
//! recorded in `ops/guarded_run.py`); errors are this crate's `io::Error`, not the app's `CliError`.
//!
//! Reading guide: [`ProcessIdentity`] is the only thing that may authorise a signal. [`Sampler`] publishes
//! [`Snapshot`]s; the control loop reads [`Sampler::latest`] and never waits for a scan.

// procfix7 (gate7 B1): an ignored Result is a reviewed exception (`#[allow]` + `ignored-ok (procfix7):` reason),
// never a default; tests/procfix7_tripwire.rs checks every `let _ =` of the process layer and the supervisor.
#![deny(clippy::let_underscore_must_use, unused_must_use)]
#[cfg(not(target_os = "linux"))]
compile_error!(
    "rustred-campaign's process layer (resources, launch, cpu_policy) requires Linux: /proc, sched_setaffinity, prctl"
);

mod bounded;
mod children;
pub mod lineage;
mod marked;
mod pid;
pub mod procfs;
mod scan;

pub use bounded::{
    Bounded, MAX_STUCK, ReadOutcome, outstanding_helpers, read_bounded, run_bounded,
    run_bounded_with,
};
pub(crate) use children::listing_hold;
#[cfg(debug_assertions)]
pub use children::set_listing_hold;
pub(crate) use children::{
    FactRead, MarkerFact, ProcFacts, children_of, own_children, own_children_by_scan,
    read_proc_facts, reap_gate, start_reaper, strict_listing_counted,
};
pub use children::{ReaperStats, reaper_stats, request_reap};
pub use lineage::{HiddenProcess, describe_hidden};
pub use marked::{
    CampaignMarker, CampaignScan, ENVIRON_READ_TIMEOUT, InvalidMarker, MARKER_VAR, MarkedProcess,
    MarkerRead, environ_readable_at, hidden_descendants, hidden_descendants_at, marker_in_environ,
    our_uid, owner_uid_at, read_marker_at, scan_campaign, validate_campaign_id,
};
pub(crate) use marked::{
    CandidateCheck, STRICT_ENVIRON_TIMEOUT, check_candidate, scan_marked_inline,
};
pub use pid::{InvalidPid, PID_MAX_LIMIT, ValidPid, pid_max};
pub use procfs::{ProcError, ProcResult, ProcRoot};
pub(crate) use scan::Leaders;
pub use scan::{
    EscapeeRecord, MemKind, Sampler, SamplerConfig, SamplerHealth, SessionUsage, Snapshot,
    scan_sessions_now,
};

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// One `/proc/<pid>/stat` line, the fields this crate uses.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcStat {
    pub pid: u32,
    /// Command name (field 2, without the parentheses; may contain spaces and ')').
    pub comm: String,
    /// State letter: R, S, D, Z, T, t, X, I ...
    pub state: char,
    pub ppid: u32,
    pub pgrp: u32,
    pub session: u32,
    /// utime + stime, in clock ticks.
    pub cpu_ticks: u64,
    /// Start time after boot, in clock ticks (field 22). With the boot id this identifies a process.
    pub start_ticks: u64,
    pub rss_bytes: u64,
}

impl ProcStat {
    /// Zombie or dead: holds no memory, cannot be signalled usefully.
    pub fn is_dead(&self) -> bool {
        matches!(self.state, 'Z' | 'X' | 'x')
    }
}

/// Parse a stat line. `comm` may itself contain spaces and ')' characters: split after the LAST ')'.
/// (Copied from shards/resources.rs `parse_stat` at b9f06de7, extended with comm, state, ppid, session.)
pub fn parse_stat(pid: u32, text: &str, page_size: u64) -> Option<ProcStat> {
    let open = text.find('(')?;
    let close = text.rfind(')')?;
    let comm = text.get(open + 1..close)?.to_string();
    let fields: Vec<&str> = text.get(close + 2..)?.split_whitespace().collect();
    Some(ProcStat {
        pid,
        comm,
        state: fields.first()?.chars().next()?,
        ppid: fields.get(1)?.parse().ok()?,
        pgrp: fields.get(2)?.parse().ok()?,
        session: fields.get(3)?.parse().ok()?,
        cpu_ticks: fields
            .get(11)?
            .parse::<u64>()
            .ok()?
            .checked_add(fields.get(12)?.parse::<u64>().ok()?)?,
        start_ticks: fields.get(19)?.parse().ok()?,
        rss_bytes: fields
            .get(21)?
            .parse::<u64>()
            .ok()?
            .checked_mul(page_size)?,
    })
}

/// `sysconf(_SC_PAGESIZE)`, cached.
pub fn page_size() -> u64 {
    static PAGE: OnceLock<u64> = OnceLock::new();
    *PAGE.get_or_init(|| {
        // SAFETY: sysconf has no preconditions and touches no memory of ours.
        let page = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
        if page > 0 { page as u64 } else { 4096 }
    })
}

/// `sysconf(_SC_CLK_TCK)`, cached (copied from shards/resources.rs `ticks_per_second`).
pub fn ticks_per_second() -> f64 {
    static TICKS: OnceLock<f64> = OnceLock::new();
    *TICKS.get_or_init(|| {
        // SAFETY: sysconf has no preconditions and touches no memory of ours.
        let t = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
        if t > 0 { t as f64 } else { 100.0 }
    })
}

/// Read `/proc/<pid>/stat` (procfix8: [`procfs`] rules: Gone only for ENOENT / ESRCH; a torn or unparsable stat is
/// Unreadable, never gone). This read can block in the kernel (see guarded_run.py): call it from a helper thread or
/// for single pids only.
pub fn read_stat(pid: u32) -> ProcResult<ProcStat> {
    procfs::sys().stat(pid)
}

/// A live thread of a thread group (for a process whose main thread is a zombie: a dead leader with live threads still
/// holds memory and can still fork; guarded_run.py `live_thread`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LiveThread {
    /// A live thread: its id and its stat (the pid field is the process's pid).
    Found(u32, ProcStat),
    /// The kernel's ATOMIC thread count (`/proc/<pid>/status` `Threads:`) is 1 with the leader a zombie: no other
    /// thread exists, and none can appear (only a live thread can create one).
    NoneAlive,
}

/// Task listings [`live_thread_at`] tries to catch a live thread in while the atomic count says the group has more
/// than its zombie leader.
pub const LIVE_THREAD_LISTINGS: usize = 8;

/// The first live thread of `pid` under `root`. Gone: the process is gone.
///
/// procfix7 round 5 (gate9 G9-B2, a NON-ATOMIC observation taken as a proof of absence): "every thread of ONE task
/// listing reads dead or gone" is NOT "no live thread": a thread created after the listing began by a listed thread that
/// then exits is never examined (a zombie leader whose threads replace themselves every 50 us was "a zombie without a
/// live thread" for every scan, and its kill was dropped). The proof of absence is the kernel's ATOMIC count:
/// `Threads:` (signal->nr_threads, one read) == 1 with the leader a zombie => [`LiveThread::NoneAlive`] (sound: the
/// zombie leader stays counted until every other thread is released, and no thread can appear without a live thread to
/// create it). Any larger count: the group may live; up to [`LIVE_THREAD_LISTINGS`] listings look for a live thread
/// (whose id the environ is read through); none caught => Unreadable ("possibly ours": inconclusive, named), NEVER
/// NoneAlive. A thread that vanished between a listing and its read (ENOENT / ESRCH of ITS stat) is skipped; any other
/// failed read is Unreadable.
pub fn live_thread_at(root: &ProcRoot, pid: u32) -> ProcResult<LiveThread> {
    let mut counted = 0;
    for _ in 0..LIVE_THREAD_LISTINGS {
        let status = root.status(pid)?;
        counted = status.threads()?;
        let leader_dead = matches!(status.state()?, 'Z' | 'X' | 'x');
        if leader_dead && counted <= 1 {
            return Ok(LiveThread::NoneAlive);
        }
        for tid in root.tasks(pid)? {
            match root.task_stat(pid, tid) {
                Ok(st) if !st.is_dead() => return Ok(LiveThread::Found(tid, st)),
                Ok(_) | Err(ProcError::Gone) => {}
                Err(e) => return Err(e),
            }
        }
    }
    Err(ProcError::Unreadable(format!(
        "pid {pid}: the kernel counts {counted} thread(s) (status Threads:) but no live thread was caught in \
         {LIVE_THREAD_LISTINGS} task listings (threads replacing themselves?): possibly alive, possibly ours"
    )))
}

/// [`live_thread_at`] on the real /proc.
pub fn live_thread(pid: u32) -> ProcResult<LiveThread> {
    live_thread_at(&procfs::sys(), pid)
}

/// `/proc/sys/kernel/random/boot_id`, cached for the life of the process.
pub fn boot_id() -> ProcResult<String> {
    static BOOT: OnceLock<String> = OnceLock::new();
    if let Some(b) = BOOT.get() {
        return Ok(b.clone());
    }
    let b = procfs::sys().boot_id()?;
    Ok(BOOT.get_or_init(|| b).clone())
}

/// The identity of a child: (pid, start time in ticks after boot, boot id). Rule: a signal is sent to a pid (or to the
/// group or session it leads) only after its CURRENT start time and the current boot id were found equal to the
/// recorded ones. A different boot id proves the recorded process dead; a different start time proves the pid recycled.
///
/// Deserialisation REJECTS pids outside 2..=pid_max (audit B C1/C2: a hand-edited or corrupted record must never name
/// pid 0, 1 or a value that turns negative). The fields stay public for tests and fakes; every signal path re-checks
/// the pid through [`ValidPid`] anyway.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "RawIdentity")]
pub struct ProcessIdentity {
    pub pid: u32,
    pub start_ticks: u64,
    pub boot_id: String,
}

#[derive(Deserialize)]
struct RawIdentity {
    pid: i64,
    start_ticks: u64,
    boot_id: String,
}

impl TryFrom<RawIdentity> for ProcessIdentity {
    type Error = InvalidPid;
    fn try_from(r: RawIdentity) -> Result<Self, InvalidPid> {
        let pid = ValidPid::new(r.pid)?.get();
        Ok(Self {
            pid,
            start_ticks: r.start_ticks,
            boot_id: r.boot_id,
        })
    }
}

/// What a recorded identity corresponds to now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Liveness {
    /// Same pid, same start time, same boot: it is the recorded process (possibly a zombie: `zombie`).
    Alive { zombie: bool },
    /// No process with that pid.
    Gone,
    /// The pid exists but started at another time: recycled, NOT ours.
    Recycled { current_start_ticks: u64 },
    /// Recorded under another boot: provably dead.
    OtherBoot,
}

impl ProcessIdentity {
    /// Identity of a live pid, read now.
    /// Refuses pids outside 2..=pid_max (`InvalidPid` payload).
    pub fn capture(pid: u32) -> io::Result<Self> {
        ValidPid::new(i64::from(pid)).map_err(InvalidPid::into_io)?;
        let st = read_stat(pid).map_err(|e| match e {
            ProcError::Gone => io::Error::new(
                io::ErrorKind::NotFound,
                format!("pid {pid} vanished before its identity was read"),
            ),
            e => io::Error::other(format!("identity of pid {pid}: {e}")),
        })?;
        Ok(Self {
            pid,
            start_ticks: st.start_ticks,
            boot_id: boot_id()?,
        })
    }

    /// Errors with an `InvalidPid` payload for a pid outside 2..=pid_max (no /proc read is made).
    pub fn check(&self) -> io::Result<Liveness> {
        ValidPid::new(i64::from(self.pid)).map_err(InvalidPid::into_io)?;
        if self.boot_id != boot_id()? {
            return Ok(Liveness::OtherBoot);
        }
        match read_stat(self.pid) {
            Err(ProcError::Gone) => Ok(Liveness::Gone),
            // procfix8: unreadable is an error, never "gone"
            Err(e) => Err(io::Error::other(format!("pid {}: {e}", self.pid))),
            Ok(st) if st.start_ticks != self.start_ticks => Ok(Liveness::Recycled {
                current_start_ticks: st.start_ticks,
            }),
            Ok(st) => Ok(Liveness::Alive {
                zombie: st.is_dead(),
            }),
        }
    }

    /// True iff [`Self::check`] says Alive (zombie or not).
    pub fn is_same_process(&self) -> bool {
        matches!(self.check(), Ok(Liveness::Alive { .. }))
    }
}

// ------------------------------------------------------------------------------------------------ host memory

/// Host memory as the admission rule needs it. All in bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemInfo {
    pub mem_total: u64,
    /// `MemAvailable` of /proc/meminfo.
    pub mem_available: u64,
    /// Smallest `memory.max - memory.current` over this process's cgroup-v2 ancestry (None: no limit found).
    pub cgroup_headroom: Option<u64>,
}

impl MemInfo {
    /// What the rule compares with the floor: MemAvailable, reduced to the cgroup headroom when that is smaller
    /// (host RAM alone can overstate what this process may use inside a container or a capped slice).
    ///
    /// Audit A m8: the cgroup headroom gets its OWN margin (zero), not the host floor: the value is
    /// `min(MemAvailable, headroom + floor)`, so that `effective - outstanding - cap >= floor` means for the cgroup
    /// `headroom - outstanding - cap >= 0`. Uses the default 50 GB floor; [`Self::effective_available_for`] takes another.
    pub fn effective_available(&self) -> u64 {
        self.effective_available_for(crate::admission::DEFAULT_FLOOR_BYTES)
    }

    pub fn effective_available_for(&self, floor: u64) -> u64 {
        match self.cgroup_headroom {
            Some(h) => self.mem_available.min(h.saturating_add(floor)),
            None => self.mem_available,
        }
    }
}

/// Parse /proc/meminfo text: (MemTotal, MemAvailable) in bytes.
pub fn parse_meminfo(text: &str) -> Option<(u64, u64)> {
    let field = |name: &str| {
        text.lines().find_map(|line| {
            line.strip_prefix(name)
                .and_then(|s| s.split_whitespace().next())
                .and_then(|s| s.parse::<u64>().ok())
                .and_then(|kb| kb.checked_mul(1024))
        })
    };
    Some((field("MemTotal:")?, field("MemAvailable:")?))
}

/// cgroup-v2 headroom walk (copied from shards/resources.rs `host_available` at b9f06de7). procfix8: an unreadable
/// `/proc/self/cgroup` is an error (host_memory fails: admission charges as if nothing were available), never "no
/// limit"; a host without the unified line has no headroom limit.
fn cgroup_headroom() -> io::Result<Option<u64>> {
    let group = match procfs::sys().self_cgroup_v2()? {
        procfs::CgroupLine::Path(g) => g,
        procfs::CgroupLine::NoUnified => return Ok(None),
    };
    let root = Path::new("/sys/fs/cgroup");
    let mut path = root.join(group.trim_start_matches('/'));
    let mut best: Option<u64> = None;
    // procfix7 round 5: a level's file that is proven absent (the root has none) or says "max" sets no limit; a file
    // that cannot be read or holds anything else is an ERROR (never "no limit": admission would over-admit)
    let read = |p: &Path| -> io::Result<Option<u64>> {
        match crate::fsx::read_to_string("cgroup", p) {
            crate::fsx::Read::Content(t) if t.trim() == "max" => Ok(None),
            crate::fsx::Read::Content(t) => t.trim().parse::<u64>().map(Some).map_err(|e| {
                io::Error::new(io::ErrorKind::InvalidData, format!("{}: {e}", p.display()))
            }),
            crate::fsx::Read::Absent => Ok(None),
            crate::fsx::Read::Failed(e) => {
                Err(io::Error::new(e.kind(), format!("{}: {e}", p.display())))
            }
        }
    };
    loop {
        if let Some(max) = read(&path.join("memory.max"))? {
            let Some(current) = read(&path.join("memory.current"))? else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("{}: memory.max without memory.current", path.display()),
                ));
            };
            let h = max.saturating_sub(current);
            best = Some(best.map_or(h, |b| b.min(h)));
        }
        if path == root || !path.pop() {
            break;
        }
    }
    Ok(best)
}

/// Read host memory now. /proc/meminfo does not block like per-pid files do (guarded_run.py measured it), so this
/// may be called from the control loop.
pub fn host_memory() -> io::Result<MemInfo> {
    let text = procfs::sys().meminfo()?;
    let (mem_total, mem_available) = parse_meminfo(&text).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "MemTotal/MemAvailable missing or malformed",
        )
    })?;
    Ok(MemInfo {
        mem_total,
        mem_available,
        cgroup_headroom: cgroup_headroom()?,
    })
}

// ------------------------------------------------------------------------------------------------ CPU sets

/// CPUs this process may run on (`sched_getaffinity`; copied from shards/resources.rs `allowed_cpus`).
pub fn allowed_cpus() -> io::Result<BTreeSet<usize>> {
    // SAFETY: cpu_set_t is plain data; zeroed is a valid empty set.
    let mut set: libc::cpu_set_t = unsafe { std::mem::zeroed() };
    // SAFETY: pid 0 = this thread; the size and pointer describe `set`, which lives across the call.
    if unsafe { libc::sched_getaffinity(0, std::mem::size_of_val(&set), &mut set) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok((0..libc::CPU_SETSIZE as usize)
        // SAFETY: i < CPU_SETSIZE, the bound of `set`.
        .filter(|&i| unsafe { libc::CPU_ISSET(i, &set) })
        .collect())
}

/// Parse a kernel-style CPU list ("0-3,8,10-11"; also accepts spaces as separators). Errors name the bad part.
/// Audit B M3: every CPU number must be below `CPU_SETSIZE` (1024); a range like "0-60000000" is an error, never
/// materialised.
pub fn parse_cpu_list(text: &str) -> Result<BTreeSet<usize>, String> {
    let limit = libc::CPU_SETSIZE as usize;
    let mut out = BTreeSet::new();
    for part in text
        .split([',', ' ', '\n', '\t'])
        .map(str::trim)
        .filter(|p| !p.is_empty())
    {
        if let Some((a, b)) = part.split_once('-') {
            let a: usize = a
                .trim()
                .parse()
                .map_err(|_| format!("bad CPU range {part:?}"))?;
            let b: usize = b
                .trim()
                .parse()
                .map_err(|_| format!("bad CPU range {part:?}"))?;
            if a > b {
                return Err(format!("bad CPU range {part:?}"));
            }
            if b >= limit {
                return Err(format!("CPU range {part:?} beyond CPU_SETSIZE {limit}"));
            }
            out.extend(a..=b);
        } else {
            let n: usize = part
                .parse()
                .map_err(|_| format!("bad CPU number {part:?}"))?;
            if n >= limit {
                return Err(format!("CPU number {part:?} beyond CPU_SETSIZE {limit}"));
            }
            out.insert(n);
        }
    }
    Ok(out)
}

/// Compact form of a CPU set ("0-3,8").
pub fn format_cpu_list(cpus: &BTreeSet<usize>) -> String {
    let mut parts = Vec::new();
    let mut iter = cpus.iter().copied().peekable();
    while let Some(start) = iter.next() {
        let mut end = start;
        while iter.peek() == Some(&(end + 1)) {
            end = iter.next().unwrap_or(end);
        }
        parts.push(if start == end {
            start.to_string()
        } else {
            format!("{start}-{end}")
        });
    }
    parts.join(",")
}

/// `Cpus_allowed_list` of another process (from /proc/<pid>/status). May block: helper thread.
pub fn affinity_of(pid: u32) -> ProcResult<BTreeSet<usize>> {
    let st = procfs::sys().status(pid)?;
    parse_cpu_list(st.field("Cpus_allowed_list")?)
        .map_err(|e| ProcError::Unreadable(format!("Cpus_allowed_list of {pid}: {e}")))
}

/// Per-CPU (idle + iowait, total) jiffies from /proc/stat.
pub fn read_cpu_times() -> io::Result<BTreeMap<usize, (u64, u64)>> {
    Ok(parse_cpu_times(&procfs::sys().cpu_stat()?))
}

pub fn parse_cpu_times(text: &str) -> BTreeMap<usize, (u64, u64)> {
    let mut out = BTreeMap::new();
    for line in text.lines() {
        let Some(rest) = line.strip_prefix("cpu") else {
            continue;
        };
        let mut it = rest.split_whitespace();
        let Some(Ok(cpu)) = it.next().map(str::parse::<usize>) else {
            continue;
        };
        let v: Vec<u64> = it.filter_map(|x| x.parse().ok()).collect();
        if v.len() < 5 {
            continue;
        }
        out.insert(cpu, (v[3] + v[4], v.iter().sum()));
    }
    out
}

/// Per-CPU (idle + iowait, total) jiffies.
pub type CpuTimes = BTreeMap<usize, (u64, u64)>;

/// Per-CPU idleness from /proc/stat deltas between two calls of [`IdleTracker::refresh`] (the pick.py rule, without its
/// 3 s sleep: the control loop refreshes once per tick, and the delta spans the time between refreshes).
#[derive(Debug, Default)]
pub struct IdleTracker {
    prev: Option<(std::time::Instant, CpuTimes)>,
    idle: BTreeMap<usize, f64>,
    span: Option<std::time::Duration>,
}

impl IdleTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Take a new /proc/stat sample; idleness is computed against the previous one if at least `min_span` old
    /// (otherwise the previous sample is kept, so that quick successive calls do not shrink the window to noise).
    pub fn refresh(&mut self, min_span: std::time::Duration) -> io::Result<()> {
        let now = std::time::Instant::now();
        let cur = read_cpu_times()?;
        match &self.prev {
            Some((t, _)) if now.duration_since(*t) < min_span => {}
            Some((t, prev)) => {
                self.idle = idle_fractions(prev, &cur);
                self.span = Some(now.duration_since(*t));
                self.prev = Some((now, cur));
            }
            None => self.prev = Some((now, cur)),
        }
        Ok(())
    }

    /// Idle fraction of a CPU over the last window, None before two samples exist.
    pub fn idle(&self, cpu: usize) -> Option<f64> {
        self.idle.get(&cpu).copied()
    }

    pub fn window(&self) -> Option<std::time::Duration> {
        self.span
    }

    /// For tests and for callers that measure elsewhere.
    pub fn set_idle(&mut self, idle: BTreeMap<usize, f64>) {
        self.idle = idle;
        self.span = Some(std::time::Duration::from_secs(1));
    }
}

pub fn idle_fractions(
    a: &BTreeMap<usize, (u64, u64)>,
    b: &BTreeMap<usize, (u64, u64)>,
) -> BTreeMap<usize, f64> {
    let mut out = BTreeMap::new();
    for (cpu, &(ia, ta)) in a {
        if let Some(&(ib, tb)) = b.get(cpu) {
            let dt = tb.saturating_sub(ta);
            if dt > 0 {
                out.insert(*cpu, ib.saturating_sub(ia) as f64 / dt as f64);
            }
        }
    }
    out
}

/// Path of this process's cgroup-v2 directory (the unified hierarchy mounted at /sys/fs/cgroup). Err: unreadable, or no
/// unified line.
pub fn own_cgroup_dir() -> Result<PathBuf, String> {
    match procfs::sys().self_cgroup_v2() {
        Ok(procfs::CgroupLine::Path(g)) => {
            Ok(Path::new("/sys/fs/cgroup").join(g.trim_start_matches('/')))
        }
        Ok(procfs::CgroupLine::NoUnified) => Err("no cgroup-v2 membership".into()),
        Err(e) => Err(format!("own cgroup: {e}")),
    }
}

#[allow(clippy::let_underscore_must_use)] // test cleanup (outside the reviewed scope, procfix7)
#[allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_stat_handles_parenthesized_names() {
        // Same case as the source test at b9f06de7, extended to the new fields.
        let mut fields = vec!["0"; 22];
        fields[0] = "S";
        fields[1] = "77";
        fields[2] = "123";
        fields[3] = "120";
        fields[11] = "7";
        fields[12] = "8";
        fields[19] = "999";
        fields[21] = "4";
        let s = parse_stat(
            123,
            &format!("123 (owner worker (2)) {}", fields.join(" ")),
            4096,
        )
        .unwrap();
        assert_eq!(s.comm, "owner worker (2)");
        assert_eq!((s.state, s.ppid, s.pgrp, s.session), ('S', 77, 123, 120));
        assert_eq!((s.start_ticks, s.cpu_ticks, s.rss_bytes), (999, 15, 16384));
        assert!(parse_stat(1, "1 (x) S 0", 4096).is_none());
    }

    #[test]
    fn own_identity_is_alive_and_wrong_start_is_recycled() {
        // A child of the test (our own pid is 1 inside the test pid namespace, which capture refuses).
        let mut child = std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .unwrap();
        let me = ProcessIdentity::capture(child.id()).unwrap();
        assert_eq!(me.check().unwrap(), Liveness::Alive { zombie: false });
        let wrong = ProcessIdentity {
            start_ticks: me.start_ticks + 1,
            ..me.clone()
        };
        assert!(matches!(wrong.check().unwrap(), Liveness::Recycled { .. }));
        let other_boot = ProcessIdentity {
            boot_id: "not-this-boot".into(),
            ..me
        };
        assert_eq!(other_boot.check().unwrap(), Liveness::OtherBoot);
        child.kill().unwrap();
        child.wait().unwrap();
    }

    #[test]
    fn meminfo_parse_and_host() {
        let t = "MemTotal:       1000 kB\nMemFree: 5 kB\nMemAvailable:    600 kB\n";
        assert_eq!(parse_meminfo(t), Some((1_024_000, 614_400)));
        assert_eq!(parse_meminfo("MemTotal: 1 kB\n"), None);
        let m = host_memory().unwrap();
        assert!(m.mem_total > 0 && m.mem_available <= m.mem_total);
        assert!(m.effective_available() <= m.mem_available);
    }

    #[test]
    fn cpu_lists_round_trip() {
        let s = parse_cpu_list("0-3,8, 10-11").unwrap();
        assert_eq!(
            s.iter().copied().collect::<Vec<_>>(),
            vec![0, 1, 2, 3, 8, 10, 11]
        );
        assert_eq!(format_cpu_list(&s), "0-3,8,10-11");
        assert!(parse_cpu_list("3-1").is_err());
        assert!(parse_cpu_list("x").is_err());
        assert!(!allowed_cpus().unwrap().is_empty());
        let mine = affinity_of(std::process::id()).unwrap();
        assert_eq!(mine, allowed_cpus().unwrap());
    }

    #[test]
    fn idle_from_deltas() {
        let a = parse_cpu_times("cpu  1 1 1 1 1\ncpu0 10 0 10 70 10 0 0\ncpu1 0 0 0 0 0\n");
        let b = parse_cpu_times("cpu0 20 0 10 150 10 0 0\ncpu1 0 0 0 0 0\n");
        let idle = idle_fractions(&a, &b);
        assert!((idle[&0] - 80.0 / 90.0).abs() < 1e-12);
        assert!(!idle.contains_key(&1));
        let mut t = IdleTracker::new();
        t.refresh(std::time::Duration::ZERO).unwrap();
        assert!(t.idle(0).is_none());
        std::thread::sleep(std::time::Duration::from_millis(50));
        t.refresh(std::time::Duration::ZERO).unwrap();
        assert!(t.window().is_some());
    }
}
