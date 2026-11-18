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
    /// Ended. Some((start, session)) when its facts were read (a zombie, or found exited after the reads: the caller
    /// checks the start against its listing before it uses them); None: gone before its facts were read.
    Dead(Option<(u64, u32)>),
    Inconclusive(String),
}

/// The start time a classification read (None: no facts).
fn class_start(c: &Class) -> Option<u64> {
    match c {
        Class::Ours(_, f) => Some(f.stat.start_ticks),
        Class::NotOurs { facts, .. } => facts.as_ref().map(|f| f.stat.start_ticks),
        Class::Dead(_) | Class::Inconclusive(_) => None,
    }
}

/// A candidate of the fixed point: (pid, the process whose children list named it, its start time as listed).
type Cand = (u32, Option<u32>, u64);

struct Member {
    fd: OwnedFd,
    facts: ProcFacts,
    proof: Proof,
    /// procfix6 (gate5 m1): the identity held at the stop this hunt delivered.
    hold: Option<guard::StopHold>,
}

/// procfix7 round 10 (gate13c G13c-M1): a RAW children listing; round 11 (gate14 G14-M1): of ANY process the hunt
/// follows, our own ([`Hunt::snapshot_tops`]), a stopped member's or a walked process's ([`Hunt::snapshot_children`]).
#[derive(Default, Clone, Debug, PartialEq, Eq)]
struct Listing {
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
    /// procfix7 round 11 (gate14 G14-M1): processes (pid, start) this hunt invocation already saw ENDED in a listing. A
    /// process hands its children over AT its exit (the kernel re-parents them before it becomes a zombie), so every
    /// listing read after that first sighting already shows its successors: a zombie still listed later (its parent has
    /// not reaped it) is no new hand-off suspect. A gone entry has no start time and never enters this set.
    seen_ended: BTreeSet<(u32, u64)>,
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
            Class::NotOurs { .. } | Class::Dead(_) => {}
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
                Class::NotOurs { .. } | Class::Dead(_) => {
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
            Err(Verdict::Gone) => return (Class::Dead(None), None),
            Err(Verdict::NotOurs(_)) => return (not_ours(false, None), None),
            Err(Verdict::Unverified(e)) => return (Class::Inconclusive(e), None),
            Err(Verdict::Delivered) => return (Class::Inconclusive("impossible".into()), None),
        };
        let facts = match read_proc_facts(pid, true) {
            FactRead::Gone => return (Class::Dead(None), None),
            FactRead::Failed(e) => return (Class::Inconclusive(e), None),
            FactRead::Facts(f) => f,
        };
        let ended = Some((facts.stat.start_ticks, facts.stat.session));
        match guard::pidfd_exited(&fd) {
            Some(true) => return (Class::Dead(ended), None),
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
            return (Class::Dead(ended), None);
        }
        let key = (pid, facts.stat.start_ticks);
        if let Some(&descend) = self.not_ours.get(&key) {
            // round 11: with its facts (the caller compares the start time with its listing)
            return (not_ours(descend, Some(facts)), None);
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
    /// strict full /proc listing (ppid == us), with what each entry was when its stat was read: alive, or ENDED (a zombie,
    /// or gone at the read). An ended entry may have HANDED OFF to a child born after the listing (a re-forking job), so
    /// it is never "absent": [`Self::handoff_suspects`]. Strict: a failed enumeration or stat read is an error.
    fn snapshot_tops(&self, with_scan: bool) -> Result<Listing, String> {
        let mut kids = own_children().map_err(|e| format!("own children: {e}"))?;
        crate::resources::listing_hold();
        if with_scan {
            kids.extend(own_children_by_scan().map_err(|e| format!("ppid scan: {e}"))?);
        }
        self.read_entries(kids)
    }

    /// procfix7 round 11 (gate14 G14-M1, W1/W2): the RAW children listing of `parent` (a stopped member or a walked
    /// process, born at `start`), as [`Self::snapshot_tops`] reads ours. Ok(None): `parent` ended or the pid names
    /// another process now (its children were re-parented: the caller treats that as a change). The list is `parent`'s
    /// only if `parent` is still the same live process AFTER the reads (a pid is never reused while its process exists).
    fn snapshot_children(&self, parent: u32, start: u64) -> Result<Option<Listing>, String> {
        let kids = match children_of(parent) {
            Ok(k) => k,
            Err(ProcError::Gone) => return Ok(None),
            Err(ProcError::Unreadable(e)) => return Err(format!("children of {parent}: {e}")),
        };
        crate::resources::listing_hold();
        let l = self.read_entries(kids)?;
        match self.alive_as(parent, start)? {
            true => Ok(Some(l)),
            false => Ok(None),
        }
    }

    /// Is `pid` still the live process born at `start`? (a zombie main thread with live threads is alive.)
    fn alive_as(&self, pid: u32, start: u64) -> Result<bool, String> {
        match crate::resources::read_stat(pid) {
            Ok(st) if st.start_ticks != start => Ok(false),
            Ok(st) if !st.is_dead() => Ok(true),
            Ok(_) => match read_proc_facts(pid, false) {
                FactRead::Facts(f) => Ok(!f.dead && f.stat.start_ticks == start),
                FactRead::Gone => Ok(false),
                FactRead::Failed(e) => Err(format!("facts of {pid}: {e}")),
            },
            Err(ProcError::Gone) => Ok(false),
            Err(ProcError::Unreadable(e)) => Err(format!("stat of {pid}: {e}")),
        }
    }

    /// The entries of a children list with what each was at its stat read. Round 11: an entry whose main thread is a
    /// zombie but which still has a live thread is ALIVE (it can fork): only a zombie without live threads is ended.
    fn read_entries(&self, kids: BTreeSet<u32>) -> Result<Listing, String> {
        let reg: BTreeSet<u32> = self.s.leaders.lock().clone();
        let mut out = Listing::default();
        for pid in kids {
            if reg.contains(&pid) {
                continue;
            }
            match crate::resources::read_stat(pid) {
                Ok(st) if st.is_dead() => match read_proc_facts(pid, false) {
                    FactRead::Facts(f) if !f.dead => {
                        out.live.insert(pid, st.start_ticks);
                    }
                    FactRead::Facts(_) | FactRead::Gone => {
                        out.ended.push((pid, Some((st.start_ticks, st.session))))
                    }
                    FactRead::Failed(e) => return Err(format!("facts of child {pid}: {e}")),
                },
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
    /// it died: it cannot have handed off.) Round 11 (gate14 m14-pid-alone): by pid AND start time only.
    fn killed_member(&self, pid: u32, start: u64) -> bool {
        self.killed
            .iter()
            .any(|k| k.pid == pid && k.start_ticks == start)
    }

    /// procfix7 round 10 (gate13c G13c-M1): the ENDED entries of a raw listing that may have handed off to a successor
    /// born after the listing ([`Self::ended_suspect`]; records each in `seen_ended`).
    /// Round 11: the other leaders' sessions of the listing are read ONCE under the registry lock.
    fn handoff_suspects(&mut self, l: &Listing) -> Vec<u32> {
        if l.ended.is_empty() {
            return Vec::new();
        }
        let leaders =
            self.other_leader_starts(l.ended.iter().filter_map(|(_, i)| i.map(|(_, s)| s)));
        let mut out = Vec::new();
        for &(pid, info) in &l.ended {
            if self.ended_suspect_with(
                pid,
                info.map(|(s, _)| s),
                info.map(|(_, sess)| sess),
                &leaders,
            ) {
                out.push(pid);
            }
        }
        out
    }

    /// A candidate listed alive as (pid, `listed`) and classified Dead: a hand-off suspect? The facts' session is used
    /// only when their start time is the listed one (the same process).
    fn dead_suspect(&mut self, pid: u32, listed: u64, info: Option<(u64, u32)>) -> bool {
        let session = info.filter(|(s, _)| *s == listed).map(|(_, sess)| sess);
        self.ended_suspect(pid, Some(listed), session)
    }

    /// procfix7 round 10 / 11: may the ENDED process (pid, `start`, `session`) have handed its children to a successor
    /// that a listing read before its end could not show? NOT a suspect (each by a conclusive fact):
    /// - a member this hunt stopped and killed, by pid AND start (stopped before it died: no fork after the stop);
    /// - (round 11) a process this hunt already saw ended in an earlier listing (`seen_ended`: its hand-off, if any,
    ///   happened before that sighting, and every later listing shows the successors);
    /// - a process started before J's launch, or (round 11) one this hunt classified conclusively as not J's and not to
    ///   be walked (another attempt's marker, another uid, ...): its successors are not J's either;
    /// - a zombie of the supervisor's OWN session (its helpers; every job leader setsid's);
    /// - (round 11, gate14 m14-liveness) a zombie of ANOTHER job's session whose leader is held unreaped and was born no
    ///   later than it ([`Self::other_leader_starts`]): it descends from that leader, not from J.
    ///
    /// Everything else is a suspect: a gone entry (no identity), a zombie of J's session, of a session that is no
    /// registered leader's. A suspect is recorded in `seen_ended` (the caller makes the round a change / the pass
    /// inconclusive; later listings show its successors).
    fn ended_suspect(&mut self, pid: u32, start: Option<u64>, session: Option<u32>) -> bool {
        let leaders = self.other_leader_starts(session.into_iter());
        self.ended_suspect_with(pid, start, session, &leaders)
    }

    /// [`Self::ended_suspect`] with the other leaders' sessions already read ([`Self::other_leader_starts`]).
    fn ended_suspect_with(
        &mut self,
        pid: u32,
        start: Option<u64>,
        session: Option<u32>,
        leaders: &BTreeMap<u32, u64>,
    ) -> bool {
        let Some(start) = start else { return true };
        if self.killed_member(pid, start)
            || self.seen_ended.contains(&(pid, start))
            || start < self.s.launch_ticks
            || self.not_ours.get(&(pid, start)) == Some(&false)
        {
            return false;
        }
        if let Some(sess) = session
            && (sess == self.own_session || leaders.get(&sess).is_some_and(|&l| l <= start))
        {
            return false;
        }
        self.seen_ended.insert((pid, start));
        true
    }

    /// procfix7 round 11 (gate14 m14-liveness): read under ONE registry lock (a registered leader is reaped only under it,
    /// so each stat read under it is that leader's): for every given session other than J's and ours that is a
    /// registered leader's (our child, leading its own session), that leader's start time. A zombie of such a session
    /// born NO EARLIER than that leader is exempt ([`Self::ended_suspect`]): it was born while the leader held the number
    /// (a number in use as a session id is never given to a new process, so every session with that id during the
    /// leader's life is the leader's own) and inherited the session from a member of it: it descends from that leader,
    /// never from J (J's leader is the supervisor's child, as is every leader).
    fn other_leader_starts(&self, sessions: impl Iterator<Item = u32>) -> BTreeMap<u32, u64> {
        let want: BTreeSet<u32> = sessions
            .filter(|&s| s != self.s.sid && s != self.own_session)
            .collect();
        let mut out = BTreeMap::new();
        if want.is_empty() {
            return out;
        }
        let reg = self.s.leaders.lock();
        for s in want {
            if reg.contains(&s)
                && let Ok(st) = crate::resources::read_stat(s)
                && st.ppid == me()
                && st.session == s
            {
                out.insert(s, st.start_ticks);
            }
        }
        out
    }

    /// procfix7 round 11 (gate14 L14-1): our children list keeps changing (a fast relay re-parented to us): the round
    /// cannot conclude, but every live child that any read names is still a candidate to STOP (a stopped relay hop cannot
    /// fork, so the list settles). Candidates only: nothing is concluded from this union.
    /// Only candidates not examined yet (pid AND start): a list that keeps changing because OTHER jobs' orphans come and
    /// go costs one classification per new child, not one per child and round.
    fn unstable_top_candidates(&self, examined: &BTreeMap<u32, u64>, queue: &mut Vec<Cand>) {
        let me = me();
        let reg: BTreeSet<u32> = self.s.leaders.lock().clone();
        for pid in crate::resources::own_children_candidates() {
            if reg.contains(&pid) {
                continue;
            }
            if let Ok(st) = crate::resources::read_stat(pid)
                && !st.is_dead()
                && examined.get(&pid) != Some(&st.start_ticks)
            {
                queue.push((pid, Some(me), st.start_ticks));
            }
        }
    }

    /// procfix7 round 11 (gate14 G14-M1, W1): take one raw listing of `parent` into the round: an ended entry that may
    /// have handed off makes the round a CHANGE; every live entry not examined yet (by pid AND start) is queued.
    fn take_listing(
        &mut self,
        parent: u32,
        l: &Listing,
        examined: &BTreeMap<u32, u64>,
        queue: &mut Vec<Cand>,
        changed: &mut Option<String>,
    ) {
        let ended = self.handoff_suspects(l);
        if !ended.is_empty() {
            changed.get_or_insert(format!(
                "children {ended:?} of {parent} ended between the listing and the read (hand-off?)"
            ));
        }
        for (&pid, &start) in &l.live {
            if examined.get(&pid) != Some(&start) {
                queue.push((pid, Some(parent), start));
            }
        }
    }

    /// procfix7 round 11 (gate14 G14-M1, W1): the stability check that ends the fixed point. EVERY list the hunt follows
    /// is read again: ours, every stopped member's and every walked process's; each must still exist, show no new ended
    /// entry that may have handed off and no live entry that was not examined (by pid AND start). Ok(None): stable.
    ///
    /// Round 11 (liveness, gate14 m14-liveness): a live entry that appeared during the round is classified HERE; one that
    /// is conclusively NOT the job's and not walked (another job's orphan, another uid, ...) cannot hide a member (its
    /// descendants are not the job's either: the classification rules) and is taken as examined; anything else (the
    /// job's, walked, inconclusive, found dead, another start time) makes the round unstable as before.
    fn unstable_reason(
        &mut self,
        examined: &mut BTreeMap<u32, u64>,
        parents: &[(u32, u64)],
    ) -> Result<Option<String>, String> {
        let mut lists: Vec<(u32, Listing)> = Vec::new();
        if self.s.enumeration == Enumeration::SubreaperTree {
            lists.push((me(), self.snapshot_tops(false)?));
        }
        for &(p, ps) in parents {
            match self.snapshot_children(p, ps)? {
                Some(l) => lists.push((p, l)),
                None => return Ok(Some(format!("{p} ended during the round"))),
            }
        }
        for (p, l) in &lists {
            let ended = self.handoff_suspects(l);
            if !ended.is_empty() {
                return Ok(Some(format!(
                    "children {ended:?} of {p} ended during the round (hand-off?)"
                )));
            }
            for (&c, &cs) in &l.live {
                if examined.get(&c) == Some(&cs) {
                    continue;
                }
                let class = self.classify(c, Some(*p)).0;
                self.trace_class(c, Some(*p), &class);
                match class {
                    Class::NotOurs {
                        descend: false,
                        unattributed,
                        facts,
                    } if facts.as_ref().is_none_or(|f| f.stat.start_ticks == cs) => {
                        if unattributed && let Some(f) = &facts {
                            self.note_unattributed(f);
                        }
                        examined.insert(c, cs);
                    }
                    _ => return Ok(Some(format!("child {c} of {p} appeared during the round"))),
                }
            }
        }
        Ok(None)
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
                    Class::Dead(_) => "dead".into(),
                    Class::Inconclusive(e) => format!("INCONCLUSIVE {e}"),
                }
            )
        });
    }

