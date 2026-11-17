//! THE END-OF-JOB HUNT AND THE PROOF THAT A JOB IS GONE (gate re-audit B RB-C1, RB-C2; re-audit A m-3).
//!
//! Runs on a helper thread (never on the control loop) once the job's leader has exited. Its result is either a PROOF
//! (`Verified`) built from positive evidence only, or `NotProven { reason }`: an inconclusive read is never "empty".
//!
//! Who is "ours" (the job J): a process in J's session `sid` while J's leader is held UNREAPED in the registry (the
//! number `sid` is then pinned: only J's descendants can be in that session), or a process whose environ carries J's
//! marker (campaign id AND attempt). "Not ours" only by a conclusive read: environ readable without J's marker and
//! another session, or a real uid that is not ours, or a start time before J's launch, or the supervisor's own session.
//! An unreadable environ of a process of our uid, any failed read, or a failed enumeration makes the pass INCONCLUSIVE.
//!
//! Enumeration:
//! - `SubreaperTree` (launched jobs, the supervisor is a child subreaper or pid 1): every live process J created is in the
//!   supervisor's descendant tree. Candidates = our direct children (kernel children lists, read until stable) and the
//!   children of every member we stopped. In the proof passes the direct children are ALSO enumerated by a strict full
//!   /proc listing (ppid == us), so a child missed by one enumeration is caught by the other.
//! - `FullProc` (adopted jobs of a previous supervisor, or no subreaper): every process of a strict full /proc listing;
//!   an entry that vanishes during a proof pass makes that pass inconclusive.
//!
//! The hunt (kill mode):
//! 1. job cgroup (delegated subtree only): `cgroup.freeze` = 1, wait for `frozen 1` (bounded), `cgroup.kill`;
//! 2. SIGSTOP to the pinned leader group, then a FIXED-POINT loop: every candidate that is ours is SIGSTOPped through the
//!    guard (pidfd opened BEFORE the facts were read; the same pidfd carries the signal), we wait until all its threads
//!    are stopped, then read ITS children (a stopped process cannot fork; a fork that completed before the stop left the
//!    child in its children list). The loop ends when a complete round (our children + children of every stopped member)
//!    finds nothing new, every member is still stopped, and no read was inconclusive; it is bounded (rounds and time);
//! 3. SIGKILL to the pinned group and to every stopped member (each through the guard), wait until every pidfd reports
//!    the exit;
//! 4. TWO consecutive conclusive proof passes, `pass_gap` apart, both empty (no member of J's session, no process with J's
//!    marker, and for a cgroup job an empty `cgroup.procs`), with the leader held unreaped during both. A pass that finds
//!    a survivor sends the hunt back to step 2 while the budget lasts.
//!
//! Observe mode (an adopted job the caller only monitors): no signal at all; two passes; survivors are reported.
//!
//! Third fix package (procfix3, gate-A M-1): a live process of the job that DROPPED the marker (exec with `env -i` or
//! without the variable) in another session is not ours and is never signalled, but it no longer shields its
//! descendants: in tree mode the walk DESCENDS through such marker-less processes (it reads their children lists without
//! signalling them; a marker-less process cannot hand the marker to new children unless it re-sets it, which is out of
//! scope) and every descendant is classified as usual. Each proof pass also carries two BELTS that can only add
//! survivors, never make a pass empty: a strict full /proc scan for the job's EXACT marker (any live process of our uid
//! carrying it is a survivor, wherever it is in the process tree), and, for a job in a systemd scope, the scope's
//! kernel-maintained `cgroup.procs` (a live process in it other than the pinned leader is a survivor; after the guarded
//! kill the scope gets `cgroup.kill`). A process whose STOP was delivered but whose stop could not be confirmed stays a
//! member and gets the SIGKILL (gate-A m-2).

use super::cgroup::ScopeRef;
use super::guard::{self, ProcessTarget, Proof, StartRule, Verdict};
use super::{Enumeration, LeftoverRecord, UnattributedChild, VerifiedBy};
use crate::resources::procfs::{self, ProcError};
use crate::resources::{
    CampaignMarker, FactRead, Leaders, MarkerFact, ProcFacts, children_of, our_uid, own_children,
    own_children_by_scan, read_proc_facts, request_reap, scan_marked_inline,
    strict_listing_counted,
};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::os::fd::OwnedFd;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// procfix7 round 6 (gate10 M10-2): pinned groups (session, launch ticks) whose group SIGKILL was delivered.
static GROUP_KILLED: std::sync::Mutex<std::collections::BTreeSet<(u32, u64)>> =
    std::sync::Mutex::new(std::collections::BTreeSet::new());

/// What to hunt.
#[derive(Clone, Debug)]
pub(crate) struct HuntSpec {
    /// The job's session id (= the leader's pid).
    pub sid: u32,
    /// Start ticks of the leader = the job's launch time; nothing of the job started earlier.
    pub launch_ticks: u64,
    pub marker: CampaignMarker,
    pub boot_id: String,
    /// The leader is our unreaped, registered child (session proof available).
    pub pinned: bool,
    /// Kill mode (false: observe only).
    pub kill: bool,
    pub enumeration: Enumeration,
    /// The job's own cgroup (created by us in a delegated subtree), if any.
    pub cgroup: Option<PathBuf>,
    /// The job's systemd scope (an additional belt, procfix3), if recorded.
    pub scope: Option<ScopeRef>,
    pub leaders: Arc<Leaders>,
    /// Time budget of this invocation.
    pub budget: Duration,
    pub max_rounds: u32,
    pub pass_gap: Duration,
    pub stop_wait: Duration,
}

#[derive(Clone, Debug)]
pub(crate) enum HuntResult {
    Proven {
        by: VerifiedBy,
        killed: Vec<LeftoverRecord>,
        unattributed: Vec<UnattributedChild>,
    },
    NotProven {
        reason: String,
        left: Vec<LeftoverRecord>,
        killed: Vec<LeftoverRecord>,
        unattributed: Vec<UnattributedChild>,
    },
    /// Observe mode: the job still has live processes.
    Survivors { left: Vec<LeftoverRecord> },
}

enum Class {
    Ours(Proof, ProcFacts),
    NotOurs {
        unattributed: bool,
        /// A live process of our uid, started after the launch, outside our own session, WITHOUT a marker (it dropped
        /// it): never signalled, but the walk descends through it (procfix3, gate-A M-1).
        descend: bool,
        facts: Option<ProcFacts>,
    },
    Dead,
    Inconclusive(String),
}

