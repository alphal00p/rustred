//! procfix7 (gate6 MJ-A): WHAT CAN BE PROVED ABOUT A PROCESS OF OUR UID WHOSE ENVIRON CANNOT BE READ.
//!
//! Kernel facts (established by experiment on 6.18, WS/notes/implementer.procfix7.md): for a non-dumpable process
//! (PR_SET_DUMPABLE 0, or a fork of one) `/proc/<pid>` itself and `/proc/<pid>/task` stay owned by the process's
//! EFFECTIVE uid; every other entry (environ, stat, status, cmdline, fd, cgroup, maps, ns) is owned by the root of the
//! process's memory user namespace (0 on the host). A capability-less reader of the same uid gets EACCES on `environ`,
//! `fd`, `maps` and the `ns` links, but can read `stat` (ppid, pgrp, session, start time), `status` (Uid, PPid, NSpid),
//! `cmdline` and `cgroup`. `stat` field 4 / `status` PPid is the REAL parent (a ptrace tracer shows only as
//! TracerPid). Such a process is a candidate of every strict marker scan (owner of `/proc/<pid>` = our uid) and its
//! marker cannot be read: before this module every sweep then stayed INCONCLUSIVE (exit 8 with an empty pid list on
//! a host with tailscale SSH logins of the user, or with a second `rr_campaign` supervisor, which is non-dumpable).
//!
//! The rule (implemented: (a) and (b); never used to SIGNAL anything: it only decides whether a scan is conclusive):
//! - (a) A process that started before the bound (the earliest supervisor session of this campaign in this boot) is
//!   never a candidate (unchanged: every scan already applies it).
//! - (b) Every process that can carry this campaign's marker descends (fork lineage) from a job one of the campaign's
//!   supervisor SESSIONS launched (assumption 5 of the run book: nobody outside a job carries the marker). Its REAL
//!   parent is at all times either a process of the same session subtree (all started at or after that session's
//!   start), or a RE-PARENT TARGET: the nearest live child-subreaper ancestor of its dead parent, or the init of the
//!   dead parent's pid namespace. While the LIVE supervisor runs it is a child subreaper, so nothing it launched can
//!   leave its subtree except through a nested pid namespace (setns + fork, then the dying parent's namespace init).
//!   The subreaper ancestors of a DEAD session are among its ancestors at its start (recorded in the ledger note
//!   [`LINEAGE_NOTE`] committed with `SupervisorStarted`, before the session launched anything; a process that turns
//!   subreaper later and adopts one of our orphans is necessarily an ancestor of the orphan's dead parent: inside our
//!   subtree, and walked through, or a recorded ancestor, and already a target). Hence, walking the VERIFIED parent chain
//!   of the process (each link: parent read, then the child re-read with the same start time and the same parent; a
//!   parent never starts after its child):
//!   * a link that is the live supervisor, the init of a NESTED pid namespace (`NSpid` with two or more fields ending
//!     in 1), or a recorded ancestor (or the session itself) of an earlier session of this boot -> POSSIBLY OURS
//!     (stays inconclusive);
//!   * the first link that started before EVERY session of this boot (so it lies in no session's subtree) and is none
//!     of the above -> NOT OURS: the chain left every subtree through a process that cannot be a re-parent target.
//!
//!   A chain that merely ends at init proves nothing (a daemonised descendant is re-parented there): pid 1 is a
//!   recorded ancestor of every session and therefore a stop, never a proof.
//! - (c) Session, process group and cgroup are NOT used: setsid / setpgid change the first two, and a process may move
//!   itself between the cgroups of the user's delegated subtree.
//!
//! The rule is OFF (every unreadable candidate stays inconclusive, as before) when a session of this boot recorded no
//! lineage (directories of older binaries), when a recorded lineage disagrees with its session record (boot id) or
//! comes from another pid namespace, or when no rules were installed for the campaign in this process. Sessions of
//! another boot contribute nothing (their processes cannot exist any more).

