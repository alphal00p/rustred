//! Scripted in-memory launcher for deterministic scheduler tests: no processes, virtual time.
//!
//! Time: [`TimeSource`] is this package's minimal time abstraction. TODO(merge with the ledger package's `clock.rs`):
//! once its manual clock exists, implement `TimeSource` for it (one `now() -> Duration` method) and drop
//! [`ManualTime`]; nothing else here depends on the concrete clock.
//!
//! Semantics of one scripted child: it exits by itself at `launched + run_for` with `exit`; `terminate(grace)` at t:
//! `OnTerm::ExitAfter { delay, exit }` exits at `t + delay` if `delay <= grace`, else it is SIGKILLed at `t + grace`;
//! `OnTerm::Ignore` is SIGKILLed at `t + grace`; `kill_now` kills at once. Memory: `rss` is a step function of the time
//! since launch; when it exceeds the spec's cap at a poll, the child is killed with `KillReason::MemoryCap` (the real
//! launcher's sampling rule). Whichever event comes first wins. Every launch, signal and end is logged as a
//! [`FakeEvent`] with its virtual time.

use super::{
    ChildHandle, ChildLauncher, ChildProgress, ExitState, FinalReport, KillReason, LaunchError,
    LaunchSpec, LostReason, MemoryMechanism, ReattachChoice, Reattached, SignalOutcome,
};
use crate::resources::{ProcessIdentity, SessionUsage};
use std::collections::{BTreeMap, VecDeque};
use std::io;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Minimal clock: time since an arbitrary origin.
pub trait TimeSource: Send + Sync {
    fn now(&self) -> Duration;
}

/// Manual virtual time (tests advance it explicitly).
#[derive(Debug, Default)]
pub struct ManualTime {
    nanos: AtomicU64,
}

impl ManualTime {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn advance(&self, d: Duration) {
        self.nanos.fetch_add(d.as_nanos() as u64, Ordering::SeqCst);
    }
    pub fn set(&self, t: Duration) {
        self.nanos.store(t.as_nanos() as u64, Ordering::SeqCst);
    }
}

impl TimeSource for ManualTime {
    fn now(&self) -> Duration {
        Duration::from_nanos(self.nanos.load(Ordering::SeqCst))
    }
}

/// Real monotonic time from a fixed origin.
#[derive(Debug)]
pub struct MonotonicTime {
    origin: Instant,
}

impl Default for MonotonicTime {
    fn default() -> Self {
        Self {
            origin: Instant::now(),
        }
    }
}

