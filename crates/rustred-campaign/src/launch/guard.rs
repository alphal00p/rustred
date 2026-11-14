//! THE ONE PATH THROUGH WHICH THIS CRATE SIGNALS A PROCESS (audit A C1, M1, m1, m2; audit B C1-C3, M2).
//!
//! Governing principle: the process layer must be INCAPABLE of signalling a process that is not provably a child job of
//! this campaign, in every interleaving, including after a supervisor crash, a reboot, pid wrap-around and stale
//! records. When in doubt it does not signal and reports `Unverified` (a leaked job is a recoverable nuisance, a killed
//! foreign job is not).
//!
//! Signal syscalls exist only in the private module `sys` below, in three forms:
//! 1. `pidfd_send_signal(pidfd, sig)` on a pidfd opened BEFORE the identity was verified (the normal path);
//! 2. `kill(pid, sig)` with a [`ValidPid`] (2..=pid_max, never this process / parent / own session or group leader), only
//!    when the kernel lacks pidfds, right after a second identity read;
//! 3. `killpg(pgid, sig)` with a [`ValidPid`], only for the process group of a leader that is in the launcher's
//!    leader registry (see [`signal_pinned_group`]).
//!
//! No other code of the crate calls kill/killpg/pidfd_send_signal (a test greps the sources for it).
//!
//! IDENTITY CHECKLIST for a single process (`signal_verified`), all at the moment of signalling:
//! 1. the pid is a [`ValidPid::for_signal`];
//! 2. the recorded boot id equals the current one (after a reboot nothing recorded is signalled);
//! 3. a pidfd is opened (it pins THIS process: if the pid is recycled later, signalling the pidfd fails with ESRCH);
//! 4. `/proc/<pid>/stat` is read: gone -> `Gone`; a zombie without live threads -> `Gone` (nothing to signal);
//!    the start time (field 22) equals the recorded one (`StartRule::Exact`) or is not earlier than the job's
//!    recorded launch time (`StartRule::NotBefore`, for discovered members);
//! 5. optionally: the process leads its own session (every launched child does: setsid);
//! 6. the owner uid of `/proc/<pid>` is ours (root-owned = non-dumpable or foreign: refused);
//! 7. the PROOF of belonging:
//!    - `Proof::Marker` / `Proof::Campaign`: `/proc/<pid>/environ` carries this campaign's marker (exact attempt, or any
//!      attempt of the campaign); unreadable environ or another campaign id -> not ours;
//!    - `Proof::PinnedSession(sid)`: the process is in session `sid` AND `sid` is in the leader registry at the moment of
//!      the check (our unreaped leader pins the session number, so every member is a descendant of our child);
//! 8. the signal is sent through the pidfd of step 3. Since the pidfd refers to the process that held the pid at step 3
//!    and the send fails if that process has been reaped, a successful send proves the process read in steps 4-7 is
//!    the one signalled.
//!
//! The reads of steps 4-7 run on a helper thread with a bound ([`DEFAULT_VERIFY_TIMEOUT`]): a blocked /proc read gives
//! `Unverified`, never a hung control loop (audit A M5).
//!
//! GROUP FAST PATH ([`signal_pinned_group`]): `killpg(sid)` for a launched child whose leader we hold UNREAPED. Why it
//! is safe there and only there: the leader's pid number `sid` is the session id and the group id of the leader; a pid
//! number is not reallocated while any task still uses it as pid, pgid or sid, and our unreaped leader (alive or zombie)
//! uses it. Every member of group `sid` is in session `sid` (setpgid only moves within a session), and every member of
//! session `sid` descends from our child (only setsid creates a session, with the caller's own pid). The proof that the
//! leader is unreaped: it is in the leader registry, whose lock we hold while signalling (every reap of a leader happens
//! under that lock), and `waitid(WNOWAIT)` still finds it (an inherited SIG_IGN for SIGCHLD would have auto-reaped it:
//! audit A M4, also checked). Group numbers of OTHER groups (from a scan) are never signalled as groups (audit A M1):
//! their members are signalled one by one through the checklist.

