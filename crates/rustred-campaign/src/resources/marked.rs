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
//! environ reads take the target's mmap lock and may block: every read here goes through
//! [`super::bounded::read_bounded`] (default 1 s); a timed-out read counts as unreadable (not ours, retried next scan).

use super::bounded::{ReadOutcome, read_bounded};
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
    match read_bounded(&path, timeout) {
        ReadOutcome::Data(b) if b.is_empty() => MarkerRead::Unreadable, // zombie / kernel thread: no mm
        ReadOutcome::Data(b) => match marker_in_environ(&b) {
            Some(v) => MarkerRead::Marker(v),
            None => MarkerRead::NoMarker,
        },
        ReadOutcome::Gone => MarkerRead::Gone,
        ReadOutcome::Error(_) | ReadOutcome::TimedOut => MarkerRead::Unreadable,
    }
}

/// Effective uid of this process.
pub fn our_uid() -> u32 {
    // SAFETY: geteuid has no arguments and cannot fail.
    unsafe { libc::geteuid() }
}

/// Owner uid of `/proc/<pid>` (the process's effective uid; root for non-dumpable processes, which therefore never
/// count as ours). None: gone.
pub fn owner_uid_at(root: &Path, pid: u32) -> Option<u32> {
    fs::metadata(root.join(pid.to_string()))
        .ok()
        .map(|m| m.uid())
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
    /// Candidates (our uid, started late enough) whose environ could not be read: unknown, never signalled.
    pub unreadable: usize,
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
            MarkerRead::Unreadable => out.unreadable += 1,
            MarkerRead::NoMarker | MarkerRead::Gone => {}
        }
    }
    out.processes.sort_by_key(|p| (p.start_ticks, p.pid));
    Ok(out)
}

#[cfg(test)]
mod tests {
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
