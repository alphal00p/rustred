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

use super::guard::{self, ProcessTarget, Proof, StartRule, Verdict};
use super::{Enumeration, LeftoverRecord, UnattributedChild, VerifiedBy};
use crate::resources::{
    CampaignMarker, FactRead, Leaders, MarkerFact, ProcFacts, children_of, our_uid, own_children,
    own_children_by_scan, read_proc_facts, request_reap, strict_listing_counted,
};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::os::fd::OwnedFd;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

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
    NotOurs { unattributed: bool, facts: Option<ProcFacts> },
    Dead,
    Inconclusive(String),
}

struct Member {
    fd: OwnedFd,
    facts: ProcFacts,
    proof: Proof,
}

enum Stop {
    Stopped,
    Exited,
    Failed(String),
}

fn me() -> u32 {
    std::process::id()
}

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
    not_ours: HashSet<(u32, u64)>,
    unattributed: BTreeMap<(u32, u64), UnattributedChild>,
    killed: Vec<LeftoverRecord>,
    cgroup_ok: bool,
}

impl Hunt<'_> {
    fn target(&self, f: &ProcFacts, proof: Proof) -> ProcessTarget {
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
        let me = me();
        if pid <= 1 || pid == me {
            return (Class::NotOurs { unattributed: false, facts: None }, None);
        }
        if self.registered(pid) {
            // Another job's live leader, or J's own zombie leader: never J's survivor.
            return (Class::NotOurs { unattributed: false, facts: None }, None);
        }
        let fd = match guard::open_pidfd(pid) {
            Ok(fd) => fd,
            Err(Verdict::Gone) => return (Class::Dead, None),
            Err(Verdict::NotOurs(_)) => {
                return (Class::NotOurs { unattributed: false, facts: None }, None);
            }
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
        if self.not_ours.contains(&key) {
            return (Class::NotOurs { unattributed: false, facts: None }, None);
        }
        let direct_child = facts.stat.ppid == me;
        if facts.stat.start_ticks < self.s.launch_ticks {
            self.not_ours.insert(key);
            return (Class::NotOurs { unattributed: false, facts: Some(facts) }, None);
        }
        if self.s.pinned && facts.stat.session == self.s.sid {
            return (Class::Ours(Proof::PinnedSession(self.s.sid), facts), Some(fd));
        }
        if facts.stat.session == self.own_session {
            self.not_ours.insert(key);
            return (Class::NotOurs { unattributed: false, facts: Some(facts) }, None);
        }
        match &facts.marker {
            MarkerFact::Value(v) => match CampaignMarker::parse(v) {
                Some(m) if m == self.s.marker => (Class::Ours(Proof::Marker(m), facts), Some(fd)),
                _ => {
                    self.not_ours.insert(key);
                    (Class::NotOurs { unattributed: false, facts: Some(facts) }, None)
                }
            },
            MarkerFact::Absent => {
                self.not_ours.insert(key);
                (Class::NotOurs { unattributed: direct_child, facts: Some(facts) }, None)
            }
            MarkerFact::Unreadable(e) => {
                if facts.real_uid != self.uid {
                    self.not_ours.insert(key);
                    (Class::NotOurs { unattributed: false, facts: Some(facts) }, None)
                } else {
                    (
                        Class::Inconclusive(format!("pid {pid}: environ unreadable ({e})")),
                        None,
                    )
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

    fn stop(&self, fd: &OwnedFd, f: &ProcFacts, proof: &Proof) -> Stop {
        let t = self.target(f, proof.clone());
        match guard::signal_with_pidfd(fd, &t, libc::SIGSTOP, Some(&self.s.leaders)) {
            Verdict::Delivered => self.wait_stopped(f.stat.pid, fd),
            Verdict::Gone => Stop::Exited,
            Verdict::NotOurs(r) => Stop::Failed(format!("pid {}: refused: {r:?}", f.stat.pid)),
            Verdict::Unverified(e) => Stop::Failed(e),
        }
    }

    fn out_of_time(&self) -> bool {
        self.cancel.load(Ordering::SeqCst) || Instant::now() >= self.deadline
    }

    /// Step 2: stop every member until a fixed point. Ok(members) at the fixed point; Err(reason) when the bound is hit.
    fn freeze_fixed_point(&mut self) -> Result<BTreeMap<u32, Member>, (String, BTreeMap<u32, Member>)> {
        let mut known: BTreeMap<u32, Member> = BTreeMap::new();
        if self.s.pinned {
            let _ = guard::signal_pinned_group(&self.s.leaders, self.s.sid, libc::SIGSTOP);
        }
        let me = me();
        let mut last_reason = String::from("no round completed");
        for round in 0..self.s.max_rounds {
            if self.out_of_time() {
                return Err((
                    format!("reforking: hunt bound reached after {round} rounds ({last_reason})"),
                    known,
                ));
            }
            let mut changed = false;
            let mut inconclusive: Option<String> = None;
            let mut queue: Vec<(u32, Option<u32>)> = Vec::new();
            match self.s.enumeration {
                Enumeration::SubreaperTree => match own_children() {
                    Ok(k) => queue.extend(k.into_iter().map(|p| (p, Some(me)))),
                    Err(e) => inconclusive = Some(format!("own children: {e}")),
                },
                Enumeration::FullProc => match strict_listing_counted() {
                    Ok((all, _)) => queue.extend(
                        all.into_iter()
                            .filter(|st| st.start_ticks >= self.s.launch_ticks)
                            .map(|st| (st.pid, None)),
                    ),
                    Err(e) => inconclusive = Some(format!("/proc listing: {e}")),
                },
            }
            // Children of every stopped member (their lists are stable: a stopped process cannot fork).
            let member_pids: Vec<u32> = known.keys().copied().collect();
            for m in member_pids {
                match children_of(m) {
                    Ok(Some(k)) => queue.extend(k.into_iter().map(|p| (p, Some(m)))),
                    Ok(None) => {}
                    Err(e) => inconclusive = Some(e),
                }
            }
            let mut seen: BTreeSet<u32> = BTreeSet::new();
            while let Some((pid, parent)) = queue.pop() {
                if known.contains_key(&pid) || !seen.insert(pid) {
                    continue;
                }
                let (class, fd) = self.classify(pid, parent);
                match class {
                    Class::Ours(proof, facts) => {
                        let Some(fd) = fd else { continue };
                        match self.stop(&fd, &facts, &proof) {
                            Stop::Stopped => {
                                changed = true;
                                if self.s.enumeration == Enumeration::SubreaperTree {
                                    match children_of(pid) {
                                        Ok(Some(k)) => {
                                            queue.extend(k.into_iter().map(|c| (c, Some(pid))))
                                        }
                                        Ok(None) => {}
                                        Err(e) => inconclusive = Some(e),
                                    }
                                }
                                known.insert(pid, Member { fd, facts, proof });
                            }
                            Stop::Exited => changed = true,
                            Stop::Failed(e) => inconclusive = Some(e),
                        }
                    }
                    Class::NotOurs { unattributed: true, facts: Some(f) } => self.note_unattributed(&f),
                    Class::NotOurs { .. } | Class::Dead => {}
                    Class::Inconclusive(e) => inconclusive = Some(e),
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
                        if let Stop::Failed(e) = self.stop(fd, &facts, &proof) {
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
            match inconclusive {
                None if !changed => return Ok(known),
                Some(e) => {
                    last_reason = e;
                    std::thread::sleep(Duration::from_millis(2));
                }
                None => last_reason = "new members in the last round".into(),
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

    /// Step 3: SIGKILL every stopped member (and the pinned group, and the cgroup); wait for the exits (bounded).
    fn kill_all(&mut self, known: BTreeMap<u32, Member>) -> Vec<LeftoverRecord> {
        if self.s.pinned {
            let _ = guard::signal_pinned_group(&self.s.leaders, self.s.sid, libc::SIGKILL);
        }
        if let Some(cg) = &self.s.cgroup {
            let _ = super::cgroup::kill_path(cg);
        }
        let mut pending: Vec<Member> = Vec::new();
        for (_, m) in known {
            let t = self.target(&m.facts, m.proof.clone());
            match guard::signal_with_pidfd(&m.fd, &t, libc::SIGKILL, Some(&self.s.leaders)) {
                Verdict::Delivered | Verdict::Gone => {
                    self.killed.push(record(&m.facts));
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
        request_reap();
        left
    }

    /// One proof pass. Ok(survivors) when conclusive; Err(reason) when not.
    fn pass(&mut self) -> Result<Vec<LeftoverRecord>, String> {
        let me = me();
        let mut survivors = Vec::new();
        if self.s.pinned && !self.leader_pinned() {
            return Err("the leader is no longer held unreaped".into());
        }
        match self.s.enumeration {
            Enumeration::SubreaperTree => {
                let mut tops = own_children().map_err(|e| format!("own children: {e}"))?;
                tops.extend(own_children_by_scan().map_err(|e| format!("ppid scan: {e}"))?);
                for pid in tops {
                    match self.classify(pid, Some(me)) {
                        (Class::Ours(_, f), _) => survivors.push(record(&f)),
                        (Class::Inconclusive(e), _) => return Err(e),
                        (Class::NotOurs { unattributed: true, facts: Some(f) }, _) => {
                            self.note_unattributed(&f)
                        }
                        _ => {}
                    }
                }
            }
            Enumeration::FullProc => {
                let (all, vanished) =
                    strict_listing_counted().map_err(|e| format!("/proc listing: {e}"))?;
                if vanished > 0 {
                    return Err(format!("{vanished} /proc entries vanished during the pass"));
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
        if let Some(cg) = &self.s.cgroup {
            match super::cgroup::procs_of(cg) {
                Ok(p) if p.is_empty() => {}
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

    fn leader_pinned(&self) -> bool {
        self.s.leaders.lock().contains(&self.s.sid) && super::real::peek_exit(self.s.sid).is_ok()
    }

    fn two_passes(&mut self) -> Result<Vec<LeftoverRecord>, String> {
        let a = self.pass()?;
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
    let mut h = Hunt {
        s: spec,
        cancel,
        deadline: Instant::now() + spec.budget,
        own_session: own_session(),
        uid: our_uid(),
        not_ours: HashSet::new(),
        unattributed: BTreeMap::new(),
        killed: Vec::new(),
        cgroup_ok: spec.cgroup.is_some(),
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
        std::thread::sleep(Duration::from_millis(5));
    }
    HuntResult::NotProven {
        reason,
        left,
        killed: h.killed,
        unattributed: h.unattributed.into_values().collect(),
    }
}