use crate::resources::{
    CampaignMarker, InvalidPid, Leaders, MARKER_VAR, MarkedProcess, ProcessIdentity, ValidPid,
    boot_id, marker_in_environ, our_uid, page_size, parse_stat, run_bounded,
};
use serde::Serialize;
use std::io;
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Bound of the /proc reads of one verification.
pub const DEFAULT_VERIFY_TIMEOUT: Duration = Duration::from_secs(2);

/// Why a process was NOT signalled.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum Refusal {
    InvalidPid(InvalidPid),
    /// The record is from another boot: nothing recorded may be signalled.
    OtherBoot,
    StartMismatch {
        recorded: u64,
        current: u64,
    },
    StartedBeforeLaunch {
        launch: u64,
        current: u64,
    },
    NotSessionLeader {
        session: u32,
    },
    OtherSession {
        session: u32,
        expected: u32,
    },
    OtherUid {
        uid: u32,
    },
    /// The environ has no marker.
    NoMarker,
    /// The environ carries a marker of another campaign or attempt (the value is not secret: ids only).
    OtherMarker {
        found: String,
    },
    /// The session's leader is not (or no longer) an unreaped leader of ours.
    SessionNotPinned {
        sid: u32,
    },
}

/// Result of a verified signal request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum Verdict {
    Delivered,
    /// No such process (or it exited / became a zombie before the signal).
    Gone,
    NotOurs(Refusal),
    /// Could not prove either way (a read failed or timed out): NOT signalled; a human may look.
    Unverified(String),
}

impl Verdict {
    pub fn delivered(&self) -> bool {
        matches!(self, Verdict::Delivered)
    }
}

/// What the start time must satisfy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartRule {
    /// A recorded pid: exactly this start time.
    Exact(u64),
    /// A discovered member of a job launched at this start time: not earlier.
    NotBefore(u64),
}

/// The proof that the process belongs to us.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Proof {
    /// The environ carries exactly this marker.
    Marker(CampaignMarker),
    /// The environ carries a marker of this campaign (any attempt).
    Campaign(String),
    /// Member of this session, pinned by an unreaped leader of the given registry (checked during the call).
    PinnedSession(u32),
}

/// A single process to signal.
#[derive(Clone, Debug)]
pub struct ProcessTarget {
    pub pid: u32,
    /// The boot id of the record (or of the scan) that named this pid.
    pub boot_id: String,
    pub start: StartRule,
    pub require_session_leader: bool,
    pub proof: Proof,
}

// ------------------------------------------------------------------------------------------------ syscalls

mod sys {
    //! The ONLY signal syscalls of the crate.
    use super::ValidPid;
    use std::io;
    use std::os::fd::{AsRawFd, BorrowedFd, FromRawFd, OwnedFd};

    pub(super) fn pidfd_open(pid: ValidPid) -> io::Result<OwnedFd> {
        // SAFETY: pidfd_open(pid, flags) takes two integers and returns a new fd or -1.
        let r = unsafe { libc::syscall(libc::SYS_pidfd_open, pid.as_pid_t(), 0) };
        if r < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: r is a fresh descriptor owned by nobody else.
        Ok(unsafe { OwnedFd::from_raw_fd(r as i32) })
    }

