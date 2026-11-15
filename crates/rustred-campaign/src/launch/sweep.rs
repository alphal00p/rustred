//! THE CAMPAIGN SWEEP (procfix3; gate-A M-1, gate-B M1): find every live process that carries a marker of THIS campaign
//! (optionally of exactly one attempt), stop it, kill it, and PROVE that none is left.
//!
//! Why a sweep needs more than "scan, SIGKILL, repeat until a scan is empty" (the earlier orphan sweep): a marked chain
//! that re-forks into a new session at every hop (orphaned by a supervisor CRASH, so not in our children tree) always
//! has a live member; SIGKILL of the member a scan saw does not stop the chain when that member has already forked the
//! next one, and a scan that reads a hop inside its execve window (empty environ) does not see it at all. Hence:
//!
//! 1. FREEZE to a fixed point: a STRICT full /proc scan for marked processes (our uid, started late enough, marker of the
//!    campaign / attempt; an empty environ is re-read once, an unreadable one is COUNTED); every hit that passes the
//!    caller's narrowing is SIGSTOPped through the guard (pidfd opened first; the identity checklist of `guard` with
//!    `Proof::Campaign` / `Proof::Marker`, exact start time, our uid, boot id) and kept in the member set even when the
//!    wait for the stop fails (gate-A m-2); a stopped process cannot fork. The round repeats until a complete scan finds
//!    no member that is not yet stopped and nothing unreadable.
//! 2. SIGKILL every member through the same pidfd (the guard re-checks the identity at the send) and wait for the exits.
//!    procfix6 (gate5 m1): a member the marker check refuses at this point (it exec'd into an unmarked image between the
//!    batch's environ read and its SIGSTOP) is killed by the identity HELD AT ITS STOP (`guard::StopHold`: the same
//!    pidfd, the exact start time re-read, our uid); if that is impossible it is CONTINUED (SIGCONT through the same
//!    hold) and reported in `continued`; if neither can be sent it is `stranded`: reported in `left` (the caller ends
//!    with exit 8 and the exact pids), kept with its pidfd and retried by every later sweep of this process. A process
//!    this sweep stopped is never silently left stopped.
//! 3. TWO consecutive CONCLUSIVE scans (`pass_gap` apart) that find no marked process (other than the ones the narrowing
//!    forbids signalling, reported as `left`). A scan with an unreadable candidate or a failed listing is inconclusive.
//!    A survivor sends the sweep back to 1 while the budget lasts.
//!
//! Ownership is never widened here: what may be signalled is exactly what `guard::signal_with_pidfd` accepts for a
//! campaign marker; the caller's `narrowing` can only refuse more (the supervisor's cwd rule, coord-site-policy rule 5).
//! Nothing is ever selected by name or pattern. Runs on the calling thread (blocking, bounded by `budget` plus the reads
//! of one round): call it off the control loop.

use super::guard::{self, ProcessTarget, StartRule, Verdict};
use crate::resources::{
    CampaignMarker, FactRead, HiddenProcess, MarkedProcess, MarkerRead, STRICT_ENVIRON_TIMEOUT,
    boot_id, children_of, describe_hidden, our_uid, owner_uid_checked, read_marker_at,
    read_proc_facts, read_stat, request_reap, scan_marked_inline,
};
use serde::Serialize;
use std::collections::BTreeMap;
use std::os::fd::OwnedFd;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// The caller's narrowing: `Err(why)` = do not signal this marked process (it is reported in `left`). Never widens.
pub type Narrowing = Arc<dyn Fn(&MarkedProcess) -> Result<(), String> + Send + Sync>;

/// What to sweep.
#[derive(Clone)]
pub struct SweepSpec {
    /// The campaign id of the marker (validated).
    pub campaign_id: String,
    /// Only this attempt id (`<job>:a<k>`); None = every attempt of the campaign.
    pub only_attempt: Option<String>,
    /// Processes that started earlier (clock ticks after boot) are never candidates.
    pub not_before_ticks: u64,
    /// Time budget of the whole sweep.
    pub budget: Duration,
    /// Interval between the two proof scans.
    pub pass_gap: Duration,
    pub narrowing: Option<Narrowing>,
}

