//! The campaign MARKER (plan 2.7.1) and the ORPHAN SCAN.
//!
//! Every child of the real launcher gets `RR_CAMPAIGN_JOB=<campaign id>:<attempt id>` in its environment; every fork
//! inherits it, and an exec inherits it unless the program clears its environment. `/proc/<pid>/environ` shows the
//! environment the process was exec'd with. A process is treated as belonging to this campaign at restart / adoption /
//! orphan time ONLY if
//! - its owner uid is ours (`/proc/<pid>` owner), and
//! - its environ is readable and carries a marker whose campaign id is ours (exact attempt id where one is required).
//!
//! Unreadable environ, no marker, or another campaign id: NOT ours, never signalled.
//!
//! procfix7 (gate6 MJ-A): the owner of `/proc/<pid>` is the process's EFFECTIVE uid also for a non-dumpable process
//! (only the entries inside become root-owned), so non-dumpable processes of our uid ARE candidates of the strict scans
//! and their environ is unreadable. Such a candidate is classified by [`super::lineage`] from what stays readable (its
//! verified parent chain and start times): proved NOT ours (not counted), or possibly ours (counted as unreadable and
//! LISTED in [`CampaignScan::hidden`]: the scan is inconclusive and the caller names the pids). Never signalled either way.
//!
//! environ reads take the target's mmap lock and may block: every read here is bounded ([`procfs::ProcRoot::environ`],
//! default 1 s); a timed-out read counts as unreadable (COUNTED by the strict scans, retried next scan).
//!
//! procfix8 (gate8 G8-B1, the class): every read goes through [`procfs`]: Gone only for ENOENT / ESRCH; a failed read
//! of a candidate is counted and listed, never "not a candidate".

use super::lineage::{Hidden, HiddenProcess, classify_unreadable};
use super::procfs::{self, ProcError, ProcResult, ProcRoot};
use super::{LiveThread, ProcStat, live_thread_at};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;

/// Name of the marker variable.
pub const MARKER_VAR: &str = "RR_CAMPAIGN_JOB";

/// Default bound of one environ read.
pub const ENVIRON_READ_TIMEOUT: Duration = Duration::from_secs(1);

/// `<campaign id>:<attempt id>`. Campaign id: 1-64 of `[A-Za-z0-9._-]`; attempt id: 1-256 bytes, no NUL (may contain
/// ':' so `job:attempt` works, plan 2.7.1).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CampaignMarker {
    pub campaign_id: String,
    pub attempt_id: String,
}

/// Why a campaign or attempt id was refused (never echoes the offending text: it may be mistyped secret material).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvalidMarker(pub &'static str);

impl fmt::Display for InvalidMarker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid campaign marker: {}", self.0)
    }
}

impl std::error::Error for InvalidMarker {}

pub fn validate_campaign_id(id: &str) -> Result<(), InvalidMarker> {
    if id.is_empty() || id.len() > 64 {
        return Err(InvalidMarker("campaign id must have 1-64 characters"));
    }
    if !id
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
    {
        return Err(InvalidMarker("campaign id allows only [A-Za-z0-9._-]"));
    }
    Ok(())
}

impl CampaignMarker {
    pub fn new(
        campaign_id: impl Into<String>,
        attempt_id: impl Into<String>,
    ) -> Result<Self, InvalidMarker> {
        let (campaign_id, attempt_id) = (campaign_id.into(), attempt_id.into());
        validate_campaign_id(&campaign_id)?;
        if attempt_id.is_empty() || attempt_id.len() > 256 {
            return Err(InvalidMarker("attempt id must have 1-256 bytes"));
        }
        if attempt_id.contains('\0') {
            return Err(InvalidMarker("attempt id contains NUL"));
        }
        Ok(Self {
            campaign_id,
            attempt_id,
        })
    }

    /// The variable's value.
    pub fn value(&self) -> String {
        format!("{}:{}", self.campaign_id, self.attempt_id)
    }

    /// Parse a value (split at the FIRST ':').
    pub fn parse(value: &str) -> Option<Self> {
        let (c, a) = value.split_once(':')?;
        Self::new(c, a).ok()
    }
}

