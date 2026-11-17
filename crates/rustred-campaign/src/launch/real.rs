//! The real launcher: children as ordinary processes in their own sessions (R6: no systemd unit, no agent harness).
//!
//! Spawn sequence in the forked child, before exec (`pre_exec`; only async-signal-safe syscalls, nothing allocates):
//! `setsid` -> (job cgroup: write "0" to its pre-opened `cgroup.procs`) -> `prctl(PR_SET_PDEATHSIG, SIGKILL)` then
//! `getppid() == recorded parent` else `_exit(126)` -> `sched_setaffinity` -> optional `setrlimit(RLIMIT_AS)` ->
//! `close_range(3, ~0, CLOSE_RANGE_CLOEXEC)` (fallback: fcntl loop) -> clear FD_CLOEXEC on the fds to inherit (CPU-lease
//! flock fds: the ONE deliberate exception to close-all-fds, so a CPU stays locked while any process of the job lives,
//! even after a supervisor crash; audit A M6).
//!
//! Every child carries the campaign MARKER `RR_CAMPAIGN_JOB=<campaign id>:<label>` (the label is the attempt id).
//! With a memory cap and a reachable user systemd, the child execs `systemd-run --user --scope -p MemoryMax=<cap>
//! -p MemorySwapMax=0 -p OOMPolicy=kill -- <program>`: systemd-run registers its own pid as the scope and execs the
//! program IN PLACE, so the pid, the session, PDEATHSIG, the affinity, the marker and the inherited fds are kept
//! (measured on this host, see process-layer.md section 6).
//!
//! PDEATHSIG race: the parent may die between fork and the prctl; the child would then never get the signal. Reading
//! `getppid()` AFTER the prctl closes it: if the parent already died, the child was re-parented and `getppid()` differs.
//! PDEATHSIG fires when the THREAD that forked exits, not the process: spawn from a thread that lives as long as the
//! supervisor (the control thread). It is not inherited by grandchildren; those die by the group kill, by their own
//! PDEATHSIG (child-job contract 3.4.6), by the restarted supervisor's marker scan, or not at all if they cleared the
//! marker as well (see process-layer.md, orphans).
//!
//! Every signal goes through `guard` (identity checklist, pidfd delivery, leader-registry group fast path).

use super::cgroup::{ExitScope, ScopeRef};
use super::guard::{
    self, DEFAULT_VERIFY_TIMEOUT, ProcessTarget, Proof, StartRule, signal_pinned_group,
};
use super::hunt::{self, HuntResult, HuntSpec};
use super::idlock::{self, IdLock, IdLockError};
use super::{
    CapRequest, CgroupRoot, ChildHandle, ChildLauncher, ChildProgress, Enumeration, ExitState,
    FinalReport, JobCgroup, KillReason, LaunchError, LaunchSpec, LeftoverRecord, LostReason,
    MemoryMechanism, ReattachChoice, Reattached, SignalOutcome, UnattributedChild, UsageReading,
    Verification, VerifiedBy,
};
use crate::cpu_policy::CpuLease;
use crate::resources::{
    CampaignMarker, InvalidMarker, InvalidPid, Leaders, MARKER_VAR, MarkedProcess, MemKind,
    ProcStat, ProcessIdentity, Sampler, SessionUsage, Snapshot, ValidPid, boot_id, request_reap,
    run_bounded, scan_campaign, ticks_per_second,
};
use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io;
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// How the launcher should cap a job's memory when the spec says `CapRequest::Auto`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapPreference {
    /// Delegated cgroup (spec.cgroup) > systemd scope (if the probe at construction worked) > sampling.
    Auto,
    /// Never wrap in systemd-run (R6 runs without systemd; tests of the sampled cap).
    NoSystemd,
}

#[derive(Clone, Debug)]
pub struct RealConfig {
    /// After the leader exited: how long to wait for a scan showing the job empty (killing leftovers meanwhile) before
    /// reporting `Verification::Unverified` (the handle keeps working on it afterwards).
    pub verify_timeout: Duration,
    /// The campaign id of the marker. Default: an ad-hoc id unique to this supervisor process, so a restarted
    /// supervisor with a default config recognises NOTHING (fail safe). The supervisor sets the campaign's id.
    pub campaign_id: String,
    /// Become a child subreaper (PR_SET_CHILD_SUBREAPER) at construction (default true).
    pub child_subreaper: bool,
    /// A sampler whose latest scan is older than this is STALE (audit A M2; default 30 s, as guarded_run's patience).
    pub stale_after: Duration,
    /// Bound of the /proc reads of one verified signal (default 2 s).
    pub signal_verify_timeout: Duration,
    pub cap_preference: CapPreference,
    /// Directory of the host-wide campaign-id lock (None: `launch::default_lock_dir()`).
    pub id_lock_dir: Option<PathBuf>,
    /// Per-job tracking cgroup in a delegated subtree when available (Auto), or never (ProcOnly: the /proc proof only,
    /// as in the user's zellij run; tests of the fallback).
    pub tracking: TrackingPreference,
    /// Time budget of one end-of-job hunt invocation (helper thread).
    pub hunt_budget: Duration,
    /// Interval between hunts of a job whose end is Unverified (handle or quarantine).
    pub rehunt_every: Duration,
    /// Interval between the two proof passes.
    pub pass_gap: Duration,
}

/// Whether a job gets its own tracking cgroup (see [`CgroupRoot::probe_tracking`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrackingPreference {
    Auto,
    ProcOnly,
}

impl Default for RealConfig {
    fn default() -> Self {
        Self {
            verify_timeout: Duration::from_secs(120),
            campaign_id: adhoc_campaign_id(),
            child_subreaper: true,
            stale_after: Duration::from_secs(30),
            signal_verify_timeout: DEFAULT_VERIFY_TIMEOUT,
            cap_preference: CapPreference::Auto,
            id_lock_dir: None,
            tracking: TrackingPreference::Auto,
            hunt_budget: Duration::from_secs(5),
            rehunt_every: Duration::from_secs(5),
            pass_gap: Duration::from_millis(20),
        }
    }
}

/// "adhoc-<pid ns>-<pid>-<start ticks>" of this process: unique per supervisor process on the host (the pid namespace
/// inode separates processes that are pid 1 of different namespaces), the same for every launcher of the process.
pub fn adhoc_campaign_id() -> String {
    let me = std::process::id();
    // An identifier, never a proof: an unreadable part is written as 0 (procfix8: explicit, not a default of a read).
    let start = match crate::resources::read_stat(me) {
        Ok(s) => s.start_ticks,
        Err(_) => 0,
    };
    let ns = match crate::resources::procfs::sys().self_ns_link("pid") {
        Ok(l) => l
            .strip_prefix("pid:[")
            .and_then(|s| s.strip_suffix(']'))
            .unwrap_or("0")
            .to_string(),
        Err(_) => "0".into(),
    };
    format!("adhoc-{ns}-{me}-{start}")
}

/// The start of the error text of a [`Launcher::reattach`] that could not decide (an unreadable candidate): retryable.
pub const REATTACH_UNDECIDED: &str = "reattach undecided";

/// Why [`RealLauncher::try_with_config`] refused.
#[derive(Debug)]
pub enum LauncherError {
    InvalidId(InvalidMarker),
    /// Another live process holds this campaign id (gate re-audit B RB-M2), or the lock is unusable.
    IdLock(IdLockError),
}

impl std::fmt::Display for LauncherError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LauncherError::InvalidId(e) => write!(f, "{e}"),
            LauncherError::IdLock(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for LauncherError {}

/// What the launcher found at construction (for the supervisor's log).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LauncherSetup {
    pub sigchld: guard::SigchldSetup,
    pub child_subreaper: bool,
    pub pidfd: bool,
    /// Path of systemd-run when the probe scope with MemoryMax / MemorySwapMax=0 / OOMPolicy=kill worked.
    pub systemd_run: Option<PathBuf>,
    /// Why the systemd scope mechanism is not used (None when it is available).
    pub systemd_unavailable: Option<String>,
    /// Delegated cgroup under which each job gets its own tracking leaf (verification `Cgroup`), if any.
    pub tracking_root: Option<PathBuf>,
    /// Why there is no tracking root (then the proof is `ProcTwoPass`).
    pub tracking_unavailable: Option<String>,
    /// How launched jobs are enumerated for the proof.
    pub enumeration: Enumeration,
    /// The host-wide id lock held for this campaign id.
    pub id_lock: Option<PathBuf>,
    /// Set when the configured campaign id was NOT used (invalid, or held by another live process): `with_config` then
    /// runs on an ad-hoc id that recognises nothing of the configured campaign. `try_with_config` refuses instead.
    pub campaign_id_replaced: Option<String>,
}

/// A finished job whose end is not proven: kept with its CPU lease until a later hunt proves it gone.
struct Retired {
    handle: RealChild,
    lease: Option<CpuLease>,
    since: Instant,
    last_reason: String,
}

/// What the quarantine of [`RealLauncher`] did.
#[derive(Clone, Debug)]
pub enum RetireEvent {
    /// The job is proven gone now; its lease was released (`lease_note`: e.g. a CPU still held by an unmarked survivor).
    Cleared {
        label: String,
        report: Box<FinalReport>,
        lease_note: Option<String>,
        after: Duration,
    },
    /// Still not proven (kept, lease kept, hunting continues).
    StillUnverified {
        label: String,
        reason: String,
        left: usize,
    },
}

pub struct RealLauncher {
    sampler: Arc<Sampler>,
    config: RealConfig,
    setup: LauncherSetup,
    id_lock: Option<Arc<IdLock>>,
    tracking: Option<CgroupRoot>,
    quarantine: Vec<Retired>,
}

/// Find an executable on the supervisor's PATH.
fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    // an entry that cannot be checked is skipped like execvp skips it (the search goes on); only a PROVEN regular
    // file is taken
    std::env::split_paths(&path)
        .map(|d| d.join(name))
        .find(|p| crate::fsx::presence("real.program", p).is_file())
}

/// Environment needed by `systemd-run --user` to reach the user manager.
fn systemd_env() -> Vec<(String, std::ffi::OsString)> {
    let mut v = Vec::new();
    match std::env::var_os("XDG_RUNTIME_DIR") {
        Some(x) => v.push(("XDG_RUNTIME_DIR".to_string(), x)),
        None => v.push((
            "XDG_RUNTIME_DIR".to_string(),
            format!("/run/user/{}", crate::resources::our_uid()).into(),
        )),
    }
    if let Some(d) = std::env::var_os("DBUS_SESSION_BUS_ADDRESS") {
        v.push(("DBUS_SESSION_BUS_ADDRESS".to_string(), d));
    }
    v
}

/// procfix7 round 8b (host step 1): NO `--quiet`: systemd-run then writes its start acknowledgment to the job's stderr
/// after systemd started the scope and before it execs the job (round 9: the SECOND source of evidence for a job that
/// dies before its cgroup is checked, [`scope_acknowledgment`]; the first is the kernel's record of the leader's cgroup at
/// its exit, [`ExitScope`]); NO `--collect`: a scope that FAILED (e.g. result `oom-kill`) stays loaded until the
/// supervisor has read and RECORDED its state ([`scope_unit_state`]) and reset it ([`scope_reset_failed`]), whatever
/// ended the job (round 9, gate13 G13-M2); a scope that ends without failure is unloaded by systemd as before.
fn systemd_args(cap: u64, unit: Option<&str>) -> Vec<String> {
    let mut v: Vec<String> = vec!["--user".into(), "--scope".into()];
    // gate3 M5: a deterministic unit name (campaign id + label) instead of systemd-run's anonymous run-p<pid>-i<n>
    if let Some(u) = unit {
        v.push(format!("--unit={u}"));
    }
    v.extend([
        "-p".into(),
        format!("MemoryMax={cap}"),
        "-p".into(),
        "MemorySwapMax=0".into(),
        "-p".into(),
        "OOMPolicy=kill".into(),
        "--".into(),
    ]);
    v
}

/// procfix7 round 9 (gate13 G13-M1): systemd-run's start acknowledgment of the scope `unit` in `text` (the job's
/// stderr), in one of its DOCUMENTED forms, each a whole line naming EXACTLY `<unit>.scope`:
/// - `Running scope as unit: <unit>.scope` (systemd-run of older systemd releases; the round-8b form);
/// - `Running as unit: <unit>.scope` and `Running as unit: <unit>.scope; invocation ID: <32 hex>` (systemd 261 on this
///   host: the only two forms its binary holds, gate13).
///
/// Some(the form's name) or None. A SECOND source only: the first is the kernel's record of the leader's cgroup at its
/// exit ([`ExitScope`]), which does not depend on any message text. The job's own program writes to the same file, so a
/// line can only ever be counted for that job's own scope (classification, never a signal).
pub fn scope_acknowledgment(text: &str, unit: &str) -> Option<&'static str> {
    let scope = format!("{unit}.scope");
    text.lines().find_map(|l| {
        let l = l.trim_end_matches(['\r', ' ']);
        if l.strip_prefix("Running scope as unit: ") == Some(scope.as_str()) {
            return Some("Running scope as unit: <unit>.scope");
        }
        let rest = l
            .strip_prefix("Running as unit: ")?
            .strip_prefix(scope.as_str())?;
        if rest.is_empty() {
            return Some("Running as unit: <unit>.scope");
        }
        let id = rest.strip_prefix("; invocation ID: ")?;
        (id.len() == 32 && id.bytes().all(|b| b.is_ascii_hexdigit()))
            .then_some("Running as unit: <unit>.scope; invocation ID: <id>")
    })
}

/// procfix7 round 9: the state of a scope unit as systemd reports it (`systemctl --user show`), read by its exact name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScopeUnitState {
    /// `loaded`, `not-found`, ...
    pub load: String,
    /// `active`, `deactivating`, `inactive`, `failed`, ...
    pub active: String,
    /// `success`, `oom-kill`, `resources`, `timeout`, ...
    pub result: String,
}

impl ScopeUnitState {
    /// Still loaded by the user manager (a failed unit stays loaded until it is reset).
    pub fn loaded(&self) -> bool {
        self.load == "loaded"
    }
    /// A final state: not loaded, or loaded and `inactive` / `failed` (nothing will change it any more by itself
    /// except systemd's garbage collection of an inactive unit).
    pub fn settled(&self) -> bool {
        !self.loaded() || self.active == "inactive" || self.active == "failed"
    }
}

