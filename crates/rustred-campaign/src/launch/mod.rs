//! Child launching and termination: the [`ChildLauncher`] / [`ChildHandle`] traits, the real implementation
//! ([`RealLauncher`]: own session, CPU pinning, memory cap, PDEATHSIG, explicit environment, CLOEXEC sweep) and a
//! scripted in-memory fake ([`ScriptedLauncher`]) for deterministic scheduler tests.
//!
//! Termination protocol (real launcher; `interfaces/process-layer.md` has the timeline):
//! 1. `terminate(grace)`: SIGTERM + SIGCONT to every process group of the child's session; groups that a later scan
//!    names get them at once.
//! 2. At the grace deadline: SIGKILL to every group of the session (and `cgroup.kill` when the job has a cgroup).
//! 3. The leader's exit is observed with `waitid(WNOWAIT)`: its zombie stays unreaped, which pins the session number
//!    and the leader's group number, so no signal can reach a recycled number.
//! 4. The end-of-job HUNT (`hunt`, on a helper thread; second fix package): freeze every process of the job (job
//!    cgroup, pinned group, then each member found through the supervisor's children lists, through the guard), reach a
//!    fixed point, SIGKILL them all, then PROVE the job empty with two consecutive conclusive passes while the leader is
//!    still held unreaped. Leftovers killed are recorded.
//! 5. Only then is the leader reaped and the session untracked (`Verification::Verified { by }`). If the proof cannot be
//!    made within `verify_timeout`, the report says `Verification::Unverified { leftovers, reason }`, the zombie stays
//!    unreaped, and the handle (or the launcher's quarantine, [`RealLauncher::retire`]) keeps hunting.
//!
//! Every signal of this crate goes through `guard` (see its module documentation for the identity checklist).

mod cgroup;
mod fake;
pub mod guard;
mod hunt;
mod idlock;
mod real;
pub mod sweep;

pub use cgroup::{CgroupRoot, JobCgroup, kill_recorded_scope, scope_unit_name};

/// Run `f` with a signalling context: every delivery `f` makes on this thread is logged with it (gate3 M2).
pub fn guard_context<R>(by: &str, label: Option<&str>, f: impl FnOnce() -> R) -> R {
    guard::with_context(by, label, f)
}
pub use fake::{
    FakeEvent, FakeEventKind, ManualTime, MonotonicTime, OnTerm, Script, ScriptedChild,
    ScriptedLauncher, TimeSource,
};
pub(crate) use guard::reap_adopted;
pub use guard::{
    DELIVERY_LOG_MAX, Delivery, INTENT_LOG_FILE, INTENT_RESERVE_FILE, INTENT_SCHEMA, IntentLine,
    LoggedIntent, PidfdProbe, ProcessTarget, Proof, Refusal, SigchldSetup, StartRule, Verdict,
    ensure_sigchld_default, ignore_sigxfsz, install_intent_log, intent_id_in, intent_log_installed,
    is_child_subreaper, kill_orphan, pidfd_probe, pidfd_supported, read_intent_log,
    set_child_subreaper, sigchld_autoreaps, signal_verified_process, take_deliveries,
    uninstall_intent_log,
};
pub use idlock::{IdLockError, default_lock_dir};
pub use real::{
    CapPreference, LauncherError, LauncherSetup, RealChild, RealConfig, RealLauncher, RetireEvent,
    TrackingPreference, adhoc_campaign_id, audit_inheritable_fds, in_private_namespace, peek_exit,
};
pub use sweep::{LeftMarked, Narrowing, SweepReport, SweepSpec};

