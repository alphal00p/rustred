//! The supervisor run loop (WP-I6; interfaces/supervisor.md). One control thread owns the ledger (the only truth), the
//! children and all state; the sampler is the only helper thread. Start and resume are the same entry point [`run`].
//!
//! Write-ahead order (section 4 of the interface): the run directory and `job.json` exist before the `job.launch`
//! record; the child is spawned after it; `job.started` names its identity; at the end the classification is computed
//! from the run directory, accepted record files are written atomically, and ONE group commit records `job.finished`
//! together with the work source's routing events. Every crash point is recovered by [`Supervisor::recover`].

pub mod failpoint;
pub mod identity;
pub mod orphans;
pub mod result;
pub mod spec;
pub mod worksource;

use crate::adapters::{self, AdapterInput, AdapterKind};
use crate::admission::{self, AdmissionPolicy, Candidate, Decision, HostView, RunningJob};
use crate::clock::{Clock, SystemClock};
use crate::cpu_policy::{
    Acquire, CpuLease, CpuPolicy, CpuSpec, PlainPolicy, SiteConfig, SitePolicy,
};
use crate::launch::{
    ChildHandle, ChildLauncher, ChildProgress, ExitState, FinalReport, KillReason, LaunchError,
    RealChild, RealLauncher, SweepReport,
};
use crate::ledger::{
    CampaignMode, ControlEvent, ControlTarget, ControlVerb, Event, FileOps, FileRef, Genesis,
    JobEvent, Ledger, LedgerConfig, LedgerError, OpenOptions, OutcomeBinding, RealOps,
};
use crate::lock::{self, CampaignLock, LockOutcome};
use crate::progress::{LiveJob, ProgressWriter, Resources as ProgressResources, SupervisorInfo};
use crate::resources::{Sampler, SamplerConfig, host_memory};
use crate::signals::{StopHandle, StopLevel, StopSignals};
use crate::state::{
    AttemptState, AttemptTarget, CampaignPhase, CampaignState, ContractAccept, FinishClass,
    class_target, class_word, contract_accepts,
};
use failpoint::Failpoints;
use orphans::ScanRules;

use rustred_campaign_contract::classify::{self, KillReason as CKill, OutcomeState, WaitStatus};
use rustred_campaign_contract::events::{Event as ChildEvent, EventsTail, TailItem};
use rustred_campaign_contract::layout::{RunLayout, attempt_dir};
use rustred_campaign_contract::resume::ResumeRef;
use rustred_campaign_contract::spec::JobSpec;
use spec::CampaignSpec;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use worksource::{JobRequest, ScriptedWorkSource, Settled, SourceContext, WorkSource};

/// Exit codes of `rr_campaign` (interfaces/supervisor.md section 11).
pub mod exit {
    pub const OK: i32 = 0;
    pub const REFUSED: i32 = 1;
    pub const USAGE: i32 = 2;
    pub const NOT_FOUND: i32 = 3;
    pub const VERIFY_DIFFERS: i32 = 4;
    pub const BINARY_CHANGED: i32 = 6;
    pub const MOVED: i32 = 7;
    /// Stopped for an operator decision: an attempt whose launch was journalled but never confirmed and whose execution
    /// could not be authenticated (requirement 05:23Z; `rr_campaign control CDIR resolve JOB`), and nothing else to do.
    pub const ATTENTION: i32 = 8;
    pub const PF_SPEC: i32 = 20;
    pub const PF_PROGRAM: i32 = 21;
    pub const PF_LICENCE: i32 = 22;
    pub const PF_MEMORY: i32 = 23;
    pub const PF_DISK: i32 = 24;
    pub const PF_LOCK: i32 = 25;
    pub const PF_INPUT: i32 = 26;
    pub const INTERNAL: i32 = 70;
    /// The SUPERVISOR ran out of a resource it needs to launch (EMFILE, ENFILE, ENOMEM, EAGAIN) a bounded number of times
    /// in a row (gate-B M3): stopped cleanly; free the resource (e.g. `ulimit -n`) and run the same command.
    pub const RESOURCES: i32 = 71;
    pub const DISK_FULL: i32 = 75;
    pub const DISK_FLOOR: i32 = 76;
    pub const SIGHUP: i32 = 129;
    pub const SIGINT: i32 = 130;
    pub const CONTROL_STOP: i32 = 140;
    pub const SIGTERM: i32 = 143;
}

/// The longest interval between two progress writes of a live supervisor (heartbeat.json; the requirement is at
/// least every 10 s).
pub const HEARTBEAT_EVERY: Duration = Duration::from_secs(5);

/// `RUN/sup/SPAWN`: written and fsynced right before the spawn (requirement 05:23Z): its ABSENCE proves that a journalled
/// launch was never spawned.
pub const SPAWN_SENTINEL: &str = "SPAWN";
/// `RUN/sup/CAMPAIGN`: the campaign's marker id, written before the launch record (run directory authentication).
pub const CAMPAIGN_ID_FILE: &str = "CAMPAIGN";

/// How a `run` ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunExit {
    Finished,
    Stopped {
        code: i32,
    },
    DiskFloor,
    DiskFull,
    BinaryChanged(String),
    /// The campaign directory was copied / moved / restored (its marker id differs from the ledger's).
    Moved(String),
    Refused(String),
    Internal(String),
    /// Stopped after repeated resource failures of the supervisor itself at launch (gate-B M3).
    Resources(String),
    /// Stopped for an operator decision (execution-unknown attempts block the remaining work).
    Attention(String),
}

impl RunExit {
    pub fn code(&self) -> i32 {
        match self {
            RunExit::Finished => exit::OK,
            RunExit::Stopped { code } => *code,
            RunExit::DiskFloor => exit::DISK_FLOOR,
            RunExit::DiskFull => exit::DISK_FULL,
            RunExit::BinaryChanged(_) => exit::BINARY_CHANGED,
            RunExit::Moved(_) => exit::MOVED,
            RunExit::Refused(_) => exit::REFUSED,
            RunExit::Internal(_) => exit::INTERNAL,
            RunExit::Resources(_) => exit::RESOURCES,
            RunExit::Attention(_) => exit::ATTENTION,
        }
    }
}

/// Options of `rr_campaign run`.
#[derive(Clone, Debug)]
pub struct RunOptions {
    pub spec: CampaignSpec,
    pub spec_blake3: String,
    pub cdir: PathBuf,
    pub plain: bool,
    pub fake_child: bool,
    pub accept_binary_change: bool,
    /// Continue in a copied / moved campaign directory: re-keys FUTURE jobs only; recorded processes are never ours.
    pub adopt_moved: bool,
    pub cores: Option<usize>,
    pub mem_total: Option<u64>,
    pub argv: Vec<String>,
    /// Tick of the control loop.
    pub tick: Duration,
    /// Operator override of a corrupt active ledger segment (`--accept-truncation-to SEQ`, ledger interface 8).
    pub accept_truncation_to: Option<u64>,
    /// Set while a dashboard owns the terminal: log lines go to `CDIR/log/supervisor.log` only (not stderr).
    pub quiet_stderr: Option<Arc<AtomicBool>>,
}

/// Plain log: `<utc> <LEVEL> <text>` on stderr and in CDIR/log/supervisor.log.
pub struct Log {
    file: Option<std::fs::File>,
    /// When set (a dashboard owns the terminal), lines go to the file only.
    quiet: Option<Arc<AtomicBool>>,
}

impl Log {
    pub fn open(cdir: &Path) -> Log {
        let dir = cdir.join("log");
        let _ = std::fs::create_dir_all(&dir);
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("supervisor.log"))
            .ok();
        Log { file, quiet: None }
    }
    /// Log lines go to the file only while `flag` is set.
    pub fn with_quiet(mut self, flag: Option<Arc<AtomicBool>>) -> Log {
        self.quiet = flag;
        self
    }
    pub fn line(&mut self, level: &str, text: &str) {
        let l = format!("{} {level} {text}\n", crate::cpu_policy::utc_now());
        if !self
            .quiet
            .as_ref()
            .is_some_and(|q| q.load(Ordering::SeqCst))
        {
            let _ = std::io::stderr().write_all(l.as_bytes());
        }
        if let Some(f) = &mut self.file
            && f.write_all(l.as_bytes()).is_err()
        {
            self.file = None;
        }
    }
}

/// Signals of the supervisor: the shared stop counter plus which signal came (for the exit code).
struct Signals {
    _stop: StopSignals,
    term: Arc<AtomicBool>,
    hup: Arc<AtomicBool>,
    handle: StopHandle,
    _ids: Vec<signal_hook::SigId>,
}

impl Signals {
    fn install() -> std::io::Result<Signals> {
        let stop = StopSignals::install()?;
        let term = Arc::new(AtomicBool::new(false));
        let hup = Arc::new(AtomicBool::new(false));
        let ids = vec![
            signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&term))?,
            signal_hook::flag::register(signal_hook::consts::SIGHUP, Arc::clone(&hup))?,
        ];
        Ok(Signals {
            handle: stop.handle(),
            _stop: stop,
            term,
            hup,
            _ids: ids,
        })
    }
}

struct Child {
    job: String,
    attempt: u32,
    item: String,
    handle: RealChild,
    lease: Option<CpuLease>,
    spec: JobSpec,
    spec_digest: String,
    layout: RunLayout,
    tail: EventsTail,
    last_event: Instant,
    started_seen: bool,
    launched_at: Instant,
    cap: u64,
    peak: u64,
    cores: usize,
    hb_timeout: Duration,
    seq: u64,
    stopping: bool,
    /// The child's latest progress phase word (from its `phase` / `heartbeat` events).
    phase: Option<String>,
    /// UTC of the child's latest event (its heartbeat for the progress files).
    last_event_utc: Option<String>,
    /// Wall budget of the job (`budgets.wall.job_s`), seconds.
    budget_s: Option<u64>,
}

/// A marker sweep running on a helper thread (never on the control loop: gate-A m-4).
struct SweepTask {
    rx: mpsc::Receiver<SweepReport>,
    cancel: Arc<AtomicBool>,
}

impl SweepTask {
    fn spawn(spec: crate::launch::SweepSpec) -> Option<SweepTask> {
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let c = Arc::clone(&cancel);
        std::thread::Builder::new()
            .name("rr-sweep".into())
            .spawn(move || {
                let _ = tx.send(crate::launch::sweep::sweep(&spec, &c));
            })
            .ok()?;
        Some(SweepTask { rx, cancel })
    }
    /// The report when the sweep has finished; `Err(())` when its thread died without one.
    fn poll(&self) -> Option<Result<SweepReport, ()>> {
        match self.rx.try_recv() {
            Ok(r) => Some(Ok(r)),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => Some(Err(())),
        }
    }
}

impl Drop for SweepTask {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::SeqCst);
    }
}

/// A finished attempt that is NOT settled yet (ac2-M1, gate-A M-1): settle only after the job's processes are proven
/// gone by the process layer (`Verified`; an unverified end waits in the launcher's quarantine) AND a marker sweep of the
/// attempt found no live marked process. The settle then reads the sealed table once, bound to the verified stream.
struct PendingSettle {
    job: String,
    attempt: u32,
    layout: RunLayout,
    spec: JobSpec,
    spec_digest: String,
    report: FinalReport,
    /// The end is not proven yet (Unverified): waiting for the quarantine's `Cleared`.
    awaiting_proof: bool,
    sweep: Option<SweepTask>,
    next_sweep: Instant,
    inconclusive: u32,
    survivors_noted: bool,
    /// The CPU lease and the memory reservation stay with the attempt until it is settled (the ledger shows it running
    /// until then; admission counts it).
    lease: Option<CpuLease>,
    cap: u64,
    cores: usize,
    seq: u64,
}

impl PendingSettle {
    fn label(&self) -> String {
        format!("{}:a{}", self.job, self.attempt)
    }
}

/// What blocks launching now (for logs and the next progress write).
#[derive(Default)]
struct Blockers {
    reasons: Vec<String>,
}

/// The supervisor of one campaign directory.
pub struct Supervisor {
    opts: RunOptions,
    cdir: PathBuf,
    work_root: PathBuf,
    clock: Arc<dyn Clock>,
    ledger: Ledger,
    lock: Option<CampaignLock>,
    source: Box<dyn WorkSource>,
    ctx: SourceContext,
    programs: BTreeMap<String, (PathBuf, String)>,
    sampler: Arc<Sampler>,
    launcher: RealLauncher,
    moved: bool,
    policy: Box<dyn CpuPolicy>,
    admission: AdmissionPolicy,
    signals: Signals,
    children: Vec<Child>,
    progress: ProgressWriter,
    last_progress: Option<(Instant, u64)>,
    fp: Failpoints,
    log: Log,
    bypassed: BTreeMap<String, u32>,
    launch_seq: u64,
    ledger_dead: Option<RunExit>,
    control_stop: bool,
    disk_floor_hit: bool,
    last_disk_check: Option<Instant>,
    last_block_log: String,
    last_wait_log: String,
    boot_id: String,
    started_at: Instant,
    started_utc: String,
    /// When the first stop request arrived (grace countdown in the heartbeat file).
    drain_started: Option<Instant>,
    /// (dev, inode) of CDIR and of CDIR/ledger when the ledger was opened (procfix3 item 0: a directory removed or
    /// replaced under a running supervisor is an internal error, never a finished campaign).
    dir_ids: Option<((u64, u64), (u64, u64))>,
    last_dir_check: Option<Instant>,
    /// Finished attempts waiting for proof + marker sweep before settle.
    pending: Vec<PendingSettle>,
    /// Attempts with a journalled but unconfirmed launch whose execution is UNKNOWN (requirement 05:23Z): never retried
    /// automatically; `control resolve JOB` requeues them. job -> attempt.
    unknown: BTreeMap<String, u32>,
    /// Start bound (clock ticks) of campaign-wide sweeps: this boot's first supervisor start of this campaign.
    campaign_min_ticks: Option<u64>,
    /// The periodic campaign-wide sweep for orphans of attempts that are not live (gate-B M1).
    periodic: Option<SweepTask>,
    last_periodic: Instant,
    /// Requests the work source offered in the last scheduling pass (0: nothing ready).
    last_ready: usize,
    /// Consecutive launch failures that were the supervisor's own resource problem (gate-B M3), and the back-off.
    resource_failures: u32,
    resource_backoff_until: Option<Instant>,
    resource_stop: Option<String>,
}

/// Bound of consecutive supervisor resource failures at launch before the campaign stops (exit 71).
pub const MAX_RESOURCE_FAILURES: u32 = 6;

/// The errno of a launch failure that is the SUPERVISOR's resource problem (gate-B M3), not the job's.
fn resource_errno(e: &LaunchError) -> Option<i32> {
    let io = match e {
        LaunchError::Spawn(e) | LaunchError::Identity(e) | LaunchError::Io { error: e, .. } => e,
        _ => return None,
    };
    io_resource_errno(io)
}

fn io_resource_errno(e: &std::io::Error) -> Option<i32> {
    match e.raw_os_error() {
        Some(c @ (libc::EMFILE | libc::ENFILE | libc::ENOMEM | libc::EAGAIN)) => Some(c),
        _ => None,
    }
}