/// procfix7 round 8b / 9: the `LoadState`, `ActiveState` and `Result` of a scope unit (`systemctl --user show`, bounded
/// 10 s), by its exact name. Err when it cannot be read (never a guessed state). Never inside a private namespace.
pub fn scope_unit_state(unit: &str) -> Result<ScopeUnitState, String> {
    if systemd_units_unavailable() {
        return Err("inside a private namespace: no systemd unit".into());
    }
    exact_unit_name(unit)?;
    let exe = which("systemctl").ok_or("systemctl not on PATH")?;
    let name = format!("{unit}.scope");
    let args = [
        "--user",
        "show",
        "--property=LoadState,ActiveState,Result",
        "--",
        name.as_str(),
    ]
    .map(String::from)
    .to_vec();
    let out = stdout_bounded(exe, args, Duration::from_secs(10))
        .map_err(|e| format!("systemctl show: {e}"))?;
    let t = String::from_utf8_lossy(&out);
    let get = |k: &str| {
        t.lines()
            .find_map(|l| l.strip_prefix(k).and_then(|v| v.strip_prefix('=')))
            .map(str::to_string)
    };
    match (get("LoadState"), get("ActiveState"), get("Result")) {
        (Some(load), Some(active), Some(result)) => Ok(ScopeUnitState {
            load,
            active,
            result,
        }),
        _ => Err(format!("systemctl show: unparsable answer {t:?}")),
    }
}

/// procfix7 round 9: the standard output of `exe args` (the systemd environment only, stdin / stderr null), bounded by
/// `timeout` (helper thread). `.status()` with stdout on a pipe (the process-form tripwire allows no `.output()`): the
/// answer is a few bytes, far below the pipe's capacity, read after the exit. The `Command` is DROPPED before the read:
/// it holds the pipe's write end, and while it lives the read never sees EOF (the round-8b `scope_unit_result` kept it
/// and timed out on every call, leaving its helper thread blocked: found by the real-systemd check of round 9).
fn stdout_bounded(exe: PathBuf, args: Vec<String>, timeout: Duration) -> Result<Vec<u8>, String> {
    let (mut rd, wr) = std::io::pipe().map_err(|e| format!("pipe: {e}"))?;
    let r = run_bounded(timeout, move || {
        let st = {
            let mut c = Command::new(&exe);
            c.args(&args)
                .env_clear()
                .stdin(Stdio::null())
                .stdout(wr)
                .stderr(Stdio::null());
            for (k, v) in systemd_env() {
                c.env(k, v);
            }
            c.status()?
            // `c` and its copy of the write end are dropped here
        };
        let mut out = Vec::new();
        std::io::Read::read_to_end(&mut rd, &mut out)?;
        Ok::<_, std::io::Error>((st, out))
    });
    match r {
        crate::resources::Bounded::Done(Ok((st, out))) if st.success() => Ok(out),
        crate::resources::Bounded::Done(Ok((st, _))) => Err(format!("{st}")),
        crate::resources::Bounded::Done(Err(e)) => Err(e.to_string()),
        crate::resources::Bounded::TimedOut => {
            Err(format!("no answer within {} s", timeout.as_secs_f64()))
        }
        crate::resources::Bounded::SpawnFailed(e) => Err(e.to_string()),
    }
}

/// procfix7 round 9: a unit name we pass to systemctl must be ONE literal name: only [A-Za-z0-9_.-] (our scope names are
/// sanitized to exactly that, cgroup.rs `scope_unit_name`): no glob character can reach `reset-failed`'s pattern match.
fn exact_unit_name(unit: &str) -> Result<(), String> {
    if !unit.is_empty()
        && unit.len() <= 240
        && unit
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
    {
        Ok(())
    } else {
        Err(format!("not a literal unit name of ours: {unit:?}"))
    }
}

/// procfix7 round 8b / 9: unload a FAILED scope of this campaign by its exact name (`systemctl --user reset-failed --
/// <unit>.scope`: one literal unit name, never a pattern), after its state was read and recorded. A unit left loaded is
/// listed by P8 / the operator; never inside a namespace. Sends no signal (it only clears systemd's failed state).
pub fn scope_reset_failed(unit: &str) -> Result<(), String> {
    if systemd_units_unavailable() {
        return Err("inside a private namespace: no systemd unit".into());
    }
    exact_unit_name(unit)?;
    let exe = which("systemctl").ok_or("systemctl not on PATH")?;
    let name = format!("{unit}.scope");
    match run_bounded(Duration::from_secs(10), move || {
        let mut c = Command::new(&exe);
        c.args(["--user", "reset-failed", "--", &name])
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        for (k, v) in systemd_env() {
            c.env(k, v);
        }
        c.status()
    }) {
        crate::resources::Bounded::Done(Ok(s)) if s.success() => Ok(()),
        crate::resources::Bounded::Done(Ok(s)) => Err(format!("systemctl reset-failed: {s}")),
        crate::resources::Bounded::Done(Err(e)) => Err(format!("systemctl reset-failed: {e}")),
        crate::resources::Bounded::TimedOut => {
            Err("systemctl reset-failed: no answer within 10 s".into())
        }
        crate::resources::Bounded::SpawnFailed(e) => Err(format!("systemctl reset-failed: {e}")),
    }
}

/// procfix7 round 9 (gate13 G13-M1): the outcome of [`scope_evidence_check`].
#[derive(Clone, Debug)]
pub struct ScopeCheck {
    /// The unit started (`<unit>.scope`).
    pub unit: String,
    /// What was seen, one line each (printed by `rr_campaign scope-check`).
    pub lines: Vec<String>,
    /// Both sources of evidence named the exact unit and nothing was left loaded.
    pub pass: bool,
}

/// procfix7 round 9 (gate13 G13-M1): the REAL-systemd check of the evidence a job that dies before its first check is
/// recorded from (`rr_campaign scope-check`, the host step's pre-flight). It starts ONE transient scope exactly as a job's
/// (`systemd-run --user --scope --unit=<unit> -p MemoryMax=64M -p MemorySwapMax=0 -p OOMPolicy=kill -- true`, no
/// `--quiet`, stderr on a pipe), observes the exit WITHOUT reaping (waitid WNOWAIT: the pid stays ours), reads the
/// kernel's record of the leader's cgroup at the exit ([`ExitScope`]) once at once and once after systemd removed the
/// unit (it must then read ` (deleted)`), parses systemd-run's start acknowledgment ([`scope_acknowledgment`]), reaps,
/// and checks that nothing named `<unit>.scope` stays loaded (a failed one is recorded here and reset by its exact name).
/// It starts no supervisor, launcher, sampler or sweep and SENDS NO SIGNAL (none is in this code path): if the scope
/// does not end within 30 s the check fails and the child is left to end by itself. Never inside a private namespace.
pub fn scope_evidence_check(unit: Option<&str>) -> ScopeCheck {
    let unit = match unit {
        Some(u) => u.to_string(),
        None => {
            let h = release_token()
                .map(|t| blake3::hash(&t).to_hex()[..16].to_string())
                .unwrap_or_else(|_| format!("{:016x}", Instant::now().elapsed().as_nanos()));
            format!("rr-scope-check-{}-{h}", std::process::id())
        }
    };
    let mut c = ScopeCheck {
        unit: format!("{unit}.scope"),
        lines: Vec::new(),
        pass: false,
    };
    fn fail(c: &mut ScopeCheck, m: String) {
        c.lines.push(format!("FAIL: {m}"));
    }
    if let Err(e) = exact_unit_name(&unit) {
        fail(&mut c, e);
        return c;
    }
    match namespace_state() {
        Ok(false) => {}
        Ok(true) => {
            fail(&mut c, "inside a private user / pid namespace: systemd would see foreign pids (run it on the host)".into());
            return c;
        }
        Err(e) => {
            fail(&mut c, format!("the namespace cannot be determined ({e})"));
            return c;
        }
    }
    let (Some(exe), Some(truth)) = (which("systemd-run"), which("true")) else {
        fail(&mut c, "systemd-run or true not on PATH".into());
        return c;
    };
    c.lines.push(format!("systemd-run: {}", exe.display()));
    match scope_unit_state(&unit) {
        Ok(st) if st.loaded() => {
            fail(
                &mut c,
                format!(
                    "{}.scope is already loaded ({st:?}): choose another name",
                    unit
                ),
            );
            return c;
        }
        Ok(_) => {}
        Err(e) => {
            fail(&mut c, format!("the user manager cannot be asked ({e})"));
            return c;
        }
    }
    let (mut rd, wr) = match std::io::pipe() {
        Ok(p) => p,
        Err(e) => {
            fail(&mut c, format!("pipe: {e}"));
            return c;
        }
    };
    let args = systemd_args(64 << 20, Some(&unit));
    c.lines.push(format!(
        "started: {} {} {}",
        exe.display(),
        args.join(" "),
        truth.display()
    ));
    let spawned = {
        let mut cmd = Command::new(&exe);
        cmd.args(&args)
            .arg(&truth)
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(wr);
        for (k, v) in systemd_env() {
            cmd.env(k, v);
        }
        cmd.spawn()
        // `cmd` (and its copy of the pipe's write end) is dropped here: EOF once the scope's process exits
    };
    let mut child = match spawned {
        Ok(ch) => ch,
        Err(e) => {
            fail(&mut c, format!("systemd-run could not be started: {e}"));
            return c;
        }
    };
    let pid = child.id();
    let until = Instant::now() + Duration::from_secs(30);
    let exit = loop {
        match peek_exit(pid) {
            Ok(Some(e)) => break Some(e),
            Ok(None) if Instant::now() < until => std::thread::sleep(Duration::from_millis(20)),
            Ok(None) => break None,
            Err(e) => {
                fail(&mut c, format!("waitid of our child {pid}: {e}"));
                return c;
            }
        }
    };
    let Some(exit) = exit else {
        fail(
            &mut c,
            format!(
                "the scope's process {pid} did not end within 30 s (left to end by itself; nothing was signalled)"
            ),
        );
        return c;
    };
    let uid = crate::resources::our_uid();
    let read = |what: &str| match crate::resources::procfs::sys()
        .cgroup_text(pid, Duration::from_secs(1))
    {
        Ok(t) => (
            t.trim_end().to_string(),
            ExitScope::from_cgroup_text(&t, uid, &unit),
            what.to_string(),
        ),
        Err(e) => (
            String::new(),
            ExitScope::Unknown(e.to_string()),
            what.to_string(),
        ),
    };
    let first = read("at the exit");
    // systemd stops the empty scope and removes its cgroup; a scope that ended without failure is then unloaded
    let until = Instant::now() + Duration::from_secs(15);
    let mut st = scope_unit_state(&unit);
    while matches!(&st, Ok(s) if s.loaded() && s.active != "failed") && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(50));
        st = scope_unit_state(&unit);
    }
    match &st {
        Ok(s) if s.loaded() && s.active == "failed" => {
            c.lines.push(format!(
                "the unit FAILED: {s:?} (recorded here); reset by its exact name"
            ));
            match scope_reset_failed(&unit) {
                Ok(()) => {}
                Err(e) => c.lines.push(format!("reset-failed: {e}")),
            }
        }
        Ok(s) if s.loaded() => c
            .lines
            .push(format!("the unit is still loaded after 15 s: {s:?}")),
        Ok(_) => c.lines.push("the unit was unloaded by systemd".into()),
        Err(e) => c
            .lines
            .push(format!("the unit's state could not be read: {e}")),
    }
    let second = read("after systemd removed the unit");
    let reaped = child.try_wait();
    c.lines.push(format!(
        "exit of the scope's process {pid}: {exit:?}; reaped: {reaped:?}"
    ));
    let mut text = Vec::new();
    if let Err(e) = std::io::Read::read_to_end(&mut rd, &mut text) {
        c.lines.push(format!("stderr of systemd-run: {e}"));
    }
    let text = String::from_utf8_lossy(&text).into_owned();
    for l in text.lines() {
        c.lines.push(format!("systemd-run stderr: {l}"));
    }
    let form = scope_acknowledgment(&text, &unit);
    c.lines.push(format!(
        "start acknowledgment naming exactly {unit}.scope: {}",
        form.map_or("NONE RECOGNIZED".to_string(), |f| format!("form {f:?}"))
    ));
    for (line, ev, what) in [&first, &second] {
        c.lines
            .push(format!("kernel record {what}: {line:?} -> {ev:?}"));
    }
    let after = scope_unit_state(&unit);
    c.lines
        .push(format!("{unit}.scope after the check: {after:?}"));
    let inside = |e: &ExitScope| matches!(e, ExitScope::Inside { .. });
    let mut why = Vec::new();
    if form.is_none() {
        why.push("no start acknowledgment in a documented form names the exact unit");
    }
    if !inside(&first.1) {
        why.push("the kernel's record at the exit does not name the exact unit");
    }
    if !matches!(second.1, ExitScope::Inside { deleted: true, .. }) {
        why.push(
            "the kernel's record after the unit was gone is not the exact unit marked (deleted)",
        );
    }
    if !matches!(&after, Ok(s) if !s.loaded()) {
        why.push("the unit is still loaded (or its state is unknown) after the check");
    }
    if !matches!(exit, ExitState::Exited(0)) {
        why.push("the scope's process did not exit 0");
    }
    if why.is_empty() {
        c.pass = true;
        c.lines.push("PASS".into());
    } else {
        fail(&mut c, why.join("; "));
    }
    c
}

/// TEST BUILDS ONLY (procfix7 round 8b): `scope_pending:1` in RR_CAMPAIGN_FAILPOINT makes every job's containment
/// `Pending` with the requested unit [`TEST_PENDING_UNIT`] ("the cgroup does not name the scope"), also inside a
/// namespace.
fn test_scope_pending() -> Option<&'static str> {
    #[cfg(debug_assertions)]
    {
        static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        let on = *ON.get_or_init(|| {
            std::env::var(crate::supervisor::failpoint::FAILPOINT_ENV)
                .is_ok_and(|v| v.split(',').any(|p| p.trim() == "scope_pending:1"))
        });
        on.then_some(TEST_PENDING_UNIT)
    }
    #[cfg(not(debug_assertions))]
    {
        None
    }
}

