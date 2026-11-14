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

use super::guard::{
    self, DEFAULT_VERIFY_TIMEOUT, ProcessTarget, Proof, StartRule, signal_pinned_group,
    signal_verified,
};
use super::{
    CapRequest, ChildHandle, ChildLauncher, ChildProgress, ExitState, FinalReport, JobCgroup,
    KillReason, LaunchError, LaunchSpec, LeftoverRecord, LostReason, MemoryMechanism,
    ReattachChoice, Reattached, SignalOutcome, UsageReading, Verification,
};
use crate::resources::{
    CampaignMarker, InvalidPid, Leaders, MARKER_VAR, MarkedProcess, MemKind, ProcStat,
    ProcessIdentity, Sampler, SessionUsage, Snapshot, ValidPid, boot_id, read_bounded, run_bounded,
    scan_campaign, ticks_per_second,
};
use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io;
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
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
        }
    }
}

/// "adhoc-<pid>-<start ticks>" of this process (unique per supervisor instance within a boot).
pub fn adhoc_campaign_id() -> String {
    let me = std::process::id();
    let start = crate::resources::read_stat(me)
        .ok()
        .flatten()
        .map_or(0, |s| s.start_ticks);
    format!("adhoc-{me}-{start}")
}

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
}

pub struct RealLauncher {
    sampler: Arc<Sampler>,
    config: RealConfig,
    setup: LauncherSetup,
}

/// Find an executable on the supervisor's PATH.
fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|d| d.join(name))
        .find(|p| p.is_file())
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

fn systemd_args(cap: u64) -> Vec<String> {
    vec![
        "--user".into(),
        "--scope".into(),
        "--quiet".into(),
        "--collect".into(),
        "-p".into(),
        format!("MemoryMax={cap}"),
        "-p".into(),
        "MemorySwapMax=0".into(),
        "-p".into(),
        "OOMPolicy=kill".into(),
        "--".into(),
    ]
}

