//! The real launcher: children as ordinary processes in their own sessions (R6: no systemd unit, no agent harness).
//!
//! Spawn sequence in the forked child, before exec (`pre_exec`; only async-signal-safe syscalls, nothing allocates):
//! `setsid` -> (job cgroup: write "0" to its pre-opened `cgroup.procs`) -> `prctl(PR_SET_PDEATHSIG, SIGKILL)` then
//! `getppid() == recorded parent` else `_exit(126)` -> `sched_setaffinity` -> optional `setrlimit(RLIMIT_AS)` ->
//! `close_range(3, ~0, CLOSE_RANGE_CLOEXEC)` (fallback: fcntl loop) -> clear FD_CLOEXEC on the fds to inherit.
//!
//! PDEATHSIG race: the parent may die between fork and the prctl; the child would then never get the signal. Reading
//! `getppid()` AFTER the prctl closes it: if the parent already died, the child was re-parented and `getppid()` differs.
//! PDEATHSIG fires when the THREAD that forked exits, not the process: spawn from a thread that lives as long as the
//! supervisor (the control thread). It is not inherited by grandchildren; those die by the group kill, by their own
//! PDEATHSIG (child-job contract 3.4.6), or not at all if the supervisor is SIGKILLed (see process-layer.md, orphans).

use super::{
    ChildHandle, ChildLauncher, ChildProgress, ExitState, FinalReport, JobCgroup, KillReason,
    LaunchError, LaunchSpec, LeftoverRecord, LostReason, MemoryMechanism, ReattachChoice,
    Reattached, SignalOutcome,
};
use crate::resources::{
    Liveness, ProcessIdentity, Sampler, SessionUsage, Snapshot, read_stat, scan_sessions_now,
    ticks_per_second,
};
use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io;
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub struct RealConfig {
    /// After the leader exited: how long to wait for a scan showing the session empty (killing leftovers meanwhile)
    /// before reporting `session_verified_empty: false`.
    pub verify_timeout: Duration,
}

impl Default for RealConfig {
    fn default() -> Self {
        Self {
            verify_timeout: Duration::from_secs(120),
        }
    }
}

pub struct RealLauncher {
    sampler: Arc<Sampler>,
    config: RealConfig,
}