use super::{ProcStat, read_stat_path};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// Prefix of the ledger note that records a session's lineage (`session_lineage {json}`).
pub const LINEAGE_NOTE: &str = "session_lineage";

/// Bound on the links of one parent chain.
const MAX_HOPS: usize = 4096;

/// A process identity inside one boot and pid namespace: (pid, start ticks).
pub type Link = (u32, u64);

/// One supervisor session's lineage, recorded at its start (ledger note [`LINEAGE_NOTE`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionLineage {
    pub pid: u32,
    pub start_ticks: u64,
    pub boot_id: String,
    /// Inode of `/proc/self/ns/pid` of the session (pids and start times are only comparable inside one namespace).
    pub pid_ns: u64,
    /// The verified parent chain at the session's start, parent first, up to the top of its pid namespace.
    pub ancestors: Vec<Link>,
    /// procfix7 (gate7 MJ-2): inode of `/proc/self/ns/time` of the session (start times are shifted by a time
    /// namespace's boottime offset: only sessions of ONE time namespace compare). None: unknown (older notes / kernel).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_ns: Option<u64>,
}

impl SessionLineage {
    /// The lineage of THIS process (pid, start, boot, pid namespace, verified ancestor chain).
    pub fn of_self() -> Result<Self, String> {
        let root = Path::new("/proc");
        let pid = std::process::id();
        let st = read_stat_path(pid, &root.join(format!("{pid}/stat")))
            .map_err(|e| format!("own stat: {e}"))?
            .ok_or("own stat: gone")?;
        Ok(Self {
            pid,
            start_ticks: st.start_ticks,
            boot_id: super::boot_id().map_err(|e| format!("boot id: {e}"))?,
            pid_ns: own_pid_ns().map_err(|e| format!("pid namespace: {e}"))?,
            ancestors: verified_ancestors_at(root, pid, st.start_ticks)?,
            time_ns: own_time_ns(),
        })
    }

    /// The ledger note text.
    pub fn note_text(&self) -> String {
        format!(
            "{LINEAGE_NOTE} {}",
            serde_json::to_string(self).unwrap_or_default()
        )
    }

    /// Parse a ledger note (None: not a lineage note, or unparsable).
    pub fn parse_note(text: &str) -> Option<Self> {
        let json = text.strip_prefix(LINEAGE_NOTE)?.strip_prefix(' ')?;
        serde_json::from_str(json).ok()
    }
}

/// Inode of this process's pid namespace.
pub fn own_pid_ns() -> std::io::Result<u64> {
    Ok(std::fs::metadata("/proc/self/ns/pid")?.ino())
}

/// procfix7 (gate7 MJ-2): inode of this process's time namespace (None: no time namespaces on this kernel).
pub fn own_time_ns() -> Option<u64> {
    std::fs::metadata("/proc/self/ns/time")
        .ok()
        .map(|m| m.ino())
}

impl SessionLineage {
    /// procfix7 (gate7 MJ-2): did this session run in ANOTHER time namespace than `current`? Recorded and current differ,
    /// or recorded while the current one is unknown (cannot be shown the same: different). Nothing recorded (an older
    /// binary, a failed read) counts as the same (RUNBOOK A4).
    pub fn other_time_ns(&self, current: &SessionLineage) -> bool {
        match (self.time_ns, current.time_ns) {
            (Some(a), Some(b)) => a != b,
            (Some(_), None) => true,
            (None, _) => false,
        }
    }
}

/// One verified link read.
enum LinkRead {
    /// `x` is the top of its pid namespace (ppid 0).
    Top,
    /// The real parent of `x` (identity and stat), verified: `x` re-read with the same start and the same parent.
    Parent(Link, ProcStat),
    /// `x` is gone or its pid names another process now.
    Gone,
    /// The parent changed while it was read (re-parenting, pid reuse): read the chain again.
    Changed,
    /// A contradiction the kernel's rules exclude (a parent that started after its child): never a proof.
    Inconsistent(String),
}