/// Are we inside a non-initial user or pid namespace? Then the host's user manager would interpret the pids that
/// systemd-run passes in ITS namespace, i.e. as other processes: the scope mechanism must not be used.
pub fn in_private_namespace() -> bool {
    // SAFETY: getpid cannot fail.
    if unsafe { libc::getpid() } == 1 {
        return true;
    }
    match fs::read_to_string("/proc/self/uid_map") {
        Ok(m) => {
            let f: Vec<&str> = m.split_whitespace().collect();
            f != ["0", "0", "4294967295"]
        }
        Err(_) => true,
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
        c.args(systemd_args(64 << 20))
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

    /// Process-wide effects (documented): SIGCHLD reset to SIG_DFL if inherited as ignored (audit A M4); child
    /// subreaper if configured; the sampler's campaign is set to `config.campaign_id`.
    pub fn with_config(sampler: Arc<Sampler>, config: RealConfig) -> Self {
        let sigchld =
            guard::ensure_sigchld_default().unwrap_or(guard::SigchldSetup::AlreadyDefault);
        let child_subreaper = config.child_subreaper && guard::set_child_subreaper(true).is_ok();
        if crate::resources::validate_campaign_id(&config.campaign_id).is_err() {
            // An invalid id would make every marker invalid: fall back to an ad-hoc one (recognises nothing old).
            let mut c = config.clone();
            c.campaign_id = adhoc_campaign_id();
            return Self::with_config(sampler, c);
        }
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
        };
        Self {
            sampler,
            config,
            setup,
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

    /// SIGKILL every process group of an unreaped leader of this launcher's sampler, at once (fast exit). Returns the
    /// number of groups signalled. Grandchildren in other groups or sessions are left to the restart's orphan scan.
    pub fn kill_all_pinned(&self) -> usize {
        guard::kill_all_pinned(self.sampler.leaders())
    }
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
    for entry in fs::read_dir("/proc/self/fd")? {
        let entry = entry?;
        let Some(fd) = entry
            .file_name()
            .to_str()
            .and_then(|s| s.parse::<RawFd>().ok())
        else {
            continue;
        };
        if fd < 3 {
            continue;
        }
        // SAFETY: fcntl F_GETFD on an integer; EBADF if it was the directory fd of this very listing, now closed.
        let fl = unsafe { libc::fcntl(fd, libc::F_GETFD) };
        if fl >= 0 && fl & libc::FD_CLOEXEC == 0 {
            let target = fs::read_link(entry.path())
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            out.push((fd, target));
        }
    }
    Ok(out)
}

/// Hand a `Child` that may not have exited yet to a helper thread that reaps it (never block the caller; audit A M5).
fn reap_in_background(mut c: Child) {
    if let Ok(Some(_)) = c.try_wait() {
        return;
    }
    let _ = std::thread::Builder::new()
        .name("rr-reaper".into())
        .spawn(move || {
            let _ = c.wait();
        });
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
        let mut cmd = match &mechanism_choice {
            Chosen::Scope(exe, cap) => {
                let mut c = Command::new(exe);
                c.args(systemd_args(*cap)).arg(&spec.program);
                c
            }
            _ => Command::new(&spec.program),
        };
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

        let cgroup: Option<JobCgroup> = match mechanism_choice {
            Chosen::Cgroup(c) => Some(c),
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
        let keep = spec.inherit_fds.clone();
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
        // Our unreaped child cannot be recycled, so reading its identity now is race-free (bounded: audit A M5).
        let identity = match run_bounded(Duration::from_secs(5), move || {
            ProcessIdentity::capture(pid)
        })
        .done()
        {
            Some(Ok(i)) => i,
            other => {
                let _ = signal_pinned_group(&leaders, pid, libc::SIGKILL);
                leaders.lock().remove(&pid);
                reap_in_background(child);
                if let Some(cg) = cgroup {
                    cg.kill_all();
                }
                return Err(LaunchError::Identity(match other {
                    Some(Err(e)) => e,
                    _ => io::Error::new(io::ErrorKind::TimedOut, "identity read timed out"),
                }));
            }
        };
        self.sampler.track(pid);
        let mechanism = match (
            &cgroup,
            &mechanism_choice_kind(
                &spec.memory_cap_bytes,
                &cgroup,
                &self.setup,
                spec.cap_mechanism,
            ),
        ) {
            (Some(c), _) => MemoryMechanism::Cgroup(c.path().to_path_buf()),
            (None, k) => k.clone(),
        };
        Ok(RealChild::new(
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
        ))
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

fn mechanism_choice_kind(
    cap: &Option<u64>,
    cgroup: &Option<JobCgroup>,
    setup: &LauncherSetup,
    req: CapRequest,
) -> MemoryMechanism {
    match (cap, cgroup) {
        (None, _) => MemoryMechanism::None,
        (Some(_), Some(c)) => MemoryMechanism::Cgroup(c.path().to_path_buf()),
        (Some(_), None) => match (req, &setup.systemd_run) {
            (CapRequest::Sampling, _) | (_, None) => MemoryMechanism::Sampling,
            (_, Some(_)) => MemoryMechanism::SystemdScope { cgroup: None },
        },
    }
}

/// A child of [`RealLauncher`] (or an adopted job of a previous supervisor).
pub struct RealChild {
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
    peak_rss: u64,
    cpu_ticks: u64,
    finished: Option<FinalReport>,
    reaped: bool,
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
            peak_rss: 0,
            cpu_ticks: 0,
            finished: None,
            reaped: false,
        }
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
            out.push((
                self.target(p.pid, start, Proof::Marker(self.marker.clone())),
                (p.pid, p.start_ticks),
            ));
        }
        out
    }

    /// Signal the whole job: the pinned leader group (fast path) and every other known process through the checklist.
    /// Returns (deliveries, identities delivered to).
    fn signal_job(&mut self, sig: i32, only_new: bool) -> usize {
        let mut n = 0;
        if self.pinned()
            && !only_new
            && signal_pinned_group(&self.leaders, self.sid, sig).delivered()
        {
            n += 1;
        }
        let snap = self.sampler.latest();
        for (t, key) in self.individual_targets(snap.as_deref()) {
            if only_new && self.term_signalled.contains(&key) {
                continue;
            }
            let v = signal_verified(
                &t,
                sig,
                Some(&self.leaders),
                self.config.signal_verify_timeout,
            );
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
            cg.kill_all();
        }
        self.sigkill_at = Some(Instant::now());
        self.sampler.request_now();
    }

    /// What is left of the job in a fresh snapshot: session members other than the (zombie) leader, plus marked
    /// processes of this attempt (escapees, double-forked daemons; audit B C4).
    fn survivors(&self, snap: &Snapshot) -> Vec<(ProcStat, bool)> {
        let launch = self.launch_ticks();
        let mut out: Vec<(ProcStat, bool)> = Vec::new();
        let mut seen: BTreeSet<(u32, u64)> = BTreeSet::new();
        if self.pinned()
            && let Some(u) = snap.session(self.sid)
        {
            for m in &u.members {
                // A zombie leader with live threads is reported as a live member by the scan (audit B M1): keep it.
                if m.pid == self.identity.pid && self.exit.is_some() && m.is_dead() {
                    continue;
                }
                if m.pid == self.identity.pid && !self.leader_has_live_threads(m) {
                    continue;
                }
                if m.start_ticks >= launch {
                    seen.insert((m.pid, m.start_ticks));
                    out.push((m.clone(), false));
                }
            }
        }
        for p in snap.marked_attempt(&self.marker, launch) {
            if seen.contains(&(p.pid, p.start_ticks)) {
                continue;
            }
            if self.pinned() && p.pid == self.identity.pid {
                continue; // our zombie leader is handled by the reap
            }
            out.push((marked_to_stat(p), true));
        }
        out
    }

    /// For our own leader: the scan lists it as a live member only while some thread lives (a zombie leader with a
    /// live thread still runs). For a pinned leader whose exit was observed by waitid, the thread group is dead.
    fn leader_has_live_threads(&self, m: &ProcStat) -> bool {
        self.exit.is_none() && !m.is_dead()
    }

    fn kill_leftovers(&mut self, survivors: &[(ProcStat, bool)]) {
        let launch = self.launch_ticks();
        if self.pinned() {
            let _ = signal_pinned_group(&self.leaders, self.sid, libc::SIGKILL);
        }
        for (m, marked) in survivors {
            let proof = if !marked && self.pinned() {
                Proof::PinnedSession(self.sid)
            } else {
                Proof::Marker(self.marker.clone())
            };
            let start = if m.pid == self.identity.pid {
                StartRule::Exact(self.identity.start_ticks)
            } else {
                StartRule::NotBefore(launch)
            };
            let t = self.target(m.pid, start, proof);
            let _ = signal_verified(
                &t,
                libc::SIGKILL,
                Some(&self.leaders),
                self.config.signal_verify_timeout,
            );
            if !self
                .leftovers
                .iter()
                .any(|l| l.pid == m.pid && l.start_ticks == m.start_ticks)
            {
                self.leftovers.push(LeftoverRecord {
                    pid: m.pid,
                    start_ticks: m.start_ticks,
                    pgrp: m.pgrp,
                    comm: m.comm.clone(),
                });
            }
        }
        if let Some(cg) = &self.cgroup {
            cg.kill_all();
        }
        self.sigkill_at = Some(Instant::now());
        self.sampler.request_now();
    }

    /// Reap the leader (under the registry lock) and untrack, after a verified-empty job.
    fn reap_and_untrack(&mut self) {
        if let Some(mut c) = self.child.take() {
            let mut reg = self.leaders.lock();
            if self.exit.is_some() {
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
            session_verified_empty: verification == Verification::Verified,
            verification,
            peak_rss_bytes: self.peak_rss,
            cpu_seconds: self.cpu_ticks as f64 / ticks_per_second(),
            wall_seconds: self.launched_at.elapsed().as_secs_f64(),
            memory_mechanism: self.mechanism.clone(),
            rlimit_as_bytes: self.rlimit_as,
        }
    }

    fn finish_verified(&mut self) -> FinalReport {
        self.reap_and_untrack();
        let r = self.report(Verification::Verified);
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
    fn scope_memory(&mut self) -> Option<(u64, u64)> {
        if !matches!(self.mechanism, MemoryMechanism::SystemdScope { .. }) || !self.pinned() {
            return None;
        }
        if self.scope_dir.is_none() {
            let path = PathBuf::from(format!("/proc/{}/cgroup", self.sid));
            if let crate::resources::ReadOutcome::Data(b) =
                read_bounded(&path, Duration::from_millis(200))
            {
                let text = String::from_utf8_lossy(&b);
                if let Some(rel) = text.lines().find_map(|l| l.strip_prefix("0::"))
                    && rel.trim_end().ends_with(".scope")
                    && rel.contains("/run-")
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
        let cur = fs::read_to_string(dir.join("memory.current"))
            .ok()?
            .trim()
            .parse::<u64>()
            .ok()?;
        let oom = fs::read_to_string(dir.join("memory.events"))
            .ok()
            .and_then(|t| {
                t.lines()
                    .find_map(|l| l.strip_prefix("oom_kill "))
                    .and_then(|v| v.trim().parse().ok())
            })
            .unwrap_or(0);
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
            if r.verification == Verification::Verified {
                return Ok(ChildProgress::Finished(r.clone()));
            }
            // Unverified: keep working on it (kills of verified leftovers; upgrade when empty).
            let snap = self.sampler.latest();
            let since = self.sigkill_at.unwrap_or(self.launched_at);
            if let Some(s) = snap.as_deref().filter(|s| s.started_at > since) {
                let surv = self.survivors(s);
                let leader_exited = self.exit.is_some() || self.child.is_none();
                if leader_exited && self.child.is_some() {
                    // leader still unreaped zombie: fine
                }
                if surv.is_empty() && self.exit.is_some() {
                    return Ok(ChildProgress::Finished(self.finish_verified()));
                }
                if !surv.is_empty() {
                    self.kill_leftovers(&surv);
                }
            }
            let _ = self.poll_exit();
            self.sampler.request_now();
            return Ok(ChildProgress::Finished(
                self.finished
                    .clone()
                    .unwrap_or_else(|| self.report(Verification::Verified)),
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
                self.peak_rss = self.peak_rss.max(u.rss_bytes);
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
            let oom = self.cgroup.as_ref().is_some_and(|c| c.oom_kills() > 0)
                || scope.is_some_and(|(_, o)| o > 0);
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
        // The leader exited: verify the job empty with a FRESH scan begun after the exit and after the last SIGKILL.
        let started = *self.verify_started.get_or_insert(now);
        let since = match (self.exit_seen_at, self.sigkill_at) {
            (Some(a), Some(b)) => a.max(b),
            (Some(a), None) => a,
            (None, Some(b)) => b,
            (None, None) => now,
        };
        let fresh = snap.as_deref().filter(|s| s.started_at > since);
        match fresh.map(|s| self.survivors(s)) {
            Some(survivors) if survivors.is_empty() => {
                return Ok(ChildProgress::Finished(self.finish_verified()));
            }
            Some(survivors) => {
                if !self.kill_requested && !self.pinned() {
                    // Adopt never kills: the job is still running without its leader.
                    return Ok(ChildProgress::Running);
                }
                if now.duration_since(started) > self.config.verify_timeout {
                    let left = survivors
                        .iter()
                        .map(|(m, _)| LeftoverRecord {
                            pid: m.pid,
                            start_ticks: m.start_ticks,
                            pgrp: m.pgrp,
                            comm: m.comm.clone(),
                        })
                        .collect();
                    return Ok(ChildProgress::Finished(self.finish_unverified(
                        left,
                        "job not empty at verify timeout".into(),
                    )));
                }
                self.kill_leftovers(&survivors);
            }
            None => {
                if now.duration_since(started) > self.config.verify_timeout {
                    return Ok(ChildProgress::Finished(self.finish_unverified(
                        Vec::new(),
                        "no fresh scan since the exit (sampler stale)".into(),
                    )));
                }
                self.sampler.request_now();
            }
        }
        Ok(ChildProgress::Stopping)
    }
}

impl Drop for RealChild {
    /// A handle dropped while its child runs: SIGKILL the pinned group (never leave an unsupervised child behind) and
    /// hand the reap to a helper thread (never block in Drop, also not during unwinding: audit A M5).
    fn drop(&mut self) {
        if let Some(c) = self.child.take() {
            if !self.reaped {
                let finished_verified = self
                    .finished
                    .as_ref()
                    .is_some_and(|r| r.verification == Verification::Verified);
                if !finished_verified {
                    let _ = signal_pinned_group(&self.leaders, self.sid, libc::SIGKILL);
                    if let Some(cg) = &self.cgroup {
                        cg.kill_all();
                    }
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