    /// Step 2: stop every member until a fixed point. Ok(members) at the fixed point; Err(reason) when the bound is hit.
    ///
    /// Why a fixed point needs STABLE children lists (and not just "every candidate examined was dead or not ours"): a
    /// candidate alive when a children list was read but dead when examined may have handed off to a child born after
    /// that read (the relay of re-audit B); that child is re-parented, AT the candidate's exit, to the nearest living
    /// child subreaper above it: us, a stopped member, or (gate14 G14-M1) a WALKED marker-less process of the job that
    /// made itself a subreaper. So (round 11, W1) every round re-reads the RAW children lists of all three (ours, every
    /// stopped member's, every walked process's), an ended entry that may have handed off is a CHANGE at every level, a
    /// walked process that ended is a change (its children moved), and the round ends with the same lists read once
    /// more: any entry not examined yet (pid AND start) or newly ended sends the loop round again. The newest children
    /// are examined first.
    fn freeze_fixed_point(
        &mut self,
    ) -> Result<BTreeMap<u32, Member>, (String, BTreeMap<u32, Member>)> {
        let mut known: BTreeMap<u32, Member> = BTreeMap::new();
        if self.s.pinned && !self.group_killed() {
            #[allow(clippy::let_underscore_must_use)]
            // ignored-ok (procfix7): the fixed point and two proof passes decide; a failed stop only costs rounds
            let _ = guard::signal_pinned_group(&self.s.leaders, self.s.sid, libc::SIGSTOP);
        }
        let tree = self.s.enumeration == Enumeration::SubreaperTree;
        // Candidates examined conclusively in this freeze (alive when classified): pid -> start.
        let mut examined: BTreeMap<u32, u64> = BTreeMap::new();
        // round 11 (W1): the walked processes (marker-less processes of the job, never signalled, followed): pid -> start
        let mut walked: BTreeMap<u32, u64> = BTreeMap::new();
        let mut last_reason = String::from("no round completed");
        for round in 0..self.s.max_rounds {
            if self.out_of_time() {
                return Err((
                    format!("reforking: hunt bound reached after {round} rounds ({last_reason})"),
                    known,
                ));
            }
            // why this round changed (diagnostics: the reason of a bound hit)
            let mut changed: Option<String> = None;
            let mut inconclusive: Option<String> = None;
            let mut queue: Vec<Cand> = Vec::new();
            match self.s.enumeration {
                Enumeration::SubreaperTree => match self.snapshot_tops(false) {
                    Ok(t) => self.take_listing(me(), &t, &examined, &mut queue, &mut changed),
                    Err(e) => {
                        // round 11 (gate14 L14-1): nothing is concluded, but the listed children are still stopped
                        self.unstable_top_candidates(&examined, &mut queue);
                        inconclusive = Some(e);
                    }
                },
                Enumeration::FullProc => match strict_listing_counted() {
                    Ok((all, _)) => queue.extend(
                        all.into_iter()
                            .filter(|st| st.start_ticks >= self.s.launch_ticks)
                            .map(|st| (st.pid, None, st.start_ticks)),
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
                    Ok(scan) => queue.extend(
                        scan.processes
                            .into_iter()
                            .map(|p| (p.pid, None, p.start_ticks)),
                    ),
                    Err(e) => inconclusive = Some(format!("marker scan: {e}")),
                }
            }
            // Round 11 (W1): the raw children lists of every stopped member (a stopped process cannot fork, but a child
            // subreaper adopts) and of every WALKED process (not stopped: it forks and adopts at any time), every round.
            let parents: Vec<(u32, u64)> = known
                .iter()
                .map(|(p, m)| (*p, m.facts.stat.start_ticks))
                .chain(walked.iter().map(|(p, st)| (*p, *st)))
                .collect();
            for (p, ps) in parents {
                match self.snapshot_children(p, ps) {
                    Ok(Some(l)) => self.take_listing(p, &l, &examined, &mut queue, &mut changed),
                    Ok(None) => {
                        // a walked process that ended handed its children over; a member: the member check below
                        if walked.remove(&p).is_some() {
                            changed.get_or_insert(format!(
                                "walked process {p} ended (its children were re-parented)"
                            ));
                        }
                    }
                    Err(e) => inconclusive = Some(e),
                }
            }
            let mut seen: BTreeSet<u32> = BTreeSet::new();
            trace(|| {
                format!(
                    "sid {} round {round} queue {queue:?} known {:?} walked {walked:?}",
                    self.s.sid,
                    known.keys().collect::<Vec<_>>()
                )
            });
            // pop() takes the highest pid first: the newest child (a re-forking process's current member).
            queue.sort_unstable();
            while let Some((pid, parent, start)) = queue.pop() {
                if known.contains_key(&pid) || !seen.insert(pid) {
                    continue;
                }
                let (class, fd) = self.classify(pid, parent);
                self.trace_class(pid, parent, &class);
                // round 11: the listed process ended and the pid names another process now (examined below as itself)
                if class_start(&class).is_some_and(|now| now != start)
                    && self.ended_suspect(pid, Some(start), None)
                {
                    changed.get_or_insert(format!(
                        "candidate {pid} ended before its examination (hand-off?)"
                    ));
                }
                match class {
                    Class::Ours(proof, facts) => {
                        let Some(fd) = fd else { continue };
                        let fstart = facts.stat.start_ticks;
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
                                changed.get_or_insert(format!("member {pid} stopped"));
                                examined.insert(pid, fstart);
                                if tree {
                                    match self.snapshot_children(pid, fstart) {
                                        Ok(Some(l)) => self.take_listing(
                                            pid,
                                            &l,
                                            &examined,
                                            &mut queue,
                                            &mut changed,
                                        ),
                                        Ok(None) => {} // it ended: the member check below
                                        Err(e) => inconclusive = Some(e),
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
                                changed.get_or_insert(format!("member {pid} stop unconfirmed"));
                                examined.insert(pid, fstart);
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
                            Stop::Exited => {
                                changed.get_or_insert(format!(
                                    "candidate {pid} of the job exited before its stop (hand-off?)"
                                ));
                            }
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
                        let fstart = facts.as_ref().map_or(start, |f| f.stat.start_ticks);
                        examined.insert(pid, fstart);
                        // gate-A M-1: a marker-less process of the job does not shield its descendants; round 11 (W1): it
                        // is WALKED from now on (its list is re-read in every round)
                        if descend && tree && walked.insert(pid, fstart).is_none() {
                            match self.snapshot_children(pid, fstart) {
                                Ok(Some(l)) => {
                                    self.take_listing(pid, &l, &examined, &mut queue, &mut changed)
                                }
                                Ok(None) => {
                                    walked.remove(&pid);
                                    changed.get_or_insert(format!(
                                        "walked process {pid} ended (its children were re-parented)"
                                    ));
                                }
                                Err(e) => inconclusive = Some(e),
                            }
                        }
                    }
                    // procfix7 round 10 (F1) / round 11: a candidate found DEAD may have handed off to a child born after
                    // it was listed: a CHANGE unless a conclusive fact says otherwise ([`Self::ended_suspect`])
                    Class::Dead(info) => {
                        if self.dead_suspect(pid, start, info) {
                            changed.get_or_insert(format!(
                                "candidate {pid} ended before its examination (hand-off?)"
                            ));
                        }
                    }
                    // not examined: queued again next round
                    Class::Inconclusive(e) => inconclusive = Some(e),
                }
            }
            // Every member still stopped? (a member could be continued by a SIGCONT sent before it was found.)
            let pids: Vec<u32> = known.keys().copied().collect();
            for pid in pids {
                let m = &known[&pid];
                if guard::pidfd_exited(&m.fd) == Some(true) {
                    known.remove(&pid);
                    changed.get_or_insert(format!("member {pid} exited"));
                    continue;
                }
                match read_proc_facts(pid, false) {
                    FactRead::Facts(f) if f.all_stopped || f.dead => {}
                    FactRead::Facts(_) => {
                        changed.get_or_insert(format!("member {pid} was continued"));
                        let (fd, facts, proof) = (&m.fd, m.facts.clone(), m.proof.clone());
                        if let (Stop::Failed(e) | Stop::DeliveredUnconfirmed(e), _) =
                            self.stop(fd, &facts, &proof)
                        {
                            inconclusive = Some(e);
                        }
                    }
                    FactRead::Gone => {
                        known.remove(&pid);
                        changed.get_or_insert(format!("member {pid} exited"));
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
            if let Some(why) = changed {
                last_reason = why;
                continue;
            }
            // Stability (round 10 F1 on our list; round 11 W1 on EVERY list the hunt follows).
            let parents: Vec<(u32, u64)> = known
                .iter()
                .map(|(p, m)| (*p, m.facts.stat.start_ticks))
                .chain(walked.iter().map(|(p, st)| (*p, *st)))
                .collect();
            match self.unstable_reason(&mut examined, &parents) {
                Ok(None) => return Ok(known),
                Ok(Some(why)) => last_reason = why,
                Err(e) => last_reason = e,
            }
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
    /// inconclusive (a candidate that died while the pass ran may have handed off to it). Round 11 (gate14 G14-M1, W2):
    /// the same bracket for EVERY walked process: its raw children list is read when it is walked and again after the
    /// whole walk and the closing snapshot; the pass is inconclusive when a walked process ended, when its list changed,
    /// or when an entry of it ended (or a listed child is found dead) that may have handed off.
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
                // strict ppid scan); a pass is INCONCLUSIVE when either shows a child that ended before its read and may
                // have handed off, when a listed child ends before its examination, when a child appears and has ended
                // by its examination, or when a marker-less child whose subtree was walked is gone at the end.
                let t0 = self.snapshot_tops(true)?;
                let ended = self.handoff_suspects(&t0);
                if !ended.is_empty() {
                    return Err(format!(
                        "children {ended:?} ended between the listing and the read (hand-off?)"
                    ));
                }
                let mut tops: Vec<(u32, u64)> = t0.live.iter().map(|(p, st)| (*p, *st)).collect();
                tops.sort_unstable_by(|a, b| b.cmp(a));
                // Marker-less processes of the job whose subtrees are walked (gate-A M-1), with their listings (W2).
                let mut through: Vec<(u32, u64)> = Vec::new();
                let mut walked: BTreeMap<u32, (u64, Listing)> = BTreeMap::new();
                for (pid, start) in tops {
                    let (class, _) = self.classify(pid, Some(me));
                    self.listed_as(pid, start, &class)?;
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
                                through.push((pid, start));
                            }
                        }
                        Class::Dead(info) => {
                            if self.dead_suspect(pid, start, info) {
                                return Err(format!(
                                    "child {pid} ended before its examination (hand-off?)"
                                ));
                            }
                        }
                    }
                }
                self.walk_through(through.clone(), &mut survivors, &mut walked)?;
                let t1 = self.snapshot_tops(true)?;
                let ended = self.handoff_suspects(&t1);
                if !ended.is_empty() {
                    return Err(format!(
                        "children {ended:?} ended during the pass (hand-off?)"
                    ));
                }
                for (pid, _) in &through {
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
                    let class = self.classify(pid, Some(me)).0;
                    self.listed_as(pid, start, &class)?;
                    match class {
                        Class::Ours(_, f) => survivors.push(record(&f)),
                        Class::Inconclusive(e) => {
                            return Err(format!("child appeared during the pass: {e}"));
                        }
                        Class::NotOurs { descend: true, .. } => {
                            self.walk_through(vec![(pid, start)], &mut survivors, &mut walked)?
                        }
                        Class::NotOurs { .. } => {}
                        Class::Dead(info) => {
                            if self.dead_suspect(pid, start, info) {
                                return Err(format!(
                                    "child {pid} appeared and ended during the pass (hand-off?)"
                                ));
                            }
                        }
                    }
                }
                // Round 11 (W2): every walked list again, after the whole walk and the closing snapshot.
                self.recheck_walked(&walked)?;
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

    /// Round 11: a classification of the entry listed as (pid, `listed`) that read ANOTHER start time: the listed process
    /// ended and the pid names a new process; Err when the ended one may have handed off.
    fn listed_as(&mut self, pid: u32, listed: u64, class: &Class) -> Result<(), String> {
        if class_start(class).is_some_and(|now| now != listed)
            && self.ended_suspect(pid, Some(listed), None)
        {
            return Err(format!(
                "child {pid} ended and its pid was reused during the pass (hand-off?)"
            ));
        }
        Ok(())
    }

    /// Classify the descendants of marker-less processes of the job (not stopped, never signalled: their children lists
    /// are read as they are). Round 11 (gate14 G14-M1, W2): each walked process's RAW list is kept in `walked` for the
    /// closing re-read ([`Self::recheck_walked`]); an ended entry or a listed child found dead that may have handed off,
    /// or a walked process that ended, makes the pass inconclusive (never "absent"). Bounded.
    fn walk_through(
        &mut self,
        start: Vec<(u32, u64)>,
        survivors: &mut Vec<LeftoverRecord>,
        walked: &mut BTreeMap<u32, (u64, Listing)>,
    ) -> Result<(), String> {
        let mut stack: Vec<(u32, u64)> = start;
        while let Some((n, ns)) = stack.pop() {
            if walked.contains_key(&n) {
                continue;
            }
            if walked.len() >= 65_536 {
                return Err("more than 65,536 processes below marker-less job processes".into());
            }
            let Some(l) = self.snapshot_children(n, ns)? else {
                return Err(format!(
                    "walked process {n} ended during the pass (its children were re-parented)"
                ));
            };
            let ended = self.handoff_suspects(&l);
            if !ended.is_empty() {
                return Err(format!(
                    "children {ended:?} of walked process {n} ended before their read (hand-off?)"
                ));
            }
            for (&c, &cs) in &l.live {
                let class = self.classify(c, Some(n)).0;
                self.listed_as(c, cs, &class)?;
                match class {
                    Class::Ours(_, f) => survivors.push(record(&f)),
                    Class::Inconclusive(e) => return Err(e),
                    Class::NotOurs {
                        descend: true,
                        facts,
                        ..
                    } => stack.push((c, facts.map_or(cs, |f| f.stat.start_ticks))),
                    Class::NotOurs { .. } => {}
                    Class::Dead(info) => {
                        if self.dead_suspect(c, cs, info) {
                            return Err(format!(
                                "child {c} of walked process {n} ended before its examination (hand-off?)"
                            ));
                        }
                    }
                }
            }
            walked.insert(n, (ns, l));
        }
        Ok(())
    }

    /// Round 11 (gate14 G14-M1, W2): the closing read of every walked process: still the same live process, the SAME live
    /// children (pid and start), and no new ended entry that may have handed off.
    fn recheck_walked(&mut self, walked: &BTreeMap<u32, (u64, Listing)>) -> Result<(), String> {
        for (&w, (ws, before)) in walked {
            let Some(after) = self.snapshot_children(w, *ws)? else {
                return Err(format!(
                    "walked process {w} ended during the pass (its children were re-parented)"
                ));
            };
            if after.live != before.live {
                return Err(format!(
                    "the children of walked process {w} changed during the pass (hand-off?)"
                ));
            }
            let ended = self.handoff_suspects(&after);
            if !ended.is_empty() {
                return Err(format!(
                    "children {ended:?} of walked process {w} ended during the pass (hand-off?)"
                ));
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
        seen_ended: BTreeSet::new(),
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