fn stat_at(root: &Path, pid: u32) -> Option<ProcStat> {
    read_stat_path(pid, &root.join(format!("{pid}/stat")))
        .ok()
        .flatten()
}

fn read_link(root: &Path, x: Link) -> LinkRead {
    let Some(sx) = stat_at(root, x.0) else {
        return LinkRead::Gone;
    };
    if sx.start_ticks != x.1 {
        return LinkRead::Gone;
    }
    if sx.ppid == 0 {
        return LinkRead::Top;
    }
    let Some(sy) = stat_at(root, sx.ppid) else {
        return LinkRead::Changed;
    };
    // The child again: same process, same parent -> the parent read above IS its real parent (a real parent is always
    // older than its child, so a pid reused by a newer process cannot be named by the child's ppid at the re-read).
    match stat_at(root, x.0) {
        Some(s) if s.start_ticks == x.1 && s.ppid == sx.ppid => {}
        Some(s) if s.start_ticks == x.1 => return LinkRead::Changed,
        _ => return LinkRead::Gone,
    }
    if sy.start_ticks > x.1 {
        return LinkRead::Inconsistent(format!(
            "parent {} started after its child {} ({} > {} ticks)",
            sy.pid, x.0, sy.start_ticks, x.1
        ));
    }
    LinkRead::Parent((sy.pid, sy.start_ticks), sy)
}

/// The verified ancestor chain of `(pid, start)` under a proc root, parent first, up to the top of the namespace.
pub fn verified_ancestors_at(root: &Path, pid: u32, start: u64) -> Result<Vec<Link>, String> {
    'attempt: for _ in 0..5 {
        let mut out: Vec<Link> = Vec::new();
        let mut x = (pid, start);
        loop {
            match read_link(root, x) {
                LinkRead::Top => return Ok(out),
                LinkRead::Parent(y, _) => {
                    out.push(y);
                    if out.len() > MAX_HOPS {
                        return Err("parent chain longer than the bound".into());
                    }
                    x = y;
                }
                LinkRead::Gone if x.0 == pid => return Err(format!("pid {pid} is gone")),
                LinkRead::Gone | LinkRead::Changed => continue 'attempt,
                LinkRead::Inconsistent(e) => return Err(e),
            }
        }
    }
    Err("the parent chain changed during five reads".into())
}

/// Is `pid` the init of a pid namespace NESTED below the reader's (`NSpid` with two or more fields, the last 1)?
/// None: unreadable / gone.
fn nested_init(root: &Path, pid: u32) -> Option<bool> {
    let t = std::fs::read_to_string(root.join(format!("{pid}/status"))).ok()?;
    let line = t.lines().find_map(|l| l.strip_prefix("NSpid:"))?;
    let f: Vec<&str> = line.split_whitespace().collect();
    Some(f.len() >= 2 && f.last() == Some(&"1"))
}

/// The rules that classify an unreadable candidate (see the module documentation).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineageRules {
    /// The live supervisor (this process): anything below it may be ours.
    pub live: Link,
    /// Possible re-parent targets: every recorded ancestor of every earlier session of this boot, the sessions
    /// themselves, and (only when the live supervisor is not a child subreaper) the live supervisor's own ancestors.
    pub targets: BTreeSet<Link>,
    /// The earliest session start of this boot (and pid namespace), the live one included.
    pub earliest: u64,
}

/// A supervisor session as the ledger records it (`SupervisorStarted`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordedSession {
    pub pid: u32,
    pub start_ticks: u64,
    pub boot_id: String,
}