struct Member {
    fd: OwnedFd,
    facts: ProcFacts,
    proof: Proof,
    /// procfix6 (gate5 m1): the identity held at the stop this hunt delivered.
    hold: Option<guard::StopHold>,
}

/// procfix7 round 10 (gate13c G13c-M1): a raw listing of our direct children ([`Hunt::snapshot_tops`]).
#[derive(Default)]
struct Tops {
    /// Alive when read: pid -> start ticks.
    live: BTreeMap<u32, u64>,
    /// Listed, but ended when read: (pid, (start, session) of a zombie; None: gone before its stat was read).
    ended: Vec<(u32, Option<(u64, u32)>)>,
}

enum Stop {
    Stopped,
    Exited,
    /// The STOP was delivered but the stop could not be confirmed (D state, a read failed): still a member (gate-A m-2).
    DeliveredUnconfirmed(String),
    Failed(String),
}

fn me() -> u32 {
    std::process::id()
}

/// Opt-in trace of the hunt's decisions to stderr (`RR_HUNT_TRACE=1`; diagnostics, no secrets: pids, states, ids).
/// TEST BUILDS ONLY (gate3 m1, gate rule d: a debugging variable must not change a release build's behaviour): a release
/// build never reads the variable. The write ignores errors (a broken pipe / hung-up terminal never panics a hunt
/// thread, which would leave the job end unproven forever).
#[cfg(debug_assertions)]
fn trace(msg: impl FnOnce() -> String) {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if *ON.get_or_init(|| std::env::var_os("RR_HUNT_TRACE").is_some()) {
        use std::io::Write;
        #[allow(clippy::let_underscore_must_use)]
        // ignored-ok (procfix7): a trace line on stderr (diagnostics only)
        let _ = writeln!(std::io::stderr(), "HUNT {}", msg());
    }
}

#[cfg(not(debug_assertions))]
#[inline(always)]
fn trace(_msg: impl FnOnce() -> String) {}

fn own_session() -> u32 {
    // SAFETY: getsid(0) reads the caller's session id.
    unsafe { libc::getsid(0) }.max(0) as u32
}

fn record(f: &ProcFacts) -> LeftoverRecord {
    LeftoverRecord {
        pid: f.stat.pid,
        start_ticks: f.stat.start_ticks,
        pgrp: f.stat.pgrp,
        comm: f.stat.comm.clone(),
    }
}

struct Hunt<'a> {
    s: &'a HuntSpec,
    cancel: &'a AtomicBool,
    deadline: Instant,
    own_session: u32,
    uid: u32,
    /// Conclusive "not ours" verdicts, with whether the walk descends through the process.
    not_ours: HashMap<(u32, u64), bool>,
    unattributed: BTreeMap<(u32, u64), UnattributedChild>,
    killed: Vec<LeftoverRecord>,
    cgroup_ok: bool,
    /// procfix8 (gate8, item 4): every candidate of this hunt invocation that was ours or inconclusive (pid -> start
    /// held, 0 when unknown). A proof pass classifies directly every suspect its enumeration did not reach; one leaves
    /// only by a complete read (its process gone by pidfd / ENOENT / ESRCH or another start time, or conclusively not
    /// J's), so a pass whose enumeration misses one cannot prove the end.
    suspects: BTreeMap<u32, u64>,
    /// The pids classified in the current proof pass.
    classified_now: BTreeSet<u32>,
}

