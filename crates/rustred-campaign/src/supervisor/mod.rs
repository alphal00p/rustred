//! The supervisor run loop (WP-I6; interfaces/supervisor.md). One control thread owns the ledger (the only truth), the
//! children and all state; the sampler is the only helper thread. Start and resume are the same entry point [`run`].
//!
//! Write-ahead order (section 4 of the interface): the run directory and `job.json` exist before the `job.launch`
//! record; the child is spawned after it; `job.started` names its identity; at the end the classification is computed
//! from the run directory, accepted record files are written atomically, and ONE group commit records `job.finished`
//! together with the work source's routing events. Every crash point is recovered by [`Supervisor::recover`].

pub mod failpoint;
pub mod orphans;
pub mod result;
pub mod spec;
pub mod worksource;

use crate::adapters::{self, AdapterInput, AdapterKind};
use crate::admission::{self, AdmissionPolicy, Candidate, Decision, HostView, RunningJob};
use crate::clock::{Clock, SystemClock};
use crate::cpu_policy::{Acquire, CpuLease, CpuPolicy, CpuSpec, PlainPolicy, SiteConfig, SitePolicy};
use crate::launch::{
    ChildHandle, ChildLauncher, ChildProgress, ExitState, FinalReport, KillReason, LaunchError,
    RealChild, RealLauncher,
};
use crate::ledger::{Event, FileRef, JobEvent, Ledger, LedgerConfig, LedgerError};
use crate::lock::{self, CampaignLock, LockOutcome};
use crate::progress::{ProgressWriter, Resources as ProgressResources};
use crate::resources::{Sampler, SamplerConfig, host_memory};
use crate::signals::{StopHandle, StopLevel, StopSignals};
use crate::state::{AttemptState, AttemptTarget, CampaignPhase, CampaignState, FinishClass, class_target, class_word};
use failpoint::Failpoints;
use orphans::{MARKER_ENV, ScanRules, marker_value};
use rustred_campaign_contract::canon::{CanonicalDigest, canonical_line};
use rustred_campaign_contract::classify::{self, KillReason as CKill, WaitStatus};
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

/// How a `run` ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunExit {
    Finished,
    Stopped { code: i32 },
    DiskFloor,
    DiskFull,
    BinaryChanged(String),
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
    pub cores: Option<usize>,
    pub mem_total: Option<u64>,
    pub argv: Vec<String>,
    /// Tick of the control loop.
    pub tick: Duration,
}