/// TEST BUILDS ONLY (procfix7 round 9): `scope_exit:1` in RR_CAMPAIGN_FAILPOINT makes the kernel's record of a leader's
/// cgroup at its exit read as the requested scope, removed (` (deleted)`); `scope_exit:2` makes it unknown (as when
/// the read fails); also inside a namespace. None: no override.
fn test_exit_override() -> Option<u8> {
    #[cfg(debug_assertions)]
    {
        static ON: std::sync::OnceLock<Option<u8>> = std::sync::OnceLock::new();
        *ON.get_or_init(|| {
            let v = std::env::var(crate::supervisor::failpoint::FAILPOINT_ENV).ok()?;
            v.split(',').find_map(|p| match p.trim() {
                "scope_exit:1" => Some(1),
                "scope_exit:2" => Some(2),
                _ => None,
            })
        })
    }
    #[cfg(not(debug_assertions))]
    {
        None
    }
}

/// TEST BUILDS ONLY (procfix7 round 9): `fake_systemd:1` in RR_CAMPAIGN_FAILPOINT lets the unit queries and resets
/// ([`scope_unit_state`], [`scope_reset_failed`]) run inside a private namespace, where the `systemctl` found on PATH is
/// a test's fake (a namespaced test of the record-then-reset order). Release builds: false.
pub fn test_fake_systemd() -> bool {
    #[cfg(debug_assertions)]
    {
        static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *ON.get_or_init(|| {
            std::env::var(crate::supervisor::failpoint::FAILPOINT_ENV)
                .is_ok_and(|v| v.split(',').any(|p| p.trim() == "fake_systemd:1"))
        })
    }
    #[cfg(not(debug_assertions))]
    {
        false
    }
}

/// procfix7 round 9: no systemd unit can be asked about or reset here (inside a private namespace, unless a test build
/// armed `fake_systemd:1`).
pub fn systemd_units_unavailable() -> bool {
    in_private_namespace() && !test_fake_systemd()
}

/// The requested unit name under the test failpoint `scope_pending:1`.
#[cfg_attr(not(debug_assertions), allow(dead_code))]
pub const TEST_PENDING_UNIT: &str = "rr-fp-pending";

/// Are we inside a non-initial user or pid namespace? Then the host's user manager would interpret the pids that
/// systemd-run passes in ITS namespace, i.e. as other processes: the scope mechanism must not be used.
/// procfix8: an unreadable uid map counts as a namespace here (the conservative answer for a LAUNCH: no systemd scope).
/// A decision for which "unknown" is not conservative (gate9 M9-1: the recovery's scope backstop) uses
/// [`namespace_state`].
pub fn in_private_namespace() -> bool {
    namespace_state().unwrap_or(true)
}

/// procfix7 round 5 (gate9 M9-1): the namespace question, typed: Ok(true) inside a non-initial user or pid namespace,
/// Ok(false) in the initial ones, Err when `/proc/self/uid_map` cannot be read (never "not a namespace").
pub fn namespace_state() -> Result<bool, String> {
    // SAFETY: getpid cannot fail.
    if unsafe { libc::getpid() } == 1 {
        return Ok(true);
    }
    match crate::resources::procfs::sys().self_uid_map() {
        Ok(m) => {
            let f: Vec<&str> = m.split_whitespace().collect();
            Ok(f != ["0", "0", "4294967295"])
        }
        Err(e) => Err(format!("the uid map: {e}")),
    }
}

/// Probe: a transient scope with the job properties around /bin/true, bounded to 15 s.
fn probe_systemd() -> Result<PathBuf, String> {
    if in_private_namespace() {
        return Err("inside a private user/pid namespace: systemd would see foreign pids".into());
    }
    let exe = which("systemd-run").ok_or("systemd-run not on PATH")?;
    let truth = which("true").ok_or("true not on PATH")?;
    let exe2 = exe.clone();
    let r = run_bounded(Duration::from_secs(15), move || {
        let mut c = Command::new(&exe2);
        c.args(systemd_args(64 << 20, None))
            .arg(&truth)
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        for (k, v) in systemd_env() {
            c.env(k, v);
        }
        c.status()
    });
    match r.done() {
        Some(Ok(s)) if s.success() => Ok(exe),
        Some(Ok(s)) => Err(format!("probe scope failed: {s}")),
        Some(Err(e)) => Err(format!("probe scope: {e}")),
        None => Err("probe scope timed out (15 s)".into()),
    }
}

impl RealLauncher {
    /// The sampler must be shared with the control loop (one scanner per supervisor).
    pub fn new(sampler: Arc<Sampler>) -> Self {
        Self::with_config(sampler, RealConfig::default())
    }

    /// As [`Self::with_config`], but an invalid `campaign_id` is a hard ERROR instead of a silent ad-hoc id (gate
    /// re-audit C-1 (a): the campaign id is the only proof of ownership; a swapped id would make a restarted supervisor
    /// recognise nothing). The supervisor uses this. Additive (lane I-SUP, 2026-11-14).
    ///
    /// Second fix package: also refuses (`LauncherError::IdLock`) when another LIVE process holds the host-wide lock of
    /// this campaign id (gate re-audit B RB-M2); nothing is signalled then.
    pub fn try_with_config(
        sampler: Arc<Sampler>,
        config: RealConfig,
    ) -> Result<Self, LauncherError> {
        crate::resources::validate_campaign_id(&config.campaign_id)
            .map_err(LauncherError::InvalidId)?;
        let lock = idlock::acquire(&config.campaign_id, config.id_lock_dir.as_deref())
            .map_err(LauncherError::IdLock)?;
        Ok(Self::build(sampler, config, Some(lock), None))
    }

    /// Process-wide effects (documented): SIGCHLD reset to SIG_DFL if inherited as ignored (audit A M4); child
    /// subreaper if configured; the sampler's campaign is set to `config.campaign_id`; the adopted-children reaper thread
    /// is started (once per process).
    ///
    /// An invalid id, or an id whose host-wide lock another live process holds, is REPLACED by an ad-hoc id (recorded in
    /// `setup().campaign_id_replaced`); the supervisor uses [`Self::try_with_config`], which refuses instead.
    pub fn with_config(sampler: Arc<Sampler>, config: RealConfig) -> Self {
        let replaced = match crate::resources::validate_campaign_id(&config.campaign_id) {
            Err(e) => Some(e.to_string()),
            Ok(()) => match idlock::acquire(&config.campaign_id, config.id_lock_dir.as_deref()) {
                Ok(lock) => return Self::build(sampler, config, Some(lock), None),
                Err(e) => Some(e.to_string()),
            },
        };
        let mut c = config;
        c.campaign_id = adhoc_campaign_id();
        let lock = idlock::acquire(&c.campaign_id, c.id_lock_dir.as_deref()).ok();
        Self::build(sampler, c, lock, replaced)
    }

    fn build(
        sampler: Arc<Sampler>,
        config: RealConfig,
        id_lock: Option<Arc<IdLock>>,
        replaced: Option<String>,
    ) -> Self {
        let sigchld =
            guard::ensure_sigchld_default().unwrap_or(guard::SigchldSetup::AlreadyDefault);
        let child_subreaper = config.child_subreaper && guard::set_child_subreaper(true).is_ok();
        // SAFETY: getpid cannot fail.
        let init = unsafe { libc::getpid() } == 1;
        crate::resources::start_reaper(
            Arc::clone(sampler.leaders()),
            Duration::from_millis(100),
            1024,
        );
        let (tracking, tracking_unavailable) = match config.tracking {
            TrackingPreference::ProcOnly => (None, Some("disabled by configuration".to_string())),
            TrackingPreference::Auto => match CgroupRoot::probe_tracking() {
                Ok(r) => (Some(r), None),
                Err(e) => (None, Some(e)),
            },
        };
        sampler.set_campaign(&config.campaign_id, 0);
        let (systemd_run, systemd_unavailable) = match config.cap_preference {
            CapPreference::NoSystemd => (None, Some("disabled by configuration".into())),
            CapPreference::Auto => match probe_systemd() {
                Ok(p) => (Some(p), None),
                Err(e) => (None, Some(e)),
            },
        };
        let setup = LauncherSetup {
            sigchld,
            child_subreaper,
            pidfd: guard::pidfd_supported(),
            systemd_run,
            systemd_unavailable,
            tracking_root: tracking.as_ref().map(|t| t.path().to_path_buf()),
            tracking_unavailable,
            enumeration: if child_subreaper || init {
                Enumeration::SubreaperTree
            } else {
                Enumeration::FullProc
            },
            id_lock: id_lock.as_ref().map(|l| l.path().to_path_buf()),
            campaign_id_replaced: replaced,
        };
        Self {
            sampler,
            config,
            setup,
            id_lock,
            tracking,
            quarantine: Vec::new(),
        }
    }

    pub fn sampler(&self) -> &Arc<Sampler> {
        &self.sampler
    }

    pub fn config(&self) -> &RealConfig {
        &self.config
    }

    pub fn setup(&self) -> &LauncherSetup {
        &self.setup
    }

    /// procfix6 (gate5 m5): would `launch(spec)` put the job into a systemd scope? (The same choice `launch` makes: a
    /// memory cap, mechanism auto / scope, no delegated cgroup, systemd-run available.)
    pub fn scope_requested(&self, spec: &LaunchSpec) -> bool {
        spec.memory_cap_bytes.is_some()
            && matches!(
                spec.cap_mechanism,
                CapRequest::Auto | CapRequest::SystemdScope
            )
            && spec.cgroup.is_none()
            && self.setup.systemd_run.is_some()
    }

    /// SIGKILL every process group of an unreaped leader of this launcher's sampler, at once (fast exit). Returns the
    /// number of groups signalled. Grandchildren in other groups or sessions are left to the restart's orphan scan.
    pub fn kill_all_pinned(&self) -> usize {
        guard::with_context("fast exit", None, || {
            guard::kill_all_pinned(self.sampler.leaders())
        })
    }

    /// The host-wide id lock file held by this launcher (None only if even the ad-hoc lock failed).
    pub fn id_lock_path(&self) -> Option<&Path> {
        self.id_lock.as_deref().map(IdLock::path)
    }

    /// Hand over a FINISHED handle together with its CPU lease (gate re-audit B RB-C1 d). A `Verified` job's lease is
    /// released at once (`Some(Cleared)`, with a note when a CPU's flock is still held by another process). An
    /// `Unverified` job is kept in the QUARANTINE with its lease and its unreaped leader (the session stays pinned):
    /// [`Self::tick`] keeps hunting it and releases the lease only once a later hunt proves the job gone. A handle that is
    /// not finished yet is kept as well (it is polled by `tick`).
    pub fn retire(&mut self, handle: RealChild, lease: Option<CpuLease>) -> Option<RetireEvent> {
        match &handle.finished {
            Some(r) if r.verification.is_verified() => {
                let report = Box::new(r.clone());
                let lease_note = lease.and_then(release_lease);
                Some(RetireEvent::Cleared {
                    label: handle.label.clone(),
                    report,
                    lease_note,
                    after: Duration::ZERO,
                })
            }
            _ => {
                self.quarantine.push(Retired {
                    handle,
                    lease,
                    since: Instant::now(),
                    last_reason: String::new(),
                });
                None
            }
        }
    }

    /// Drive the quarantine (non-blocking: hunts run on helper threads). Returns what changed.
    pub fn tick(&mut self) -> Vec<RetireEvent> {
        let mut out = Vec::new();
        let mut i = 0;
        while i < self.quarantine.len() {
            let q = &mut self.quarantine[i];
            match q.handle.poll() {
                Ok(ChildProgress::Finished(r)) if r.verification.is_verified() => {
                    let q = self.quarantine.swap_remove(i);
                    let lease_note = q.lease.and_then(release_lease);
                    out.push(RetireEvent::Cleared {
                        label: q.handle.label.clone(),
                        report: Box::new(r),
                        lease_note,
                        after: q.since.elapsed(),
                    });
                    continue;
                }
                Ok(ChildProgress::Finished(r)) => {
                    if let Verification::Unverified { leftovers, reason } = &r.verification
                        && *reason != q.last_reason
                    {
                        q.last_reason = reason.clone();
                        out.push(RetireEvent::StillUnverified {
                            label: q.handle.label.clone(),
                            reason: reason.clone(),
                            left: leftovers.len(),
                        });
                    }
                }
                _ => {}
            }
            i += 1;
        }
        out
    }

    /// procfix7 (gate7 MJ-1): release the CPU lease of a quarantined job whose end the caller stopped waiting for (the
    /// handle stays: its hunts go on and may still prove the end). Some(note): released (with the note of a CPU whose
    /// flock is still held); None: no such label, or no lease left.
    pub fn release_quarantined_lease(&mut self, label: &str) -> Option<String> {
        let q = self
            .quarantine
            .iter_mut()
            .find(|q| q.handle.label == label)?;
        let lease = q.lease.take()?;
        Some(release_lease(lease).unwrap_or_default())
    }

    /// Labels in the quarantine with the time since they were retired and the latest reason.
    pub fn quarantined(&self) -> Vec<(String, Duration, String)> {
        self.quarantine
            .iter()
            .map(|q| {
                (
                    q.handle.label.clone(),
                    q.since.elapsed(),
                    q.last_reason.clone(),
                )
            })
            .collect()
    }

    /// Drop every quarantined handle NOW (each Drop SIGKILLs its pinned leader group through the guard and hands the
    /// zombie to the reaper): the supervisor calls it before it releases its campaign locks (gate-A m-7). Returns how
    /// many were dropped.
    pub fn abandon_quarantine(&mut self) -> usize {
        let n = self.quarantine.len();
        self.quarantine.clear();
        n
    }

    /// Blocking (bounded): drive the quarantine until it is empty or `timeout` passes (supervisor exit). What is left
    /// stays unproven: its handles are dropped with the launcher (the pinned groups get SIGKILL) and the restart's orphan
    /// scan looks for the markers.
    pub fn shutdown(&mut self, timeout: Duration) -> Vec<RetireEvent> {
        let t0 = Instant::now();
        let mut out = Vec::new();
        while !self.quarantine.is_empty() && t0.elapsed() < timeout {
            out.extend(self.tick());
            std::thread::sleep(Duration::from_millis(50));
        }
        out
    }
}