/// The barrier shim (procfix5 m1), run by the hidden verb [`BARRIER_VERB`] of a binary that hosts the supervisor:
/// `args` = `<fd> -- <program> <args...>`. Waits for one byte on `fd` (the supervisor writes it after recording the
/// spawn durably), closes it and execs the program in place with the current environment. EOF or anything else: exit
/// [`BARRIER_NOT_RELEASED`] without executing anything. Returns only on failure (the exit code to use).
pub fn barrier_shim(args: &[String]) -> i32 {
    use std::io::Write;
    use std::os::unix::process::CommandExt;
    let (Some(fd), Some("--"), Some(prog)) = (
        args.first().and_then(|s| s.parse::<RawFd>().ok()),
        args.get(1).map(String::as_str),
        args.get(2),
    ) else {
        let _ = writeln!(
            std::io::stderr(),
            "{BARRIER_VERB}: usage <fd> -- <program> [args]"
        );
        return 2;
    };
    if fd < 3 {
        return 2;
    }
    let mut b = [0u8; 1];
    let n = loop {
        // SAFETY: a one-byte read into a stack buffer from an inherited descriptor.
        let r = unsafe { libc::read(fd, b.as_mut_ptr().cast(), 1) };
        if r < 0 && io::Error::last_os_error().raw_os_error() == Some(libc::EINTR) {
            continue;
        }
        break r;
    };
    // SAFETY: closing the inherited barrier descriptor (the program must not inherit it).
    unsafe { libc::close(fd) };
    if n != 1 || b[0] != b'G' {
        let _ = writeln!(
            std::io::stderr(),
            "{BARRIER_VERB}: the supervisor ended before it recorded this spawn: the program was not started"
        );
        return BARRIER_NOT_RELEASED;
    }
    let e = std::process::Command::new(prog).args(&args[3..]).exec();
    let _ = writeln!(std::io::stderr(), "{BARRIER_VERB}: exec {prog}: {e}");
    127
}

use crate::resources::{CampaignMarker, EscapeeRecord, MARKER_VAR, ProcessIdentity, SessionUsage};
use serde::Serialize;
use std::ffi::OsString;
use std::fmt;
use std::io;
use std::os::fd::RawFd;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// How a child ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum ExitState {
    Exited(i32),
    Signaled {
        signal: i32,
        core_dumped: bool,
    },
    /// The status cannot be known (an adopted child of a previous supervisor; init reaped it).
    Unknown,
}

impl ExitState {
    pub fn code(&self) -> Option<i32> {
        match self {
            ExitState::Exited(c) => Some(*c),
            _ => None,
        }
    }
    pub fn signal(&self) -> Option<i32> {
        match self {
            ExitState::Signaled { signal, .. } => Some(*signal),
            _ => None,
        }
    }
}

/// Why the supervisor stopped or killed a child. `outcome()` gives the code the scheduler records.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum KillReason {
    /// Campaign stop (first Ctrl-C / SIGTERM / SIGHUP): graceful, with grace period.
    Stop,
    /// Second Ctrl-C: short grace, then SIGKILL.
    ForceStop,
    /// The job's session RSS went above its cap (sampling), or its cgroup reported an OOM kill.
    MemoryCap {
        rss_bytes: u64,
        cap_bytes: u64,
    },
    /// Chosen as a victim of the host-floor emergency (admission::choose_victims).
    HostEmergency,
    /// Wall-clock limit of the job.
    Timeout,
    /// Heartbeat missing (contract 6.2).
    Unresponsive,
    /// Supervisor restart: a recorded child of the previous supervisor, killed on request.
    Reattach,
    Other(String),
}

/// Outcome code for a per-job memory cap excess (admission and scheduler use the same string).
pub const MEMORY_CAP_EXCEEDED: &str = "memory_cap_exceeded";

impl KillReason {
    pub fn outcome(&self) -> &'static str {
        match self {
            KillReason::Stop | KillReason::ForceStop | KillReason::Reattach => "interrupted",
            KillReason::MemoryCap { .. } => MEMORY_CAP_EXCEEDED,
            KillReason::HostEmergency => "host_memory_emergency",
            KillReason::Timeout => "killed_time",
            KillReason::Unresponsive => "unresponsive",
            KillReason::Other(_) => "killed_other",
        }
    }
}

/// A process found in the child's session after its leader exited, and killed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LeftoverRecord {
    pub pid: u32,
    pub start_ticks: u64,
    pub pgrp: u32,
    pub comm: String,
}

/// Which mechanism enforced the job's memory cap (audit A M7: chosen per job, recorded in the report).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum MemoryMechanism {
    /// cgroup-v2 `memory.max` (kernel-enforced, whole tree incl. escapees), path of the job cgroup.
    Cgroup(PathBuf),
    /// A transient `systemd-run --user --scope` with MemoryMax = cap, MemorySwapMax = 0, OOMPolicy = kill
    /// (kernel-enforced); the scope's cgroup directory once seen.
    SystemdScope { cgroup: Option<PathBuf> },
    /// The supervisor samples the session's RSS and kills the session above the cap.
    Sampling,
    /// No cap requested.
    None,
}