    pub(super) fn pidfd_send_signal(fd: BorrowedFd<'_>, sig: i32) -> io::Result<()> {
        // SAFETY: pidfd_send_signal(fd, sig, info = NULL, flags = 0): integers and a null pointer.
        let r = unsafe {
            libc::syscall(
                libc::SYS_pidfd_send_signal,
                fd.as_raw_fd(),
                sig,
                std::ptr::null::<libc::siginfo_t>(),
                0,
            )
        };
        if r < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    /// Fallback without pidfds. `pid` is positive and > 1 by construction.
    pub(super) fn kill_pid(pid: ValidPid, sig: i32) -> io::Result<()> {
        // SAFETY: kill with a positive pid (> 1) targets exactly that process.
        if unsafe { libc::kill(pid.as_pid_t(), sig) } == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    /// Group signal; the caller holds the leader-registry proof. `pgid` is positive and > 1 by construction.
    pub(super) fn killpg_pinned(pgid: ValidPid, sig: i32) -> io::Result<()> {
        // SAFETY: killpg with a positive group id (> 1) targets exactly that group.
        if unsafe { libc::killpg(pgid.as_pid_t(), sig) } == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    /// Reap exactly the process of `fd` (waitid(P_PIDFD, WEXITED | WNOHANG)); true if reaped.
    pub(super) fn reap_pidfd(fd: &OwnedFd) -> bool {
        const P_PIDFD: libc::idtype_t = 3;
        // SAFETY: siginfo_t is plain data.
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        // SAFETY: valid out-pointer; P_PIDFD with our pidfd.
        let r = unsafe {
            libc::waitid(
                P_PIDFD,
                fd.as_raw_fd() as libc::id_t,
                &mut info,
                libc::WEXITED | libc::WNOHANG,
            )
        };
        // SAFETY: reading the SIGCHLD union field of a filled (or zeroed) siginfo.
        r == 0 && unsafe { info.si_pid() } != 0
    }
}

/// Result of one pidfd probe (gate re-audit m-2, lane I-SUP 2026-11-14).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PidfdProbe {
    Supported,
    /// The kernel has no pidfd_open (ENOSYS) or forbids it (EPERM): permanent, cached.
    Unsupported,
    /// A transient failure (EMFILE, ENFILE, ENOMEM, ...): NOT cached; the caller must not fall back to `kill`.
    Transient(String),
}

static PIDFD_STATE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0); // 0 unknown, 1 yes, 2 no

/// Probe pidfd support (cached once the answer is permanent).
pub fn pidfd_probe() -> PidfdProbe {
    use std::sync::atomic::Ordering;
    match PIDFD_STATE.load(Ordering::SeqCst) {
        1 => return PidfdProbe::Supported,
        2 => return PidfdProbe::Unsupported,
        _ => {}
    }
    // SAFETY: pidfd_open on our own pid; an fd is closed right away.
    let r = unsafe { libc::syscall(libc::SYS_pidfd_open, libc::getpid(), 0) };
    if r >= 0 {
        // SAFETY: closing the probe descriptor we just got.
        unsafe { libc::close(r as i32) };
        PIDFD_STATE.store(1, Ordering::SeqCst);
        return PidfdProbe::Supported;
    }
    let e = io::Error::last_os_error();
    match e.raw_os_error() {
        Some(libc::ENOSYS) | Some(libc::EPERM) => {
            PIDFD_STATE.store(2, Ordering::SeqCst);
            PidfdProbe::Unsupported
        }
        _ => PidfdProbe::Transient(e.to_string()),
    }
}

/// Does the kernel support pidfd_open / pidfd_send_signal? (Linux >= 5.3.) A transient probe failure answers false
/// for THIS call only (never cached): callers that would otherwise fall back to plain `kill` must use
/// [`pidfd_probe`] and refuse on `Transient`.
pub fn pidfd_supported() -> bool {
    pidfd_probe() == PidfdProbe::Supported
}

// ------------------------------------------------------------------------------------------------ verification

struct Facts {
    zombie_without_threads: bool,
    start_ticks: u64,
    session: u32,
    uid: Option<u32>,
    /// None: not read (not needed) ; Some(None): no marker ; Some(Some(v)): value.
    marker: Option<Result<Option<String>, String>>,
}

enum Read {
    Gone,
    Facts(Facts),
    Failed(String),
}

/// Steps 4-7 reads (blocking; run on a helper thread). `root` is "/proc" in production (a seam for tests).
fn read_facts(root: &Path, pid: u32, want_marker: bool) -> Read {
    let stat_path = root.join(format!("{pid}/stat"));
    let st = match std::fs::read(&stat_path) {
        Ok(b) => match parse_stat(pid, &String::from_utf8_lossy(&b), page_size()) {
            Some(s) => s,
            None => return Read::Failed("unparsable stat".into()),
        },
        Err(e) if matches!(e.raw_os_error(), Some(libc::ENOENT) | Some(libc::ESRCH)) => {
            return Read::Gone;
        }
        Err(e) => return Read::Failed(format!("stat: {e}")),
    };
    let mut env_path = root.join(format!("{pid}/environ"));
    let mut zombie_without_threads = false;
    if st.is_dead() {
        // A dead main thread with live threads still runs (audit B M1): read the live thread's environ.
        match live_thread_at(root, pid) {
            Some(tid) => env_path = root.join(format!("{pid}/task/{tid}/environ")),
            None => zombie_without_threads = true,
        }
    }
    let uid = {
        use std::os::unix::fs::MetadataExt;
        std::fs::metadata(root.join(pid.to_string()))
            .ok()
            .map(|m| m.uid())
    };
    let marker = if want_marker && !zombie_without_threads {
        Some(match std::fs::read(&env_path) {
            Ok(b) if b.is_empty() => Err("environ empty (no mm)".into()),
            Ok(b) => Ok(marker_in_environ(&b)),
            Err(e) => Err(format!("environ unreadable: {e}")),
        })
    } else {
        None
    };
    Read::Facts(Facts {
        zombie_without_threads,
        start_ticks: st.start_ticks,
        session: st.session,
        uid,
        marker,
    })
}

fn live_thread_at(root: &Path, pid: u32) -> Option<u32> {
    for e in std::fs::read_dir(root.join(format!("{pid}/task"))).ok()? {
        let Ok(e) = e else { continue };
        let Some(tid) = e.file_name().to_str().and_then(|s| s.parse::<u32>().ok()) else {
            continue;
        };
        if let Ok(b) = std::fs::read(root.join(format!("{pid}/task/{tid}/stat")))
            && let Some(st) = parse_stat(pid, &String::from_utf8_lossy(&b), page_size())
            && !st.is_dead()
        {
            return Some(tid);
        }
    }
    None
}

/// Judge the facts against the target (pure). Ok(()) = may signal.
fn judge(
    t: &ProcessTarget,
    f: &Facts,
    uid: u32,
    pinned: &dyn Fn(u32) -> bool,
) -> Result<(), Verdict> {
    if f.zombie_without_threads {
        return Err(Verdict::Gone);
    }
    match t.start {
        StartRule::Exact(s) if s != f.start_ticks => {
            return Err(Verdict::NotOurs(Refusal::StartMismatch {
                recorded: s,
                current: f.start_ticks,
            }));
        }
        StartRule::NotBefore(s) if f.start_ticks < s => {
            return Err(Verdict::NotOurs(Refusal::StartedBeforeLaunch {
                launch: s,
                current: f.start_ticks,
            }));
        }
        _ => {}
    }
    if t.require_session_leader && f.session != t.pid {
        return Err(Verdict::NotOurs(Refusal::NotSessionLeader {
            session: f.session,
        }));
    }
    match f.uid {
        Some(u) if u == uid => {}
        Some(u) => return Err(Verdict::NotOurs(Refusal::OtherUid { uid: u })),
        None => return Err(Verdict::Gone),
    }
    match &t.proof {
        Proof::PinnedSession(sid) => {
            if f.session != *sid {
                return Err(Verdict::NotOurs(Refusal::OtherSession {
                    session: f.session,
                    expected: *sid,
                }));
            }
            if !pinned(*sid) {
                return Err(Verdict::NotOurs(Refusal::SessionNotPinned { sid: *sid }));
            }
        }
        Proof::Marker(_) | Proof::Campaign(_) => {
            let found = match &f.marker {
                Some(Ok(Some(v))) => v,
                Some(Ok(None)) => return Err(Verdict::NotOurs(Refusal::NoMarker)),
                Some(Err(e)) => return Err(Verdict::Unverified(e.clone())),
                None => return Err(Verdict::Unverified("environ not read".into())),
            };
            let ok = match (&t.proof, CampaignMarker::parse(found)) {
                (Proof::Marker(m), Some(p)) => &p == m,
                (Proof::Campaign(c), Some(p)) => &p.campaign_id == c,
                _ => false,
            };
            if !ok {
                return Err(Verdict::NotOurs(Refusal::OtherMarker {
                    found: found.clone(),
                }));
            }
        }
    }
    Ok(())
}

/// Validate a signal number (0 = existence probe .. SIGRTMAX).
fn valid_signal(sig: i32) -> bool {
    (0..=64).contains(&sig)
}

/// The identity checklist + delivery. `leaders` is required for `Proof::PinnedSession` (else refused).
pub(crate) fn signal_verified(
    t: &ProcessTarget,
    sig: i32,
    leaders: Option<&Leaders>,
    timeout: Duration,
) -> Verdict {
    signal_verified_at(Path::new("/proc"), t, sig, leaders, timeout)
}

pub(crate) fn signal_verified_at(
    root: &Path,
    t: &ProcessTarget,
    sig: i32,
    leaders: Option<&Leaders>,
    timeout: Duration,
) -> Verdict {
    if !valid_signal(sig) {
        return Verdict::Unverified(format!("signal number {sig} out of range"));
    }
    // 1.
    let pid = match ValidPid::for_signal(i64::from(t.pid)) {
        Ok(p) => p,
        Err(e) => return Verdict::NotOurs(Refusal::InvalidPid(e)),
    };
    // 2.
    match boot_id() {
        Ok(b) if b == t.boot_id => {}
        Ok(_) => return Verdict::NotOurs(Refusal::OtherBoot),
        Err(e) => return Verdict::Unverified(format!("boot id unreadable: {e}")),
    }
    // 3.
    let pidfd = match pidfd_probe() {
        PidfdProbe::Supported => match sys::pidfd_open(pid) {
            Ok(fd) => Some(fd),
            Err(e) if e.raw_os_error() == Some(libc::ESRCH) => return Verdict::Gone,
            Err(e) => return Verdict::Unverified(format!("pidfd_open: {e}")),
        },
        PidfdProbe::Unsupported => None,
        // EMFILE / ENFILE / ENOMEM now: refuse this attempt (retry later), never the weaker kill path.
        PidfdProbe::Transient(e) => {
            return Verdict::Unverified(format!("pidfd probe failed transiently: {e}"));
        }
    };
    // 4-7 on a helper thread, bounded.
    let want_marker = matches!(t.proof, Proof::Marker(_) | Proof::Campaign(_));
    let read = {
        let root: PathBuf = root.to_path_buf();
        let p = t.pid;
        match run_bounded(timeout, move || read_facts(&root, p, want_marker)).done() {
            Some(r) => r,
            None => return Verdict::Unverified(format!("/proc reads of pid {p} timed out")),
        }
    };
    let facts = match read {
        Read::Gone => return Verdict::Gone,
        Read::Failed(e) => return Verdict::Unverified(e),
        Read::Facts(f) => f,
    };
    // Pinned-session proof: the registry lock is held from the judgement through the send.
    let guard = leaders.map(Leaders::lock);
    let pinned = |sid: u32| {
        guard
            .as_ref()
            .is_some_and(|g| g.contains(&sid) && leader_unreaped(sid))
    };
    if let Err(v) = judge(t, &facts, our_uid(), &pinned) {
        return v;
    }
    // 8.
    let r = match &pidfd {
        Some(fd) => sys::pidfd_send_signal(fd_borrow(fd), sig),
        None => {
            // No pidfd: re-read the start time right before kill (residual race of microseconds, documented).
            match crate::resources::read_stat(t.pid) {
                Ok(Some(st)) if st.start_ticks == facts.start_ticks => sys::kill_pid(pid, sig),
                Ok(_) => return Verdict::Gone,
                Err(e) => return Verdict::Unverified(format!("re-check: {e}")),
            }
        }
    };
    drop(guard);
    match r {
        Ok(()) => Verdict::Delivered,
        Err(e) if e.raw_os_error() == Some(libc::ESRCH) => Verdict::Gone,
        Err(e) => Verdict::Unverified(format!("send: {e}")),
    }
}

fn fd_borrow(fd: &OwnedFd) -> std::os::fd::BorrowedFd<'_> {
    use std::os::fd::AsFd;
    fd.as_fd()
}

