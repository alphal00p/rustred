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
use crate::resources::procfs::{self, ProcError};
use crate::resources::{
    CampaignMarker, CandidateCheck, FactRead, HiddenProcess, MarkedProcess, MarkerRead,
    STRICT_ENVIRON_TIMEOUT, boot_id, check_candidate, children_of, describe_hidden, our_uid,
    owner_uid_at, read_marker_at, read_proc_facts, read_stat, request_reap, scan_marked_inline,
};
use serde::Serialize;
use std::collections::BTreeMap;
use std::os::fd::OwnedFd;
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
    /// procfix7 round 5 (gate9 G9-B2, process churn) / round 6 (gate10 M10-1): marked processes a scan of this sweep saw
    /// ALIVE that were gone, or no longer the same marked process (pid reused, marker dropped by an exec, another uid),
    /// before their SIGSTOP (not killed by this sweep; a new holder of the pid is never signalled). Such a process may
    /// have started a successor that no scan saw (a chain that forks and exits faster than a scan reads it is invisible
    /// to a /proc scan): a sweep with one is never proven, and the supervisor keeps it open until an ATOMIC proof covers
    /// its successors (RUNBOOK 2.10), else the end is exit 8 naming it.
    pub vanished: Vec<MarkedProcess>,
}

/// procfix7 round 5 (gate9 G9-B2): what a "proven" sweep means. A /proc scan is NOT atomic: a chain of processes that
/// each fork a successor and exit between a scan's listing and its read of them (a gap of milliseconds in a namespace,
/// more on a busy host) is never seen, and no userspace reading of /proc can exclude it. Without a job scope / cgroup
/// (the test namespaces; a run outside systemd) "proven" therefore means exactly this sentence; with a scope, the
/// kernel's cgroup membership (`cgroup.events` populated 0, `cgroup.kill`) is the atomic proof for the processes inside
/// it (RUNBOOK 2.7).
pub fn proof_words(scans: u32) -> String {
    format!(
        "no marked process seen in {scans} consecutive stable scans (a /proc scan cannot exclude a process chain that \
         forks and exits faster than it reads; only a job scope's cgroup membership is atomic)"
    )
}

/// procfix7 round 6 (gate10 mutation M4: the vanished rule must be pinned by a test). TEST BUILDS ONLY:
/// `sweep_vanish_pid:<pid>` in RR_CAMPAIGN_FAILPOINT makes every sweep report that marked process as seen alive and
/// gone before its stop (it is left alive): the end must stay unproven.
fn test_vanish_pid() -> Option<u32> {
    #[cfg(debug_assertions)]
    {
        static P: std::sync::OnceLock<Option<u32>> = std::sync::OnceLock::new();
        *P.get_or_init(|| {
            std::env::var(crate::supervisor::failpoint::FAILPOINT_ENV)
                .ok()?
                .split(',')
                .find_map(|p| p.trim().strip_prefix("sweep_vanish_pid:")?.parse().ok())
        })
    }
    #[cfg(not(debug_assertions))]
    {
        None
    }
}

/// The number of consecutive conclusive scans a proven sweep rests on (step 3 of the module doc).
pub const PROOF_SCANS: u32 = 2;

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
    /// procfix8 (gate8, item 4): EVERY possibly-ours candidate any scan of this sweep reported (pid -> the start time held,
    /// if one was read). A suspect leaves only by a COMPLETE direct read ([`check_candidate`]) that proves it dead, not a
    /// candidate, unmarked or not ours; every proof pass re-checks the suspects its scan did not report, so a later scan
    /// that misses one (a listing, a misread) cannot make the sweep conclusive.
    suspects: BTreeMap<u32, Option<u64>>,
    /// procfix7 round 5: marked processes seen alive and gone before their stop (see [`SweepReport::vanished`]).
    vanished: BTreeMap<Key, MarkedProcess>,
}

fn key(p: &MarkedProcess) -> Key {
    (p.pid, p.start_ticks)
}