/// The marker value in an environ block (NUL-separated `K=V`), if any. The first occurrence wins, as getenv.
pub fn marker_in_environ(environ: &[u8]) -> Option<String> {
    let prefix = format!("{MARKER_VAR}=");
    environ
        .split(|&b| b == 0)
        .find_map(|kv| kv.strip_prefix(prefix.as_bytes()))
        .map(|v| String::from_utf8_lossy(v).into_owned())
}

/// What the environ of a process says about the marker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MarkerRead {
    /// The value (possibly of another campaign).
    Marker(String),
    NoMarker,
    /// Not readable (permission, non-dumpable, timeout, I/O error, a torn read, the live thread read through exited):
    /// unknown; procfix8: never "not marked", never gone.
    Unreadable(String),
    /// The PROCESS is gone (ENOENT / ESRCH of its environ; never for a read through a thread).
    Gone,
}

/// Read the marker of `pid` under a proc root. For a thread group whose main thread is a zombie, `tid` names a live
/// thread whose `environ` (same mm) is read instead. procfix8: the environ is read COMPLETELY (its length equals the
/// bounds the kernel reads, [`procfs`]); a read through `tid` that finds the thread gone is Unreadable (the process may
/// live on with other threads), never Gone.
pub fn read_marker_at(
    root: &ProcRoot,
    pid: u32,
    tid: Option<u32>,
    timeout: Duration,
) -> MarkerRead {
    // The execve window (procfix3, gate-B M1: a marked re-forking chain read inside its exec windows looked unmarked) is
    // handled by evidence inside `ProcRoot::environ` (gate8 m8-ew: the environment bounds are 0 / 0 until the new image
    // sets them; the read waits while that holds, bounded, then Unreadable): an EMPTY result here is a real empty
    // environment (`env -i`), a zombie or a kernel thread: no marker now.
    match root.environ(pid, tid, timeout) {
        Ok(b) => match marker_in_environ(&b) {
            Some(v) => MarkerRead::Marker(v),
            None => MarkerRead::NoMarker,
        },
        Err(ProcError::Gone) => match tid {
            None => MarkerRead::Gone,
            Some(t) => MarkerRead::Unreadable(format!(
                "the live thread {t} of {pid} exited while its environ was read"
            )),
        },
        Err(ProcError::Unreadable(w)) => MarkerRead::Unreadable(w),
    }
}

/// Bound of one environ read of the strict scan (a process holding its mmap lock must not hold a sweep: a timed-out read
/// is counted as unreadable, which makes the scan inconclusive, never hung).
pub(crate) const STRICT_ENVIRON_TIMEOUT: Duration = Duration::from_millis(250);

/// A STRICT scan for live processes of our uid carrying a marker of `campaign_id` (optionally exactly attempt
/// `only`), started at or after `not_before_ticks` (call it off the control loop). Unlike [`scan_campaign`], an
/// unreadable environ of a candidate is COUNTED (the caller decides whether the scan is conclusive); every environ read
/// is bounded ([`STRICT_ENVIRON_TIMEOUT`]) and an empty environ is re-read once (exec window). A process that vanished is
/// not a candidate.
pub(crate) fn scan_marked_inline(
    campaign_id: &str,
    not_before_ticks: u64,
    only: Option<&str>,
) -> std::io::Result<CampaignScan> {
    strict_scan(
        &procfs::sys(),
        campaign_id,
        not_before_ticks,
        &|a| only.is_none_or(|o| o == a),
        STRICT_ENVIRON_TIMEOUT,
    )
}

/// The verdict on one candidate of a strict scan ([`check_candidate`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CandidateCheck {
    /// Not a candidate, by a COMPLETE read: gone (ENOENT / ESRCH of the process, or its pid names a process with another
    /// start time than the one held), another owner, started before the bound, a zombie without a live thread, or a
    /// readable environ without this campaign's (wanted) marker.
    NotCandidate,
    /// Proved unable to carry the marker by its complete parent chain (`lineage`; diagnostics only).
    NotOurs,
    /// Alive with a wanted marker of this campaign.
    Marked(MarkedProcess),
    /// Possibly ours and unknown (a failed read of its facts, or an unreadable environ the parent chain does not clear):
    /// counted and listed, the scan is inconclusive.
    Hidden(HiddenProcess),
}