/// Is `pid` still an unreaped child of ours (waitid WNOWAIT finds it) while SIGCHLD does not auto-reap?
fn leader_unreaped(pid: u32) -> bool {
    !sigchld_autoreaps() && super::real::peek_exit(pid).is_ok()
}

/// The group fast path: SIGNAL the process group `sid` of a launched child whose leader is in `leaders` (see the module
/// documentation for why this is safe here and only here).
pub(crate) fn signal_pinned_group(leaders: &Leaders, sid: u32, sig: i32) -> Verdict {
    if !valid_signal(sig) {
        return Verdict::Unverified(format!("signal number {sig} out of range"));
    }
    let pgid = match ValidPid::for_signal(i64::from(sid)) {
        Ok(p) => p,
        Err(e) => return Verdict::NotOurs(Refusal::InvalidPid(e)),
    };
    let guard = leaders.lock();
    if !guard.contains(&sid) || !leader_unreaped(sid) {
        return Verdict::NotOurs(Refusal::SessionNotPinned { sid });
    }
    let r = sys::killpg_pinned(pgid, sig);
    drop(guard);
    match r {
        Ok(()) => Verdict::Delivered,
        Err(e) if e.raw_os_error() == Some(libc::ESRCH) => Verdict::Gone,
        Err(e) => Verdict::Unverified(format!("killpg: {e}")),
    }
}