impl std::fmt::Debug for SweepSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SweepSpec")
            .field("campaign_id", &self.campaign_id)
            .field("only_attempt", &self.only_attempt)
            .field("not_before_ticks", &self.not_before_ticks)
            .field("budget", &self.budget)
            .finish_non_exhaustive()
    }
}

/// A live marked process the sweep did not signal, and why.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LeftMarked {
    pub process: MarkedProcess,
    pub why: String,
}

/// Result of [`sweep`].
#[derive(Clone, Debug, Default, Serialize)]
pub struct SweepReport {
    /// SIGKILL delivered through the guard (each process once).
    pub killed: Vec<MarkedProcess>,
    /// procfix6 (gate5 m1): of `killed`, those killed by the identity held at their stop (the marker was gone).
    pub killed_by_hold: Vec<MarkedProcess>,
    /// procfix6 (gate5 m1): stopped by a sweep, marker gone, could not be killed by the held identity: SIGCONT sent
    /// (no longer stopped); reported, never silently dropped.
    pub continued: Vec<LeftMarked>,
    /// Alive and marked in the last scan, but not signalled (narrowing refused, or the guard could not verify).
    pub left: Vec<LeftMarked>,
    /// Two consecutive conclusive scans found no marked process other than those in `left`.
    pub proven: bool,
    /// Why not proven (None when proven).
    pub reason: Option<String>,
    /// Freeze rounds run.
    pub rounds: u32,
    /// Distinct marked processes seen alive.
    pub seen: usize,
    /// procfix7 (gate6 MJ-A): when not proven because of unreadable candidates, those of the last scan that may descend
    /// from the campaign (pid, name, parent, why); never signalled. Empty when proven.
    pub hidden: Vec<HiddenProcess>,
}

impl SweepReport {
    /// Proven AND nothing left alive: no live process carries the swept marker, and no process a sweep stopped is left
    /// stopped (those are in `left`, see `stranded`).
    pub fn empty(&self) -> bool {
        self.proven && self.left.is_empty()
    }

    /// procfix6 (gate5 m1): the processes a sweep of this process STOPPED and could neither kill nor continue (also in
    /// `left`, with a reason starting with [`STRANDED_WHY`]).
    pub fn stranded(&self) -> impl Iterator<Item = &LeftMarked> {
        self.left.iter().filter(|l| l.why.starts_with(STRANDED_WHY))
    }
}

/// Prefix of the `why` of a stranded process (stopped by a sweep, neither killed nor continued).
pub const STRANDED_WHY: &str = "STOPPED by the sweep and left stopped";

type Key = (u32, u64);

struct Member {
    fd: OwnedFd,
    process: MarkedProcess,
    /// The identity held at the stop (procfix6 m1); None only when the stop was not delivered by this sweep.
    hold: Option<guard::StopHold>,
}

/// procfix6 (gate5 m1): processes a sweep of THIS process stopped and could neither kill nor continue, kept with their
/// pidfd and hold: every later sweep retries them first and reports them in `left` while they stay stranded.
/// Each entry: (campaign id, member); a sweep takes only the entries of its own campaign and attempt narrowing.
static STRANDED: std::sync::Mutex<Vec<(String, Member)>> = std::sync::Mutex::new(Vec::new());

struct Sweep<'a> {
    s: &'a SweepSpec,
    cancel: &'a AtomicBool,
    deadline: Instant,
    boot: String,
    uid: u32,
    members: BTreeMap<Key, Member>,
    left: BTreeMap<Key, LeftMarked>,
    seen: BTreeMap<Key, ()>,
    killed: BTreeMap<Key, MarkedProcess>,
    killed_by_hold: BTreeMap<Key, MarkedProcess>,
    continued: BTreeMap<Key, LeftMarked>,
    stranded: BTreeMap<Key, (Member, String)>,
    rounds: u32,
    /// procfix7: the possibly-ours unreadable candidates of the last strict scan.
    hidden: Vec<HiddenProcess>,
}