fn errno_name(c: i32) -> &'static str {
    match c {
        libc::EMFILE => "EMFILE (too many open files)",
        libc::ENFILE => "ENFILE (file table full)",
        libc::ENOMEM => "ENOMEM",
        libc::EAGAIN => "EAGAIN (process limit?)",
        _ => "resource error",
    }
}

/// (dev, inode) of a directory (following symlinks), if it exists.
fn dir_id(p: &Path) -> Option<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(p)
        .ok()
        .filter(|m| m.is_dir())
        .map(|m| (m.dev(), m.ino()))
}

fn statvfs_avail(p: &Path) -> Option<u64> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let c = CString::new(p.as_os_str().as_bytes()).ok()?;
    // SAFETY: zeroed statvfs is a valid out-parameter; c is a valid C string.
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: as above.
    if unsafe { libc::statvfs(c.as_ptr(), &mut s) } != 0 {
        return None;
    }
    Some(s.f_bavail as u64 * s.f_frsize as u64)
}

/// BLAKE3 of a file.
pub fn file_digest(p: &Path) -> std::io::Result<(u64, String)> {
    crate::atomic::blake3_file(p)
}

fn contract_kill_reason(k: Option<&KillReason>) -> CKill {
    match k {
        None => CKill::None,
        Some(
            KillReason::Stop
            | KillReason::ForceStop
            | KillReason::Reattach
            | KillReason::HostEmergency,
        ) => CKill::Stop,
        Some(KillReason::MemoryCap { .. }) => CKill::Memory,
        Some(KillReason::Timeout) => CKill::Time,
        Some(KillReason::Unresponsive) => CKill::Unresponsive,
        Some(KillReason::Other(_)) => CKill::None,
    }
}

fn map_wait(e: Option<ExitState>) -> WaitStatus {
    match e {
        Some(ExitState::Exited(c)) => WaitStatus::Exited(c),
        Some(ExitState::Signaled { signal, .. }) => WaitStatus::Signaled(signal),
        _ => WaitStatus::Unknown,
    }
}

/// What a host (the CLI's dashboard) gets once the supervisor holds the lock and its signal handlers: the stop
/// handle (a Ctrl-C key in raw mode calls `request_stop()` on it: first = drain, second = force, like the signals)
/// and the drain grace.
#[derive(Clone, Debug)]
pub struct Started {
    pub handle: StopHandle,
    pub grace_s: u64,
}

/// Start or resume a campaign (the SAME command).
pub fn run(opts: RunOptions) -> RunExit {
    run_with(opts, &mut |_| {})
}

/// [`run`] with a hook called once the supervisor has started (before recovery and scheduling). The hook must not
/// block: the control loop runs on this thread; a dashboard runs on its own thread and only reads progress files.
pub fn run_with(opts: RunOptions, on_started: &mut dyn FnMut(&Started)) -> RunExit {
    let mut log = Log::open(&opts.cdir).with_quiet(opts.quiet_stderr.clone());
    match Supervisor::start(opts, &mut log) {
        Ok(mut s) => {
            s.log = log;
            on_started(&Started {
                handle: s.signals.handle.clone(),
                grace_s: s.opts.spec.resources.grace_seconds,
            });
            let r = s.main_loop();
            s.finish_run(&r);
            r
        }
        Err(e) => {
            log.line("ERROR", &format!("{e:?}"));
            e
        }
    }
}

impl Supervisor {
    fn start(opts: RunOptions, log: &mut Log) -> Result<Supervisor, RunExit> {
        let cdir = opts.cdir.clone();
        opts.spec.check_dirs(&cdir).map_err(RunExit::Refused)?;
        std::fs::create_dir_all(&cdir)
            .map_err(|e| RunExit::Refused(format!("create {}: {e}", cdir.display())))?;
        let work_root = opts
            .spec
            .dirs
            .work_dir
            .clone()
            .unwrap_or_else(|| cdir.clone());
        std::fs::create_dir_all(&work_root)
            .map_err(|e| RunExit::Refused(format!("create {}: {e}", work_root.display())))?;
        let clock: Arc<dyn Clock> = Arc::new(SystemClock::new());
        let signals = Signals::install().map_err(|e| RunExit::Internal(format!("signals: {e}")))?;
        let (lock, outcome) = lock::acquire(&cdir, clock.as_ref(), &opts.argv)
            .map_err(|e| RunExit::Refused(format!("lock: {e}")))?;
        let fp = Failpoints::from_env();
        if let Some(a) = fp.active() {
            log.line(
                "WARN",
                &format!("TEST BUILD: failpoint {a} active (RR_CAMPAIGN_FAILPOINT); release builds have none"),
            );
        }
        let cfg = LedgerConfig::default();
        let manifest = crate::ledger::ledger_dir(&cdir).join(crate::ledger::MANIFEST_FILE);
        let ops: Arc<dyn FileOps> = fp.ledger_ops().unwrap_or_else(|| Arc::new(RealOps));
        // Every write handle is opened with the HELD campaign lock (ledger interface 4, audit-ledger-A M6).
        let mut ledger = if manifest.exists() {
            let (l, rep) = Ledger::open_with(
                &cdir,
                &lock,
                Arc::clone(&clock),
                cfg,
                ops,
                OpenOptions {
                    accept_truncation_to: opts.accept_truncation_to,
                },
            )
            .map_err(|e| RunExit::Refused(format!("ledger: {e}")))?;
            if let Some(r) = rep.recovery {
                log.line(
                    "WARN",
                    &format!(
                        "ledger {} repaired: {} bytes after seq {} saved as {}",
                        if r.forced {
                            "tail (operator override)"
                        } else {
                            "torn tail"
                        },
                        r.truncated_bytes,
                        r.last_good_seq,
                        r.saved_as.as_deref().unwrap_or("-")
                    ),
                );
            }
            l
        } else {
            let mode = match opts.spec.mode.as_str() {
                "optimise" => CampaignMode::Optimise,
                _ => CampaignMode::Close,
            };
            let genesis = Genesis {
                mode,
                ..Genesis::new(
                    &opts.spec.campaign,
                    &opts.spec_blake3,
                    opts.spec.terminal_bound,
                )
            };
            Ledger::create_with(&cdir, &lock, Arc::clone(&clock), cfg, ops, genesis).map_err(
                |e| match e {
                    LedgerError::NoSpace { .. } => RunExit::DiskFull,
                    e => RunExit::Refused(format!("ledger: {e}")),
                },
            )?
        };
        for w in ledger.warnings() {
            log.line("WARN", &format!("ledger: {w}"));
        }
        let st = ledger.state();
        if st.spec_blake3 != opts.spec_blake3 || st.campaign_id != opts.spec.campaign {
            return Err(RunExit::Refused(format!(
                "spec digest differs from the directory's (campaign {}, spec {}): use the spec this campaign was created with",
                st.campaign_id,
                &st.spec_blake3[..16.min(st.spec_blake3.len())]
            )));
        }
        let mut first = Vec::new();
        if let LockOutcome::TookOver {
            previous,
            reason,
            orphans,
        } = outcome
        {
            log.line(
                "WARN",
                &format!(
                    "stale lock of pid {} taken over ({reason:?}); {} process(es) still hold its token",
                    previous.identity.pid,
                    orphans.len()
                ),
            );
            first.push(Event::LockTakeover {
                previous,
                reason,
                orphans,
            });
        }
        first.push(Event::Campaign {
            ev: crate::state::CampaignEvent::Start,
            reason: "supervisor session".into(),
        });
        first.push(Event::SupervisorStarted {
            identity: lock.owner.identity.clone(),
            argv: opts.argv.clone(),
            version: env!("CARGO_PKG_VERSION").into(),
            lock_token: lock.token().to_string(),
        });
        ledger.append_all(first).map_err(|e| match e {
            LedgerError::NoSpace { .. } => RunExit::DiskFull,
            e => RunExit::Refused(format!("ledger: {e}")),
        })?;
        // The marker campaign id (C-1): per directory, recorded in the ledger; a copy has another id.
        let key =
            identity::campaign_marker_id(&cdir, &opts.spec.campaign).map_err(RunExit::Refused)?;
        let mut moved = false;
        match identity::recorded_id(ledger.state()) {
            None => {
                ledger
                    .append(identity::id_event(&key))
                    .map_err(|e| RunExit::Refused(format!("ledger: {e}")))?;
            }
            Some(r) if r == key => {}
            Some(r) => {
                let msg = format!(
                    "campaign directory {} was copied, moved or restored: its marker id is {key}, the ledger recorded {r}; \
                     processes of the recorded attempts are NOT this directory's and are never signalled",
                    cdir.display()
                );
                if !opts.adopt_moved {
                    return Err(RunExit::Moved(format!(
                        "{msg}. Refused: pass --adopt-moved-campaign to continue here (recorded live attempts become lost)"
                    )));
                }
                log.line(
                    "WARN",
                    &format!("{msg}; continuing with --adopt-moved-campaign"),
                );
                ledger
                    .append(identity::id_event(&key))
                    .map_err(|e| RunExit::Refused(format!("ledger: {e}")))?;
                moved = true;
            }
        }
        // A transient probe failure (EMFILE ...) is not "unsupported" (gate re-audit m-2/m-3): only a permanent answer refuses.
        if crate::launch::pidfd_probe() == crate::launch::PidfdProbe::Unsupported
            && !opts.spec.resources.allow_no_pidfd
        {
            return Err(RunExit::Refused(
                "this kernel offers no pidfd (verified signals impossible); refused unless resources.allow_no_pidfd".into(),
            ));
        }
        let sampler = Arc::new(
            Sampler::spawn(SamplerConfig {
                interval: Duration::from_millis(opts.spec.resources.sampler_ms.max(100)),
                collect_foreign_pinning: false,
            })
            .map_err(|e| RunExit::Internal(format!("sampler: {e}")))?,
        );
        let launcher = RealLauncher::try_with_config(
            Arc::clone(&sampler),
            crate::launch::RealConfig {
                campaign_id: key.clone(),
                ..crate::launch::RealConfig::default()
            },
        )
        .map_err(|e| RunExit::Refused(format!("marker id {key:?}: {e}")))?;
        let mut pause_files = opts.spec.resources.pause_files.clone();
        pause_files.push(cdir.join("control").join("PAUSED"));
        let policy: Box<dyn CpuPolicy> = match opts.spec.resources.policy.as_str() {
            "site" => {
                let path = opts.spec.resources.site_policy.clone().unwrap_or_default();
                let mut cfg =
                    SiteConfig::read(&path).map_err(|e| RunExit::Refused(e.to_string()))?;
                if let Some(c) = opts.cores.or(opts.spec.resources.max_cores) {
                    cfg.max_cores = c;
                }
                let mut p = SitePolicy::new(cfg).map_err(|e| RunExit::Refused(e.to_string()))?;
                for f in pause_files {
                    p.add_pause_file(f);
                }
                match p.reconcile_own_claims() {
                    Ok(r) if !r.released.is_empty() || !r.held.is_empty() => log.line(
                        "INFO",
                        &format!(
                            "CPU ledger: released stale own claims {:?}; own claims still held {:?}",
                            r.released, r.held
                        ),
                    ),
                    Ok(_) => {}
                    Err(e) => log.line("WARN", &format!("CPU ledger reconciliation: {e}")),
                }
                Box::new(p)
            }
            _ => {
                let cs = CpuSpec::parse(opts.spec.resources.cpus.as_deref().unwrap_or("1"))
                    .map_err(RunExit::Refused)?;
                let budget = opts.cores.or(opts.spec.resources.max_cores);
                Box::new(
                    PlainPolicy::new(&cs, budget)
                        .map_err(|e| RunExit::Refused(e.to_string()))?
                        .with_pause_files(pause_files),
                )
            }
        };
        let mut admission = AdmissionPolicy::new(policy.budget());
        admission.floor_bytes = opts.spec.resources.host_floor.0;
        admission.campaign_mem_cap_bytes =
            Some(opts.mem_total.unwrap_or(opts.spec.resources.mem_total.0));
        let mut programs = BTreeMap::new();
        for (name, p) in &opts.spec.programs {
            match file_digest(&p.path) {
                Ok((_, d)) => {
                    programs.insert(name.clone(), (p.path.clone(), d));
                }
                Err(e) if p.optional => {
                    log.line("WARN", &format!("optional program {name} unavailable: {e}"));
                }
                Err(e) => {
                    return Err(RunExit::Refused(format!(
                        "program {name} ({}): {e}",
                        p.path.display()
                    )));
                }
            }
        }
        let source: Box<dyn WorkSource> = match &opts.spec.synthetic {
            Some(s) => Box::new(ScriptedWorkSource::new(s.clone())),
            None => return Err(RunExit::Refused("no work source for this spec".into())),
        };
        let ctx = SourceContext {
            binaries: programs
                .keys()
                .map(|n| (n.clone(), format!("bin:{n}")))
                .collect(),
            campaign_key: key,
        };
        let boot_id = crate::resources::boot_id().unwrap_or_default();
        let started_utc = clock.now_utc().to_iso();
        let dir_ids = dir_id(&cdir).zip(dir_id(&crate::ledger::ledger_dir(&cdir)));
        Ok(Supervisor {
            cdir,
            work_root,
            clock,
            ledger,
            lock: Some(lock),
            source,
            ctx,
            programs,
            sampler,
            launcher,
            moved,
            policy,
            admission,
            signals,
            children: Vec::new(),
            progress: ProgressWriter::new(),
            last_progress: None,
            fp,
            log: Log {
                file: None,
                quiet: None,
            },
            bypassed: BTreeMap::new(),
            launch_seq: 0,
            ledger_dead: None,
            control_stop: false,
            disk_floor_hit: false,
            last_disk_check: None,
            last_block_log: String::new(),
            last_wait_log: String::new(),
            boot_id,
            started_at: Instant::now(),
            started_utc,
            drain_started: None,
            dir_ids,
            last_dir_check: None,
            pending: Vec::new(),
            unknown: BTreeMap::new(),
            campaign_min_ticks: None,
            periodic: None,
            last_periodic: Instant::now(),
            last_ready: 0,
            resource_failures: 0,
            resource_backoff_until: None,
            resource_stop: None,
            opts,
        })
    }

    fn st(&self) -> &CampaignState {
        self.ledger.state()
    }

    /// Appends a batch; on failure marks the ledger dead (ENOSPC -> disk full; anything else -> internal).
    fn commit(&mut self, events: Vec<Event>) -> bool {
        if self.ledger_dead.is_some() {
            return false;
        }
        if events.is_empty() {
            return true;
        }
        match self.ledger.append_all(events) {
            Ok(_) => true,
            Err(LedgerError::NoSpace { path }) => {
                self.log.line(
                    "ERROR",
                    &format!(
                        "disk full appending to {}: no new job; running jobs get the grace; the ledger stays valid; free space and run the same command",
                        path.display()
                    ),
                );
                self.ledger_dead = Some(RunExit::DiskFull);
                false
            }
            Err(e) => {
                self.log.line("ERROR", &format!("ledger: {e}"));
                self.ledger_dead = Some(RunExit::Internal(format!("ledger: {e}")));
                false
            }
        }
    }