/// Release a lease and report CPUs whose flock is STILL held afterwards (an unmarked survivor that inherited the lock
/// descriptor: gate re-audit B m2). The holder is listed for a human, never signalled.
fn release_lease(lease: CpuLease) -> Option<String> {
    let (r, held) = lease.release_checked();
    let mut notes = Vec::new();
    if let Err(e) = r {
        notes.push(format!("release: {e}"));
    }
    for h in held {
        notes.push(format!(
            "CPU {} still held after release by {} (unmarked survivor? not signalled; its claim stays in the ledger)",
            h.cpu,
            if h.holders.is_empty() {
                "an unknown process".to_string()
            } else {
                h.holders
                    .iter()
                    .map(|p| format!("pid {p}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        ));
    }
    (!notes.is_empty()).then(|| notes.join("; "))
}

fn last_err() -> io::Error {
    io::Error::last_os_error()
}

/// Body of `pre_exec`. SAFETY contract of the caller: runs in the forked child, single-threaded; only async-signal-safe
/// calls; no allocation (`io::Error::last_os_error` does not allocate).
#[allow(clippy::too_many_arguments)]
fn child_setup(
    parent: libc::pid_t,
    pin: Option<&libc::cpu_set_t>,
    rlimit_as: Option<u64>,
    keep: &[RawFd],
    fd_limit: RawFd,
    procs_fd: Option<RawFd>,
    pdeathsig: bool,
) -> io::Result<()> {
    // SAFETY: setsid has no memory arguments; a forked child is never a group leader, so it succeeds or reports errno.
    if unsafe { libc::setsid() } < 0 {
        return Err(last_err());
    }
    // procfix5 (gate4 m2): the supervisor ignores SIGXFSZ (EFBIG -> exit 75); the child gets the default back
    // (signal() is async-signal-safe).
    // SAFETY: plain integers; SIG_DFL.
    unsafe { libc::signal(libc::SIGXFSZ, libc::SIG_DFL) };
    if let Some(fd) = procs_fd {
        // SAFETY: fd is an open descriptor of cgroup.procs owned by the parent's JobCgroup; the buffer is a static
        // one-byte slice. Writing "0" migrates the writing process (this child) into the job cgroup.
        if unsafe { libc::write(fd, b"0".as_ptr().cast(), 1) } != 1 {
            return Err(last_err());
        }
    }
    if pdeathsig {
        // SAFETY: PR_SET_PDEATHSIG takes a signal number; the remaining arguments are unused.
        if unsafe {
            libc::prctl(
                libc::PR_SET_PDEATHSIG,
                libc::SIGKILL as libc::c_ulong,
                0,
                0,
                0,
            )
        } != 0
        {
            return Err(last_err());
        }
        // SAFETY: getppid has no arguments and cannot fail.
        if unsafe { libc::getppid() } != parent {
            // The supervisor died before the prctl: nobody will ever signal us.
            // SAFETY: _exit is async-signal-safe and never returns.
            unsafe { libc::_exit(126) };
        }
    }
    if let Some(set) = pin {
        // SAFETY: pid 0 = this process; size and pointer describe a valid cpu_set_t owned by the closure.
        if unsafe { libc::sched_setaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), set) } != 0 {
            return Err(last_err());
        }
    }
    if let Some(bytes) = rlimit_as {
        let r = libc::rlimit {
            rlim_cur: bytes as libc::rlim_t,
            rlim_max: bytes as libc::rlim_t,
        };
        // SAFETY: r is a valid rlimit living across the call.
        if unsafe { libc::setrlimit(libc::RLIMIT_AS, &r) } != 0 {
            return Err(last_err());
        }
    }
    // CLOEXEC sweep: every fd >= 3 becomes close-on-exec (std's own error pipe already is).
    // SAFETY: close_range with the CLOEXEC flag only changes descriptor flags; arguments are plain integers.
    let swept = unsafe {
        libc::syscall(
            libc::SYS_close_range,
            3u32,
            u32::MAX,
            libc::CLOSE_RANGE_CLOEXEC,
        )
    } == 0;
    if !swept {
        for fd in 3..fd_limit {
            // SAFETY: fcntl on an arbitrary integer is harmless: EBADF for closed slots.
            unsafe {
                let fl = libc::fcntl(fd, libc::F_GETFD);
                if fl >= 0 && fl & libc::FD_CLOEXEC == 0 {
                    libc::fcntl(fd, libc::F_SETFD, fl | libc::FD_CLOEXEC);
                }
            }
        }
    }
    for &fd in keep {
        // SAFETY: as above; the caller listed fds it wants inherited.
        let ok = unsafe {
            let fl = libc::fcntl(fd, libc::F_GETFD);
            fl >= 0 && libc::fcntl(fd, libc::F_SETFD, fl & !libc::FD_CLOEXEC) >= 0
        };
        if !ok {
            return Err(last_err());
        }
    }
    Ok(())
}

fn nofile_limit() -> RawFd {
    let mut r = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    // SAFETY: r is a valid out-pointer.
    if unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut r) } != 0 {
        return 1024;
    }
    r.rlim_cur.clamp(64, 1 << 20) as RawFd
}

/// Peek at a child's exit without reaping it (`waitid(P_PID, WEXITED | WNOHANG | WNOWAIT)`).
/// Only valid for direct children of this process. ECHILD: not (or no longer) a waitable child of ours.
pub fn peek_exit(pid: u32) -> io::Result<Option<ExitState>> {
    let p = ValidPid::new(i64::from(pid)).map_err(InvalidPid::into_io)?;
    // SAFETY: siginfo_t is plain data; zeroed is valid and makes si_pid 0 when no child is waitable.
    let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
    // SAFETY: info is a valid out-pointer; P_PID with a positive pid.
    let r = unsafe {
        libc::waitid(
            libc::P_PID,
            p.get() as libc::id_t,
            &mut info,
            libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
        )
    };
    if r != 0 {
        return Err(last_err());
    }
    // SAFETY: waitid filled info (or left it zeroed); these accessors read the SIGCHLD union fields.
    let (spid, status) = unsafe { (info.si_pid(), info.si_status()) };
    if spid == 0 {
        return Ok(None);
    }
    Ok(Some(match info.si_code {
        libc::CLD_EXITED => ExitState::Exited(status),
        libc::CLD_KILLED => ExitState::Signaled {
            signal: status,
            core_dumped: false,
        },
        libc::CLD_DUMPED => ExitState::Signaled {
            signal: status,
            core_dumped: true,
        },
        _ => return Ok(None),
    }))
}

/// fds >= 3 of this process WITHOUT FD_CLOEXEC, with their targets (for the pre-flight / logs). The launcher makes
/// every such fd close-on-exec in the child anyway; this lists what the supervisor inherited from its own parent.
pub fn audit_inheritable_fds() -> io::Result<Vec<(RawFd, String)>> {
    let mut out = Vec::new();
    for (fd, target) in crate::resources::procfs::sys().self_fds()? {
        if fd < 3 {
            continue;
        }
        // SAFETY: fcntl F_GETFD on an integer; EBADF if it was the directory fd of this very listing, now closed.
        let fl = unsafe { libc::fcntl(fd, libc::F_GETFD) };
        if fl >= 0 && fl & libc::FD_CLOEXEC == 0 {
            out.push((fd, target.display().to_string()));
        }
    }
    Ok(out)
}

/// Hand a `Child` that may not have exited yet to the adopted-children reaper (never block the caller; audit A M5).
/// The caller removed it from the leader registry first: the reaper then reaps its zombie by exact pid, through a pidfd,
/// under the registry lock (no thread blocked per child: gate re-audit A m-4). No wait by NUMBER here any more (gate-A
/// m-1: a `try_wait` after the registry lock was released could, after a full pid wrap, have waited for a new leader).
fn reap_in_background(c: Child) {
    drop(c);
    request_reap();
}

/// Unattributed children already reported in some job's report (each is reported once).
fn first_report_of(list: Vec<UnattributedChild>) -> Vec<UnattributedChild> {
    static SEEN: std::sync::OnceLock<std::sync::Mutex<BTreeSet<(u32, u64)>>> =
        std::sync::OnceLock::new();
    let mut seen = SEEN
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    list.into_iter()
        .filter(|u| seen.insert((u.pid, u.start_ticks)))
        .collect()
}

/// A running end-of-job hunt (helper thread).
struct HuntTask {
    rx: mpsc::Receiver<HuntResult>,
}

impl ChildLauncher for RealLauncher {
    type Handle = RealChild;

    fn launch(&mut self, spec: &LaunchSpec) -> Result<RealChild, LaunchError> {
        spec.validate()?;
        let marker = CampaignMarker::new(&self.config.campaign_id, &spec.label)
            .map_err(|e| LaunchError::InvalidSpec(e.to_string()))?;
        // Which cap mechanism (M7).
        let mechanism_choice = match (spec.memory_cap_bytes, spec.cap_mechanism) {
            (None, _) => Chosen::None,
            (Some(_), CapRequest::Sampling) => Chosen::Sampling,
            (Some(cap), CapRequest::Auto | CapRequest::SystemdScope) => match &spec.cgroup {
                Some(root) => Chosen::Cgroup(
                    root.create_job(&spec.label, cap)
                        .map_err(LaunchError::Cgroup)?,
                ),
                None => match (&self.setup.systemd_run, spec.cap_mechanism) {
                    (Some(p), _) => Chosen::Scope(p.clone(), cap),
                    (None, CapRequest::SystemdScope) => {
                        return Err(LaunchError::InvalidSpec(format!(
                            "systemd scope requested but unavailable: {}",
                            self.setup
                                .systemd_unavailable
                                .as_deref()
                                .unwrap_or("unknown")
                        )));
                    }
                    (None, _) => Chosen::Sampling,
                },
            },
        };
        // The program and its leading arguments (systemd-run wraps the program when the scope mechanism is chosen).
        let (prog, mut lead): (std::ffi::OsString, Vec<std::ffi::OsString>) =
            match &mechanism_choice {
                Chosen::Scope(exe, cap) => {
                    let mut a: Vec<std::ffi::OsString> =
                        systemd_args(*cap, spec.scope_unit.as_deref())
                            .into_iter()
                            .map(Into::into)
                            .collect();
                    a.push(spec.program.clone().into_os_string());
                    (exe.clone().into_os_string(), a)
                }
                _ => (spec.program.clone().into_os_string(), Vec::new()),
            };
        // procfix5 (gate4 m1): the spawn barrier. The read end is inherited by the child (kept across its exec into the
        // shim); the write end stays here (close-on-exec) until `release_barrier`. A dead caller = EOF = no program.
        // procfix6 (gate5 m2): the release token (32 random bytes; the shim gets only their BLAKE3).
        let token = if spec.spawn_barrier {
            Some(release_token().map_err(|error| LaunchError::Io {
                what: "spawn barrier token (getrandom)".into(),
                error,
            })?)
        } else {
            None
        };
        let barrier = if spec.spawn_barrier {
            let mut fds = [0 as libc::c_int; 2];
            // SAFETY: pipe2 writes two descriptors into the array.
            if unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) } != 0 {
                return Err(LaunchError::Io {
                    what: "spawn barrier pipe".into(),
                    error: last_err(),
                });
            }
            // SAFETY: two fresh descriptors owned by nobody else.
            let (r, w) = unsafe {
                use std::os::fd::FromRawFd;
                (
                    std::os::fd::OwnedFd::from_raw_fd(fds[0]),
                    std::os::fd::OwnedFd::from_raw_fd(fds[1]),
                )
            };
            Some((r, w))
        } else {
            None
        };
        // procfix7 (gate7 m-br): the shim's REPORT PIPE (write end to the shim only; read end here)
        let report_pipe = if barrier.is_some() {
            let mut fds = [0 as libc::c_int; 2];
            // SAFETY: pipe2 writes two descriptors into the array.
            if unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC | libc::O_NONBLOCK) } != 0 {
                return Err(LaunchError::Io {
                    what: "spawn barrier report pipe".into(),
                    error: last_err(),
                });
            }
            // SAFETY: two fresh descriptors owned by nobody else.
            Some(unsafe {
                use std::os::fd::FromRawFd;
                (
                    std::os::fd::OwnedFd::from_raw_fd(fds[0]),
                    std::os::fd::OwnedFd::from_raw_fd(fds[1]),
                )
            })
        } else {
            None
        };
        let mut cmd = match &barrier {
            Some((r, _)) => {
                let mut c = Command::new(crate::resources::procfs::SELF_EXE);
                let hash = token
                    .as_ref()
                    .map(|t| blake3::hash(t).to_hex().to_string())
                    .unwrap_or_default();
                c.arg(super::BARRIER_VERB)
                    .arg(r.as_raw_fd().to_string())
                    .arg(hash);
                if let Some((_, w)) = &report_pipe {
                    c.arg("--report").arg(w.as_raw_fd().to_string());
                }
                c.arg("--").arg(&prog);
                c
            }
            None => Command::new(&prog),
        };
        cmd.args(lead.drain(..));
        cmd.args(&spec.args).current_dir(&spec.cwd).env_clear();
        for (k, v) in &spec.env {
            cmd.env(k, v);
        }
        for name in &spec.env_passthrough {
            match std::env::var_os(name) {
                Some(v) => {
                    cmd.env(name, v);
                }
                None => return Err(LaunchError::MissingEnv(name.clone())),
            }
        }
        if matches!(mechanism_choice, Chosen::Scope(..)) {
            for (k, v) in systemd_env() {
                if !spec.env.iter().any(|(a, _)| a == k.as_str()) {
                    cmd.env(k, v);
                }
            }
        }
        cmd.env(MARKER_VAR, marker.value());
        let open = |p: &std::path::Path| {
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(p)
                .map_err(|error| LaunchError::Io {
                    what: format!("open {}", p.display()),
                    error,
                })
        };
        cmd.stdin(Stdio::null())
            .stdout(open(&spec.stdout)?)
            .stderr(open(&spec.stderr)?);

        // The memory mechanism is decided before a tracking-only leaf is added.
        let mechanism_kind = match &mechanism_choice {
            Chosen::Cgroup(c) => MemoryMechanism::Cgroup(c.path().to_path_buf()),
            Chosen::Scope(..) => MemoryMechanism::SystemdScope { cgroup: None },
            Chosen::Sampling => MemoryMechanism::Sampling,
            Chosen::None => MemoryMechanism::None,
        };
        let in_scope = matches!(mechanism_choice, Chosen::Scope(..));
        let cgroup: Option<JobCgroup> = match mechanism_choice {
            Chosen::Cgroup(c) => Some(c),
            // A tracking-only leaf in a delegated subtree (verification `Cgroup`). Not for a job that systemd-run moves
            // into its own scope: the leaf would be left at once and prove nothing.
            _ if !in_scope => self
                .tracking
                .as_ref()
                .and_then(|t| t.create_tracking_leaf(&spec.label).ok()),
            _ => None,
        };
        let procs_fd = cgroup.as_ref().map(|c| c.procs_fd());

        // SAFETY: getpid has no arguments and cannot fail.
        let parent = unsafe { libc::getpid() };
        let pin: Option<libc::cpu_set_t> = if spec.cpus.is_empty() {
            None
        } else {
            // SAFETY: zeroed cpu_set_t is the empty set; CPU_SET indices were validated < CPU_SETSIZE.
            let mut set: libc::cpu_set_t = unsafe { std::mem::zeroed() };
            for &c in &spec.cpus {
                // SAFETY: c < CPU_SETSIZE (validate).
                unsafe { libc::CPU_SET(c, &mut set) };
            }
            Some(set)
        };
        let rlimit_as = spec.rlimit_as_bytes;
        let mut keep = spec.inherit_fds.clone();
        if let Some((r, _)) = &barrier {
            keep.push(r.as_raw_fd());
        }
        if let Some((_, w)) = &report_pipe {
            keep.push(w.as_raw_fd());
        }
        let fd_limit = nofile_limit();
        let pdeathsig = spec.pdeathsig;
        // SAFETY: the closure only calls child_setup, which uses async-signal-safe syscalls and does not allocate;
        // all captured data was prepared in the parent.
        unsafe {
            cmd.pre_exec(move || {
                child_setup(
                    parent,
                    pin.as_ref(),
                    rlimit_as,
                    &keep,
                    fd_limit,
                    procs_fd,
                    pdeathsig,
                )
            });
        }
        // Spawn and register under the leader-registry lock: the stray reaper (which takes the same lock) can never
        // reap a leader before it is registered.
        let leaders = Arc::clone(self.sampler.leaders());
        let spawned = {
            let mut reg = leaders.lock();
            match cmd.spawn() {
                Ok(c) => {
                    reg.insert(c.id());
                    Ok(c)
                }
                Err(e) => Err(e),
            }
        };
        let child = match spawned {
            Ok(c) => c,
            Err(e) => {
                if let Some(cg) = cgroup {
                    cg.remove(); // audit A m11: no leaked job cgroup
                }
                return Err(LaunchError::Spawn(e));
            }
        };
        let pid = child.id();
        // The child has its copy of the barrier's read end (or is gone): ours is closed now.
        let barrier_w = barrier.and_then(|(r, w)| {
            drop(r);
            token.map(|t| (w, t))
        });
        // the shim holds the report pipe's write end now: ours is closed (EOF once the shim execs or exits)
        let report_r = report_pipe.map(|(r, w)| {
            drop(w);
            r
        });
        // Our unreaped child cannot be recycled, so reading its identity now is race-free (bounded: audit A M5).
        let identity = match run_bounded(Duration::from_secs(5), move || {
            ProcessIdentity::capture(pid)
        }) {
            crate::resources::Bounded::Done(Ok(i)) => i,
            other => {
                #[allow(clippy::let_underscore_must_use)]
                // ignored-ok (procfix7): the child keeps the marker: if this kill fails, the sweeps find and kill it
                let _ =
                    guard::with_context("launch: identity unreadable", Some(&spec.label), || {
                        signal_pinned_group(&leaders, pid, libc::SIGKILL)
                    });
                leaders.lock().remove(&pid);
                reap_in_background(child);
                if let Some(cg) = cgroup {
                    cg.kill_all();
                }
                return Err(LaunchError::Identity(match other {
                    crate::resources::Bounded::Done(Err(e)) => e,
                    // gate3 M3: no helper thread = the supervisor's resource failure (EAGAIN / ENOMEM), typed as such
                    crate::resources::Bounded::SpawnFailed(e) => e,
                    _ => io::Error::new(io::ErrorKind::TimedOut, "identity read timed out"),
                }));
            }
        };
        self.sampler.track(pid);
        let mechanism = mechanism_kind;
        let mut h = RealChild::new(
            spec.label.clone(),
            marker,
            identity,
            Some(child),
            Arc::clone(&self.sampler),
            self.config.clone(),
            spec.memory_cap_bytes,
            spec.rlimit_as_bytes,
            cgroup,
            mechanism,
        );
        h.enumeration = self.setup.enumeration;
        h.scope_unit = spec.scope_unit.clone();
        h.barrier = barrier_w;
        h.barrier_report = report_r;
        Ok(h)
    }

    /// RESTART-TIME call (it performs one blocking /proc scan; never call it from the running control loop).
    ///
    /// Rules (audit A C1, audit B C1-C3, M1, M2): the recorded pid must be a valid target pid (else `Err` with an
    /// `InvalidPid` payload, before any other syscall); a record of another boot is `Lost(OtherBoot)` and nothing is
    /// signalled; the job's processes are found ONLY by this campaign's marker with the record's label as attempt id,
    /// our uid, and a start time not earlier than the recorded leader's. The leader counts only if it has exactly the
    /// recorded start time AND leads its own session. If nothing carries the marker, the job is `Lost` (with the
    /// reason the recorded pid gives: Gone / Recycled / NotOurs). `Adopt` never signals; `Kill` terminates through the
    /// per-process checklist.
    fn reattach(
        &mut self,
        recorded: &ProcessIdentity,
        label: &str,
        choice: ReattachChoice,
    ) -> io::Result<Reattached<RealChild>> {
        ValidPid::for_signal(i64::from(recorded.pid)).map_err(InvalidPid::into_io)?;
        if recorded.boot_id != boot_id()? {
            return Ok(Reattached::Lost(LostReason::OtherBoot));
        }
        let marker = CampaignMarker::new(&self.config.campaign_id, label)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e.to_string()))?;
        let scan = scan_campaign(
            &self.config.campaign_id,
            recorded.start_ticks,
            &BTreeSet::new(),
        )?;
        // procfix7 round 4 (gate8 m8-reattach, the /proc class): a candidate whose environ could not be read (a timed-out
        // or refused read under load) may be a member: UNDECIDED (an error, try again), never "not a member" (it made a
        // live adopted job Lost(Gone): the rb_recover flake)
        if scan.unreadable > 0 {
            return Err(io::Error::other(format!(
                "{REATTACH_UNDECIDED}: {} candidate(s) of our uid whose environ cannot be read may carry this attempt's marker ({}); nothing was signalled; try again",
                scan.unreadable,
                crate::resources::describe_hidden(&scan.hidden)
            )));
        }
        let members: Vec<&MarkedProcess> = scan
            .processes
            .iter()
            .filter(|p| p.attempt_id == marker.attempt_id)
            .collect();
        if members.is_empty() {
            let reason = match recorded.check()? {
                crate::resources::Liveness::Gone => LostReason::Gone,
                crate::resources::Liveness::Recycled { .. } => LostReason::Recycled,
                crate::resources::Liveness::OtherBoot => LostReason::OtherBoot,
                crate::resources::Liveness::Alive { zombie: true } => LostReason::Gone,
                crate::resources::Liveness::Alive { zombie: false } => LostReason::NotOurs {
                    why: "the recorded process does not carry this attempt's campaign marker"
                        .into(),
                },
            };
            return Ok(Reattached::Lost(reason));
        }
        let leader_alive = members.iter().any(|p| {
            p.pid == recorded.pid && p.start_ticks == recorded.start_ticks && p.session == p.pid
        });
        let mut h = RealChild::new(
            label.to_string(),
            marker,
            recorded.clone(),
            None,
            Arc::clone(&self.sampler),
            self.config.clone(),
            None,
            None,
            None,
            MemoryMechanism::None,
        );
        h.enumeration = Enumeration::FullProc;
        h.adopted_members = members.iter().map(|p| (p.pid, p.start_ticks)).collect();
        if !leader_alive {
            h.exit = Some(ExitState::Unknown);
            h.exit_seen_at = Some(Instant::now());
        }
        self.sampler.request_now();
        if let ReattachChoice::Kill { grace } = choice {
            h.kill_requested = true;
            h.terminate(grace, KillReason::Reattach);
        }
        Ok(Reattached::Live(h))
    }
}