fn key(p: &MarkedProcess) -> Key {
    (p.pid, p.start_ticks)
}

impl Sweep<'_> {
    fn out_of_time(&self) -> bool {
        self.cancel.load(Ordering::SeqCst) || Instant::now() >= self.deadline
    }

    /// The target through THE ONE OWNERSHIP RULE (`guard::marker_target`, shared with the hunt and the job signaller).
    fn target(&self, p: &MarkedProcess) -> ProcessTarget {
        guard::marker_target(
            &self.s.campaign_id,
            self.s.only_attempt.as_deref(),
            p.pid,
            StartRule::Exact(p.start_ticks),
            &self.boot,
        )
        .expect("campaign id and attempt label validated at the start of the sweep")
    }

    /// Does this marked process match what we sweep?
    fn wanted(&self, attempt: &str) -> bool {
        self.s.only_attempt.as_deref().is_none_or(|o| o == attempt)
    }

    /// Marked children of a STOPPED member (its list is stable). Unreadable facts of a child: Err (inconclusive).
    fn marked_children(&mut self, pid: u32) -> Result<Vec<MarkedProcess>, String> {
        let mut out = Vec::new();
        let Some(kids) = children_of(pid)? else {
            return Ok(out);
        };
        for c in kids {
            // procfix7 (gate7 B1, the class): a child whose owner cannot be read makes the round inconclusive
            match owner_uid_checked(Path::new("/proc"), c) {
                Ok(Some(u)) if u == self.uid => {}
                Ok(_) => continue,
                Err(e) => return Err(format!("owner of child {c}: {e}")),
            }
            let st = match read_stat(c) {
                Ok(Some(st)) => st,
                Ok(None) => continue,
                Err(e) => return Err(format!("stat of child {c}: {e}")),
            };
            if st.start_ticks < self.s.not_before_ticks || st.is_dead() {
                continue;
            }
            match read_marker_at(Path::new("/proc"), c, None, STRICT_ENVIRON_TIMEOUT) {
                MarkerRead::Marker(v) => {
                    if let Some(m) = CampaignMarker::parse(&v)
                        && m.campaign_id == self.s.campaign_id
                        && self.wanted(&m.attempt_id)
                    {
                        out.push(MarkedProcess::from_stat(&st, m.attempt_id));
                    }
                }
                MarkerRead::Unreadable => {
                    // A child of a stopped member descends from the campaign: never classified away.
                    self.hidden = vec![HiddenProcess {
                        pid: c,
                        start_ticks: st.start_ticks,
                        ppid: pid,
                        comm: st.comm.clone(),
                        why: format!("environ unreadable; a child of the stopped member {pid}"),
                    }];
                    return Err(format!(
                        "environ of child {c} ({}) of the stopped member {pid} unreadable",
                        st.comm
                    ));
                }
                MarkerRead::NoMarker | MarkerRead::Gone => {}
            }
        }
        Ok(out)
    }

    fn wait_stopped(&self, pid: u32, fd: &OwnedFd) -> Result<(), String> {
        let t0 = Instant::now();
        loop {
            if guard::pidfd_exited(fd) == Some(true) {
                return Ok(());
            }
            match read_proc_facts(pid, false) {
                FactRead::Gone => return Ok(()),
                FactRead::Failed(e) => return Err(e),
                FactRead::Facts(f) if f.dead || f.all_stopped => return Ok(()),
                FactRead::Facts(_) => {}
            }
            if t0.elapsed() > Duration::from_millis(500) {
                return Err(format!("pid {pid} did not stop (D state?)"));
            }
            std::thread::sleep(Duration::from_micros(200));
        }
    }

    /// Stop the candidates of one round through the guard (each: Ok(true) a new member; Ok(false) gone, not ours any
    /// more or already a member; Err could not decide): narrowing and pidfds per candidate, then ONE guarded SIGSTOP
    /// batch.
    fn stop_candidates(&mut self, queue: Vec<MarkedProcess>) -> Vec<Result<bool, String>> {
        let mut out = Vec::with_capacity(queue.len());
        let mut cands: Vec<(OwnedFd, MarkedProcess)> = Vec::new();
        let mut seen_now: std::collections::BTreeSet<Key> = Default::default();
        for p in queue {
            let k = key(&p);
            if self.members.contains_key(&k) || !seen_now.insert(k) {
                out.push(Ok(false));
                continue;
            }
            self.seen.insert(k, ());
            if let Some(n) = &self.s.narrowing
                && let Err(why) = n(&p)
            {
                self.left.insert(k, LeftMarked { process: p, why });
                out.push(Ok(false));
                continue;
            }
            match guard::open_pidfd(p.pid) {
                Ok(fd) => cands.push((fd, p)),
                Err(Verdict::Gone) | Err(Verdict::NotOurs(_)) => out.push(Ok(false)),
                Err(Verdict::Unverified(e)) => out.push(Err(e)),
                Err(Verdict::Delivered) => out.push(Err("impossible verdict".into())),
            }
        }
        let items: Vec<(&OwnedFd, ProcessTarget)> =
            cands.iter().map(|(fd, p)| (fd, self.target(p))).collect();
        let verdicts = guard::stop_with_pidfds(&items, None);
        drop(items);
        for ((fd, p), (v, hold)) in cands.into_iter().zip(verdicts) {
            let k = key(&p);
            out.push(match v {
                Verdict::Delivered => {
                    // Kept as a member whatever the wait says (gate-A m-2): it will get the SIGKILL.
                    let w = self.wait_stopped(p.pid, &fd);
                    self.left.remove(&k);
                    self.members.insert(
                        k,
                        Member {
                            fd,
                            process: p,
                            hold,
                        },
                    );
                    w.map(|()| true)
                }
                Verdict::Gone | Verdict::NotOurs(_) => Ok(false),
                Verdict::Unverified(e) => {
                    self.left.insert(
                        k,
                        LeftMarked {
                            process: p,
                            why: format!("not verified: {e}"),
                        },
                    );
                    Err(e)
                }
            });
        }
        out
    }

    /// Step 1. Ok at the fixed point; Err(reason) at the bound.
    fn freeze(&mut self) -> Result<(), String> {
        let mut last = String::from("no round completed");
        loop {
            if self.out_of_time() {
                return Err(format!(
                    "budget reached while freezing after {} rounds ({last})",
                    self.rounds
                ));
            }
            self.rounds += 1;
            let mut new = false;
            let mut inconclusive: Option<String> = None;
            let mut queue: Vec<MarkedProcess> = Vec::new();
            match scan_marked_inline(
                &self.s.campaign_id,
                self.s.not_before_ticks,
                self.s.only_attempt.as_deref(),
            ) {
                Ok(scan) => {
                    if scan.unreadable > 0 {
                        inconclusive = Some(format!(
                            "{} candidate(s) with an unreadable environ that may descend from the campaign: {}",
                            scan.unreadable,
                            describe_hidden(&scan.hidden)
                        ));
                    }
                    self.hidden = scan.hidden;
                    queue.extend(scan.processes);
                }
                Err(e) => inconclusive = Some(format!("/proc scan: {e}")),
            }
            let pids: Vec<u32> = self.members.keys().map(|k| k.0).collect();
            for pid in pids {
                match self.marked_children(pid) {
                    Ok(k) => queue.extend(k),
                    Err(e) => inconclusive = Some(e),
                }
            }
            // Newest first (a re-forking chain's current hop).
            queue.sort_by_key(|p| std::cmp::Reverse((p.start_ticks, p.pid)));
            // procfix5 M1: the SIGSTOPs of one round go out as ONE batch (one intent sync).
            for r in self.stop_candidates(queue) {
                match r {
                    Ok(true) => new = true,
                    Ok(false) => {}
                    Err(e) => inconclusive = Some(e),
                }
            }
            // Members still stopped? (a SIGCONT sent before a member was found may continue it.)
            let keys: Vec<Key> = self.members.keys().copied().collect();
            for k in keys {
                let m = &self.members[&k];
                if guard::pidfd_exited(&m.fd) == Some(true) {
                    continue;
                }
                match read_proc_facts(k.0, false) {
                    FactRead::Facts(f) if f.all_stopped || f.dead => {}
                    FactRead::Facts(_) => {
                        // procfix6 m1: a member that runs again WITHOUT the marker is refused here; it is not a reason
                        // for another round (it cannot hand the marker on); the kill round takes it by its stop hold.
                        let t = self.target(&m.process);
                        match guard::signal_with_pidfd(&m.fd, &t, libc::SIGSTOP, None) {
                            Verdict::Delivered => new = true,
                            Verdict::Unverified(e) => inconclusive = Some(e),
                            Verdict::Gone | Verdict::NotOurs(_) => {}
                        }
                    }
                    FactRead::Gone => {}
                    FactRead::Failed(e) => inconclusive = Some(e),
                }
            }
            if let Some(e) = inconclusive {
                last = e;
                std::thread::sleep(Duration::from_millis(2));
                continue;
            }
            if new {
                last = "new members in the last round".into();
                continue;
            }
            return Ok(());
        }
    }

    /// Step 2: SIGKILL every member through its pidfd; wait (bounded) for the exits.
    fn kill_members(&mut self) {
        let members = std::mem::take(&mut self.members);
        let mut pending = Vec::new();
        // procfix5 M1: one batch (one intent sync) for the whole kill round.
        let items: Vec<(&OwnedFd, ProcessTarget)> = members
            .values()
            .map(|m| (&m.fd, self.target(&m.process)))
            .collect();
        let verdicts = guard::signal_with_pidfds(&items, libc::SIGKILL, None);
        drop(items);
        let mut refused: Vec<(Key, Member, String)> = Vec::new();
        for ((k, m), v) in members.into_iter().zip(verdicts) {
            match v {
                Verdict::Delivered => {
                    self.killed.insert(k, m.process.clone());
                    pending.push(m);
                }
                Verdict::Gone => {}
                // procfix6 (gate5 m1): never dropped silently: a member this sweep STOPPED goes through its stop hold.
                Verdict::NotOurs(r) if m.hold.is_some() => refused.push((
                    k,
                    m,
                    format!("marker check refused at the kill round: {r:?}"),
                )),
                Verdict::Unverified(e) if m.hold.is_some() => {
                    refused.push((k, m, format!("SIGKILL not verified: {e}")))
                }
                Verdict::NotOurs(_) => {}
                Verdict::Unverified(e) => {
                    self.left.insert(
                        k,
                        LeftMarked {
                            process: m.process.clone(),
                            why: format!("SIGKILL not verified: {e}"),
                        },
                    );
                }
            }
        }
        pending.extend(self.settle_held(refused));
        let t0 = Instant::now();
        for m in pending {
            while guard::pidfd_exited(&m.fd) != Some(true) && t0.elapsed() < Duration::from_secs(2)
            {
                std::thread::sleep(Duration::from_micros(500));
            }
        }
        request_reap();
    }

    /// procfix6 (gate5 m1): members STOPPED by a sweep that the marker path did not kill: SIGKILL by the identity held
    /// at the stop; else SIGCONT through the same hold (reported in `continued`); else stranded (reported in `left`, kept
    /// for the next sweep). Returns the members to wait for (killed).
    fn settle_held(&mut self, refused: Vec<(Key, Member, String)>) -> Vec<Member> {
        let mut pending = Vec::new();
        if refused.is_empty() {
            return pending;
        }
        let kill: Vec<(&OwnedFd, &guard::StopHold)> = refused
            .iter()
            .filter_map(|(_, m, _)| m.hold.as_ref().map(|h| (&m.fd, h)))
            .collect();
        let kv = guard::kill_held(&kill);
        drop(kill);
        let mut cont: Vec<(Key, Member, String)> = Vec::new();
        for ((k, m, why), v) in refused.into_iter().zip(kv) {
            match v {
                Verdict::Delivered => {
                    self.killed.insert(k, m.process.clone());
                    self.killed_by_hold.insert(k, m.process.clone());
                    self.left.remove(&k);
                    pending.push(m);
                }
                Verdict::Gone => {
                    self.left.remove(&k);
                }
                Verdict::NotOurs(r) => {
                    cont.push((k, m, format!("{why}; held SIGKILL refused: {r:?}")))
                }
                Verdict::Unverified(e) => {
                    cont.push((k, m, format!("{why}; held SIGKILL not sent: {e}")))
                }
            }
        }
        if cont.is_empty() {
            return pending;
        }
        let items: Vec<(&OwnedFd, &guard::StopHold)> = cont
            .iter()
            .filter_map(|(_, m, _)| m.hold.as_ref().map(|h| (&m.fd, h)))
            .collect();
        let cv = guard::continue_held(&items);
        drop(items);
        for ((k, m, why), v) in cont.into_iter().zip(cv) {
            match v {
                Verdict::Delivered => {
                    self.left.remove(&k);
                    self.continued.insert(
                        k,
                        LeftMarked {
                            process: m.process.clone(),
                            why: format!("{why}; CONTINUED (continue signal through the stop hold): alive, not stopped"),
                        },
                    );
                }
                Verdict::Gone => {
                    self.left.remove(&k);
                }
                Verdict::NotOurs(r) => {
                    let w = format!("{STRANDED_WHY}: {why}; continue refused: {r:?}");
                    self.stranded.insert(k, (m, w));
                }
                Verdict::Unverified(e) => {
                    let w = format!("{STRANDED_WHY}: {why}; continue not sent: {e}");
                    self.stranded.insert(k, (m, w));
                }
            }
        }
        pending
    }

    /// procfix6 (gate5 m1): the processes earlier sweeps of this process left stranded: tried again first (kill by the
    /// hold, else continue).
    fn retry_stranded(&mut self) {
        let old: Vec<Member> = {
            let mut g = STRANDED.lock().unwrap_or_else(|p| p.into_inner());
            let (mine, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut *g)
                .into_iter()
                .partition(|(c, m)| *c == self.s.campaign_id && self.wanted(&m.process.attempt_id));
            *g = rest;
            mine.into_iter().map(|(_, m)| m).collect()
        };
        if old.is_empty() {
            return;
        }
        let refused: Vec<(Key, Member, String)> = old
            .into_iter()
            .map(|m| {
                (
                    key(&m.process),
                    m,
                    "stranded by an earlier sweep of this process".to_string(),
                )
            })
            .collect();
        let pending = self.settle_held(refused);
        let t0 = Instant::now();
        for m in pending {
            while guard::pidfd_exited(&m.fd) != Some(true) && t0.elapsed() < Duration::from_secs(2)
            {
                std::thread::sleep(Duration::from_micros(500));
            }
        }
    }

    /// One proof scan: Ok(marked processes alive that are not in `left`) when conclusive.
    fn pass(&mut self) -> Result<Vec<MarkedProcess>, String> {
        let scan = scan_marked_inline(
            &self.s.campaign_id,
            self.s.not_before_ticks,
            self.s.only_attempt.as_deref(),
        )
        .map_err(|e| format!("/proc scan: {e}"))?;
        // Left entries that are no longer alive and marked drop out of the report.
        let alive: BTreeMap<Key, ()> = scan.processes.iter().map(|p| (key(p), ())).collect();
        self.left.retain(|k, _| alive.contains_key(k));
        self.hidden = scan.hidden.clone();
        if scan.unreadable > 0 {
            return Err(format!(
                "{} candidate(s) with an unreadable environ that may descend from the campaign: {}",
                scan.unreadable,
                describe_hidden(&scan.hidden)
            ));
        }
        Ok(scan
            .processes
            .into_iter()
            .filter(|p| !self.left.contains_key(&key(p)))
            .collect())
    }

    fn two_passes(&mut self) -> Result<Vec<MarkedProcess>, String> {
        let a = self.pass()?;
        if !a.is_empty() {
            return Ok(a);
        }
        std::thread::sleep(self.s.pass_gap);
        self.pass()
    }
}