    /// Like [`Self::commit`], but a record the reducer REFUSES (state unchanged, nothing written) is returned as
    /// `Err(why)` instead of stopping the supervisor: for operator controls, whose legality is the ledger's.
    fn commit_or_refused(&mut self, events: Vec<Event>) -> Result<bool, String> {
        if self.ledger_dead.is_some() {
            return Ok(false);
        }
        match self.ledger.append_all(events.clone()) {
            Ok(_) => Ok(true),
            Err(LedgerError::Refused(e)) => Err(e.to_string()),
            Err(_) => Ok(self.commit(events)),
        }
    }

    fn note(&mut self, text: String) {
        self.log.line("INFO", &text);
        self.commit(vec![Event::Note { text }]);
    }

    /// An `orphan_killed` record (ledger kind; `marker` = the attempt label `<job>:a<n>`, its job recorded).
    fn orphan_killed(&mut self, marker: &str, pid: u32, start_ticks: u64, by: &str) {
        let job = marker
            .rsplit_once(":a")
            .map(|(j, _)| j.to_string())
            .filter(|j| self.st().jobs.contains_key(j));
        self.log.line(
            "WARN",
            &format!("orphan killed: pid {pid} (start {start_ticks}) of {marker} by {by}"),
        );
        self.commit(vec![Event::OrphanKilled {
            job,
            pid,
            start_ticks,
        }]);
    }

    fn rel_path(&self, p: &Path) -> String {
        if let Ok(r) = p.strip_prefix(&self.cdir) {
            r.to_string_lossy().into_owned()
        } else if let Ok(r) = p.strip_prefix(&self.work_root) {
            format!("large:{}", r.to_string_lossy())
        } else {
            p.to_string_lossy().into_owned()
        }
    }

    fn scan_rules(&self) -> ScanRules {
        let mut roots = vec![
            orphans::canonical_root(&self.cdir),
            orphans::canonical_root(&self.work_root),
        ];
        roots.dedup();
        ScanRules {
            campaign_id: self.ctx.campaign_key.clone(),
            recorded_boot_id: Some(self.boot_id.clone()),
            min_start_ticks: 0,
            roots,
        }
    }

    // ------------------------------------------------------------------------------------------------ recovery

    /// Recovery at start (interface section 4): the FREEZING marker sweep of the whole campaign (gate-B M1), then every
    /// unsettled attempt is settled; an attempt whose launch was journalled but never confirmed is authenticated first
    /// and is never treated as "did not run" without proof (requirement 05:23Z).
    fn recover(&mut self) {
        let unsettled: Vec<(String, u32, bool)> = self
            .st()
            .unsettled_jobs()
            .into_iter()
            .map(|(id, j)| (id.clone(), j.attempt, j.state == AttemptState::Launching))
            .collect();
        // The boot and start bounds of the previous sessions, from the ledger itself.
        let mut rules = self.scan_rules();
        let mut min_ticks: Option<u64> = None;
        if let Ok(recs) = crate::ledger::read_records(&self.cdir) {
            for r in &recs {
                if let Event::SupervisorStarted { identity, .. } = &r.event
                    && identity.boot_id == self.boot_id
                {
                    min_ticks = Some(
                        min_ticks.map_or(identity.start_ticks, |m| m.min(identity.start_ticks)),
                    );
                }
            }
        }
        // Sessions of another boot contribute no bound: everything they started is gone (min_ticks stays None
        // when no session of this boot launched anything, and then nothing is signalled).
        rules.min_start_ticks = min_ticks.unwrap_or(u64::MAX);
        // A copied / moved directory (--adopt-moved-campaign): nothing recorded is ours; no scan at all.
        let scan_allowed = min_ticks.is_some() && !self.moved;
        if scan_allowed {
            self.campaign_min_ticks = min_ticks;
        }
        let report = if scan_allowed {
            orphans::sweep(&rules, None, Duration::from_secs(30))
        } else {
            SweepReport {
                proven: true,
                ..Default::default()
            }
        };
        if scan_allowed {
            self.log.line(
                "INFO",
                &format!(
                    "recovery: marker sweep: {} marked process(es) seen, {} killed, {} left, {} round(s){}",
                    report.seen,
                    report.killed.len(),
                    report.left.len(),
                    report.rounds,
                    report
                        .reason
                        .as_ref()
                        .map(|r| format!("; NOT PROVEN: {r}"))
                        .unwrap_or_default()
                ),
            );
        }
        for k in &report.killed {
            self.orphan_killed(&k.attempt_id, k.pid, k.start_ticks, "recovery sweep");
        }
        self.note_survivors(&report, "recovery");
        if let Some(r) = &report.reason {
            self.note(format!(
                "recovery_sweep_unproven {{\"reason\":{r:?}}} (attempts are settled from their files; the periodic sweep continues)"
            ));
        }
        let survivors: BTreeSet<String> =
            report.left.iter().map(|l| l.process.attempt_id.clone()).collect();
        for (job, attempt, launching) in unsettled {
            if self.moved {
                // Copied / moved directory: the recorded attempt's process (if any) belongs to the ORIGINAL directory.
                let ev = vec![
                    Event::Job {
                        job: job.clone(),
                        attempt,
                        ev: JobEvent::Lose {
                            reason: "campaign directory copied / moved: recorded process is not ours (not signalled)"
                                .into(),
                        },
                    },
                    Event::Job {
                        job: job.clone(),
                        attempt: attempt + 1,
                        ev: JobEvent::Requeue {
                            reason: "lost (moved campaign directory)".into(),
                        },
                    },
                ];
                self.log.line(
                    "WARN",
                    &format!("{job} a{attempt}: recorded in the original directory; marked lost, not signalled"),
                );
                self.commit(ev);
                continue;
            }
            let marker = format!("{job}:a{attempt}");
            if survivors.contains(&marker) {
                // gate-A M-1: never settle while a marked process of the attempt lives.
                self.log.line(
                    "ERROR",
                    &format!("{marker}: a marked process is alive and may not be signalled (see marked_survivor); not settled"),
                );
                continue;
            }
            let was_killed = report.killed.iter().any(|k| k.attempt_id == marker);
            let Some(run_dir) = self.st().jobs.get(&job).and_then(|j| j.run_dir.clone()) else {
                continue;
            };
            let layout = RunLayout::new(&run_dir);
            let launched = self
                .st()
                .jobs
                .get(&job)
                .and_then(|j| j.attempt_spec.clone());
            if launching {
                self.recover_unconfirmed(&job, attempt, &run_dir, &layout, launched.as_deref(), was_killed);
                continue;
            }
            match read_spec(&layout, launched.as_deref()) {
                Some((spec, spec_digest)) => {
                    let kill = if was_killed { CKill::Stop } else { CKill::None };
                    self.settle(
                        &job,
                        attempt,
                        &layout,
                        &spec,
                        &spec_digest,
                        WaitStatus::Unknown,
                        kill,
                        was_killed,
                        None,
                    );
                }
                None => {
                    let ev = vec![
                        Event::Job {
                            job: job.clone(),
                            attempt,
                            ev: JobEvent::Lose {
                                reason: "run directory without a job.json matching its launch record at recovery"
                                    .into(),
                            },
                        },
                        Event::Job {
                            job: job.clone(),
                            attempt: attempt + 1,
                            ev: JobEvent::Requeue {
                                reason: "lost at recovery".into(),
                            },
                        },
                    ];
                    self.commit(ev);
                }
            }
        }
    }

    /// An attempt in `launching` (journalled launch, never confirmed by `job.started`) at recovery (requirement 05:23Z):
    /// - no spawn sentinel `RUN/sup/SPAWN` (written and fsynced right before the spawn): the spawn was provably never
    ///   attempted -> lost + requeue (the ONLY case treated as "did not run", and it is proven);
    /// - otherwise the run directory is AUTHENTICATED (path = this attempt's layout path, `RUN/sup/CAMPAIGN` = this
    ///   campaign's marker id, job.json digest = the launch record's, job id / attempt / run dir inside job.json), any
    ///   live process of it was handled by the sweep before, and a valid OUTCOME (bound to the launch digest) is settled
    ///   as usual (valid sealed results are consumed, never re-solved);
    /// - anything else: EXECUTION UNKNOWN: a ledger note, the attempt stays as it is, NO automatic retry; the operator
    ///   decides with `rr_campaign control CDIR resolve JOB` (requeue). HOOK NEEDED FROM LEDGERFIX2: an attempt state
    ///   `execution_unknown` (job event `unknown {reason}`) and a control verb `resolve` legal only from it; until then the
    ///   attempt stays `launching` and the resolve is recorded as a note + `lose` + `requeue`.
    fn recover_unconfirmed(
        &mut self,
        job: &str,
        attempt: u32,
        run_dir: &str,
        layout: &RunLayout,
        launched: Option<&str>,
        was_killed: bool,
    ) {
        if !was_killed && !layout.sup_dir().join(SPAWN_SENTINEL).exists() {
            let ev = vec![
                Event::Job {
                    job: job.into(),
                    attempt,
                    ev: JobEvent::Lose {
                        reason: "launch journalled but the spawn was never attempted (no RUN/sup/SPAWN): provably did not run"
                            .into(),
                    },
                },
                Event::Job {
                    job: job.into(),
                    attempt: attempt + 1,
                    ev: JobEvent::Requeue {
                        reason: "spawn never attempted".into(),
                    },
                },
            ];
            self.log.line(
                "INFO",
                &format!("{job} a{attempt}: launch journalled, spawn never attempted: requeued"),
            );
            self.commit(ev);
            return;
        }
        let (spec, digest) = match self.authenticate_run(job, attempt, run_dir, layout, launched) {
            Ok(x) => x,
            Err(why) => {
                self.execution_unknown(job, attempt, &format!("run directory not authenticated: {why}"));
                return;
            }
        };
        let kill = if was_killed { CKill::Stop } else { CKill::None };
        let ev = classify::assess(layout, &spec, &digest, WaitStatus::Unknown, kill, false);
        if matches!(ev.outcome, OutcomeState::Valid(_)) {
            self.settle(
                job,
                attempt,
                layout,
                &spec,
                &digest,
                WaitStatus::Unknown,
                kill,
                was_killed,
                None,
            );
            return;
        }
        let why = match &ev.outcome {
            OutcomeState::Absent => "no OUTCOME".to_string(),
            OutcomeState::Unreadable(e) => format!("OUTCOME unreadable: {e}"),
            OutcomeState::Invalid(e) => format!("OUTCOME invalid: {e}"),
            OutcomeState::Valid(_) => unreachable!(),
        };
        self.execution_unknown(
            job,
            attempt,
            &format!(
                "spawned (RUN/sup/SPAWN present){}, {why}",
                if was_killed { ", a live process of it was killed by the recovery sweep" } else { "" }
            ),
        );
    }

    fn authenticate_run(
        &self,
        job: &str,
        attempt: u32,
        run_dir: &str,
        layout: &RunLayout,
        launched: Option<&str>,
    ) -> Result<(JobSpec, String), String> {
        let expected = attempt_dir(&self.work_root, job, attempt);
        if Path::new(run_dir) != expected {
            return Err(format!("launch record names {run_dir}, the layout path is {}", expected.display()));
        }
        let id = std::fs::read_to_string(layout.sup_dir().join(CAMPAIGN_ID_FILE))
            .map_err(|e| format!("RUN/sup/{CAMPAIGN_ID_FILE}: {e}"))?;
        if id.trim() != self.ctx.campaign_key {
            return Err(format!("RUN/sup/{CAMPAIGN_ID_FILE} names another campaign id"));
        }
        let (spec, digest) = read_spec(layout, launched)
            .ok_or("job.json does not have the digest of its launch record")?;
        if spec.job_id != job || spec.attempt != attempt || Path::new(&spec.run_dir) != expected {
            return Err("job.json names another job, attempt or run directory".into());
        }
        Ok((spec, digest))
    }

    fn execution_unknown(&mut self, job: &str, attempt: u32, why: &str) {
        self.unknown.insert(job.to_string(), attempt);
        self.log.line(
            "WARN",
            &format!(
                "{job} a{attempt}: EXECUTION UNKNOWN ({why}); not retried automatically: `rr_campaign control CDIR resolve {job}` requeues it"
            ),
        );
        self.commit(vec![Event::Note {
            text: format!(
                "execution_unknown {{\"job\":{job:?},\"attempt\":{attempt},\"why\":{why:?}}} (never retried automatically; rr_campaign control CDIR resolve JOB)"
            ),
        }]);
    }

    /// Ledger notes for live marked processes a sweep did not signal (each reported; gate-A M-1, gate-B M1).
    fn note_survivors(&mut self, report: &SweepReport, by: &str) {
        for l in &report.left {
            self.log.line(
                "ERROR",
                &format!(
                    "marked process alive and not signalled: pid {} (start {}) of {}: {}",
                    l.process.pid, l.process.start_ticks, l.process.attempt_id, l.why
                ),
            );
            self.commit(vec![Event::Note {
                text: format!(
                    "marked_survivor {{\"pid\":{},\"start_ticks\":{},\"marker\":{:?},\"comm\":{:?},\"by\":{by:?},\"why\":{:?}}}",
                    l.process.pid, l.process.start_ticks, l.process.attempt_id, l.process.comm, l.why
                ),
            }]);
        }
    }

    // ------------------------------------------------------------------------------------------------ binaries

    fn register_binaries(&mut self) -> Result<(), RunExit> {
        let mut ev = Vec::new();
        let mut changed = Vec::new();
        for (name, (path, digest)) in &self.programs {
            let id = format!("bin:{name}");
            match self.st().inputs.get(&id) {
                Some(i) if i.blake3 == *digest => continue,
                Some(_) => changed.push(name.clone()),
                None => {}
            }
            let bytes = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
            ev.push(Event::InputRegistered {
                input: id,
                role: "binary".into(),
                path: path.to_string_lossy().into_owned(),
                bytes,
                blake3: digest.clone(),
            });
        }
        for i in &self.opts.spec.inputs {
            let bytes = i.check().map_err(RunExit::Refused)?;
            if self
                .st()
                .inputs
                .get(&i.id)
                .is_some_and(|x| x.blake3 == i.blake3)
            {
                continue;
            }
            ev.push(Event::InputRegistered {
                input: i.id.clone(),
                role: i.role.clone(),
                path: i.path.to_string_lossy().into_owned(),
                bytes,
                blake3: i.blake3.clone(),
            });
        }
        if !changed.is_empty() && !self.opts.accept_binary_change {
            return Err(RunExit::BinaryChanged(format!(
                "binary changed since the campaign started: {} (resume refused; --accept-binary-change marks its rows stale and redoes them)",
                changed.join(", ")
            )));
        }
        if !changed.is_empty() {
            self.log.line(
                "WARN",
                &format!(
                    "binary changed: {}; dependent rows are marked stale and redone",
                    changed.join(", ")
                ),
            );
        }
        if !self.commit(ev) {
            return Err(self.ledger_dead.clone().unwrap_or(RunExit::DiskFull));
        }
        Ok(())
    }