impl Hunt<'_> {
    /// A marker-found process through THE ONE OWNERSHIP RULE (`guard::marker_target`, shared with the campaign sweep
    /// and the job signaller); a member of the pinned session through the session proof.
    fn target(&self, f: &ProcFacts, proof: Proof) -> ProcessTarget {
        if let Proof::Marker(m) = &proof
            && let Some(t) = guard::marker_target(
                &m.campaign_id,
                Some(&m.attempt_id),
                f.stat.pid,
                StartRule::Exact(f.stat.start_ticks),
                &self.s.boot_id,
            )
        {
            return t;
        }
        ProcessTarget {
            pid: f.stat.pid,
            boot_id: self.s.boot_id.clone(),
            start: StartRule::Exact(f.stat.start_ticks),
            require_session_leader: false,
            proof,
        }
    }

    fn registered(&self, pid: u32) -> bool {
        self.s.leaders.lock().contains(&pid)
    }

    /// Classify one candidate. `parent`: the process whose children list named it (None: a full listing). The pidfd is
    /// opened FIRST; the facts are valid only if the pidfd still reports the process alive after the reads.
    fn classify(&mut self, pid: u32, parent: Option<u32>) -> (Class, Option<OwnedFd>) {
        let (class, fd) = self.classify_inner(pid, parent);
        self.classified_now.insert(pid);
        match &class {
            Class::Ours(_, f) => {
                self.suspects.insert(pid, f.stat.start_ticks);
            }
            Class::Inconclusive(_) => {
                self.suspects.entry(pid).or_insert(0);
            }
            Class::NotOurs { .. } | Class::Dead => {}
        }
        (class, fd)
    }

    /// procfix8 (item 4): the suspects of this hunt that the current pass did not classify: Ok(survivors) or Err
    /// (inconclusive).
    fn recheck_suspects(&mut self, survivors: &mut Vec<LeftoverRecord>) -> Result<(), String> {
        let pending: Vec<(u32, u64)> = self
            .suspects
            .iter()
            .filter(|(p, _)| !self.classified_now.contains(p))
            .map(|(p, s)| (*p, *s))
            .collect();
        for (pid, start) in pending {
            if self.registered(pid) || pid == self.s.sid {
                self.suspects.remove(&pid);
                continue;
            }
            match crate::resources::read_stat(pid) {
                Err(ProcError::Gone) => {
                    self.suspects.remove(&pid);
                    continue;
                }
                Err(ProcError::Unreadable(w)) => {
                    return Err(format!("suspect {pid} of this hunt: stat unreadable: {w}"));
                }
                Ok(st) if start != 0 && st.start_ticks != start => {
                    // the pid names a newer process: the suspect is dead
                    self.suspects.remove(&pid);
                    continue;
                }
                Ok(_) => {}
            }
            match self.classify(pid, None).0 {
                Class::Ours(_, f) => survivors.push(record(&f)),
                Class::Inconclusive(e) => {
                    return Err(format!("suspect {pid} of this hunt: {e}"));
                }
                Class::NotOurs { .. } | Class::Dead => {
                    self.suspects.remove(&pid);
                }
            }
        }
        Ok(())
    }

    fn classify_inner(&mut self, pid: u32, parent: Option<u32>) -> (Class, Option<OwnedFd>) {
        let me = me();
        let not_ours = |descend: bool, facts: Option<ProcFacts>| Class::NotOurs {
            unattributed: false,
            descend,
            facts,
        };
        if pid <= 1 || pid == me {
            return (not_ours(false, None), None);
        }
        if self.registered(pid) {
            // Another job's live leader, or J's own zombie leader: never J's survivor.
            return (not_ours(false, None), None);
        }
        let fd = match guard::open_pidfd(pid) {
            Ok(fd) => fd,
            Err(Verdict::Gone) => return (Class::Dead, None),
            Err(Verdict::NotOurs(_)) => return (not_ours(false, None), None),
            Err(Verdict::Unverified(e)) => return (Class::Inconclusive(e), None),
            Err(Verdict::Delivered) => return (Class::Inconclusive("impossible".into()), None),
        };
        let facts = match read_proc_facts(pid, true) {
            FactRead::Gone => return (Class::Dead, None),
            FactRead::Failed(e) => return (Class::Inconclusive(e), None),
            FactRead::Facts(f) => f,
        };
        match guard::pidfd_exited(&fd) {
            Some(true) => return (Class::Dead, None),
            None => return (Class::Inconclusive(format!("poll pidfd of {pid}")), None),
            Some(false) => {}
        }
        if let Some(p) = parent
            && facts.stat.ppid != p
            && facts.stat.ppid != me
        {
            return (
                Class::Inconclusive(format!(
                    "pid {pid}: parent changed during the walk ({} != {p})",
                    facts.stat.ppid
                )),
                None,
            );
        }
        if facts.dead {
            return (Class::Dead, None);
        }
        let key = (pid, facts.stat.start_ticks);
        if let Some(&descend) = self.not_ours.get(&key) {
            return (not_ours(descend, None), None);
        }
        let direct_child = facts.stat.ppid == me;
        if facts.stat.start_ticks < self.s.launch_ticks {
            self.not_ours.insert(key, false);
            return (not_ours(false, Some(facts)), None);
        }
        if self.s.pinned && facts.stat.session == self.s.sid {
            return (
                Class::Ours(Proof::PinnedSession(self.s.sid), facts),
                Some(fd),
            );
        }
        if facts.stat.session == self.own_session {
            // gate3 M2 / m2 (lane int5): ONE ownership rule. A process in the supervisor's own session cannot descend
            // from a job (every job leader setsid's) and is never walked through; but if it carries J's EXACT marker
            // it is ours by the marker rule (as for the sweep and the job signaller): stopped and killed through the
            // guard and recorded, instead of being skipped by the walk and spun on by the marker belt for the whole
            // budget, then killed unrecorded by the final job signal (gate3 m2, M2).
            if let MarkerFact::Value(v) = &facts.marker
                && CampaignMarker::parse(v).is_some_and(|m| m == self.s.marker)
            {
                return (
                    Class::Ours(Proof::Marker(self.s.marker.clone()), facts),
                    Some(fd),
                );
            }
            self.not_ours.insert(key, false);
            return (not_ours(false, Some(facts)), None);
        }
        // Marker-less processes of our uid, started after the launch, outside our session: walked through.
        let descend = facts.real_uid == self.uid;
        match &facts.marker {
            MarkerFact::Value(v) => match CampaignMarker::parse(v) {
                Some(m) if m == self.s.marker => (Class::Ours(Proof::Marker(m), facts), Some(fd)),
                _ => {
                    self.not_ours.insert(key, false);
                    (not_ours(false, Some(facts)), None)
                }
            },
            MarkerFact::Absent => {
                self.not_ours.insert(key, descend);
                (
                    Class::NotOurs {
                        unattributed: direct_child,
                        descend,
                        facts: Some(facts),
                    },
                    None,
                )
            }
            // Not cached: an empty environment is re-examined in every round and pass.
            MarkerFact::EmptyEnviron => (
                Class::NotOurs {
                    unattributed: direct_child,
                    descend,
                    facts: Some(facts),
                },
                None,
            ),
            MarkerFact::Unreadable(e) => {
                if facts.real_uid != self.uid {
                    self.not_ours.insert(key, false);
                    return (not_ours(false, Some(facts)), None);
                }
                // procfix7 (gate6 MJ-A): a process of our uid whose environ cannot be read (non-dumpable) is NOT J's
                // when its verified parent chain proves it outside every session of the campaign (resources::lineage);
                // otherwise (e.g. below this supervisor) the pass stays inconclusive. Never signalled either way.
                match crate::resources::lineage::classify_unreadable(
                    &procfs::sys(),
                    &facts.stat,
                    &self.s.marker.campaign_id,
                ) {
                    crate::resources::lineage::Hidden::NotOurs(_) => {
                        self.not_ours.insert(key, false);
                        (not_ours(false, Some(facts)), None)
                    }
                    // (facts.stat may be a live thread's stat when the leader is a zombie: never "dead" from here)
                    crate::resources::lineage::Hidden::Gone => (
                        Class::Inconclusive(format!(
                            "pid {pid}: environ unreadable ({e}); identity changed during the parent-chain walk"
                        )),
                        None,
                    ),
                    crate::resources::lineage::Hidden::Possible(why) => (
                        Class::Inconclusive(format!(
                            "pid {pid} ({}): environ unreadable ({e}); {why}",
                            facts.stat.comm
                        )),
                        None,
                    ),
                }
            }
        }
    }

    fn note_unattributed(&mut self, f: &ProcFacts) {
        self.unattributed
            .entry((f.stat.pid, f.stat.start_ticks))
            .or_insert_with(|| UnattributedChild {
                pid: f.stat.pid,
                start_ticks: f.stat.start_ticks,
                session: f.stat.session,
                comm: f.stat.comm.clone(),
            });
    }

    fn wait_stopped(&self, pid: u32, fd: &OwnedFd) -> Stop {
        let t0 = Instant::now();
        loop {
            if guard::pidfd_exited(fd) == Some(true) {
                return Stop::Exited;
            }
            match read_proc_facts(pid, false) {
                FactRead::Gone => return Stop::Exited,
                FactRead::Failed(e) => return Stop::Failed(e),
                FactRead::Facts(f) if f.dead => return Stop::Exited,
                FactRead::Facts(f) if f.all_stopped => return Stop::Stopped,
                FactRead::Facts(_) => {}
            }
            if t0.elapsed() > self.s.stop_wait {
                return Stop::Failed(format!("pid {pid} did not stop (D state?)"));
            }
            std::thread::sleep(Duration::from_micros(200));
        }
    }

    fn stop(&self, fd: &OwnedFd, f: &ProcFacts, proof: &Proof) -> (Stop, Option<guard::StopHold>) {
        let t = self.target(f, proof.clone());
        let (v, hold) = guard::stop_with_pidfd(fd, &t, Some(&self.s.leaders));
        let st = match v {
            Verdict::Delivered => match self.wait_stopped(f.stat.pid, fd) {
                Stop::Failed(e) => Stop::DeliveredUnconfirmed(e),
                other => other,
            },
            Verdict::Gone => Stop::Exited,
            Verdict::NotOurs(r) => Stop::Failed(format!("pid {}: refused: {r:?}", f.stat.pid)),
            Verdict::Unverified(e) => Stop::Failed(e),
        };
        (st, hold)
    }

    fn out_of_time(&self) -> bool {
        self.cancel.load(Ordering::SeqCst) || Instant::now() >= self.deadline
    }

    /// procfix7 round 10 (gate13c G13c-M1): a RAW listing of our direct children (registered leaders left out: other
    /// jobs' live leaders and J's own zombie), from the kernel's children lists and, with `with_scan`, also from the
    /// strict full /proc listing (ppid == us), with what each entry was when its stat was read: alive, or ENDED (gone at
    /// the read, or a zombie). An ended entry may have HANDED OFF to a child born after the listing (a re-forking job),
    /// so it is never "absent": [`Self::handoff_suspects`]. Strict: a failed enumeration or stat read is an error.
    fn snapshot_tops(&self, with_scan: bool) -> Result<Tops, String> {
        let mut kids = own_children().map_err(|e| format!("own children: {e}"))?;
        crate::resources::listing_hold();
        if with_scan {
            kids.extend(own_children_by_scan().map_err(|e| format!("ppid scan: {e}"))?);
        }
        let reg: BTreeSet<u32> = self.s.leaders.lock().clone();
        let mut out = Tops::default();
        for pid in kids {
            if reg.contains(&pid) {
                continue;
            }
            match crate::resources::read_stat(pid) {
                Ok(st) if st.is_dead() => out.ended.push((pid, Some((st.start_ticks, st.session)))),
                Ok(st) => {
                    out.live.insert(pid, st.start_ticks);
                }
                Err(ProcError::Gone) => out.ended.push((pid, None)),
                Err(ProcError::Unreadable(e)) => return Err(format!("stat of child {pid}: {e}")),
            }
        }
        Ok(out)
    }

    /// procfix7 round 10 (gate13c G13c-M1): was (pid, start) a member this hunt STOPPED and then killed? (stopped before
    /// it died: it cannot have handed off.) `start` None: the pid alone (an entry gone before its stat was read).
    fn killed_member(&self, pid: u32, start: Option<u64>) -> bool {
        self.killed
            .iter()
            .any(|k| k.pid == pid && start.is_none_or(|s| k.start_ticks == s))
    }

    /// procfix7 round 10 (gate13c G13c-M1): the ENDED entries of a raw listing that may have handed off to a successor
    /// born after the listing. Not suspects: a member this hunt stopped and killed; a zombie of the supervisor's OWN
    /// session (its helpers; no job process is in it: every job leader setsid's); a zombie that started before J's
    /// launch (not J's). Every other ended entry (gone at its read, or a zombie of another session) is a suspect.
    fn handoff_suspects(&self, t: &Tops) -> Vec<u32> {
        t.ended
            .iter()
            .filter(|(pid, info)| match info {
                Some((start, session)) => {
                    !(self.killed_member(*pid, Some(*start))
                        || *session == self.own_session
                        || *start < self.s.launch_ticks)
                }
                None => !self.killed_member(*pid, None),
            })
            .map(|(pid, _)| *pid)
            .collect()
    }

    fn trace_class(&self, pid: u32, parent: Option<u32>, class: &Class) {
        trace(|| {
            format!(
                "sid {} pid {pid} parent {parent:?}: {}",
                self.s.sid,
                match class {
                    Class::Ours(p, f) => format!("OURS {p:?} state {}", f.stat.state),
                    Class::NotOurs {
                        unattributed,
                        descend,
                        facts,
                    } => format!(
                        "not ours (unattributed {unattributed}, descend {descend}) {:?}",
                        facts.as_ref().map(|f| (f.stat.session, f.marker.clone()))
                    ),
                    Class::Dead => "dead".into(),
                    Class::Inconclusive(e) => format!("INCONCLUSIVE {e}"),
                }
            )
        });
    }

    /// Step 2: stop every member until a fixed point. Ok(members) at the fixed point; Err(reason) when the bound is hit.
    ///
    /// Why a fixed point needs a STABLE set of our children (and not just "every candidate examined was dead or not
    /// ours"): a candidate alive when the children list was read but dead when examined may have handed off to a child
    /// born after that read (the relay of re-audit B); that child is re-parented to us when the candidate dies. So every
    /// round ends with a fresh snapshot of our children (pid AND start time): any entry not examined yet sends the loop
    /// round again; the newest children are examined first.
    fn freeze_fixed_point(
        &mut self,
    ) -> Result<BTreeMap<u32, Member>, (String, BTreeMap<u32, Member>)> {
        let mut known: BTreeMap<u32, Member> = BTreeMap::new();
        if self.s.pinned && !self.group_killed() {
            #[allow(clippy::let_underscore_must_use)]
            // ignored-ok (procfix7): the fixed point and two proof passes decide; a failed stop only costs rounds
            let _ = guard::signal_pinned_group(&self.s.leaders, self.s.sid, libc::SIGSTOP);
        }
        let me = me();
        // Direct children already examined in this hunt, by (pid, start).
        let mut tops_seen: BTreeMap<u32, u64> = BTreeMap::new();
        let mut last_reason = String::from("no round completed");
        for round in 0..self.s.max_rounds {
            if self.out_of_time() {
                return Err((
                    format!("reforking: hunt bound reached after {round} rounds ({last_reason})"),
                    known,
                ));
            }
            let mut changed = false;
            // procfix7 round 10: why this round changed (diagnostics: the reason of a bound hit)
            let mut why_changed: Option<String> = None;
            let mut inconclusive: Option<String> = None;
            let mut queue: Vec<(u32, Option<u32>)> = Vec::new();
            match self.s.enumeration {
                Enumeration::SubreaperTree => match self.snapshot_tops(false) {
                    Ok(t) => {
                        // procfix7 round 10 (gate13c G13c-M1, F1): a listed child that ended before its read is a
                        // CHANGE (its successor may have been born after the listing), never "absent"
                        let ended = self.handoff_suspects(&t);
                        if !ended.is_empty() {
                            changed = true;
                            why_changed = Some(format!(
                                "children {ended:?} ended between the listing and the read (hand-off?)"
                            ));
                        }
                        for (pid, start) in t.live {
                            if tops_seen.get(&pid) != Some(&start) {
                                tops_seen.insert(pid, start);
                                queue.push((pid, Some(me)));
                            }
                        }
                    }
                    Err(e) => inconclusive = Some(e),
                },
                Enumeration::FullProc => match strict_listing_counted() {
                    Ok((all, _)) => queue.extend(
                        all.into_iter()
                            .filter(|st| st.start_ticks >= self.s.launch_ticks)
                            .map(|st| (st.pid, None)),
                    ),
                    Err(e) => inconclusive = Some(format!("proc listing: {e}")),
                },
            }
            // The belt (procfix3): every live process of our uid carrying J's exact marker, wherever it is.
            if round == 0 {
                match scan_marked_inline(
                    &self.s.marker.campaign_id,
                    self.s.launch_ticks,
                    Some(&self.s.marker.attempt_id),
                ) {
                    Ok(scan) => queue.extend(scan.processes.into_iter().map(|p| (p.pid, None))),
                    Err(e) => inconclusive = Some(format!("marker scan: {e}")),
                }
            }
            // Children of every stopped member (their lists are stable: a stopped process cannot fork).
            let member_pids: Vec<u32> = known.keys().copied().collect();
            for m in member_pids {
                match children_of(m) {
                    Ok(k) => queue.extend(k.into_iter().map(|p| (p, Some(m)))),
                    Err(ProcError::Gone) => {}
                    Err(e) => inconclusive = Some(format!("children of member {m}: {e}")),
                }
            }
            let mut seen: BTreeSet<u32> = BTreeSet::new();
            trace(|| {
                format!(
                    "sid {} round {round} queue {queue:?} known {:?}",
                    self.s.sid,
                    known.keys().collect::<Vec<_>>()
                )
            });
            // pop() takes the highest pid first: the newest child (a re-forking process's current member).
            queue.sort_unstable();
            while let Some((pid, parent)) = queue.pop() {
                if known.contains_key(&pid) || !seen.insert(pid) {
                    continue;
                }
                let (class, fd) = self.classify(pid, parent);
                self.trace_class(pid, parent, &class);
                match class {
                    Class::Ours(proof, facts) => {
                        let Some(fd) = fd else { continue };
                        let (st, hold) = self.stop(&fd, &facts, &proof);
                        match st {
                            Stop::Stopped => {
                                trace(|| {
                                    format!(
                                        "sid {} stopped {pid}; children {:?}",
                                        self.s.sid,
                                        children_of(pid)
                                    )
                                });
                                changed = true;
                                if self.s.enumeration == Enumeration::SubreaperTree {
                                    match children_of(pid) {
                                        Ok(k) => {
                                            queue.extend(k.into_iter().map(|c| (c, Some(pid))))
                                        }
                                        Err(ProcError::Gone) => {}
                                        Err(e) => {
                                            inconclusive =
                                                Some(format!("children of member {pid}: {e}"))
                                        }
                                    }
                                }
                                known.insert(
                                    pid,
                                    Member {
                                        fd,
                                        facts,
                                        proof,
                                        hold,
                                    },
                                );
                            }
                            Stop::DeliveredUnconfirmed(e) => {
                                // gate-A m-2: the STOP was delivered; keep it so that it gets the SIGKILL.
                                changed = true;
                                known.insert(
                                    pid,
                                    Member {
                                        fd,
                                        facts,
                                        proof,
                                        hold,
                                    },
                                );
                                inconclusive = Some(e);
                            }
                            Stop::Exited => changed = true,
                            Stop::Failed(e) => inconclusive = Some(e),
                        }
                    }
                    Class::NotOurs {
                        unattributed,
                        descend,
                        facts,
                    } => {
                        if unattributed && let Some(f) = &facts {
                            self.note_unattributed(f);
                        }
                        // gate-A M-1: a marker-less process of the job does not shield its descendants.
                        if descend && self.s.enumeration == Enumeration::SubreaperTree {
                            match children_of(pid) {
                                Ok(k) => queue.extend(k.into_iter().map(|c| (c, Some(pid)))),
                                Err(ProcError::Gone) => {}
                                Err(e) => inconclusive = Some(format!("children of {pid}: {e}")),
                            }
                        }
                    }
                    // procfix7 round 10 (gate13c G13c-M1, F1): a candidate found DEAD (a child of ours, or of a stopped
                    // member) may have handed off to a child born after it was listed: a CHANGE, never "absent" (a member
                    // this hunt stopped and killed cannot have handed off)
                    Class::Dead => {
                        if !self.killed_member(pid, None) {
                            changed = true;
                            why_changed = Some(format!(
                                "candidate {pid} ended before its examination (hand-off?)"
                            ));
                        }
                    }
                    Class::Inconclusive(e) => {
                        // Examine it again next round.
                        if parent == Some(me) {
                            tops_seen.remove(&pid);
                        }
                        inconclusive = Some(e);
                    }
                }
            }
            // Every member still stopped? (a member could be continued by a SIGCONT sent before it was found.)
            let pids: Vec<u32> = known.keys().copied().collect();
            for pid in pids {
                let m = &known[&pid];
                if guard::pidfd_exited(&m.fd) == Some(true) {
                    known.remove(&pid);
                    changed = true;
                    continue;
                }
                match read_proc_facts(pid, false) {
                    FactRead::Facts(f) if f.all_stopped || f.dead => {}
                    FactRead::Facts(_) => {
                        changed = true;
                        let (fd, facts, proof) = (&m.fd, m.facts.clone(), m.proof.clone());
                        if let (Stop::Failed(e) | Stop::DeliveredUnconfirmed(e), _) =
                            self.stop(fd, &facts, &proof)
                        {
                            inconclusive = Some(e);
                        }
                    }
                    FactRead::Gone => {
                        known.remove(&pid);
                        changed = true;
                    }
                    FactRead::Failed(e) => inconclusive = Some(e),
                }
            }
            request_reap();
            if let Some(e) = inconclusive {
                last_reason = e;
                std::thread::sleep(Duration::from_millis(2));
                continue;
            }
            if changed {
                last_reason = why_changed.unwrap_or_else(|| "new members in the last round".into());
                continue;
            }
            // Stability (procfix7 round 10, F1: on the RAW listing): no child of ours appeared since the round's
            // snapshot, and none of the listed ones ended before its read.
            if self.s.enumeration == Enumeration::SubreaperTree {
                match self.snapshot_tops(false) {
                    Ok(t)
                        if self.handoff_suspects(&t).is_empty()
                            && t.live.iter().all(|(p, st)| tops_seen.get(p) == Some(st)) => {}
                    Ok(_) => {
                        last_reason =
                            "children appeared or ended during the round (hand-off?)".into();
                        continue;
                    }
                    Err(e) => {
                        last_reason = e;
                        continue;
                    }
                }
            }
            return Ok(known);
        }
        Err((
            format!(
                "reforking: no fixed point within {} rounds ({last_reason})",
                self.s.max_rounds
            ),
            known,
        ))
    }

    /// procfix7 round 6 (gate10 M10-2: an unproven end re-ran the hunt every few seconds and every round sent the pinned
    /// group SIGSTOP + SIGKILL again, each a durable intent and a ledger record: ~97,000 records in 10 min). A SIGKILL
    /// delivered to the pinned group (leader held UNREAPED: the session cannot be reused) reached every member of that
    /// moment, and no fork completes with a fatal signal pending: later rounds of this job skip the GROUP signals; a live
    /// member that is still found is stopped and killed by its own pidfd as before. Keyed by (session, launch time).
    fn group_killed(&self) -> bool {
        GROUP_KILLED
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .contains(&(self.s.sid, self.s.launch_ticks))
    }

    /// Step 3: SIGKILL every stopped member (and the pinned group, and the cgroup); wait for the exits (bounded).
    fn kill_all(&mut self, known: BTreeMap<u32, Member>) -> Vec<LeftoverRecord> {
        if self.s.pinned
            && !self.group_killed()
            && guard::signal_pinned_group(&self.s.leaders, self.s.sid, libc::SIGKILL).delivered()
        {
            // procfix7 round 6 (gate10 M10-2): delivered once per pinned group (the proof passes decide what is left)
            GROUP_KILLED
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .insert((self.s.sid, self.s.launch_ticks));
        }
        if let Some(cg) = &self.s.cgroup {
            #[allow(clippy::let_underscore_must_use)]
            // ignored-ok (procfix7): the proof passes decide; a survivor sends the hunt back (never a proof by itself)
            let _ = super::cgroup::kill_path(cg);
        }
        let mut pending: Vec<Member> = Vec::new();
        for (_, m) in known {
            let t = self.target(&m.facts, m.proof.clone());
            let v = guard::signal_with_pidfd(&m.fd, &t, libc::SIGKILL, Some(&self.s.leaders));
            trace(|| format!("sid {} SIGKILL pid {}: {v:?}", self.s.sid, m.facts.stat.pid));
            match v {
                Verdict::Delivered | Verdict::Gone => {
                    self.killed.push(record(&m.facts));
                    pending.push(m);
                }
                // procfix6 (gate5 m1): a member this hunt STOPPED that the proof check refuses now (it dropped the marker
                // between the read and the stop) is killed by the identity held at its stop, else continued; if neither
                // can be sent it stays in `left` below (NotProven, reported). Never left stopped silently.
                _ if m.hold.is_some() && guard::pidfd_exited(&m.fd) != Some(true) => {
                    let h = m.hold.as_ref().expect("checked");
                    let kv = guard::kill_held(&[(&m.fd, h)])
                        .pop()
                        .unwrap_or(Verdict::Unverified("no verdict".into()));
                    trace(|| format!("sid {} held SIGKILL pid {}: {kv:?}", self.s.sid, h.pid()));
                    match kv {
                        Verdict::Delivered | Verdict::Gone => self.killed.push(record(&m.facts)),
                        _ => {
                            let cv = guard::continue_held(&[(&m.fd, h)]);
                            trace(|| {
                                format!("sid {} held continue pid {}: {cv:?}", self.s.sid, h.pid())
                            });
                        }
                    }
                    pending.push(m);
                }
                // The pinned-group SIGKILL may have killed it already (then the pidfd says so below).
                _ => pending.push(m),
            }
        }
        let t0 = Instant::now();
        let mut left = Vec::new();
        for m in pending {
            loop {
                if guard::pidfd_exited(&m.fd) == Some(true) {
                    break;
                }
                if t0.elapsed() > Duration::from_secs(2) {
                    left.push(record(&m.facts));
                    break;
                }
                std::thread::sleep(Duration::from_micros(500));
            }
        }
        // The scope belt (procfix3): AFTER the guarded kills, whatever the job's own systemd scope still holds (e.g. a
        // descendant that cleared the marker and left the session) is killed by the kernel through `cgroup.kill` of
        // that scope (recorded from the pinned leader; checked again: same directory, our uid, our user manager).
        // procfix7 round 6 (gate10 m10-3): the belt runs unless the scope is PROVEN empty (members() Ok and empty, which
        // now includes the kernel's recursive populated 0): an unreadable scope is killed too (cgroup.kill is recursive)
        let scope_live = self.s.scope.as_ref().map(|s| s.live_members());
        if let (Some(scope), Some(live)) = (&self.s.scope, scope_live)
            && !matches!(&live, Ok(v) if v.is_empty())
        {
            let live = live.unwrap_or_default();
            for pid in &live {
                // a record of what the scope kill takes (diagnostics); an unreadable stat is recorded without details
                match crate::resources::read_stat(*pid) {
                    Ok(st) => self.killed.push(LeftoverRecord {
                        pid: *pid,
                        start_ticks: st.start_ticks,
                        pgrp: st.pgrp,
                        comm: format!("{} <killed through the job's systemd scope>", st.comm),
                    }),
                    Err(ProcError::Gone) => {}
                    Err(ProcError::Unreadable(_)) => self.killed.push(LeftoverRecord {
                        pid: *pid,
                        start_ticks: 0,
                        pgrp: 0,
                        comm: "<stat unreadable, killed through the job's systemd scope>".into(),
                    }),
                }
            }
            let r = scope.kill_members();
            trace(|| format!("sid {} scope kill: {r:?}", self.s.sid));
        }
        request_reap();
        left
    }

    /// One proof pass. Ok(survivors) when conclusive; Err(reason) when not. In tree mode the pass is bracketed by two
    /// snapshots of our children: a child that appears in between and is ours (or cannot be classified) makes the pass
    /// inconclusive (a candidate that died while the pass ran may have handed off to it).
    fn pass(&mut self) -> Result<Vec<LeftoverRecord>, String> {
        let me = me();
        let mut survivors = Vec::new();
        self.classified_now.clear();
        if self.s.pinned && !self.leader_pinned() {
            return Err("the leader is no longer held unreaped".into());
        }
        match self.s.enumeration {
            Enumeration::SubreaperTree => {
                // procfix7 round 10 (gate13c G13c-M1, F2): both bracketing snapshots are RAW (children lists AND the
                // strict ppid scan); a pass is INCONCLUSIVE when either shows a child that ended before its read, when a
                // listed child ends before its examination, when a child appears and has ended by its examination, or
                // when a marker-less child whose subtree was walked is gone at the end: each may have handed off to a
                // successor born after a listing.
                let t0 = self.snapshot_tops(true)?;
                let ended = self.handoff_suspects(&t0);
                if !ended.is_empty() {
                    return Err(format!(
                        "children {ended:?} ended between the listing and the read (hand-off?)"
                    ));
                }
                let mut tops: Vec<u32> = t0.live.keys().copied().collect();
                tops.sort_unstable_by(|a, b| b.cmp(a));
                // Marker-less processes of the job whose subtrees are walked (gate-A M-1).
                let mut through: Vec<u32> = Vec::new();
                for pid in tops {
                    let (class, _) = self.classify(pid, Some(me));
                    match class {
                        Class::Ours(_, f) => survivors.push(record(&f)),
                        Class::Inconclusive(e) => return Err(e),
                        Class::NotOurs {
                            unattributed,
                            descend,
                            facts,
                        } => {
                            if unattributed && let Some(f) = &facts {
                                self.note_unattributed(f);
                            }
                            if descend {
                                through.push(pid);
                            }
                        }
                        Class::Dead => {
                            if !self.killed_member(pid, t0.live.get(&pid).copied()) {
                                return Err(format!(
                                    "child {pid} ended before its examination (hand-off?)"
                                ));
                            }
                        }
                    }
                }
                self.walk_through(through.clone(), &mut survivors)?;
                let t1 = self.snapshot_tops(true)?;
                let ended = self.handoff_suspects(&t1);
                if !ended.is_empty() {
                    return Err(format!(
                        "children {ended:?} ended during the pass (hand-off?)"
                    ));
                }
                for pid in &through {
                    if t1.live.get(pid) != t0.live.get(pid) {
                        return Err(format!(
                            "marker-less child {pid} ended during the pass (hand-off?)"
                        ));
                    }
                }
                for (pid, start) in t1.live {
                    if t0.live.get(&pid) == Some(&start) {
                        continue;
                    }
                    match self.classify(pid, Some(me)).0 {
                        Class::Ours(_, f) => survivors.push(record(&f)),
                        Class::Inconclusive(e) => {
                            return Err(format!("child appeared during the pass: {e}"));
                        }
                        Class::NotOurs { descend: true, .. } => {
                            self.walk_through(vec![pid], &mut survivors)?
                        }
                        Class::NotOurs { .. } => {}
                        Class::Dead => {
                            return Err(format!(
                                "child {pid} appeared and ended during the pass (hand-off?)"
                            ));
                        }
                    }
                }
            }
            Enumeration::FullProc => {
                let (all, vanished) =
                    strict_listing_counted().map_err(|e| format!("proc listing: {e}"))?;
                if vanished > 0 {
                    return Err(format!("{vanished} proc entries vanished during the pass"));
                }
                for st in all {
                    if st.start_ticks < self.s.launch_ticks || st.pid == me || st.pid <= 1 {
                        continue;
                    }
                    match self.classify(st.pid, None) {
                        (Class::Ours(_, f), _) => survivors.push(record(&f)),
                        (Class::Inconclusive(e), _) => return Err(e),
                        _ => {}
                    }
                }
            }
        }
        // procfix8 (item 4): the suspects of this hunt the enumeration above did not reach
        self.recheck_suspects(&mut survivors)?;
        // Belt 1 (procfix3): any live process of our uid with J's EXACT marker is a survivor, wherever it is. Only adds
        // survivors (an unreadable environ elsewhere on the host does not make the pass inconclusive: the strict tree
        // enumeration above is the proof; this belt catches what a tree walk cannot see).
        let scan = scan_marked_inline(
            &self.s.marker.campaign_id,
            self.s.launch_ticks,
            Some(&self.s.marker.attempt_id),
        )
        .map_err(|e| format!("marker scan: {e}"))?;
        for p in scan.processes {
            if p.pid == self.s.sid || survivors.iter().any(|s| s.pid == p.pid) {
                continue;
            }
            survivors.push(LeftoverRecord {
                pid: p.pid,
                start_ticks: p.start_ticks,
                pgrp: p.pgrp,
                comm: format!("{} <marked, found by the full scan>", p.comm),
            });
        }
        // Belt 2 (procfix3): the job's systemd scope (kernel-maintained membership).
        if let Some(scope) = &self.s.scope {
            for pid in scope.live_members()? {
                if !survivors.iter().any(|s| s.pid == pid) {
                    survivors.push(LeftoverRecord {
                        pid,
                        start_ticks: 0,
                        pgrp: 0,
                        comm: "<in the job's systemd scope>".into(),
                    });
                }
            }
        }
        if let Some(cg) = &self.s.cgroup {
            // procfix7 round 6 (gate10 m10-3): the job cgroup's subtree, cross-checked with the recursive populated flag
            match super::cgroup::procs_recursive(cg) {
                Ok(p) if p.is_empty() => match super::cgroup::populated(cg) {
                    Ok(false) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Ok(true) => {
                        self.cgroup_ok = false;
                        return Err(format!(
                            "{}: populated (cgroup.events) but no member listed in the subtree's cgroup.procs",
                            cg.display()
                        ));
                    }
                    Err(e) => {
                        self.cgroup_ok = false;
                        return Err(format!("cgroup.events: {e}"));
                    }
                },
                Ok(p) => {
                    for pid in p {
                        survivors.push(LeftoverRecord {
                            pid,
                            start_ticks: 0,
                            pgrp: 0,
                            comm: "<in the job cgroup>".into(),
                        });
                    }
                }
                Err(e) => {
                    self.cgroup_ok = false;
                    return Err(format!("cgroup.procs: {e}"));
                }
            }
        }
        if self.s.pinned && !self.leader_pinned() {
            return Err("the leader is no longer held unreaped".into());
        }
        Ok(survivors)
    }

    /// Classify the descendants of marker-less processes of the job (not stopped, never signalled: their children lists
    /// are read as they are; anything handed off meanwhile is re-parented to us and seen by the bracketing snapshots, or
    /// carries the marker and is seen by the full-scan belt). Bounded.
    fn walk_through(
        &mut self,
        start: Vec<u32>,
        survivors: &mut Vec<LeftoverRecord>,
    ) -> Result<(), String> {
        let mut stack: Vec<u32> = start;
        let mut visited: BTreeSet<u32> = BTreeSet::new();
        while let Some(n) = stack.pop() {
            if !visited.insert(n) {
                continue;
            }
            if visited.len() > 65_536 {
                return Err("more than 65,536 processes below marker-less job processes".into());
            }
            let kids = match children_of(n) {
                Ok(k) => k,
                Err(ProcError::Gone) => continue,
                Err(e) => return Err(format!("children of {n}: {e}")),
            };
            for c in kids {
                match self.classify(c, Some(n)).0 {
                    Class::Ours(_, f) => survivors.push(record(&f)),
                    Class::Inconclusive(e) => return Err(e),
                    Class::NotOurs { descend: true, .. } => stack.push(c),
                    _ => {}
                }
            }
        }
        Ok(())
    }

    fn leader_pinned(&self) -> bool {
        self.s.leaders.lock().contains(&self.s.sid) && super::real::peek_exit(self.s.sid).is_ok()
    }

    fn two_passes(&mut self) -> Result<Vec<LeftoverRecord>, String> {
        let a = self.pass();
        trace(|| format!("sid {} pass 1: {a:?}", self.s.sid));
        let a = a?;
        if !a.is_empty() {
            return Ok(a);
        }
        std::thread::sleep(self.s.pass_gap);
        self.pass()
    }

    fn by(&self) -> VerifiedBy {
        if self.s.cgroup.is_some() && self.cgroup_ok {
            VerifiedBy::Cgroup
        } else {
            VerifiedBy::ProcTwoPass {
                enumeration: self.s.enumeration,
            }
        }
    }
}

