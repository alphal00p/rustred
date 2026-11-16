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
    ItemPhase, class_target, class_word, contract_accepts,
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
    /// procfix5 (gate4 m3): the campaign FINISHED its work (final sweep proven, end invariant holds) but NOT every node
    /// is closed (items in attention, skipped, rejected, or behind such items): read `rr_campaign result` / the node
    /// results; exit 0 means every node closed.
    pub const FINISHED_OPEN: i32 = 10;
    /// procfix5 (gate4 m3): the campaign finished its work and a node ended in ERROR (the work source's verdict).
    pub const FINISHED_ERROR: i32 = 11;
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
/// Content of `RUN/sup/SPAWN` for a launch behind the spawn barrier (procfix5, gate4 m1).
pub const SPAWN_BARRIER_TAG: &str = "barrier v1\n";
/// `RUN/sup/CAMPAIGN`: the campaign's marker id, written before the launch record (run directory authentication).
pub const CAMPAIGN_ID_FILE: &str = "CAMPAIGN";
/// The `item.attention` code of an unconfirmed launch whose execution is unknown (lane int5): the operator decides with
/// `rr_campaign control CDIR resolve JOB`.
pub const EXECUTION_UNKNOWN: &str = "execution_unknown";
/// The `item.attention` code of a job whose child ended [`MAX_CHILD_RESOURCE_ENDS`] times in a row for lack of a host
/// resource (gate3 M4; decision rows 15 / 15b): `resolve JOB` (or `control retry ITEM`) reopens it.
pub const CHILD_RESOURCE_FAILURE: &str = "child_resource_failure";
/// Ends of one job in a row for lack of a host resource before its item goes to attention (gate3 M4).
pub const MAX_CHILD_RESOURCE_ENDS: u32 = 5;

// gate (d), lane int5: lives OUTSIDE the failpoint module (a release binary carries no symbol of that module).
/// BLAKE3 of the variable's NAME (gate (d), lane int5): a release build recognises the variable, and REFUSES to run
/// with it, without carrying its name (the `strings` check finds neither the name nor a site).
#[cfg_attr(debug_assertions, allow(dead_code))]
const TEST_HOOK_ENV_BLAKE3: &str =
    "faf147e7d740c975e49c408a3e030816c176ef73bb706840b7b077de3b3a4d70";

/// The name of the test-hook variable when it is set in a RELEASE build (which has no hooks: running on would let an
/// operator believe a rehearsal hook is armed; the supervisor refuses, exit 1). Always `None` in a test build.
pub fn release_refusal() -> Option<String> {
    #[cfg(not(debug_assertions))]
    {
        std::env::vars_os()
            .map(|(k, _)| k)
            .find(|k| blake3::hash(k.as_encoded_bytes()).to_hex().as_str() == TEST_HOOK_ENV_BLAKE3)
            .map(|k| k.to_string_lossy().into_owned())
    }
    #[cfg(debug_assertions)]
    {
        None
    }
}

/// How a `run` ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunExit {
    /// Finished, every node closed (exit 0).
    Finished,
    /// procfix5 m3: finished, not every node closed (exit 10).
    FinishedOpen(String),
    /// procfix5 m3: finished, a node in error (exit 11).
    FinishedError(String),
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
            RunExit::FinishedOpen(_) => exit::FINISHED_OPEN,
            RunExit::FinishedError(_) => exit::FINISHED_ERROR,
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
    /// The campaign IDENTITY digest of the spec (lane closeloop4: without the launch-time CPU list).
    pub spec_blake3: String,
    /// lane closeloop4: the BLAKE3 of the spec's bytes (the identity a ledger created before closeloop4 recorded):
    /// such a campaign directory still resumes with the very same spec file.
    pub spec_bytes_blake3: Option<String>,
    /// lane closeloop4 (closeloop.md 20): this SESSION's CPU list (`--cpus`; launch-time, never identity).
    pub cpus: Option<String>,
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
    /// A line to the log file only (the terminal is gone or blocked).
    pub fn line_file_only(&mut self, level: &str, text: &str) {
        let l = format!("{} {level} {text}\n", crate::cpu_policy::utc_now());
        if let Some(f) = &mut self.file {
            let _ = f.write_all(l.as_bytes());
        }
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
    /// procfix6 (gate5 m5): the job's containment has been logged and noted.
    containment_noted: bool,
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
    /// Sweeps in a row that found a live marked process of the attempt they could not verify / signal (gate3 M1).
    survivor_sweeps: u32,
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
    /// procfix6 (gate5 m4): a child died by SIGXFSZ under THIS supervisor's own finite file-size limit (inherited by its
    /// children): drain and stop with exit 75.
    fsize_stop: bool,
    /// procfix6: items reopened at recovery for foreign attempts; the work source's retry follow-up runs after bootstrap.
    deferred_source_retries: Vec<String>,
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
    /// What each execution-unknown attempt is charged in admission (its cap and cores: the ledger shows it launching,
    /// so the reservation stays until the operator resolves it). job -> (cap, cores).
    unknown_charge: BTreeMap<String, (u64, usize)>,
    /// Jobs whose item went to attention for an operator decision other than an unknown execution (gate3 M4: repeated
    /// host-resource ends; gate3 M1: live marked processes that could not be killed): they keep the exit-8 rule
    /// armed. job -> attempt.
    attention: BTreeMap<String, u32>,
    /// Ends of a job in a row for lack of a host resource (rows 15 / 15b), this session (gate3 M4).
    child_resource_ends: BTreeMap<String, u32>,
    /// procfix6 (gate5 m3): signals the guard did NOT send because their intent could not be made durable, by (target,
    /// pid, start, signal); each logged once, all listed again at exit.
    not_signalled: BTreeMap<(&'static str, u32, Option<u64>, i32), crate::launch::NotSignalled>,
    /// Attempts held unsettled by live marked processes that could not be verified / signalled (gate3 M1): attempt
    /// label -> pids. They arm the exit-8 rule (never an idle campaign, never exit 0).
    survivors: BTreeMap<String, Vec<u32>>,
    /// Since when nothing runs or waits to be settled while execution-unknown attempts exist (operator-decision exit).
    blocked_since: Option<Instant>,
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

/// How a settle ended (gate (c), lane int5).
enum SettleEnd {
    /// Settled (or nothing to do / the ledger decided).
    Done,
    /// Not settled now: the supervisor's own resource failure (errno) while reading or writing the attempt's files.
    Deferred { errno: i32, what: String },
}

/// A resource errno of THIS process named in an error text (the Display of an OS error: "... (os error N)", or its
/// Debug "Os { code: N"): EAGAIN, ENOMEM, ENFILE, EMFILE (gate (c), lane int5).
fn text_resource_errno(text: &str) -> Option<i32> {
    for pat in ["(os error ", "Os { code: "] {
        for (i, _) in text.match_indices(pat) {
            let d: String = text[i + pat.len()..]
                .chars()
                .take_while(char::is_ascii_digit)
                .collect();
            if let Ok(n) = d.parse::<i32>()
                && matches!(n, libc::EAGAIN | libc::ENOMEM | libc::ENFILE | libc::EMFILE)
            {
                return Some(n);
            }
        }
    }
    None
}

/// The supervisor's own resource failure in the evidence `assess` gathered (an OUTCOME or a sealed stream it could not
/// open for lack of descriptors / memory): the errno and what failed.
fn evidence_resource_failure(ev: &classify::Evidence) -> Option<(i32, String)> {
    let texts: Vec<&str> = match &ev.outcome {
        OutcomeState::Unreadable(t) | OutcomeState::Invalid(t) => vec![t.as_str()],
        _ => vec![],
    }
    .into_iter()
    .chain(ev.unsealed_why.as_deref())
    .collect();
    texts.into_iter().find_map(|t| {
        text_resource_errno(t).map(|n| (n, format!("reading the attempt's files: {t}")))
    })
}

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
            // gate-B M3 at START: the supervisor's own resources (open files, file table, memory, processes) are not a
            // refusal of the campaign: exit 71 like a launch-time resource stop (nothing was launched).
            let e = match e {
                RunExit::Refused(m) | RunExit::Internal(m)
                    if [
                        "(os error 24)",
                        "(os error 23)",
                        "(os error 12)",
                        "(os error 11)",
                    ]
                    .iter()
                    .any(|c| m.contains(c)) =>
                {
                    RunExit::Resources(format!(
                        "supervisor resource error at start: {m}; free it (e.g. raise `ulimit -n`) and run the same command"
                    ))
                }
                other => other,
            };
            log.line("ERROR", &format!("{e:?}"));
            e
        }
    }
}