impl TimeSource for MonotonicTime {
    fn now(&self) -> Duration {
        self.origin.elapsed()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OnTerm {
    ExitAfter { delay: Duration, exit: ExitState },
    Ignore,
}

/// What one scripted child does.
#[derive(Clone, Debug)]
pub struct Script {
    pub run_for: Duration,
    pub exit: ExitState,
    pub on_sigterm: OnTerm,
    /// (offset since launch, session RSS) steps, sorted by offset.
    pub rss: Vec<(Duration, u64)>,
    /// Fail the launch itself with this message.
    pub launch_error: Option<String>,
}

impl Script {
    /// Exits with `code` after `after`; on SIGTERM exits with 30 (contract INTERRUPTED) after 100 ms.
    pub fn exits(code: i32, after: Duration) -> Self {
        Self {
            run_for: after,
            exit: ExitState::Exited(code),
            on_sigterm: OnTerm::ExitAfter {
                delay: Duration::from_millis(100),
                exit: ExitState::Exited(30),
            },
            rss: Vec::new(),
            launch_error: None,
        }
    }
    pub fn ignoring_term(mut self) -> Self {
        self.on_sigterm = OnTerm::Ignore;
        self
    }
    pub fn on_term(mut self, delay: Duration, exit: ExitState) -> Self {
        self.on_sigterm = OnTerm::ExitAfter { delay, exit };
        self
    }
    pub fn rss_step(mut self, at: Duration, bytes: u64) -> Self {
        self.rss.push((at, bytes));
        self.rss.sort_by_key(|x| x.0);
        self
    }
    pub fn failing_launch(msg: &str) -> Self {
        let mut s = Self::exits(0, Duration::ZERO);
        s.launch_error = Some(msg.into());
        s
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FakeEventKind {
    Launched {
        cpus: Vec<usize>,
        memory_cap_bytes: Option<u64>,
    },
    LaunchFailed(String),
    Signal(i32),
    Terminate {
        grace: Duration,
    },
    KillNow,
    Finished {
        exit: ExitState,
        kill_reason: Option<KillReason>,
    },
    Reattached,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FakeEvent {
    pub at: Duration,
    pub label: String,
    pub pid: u32,
    pub kind: FakeEventKind,
}

const FAKE_BOOT: &str = "scripted";

struct Proc {
    label: String,
    identity: ProcessIdentity,
    script: Script,
    launched_at: Duration,
    cap: Option<u64>,
    term: Option<(Duration, Duration)>,
    killed_at: Option<Duration>,
    kill_reason: Option<KillReason>,
    sigterm_sent: bool,
    report: Option<FinalReport>,
}

impl Proc {
    fn rss_at(&self, now: Duration) -> u64 {
        let since = now.saturating_sub(self.launched_at);
        self.script
            .rss
            .iter()
            .take_while(|(t, _)| *t <= since)
            .last()
            .map_or(0, |x| x.1)
    }
    fn peak_until(&self, now: Duration) -> u64 {
        let since = now.saturating_sub(self.launched_at);
        self.script
            .rss
            .iter()
            .take_while(|(t, _)| *t <= since)
            .map(|x| x.1)
            .max()
            .unwrap_or(0)
    }
    /// Earliest end: (time, exit, was it a SIGKILL).
    fn end(&self) -> (Duration, ExitState, bool) {
        let mut best = (
            self.launched_at + self.script.run_for,
            self.script.exit,
            false,
        );
        let sigkill = ExitState::Signaled {
            signal: libc::SIGKILL,
            core_dumped: false,
        };
        if let Some((t, grace)) = self.term {
            let cand = match &self.script.on_sigterm {
                OnTerm::ExitAfter { delay, exit } if *delay <= grace => (t + *delay, *exit, false),
                _ => (t + grace, sigkill, true),
            };
            if cand.0 < best.0 {
                best = cand;
            }
        }
        if let Some(k) = self.killed_at
            && k < best.0
        {
            best = (k, sigkill, true);
        }
        best
    }
}

type Registry = Arc<Mutex<BTreeMap<u32, Arc<Mutex<Proc>>>>>;

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

type Chooser = Box<dyn FnMut(&LaunchSpec) -> Script + Send>;

/// The fake launcher. Scripts are taken in order, or chosen per spec by a closure.
pub struct ScriptedLauncher {
    time: Arc<dyn TimeSource>,
    scripts: VecDeque<Script>,
    chooser: Option<Chooser>,
    events: Arc<Mutex<Vec<FakeEvent>>>,
    registry: Registry,
    next_pid: u32,
}

impl ScriptedLauncher {
    pub fn new(time: Arc<dyn TimeSource>, scripts: impl IntoIterator<Item = Script>) -> Self {
        Self {
            time,
            scripts: scripts.into_iter().collect(),
            chooser: None,
            events: Arc::new(Mutex::new(Vec::new())),
            registry: Arc::new(Mutex::new(BTreeMap::new())),
            next_pid: 1_000_000,
        }
    }

    pub fn with_chooser(
        time: Arc<dyn TimeSource>,
        chooser: impl FnMut(&LaunchSpec) -> Script + Send + 'static,
    ) -> Self {
        let mut s = Self::new(time, []);
        s.chooser = Some(Box::new(chooser));
        s
    }

    /// Every event so far, in order.
    pub fn events(&self) -> Vec<FakeEvent> {
        lock(&self.events).clone()
    }

    /// Children not finished yet (by label).
    pub fn live_labels(&self) -> Vec<String> {
        lock(&self.registry)
            .values()
            .filter_map(|p| {
                let p = lock(p);
                p.report.is_none().then(|| p.label.clone())
            })
            .collect()
    }

    fn handle(&self, proc_: Arc<Mutex<Proc>>) -> ScriptedChild {
        let (label, identity) = {
            let p = lock(&proc_);
            (p.label.clone(), p.identity.clone())
        };
        ScriptedChild {
            proc_,
            time: Arc::clone(&self.time),
            events: Arc::clone(&self.events),
            label,
            identity,
        }
    }
}

impl ChildLauncher for ScriptedLauncher {
    type Handle = ScriptedChild;

    fn launch(&mut self, spec: &LaunchSpec) -> Result<ScriptedChild, LaunchError> {
        spec.validate()?;
        let now = self.time.now();
        let script = match self.chooser.as_mut() {
            Some(f) => f(spec),
            None => self
                .scripts
                .pop_front()
                .ok_or_else(|| LaunchError::Scripted("no script left".into()))?,
        };
        if let Some(msg) = &script.launch_error {
            lock(&self.events).push(FakeEvent {
                at: now,
                label: spec.label.clone(),
                pid: 0,
                kind: FakeEventKind::LaunchFailed(msg.clone()),
            });
            return Err(LaunchError::Scripted(msg.clone()));
        }
        let pid = self.next_pid;
        self.next_pid += 1;
        let identity = ProcessIdentity {
            pid,
            start_ticks: (now.as_millis() / 10) as u64,
            boot_id: FAKE_BOOT.into(),
        };
        lock(&self.events).push(FakeEvent {
            at: now,
            label: spec.label.clone(),
            pid,
            kind: FakeEventKind::Launched {
                cpus: spec.cpus.clone(),
                memory_cap_bytes: spec.memory_cap_bytes,
            },
        });
        let p = Arc::new(Mutex::new(Proc {
            label: spec.label.clone(),
            identity,
            script,
            launched_at: now,
            cap: spec.memory_cap_bytes,
            term: None,
            killed_at: None,
            kill_reason: None,
            sigterm_sent: false,
            report: None,
        }));
        lock(&self.registry).insert(pid, Arc::clone(&p));
        Ok(self.handle(p))
    }

    fn reattach(
        &mut self,
        recorded: &ProcessIdentity,
        label: &str,
        choice: ReattachChoice,
    ) -> io::Result<Reattached<ScriptedChild>> {
        if recorded.boot_id != FAKE_BOOT {
            return Ok(Reattached::Lost(LostReason::OtherBoot));
        }
        let Some(p) = lock(&self.registry).get(&recorded.pid).cloned() else {
            return Ok(Reattached::Lost(LostReason::Gone));
        };
        {
            let g = lock(&p);
            if g.identity.start_ticks != recorded.start_ticks {
                return Ok(Reattached::Lost(LostReason::Recycled));
            }
            if g.report.is_some() || g.end().0 <= self.time.now() {
                return Ok(Reattached::Lost(LostReason::Gone));
            }
        }
        lock(&self.events).push(FakeEvent {
            at: self.time.now(),
            label: label.into(),
            pid: recorded.pid,
            kind: FakeEventKind::Reattached,
        });
        let mut h = self.handle(p);
        if let ReattachChoice::Kill { grace } = choice {
            h.terminate(grace, KillReason::Reattach);
        }
        Ok(Reattached::Live(h))
    }
}

/// A scripted child.
pub struct ScriptedChild {
    proc_: Arc<Mutex<Proc>>,
    time: Arc<dyn TimeSource>,
    events: Arc<Mutex<Vec<FakeEvent>>>,
    label: String,
    identity: ProcessIdentity,
}

impl ScriptedChild {
    fn log(&self, kind: FakeEventKind) {
        lock(&self.events).push(FakeEvent {
            at: self.time.now(),
            label: self.label.clone(),
            pid: self.identity.pid,
            kind,
        });
    }
}

impl ChildHandle for ScriptedChild {
    fn label(&self) -> &str {
        &self.label
    }

    fn identity(&self) -> &ProcessIdentity {
        &self.identity
    }

    fn poll_exit(&mut self) -> io::Result<Option<ExitState>> {
        let now = self.time.now();
        let p = lock(&self.proc_);
        let (t, e, _) = p.end();
        Ok((t <= now).then_some(e))
    }

    fn usage(&self) -> Option<SessionUsage> {
        let now = self.time.now();
        let p = lock(&self.proc_);
        Some(SessionUsage {
            session: p.identity.pid,
            rss_bytes: p.rss_at(now),
            peak_rss_bytes: p.peak_until(now),
            ..Default::default()
        })
    }

    fn signal_group(&mut self, signal: i32) -> io::Result<SignalOutcome> {
        let now = self.time.now();
        let alive = {
            let p = lock(&self.proc_);
            p.report.is_none() && p.end().0 > now
        };
        if !alive {
            return Ok(SignalOutcome::AlreadyGone);
        }
        self.log(FakeEventKind::Signal(signal));
        if signal == libc::SIGKILL {
            let mut p = lock(&self.proc_);
            p.killed_at.get_or_insert(now);
        }
        Ok(SignalOutcome::Delivered { groups: 1 })
    }

    fn terminate(&mut self, grace: Duration, reason: KillReason) {
        let now = self.time.now();
        {
            let mut p = lock(&self.proc_);
            if p.report.is_some() {
                return;
            }
            p.kill_reason.get_or_insert(reason);
            p.sigterm_sent = true;
            let deadline = now + grace;
            p.term = Some(match p.term {
                Some((t, g)) if t + g <= deadline => (t, g),
                Some((t, _)) => (t, deadline.saturating_sub(t)),
                None => (now, grace),
            });
        }
        self.log(FakeEventKind::Terminate { grace });
    }

    fn kill_now(&mut self, reason: KillReason) {
        let now = self.time.now();
        {
            let mut p = lock(&self.proc_);
            if p.report.is_some() {
                return;
            }
            p.kill_reason.get_or_insert(reason);
            p.killed_at.get_or_insert(now);
        }
        self.log(FakeEventKind::KillNow);
    }

    fn poll(&mut self) -> io::Result<ChildProgress> {
        let now = self.time.now();
        let mut p = lock(&self.proc_);
        if let Some(r) = &p.report {
            return Ok(ChildProgress::Finished(r.clone()));
        }
        // Sampling cap rule.
        if let Some(cap) = p.cap {
            let rss = p.rss_at(now);
            if rss > cap
                && p.end().0 > now
                && !matches!(p.kill_reason, Some(KillReason::MemoryCap { .. }))
            {
                p.kill_reason = Some(KillReason::MemoryCap {
                    rss_bytes: rss,
                    cap_bytes: cap,
                });
                p.killed_at = Some(now);
            }
        }
        let (t, exit, sigkill) = p.end();
        if t > now {
            return Ok(if p.term.is_some() || p.killed_at.is_some() {
                ChildProgress::Stopping
            } else {
                ChildProgress::Running
            });
        }
        let report = FinalReport {
            label: p.label.clone(),
            identity: p.identity.clone(),
            marker: crate::resources::CampaignMarker {
                campaign_id: "fake".into(),
                attempt_id: p.label.clone(),
            },
            exit: Some(exit),
            kill_reason: p.kill_reason.clone(),
            sigterm_sent: p.sigterm_sent,
            sigkill_sent: sigkill,
            leftovers_killed: Vec::new(),
            escaped: Vec::new(),
            unattributed_children: Vec::new(),
            session_verified_empty: true,
            verification: super::Verification::Verified {
                by: super::VerifiedBy::Scripted,
            },
            peak_rss_bytes: p.peak_until(t),
            cpu_seconds: t.saturating_sub(p.launched_at).as_secs_f64(),
            wall_seconds: t.saturating_sub(p.launched_at).as_secs_f64(),
            memory_mechanism: if p.cap.is_some() {
                MemoryMechanism::Sampling
            } else {
                MemoryMechanism::None
            },
            rlimit_as_bytes: None,
        };
        p.report = Some(report.clone());
        let kill_reason = p.kill_reason.clone();
        drop(p);
        self.log(FakeEventKind::Finished { exit, kill_reason });
        Ok(ChildProgress::Finished(report))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn spec(label: &str) -> LaunchSpec {
        LaunchSpec::new(label, "/nonexistent/rr_node", Path::new("/nonexistent/run"))
    }

    fn finished(h: &mut ScriptedChild) -> Option<FinalReport> {
        match h.poll().unwrap() {
            ChildProgress::Finished(r) => Some(r),
            _ => None,
        }
    }

    #[test]
    fn natural_exit_graceful_stop_ignore_and_cap() {
        let t = Arc::new(ManualTime::new());
        let mut l = ScriptedLauncher::new(
            t.clone(),
            [
                Script::exits(0, Duration::from_secs(10)),
                Script::exits(0, Duration::from_secs(100)),
                Script::exits(0, Duration::from_secs(100)).ignoring_term(),
                Script::exits(0, Duration::from_secs(100)).rss_step(Duration::from_secs(5), 2_000),
            ],
        );
        let mut a = l.launch(&spec("a")).unwrap();
        let mut b = l.launch(&spec("b")).unwrap();
        let mut c = l.launch(&spec("c")).unwrap();
        let mut sd = spec("d");
        sd.memory_cap_bytes = Some(1_000);
        let mut d = l.launch(&sd).unwrap();
        t.advance(Duration::from_secs(1));
        b.terminate(Duration::from_secs(20), KillReason::Stop);
        c.terminate(Duration::from_secs(20), KillReason::Stop);
        assert!(matches!(b.poll().unwrap(), ChildProgress::Stopping));
        t.advance(Duration::from_secs(5)); // t = 6 s
        assert_eq!(finished(&mut b).unwrap().exit, Some(ExitState::Exited(30)));
        let rd = finished(&mut d).unwrap();
        assert_eq!(
            rd.supervisor_outcome(),
            Some(crate::launch::MEMORY_CAP_EXCEEDED)
        );
        assert!(finished(&mut a).is_none());
        t.advance(Duration::from_secs(4)); // 10 s
        assert_eq!(finished(&mut a).unwrap().exit, Some(ExitState::Exited(0)));
        assert!(finished(&mut c).is_none());
        t.advance(Duration::from_secs(11)); // 21 s = 1 + 20
        let rc = finished(&mut c).unwrap();
        assert_eq!(rc.exit.unwrap().signal(), Some(libc::SIGKILL));
        assert!(rc.sigkill_sent);
        assert!(l.live_labels().is_empty());
        assert!(l.launch(&spec("e")).is_err()); // no script left
    }

    #[test]
    fn reattach_rules() {
        let t = Arc::new(ManualTime::new());
        let mut l = ScriptedLauncher::new(t.clone(), [Script::exits(0, Duration::from_secs(50))]);
        let h = l.launch(&spec("a")).unwrap();
        let id = h.identity().clone();
        drop(h);
        let wrong = ProcessIdentity {
            start_ticks: id.start_ticks + 1,
            ..id.clone()
        };
        assert!(matches!(
            l.reattach(&wrong, "a", ReattachChoice::Adopt).unwrap(),
            Reattached::Lost(LostReason::Recycled)
        ));
        let other = ProcessIdentity {
            boot_id: "x".into(),
            ..id.clone()
        };
        assert!(matches!(
            l.reattach(&other, "a", ReattachChoice::Adopt).unwrap(),
            Reattached::Lost(LostReason::OtherBoot)
        ));
        let Reattached::Live(mut k) = l
            .reattach(
                &id,
                "a",
                ReattachChoice::Kill {
                    grace: Duration::from_secs(2),
                },
            )
            .unwrap()
        else {
            panic!("expected live")
        };
        t.advance(Duration::from_secs(1));
        let r = finished(&mut k).unwrap();
        assert_eq!(r.exit, Some(ExitState::Exited(30)));
        assert_eq!(r.kill_reason, Some(KillReason::Reattach));
        assert!(matches!(
            l.reattach(&id, "a", ReattachChoice::Adopt).unwrap(),
            Reattached::Lost(LostReason::Gone)
        ));
    }
}