/// How the job's processes were enumerated for the proof (second fix package).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Enumeration {
    /// The supervisor is a child subreaper (or pid 1): every live descendant is in its children tree; direct children are
    /// enumerated by the kernel's children lists AND a strict full /proc listing (ppid == us).
    SubreaperTree,
    /// A strict full /proc listing (adopted jobs of a previous supervisor; no subreaper). An entry that vanishes during a
    /// pass makes it inconclusive.
    FullProc,
}

/// What a `Verified` proof rests on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum VerifiedBy {
    /// The job's own cgroup (created by us in a DELEGATED subtree) was frozen and killed, and `cgroup.procs` was empty in
    /// two passes (kernel-maintained membership), with the /proc passes also empty.
    Cgroup,
    /// Two consecutive conclusive /proc passes, a short interval apart, with the leader held unreaped during both.
    ProcTwoPass { enumeration: Enumeration },
    /// The scripted fake launcher (tests).
    Scripted,
}

/// A direct child of the supervisor (re-parented) that is NOT proven to be any job's: no marker, not in the supervisor's
/// session, not a registered leader (e.g. a job's descendant that cleared its environment and called setsid). Never
/// signalled; reported for a human (gate re-audit B m1).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct UnattributedChild {
    pub pid: u32,
    pub start_ticks: u64,
    pub session: u32,
    pub comm: String,
}

/// How far the end of a job was verified (audit A M2: a verification never gives up silently).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum Verification {
    /// PROVEN gone (positive evidence only, see [`VerifiedBy`]): no session member and no marked process of the attempt.
    Verified { by: VerifiedBy },
    /// Not shown empty in time: what is left (as far as known) and why. Keep the job's CPU lease and memory
    /// reservation, and keep polling the handle: it keeps killing verified leftovers and may still upgrade to Verified.
    Unverified {
        leftovers: Vec<LeftoverRecord>,
        reason: String,
    },
}

impl Verification {
    pub fn is_verified(&self) -> bool {
        matches!(self, Verification::Verified { .. })
    }
}

/// Everything known about a finished child. Written to the ledger by the scheduler.
#[derive(Clone, Debug, Serialize)]
pub struct FinalReport {
    pub label: String,
    pub identity: ProcessIdentity,
    /// The campaign marker the job's processes carried.
    pub marker: CampaignMarker,
    /// None only if the leader never exited (gave up) — then `session_verified_empty` is false.
    pub exit: Option<ExitState>,
    pub kill_reason: Option<KillReason>,
    pub sigterm_sent: bool,
    pub sigkill_sent: bool,
    pub leftovers_killed: Vec<LeftoverRecord>,
    pub escaped: Vec<EscapeeRecord>,
    /// Re-parented children not proven to be any job's, first seen while this job's end was proven (never signalled).
    pub unattributed_children: Vec<UnattributedChild>,
    /// `verification == Verified` (kept for readers of the first interface).
    pub session_verified_empty: bool,
    pub verification: Verification,
    pub peak_rss_bytes: u64,
    pub cpu_seconds: f64,
    pub wall_seconds: f64,
    pub memory_mechanism: MemoryMechanism,
    pub rlimit_as_bytes: Option<u64>,
}

impl FinalReport {
    /// The outcome code when the supervisor killed the job, else None (the child's own exit decides).
    pub fn supervisor_outcome(&self) -> Option<&'static str> {
        self.kill_reason.as_ref().map(KillReason::outcome)
    }
}

/// Result of polling a child (the report is large but returned once per child).
#[derive(Clone, Debug)]
#[allow(clippy::large_enum_variant)]
pub enum ChildProgress {
    Running,
    /// Termination requested or the leader exited; waiting for grace, kills, or the empty-session check.
    Stopping,
    Finished(FinalReport),
}