/// procfix8: THE ONE CANDIDATE CHECK of the strict scans and of the sweep's re-checks of earlier suspects. `expect_start`:
/// the start time a caller already holds for `pid` (a different one now proves that process dead). Every read through
/// [`procfs`]: only ENOENT / ESRCH of the process (never of a thread) makes it NotCandidate as "gone"; any other failure
/// makes it Hidden.
pub(crate) fn check_candidate(
    root: &ProcRoot,
    pid: u32,
    expect_start: Option<u64>,
    campaign_id: &str,
    not_before_ticks: u64,
    wanted: &dyn Fn(&str) -> bool,
    timeout: Duration,
) -> CandidateCheck {
    let facts = |why: String| {
        CandidateCheck::Hidden(HiddenProcess {
            pid,
            start_ticks: expect_start.unwrap_or(0),
            ppid: 0,
            comm: String::new(),
            why,
        })
    };
    match root.owner_uid(pid) {
        Ok(u) if u == our_uid() => {}
        Ok(_) | Err(ProcError::Gone) => return CandidateCheck::NotCandidate,
        Err(ProcError::Unreadable(e)) => return facts(format!("owner unreadable: {e}")),
    }
    let st = match root.stat(pid) {
        Ok(st) => st,
        Err(ProcError::Gone) => return CandidateCheck::NotCandidate,
        Err(ProcError::Unreadable(e)) => return facts(format!("stat unreadable: {e}")),
    };
    if expect_start.is_some_and(|s| s != st.start_ticks) {
        // the pid names a newer process: the one held is dead
        return CandidateCheck::NotCandidate;
    }
    if st.start_ticks < not_before_ticks {
        return CandidateCheck::NotCandidate;
    }
    let tid = if st.is_dead() {
        match live_thread_at(root, pid) {
            Ok(LiveThread::Found(t, _)) => Some(t),
            // a zombie without a live thread holds nothing and cannot fork: not a candidate
            Ok(LiveThread::NoneAlive) | Err(ProcError::Gone) => {
                return CandidateCheck::NotCandidate;
            }
            Err(ProcError::Unreadable(e)) => {
                return CandidateCheck::Hidden(HiddenProcess::new(
                    &st,
                    format!("thread list unreadable: {e}"),
                ));
            }
        }
    } else {
        None
    };
    match read_marker_at(root, pid, tid, timeout) {
        MarkerRead::Marker(v) => match CampaignMarker::parse(&v) {
            Some(m) if m.campaign_id == campaign_id && wanted(&m.attempt_id) => {
                CandidateCheck::Marked(MarkedProcess::from_stat(&st, m.attempt_id))
            }
            _ => CandidateCheck::NotCandidate,
        },
        MarkerRead::NoMarker | MarkerRead::Gone => CandidateCheck::NotCandidate,
        MarkerRead::Unreadable(w) => match classify_unreadable(root, &st, campaign_id) {
            Hidden::NotOurs(_) => CandidateCheck::NotOurs,
            // the candidate itself PROVED dead (ENOENT / ESRCH of its stat, or another start time)
            Hidden::Gone => CandidateCheck::NotCandidate,
            Hidden::Possible(why) => {
                CandidateCheck::Hidden(HiddenProcess::new(&st, format!("environ: {w}; {why}")))
            }
        },
    }
}

/// THE STRICT SCAN (procfix8: one implementation for both scans; every read through [`procfs`]): every pid of the root
/// (the two-enumeration listing) through [`check_candidate`]. A read that fails with anything but ENOENT / ESRCH is
/// COUNTED and LISTED, never skipped.
fn strict_scan(
    root: &ProcRoot,
    campaign_id: &str,
    not_before_ticks: u64,
    wanted: &dyn Fn(&str) -> bool,
    timeout: Duration,
) -> std::io::Result<CampaignScan> {
    validate_campaign_id(campaign_id).map_err(|e| std::io::Error::other(e.to_string()))?;
    let me = std::process::id();
    let mut out = CampaignScan::default();
    for pid in root.pids()? {
        if pid <= 1 || pid == me {
            continue;
        }
        match check_candidate(
            root,
            pid,
            None,
            campaign_id,
            not_before_ticks,
            wanted,
            timeout,
        ) {
            CandidateCheck::NotCandidate => {}
            CandidateCheck::NotOurs => out.hidden_not_ours += 1,
            CandidateCheck::Marked(m) => out.processes.push(m),
            CandidateCheck::Hidden(h) => {
                out.unreadable += 1;
                out.hidden.push(h);
            }
        }
    }
    out.processes.sort_by_key(|p| (p.start_ticks, p.pid));
    Ok(out)
}

