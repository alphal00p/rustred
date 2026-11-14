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
//! 3. TWO consecutive CONCLUSIVE scans (`pass_gap` apart) that find no marked process (other than the ones the narrowing
//!    forbids signalling, reported as `left`). A scan with an unreadable candidate or a failed listing is inconclusive.
//!    A survivor sends the sweep back to 1 while the budget lasts.
//!
//! Ownership is never widened here: what may be signalled is exactly what `guard::signal_with_pidfd` accepts for a
//! campaign marker; the caller's `narrowing` can only refuse more (the supervisor's cwd rule, coord-site-policy rule 5).
//! Nothing is ever selected by name or pattern. Runs on the calling thread (blocking, bounded by `budget` plus the reads
//! of one round): call it off the control loop.

use super::guard::{self, ProcessTarget, Proof, StartRule, Verdict};
use crate::resources::{
    CampaignMarker, FactRead, MarkedProcess, MarkerRead, boot_id, children_of, our_uid,
    owner_uid_at, read_marker_inline, read_proc_facts, read_stat, request_reap,
    scan_marked_inline,
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
}

impl SweepReport {
    /// Proven AND nothing left alive: no live process carries the swept marker.
    pub fn empty(&self) -> bool {
        self.proven && self.left.is_empty()
    }
}

type Key = (u32, u64);

struct Member {
    fd: OwnedFd,
    process: MarkedProcess,
}

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
    rounds: u32,
}

fn key(p: &MarkedProcess) -> Key {
    (p.pid, p.start_ticks)
}

impl Sweep<'_> {
    fn out_of_time(&self) -> bool {
        self.cancel.load(Ordering::SeqCst) || Instant::now() >= self.deadline
    }

    fn proof(&self) -> Proof {
        match &self.s.only_attempt {
            Some(a) => match CampaignMarker::new(&self.s.campaign_id, a) {
                Ok(m) => Proof::Marker(m),
                Err(_) => Proof::Campaign(self.s.campaign_id.clone()),
            },
            None => Proof::Campaign(self.s.campaign_id.clone()),
        }
    }

    fn target(&self, p: &MarkedProcess) -> ProcessTarget {
        ProcessTarget {
            pid: p.pid,
            boot_id: self.boot.clone(),
            start: StartRule::Exact(p.start_ticks),
            require_session_leader: false,
            proof: self.proof(),
        }
    }

    /// Does this marked process match what we sweep?
    fn wanted(&self, attempt: &str) -> bool {
        self.s.only_attempt.as_deref().is_none_or(|o| o == attempt)
    }

    /// Marked children of a STOPPED member (its list is stable). Unreadable facts of a child: Err (inconclusive).
    fn marked_children(&self, pid: u32) -> Result<Vec<MarkedProcess>, String> {
        let mut out = Vec::new();
        let Some(kids) = children_of(pid)? else {
            return Ok(out);
        };
        for c in kids {
            if owner_uid_at(Path::new("/proc"), c) != Some(self.uid) {
                continue;
            }
            let st = match read_stat(c) {
                Ok(Some(st)) => st,
                Ok(None) => continue,
                Err(e) => return Err(format!("stat of child {c}: {e}")),
            };
            if st.start_ticks < self.s.not_before_ticks || st.is_dead() {
                continue;
            }
            match read_marker_inline(c, None) {
                MarkerRead::Marker(v) => {
                    if let Some(m) = CampaignMarker::parse(&v)
                        && m.campaign_id == self.s.campaign_id
                        && self.wanted(&m.attempt_id)
                    {
                        out.push(MarkedProcess::from_stat(&st, m.attempt_id));
                    }
                }
                MarkerRead::Unreadable => return Err(format!("environ of child {c} unreadable")),
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

    /// Stop one candidate through the guard. Ok(true): a new member; Ok(false): gone / not ours any more; Err: could
    /// not decide (inconclusive).
    fn stop_candidate(&mut self, p: MarkedProcess) -> Result<bool, String> {
        let k = key(&p);
        if self.members.contains_key(&k) {
            return Ok(false);
        }
        self.seen.insert(k, ());
        if let Some(n) = &self.s.narrowing
            && let Err(why) = n(&p)
        {
            self.left.insert(k, LeftMarked { process: p, why });
            return Ok(false);
        }
        let fd = match guard::open_pidfd(p.pid) {
            Ok(fd) => fd,
            Err(Verdict::Gone) => return Ok(false),
            Err(Verdict::NotOurs(_)) => return Ok(false),
            Err(Verdict::Unverified(e)) => return Err(e),
            Err(Verdict::Delivered) => return Err("impossible verdict".into()),
        };
        let t = self.target(&p);
        match guard::signal_with_pidfd(&fd, &t, libc::SIGSTOP, None) {
            Verdict::Delivered => {
                // Kept as a member whatever the wait says (gate-A m-2): it will get the SIGKILL.
                let w = self.wait_stopped(p.pid, &fd);
                self.left.remove(&k);
                self.members.insert(k, Member { fd, process: p });
                w.map(|()| true)
            }
            Verdict::Gone => Ok(false),
            // The pid was recycled, the environment changed (exec without the marker), another uid: not ours now.
            Verdict::NotOurs(_) => Ok(false),
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
        }
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
                            "{} candidate(s) with an unreadable environ",
                            scan.unreadable
                        ));
                    }
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
            for p in queue {
                match self.stop_candidate(p) {
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
                        new = true;
                        let t = self.target(&m.process);
                        if let Verdict::Unverified(e) =
                            guard::signal_with_pidfd(&m.fd, &t, libc::SIGSTOP, None)
                        {
                            inconclusive = Some(e);
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
        for (k, m) in members {
            let t = self.target(&m.process);
            match guard::signal_with_pidfd(&m.fd, &t, libc::SIGKILL, None) {
                Verdict::Delivered => {
                    self.killed.insert(k, m.process.clone());
                    pending.push(m);
                }
                Verdict::Gone => {}
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
        let t0 = Instant::now();
        for m in pending {
            while guard::pidfd_exited(&m.fd) != Some(true) && t0.elapsed() < Duration::from_secs(2)
            {
                std::thread::sleep(Duration::from_micros(500));
            }
        }
        request_reap();
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
        if scan.unreadable > 0 {
            return Err(format!(
                "{} candidate(s) with an unreadable environ",
                scan.unreadable
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

/// Run one sweep (blocking; call it off the control loop). Signals only through `guard` (see the module documentation).
pub fn sweep(spec: &SweepSpec, cancel: &AtomicBool) -> SweepReport {
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
        rounds: 0,
    };
    let reason = loop {
        // A freeze that hit the bound still ends in the SIGKILL of every member; the proof below decides.
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
    SweepReport {
        killed: w.killed.into_values().collect(),
        left: w.left.into_values().collect(),
        proven: reason.is_none(),
        reason,
        rounds: w.rounds,
        seen: w.seen.len(),
    }
}