enum Chosen {
    None,
    Sampling,
    Cgroup(JobCgroup),
    Scope(PathBuf, u64),
}

/// A child of [`RealLauncher`] (or an adopted job of a previous supervisor).
pub struct RealChild {
    /// procfix5 m1: the write end of the spawn barrier while the child waits behind it (see `LaunchSpec::spawn_barrier`),
    /// with the release token (procfix6 m2).
    barrier: Option<(std::os::fd::OwnedFd, [u8; RELEASE_TOKEN_BYTES])>,
    /// procfix7 (gate7 m-br): the read end of the shim's report pipe, and what it said (read once, at the first report).
    barrier_report: Option<std::os::fd::OwnedFd>,
    barrier_refused: std::cell::OnceCell<Option<String>>,
    label: String,
    marker: CampaignMarker,
    identity: ProcessIdentity,
    sid: u32,
    /// None for an adopted job (not ours to wait for), or after the pin was lost (ECHILD: audit A M4).
    child: Option<Child>,
    sampler: Arc<Sampler>,
    leaders: Arc<Leaders>,
    config: RealConfig,
    launched_at: Instant,
    memory_cap: Option<u64>,
    rlimit_as: Option<u64>,
    cgroup: Option<JobCgroup>,
    mechanism: MemoryMechanism,
    exit: Option<ExitState>,
    exit_seen_at: Option<Instant>,
    kill_reason: Option<KillReason>,
    /// Signals beyond monitoring are allowed (always for launched children; for adopted ones only after Kill or an
    /// explicit terminate / kill_now / signal_group).
    kill_requested: bool,
    sigterm_sent: bool,
    grace_deadline: Option<Instant>,
    term_signalled: BTreeSet<(u32, u64)>,
    sigkill_at: Option<Instant>,
    leftovers: Vec<LeftoverRecord>,
    verify_started: Option<Instant>,
    last_usage: Option<SessionUsage>,
    last_usage_at: Option<Instant>,
    /// Marked processes of this attempt outside the session (escapees) in the latest fresh scan.
    marked_outside: Vec<MarkedProcess>,
    /// Members of an adopted job (pid, start) at adoption.
    adopted_members: BTreeSet<(u32, u64)>,
    over_cap_seq: Option<u64>,
    scope_dir: Option<PathBuf>,
    /// The job's systemd scope, recorded from the pinned leader (procfix3 belt; see `ScopeRef`).
    scope: Option<ScopeRef>,
    /// The deterministic unit name the scope was created with (gate3 M5), if any.
    scope_unit: Option<String>,
    /// procfix7 round 9 (gate13 G13-M1): the kernel's record of the cgroup the leader exited in, read once when its exit
    /// is first observed (still unreaped), if a scope was requested and not verified before.
    exit_scope: Option<ExitScope>,
    peak_rss: u64,
    cpu_ticks: u64,
    finished: Option<FinalReport>,
    reaped: bool,
    /// How this job's processes are enumerated for the end-of-job proof.
    enumeration: Enumeration,
    hunt: Option<HuntTask>,
    hunt_cancel: Arc<AtomicBool>,
    next_hunt_at: Option<Instant>,
    /// procfix7 round 6 (gate10 M10-2): hunts after an unverified end so far (the re-hunt interval doubles, capped).
    rehunts: u32,
    last_hunt_reason: String,
    last_hunt_left: Vec<LeftoverRecord>,
    unattributed: Vec<UnattributedChild>,
}

impl RealChild {
    #[allow(clippy::too_many_arguments)]
    fn new(
        label: String,
        marker: CampaignMarker,
        identity: ProcessIdentity,
        child: Option<Child>,
        sampler: Arc<Sampler>,
        config: RealConfig,
        memory_cap: Option<u64>,
        rlimit_as: Option<u64>,
        cgroup: Option<JobCgroup>,
        mechanism: MemoryMechanism,
    ) -> Self {
        let leaders = Arc::clone(sampler.leaders());
        let launched = child.is_some();
        Self {
            barrier: None,
            barrier_report: None,
            barrier_refused: std::cell::OnceCell::new(),
            label,
            marker,
            sid: identity.pid,
            identity,
            child,
            sampler,
            leaders,
            config,
            launched_at: Instant::now(),
            memory_cap,
            rlimit_as,
            cgroup,
            mechanism,
            exit: None,
            exit_seen_at: None,
            kill_reason: None,
            kill_requested: launched,
            sigterm_sent: false,
            grace_deadline: None,
            term_signalled: BTreeSet::new(),
            sigkill_at: None,
            leftovers: Vec::new(),
            verify_started: None,
            last_usage: None,
            last_usage_at: None,
            marked_outside: Vec::new(),
            adopted_members: BTreeSet::new(),
            over_cap_seq: None,
            scope_dir: None,
            scope: None,
            scope_unit: None,
            exit_scope: None,
            peak_rss: 0,
            cpu_ticks: 0,
            finished: None,
            reaped: false,
            enumeration: Enumeration::SubreaperTree,
            hunt: None,
            hunt_cancel: Arc::new(AtomicBool::new(false)),
            next_hunt_at: None,
            rehunts: 0,
            last_hunt_reason: String::new(),
            last_hunt_left: Vec::new(),
            unattributed: Vec::new(),
        }
    }

    /// procfix6 (gate5 m5): the containment in force for this job, as VERIFIED: a systemd scope only once the leader's own
    /// cgroup names the job's unit under our user manager (the strict `ScopeRef` check), else `Pending` while a scope
    /// was requested, or the job's delegated cgroup, or the marker only.
    pub fn containment(&mut self) -> Containment {
        if test_scope_pending().is_some() {
            return Containment::Pending;
        }
        match &self.mechanism {
            MemoryMechanism::SystemdScope { .. } => {
                if self.scope.is_none() {
                    #[allow(clippy::let_underscore_must_use)]
                    // ignored-ok (procfix7): memory accounting only
                    let _ = self.scope_memory();
                }
                match &self.scope {
                    Some(s) => Containment::Scope(
                        s.dir
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("?")
                            .to_string(),
                        s.dir.clone(),
                    ),
                    None => Containment::Pending,
                }
            }
            MemoryMechanism::Cgroup(p) => Containment::Cgroup(p.clone()),
            MemoryMechanism::Sampling => {
                Containment::MarkerOnly("no systemd scope (memory cap by sampling)".into())
            }
            MemoryMechanism::None => Containment::MarkerOnly("no memory cap, no scope".into()),
        }
    }

    /// The scope unit requested for this job (`systemd-run --unit=`), if the scope mechanism was chosen.
    pub fn requested_scope_unit(&self) -> Option<&str> {
        if let Some(u) = test_scope_pending() {
            return Some(u);
        }
        matches!(self.mechanism, MemoryMechanism::SystemdScope { .. })
            .then_some(self.scope_unit.as_deref())
            .flatten()
    }