/// Result of a signal request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SignalOutcome {
    /// Delivered this many times (pinned leader group + individually verified processes).
    Delivered { groups: usize },
    /// Refused: the recorded identity does not match the process now holding the pid.
    NotOurs(crate::resources::Liveness),
    /// Nothing left to signal.
    AlreadyGone,
}

/// After a supervisor restart: what to do with a recorded live child.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReattachChoice {
    /// Monitor it (exit status will be `Unknown`; usage and termination work).
    Adopt,
    /// Terminate it (escalation with this grace); the handle comes back already stopping.
    Kill { grace: Duration },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LostReason {
    /// Recorded under another boot.
    OtherBoot,
    /// No process with that pid and no member of its session.
    Gone,
    /// The pid belongs to another process now (and nothing carries the attempt's marker).
    Recycled,
    /// The recorded process lives but does not carry this attempt's marker (or another proof failed).
    NotOurs { why: String },
}

pub enum Reattached<H> {
    Lost(LostReason),
    Live(H),
}

/// Spec of one launch. Construct with [`LaunchSpec::new`], then set fields.
#[derive(Clone)]
pub struct LaunchSpec {
    /// Job label for logs and reports (no secrets).
    pub label: String,
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub cwd: PathBuf,
    pub stdout: PathBuf,
    pub stderr: PathBuf,
    /// The child's whole environment besides `env_passthrough`: the child starts from an EMPTY environment.
    pub env: Vec<(OsString, OsString)>,
    /// Names whose values are copied from the supervisor's environment at spawn time (e.g. the licence variable).
    /// Values are never stored in the spec, logged, or written to a file; a missing name fails the launch by name.
    pub env_passthrough: Vec<String>,
    /// CPUs to pin to (empty: no pinning).
    pub cpus: Vec<usize>,
    /// Memory cap of the whole session (expected peak + margin). Enforced by cgroup when `cgroup` is set and usable,
    /// else by sampling.
    pub memory_cap_bytes: Option<u64>,
    /// Optional per-process RLIMIT_AS backstop (off by default: address-space limits break mmap-heavy allocators).
    pub rlimit_as_bytes: Option<u64>,
    /// Delegated cgroup-v2 root (probed with [`CgroupRoot::probe`]); None = systemd scope or sampling.
    pub cgroup: Option<CgroupRoot>,
    /// Cap mechanism wanted (default Auto: delegated cgroup > systemd scope > sampling; see [`CapRequest`]).
    pub cap_mechanism: CapRequest,
    /// Supervisor fds the child must inherit (CPU lease flocks: [`LaunchSpec::with_lease`] adds them; the one deliberate
    /// exception to close-on-exec). Every other fd >= 3 is closed on exec.
    pub inherit_fds: Vec<RawFd>,
    /// PR_SET_PDEATHSIG(SIGKILL) with the parent check (default true).
    pub pdeathsig: bool,
    /// Lane int5 (gate3 M5): the deterministic systemd unit name (`cgroup::scope_unit_name`) of the job's transient
    /// scope, used with `systemd-run --unit=` when the systemd-scope mechanism is chosen (recorded by the supervisor in
    /// the launch record, so that a restart after a crash can find exactly this scope). None: an anonymous scope.
    pub scope_unit: Option<String>,
    /// procfix5 (gate4 m1): start the child behind a SPAWN BARRIER: the child execs this process's own binary
    /// (`/proc/self/exe`) with the hidden verb [`BARRIER_VERB`], which waits on an inherited pipe until the caller calls
    /// [`RealChild::release_barrier`] (after recording the spawn durably) and only then execs the program in place
    /// (same pid, session, start time, marker, PDEATHSIG, affinity, scope). If the caller dies first, the pipe reports
    /// EOF and the program is never executed. Only for binaries whose `main` dispatches [`BARRIER_VERB`] (the
    /// `rr_campaign` CLI does: `cli::main_with`). Default false.
    pub spawn_barrier: bool,
}

/// The hidden verb of the spawn barrier: `<exe> __spawn-barrier <fd> -- <program> <args...>` (see
/// [`LaunchSpec::spawn_barrier`]).
pub const BARRIER_VERB: &str = "__spawn-barrier";
/// Exit code of a barrier shim whose barrier was never released (the program was not executed).
pub const BARRIER_NOT_RELEASED: i32 = 125;