impl RealLauncher {
    /// The sampler must be shared with the control loop (one scanner per supervisor).
    pub fn new(sampler: Arc<Sampler>) -> Self {
        Self {
            sampler,
            config: RealConfig::default(),
        }
    }
    pub fn with_config(sampler: Arc<Sampler>, config: RealConfig) -> Self {
        Self { sampler, config }
    }
    pub fn sampler(&self) -> &Arc<Sampler> {
        &self.sampler
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
    (r.rlim_cur as u64).clamp(64, 1 << 20) as RawFd
}

/// Peek at a child's exit without reaping it (`waitid(P_PID, WEXITED | WNOHANG | WNOWAIT)`).
/// Only valid for direct children of this process.
pub fn peek_exit(pid: u32) -> io::Result<Option<ExitState>> {
    // SAFETY: siginfo_t is plain data; zeroed is valid and makes si_pid 0 when no child is waitable.
    let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
    // SAFETY: info is a valid out-pointer; P_PID with our child's pid.
    let r = unsafe {
        libc::waitid(
            libc::P_PID,
            pid as libc::id_t,
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

fn kill(pid: libc::pid_t, sig: libc::c_int) -> io::Result<()> {
    // SAFETY: kill takes plain integers.
    if unsafe { libc::kill(pid, sig) } == 0 {
        Ok(())
    } else {
        Err(last_err())
    }
}

fn getsid(pid: u32) -> Result<u32, i32> {
    // SAFETY: getsid takes a plain integer.
    let r = unsafe { libc::getsid(pid as libc::pid_t) };
    if r < 0 {
        Err(last_err().raw_os_error().unwrap_or(0))
    } else {
        Ok(r as u32)
    }
}

/// Signal the process group led by a RECORDED identity, after checking that identity now (pid, start, boot).
/// Refuses (`NotOurs`) when the start time or boot changed: a recycled pid is never signalled.
pub fn signal_group_by_identity(
    identity: &ProcessIdentity,
    signal: i32,
) -> io::Result<SignalOutcome> {
    match identity.check()? {
        Liveness::Alive { .. } => match kill(-(identity.pid as libc::pid_t), signal) {
            Ok(()) => Ok(SignalOutcome::Delivered { groups: 1 }),
            Err(e) if e.raw_os_error() == Some(libc::ESRCH) => Ok(SignalOutcome::AlreadyGone),
            Err(e) => Err(e),
        },
        Liveness::Gone => Ok(SignalOutcome::AlreadyGone),
        other => Ok(SignalOutcome::NotOurs(other)),
    }
}

impl ChildLauncher for RealLauncher {
    type Handle = RealChild;

    fn launch(&mut self, spec: &LaunchSpec) -> Result<RealChild, LaunchError> {
        spec.validate()?;
        let mut cmd = Command::new(&spec.program);
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

        let cgroup: Option<JobCgroup> = match (&spec.cgroup, spec.memory_cap_bytes) {
            (Some(root), Some(cap)) => Some(
                root.create_job(&spec.label, cap)
                    .map_err(LaunchError::Cgroup)?,
            ),
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
        let mut child = cmd.spawn().map_err(LaunchError::Spawn)?;
        let pid = child.id();
        // Our unreaped child cannot be recycled, so reading its identity now is race-free.
        let identity = match ProcessIdentity::capture(pid) {
            Ok(i) => i,
            Err(e) => {
                let _ = kill(-(pid as libc::pid_t), libc::SIGKILL);
                let _ = kill(pid as libc::pid_t, libc::SIGKILL);
                let _ = child.wait();
                return Err(LaunchError::Identity(e));
            }
        };
        self.sampler.track(pid);
        let mechanism = match (&cgroup, spec.memory_cap_bytes) {
            (Some(c), _) => MemoryMechanism::Cgroup(c.path().to_path_buf()),
            (None, Some(_)) => MemoryMechanism::Sampling,
            (None, None) => MemoryMechanism::None,
        };
        Ok(RealChild::new(
            spec.label.clone(),
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

    fn reattach(
        &mut self,
        recorded: &ProcessIdentity,
        label: &str,
        choice: ReattachChoice,
    ) -> io::Result<Reattached<RealChild>> {
        let leftovers_exist = |sid: u32| {
            let sids: BTreeSet<u32> = [sid].into();
            scan_sessions_now(&sids).get(&sid).is_some_and(|u| {
                u.members
                    .iter()
                    .any(|m| m.start_ticks >= recorded.start_ticks)
            })
        };
        let leader_gone = match recorded.check()? {
            Liveness::OtherBoot => return Ok(Reattached::Lost(LostReason::OtherBoot)),
            Liveness::Alive { zombie } => zombie,
            Liveness::Gone => {
                if !leftovers_exist(recorded.pid) {
                    return Ok(Reattached::Lost(LostReason::Gone));
                }
                true
            }
            Liveness::Recycled { .. } => {
                // If the new holder of the pid leads its own session, the session number is someone else's.
                let leads = read_stat(recorded.pid)?.is_some_and(|s| s.session == recorded.pid);
                if leads || !leftovers_exist(recorded.pid) {
                    return Ok(Reattached::Lost(LostReason::Recycled));
                }
                true
            }
        };
        self.sampler.track(recorded.pid);
        let mut h = RealChild::new(
            label.to_string(),
            recorded.clone(),
            None,
            Arc::clone(&self.sampler),
            self.config.clone(),
            None,
            None,
            None,
            MemoryMechanism::None,
        );
        if leader_gone {
            h.exit = Some(ExitState::Unknown);
            h.exit_seen_at = Some(Instant::now());
        }
        if let ReattachChoice::Kill { grace } = choice {
            h.terminate(grace, KillReason::Reattach);
        }
        Ok(Reattached::Live(h))
    }
}

/// A child of [`RealLauncher`] (or an adopted child of a previous supervisor).
pub struct RealChild {
    label: String,
    identity: ProcessIdentity,
    sid: u32,
    /// None for an adopted child (not ours to wait for).
    child: Option<Child>,
    sampler: Arc<Sampler>,
    config: RealConfig,
    launched_at: Instant,
    memory_cap: Option<u64>,
    rlimit_as: Option<u64>,
    cgroup: Option<JobCgroup>,
    mechanism: MemoryMechanism,
    exit: Option<ExitState>,
    exit_seen_at: Option<Instant>,
    kill_reason: Option<KillReason>,
    sigterm_sent: bool,
    grace_deadline: Option<Instant>,
    term_groups: BTreeSet<u32>,
    sigkill_at: Option<Instant>,
    leftovers: Vec<LeftoverRecord>,
    verify_started: Option<Instant>,
    last_usage: Option<SessionUsage>,
    peak_rss: u64,
    cpu_ticks: u64,
    finished: Option<FinalReport>,
}

impl RealChild {
    #[allow(clippy::too_many_arguments)]
    fn new(
        label: String,
        identity: ProcessIdentity,
        child: Option<Child>,
        sampler: Arc<Sampler>,
        config: RealConfig,
        memory_cap: Option<u64>,
        rlimit_as: Option<u64>,
        cgroup: Option<JobCgroup>,
        mechanism: MemoryMechanism,
    ) -> Self {
        Self {
            label,
            sid: identity.pid,
            identity,
            child,
            sampler,
            config,
            launched_at: Instant::now(),
            memory_cap,
            rlimit_as,
            cgroup,
            mechanism,
            exit: None,
            exit_seen_at: None,
            kill_reason: None,
            sigterm_sent: false,
            grace_deadline: None,
            term_groups: BTreeSet::new(),
            sigkill_at: None,
            leftovers: Vec::new(),
            verify_started: None,
            last_usage: None,
            peak_rss: 0,
            cpu_ticks: 0,
            finished: None,
        }
    }

    /// The session id (= the leader's pid).
    pub fn session(&self) -> u32 {
        self.sid
    }

    pub fn is_adopted(&self) -> bool {
        self.child.is_none()
    }

    /// May the leader's group number be signalled without a fresh identity read? True while we hold the unreaped
    /// child (a zombie or live child of ours cannot be recycled). For an adopted child: identity check now.
    fn leader_group_is_ours(&self) -> bool {
        if self.finished.is_some() {
            return false;
        }
        if self.child.is_some() {
            return true;
        }
        matches!(self.identity.check(), Ok(Liveness::Alive { .. }))
    }

    /// Is another group number of a scanned member still ours to signal? (guarded_run.py rule: signal unless
    /// getsid names another session; an adopted session is not pinned, so require getsid == sid there.)
    fn other_group_is_ours(&self, g: u32) -> bool {
        match getsid(g) {
            Ok(s) => s == self.sid,
            Err(e) => e == libc::ESRCH && self.child.is_some(),
        }
    }

    fn groups_from(&self, snap: Option<&Snapshot>) -> BTreeSet<u32> {
        let mut g: BTreeSet<u32> = snap
            .and_then(|s| s.session(self.sid))
            .map(|u| u.groups.clone())
            .unwrap_or_default();
        if let Some(u) = &self.last_usage {
            g.extend(u.groups.iter().copied());
        }
        g
    }

    /// Send a signal to the leader's group and to every other group of the session. Returns groups signalled.
    fn signal_session(&mut self, sig: libc::c_int, groups: &BTreeSet<u32>) -> usize {
        let mut n = 0;
        if self.leader_group_is_ours() && kill(-(self.sid as libc::pid_t), sig).is_ok() {
            n += 1;
        }
        for &g in groups.iter().filter(|&&g| g != self.sid) {
            if self.other_group_is_ours(g) && kill(-(g as libc::pid_t), sig).is_ok() {
                n += 1;
            }
        }
        n
    }

    fn sigkill_all(&mut self) {
        let snap = self.sampler.latest();
        let groups = self.groups_from(snap.as_deref());
        self.signal_session(libc::SIGKILL, &groups);
        if let Some(cg) = &self.cgroup {
            cg.kill_all();
        }
        self.sigkill_at = Some(Instant::now());
        self.sampler.request_now();
    }

    /// Members of the session in `snap` other than the (zombie) leader, started no earlier than the leader.
    fn survivors(&self, snap: &Snapshot) -> Option<Vec<crate::resources::ProcStat>> {
        let u = snap.session(self.sid)?;
        Some(
            u.members
                .iter()
                .filter(|m| {
                    m.pid != self.identity.pid && m.start_ticks >= self.identity.start_ticks
                })
                .cloned()
                .collect(),
        )
    }

    fn kill_leftovers(&mut self, survivors: &[crate::resources::ProcStat]) {
        let groups: BTreeSet<u32> = survivors.iter().map(|m| m.pgrp).collect();
        self.signal_session(libc::SIGKILL, &groups);
        for m in survivors {
            // getsid == sid: still a member of our session (pinned by our zombie leader, or checked for adopted).
            if getsid(m.pid) == Ok(self.sid) {
                let _ = kill(m.pid as libc::pid_t, libc::SIGKILL);
            }
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

    fn finish(&mut self, verified: bool) -> FinalReport {
        if let Some(mut c) = self.child.take() {
            if self.exit.is_some() {
                let _ = c.wait(); // immediate: the zombie is waiting
            }
            // else: the leader never exited (D state); dropping Child neither kills nor reaps.
        }
        self.sampler.untrack(self.sid);
        if let Some(cg) = self.cgroup.take() {
            cg.remove();
        }
        let r = FinalReport {
            label: self.label.clone(),
            identity: self.identity.clone(),
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
            session_verified_empty: verified,
            peak_rss_bytes: self.peak_rss,
            cpu_seconds: self.cpu_ticks as f64 / ticks_per_second(),
            wall_seconds: self.launched_at.elapsed().as_secs_f64(),
            memory_mechanism: self.mechanism.clone(),
            rlimit_as_bytes: self.rlimit_as,
        };
        self.finished = Some(r.clone());
        r
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
            peek_exit(self.identity.pid)?
        } else {
            match self.identity.check()? {
                Liveness::Alive { zombie: false } => None,
                _ => Some(ExitState::Unknown),
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

    fn signal_group(&mut self, signal: i32) -> io::Result<SignalOutcome> {
        if self.finished.is_some() {
            return Ok(SignalOutcome::AlreadyGone);
        }
        if self.child.is_none() {
            match self.identity.check()? {
                Liveness::Alive { .. } => {}
                Liveness::Gone => return Ok(SignalOutcome::AlreadyGone),
                other => return Ok(SignalOutcome::NotOurs(other)),
            }
        }
        let snap = self.sampler.latest();
        let groups = self.groups_from(snap.as_deref());
        let n = self.signal_session(signal, &groups);
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
        if self.kill_reason.is_none() {
            self.kill_reason = Some(reason);
        }
        let deadline = Instant::now() + grace;
        self.grace_deadline = Some(self.grace_deadline.map_or(deadline, |d| d.min(deadline)));
        if !self.sigterm_sent {
            self.sigterm_sent = true;
            let snap = self.sampler.latest();
            let groups = self.groups_from(snap.as_deref());
            self.signal_session(libc::SIGTERM, &groups);
            self.signal_session(libc::SIGCONT, &groups);
            self.term_groups = groups;
            self.term_groups.insert(self.sid);
        }
        self.sampler.request_now();
    }

    fn kill_now(&mut self, reason: KillReason) {
        if self.finished.is_some() {
            return;
        }
        if self.kill_reason.is_none() {
            self.kill_reason = Some(reason);
        }
        self.sigkill_all();
    }

    fn poll(&mut self) -> io::Result<ChildProgress> {
        if let Some(r) = &self.finished {
            return Ok(ChildProgress::Finished(r.clone()));
        }
        let now = Instant::now();
        self.poll_exit()?;
        let snap = self.sampler.latest();
        if let Some(s) = snap.as_deref() {
            if s.started_at >= self.launched_at {
                if let Some(u) = s.session(self.sid) {
                    self.peak_rss = self.peak_rss.max(u.rss_bytes);
                    self.cpu_ticks = self.cpu_ticks.max(u.cpu_ticks);
                    self.last_usage = Some(u.clone());
                }
            }
        }
        // Memory cap (sampling, or the cgroup's own OOM kill).
        let cap_hit = matches!(self.kill_reason, Some(KillReason::MemoryCap { .. }));
        if self.exit.is_none() && !cap_hit {
            if let Some(cap) = self.memory_cap {
                let rss = self.last_usage.as_ref().map_or(0, |u| u.rss_bytes);
                let oom = self.cgroup.as_ref().is_some_and(|c| c.oom_kills() > 0);
                if rss > cap || oom {
                    // A cap kill overrides a pending graceful stop: the job is over its reservation now.
                    self.kill_reason = Some(KillReason::MemoryCap {
                        rss_bytes: rss.max(self.peak_rss),
                        cap_bytes: cap,
                    });
                    self.sigkill_all();
                }
            }
        }
        // Escalation.
        match (self.sigkill_at, self.grace_deadline) {
            (None, Some(d)) if now >= d => self.sigkill_all(),
            (None, Some(_)) => {
                // Groups a later scan names get SIGTERM + SIGCONT at once.
                let groups = self.groups_from(snap.as_deref());
                let new: BTreeSet<u32> = groups.difference(&self.term_groups).copied().collect();
                if !new.is_empty() {
                    self.signal_session(libc::SIGTERM, &new);
                    self.signal_session(libc::SIGCONT, &new);
                    self.term_groups.extend(new);
                }
            }
            _ => {}
        }
        // Leader stuck after SIGKILL beyond the verify timeout (D state): give up, report.
        if self.exit.is_none() {
            if let Some(k) = self.sigkill_at {
                if now.duration_since(k) > self.config.verify_timeout {
                    return Ok(ChildProgress::Finished(self.finish(false)));
                }
            }
            return Ok(if self.sigterm_sent || self.sigkill_at.is_some() {
                ChildProgress::Stopping
            } else {
                ChildProgress::Running
            });
        }
        // The leader exited: verify the session empty with a scan begun after the exit and after the last SIGKILL.
        let started = *self.verify_started.get_or_insert(now);
        let since = match (self.exit_seen_at, self.sigkill_at) {
            (Some(a), Some(b)) => a.max(b),
            (Some(a), None) => a,
            (None, Some(b)) => b,
            (None, None) => now,
        };
        let fresh = snap.as_deref().filter(|s| s.started_at > since);
        match fresh.and_then(|s| self.survivors(s)) {
            Some(survivors) if survivors.is_empty() => {
                return Ok(ChildProgress::Finished(self.finish(true)));
            }
            Some(survivors) => {
                if now.duration_since(started) > self.config.verify_timeout {
                    return Ok(ChildProgress::Finished(self.finish(false)));
                }
                self.kill_leftovers(&survivors);
            }
            None => {
                if now.duration_since(started) > self.config.verify_timeout {
                    return Ok(ChildProgress::Finished(self.finish(false)));
                }
                self.sampler.request_now();
            }
        }
        Ok(ChildProgress::Stopping)
    }
}

impl Drop for RealChild {
    /// A handle dropped while its child runs: kill the session (never leave an unsupervised child behind).
    fn drop(&mut self) {
        if self.finished.is_none() && self.child.is_some() {
            let groups = self.groups_from(self.sampler.latest().as_deref());
            self.signal_session(libc::SIGKILL, &groups);
            if let Some(cg) = &self.cgroup {
                cg.kill_all();
            }
            if let Some(mut c) = self.child.take() {
                let _ = c.wait();
            }
            self.sampler.untrack(self.sid);
        }
    }
}

/// Used by `CgroupRoot` users: fd of an open file.
pub(super) fn raw(f: &fs::File) -> RawFd {
    f.as_raw_fd()
}