/// Effective uid of this process.
pub fn our_uid() -> u32 {
    // SAFETY: geteuid has no arguments and cannot fail.
    unsafe { libc::geteuid() }
}

/// Owner uid of `/proc/<pid>`: the process's EFFECTIVE uid (procfix7, gate6 MJ-A: also for a non-dumpable process;
/// kernel 6.18 makes only the entries INSIDE the directory root-owned, see [`super::lineage`]). Kernel threads: root.
pub fn owner_uid_at(root: &ProcRoot, pid: u32) -> ProcResult<u32> {
    root.owner_uid(pid)
}

/// procfix5 (gate4 m5): processes that DESCEND from `ancestor` by the parent chain (the `PPid:` of
/// `/proc/<pid>/status`, which stays readable), whose REAL uid (first field of `Uid:`) is `uid`, but whose ENVIRON this
/// process cannot read (`inspectable(pid)` Unreadable; procfix7, gate6 MJ-A: a non-dumpable process or a setuid
/// program). No marker scan can say whether they carry a marker: they are LISTED (pid, name) for the report, never
/// judged ours and never signalled.
///
/// procfix8 (gate8, the class): an uninspectable process whose status cannot be read is LISTED; a parent chain that
/// cannot be read (any link Unreadable) LISTS the candidate; a chain broken because a link died (Gone) is read again
/// (three times) and then listed. Only a complete chain that ends at pid 1 / 0 without passing `ancestor`, or a real
/// uid that is not ours, or the candidate's own death (Gone) leaves a process out.
pub fn hidden_descendants_at(
    root: &ProcRoot,
    ancestor: u32,
    uid: u32,
    inspectable: &dyn Fn(u32) -> ProcResult<()>,
) -> std::io::Result<Vec<(u32, String)>> {
    let facts = |pid: u32| -> ProcResult<(u32, u32, String)> {
        let st = root.status(pid)?;
        Ok((st.ppid()?, st.real_uid()?, st.name().unwrap_or_default()))
    };
    let mut out = Vec::new();
    for pid in root.pids()? {
        if pid == ancestor {
            continue;
        }
        match inspectable(pid) {
            Ok(()) | Err(ProcError::Gone) => continue,
            Err(ProcError::Unreadable(_)) => {}
        }
        let (ppid0, ruid, name) = match facts(pid) {
            Ok(f) => f,
            Err(ProcError::Gone) => continue,
            Err(ProcError::Unreadable(w)) => {
                out.push((pid, format!("<status unreadable: {w}>")));
                continue;
            }
        };
        if ruid != uid {
            continue;
        }
        // The chain walk; Gone of a link (it died: the candidate is being re-parented) -> walk again, at most 3 times.
        let mut verdict: Option<Result<bool, String>> = None;
        'walk: for _ in 0..3 {
            let mut ppid = if verdict.is_none() {
                ppid0
            } else {
                match facts(pid) {
                    Ok((p, _, _)) => p,
                    Err(ProcError::Gone) => {
                        verdict = Some(Ok(false));
                        break 'walk;
                    }
                    Err(ProcError::Unreadable(w)) => {
                        verdict = Some(Err(w));
                        break 'walk;
                    }
                }
            };
            let mut hops = 0;
            loop {
                if ppid == ancestor {
                    verdict = Some(Ok(true));
                    break 'walk;
                }
                if ppid <= 1 {
                    verdict = Some(Ok(false));
                    break 'walk;
                }
                if hops > 4096 {
                    verdict = Some(Err("parent chain longer than 4096 links".into()));
                    break 'walk;
                }
                hops += 1;
                match facts(ppid) {
                    Ok((pp, _, _)) => ppid = pp,
                    Err(ProcError::Gone) => {
                        verdict = Some(Err(format!("link {ppid} of its parent chain died")));
                        continue 'walk;
                    }
                    Err(ProcError::Unreadable(w)) => {
                        verdict = Some(Err(format!("link {ppid} of its parent chain: {w}")));
                        break 'walk;
                    }
                }
            }
        }
        match verdict {
            Some(Ok(true)) => out.push((pid, name)),
            Some(Ok(false)) => {}
            Some(Err(w)) => out.push((pid, format!("{name} <parent chain unreadable: {w}>"))),
            None => out.push((pid, format!("{name} <parent chain not read>"))),
        }
    }
    out.sort();
    Ok(out)
}