impl LineageRules {
    /// The rules for the live session `current` from every session the ledger records and every lineage note it holds.
    /// `Err(why)`: the rule is OFF (a session of this boot without a lineage, a lineage that disagrees with its session
    /// record, or one from another pid namespace).
    pub fn build(
        sessions: &[RecordedSession],
        notes: &[SessionLineage],
        current: &SessionLineage,
        live_subreaper: bool,
    ) -> Result<Self, String> {
        let boot = &current.boot_id;
        let me: Link = (current.pid, current.start_ticks);
        let mut earliest = current.start_ticks;
        let mut targets: BTreeSet<Link> = BTreeSet::new();
        // A lineage note for a recorded session must agree with it (boot id) and come from this pid namespace.
        for n in notes {
            if let Some(s) = sessions
                .iter()
                .find(|s| s.pid == n.pid && s.start_ticks == n.start_ticks)
                && s.boot_id != n.boot_id
                && (s.boot_id == *boot || n.boot_id == *boot)
            {
                return Err(format!(
                    "the lineage recorded for session pid {} (start {}) names another boot than its session record",
                    n.pid, n.start_ticks
                ));
            }
            if n.boot_id == *boot && n.pid_ns != current.pid_ns {
                return Err(format!(
                    "session pid {} (start {}) of this boot ran in another pid namespace",
                    n.pid, n.start_ticks
                ));
            }
            // procfix7 (gate7 MJ-2): start times of another time namespace do not compare
            if n.boot_id == *boot && n.other_time_ns(current) {
                return Err(format!(
                    "session pid {} (start {}) of this boot ran in another time namespace",
                    n.pid, n.start_ticks
                ));
            }
        }
        for s in sessions.iter().filter(|s| s.boot_id == *boot) {
            if (s.pid, s.start_ticks) == me {
                continue;
            }
            let Some(n) = notes
                .iter()
                .find(|n| n.pid == s.pid && n.start_ticks == s.start_ticks && n.boot_id == *boot)
            else {
                return Err(format!(
                    "session pid {} (start {}) of this boot recorded no lineage (an older binary)",
                    s.pid, s.start_ticks
                ));
            };
            earliest = earliest.min(s.start_ticks);
            targets.insert((s.pid, s.start_ticks));
            targets.extend(n.ancestors.iter().copied());
        }
        if !live_subreaper {
            targets.extend(current.ancestors.iter().copied());
        }
        Ok(Self {
            live: me,
            targets,
            earliest,
        })
    }
}

/// The classification of one unreadable candidate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Hidden {
    /// Proved unable to carry this campaign's marker (the reason names the decisive link).
    NotOurs(String),
    /// Could descend from this campaign: the scan stays inconclusive.
    Possible(String),
    /// The candidate itself is gone (or its pid names another process now): not a candidate.
    Gone,
}

/// Classify the candidate `(pid, start)` (its environ unreadable) under a proc root with `rules`.
pub fn classify_hidden_at(root: &Path, pid: u32, start: u64, rules: &LineageRules) -> Hidden {
    for _ in 0..3 {
        let mut x: Link = (pid, start);
        let mut hops = 0usize;
        loop {
            match read_link(root, x) {
                LinkRead::Gone if x.0 == pid => return Hidden::Gone,
                LinkRead::Gone | LinkRead::Changed => break,
                LinkRead::Inconsistent(e) => return Hidden::Possible(e),
                LinkRead::Top => {
                    // Every link examined was young, not a stop: no session subtree and no re-parent target contains
                    // the chain (the top of the namespace is a recorded ancestor of every session).
                    return Hidden::NotOurs(format!(
                        "its parent chain reaches pid {} (top of the pid namespace) without passing this campaign's supervisor or a recorded ancestor of a session",
                        x.0
                    ));
                }
                LinkRead::Parent(y, sy) => {
                    if y == rules.live {
                        return Hidden::Possible(format!(
                            "descends from this supervisor (pid {})",
                            y.0
                        ));
                    }
                    match nested_init(root, y.0) {
                        Some(true) => {
                            return Hidden::Possible(format!(
                                "its parent chain passes pid {} ({}), the init of a nested pid namespace",
                                y.0, sy.comm
                            ));
                        }
                        Some(false) => {}
                        None => break,
                    }
                    if rules.targets.contains(&y) {
                        return Hidden::Possible(format!(
                            "its parent chain passes pid {} ({}), an ancestor of a supervisor session of this campaign (a possible re-parent target)",
                            y.0, sy.comm
                        ));
                    }
                    if sy.start_ticks < rules.earliest {
                        return Hidden::NotOurs(format!(
                            "its parent chain leaves through pid {} ({}, start {}), older than every session of this campaign and not a possible re-parent target",
                            y.0, sy.comm, sy.start_ticks
                        ));
                    }
                    hops += 1;
                    if hops > MAX_HOPS {
                        return Hidden::Possible("parent chain longer than the bound".into());
                    }
                    x = y;
                }
            }
        }
    }
    Hidden::Possible("its parent chain changed during three reads".into())
}

