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
    RealChild, RealLauncher,
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
use rustred_campaign_contract::canon::{CanonicalDigest, canonical_line};
use rustred_campaign_contract::classify::{self, KillReason as CKill, OutcomeState, WaitStatus};
use rustred_campaign_contract::events::{Event as ChildEvent, EventsTail, TailItem};
use rustred_campaign_contract::layout::{RunLayout, attempt_dir};
use rustred_campaign_contract::resume::ResumeRef;
use rustred_campaign_contract::spec::JobSpec;
use spec::CampaignSpec;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
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
    pub const PF_SPEC: i32 = 20;
    pub const PF_PROGRAM: i32 = 21;
    pub const PF_LICENCE: i32 = 22;
    pub const PF_MEMORY: i32 = 23;
    pub const PF_DISK: i32 = 24;
    pub const PF_LOCK: i32 = 25;
    pub const PF_INPUT: i32 = 26;
    pub const INTERNAL: i32 = 70;
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
            // lane closeloop (closeloop.md 6.2): an input of role `closeloop` names the close-from-material config
            None if opts.spec.inputs.iter().any(|i| i.role == crate::closeloop::CONFIG_ROLE) => Box::new(
                crate::closeloop::CloseFromMaterialSource::from_spec(&opts.spec, &cdir)
                    .map_err(RunExit::Refused)?,
            ),
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

    /// Recovery at start (interface section 4): orphan scan by marker, then every unsettled attempt is settled.
    fn recover(&mut self) {
        let unsettled: Vec<(String, u32)> = self
            .st()
            .unsettled_jobs()
            .into_iter()
            .map(|(id, j)| (id.clone(), j.attempt))
            .collect();
        // The boot and start bounds of the previous sessions, from the ledger itself.
        let mut rules = self.scan_rules();
        let mut min_ticks: Option<u64> = None;
        // streamed (ledgerfix2 m10: never every record of a long campaign in memory)
        let boot_id = self.boot_id.clone();
        let _ = crate::ledger::for_each_record(&self.cdir, &mut |r, _| {
            if let Event::SupervisorStarted { identity, .. } = &r.event
                && identity.boot_id == boot_id
            {
                min_ticks =
                    Some(min_ticks.map_or(identity.start_ticks, |m| m.min(identity.start_ticks)));
            }
            Ok(())
        });
        // Sessions of another boot contribute no bound: everything they started is gone (min_ticks stays None
        // when no session of this boot launched anything, and then nothing is signalled).
        rules.min_start_ticks = min_ticks.unwrap_or(u64::MAX);
        // A copied / moved directory (--adopt-moved-campaign): nothing recorded is ours; no scan at all.
        let scan_allowed = min_ticks.is_some() && !self.moved;
        let mut log_lines = Vec::new();
        let killed = if scan_allowed {
            orphans::sweep(&rules, None, Duration::from_secs(30), |round, found| {
                log_lines.push(format!(
                    "recovery: orphan scan round {round}: {found} marked process(es) of this campaign"
                ))
            })
        } else {
            Vec::new()
        };
        for l in log_lines {
            self.log.line("INFO", &l);
        }
        for k in &killed {
            self.orphan_killed(&k.attempt_id, k.pid, k.start_ticks, "recovery scan");
        }
        let left = if scan_allowed {
            rules.scan(None)
        } else {
            Vec::new()
        };
        if !left.is_empty() {
            self.log.line(
                "ERROR",
                &format!(
                    "{} marked process(es) survive SIGKILL (D state?): {left:?}",
                    left.len()
                ),
            );
        }
        for (job, attempt) in unsettled {
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
            let was_killed = killed.iter().any(|k| k.attempt_id == marker);
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
        let outcome_ref = file_ref(&layout.outcome(), self.rel_path(&layout.outcome()));
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
            let p = layout.table();
            table = file_ref(&p, self.rel_path(&p));
            if let Ok(text) = std::fs::read(&p) {
                let mut d = CanonicalDigest::new();
                for l in text.split(|&b| b == b'\n').filter(|l| !l.is_empty()) {
                    if let Ok(cl) = canonical_line(l) {
                        lines.push(String::from_utf8_lossy(&cl).into_owned());
                    }
                    let _ = d.push_line(l);
                }
                canon = Some(d.hex());
            }
            if table.is_none() {
                // A sealed table that cannot be read now (deleted?): fail closed.
                self.log.line(
                    "ERROR",
                    &format!("{job} a{attempt}: sealed table unreadable; nothing accepted"),
                );
            }
        } else if wants_table
            && let Some(s) = ev.verified.as_ref().and_then(|v| {
                v.streams
                    .iter()
                    .find(|s| s.rel == RunLayout::out_rel(rustred_campaign_contract::layout::RESULTS))
            })
        {
            // lane closeloop (closeloop.md 6.4): a kind without a table whose sealed `results` stream the contract
            // verified (solve-recipe): its canonical lines come through the VerifiedHandle, never a re-read path.
            let p = layout.root.join(&s.rel);
            match rustred_campaign_contract::seal::VerifiedHandle::reopen(&layout.root, s)
                .and_then(|h| h.canonical_lines())
            {
                Ok(ls) => {
                    table = file_ref(&p, self.rel_path(&p));
                    lines = ls
                        .into_iter()
                        .map(|l| String::from_utf8_lossy(&l).into_owned())
                        .collect();
                    canon = Some(s.canonical_blake3.clone());
                }
                Err(e) => self.log.line(
                    "ERROR",
                    &format!("{job} a{attempt}: verified results stream unreadable ({e}); nothing accepted"),
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
        let routing = if sealed_ok {
            self.source.route(self.ledger.state(), &self.ctx, &settled)
        } else {
            Err("sealed table unreadable".into())
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
        let (mut ls, spec_digest) =
            adapters::prepare_run(&spec, &prog_path).map_err(|e| format!("run dir: {e}"))?;
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
        let handle = match self.launcher.launch(&ls) {
            Ok(h) => h,
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
                Err(e) => eprintln!("poll {}: {e}", ch.job),
            }
        }
        for (idx, report) in finished.into_iter().rev() {
            let ch = self.children.remove(idx);
            if self.ledger_dead.is_none() {
                self.settle(
                    &ch.job,
                    ch.attempt,
                    &ch.layout,
                    &ch.spec,
                    &ch.spec_digest,
                    map_wait(report.exit),
                    contract_kill_reason(report.kill_reason.as_ref()),
                    false,
                    Some(&report),
                );
            }
            // Leftovers the process layer already killed through its verified path (helpers that outlived the job).
            let marker = format!("{}:a{}", ch.job, ch.attempt);
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
            // Anything still carrying the attempt's marker (e.g. a helper that escaped the group before any scan).
            let rules = self.scan_rules_launched();
            let killed = orphans::sweep(&rules, Some(&marker), Duration::from_secs(10), |_, _| {});
            for k in killed {
                self.orphan_killed(
                    &marker,
                    k.pid,
                    k.start_ticks,
                    "marker sweep: contract violation, process outlived its job",
                );
            }
            if let crate::launch::Verification::Unverified { leftovers, reason } =
                &report.verification
            {
                // Process layer (procfix2, RB-C1 d): an unproven end is a ledger record; the job and its CPU lease stay
                // in the launcher's quarantine (below) until a later hunt proves it gone.
                self.note(format!(
                    "unverified_end {{\"marker\":\"{marker}\",\"left\":{},\"reason\":{:?}}} (lease kept, hunting continues)",
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
            }) = self.launcher.retire(ch.handle, ch.lease)
            {
                self.note(format!("lease {marker}: {n}"));
            }
            let _ = &ch.item;
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
                    ..
                } => self.note(format!(
                    "unverified_end_cleared {{\"marker\":\"{label}\",\"after_s\":{}}}{}",
                    after.as_secs(),
                    lease_note.map(|n| format!(" ({n})")).unwrap_or_default()
                )),
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
        files.sort();
        for p in files {
            let name = p.file_name().unwrap().to_owned();
            let done = dir.join("done");
            let _ = std::fs::create_dir_all(&done);
            let req: Option<ControlRequest> = std::fs::read(&p)
                .ok()
                .and_then(|b| serde_json::from_slice(&b).ok());
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
                self.log
                    .line("ERROR", &format!("source record {}: {e}; not recorded", a.path));
                return;
            }
        }
        if let Err(e) = self.commit_or_refused(routing.events) {
            self.log
                .line("ERROR", &format!("work source events refused by the ledger: {e}"));
        }
    }

    fn schedule(&mut self) {
        let max_jobs = self.opts.spec.resources.max_jobs;
        if self.children.len() >= max_jobs {
            return;
        }
        let limit = (max_jobs - self.children.len()) * 4 + 8;
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
            .filter(|r| !running_ids.contains(&r.job) && !paused(&r.item))
            .collect();
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
                    if self.children.len() >= max_jobs || self.ledger_dead.is_some() {
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
            reserved_bytes: Some(self.children.iter().map(|c| c.cap).sum()),
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
                    self.commit(vec![Event::Campaign {
                        ev: crate::state::CampaignEvent::Drain,
                        reason: "stop request".into(),
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
            }
            // 3. children (and the launcher's quarantine of unproven job ends)
            self.poll_children();
            self.tick_quarantine();
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
                if self.children.is_empty() || Instant::now() > d {
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
            if self.children.is_empty() {
                if level > StopLevel::Running {
                    self.commit(vec![Event::Campaign {
                        ev: crate::state::CampaignEvent::Stop,
                        reason: "drained".into(),
                    }]);
                    self.write_progress(true);
                    if self.disk_floor_hit {
                        return RunExit::DiskFloor;
                    }
                    return RunExit::Stopped {
                        code: self.stop_code(),
                    };
                }
                if self.source.finished(self.ledger.state()) {
                    if self.st().phase == Some(CampaignPhase::Running) {
                        self.commit(vec![Event::Campaign {
                            ev: crate::state::CampaignEvent::Finish,
                            reason: "all work final".into(),
                        }]);
                    }
                    self.write_progress(true);
                    return RunExit::Finished;
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