    // ------------------------------------------------------------------------------------------------ settle

    /// Classifies a finished (or, at recovery, process-less) attempt and records it in ONE group commit with the
    /// routing of the work source. `killed_at_recovery`: the attempt's process was killed by the orphan scan.
    #[allow(clippy::too_many_arguments)]
    fn settle(
        &mut self,
        job: &str,
        attempt: u32,
        layout: &RunLayout,
        spec: &JobSpec,
        spec_digest: &str,
        wait: WaitStatus,
        kill: CKill,
        killed_at_recovery: bool,
        report: Option<&FinalReport>,
    ) {
        // The supervisor's own kill reason K is an input of the decision table: a child stopped for time, silence or
        // memory that still writes an `interrupted` OUTCOME is killed_time / killed_memory there (rows 11b / 11c).
        let ev = classify::assess(layout, spec, spec_digest, wait, kill, false);
        let c = classify::classify(&ev);
        // The OUTCOME binding the ledger checks before any record may come from this attempt: (job, attempt, the
        // digest of the job.json this attempt was launched with), present when the contract found OUTCOME valid.
        let binding = matches!(ev.outcome, OutcomeState::Valid(_)).then(|| OutcomeBinding {
            job: job.to_string(),
            attempt,
            spec_blake3: spec_digest.to_string(),
        });
        let target = class_target(c.kind);
        let Some(jr) = self.st().jobs.get(job).cloned() else {
            return;
        };
        if jr.attempt != attempt || jr.state.is_settled() {
            return;
        }
        let (cpu_ms, wall_ms, peak) = report.map_or((0, 0, 0), |r| {
            (
                (r.cpu_seconds * 1000.0) as u64,
                (r.wall_seconds * 1000.0) as u64,
                r.peak_rss_bytes,
            )
        });
        // OUTCOME's FileRef from ONE read bound to what the classification parsed (ac2-M1): the bytes recorded are bytes
        // whose parse equals the verified OUTCOME, read through the run-relative no-follow open.
        let outcome_ref = match &ev.outcome {
            OutcomeState::Valid(o) => match read_bound_outcome(layout, o) {
                Ok((bytes, blake3)) => Some(FileRef {
                    path: self.rel_path(&layout.outcome()),
                    bytes,
                    blake3,
                }),
                Err(why) => {
                    self.log.line(
                        "WARN",
                        &format!("{job} a{attempt}: OUTCOME changed since it was classified ({why}); not recorded"),
                    );
                    None
                }
            },
            _ => file_ref(&layout.outcome(), self.rel_path(&layout.outcome())),
        };
        let mut events = Vec::new();
        let interrupted = killed_at_recovery && target != AttemptTarget::SucceededSealed;
        let requeue_class = matches!(c.kind, FinishClass::Interrupted | FinishClass::Lost);
        if interrupted || requeue_class {
            if interrupted && !requeue_class {
                events.push(Event::Job {
                    job: job.into(),
                    attempt,
                    ev: JobEvent::Interrupt {
                        reason: "process of a previous supervisor killed at recovery".into(),
                    },
                });
            } else {
                events.push(finished_event(
                    job,
                    attempt,
                    c.kind,
                    ContractAccept::Nothing,
                    &ev,
                    None,
                    outcome_ref,
                    binding.clone(),
                    cpu_ms,
                    wall_ms,
                    peak,
                    c.time_decided,
                ));
            }
            events.push(Event::Job {
                job: job.into(),
                attempt: attempt + 1,
                ev: JobEvent::Requeue {
                    reason: format!("{} (row {})", class_word(c.kind), c.row),
                },
            });
            self.log.line(
                "INFO",
                &format!(
                    "{job} a{attempt}: {} (row {}): requeued",
                    class_word(c.kind),
                    c.row
                ),
            );
            self.commit(events);
            return;
        }
        let mut table = None;
        let mut lines = Vec::new();
        let mut canon = None;
        // A sealed table is read for every attempt whose row may accept records (sealed successes, and the committed
        // rules of a sealed partial table: spill row 8, no_rule row 4).
        let wants_table =
            target == AttemptTarget::SucceededSealed || c.accept != ContractAccept::Nothing;
        if wants_table && spec.kind.produces_table() {
            // ac2-M1: ONE read of the table through the contract's run-relative no-follow open, hashed while reading,
            // bound to the stream `assess` verified (dev, inode, byte count, BLAKE3, canonical digest); a line without a
            // canonical form or any mismatch accepts NOTHING (class crashed). The contract lane's verified-stream handle
            // (contractfix2) is not in this tree yet: the checks are local (read_verified_table).
            match read_verified_table(layout, ev.verified.as_ref()) {
                Ok(t) => {
                    table = Some(FileRef {
                        path: self.rel_path(&layout.root.join(&t.rel)),
                        bytes: t.bytes,
                        blake3: t.blake3,
                    });
                    lines = t.lines;
                    canon = Some(t.canonical);
                }
                Err(why) => self.log.line(
                    "ERROR",
                    &format!("{job} a{attempt}: sealed table not accepted: {why}; nothing accepted"),
                ),
            }
        }
        let sealed_ok = !wants_table || !spec.kind.produces_table() || table.is_some();
        let settled = Settled {
            job: job.into(),
            attempt,
            item: jr.item.clone(),
            rung: jr.rung,
            classification: c.clone(),
            table: table.clone(),
            canonical_lines: lines,
            canonical_blake3: canon,
        };
        let mut settled = settled;
        let routing = if !sealed_ok {
            Err("sealed table unreadable".into())
        } else if let Some(r) = row3_routing(self.source.as_mut(), self.ledger.state(), &self.ctx, &mut settled) {
            self.log.line(
                "INFO",
                &format!("{job} a{attempt}: row 3 claims confirmed by the work source's coverage check"),
            );
            Ok(r)
        } else {
            if settled.classification.row == 3 {
                self.log.line(
                    "INFO",
                    &format!("{job} a{attempt}: row 3 (no_rule, per-case claims only) not confirmed by coverage: next rung"),
                );
            }
            self.source.route(self.ledger.state(), &self.ctx, &settled)
        };
        let routing = match routing {
            Ok(r) => r,
            Err(e) => {
                self.log
                    .line("ERROR", &format!("{job} a{attempt}: routing refused: {e}"));
                worksource::Routing::default()
            }
        };
        for a in &routing.accepts {
            let p = self.cdir.join(&a.path);
            if let Some(d) = p.parent() {
                let _ = std::fs::create_dir_all(d);
            }
            if let Err(e) = crate::atomic::write_atomic(&p, &a.bytes) {
                if e.is_no_space() {
                    self.ledger_dead = Some(RunExit::DiskFull);
                }
                self.log.line(
                    "ERROR",
                    &format!("accepted record {}: {e}; not recorded", a.path),
                );
                return;
            }
        }
        let class = if sealed_ok {
            c.kind
        } else {
            FinishClass::Crashed
        };
        // The contract row's accept column (the ledger refuses a value the row of `class` does not allow).
        let accept = if contract_accepts(class).contains(&c.accept) {
            c.accept
        } else {
            ContractAccept::Nothing
        };
        let sealed = if class_target(class) == AttemptTarget::SucceededSealed
            || accept != ContractAccept::Nothing
        {
            table
        } else {
            None
        };
        events.push(finished_event(
            job,
            attempt,
            class,
            accept,
            &ev,
            sealed,
            outcome_ref,
            binding,
            cpu_ms,
            wall_ms,
            peak,
            c.time_decided,
        ));
        events.extend(routing.events);
        self.fp.hit("before_settle_commit");
        self.log.line(
            "INFO",
            &format!(
                "{job} a{attempt}: {} (row {}){}",
                class_word(class),
                c.row,
                if routing.accepts.is_empty() {
                    ""
                } else {
                    ", accepted"
                }
            ),
        );
        self.commit(events);
        self.fp.hit("after_settle_commit");
    }

    // ------------------------------------------------------------------------------------------------ launch

    /// One launch failure that is the supervisor's own resource problem (gate-B M3): one ledger note per episode, a
    /// bounded exponential back-off of launching (1, 2, 4, 8, 16 s), and after [`MAX_RESOURCE_FAILURES`] in a row a clean
    /// stop (drain; exit 71, the reason in the ledger's drain record and on the dashboard's events list).
    fn resource_failure(&mut self, errno: i32, what: &str) {
        self.resource_failures += 1;
        let n = self.resource_failures;
        if n == 1 {
            self.log.line(
                "WARN",
                &format!("supervisor resource error ({}): {what}; not a job attempt, launching backs off", errno_name(errno)),
            );
            self.commit(vec![Event::Note {
                text: format!(
                    "supervisor_resource_error {{\"errno\":{errno:?},\"what\":{what:?}}} (not a job attempt; launching backs off; stop after {MAX_RESOURCE_FAILURES} in a row)",
                    errno = errno_name(errno)
                ),
            }]);
        }
        let backoff = Duration::from_secs(1u64 << (n - 1).min(4));
        self.resource_backoff_until = Some(Instant::now() + backoff);
        if n >= MAX_RESOURCE_FAILURES && self.resource_stop.is_none() {
            let msg = format!(
                "{n} launch failures in a row for lack of a supervisor resource ({}): stopping; free it (e.g. raise `ulimit -n`) and run the same command",
                errno_name(errno)
            );
            self.log.line("ERROR", &msg);
            self.resource_stop = Some(msg);
            self.signals.handle.request_stop();
        }
    }

    fn launch(&mut self, req: &JobRequest, lease: CpuLease) -> Result<(), String> {
        let existing = self.st().jobs.get(&req.job).cloned();
        let attempt = match &existing {
            Some(j) if j.state == AttemptState::Queued => j.attempt,
            Some(j) => return Err(format!("job {} is {}", req.job, j.state.tag())),
            None => 1,
        };
        let (prog_path, prog_digest) = self
            .programs
            .get(&req.program)
            .cloned()
            .ok_or_else(|| format!("program {} not in the spec", req.program))?;
        let run_dir = attempt_dir(&self.work_root.clone(), &req.job, attempt);
        if run_dir.exists() {
            // Left by a crash between job.json and the launch record: never recorded, move it aside.
            let aside =
                run_dir.with_file_name(format!("a{attempt}.unrecorded-{}", self.clock.now_utc().0));
            std::fs::rename(&run_dir, &aside)
                .map_err(|e| format!("move aside {}: {e}", run_dir.display()))?;
        }
        let resume = if attempt > 1 {
            let prev_spec = existing.as_ref().and_then(|j| j.prev_attempt_spec.clone());
            resume_from(
                &attempt_dir(&self.work_root.clone(), &req.job, attempt - 1),
                prev_spec.as_deref(),
            )
        } else {
            None
        };
        let kind = if self.opts.fake_child || req.fake.is_some() {
            AdapterKind::Fake
        } else {
            AdapterKind::Contract
        };
        let spec = adapters::job_spec(
            kind,
            req,
            &AdapterInput {
                run_dir: &run_dir,
                attempt,
                program_name: &req.program,
                program_path: &prog_path,
                program_blake3: &prog_digest,
                resume,
            },
        );
        let (mut ls, spec_digest) = match adapters::prepare_run(&spec, &prog_path) {
            Ok(x) => x,
            Err(e) => {
                if let Some(c) = io_resource_errno(&e) {
                    self.resource_failure(c, &format!("preparing {}: {e}", run_dir.display()));
                }
                return Err(format!("run dir: {e}"));
            }
        };
        // Run directory authentication at recovery (requirement 05:23Z): the campaign's marker id, before the launch record.
        if let Err(e) = crate::atomic::write_atomic(
            &RunLayout::new(&run_dir).sup_dir().join(CAMPAIGN_ID_FILE),
            format!("{}\n", self.ctx.campaign_key).as_bytes(),
        ) {
            if let Some(c) = io_resource_errno(&std::io::Error::other(e.to_string())) {
                self.resource_failure(c, "writing RUN/sup/CAMPAIGN");
            }
            return Err(format!("RUN/sup/{CAMPAIGN_ID_FILE}: {e}"));
        }
        // The launcher sets the marker `<campaign id>:<label>`: the label is the attempt id.
        ls.label = format!("{}:a{attempt}", req.job);
        if let Some(l) = &self.lock {
            ls.env.push((lock::TOKEN_ENV.into(), l.token().into()));
        }
        if let Some(name) = &self.opts.spec.licence.env {
            ls.env_passthrough.push(name.clone());
        }
        ls.cpus = lease.cpus().to_vec();
        ls.memory_cap_bytes = Some(req.cap_bytes);
        let mut events = Vec::new();
        if existing.is_none() {
            // Input identity (ledger 5.5): beyond its binary and the item's / node's inputs (the reducer adds those),
            // the job depends on what its request names (tables, seed policy, ...) and, conservatively, on every
            // campaign-level input of the spec: a new digest of any of them makes the job stale.
            let mut inputs: Vec<String> = req.depends_on.clone();
            for i in &self.opts.spec.inputs {
                if !inputs.contains(&i.id) {
                    inputs.push(i.id.clone());
                }
            }
            events.push(Event::JobQueued {
                job: req.job.clone(),
                item: req.item.clone(),
                rung: req.rung,
                kind: format!("{:?}", spec.kind).to_lowercase(),
                spec_blake3: req.digest(),
                binary: self
                    .ctx
                    .binaries
                    .get(&req.program)
                    .cloned()
                    .unwrap_or_else(|| format!("bin:{}", req.program)),
                reserved_bytes: req.cap_bytes,
                inputs,
            });
            events.push(Event::Item {
                item: req.item.clone(),
                ev: crate::ledger::ItemEvent::Start {
                    job: Some(req.job.clone()),
                },
            });
        }
        events.push(Event::Job {
            job: req.job.clone(),
            attempt,
            ev: JobEvent::Launch {
                run_dir: run_dir.to_string_lossy().into_owned(),
                cpus: lease.cpus().iter().map(|&c| c as u32).collect(),
                reserved_bytes: req.cap_bytes,
                // The digest of job.json as written: the ONE record of it (no side file); OUTCOME / RESUME and the
                // `job.finished` binding are checked against it.
                spec_blake3: spec_digest.clone(),
            },
        });
        if !self.commit(events) {
            return Err("ledger unavailable".into());
        }
        self.fp.hit("after_launch_record");
        // The spawn sentinel (requirement 05:23Z): durable BEFORE the spawn, so that its absence proves "never spawned".
        if let Err(e) = write_spawn_sentinel(&RunLayout::new(&run_dir)) {
            self.commit(not_an_attempt(&req.job, attempt, &format!("spawn sentinel not written: {e}")));
            if let Some(c) = io_resource_errno(&e) {
                self.resource_failure(c, &format!("writing RUN/sup/{SPAWN_SENTINEL}: {e}"));
            }
            return Err(format!("spawn sentinel: {e}"));
        }
        let handle = match self.launcher.launch(&ls) {
            Ok(h) => h,
            Err(e) if resource_errno(&e).is_some() => {
                // gate-B M3: the SUPERVISOR's resource problem, not a job attempt: the same job is requeued (no new job
                // id), launching backs off, and after a bounded number of failures in a row the campaign stops (71).
                let c = resource_errno(&e).unwrap_or(libc::EMFILE);
                self.commit(not_an_attempt(
                    &req.job,
                    attempt,
                    &format!("supervisor resource error at launch ({}): {e}", errno_name(c)),
                ));
                self.resource_failure(c, &format!("launch of {}: {e}", req.job));
                return Err(format!("launch: {e}"));
            }
            Err(e) => {
                let fatal = matches!(e, LaunchError::MissingEnv(_) | LaunchError::InvalidSpec(_));
                let ev = vec![Event::Job {
                    job: req.job.clone(),
                    attempt,
                    ev: JobEvent::Finished {
                        class: FinishClass::SetupError,
                        accept: ContractAccept::Nothing,
                        exit_code: None,
                        signal: None,
                        sealed_output: None,
                        outcome: None,
                        binding: None,
                        cpu_ms: 0,
                        wall_ms: 0,
                        peak_rss_bytes: 0,
                        time_decided: false,
                    },
                }];
                self.commit(ev);
                if fatal {
                    self.log.line(
                        "ERROR",
                        &format!("launch of {} failed: {e}; draining", req.job),
                    );
                    self.signals.handle.request_stop();
                }
                return Err(format!("launch: {e}"));
            }
        };
        self.fp.hit("after_spawn");
        self.resource_failures = 0;
        self.resource_backoff_until = None;
        let id = handle.identity().clone();
        self.commit(vec![Event::Job {
            job: req.job.clone(),
            attempt,
            ev: JobEvent::Started {
                pid: id.pid,
                start_ticks: id.start_ticks,
                boot_id: id.boot_id.clone(),
            },
        }]);
        self.launch_seq += 1;
        let hb = self.opts.spec.heartbeat_timeout(spec.heartbeat_s);
        let budget_s = spec.budgets.wall.job_s;
        self.children.push(Child {
            job: req.job.clone(),
            attempt,
            item: req.item.clone(),
            handle,
            lease: Some(lease),
            layout: RunLayout::new(&run_dir),
            spec,
            spec_digest,
            tail: EventsTail::new(1 << 20),
            last_event: Instant::now(),
            started_seen: false,
            launched_at: Instant::now(),
            cap: req.cap_bytes,
            peak: req.expected_peak_bytes,
            cores: req.cores,
            hb_timeout: hb,
            seq: self.launch_seq,
            stopping: false,
            phase: None,
            last_event_utc: None,
            budget_s,
        });
        Ok(())
    }

