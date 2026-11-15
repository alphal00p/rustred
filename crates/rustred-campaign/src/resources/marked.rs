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
//! environ reads take the target's mmap lock and may block: every read here goes through
//! [`super::bounded::read_bounded`] (default 1 s); a timed-out read counts as unreadable (not ours, retried next scan).

use super::bounded::{ReadOutcome, read_bounded};
use super::lineage::{Hidden, HiddenProcess, classify_unreadable};
use super::{ProcStat, live_thread, read_stat_path};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
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
    /// Not readable (permission, non-dumpable, timeout, I/O error): not ours.
    Unreadable,
    Gone,
}

/// Read the marker of `pid` under a proc root (`/proc` in production). For a thread group whose main thread is a
/// zombie, `tid` names a live thread whose `environ` (same mm) is read instead.
pub fn read_marker_at(root: &Path, pid: u32, tid: Option<u32>, timeout: Duration) -> MarkerRead {
    let path = match tid {
        Some(t) => root.join(format!("{pid}/task/{t}/environ")),
        None => root.join(format!("{pid}/environ")),
    };
    let first = read_bounded(&path, timeout);
    // An EMPTY environ of a live process: either the execve window (the new mm is installed before its environment
    // bounds are set; microseconds) or a real empty environment (`env -i`). Re-read once after 1 ms (procfix3, gate-B
    // M1: a marked re-forking chain read in its exec windows looked unmarked and the orphan sweep stopped early).
    let settled = match first {
        ReadOutcome::Data(b) if b.is_empty() => {
            std::thread::sleep(Duration::from_millis(1));
            match read_bounded(&path, timeout) {
                // Still empty: zombie (no mm), exiting, or `env -i`: no marker now.
                ReadOutcome::Data(b) if b.is_empty() => return MarkerRead::NoMarker,
                other => other,
            }
        }
        other => other,
    };
    match settled {
        ReadOutcome::Data(b) => match marker_in_environ(&b) {
            Some(v) => MarkerRead::Marker(v),
            None => MarkerRead::NoMarker,
        },
        ReadOutcome::Gone => MarkerRead::Gone,
        ReadOutcome::Error(_) | ReadOutcome::TimedOut => MarkerRead::Unreadable,
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
    validate_campaign_id(campaign_id).map_err(|e| std::io::Error::other(e.to_string()))?;
    let root = Path::new("/proc");
    let uid = our_uid();
    let me = std::process::id();
    let mut out = CampaignScan::default();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|s| s.parse::<u32>().ok())
        else {
            continue;
        };
        if pid <= 1 || pid == me {
            continue;
        }
        if owner_uid_at(root, pid) != Some(uid) {
            continue;
        }
        let st = match read_stat_path(pid, &root.join(format!("{pid}/stat"))) {
            Ok(Some(st)) => st,
            Ok(None) => continue,
            Err(e) => {
                out.unreadable += 1;
                out.hidden.push(HiddenProcess {
                    pid,
                    start_ticks: 0,
                    ppid: 0,
                    comm: String::new(),
                    why: format!("stat unreadable: {e}"),
                });
                continue;
            }
        };
        if st.start_ticks < not_before_ticks {
            continue;
        }
        let Some(tid) = live_tid(&st) else { continue };
        match read_marker_at(root, pid, tid, STRICT_ENVIRON_TIMEOUT) {
            MarkerRead::Marker(v) => {
                if let Some(m) = CampaignMarker::parse(&v)
                    && m.campaign_id == campaign_id
                    && only.is_none_or(|o| o == m.attempt_id)
                {
                    out.processes
                        .push(MarkedProcess::from_stat(&st, m.attempt_id));
                }
            }
            MarkerRead::Unreadable => out.unreadable_candidate(root, &st, campaign_id),
            MarkerRead::NoMarker | MarkerRead::Gone => {}
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
/// None: gone.
pub fn owner_uid_at(root: &Path, pid: u32) -> Option<u32> {
    fs::metadata(root.join(pid.to_string()))
        .ok()
        .map(|m| m.uid())
}

/// procfix5 (gate4 m5): processes that DESCEND from `ancestor` by the parent chain (the `PPid:` of
/// `/proc/<pid>/status`, which stays readable), whose REAL uid (first field of `Uid:`) is `uid`, but whose ENVIRON this
/// process cannot read (`inspectable(pid)` = Some(false); procfix7, gate6 MJ-A: a non-dumpable process or a setuid
/// program; the former test, "the /proc DIRECTORY is not owned by `uid`", never fired for a non-dumpable process,
/// whose directory stays owned by its effective uid). No marker scan can say whether they carry a marker: they are
/// LISTED (pid, name) for the report, never judged ours and never signalled. `root` is "/proc" in production (a test
/// seam).
pub fn hidden_descendants_at(
    root: &Path,
    ancestor: u32,
    uid: u32,
    inspectable: &dyn Fn(u32) -> Option<bool>,
) -> std::io::Result<Vec<(u32, String)>> {
    use std::collections::BTreeMap;
    // pid -> (ppid, real uid, name), read once per pid
    let mut facts: BTreeMap<u32, Option<(u32, u32, String)>> = BTreeMap::new();
    let mut read = |pid: u32| -> Option<(u32, u32, String)> {
        facts
            .entry(pid)
            .or_insert_with(|| {
                let t = fs::read_to_string(root.join(format!("{pid}/status"))).ok()?;
                let field = |k: &str| {
                    t.lines()
                        .find_map(|l| l.strip_prefix(k))
                        .map(|v| v.trim().to_string())
                };
                let ppid = field("PPid:")?.parse().ok()?;
                let ruid = field("Uid:")?.split_whitespace().next()?.parse().ok()?;
                let name = field("Name:").unwrap_or_default();
                Some((ppid, ruid, name))
            })
            .clone()
    };
    let mut out = Vec::new();
    for entry in fs::read_dir(root)? {
        let Ok(entry) = entry else { continue };
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|s| s.parse::<u32>().ok())
        else {
            continue;
        };
        if pid == ancestor || inspectable(pid).is_none_or(|ok| ok) {
            continue;
        }
        let Some((mut ppid, ruid, name)) = read(pid) else {
            continue;
        };
        if ruid != uid {
            continue;
        }
        let mut hops = 0;
        let descends = loop {
            if ppid == ancestor {
                break true;
            }
            if ppid <= 1 || hops > 4096 {
                break false;
            }
            hops += 1;
            match read(ppid) {
                Some((pp, _, _)) => ppid = pp,
                None => break false,
            }
        };
        if descends {
            out.push((pid, name));
        }
    }
    out.sort();
    Ok(out)
}

/// [`hidden_descendants_at`] of THIS process on the real /proc (our effective uid; inspectable = environ readable).
pub fn hidden_descendants() -> std::io::Result<Vec<(u32, String)>> {
    let root = Path::new("/proc");
    hidden_descendants_at(root, std::process::id(), our_uid(), &|p| {
        environ_readable_at(root, p)
    })
}

/// Can this process open `/proc/<pid>/environ`? Some(false): permission refused (non-dumpable, setuid, another euid);
/// None: gone. (procfix7, gate6 MJ-A: the OWNER of the entry is no criterion across namespaces: in a user namespace
/// whose root is our uid a non-dumpable process's environ is owned by our uid and still refused to a reader without
/// CAP_SYS_PTRACE.)
pub fn environ_readable_at(root: &Path, pid: u32) -> Option<bool> {
    match fs::File::open(root.join(format!("{pid}/environ"))) {
        Ok(_) => Some(true),
        Err(e) if matches!(e.raw_os_error(), Some(libc::EACCES) | Some(libc::EPERM)) => Some(false),
        Err(_) => None,
    }
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

impl CampaignScan {
    /// An unreadable candidate: classified by its parent chain (`lineage`); counted only when it may be ours.
    fn unreadable_candidate(&mut self, root: &Path, st: &ProcStat, campaign_id: &str) {
        match classify_unreadable(root, st, campaign_id) {
            Hidden::NotOurs(_) => self.hidden_not_ours += 1,
            Hidden::Gone => {}
            Hidden::Possible(why) => {
                self.unreadable += 1;
                self.hidden.push(HiddenProcess::new(st, why));
            }
        }
    }
}

/// For a stat whose main thread is dead: a live thread of the group (None if the whole group is dead).
pub(crate) fn live_tid(st: &ProcStat) -> Option<Option<u32>> {
    if !st.is_dead() {
        return Some(None);
    }
    match live_thread(st.pid) {
        Ok(Some((tid, _))) => Some(Some(tid)),
        _ => None,
    }
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
    scan_campaign_at(
        Path::new("/proc"),
        campaign_id,
        not_before_ticks,
        live_attempts,
    )
}

pub(crate) fn scan_campaign_at(
    root: &Path,
    campaign_id: &str,
    not_before_ticks: u64,
    live_attempts: &std::collections::BTreeSet<String>,
) -> std::io::Result<CampaignScan> {
    validate_campaign_id(campaign_id).map_err(|e| std::io::Error::other(e.to_string()))?;
    let uid = our_uid();
    let me = std::process::id();
    let mut out = CampaignScan::default();
    for entry in fs::read_dir(root)? {
        let Ok(entry) = entry else { continue };
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|s| s.parse::<u32>().ok())
        else {
            continue;
        };
        if pid <= 1 || pid == me {
            continue;
        }
        if owner_uid_at(root, pid) != Some(uid) {
            continue;
        }
        let Ok(Some(st)) = read_stat_path(pid, &root.join(format!("{pid}/stat"))) else {
            continue;
        };
        if st.start_ticks < not_before_ticks {
            continue;
        }
        let Some(tid) = live_tid(&st) else { continue };
        match read_marker_at(root, pid, tid, ENVIRON_READ_TIMEOUT) {
            MarkerRead::Marker(v) => {
                if let Some(m) = CampaignMarker::parse(&v)
                    && m.campaign_id == campaign_id
                    && !live_attempts.contains(&m.attempt_id)
                {
                    out.processes
                        .push(MarkedProcess::from_stat(&st, m.attempt_id));
                }
            }
            MarkerRead::Unreadable => out.unreadable_candidate(root, &st, campaign_id),
            MarkerRead::NoMarker | MarkerRead::Gone => {}
        }
    }
    out.processes.sort_by_key(|p| (p.start_ticks, p.pid));
    Ok(out)
}

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
        let inspectable = |p: u32| Some(!matches!(p, 102..=104));
        let v = hidden_descendants_at(&root, 100, 7, &inspectable).unwrap();
        assert_eq!(v, vec![(102, "agent".to_string())]);
        let _ = std::fs::remove_dir_all(&root);
    }

    use super::*;

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