impl LaunchSpec {
    /// Defaults: cwd = run_dir, stdout/stderr = run_dir/stdout.log, run_dir/stderr.log, empty environment, no pinning,
    /// no cap, PDEATHSIG on.
    pub fn new(label: impl Into<String>, program: impl Into<PathBuf>, run_dir: &Path) -> Self {
        Self {
            label: label.into(),
            program: program.into(),
            args: Vec::new(),
            cwd: run_dir.to_path_buf(),
            stdout: run_dir.join("stdout.log"),
            stderr: run_dir.join("stderr.log"),
            env: Vec::new(),
            env_passthrough: Vec::new(),
            cpus: Vec::new(),
            memory_cap_bytes: None,
            rlimit_as_bytes: None,
            cgroup: None,
            cap_mechanism: CapRequest::Auto,
            inherit_fds: Vec::new(),
            pdeathsig: true,
            scope_unit: None,
            spawn_barrier: false,
        }
    }

    /// Pin to the lease's CPUs AND let the child inherit the lease's flock descriptors (the DEFAULT way to use a lease;
    /// audit A M6): the CPUs stay locked while any process of the job lives, even if the supervisor dies.
    pub fn with_lease(mut self, lease: &crate::cpu_policy::CpuLease) -> Self {
        self.cpus = lease.cpus().to_vec();
        for fd in lease.lock_fds() {
            if !self.inherit_fds.contains(&fd) {
                self.inherit_fds.push(fd);
            }
        }
        self
    }

    pub fn arg(mut self, a: impl Into<OsString>) -> Self {
        self.args.push(a.into());
        self
    }

    pub fn env(mut self, k: impl Into<OsString>, v: impl Into<OsString>) -> Self {
        self.env.push((k.into(), v.into()));
        self
    }

    /// The minimal environment of child-job contract 6.1 without the licence: PATH and HOME copied by value from the
    /// supervisor, LANG=C, one thread for Rayon/OpenMP, TMPDIR. The licence goes through `env_passthrough` by name.
    pub fn with_contract_env(mut self, tmpdir: &Path) -> Self {
        for k in ["PATH", "HOME"] {
            if let Some(v) = std::env::var_os(k) {
                self.env.push((k.into(), v));
            }
        }
        self.env.push(("LANG".into(), "C".into()));
        self.env.push(("RAYON_NUM_THREADS".into(), "1".into()));
        self.env.push(("OMP_NUM_THREADS".into(), "1".into()));
        self.env.push(("SYMBOLICA_HIDE_BANNER".into(), "1".into()));
        self.env.push(("TMPDIR".into(), tmpdir.into()));
        self
    }

    pub fn validate(&self) -> Result<(), LaunchError> {
        if self.label.is_empty() {
            return Err(LaunchError::InvalidSpec("empty label".into()));
        }
        if self.label.contains('\0') || self.label.len() > 256 {
            return Err(LaunchError::InvalidSpec(
                "label must be 1-256 bytes without NUL (it is the marker's attempt id)".into(),
            ));
        }
        for (i, name) in self.env_passthrough.iter().enumerate() {
            // Audit B m8: a name is validated as a NAME and never echoed (a misuse like "NAME=value" would leak).
            if !is_env_name(name) {
                return Err(LaunchError::InvalidSpec(format!(
                    "passthrough entry #{i} is not a variable name ([A-Za-z_][A-Za-z0-9_]*)"
                )));
            }
            if name == MARKER_VAR {
                return Err(LaunchError::InvalidSpec(format!(
                    "{MARKER_VAR} is set by the launcher"
                )));
            }
            if self.env.iter().any(|(k, _)| k == name.as_str()) {
                return Err(LaunchError::InvalidSpec(format!(
                    "variable {name} given both by value and by name"
                )));
            }
        }
        for (i, (k, _)) in self.env.iter().enumerate() {
            // Gate re-audit B m4: a key must be a NAME (a misuse "NAME=value" is refused and never echoed).
            let ok = k.to_str().is_some_and(is_env_name);
            if !ok {
                return Err(LaunchError::InvalidSpec(format!(
                    "env entry #{i} has an invalid name"
                )));
            }
            if k == MARKER_VAR {
                return Err(LaunchError::InvalidSpec(format!(
                    "{MARKER_VAR} is set by the launcher"
                )));
            }
        }
        if let Some(c) = self.cpus.iter().find(|&&c| c >= libc::CPU_SETSIZE as usize) {
            return Err(LaunchError::InvalidSpec(format!(
                "CPU {c} beyond CPU_SETSIZE"
            )));
        }
        if self.inherit_fds.iter().any(|&fd| fd < 3) {
            return Err(LaunchError::InvalidSpec("inherit_fds must be >= 3".into()));
        }
        Ok(())
    }
}