impl Supervisor {
    fn start(opts: RunOptions, log: &mut Log) -> Result<Supervisor, RunExit> {
        // gate (d), lane int5: a release build has no test hooks and refuses their variable (before anything is touched).
        if let Some(name) = release_refusal() {
            return Err(RunExit::Refused(format!(
                "{name} is set, but this release build has no test hooks: unset it (hooked rehearsals need a test build)"
            )));
        }
        // procfix5 (gate4 m2): never killed by SIGXFSZ (`ulimit -f`): EFBIG is "disk full" (exit 75).
        crate::launch::ignore_sigxfsz()
            .map_err(|e| RunExit::Internal(format!("SIGXFSZ disposition: {e}")))?;
        // procfix6 (gate5 m2): a binary that hosts the spawn barrier holds the barrier pipes: non-dumpable, so no other
        // process of the uid can open them through /proc/<pid>/fd (and no core dump). Not in library / test hosts.
        if crate::cli::barrier_hosted() {
            crate::launch::set_not_dumpable()
                .map_err(|e| RunExit::Internal(format!("PR_SET_DUMPABLE: {e}")))?;
        }
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
        // procfix5 (gate4 M1): the write-ahead signal intent log of this session, before anything can be signalled.
        crate::launch::install_intent_log(
            &cdir.join("log"),
            &format!(
                "{}.{}.{}",
                lock.owner.identity.boot_id.get(..8).unwrap_or("boot"),
                lock.owner.identity.pid,
                lock.owner.identity.start_ticks
            ),
        )
        .map_err(|e| {
            if matches!(
                e.raw_os_error(),
                Some(libc::ENOSPC) | Some(libc::EDQUOT) | Some(libc::EFBIG)
            ) {
                RunExit::DiskFull
            } else {
                RunExit::Refused(format!("signal intent log: {e}"))
            }
        })?;
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
                    // ledgerfix2 M1: files of a ledger without its manifest: a damaged ledger, never a new campaign
                    e @ LedgerError::ManifestMissing { .. } => RunExit::Refused(format!("{e}")),
                    e => RunExit::Refused(format!("ledger: {e}")),
                },
            )?
        };
        for w in ledger.warnings() {
            log.line("WARN", &format!("ledger: {w}"));
        }
        let st = ledger.state();
        // lane closeloop4: the identity digest (no CPU list); a ledger of an older build recorded the bytes' BLAKE3
        let same_spec = st.spec_blake3 == opts.spec_blake3
            || opts
                .spec_bytes_blake3
                .as_deref()
                .is_some_and(|b| st.spec_blake3 == b);
        if !same_spec || st.campaign_id != opts.spec.campaign {
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
        // lane t266run (L17): a session with a launch-time memory cap records its resources (not part of the identity)
        if let Some(cap) = opts.spec.resources.job_mem_cap {
            first.push(Event::Note {
                text: format!(
                    "session_resources {}",
                    serde_json::json!({"cpus": opts.spec.resources.cpus, "max_jobs": opts.spec.resources.max_jobs,
                        "job_mem_cap": cap.0})
                ),
            });
        }
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
        // procfix5 (run book): where the operator finds the campaign id (every job's marker starts with it).
        log.line(
            "INFO",
            &format!(
                "campaign id {key} (every job carries RR_CAMPAIGN_JOB={key}:<job>:a<k>; ledger input campaign:marker_id)"
            ),
        );
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
        let mut session_cpus: Option<String> = None;
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
                // lane closeloop4 (closeloop.md 20): the CPUs are a LAUNCH-TIME resource chosen at each session start:
                // `--cpus` > the spec's default list > the process's affinity mask (the launcher's pick), minus
                // `exclude_cpus`, capped by the budget (`--cores` / `max_cores`); recorded in the ledger per session
                let budget = opts.cores.or(opts.spec.resources.max_cores);
                let (cs, from) = match (&opts.cpus, &opts.spec.resources.cpus) {
                    (Some(c), _) => (c.clone(), "--cpus"),
                    (None, Some(c)) => (c.clone(), "resources.cpus"),
                    (None, None) => (
                        format!("count:{}", budget.unwrap_or(usize::MAX >> 1)),
                        "affinity mask",
                    ),
                };
                let mut cs = CpuSpec::parse(&cs).map_err(RunExit::Refused)?;
                if let Some(x) = &opts.spec.resources.exclude_cpus {
                    let ex = crate::resources::parse_cpu_list(x).map_err(RunExit::Refused)?;
                    cs = match cs {
                        CpuSpec::List(l) => CpuSpec::List(l.difference(&ex).copied().collect()),
                        CpuSpec::Count(n) => {
                            let aff = crate::resources::allowed_cpus()
                                .map_err(|e| RunExit::Refused(format!("sched_getaffinity: {e}")))?;
                            let l: std::collections::BTreeSet<usize> =
                                aff.difference(&ex).copied().collect();
                            let n = n.min(l.len());
                            if n == 0 {
                                return Err(RunExit::Refused(
                                    "no CPU left after resources.exclude_cpus".into(),
                                ));
                            }
                            CpuSpec::List(l.into_iter().take(n).collect())
                        }
                    };
                }
                let p = PlainPolicy::new(&cs, budget)
                    .map_err(|e| RunExit::Refused(e.to_string()))?
                    .with_pause_files(pause_files);
                session_cpus = Some(format!(
                    "session_cpus {{\"from\":{from:?},\"cpus\":{:?},\"budget\":{},\"exclude\":{:?}}}",
                    crate::resources::format_cpu_list(p.candidates()),
                    p.budget(),
                    opts.spec.resources.exclude_cpus.as_deref().unwrap_or("")
                ));
                Box::new(p)
            }
        };
        // lane closeloop4 (closeloop.md 20): the session's CPUs in the ledger (they may differ between sessions)
        let session_note = session_cpus.unwrap_or_else(|| {
            format!(
                "session_cpus {{\"from\":\"site policy\",\"budget\":{}}}",
                policy.budget()
            )
        });
        ledger
            .append(Event::Note { text: session_note })
            .map_err(|e| RunExit::Refused(format!("ledger: {e}")))?;
        let mut admission = AdmissionPolicy::new(policy.budget());
        admission.floor_bytes = opts.spec.resources.host_floor.0;
        admission.campaign_mem_cap_bytes =
            Some(opts.mem_total.unwrap_or(opts.spec.resources.mem_total.0));
        // lane wireflow (zerosfix finding): the most admission can ever see here; a configuration under which nothing
        // can ever be admitted is a TYPED refusal at start, not a silent wait
        if let Ok(m) = host_memory() {
            let lim = crate::resources::cgroup_limit();
            admission.host_ceiling_bytes = Some(admission::host_ceiling(
                m.mem_total,
                lim,
                admission.floor_bytes,
            ));
            if let Some(why) = admission::never_admits(
                m.mem_total,
                lim,
                admission.floor_bytes,
                admission.campaign_mem_cap_bytes,
            ) {
                log.line("ERROR", &format!("admission: {why}"));
                return Err(RunExit::Refused(format!("admission: {why}")));
            }
        }
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
        // procfix5 (gate4 letter (a), item 5): CONTAINMENT of marker-dropping descendants. Real children run only inside
        // the recorded systemd scopes (stopped by name + leader after a crash) unless the spec explicitly accepts the
        // marker as the only containment; fake children are never refused.
        let containment = opts.spec.resources.containment.as_deref();
        if !matches!(containment, None | Some("scope") | Some("marker_only")) {
            return Err(RunExit::Refused(format!(
                "resources.containment {containment:?}: \"scope\" (default) or \"marker_only\""
            )));
        }
        if launcher.setup().systemd_run.is_none() {
            let fake = opts.spec.synthetic.is_some()
                || opts.fake_child
                || programs.values().all(|(p, _)| fake_program(p));
            match (fake, containment) {
                (true, _) => {}
                (false, Some("marker_only")) => log.line(
                    "WARN",
                    &format!(
                        "containment: marker only (resources.containment = \"marker_only\"; no systemd scope: {}): a descendant that drops the marker survives a supervisor crash",
                        launcher.setup().systemd_unavailable.as_deref().unwrap_or("unavailable")
                    ),
                ),
                (false, _) => {
                    return Err(RunExit::Refused(format!(
                        "real children need scope containment, and the systemd scope mechanism is unavailable ({}); \
                         run where `systemd-run --user --scope` works, or accept the marker as the only containment with \
                         resources.containment = \"marker_only\" in the spec",
                        launcher.setup().systemd_unavailable.as_deref().unwrap_or("unknown")
                    )));
                }
            }
        }
        // procfix6 (gate5 m5): the start log names the containment mechanism (per job: `job_containment`).
        log.line(
            "INFO",
            &match &launcher.setup().systemd_run {
                Some(p) => format!(
                    "containment: one transient systemd scope per job ({} --user --scope --unit=rr-<campaign id>-<label>-<hex>); each job's scope is verified after its start (ledger note job_containment)",
                    p.display()
                ),
                None => format!(
                    "containment: marker only (no systemd scope: {})",
                    launcher
                        .setup()
                        .systemd_unavailable
                        .as_deref()
                        .unwrap_or("unavailable")
                ),
            },
        );
        let source: Box<dyn WorkSource> = match &opts.spec.synthetic {
            Some(s) => Box::new(ScriptedWorkSource::new(s.clone())),
            // lane closeloop (closeloop.md 6.2): an input of role `closeloop` names the close-from-material config
            None if opts
                .spec
                .inputs
                .iter()
                .any(|i| i.role == crate::closeloop::CONFIG_ROLE) =>
            {
                Box::new(
                    // lane repairflow (repairflow.md 6): the repair flow wraps phase E and the closing loop
                    crate::closeloop::repair::wrap(
                        // lane termmin (termmin.md 6): phase E wraps the closing loop (terminal minimisation)
                        crate::closeloop::termmin::wrap(
                            crate::closeloop::CloseFromMaterialSource::from_spec(&opts.spec, &cdir)
                                .map_err(RunExit::Refused)?,
                            &opts.spec,
                            &cdir,
                        )
                        .map_err(RunExit::Refused)?,
                        &opts.spec,
                        &cdir,
                    )
                    .map_err(RunExit::Refused)?,
                )
            }
            // lane shipecon (shipecon.md 13): an input of role `shipecon` names the nodes of a ship-economy campaign.
            // REQUEST TO I-INT: keep this branch.
            None if opts
                .spec
                .inputs
                .iter()
                .any(|i| i.role == crate::closeloop::shipecon_source::ROLE) =>
            {
                Box::new(
                    crate::closeloop::shipecon_source::ShipCampaignSource::from_spec(
                        &opts.spec, &cdir,
                    )
                    .map_err(RunExit::Refused)?,
                )
            }
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
            fsize_stop: false,
            deferred_source_retries: Vec::new(),
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
            unknown_charge: BTreeMap::new(),
            attention: BTreeMap::new(),
            child_resource_ends: BTreeMap::new(),
            not_signalled: BTreeMap::new(),
            survivors: BTreeMap::new(),
            blocked_since: None,
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
            Err(e) if ledger_error_is_disk_full(&e) => {
                // gate-B m3: EFBIG (file size limit) / EDQUOT are "the disk is full" for the operator: exit 75.
                self.log.line(
                    "ERROR",
                    &format!(
                        "disk full ({e}): no new job; running jobs get the grace; the ledger stays valid; free space (or raise the file-size limit) and run the same command"
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
            // procfix6: a full disk is exit 75 here as in `commit` (retrying through `commit` met only the poisoned
            // handle and stopped with 70: seen under `ulimit -f` once m4 routes SIGXFSZ ends to the attention path).
            Err(e) if ledger_error_is_disk_full(&e) => {
                self.log.line(
                    "ERROR",
                    &format!("disk full ({e}): no new job; running jobs get the grace; the ledger stays valid; free space (or raise the file-size limit) and run the same command"),
                );
                self.ledger_dead = Some(RunExit::DiskFull);
                Ok(false)
            }
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
        // THE RECOVERY HOOK, step 1 (ledger-and-progress.md 16; lane int5): every launch journalled in an earlier
        // session and never confirmed becomes `unconfirmed` (EXECUTION UNKNOWN) before anything is decided about it.
        self.mark_unconfirmed();
        // procfix5 (gate4 M1): signals of earlier sessions that never reached the ledger, from the intent log.
        self.reconcile_intents();
        if self.ledger_dead.is_some() {
            return;
        }
        let unsettled: Vec<(String, u32, AttemptState)> = self
            .st()
            .unsettled_jobs()
            .into_iter()
            .map(|(id, j)| (id.clone(), j.attempt, j.state))
            .collect();
        // The boot and start bounds of the previous sessions, from the ledger itself.
        let mut rules = self.scan_rules();
        let mut min_ticks: Option<u64> = None;
        // streamed (ledgerfix2 m10: never every record of a long campaign in memory)
        let boot_id = self.boot_id.clone();
        // gate3 M5: the scope unit of each launch and the leader of each start (this boot), for the scope backstop.
        let mut scope_of: BTreeMap<(String, u32), String> = BTreeMap::new();
        let mut started_this_boot: BTreeSet<(String, u32)> = BTreeSet::new();
        // procfix5 (gate4 m6): the row-15 / 15b count of each job comes from the LEDGER (the trailing run of resource
        // requeues), so a restart never re-arms it.
        let mut resource_ends: BTreeMap<String, u32> = BTreeMap::new();
        let _ = crate::ledger::for_each_record(&self.cdir, &mut |r, _| {
            fold_resource_ends(&mut resource_ends, &r.event);
            match &r.event {
                Event::SupervisorStarted { identity, .. } if identity.boot_id == boot_id => {
                    min_ticks = Some(
                        min_ticks.map_or(identity.start_ticks, |m| m.min(identity.start_ticks)),
                    );
                }
                Event::Job {
                    job,
                    attempt,
                    ev:
                        JobEvent::Launch {
                            scope_unit: Some(u),
                            ..
                        },
                } => {
                    scope_of.insert((job.clone(), *attempt), u.clone());
                }
                Event::Job {
                    job,
                    attempt,
                    ev: JobEvent::Started { boot_id: b, .. },
                } if *b == boot_id => {
                    started_this_boot.insert((job.clone(), *attempt));
                }
                _ => {}
            }
            Ok(())
        });
        self.child_resource_ends = resource_ends;
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
        self.note_stop_holds(&report, "recovery");
        self.note_survivors(&report, "recovery");
        // gate3 M5: the scope backstop. For every attempt left unsettled by the previous session whose launch record
        // names a scope unit AND whose leader is recorded (`started`, this boot) in THIS campaign's ledger, whatever is
        // still in that exact scope (e.g. a descendant that cleared the marker and left the session) is killed, after
        // the guarded sweep. Never by pattern; never inside a private namespace (no systemd scopes are used there).
        if scan_allowed && !crate::launch::in_private_namespace() {
            for (job, attempt, _) in &unsettled {
                let key = (job.clone(), *attempt);
                let (Some(unit), true) = (scope_of.get(&key), started_this_boot.contains(&key))
                else {
                    continue;
                };
                let label = format!("{job}:a{attempt}");
                match crate::launch::guard_context("restart: recorded job scope", Some(&label), || {
                    crate::launch::kill_recorded_scope(unit)
                }) {
                    Ok(Some(0)) | Ok(None) => {}
                    Ok(Some(n)) => self.note(format!(
                        "scope_killed {{\"job\":{job:?},\"attempt\":{attempt},\"unit\":{unit:?},\"processes\":{n}}}"
                    )),
                    Err(e) => self.note(format!(
                        "scope_backstop_failed {{\"job\":{job:?},\"attempt\":{attempt},\"unit\":{unit:?},\"error\":{:?}}}",
                        e.to_string()
                    )),
                }
            }
        }
        if let Some(r) = &report.reason {
            self.note(format!(
                "recovery_sweep_unproven {{\"reason\":{r:?}}} (attempts are settled from their files; the periodic sweep continues)"
            ));
        }
        for (job, attempt, state) in unsettled {
            if self.ledger_dead.is_some() {
                return;
            }
            if state == AttemptState::Launching {
                // Impossible after the hook (this session has launched nothing yet): never decided here.
                self.log.line(
                    "ERROR",
                    &format!("{job} a{attempt}: launching at recovery after the unconfirmed-launch hook; left as it is"),
                );
                continue;
            }
            let unconfirmed = state == AttemptState::Unconfirmed;
            if self.moved {
                if unconfirmed {
                    // procfix6 (directing session's ruling, 2026-11-14): an unconfirmed launch of the ORIGINAL campaign
                    // is not ours to authenticate and never waits on an operator here: a foreign attempt, its item
                    // reopened as new work of this copy (never execution unknown, never signalled, nothing consumed).
                    self.foreign_unconfirmed(&job, attempt);
                    continue;
                }
                // Copied / moved directory: the recorded attempt's process (if any) belongs to the ORIGINAL directory.
                let ev = vec![
                    Event::Job {
                        job: job.clone(),
                        attempt,
                        ev: JobEvent::Lose {
                            reason: format!(
                                "{FOREIGN_ATTEMPT}: of the original directory (campaign directory copied / moved: recorded process is not ours, not signalled)"
                            ),
                        },
                    },
                    Event::Job {
                        job: job.clone(),
                        attempt: attempt + 1,
                        ev: JobEvent::Requeue {
                            reason: "lost (moved campaign directory)".into(),
                        },
                    },
                    self.foreign_note(&job, attempt, "started"),
                ];
                self.log.line(
                    "WARN",
                    &format!("{job} a{attempt}: {FOREIGN_ATTEMPT} of the original directory; marked lost, not signalled, its work redone here"),
                );
                self.commit(ev);
                continue;
            }
            let marker = format!("{job}:a{attempt}");
            // procfix5 (gate4 m1): an unconfirmed launch behind the spawn barrier never ran its program (the barrier is
            // released only after `job.started` is durable): a marked process of it is the exiting shim or a copied
            // environment, never a late confirmation.
            let barrier = unconfirmed && self.barrier_launch(&job, attempt);
            if let Some(alive) = report
                .left
                .iter()
                .filter(|l| l.process.attempt_id == marker)
                .min_by_key(|l| l.process.start_ticks)
                .map(|l| l.process.clone())
            {
                // A live marked process: an unconfirmed launch DID start (late confirmation, `started`).
                if unconfirmed && !barrier {
                    self.confirm_late(
                        &job,
                        attempt,
                        &alive,
                        "alive and not signalled by the recovery sweep",
                    );
                }
                // gate-A M-1: never settle while a marked process of the attempt lives; gate3 M1: it arms exit 8.
                let pids: Vec<u32> = report
                    .left
                    .iter()
                    .filter(|l| l.process.attempt_id == marker)
                    .map(|l| l.process.pid)
                    .collect();
                self.log.line(
                    "ERROR",
                    &format!("{marker}: marked process(es) {pids:?} alive and not signalled (see marked_survivor); not settled"),
                );
                self.survivors.insert(marker, pids);
                continue;
            }
            let killed = report
                .killed
                .iter()
                .filter(|k| k.attempt_id == marker)
                .min_by_key(|k| k.start_ticks)
                .cloned();
            let was_killed = killed.is_some();
            if unconfirmed && barrier {
                if let Some(k) = &killed {
                    self.log.line(
                        "WARN",
                        &format!(
                            "{job} a{attempt}: marked process pid {} (start {}) killed by the recovery sweep although the launch never passed its spawn barrier (the exiting shim, or a copied environment): not a confirmation",
                            k.pid, k.start_ticks
                        ),
                    );
                }
                self.recover_unconfirmed(&job, attempt);
                continue;
            }
            if unconfirmed {
                let Some(k) = killed else {
                    // No process of it exists: authenticate its run directory (never "did not run" without proof).
                    self.recover_unconfirmed(&job, attempt);
                    continue;
                };
                // The recovery sweep killed a live marked process of it: the spawn happened and the child ran (late
                // confirmation); from here on it is a running attempt killed at recovery.
                if !self.confirm_late(&job, attempt, &k, "killed by the recovery sweep") {
                    continue;
                }
                if !self.feeds_item(&job) {
                    // Its work was superseded (operator retry / input change): interrupted, nothing to requeue.
                    self.commit(vec![Event::Job {
                        job: job.clone(),
                        attempt,
                        ev: JobEvent::Interrupt {
                            reason: "process of a previous supervisor killed at recovery; its work was superseded"
                                .into(),
                        },
                    }]);
                    continue;
                }
                // procfix5 item 0: the marked process proves that the spawn happened, NOT that the run directory is the
                // launched one. Its contents (a sealed success the crashed child may have written while the dying
                // supervisor dumped core) are consumed only from an AUTHENTICATED directory, exactly as for an
                // unconfirmed launch without a process; otherwise the attempt is interrupted and requeued and nothing is
                // read from it. The decision depends only on the facts on disk, never on when the child stopped.
                if let Some(rd) = self.st().jobs.get(&job).and_then(|j| j.run_dir.clone()) {
                    let launched = self
                        .st()
                        .jobs
                        .get(&job)
                        .and_then(|j| j.attempt_spec.clone());
                    let layout = RunLayout::new(&rd);
                    if let Err(why) =
                        self.authenticate_run(&job, attempt, &rd, &layout, launched.as_deref())
                    {
                        self.log.line(
                            "WARN",
                            &format!("{job} a{attempt}: confirmed late, run directory not authenticated ({why}): interrupted, nothing consumed"),
                        );
                        self.commit(vec![
                            Event::Job {
                                job: job.clone(),
                                attempt,
                                ev: JobEvent::Interrupt {
                                    reason: format!(
                                        "process of a previous supervisor killed at recovery; run directory not authenticated ({why}): nothing consumed"
                                    ),
                                },
                            },
                            Event::Job {
                                job: job.clone(),
                                attempt: attempt + 1,
                                ev: JobEvent::Requeue {
                                    reason: "interrupted at recovery (unauthenticated run directory)"
                                        .into(),
                                },
                            },
                        ]);
                        continue;
                    }
                }
            }
            let Some(run_dir) = self.st().jobs.get(&job).and_then(|j| j.run_dir.clone()) else {
                continue;
            };
            let layout = RunLayout::new(&run_dir);
            let launched = self
                .st()
                .jobs
                .get(&job)
                .and_then(|j| j.attempt_spec.clone());
            match read_spec(&layout, launched.as_deref()) {
                Some((spec, spec_digest)) => {
                    let kill = if was_killed { CKill::Stop } else { CKill::None };
                    self.settle_at_recovery(
                        &job,
                        attempt,
                        &layout,
                        &spec,
                        &spec_digest,
                        kill,
                        was_killed,
                    );
                }
                None => {
                    // A CONFIRMED (running) attempt whose run directory no longer matches its launch record (strict mode
                    // concerns only unconfirmed launches: a running attempt's process existed and is gone now).
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
        // Operator decisions left by earlier sessions (items in attention for an unknown execution whose attempt is no
        // longer unconfirmed, or for repeated host-resource ends): they keep the exit-8 rule armed (never idle).
        let held: Vec<(String, u32)> = self
            .st()
            .items
            .values()
            .filter(|it| {
                matches!(&it.phase, ItemPhase::Attention(c) if c == EXECUTION_UNKNOWN || c == CHILD_RESOURCE_FAILURE)
            })
            .filter_map(|it| it.jobs.last().cloned())
            .filter_map(|j| self.st().jobs.get(&j).map(|r| (j, r.attempt)))
            .collect();
        for (j, a) in held {
            if !self.unknown.contains_key(&j) {
                self.attention.insert(j, a);
            }
        }
        self.record_deliveries();
    }

    /// THE RECOVERY HOOK, step 1: `job.unconfirmed` for every attempt of `state.unconfirmed_launches()` (launches
    /// journalled in an earlier session, never confirmed by `job.started`), one group commit, before the sweep and
    /// before any decision about them. From then on the reducer (strict mode, `ENFORCE_UNCONFIRMED_ACROSS_SESSIONS`)
    /// refuses lose / interrupt / requeue / an unauthenticated finish of them.
    fn mark_unconfirmed(&mut self) {
        let ev: Vec<Event> = self
            .st()
            .unconfirmed_launches()
            .into_iter()
            .map(|(job, j)| Event::Job {
                job: job.clone(),
                attempt: j.attempt,
                ev: JobEvent::Unconfirmed {
                    reason: format!(
                        "launch journalled in session {}, never confirmed by job.started before that supervisor ended",
                        j.launch_session.unwrap_or(0)
                    ),
                },
            })
            .collect();
        if !ev.is_empty() {
            self.log.line(
                "WARN",
                &format!(
                    "recovery: {} unconfirmed launch(es) marked: execution unknown until authenticated",
                    ev.len()
                ),
            );
            self.commit(ev);
        }
    }

    /// The job can still feed its item: not stale, of the item's current generation, the item open or in progress.
    fn feeds_item(&self, job: &str) -> bool {
        let st = self.st();
        st.jobs.get(job).is_some_and(|j| {
            j.stale.is_none()
                && st.items.get(&j.item).is_some_and(|it| {
                    it.generation == j.generation
                        && matches!(it.phase, ItemPhase::Open | ItemPhase::InProgress)
                })
        })
    }

    /// Late confirmation of an unconfirmed launch: a live marked process of the attempt was found (and possibly killed
    /// by the sweep), so the spawn happened: `job.started` with that process's identity.
    fn confirm_late(
        &mut self,
        job: &str,
        attempt: u32,
        p: &crate::resources::MarkedProcess,
        how: &str,
    ) -> bool {
        let ev = vec![Event::Job {
            job: job.into(),
            attempt,
            ev: JobEvent::Started {
                pid: p.pid,
                start_ticks: p.start_ticks,
                boot_id: self.boot_id.clone(),
            },
        }];
        match self.commit_or_refused(ev) {
            Ok(true) => {
                self.log.line(
                    "WARN",
                    &format!(
                        "{job} a{attempt}: unconfirmed launch confirmed late: marked process pid {} (start {}) {how}",
                        p.pid, p.start_ticks
                    ),
                );
                true
            }
            Ok(false) => false,
            Err(why) => {
                self.log.line(
                    "ERROR",
                    &format!("{job} a{attempt}: late confirmation refused by the ledger: {why}"),
                );
                false
            }
        }
    }

    /// procfix6: the note `foreign_attempt` (once per attempt of the ORIGINAL campaign met in a copied / moved directory).
    fn foreign_note(&self, job: &str, attempt: u32, was: &str) -> Event {
        let original = identity::recorded_id(self.st()).unwrap_or_default();
        Event::Note {
            text: format!(
                "{FOREIGN_ATTEMPT} {{\"job\":{job:?},\"attempt\":{attempt},\"was\":{was:?},\"original_id\":{original:?},\"this_id\":{:?}}} (of the original directory: not authenticated here, not signalled, nothing of it consumed; its item is new work of this campaign)",
                self.ctx.campaign_key
            ),
        }
    }

    /// procfix6 (directing session's ruling, 2026-11-14): in a COPIED / MOVED campaign directory an UNCONFIRMED launch of
    /// the ORIGINAL campaign (another marker id) is a FOREIGN attempt: its run directory names the original's id (it can
    /// never authenticate here), its process (if any) is the original's (never signalled), and the copy never waits on an
    /// operator for it. Recorded in the reducer's own verbs (it stays the arbiter): the item goes to attention
    /// `foreign_attempt`, a SUPERVISOR `control retry` reopens it (a new generation), `job.cancel` closes the attempt
    /// (reason `foreign_attempt: of the original directory ...`), a note `foreign_attempt`; the work source then gets the
    /// retry like an operator's. The rule "unconfirmed launch = execution unknown, never retried automatically" stays in
    /// force for launches of THIS campaign id.
    fn foreign_unconfirmed(&mut self, job: &str, attempt: u32) {
        let Some(jr) = self.st().jobs.get(job).cloned() else {
            return;
        };
        if jr.state != AttemptState::Unconfirmed {
            return;
        }
        let why = format!(
            "{FOREIGN_ATTEMPT}: of the original directory (unconfirmed launch recorded under campaign id {}; this copy is {}: not authenticated here, not signalled, nothing consumed; its item is reopened as new work of this campaign)",
            identity::recorded_id(self.st()).unwrap_or_default(),
            self.ctx.campaign_key
        );
        let note = self.foreign_note(job, attempt, "unconfirmed");
        // Work already superseded (the item moved on): a plain cancel, as for this campaign's own launches.
        if !self.feeds_item(job) {
            let ev = vec![
                Event::Job {
                    job: job.into(),
                    attempt,
                    ev: JobEvent::Cancel {
                        reason: why.clone(),
                    },
                },
                note,
            ];
            if let Err(e) = self.commit_or_refused(ev) {
                self.log.line(
                    "ERROR",
                    &format!("{job} a{attempt}: foreign attempt not recorded: {e}"),
                );
            }
            return;
        }
        let item = jr.item.clone();
        let id = format!("foreign-{job}-a{attempt}");
        let mut first = Vec::new();
        if !matches!(
            self.st().items.get(&item).map(|it| &it.phase),
            Some(ItemPhase::Attention(_))
        ) {
            first.push(Event::Item {
                item: item.clone(),
                ev: crate::ledger::ItemEvent::Attention {
                    code: FOREIGN_ATTEMPT.into(),
                    text: why.clone(),
                },
            });
        }
        first.push(Event::Control {
            id: id.clone(),
            ev: ControlEvent::Requested {
                verb: ControlVerb::Retry,
                target: ControlTarget::Item { item: item.clone() },
                by: "supervisor (copied / moved campaign directory)".into(),
                reason: why.clone(),
            },
        });
        match self.commit_or_refused(first) {
            Ok(true) => {}
            Ok(false) => return,
            Err(e) => {
                self.log.line(
                    "ERROR",
                    &format!("{job} a{attempt}: foreign attempt not recorded: {e}"),
                );
                return;
            }
        }
        let ev = vec![
            Event::Control {
                id: id.clone(),
                ev: ControlEvent::Applied {
                    effect: format!(
                        "item {item} reopened: {job} a{attempt} is a foreign attempt of the original directory"
                    ),
                },
            },
            Event::Job {
                job: job.into(),
                attempt,
                ev: JobEvent::Cancel { reason: why },
            },
            note,
        ];
        match self.commit_or_refused(ev) {
            Ok(true) => {
                self.log.line(
                    "WARN",
                    &format!("{job} a{attempt}: {FOREIGN_ATTEMPT} (unconfirmed launch of the original directory): not signalled, item {item} reopened as new work of this campaign"),
                );
                // the source's follow-up after its bootstrap (main loop), as for an operator's retry
                self.deferred_source_retries.push(item);
            }
            Ok(false) => {}
            Err(e) => self.log.line(
                "ERROR",
                &format!("{job} a{attempt}: foreign attempt not recorded: {e}"),
            ),
        }
    }

    /// An UNCONFIRMED launch (execution unknown) of which no live process exists (rule, binding: never "did not run"
    /// without proof, never an automatic retry):
    /// - its work was superseded (operator retry -> newer generation, input change -> stale, item final): `cancel`;
    /// - its item already waits for the operator (attention from an earlier session): still waiting;
    /// - the run directory is AUTHENTICATED (layout path, `RUN/sup/CAMPAIGN` = this campaign's marker id, job.json digest
    ///   = the launch record's, job id / attempt / run dir inside job.json), else execution unknown;
    /// - authenticated and NO spawn sentinel `RUN/sup/SPAWN` (written and fsynced right before every spawn): the spawn
    ///   provably never happened -> `not_spawned` + `requeue` (the ONLY automatic requeue of an unconfirmed launch);
    /// - authenticated, spawned, and a valid OUTCOME bound to the launch digest: settled as usual (a bound `finished`;
    ///   valid sealed results are CONSUMED, never re-solved);
    /// - anything else: EXECUTION UNKNOWN: `item.attention {code: execution_unknown}`, the attempt stays `unconfirmed`;
    ///   the operator decides with `rr_campaign control CDIR resolve JOB` (control retry of the item + cancel).
    fn recover_unconfirmed(&mut self, job: &str, attempt: u32) {
        let Some(jr) = self.st().jobs.get(job).cloned() else {
            return;
        };
        let phase = self.st().items.get(&jr.item).map(|it| it.phase.clone());
        let in_attention = matches!(phase, Some(ItemPhase::Attention(_)));
        if !self.feeds_item(job) && !(in_attention && self.current_generation(job)) {
            let ev = vec![Event::Job {
                job: job.into(),
                attempt,
                ev: JobEvent::Cancel {
                    reason: "execution unknown (unconfirmed launch); its work was superseded (operator retry, input change or a final item)".into(),
                },
            }];
            match self.commit_or_refused(ev) {
                Ok(_) => self.log.line(
                    "INFO",
                    &format!("{job} a{attempt}: unconfirmed launch of superseded work: cancelled"),
                ),
                Err(why) => self.execution_unknown(job, attempt, &format!("cancel refused: {why}")),
            }
            return;
        }
        if in_attention {
            self.execution_unknown(
                job,
                attempt,
                "still waiting for the operator (item in attention)",
            );
            return;
        }
        let Some(run_dir) = jr.run_dir.clone() else {
            self.execution_unknown(job, attempt, "launch record without a run directory");
            return;
        };
        let layout = RunLayout::new(&run_dir);
        let (spec, digest) = match self.authenticate_run(
            job,
            attempt,
            &run_dir,
            &layout,
            jr.attempt_spec.as_deref(),
        ) {
            Ok(x) => x,
            Err(why) => {
                self.execution_unknown(
                    job,
                    attempt,
                    &format!("run directory not authenticated: {why}"),
                );
                return;
            }
        };
        let sentinel = layout.sup_dir().join(SPAWN_SENTINEL);
        // procfix5 (gate4 m1): a launch behind the spawn barrier whose `job.started` is absent (this attempt is
        // unconfirmed) never ran its program: the barrier is released only after that record is durable, and a dead
        // supervisor's barrier reads EOF (the shim exits without executing anything). The proof is the ABSENCE OF THE
        // LEDGER RECORD; the sentinel's content (written before the spawn, untouchable by a child that never ran) only
        // says that the barrier was used.
        let barrier_launch = std::fs::read(&sentinel)
            .map(|b| b == SPAWN_BARRIER_TAG.as_bytes())
            .unwrap_or(false);
        if barrier_launch {
            if layout.outcome().exists() {
                self.execution_unknown(
                    job,
                    attempt,
                    "launched behind the spawn barrier and never confirmed, yet an OUTCOME exists: not this launch's child",
                );
                return;
            }
            let ev = vec![
                Event::Job {
                    job: job.into(),
                    attempt,
                    ev: JobEvent::NotSpawned {
                        reason: format!(
                            "launched behind the spawn barrier; no job.started record (the barrier is released only after it), no marked process of {job}:a{attempt}: the program never ran"
                        ),
                    },
                },
                Event::Job {
                    job: job.into(),
                    attempt: attempt + 1,
                    ev: JobEvent::Requeue {
                        reason: "spawn barrier never released: the program never ran".into(),
                    },
                },
            ];
            self.log.line(
                "INFO",
                &format!("{job} a{attempt}: launch journalled, spawned behind the barrier, never released: requeued"),
            );
            self.commit(ev);
            return;
        }
        match std::fs::symlink_metadata(&sentinel) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let ev = vec![
                    Event::Job {
                        job: job.into(),
                        attempt,
                        ev: JobEvent::NotSpawned {
                            reason: format!(
                                "authenticated run directory {} has no RUN/sup/{SPAWN_SENTINEL}; no marked process of {job}:a{attempt}",
                                self.rel_path(&layout.root)
                            ),
                        },
                    },
                    Event::Job {
                        job: job.into(),
                        attempt: attempt + 1,
                        ev: JobEvent::Requeue {
                            reason: "spawn provably never happened".into(),
                        },
                    },
                ];
                self.log.line(
                    "INFO",
                    &format!("{job} a{attempt}: launch journalled, spawn provably never attempted: requeued"),
                );
                self.commit(ev);
                return;
            }
            Err(e) => {
                self.execution_unknown(
                    job,
                    attempt,
                    &format!("spawn sentinel unreadable ({e}): the spawn cannot be ruled out"),
                );
                return;
            }
            Ok(_) => {}
        }
        let ev = classify::assess(
            &layout,
            &spec,
            &digest,
            WaitStatus::Unknown,
            CKill::None,
            false,
        );
        if matches!(ev.outcome, OutcomeState::Valid(_)) {
            self.settle_at_recovery(job, attempt, &layout, &spec, &digest, CKill::None, false);
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
            &format!("spawned (RUN/sup/{SPAWN_SENTINEL} present), no process found, {why}"),
        );
    }

    /// procfix5 (gate4 m1): was this attempt launched behind the spawn barrier? Its run directory must authenticate and
    /// its `RUN/sup/SPAWN` (written before the spawn) carry the barrier tag.
    fn barrier_launch(&self, job: &str, attempt: u32) -> bool {
        let Some(jr) = self.st().jobs.get(job) else {
            return false;
        };
        let Some(rd) = jr.run_dir.clone() else {
            return false;
        };
        let layout = RunLayout::new(&rd);
        self.authenticate_run(job, attempt, &rd, &layout, jr.attempt_spec.as_deref())
            .is_ok()
            && std::fs::read(layout.sup_dir().join(SPAWN_SENTINEL))
                .is_ok_and(|b| b == SPAWN_BARRIER_TAG.as_bytes())
    }

    /// The job is of its item's current generation and not stale.
    fn current_generation(&self, job: &str) -> bool {
        let st = self.st();
        st.jobs.get(job).is_some_and(|j| {
            j.stale.is_none()
                && st
                    .items
                    .get(&j.item)
                    .is_some_and(|it| it.generation == j.generation)
        })
    }

    /// A settle at recovery: the supervisor's own resource failure while reading the attempt's files is retried a few
    /// times (1 s apart); if it persists the campaign stops with exit 71 before deciding anything about the attempt
    /// (gate (c), lane int5: never a verdict from the supervisor's lack of file descriptors / memory / threads).
    #[allow(clippy::too_many_arguments)]
    fn settle_at_recovery(
        &mut self,
        job: &str,
        attempt: u32,
        layout: &RunLayout,
        spec: &JobSpec,
        digest: &str,
        kill: CKill,
        killed: bool,
    ) {
        for k in 0..5 {
            match self.settle(
                job,
                attempt,
                layout,
                spec,
                digest,
                WaitStatus::Unknown,
                kill,
                killed,
                None,
            ) {
                SettleEnd::Done => return,
                SettleEnd::Deferred { errno, what } => {
                    self.log.line(
                        "WARN",
                        &format!("recovery: {what} ({}): retry {}", errno_name(errno), k + 1),
                    );
                    std::thread::sleep(Duration::from_secs(1));
                }
            }
        }
        let msg = format!(
            "recovery cannot read the files of {job} a{attempt} for lack of a supervisor resource; free it (e.g. raise ulimit -n / -u) and run the same command"
        );
        self.log.line("ERROR", &msg);
        self.ledger_dead = Some(RunExit::Resources(msg));
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
            return Err(format!(
                "launch record names {run_dir}, the layout path is {}",
                expected.display()
            ));
        }
        let id = std::fs::read_to_string(layout.sup_dir().join(CAMPAIGN_ID_FILE))
            .map_err(|e| format!("RUN/sup/{CAMPAIGN_ID_FILE}: {e}"))?;
        if id.trim() != self.ctx.campaign_key {
            return Err(format!(
                "RUN/sup/{CAMPAIGN_ID_FILE} names another campaign id"
            ));
        }
        let (spec, digest) = read_spec(layout, launched)
            .ok_or("job.json does not have the digest of its launch record")?;
        if spec.job_id != job || spec.attempt != attempt || Path::new(&spec.run_dir) != expected {
            return Err("job.json names another job, attempt or run directory".into());
        }
        Ok((spec, digest))
    }

    /// EXECUTION UNKNOWN of an unconfirmed launch (the attempt stays `unconfirmed`): its item goes to
    /// `attention {code: execution_unknown}` (once), the attempt is charged in admission (cap + cores: a process of it
    /// may exist where no scan can prove otherwise) and the campaign stops with exit 8 when nothing else can run. Never
    /// retried automatically: `rr_campaign control CDIR resolve JOB` (control retry of the item + cancel).
    fn execution_unknown(&mut self, job: &str, attempt: u32, why: &str) {
        let Some(jr) = self.st().jobs.get(job).cloned() else {
            return;
        };
        if jr.state != AttemptState::Unconfirmed {
            return;
        }
        self.unknown.insert(job.to_string(), attempt);
        // The cores of its launch record (streamed: never every record of a long campaign in memory, ledgerfix2 m10).
        let mut cores = 1usize;
        let _ = crate::ledger::for_each_record(&self.cdir, &mut |r, _| {
            if let Event::Job {
                job: j,
                attempt: a,
                ev: JobEvent::Launch { cpus, .. },
            } = &r.event
                && j == job
                && *a == attempt
            {
                cores = cpus.len().max(1);
            }
            Ok(())
        });
        self.unknown_charge
            .insert(job.to_string(), (jr.reserved_bytes, cores));
        self.log.line(
            "WARN",
            &format!(
                "{job} a{attempt}: EXECUTION UNKNOWN ({why}); not retried automatically: `rr_campaign control CDIR resolve {job}` reopens its item"
            ),
        );
        let note = Event::Note {
            text: format!(
                "execution_unknown {{\"job\":{job:?},\"attempt\":{attempt},\"why\":{why:?}}} (never retried automatically; rr_campaign control CDIR resolve JOB)"
            ),
        };
        let in_attention = matches!(
            self.st().items.get(&jr.item).map(|it| &it.phase),
            Some(ItemPhase::Attention(_))
        );
        if in_attention {
            self.commit(vec![note]);
            return;
        }
        let attention = Event::Item {
            item: jr.item.clone(),
            ev: crate::ledger::ItemEvent::Attention {
                code: EXECUTION_UNKNOWN.into(),
                text: format!(
                    "job {job} attempt {attempt}: execution unknown ({why}); decide with `rr_campaign control CDIR resolve {job}`"
                ),
            },
        };
        match self.commit_or_refused(vec![attention, note.clone()]) {
            Ok(_) => {}
            Err(e) => {
                self.log.line(
                    "WARN",
                    &format!(
                        "{job} a{attempt}: item {} not put in attention: {e}",
                        jr.item
                    ),
                );
                self.commit(vec![note]);
            }
        }
    }

    /// procfix5 (gate4 M1), at recovery before any signal of this session: every intent of an earlier session's signal
    /// intent log that no `signal_sent` record names becomes one: with a `done delivered` line as "delivered, not
    /// recorded before the supervisor ended", WITHOUT a completion line as "POSSIBLY SENT" (the crash came between the
    /// durable intent and the completion: the signal may or may not have gone out). Never re-sent: whatever still lives is
    /// judged by the recovery sweep's identity checklist. Intents completed `gone` / `failed` were not delivered.
    fn reconcile_intents(&mut self) {
        let path = self.cdir.join("log").join(crate::launch::INTENT_LOG_FILE);
        let (intents, bad) = match crate::launch::read_intent_log(&path) {
            Ok(x) => x,
            Err(e) => {
                self.log.line(
                    "ERROR",
                    &format!("signal intent log {} unreadable ({e}): signals of an earlier session may be missing from the ledger", path.display()),
                );
                self.note(format!(
                    "signal_intents_unreadable {{\"error\":{:?}}}",
                    e.to_string()
                ));
                return;
            }
        };
        if intents.is_empty() && bad == 0 {
            return;
        }
        let mut recorded: BTreeSet<String> = BTreeSet::new();
        let _ = crate::ledger::for_each_record(&self.cdir, &mut |r, _| {
            if let Event::SignalSent { by, .. } = &r.event
                && let Some(id) = crate::launch::intent_id_in(by)
            {
                recorded.insert(id.to_string());
            }
            Ok(())
        });
        let (mut delivered, mut possibly) = (0usize, 0usize);
        let mut ev = Vec::new();
        for li in intents {
            let l = li.line;
            if recorded.contains(&l.id) {
                continue;
            }
            let state = match li.result.as_deref() {
                Some("delivered") => {
                    delivered += 1;
                    "delivered, not recorded before the supervisor ended"
                }
                None => {
                    possibly += 1;
                    "POSSIBLY SENT: intent without completion"
                }
                Some(_) => continue,
            };
            let label = l
                .marker
                .as_deref()
                .and_then(crate::resources::CampaignMarker::parse)
                .map(|m| m.attempt_id)
                .or(l.label.clone());
            let job = label
                .as_deref()
                .and_then(|x| x.rsplit_once(":a"))
                .map(|(j, _)| j.to_string())
                .filter(|j| self.st().jobs.contains_key(j));
            let base = l.by.clone().unwrap_or_else(|| "unspecified".into());
            let base = match &l.detail {
                Some(x) => format!("{base} ({x})"),
                None => base,
            };
            let by = format!("{base} [intent {}; recovered at restart: {state}]", l.id);
            self.log.line(
                "WARN",
                &format!(
                    "signal {} to {} {} (start {:?}, {}) {state}: recorded from the intent log",
                    l.signal.unwrap_or(0),
                    l.target.as_deref().unwrap_or("?"),
                    l.pid.unwrap_or(0),
                    l.start_ticks,
                    l.proof.as_deref().unwrap_or("?")
                ),
            );
            let target = match l.target.as_deref() {
                Some("group") => "group",
                Some("cgroup") => "cgroup",
                _ => "process",
            };
            ev.push(Event::SignalSent {
                job,
                pid: l.pid.unwrap_or(0),
                start_ticks: l.start_ticks,
                signal: l.signal.unwrap_or(0).clamp(0, 64),
                target: target.into(),
                marker: l.marker.clone(),
                proof: l
                    .proof
                    .clone()
                    .filter(|p| !p.is_empty())
                    .unwrap_or_else(|| "unspecified".into()),
                by,
            });
        }
        if delivered + possibly == 0 && bad == 0 {
            return;
        }
        ev.push(Event::Note {
            text: format!(
                "signal_intents_reconciled {{\"delivered_unrecorded\":{delivered},\"possibly_sent\":{possibly},\"unparsable_lines\":{bad}}}"
            ),
        });
        self.log.line(
            if possibly > 0 { "WARN" } else { "INFO" },
            &format!(
                "recovery: signal intent log: {delivered} delivered and {possibly} possibly sent signal(s) of an earlier session recorded{}",
                if bad > 0 { format!(" ({bad} unparsable line(s): a torn tail)") } else { String::new() }
            ),
        );
        self.commit(ev);
    }

    /// gate3 M2 (lane int5): every signal the process layer delivered since the last call (guard and cgroup kills, any
    /// thread) becomes one `signal_sent` record (pid, start time, marker, proof, who and why), one group commit.
    fn record_deliveries(&mut self) {
        self.check_not_signalled();
        let (ds, dropped) = crate::launch::take_deliveries();
        if ds.is_empty() && dropped == 0 {
            return;
        }
        let mut ev = Vec::with_capacity(ds.len() + 1);
        for d in ds {
            let label = d
                .marker
                .as_deref()
                .and_then(crate::resources::CampaignMarker::parse)
                .map(|m| m.attempt_id)
                .or(d.label.clone());
            let job = label
                .as_deref()
                .and_then(|l| l.rsplit_once(":a"))
                .map(|(j, _)| j.to_string())
                .filter(|j| self.st().jobs.contains_key(j));
            let mut by = match &d.detail {
                Some(x) => format!("{} ({x})", d.by),
                None => d.by.clone(),
            };
            // procfix5 M1: the durable intent this delivery completes (reconciliation at recovery matches on it).
            if let Some(id) = &d.intent {
                by.push_str(&format!(" [intent {id}]"));
            }
            self.log.line_file_only(
                "INFO",
                &format!(
                    "signal {} delivered to {} {} (start {:?}, {}, by {by})",
                    d.signal, d.target, d.pid, d.start_ticks, d.proof
                ),
            );
            ev.push(Event::SignalSent {
                job,
                pid: d.pid,
                start_ticks: d.start_ticks,
                signal: d.signal,
                target: d.target.into(),
                marker: d.marker,
                proof: d.proof.into(),
                by,
            });
        }
        if dropped > 0 {
            ev.push(Event::Note {
                text: format!("signal_log_overflow {{\"not_recorded\":{dropped}}}"),
            });
        }
        if self.ledger_dead.is_some() {
            self.log.line(
                "WARN",
                &format!("{} delivered signal(s) not recorded: the ledger is unavailable (see the log file)", ev.len()),
            );
            return;
        }
        self.commit(ev);
    }

    /// procfix6 (gate5 m5): ONE log line and ONE ledger note per job naming the containment actually in force:
    /// `scope:<unit>` only when the scope exists (verified from the leader's cgroup), else `marker_only` (with why) or
    /// `cgroup:<path>`.
    fn note_containment(
        &mut self,
        job: &str,
        attempt: u32,
        c: &crate::launch::Containment,
        requested: Option<&str>,
    ) {
        use crate::launch::Containment;
        let (word, why) = match c {
            Containment::Scope(_) => (
                c.word(),
                "verified: the job leader's cgroup is this scope".to_string(),
            ),
            Containment::Cgroup(_) => (c.word(), "the job's delegated cgroup".to_string()),
            Containment::MarkerOnly(w) => ("marker_only".to_string(), w.clone()),
            Containment::Pending => (
                "marker_only".to_string(),
                format!(
                    "scope {} requested but not verified (the leader's cgroup does not name it)",
                    requested.unwrap_or("?")
                ),
            ),
        };
        let level = if matches!(c, Containment::Pending) {
            "WARN"
        } else {
            "INFO"
        };
        self.log.line(
            level,
            &format!("{job} a{attempt}: containment {word} ({why})"),
        );
        self.commit(vec![Event::Note {
            text: format!(
                "job_containment {{\"job\":{job:?},\"attempt\":{attempt},\"containment\":{word:?},\"why\":{why:?}}}"
            ),
        }]);
    }

    /// procfix6 (gate5 m3): signals NOT sent because their intent could not be made durable: one ERROR line each (pid,
    /// start, label, signal, who); a full disk under the intent log ends the run with exit 75 at once (the ledger-dead
    /// path: children get the grace, nothing can be signalled anyway), never after the hunt bound with a stop code.
    fn check_not_signalled(&mut self) {
        let (v, dropped) = crate::launch::take_not_signalled();
        let mut fresh = Vec::new();
        for n in v {
            let k = (n.target, n.pid, n.start_ticks, n.signal);
            if let std::collections::btree_map::Entry::Vacant(e) = self.not_signalled.entry(k) {
                fresh.push(n.clone());
                e.insert(n);
            }
        }
        for n in &fresh {
            self.log.line(
                "ERROR",
                &format!(
                    "signal {} NOT sent to {} {} (start {:?}, {}, by {}): {}",
                    n.signal,
                    n.target,
                    n.pid,
                    n.start_ticks,
                    n.label
                        .clone()
                        .or_else(|| n.marker.clone())
                        .unwrap_or_else(|| "no label".into()),
                    n.by,
                    n.error
                ),
            );
        }
        if dropped > 0 {
            self.log.line(
                "ERROR",
                &format!(
                    "{dropped} further signal(s) NOT sent (intent not durable) and not listed"
                ),
            );
        }
        if crate::launch::intent_disk_full() && self.ledger_dead.is_none() {
            self.log.line(
                "ERROR",
                "the signal intent log cannot be written (disk full): no signal can be sent; stopping with exit 75 (DISK FULL); free space, then run the same command (its recovery sweep stops what is left)",
            );
            if !fresh.is_empty() {
                let pids: Vec<u32> = fresh.iter().map(|n| n.pid).collect();
                self.commit(vec![Event::Note {
                    text: format!("signals_not_sent {{\"pids\":{pids:?},\"reason\":\"signal intent not durable (disk full)\"}}"),
                }]);
            }
            self.ledger_dead = Some(RunExit::DiskFull);
        }
    }

    /// The exit line of signals NOT sent in this session (procfix6 m3): exactly which processes (pid, start, label).
    fn not_signalled_summary(&self) -> Option<String> {
        if self.not_signalled.is_empty() {
            return None;
        }
        let list: Vec<String> = self
            .not_signalled
            .values()
            .map(|n| {
                format!(
                    "{} {} (start {}, {}, signal {})",
                    n.target,
                    n.pid,
                    n.start_ticks.map_or("-".into(), |t| t.to_string()),
                    n.label
                        .clone()
                        .or_else(|| n.marker.clone())
                        .unwrap_or_else(|| "no label".into()),
                    n.signal
                )
            })
            .collect();
        Some(format!(
            "{} signal(s) NOT sent because their intent could not be written: {}; these processes may still run: free space and run the same command (its recovery sweep stops them), or check each: ps -o pid,stat,lstart,cmd -p PID",
            list.len(),
            list.join("; ")
        ))
    }

    /// procfix6 (gate5 m1): processes a sweep STOPPED whose marker was gone at the kill round: killed by the identity held
    /// at the stop (note `killed_by_stop_hold`), or continued (note `continued_after_stop`, a WARN line: alive, not
    /// stopped, not ours by the marker any more). Stranded ones (neither) are in `left` (note_survivors, exit 8).
    fn note_stop_holds(&mut self, report: &SweepReport, by: &str) {
        for p in &report.killed_by_hold {
            self.commit(vec![Event::Note {
                text: format!(
                    "killed_by_stop_hold {{\"pid\":{},\"start_ticks\":{},\"marker\":{:?},\"by\":{by:?}}}",
                    p.pid, p.start_ticks, p.attempt_id
                ),
            }]);
        }
        for l in &report.continued {
            self.log.line(
                "WARN",
                &format!(
                    "process stopped by the sweep had dropped the marker and could not be killed: CONTINUED, pid {} (start {}) of {}: {}",
                    l.process.pid, l.process.start_ticks, l.process.attempt_id, l.why
                ),
            );
            self.commit(vec![Event::Note {
                text: format!(
                    "continued_after_stop {{\"pid\":{},\"start_ticks\":{},\"marker\":{:?},\"comm\":{:?},\"by\":{by:?},\"why\":{:?}}}",
                    l.process.pid, l.process.start_ticks, l.process.attempt_id, l.process.comm, l.why
                ),
            }]);
        }
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
    ) -> SettleEnd {
        // The supervisor's own kill reason K is an input of the decision table: a child stopped for time, silence or
        // memory that still writes an `interrupted` OUTCOME is killed_time / killed_memory there (rows 11b / 11c).
        // lane closeloop2 (closeloop.md 13.1): the legacy rr_proof kinds are sealed by the wrapper before classification
        let wait = crate::adapters::legacy::wrap(layout, spec, spec_digest, wait);
        let ev = classify::assess(layout, spec, spec_digest, wait, kill, false);
        // gate (c), lane int5: the SUPERVISOR's own resource failure while reading the attempt's files (EMFILE, ENFILE,
        // ENOMEM, EAGAIN) is never a verdict about the attempt (never "contract violation", never "crashed"): settled
        // later, after the supervisor's resource back-off.
        if let Some((errno, what)) = evidence_resource_failure(&ev) {
            return SettleEnd::Deferred { errno, what };
        }
        let c = classify::classify(&ev);
        // procfix6 (gate5 m4): a death attributable to the HOST's disk state (SIGXFSZ, an OUTCOME naming EFBIG, or no /
        // a torn OUTCOME while the run directory's or the campaign directory's file system is full at settle time) is a
        // resource end of the attempt (row 15b: retried after a back-off, then attention `child_resource_failure`),
        // never a crash count.
        let c = match disk_attributed_end(&ev, &c, &[layout.root.as_path(), self.cdir.as_path()]) {
            Some(why) => {
                self.log.line(
                    "WARN",
                    &format!("{job} a{attempt}: {why}: a resource end (row 15b), not a crash (was row {}{})", c.row, c.sub),
                );
                // The limit is this supervisor's own (`ulimit -f`, inherited by every child): retrying cannot help.
                if why.starts_with(SIGXFSZ_WHY)
                    && !self.fsize_stop
                    && let Some(limit) = own_file_size_limit()
                {
                    self.fsize_stop = true;
                    self.log.line(
                        "ERROR",
                        &format!("this supervisor runs under a file-size limit ({limit} bytes, `ulimit -f`) that its children inherit: draining, then exit 75 (DISK FULL); raise the limit (`ulimit -f unlimited`) and run the same command"),
                    );
                    self.signals.handle.request_stop();
                }
                resource_classification(&c, why)
            }
            None => c,
        };
        // The OUTCOME binding the ledger checks before any record may come from this attempt: (job, attempt, the
        // digest of the job.json this attempt was launched with), present when the contract found OUTCOME valid.
        let binding = matches!(ev.outcome, OutcomeState::Valid(_)).then(|| OutcomeBinding {
            job: job.to_string(),
            attempt,
            spec_blake3: spec_digest.to_string(),
        });
        let target = class_target(c.kind);
        let Some(jr) = self.st().jobs.get(job).cloned() else {
            return SettleEnd::Done;
        };
        if jr.attempt != attempt || jr.state.is_settled() {
            return SettleEnd::Done;
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
                    if let Some(errno) = text_resource_errno(&why) {
                        return SettleEnd::Deferred {
                            errno,
                            what: format!("re-reading OUTCOME of {job} a{attempt}: {why}"),
                        };
                    }
                    self.log.line(
                        "WARN",
                        &format!("{job} a{attempt}: OUTCOME changed since it was classified ({why}); not recorded"),
                    );
                    None
                }
            },
            _ => file_ref(&layout.outcome(), self.rel_path(&layout.outcome())),
        };
        // An UNCONFIRMED launch is settled only by the authenticated OUTCOME of exactly this launch (binding + the
        // OUTCOME file); anything less stays execution unknown (the reducer would refuse it).
        if jr.state == AttemptState::Unconfirmed && (binding.is_none() || outcome_ref.is_none()) {
            self.execution_unknown(
                job,
                attempt,
                "its OUTCOME could not be bound to the launch at settle",
            );
            return SettleEnd::Done;
        }
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
            // gate3 M4 (lane int5): an end for lack of a host resource (rows 15 / 15b: the child named ENOSPC, EAGAIN,
            // EMFILE, ENFILE, ENOMEM, EDQUOT) is never evidence about the work: queued again after a launch back-off
            // (1, 2, 4, 8, 16 s), and after MAX_CHILD_RESOURCE_ENDS such ends of one job in a row its item goes to
            // attention (`resolve JOB` / `control retry ITEM` reopens it). Never routed to the work source.
            let resource_end = c.row == 15 && !interrupted;
            let n = if resource_end {
                let n = self.child_resource_ends.entry(job.to_string()).or_insert(0);
                *n += 1;
                let backoff = Duration::from_secs(1 << (*n - 1).min(4));
                self.resource_backoff_until = Some(Instant::now() + backoff);
                *n
            } else {
                0
            };
            if resource_end && n >= MAX_CHILD_RESOURCE_ENDS {
                let text = format!(
                    "job {job}: {n} ends in a row for lack of a host resource (row {}{}: {}); never evidence about the work; check the host limits, then `rr_campaign control CDIR resolve {job}`",
                    c.row, c.sub, c.why
                );
                events.push(Event::Item {
                    item: jr.item.clone(),
                    ev: crate::ledger::ItemEvent::Attention {
                        code: CHILD_RESOURCE_FAILURE.into(),
                        text: text.clone(),
                    },
                });
                self.log.line("WARN", &text);
                match self.commit_or_refused(events) {
                    Ok(_) => {
                        self.attention.insert(job.to_string(), attempt);
                    }
                    Err(why) => self.log.line(
                        "ERROR",
                        &format!("{job} a{attempt}: attention refused by the ledger: {why}"),
                    ),
                }
                return SettleEnd::Done;
            }
            events.push(Event::Job {
                job: job.into(),
                attempt: attempt + 1,
                ev: JobEvent::Requeue {
                    reason: format!("{} (row {}{})", class_word(c.kind), c.row, c.sub),
                },
            });
            self.log.line(
                "INFO",
                &format!(
                    "{job} a{attempt}: {} (row {}{}): requeued{}",
                    class_word(c.kind),
                    c.row,
                    c.sub,
                    if resource_end {
                        format!(" after a launch back-off ({n} resource end(s) in a row)")
                    } else {
                        String::new()
                    }
                ),
            );
            self.commit(events);
            return SettleEnd::Done;
        }
        self.child_resource_ends.remove(job);
        let mut table = None;
        let mut lines = Vec::new();
        let mut canon = None;
        // A sealed output is read for every attempt whose row may accept records (sealed successes, and the committed
        // rules of a sealed partial table: spill row 8, no_rule row 4): the table of a table kind, or the sealed
        // `results` stream of a table-less kind (lane closeloop, closeloop.md 6.4: solve-recipe). ONE function for both
        // (lane int5): ac2-M1, one read through the contract's verified-stream handle, bound to the stream `assess`
        // verified (dev, inode, byte count, BLAKE3, canonical digest); a line without a canonical form or any mismatch
        // accepts NOTHING (class crashed). The FileRef recorded is the verified figures, never a re-read of the path.
        let wants_table =
            target == AttemptTarget::SucceededSealed || c.accept != ContractAccept::Nothing;
        let mut sealed_ok = true;
        match settle_output(layout, spec.kind, ev.verified.as_ref(), wants_table) {
            Ok(Some(t)) => {
                table = Some(FileRef {
                    path: self.rel_path(&layout.root.join(&t.rel)),
                    bytes: t.bytes,
                    blake3: t.blake3,
                });
                lines = t.lines;
                canon = Some(t.canonical);
            }
            Ok(None) => {}
            Err(why) if text_resource_errno(&why).is_some() => {
                return SettleEnd::Deferred {
                    errno: text_resource_errno(&why).unwrap_or(libc::EMFILE),
                    what: format!("reading the sealed output of {job} a{attempt}: {why}"),
                };
            }
            Err(why) => {
                sealed_ok = false;
                self.log.line(
                    "ERROR",
                    &format!(
                        "{job} a{attempt}: sealed output not accepted: {why}; nothing accepted"
                    ),
                );
            }
        }
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
        } else if let Some(r) = row3_routing(
            self.source.as_mut(),
            self.ledger.state(),
            &self.ctx,
            &mut settled,
        ) {
            self.log.line(
                "INFO",
                &format!(
                    "{job} a{attempt}: row 3 claims confirmed by the work source's coverage check"
                ),
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
                    self.log.line(
                        "ERROR",
                        &format!("accepted record {}: {e}; not recorded", a.path),
                    );
                    return SettleEnd::Done;
                }
                // Not settled now (nothing committed): retried after the supervisor's back-off (an attempt is never
                // left unsettled with no one to settle it).
                let t = e.to_string();
                return SettleEnd::Deferred {
                    errno: text_resource_errno(&t).unwrap_or(libc::EIO),
                    what: format!("writing accepted record {}: {t}", a.path),
                };
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
        SettleEnd::Done
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
        // lane closeloop (closeloop.md 6.3): `<path> <verb> --job RUN/job.json` for a program with a job verb
        if kind == AdapterKind::Contract
            && let Some(verb) = self
                .opts
                .spec
                .programs
                .get(&req.program)
                .and_then(|p| p.job_verb.clone())
        {
            let jp = RunLayout::new(&spec.run_dir).job_spec();
            ls.args = vec![verb.into(), "--job".into(), jp.into_os_string()];
        }
        // lane closeloop2 (closeloop.md 13.1): a legacy-wrapped rr_proof kind gets the argv derived from its spec
        if kind == AdapterKind::Contract && spec.kind.legacy_wrapped() {
            ls.args = crate::adapters::legacy::argv(&spec)?
                .into_iter()
                .map(Into::into)
                .collect();
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
        // gate3 M5: the job's systemd scope (when that mechanism is used) gets a deterministic name, recorded in the
        // launch record below, so that a restart after a crash can stop exactly this scope. procfix6 (gate5 m5): named
        // (and recorded) ONLY when the launcher will really ask systemd for a scope (never in a private namespace or
        // with the marker only); its existence is verified after the start (note `job_containment`).
        ls.scope_unit = self
            .launcher
            .scope_requested(&ls)
            .then(|| crate::launch::scope_unit_name(&self.ctx.campaign_key, &ls.label));
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
                scope_unit: ls.scope_unit.clone(),
            },
        });
        if !self.commit(events) {
            return Err("ledger unavailable".into());
        }
        self.fp.hit("after_launch_record");
        // procfix5 (gate4 m1): the child waits behind the spawn barrier until `job.started` is durable (below).
        ls.spawn_barrier = crate::cli::barrier_hosted() && !self.fp.armed("no_spawn_barrier");
        // The spawn sentinel (requirement 05:23Z): durable BEFORE the spawn, so that its absence proves "never spawned".
        if let Err(e) = write_spawn_sentinel(&RunLayout::new(&run_dir), ls.spawn_barrier) {
            self.commit(not_an_attempt(
                &req.job,
                attempt,
                &format!("spawn sentinel not written: {e}"),
            ));
            if let Some(c) = io_resource_errno(&e) {
                self.resource_failure(c, &format!("writing RUN/sup/{SPAWN_SENTINEL}: {e}"));
            }
            return Err(format!("spawn sentinel: {e}"));
        }
        let mut handle = match self.launcher.launch(&ls) {
            Ok(h) => h,
            Err(e) if resource_errno(&e).is_some() => {
                // gate-B M3: the SUPERVISOR's resource problem, not a job attempt: the same job is requeued (no new job
                // id), launching backs off, and after a bounded number of failures in a row the campaign stops (71).
                let c = resource_errno(&e).unwrap_or(libc::EMFILE);
                self.commit(not_an_attempt(
                    &req.job,
                    attempt,
                    &format!(
                        "supervisor resource error at launch ({}): {e}",
                        errno_name(c)
                    ),
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
        let recorded = self.commit(vec![Event::Job {
            job: req.job.clone(),
            attempt,
            ev: JobEvent::Started {
                pid: id.pid,
                start_ticks: id.start_ticks,
                boot_id: id.boot_id.clone(),
            },
        }]);
        // procfix5 (gate4 m1): only now (the spawn is in the ledger) may the child run its program.
        if recorded {
            if let Err(e) = handle.release_barrier() {
                self.log.line(
                    "WARN",
                    &format!("{} a{attempt}: spawn barrier release failed ({e}): the child is classified from its end", req.job),
                );
            }
        } else {
            handle.cancel_barrier();
        }
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
            containment_noted: false,
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
        let mut contain = Vec::new();
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
            // procfix6 (gate5 m5): the containment actually in force, once verified (or after 10 s / at the end).
            if !ch.containment_noted {
                let c = ch.handle.containment();
                if c != crate::launch::Containment::Pending
                    || ch.launched_at.elapsed() > Duration::from_secs(10)
                {
                    ch.containment_noted = true;
                    let req = ch.handle.requested_scope_unit().map(str::to_string);
                    contain.push((ch.job.clone(), ch.attempt, c, req));
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
        for (job, attempt, c, req) in contain {
            self.note_containment(&job, attempt, &c, req.as_deref());
        }
        for (idx, report) in finished.into_iter().rev() {
            let mut ch = self.children.remove(idx);
            if !ch.containment_noted {
                ch.containment_noted = true;
                let c = ch.handle.containment();
                let req = ch.handle.requested_scope_unit().map(str::to_string);
                self.note_containment(&ch.job, ch.attempt, &c, req.as_deref());
            }
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
                survivor_sweeps: 0,
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
            self.note_stop_holds(&report, "end-of-job sweep");
            if report.empty() {
                self.survivors.remove(&label);
                let p = self.pending.remove(i);
                self.settle_pending(p);
                continue;
            }
            if !report.left.is_empty() {
                // A live marked process of the attempt that could not be verified / signalled: never settled
                // meanwhile; after 3 such sweeps it arms the exit-8 rule (gate3 M1: never an idle campaign).
                if !self.pending[i].survivors_noted {
                    self.note_survivors(&report, "end-of-job sweep");
                    self.pending[i].survivors_noted = true;
                }
                self.pending[i].survivor_sweeps += 1;
                if self.pending[i].survivor_sweeps >= 3 {
                    self.survivors.insert(
                        label.clone(),
                        report.left.iter().map(|l| l.process.pid).collect(),
                    );
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
        if let SettleEnd::Deferred { errno, what } = self.settle_pending_inner(&p) {
            // gate (c), lane int5: the supervisor's own resource failure while settling: the attempt keeps its lease and
            // waits (back-off; after MAX_RESOURCE_FAILURES in a row the campaign stops with exit 71).
            self.resource_failure(errno, &what);
            p.next_sweep = self
                .resource_backoff_until
                .unwrap_or_else(|| Instant::now() + Duration::from_secs(1));
            self.pending.push(p);
            return;
        }
        if let Some(l) = p.lease.take() {
            // Released after the settle above has been committed.
            let lease_after = l;
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
        }
    }

    fn settle_pending_inner(&mut self, p: &PendingSettle) -> SettleEnd {
        if self.ledger_dead.is_some() {
            return SettleEnd::Done;
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
        )
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
                            self.orphan_killed(
                                &k.attempt_id,
                                k.pid,
                                k.start_ticks,
                                "periodic sweep",
                            );
                        }
                        self.note_stop_holds(&r, "periodic sweep");
                        self.note_survivors(&r, "periodic sweep");
                    }
                }
            }
        }
        if self.last_periodic.elapsed() < Duration::from_secs(30) {
            return;
        }
        self.last_periodic = Instant::now();
        let Some(min) = self
            .campaign_min_ticks
            .or_else(|| Some(orphans::own_start_ticks()))
        else {
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

    /// At "finished" (and nothing live): campaign-wide sweeps (blocking, bounded; up to 3 tries), so that the campaign
    /// never ends with a live marked process (gate-B M1). gate3 M1 (lane int5): NEVER exit 0 while the final sweep is
    /// unproven: `Err(message with the exact pids)` when a marked process could not be killed or the sweep could not
    /// complete (the caller stops with exit 8; the pids are in the ledger note `final_sweep_unproven` and on stderr).
    fn final_sweep(&mut self) -> Result<(), String> {
        let mut rules = self.scan_rules();
        rules.min_start_ticks = self
            .campaign_min_ticks
            .unwrap_or_else(orphans::own_start_ticks);
        let mut last = None;
        for _ in 0..3 {
            let mut r = orphans::sweep(&rules, None, Duration::from_secs(10));
            if self.fp.armed("final_sweep_fails") {
                r.proven = false;
                r.reason = Some("TEST BUILD hook final_sweep_fails".into());
            }
            for k in &r.killed {
                self.orphan_killed(&k.attempt_id, k.pid, k.start_ticks, "final sweep");
            }
            self.note_stop_holds(&r, "final sweep");
            self.record_deliveries();
            if r.empty() {
                return self.hidden_descendants_check();
            }
            last = Some(r);
        }
        let r = last.unwrap_or_default();
        self.note_survivors(&r, "final sweep");
        let pids: Vec<u32> = r.left.iter().map(|l| l.process.pid).collect();
        let why = r
            .reason
            .clone()
            .unwrap_or_else(|| "marked processes left".into());
        let msg = format!(
            "final marker sweep not proven ({why}); live marked process(es) not killed: {pids:?}; check each pid (its /proc/PID/environ carries this campaign's id) and kill exactly those, then run the same command"
        );
        self.commit(vec![Event::Note {
            text: format!("final_sweep_unproven {{\"pids\":{pids:?},\"reason\":{why:?}}}"),
        }]);
        Err(msg)
    }

    /// procfix5 (gate4 m5), after a proven final sweep: descendants of this supervisor (parent chain) that run under our
    /// real uid but whose /proc entry is root-owned (non-dumpable): no marker can be read, so the sweep could not see
    /// them. They are REPORTED (note `hidden_descendants`, the pids and names) and the run does not claim a clean end
    /// (exit 8); they are never judged ours and never signalled.
    fn hidden_descendants_check(&mut self) -> Result<(), String> {
        let hidden = match crate::resources::hidden_descendants() {
            Ok(v) => v,
            Err(e) => {
                self.log.line(
                    "WARN",
                    &format!("non-dumpable descendant check: /proc unreadable ({e})"),
                );
                return Ok(());
            }
        };
        if hidden.is_empty() {
            return Ok(());
        }
        let list: Vec<String> = hidden.iter().map(|(p, n)| format!("{p} ({n})")).collect();
        self.commit(vec![Event::Note {
            text: format!(
                "hidden_descendants {{\"pids\":{:?},\"names\":{:?}}}",
                hidden.iter().map(|x| x.0).collect::<Vec<_>>(),
                hidden.iter().map(|x| x.1.clone()).collect::<Vec<_>>()
            ),
        }]);
        Err(format!(
            "descendant process(es) of this supervisor under our uid that cannot be inspected (non-dumpable: /proc entry owned by root): {}; their marker is unreadable, nothing was signalled; check them by hand (ps -o pid,ppid,uid,cmd -p PID), then run the same command",
            list.join(", ")
        ))
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
            let name = p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            (control_seq_of(&name), name)
        });
        for p in files {
            let name = p.file_name().unwrap().to_owned();
            let done = dir.join("done");
            let _ = std::fs::create_dir_all(&done);
            let req: Option<ControlRequest> = std::fs::read(&p)
                .ok()
                .and_then(|b| serde_json::from_slice(&b).ok());
            // lane closeloop4 (closeloop.md 19): an operator decision of the work source (terminal proposals; lane
            // wireflow, closeloop.md 26: a facts-budget raise)
            if let Some(r) = req
                .as_ref()
                .filter(|r| r.verb == "resolve-terminals" || r.verb == "raise-facts-budget")
            {
                self.apply_source_decision(r);
                if self.ledger_dead.is_some() {
                    return;
                }
                let _ = std::fs::rename(&p, done.join(&name));
                continue;
            }
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

    /// lane closeloop4 (closeloop.md 19): `resolve-terminals` = the work source's operator decision: its records are
    /// written and its events committed like a poll routing (the decision is a ledger input); a refusal is a note.
    fn apply_source_decision(&mut self, r: &ControlRequest) {
        let routing = match self
            .source
            .resolve_operator_decision(self.ledger.state(), r)
        {
            Ok(x) => x,
            Err(why) => {
                self.log.line(
                    "WARN",
                    &format!("control {} ({}) refused: {why}", r.id, r.verb),
                );
                self.note(format!(
                    "control_refused {{\"id\":{:?},\"verb\":{:?},\"reason\":{why:?}}}",
                    r.id, r.verb
                ));
                return;
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
                    &format!("decision record {}: {e}; not recorded", a.path),
                );
                return;
            }
        }
        match self.commit_or_refused(routing.events) {
            Ok(true) => self.log.line(
                "INFO",
                &format!(
                    "control {}: {} recorded (by {})",
                    r.id,
                    r.verb,
                    if r.by.is_empty() { "operator" } else { &r.by }
                ),
            ),
            // int6 (closeloop4 x procfix6): a ledger that cannot take the record (disk full: procfix6) records nothing;
            // the request stays pending (the caller keeps it for the next session)
            Ok(false) => self.log.line(
                "ERROR",
                &format!(
                    "control {} (resolve-terminals): not recorded (the ledger cannot be written); the request stays pending",
                    r.id
                ),
            ),
            Err(e) => self.log.line(
                "ERROR",
                &format!("control {} (resolve-terminals): ledger refused: {e}", r.id),
            ),
        }
    }

    /// `resolve JOB` (requirement 05:23Z; lane int5 on the ledger's unconfirmed state): the operator's decision for an
    /// attempt whose execution is UNKNOWN (an `unconfirmed` launch, or the job of an item in attention with code
    /// `execution_unknown`). Recorded with the ledger's own verbs: `control retry` of the item (attention -> open, a new
    /// generation: the work is reopened, not counted as a failure) and `job.cancel` of the unconfirmed attempt (which can
    /// no longer feed the item); a note `control_resolve`. Refused (`control_refused`) for anything else.
    fn apply_resolve(&mut self, r: &ControlRequest) {
        let target = r.target.clone().unwrap_or_default();
        let found = self.st().jobs.get(&target).map(|j| {
            let attention = matches!(
                self.st().items.get(&j.item).map(|it| &it.phase),
                Some(ItemPhase::Attention(code)) if code == EXECUTION_UNKNOWN || code == CHILD_RESOURCE_FAILURE
            );
            (
                j.item.clone(),
                j.attempt,
                j.state == AttemptState::Unconfirmed,
                attention,
            )
        });
        let refuse = |s: &mut Self, why: &str| {
            s.note(format!(
                "control_refused {{\"id\":{:?},\"verb\":\"resolve\",\"target\":{target:?},\"reason\":{why:?}}}",
                r.id
            ));
        };
        let Some((item, attempt, unconfirmed, attention)) = found else {
            refuse(self, "unknown job");
            return;
        };
        if !unconfirmed && !attention {
            refuse(
                self,
                "not an attempt of unknown execution (unconfirmed launch) nor of an item in attention for repeated host-resource ends",
            );
            return;
        }
        if r.id.is_empty() || r.id.len() > 128 {
            refuse(self, "bad id");
            return;
        }
        if self.st().controls.contains_key(&r.id) {
            refuse(self, "control id already used");
            return;
        }
        let by = if r.by.is_empty() {
            "operator".to_string()
        } else {
            r.by.clone()
        };
        let reason = r.reason.clone().unwrap_or_default();
        let requested = Event::Control {
            id: r.id.clone(),
            ev: ControlEvent::Requested {
                verb: ControlVerb::Retry,
                target: ControlTarget::Item { item: item.clone() },
                by: by.clone(),
                reason: format!("resolve {target} a{attempt} (execution unknown): {reason}"),
            },
        };
        match self.commit_or_refused(vec![requested]) {
            Ok(true) => {}
            Ok(false) => return,
            Err(why) => {
                refuse(self, &why);
                return;
            }
        }
        let mut ev = vec![Event::Control {
            id: r.id.clone(),
            ev: ControlEvent::Applied {
                effect: format!(
                    "item {item} reopened by the operator's resolve of {target} a{attempt} (execution unknown; not a failure)"
                ),
            },
        }];
        if unconfirmed {
            ev.push(Event::Job {
                job: target.clone(),
                attempt,
                ev: JobEvent::Cancel {
                    reason: format!(
                        "execution unknown; resolved by {by} (control {}): its item was reopened",
                        r.id
                    ),
                },
            });
        }
        ev.push(Event::Note {
            text: format!(
                "control_resolve {{\"id\":{:?},\"job\":{target:?},\"attempt\":{attempt},\"item\":{item:?},\"by\":{by:?},\"reason\":{reason:?}}}",
                r.id
            ),
        });
        match self.commit_or_refused(ev) {
            Ok(true) => {
                self.unknown.remove(&target);
                self.unknown_charge.remove(&target);
                self.attention.remove(&target);
                self.child_resource_ends.remove(&target);
                self.log.line(
                    "INFO",
                    &format!(
                        "control {}: {target} a{attempt} resolved: item {item} reopened, attempt cancelled",
                        r.id
                    ),
                );
                match self.source.control(self.ledger.state(), "retry", &item) {
                    Ok(ev) => {
                        self.commit(ev);
                    }
                    Err(e) => self.log.line("WARN", &format!("control {}: {e}", r.id)),
                }
            }
            Ok(false) => {}
            Err(why) => {
                self.log.line(
                    "INFO",
                    &format!("control {} (resolve) refused: {why}", r.id),
                );
                self.commit(vec![Event::Control {
                    id: r.id.clone(),
                    ev: ControlEvent::Refused { why },
                }]);
            }
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
            return Err(
                "the ledger on disk reduces to another final result than the live state".into(),
            );
        }
        for (item, it) in &view.state.items {
            if let Some(a) = &it.accepted
                && matches!(it.phase, ItemPhase::Accepted(_))
                && !a.record.path.starts_with("large:")
            {
                let p = self.cdir.join(&a.record.path);
                match crate::atomic::blake3_file(&p) {
                    Ok((bytes, d)) if bytes == a.record.bytes && d == a.record.blake3 => {}
                    Ok(_) => {
                        return Err(format!(
                            "accepted record of {item} ({}) changed",
                            p.display()
                        ));
                    }
                    Err(e) => {
                        return Err(format!("accepted record of {item} ({}): {e}", p.display()));
                    }
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
                "supervisor resource back-off after {} failure(s)",
                self.resource_failures
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

    /// Lane closeloop (closeloop.md 6.1): the work source's own in-process pass results, written and committed as a
    /// routing (records first, then one group commit), exactly as at a settle.
    fn poll_source(&mut self) {
        let routing = match self.source.poll(self.ledger.state(), &self.ctx) {
            Ok(r) => r,
            Err(e) => {
                self.log.line("ERROR", &format!("work source poll: {e}"));
                return;
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
                    &format!("source record {}: {e}; not recorded", a.path),
                );
                return;
            }
        }
        if let Err(e) = self.commit_or_refused(routing.events) {
            self.log.line(
                "ERROR",
                &format!("work source events refused by the ledger: {e}"),
            );
        }
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
        // lane t266run (L17): the session's per-job memory cap (launch-time resource) replaces the source's cap
        let mem_cap = self.opts.spec.resources.job_mem_cap.map(|b| b.0);
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
            .map(|mut r| {
                if let Some(c) = mem_cap {
                    r.cap_bytes = c;
                    r.expected_peak_bytes = r.expected_peak_bytes.min(c);
                }
                r
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
            .chain(
                self.unknown_charge
                    .values()
                    .enumerate()
                    .map(|(k, &(cap, cores))| RunningJob {
                        // Execution unknown: charged in full until the operator resolves it (the ledger shows it
                        // launching).
                        id: u64::MAX / 2 - k as u64,
                        cap_bytes: cap,
                        expected_peak_bytes: cap,
                        rss_bytes: 0,
                        rss_measured_at: None,
                        cores,
                        started_seq: 0,
                        cpu_seconds: 0.0,
                        stopping: false,
                    }),
            )
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
            // lane wireflow (zerosfix finding): the cgroup headroom gets the SPEC's floor, the one it is compared with
            mem_available_bytes: host_memory()
                .map(|m| m.effective_available_for(self.admission.floor_bytes))
                .unwrap_or(0),
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
            host_mem_available_bytes: mem
                .map(|m| m.effective_available_for(self.admission.floor_bytes)),
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
        for item in std::mem::take(&mut self.deferred_source_retries) {
            match self.source.control(self.ledger.state(), "retry", &item) {
                Ok(ev) => {
                    if !self.commit(ev) {
                        return self.ledger_dead.clone().unwrap_or(RunExit::DiskFull);
                    }
                }
                Err(e) => self
                    .log
                    .line("WARN", &format!("item {item}: work source retry: {e}")),
            }
        }
        // lane t266run (L17): an item in attention for a memory-cap end whose last attempt ran under a LOWER cap than
        // this session's `--mem-cap` is reopened (the cause is gone; a new attempt, never the old result)
        if let Some(cap) = self.opts.spec.resources.job_mem_cap.map(|b| b.0) {
            let st = self.ledger.state();
            let mut ev = vec![];
            for (id, it) in st.items.iter() {
                if crate::launch::ended_at_memory_cap(st, id) {
                    let last = it
                        .jobs
                        .last()
                        .and_then(|j| st.jobs.get(j))
                        .map_or(0, |j| j.reserved_bytes);
                    if last < cap {
                        ev.push(Event::Item {
                            item: id.clone(),
                            ev: crate::ledger::ItemEvent::Reopen {
                                reason: format!(
                                    "memory cap raised from {last} to {cap} bytes (launch-time resource --mem-cap): a new attempt"
                                ),
                            },
                        });
                    }
                }
            }
            if !ev.is_empty() {
                self.log.line("INFO", &format!("{} item(s) in attention for a memory-cap end reopened at the raised cap {cap}", ev.len()));
                if !self.commit(ev) {
                    return self.ledger_dead.clone().unwrap_or(RunExit::DiskFull);
                }
            }
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
            // gate3 M2: every delivered signal becomes a ledger record
            self.record_deliveries();
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
                // lane closeloop (closeloop.md 6.1): results of the source's own in-process passes
                self.poll_source();
                let b = self.blockers();
                if b.reasons.is_empty() {
                    self.schedule();
                }
            }
            // 6. progress
            self.write_progress(false);
            // 7. exit conditions
            if !self.children.is_empty() || !self.pending.is_empty() {
                self.blocked_since = None;
            }
            if self.children.is_empty() && level > StopLevel::Running {
                // Draining: attempts still waiting for a proof, or held by live marked processes that cannot be killed
                // (gate3 M1), stay unsettled (the restart's recovery handles them).
                let held = &self.survivors;
                self.pending.retain(|p| {
                    (!p.awaiting_proof || p.sweep.is_some()) && !held.contains_key(&p.label())
                });
            }
            // gate3 M1: attempts held only by live marked processes that could not be killed do not keep the loop
            // waiting: the exit-8 rule below lists their pids.
            let blocked_pending = self
                .pending
                .iter()
                .all(|p| self.survivors.contains_key(&p.label()) && p.sweep.is_none());
            if self.children.is_empty() && (self.pending.is_empty() || blocked_pending) {
                if level > StopLevel::Running {
                    self.commit(vec![Event::Campaign {
                        ev: crate::state::CampaignEvent::Stop,
                        reason: "drained".into(),
                    }]);
                    self.write_progress(true);
                    if self.fsize_stop {
                        return RunExit::DiskFull;
                    }
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
                // procfix5: a source that is "finished" while operator decisions are pending (unknown executions,
                // items in attention for repeated resource ends, live marked survivors) does NOT finish: the exit-8 rule
                // below decides (a source may count an item in attention as settled for its own purposes).
                // lane closeloop4: a source waiting for an operator decision (terminal proposals) does not finish
                let source_awaits = self.source.operator_decision(self.ledger.state());
                let decisions_pending = !self.unknown.is_empty()
                    || !self.attention.is_empty()
                    || !self.survivors.is_empty()
                    || source_awaits.is_some();
                if !decisions_pending && self.source.finished(self.ledger.state()) {
                    if let Err(msg) = self.final_sweep() {
                        // procfix6 (gate5 m3): a final sweep that could not signal for lack of space is exit 75.
                        self.check_not_signalled();
                        if let Some(e) = self.ledger_dead.clone() {
                            self.log.line("WARN", &msg);
                            return e;
                        }
                        self.log.line("WARN", &msg);
                        self.commit(vec![Event::Campaign {
                            ev: crate::state::CampaignEvent::Stop,
                            reason: msg.clone(),
                        }]);
                        self.write_progress(true);
                        return RunExit::Attention(msg);
                    }
                    // lane t266run: the work is final but an operator must decide (e.g. terminal proposals)
                    if let Some(msg) = self.source.operator_decision(self.ledger.state()) {
                        let msg = format!("operator decision needed: {msg}");
                        self.log.line("WARN", &msg);
                        self.commit(vec![Event::Campaign {
                            ev: crate::state::CampaignEvent::Stop,
                            reason: msg.clone(),
                        }]);
                        self.write_progress(true);
                        return RunExit::Attention(msg);
                    }
                    if self.st().phase == Some(CampaignPhase::Running) {
                        self.commit(vec![Event::Campaign {
                            ev: crate::state::CampaignEvent::Finish,
                            reason: "all work final".into(),
                        }]);
                    }
                    // procfix5: a refused / failed finish record is never reported as a finished campaign.
                    if let Some(e) = self.ledger_dead.clone() {
                        return e;
                    }
                    self.write_progress(true);
                    if let Err(e) = self.final_consistency() {
                        self.log.line("ERROR", &format!("NOT FINISHED: {e}"));
                        return RunExit::Internal(e);
                    }
                    // procfix5 (gate4 m3): exit 0 only when every node is closed.
                    return match self.source.outcome(self.ledger.state()) {
                        worksource::CampaignOutcome::Closed => RunExit::Finished,
                        worksource::CampaignOutcome::Open(m) => {
                            self.log.line(
                                "WARN",
                                &format!(
                                    "FINISHED, NOT ALL CLOSED (exit {}): {m}",
                                    exit::FINISHED_OPEN
                                ),
                            );
                            RunExit::FinishedOpen(m)
                        }
                        worksource::CampaignOutcome::Error(m) => {
                            self.log.line(
                                "WARN",
                                &format!(
                                    "FINISHED WITH AN ERROR (exit {}): {m}",
                                    exit::FINISHED_ERROR
                                ),
                            );
                            RunExit::FinishedError(m)
                        }
                    };
                }
                // lane t266run (L17): nothing runs, nothing is ready, and the source waits for an operator (e.g. a higher
                // --mem-cap for items that ended at their memory cap)
                if level == StopLevel::Running
                    && self.last_ready == 0
                    && let Some(m) = self.source.blocked_on_operator(self.ledger.state())
                {
                    let msg = format!("operator decision needed: {m}");
                    self.log.line("WARN", &msg);
                    self.commit(vec![Event::Campaign {
                        ev: crate::state::CampaignEvent::Stop,
                        reason: msg.clone(),
                    }]);
                    self.write_progress(true);
                    return RunExit::Attention(msg);
                }
                // Nothing runs and attempts of unknown execution block the rest (nothing is ready, or their reservations
                // keep admission waiting for more than 2 s with nothing running): an operator decides.
                let blocked_for = self
                    .blocked_since
                    .get_or_insert_with(Instant::now)
                    .elapsed();
                if level == StopLevel::Running
                    && (!self.unknown.is_empty()
                        || !self.attention.is_empty()
                        || !self.survivors.is_empty()
                        || source_awaits.is_some())
                    && (self.last_ready == 0 || blocked_for > Duration::from_secs(2))
                    && self
                        .resource_backoff_until
                        .is_none_or(|t| Instant::now() >= t)
                    && !self.st().paused
                {
                    let mut parts = Vec::new();
                    if !self.unknown.is_empty() {
                        let jobs: Vec<String> = self
                            .unknown
                            .iter()
                            .map(|(j, a)| format!("{j} a{a}"))
                            .collect();
                        parts.push(format!(
                            "execution unknown for {} (`rr_campaign control CDIR resolve JOB` reopens its item)",
                            jobs.join(", ")
                        ));
                    }
                    if !self.attention.is_empty() {
                        let jobs: Vec<String> = self
                            .attention
                            .iter()
                            .map(|(j, a)| format!("{j} a{a}"))
                            .collect();
                        parts.push(format!(
                            "items in attention for {} (`rr_campaign control CDIR resolve JOB`)",
                            jobs.join(", ")
                        ));
                    }
                    if !self.survivors.is_empty() {
                        let s: Vec<String> = self
                            .survivors
                            .iter()
                            .map(|(l, p)| format!("{l}: pids {p:?}"))
                            .collect();
                        parts.push(format!(
                            "live marked processes that could not be killed hold attempts unsettled ({}): check and kill exactly those pids, then run the same command",
                            s.join("; ")
                        ));
                    }
                    if let Some(w) = &source_awaits {
                        parts.push(w.clone());
                    }
                    let msg = format!("operator decision needed: {}", parts.join("; "));
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
            RunExit::FinishedOpen(m) | RunExit::FinishedError(m) => format!(
                "FINISHED (exit {}), NOT ALL CLOSED: {} accepted, {} terminal(s); {m}",
                r.code(),
                self.st().rules.accepted,
                self.st().terminals_total
            ),
            RunExit::Stopped { code } => {
                format!("STOPPED (exit {code}); resume with the same command")
            }
            other => format!("{other:?}"),
        };
        self.log.line("INFO", &msg);
        // gate-A m-7: the last pinned-group SIGKILLs (live children of a fast exit, unproven ends) happen while this
        // supervisor still holds the campaign lock and the id lock, never after.
        let before = crate::launch::delivered_total();
        self.children.clear();
        self.pending.clear();
        let n = self.launcher.abandon_quarantine();
        // procfix6 (gate5 m3): count what was really delivered (a full intent log sends nothing).
        let delivered = crate::launch::delivered_total().saturating_sub(before);
        self.check_not_signalled();
        if n > 0 {
            self.log.line(
                "WARN",
                &format!("{n} unproven job end(s) abandoned at exit ({delivered} pinned-group / process signal(s) delivered while dropping the live jobs and them); the restart's sweep looks for their markers"),
            );
        }
        if let Some(m) = self.not_signalled_summary() {
            self.log.line("ERROR", &m);
        }
        // gate3 M2: the last deliveries (shutdown, abandoned quarantine) are recorded while the lock is still held.
        self.record_deliveries();
        let _ = self.ledger.sync();
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
    /// lane closeloop4: the data of a work-source decision (`resolve-terminals`: `{"accept": [keys]}` | `{"reject":
    /// true}`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
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

/// The next control request sequence number of `CDIR/control` (gate-B m1), WITHOUT any lock (a control command takes
/// no flock: gate-B's read-only rule): the number is claimed by creating `control/seq/<n>` with O_EXCL, starting after the
/// largest claimed number; two concurrent writers get distinct numbers in the order of their claims.
pub fn next_control_seq(control_dir: &Path) -> Result<u64, String> {
    let dir = control_dir.join("seq");
    std::fs::create_dir_all(&dir).map_err(|e| format!("control/seq: {e}"))?;
    let mut n = std::fs::read_dir(&dir)
        .map_err(|e| format!("control/seq: {e}"))?
        .flatten()
        .filter_map(|e| e.file_name().to_str().and_then(|s| s.parse::<u64>().ok()))
        .max()
        .unwrap_or(0);
    for _ in 0..10_000 {
        n += 1;
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(dir.join(n.to_string()))
        {
            Ok(_) => return Ok(n),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("control/seq/{n}: {e}")),
        }
    }
    Err("control/seq: no free number after 10,000 tries".into())
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

/// A sealed output as read ONCE by [`read_verified_stream`].
#[derive(Debug)]
struct BoundTable {
    rel: String,
    bytes: u64,
    blake3: String,
    canonical: String,
    lines: Vec<String>,
}

/// The verified stream a settle reads: the table of a table-producing kind (the FIRST verified stream: `assess` lists
/// the table first), or the sealed `results` stream of a table-less kind (lane closeloop: solve-recipe). `None` when
/// the classification verified no such stream.
fn settle_stream(
    kind: rustred_campaign_contract::spec::JobKind,
    verified: Option<&classify::VerifiedOutputs>,
) -> Option<&rustred_campaign_contract::seal::VerifiedStream> {
    let v = verified?;
    if kind.produces_table() {
        v.streams.first()
    } else {
        let rel = RunLayout::out_rel(rustred_campaign_contract::layout::RESULTS);
        v.streams.iter().find(|s| s.rel == rel)
    }
}

/// The settle's sealed-output decision (lane int5: ONE function for the table AND closeloop's results stream).
/// `wants_records`: the row may accept records. `Ok(None)`: nothing to read (no such row, or a table-less kind whose
/// classification verified no results stream); `Ok(Some(t))`: the lines, read once through the verified handle;
/// `Err(why)`: a stream had to be read and could not be (swapped, changed, removed or unreadable since it was
/// verified; a table kind without its verified table): the attempt accepts NOTHING (the settle records class crashed).
fn settle_output(
    layout: &RunLayout,
    kind: rustred_campaign_contract::spec::JobKind,
    verified: Option<&classify::VerifiedOutputs>,
    wants_records: bool,
) -> Result<Option<BoundTable>, String> {
    if !wants_records {
        return Ok(None);
    }
    match settle_stream(kind, verified) {
        Some(st) => read_verified_stream(layout, st).map(Some),
        None if kind.produces_table() => Err("no verified stream for the sealed table".into()),
        None => Ok(None),
    }
}

/// ac2-M1 (procfix3 item 5; lane int5: the ONE reader of accepted lines, for the table AND for closeloop's results
/// stream): the stream that `assess` verified, re-opened through the contract's verified-stream handle
/// (`seal::VerifiedHandle::reopen`: run-relative no-follow open, the verified (dev, inode) or refused) and read ONCE:
/// `canonical_lines()` returns the lines only if the bytes, raw BLAKE3, record count and canonical digest equal the
/// verified stream, every line with its canonical form (no silent drop): all or nothing.
fn read_verified_stream(
    layout: &RunLayout,
    st: &rustred_campaign_contract::seal::VerifiedStream,
) -> Result<BoundTable, String> {
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
    f.read_to_end(&mut b)
        .map_err(|e| format!("read {rel}: {e}"))?;
    let o = rustred_campaign_contract::outcome::Outcome::from_slice(&b)?;
    if o.job != *verified {
        return Err("its content is not the classified OUTCOME".into());
    }
    Ok((b.len() as u64, blake3::hash(&b).to_hex().to_string()))
}

/// `RUN/sup/SPAWN`, created and fsynced (file and directory) right before the spawn.
fn write_spawn_sentinel(layout: &RunLayout, barrier: bool) -> std::io::Result<()> {
    use std::io::Write as _;
    let dir = layout.sup_dir();
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(dir.join(SPAWN_SENTINEL))?;
    if barrier {
        // procfix5 m1: the child runs nothing of its program before `job.started` is durable (spawn barrier): while the
        // attempt has no `job.started` record, the child never acted, so this content is intact evidence.
        f.write_all(SPAWN_BARRIER_TAG.as_bytes())?;
    }
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

/// A ledger error that means "no more room" (gate-B m3): ENOSPC / EDQUOT / EFBIG in an I/O error, or a poisoned handle
/// whose cause text names them (the ledger keeps only the text there: FOR LEDGERFIX2, carry the kind).
/// procfix5 (gate4 m6): fold one ledger record into the per-job count of host-resource ends IN A ROW (the trailing run
/// of row 15 / 15b requeues of the job; any other requeue ends the run; the operator's `resolve` of the job, recorded as
/// the note `control_resolve`, re-arms it). The supervisor derives the count at recovery: a restart never re-arms it.
fn fold_resource_ends(map: &mut BTreeMap<String, u32>, ev: &Event) {
    match ev {
        Event::Job {
            job,
            ev: JobEvent::Requeue { reason },
            ..
        } => {
            if is_resource_requeue(reason) {
                *map.entry(job.clone()).or_insert(0) += 1;
            } else {
                map.remove(job);
            }
        }
        Event::Note { text } => {
            if let Some(js) = text.strip_prefix("control_resolve ")
                && let Ok(v) = serde_json::from_str::<serde_json::Value>(js)
                && let Some(j) = v.get("job").and_then(|j| j.as_str())
            {
                map.remove(j);
            }
        }
        _ => {}
    }
}

/// procfix5 (item 5): does `p version --json` say it is the fake child (`"program": "rr_fake_child"`)? A program
/// that does not answer counts as real.
fn fake_program(p: &Path) -> bool {
    crate::cli::version_line(p)
        .ok()
        .and_then(|l| serde_json::from_str::<serde_json::Value>(l.trim()).ok())
        .is_some_and(|v| v.get("program").and_then(|x| x.as_str()) == Some("rr_fake_child"))
}

/// procfix6: the word of an attempt of the ORIGINAL campaign met in a copied / moved directory (note, attention code,
/// cancel / lose reasons).
pub const FOREIGN_ATTEMPT: &str = "foreign_attempt";

/// Start of the m4 reason of a SIGXFSZ death.
const SIGXFSZ_WHY: &str = "killed by SIGXFSZ";

/// This process's soft RLIMIT_FSIZE in bytes when it is finite (procfix6 m4).
fn own_file_size_limit() -> Option<u64> {
    // SAFETY: getrlimit fills a plain struct.
    let mut r: libc::rlimit = unsafe { std::mem::zeroed() };
    // SAFETY: valid out-pointer.
    if unsafe { libc::getrlimit(libc::RLIMIT_FSIZE, &mut r) } != 0 {
        return None;
    }
    (r.rlim_cur != libc::RLIM_INFINITY).then_some(r.rlim_cur as u64)
}

/// procfix6 (gate5 m4): free bytes below which a file system counts as FULL when a child ended without a usable OUTCOME.
const DISK_FULL_BELOW_BYTES: u64 = 1 << 20;

/// Free bytes (for an unprivileged writer) of the file system holding `p`; None when statvfs fails.
fn free_bytes(p: &Path) -> Option<u64> {
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(p.as_os_str().as_bytes()).ok()?;
    // SAFETY: statvfs is plain data; zeroed is a valid initial value.
    let mut v: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: a NUL-terminated path and a valid out-pointer.
    if unsafe { libc::statvfs(c.as_ptr(), &mut v) } != 0 {
        return None;
    }
    Some((v.f_bavail as u64).saturating_mul(v.f_frsize as u64))
}

/// Does an OS error rendering in `m` name errno `n` (`os error N` / `Os { code: N`)?
fn names_errno(m: &str, n: i32) -> bool {
    ["os error ", "Os { code: "].iter().any(|pat| {
        m.match_indices(pat).any(|(i, _)| {
            let d: String = m[i + pat.len()..]
                .chars()
                .take_while(char::is_ascii_digit)
                .collect();
            d.parse::<i32>().ok() == Some(n)
        })
    })
}

/// procfix6 (gate5 m4): why a CRASHED classification is really the host's disk state, if it is: the child was killed by
/// SIGXFSZ (`ulimit -f` / a file-size limit), its OUTCOME names EFBIG, or it left no usable OUTCOME (absent, torn) while
/// one of `dirs` (the run directory, the campaign directory) is on a file system with less than
/// [`DISK_FULL_BELOW_BYTES`] free at settle time. Never for a valid OUTCOME that names something else.
fn disk_attributed_end(
    ev: &classify::Evidence,
    c: &classify::Classification,
    dirs: &[&Path],
) -> Option<String> {
    if c.kind != classify::OutcomeKind::Crashed {
        return None;
    }
    let sig = match ev.wait {
        WaitStatus::Signaled(s) => Some(s),
        WaitStatus::Exited(x) if (129..=192).contains(&x) => Some(x - 128),
        _ => None,
    };
    if sig == Some(libc::SIGXFSZ) {
        return Some(format!(
            "{SIGXFSZ_WHY} (a file-size limit: the host's `ulimit -f` or the file system)"
        ));
    }
    match &ev.outcome {
        OutcomeState::Valid(o) => o
            .error
            .as_ref()
            .filter(|e| names_errno(&e.message, libc::EFBIG))
            .map(|_| "its OUTCOME names EFBIG (file too large)".to_string()),
        OutcomeState::Absent | OutcomeState::Invalid(_) | OutcomeState::Unreadable(_) => {
            dirs.iter().find_map(|d| {
                free_bytes(d)
                    .filter(|&f| f < DISK_FULL_BELOW_BYTES)
                    .map(|f| {
                        format!(
                            "no usable OUTCOME while the file system of {} is full ({f} bytes free)",
                            d.display()
                        )
                    })
            })
        }
    }
}

/// The row-15b classification that replaces a disk-attributed crash (procfix6 m4).
fn resource_classification(c: &classify::Classification, why: String) -> classify::Classification {
    classify::Classification {
        row: 15,
        sub: "b".into(),
        kind: classify::OutcomeKind::Lost,
        accept: classify::Accept::Nothing,
        action: classify::Action::RetryAfterResourceBackoff,
        resume_from_r: false,
        resume_from: None,
        time_decided: false,
        attention: false,
        why: format!("{why} (classified row {}{}: {})", c.row, c.sub, c.why),
    }
}

/// Is this `job.requeue` reason the one of a host-resource end (decision rows 15 / 15b; written by `settle` as
/// `"<class> (row 15<sub>)"`)? procfix5 m6: the count of such ends in a row is derived from the ledger at recovery.
fn is_resource_requeue(reason: &str) -> bool {
    reason.ends_with("(row 15)") || reason.ends_with("(row 15b)")
}

fn ledger_error_is_disk_full(e: &LedgerError) -> bool {
    match e {
        LedgerError::NoSpace { .. } => true,
        LedgerError::Io { source, .. } => matches!(
            source.raw_os_error(),
            Some(libc::ENOSPC | libc::EDQUOT | libc::EFBIG)
        ),
        LedgerError::Poisoned { why } => {
            let w = why.to_ascii_lowercase();
            w.contains("file too large")
                || w.contains("no space left")
                || w.contains("quota exceeded")
                // procfix6: a handle poisoned by ENOSPC carries `LedgerError::NoSpace`'s text
                || w.contains("disk full appending")
        }
        _ => false,
    }
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
        w.append(br#"{"type":"header","zero_seconds":0.25}"#)
            .unwrap();
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
            relations: None,
        }
    }

    /// The settle's selection + read for a table kind.
    fn read_table(
        l: &RunLayout,
        v: Option<&classify::VerifiedOutputs>,
    ) -> Result<BoundTable, String> {
        settle_stream(rustred_campaign_contract::spec::JobKind::Fake, v)
            .ok_or_else(|| "no verified stream".to_string())
            .and_then(|st| read_verified_stream(l, st))
    }

    /// Lane int5: closeloop's sealed `results` stream (a table-less kind) goes through the SAME reader as a table: the
    /// stream is selected by its layout path (never the first stream of another kind), its lines come from the verified
    /// handle, and a results file changed after the classification accepts nothing.
    #[test]
    fn results_stream_of_a_table_less_kind_is_read_through_the_same_verified_handle() {
        use rustred_campaign_contract::spec::{JobKind, RESULT_END_TYPES};
        let l = run_dir("results");
        let rel = RunLayout::out_rel(rustred_campaign_contract::layout::RESULTS);
        {
            let mut w = RecordWriter::create(&l.root.join(&rel)).unwrap();
            for i in 0..3 {
                w.append(format!(r#"{{"type":"relation","key":"I({i})","index":{i}}}"#).as_bytes())
                    .unwrap();
            }
            w.seal(br#"{"type":"end","records":3}"#).unwrap();
        }
        let (rs, _) = seal::verify_stream_handle(&l.root, &rel, &RESULT_END_TYPES).unwrap();
        let decoy = {
            let mut d = rs.clone();
            d.rel = "out/other.jsonl".into();
            d
        };
        let v = classify::VerifiedOutputs {
            rules: 0,
            elided_loci: 0,
            streams: vec![decoy, rs.clone()],
            relations: None,
        };
        let st = settle_stream(JobKind::SolveRecipe, Some(&v)).expect("results stream selected");
        assert_eq!(st.rel, rel);
        let t = read_verified_stream(&l, st).unwrap();
        assert_eq!(t.lines.len(), 4);
        assert_eq!(
            (t.bytes, t.canonical.as_str()),
            (rs.bytes, rs.canonical_blake3.as_str())
        );
        // no results stream verified: nothing selected (the settle then reads nothing and accepts nothing)
        let none = classify::VerifiedOutputs {
            streams: vec![v.streams[0].clone()],
            ..v.clone()
        };
        assert!(settle_stream(JobKind::SolveRecipe, Some(&none)).is_none());
        // appended after the classification: refused (all or nothing)
        {
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new()
                .append(true)
                .open(l.root.join(&rel))
                .unwrap();
            f.write_all(b"{\"type\":\"relation\",\"key\":\"I(9)\"}\n")
                .unwrap();
        }
        assert!(read_verified_stream(&l, st).is_err());
        let _ = std::fs::remove_dir_all(&l.root);
    }

    /// Lane int5 regression (the merge's settle defect: closeloop's results branch re-read the file by PATH and a failed
    /// read still counted as sealed): for a table-less kind whose results stream was verified, a results file SWAPPED
    /// (another sealed file renamed over it, even byte-identical: another inode), REMOVED, or rewritten in place after
    /// the verification yields `Err` = nothing accepted, never `Ok(None)` (which would settle as sealed with no lines).
    #[test]
    fn results_stream_swapped_or_unreadable_after_verification_accepts_nothing() {
        use rustred_campaign_contract::spec::{JobKind, RESULT_END_TYPES};
        let rel = RunLayout::out_rel(rustred_campaign_contract::layout::RESULTS);
        let write = |l: &RunLayout, n: u64| {
            let mut w = RecordWriter::create(&l.root.join(&rel)).unwrap();
            for i in 0..n {
                w.append(format!(r#"{{"type":"relation","key":"I({i})","index":{i}}}"#).as_bytes())
                    .unwrap();
            }
            w.seal(format!(r#"{{"type":"end","records":{n}}}"#).as_bytes())
                .unwrap();
        };
        let fresh = |tag: &str| {
            let l = run_dir(tag);
            write(&l, 3);
            let (st, _) = seal::verify_stream_handle(&l.root, &rel, &RESULT_END_TYPES).unwrap();
            let v = classify::VerifiedOutputs {
                rules: 0,
                elided_loci: 0,
                streams: vec![st],
                relations: None,
            };
            (l, v)
        };
        let k = JobKind::SolveRecipe;
        // honest: the lines
        let (l, v) = fresh("res-honest");
        let t = settle_output(&l, k, Some(&v), true).unwrap().expect("read");
        assert_eq!(t.lines.len(), 4);
        // a row that accepts no records reads nothing, whatever the files
        assert!(settle_output(&l, k, Some(&v), false).unwrap().is_none());
        // swapped by another sealed results file (different content), then by a byte-identical copy (new inode)
        let other = run_dir("res-other");
        write(&other, 5);
        std::fs::rename(other.root.join(&rel), l.root.join(&rel)).unwrap();
        let e = settle_output(&l, k, Some(&v), true).unwrap_err();
        assert!(e.contains("InodeChanged") || e.contains("changed"), "{e}");
        let (l2, v2) = fresh("res-copy");
        let copy = l2.root.join("out/copy.jsonl");
        std::fs::copy(l2.root.join(&rel), &copy).unwrap();
        std::fs::rename(&copy, l2.root.join(&rel)).unwrap();
        assert!(
            settle_output(&l2, k, Some(&v2), true).is_err(),
            "identical bytes, other inode"
        );
        // removed after the verification
        let (l3, v3) = fresh("res-removed");
        std::fs::remove_file(l3.root.join(&rel)).unwrap();
        assert!(settle_output(&l3, k, Some(&v3), true).is_err());
        // rewritten in place (same inode, truncated)
        let (l4, v4) = fresh("res-truncated");
        std::fs::OpenOptions::new()
            .write(true)
            .open(l4.root.join(&rel))
            .unwrap()
            .set_len(10)
            .unwrap();
        assert!(settle_output(&l4, k, Some(&v4), true).is_err());
        // a table-less kind with no verified results stream: nothing to read (checker kinds); a table kind without
        // its verified table: refused
        let none = classify::VerifiedOutputs {
            streams: vec![],
            ..v.clone()
        };
        assert!(settle_output(&l, k, Some(&none), true).unwrap().is_none());
        assert!(settle_output(&l, JobKind::Fake, Some(&none), true).is_err());
        assert!(settle_output(&l, JobKind::Fake, None, true).is_err());
        for d in [&l, &other, &l2, &l3, &l4] {
            let _ = std::fs::remove_dir_all(&d.root);
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
        let t = read_table(&l, Some(&v)).unwrap();
        assert_eq!(t.lines.len(), 5);
        assert_eq!(t.canonical, v.streams[0].canonical_blake3);
        assert_eq!(
            (t.bytes, t.blake3.as_str()),
            (v.streams[0].bytes, v.streams[0].blake3.as_str())
        );
        // (b) garbage appended after the classification
        {
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new()
                .append(true)
                .open(l.table())
                .unwrap();
            f.write_all(b"{\"type\":\"rule\",\"index\":99} trailing garbage\n")
                .unwrap();
        }
        assert!(read_table(&l, Some(&v)).is_err());
        // (a) replaced by another sealed table (rename: another inode)
        let other = run_dir("bound-other");
        sealed_table(&other, 4);
        std::fs::rename(other.table(), l.table()).unwrap();
        let e = read_table(&l, Some(&v)).unwrap_err();
        assert!(e.contains("InodeChanged") || e.contains("changed"), "{e}");
        // no verified stream at all: nothing
        assert!(read_table(&l, None).is_err());
        let _ = std::fs::remove_dir_all(&l.root);
        let _ = std::fs::remove_dir_all(&other.root);
    }

    struct Confirming;
    impl WorkSource for Confirming {
        fn bootstrap(
            &mut self,
            _: &CampaignState,
            _: &SourceContext,
        ) -> Result<Vec<Event>, String> {
            Ok(vec![])
        }
        fn ready(&mut self, _: &CampaignState, _: &SourceContext, _: usize) -> Vec<JobRequest> {
            vec![]
        }
        fn route(
            &mut self,
            _: &CampaignState,
            _: &SourceContext,
            _: &Settled,
        ) -> Result<worksource::Routing, String> {
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
        assert_eq!(
            s.classification.action,
            classify::Action::ConfirmCoverageElseNextRung
        );
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

    /// gate (d): the release build's refusal matches exactly the test-hook variable's name.
    /// procfix5 (gate4 m6): the count of host-resource ends in a row is derived from the ledger (it survives a
    /// restart), any other requeue ends the run, the operator's resolve re-arms it.
    #[test]
    fn m6_resource_end_count_is_derived_from_the_ledger() {
        let rq = |job: &str, a: u32, reason: &str| Event::Job {
            job: job.into(),
            attempt: a,
            ev: JobEvent::Requeue {
                reason: reason.into(),
            },
        };
        let mut m = BTreeMap::new();
        for (a, r) in [
            (2, "lost (row 15b)"),
            (3, "lost (row 15b)"),
            (4, "lost (row 15)"),
        ] {
            fold_resource_ends(&mut m, &rq("j1", a, r));
        }
        fold_resource_ends(&mut m, &rq("j2", 2, "lost (row 15b)"));
        assert_eq!(m.get("j1"), Some(&3), "three in a row, across any sessions");
        fold_resource_ends(&mut m, &rq("j2", 3, "lost (row 21c)"));
        assert_eq!(m.get("j2"), None, "another end breaks the run");
        fold_resource_ends(
            &mut m,
            &Event::Note {
                text: r#"control_resolve {"id":"c1","job":"j1","attempt":4,"item":"i","by":"op","reason":null}"#
                    .into(),
            },
        );
        assert_eq!(m.get("j1"), None, "resolve re-arms");
        assert!(is_resource_requeue("lost (row 15b)") && !is_resource_requeue("lost (row 15c)"));
    }

    #[test]
    fn release_refusal_digest_is_the_test_hook_variable_name() {
        assert_eq!(
            blake3::hash(failpoint::FAILPOINT_ENV.as_bytes())
                .to_hex()
                .as_str(),
            TEST_HOOK_ENV_BLAKE3
        );
        assert_eq!(release_refusal(), None, "a test build never refuses");
    }

    #[test]
    fn control_sequence_numbers_order_requests_of_one_second() {
        let d = run_dir("seq").root;
        std::fs::create_dir_all(&d).unwrap();
        let a = next_control_seq(&d).unwrap();
        let b = next_control_seq(&d).unwrap();
        assert_eq!(b, a + 1);
        assert_eq!(
            control_seq_of(&format!("c-{b:012}-20261114T000000Z-ab.json")),
            b
        );
        assert_eq!(control_seq_of("c-20261114T000000Z-ab.json"), 0);
        let mut names = [
            format!("c-{b:012}-20261114T000000Z-00.json"),
            format!("c-{a:012}-20261114T000000Z-ff.json"),
        ];
        names.sort_by_key(|n| (control_seq_of(n), n.clone()));
        assert!(names[0].contains(&format!("{a:012}")));
        let _ = std::fs::remove_dir_all(&d);
    }

    // ------------------------------------------------------------------------------------- procfix6 (gate5 m4)

    fn evidence(wait: WaitStatus) -> classify::Evidence {
        classify::Evidence {
            kind: rustred_campaign_contract::spec::JobKind::Generate,
            wait,
            outcome: OutcomeState::Absent,
            verified: None,
            unsealed_why: None,
            resume_valid: false,
            resume_ref: None,
            kill: CKill::None,
            oom_kill_observed: false,
            elision_forbidden: true,
        }
    }

    /// A small tmpfs, filled up (inside the pid / mount namespace of the crate's test run); None outside one.
    fn full_tmpfs(tag: &str) -> Option<PathBuf> {
        if !crate::launch::in_private_namespace() {
            return None;
        }
        let d = run_dir(tag).root;
        let c = std::ffi::CString::new(d.to_string_lossy().as_bytes()).ok()?;
        // SAFETY: mount with NUL-terminated strings (tmpfs may be mounted by the root of a user namespace).
        let r = unsafe {
            libc::mount(
                c"tmpfs".as_ptr(),
                c.as_ptr(),
                c"tmpfs".as_ptr(),
                0,
                c"size=256k".as_ptr().cast(),
            )
        };
        if r != 0 {
            eprintln!("tmpfs mount failed: {}", std::io::Error::last_os_error());
            return None;
        }
        let mut f = std::fs::File::create(d.join("filler")).ok()?;
        use std::io::Write;
        while f.write_all(&[0u8; 4096]).is_ok() {}
        Some(d)
    }

    /// procfix6 (gate5 m4): SIGXFSZ (as a signal or as the shell's 128+25), an OUTCOME naming EFBIG, and a missing OUTCOME
    /// while the run directory's file system is full are RESOURCE ends (row 15b, lost, retried after the back-off);
    /// DECOYS stay what they were: a missing OUTCOME on a file system with space is a crash (row 23); SIGKILL from
    /// outside is row 21 (not a crash, untouched); an OUTCOME naming another errno is not re-read as a disk end.
    #[test]
    fn m4_disk_attributed_child_ends_are_resource_ends() {
        let roomy = std::env::temp_dir();
        for w in [
            WaitStatus::Signaled(libc::SIGXFSZ),
            WaitStatus::Exited(128 + libc::SIGXFSZ),
        ] {
            let ev = evidence(w);
            let c = classify::classify(&ev);
            assert_eq!(c.kind, classify::OutcomeKind::Crashed, "{c:?}");
            let why = disk_attributed_end(&ev, &c, &[roomy.as_path()]).expect("SIGXFSZ");
            let r = resource_classification(&c, why);
            assert_eq!((r.row, r.sub.as_str()), (15, "b"));
            assert_eq!(r.kind, classify::OutcomeKind::Lost);
            assert_eq!(r.action, classify::Action::RetryAfterResourceBackoff);
            assert!(!r.attention);
        }
        // decoy: no OUTCOME, a file system with space: still a crash
        let ev = evidence(WaitStatus::Exited(1));
        let c = classify::classify(&ev);
        assert_eq!(c.row, 23);
        assert_eq!(disk_attributed_end(&ev, &c, &[roomy.as_path()]), None);
        // decoy: SIGKILL from outside, no OUTCOME: row 21 (lost), never re-classified
        let ev = evidence(WaitStatus::Signaled(libc::SIGKILL));
        let c = classify::classify(&ev);
        assert_eq!(c.row, 21);
        assert_eq!(disk_attributed_end(&ev, &c, &[roomy.as_path()]), None);
        // errno renderings
        assert!(names_errno(
            "write OUTCOME: File too large (os error 27)",
            libc::EFBIG
        ));
        assert!(names_errno(
            "Os { code: 27, kind: FileTooLarge }",
            libc::EFBIG
        ));
        assert!(!names_errno("Permission denied (os error 13)", libc::EFBIG));
        assert!(!names_errno("os error 270", libc::EFBIG));
        // the same missing OUTCOME on a FULL file system: a resource end
        match full_tmpfs("m4full") {
            Some(full) => {
                let ev = evidence(WaitStatus::Exited(1));
                let c = classify::classify(&ev);
                let why = disk_attributed_end(&ev, &c, &[full.as_path()]).expect("full fs");
                assert!(why.contains("is full"), "{why}");
                // SAFETY: unmount our own test mount.
                let cs = std::ffi::CString::new(full.to_string_lossy().as_bytes()).unwrap();
                unsafe { libc::umount2(cs.as_ptr(), libc::MNT_DETACH) };
                let _ = std::fs::remove_dir_all(&full);
            }
            None => eprintln!(
                "full-fs part SKIPPED: needs the private namespace of the crate's test run"
            ),
        }
    }
}