/// A candidate of a strict scan that could not be inspected and may descend from the campaign (never signalled).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct HiddenProcess {
    pub pid: u32,
    pub start_ticks: u64,
    pub ppid: u32,
    pub comm: String,
    pub why: String,
}

impl HiddenProcess {
    pub(crate) fn new(st: &ProcStat, why: String) -> Self {
        Self {
            pid: st.pid,
            start_ticks: st.start_ticks,
            ppid: st.ppid,
            comm: st.comm.clone(),
            why,
        }
    }
}

/// Short text for logs: up to 8 entries.
pub fn describe_hidden(h: &[HiddenProcess]) -> String {
    let mut parts: Vec<String> = h
        .iter()
        .take(8)
        .map(|p| {
            format!(
                "pid {} ({}, start {}, ppid {}): {}",
                p.pid, p.comm, p.start_ticks, p.ppid, p.why
            )
        })
        .collect();
    if h.len() > 8 {
        parts.push(format!("and {} more", h.len() - 8));
    }
    parts.join("; ")
}

/// The rules in force per campaign id in THIS process (installed by the supervisor after reading its ledger).
static REGISTRY: Mutex<BTreeMap<String, Arc<LineageRules>>> = Mutex::new(BTreeMap::new());

/// Install (Some) or remove (None) the rules of a campaign id in this process.
pub fn install(campaign_id: &str, rules: Option<LineageRules>) {
    let mut g = REGISTRY.lock().unwrap_or_else(|p| p.into_inner());
    match rules {
        Some(r) => {
            g.insert(campaign_id.to_string(), Arc::new(r));
        }
        None => {
            g.remove(campaign_id);
        }
    }
}

/// The rules of a campaign id, only if they were installed by THIS process (the live supervisor of the rules).
pub fn rules_for(campaign_id: &str) -> Option<Arc<LineageRules>> {
    let g = REGISTRY.lock().unwrap_or_else(|p| p.into_inner());
    g.get(campaign_id)
        .filter(|r| r.live.0 == std::process::id())
        .cloned()
}

/// Classify a candidate of a strict scan of `campaign_id` whose environ could not be read (production: `/proc`).
pub(crate) fn classify_unreadable(root: &Path, st: &ProcStat, campaign_id: &str) -> Hidden {
    match rules_for(campaign_id) {
        Some(r) => classify_hidden_at(root, st.pid, st.start_ticks, &r),
        None => Hidden::Possible(
            "environ unreadable and no lineage rule in force for this campaign".into(),
        ),
    }
}

#[allow(clippy::let_underscore_must_use)] // test cleanup (outside the reviewed scope, procfix7)
#[cfg(test)]
mod tests {
    use super::*;