/// `[A-Za-z_][A-Za-z0-9_]*`.
pub fn is_env_name(s: &str) -> bool {
    let mut b = s.bytes();
    matches!(b.next(), Some(c) if c.is_ascii_alphabetic() || c == b'_')
        && b.all(|c| c.is_ascii_alphanumeric() || c == b'_')
}

/// Which memory-cap mechanism a spec asks for (only meaningful with `memory_cap_bytes`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CapRequest {
    /// Delegated cgroup (spec.cgroup) > systemd scope (if the launcher's probe worked) > sampling.
    #[default]
    Auto,
    /// The sampled cap only (residual risk: overshoot between samples, see process-layer.md section 6).
    Sampling,
    /// A systemd scope (or the delegated cgroup if given); the launch fails if neither is available.
    SystemdScope,
}

/// The latest usage with its freshness (audit A M2).
#[derive(Clone, Debug)]
pub enum UsageReading {
    Fresh {
        usage: SessionUsage,
        age: std::time::Duration,
    },
    /// The sampler has not reported for longer than the threshold: "cannot verify memory". Admission must charge the
    /// job its full cap; a sampled cap cannot be enforced meanwhile (kernel caps still are).
    Stale {
        last: Option<SessionUsage>,
        age: Option<std::time::Duration>,
    },
}

impl UsageReading {
    pub fn is_fresh(&self) -> bool {
        matches!(self, UsageReading::Fresh { .. })
    }
}

/// Debug never prints environment values: passthrough values are not stored, `env` values are never printed, and a key
/// or passthrough entry that is not a valid name (a misuse like "NAME=value") is printed as `<not a name: hidden>`.
impl fmt::Debug for LaunchSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LaunchSpec")
            .field("label", &self.label)
            .field("program", &self.program)
            .field("args", &self.args)
            .field("cwd", &self.cwd)
            .field(
                "env_keys",
                &self
                    .env
                    .iter()
                    .map(|(k, _)| match k.to_str() {
                        Some(n) if is_env_name(n) => n,
                        _ => "<not a name: hidden>",
                    })
                    .collect::<Vec<_>>(),
            )
            .field(
                "env_passthrough_names",
                &self
                    .env_passthrough
                    .iter()
                    .map(|n| {
                        if is_env_name(n) {
                            n.as_str()
                        } else {
                            "<not a name: hidden>"
                        }
                    })
                    .collect::<Vec<_>>(),
            )
            .field("scope_unit", &self.scope_unit)
            .field("cpus", &self.cpus)
            .field("memory_cap_bytes", &self.memory_cap_bytes)
            .field("rlimit_as_bytes", &self.rlimit_as_bytes)
            .field("cgroup", &self.cgroup)
            .field("cap_mechanism", &self.cap_mechanism)
            .field("inherit_fds", &self.inherit_fds)
            .field("pdeathsig", &self.pdeathsig)
            .field("scope_unit", &self.scope_unit)
            .field("spawn_barrier", &self.spawn_barrier)
            .finish()
    }
}

#[derive(Debug)]
pub enum LaunchError {
    InvalidSpec(String),
    /// A passthrough variable is not set in the supervisor's environment (the NAME, never a value).
    MissingEnv(String),
    Io {
        what: String,
        error: io::Error,
    },
    Spawn(io::Error),
    /// The child started but its identity could not be read; it was killed.
    Identity(io::Error),
    Cgroup(String),
    /// Scripted failure of the fake launcher.
    Scripted(String),
}