/// SIGKILL to every pinned leader group at once (fast exit, audit A M6: "before any fast exit, killpg every unreaped
/// leader group"). Returns how many groups were signalled.
pub(crate) fn kill_all_pinned(leaders: &Leaders) -> usize {
    let sids: Vec<u32> = leaders.lock().iter().copied().collect();
    sids.into_iter()
        .filter(|&s| signal_pinned_group(leaders, s, libc::SIGKILL).delivered())
        .count()
}

/// Reap exactly (pid, start) if it is a zombie child of ours: pidfd, start-time check, waitid(P_PIDFD). The caller holds
/// the leader registry lock and has checked the pid is not a leader.
pub(crate) fn reap_exact(pid: u32, start_ticks: u64) -> bool {
    let Ok(p) = ValidPid::for_signal(i64::from(pid)) else {
        return false;
    };
    if !pidfd_supported() {
        return false;
    }
    let Ok(fd) = sys::pidfd_open(p) else {
        return false;
    };
    match crate::resources::read_stat(pid) {
        Ok(Some(st)) if st.start_ticks == start_ticks && st.is_dead() => sys::reap_pidfd(&fd),
        _ => false,
    }
}

// ------------------------------------------------------------------------------------------------ public verified kills

/// Signal ONE process named by a record, if it passes the whole checklist with the exact marker of its attempt (the
/// replacement of the former `signal_group_by_identity`, which signalled a group number from a record).
pub fn signal_verified_process(
    identity: &ProcessIdentity,
    marker: &CampaignMarker,
    sig: i32,
) -> Verdict {
    let t = ProcessTarget {
        pid: identity.pid,
        boot_id: identity.boot_id.clone(),
        start: StartRule::Exact(identity.start_ticks),
        require_session_leader: false,
        proof: Proof::Marker(marker.clone()),
    };
    signal_verified(&t, sig, None, DEFAULT_VERIFY_TIMEOUT)
}

