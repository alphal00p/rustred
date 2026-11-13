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
//! 4. A scan that BEGAN after the exit (and after the last SIGKILL) must show the session empty; leftovers are killed
//!    (SIGKILL by group and by pid, each checked with `getsid`) and recorded; escapees (left by setsid) are reported as
//!    `escaped` and never chased.
//! 5. Only then is the leader reaped and the session untracked. If the session does not empty within
//!    `verify_timeout`, the report says `session_verified_empty: false` and lists what is left.

mod cgroup;
mod fake;
mod real;

pub use cgroup::{CgroupRoot, JobCgroup};
pub use fake::{
    FakeEvent, FakeEventKind, ManualTime, MonotonicTime, OnTerm, Script, ScriptedChild,
    ScriptedLauncher, TimeSource,
};
pub use real::{
    RealChild, RealConfig, RealLauncher, audit_inheritable_fds, peek_exit, signal_group_by_identity,
};

use crate::resources::{EscapeeRecord, ProcessIdentity, SessionUsage};
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

/// Which mechanism enforced the job's memory cap.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum MemoryMechanism {
    /// cgroup-v2 `memory.max` (kernel-enforced, whole tree incl. escapees), path of the job cgroup.
    Cgroup(PathBuf),
    /// The supervisor samples the session's RSS and kills the session above the cap.
    Sampling,
    /// No cap requested.
    None,
}

/// Everything known about a finished child. Written to the ledger by the scheduler.
#[derive(Clone, Debug, Serialize)]
pub struct FinalReport {
    pub label: String,
    pub identity: ProcessIdentity,
    /// None only if the leader never exited (gave up) — then `session_verified_empty` is false.
    pub exit: Option<ExitState>,
    pub kill_reason: Option<KillReason>,
    pub sigterm_sent: bool,
    pub sigkill_sent: bool,
    pub leftovers_killed: Vec<LeftoverRecord>,
    pub escaped: Vec<EscapeeRecord>,
    pub session_verified_empty: bool,
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
    /// Sent to this many process groups.
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
    /// The pid belongs to another process now (and its session number is not ours).
    Recycled,
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
    /// Delegated cgroup-v2 root (probed with [`CgroupRoot::probe`]); None = sampling only.
    pub cgroup: Option<CgroupRoot>,
    /// Supervisor fds the child must inherit (e.g. CPU lease flocks). Every other fd >= 3 is closed on exec.
    pub inherit_fds: Vec<RawFd>,
    /// PR_SET_PDEATHSIG(SIGKILL) with the parent check (default true).
    pub pdeathsig: bool,
}

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
            inherit_fds: Vec::new(),
            pdeathsig: true,
        }
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
        for name in &self.env_passthrough {
            if name.is_empty() || name.contains('=') {
                return Err(LaunchError::InvalidSpec(format!(
                    "bad passthrough variable name {name:?}"
                )));
            }
            if self.env.iter().any(|(k, _)| k == name.as_str()) {
                return Err(LaunchError::InvalidSpec(format!(
                    "variable {name} given both by value and by name"
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

/// Debug never prints environment values of passthrough variables (they are not stored) and prints `env` keys only.
impl fmt::Debug for LaunchSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LaunchSpec")
            .field("label", &self.label)
            .field("program", &self.program)
            .field("args", &self.args)
            .field("cwd", &self.cwd)
            .field(
                "env_keys",
                &self.env.iter().map(|(k, _)| k).collect::<Vec<_>>(),
            )
            .field("env_passthrough_names", &self.env_passthrough)
            .field("cpus", &self.cpus)
            .field("memory_cap_bytes", &self.memory_cap_bytes)
            .field("rlimit_as_bytes", &self.rlimit_as_bytes)
            .field("cgroup", &self.cgroup)
            .field("inherit_fds", &self.inherit_fds)
            .field("pdeathsig", &self.pdeathsig)
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
        let mut fd = s;
        fd.inherit_fds.push(2);
        assert!(fd.validate().is_err());
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