impl fmt::Display for LaunchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LaunchError::InvalidSpec(s) => write!(f, "invalid launch spec: {s}"),
            LaunchError::MissingEnv(n) => write!(f, "environment variable {n} is not set"),
            LaunchError::Io { what, error } => write!(f, "{what}: {error}"),
            LaunchError::Spawn(e) => write!(f, "spawn failed: {e}"),
            LaunchError::Identity(e) => write!(f, "child identity unreadable (child killed): {e}"),
            LaunchError::Cgroup(s) => write!(f, "cgroup: {s}"),
            LaunchError::Scripted(s) => write!(f, "scripted launch failure: {s}"),
        }
    }
}

impl std::error::Error for LaunchError {}

/// One running child. All methods are non-blocking.
pub trait ChildHandle {
    fn label(&self) -> &str;
    fn identity(&self) -> &ProcessIdentity;
    /// Has the leader exited? Does not reap (the zombie pins the session number until [`Self::poll`] finishes).
    fn poll_exit(&mut self) -> io::Result<Option<ExitState>>;
    /// Latest sampled usage of the session (None before the first scan that saw it).
    fn usage(&self) -> Option<SessionUsage>;
    /// The latest usage with its freshness (default: `usage()` as fresh, for handles without a sampler).
    fn usage_reading(&self) -> UsageReading {
        match self.usage() {
            Some(u) => UsageReading::Fresh {
                usage: u,
                age: Duration::ZERO,
            },
            None => UsageReading::Stale {
                last: None,
                age: None,
            },
        }
    }
    /// Signal every process group of the session, after the identity rule.
    fn signal_group(&mut self, signal: i32) -> io::Result<SignalOutcome>;
    /// Start the escalation: SIGTERM now, SIGKILL after `grace` (a second call can only shorten the grace).
    fn terminate(&mut self, grace: Duration, reason: KillReason);
    /// SIGKILL now (memory cap, force stop).
    fn kill_now(&mut self, reason: KillReason);
    /// Drive everything: observe exit, enforce the memory cap, escalate, verify the session empty, reap.
    fn poll(&mut self) -> io::Result<ChildProgress>;
}

/// Starts children. The scheduler is generic over it; tests use [`ScriptedLauncher`].
pub trait ChildLauncher {
    type Handle: ChildHandle;
    fn launch(&mut self, spec: &LaunchSpec) -> Result<Self::Handle, LaunchError>;
    /// After a supervisor restart: find a recorded child, then adopt or kill it as the caller chooses.
    fn reattach(
        &mut self,
        recorded: &ProcessIdentity,
        label: &str,
        choice: ReattachChoice,
    ) -> io::Result<Reattached<Self::Handle>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_validation_and_debug_hide_values() {
        let s = LaunchSpec::new("j1", "/bin/true", Path::new("/tmp")).env("A", "secret-value-123");
        let mut bad = s.clone();
        bad.env_passthrough.push("A".into());
        assert!(matches!(bad.validate(), Err(LaunchError::InvalidSpec(_))));
        let mut ok = s.clone();
        ok.env_passthrough.push("SYMBOLICA_LICENSE".into());
        ok.validate().unwrap();
        let dbg = format!("{ok:?}");
        assert!(!dbg.contains("secret-value-123"));
        assert!(dbg.contains("SYMBOLICA_LICENSE"));
        let mut fd = s.clone();
        fd.inherit_fds.push(2);
        assert!(fd.validate().is_err());
        // Audit B m8: a misused passthrough entry is refused WITHOUT echoing it, also not in Debug.
        let mut leak = s.clone();
        leak.env_passthrough
            .push("SYMBOLICA_LICENSE=dummy-secret-77".into());
        let e = leak.validate().unwrap_err().to_string();
        assert!(!e.contains("dummy-secret-77"), "{e}");
        assert!(!format!("{leak:?}").contains("dummy-secret-77"));
        let mut m = s;
        m.env.push((MARKER_VAR.into(), "x:y".into()));
        assert!(m.validate().is_err());
    }

    #[test]
    fn outcome_codes() {
        assert_eq!(
            KillReason::MemoryCap {
                rss_bytes: 2,
                cap_bytes: 1
            }
            .outcome(),
            MEMORY_CAP_EXCEEDED
        );
        assert_eq!(KillReason::Stop.outcome(), "interrupted");
    }
}