/// The separate VERIFIED KILL of the orphan scan: signal a process returned by `resources::scan_campaign` (or found in
/// `Snapshot::marked`) only if, now, it still has that start time and still carries a marker of `campaign_id`.
pub fn kill_orphan(campaign_id: &str, scan_boot_id: &str, p: &MarkedProcess, sig: i32) -> Verdict {
    let t = ProcessTarget {
        pid: p.pid,
        boot_id: scan_boot_id.to_string(),
        start: StartRule::Exact(p.start_ticks),
        require_session_leader: false,
        proof: Proof::Campaign(campaign_id.to_string()),
    };
    signal_verified(&t, sig, None, DEFAULT_VERIFY_TIMEOUT)
}

// ------------------------------------------------------------------------------------------------ process-wide setup

/// What [`ensure_sigchld_default`] found.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum SigchldSetup {
    AlreadyDefault,
    /// SIGCHLD was inherited as SIG_IGN or with SA_NOCLDWAIT (auto-reaping): reset to SIG_DFL.
    ResetFromIgnored,
    /// A handler of the program (e.g. signal-hook) is installed without SA_NOCLDWAIT: left alone.
    HandlerKept,
}

/// Would the kernel auto-reap our children (SIGCHLD = SIG_IGN, or SA_NOCLDWAIT)?
pub fn sigchld_autoreaps() -> bool {
    // SAFETY: sigaction with a null new action only reads the current one into `old`.
    let mut old: libc::sigaction = unsafe { std::mem::zeroed() };
    let r = unsafe { libc::sigaction(libc::SIGCHLD, std::ptr::null(), &mut old) };
    r == 0 && (old.sa_sigaction == libc::SIG_IGN || old.sa_flags & libc::SA_NOCLDWAIT != 0)
}