    /// procfix7 round 8b: has the job's leader exited (its exit observed; the zombie may still be pinned)?
    pub fn leader_exited(&self) -> bool {
        self.exit.is_some()
    }

    /// procfix7 round 9 (gate13 G13-M1): the kernel's record of the cgroup the leader EXITED in (read once, from the
    /// unreaped zombie, when its exit was first observed), for a requested scope that was not verified before. None:
    /// no scope requested, the scope was verified, or the exit not observed yet.
    pub fn scope_at_exit(&self) -> Option<&ExitScope> {
        self.exit_scope.as_ref()
    }

    /// procfix7 round 9: read the kernel's record of the leader's cgroup now that its exit was observed (the leader is
    /// still our UNREAPED child: its pid cannot be reused). Reads only; signals nothing.
    fn record_exit_scope(&mut self) {
        if self.exit_scope.is_some() || self.scope.is_some() {
            return;
        }
        let Some(unit) = self.requested_scope_unit().map(str::to_string) else {
            return;
        };
        let uid = crate::resources::our_uid();
        match test_exit_override() {
            Some(1) => {
                // TEST BUILDS ONLY (`scope_exit:1`): the kernel's line as it reads for a scope removed after the exit
                let text = format!(
                    "0::/user.slice/user-{uid}.slice/user@{uid}.service/app.slice/{unit}.scope (deleted)\n"
                );
                self.exit_scope = Some(ExitScope::from_cgroup_text(&text, uid, &unit));
                return;
            }
            Some(_) => {
                self.exit_scope = Some(ExitScope::Unknown("test failpoint scope_exit:2".into()));
                return;
            }
            None => {}
        }
        self.exit_scope = Some(if !self.pinned() {
            ExitScope::Unknown("the leader is not pinned any more".into())
        } else {
            match crate::resources::procfs::sys().cgroup_text(self.sid, Duration::from_millis(200))
            {
                Ok(text) => ExitScope::from_cgroup_text(&text, uid, &unit),
                Err(e) => ExitScope::Unknown(format!("the leader's cgroup file: {e}")),
            }
        });
    }

    /// Is the child still waiting behind its spawn barrier (procfix5 m1)?
    pub fn behind_barrier(&self) -> bool {
        self.barrier.is_some()
    }

    /// Let the child run its program: the release token on the barrier pipe, then close it (procfix5 m1, procfix6 m2).
    /// Call it only after the spawn is recorded durably. No barrier: Ok (nothing to do). An error (the shim is gone,
    /// EPIPE) is reported; the child's end is then classified as usual.
    pub fn release_barrier(&mut self) -> io::Result<()> {
        let Some((w, token)) = self.barrier.take() else {
            return Ok(());
        };
        // One write of 32 bytes (< PIPE_BUF: atomic on a pipe).
        // SAFETY: a 32-byte buffer we own, written to our own pipe descriptor.
        let r = unsafe { libc::write(w.as_raw_fd(), token.as_ptr().cast(), token.len()) };
        if r == token.len() as isize {
            Ok(())
        } else if r < 0 {
            Err(last_err())
        } else {
            Err(io::Error::other(format!(
                "short write of the release token ({r} bytes)"
            )))
        }
    }

    /// Never release the barrier: close it, so the shim exits (`BARRIER_NOT_RELEASED`) without executing the program.
    pub fn cancel_barrier(&mut self) {
        self.barrier = None;
    }

    /// The session id (= the leader's pid).
    pub fn session(&self) -> u32 {
        self.sid
    }

    pub fn is_adopted(&self) -> bool {
        self.child.is_none()
    }

    pub fn marker(&self) -> &CampaignMarker {
        &self.marker
    }

    pub fn memory_mechanism(&self) -> &MemoryMechanism {
        &self.mechanism
    }

    fn launch_ticks(&self) -> u64 {
        self.identity.start_ticks
    }

    fn pinned(&self) -> bool {
        self.child.is_some() && !self.reaped
    }

    /// The latest snapshot if it began after this handle existed and is not stale.
    fn fresh_snapshot(&self) -> Option<Arc<Snapshot>> {
        let s = self.sampler.latest()?;
        let age = Instant::now().saturating_duration_since(s.finished_at);
        (s.started_at >= self.launched_at && age <= self.config.stale_after).then_some(s)
    }

    fn target(&self, pid: u32, start: StartRule, proof: Proof) -> ProcessTarget {
        ProcessTarget {
            pid,
            boot_id: self.identity.boot_id.clone(),
            start,
            require_session_leader: false,
            proof,
        }
    }

    /// Processes to signal individually, from the latest scan: members of our pinned session outside the leader's
    /// group (session proof), and every marked process of this attempt not covered that way (marker proof).
    fn individual_targets(&self, snap: Option<&Snapshot>) -> Vec<(ProcessTarget, (u32, u64))> {
        let mut out = Vec::new();
        let Some(snap) = snap else { return out };
        let launch = self.launch_ticks();
        let mut covered: BTreeSet<(u32, u64)> = BTreeSet::new();
        if self.pinned()
            && let Some(u) = snap.session(self.sid)
        {
            for m in &u.members {
                covered.insert((m.pid, m.start_ticks));
                if m.pgrp == self.sid || m.pid == self.sid {
                    continue; // reached by the pinned group signal
                }
                out.push((
                    self.target(
                        m.pid,
                        StartRule::NotBefore(launch),
                        Proof::PinnedSession(self.sid),
                    ),
                    (m.pid, m.start_ticks),
                ));
            }
        }
        for p in snap.marked_attempt(&self.marker, launch) {
            if covered.contains(&(p.pid, p.start_ticks)) {
                continue;
            }
            if self.pinned() && p.session == self.sid && p.pgrp == self.sid {
                continue;
            }
            let start = if p.pid == self.identity.pid {
                StartRule::Exact(self.identity.start_ticks)
            } else {
                StartRule::NotBefore(launch)
            };
            // THE ONE OWNERSHIP RULE (gate3 M2, lane int5): the same target as the campaign sweep and the hunt.
            let Some(t) = guard::marker_target(
                &self.marker.campaign_id,
                Some(&self.marker.attempt_id),
                p.pid,
                start,
                &self.identity.boot_id,
            ) else {
                continue;
            };
            out.push((t, (p.pid, p.start_ticks)));
        }
        out
    }

    /// Signal the whole job: the pinned leader group (fast path) and every other known process through the checklist.
    /// Returns (deliveries, identities delivered to).
    fn signal_job(&mut self, sig: i32, only_new: bool) -> usize {
        // gate3 M2: every delivery is logged (the supervisor records it) with this job's label.
        let label = self.label.clone();
        guard::with_context("job signal", Some(&label), || {
            self.signal_job_inner(sig, only_new)
        })
    }

    fn signal_job_inner(&mut self, sig: i32, only_new: bool) -> usize {
        let mut n = 0;
        if self.pinned()
            && !only_new
            && signal_pinned_group(&self.leaders, self.sid, sig).delivered()
        {
            n += 1;
        }
        let snap = self.sampler.latest();
        let (targets, keys): (Vec<ProcessTarget>, Vec<(u32, u64)>) = self
            .individual_targets(snap.as_deref())
            .into_iter()
            .filter(|(_, key)| !(only_new && self.term_signalled.contains(key)))
            .unzip();
        // One helper thread for the whole batch, one bound (gate re-audit A m-3).
        let verdicts = guard::signal_verified_batch(
            targets,
            sig,
            Some(Arc::clone(&self.leaders)),
            self.config.signal_verify_timeout,
        );
        for (v, key) in verdicts.iter().zip(keys) {
            if v.delivered() {
                n += 1;
                if sig == libc::SIGTERM {
                    self.term_signalled.insert(key);
                }
            }
        }
        n
    }

    fn sigkill_all(&mut self) {
        self.signal_job(libc::SIGKILL, false);
        if let Some(cg) = &self.cgroup {
            guard::with_context("job signal", Some(&self.label), || cg.kill_all());
        }
        self.sigkill_at = Some(Instant::now());
        self.sampler.request_now();
    }

    /// The spec of an end-of-job hunt for this job.
    fn hunt_spec(&self) -> HuntSpec {
        let pinned = self.pinned();
        HuntSpec {
            sid: self.sid,
            launch_ticks: self.launch_ticks(),
            marker: self.marker.clone(),
            boot_id: self.identity.boot_id.clone(),
            pinned,
            kill: self.kill_requested,
            enumeration: if pinned {
                self.enumeration
            } else {
                Enumeration::FullProc
            },
            cgroup: self.cgroup.as_ref().map(|c| c.path().to_path_buf()),
            scope: if pinned { self.scope.clone() } else { None },
            leaders: Arc::clone(&self.leaders),
            budget: self.config.hunt_budget,
            max_rounds: 1_000_000,
            pass_gap: self.config.pass_gap,
            stop_wait: Duration::from_millis(500),
        }
    }

    /// procfix7 round 6 (gate10 M10-2): the interval before the next hunt of an unverified end: `rehunt_every` doubled
    /// after every hunt that did not prove the end, capped at 64 x (5 s -> 320 s by default).
    fn rehunt_delay(&mut self) -> Duration {
        let d = self.config.rehunt_every * (1u32 << self.rehunts.min(6));
        self.rehunts = self.rehunts.saturating_add(1);
        d
    }

    fn start_hunt(&mut self) {
        let spec = self.hunt_spec();
        let cancel = Arc::clone(&self.hunt_cancel);
        let (tx, rx) = mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("rr-hunt".into())
            .spawn(move || {
                let r = hunt::run(&spec, &cancel);
                #[allow(clippy::let_underscore_must_use)]
                // ignored-ok (procfix7): the receiver is gone only when the launcher shut down
                let _ = tx.send(r);
            });
        if spawned.is_ok() {
            self.hunt = Some(HuntTask { rx });
        }
    }

    /// The result of the running hunt, if it finished.
    fn poll_hunt(&mut self) -> Option<HuntResult> {
        let t = self.hunt.as_ref()?;
        match t.rx.try_recv() {
            Ok(r) => {
                self.hunt = None;
                Some(r)
            }
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => {
                self.hunt = None;
                None
            }
        }
    }

    fn merge_killed(&mut self, killed: Vec<LeftoverRecord>) {
        for k in killed {
            if !self
                .leftovers
                .iter()
                .any(|l| l.pid == k.pid && l.start_ticks == k.start_ticks)
            {
                self.leftovers.push(k);
            }
        }
    }

    fn merge_unattributed(&mut self, u: Vec<UnattributedChild>) {
        self.unattributed.extend(first_report_of(u));
    }

    /// Reap the leader (under the registry lock) and untrack, after a verified-empty job.
    fn reap_and_untrack(&mut self) {
        if let Some(mut c) = self.child.take() {
            let mut reg = self.leaders.lock();
            if self.exit.is_some() {
                #[allow(clippy::let_underscore_must_use)]
                // ignored-ok (procfix7): reaping a zombie (immediate); its exit was already observed
                let _ = c.try_wait(); // a zombie: immediate
            }
            reg.remove(&self.sid);
            drop(reg);
            reap_in_background(c);
        }
        self.reaped = true;
        self.sampler.untrack(self.sid);
        if let Some(cg) = self.cgroup.take() {
            cg.remove();
        }
    }

    fn report(&self, verification: Verification) -> FinalReport {
        FinalReport {
            label: self.label.clone(),
            identity: self.identity.clone(),
            marker: self.marker.clone(),
            exit: self.exit,
            kill_reason: self.kill_reason.clone(),
            sigterm_sent: self.sigterm_sent,
            sigkill_sent: self.sigkill_at.is_some(),
            leftovers_killed: self.leftovers.clone(),
            escaped: self
                .last_usage
                .as_ref()
                .map(|u| u.escapees.clone())
                .unwrap_or_default(),
            unattributed_children: self.unattributed.clone(),
            session_verified_empty: verification.is_verified(),
            verification,
            peak_rss_bytes: self.peak_rss,
            cpu_seconds: self.cpu_ticks as f64 / ticks_per_second(),
            wall_seconds: self.launched_at.elapsed().as_secs_f64(),
            memory_mechanism: self.mechanism.clone(),
            rlimit_as_bytes: self.rlimit_as,
            barrier_refused: self.barrier_refused().clone(),
        }
    }

    /// procfix7 (gate7 m-br): what the shim wrote on its report pipe (`R<reason>`: it refused the release), read once
    /// (non-blocking: the shim has exited or exec'd by the time a report is made).
    fn barrier_refused(&self) -> &Option<String> {
        self.barrier_refused.get_or_init(|| {
            let fd = self.barrier_report.as_ref()?;
            let mut buf = Vec::new();
            let mut chunk = [0u8; 512];
            loop {
                // SAFETY: a read into our stack buffer from our own non-blocking descriptor.
                let n =
                    unsafe { libc::read(fd.as_raw_fd(), chunk.as_mut_ptr().cast(), chunk.len()) };
                if n > 0 && buf.len() < 4096 {
                    buf.extend_from_slice(&chunk[..n as usize]);
                    continue;
                }
                if n < 0 && io::Error::last_os_error().raw_os_error() == Some(libc::EINTR) {
                    continue;
                }
                // EOF, EAGAIN or an error: no refusal read = "the program may have executed" (the conservative
                // reading: a refusal is only ever taken from bytes the shim wrote)
                break;
            }
            buf.strip_prefix(b"R")
                .map(|r| String::from_utf8_lossy(r).trim().to_string())
        })
    }

    fn finish_verified(&mut self, by: VerifiedBy) -> FinalReport {
        self.reap_and_untrack();
        let r = self.report(Verification::Verified { by });
        self.finished = Some(r.clone());
        r
    }

    /// Give up waiting (verify timeout or D state): final SIGKILL to what is known, keep the zombie unreaped and the
    /// session tracked (the handle keeps working on later polls), report Unverified with what is left (audit A M2, m3).
    fn finish_unverified(&mut self, left: Vec<LeftoverRecord>, reason: String) -> FinalReport {
        self.sigkill_all();
        let r = self.report(Verification::Unverified {
            leftovers: left,
            reason,
        });
        self.finished = Some(r.clone());
        r
    }