/// Plain log: `<utc> <LEVEL> <text>` on stderr and in CDIR/log/supervisor.log.
pub struct Log {
    file: Option<std::fs::File>,
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
        Log { file }
    }
    pub fn line(&mut self, level: &str, text: &str) {
        let l = format!("{} {level} {text}\n", crate::cpu_policy::utc_now());
        let _ = std::io::stderr().write_all(l.as_bytes());
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
    boot_id: String,
    started_at: Instant,
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

/// The campaign key of the marker: `<id>@<12 hex>` from the spec digest and the creation time (unique per CDIR).
pub fn campaign_key(st: &CampaignState) -> String {
    let h = blake3::hash(format!("{}|{}", st.spec_blake3, st.created_utc).as_bytes()).to_hex();
    format!("{}@{}", st.campaign_id, &h[..12])
}

fn map_kill(k: Option<&KillReason>) -> CKill {
    match k {
        None => CKill::None,
        Some(KillReason::Stop | KillReason::ForceStop | KillReason::Reattach | KillReason::HostEmergency) => {
            CKill::Stop
        }
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

/// Start or resume a campaign (the SAME command).
pub fn run(opts: RunOptions) -> RunExit {
    let mut log = Log::open(&opts.cdir);
    match Supervisor::start(opts, &mut log) {
        Ok(mut s) => {
            s.log = log;
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
        std::fs::create_dir_all(&cdir).map_err(|e| RunExit::Refused(format!("create {}: {e}", cdir.display())))?;
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
        let mut ledger = if manifest.exists() {
            let (l, rep) = match fp.ledger_opener() {
                Some(o) => Ledger::open_with(&cdir, Arc::clone(&clock), cfg, o),
                None => Ledger::open(&cdir, Arc::clone(&clock), cfg),
            }
            .map_err(|e| RunExit::Refused(format!("ledger: {e}")))?;
            if let Some(r) = rep.recovery {
                log.line(
                    "WARN",
                    &format!("ledger torn tail repaired: {} bytes after seq {}", r.truncated_bytes, r.last_good_seq),
                );
            }
            l
        } else {
            let r = match fp.ledger_opener() {
                Some(o) => Ledger::create_with(
                    &cdir,
                    Arc::clone(&clock),
                    cfg,
                    o,
                    &opts.spec.campaign,
                    &opts.spec_blake3,
                    100,
                ),
                None => Ledger::create(
                    &cdir,
                    Arc::clone(&clock),
                    cfg,
                    &opts.spec.campaign,
                    &opts.spec_blake3,
                    100,
                ),
            };
            r.map_err(|e| match e {
                LedgerError::NoSpace { .. } => RunExit::DiskFull,
                e => RunExit::Refused(format!("ledger: {e}")),
            })?
        };
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
        let key = campaign_key(ledger.state());
        let sampler = Arc::new(
            Sampler::spawn(SamplerConfig {
                interval: Duration::from_millis(opts.spec.resources.sampler_ms.max(100)),
                collect_foreign_pinning: false,
            })
            .map_err(|e| RunExit::Internal(format!("sampler: {e}")))?,
        );
        let launcher = RealLauncher::new(Arc::clone(&sampler));
        let mut pause_files = opts.spec.resources.pause_files.clone();
        pause_files.push(cdir.join("control").join("PAUSED"));
        let policy: Box<dyn CpuPolicy> = match opts.spec.resources.policy.as_str() {
            "site" => {
                let path = opts.spec.resources.site_policy.clone().unwrap_or_default();
                let mut cfg = SiteConfig::read(&path).map_err(|e| RunExit::Refused(e.to_string()))?;
                if let Some(c) = opts.cores.or(opts.spec.resources.max_cores) {
                    cfg.max_cores = c;
                }
                let mut p = SitePolicy::new(cfg).map_err(|e| RunExit::Refused(e.to_string()))?;
                for f in pause_files {
                    p.add_pause_file(f);
                }
                match p.reconcile_own_claims() {
                    Ok(v) if !v.is_empty() => log.line(
                        "INFO",
                        &format!("CPU ledger: released {} stale own claim(s): {v:?}", v.len()),
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
        admission.campaign_mem_cap_bytes = Some(opts.mem_total.unwrap_or(opts.spec.resources.mem_total.0));
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
            binaries: programs.keys().map(|n| (n.clone(), format!("bin:{n}"))).collect(),
            campaign_key: key,
        };
        let boot_id = crate::resources::boot_id().unwrap_or_default();
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
            policy,
            admission,
            signals,
            children: Vec::new(),
            progress: ProgressWriter::new(),
            last_progress: None,
            fp,
            log: Log { file: None },
            bypassed: BTreeMap::new(),
            launch_seq: 0,
            ledger_dead: None,
            control_stop: false,
            disk_floor_hit: false,
            last_disk_check: None,
            last_block_log: String::new(),
            boot_id,
            started_at: Instant::now(),
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

    fn note(&mut self, text: String) {
        self.log.line("INFO", &text);
        self.commit(vec![Event::Note { text }]);
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
        let mut roots = vec![orphans::canonical_root(&self.cdir), orphans::canonical_root(&self.work_root)];
        roots.dedup();
        ScanRules {
            campaign_key: self.ctx.campaign_key.clone(),
            recorded_boot_id: Some(self.boot_id.clone()),
            min_start_ticks: 0,
            leaders: BTreeMap::new(),
            roots,
        }
    }

    // ------------------------------------------------------------------------------------------------ recovery

    /// Recovery at start (interface section 4): orphan scan by marker, then every unsettled attempt is settled.
    fn recover(&mut self) {
        let unsettled: Vec<(String, u32, Option<u32>, Option<u64>, Option<String>)> = self
            .st()
            .unsettled_jobs()
            .into_iter()
            .map(|(id, j)| (id.clone(), j.attempt, j.pid, j.start_ticks, j.boot_id.clone()))
            .collect();
        // The boot and start bounds of the previous sessions, from the ledger itself.
        let mut rules = self.scan_rules();
        let mut min_ticks: Option<u64> = None;
        let mut last_boot: Option<String> = None;
        if let Ok(recs) = crate::ledger::read_records(&self.cdir) {
            for r in &recs {
                if let Event::SupervisorStarted { identity, .. } = &r.event {
                    last_boot = Some(identity.boot_id.clone());
                    if identity.boot_id == self.boot_id {
                        min_ticks = Some(min_ticks.map_or(identity.start_ticks, |m| m.min(identity.start_ticks)));
                    }
                }
            }
        }
        // The previous session (the one before ours) ran on another boot: everything it started is gone.
        let _ = last_boot;
        rules.min_start_ticks = min_ticks.unwrap_or(u64::MAX);
        for (_, _, pid, st, boot) in &unsettled {
            if let (Some(p), Some(t), Some(b)) = (pid, st, boot)
                && *b == self.boot_id
            {
                rules.leaders.insert(*p, *t);
            }
        }
        let killed = if min_ticks.is_some() {
            orphans::sweep(&rules, None, Duration::from_secs(30))
        } else {
            Vec::new()
        };
        for k in &killed {
            self.note(format!(
                "orphan_killed {{\"marker\":\"{}\",\"pid\":{},\"start_ticks\":{}}}",
                k.job_attempt, k.pid, k.start_ticks
            ));
        }
        let left = if min_ticks.is_some() { rules.scan(None) } else { Vec::new() };
        if !left.is_empty() {
            self.log.line(
                "ERROR",
                &format!("{} marked process(es) survive SIGKILL (D state?): {left:?}", left.len()),
            );
        }
        for (job, attempt, _, _, _) in unsettled {
            let marker = format!("{job}:a{attempt}");
            let was_killed = killed.iter().any(|k| k.job_attempt == marker);
            let Some(run_dir) = self.st().jobs.get(&job).and_then(|j| j.run_dir.clone()) else {
                continue;
            };
            let layout = RunLayout::new(&run_dir);
            match read_spec(&layout) {
                Some(spec) => {
                    let kill = if was_killed { CKill::Stop } else { CKill::None };
                    self.settle(&job, attempt, &layout, &spec, WaitStatus::Unknown, kill, was_killed, None);
                }
                None => {
                    let ev = vec![
                        Event::Job {
                            job: job.clone(),
                            attempt,
                            ev: JobEvent::Lose {
                                reason: "run directory without job.json at recovery".into(),
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
            if self.st().inputs.get(&i.id).is_some_and(|x| x.blake3 == i.blake3) {
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
                &format!("binary changed: {}; dependent rows are marked stale and redone", changed.join(", ")),
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
        wait: WaitStatus,
        kill: CKill,
        killed_at_recovery: bool,
        report: Option<&FinalReport>,
    ) {
        let ev = classify::assess(layout, spec, wait, kill, false);
        let c = classify::classify(&ev);
        let target = class_target(c.kind);
        let Some(jr) = self.st().jobs.get(job).cloned() else {
            return;
        };
        if jr.attempt != attempt || jr.state.is_settled() {
            return;
        }
        let (cpu_ms, wall_ms, peak) = report.map_or((0, 0, 0), |r| {
            ((r.cpu_seconds * 1000.0) as u64, (r.wall_seconds * 1000.0) as u64, r.peak_rss_bytes)
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
                events.push(finished_event(job, attempt, c.kind, &ev, None, outcome_ref, cpu_ms, wall_ms, peak, c.time_decided));
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
                &format!("{job} a{attempt}: {} (row {}): requeued", class_word(c.kind), c.row),
            );
            self.commit(events);
            return;
        }
        let mut table = None;
        let mut lines = Vec::new();
        let mut canon = None;
        if target == AttemptTarget::SucceededSealed && spec.kind.produces_table() {
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
                self.log.line("ERROR", &format!("{job} a{attempt}: sealed table unreadable; nothing accepted"));
            }
        }
        let sealed_ok = target != AttemptTarget::SucceededSealed || !spec.kind.produces_table() || table.is_some();
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
                self.log.line("ERROR", &format!("{job} a{attempt}: routing refused: {e}"));
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
                self.log.line("ERROR", &format!("accepted record {}: {e}; not recorded", a.path));
                return;
            }
        }
        let class = if sealed_ok { c.kind } else { FinishClass::Crashed };
        events.push(finished_event(
            job,
            attempt,
            class,
            &ev,
            if class_target(class) == AttemptTarget::SucceededSealed { table } else { None },
            outcome_ref,
            cpu_ms,
            wall_ms,
            peak,
            c.time_decided,
        ));
        events.extend(routing.events);
        self.fp.hit("before_settle_commit");
        self.log.line(
            "INFO",
            &format!("{job} a{attempt}: {} (row {}){}", class_word(class), c.row, if routing.accepts.is_empty() { "" } else { ", accepted" }),
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
            let aside = run_dir.with_file_name(format!("a{attempt}.unrecorded-{}", self.clock.now_utc().0));
            std::fs::rename(&run_dir, &aside).map_err(|e| format!("move aside {}: {e}", run_dir.display()))?;
        }
        let resume = if attempt > 1 {
            resume_from(&attempt_dir(&self.work_root.clone(), &req.job, attempt - 1))
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
        let mut ls = adapters::prepare_run(&spec, &prog_path).map_err(|e| format!("run dir: {e}"))?;
        ls.env.push((
            MARKER_ENV.into(),
            marker_value(&self.ctx.campaign_key, &req.job, attempt).into(),
        ));
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
                        exit_code: None,
                        signal: None,
                        sealed_output: None,
                        outcome: None,
                        cpu_ms: 0,
                        wall_ms: 0,
                        peak_rss_bytes: 0,
                        time_decided: false,
                    },
                }];
                self.commit(ev);
                if fatal {
                    self.log.line("ERROR", &format!("launch of {} failed: {e}; draining", req.job));
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
        self.children.push(Child {
            job: req.job.clone(),
            attempt,
            item: req.item.clone(),
            handle,
            lease: Some(lease),
            layout: RunLayout::new(&run_dir),
            spec,
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
        for (idx, ch) in self.children.iter_mut().enumerate() {
            if let Ok(items) = ch.tail.poll(&ch.layout.events()) {
                for it in items {
                    if let TailItem::Event(e) = it {
                        ch.last_event = Instant::now();
                        if matches!(e.event, ChildEvent::JobStarted { .. }) {
                            ch.started_seen = true;
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
                    map_wait(report.exit),
                    map_kill(report.kill_reason.as_ref()),
                    false,
                    Some(&report),
                );
            }
            // Leftovers carrying the attempt's marker (e.g. a helper that escaped the group).
            let rules = self.scan_rules_launched();
            let marker = format!("{}:a{}", ch.job, ch.attempt);
            let killed = orphans::sweep(&rules, Some(&marker), Duration::from_secs(10));
            for k in killed {
                self.note(format!(
                    "orphan_killed {{\"marker\":\"{marker}\",\"pid\":{},\"start_ticks\":{}}} (contract violation: process outlived its job)",
                    k.pid, k.start_ticks
                ));
            }
            if !report.session_verified_empty {
                self.log.line(
                    "WARN",
                    &format!("{}: session not verified empty ({} escaped)", ch.job, report.escaped.len()),
                );
            }
            if let Some(l) = ch.lease {
                let _ = l.release();
            }
            let _ = &ch.item;
        }
    }

    fn scan_rules_launched(&self) -> ScanRules {
        let mut r = self.scan_rules();
        r.min_start_ticks = orphans::own_start_ticks();
        r
    }

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
            let req: Option<ControlRequest> = std::fs::read(&p).ok().and_then(|b| serde_json::from_slice(&b).ok());
            let text = match req {
                None => format!("control_refused {{\"file\":{:?},\"reason\":\"unparsable\"}}", name),
                Some(r) => {
                    let res: Result<(), String> = match r.verb.as_str() {
                        "pause" => std::fs::write(dir.join("PAUSED"), &r.id).map_err(|e| e.to_string()),
                        "resume" => match std::fs::remove_file(dir.join("PAUSED")) {
                            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
                            _ => Ok(()),
                        },
                        "stop" => {
                            if self.signals.handle.level() == StopLevel::Running {
                                self.control_stop = true;
                                self.signals.handle.request_stop();
                            }
                            Ok(())
                        }
                        "retry" | "skip" => {
                            let target = r.target.clone().unwrap_or_default();
                            match self.source.control(self.ledger.state(), &r.verb, &target) {
                                Ok(ev) => {
                                    if self.commit(ev) { Ok(()) } else { Err("ledger refused".into()) }
                                }
                                Err(e) => Err(e),
                            }
                        }
                        v => Err(format!("unknown verb {v}")),
                    };
                    match res {
                        Ok(()) => format!(
                            "control_applied {{\"id\":{:?},\"verb\":{:?},\"target\":{:?},\"by\":{:?}}}",
                            r.id, r.verb, r.target, r.by
                        ),
                        Err(e) => format!("control_refused {{\"id\":{:?},\"verb\":{:?},\"reason\":{:?}}}", r.id, r.verb, e),
                    }
                }
            };
            self.note(text);
            let _ = std::fs::rename(&p, done.join(&name));
        }
    }

    fn check_disk(&mut self) {
        if self.last_disk_check.is_some_and(|t| t.elapsed() < Duration::from_secs(5)) {
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
            b.reasons.push("process sampler stale (> 30 s): no launch".into());
        }
        if self.ledger_dead.is_some() {
            b.reasons.push("ledger unavailable".into());
        }
        let joined = b.reasons.join("; ");
        if joined != self.last_block_log {
            if !joined.is_empty() {
                self.log.line("INFO", &format!("launching paused: {joined}"));
            } else if !self.last_block_log.is_empty() {
                self.log.line("INFO", "launching resumed");
            }
            self.last_block_log = joined;
        }
        b
    }

    fn schedule(&mut self) {
        let max_jobs = self.opts.spec.resources.max_jobs;
        if self.children.len() >= max_jobs {
            return;
        }
        let limit = (max_jobs - self.children.len()) * 4 + 8;
        let running_ids: Vec<String> = self.children.iter().map(|c| c.job.clone()).collect();
        let reqs: Vec<JobRequest> = self
            .source
            .ready(self.ledger.state(), &self.ctx, limit)
            .into_iter()
            .filter(|r| !running_ids.contains(&r.job))
            .collect();
        if reqs.is_empty() {
            return;
        }
        let fresh = self.sampler_fresh();
        let running: Vec<RunningJob> = self
            .children
            .iter()
            .map(|c| RunningJob {
                id: u64::MAX - c.seq,
                cap_bytes: c.cap,
                expected_peak_bytes: c.peak.min(c.cap),
                rss_bytes: if fresh { c.handle.usage().map_or(0, |u| u.rss_bytes) } else { 0 },
                cores: c.cores,
                started_seq: c.seq,
                cpu_seconds: 0.0,
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
        let host = HostView {
            mem_available_bytes: host_memory().map(|m| m.effective_available()).unwrap_or(0),
        };
        let out = admission::decide_batch(&self.admission, host, &running, &cands);
        for b in &out.bypassed {
            *self.bypassed.entry(reqs[*b as usize].job.clone()).or_default() += 1;
        }
        let mut first_wait = None;
        for (id, d) in &out.decisions {
            let r = &reqs[*id as usize];
            match d {
                Decision::Admit => {
                    if self.children.len() >= max_jobs || self.ledger_dead.is_some() {
                        break;
                    }
                    match self.policy.acquire(r.cores, &format!("{} {}", self.ctx.campaign_key, r.job)) {
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
        if let Some(w) = first_wait {
            if w != self.last_block_log {
                self.log.line("INFO", &format!("admission waits: {w}"));
                self.last_block_log = w;
            }
        } else if self.last_block_log.starts_with("HostFloor") || self.last_block_log.starts_with("CampaignMemCap") {
            self.last_block_log.clear();
        }
    }

    fn write_progress(&mut self, force: bool) {
        let seq = self.ledger.seq();
        let due = match self.last_progress {
            None => true,
            Some((t, s)) => {
                force || (s != seq && t.elapsed() >= Duration::from_secs(self.opts.spec.progress.cadence_seconds))
            }
        };
        if !due {
            return;
        }
        let mem = host_memory().ok();
        let res = ProgressResources {
            cpus_in_use: self.policy.in_use() as u32,
            cpus_allowed: self.policy.budget() as u32,
            live_jobs: self.children.len() as u32,
            reserved_bytes: self.children.iter().map(|c| c.cap).sum(),
            measured_rss_bytes: self.children.iter().map(|c| c.handle.usage().map_or(0, |u| u.rss_bytes)).sum(),
            mem_cap_bytes: self.admission.campaign_mem_cap_bytes.unwrap_or(0),
            host_mem_available_bytes: mem.map_or(0, |m| m.effective_available()),
            host_floor_bytes: self.admission.floor_bytes,
            sampled_utc: Some(self.clock.now_utc().to_iso()),
        };
        self.fp.hit("progress_write");
        match self.progress.write(&self.cdir, self.ledger.state(), self.ledger.head(), &res, self.clock.as_ref()) {
            Ok(_) => self.last_progress = Some((Instant::now(), seq)),
            Err(e) => self.log.line("WARN", &format!("progress files: {e}")),
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
                    self.log.line("INFO", "second stop request: killing every job's group now");
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
                self.log.line("INFO", "third stop request: exiting now (children killed; resume handles the rest)");
                return RunExit::Stopped { code: self.stop_code() };
            }
            // 2. controls, disk
            if self.ledger_dead.is_none() {
                self.apply_controls();
                self.check_disk();
            }
            // 3. children
            self.poll_children();
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
                    return RunExit::Stopped { code: self.stop_code() };
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
        let _ = self.ledger.sync();
        let msg = match r {
            RunExit::Finished => format!(
                "FINISHED: {} accepted, {} terminal(s)",
                self.st().rules.accepted,
                self.st().terminals_total
            ),
            RunExit::Stopped { code } => format!("STOPPED (exit {code}); resume with the same command"),
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
}

pub const CONTROL_SCHEMA: &str = "rustred.campaign.control.v1";

fn read_spec(layout: &RunLayout) -> Option<JobSpec> {
    let b = std::fs::read(layout.job_spec()).ok()?;
    JobSpec::from_json(&b).ok()
}

/// RESUME of a previous attempt directory, when valid (child-job 3.4.4 item 3).
fn resume_from(prev: &Path) -> Option<ResumeRef> {
    let layout = RunLayout::new(prev);
    let spec = read_spec(&layout)?;
    let r = classify::resume_state(&layout, &spec).ok()?;
    let (_, b3) = crate::atomic::blake3_file(&layout.resume()).ok()?;
    Some(ResumeRef {
        from: layout.resume().to_string_lossy().into_owned(),
        blake3: b3,
        table: layout.table().to_string_lossy().into_owned(),
        committed_bytes: r.table_committed_bytes,
        prefix_blake3: r.table_prefix_blake3,
    })
}

fn file_ref(p: &Path, rel: String) -> Option<FileRef> {
    let (bytes, blake3) = crate::atomic::blake3_file(p).ok()?;
    Some(FileRef { path: rel, bytes, blake3 })
}

#[allow(clippy::too_many_arguments)]
fn finished_event(
    job: &str,
    attempt: u32,
    class: FinishClass,
    ev: &classify::Evidence,
    sealed: Option<FileRef>,
    outcome: Option<FileRef>,
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
            exit_code: code,
            signal,
            sealed_output: sealed,
            outcome,
            cpu_ms,
            wall_ms,
            peak_rss_bytes: peak,
            time_decided,
        },
    }
}