    // ------------------------------------------------------------------------------------------------ the loop

    fn sampler_fresh(&self) -> bool {
        match self.sampler.latest() {
            Some(s) => s.finished_at.elapsed() < Duration::from_secs(30),
            None => self.started_at.elapsed() < Duration::from_secs(30),
        }
    }

    fn poll_children(&mut self) {
        let mut finished = Vec::new();
        let stopping_grace = Duration::from_secs(self.opts.spec.resources.grace_seconds + 5);
        let start_timeout = Duration::from_secs(self.opts.spec.resources.start_timeout_s);
        let now_iso = self.clock.now_utc().to_iso();
        for (idx, ch) in self.children.iter_mut().enumerate() {
            if let Ok(items) = ch.tail.poll(&ch.layout.events()) {
                for it in items {
                    if let TailItem::Event(e) = it {
                        ch.last_event = Instant::now();
                        ch.last_event_utc = Some(now_iso.clone());
                        match &e.event {
                            ChildEvent::JobStarted { .. } => ch.started_seen = true,
                            ChildEvent::Phase { phase } | ChildEvent::Heartbeat { phase, .. } => {
                                ch.phase = serde_json::to_value(phase)
                                    .ok()
                                    .and_then(|v| v.as_str().map(str::to_string));
                            }
                            _ => {}
                        }
                    }
                }
            }
            if !ch.stopping {
                let silent = if ch.started_seen {
                    ch.last_event.elapsed() > ch.hb_timeout
                } else {
                    ch.launched_at.elapsed() > start_timeout.max(ch.hb_timeout)
                };
                if silent {
                    ch.stopping = true;
                    ch.handle.terminate(
                        Duration::from_secs(ch.spec.stop_grace_s as u64 + 5).min(stopping_grace),
                        KillReason::Unresponsive,
                    );
                }
            }
            match ch.handle.poll() {
                Ok(ChildProgress::Finished(r)) => finished.push((idx, r)),
                Ok(_) => {}
                Err(e) => {
                    let _ = writeln!(std::io::stderr(), "poll {}: {e}", ch.job);
                }
            }
        }
        for (idx, report) in finished.into_iter().rev() {
            let ch = self.children.remove(idx);
            let marker = format!("{}:a{}", ch.job, ch.attempt);
            // Leftovers the process layer already killed through its verified path (helpers that outlived the job).
            for l in &report.leftovers_killed {
                self.orphan_killed(
                    &marker,
                    l.pid,
                    l.start_ticks,
                    &format!(
                        "launcher ({}): contract violation, process outlived its job",
                        l.comm
                    ),
                );
            }
            let verified = report.verification.is_verified();
            if let crate::launch::Verification::Unverified { leftovers, reason } =
                &report.verification
            {
                // Process layer (procfix2, RB-C1 d): an unproven end is a ledger record; the job and its CPU lease stay
                // in the launcher's quarantine until a later hunt proves it gone. The attempt is settled only then.
                self.note(format!(
                    "unverified_end {{\"marker\":\"{marker}\",\"left\":{},\"reason\":{:?}}} (lease kept, hunting continues, settle waits)",
                    leftovers.len(),
                    reason
                ));
            }
            for u in &report.unattributed_children {
                self.note(format!(
                    "unattributed_child {{\"pid\":{},\"start_ticks\":{},\"session\":{},\"comm\":{:?}}} (re-parented to the supervisor, no marker: not signalled)",
                    u.pid, u.start_ticks, u.session, u.comm
                ));
            }
            if let Some(crate::launch::RetireEvent::Cleared {
                lease_note: Some(n),
                ..
            }) = self.launcher.retire(ch.handle, None)
            {
                self.note(format!("lease {marker}: {n}"));
            }
            // ac2-M1 / gate-A M-1: settle only after the job is proven gone AND a marker sweep found nothing of it.
            self.pending.push(PendingSettle {
                job: ch.job,
                attempt: ch.attempt,
                layout: ch.layout,
                spec: ch.spec,
                spec_digest: ch.spec_digest,
                report,
                awaiting_proof: !verified,
                sweep: None,
                next_sweep: Instant::now(),
                inconclusive: 0,
                survivors_noted: false,
                lease: ch.lease,
                cap: ch.cap,
                cores: ch.cores,
                seq: ch.seq,
            });
            let _ = &ch.item;
        }
        self.drive_pending();
    }

    /// Advance the finished-but-unsettled attempts (non-blocking; sweeps run on helper threads, gate-A m-4).
    fn drive_pending(&mut self) {
        let now = Instant::now();
        let draining = self.signals.handle.level() > StopLevel::Running;
        let mut i = 0;
        while i < self.pending.len() {
            if self.pending[i].awaiting_proof {
                i += 1;
                continue;
            }
            if self.pending[i].sweep.is_none() {
                if now >= self.pending[i].next_sweep {
                    let label = self.pending[i].label();
                    let spec = self
                        .scan_rules_launched()
                        .sweep_spec(Some(&label), Duration::from_secs(10));
                    match spec {
                        Some(spec) => self.pending[i].sweep = SweepTask::spawn(spec),
                        None => {
                            // Another boot (impossible for a job of this session): nothing to sweep.
                            let p = self.pending.remove(i);
                            self.settle_pending(p);
                            continue;
                        }
                    }
                }
                i += 1;
                continue;
            }
            let done = self.pending[i].sweep.as_ref().and_then(SweepTask::poll);
            let Some(done) = done else {
                i += 1;
                continue;
            };
            self.pending[i].sweep = None;
            let label = self.pending[i].label();
            let report = match done {
                Ok(r) => r,
                Err(()) => SweepReport {
                    reason: Some("sweep thread ended without a report".into()),
                    ..Default::default()
                },
            };
            for k in &report.killed {
                self.orphan_killed(
                    &label,
                    k.pid,
                    k.start_ticks,
                    "marker sweep: contract violation, process outlived its job",
                );
            }
            if report.empty() {
                let p = self.pending.remove(i);
                self.settle_pending(p);
                continue;
            }
            if !report.left.is_empty() {
                // A live marked process of the attempt that may not be signalled: never settled meanwhile.
                if !self.pending[i].survivors_noted {
                    self.note_survivors(&report, "end-of-job sweep");
                    self.pending[i].survivors_noted = true;
                }
                if draining {
                    self.log.line(
                        "WARN",
                        &format!("{label}: left unsettled at stop (marked process alive); the restart's sweep handles it"),
                    );
                    self.pending.remove(i);
                    continue;
                }
                self.pending[i].next_sweep = now + Duration::from_secs(30);
            } else {
                // Inconclusive (unreadable environs elsewhere on the host): retried; after 3 tries the settle proceeds
                // (the table read is bound to the verified stream whatever happens meanwhile).
                self.pending[i].inconclusive += 1;
                if self.pending[i].inconclusive >= 3 || draining {
                    let why = report.reason.clone().unwrap_or_default();
                    self.note(format!(
                        "sweep_inconclusive {{\"marker\":{label:?},\"reason\":{why:?}}} (settled; the table read is bound to the verified stream)"
                    ));
                    let p = self.pending.remove(i);
                    self.settle_pending(p);
                    continue;
                }
                self.pending[i].next_sweep = now + Duration::from_secs(2);
            }
            i += 1;
        }
    }

    fn settle_pending(&mut self, mut p: PendingSettle) {
        let label = p.label();
        if let Some(l) = p.lease.take() {
            // Released after the settle below has been committed (or attempted).
            let lease_after = l;
            self.settle_pending_inner(&p);
            let (r, held) = lease_after.release_checked();
            let mut notes: Vec<String> = Vec::new();
            if let Err(e) = r {
                notes.push(format!("release: {e}"));
            }
            for h in held {
                notes.push(format!(
                    "CPU {} still held after release by {:?} (unmarked survivor? not signalled; its claim stays in the ledger)",
                    h.cpu, h.holders
                ));
            }
            if !notes.is_empty() {
                self.note(format!("lease {label}: {}", notes.join("; ")));
            }
            return;
        }
        self.settle_pending_inner(&p);
    }

    fn settle_pending_inner(&mut self, p: &PendingSettle) {
        if self.ledger_dead.is_some() {
            return;
        }
        self.settle(
            &p.job,
            p.attempt,
            &p.layout,
            &p.spec,
            &p.spec_digest,
            map_wait(p.report.exit),
            contract_kill_reason(p.report.kill_reason.as_ref()),
            false,
            Some(&p.report),
        );
    }

    /// Every 30 s (gate-B M1): orphans of attempts that are NOT live (not running, not waiting to be settled), from the
    /// sampler's marked list, get the freezing sweep on a helper thread; what it cannot signal is reported.
    fn periodic_sweep(&mut self) {
        if let Some(t) = &self.periodic {
            match t.poll() {
                None => return,
                Some(r) => {
                    self.periodic = None;
                    if let Ok(r) = r {
                        for k in &r.killed {
                            self.orphan_killed(&k.attempt_id, k.pid, k.start_ticks, "periodic sweep");
                        }
                        self.note_survivors(&r, "periodic sweep");
                    }
                }
            }
        }
        if self.last_periodic.elapsed() < Duration::from_secs(30) {
            return;
        }
        self.last_periodic = Instant::now();
        let Some(min) = self.campaign_min_ticks.or_else(|| Some(orphans::own_start_ticks())) else {
            return;
        };
        let Some(snap) = self.sampler.latest() else {
            return;
        };
        let live: BTreeSet<String> = self
            .children
            .iter()
            .map(|c| format!("{}:a{}", c.job, c.attempt))
            .chain(self.pending.iter().map(PendingSettle::label))
            .chain(self.launcher.quarantined().into_iter().map(|q| q.0))
            .collect();
        let orphans: BTreeSet<String> = snap
            .marked
            .iter()
            .filter(|p| p.start_ticks >= min && !live.contains(&p.attempt_id))
            .map(|p| p.attempt_id.clone())
            .collect();
        let Some(only) = orphans.into_iter().next() else {
            return;
        };
        let mut rules = self.scan_rules();
        rules.min_start_ticks = min;
        if let Some(spec) = rules.sweep_spec(Some(&only), Duration::from_secs(10)) {
            self.periodic = SweepTask::spawn(spec);
        }
    }

    /// At "finished" (and nothing live): one campaign-wide sweep (blocking, bounded), so that the campaign never ends
    /// with a live marked process unreported (gate-B M1).
    fn final_sweep(&mut self) {
        let mut rules = self.scan_rules();
        rules.min_start_ticks = self
            .campaign_min_ticks
            .unwrap_or_else(orphans::own_start_ticks);
        let r = orphans::sweep(&rules, None, Duration::from_secs(10));
        for k in &r.killed {
            self.orphan_killed(&k.attempt_id, k.pid, k.start_ticks, "final sweep");
        }
        self.note_survivors(&r, "final sweep");
        if let Some(why) = &r.reason {
            self.log.line("WARN", &format!("final marker sweep not proven: {why}"));
        }
    }

    /// The launcher's quarantine (procfix2): ledger notes for jobs proven gone later (their leases are released then).
    fn tick_quarantine(&mut self) {
        for ev in self.launcher.tick() {
            match ev {
                crate::launch::RetireEvent::Cleared {
                    label,
                    lease_note,
                    after,
                    report,
                } => {
                    self.note(format!(
                        "unverified_end_cleared {{\"marker\":\"{label}\",\"after_s\":{}}}{}",
                        after.as_secs(),
                        lease_note.map(|n| format!(" ({n})")).unwrap_or_default()
                    ));
                    // The proof arrived: the attempt may now be swept and settled.
                    for l in &report.leftovers_killed {
                        self.orphan_killed(&label, l.pid, l.start_ticks, "launcher quarantine");
                    }
                    if let Some(p) = self.pending.iter_mut().find(|p| p.label() == label) {
                        p.awaiting_proof = false;
                        p.report = *report;
                    }
                }
                crate::launch::RetireEvent::StillUnverified {
                    label,
                    reason,
                    left,
                } => self.log.line(
                    "WARN",
                    &format!("{label}: end still unproven ({left} left): {reason}"),
                ),
            }
        }
    }

    fn scan_rules_launched(&self) -> ScanRules {
        let mut r = self.scan_rules();
        r.min_start_ticks = orphans::own_start_ticks();
        r
    }