/// [`hidden_descendants_at`] of THIS process on the real /proc (our effective uid; inspectable = environ openable).
pub fn hidden_descendants() -> std::io::Result<Vec<(u32, String)>> {
    let root = procfs::sys();
    hidden_descendants_at(&root, std::process::id(), our_uid(), &|p| {
        root.environ_access(p)
    })
}

/// Can this process open `/proc/<pid>/environ`? Ok: yes; Gone; Unreadable: refused (non-dumpable, setuid, another
/// euid) or any other failure. (procfix7, gate6 MJ-A: the OWNER of the entry is no criterion across namespaces.)
pub fn environ_readable_at(root: &ProcRoot, pid: u32) -> ProcResult<()> {
    root.environ_access(pid)
}

/// A live process of our uid carrying a marker of the scanned campaign.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct MarkedProcess {
    pub pid: u32,
    pub start_ticks: u64,
    pub attempt_id: String,
    pub ppid: u32,
    pub pgrp: u32,
    pub session: u32,
    pub comm: String,
    pub state: char,
    pub rss_bytes: u64,
    pub cpu_ticks: u64,
}

impl MarkedProcess {
    pub(crate) fn from_stat(st: &ProcStat, attempt_id: String) -> Self {
        Self {
            pid: st.pid,
            start_ticks: st.start_ticks,
            attempt_id,
            ppid: st.ppid,
            pgrp: st.pgrp,
            session: st.session,
            comm: st.comm.clone(),
            state: st.state,
            rss_bytes: st.rss_bytes,
            cpu_ticks: st.cpu_ticks,
        }
    }
}

/// Result of a campaign scan.
#[derive(Clone, Debug, Default, Serialize)]
pub struct CampaignScan {
    pub processes: Vec<MarkedProcess>,
    /// Candidates (our uid, started late enough) whose environ could not be read and that may descend from the
    /// campaign: unknown, never signalled (each is listed in `hidden`).
    pub unreadable: usize,
    /// procfix7 (gate6 MJ-A): the unreadable candidates that may descend from the campaign, with the reason.
    pub hidden: Vec<HiddenProcess>,
    /// procfix7 (gate6 MJ-A): unreadable candidates PROVED unable to carry the marker by their parent chain (not counted
    /// in `unreadable`; diagnostics only).
    pub hidden_not_ours: usize,
}

/// THE ORPHAN SCAN primitive (blocking: restart time, the sampler thread, tests). Every live process of our uid that
/// started at or after `not_before_ticks` (0: any) and carries a marker of `campaign_id`, except those whose attempt id is
/// in `live_attempts` (jobs this supervisor runs right now). Returns them with their attempt ids; signalling them is a
/// separate, verified step (`launch::kill_orphan`).
pub fn scan_campaign(
    campaign_id: &str,
    not_before_ticks: u64,
    live_attempts: &std::collections::BTreeSet<String>,
) -> std::io::Result<CampaignScan> {
    scan_campaign_at(&procfs::sys(), campaign_id, not_before_ticks, live_attempts)
}

pub(crate) fn scan_campaign_at(
    root: &ProcRoot,
    campaign_id: &str,
    not_before_ticks: u64,
    live_attempts: &std::collections::BTreeSet<String>,
) -> std::io::Result<CampaignScan> {
    strict_scan(
        root,
        campaign_id,
        not_before_ticks,
        &|a| !live_attempts.contains(a),
        ENVIRON_READ_TIMEOUT,
    )
}