    /// Read the job cgroup's memory.current / memory.events for the systemd-scope mechanism.
    fn scope_memory(&mut self) -> Option<(u64, Option<u64>)> {
        if !matches!(self.mechanism, MemoryMechanism::SystemdScope { .. }) || !self.pinned() {
            return None;
        }
        if self.scope_dir.is_none() {
            // procfix8: an unreadable cgroup file leaves the scope unknown for now (read again at the next sample)
            if let Ok(text) =
                crate::resources::procfs::sys().cgroup_text(self.sid, Duration::from_millis(200))
            {
                // The belt needs the strict form (this leader's own transient scope, our user manager, dev/inode).
                if self.scope.is_none() {
                    self.scope = ScopeRef::from_leader_cgroup(
                        &text,
                        self.sid,
                        crate::resources::our_uid(),
                        self.scope_unit.as_deref(),
                    );
                }
                // the scope's cgroup: systemd-run's anonymous run-p<pid>-i<n>.scope, or our named unit (gate3 M5)
                let named = self.scope_unit.as_deref().map(|u| format!("/{u}.scope"));
                if let Some(rel) = text.lines().find_map(|l| l.strip_prefix("0::"))
                    && rel.trim_end().ends_with(".scope")
                    && (rel.contains("/run-")
                        || named
                            .as_deref()
                            .is_some_and(|n| rel.trim_end().ends_with(n)))
                {
                    let dir = Path::new("/sys/fs/cgroup").join(rel.trim().trim_start_matches('/'));
                    self.mechanism = MemoryMechanism::SystemdScope {
                        cgroup: Some(dir.clone()),
                    };
                    self.scope_dir = Some(dir);
                }
            }
        }
        let dir = self.scope_dir.as_ref()?;
        // procfix7 round 5: an unreadable memory.current leaves the scope's usage UNKNOWN now (read again at the next
        // sample; the sampled usage applies meanwhile); an unreadable OOM count is unknown (None), never 0
        let crate::fsx::Read::Content(cur) =
            crate::fsx::read_to_string("cgroup", &dir.join("memory.current"))
        else {
            return None;
        };
        let cur = cur.trim().parse::<u64>().ok()?;
        let oom = match crate::fsx::read_to_string("cgroup", &dir.join("memory.events")) {
            crate::fsx::Read::Content(t) => t
                .lines()
                .find_map(|l| l.strip_prefix("oom_kill "))
                .and_then(|v| v.trim().parse().ok()),
            crate::fsx::Read::Absent | crate::fsx::Read::Failed(_) => None,
        };
        Some((cur, oom))
    }

    /// Usage of an adopted job (or one whose pin was lost) from the marked processes of a snapshot.
    fn marked_usage(&self, snap: &Snapshot) -> SessionUsage {
        let mut u = SessionUsage {
            session: self.sid,
            ..Default::default()
        };
        for p in snap.marked_attempt(&self.marker, self.launch_ticks()) {
            u.groups.insert(p.pgrp);
            *u.group_rss_bytes.entry(p.pgrp).or_default() += p.rss_bytes;
            u.rss_bytes = u.rss_bytes.saturating_add(p.rss_bytes);
            u.cpu_ticks = u.cpu_ticks.saturating_add(p.cpu_ticks);
            u.members.push(marked_to_stat(p));
        }
        u
    }
}

fn marked_to_stat(p: &MarkedProcess) -> ProcStat {
    ProcStat {
        pid: p.pid,
        comm: p.comm.clone(),
        state: p.state,
        ppid: p.ppid,
        pgrp: p.pgrp,
        session: p.session,
        cpu_ticks: p.cpu_ticks,
        start_ticks: p.start_ticks,
        rss_bytes: p.rss_bytes,
    }
}

impl ChildHandle for RealChild {
    fn label(&self) -> &str {
        &self.label
    }

    fn identity(&self) -> &ProcessIdentity {
        &self.identity
    }

    fn poll_exit(&mut self) -> io::Result<Option<ExitState>> {
        if self.exit.is_some() || self.finished.is_some() {
            return Ok(self.exit);
        }
        let e = if self.child.is_some() {
            match peek_exit(self.identity.pid) {
                Ok(e) => e,
                Err(err) if err.raw_os_error() == Some(libc::ECHILD) => {
                    // Audit A M4: auto-reaped (SIGCHLD ignored behind our back): the number is no longer pinned.
                    // Drop the Child without waiting and fall back to marker-verified signalling.
                    let mut reg = self.leaders.lock();
                    reg.remove(&self.sid);
                    drop(reg);
                    self.child = None;
                    Some(ExitState::Unknown)
                }
                Err(err) => return Err(err),
            }
        } else {
            // Adopted: from the sampler's marked list (no per-pid read on the control thread; audit A M5).
            match self.fresh_snapshot() {
                Some(s) => {
                    let alive = s.marked.iter().any(|p| {
                        p.pid == self.identity.pid && p.start_ticks == self.identity.start_ticks
                    });
                    if alive {
                        None
                    } else {
                        Some(ExitState::Unknown)
                    }
                }
                None => None,
            }
        };
        if e.is_some() {
            self.exit = e;
            self.exit_seen_at = Some(Instant::now());
            self.sampler.request_now();
            self.record_exit_scope();
        }
        Ok(e)
    }

    fn usage(&self) -> Option<SessionUsage> {
        self.last_usage.clone()
    }

    fn usage_reading(&self) -> UsageReading {
        let age = self.last_usage_at.map(|t| t.elapsed());
        match (&self.last_usage, age) {
            (Some(u), Some(a)) if a <= self.config.stale_after => UsageReading::Fresh {
                usage: u.clone(),
                age: a,
            },
            _ => UsageReading::Stale {
                last: self.last_usage.clone(),
                age,
            },
        }
    }

    fn signal_group(&mut self, signal: i32) -> io::Result<SignalOutcome> {
        if self.finished.is_some() {
            return Ok(SignalOutcome::AlreadyGone);
        }
        self.kill_requested = true;
        let n = self.signal_job(signal, false);
        Ok(if n == 0 {
            SignalOutcome::AlreadyGone
        } else {
            SignalOutcome::Delivered { groups: n }
        })
    }

    fn terminate(&mut self, grace: Duration, reason: KillReason) {
        if self.finished.is_some() {
            return;
        }
        // Audit B m3: a job whose leader already exited on its own is not "interrupted"; leftovers are handled by VERIFY.
        if self.exit.is_some() && self.child.is_some() {
            return;
        }
        self.kill_requested = true;
        if self.kill_reason.is_none() {
            self.kill_reason = Some(reason);
        }
        let deadline = Instant::now() + grace;
        self.grace_deadline = Some(self.grace_deadline.map_or(deadline, |d| d.min(deadline)));
        if !self.sigterm_sent {
            self.sigterm_sent = true;
            self.signal_job(libc::SIGTERM, false);
            self.signal_job(libc::SIGCONT, false);
        }
        self.sampler.request_now();
    }

    fn kill_now(&mut self, reason: KillReason) {
        if self.finished.is_some() {
            return;
        }
        if self.exit.is_some() && self.child.is_some() {
            return;
        }
        self.kill_requested = true;
        if self.kill_reason.is_none() {
            self.kill_reason = Some(reason);
        }
        self.sigkill_all();
    }

    fn poll(&mut self) -> io::Result<ChildProgress> {
        if let Some(r) = &self.finished {
            if r.verification.is_verified() {
                return Ok(ChildProgress::Finished(r.clone()));
            }
            // Unverified: keep hunting (helper thread) every `rehunt_every`; upgrade to Verified once proven.
            let now = Instant::now();
            #[allow(clippy::let_underscore_must_use)]
            // ignored-ok (procfix7): the hunt below decides (an unverified end keeps hunting)
            let _ = self.poll_exit();
            match self.poll_hunt() {
                Some(HuntResult::Proven {
                    by,
                    killed,
                    unattributed,
                }) => {
                    self.merge_killed(killed);
                    self.merge_unattributed(unattributed);
                    return Ok(ChildProgress::Finished(self.finish_verified(by)));
                }
                Some(HuntResult::NotProven {
                    reason,
                    left,
                    killed,
                    unattributed,
                }) => {
                    self.merge_killed(killed);
                    self.merge_unattributed(unattributed);
                    self.next_hunt_at = Some(now + self.rehunt_delay());
                    self.finished = Some(self.report(Verification::Unverified {
                        leftovers: left,
                        reason,
                    }));
                }
                Some(HuntResult::Survivors { left }) => {
                    self.next_hunt_at = Some(now + self.rehunt_delay());
                    self.finished = Some(self.report(Verification::Unverified {
                        leftovers: left,
                        reason: "the job still has live processes (monitored, not killed)".into(),
                    }));
                }
                None => {}
            }
            if self.hunt.is_none()
                && self.exit.is_some()
                && self.next_hunt_at.is_none_or(|t| now >= t)
            {
                self.next_hunt_at = Some(now + self.rehunt_delay());
                self.start_hunt();
            }
            return Ok(ChildProgress::Finished(
                self.finished.clone().unwrap_or_else(|| {
                    self.report(Verification::Unverified {
                        leftovers: Vec::new(),
                        reason: "unknown".into(),
                    })
                }),
            ));
        }
        let now = Instant::now();
        self.poll_exit()?;
        let snap = self.fresh_snapshot();
        if let Some(s) = snap.as_deref() {
            let u = if self.pinned() {
                s.session(self.sid).cloned()
            } else {
                Some(self.marked_usage(s))
            };
            if let Some(u) = u {
                // Peak memory: the larger of RSS (stat, kernel counters that may lag) and PSS (page walk).
                self.peak_rss = self.peak_rss.max(u.rss_bytes).max(u.pss_bytes.unwrap_or(0));
                self.cpu_ticks = self.cpu_ticks.max(u.cpu_ticks);
                self.last_usage = Some(u);
                self.last_usage_at = Some(s.finished_at);
            }
            let launch = self.launch_ticks();
            self.marked_outside = s
                .marked_attempt(&self.marker, launch)
                .filter(|p| !(self.pinned() && p.session == self.sid))
                .cloned()
                .collect();
        }
        // Memory cap: cgroup / systemd scope (kernel-enforced; an OOM kill is read back), else sampled.
        let cap_hit = matches!(self.kill_reason, Some(KillReason::MemoryCap { .. }));
        if !cap_hit && let Some(cap) = self.memory_cap {
            let scope = self.scope_memory();
            // an OOM count that is unknown now is no kill trigger now (it is read again at the next sample)
            let oom = self
                .cgroup
                .as_ref()
                .is_some_and(|c| c.oom_kills().is_some_and(|n| n > 0))
                || scope.is_some_and(|(_, o)| o.is_some_and(|n| n > 0));
            let sampled = snap.as_deref().and_then(|s| {
                let u = self.last_usage.as_ref()?;
                let (m, kind) = u.mem_bytes();
                // Setsid escapees count against the cap too (audit B C4c).
                let esc: u64 = self.marked_outside.iter().map(|p| p.rss_bytes).sum();
                Some((m.saturating_add(esc), kind, s.seq))
            });
            let over = match (scope, sampled) {
                (Some((cur, _)), _) => (cur > cap).then_some((cur, true)),
                (None, Some((m, kind, seq))) if m > cap => {
                    // Under the RSS fallback (shared / CoW pages counted twice: audit B m2) a kill needs two
                    // consecutive over-cap scans; PSS or a cgroup number kills at the first.
                    let confirmed =
                        kind != MemKind::Rss || self.over_cap_seq.is_some_and(|p| p != seq);
                    if self.over_cap_seq.is_none() {
                        self.over_cap_seq = Some(seq);
                    }
                    Some((m, confirmed))
                }
                (None, Some(_)) => {
                    self.over_cap_seq = None;
                    None
                }
                (None, None) => None,
            };
            if oom || over.is_some_and(|(_, c)| c) {
                let rss = over.map_or(self.peak_rss, |(m, _)| m);
                self.kill_reason = Some(KillReason::MemoryCap {
                    rss_bytes: rss.max(self.peak_rss),
                    cap_bytes: cap,
                });
                self.kill_requested = true;
                if self.exit.is_none() {
                    self.sigkill_all();
                }
            }
        }
        // Escalation.
        match (self.sigkill_at, self.grace_deadline) {
            (None, Some(d)) if now >= d => self.sigkill_all(),
            (None, Some(_)) => {
                // Processes a later scan names get SIGTERM + SIGCONT at once.
                let before = self.term_signalled.len();
                self.signal_job(libc::SIGTERM, true);
                if self.term_signalled.len() > before {
                    self.signal_job(libc::SIGCONT, false);
                }
            }
            _ => {}
        }
        // Leader stuck after SIGKILL beyond the verify timeout (D state): report Unverified, keep it pinned.
        if self.exit.is_none() {
            if let Some(k) = self.sigkill_at
                && now.duration_since(k) > self.config.verify_timeout
            {
                let left = vec![LeftoverRecord {
                    pid: self.identity.pid,
                    start_ticks: self.identity.start_ticks,
                    pgrp: self.sid,
                    comm: "<leader>".into(),
                }];
                return Ok(ChildProgress::Finished(self.finish_unverified(
                    left,
                    "the leader did not exit after SIGKILL (D state?)".into(),
                )));
            }
            return Ok(if self.sigterm_sent || self.sigkill_at.is_some() {
                ChildProgress::Stopping
            } else {
                ChildProgress::Running
            });
        }
        // The leader exited. A requested grace is honoured before anything is frozen or killed.
        let started = *self.verify_started.get_or_insert(now);
        if let Some(d) = self.grace_deadline
            && self.sigkill_at.is_none()
            && now < d
        {
            return Ok(ChildProgress::Stopping);
        }
        // The end-of-job hunt proves the job gone (helper thread; the leader stays unreaped meanwhile).
        match self.poll_hunt() {
            Some(HuntResult::Proven {
                by,
                killed,
                unattributed,
            }) => {
                self.merge_killed(killed);
                self.merge_unattributed(unattributed);
                return Ok(ChildProgress::Finished(self.finish_verified(by)));
            }
            Some(HuntResult::Survivors { .. }) => {
                // Adopt never kills: the job is still running without its leader.
                self.next_hunt_at = Some(now + self.rehunt_delay());
                return Ok(ChildProgress::Running);
            }
            Some(HuntResult::NotProven {
                reason,
                left,
                killed,
                unattributed,
            }) => {
                self.merge_killed(killed);
                self.merge_unattributed(unattributed);
                self.last_hunt_reason = reason;
                self.last_hunt_left = left;
                if now.duration_since(started) > self.config.verify_timeout {
                    let (left, reason) = (
                        std::mem::take(&mut self.last_hunt_left),
                        self.last_hunt_reason.clone(),
                    );
                    return Ok(ChildProgress::Finished(
                        self.finish_unverified(left, reason),
                    ));
                }
            }
            None => {
                if self.hunt.is_some() && now.duration_since(started) > self.config.verify_timeout {
                    // The hunt itself is stuck (a /proc read blocked in the kernel): report, keep the zombie pinned.
                    let reason = format!(
                        "end-of-job hunt did not finish within {:?} (last: {})",
                        self.config.verify_timeout, self.last_hunt_reason
                    );
                    let left = self.last_hunt_left.clone();
                    return Ok(ChildProgress::Finished(
                        self.finish_unverified(left, reason),
                    ));
                }
            }
        }
        if self.hunt.is_none() {
            if self.next_hunt_at.is_some_and(|t| now < t) {
                return Ok(if self.kill_requested {
                    ChildProgress::Stopping
                } else {
                    ChildProgress::Running
                });
            }
            self.next_hunt_at = None;
            self.start_hunt();
        }
        // An adopted job that is only monitored is still running while its (observe-only) hunt looks.
        Ok(if self.kill_requested {
            ChildProgress::Stopping
        } else {
            ChildProgress::Running
        })
    }
}