/// Run one sweep (blocking; call it off the control loop). Signals only through `guard` (see the module documentation);
/// every delivery is logged with the context "campaign sweep" (gate3 M2).
pub fn sweep(spec: &SweepSpec, cancel: &AtomicBool) -> SweepReport {
    guard::with_context("campaign sweep", spec.only_attempt.as_deref(), || {
        sweep_inner(spec, cancel)
    })
}

fn sweep_inner(spec: &SweepSpec, cancel: &AtomicBool) -> SweepReport {
    let boot = match boot_id() {
        Ok(b) => b,
        Err(e) => {
            return SweepReport {
                reason: Some(format!("boot id unreadable: {e}; nothing signalled")),
                ..Default::default()
            };
        }
    };
    if let Err(e) = crate::resources::validate_campaign_id(&spec.campaign_id) {
        return SweepReport {
            reason: Some(e.to_string()),
            ..Default::default()
        };
    }
    // Never widened: an attempt label that is not a valid marker sweeps NOTHING (not the whole campaign).
    if let Some(a) = &spec.only_attempt
        && CampaignMarker::new(&spec.campaign_id, a).is_err()
    {
        return SweepReport {
            reason: Some(format!(
                "attempt label {a:?} is not a valid marker: nothing signalled"
            )),
            ..Default::default()
        };
    }
    let mut w = Sweep {
        s: spec,
        cancel,
        deadline: Instant::now() + spec.budget,
        boot,
        uid: our_uid(),
        members: BTreeMap::new(),
        left: BTreeMap::new(),
        seen: BTreeMap::new(),
        killed: BTreeMap::new(),
        killed_by_hold: BTreeMap::new(),
        continued: BTreeMap::new(),
        stranded: BTreeMap::new(),
        rounds: 0,
        hidden: Vec::new(),
    };
    w.retry_stranded();
    let reason = loop {
        // A freeze that hit the bound still ends in the SIGKILL of every member; the proof below decides.
        #[allow(clippy::let_underscore_must_use)]
        // ignored-ok (procfix7): a freeze that hit the bound still ends in the kill round; the two proof scans decide
        let _ = w.freeze();
        w.kill_members();
        match w.two_passes() {
            Ok(s) if s.is_empty() => break None,
            Ok(s) => {
                if w.out_of_time() {
                    break Some(format!("{} marked process(es) still alive", s.len()));
                }
            }
            Err(e) => {
                if w.out_of_time() {
                    break Some(format!("proof scan inconclusive: {e}"));
                }
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    };
    // procfix6 (gate5 m1): stranded processes are reported in `left` (the caller's exit-8 rule names their pids) and
    // kept, with their pidfd and hold, for the next sweep of this process.
    let mut left = w.left;
    let mut keep = Vec::new();
    for (k, (m, why)) in std::mem::take(&mut w.stranded) {
        left.insert(
            k,
            LeftMarked {
                process: m.process.clone(),
                why,
            },
        );
        keep.push((spec.campaign_id.clone(), m));
    }
    STRANDED
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .extend(keep);
    SweepReport {
        killed: w.killed.into_values().collect(),
        killed_by_hold: w.killed_by_hold.into_values().collect(),
        continued: w.continued.into_values().collect(),
        left: left.into_values().collect(),
        proven: reason.is_none(),
        hidden: if reason.is_none() {
            Vec::new()
        } else {
            w.hidden
        },
        reason,
        rounds: w.rounds,
        seen: w.seen.len(),
    }
}

#[allow(clippy::let_underscore_must_use)] // test cleanup (outside the reviewed scope, procfix7)
#[cfg(test)]
mod tests {
    use super::*;

    /// procfix6 (gate5 m1): a member STOPPED by a sweep whose hold cannot be used (here: another boot) can neither be
    /// killed nor continued: it is STRANDED: reported in `left` with the stranded reason (the supervisor's exit-8 rule
    /// names its pid), kept in the registry, retried first by the next sweep of the SAME campaign (and reported again),
    /// never taken by a sweep of another campaign.
    #[test]
    fn m1_stranded_members_are_reported_kept_and_retried() {
        if !crate::launch::in_private_namespace() {
            eprintln!(
                "SKIPPED: spawns and signals processes: only inside a private pid namespace (rule L10)"
            );
            return;
        }
        let mut c = std::process::Command::new("sleep")
            .arg("30")
            .env_clear()
            .spawn()
            .unwrap();
        let pid = c.id();
        // test code; our own direct child (the shell's `kill` builtin: no `kill` call outside the guard's `sys` module).
        let st = std::process::Command::new("sh")
            .args(["-c", &format!("kill -STOP {pid}")])
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .status()
            .unwrap();
        assert!(st.success());
        let t0 = Instant::now();
        while crate::resources::read_stat(pid).unwrap().unwrap().state != 'T' {
            assert!(t0.elapsed() < Duration::from_secs(10));
            std::thread::sleep(Duration::from_millis(5));
        }
        let st = crate::resources::read_stat(pid).unwrap().unwrap();
        let id = "pf6strand.0123456789abcdef";
        let spec = SweepSpec {
            campaign_id: id.into(),
            only_attempt: None,
            not_before_ticks: 0,
            budget: Duration::from_secs(2),
            pass_gap: Duration::from_millis(20),
            narrowing: None,
        };
        let cancel = AtomicBool::new(false);
        let mut w = Sweep {
            s: &spec,
            cancel: &cancel,
            deadline: Instant::now() + spec.budget,
            boot: boot_id().unwrap(),
            uid: our_uid(),
            members: BTreeMap::new(),
            left: BTreeMap::new(),
            seen: BTreeMap::new(),
            killed: BTreeMap::new(),
            killed_by_hold: BTreeMap::new(),
            continued: BTreeMap::new(),
            stranded: BTreeMap::new(),
            rounds: 0,
            hidden: Vec::new(),
        };
        let process = MarkedProcess::from_stat(&st, "j1:a1".into());
        let m = Member {
            fd: guard::open_pidfd(pid).unwrap(),
            process,
            hold: Some(guard::StopHold::for_test(
                pid,
                st.start_ticks,
                "00000000-0000-0000-0000-000000000000",
            )),
        };
        let k = (pid, st.start_ticks);
        let pending = w.settle_held(vec![(k, m, "test".into())]);
        assert!(pending.is_empty());
        assert!(w.killed.is_empty() && w.continued.is_empty());
        let (m, why) = w.stranded.remove(&k).expect("stranded");
        assert!(why.starts_with(STRANDED_WHY), "{why}");
        STRANDED.lock().unwrap().push((id.to_string(), m));
        // Another campaign's sweep never takes it.
        let other = SweepSpec {
            campaign_id: "pf6other.0123456789abcdef".into(),
            ..spec.clone()
        };
        let r = sweep(&other, &cancel);
        assert_eq!(r.stranded().count(), 0);
        // The same campaign's next sweep retries it first and reports it again (not empty: exit 8 for the caller).
        let r = sweep(&spec, &cancel);
        let s: Vec<&LeftMarked> = r.stranded().collect();
        assert_eq!(s.len(), 1, "{r:?}");
        assert_eq!(s[0].process.pid, pid);
        assert!(!r.empty());
        assert_eq!(
            crate::resources::read_stat(pid).unwrap().unwrap().state,
            'T',
            "neither killed nor continued (the hold is unusable)"
        );
        STRANDED.lock().unwrap().retain(|(c, _)| c != id);
        let _ = c.kill();
        let _ = c.wait();
    }
}