    /// Operator control requests (`CDIR/control/*.json`, written by `rr_campaign control`): each becomes a ledger
    /// `control {requested}` record followed by `applied {effect}` or `refused {why}` (ledger 5.7). The ledger decides
    /// legality (a retry / skip the item machine does not allow is refused). A request the ledger cannot even
    /// represent (unparsable file, unknown verb, unknown target) is recorded as a `note` starting `control_refused`.
    fn apply_controls(&mut self) {
        let dir = self.cdir.join("control");
        let Ok(rd) = std::fs::read_dir(&dir) else {
            return;
        };
        let mut files: Vec<PathBuf> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == "json"))
            .collect();
        // gate-B m1: in the order of the CLI's sequence number (then the name); files without one first, by name.
        files.sort_by_key(|p| {
            let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            (control_seq_of(&name), name)
        });
        for p in files {
            let name = p.file_name().unwrap().to_owned();
            let done = dir.join("done");
            let _ = std::fs::create_dir_all(&done);
            let req: Option<ControlRequest> = std::fs::read(&p)
                .ok()
                .and_then(|b| serde_json::from_slice(&b).ok());
            if let Some(r) = req.as_ref().filter(|r| r.verb == "resolve") {
                self.apply_resolve(r);
                if self.ledger_dead.is_some() {
                    return;
                }
                let _ = std::fs::rename(&p, done.join(&name));
                continue;
            }
            match req.as_ref().map(|r| (r, self.control_record(r))) {
                None => self.note(format!(
                    "control_refused {{\"file\":{name:?},\"reason\":\"unparsable\"}}"
                )),
                Some((r, Err(why))) => self.note(format!(
                    "control_refused {{\"id\":{:?},\"verb\":{:?},\"target\":{:?},\"reason\":{why:?}}}",
                    r.id, r.verb, r.target
                )),
                Some((r, Ok((verb, target)))) => self.apply_control(r, verb, target),
            }
            if self.ledger_dead.is_some() {
                return;
            }
            let _ = std::fs::rename(&p, done.join(&name));
        }
    }

    /// `resolve JOB` (requirement 05:23Z): the operator's decision for an attempt whose execution is UNKNOWN (journalled,
    /// unconfirmed launch, not authenticated by its run directory): lost + requeue of exactly that attempt. Refused for
    /// anything else. Recorded as a note (the ledger has no `resolve` control yet: HOOK NEEDED FROM LEDGERFIX2).
    fn apply_resolve(&mut self, r: &ControlRequest) {
        let target = r.target.clone().unwrap_or_default();
        let ok = self.unknown.get(&target).copied().filter(|a| {
            self.st()
                .jobs
                .get(&target)
                .is_some_and(|j| j.attempt == *a && j.state == AttemptState::Launching)
        });
        let Some(attempt) = ok else {
            self.note(format!(
                "control_refused {{\"id\":{:?},\"verb\":\"resolve\",\"target\":{target:?},\"reason\":\"not an attempt of unknown execution\"}}",
                r.id
            ));
            return;
        };
        let by = if r.by.is_empty() { "operator" } else { &r.by };
        let reason = r.reason.clone().unwrap_or_default();
        let mut ev = vec![Event::Note {
            text: format!(
                "control_resolve {{\"id\":{:?},\"job\":{target:?},\"attempt\":{attempt},\"by\":{by:?},\"reason\":{reason:?}}}",
                r.id
            ),
        }];
        ev.extend(not_an_attempt(
            &target,
            attempt,
            &format!("execution unknown; resolved by {by} (requeue): {reason}"),
        ));
        if self.commit(ev) {
            self.unknown.remove(&target);
            self.log
                .line("INFO", &format!("control {}: {target} a{attempt} resolved: requeued", r.id));
        }
    }

    /// The ledger form of a request, or why it has none.
    fn control_record(&self, r: &ControlRequest) -> Result<(ControlVerb, ControlTarget), String> {
        if r.id.is_empty() || r.id.len() > 128 {
            return Err("bad id".into());
        }
        let verb = match r.verb.as_str() {
            "pause" => ControlVerb::Pause,
            "resume" => ControlVerb::Resume,
            "stop" => ControlVerb::Stop,
            "retry" => ControlVerb::Retry,
            "skip" => ControlVerb::Skip,
            v => return Err(format!("unknown verb {v}")),
        };
        let st = self.st();
        let target = match (verb, r.target.as_deref()) {
            (ControlVerb::Pause | ControlVerb::Resume | ControlVerb::Stop, None) => {
                ControlTarget::Campaign
            }
            (ControlVerb::Pause | ControlVerb::Resume, Some(t)) if st.nodes.contains_key(t) => {
                ControlTarget::Node { node: t.into() }
            }
            (_, Some(t)) if verb != ControlVerb::Stop && st.items.contains_key(t) => {
                ControlTarget::Item { item: t.into() }
            }
            (ControlVerb::Retry | ControlVerb::Skip, None) => {
                return Err(format!("{} needs an item", r.verb));
            }
            (_, Some(t)) => return Err(format!("unknown target {t}")),
        };
        if let Some(c) = st.controls.get(&r.id)
            && c.status != "requested"
        {
            return Err(format!("control id {} already {}", r.id, c.status));
        }
        Ok((verb, target))
    }

    fn apply_control(&mut self, r: &ControlRequest, verb: ControlVerb, target: ControlTarget) {
        let id = r.id.clone();
        let requested = Event::Control {
            id: id.clone(),
            ev: ControlEvent::Requested {
                verb,
                target: target.clone(),
                by: if r.by.is_empty() {
                    "operator".into()
                } else {
                    r.by.clone()
                },
                reason: r.reason.clone().unwrap_or_default(),
            },
        };
        match self.commit_or_refused(vec![requested]) {
            Ok(true) => {}
            Ok(false) => return,
            Err(why) => {
                self.note(format!(
                    "control_refused {{\"id\":{id:?},\"verb\":{:?},\"reason\":{why:?}}}",
                    r.verb
                ));
                return;
            }
        }
        let what = match &target {
            ControlTarget::Campaign => "campaign".to_string(),
            ControlTarget::Node { node } => format!("node {node}"),
            ControlTarget::Item { item } => format!("item {item}"),
        };
        let effect = match verb {
            ControlVerb::Pause => format!("{what} paused: no new launch (running jobs continue)"),
            ControlVerb::Resume => format!("{what} resumed"),
            ControlVerb::Stop => "stop requested: draining".to_string(),
            ControlVerb::Retry => format!("{what} reopened by the operator (not a failure)"),
            ControlVerb::Skip => format!("{what} skipped by the operator"),
        };
        let applied = Event::Control {
            id: id.clone(),
            ev: ControlEvent::Applied {
                effect: effect.clone(),
            },
        };
        match self.commit_or_refused(vec![applied]) {
            Ok(true) => {
                self.log.line("INFO", &format!("control {id}: {effect}"));
                if verb == ControlVerb::Stop && self.signals.handle.level() == StopLevel::Running {
                    self.control_stop = true;
                    self.signals.handle.request_stop();
                }
                if matches!(verb, ControlVerb::Retry | ControlVerb::Skip)
                    && let ControlTarget::Item { item } = &target
                {
                    // The work source's follow-up (none for the synthetic source).
                    match self.source.control(self.ledger.state(), &r.verb, item) {
                        Ok(ev) => {
                            self.commit(ev);
                        }
                        Err(e) => self.log.line("WARN", &format!("control {id}: {e}")),
                    }
                }
            }
            Ok(false) => {}
            Err(why) => {
                self.log
                    .line("INFO", &format!("control {id} refused: {why}"));
                self.commit(vec![Event::Control {
                    id,
                    ev: ControlEvent::Refused { why },
                }]);
            }
        }
    }

    /// CDIR and CDIR/ledger are still the directories the ledger was opened in, and the manifest is there.
    fn dirs_intact(&self) -> Result<(), String> {
        let ldir = crate::ledger::ledger_dir(&self.cdir);
        let now = dir_id(&self.cdir).zip(dir_id(&ldir));
        if self.dir_ids.is_none() || now != self.dir_ids {
            return Err(format!(
                "the campaign directory {} (or its ledger directory) was removed or replaced while the supervisor ran",
                self.cdir.display()
            ));
        }
        if !self.lock.as_ref().is_some_and(|l| l.is_intact()) {
            return Err(format!(
                "the campaign lock file in {} was removed or replaced while the supervisor ran",
                self.cdir.display()
            ));
        }
        if !ldir.join(crate::ledger::MANIFEST_FILE).exists() {
            return Err(format!(
                "{} disappeared while the supervisor ran",
                ldir.join(crate::ledger::MANIFEST_FILE).display()
            ));
        }
        Ok(())
    }

    /// Every 2 s: a removed / replaced campaign directory stops the supervisor as an internal error (no new launch;
    /// running jobs get the grace, then their groups are killed).
    fn check_dirs(&mut self) {
        if self
            .last_dir_check
            .is_some_and(|t| t.elapsed() < Duration::from_secs(2))
        {
            return;
        }
        self.last_dir_check = Some(Instant::now());
        if let Err(e) = self.dirs_intact() {
            self.log.line("ERROR", &format!("{e}; stopping (exit 70)"));
            self.ledger_dead = Some(RunExit::Internal(e));
        }
    }

    /// The END-OF-CAMPAIGN INVARIANT (procfix3 item 0): exit 0 only when the ledger ON DISK, re-read read-only, ends
    /// at the record this supervisor acknowledged last, reduces to the same final result as the live state, and every
    /// accepted record file still has its recorded bytes and digest. Anything else is an internal error: "finished
    /// with fewer accepted items than the ledger acknowledged" is impossible by construction.
    fn final_consistency(&mut self) -> Result<(), String> {
        self.ledger
            .sync()
            .map_err(|e| format!("final ledger sync: {e}"))?;
        self.dirs_intact()?;
        let view = crate::ledger::open_read_only(&self.cdir)
            .map_err(|e| format!("re-reading the ledger read-only: {e}"))?;
        if view.seq != self.ledger.seq() || view.head != self.ledger.head() {
            return Err(format!(
                "the ledger on disk ends at seq {} but seq {} was acknowledged",
                view.seq,
                self.ledger.seq()
            ));
        }
        let disk = result::final_result(&view.state);
        let live = result::final_result(self.ledger.state());
        if disk.accepted.len() < live.accepted.len() {
            return Err(format!(
                "the ledger on disk has {} accepted items, {} were acknowledged",
                disk.accepted.len(),
                live.accepted.len()
            ));
        }
        if disk != live {
            return Err("the ledger on disk reduces to another final result than the live state".into());
        }
        for (item, it) in &view.state.items {
            if let Some(a) = &it.accepted
                && matches!(it.phase, crate::state::ItemPhase::Accepted(_))
                && !a.record.path.starts_with("large:")
            {
                let p = self.cdir.join(&a.record.path);
                match crate::atomic::blake3_file(&p) {
                    Ok((bytes, d)) if bytes == a.record.bytes && d == a.record.blake3 => {}
                    Ok(_) => return Err(format!("accepted record of {item} ({}) changed", p.display())),
                    Err(e) => return Err(format!("accepted record of {item} ({}): {e}", p.display())),
                }
            }
        }
        Ok(())
    }

    fn check_disk(&mut self) {
        if self
            .last_disk_check
            .is_some_and(|t| t.elapsed() < Duration::from_secs(5))
        {
            return;
        }
        self.last_disk_check = Some(Instant::now());
        let floor = self.opts.spec.resources.disk_floor.0;
        for p in [self.cdir.clone(), self.work_root.clone()] {
            if let Some(a) = statvfs_avail(&p)
                && a < floor
                && !self.disk_floor_hit
            {
                self.log.line(
                    "ERROR",
                    &format!(
                        "disk below floor: {} has {a} bytes free < {floor}; draining (free space, then run the same command)",
                        p.display()
                    ),
                );
                self.disk_floor_hit = true;
                self.signals.handle.request_stop();
            }
        }
    }

    fn blockers(&mut self) -> Blockers {
        let mut b = Blockers::default();
        b.reasons.extend(self.policy.launch_blockers());
        if !self.sampler_fresh() {
            b.reasons
                .push("process sampler stale (> 30 s): no launch".into());
        }
        if self.ledger_dead.is_some() {
            b.reasons.push("ledger unavailable".into());
        }
        if let Some(t) = self.resource_backoff_until
            && Instant::now() < t
        {
            b.reasons.push(format!(
                "supervisor resource back-off after {} failure(s) ({} s left)",
                self.resource_failures,
                t.saturating_duration_since(Instant::now()).as_secs() + 1
            ));
        }
        if self.st().paused {
            b.reasons.push(
                "campaign paused by an operator control (rr_campaign control CDIR resume)".into(),
            );
        }
        let joined = b.reasons.join("; ");
        if joined != self.last_block_log {
            if !joined.is_empty() {
                self.log
                    .line("INFO", &format!("launching paused: {joined}"));
            } else if !self.last_block_log.is_empty() {
                self.log.line("INFO", "launching resumed");
            }
            self.last_block_log = joined;
        }
        b
    }

    fn schedule(&mut self) {
        let max_jobs = self.opts.spec.resources.max_jobs;
        let busy = self.children.len() + self.pending.len();
        if busy >= max_jobs {
            return;
        }
        let limit = (max_jobs - busy) * 4 + 8;
        let running_ids: Vec<String> = self.children.iter().map(|c| c.job.clone()).collect();
        let st = self.ledger.state();
        // Paused nodes / items (operator controls) get no launch (the ledger would refuse it).
        let paused = |item: &str| {
            st.items
                .get(item)
                .is_some_and(|it| it.paused || st.nodes.get(&it.node).is_some_and(|n| n.paused))
        };
        let reqs: Vec<JobRequest> = self
            .source
            .ready(st, &self.ctx, limit)
            .into_iter()
            .filter(|r| {
                !running_ids.contains(&r.job)
                    && !paused(&r.item)
                    && !self.unknown.contains_key(&r.job)
                    && !self.pending.iter().any(|p| p.job == r.job)
            })
            .collect();
        self.last_ready = reqs.len();
        if reqs.is_empty() {
            return;
        }
        // Times on one monotonic axis (origin = this supervisor's start): a measurement is used only with its age
        // (admission charges the full cap for a stale or missing one; audit-proc-A M3).
        let base = self.started_at;
        let snap_at = self
            .sampler
            .latest()
            .map(|s| s.started_at.saturating_duration_since(base));
        let running: Vec<RunningJob> = self
            .children
            .iter()
            .map(|c| {
                let usage = c.handle.usage();
                RunningJob {
                    id: u64::MAX - c.seq,
                    cap_bytes: c.cap,
                    expected_peak_bytes: c.peak.min(c.cap),
                    rss_bytes: usage.as_ref().map_or(0, |u| u.rss_bytes),
                    rss_measured_at: usage.and(snap_at),
                    cores: c.cores,
                    started_seq: c.seq,
                    cpu_seconds: 0.0,
                    stopping: c.stopping,
                }
            })
            .chain(self.pending.iter().map(|p| RunningJob {
                // Finished, not settled: its whole cap stays reserved and its cores leased until the settle.
                id: u64::MAX - p.seq,
                cap_bytes: p.cap,
                expected_peak_bytes: p.cap,
                rss_bytes: 0,
                rss_measured_at: None,
                cores: p.cores,
                started_seq: p.seq,
                cpu_seconds: 0.0,
                stopping: true,
            }))
            .collect();
        let cands: Vec<Candidate> = reqs
            .iter()
            .enumerate()
            .map(|(i, r)| Candidate {
                id: i as u64,
                expected_peak_bytes: r.expected_peak_bytes.min(r.cap_bytes),
                cap_bytes: r.cap_bytes,
                cores: r.cores,
                bypassed: self.bypassed.get(&r.job).copied().unwrap_or(0),
            })
            .collect();
        let now = base.elapsed();
        let host = HostView {
            mem_available_bytes: host_memory().map(|m| m.effective_available()).unwrap_or(0),
            measured_at: now,
            now,
        };
        let out = admission::decide_batch(&self.admission, host, &running, &cands);
        for b in &out.bypassed {
            *self
                .bypassed
                .entry(reqs[*b as usize].job.clone())
                .or_default() += 1;
        }
        let mut first_wait = None;
        for (id, d) in &out.decisions {
            let r = &reqs[*id as usize];
            match d {
                Decision::Admit => {
                    if self.children.len() + self.pending.len() >= max_jobs
                        || self.ledger_dead.is_some()
                    {
                        break;
                    }
                    match self
                        .policy
                        .acquire(r.cores, &format!("{} {}", self.ctx.campaign_key, r.job))
                    {
                        Ok(Acquire::Granted(lease)) => {
                            self.bypassed.remove(&r.job);
                            if let Err(e) = self.launch(r, lease) {
                                self.log.line("WARN", &format!("{}: {e}", r.job));
                                if self.resource_backoff_until.is_some() {
                                    // gate-B M3: a supervisor resource failure ends this pass (back-off).
                                    break;
                                }
                            }
                        }
                        Ok(Acquire::NotNow(why)) => {
                            first_wait.get_or_insert(why);
                            break;
                        }
                        Err(e) => {
                            first_wait.get_or_insert(e.to_string());
                            break;
                        }
                    }
                }
                Decision::Wait(w) => {
                    first_wait.get_or_insert(format!("{w:?}"));
                }
            }
        }
        // Log memory / policy waits once per change (the core budget being full is the normal state).
        let w = first_wait
            .filter(|w| !w.starts_with("CoreBudget") && !w.contains("core budget"))
            .unwrap_or_default();
        if w != self.last_wait_log {
            if !w.is_empty() {
                self.log.line("INFO", &format!("admission waits: {w}"));
            }
            self.last_wait_log = w;
        }
    }

    /// Writes the progress files: on every ledger change (at most every `progress.cadence_seconds`), at the cadence
    /// while jobs run (live samples), and at least every [`HEARTBEAT_EVERY`] even when nothing changed, so that
    /// `heartbeat.json` of an idle live supervisor never looks stale (ledger interface 9: at least every 10 s).
    fn write_progress(&mut self, force: bool) {
        let seq = self.ledger.seq();
        let cadence = Duration::from_secs(self.opts.spec.progress.cadence_seconds);
        let due = match self.last_progress {
            None => true,
            Some((t, s)) => {
                let e = t.elapsed();
                force
                    || e >= HEARTBEAT_EVERY
                    || (e >= cadence && (s != seq || !self.children.is_empty()))
            }
        };
        if !due {
            return;
        }
        let res = self.resources();
        self.fp.hit("progress_write");
        match self.progress.write(
            &self.cdir,
            self.ledger.state(),
            self.ledger.head(),
            &res,
            self.clock.as_ref(),
        ) {
            Ok(_) => self.last_progress = Some((Instant::now(), seq)),
            Err(e) => self.log.line("WARN", &format!("progress files: {e}")),
        }
    }

    /// The resource inputs of the progress files (ledger interface 9): measured values or `None` (never a 0 that
    /// means "not measured"); no sample ever goes into the ledger.
    fn resources(&self) -> ProgressResources {
        let mem = host_memory().ok();
        let now = self.clock.now_utc();
        let sampled = self.sampler.latest().is_some();
        let jobs: Vec<LiveJob> = self
            .children
            .iter()
            .map(|c| {
                let usage = c.handle.usage();
                let id = c.handle.identity();
                LiveJob {
                    job: c.job.clone(),
                    attempt: c.attempt,
                    cpu_set: c
                        .lease
                        .as_ref()
                        .map(|l| l.cpus().iter().map(|&x| x as u32).collect())
                        .unwrap_or_default(),
                    rss_bytes: usage.map(|u| u.rss_bytes),
                    budget_mem_bytes: Some(c.cap),
                    seconds: Some(c.launched_at.elapsed().as_secs()),
                    budget_seconds: c.budget_s,
                    heartbeat_utc: c.last_event_utc.clone(),
                    pid: Some(id.pid),
                    // The launcher makes every child the leader of its own process group.
                    pgid: Some(id.pid),
                    phase: c.phase.clone(),
                }
            })
            .collect();
        let rss: Option<u64> = if self.children.is_empty() {
            sampled.then_some(0)
        } else {
            jobs.iter()
                .map(|j| j.rss_bytes)
                .try_fold(0u64, |a, x| x.map(|x| a + x))
        };
        let grace = self.opts.spec.resources.grace_seconds;
        let stopping = self.signals.handle.level() > StopLevel::Running;
        ProgressResources {
            cpus_in_use: Some(self.policy.in_use() as u32),
            cpus_allowed: Some(self.policy.budget() as u32),
            live_jobs: Some(self.children.len() as u32),
            reserved_bytes: Some(
                self.children.iter().map(|c| c.cap).sum::<u64>()
                    + self.pending.iter().map(|p| p.cap).sum::<u64>(),
            ),
            measured_rss_bytes: rss,
            mem_cap_bytes: self.admission.campaign_mem_cap_bytes,
            host_mem_available_bytes: mem.map(|m| m.effective_available()),
            host_floor_bytes: Some(self.admission.floor_bytes),
            disk_free_bytes: statvfs_avail(&self.cdir),
            large_free_bytes: statvfs_avail(&self.work_root),
            budget_cpu_hours: None,
            sampled_utc: Some(now.to_iso()),
            supervisor: Some(SupervisorInfo {
                heartbeat_utc: Some(now.to_iso()),
                started_utc: Some(self.started_utc.clone()),
                pid: Some(std::process::id()),
                host: Some(
                    self.lock
                        .as_ref()
                        .map_or_else(String::new, |l| l.owner.identity.host.clone()),
                ),
                grace_left_s: self
                    .drain_started
                    .map(|t| grace.saturating_sub(t.elapsed().as_secs())),
                stopping,
            }),
            jobs,
        }
    }

    fn stop_code(&self) -> i32 {
        if self.signals.hup.load(Ordering::SeqCst) {
            exit::SIGHUP
        } else if self.signals.term.load(Ordering::SeqCst) {
            exit::SIGTERM
        } else if self.control_stop {
            exit::CONTROL_STOP
        } else {
            exit::SIGINT
        }
    }

    fn main_loop(&mut self) -> RunExit {
        self.recover();
        if let Some(e) = self.ledger_dead.clone() {
            return e;
        }
        if let Err(e) = self.register_binaries() {
            return e;
        }
        match self.source.bootstrap(self.ledger.state(), &self.ctx) {
            Ok(ev) => {
                if !self.commit(ev) {
                    return self.ledger_dead.clone().unwrap_or(RunExit::DiskFull);
                }
            }
            Err(e) => return RunExit::Internal(format!("work source: {e}")),
        }
        self.commit(vec![Event::Campaign {
            ev: crate::state::CampaignEvent::Run,
            reason: "recovered; scheduling".into(),
        }]);
        let summary = format!(
            "{}; {} jobs settled so far, {} accepted, {} terminal",
            self.source.describe(),
            self.st().jobs.len(),
            self.st().rules.accepted,
            self.st().terminals_total
        );
        self.log.line("INFO", &summary);
        self.write_progress(true);
        let mut level = StopLevel::Running;
        let mut dead_deadline: Option<Instant> = None;
        loop {
            // 1. stop level
            let now_level = self.signals.handle.level();
            if now_level > level {
                if level == StopLevel::Running {
                    self.drain_started = Some(Instant::now());
                    self.log.line(
                        "INFO",
                        &format!(
                            "stop requested: draining ({} running job(s) get {} s, then their groups are killed); press Ctrl-C again to stop at once",
                            self.children.len(),
                            self.opts.spec.resources.grace_seconds
                        ),
                    );
                    let reason = self
                        .resource_stop
                        .clone()
                        .unwrap_or_else(|| "stop request".into());
                    self.commit(vec![Event::Campaign {
                        ev: crate::state::CampaignEvent::Drain,
                        reason,
                    }]);
                    let g = Duration::from_secs(self.opts.spec.resources.grace_seconds + 5);
                    for c in &mut self.children {
                        c.stopping = true;
                        c.handle.terminate(g, KillReason::Stop);
                    }
                }
                if now_level == StopLevel::Force {
                    self.log
                        .line("INFO", "second stop request: killing every job's group now");
                    for c in &mut self.children {
                        c.stopping = true;
                        c.handle.kill_now(KillReason::ForceStop);
                    }
                }
                level = now_level;
            }
            if self.signals.handle.count() >= 3 {
                for c in &mut self.children {
                    c.handle.kill_now(KillReason::ForceStop);
                }
                self.log.line(
                    "INFO",
                    "third stop request: exiting now (children killed; resume handles the rest)",
                );
                return RunExit::Stopped {
                    code: self.stop_code(),
                };
            }
            // 2. controls, disk
            if self.ledger_dead.is_none() {
                self.apply_controls();
                self.check_disk();
                self.check_dirs();
            }
            // 3. children (and the launcher's quarantine of unproven job ends, the attempts waiting to be settled, the
            // periodic campaign-wide sweep)
            self.poll_children();
            self.tick_quarantine();
            self.drive_pending();
            if self.ledger_dead.is_none() {
                self.periodic_sweep();
            }
            // ledger dead: no launches; children get the grace, then are killed (their attempts stay unsettled).
            if let Some(e) = self.ledger_dead.clone() {
                let d = *dead_deadline.get_or_insert_with(|| {
                    let g = Duration::from_secs(self.opts.spec.resources.grace_seconds);
                    for c in &mut self.children {
                        c.stopping = true;
                        c.handle.terminate(g, KillReason::Stop);
                    }
                    Instant::now() + g + Duration::from_secs(10)
                });
                if (self.children.is_empty() && self.pending.iter().all(|p| p.sweep.is_none()))
                    || Instant::now() > d
                {
                    return e;
                }
                self.write_progress(false);
                std::thread::sleep(self.opts.tick);
                continue;
            }
            // 4-5. schedule
            if level == StopLevel::Running {
                let b = self.blockers();
                if b.reasons.is_empty() {
                    self.schedule();
                }
            }
            // 6. progress
            self.write_progress(false);
            // 7. exit conditions
            if self.children.is_empty() && level > StopLevel::Running {
                // Draining: attempts still waiting for a proof stay unsettled (the restart's recovery settles them).
                self.pending.retain(|p| {
                    !p.awaiting_proof || p.sweep.is_some()
                });
            }
            if self.children.is_empty() && self.pending.is_empty() {
                if level > StopLevel::Running {
                    self.commit(vec![Event::Campaign {
                        ev: crate::state::CampaignEvent::Stop,
                        reason: "drained".into(),
                    }]);
                    self.write_progress(true);
                    if self.disk_floor_hit {
                        return RunExit::DiskFloor;
                    }
                    if let Some(m) = self.resource_stop.clone() {
                        return RunExit::Resources(m);
                    }
                    return RunExit::Stopped {
                        code: self.stop_code(),
                    };
                }
                if self.source.finished(self.ledger.state()) {
                    self.final_sweep();
                    if self.st().phase == Some(CampaignPhase::Running) {
                        self.commit(vec![Event::Campaign {
                            ev: crate::state::CampaignEvent::Finish,
                            reason: "all work final".into(),
                        }]);
                    }
                    self.write_progress(true);
                    if let Err(e) = self.final_consistency() {
                        self.log.line("ERROR", &format!("NOT FINISHED: {e}"));
                        return RunExit::Internal(e);
                    }
                    return RunExit::Finished;
                }
                // Nothing runs, nothing is ready, and attempts of unknown execution block the rest: an operator decides.
                if level == StopLevel::Running
                    && !self.unknown.is_empty()
                    && self.last_ready == 0
                    && self.resource_backoff_until.is_none_or(|t| Instant::now() >= t)
                {
                    let jobs: Vec<String> = self
                        .unknown
                        .iter()
                        .map(|(j, a)| format!("{j} a{a}"))
                        .collect();
                    let msg = format!(
                        "operator decision needed: execution unknown for {}; `rr_campaign control CDIR resolve JOB` requeues one",
                        jobs.join(", ")
                    );
                    self.log.line("WARN", &msg);
                    self.commit(vec![Event::Campaign {
                        ev: crate::state::CampaignEvent::Stop,
                        reason: msg.clone(),
                    }]);
                    self.write_progress(true);
                    return RunExit::Attention(msg);
                }
            }
            std::thread::sleep(self.opts.tick);
        }
    }

    fn finish_run(&mut self, r: &RunExit) {
        for ev in self.launcher.shutdown(Duration::from_secs(10)) {
            if let crate::launch::RetireEvent::Cleared { label, .. } = ev {
                self.log
                    .line("INFO", &format!("{label}: end proven at shutdown"));
            }
        }
        for (label, since, reason) in self.launcher.quarantined() {
            self.log.line(
                "WARN",
                &format!(
                    "{label}: end still unproven after {}s at exit ({reason}); the restart's orphan scan looks for its marker",
                    since.as_secs()
                ),
            );
        }
        let _ = self.ledger.sync();
        let msg = match r {
            RunExit::Finished => format!(
                "FINISHED: {} accepted, {} terminal(s)",
                self.st().rules.accepted,
                self.st().terminals_total
            ),
            RunExit::Stopped { code } => {
                format!("STOPPED (exit {code}); resume with the same command")
            }
            other => format!("{other:?}"),
        };
        self.log.line("INFO", &msg);
        if let Some(l) = self.lock.take() {
            let _ = l.release();
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct ControlRequest {
    pub schema: String,
    pub id: String,
    pub utc: String,
    #[serde(default)]
    pub by: String,
    pub verb: String,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

pub const CONTROL_SCHEMA: &str = "rustred.campaign.control.v1";

/// The sequence number of a control request file `c-<seq, 12 digits>-...json` (written by [`next_control_seq`]); 0 for
/// other names (older format: applied first, by name).
pub fn control_seq_of(name: &str) -> u64 {
    name.strip_prefix("c-")
        .and_then(|r| r.split('-').next())
        .filter(|d| d.len() == 12 && d.bytes().all(|b| b.is_ascii_digit()))
        .and_then(|d| d.parse().ok())
        .unwrap_or(0)
}

/// The next control request sequence number of `CDIR/control` (gate-B m1): a counter file `control/SEQ` read and
/// rewritten under an exclusive flock of `control/SEQ.lock`, so two `rr_campaign control` processes in the same second
/// get distinct, ordered numbers. Never touches the campaign lock or the ledger.
pub fn next_control_seq(control_dir: &Path) -> Result<u64, String> {
    use std::io::{Read, Seek};
    use std::os::fd::AsRawFd;
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(control_dir.join("SEQ.lock"))
        .map_err(|e| format!("control/SEQ.lock: {e}"))?;
    // SAFETY: flock on our own open descriptor; released when `lock` is closed.
    if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX) } != 0 {
        return Err(format!("flock control/SEQ.lock: {}", std::io::Error::last_os_error()));
    }
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(control_dir.join("SEQ"))
        .map_err(|e| format!("control/SEQ: {e}"))?;
    let mut text = String::new();
    f.read_to_string(&mut text).map_err(|e| format!("control/SEQ: {e}"))?;
    let next = text.trim().parse::<u64>().unwrap_or(0) + 1;
    f.set_len(0).map_err(|e| format!("control/SEQ: {e}"))?;
    f.rewind().map_err(|e| format!("control/SEQ: {e}"))?;
    f.write_all(format!("{next}\n").as_bytes())
        .map_err(|e| format!("control/SEQ: {e}"))?;
    f.sync_all().map_err(|e| format!("control/SEQ: {e}"))?;
    Ok(next)
}

/// The attempt's spec and the digest its LAUNCH RECORD names (`job.launch.spec_blake3`, fsynced before the spawn):
/// the classification binds OUTCOME / RESUME to that digest, never to a re-read of job.json; a job.json whose bytes
/// no longer have that digest is refused (None).
fn read_spec(layout: &RunLayout, launched: Option<&str>) -> Option<(JobSpec, String)> {
    let d = launched?.to_string();
    if !rustred_campaign_contract::is_blake3_hex(&d) {
        return None;
    }
    let b = std::fs::read(layout.job_spec()).ok()?;
    if rustred_campaign_contract::spec::spec_blake3(&b) != d {
        return None;
    }
    let spec = JobSpec::from_json(&b).ok()?;
    Some((spec, d))
}

/// RESUME of a previous attempt directory, when valid (child-job 3.4.4 item 3); `launched` = that attempt's
/// launch-record digest.
fn resume_from(prev: &Path, launched: Option<&str>) -> Option<ResumeRef> {
    let layout = RunLayout::new(prev);
    let (spec, digest) = read_spec(&layout, launched)?;
    // The LAST RESUME record of that attempt (contract audit C1), never sizes taken from OUTCOME.
    classify::resume_state(&layout, &spec, &digest)
        .ok()
        .map(|(_, r)| r)
}

fn file_ref(p: &Path, rel: String) -> Option<FileRef> {
    let (bytes, blake3) = crate::atomic::blake3_file(p).ok()?;
    Some(FileRef {
        path: rel,
        bytes,
        blake3,
    })
}

#[allow(clippy::too_many_arguments)]
fn finished_event(
    job: &str,
    attempt: u32,
    class: FinishClass,
    accept: ContractAccept,
    ev: &classify::Evidence,
    sealed: Option<FileRef>,
    outcome: Option<FileRef>,
    binding: Option<OutcomeBinding>,
    cpu_ms: u64,
    wall_ms: u64,
    peak: u64,
    time_decided: bool,
) -> Event {
    let (code, signal) = match ev.wait {
        WaitStatus::Exited(c) => (Some(c), None),
        WaitStatus::Signaled(s) => (None, Some(s)),
        WaitStatus::Unknown => (None, None),
    };
    Event::Job {
        job: job.into(),
        attempt,
        ev: JobEvent::Finished {
            class,
            accept,
            exit_code: code,
            signal,
            sealed_output: sealed,
            outcome,
            binding,
            cpu_ms,
            wall_ms,
            peak_rss_bytes: peak,
            time_decided,
        },
    }
}

/// The sealed table as read ONCE by [`read_verified_table`].
#[derive(Debug)]
struct BoundTable {
    rel: String,
    bytes: u64,
    blake3: String,
    canonical: String,
    lines: Vec<String>,
}

/// ac2-M1: the table that `assess` verified, re-opened through the contract's verified-stream handle
/// (`seal::VerifiedHandle::reopen`: run-relative no-follow open, the verified (dev, inode) or refused) and read ONCE:
/// `canonical_lines()` returns the lines only if the bytes, raw BLAKE3, record count and canonical digest equal the
/// verified stream, every line with its canonical form (no silent drop): all or nothing.
fn read_verified_table(
    layout: &RunLayout,
    verified: Option<&classify::VerifiedOutputs>,
) -> Result<BoundTable, String> {
    let st = verified
        .and_then(|v| v.streams.first())
        .ok_or("no verified stream for the table")?;
    let h = rustred_campaign_contract::seal::VerifiedHandle::reopen(&layout.root, st)
        .map_err(|e| format!("{}: {e:?}", st.rel))?;
    let lines = h
        .canonical_lines()
        .map_err(|e| format!("{} changed after it was verified: {e:?}", st.rel))?
        .into_iter()
        .map(|c| String::from_utf8(c).map_err(|_| format!("{}: canonical line not UTF-8", st.rel)))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(BoundTable {
        rel: st.rel.clone(),
        bytes: st.bytes,
        blake3: st.blake3.clone(),
        canonical: st.canonical_blake3.clone(),
        lines,
    })
}

/// Decision row 3 (action `confirm_coverage_else_next_rung`): `Some(routing)` when the work source's own coverage
/// check confirms the child's per-case claims; otherwise the classification's action becomes `next_rung` (row 24
/// semantics) and the caller routes as usual. Other rows: `None`, unchanged.
fn row3_routing(
    source: &mut dyn WorkSource,
    st: &CampaignState,
    ctx: &SourceContext,
    settled: &mut Settled,
) -> Option<worksource::Routing> {
    if settled.classification.action != classify::Action::ConfirmCoverageElseNextRung {
        return None;
    }
    match source.confirm_coverage(st, ctx, settled) {
        Ok(Some(events)) => Some(worksource::Routing {
            events,
            accepts: Vec::new(),
        }),
        Ok(None) | Err(_) => {
            settled.classification.action = classify::Action::NextRung;
            None
        }
    }
}

/// OUTCOME read ONCE through the run-relative no-follow open; its parse must equal the OUTCOME the classification used.
fn read_bound_outcome(
    layout: &RunLayout,
    verified: &rustred_campaign_contract::outcome::JobOutcome,
) -> Result<(u64, String), String> {
    use std::io::Read;
    let rel = rustred_campaign_contract::layout::OUTCOME;
    let (mut f, _, _) = rustred_campaign_contract::seal::open_in_run(&layout.root, rel)
        .map_err(|e| format!("open {rel}: {e:?}"))?;
    let mut b = Vec::new();
    f.read_to_end(&mut b).map_err(|e| format!("read {rel}: {e}"))?;
    let o = rustred_campaign_contract::outcome::Outcome::from_slice(&b)?;
    if o.job != *verified {
        return Err("its content is not the classified OUTCOME".into());
    }
    Ok((b.len() as u64, blake3::hash(&b).to_hex().to_string()))
}

/// `RUN/sup/SPAWN`, created and fsynced (file and directory) right before the spawn.
fn write_spawn_sentinel(layout: &RunLayout) -> std::io::Result<()> {
    let dir = layout.sup_dir();
    let f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(dir.join(SPAWN_SENTINEL))?;
    f.sync_all()?;
    std::fs::File::open(&dir)?.sync_all()
}

/// The records of a launch that never became a job attempt (the spawn was not attempted, or failed for the
/// supervisor's own resources): lost + requeue in the SAME job (no new job id; never counted as a failure).
fn not_an_attempt(job: &str, attempt: u32, why: &str) -> Vec<Event> {
    vec![
        Event::Job {
            job: job.into(),
            attempt,
            ev: JobEvent::Lose { reason: why.into() },
        },
        Event::Job {
            job: job.into(),
            attempt: attempt + 1,
            ev: JobEvent::Requeue {
                reason: "not a job attempt".into(),
            },
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustred_campaign_contract::seal::{self, RecordWriter, TABLE_END_TYPES};

    fn run_dir(tag: &str) -> RunLayout {
        let d = std::env::temp_dir().join(format!(
            "rr-procfix3-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let l = RunLayout::new(&d);
        std::fs::create_dir_all(l.out_dir()).unwrap();
        l
    }

    fn sealed_table(l: &RunLayout, n: u64) {
        let mut w = RecordWriter::create(&l.table()).unwrap();
        w.append(br#"{"type":"header","zero_seconds":0.25}"#).unwrap();
        for i in 0..n {
            w.append(
                format!(r#"{{"type":"rule","chart":"11","index":{i},"case":"c{i}","target":"I({i})","rhs":[["I(0)","-1/2"]]}}"#)
                    .as_bytes(),
            )
            .unwrap();
        }
        w.seal(format!(r#"{{"type":"end","rules":{n},"seconds":1.5}}"#).as_bytes())
            .unwrap();
    }

    fn verified_of(l: &RunLayout) -> classify::VerifiedOutputs {
        let rel = l
            .table()
            .strip_prefix(&l.root)
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let (v, _) = seal::verify_stream_handle(&l.root, &rel, &TABLE_END_TYPES).unwrap();
        classify::VerifiedOutputs {
            rules: v.rules,
            elided_loci: 0,
            streams: vec![v],
        }
    }

    /// procfix3 item 5 (ac2-M1): the settle reads exactly the verified bytes, or nothing. Honest: the lines and the
    /// verified canonical digest. Appended garbage, an appended valid line, or a table replaced by another sealed table
    /// (atomic rename: another inode) after the classification: refused, nothing accepted.
    #[test]
    fn settle_read_is_bound_to_the_verified_stream() {
        let l = run_dir("bound");
        sealed_table(&l, 3);
        let v = verified_of(&l);
        let t = read_verified_table(&l, Some(&v)).unwrap();
        assert_eq!(t.lines.len(), 5);
        assert_eq!(t.canonical, v.streams[0].canonical_blake3);
        assert_eq!((t.bytes, t.blake3.as_str()), (v.streams[0].bytes, v.streams[0].blake3.as_str()));
        // (b) garbage appended after the classification
        {
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new().append(true).open(l.table()).unwrap();
            f.write_all(b"{\"type\":\"rule\",\"index\":99} trailing garbage\n").unwrap();
        }
        assert!(read_verified_table(&l, Some(&v)).is_err());
        // (a) replaced by another sealed table (rename: another inode)
        let other = run_dir("bound-other");
        sealed_table(&other, 4);
        std::fs::rename(other.table(), l.table()).unwrap();
        let e = read_verified_table(&l, Some(&v)).unwrap_err();
        assert!(e.contains("InodeChanged") || e.contains("changed"), "{e}");
        // no verified stream at all: nothing
        assert!(read_verified_table(&l, None).is_err());
        let _ = std::fs::remove_dir_all(&l.root);
        let _ = std::fs::remove_dir_all(&other.root);
    }

    struct Confirming;
    impl WorkSource for Confirming {
        fn bootstrap(&mut self, _: &CampaignState, _: &SourceContext) -> Result<Vec<Event>, String> {
            Ok(vec![])
        }
        fn ready(&mut self, _: &CampaignState, _: &SourceContext, _: usize) -> Vec<JobRequest> {
            vec![]
        }
        fn route(&mut self, _: &CampaignState, _: &SourceContext, _: &Settled) -> Result<worksource::Routing, String> {
            Err("route must not be called for a confirmed row 3".into())
        }
        fn control(&mut self, _: &CampaignState, _: &str, _: &str) -> Result<Vec<Event>, String> {
            Ok(vec![])
        }
        fn confirm_coverage(
            &mut self,
            _: &CampaignState,
            _: &SourceContext,
            done: &Settled,
        ) -> Result<Option<Vec<Event>>, String> {
            Ok(Some(vec![Event::Note {
                text: format!("coverage confirmed for {}", done.item),
            }]))
        }
        fn finished(&self, _: &CampaignState) -> bool {
            true
        }
        fn describe(&self) -> String {
            "confirming".into()
        }
    }

    fn row3_settled() -> Settled {
        let c = classify::Classification {
            row: 3,
            sub: String::new(),
            kind: classify::OutcomeKind::NoRule,
            accept: classify::Accept::Nothing,
            action: classify::Action::ConfirmCoverageElseNextRung,
            resume_from_r: false,
            resume_from: None,
            time_decided: false,
            attention: false,
            why: "row 3".into(),
        };
        Settled {
            job: "N0.i0000.L0-t0".into(),
            attempt: 1,
            item: "N0.i0000.L0".into(),
            rung: crate::state::Rung::default(),
            classification: c,
            table: None,
            canonical_lines: vec![],
            canonical_blake3: None,
        }
    }

    /// procfix3 / contractfix2 row 3: the per-case claims close nothing by themselves. A source whose coverage check
    /// confirms them closes the item with ITS events; the synthetic source (no coverage check) gets the attempt as
    /// row 24 (action next_rung) and escalates.
    #[test]
    fn row3_needs_the_sources_coverage_confirmation_else_next_rung() {
        let st = CampaignState::default();
        let ctx = SourceContext {
            binaries: BTreeMap::new(),
            campaign_key: "c".into(),
        };
        let mut s = row3_settled();
        let r = row3_routing(&mut Confirming, &st, &ctx, &mut s).expect("confirmed");
        assert_eq!(r.events.len(), 1);
        assert_eq!(s.classification.action, classify::Action::ConfirmCoverageElseNextRung);
        let mut synthetic = ScriptedWorkSource::new(
            serde_json::from_value(serde_json::json!({"items": 4, "seed": 1})).unwrap(),
        );
        let mut s = row3_settled();
        assert!(row3_routing(&mut synthetic, &st, &ctx, &mut s).is_none());
        assert_eq!(s.classification.action, classify::Action::NextRung);
        // any other row is untouched
        let mut s = row3_settled();
        s.classification.action = classify::Action::ItemDone;
        assert!(row3_routing(&mut Confirming, &st, &ctx, &mut s).is_none());
        assert_eq!(s.classification.action, classify::Action::ItemDone);
    }

    #[test]
    fn control_sequence_numbers_order_requests_of_one_second() {
        let d = run_dir("seq").root;
        std::fs::create_dir_all(&d).unwrap();
        let a = next_control_seq(&d).unwrap();
        let b = next_control_seq(&d).unwrap();
        assert_eq!(b, a + 1);
        assert_eq!(control_seq_of(&format!("c-{b:012}-20261114T000000Z-ab.json")), b);
        assert_eq!(control_seq_of("c-20261114T000000Z-ab.json"), 0);
        let mut names = vec![
            format!("c-{b:012}-20261114T000000Z-00.json"),
            format!("c-{a:012}-20261114T000000Z-ff.json"),
        ];
        names.sort_by_key(|n| (control_seq_of(n), n.clone()));
        assert!(names[0].contains(&format!("{a:012}")));
        let _ = std::fs::remove_dir_all(&d);
    }
}