impl Sweep<'_> {
    fn note_suspects(&mut self, hidden: &[HiddenProcess]) {
        for h in hidden {
            // the latest start time read wins (another start than the one held means the earlier process died and the
            // pid names a new suspect)
            let start = (h.start_ticks != 0).then_some(h.start_ticks);
            let e = self.suspects.entry(h.pid).or_insert(start);
            if start.is_some() {
                *e = start;
            }
        }
    }

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
        let root = procfs::sys();
        let kids = match children_of(pid) {
            Ok(k) => k,
            Err(ProcError::Gone) => return Ok(out),
            Err(e) => return Err(format!("children of the stopped member {pid}: {e}")),
        };
        for c in kids {
            // procfix7 (gate7 B1, the class): a child whose owner cannot be read makes the round inconclusive
            match owner_uid_at(&root, c) {
                Ok(u) if u == self.uid => {}
                Ok(_) | Err(ProcError::Gone) => continue,
                Err(e) => return Err(format!("owner of child {c}: {e}")),
            }
            let st = match read_stat(c) {
                Ok(st) => st,
                Err(ProcError::Gone) => continue,
                Err(e) => return Err(format!("stat of child {c}: {e}")),
            };
            if st.start_ticks < self.s.not_before_ticks || st.is_dead() {
                continue;
            }
            match read_marker_at(&root, c, None, STRICT_ENVIRON_TIMEOUT) {
                MarkerRead::Marker(v) => {
                    if let Some(m) = CampaignMarker::parse(&v)
                        && m.campaign_id == self.s.campaign_id
                        && self.wanted(&m.attempt_id)
                    {
                        out.push(MarkedProcess::from_stat(&st, m.attempt_id));
                    }
                }
                MarkerRead::Unreadable(w) => {
                    // A child of a stopped member descends from the campaign: never classified away.
                    self.hidden = vec![HiddenProcess {
                        pid: c,
                        start_ticks: st.start_ticks,
                        ppid: pid,
                        comm: st.comm.clone(),
                        why: format!(
                            "environ unreadable ({w}); a child of the stopped member {pid}"
                        ),
                    }];
                    let h = self.hidden.clone();
                    self.note_suspects(&h);
                    return Err(format!(
                        "environ of child {c} ({}) of the stopped member {pid} unreadable: {w}",
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
            if test_vanish_pid() == Some(p.pid) {
                // test builds: this candidate "exited before its stop" every time it is seen (it stays alive)
                self.vanished.insert(k, p);
                out.push(Ok(false));
                continue;
            }
            match guard::open_pidfd(p.pid) {
                Ok(fd) => cands.push((fd, p)),
                Err(Verdict::Gone) => {
                    self.vanished.insert(k, p);
                    out.push(Ok(false));
                }
                // procfix7 round 6 (gate10 M10-1 detail): the scanned MARKED process (this exact start, this marker) is
                // gone or changed identity before its stop (its pid reused: StartMismatch; an exec without the marker;
                // another uid): the marked incarnation vanished unproven; whatever holds the pid now is NOT signalled
                Err(Verdict::NotOurs(_)) => {
                    self.vanished.insert(k, p);
                    out.push(Ok(false));
                }
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
                Verdict::Gone => {
                    self.vanished.insert(k, p);
                    Ok(false)
                }
                // as above (M10-1 detail): reused pid / dropped marker between the scan and the stop = vanished, never
                // signalled (the guard refused the stop)
                Verdict::NotOurs(_) => {
                    self.vanished.insert(k, p);
                    Ok(false)
                }
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
                Ok(mut scan) => {
                    self.test_drop_known_suspects(&mut scan);
                    if scan.unreadable > 0 {
                        inconclusive = Some(format!(
                            "{} candidate(s) with an unreadable environ that may descend from the campaign: {}",
                            scan.unreadable,
                            describe_hidden(&scan.hidden)
                        ));
                    }
                    self.note_suspects(&scan.hidden);
                    // procfix7 round 7 (gate11 m11-mut M6): the suspects (the seeds: pids the ledger recorded; earlier
                    // unreadable candidates) this scan did not list are checked BY THEIR OWN PID; a marked one is a
                    // candidate like any other (stopped and killed through the guard), never only a reason to stay
                    // unproven
                    let reported: std::collections::BTreeSet<u32> = scan
                        .processes
                        .iter()
                        .map(|p| p.pid)
                        .chain(scan.hidden.iter().map(|h| h.pid))
                        .collect();
                    let (marked, hidden) = self.check_suspects(&reported);
                    if !hidden.is_empty() {
                        inconclusive = Some(format!(
                            "{} suspect(s) with an unreadable environ that may descend from the campaign: {}",
                            hidden.len(),
                            describe_hidden(&hidden)
                        ));
                    }
                    let mut all_hidden = scan.hidden;
                    all_hidden.extend(hidden);
                    self.hidden = all_hidden;
                    queue.extend(scan.processes);
                    queue.extend(marked);
                }
                Err(e) => inconclusive = Some(format!("proc scan: {e}")),
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

    /// procfix7 round 5 (gate9 minor: the mutation "drop the re-check of earlier suspects" was not caught). TEST BUILDS
    /// ONLY: with `sweep_scan_drops_suspects:1` in RR_CAMPAIGN_FAILPOINT a scan of this sweep OMITS every process that is
    /// already a suspect of an earlier scan (a later scan that misses one: a listing, a misread). Only the direct re-check
    /// of the suspects (`pass`) then still sees it (tests/procfix9.rs `g9_minor_a_suspect_missed_by_later_scans_...`).
    #[allow(unused_variables)]
    fn test_drop_known_suspects(&self, scan: &mut crate::resources::CampaignScan) {
        #[cfg(debug_assertions)]
        {
            static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
            let on = *ON.get_or_init(|| {
                std::env::var(crate::supervisor::failpoint::FAILPOINT_ENV).is_ok_and(|v| {
                    v.split(',')
                        .any(|p| p.trim() == "sweep_scan_drops_suspects:1")
                })
            });
            if on {
                let before = scan.hidden.len();
                scan.processes
                    .retain(|p| !self.suspects.contains_key(&p.pid));
                scan.hidden.retain(|h| !self.suspects.contains_key(&h.pid));
                scan.unreadable = scan.unreadable.saturating_sub(before - scan.hidden.len());
            }
        }
    }

    /// The suspects (seeds and earlier unreadable candidates) that `reported` (a scan's listing) lacks, each checked by
    /// its OWN pid: (marked ones: candidates; unreadable ones: still possibly ours). Not candidates / not ours: dropped.
    fn check_suspects(
        &mut self,
        reported: &std::collections::BTreeSet<u32>,
    ) -> (Vec<MarkedProcess>, Vec<HiddenProcess>) {
        let root = crate::resources::procfs::sys();
        let only = self.s.only_attempt.clone();
        let wanted = |a: &str| only.as_deref().is_none_or(|o| o == a);
        let pending: Vec<(u32, Option<u64>)> = self
            .suspects
            .iter()
            .filter(|(p, _)| !reported.contains(p))
            .map(|(p, s)| (*p, *s))
            .collect();
        let (mut marked, mut hidden) = (Vec::new(), Vec::new());
        for (pid, start) in pending {
            match check_candidate(
                &root,
                pid,
                start,
                &self.s.campaign_id,
                self.s.not_before_ticks,
                &wanted,
                STRICT_ENVIRON_TIMEOUT,
            ) {
                CandidateCheck::NotCandidate | CandidateCheck::NotOurs => {
                    self.suspects.remove(&pid);
                }
                CandidateCheck::Marked(m) => marked.push(m),
                CandidateCheck::Hidden(mut h) => {
                    h.why = format!("{} (a suspect of an earlier scan of this sweep)", h.why);
                    hidden.push(h);
                }
            }
        }
        (marked, hidden)
    }

    /// One proof scan: Ok(marked processes alive that are not in `left`) when conclusive. procfix8 (item 4): the
    /// suspects of earlier scans that this scan did not report are checked directly; one that is still possibly ours
    /// keeps the pass inconclusive, one that is marked is a survivor.
    fn pass(&mut self) -> Result<Vec<MarkedProcess>, String> {
        let mut scan = scan_marked_inline(
            &self.s.campaign_id,
            self.s.not_before_ticks,
            self.s.only_attempt.as_deref(),
        )
        .map_err(|e| format!("proc scan: {e}"))?;
        self.test_drop_known_suspects(&mut scan);
        self.note_suspects(&scan.hidden);
        let reported: std::collections::BTreeSet<u32> = scan
            .processes
            .iter()
            .map(|p| p.pid)
            .chain(scan.hidden.iter().map(|h| h.pid))
            .collect();
        let (marked, hidden) = self.check_suspects(&reported);
        scan.processes.extend(marked);
        scan.unreadable += hidden.len();
        scan.hidden.extend(hidden);
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
    sweep_seeded(spec, &[], cancel)
}

/// procfix7 round 6 (gate10 G10-B1 rule (3), M10-1): a sweep whose SUSPECTS start with `seeds` (pid, start time if
/// recorded): processes the ledger recorded (job leaders of this boot, marked survivors, vanished processes of earlier
/// sweeps). Each is proven dead or not ours BY ITS OWN PID (stat: ENOENT / ESRCH, another start time, unmarked, another
/// uid: `check_candidate`), never by its absence from a listing; one still possibly ours keeps the sweep unproven.
pub fn sweep_seeded(
    spec: &SweepSpec,
    seeds: &[(u32, Option<u64>)],
    cancel: &AtomicBool,
) -> SweepReport {
    guard::with_context("campaign sweep", spec.only_attempt.as_deref(), || {
        sweep_inner(spec, seeds, cancel)
    })
}

fn sweep_inner(spec: &SweepSpec, seeds: &[(u32, Option<u64>)], cancel: &AtomicBool) -> SweepReport {
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
        suspects: BTreeMap::new(),
        vanished: BTreeMap::new(),
    };
    for &(pid, start) in seeds {
        w.suspects.entry(pid).or_insert(start);
    }
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
    let vanished: Vec<MarkedProcess> = std::mem::take(&mut w.vanished)
        .into_iter()
        .filter(|(k, _)| !w.killed.contains_key(k) && !w.killed_by_hold.contains_key(k))
        .map(|(_, p)| p)
        .collect();
    // procfix7 round 6 (gate10 M10-1): a sweep in which a marked process was seen alive and exited before its stop is
    // NEVER proven (a successor it started may run unseen); the caller keeps the end unproven until a LATER sweep, seeded
    // with these pids, is proven
    let reason = match reason {
        None if !vanished.is_empty() => Some(format!(
            "marked process(es) {:?} were seen alive and exited before their stop (a successor may run unseen)",
            vanished.iter().map(|p| p.pid).collect::<Vec<_>>()
        )),
        r => r,
    };
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
        vanished,
    }
}

#[allow(clippy::let_underscore_must_use)] // test cleanup (outside the reviewed scope, procfix7)
#[allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code
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
        while crate::resources::read_stat(pid).unwrap().state != 'T' {
            assert!(t0.elapsed() < Duration::from_secs(10));
            std::thread::sleep(Duration::from_millis(5));
        }
        let st = crate::resources::read_stat(pid).unwrap();
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
            suspects: BTreeMap::new(),
            vanished: BTreeMap::new(),
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
            crate::resources::read_stat(pid).unwrap().state,
            'T',
            "neither killed nor continued (the hold is unusable)"
        );
        STRANDED.lock().unwrap().retain(|(c, _)| c != id);
        let _ = c.kill();
        let _ = c.wait();
    }
}