impl Drop for RealChild {
    /// A handle dropped while its child runs: SIGKILL the pinned group (never leave an unsupervised child behind) and
    /// hand the reap to the adopted-children reaper (never block in Drop, also not during unwinding: audit A M5). A hunt
    /// still running is cancelled.
    fn drop(&mut self) {
        self.hunt_cancel.store(true, Ordering::SeqCst);
        if let Some(c) = self.child.take() {
            if !self.reaped {
                let finished_verified = self
                    .finished
                    .as_ref()
                    .is_some_and(|r| r.verification.is_verified());
                if !finished_verified {
                    let (leaders, sid, cg) = (&self.leaders, self.sid, self.cgroup.as_ref());
                    guard::with_context("job handle dropped unproven", Some(&self.label), || {
                        #[allow(clippy::let_underscore_must_use)]
                        // ignored-ok (procfix7): a dropped unproven handle: best effort; the restart's sweep finds what is left by its marker
                        let _ = signal_pinned_group(leaders, sid, libc::SIGKILL);
                        if let Some(cg) = cg {
                            cg.kill_all();
                        }
                    });
                }
            }
            let mut reg = self.leaders.lock();
            reg.remove(&self.sid);
            drop(reg);
            reap_in_background(c);
            self.sampler.untrack(self.sid);
        }
    }
}

/// Used by `CgroupRoot` users: fd of an open file.
pub(super) fn raw(f: &fs::File) -> RawFd {
    f.as_raw_fd()
}

/// procfix6 (gate5 m5): the containment of a job's marker-dropping descendants actually in force.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Containment {
    /// The job's transient systemd scope (unit name, e.g. `rr-<id>-<label>-<hex>.scope`), verified from the leader's cgroup,
    /// and (procfix7 round 7, gate11 m11-rec-path) its verified cgroup directory, recorded for the restart backstop.
    Scope(String, PathBuf),
    /// A delegated job cgroup this supervisor created.
    Cgroup(PathBuf),
    /// A scope was requested and is not verified (yet).
    Pending,
    /// Only the marker (and the session) contain the job's processes; why.
    MarkerOnly(String),
}

impl Containment {
    /// The ledger / log word: `scope:<unit>`, `cgroup:<path>`, `pending`, `marker_only`.
    pub fn word(&self) -> String {
        match self {
            Containment::Scope(u, _) => format!("scope:{u}"),
            Containment::Cgroup(p) => format!("cgroup:{}", p.display()),
            Containment::Pending => "pending".into(),
            Containment::MarkerOnly(_) => "marker_only".into(),
        }
    }
}

/// Bytes of the spawn barrier's release token (procfix6 m2).
pub(crate) const RELEASE_TOKEN_BYTES: usize = 32;

/// A fresh release token from the kernel's random source.
fn release_token() -> io::Result<[u8; RELEASE_TOKEN_BYTES]> {
    let mut t = [0u8; RELEASE_TOKEN_BYTES];
    let mut n = 0;
    while n < t.len() {
        // SAFETY: getrandom writes at most `len - n` bytes into our buffer.
        let r = unsafe { libc::getrandom(t[n..].as_mut_ptr().cast(), t.len() - n, 0) };
        if r < 0 {
            let e = last_err();
            if e.raw_os_error() == Some(libc::EINTR) {
                continue;
            }
            return Err(e);
        }
        n += r as usize;
    }
    Ok(t)
}

/// procfix6 (gate5 m2): make THIS process non-dumpable (`PR_SET_DUMPABLE` 0): its /proc entries (fd, environ, mem, ...)
/// become accessible only to itself and to holders of CAP_SYS_PTRACE (another process of the same uid can no longer
/// open its descriptors through /proc/<pid>/fd), and it leaves no core dump. Inherited by fork, reset by execve (a job's
/// program is dumpable again). The supervisor calls it when it hosts the spawn barrier (it holds the barrier pipes).
pub fn set_not_dumpable() -> io::Result<()> {
    // SAFETY: prctl with integer arguments only.
    if unsafe { libc::prctl(libc::PR_SET_DUMPABLE, 0 as libc::c_ulong, 0, 0, 0) } != 0 {
        return Err(last_err());
    }
    Ok(())
}

/// The barrier shim (procfix5 m1), run by the hidden verb [`super::BARRIER_VERB`] of a binary that hosts the supervisor:
/// `args` = `<fd> <blake3 hex> -- <program> <args...>`. procfix6 (gate5 m2): first makes itself non-dumpable (so no other
/// process of the uid can reach the pipe through /proc/<pid>/fd from now on), then reads the release token (32 bytes) from
/// `fd` (the supervisor writes it after recording the spawn durably), closes it and execs the program in place with the
/// current environment ONLY if the token's BLAKE3 equals the hex in its argv (a write by anyone else, also one made
/// through a descriptor opened before the prctl, cannot produce it). EOF, a short or wrong token: exit
/// [`super::BARRIER_NOT_RELEASED`] without executing anything. Returns only on failure (the exit code to use).
pub fn barrier_shim(args: &[String]) -> i32 {
    use std::io::Write;
    use std::os::unix::process::CommandExt;
    // Before anything else (the descriptor is reachable through /proc only until here).
    #[allow(clippy::let_underscore_must_use)]
    // ignored-ok (procfix7): the release token is the protection (procfix6 m2, documented window)
    let _ = set_not_dumpable();
    // procfix7 (gate7 m-br): optional `--report <fd>` before `--`: the report pipe (close-on-exec at once, so the
    // program never inherits it).
    let (report, rest) = match args.get(2).map(String::as_str) {
        Some("--report") => (args.get(3).and_then(|s| s.parse::<RawFd>().ok()), 4),
        _ => (None, 2),
    };
    if let Some(r) = report.filter(|&r| r >= 3) {
        // SAFETY: fcntl on an inherited descriptor with integer arguments.
        unsafe { libc::fcntl(r, libc::F_SETFD, libc::FD_CLOEXEC) };
    }
    let (Some(fd), Some(hash), Some("--"), Some(prog)) = (
        args.first().and_then(|s| s.parse::<RawFd>().ok()),
        args.get(1),
        args.get(rest).map(String::as_str),
        args.get(rest + 1),
    ) else {
        #[allow(clippy::let_underscore_must_use)]
        // ignored-ok (procfix7): the shim's message on stderr (diagnostics only)
        let _ = writeln!(
            std::io::stderr(),
            "{}: usage <fd> <blake3 hex> [--refused <file>] -- <program> [args]",
            super::BARRIER_VERB
        );
        return 2;
    };
    // The program was NOT started: say so on the report pipe (only the launcher reads it; the program never has it).
    let record_refusal = |why: &str| {
        if let Some(r) = report.filter(|&r| r >= 3) {
            let msg = format!("R{why}");
            // SAFETY: a write of our own buffer to the inherited report descriptor (best effort: without it the
            // refusal is classified as a crash, the more conservative class).
            unsafe { libc::write(r, msg.as_ptr().cast(), msg.len()) };
        }
    };
    if fd < 3 {
        return 2;
    }
    let mut b = [0u8; RELEASE_TOKEN_BYTES];
    let mut n = 0usize;
    let failed = loop {
        if n == b.len() {
            break false;
        }
        // SAFETY: a read of at most `len - n` bytes into our stack buffer from an inherited descriptor.
        let r = unsafe { libc::read(fd, b[n..].as_mut_ptr().cast(), b.len() - n) };
        if r < 0 && io::Error::last_os_error().raw_os_error() == Some(libc::EINTR) {
            continue;
        }
        if r <= 0 {
            break true;
        }
        n += r as usize;
    };
    // SAFETY: closing the inherited barrier descriptor (the program must not inherit it).
    unsafe { libc::close(fd) };
    if failed && n == 0 {
        #[allow(clippy::let_underscore_must_use)]
        // ignored-ok (procfix7): the shim's message on stderr (diagnostics only)
        let _ = writeln!(
            std::io::stderr(),
            "{}: the supervisor ended before it recorded this spawn: the program was not started",
            super::BARRIER_VERB
        );
        record_refusal(
            "eof: no release token (the supervisor ended or cancelled, or another process took the token)",
        );
        return super::BARRIER_NOT_RELEASED;
    }
    if failed || blake3::hash(&b).to_hex().as_str() != hash.as_str() {
        #[allow(clippy::let_underscore_must_use)]
        // ignored-ok (procfix7): the shim's message on stderr (diagnostics only)
        let _ = writeln!(
            std::io::stderr(),
            "{}: the barrier was released with a wrong token ({n} byte(s); another process wrote into the barrier pipe?): the program was not started",
            super::BARRIER_VERB
        );
        record_refusal(&format!(
            "wrong token: {n} byte(s) that are not the release token (another process wrote into the barrier pipe?)"
        ));
        return super::BARRIER_NOT_RELEASED;
    }
    let e = std::process::Command::new(prog)
        .args(&args[rest + 2..])
        .exec();
    #[allow(clippy::let_underscore_must_use)]
    // ignored-ok (procfix7): the shim's message on stderr (diagnostics only)
    let _ = writeln!(
        std::io::stderr(),
        "{}: exec {prog}: {e}",
        super::BARRIER_VERB
    );
    127
}

#[cfg(test)]
mod tests {
    use super::*;

    /// procfix7 round 9 (gate13 G13-M1): systemd-run's start acknowledgment in its documented forms, each a whole line
    /// naming EXACTLY the requested unit; every near miss is no acknowledgment.
    #[test]
    fn the_start_acknowledgment_is_recognised_in_its_documented_forms_for_the_exact_unit_only() {
        let u = "rr-camp.0123456789abcdef-N0.i0001.L0-t0_a1-00112233aabbccdd";
        let id = "645e842e0123456789abcdef01234567";
        let yes = [
            (
                format!("Running as unit: {u}.scope; invocation ID: {id}\n"),
                "Running as unit: <unit>.scope; invocation ID: <id>",
            ),
            (
                format!("Running as unit: {u}.scope\n"),
                "Running as unit: <unit>.scope",
            ),
            (
                format!("Running scope as unit: {u}.scope\r\n"),
                "Running scope as unit: <unit>.scope",
            ),
            (
                format!("job output\nRunning as unit: {u}.scope; invocation ID: {id}\nmore\n"),
                "Running as unit: <unit>.scope; invocation ID: <id>",
            ),
        ];
        for (t, form) in &yes {
            assert_eq!(scope_acknowledgment(t, u), Some(*form), "{t:?}");
        }
        let no = [
            format!("Running as unit: {u}x.scope; invocation ID: {id}\n"),
            format!("Running as unit: x{u}.scope\n"),
            format!("Running as unit: {u}\n"),
            format!("Running as unit: {u}.service\n"),
            format!("Running as unit: {u}.scope; invocation ID: {}\n", &id[..31]),
            format!("Running as unit: {u}.scope; invocation ID: {id}0\n"),
            format!(
                "Running as unit: {u}.scope; invocation ID: {}zz\n",
                &id[..30]
            ),
            format!("Running as unit: {u}.scope; extra\n"),
            format!("x Running as unit: {u}.scope\n"),
            format!("Running timer as unit: {u}.scope\n"),
            format!("Will run service as unit: {u}.scope\n"),
            String::new(),
        ];
        for t in &no {
            assert_eq!(scope_acknowledgment(t, u), None, "{t:?}");
        }
    }

    /// procfix7 round 9: the bounded stdout read ends with the command's output (the round-8b form never saw EOF and
    /// timed out). Spawns `printf`: run with LIGHT_PIDNS=1 (rule L10).
    #[test]
    fn the_bounded_stdout_read_sees_the_end_of_the_output() {
        if !in_private_namespace() {
            eprintln!("SKIPPED: spawns a process: only inside a private pid namespace (rule L10)");
            return;
        }
        let Some(exe) = which("printf") else {
            eprintln!("SKIPPED: no printf on PATH");
            return;
        };
        let t0 = Instant::now();
        let out = stdout_bounded(
            exe,
            vec!["LoadState=loaded\\nActiveState=failed\\nResult=oom-kill\\n".into()],
            Duration::from_secs(10),
        )
        .unwrap();
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "LoadState=loaded\nActiveState=failed\nResult=oom-kill\n"
        );
        assert!(t0.elapsed() < Duration::from_secs(5), "{:?}", t0.elapsed());
    }

    /// procfix7 round 9: only one literal unit name of ours reaches systemctl (no glob, no option, no path).
    #[test]
    fn only_literal_unit_names_reach_systemctl() {
        assert!(exact_unit_name("rr-camp.0123-N0.i0001.L0-t0_a1-00112233aabbccdd").is_ok());
        for bad in [
            "", "rr-*", "rr-?x", "rr-[ab]", "-rr", "rr x", "rr/x", "rr\\x", "rr@x", "rr:x",
        ] {
            if bad == "-rr" {
                // a leading '-' is a valid character of ours, but systemctl gets `--` before the name
                assert!(exact_unit_name(bad).is_ok());
                continue;
            }
            assert!(exact_unit_name(bad).is_err(), "{bad:?}");
        }
        assert!(exact_unit_name(&"a".repeat(241)).is_err());
    }
}