#[allow(clippy::let_underscore_must_use)] // test cleanup (outside the reviewed scope, procfix7)
#[cfg(test)]
mod tests {
    /// procfix5 (gate4 m5): a fake /proc tree: an uninspectable entry (environ refused; procfix7: readability, not the
    /// directory's owner) whose real uid is ours and whose parent chain reaches the ancestor is listed; one of another
    /// real uid, one outside the chain, and one we can read (the marker scans see it) are not.
    #[test]
    fn m5_hidden_descendants_are_listed_never_judged() {
        let root = std::env::temp_dir().join(format!(
            "rr-hidden-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let put = |pid: u32, ppid: u32, ruid: u32, name: &str| {
            let d = root.join(pid.to_string());
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(
                d.join("status"),
                format!("Name:\t{name}\nPPid:\t{ppid}\nUid:\t{ruid}\t0\t0\t0\n"),
            )
            .unwrap();
        };
        put(100, 1, 7, "sup"); // the ancestor
        put(101, 100, 7, "job"); // our readable child
        put(102, 101, 7, "agent"); // non-dumpable grandchild, ours by real uid -> listed
        put(103, 100, 9, "foreign"); // root-owned, another real uid -> not listed
        put(104, 1, 7, "elsewhere"); // root-owned, ours, not a descendant -> not listed
        let inspectable = |p: u32| {
            if matches!(p, 102..=104) {
                Err(ProcError::Unreadable("refused".into()))
            } else {
                Ok(())
            }
        };
        let v = hidden_descendants_at(&ProcRoot::at(&root), 100, 7, &inspectable).unwrap();
        assert_eq!(v, vec![(102, "agent".to_string())]);
        // procfix8: a link of the chain whose status cannot be read LISTS the candidate (it was "not a descendant")
        std::fs::remove_file(root.join("101/status")).unwrap();
        std::fs::create_dir_all(root.join("101/status")).unwrap();
        let v = hidden_descendants_at(&ProcRoot::at(&root), 100, 7, &inspectable).unwrap();
        assert_eq!(v.len(), 1, "{v:?}");
        assert!(
            v[0].0 == 102 && v[0].1.contains("parent chain unreadable"),
            "{v:?}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    use super::*;

    /// procfix7 (gate7 B1, the class): a candidate whose facts cannot be read is COUNTED (inconclusive), never skipped:
    /// a strict scan over a fake /proc whose entry has an unreadable `stat` (a directory) reports it as unreadable; and
    /// an uninspectable process whose status cannot be read is LISTED by the hidden-descendant check.
    #[test]
    fn failed_reads_are_counted_never_skipped() {
        let root = std::env::temp_dir().join(format!(
            "rr-class-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join("4242").join("stat")).unwrap(); // `stat` unreadable (EISDIR)
        let s = scan_campaign_at(
            &ProcRoot::at(&root),
            "cls.0123456789abcdef",
            0,
            &Default::default(),
        )
        .unwrap();
        assert_eq!(s.unreadable, 1, "{s:?}");
        assert!(s.hidden[0].why.contains("stat unreadable"), "{s:?}");
        // hidden descendants: pid 4343 exists, cannot be inspected and its status cannot be read (EISDIR): listed
        std::fs::create_dir_all(root.join("4343").join("status")).unwrap();
        let v = hidden_descendants_at(&ProcRoot::at(&root), 1, 7, &|p| {
            if p == 4343 {
                Err(ProcError::Unreadable("refused".into()))
            } else {
                Ok(())
            }
        })
        .unwrap();
        assert_eq!(v.len(), 1, "{v:?}");
        assert!(
            v[0].0 == 4343 && v[0].1.starts_with("<status unreadable"),
            "{v:?}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn marker_values_and_environ_parsing() {
        let m = CampaignMarker::new("camp-1", "T162:f17:a1").unwrap();
        assert_eq!(m.value(), "camp-1:T162:f17:a1");
        assert_eq!(CampaignMarker::parse(&m.value()), Some(m));
        assert!(CampaignMarker::new("a:b", "x").is_err());
        assert!(CampaignMarker::new("", "x").is_err());
        assert!(CampaignMarker::new("a", "").is_err());
        assert!(CampaignMarker::new("a", "x\0y").is_err());
        let env = b"A=1\0RR_CAMPAIGN_JOB=c:a1\0RR_CAMPAIGN_JOB=d:z\0";
        assert_eq!(marker_in_environ(env).as_deref(), Some("c:a1"));
        assert_eq!(marker_in_environ(b"XRR_CAMPAIGN_JOB=c:a\0"), None);
    }
}