    struct Fake(std::path::PathBuf);
    impl Fake {
        fn new(tag: &str) -> Self {
            let p = std::env::temp_dir().join(format!(
                "rr-lineage-{tag}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir_all(&p).unwrap();
            Self(p)
        }
        /// A process: pid, comm, ppid, start; `ns`: the NSpid line (None: "<pid>").
        fn put(&self, pid: u32, comm: &str, ppid: u32, start: u64, ns: Option<&str>) {
            let d = self.0.join(pid.to_string());
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(
                d.join("stat"),
                format!(
                    "{pid} ({comm}) S {ppid} {pid} {pid} 0 -1 0 0 0 0 0 1 1 0 0 20 0 1 0 {start} 0 0\n"
                ),
            )
            .unwrap();
            std::fs::write(
                d.join("status"),
                format!(
                    "Name:\t{comm}\nPPid:\t{ppid}\nNSpid:\t{}\n",
                    ns.map_or(pid.to_string(), str::to_string)
                ),
            )
            .unwrap();
        }
    }
    impl Drop for Fake {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn lineage(pid: u32, start: u64, boot: &str, ns: u64, anc: &[Link]) -> SessionLineage {
        SessionLineage {
            pid,
            start_ticks: start,
            boot_id: boot.into(),
            pid_ns: ns,
            ancestors: anc.to_vec(),
            time_ns: None,
        }
    }

    fn session(pid: u32, start: u64, boot: &str) -> RecordedSession {
        RecordedSession {
            pid,
            start_ticks: start,
            boot_id: boot.into(),
        }
    }

    /// The host shape of gate6 MJ-A: init(1) -> tailscaled(root, old) -> be-child (young, unreadable): NOT OURS while
    /// the live supervisor runs below init -> zellij -> shell; a non-dumpable child of the supervisor and a daemonised
    /// orphan re-parented to init after a DEAD session are POSSIBLE; a nested pid-namespace init in the chain is a stop.
    #[test]
    fn mj_a_parent_chain_classification() {
        let f = Fake::new("chain");
        f.put(1, "systemd", 0, 1, None);
        f.put(700, "tailscaled", 1, 2_663, None);
        f.put(800, "zellij", 1, 5_000, None);
        f.put(810, "bash", 800, 6_000, None);
        f.put(900, "rr_campaign", 810, 10_000, None); // live session
        f.put(2001, "be-child ssh", 700, 12_000, None); // young, below an old non-target
        f.put(2002, "agent", 900, 12_500, None); // below the live supervisor
        f.put(2003, "grandchild", 2002, 12_600, None);
        f.put(2010, "daemon", 1, 12_700, None); // re-parented to init
        f.put(2020, "ns-init", 2021, 12_800, Some("2020 1")); // init of a nested pid namespace
        f.put(2021, "unshare", 810, 12_790, None);
        f.put(2022, "inside", 2020, 12_900, Some("2022 5"));
        f.put(2030, "other-sup", 2031, 13_000, None); // another campaign's supervisor in a young shell
        f.put(2031, "bash", 800, 12_000, None);
        let me = lineage(900, 10_000, "b1", 7, &[(810, 6_000), (800, 5_000), (1, 1)]);
        // Uninterrupted run (only the live session), the supervisor a subreaper.
        let r = LineageRules::build(
            &[session(900, 10_000, "b1")],
            std::slice::from_ref(&me),
            &me,
            true,
        )
        .unwrap();
        assert!(r.targets.is_empty() && r.earliest == 10_000);
        let c = |pid: u32, start: u64, r: &LineageRules| classify_hidden_at(&f.0, pid, start, r);
        assert!(matches!(c(2001, 12_000, &r), Hidden::NotOurs(w) if w.contains("pid 700")));
        assert!(
            matches!(c(2002, 12_500, &r), Hidden::Possible(w) if w.contains("this supervisor"))
        );
        assert!(
            matches!(c(2003, 12_600, &r), Hidden::Possible(w) if w.contains("this supervisor"))
        );
        // ppid 1 (old, and with only the live subreaper session not a target): no subtree of a session reaches it.
        assert!(matches!(c(2010, 12_700, &r), Hidden::NotOurs(_)));
        assert!(
            matches!(c(2022, 12_900, &r), Hidden::Possible(w) if w.contains("nested pid namespace"))
        );
        // The other campaign's supervisor: young shell, then zellij (old, not a target) -> not ours.
        assert!(matches!(c(2030, 13_000, &r), Hidden::NotOurs(w) if w.contains("pid 800")));
        // A wrong start time (the pid names another process): not a candidate.
        assert_eq!(c(2001, 11_999, &r), Hidden::Gone);
        // After a crash: an earlier session (pid 600, dead) started from the same shell; its ancestors are targets.
        let dead = lineage(600, 8_000, "b1", 7, &[(810, 6_000), (800, 5_000), (1, 1)]);
        let r2 = LineageRules::build(
            &[session(600, 8_000, "b1"), session(900, 10_000, "b1")],
            &[dead.clone(), me.clone()],
            &me,
            true,
        )
        .unwrap();
        assert_eq!(r2.earliest, 8_000);
        assert!(matches!(c(2001, 12_000, &r2), Hidden::NotOurs(w) if w.contains("pid 700")));
        assert!(
            matches!(c(2010, 12_700, &r2), Hidden::Possible(w) if w.contains("re-parent target"))
        );
        assert!(matches!(c(2030, 13_000, &r2), Hidden::Possible(w) if w.contains("pid 800")));
        // The live supervisor NOT a subreaper: its own ancestors are targets as well.
        let r3 = LineageRules::build(
            &[session(900, 10_000, "b1")],
            std::slice::from_ref(&me),
            &me,
            false,
        )
        .unwrap();
        assert!(matches!(c(2010, 12_700, &r3), Hidden::Possible(_)));
        assert!(matches!(c(2001, 12_000, &r3), Hidden::NotOurs(_)));
    }

    /// implementer (1): a recorded ancestor that died and whose pid was reused by a younger process is recognised by
    /// its start time (not a target); a parent that is younger than its child is never a proof.
    #[test]
    fn mj_a_reused_pid_of_a_recorded_ancestor_is_not_a_target() {
        let f = Fake::new("reuse");
        f.put(1, "init", 0, 1, None);
        f.put(700, "tailscaled", 1, 100, None);
        f.put(810, "sshd-child", 700, 15_000, None); // pid 810 REUSED: the recorded ancestor (810, 6000) is dead
        f.put(2001, "be-child", 810, 15_100, None);
        let me = lineage(900, 10_000, "b1", 7, &[(1, 1)]);
        let dead = lineage(600, 8_000, "b1", 7, &[(810, 6_000), (1, 1)]);
        let r = LineageRules::build(
            &[session(600, 8_000, "b1"), session(900, 10_000, "b1")],
            &[dead, me.clone()],
            &me,
            true,
        )
        .unwrap();
        assert!(r.targets.contains(&(810, 6_000)));
        // 810 now names a young process (start 15000): walked through, then 700 (old, not a target) decides.
        assert!(
            matches!(classify_hidden_at(&f.0, 2001, 15_100, &r), Hidden::NotOurs(w) if w.contains("pid 700"))
        );
        // A parent younger than its child: inconsistent, never a proof.
        f.put(2050, "odd", 2051, 15_200, None);
        f.put(2051, "younger-parent", 700, 15_300, None);
        assert!(matches!(
            classify_hidden_at(&f.0, 2050, 15_200, &r),
            Hidden::Possible(_)
        ));
    }

    /// implementer (2): the rule is OFF for a session of this boot without a lineage, for a lineage from another pid
    /// namespace and for a lineage that names another boot than its session record; sessions of an EARLIER boot (a
    /// ledger carried across a reboot) contribute neither the bound nor targets.
    #[test]
    fn mj_a_rule_off_without_a_complete_lineage_of_this_boot_and_namespace() {
        let me = lineage(900, 10_000, "b2", 7, &[(1, 1)]);
        let s_me = session(900, 10_000, "b2");
        // a session of this boot without a note
        assert!(
            LineageRules::build(
                &[session(600, 8_000, "b2"), s_me.clone()],
                std::slice::from_ref(&me),
                &me,
                true
            )
            .unwrap_err()
            .contains("no lineage")
        );
        // another pid namespace
        let other_ns = lineage(600, 8_000, "b2", 8, &[(1, 1)]);
        assert!(
            LineageRules::build(
                &[session(600, 8_000, "b2"), s_me.clone()],
                &[other_ns, me.clone()],
                &me,
                true
            )
            .unwrap_err()
            .contains("pid namespace")
        );
        // a note that names another boot than its session record
        let wrong_boot = lineage(600, 8_000, "b1", 7, &[(1, 1)]);
        assert!(
            LineageRules::build(
                &[session(600, 8_000, "b2"), s_me.clone()],
                &[wrong_boot, me.clone()],
                &me,
                true
            )
            .unwrap_err()
            .contains("another boot")
        );
        // a ledger carried across a reboot: the earlier boot's session (pid 600, recorded under boot b1, with or
        // without a note) contributes nothing: the bound is this boot's earliest session, no target from b1.
        let old = lineage(600, 3_000, "b1", 99, &[(1, 1), (77, 2)]);
        let r = LineageRules::build(
            &[
                session(600, 3_000, "b1"),
                session(601, 4_000, "b1"),
                s_me.clone(),
            ],
            &[old, me.clone()],
            &me,
            true,
        )
        .unwrap();
        assert_eq!(r.earliest, 10_000);
        assert!(r.targets.is_empty());
        // the note round trip
        let t = me.note_text();
        assert!(t.starts_with("session_lineage {"));
        assert_eq!(SessionLineage::parse_note(&t), Some(me));
        assert_eq!(SessionLineage::parse_note("session_lineagex {}"), None);
    }

    /// procfix7 (gate7 MJ-2): a session of this boot that ran in another TIME namespace switches the rule OFF (start
    /// times do not compare across a boottime offset); unknown time namespaces (older notes) do not.
    #[test]
    fn mj2_another_time_namespace_switches_the_rule_off() {
        let me = SessionLineage {
            time_ns: Some(11),
            ..lineage(900, 10_000, "b2", 7, &[(1, 1)])
        };
        let other = SessionLineage {
            time_ns: Some(12),
            ..lineage(600, 8_000, "b2", 7, &[(1, 1)])
        };
        let s = [session(600, 8_000, "b2"), session(900, 10_000, "b2")];
        assert!(
            LineageRules::build(&s, &[other, me.clone()], &me, true)
                .unwrap_err()
                .contains("time namespace")
        );
        let unknown = lineage(600, 8_000, "b2", 7, &[(1, 1)]);
        assert!(LineageRules::build(&s, &[unknown, me.clone()], &me, true).is_ok());
        // the current time namespace unreadable while an earlier session recorded one: different
        let me_unknown = SessionLineage {
            time_ns: None,
            ..me.clone()
        };
        let recorded = SessionLineage {
            time_ns: Some(11),
            ..lineage(600, 8_000, "b2", 7, &[(1, 1)])
        };
        assert!(
            LineageRules::build(&s, &[recorded, me_unknown.clone()], &me_unknown, true)
                .unwrap_err()
                .contains("time namespace")
        );
    }

    /// Rules installed for one campaign are not used for another, and only by the process that installed them.
    #[test]
    fn mj_a_registry_is_per_campaign_and_per_process() {
        let me = lineage(std::process::id(), 1, "b", 1, &[]);
        let r = LineageRules::build(&[], &[], &me, true).unwrap();
        install("lin-test-a.0123456789abcdef", Some(r.clone()));
        assert!(rules_for("lin-test-a.0123456789abcdef").is_some());
        assert!(rules_for("lin-test-b.0123456789abcdef").is_none());
        install(
            "lin-test-c.0123456789abcdef",
            Some(LineageRules {
                live: (std::process::id().wrapping_add(1), 1),
                ..r
            }),
        );
        assert!(rules_for("lin-test-c.0123456789abcdef").is_none());
        install("lin-test-a.0123456789abcdef", None);
        assert!(rules_for("lin-test-a.0123456789abcdef").is_none());
    }
}