/// Audit A M4: an inherited SIG_IGN for SIGCHLD (or SA_NOCLDWAIT) makes the kernel reap children at once, which would
/// void the "unreaped leader pins the session" argument. Reset it to SIG_DFL (documented behaviour of the launcher).
pub fn ensure_sigchld_default() -> io::Result<SigchldSetup> {
    // SAFETY: as in sigchld_autoreaps.
    let mut old: libc::sigaction = unsafe { std::mem::zeroed() };
    if unsafe { libc::sigaction(libc::SIGCHLD, std::ptr::null(), &mut old) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let ignored = old.sa_sigaction == libc::SIG_IGN;
    let nocldwait = old.sa_flags & libc::SA_NOCLDWAIT != 0;
    if !ignored && !nocldwait {
        return Ok(if old.sa_sigaction == libc::SIG_DFL {
            SigchldSetup::AlreadyDefault
        } else {
            SigchldSetup::HandlerKept
        });
    }
    // SAFETY: a zeroed sigaction with SIG_DFL and no flags; the old one is not needed.
    let mut new: libc::sigaction = unsafe { std::mem::zeroed() };
    new.sa_sigaction = if ignored {
        libc::SIG_DFL
    } else {
        old.sa_sigaction
    };
    new.sa_flags = old.sa_flags & !libc::SA_NOCLDWAIT;
    new.sa_mask = old.sa_mask;
    if unsafe { libc::sigaction(libc::SIGCHLD, &new, std::ptr::null_mut()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(SigchldSetup::ResetFromIgnored)
}

/// PR_SET_CHILD_SUBREAPER: orphaned descendants are re-parented to this process (so they can be found, verified by
/// their marker and reaped) instead of to init.
pub fn set_child_subreaper(on: bool) -> io::Result<()> {
    // SAFETY: prctl with integer arguments.
    let r = unsafe {
        libc::prctl(
            libc::PR_SET_CHILD_SUBREAPER,
            libc::c_ulong::from(on),
            0,
            0,
            0,
        )
    };
    if r == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

pub fn is_child_subreaper() -> bool {
    let mut v: libc::c_int = 0;
    // SAFETY: PR_GET_CHILD_SUBREAPER writes an int through the pointer.
    let r = unsafe {
        libc::prctl(
            libc::PR_GET_CHILD_SUBREAPER,
            &mut v as *mut libc::c_int,
            0,
            0,
            0,
        )
    };
    r == 0 && v != 0
}

/// The marker variable name, re-exported for callers building environments.
pub const CAMPAIGN_MARKER_VAR: &str = MARKER_VAR;

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(start: u64, session: u32, uid: u32, marker: Option<&str>) -> Facts {
        Facts {
            zombie_without_threads: false,
            start_ticks: start,
            session,
            uid: Some(uid),
            marker: Some(Ok(marker.map(str::to_string))),
        }
    }

    fn target(proof: Proof, start: StartRule) -> ProcessTarget {
        ProcessTarget {
            pid: 1000,
            boot_id: "b".into(),
            start,
            require_session_leader: false,
            proof,
        }
    }

    #[test]
    fn judge_rules() {
        let m = CampaignMarker::new("c1", "a1").unwrap();
        let yes = |_: u32| true;
        let no = |_: u32| false;
        let t = target(Proof::Marker(m.clone()), StartRule::Exact(5));
        assert_eq!(judge(&t, &facts(5, 9, 7, Some("c1:a1")), 7, &no), Ok(()));
        assert!(matches!(
            judge(&t, &facts(6, 9, 7, Some("c1:a1")), 7, &no),
            Err(Verdict::NotOurs(Refusal::StartMismatch { .. }))
        ));
        assert!(matches!(
            judge(&t, &facts(5, 9, 8, Some("c1:a1")), 7, &no),
            Err(Verdict::NotOurs(Refusal::OtherUid { .. }))
        ));
        assert!(matches!(
            judge(&t, &facts(5, 9, 7, None), 7, &no),
            Err(Verdict::NotOurs(Refusal::NoMarker))
        ));
        assert!(matches!(
            judge(&t, &facts(5, 9, 7, Some("c2:a1")), 7, &no),
            Err(Verdict::NotOurs(Refusal::OtherMarker { .. }))
        ));
        assert!(matches!(
            judge(&t, &facts(5, 9, 7, Some("c1:a2")), 7, &no),
            Err(Verdict::NotOurs(Refusal::OtherMarker { .. }))
        ));
        let c = target(Proof::Campaign("c1".into()), StartRule::NotBefore(5));
        assert_eq!(judge(&c, &facts(9, 9, 7, Some("c1:zz")), 7, &no), Ok(()));
        assert!(matches!(
            judge(&c, &facts(4, 9, 7, Some("c1:zz")), 7, &no),
            Err(Verdict::NotOurs(Refusal::StartedBeforeLaunch { .. }))
        ));
        let mut unread = facts(9, 9, 7, None);
        unread.marker = Some(Err("EACCES".into()));
        assert!(matches!(
            judge(&c, &unread, 7, &no),
            Err(Verdict::Unverified(_))
        ));
        let s = target(Proof::PinnedSession(9), StartRule::NotBefore(1));
        assert_eq!(judge(&s, &facts(9, 9, 7, None), 7, &yes), Ok(()));
        assert!(matches!(
            judge(&s, &facts(9, 9, 7, None), 7, &no),
            Err(Verdict::NotOurs(Refusal::SessionNotPinned { .. }))
        ));
        assert!(matches!(
            judge(&s, &facts(9, 10, 7, None), 7, &yes),
            Err(Verdict::NotOurs(Refusal::OtherSession { .. }))
        ));
        let mut l = target(Proof::Marker(m), StartRule::Exact(5));
        l.require_session_leader = true;
        assert!(matches!(
            judge(&l, &facts(5, 9, 7, Some("c1:a1")), 7, &no),
            Err(Verdict::NotOurs(Refusal::NotSessionLeader { .. }))
        ));
    }

    /// Audit A M5 seam: a /proc root whose `environ` is a FIFO without writer blocks a read forever; the verification
    /// must come back `Unverified` within its bound, and nothing is signalled. The pid is a child of this test (so the
    /// pidfd opens); its fake stat names it with the right start time.
    #[test]
    fn slow_proc_read_gives_unverified_within_the_bound() {
        let mut child = std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .unwrap();
        let pid = child.id();
        let real = crate::resources::read_stat(pid).unwrap().unwrap();
        let root = std::env::temp_dir().join(format!("rr-guard-fifo-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join(pid.to_string())).unwrap();
        std::fs::write(
            root.join(format!("{pid}/stat")),
            std::fs::read(format!("/proc/{pid}/stat")).unwrap(),
        )
        .unwrap();
        let fifo = std::ffi::CString::new(
            root.join(format!("{pid}/environ"))
                .to_str()
                .unwrap()
                .as_bytes(),
        )
        .unwrap();
        // SAFETY: mkfifo with a valid C string.
        assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
        let t = ProcessTarget {
            pid,
            boot_id: boot_id().unwrap(),
            start: StartRule::Exact(real.start_ticks),
            require_session_leader: false,
            proof: Proof::Campaign("c1".into()),
        };
        let t0 = std::time::Instant::now();
        let v = signal_verified_at(&root, &t, libc::SIGKILL, None, Duration::from_millis(300));
        assert!(matches!(v, Verdict::Unverified(_)), "{v:?}");
        assert!(t0.elapsed() < Duration::from_secs(5));
        assert!(
            child.try_wait().unwrap().is_none(),
            "nothing may have been signalled"
        );
        // Unblock the stuck reader (open the FIFO for writing once) and clean up.
        let _w = std::fs::OpenOptions::new()
            .write(true)
            .open(root.join(format!("{pid}/environ")));
        child.kill().unwrap();
        child.wait().unwrap();
        let _ = std::fs::remove_dir_all(&root);
    }
}