/// Run one hunt invocation (blocking; call on a helper thread).
pub(crate) fn run(spec: &HuntSpec, cancel: &AtomicBool) -> HuntResult {
    // gate3 M2: every delivery of the hunt is logged (and recorded by the supervisor) with this context.
    guard::with_context("end-of-job hunt", Some(&spec.marker.attempt_id), || {
        run_inner(spec, cancel)
    })
}

fn run_inner(spec: &HuntSpec, cancel: &AtomicBool) -> HuntResult {
    let mut h = Hunt {
        s: spec,
        cancel,
        deadline: Instant::now() + spec.budget,
        own_session: own_session(),
        uid: our_uid(),
        not_ours: HashMap::new(),
        unattributed: BTreeMap::new(),
        killed: Vec::new(),
        cgroup_ok: spec.cgroup.is_some(),
        suspects: BTreeMap::new(),
        classified_now: BTreeSet::new(),
    };
    if !spec.kill {
        return match h.two_passes() {
            Ok(left) if left.is_empty() => HuntResult::Proven {
                by: h.by(),
                killed: Vec::new(),
                unattributed: h.unattributed.into_values().collect(),
            },
            Ok(left) => HuntResult::Survivors { left },
            Err(reason) => HuntResult::NotProven {
                reason,
                left: Vec::new(),
                killed: Vec::new(),
                unattributed: h.unattributed.into_values().collect(),
            },
        };
    }
    if let Some(cg) = &spec.cgroup
        && super::cgroup::freeze_and_kill_path(cg, Duration::from_secs(1)).is_err()
    {
        h.cgroup_ok = false;
    }
    let mut reason;
    let mut left: Vec<LeftoverRecord>;
    let mut backoff = Duration::from_millis(5);
    loop {
        let known = match h.freeze_fixed_point() {
            Ok(k) => {
                reason = String::new();
                k
            }
            Err((r, k)) => {
                reason = r;
                k
            }
        };
        let fixed = reason.is_empty();
        left = h.kill_all(known);
        if fixed && left.is_empty() {
            match h.two_passes() {
                Ok(s) if s.is_empty() => {
                    let by = h.by();
                    return HuntResult::Proven {
                        by,
                        killed: h.killed,
                        unattributed: h.unattributed.into_values().collect(),
                    };
                }
                Ok(s) => {
                    reason = format!("{} survivor(s) in a proof pass", s.len());
                    left = s;
                }
                Err(e) => reason = format!("proof pass inconclusive: {e}"),
            }
        } else if reason.is_empty() {
            reason = format!("{} member(s) did not exit after SIGKILL", left.len());
        }
        if h.out_of_time() {
            break;
        }
        // procfix7 round 10: back-off between inconclusive hunts (5 ms doubling to 250 ms), bounded by the budget: a
        // proof that stays inconclusive ends NotProven (typed: unverified / unproven end), never Verified, never a hang
        std::thread::sleep(backoff);
        backoff = (backoff * 2).min(Duration::from_millis(250));
    }
    HuntResult::NotProven {
        reason,
        left,
        killed: h.killed,
        unattributed: h.unattributed.into_values().collect(),
    }
}
