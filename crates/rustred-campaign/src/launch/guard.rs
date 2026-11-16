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

use crate::resources::procfs::{self, ProcError, ProcRoot};
use crate::resources::{
    CampaignMarker, InvalidPid, Leaders, LiveThread, MARKER_VAR, MarkedProcess, ProcessIdentity,
    ValidPid, boot_id, live_thread_at, marker_in_environ, our_uid, run_bounded,
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

/// procfix6 (gate5 m1): THE IDENTITY HELD AT A STOP. Made only here, only when this guard DELIVERED a SIGSTOP through
/// a pidfd the caller opened BEFORE the identity checklist ran (marker / pinned-session proof, exact start time, our uid,
/// this boot). It names that process (pid, start time, boot) and what the checklist read (the marker); it can be used
/// only together with the same pidfd ([`signal_held`] checks that the descriptor names this pid), for SIGKILL or
/// SIGCONT only. Why it is needed: a batch reads N environs, writes the intents, then sends; a process that execs into an
/// unmarked image between its read and its SIGSTOP is stopped without the marker, and the kill round's marker check
/// then refuses it: without the hold it stayed STOPPED forever. Fields are private: no other module can make one.
#[derive(Debug)]
pub(crate) struct StopHold {
    pid: u32,
    start_ticks: u64,
    boot_id: String,
    marker: Option<String>,
    proof: &'static str,
}

impl StopHold {
    pub(crate) fn pid(&self) -> u32 {
        self.pid
    }

    /// Unit tests of other modules: a hold with chosen facts (e.g. another boot, so that it can never be used).
    #[cfg(test)]
    pub(crate) fn for_test(pid: u32, start_ticks: u64, boot_id: &str) -> StopHold {
        StopHold {
            pid,
            start_ticks,
            boot_id: boot_id.into(),
            marker: None,
            proof: "campaign marker",
        }
    }
}

// ------------------------------------------------------------------------------------------------ delivery log

/// One signal this process DELIVERED through the guard (gate3 M2, lane int5: every signal sent becomes a ledger record;
/// the supervisor drains [`take_deliveries`] into `signal_sent` records). Recorded only after a successful send; since
/// procfix5 (gate4 M1) every send is preceded by a durable intent line, so a crash before the drain loses nothing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Delivery {
    /// The process (target "process"), the process group = the pinned leader's session (target "group"), or 0 for a
    /// cgroup-wide kill (target "cgroup", path in `detail`).
    pub pid: u32,
    /// Start time (clock ticks after boot) read by the checklist; None for a group or cgroup kill.
    pub start_ticks: Option<u64>,
    pub signal: i32,
    /// "process" | "group" | "cgroup".
    pub target: &'static str,
    /// The marker value read from the process's environ (campaign id and attempt label), when the proof was a marker.
    pub marker: Option<String>,
    /// The proof of belonging: "attempt marker", "campaign marker", "pinned session", "pinned group", "job cgroup",
    /// "job scope".
    pub proof: &'static str,
    /// The caller's context ([`with_context`]): who signalled and why ("campaign sweep", "end-of-job hunt", ...).
    pub by: String,
    /// The attempt label of the caller's context (`<job>:a<k>`), when known.
    pub label: Option<String>,
    /// Extra detail (cgroup path).
    pub detail: Option<String>,
    /// procfix5 M1: the id of the durable intent written before the send (None without an installed intent log).
    pub intent: Option<String>,
}

/// Deliveries kept for the drain at most (a supervisor drains every tick; beyond this they are counted, not kept).
pub const DELIVERY_LOG_MAX: usize = 1 << 16;

static DELIVERIES: std::sync::Mutex<(Vec<Delivery>, u64)> = std::sync::Mutex::new((Vec::new(), 0));

thread_local! {
    static CONTEXT: std::cell::RefCell<Option<(String, Option<String>)>> = const { std::cell::RefCell::new(None) };
}

/// Run `f` with a signalling context (who signals, which attempt) that every delivery made by `f` on THIS thread carries.
pub(crate) fn with_context<R>(by: &str, label: Option<&str>, f: impl FnOnce() -> R) -> R {
    let prev = CONTEXT.with(|c| c.replace(Some((by.to_string(), label.map(str::to_string)))));
    let r = f();
    CONTEXT.with(|c| *c.borrow_mut() = prev);
    r
}

fn current_context() -> Option<(String, Option<String>)> {
    CONTEXT.with(|c| c.borrow().clone())
}

#[allow(clippy::too_many_arguments)]
fn log_delivery(
    pid: u32,
    start_ticks: Option<u64>,
    signal: i32,
    target: &'static str,
    marker: Option<String>,
    proof: &'static str,
    detail: Option<String>,
    intent: Option<String>,
) {
    DELIVERED_TOTAL.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let (by, label) = current_context().unwrap_or_else(|| ("unspecified".into(), None));
    let d = Delivery {
        pid,
        start_ticks,
        signal,
        target,
        marker,
        proof,
        by,
        label,
        detail,
        intent,
    };
    let mut g = DELIVERIES.lock().unwrap_or_else(|p| p.into_inner());
    if g.0.len() < DELIVERY_LOG_MAX {
        g.0.push(d);
    } else {
        g.1 += 1;
    }
}

/// Take every delivery logged since the last call, and how many could not be kept (beyond [`DELIVERY_LOG_MAX`]).
pub fn take_deliveries() -> (Vec<Delivery>, u64) {
    let mut g = DELIVERIES.lock().unwrap_or_else(|p| p.into_inner());
    let v = std::mem::take(&mut g.0);
    let dropped = std::mem::replace(&mut g.1, 0);
    (v, dropped)
}

// ------------------------------------------------------------------------------------------------ write-ahead intents

// procfix5 (gate4 M1): a delivered signal must never be missing from the record after a crash. The delivery log above
// is drained into the ledger once per tick, so a crash lost the last tick's deliveries (E3: 10 SIGKILLs in 5 of 8
// crashes). Every signal of the guard (process, group, cgroup kill) is therefore preceded by a DURABLE INTENT line
// (appended and fdatasync'ed) in `CDIR/log/signal-intents.jsonl` naming pid, start time, marker, signal, proof, who
// and why (context) and the attempt; after the send a completion line (`done`, not synced) names the result. A batch
// of sends (one sweep round, one job's signal, the fast exit) writes all its intents with ONE fdatasync before the
// first send; nothing is ever sent before its intent is durable. When the intent cannot be made durable the signal is
// NOT sent (`Unverified`). At recovery the supervisor records every intent that is in no `signal_sent` record: with a
// `done delivered` line as delivered, without a completion line as POSSIBLY SENT (`reconcile_intents`); nothing is
// ever re-sent from an intent (the recovery sweep re-runs the identity checklist on whatever lives).
// The log is a process-wide sink installed by the supervisor ([`install_intent_log`]); without it (library users,
// unit tests) the guard behaves as before.

/// File name of the intent log under the campaign's `log/` directory.
pub const INTENT_LOG_FILE: &str = "signal-intents.jsonl";
/// A small reserve next to the intent log, deleted to make room when an intent write meets a full disk.
pub const INTENT_RESERVE_FILE: &str = "signal-intents.reserve";
const INTENT_RESERVE_BYTES: usize = 64 << 10;
/// Schema word of every intent-log line.
pub const INTENT_SCHEMA: &str = "rustred.campaign.signal-intent.v1";

struct IntentLog {
    file: std::fs::File,
    reserve: PathBuf,
    session: String,
    next: u64,
    /// procfix6 (gate5 m3): an intent write of this log failed for lack of space.
    disk_full: bool,
    /// procfix7 (gate6 m3-a): the tmpfs FALLBACK log of this session (pre-allocated pages), used once the campaign
    /// directory's log is full; `on_fallback` = from then on every intent and completion line goes there.
    fallback: Option<(std::fs::File, PathBuf)>,
    on_fallback: bool,
}

static INTENTS: std::sync::Mutex<Option<IntentLog>> = std::sync::Mutex::new(None);

#[cfg(test)]
thread_local! {
    /// Unit tests: a sink of THIS thread only (other tests signalling concurrently never see it).
    static TEST_SINK: std::cell::RefCell<Option<IntentLog>> = const { std::cell::RefCell::new(None) };
}

/// Run `f` with the intent log of THIS thread (unit tests only).
#[cfg(test)]
fn with_intent_log<R>(log_dir: &Path, session: &str, f: impl FnOnce() -> R) -> R {
    std::fs::create_dir_all(log_dir).unwrap();
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_dir.join(INTENT_LOG_FILE))
        .unwrap();
    TEST_SINK.with(|t| {
        *t.borrow_mut() = Some(IntentLog {
            file,
            reserve: log_dir.join(INTENT_RESERVE_FILE),
            session: session.into(),
            next: 0,
            disk_full: false,
            fallback: None,
            on_fallback: false,
        })
    });
    let r = f();
    TEST_SINK.with(|t| *t.borrow_mut() = None);
    r
}

/// The sink to use: the thread's test sink, else the process-wide one.
fn with_sink<R>(f: impl FnOnce(Option<&mut IntentLog>) -> R) -> R {
    #[cfg(test)]
    {
        let mut f = Some(f);
        let r = TEST_SINK.with(|t| {
            let mut b = t.borrow_mut();
            b.as_mut().map(|log| (f.take().unwrap())(Some(log)))
        });
        if let Some(r) = r {
            return r;
        }
        let mut g = INTENTS.lock().unwrap_or_else(|p| p.into_inner());
        (f.take().unwrap())(g.as_mut())
    }
    #[cfg(not(test))]
    {
        let mut g = INTENTS.lock().unwrap_or_else(|p| p.into_inner());
        f(g.as_mut())
    }
}

/// One signal about to be sent (the context of the calling thread supplies who / why / attempt).
#[derive(Clone, Debug)]
struct Intent {
    pid: u32,
    start_ticks: Option<u64>,
    signal: i32,
    target: &'static str,
    marker: Option<String>,
    proof: &'static str,
    detail: Option<String>,
}

/// One line of the intent log (both kinds; fields of the other kind are absent).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, serde::Deserialize)]
pub struct IntentLine {
    pub schema: String,
    /// "intent" | "done".
    pub kind: String,
    pub id: String,
    /// procfix7 (gate7 m-fb): the campaign id, on the header line of a tmpfs fallback log.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub campaign: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub utc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_ticks: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signal: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub marker: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proof: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// For `done`: "delivered" | "gone" | "failed: <error>".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
}

/// Install the intent log of THIS process (the supervisor, before it can signal anything): `log_dir/signal-intents.jsonl`
/// opened for appending (created and its directory synced if new) plus the reserve file. `session` makes the intent ids
/// unique (boot, pid, start time of the supervisor). Replaces a previous installation.
pub fn install_intent_log(log_dir: &Path, session: &str) -> io::Result<()> {
    std::fs::create_dir_all(log_dir)?;
    let path = log_dir.join(INTENT_LOG_FILE);
    let new = !path.exists();
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    let reserve = log_dir.join(INTENT_RESERVE_FILE);
    if std::fs::symlink_metadata(&reserve).is_err() {
        // Best effort: the reserve only helps a later write on a full disk.
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&reserve)
        {
            use std::io::Write;
            #[allow(clippy::let_underscore_must_use)]
            // ignored-ok (procfix7): the reserve only helps a later full-disk write (best effort)
            let _ = f.write_all(&vec![0u8; INTENT_RESERVE_BYTES]);
            #[allow(clippy::let_underscore_must_use)]
            // ignored-ok (procfix7): the reserve only helps a later full-disk write (best effort)
            let _ = f.sync_all();
        }
    }
    prealloc_tail(&file);
    if new {
        file.sync_all()?;
        std::fs::File::open(log_dir)?.sync_all()?;
    }
    let mut g = INTENTS.lock().unwrap_or_else(|p| p.into_inner());
    *g = Some(IntentLog {
        file,
        reserve,
        session: session.to_string(),
        next: 0,
        disk_full: false,
        fallback: None,
        on_fallback: false,
    });
    Ok(())
}

// procfix7 (gate6 m3-a): on ZFS (where the host run's campaign directory lives) NOTHING written inside the campaign
// directory reserves space: fallocate returns 0 and allocates nothing, an overwrite in place is copy-on-write (it needs
// new blocks), and a deleted reserve file frees its blocks only after the transaction group syncs (and not at all while
// a snapshot holds them). A dataset reservation would, but needs the administrator. What IS sound: a second intent log
// on a TMPFS (memory-backed; `fallocate` there really allocates the pages, so appends within them cannot fail for lack
// of space), used once the campaign directory's log is full. tmpfs survives a supervisor crash (the only case the
// intents serve: the restart records what the dead session sent), not a reboot (after which no process of the earlier
// boot exists; only the ledger's record of the last signals before a crash + reboot could then be incomplete).

/// Sub-directory (of the tmpfs base) of the fallback logs of one user.
const FALLBACK_DIR_PREFIX: &str = "rr-campaign-intents-";
/// Bytes pre-allocated in a fallback log (tmpfs: real pages).
const FALLBACK_PREALLOC_BYTES: i64 = 256 << 10;
const TMPFS_MAGIC: i64 = 0x0102_1994;

fn on_tmpfs(p: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt;
    let Ok(c) = std::ffi::CString::new(p.as_os_str().as_bytes()) else {
        return false;
    };
    // SAFETY: statfs fills a plain struct through a valid pointer; the path is NUL-terminated.
    let mut v: libc::statfs = unsafe { std::mem::zeroed() };
    (unsafe { libc::statfs(c.as_ptr(), &mut v) } == 0) && v.f_type as i64 == TMPFS_MAGIC
}

/// procfix7 (gate7 m-xdg): the base of the fallback intent logs: `/dev/shm/rr-campaign-intents-<uid>`, a path derived
/// from the uid ONLY (as the id lock since procfix3: no environment variable decides where intents of a campaign go,
/// so every session of a campaign looks in the same place), on the tmpfs `/dev/shm`.
pub fn intent_fallback_base() -> PathBuf {
    PathBuf::from(format!(
        "/dev/shm/{FALLBACK_DIR_PREFIX}{}",
        crate::resources::our_uid()
    ))
}

/// procfix7 (gate7 m-xdg): a directory we may keep intents in: created 0700 if absent; refused unless it is a real
/// directory (not a symlink) owned by us and not accessible to group / others (a directory someone else created
/// first is never used: the remedy procfix3 gave the id lock).
fn private_dir(dir: &Path, create: bool) -> io::Result<()> {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
    match std::fs::symlink_metadata(dir) {
        Ok(_) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound && create => {
            match std::fs::DirBuilder::new().mode(0o700).create(dir) {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(e),
            }
        }
        Err(e) => return Err(e),
    }
    let m = std::fs::symlink_metadata(dir)?;
    if !m.file_type().is_dir() {
        return Err(io::Error::other(format!(
            "{}: not a directory (or a symlink)",
            dir.display()
        )));
    }
    if m.uid() != crate::resources::our_uid() {
        return Err(io::Error::other(format!(
            "{}: owned by uid {}, not by us",
            dir.display(),
            m.uid()
        )));
    }
    if m.permissions().mode() & 0o077 != 0 {
        return Err(io::Error::other(format!(
            "{}: mode {:o} is accessible to group / others",
            dir.display(),
            m.permissions().mode() & 0o777
        )));
    }
    Ok(())
}

/// procfix7 (gate6 m3-a, gate7 m-xdg): the directory of the fallback intent logs of the campaign directory `cdir`
/// (keyed by its device and inode under [`intent_fallback_base`]); checked (owner, mode, no symlink) and, with
/// `create`, created. Err: no usable fallback directory (NotFound when it does not exist and `create` is false).
pub fn intent_fallback_dir_checked(cdir: &Path, create: bool) -> io::Result<PathBuf> {
    use std::os::unix::fs::MetadataExt;
    let md = std::fs::metadata(cdir)?;
    let base = intent_fallback_base();
    if create && !on_tmpfs(Path::new("/dev/shm")) {
        return Err(io::Error::other("/dev/shm is not a tmpfs"));
    }
    private_dir(&base, create)?;
    let dir = base.join(format!("{}-{}", md.dev(), md.ino()));
    private_dir(&dir, create)?;
    Ok(dir)
}

/// The fallback directory of `cdir` (not created, not checked; see [`intent_fallback_dir_checked`]).
pub fn intent_fallback_dir(cdir: &Path) -> Option<PathBuf> {
    use std::os::unix::fs::MetadataExt;
    let md = std::fs::metadata(cdir).ok()?;
    Some(intent_fallback_base().join(format!("{}-{}", md.dev(), md.ino())))
}

/// procfix7 (gate6 m3-a): give the INSTALLED intent log its tmpfs fallback for this session (`<dir>/<session>.jsonl`,
/// 256 KiB pre-allocated). gate7 m-fb: its FIRST line is a header naming the campaign id and the session, written and
/// synced at install: only a log whose header names this campaign and one of its recorded sessions is ever merged.
/// Returns the path; Err: no fallback (the log works as before).
pub fn install_intent_fallback(cdir: &Path, campaign_id: &str) -> io::Result<PathBuf> {
    use std::io::Write;
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::OpenOptionsExt;
    let dir = intent_fallback_dir_checked(cdir, true)?;
    let mut g = INTENTS.lock().unwrap_or_else(|p| p.into_inner());
    let log = g
        .as_mut()
        .ok_or_else(|| io::Error::other("no intent log installed"))?;
    let path = dir.join(format!("{}.jsonl", log.session));
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .append(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&path)?;
    let header = IntentLine {
        schema: INTENT_SCHEMA.into(),
        kind: "header".into(),
        id: log.session.clone(),
        campaign: Some(campaign_id.to_string()),
        utc: Some(crate::cpu_policy::utc_now()),
        pid: None,
        start_ticks: None,
        signal: None,
        target: None,
        marker: None,
        proof: None,
        by: None,
        label: None,
        detail: None,
        result: None,
    };
    let mut line = serde_json::to_vec(&header).map_err(io::Error::other)?;
    line.push(b'\n');
    file.write_all(&line)?;
    file.sync_data()?;
    // SAFETY: fallocate on our own descriptor with integer arguments (tmpfs: allocates the pages).
    if unsafe {
        libc::fallocate(
            file.as_raw_fd(),
            libc::FALLOC_FL_KEEP_SIZE,
            0,
            FALLBACK_PREALLOC_BYTES,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    log.fallback = Some((file, path.clone()));
    Ok(path)
}

/// procfix7 (gate7 m-fb): the header of a fallback log: (campaign id, session). Ok(None): no header (a foreign file or
/// a log of an older binary: never merged).
pub fn read_fallback_header(path: &Path) -> io::Result<Option<(String, String)>> {
    use std::io::BufRead;
    let f = std::fs::File::open(path)?;
    let mut first = String::new();
    std::io::BufReader::new(f).read_line(&mut first)?;
    Ok(serde_json::from_str::<IntentLine>(first.trim())
        .ok()
        .filter(|l| l.schema == INTENT_SCHEMA && l.kind == "header")
        .and_then(|l| l.campaign.map(|c| (c, l.id))))
}

/// procfix7 (gate6 m3-a): every fallback intent log of the campaign directory `cdir` (earlier sessions of this boot).
/// gate7 m-xdg: Err when the fallback directory exists but is not ours (owner / mode / symlink): nothing in it is used.
pub fn intent_fallback_logs(cdir: &Path) -> io::Result<Vec<PathBuf>> {
    let dir = match intent_fallback_dir_checked(cdir, false) {
        Ok(d) => d,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut v = Vec::new();
    for e in std::fs::read_dir(&dir)? {
        let p = e?.path();
        if p.extension().is_some_and(|x| x == "jsonl") {
            v.push(p);
        }
    }
    v.sort();
    Ok(v)
}

/// procfix7 (gate6 m3-a, gate7 m-fb): remove exactly these fallback logs (earlier sessions of THIS campaign whose
/// intents are in the ledger now); never this session's own. Returns how many were removed.
pub fn remove_intent_fallbacks(paths: &[PathBuf]) -> usize {
    let own = INTENTS
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .as_ref()
        .and_then(|l| l.fallback.as_ref().map(|f| f.1.clone()));
    paths
        .iter()
        .filter(|p| Some(*p) != own.as_ref())
        .filter(|p| std::fs::remove_file(p).is_ok())
        .count()
}

/// procfix7 (gate6 m3-a): at the end of a session, remove its own fallback log if it was never used (a used one stays
/// for the restart), and the then empty directories.
pub fn retire_own_intent_fallback() {
    let mut g = INTENTS.lock().unwrap_or_else(|p| p.into_inner());
    let Some(log) = g.as_mut() else { return };
    if log.on_fallback {
        return;
    }
    if let Some((_, p)) = log.fallback.take() {
        #[allow(clippy::let_underscore_must_use)]
        // ignored-ok (procfix7): only our own UNUSED fallback log and then-empty directories (best effort cleanup)
        let _ = std::fs::remove_file(&p);
        if let Some(d) = p.parent() {
            #[allow(clippy::let_underscore_must_use)]
            // ignored-ok (procfix7): only our own UNUSED fallback log and then-empty directories (best effort cleanup)
            let _ = std::fs::remove_dir(d);
        }
    }
}

/// procfix7 (gate6 m3-a): the fallback log the installed intent log writes to since the campaign directory's log filled
/// up (None: still the campaign directory's log).
pub fn intent_on_fallback() -> Option<PathBuf> {
    INTENTS
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .as_ref()
        .filter(|l| l.on_fallback)
        .and_then(|l| l.fallback.as_ref().map(|f| f.1.clone()))
}

/// Remove the installation (tests; a supervisor keeps it until it exits).
pub fn uninstall_intent_log() {
    *INTENTS.lock().unwrap_or_else(|p| p.into_inner()) = None;
}

/// Is an intent log installed in this process?
pub fn intent_log_installed() -> bool {
    INTENTS.lock().unwrap_or_else(|p| p.into_inner()).is_some()
}

fn is_disk_full(e: &io::Error) -> bool {
    matches!(
        e.raw_os_error(),
        Some(libc::ENOSPC) | Some(libc::EDQUOT) | Some(libc::EFBIG)
    )
}

/// Write the intents of one batch and make them durable with ONE fdatasync. `Ok(ids)` (all `None` when no log is
/// installed); `Err`: nothing may be sent.
fn write_intents(items: &[Intent]) -> Result<Vec<Option<String>>, String> {
    if items.is_empty() {
        return Ok(Vec::new());
    }
    let (by, label) = current_context().unwrap_or_else(|| ("unspecified".into(), None));
    let r = with_sink(|log| write_intents_to(log, items, by.clone(), label.clone()));
    if let Err(e) = &r {
        note_not_signalled(items, &by, label.as_deref(), e);
    }
    r
}

/// procfix6 (gate5 m3): a signal the guard did NOT send because its intent could not be made durable.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct NotSignalled {
    /// The process, the group (= the pinned leader's session), or 0 for a cgroup.
    pub pid: u32,
    pub start_ticks: Option<u64>,
    pub signal: i32,
    /// "process" | "group" | "cgroup".
    pub target: &'static str,
    pub marker: Option<String>,
    pub by: String,
    pub label: Option<String>,
    pub detail: Option<String>,
    /// The intent write failed for lack of space (ENOSPC / EDQUOT / EFBIG, after the reserve).
    pub disk_full: bool,
    pub error: String,
}

/// Refusals kept at most (beyond: counted in [`NOT_SIGNALLED_DROPPED`]).
const NOT_SIGNALLED_MAX: usize = 4096;
static NOT_SIGNALLED: std::sync::Mutex<Vec<NotSignalled>> = std::sync::Mutex::new(Vec::new());
static NOT_SIGNALLED_DROPPED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// Signals delivered by this process (every successful send; procfix6 m3: truthful counts in exit lines).
static DELIVERED_TOTAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn note_not_signalled(items: &[Intent], by: &str, label: Option<&str>, error: &str) {
    let disk_full = error.contains(DISK_FULL_WORD);
    let mut g = NOT_SIGNALLED.lock().unwrap_or_else(|p| p.into_inner());
    for it in items {
        if g.len() >= NOT_SIGNALLED_MAX {
            NOT_SIGNALLED_DROPPED.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            continue;
        }
        g.push(NotSignalled {
            pid: it.pid,
            start_ticks: it.start_ticks,
            signal: it.signal,
            target: it.target,
            marker: it.marker.clone(),
            by: by.to_string(),
            label: label.map(str::to_string),
            detail: it.detail.clone(),
            disk_full,
            error: error.to_string(),
        });
    }
}

/// Take every refusal recorded since the last call (procfix6 m3) and how many could not be kept.
pub fn take_not_signalled() -> (Vec<NotSignalled>, u64) {
    let v = std::mem::take(&mut *NOT_SIGNALLED.lock().unwrap_or_else(|p| p.into_inner()));
    (
        v,
        NOT_SIGNALLED_DROPPED.swap(0, std::sync::atomic::Ordering::SeqCst),
    )
}

/// Has an intent write of the INSTALLED (process-wide) intent log failed for lack of space (procfix6 m3)? The supervisor
/// then ends with exit 75 promptly.
pub fn intent_disk_full() -> bool {
    INTENTS
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .as_ref()
        .is_some_and(|l| l.disk_full)
}

/// Signals delivered by this process so far (every successful send of the guard).
pub fn delivered_total() -> u64 {
    DELIVERED_TOTAL.load(std::sync::atomic::Ordering::SeqCst)
}

/// The word every disk-full intent error carries (see `write_intents_to`).
const DISK_FULL_WORD: &str = "[disk full]";
/// procfix6 (gate5 m3): bytes PRE-ALLOCATED past the end of the intent log (`fallocate` KEEP_SIZE, best effort; refreshed
/// when less than half is left): appends land in blocks the file already owns, so intents of a stop can still be made
/// durable on a full disk where the file system supports it (ext4, xfs, tmpfs; not ZFS: the reserve file then helps).
const INTENT_PREALLOC_BYTES: i64 = 256 << 10;

/// Best effort: make sure about [`INTENT_PREALLOC_BYTES`] are allocated past the end of `file`.
fn prealloc_tail(file: &std::fs::File) {
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::MetadataExt;
    #[cfg(debug_assertions)]
    if test_reached("no_intent_prealloc") {
        return;
    }
    let Ok(m) = file.metadata() else { return };
    let len = m.len() as i64;
    let allocated = (m.blocks() as i64).saturating_mul(512);
    if allocated - len >= INTENT_PREALLOC_BYTES / 2 {
        return;
    }
    // SAFETY: fallocate on our own open descriptor with integer arguments; KEEP_SIZE leaves the file size unchanged.
    #[allow(clippy::let_underscore_must_use)]
    // ignored-ok (procfix7): best-effort pre-allocation (not a decision; the reserve and the tmpfs fallback cover a full disk)
    let _ = unsafe {
        libc::fallocate(
            file.as_raw_fd(),
            libc::FALLOC_FL_KEEP_SIZE,
            len,
            INTENT_PREALLOC_BYTES,
        )
    };
}

fn write_intents_to(
    log: Option<&mut IntentLog>,
    items: &[Intent],
    by: String,
    label: Option<String>,
) -> Result<Vec<Option<String>>, String> {
    use std::io::Write;
    let Some(log) = log else {
        return Ok(vec![None; items.len()]);
    };
    let utc = crate::cpu_policy::utc_now();
    let mut buf = Vec::with_capacity(items.len() * 320);
    let mut ids = Vec::with_capacity(items.len());
    for it in items {
        log.next += 1;
        let id = format!("{}-{}", log.session, log.next);
        let line = IntentLine {
            schema: INTENT_SCHEMA.into(),
            kind: "intent".into(),
            id: id.clone(),
            campaign: None,
            utc: Some(utc.clone()),
            pid: Some(it.pid),
            start_ticks: it.start_ticks,
            signal: Some(it.signal),
            target: Some(it.target.into()),
            marker: it.marker.clone(),
            proof: Some(it.proof.into()),
            by: Some(by.clone()),
            label: label.clone(),
            detail: it.detail.clone(),
            result: None,
        };
        serde_json::to_writer(&mut buf, &line).map_err(|e| e.to_string())?;
        buf.push(b'\n');
        ids.push(Some(id));
    }
    let attempt = |log: &mut IntentLog, lead: bool| -> io::Result<()> {
        #[cfg(debug_assertions)]
        if test_reached("intent_primary_enospc") {
            return Err(io::Error::from_raw_os_error(libc::ENOSPC));
        }
        if lead {
            // A torn line of the failed attempt ends here (the reader skips it).
            log.file.write_all(b"\n")?;
        }
        log.file.write_all(&buf)?;
        log.file.sync_data()
    };
    // procfix7 (gate6 m3-a): the tmpfs fallback (a leading newline ends a torn line of an earlier failed attempt).
    let to_fallback = |log: &mut IntentLog| -> Option<io::Result<()>> {
        let (f, _) = log.fallback.as_mut()?;
        Some(
            f.write_all(b"\n")
                .and_then(|()| f.write_all(&buf))
                .and_then(|()| f.sync_data()),
        )
    };
    #[cfg(debug_assertions)]
    if test_reached("intent_enospc") {
        log.disk_full = true;
        return Err(format!(
            "signal intent not durable: TEST BUILD intent_enospc {DISK_FULL_WORD}"
        ));
    }
    if log.on_fallback {
        return match to_fallback(log) {
            Some(Ok(())) => Ok(ids),
            Some(Err(e)) => Err(format!(
                "signal intent not durable: {e} (tmpfs fallback log) {}",
                if is_disk_full(&e) { DISK_FULL_WORD } else { "" }
            )),
            None => Err(format!(
                "signal intent not durable: no fallback log {DISK_FULL_WORD}"
            )),
        };
    }
    let word = |e: &io::Error| if is_disk_full(e) { DISK_FULL_WORD } else { "" };
    let r = match attempt(log, false) {
        Ok(()) => Ok(ids.clone()),
        Err(e) if is_disk_full(&e) && std::fs::remove_file(&log.reserve).is_ok() => {
            attempt(log, true).map(|()| ids.clone()).map_err(|e2| {
                format!(
                    "signal intent not durable: {e2} (after freeing the reserve; first: {e}) {}",
                    word(&e2)
                )
            })
        }
        Err(e) => Err(format!("signal intent not durable: {e} {}", word(&e))),
    };
    // procfix7 (gate6 m3-a): the campaign directory's log is full: the batch goes to the tmpfs fallback (durable against
    // a supervisor crash), the run still ends with exit 75 (disk full), and the signals ARE sent.
    let r = match r {
        Err(e) if e.contains(DISK_FULL_WORD) => match to_fallback(log) {
            Some(Ok(())) => {
                log.on_fallback = true;
                log.disk_full = true;
                Ok(ids)
            }
            Some(Err(e2)) => Err(format!("{e}; tmpfs fallback log: {e2}")),
            None => Err(e),
        },
        other => other,
    };
    match &r {
        Ok(_) if !log.on_fallback => prealloc_tail(&log.file),
        Ok(_) => {}
        Err(e) if e.contains(DISK_FULL_WORD) => log.disk_full = true,
        Err(_) => {}
    }
    r
}

/// Completion lines (not synced: a lost completion line only makes a delivered signal "possibly sent").
fn write_done(done: &[(Option<String>, String)]) {
    with_sink(|log| {
        if let Some(log) = log {
            write_done_to(log, done)
        }
    })
}

fn write_done_to(log: &mut IntentLog, done: &[(Option<String>, String)]) {
    use std::io::Write;
    let mut buf = Vec::new();
    for (id, result) in done {
        let Some(id) = id else { continue };
        let line = IntentLine {
            schema: INTENT_SCHEMA.into(),
            kind: "done".into(),
            id: id.clone(),
            campaign: None,
            utc: None,
            pid: None,
            start_ticks: None,
            signal: None,
            target: None,
            marker: None,
            proof: None,
            by: None,
            label: None,
            detail: None,
            result: Some(result.clone()),
        };
        if serde_json::to_writer(&mut buf, &line).is_ok() {
            buf.push(b'\n');
        }
    }
    if !buf.is_empty() {
        match (&mut log.fallback, log.on_fallback) {
            (Some((f, _)), true) => {
                #[allow(clippy::let_underscore_must_use)]
                // ignored-ok (procfix7): completion lines are not synced by design: a lost one only makes a delivered signal POSSIBLY SENT
                let _ = f.write_all(&buf);
            }
            _ => {
                #[allow(clippy::let_underscore_must_use)]
                // ignored-ok (procfix7): completion lines are not synced by design: a lost one only makes a delivered signal POSSIBLY SENT
                let _ = log.file.write_all(&buf);
            }
        }
    }
}

/// An intent of an earlier session with what its log says about it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoggedIntent {
    pub line: IntentLine,
    /// The completion result, if a `done` line exists.
    pub result: Option<String>,
}

/// Read an intent log: every intent with its completion (lines that do not parse, e.g. a torn tail, are counted).
pub fn read_intent_log(path: &Path) -> io::Result<(Vec<LoggedIntent>, usize)> {
    let text = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok((Vec::new(), 0)),
        Err(e) => return Err(e),
    };
    let mut order: Vec<String> = Vec::new();
    let mut intents: std::collections::BTreeMap<String, LoggedIntent> = Default::default();
    let mut bad = 0;
    for raw in text.split(|&b| b == b'\n') {
        if raw.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let Ok(l) = serde_json::from_slice::<IntentLine>(raw) else {
            bad += 1;
            continue;
        };
        if l.schema != INTENT_SCHEMA {
            bad += 1;
            continue;
        }
        match l.kind.as_str() {
            "intent" => {
                if !intents.contains_key(&l.id) {
                    order.push(l.id.clone());
                }
                intents.insert(
                    l.id.clone(),
                    LoggedIntent {
                        line: l,
                        result: None,
                    },
                );
            }
            "done" => {
                if let Some(i) = intents.get_mut(&l.id) {
                    i.result = l.result.clone();
                }
            }
            // procfix7 (gate7 m-fb): the header of a fallback log
            "header" => {}
            _ => bad += 1,
        }
    }
    Ok((
        order
            .into_iter()
            .filter_map(|id| intents.remove(&id))
            .collect(),
        bad,
    ))
}

/// The intent id a `signal_sent` record's `by` names (`... [intent <id>]` / `... [intent <id>; ...]`).
pub fn intent_id_in(by: &str) -> Option<&str> {
    let i = by.rfind("[intent ")?;
    let rest = &by[i + "[intent ".len()..];
    let end = rest.find([']', ';'])?;
    Some(&rest[..end])
}

/// Test builds only: the abort sites around a send (`RR_CAMPAIGN_FAILPOINT=signal_after_intent:<n>` /
/// `signal_after_send:<n>`). A release build has neither the sites nor their names.
#[cfg(debug_assertions)]
fn test_site(site: &str) {
    static FP: std::sync::OnceLock<crate::supervisor::failpoint::Failpoints> =
        std::sync::OnceLock::new();
    FP.get_or_init(crate::supervisor::failpoint::Failpoints::from_env)
        .hit(site);
}

/// Test builds only: a counted, non-aborting site (`RR_CAMPAIGN_FAILPOINT=intent_enospc:<n>`, procfix6 m3).
#[cfg(debug_assertions)]
fn test_reached(site: &str) -> bool {
    static FP: std::sync::OnceLock<crate::supervisor::failpoint::Failpoints> =
        std::sync::OnceLock::new();
    FP.get_or_init(crate::supervisor::failpoint::Failpoints::from_env)
        .reached(site)
}

macro_rules! site {
    ($s:literal) => {
        #[cfg(debug_assertions)]
        test_site($s);
    };
}

fn proof_word(p: &Proof) -> &'static str {
    match p {
        Proof::Marker(_) => "attempt marker",
        Proof::Campaign(_) => "campaign marker",
        Proof::PinnedSession(_) => "pinned session",
    }
}

// ------------------------------------------------------------------------------------------------ ownership

/// THE ONE OWNERSHIP RULE for a process FOUND BY ITS MARKER (gate3 M1 / M2; lane int5): the campaign sweep, the
/// end-of-job hunt and the job signaller all build their target with this function, and `judge` re-checks it at every
/// send. A process is ours iff (a) its environ carries THIS campaign's marker (`campaign_id` = spec name + BLAKE3 of
/// the directory's nonce, device and inode: a copied directory has another id) and, when `attempt` is given, exactly
/// that attempt label; (b) its owner uid is ours; (c) it started at or after the bound (`start`: the attempt's launch,
/// or the exact start ticks a scan read); (d) the boot is the recorded one; (e) it is a valid signal target
/// (`ValidPid::for_signal`: never this process, its parent, or the leader of our own session / group). WHERE it is
/// (cwd, session, open files) is NOT part of the rule: a process carrying this campaign's exact marker is ours wherever
/// its cwd is (run-book assumption A3: no foreign process carries it). `None`: the campaign id / label is not a valid
/// marker (nothing may be signalled).
pub(crate) fn marker_target(
    campaign_id: &str,
    attempt: Option<&str>,
    pid: u32,
    start: StartRule,
    boot_id: &str,
) -> Option<ProcessTarget> {
    let proof = match attempt {
        Some(a) => Proof::Marker(CampaignMarker::new(campaign_id, a).ok()?),
        None => {
            crate::resources::validate_campaign_id(campaign_id).ok()?;
            Proof::Campaign(campaign_id.to_string())
        }
    };
    Some(ProcessTarget {
        pid,
        boot_id: boot_id.to_string(),
        start,
        require_session_leader: false,
        proof,
    })
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

    /// Has the process of `fd` exited (its thread group is dead)? `poll(POLLIN, 0)` on the pidfd. None: poll failed.
    pub(super) fn pidfd_exited(fd: BorrowedFd<'_>) -> Option<bool> {
        let mut p = libc::pollfd {
            fd: fd.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: one valid pollfd, timeout 0 (no blocking).
        let r = unsafe { libc::poll(&mut p, 1, 0) };
        if r < 0 {
            return None;
        }
        Some(p.revents & (libc::POLLIN | libc::POLLHUP) != 0)
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
    /// The owner of `/proc/<pid>` (effective uid), read completely (procfix8: an unreadable owner fails the read).
    uid: u32,
    /// None: not read (not needed) ; Some(None): no marker ; Some(Some(v)): value.
    marker: Option<Result<Option<String>, String>>,
}

enum Read {
    Gone,
    Facts(Facts),
    Failed(String),
}

/// Bound of the environ read of the checklist (it takes the target's mmap lock).
const GUARD_ENVIRON_TIMEOUT: Duration = Duration::from_secs(1);

/// Steps 4-7 reads (blocking; run on a helper thread). `root` is the real /proc in production (a seam for tests).
/// procfix8 (gate8, the class): every read through [`procfs`]: Gone only for ENOENT / ESRCH of the PROCESS (never of a
/// thread); an unreadable thread list or owner is Failed (it was "zombie without threads" / "gone": a target judged gone
/// by a failed read made a hunt or sweep count it as dead).
fn read_facts(root: &ProcRoot, pid: u32, want_marker: bool) -> Read {
    let st = match root.stat(pid) {
        Ok(s) => s,
        Err(ProcError::Gone) => return Read::Gone,
        Err(ProcError::Unreadable(w)) => return Read::Failed(format!("stat: {w}")),
    };
    let mut tid = None;
    let mut zombie_without_threads = false;
    if st.is_dead() {
        // A dead main thread with live threads still runs (audit B M1): read the live thread's environ.
        match live_thread_at(root, pid) {
            Ok(LiveThread::Found(t, _)) => tid = Some(t),
            Ok(LiveThread::NoneAlive) => zombie_without_threads = true,
            Err(ProcError::Gone) => return Read::Gone,
            Err(ProcError::Unreadable(w)) => return Read::Failed(format!("thread list: {w}")),
        }
    }
    let uid = match root.owner_uid(pid) {
        Ok(u) => u,
        Err(ProcError::Gone) => return Read::Gone,
        Err(ProcError::Unreadable(w)) => return Read::Failed(format!("owner: {w}")),
    };
    let marker = if want_marker && !zombie_without_threads {
        Some(match root.environ(pid, tid, GUARD_ENVIRON_TIMEOUT) {
            Ok(b) if b.is_empty() => Err("environ empty (no mm)".into()),
            Ok(b) => Ok(marker_in_environ(&b)),
            Err(ProcError::Gone) if tid.is_none() => return Read::Gone,
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
    if f.uid != uid {
        return Err(Verdict::NotOurs(Refusal::OtherUid { uid: f.uid }));
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
            // Defence in depth: a stopped / frozen candidate with a marker is judged like any other.
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
    signal_verified_at(&procfs::sys(), t, sig, leaders, timeout)
}

/// Steps 1, 2 and (gate re-audit A m-1) the pinned-session PRE-check: for `Proof::PinnedSession(sid)` the session must
/// be pinned BEFORE the /proc reads it protects (the lock is not held across the reads; it is taken again, and the pin
/// re-checked, from the judgement through the send).
fn precheck(t: &ProcessTarget, sig: i32, leaders: Option<&Leaders>) -> Result<ValidPid, Verdict> {
    if !valid_signal(sig) {
        return Err(Verdict::Unverified(format!(
            "signal number {sig} out of range"
        )));
    }
    // 1.
    let pid = ValidPid::for_signal(i64::from(t.pid))
        .map_err(|e| Verdict::NotOurs(Refusal::InvalidPid(e)))?;
    // 2.
    match boot_id() {
        Ok(b) if b == t.boot_id => {}
        Ok(_) => return Err(Verdict::NotOurs(Refusal::OtherBoot)),
        Err(e) => return Err(Verdict::Unverified(format!("boot id unreadable: {e}"))),
    }
    if let Proof::PinnedSession(sid) = t.proof {
        let pinned_now = leaders.is_some_and(|l| l.lock().contains(&sid) && leader_unreaped(sid));
        if !pinned_now {
            return Err(Verdict::NotOurs(Refusal::SessionNotPinned { sid }));
        }
    }
    Ok(pid)
}

/// Step 3: a pidfd on `pid`. ESRCH = Gone; any other failure = Unverified, never a fallback (gate re-audit m-2/m-3).
fn open_pidfd_checked(pid: ValidPid) -> Result<Option<OwnedFd>, Verdict> {
    match pidfd_probe() {
        PidfdProbe::Supported => match sys::pidfd_open(pid) {
            Ok(fd) => Ok(Some(fd)),
            Err(e) if e.raw_os_error() == Some(libc::ESRCH) => Err(Verdict::Gone),
            Err(e) => Err(Verdict::Unverified(format!("pidfd_open: {e}"))),
        },
        PidfdProbe::Unsupported => Ok(None),
        // EMFILE / ENFILE / ENOMEM now: refuse this attempt (retry later), never the weaker kill path.
        PidfdProbe::Transient(e) => Err(Verdict::Unverified(format!(
            "pidfd probe failed transiently: {e}"
        ))),
    }
}

pub(crate) fn signal_verified_at(
    root: &ProcRoot,
    t: &ProcessTarget,
    sig: i32,
    leaders: Option<&Leaders>,
    timeout: Duration,
) -> Verdict {
    let pid = match precheck(t, sig, leaders) {
        Ok(p) => p,
        Err(v) => return v,
    };
    // 3.
    let pidfd = match open_pidfd_checked(pid) {
        Ok(fd) => fd,
        Err(v) => return v,
    };
    // 4-7 on a helper thread, bounded.
    let want_marker = matches!(t.proof, Proof::Marker(_) | Proof::Campaign(_));
    let read = {
        let root: ProcRoot = root.clone();
        let p = t.pid;
        match run_bounded(timeout, move || read_facts(&root, p, want_marker)).done() {
            Some(r) => r,
            None => return Verdict::Unverified(format!("proc reads of pid {p} timed out")),
        }
    };
    judge_one(t, read, pidfd.as_ref(), pid, sig, leaders)
}

/// The checklist on a pidfd the CALLER opened earlier (with [`open_pidfd`]) and still holds: the hunt opens a pidfd on
/// each candidate before classifying it, so the process judged here is the one that pidfd pins (a recycled number
/// makes the send fail with ESRCH).
pub(crate) fn signal_with_pidfd(
    fd: &OwnedFd,
    t: &ProcessTarget,
    sig: i32,
    leaders: Option<&Leaders>,
) -> Verdict {
    let pid = match precheck(t, sig, leaders) {
        Ok(p) => p,
        Err(v) => return v,
    };
    let want_marker = matches!(t.proof, Proof::Marker(_) | Proof::Campaign(_));
    let read = read_facts(&procfs::sys(), t.pid, want_marker);
    judge_one(t, read, Some(fd), pid, sig, leaders)
}

/// [`signal_with_pidfd`] for a BATCH of candidates whose pidfds the caller holds (a sweep round): the reads first, then
/// the judgements, the intents of all that passed with ONE fdatasync, then the sends (procfix5 M1: the hot path of a
/// freeze / kill round pays one sync, never a send before its intent).
pub(crate) fn signal_with_pidfds(
    items: &[(&OwnedFd, ProcessTarget)],
    sig: i32,
    leaders: Option<&Leaders>,
) -> Vec<Verdict> {
    with_pidfds_held(items, sig, leaders)
        .into_iter()
        .map(|(v, _)| v)
        .collect()
}

/// procfix6 (gate5 m1): [`signal_with_pidfds`] with SIGSTOP, returning for every DELIVERED stop the [`StopHold`] of that
/// process (to be killed, or continued, by the identity held at the stop: [`signal_held`]).
pub(crate) fn stop_with_pidfds(
    items: &[(&OwnedFd, ProcessTarget)],
    leaders: Option<&Leaders>,
) -> Vec<(Verdict, Option<StopHold>)> {
    with_pidfds_held(items, libc::SIGSTOP, leaders)
}

/// procfix6 (gate5 m1): [`signal_with_pidfd`] with SIGSTOP, returning the [`StopHold`] when the stop was delivered.
pub(crate) fn stop_with_pidfd(
    fd: &OwnedFd,
    t: &ProcessTarget,
    leaders: Option<&Leaders>,
) -> (Verdict, Option<StopHold>) {
    with_pidfds_held(&[(fd, t.clone())], libc::SIGSTOP, leaders)
        .pop()
        .unwrap_or_else(|| (Verdict::Unverified("internal: no verdict".into()), None))
}

fn with_pidfds_held(
    items: &[(&OwnedFd, ProcessTarget)],
    sig: i32,
    leaders: Option<&Leaders>,
) -> Vec<(Verdict, Option<StopHold>)> {
    let mut out: Vec<Option<(Verdict, Option<StopHold>)>> =
        (0..items.len()).map(|_| None).collect();
    let mut ready = Vec::new();
    for (k, (fd, t)) in items.iter().enumerate() {
        let pid = match precheck(t, sig, leaders) {
            Ok(p) => p,
            Err(v) => {
                out[k] = Some((v, None));
                continue;
            }
        };
        let want_marker = matches!(t.proof, Proof::Marker(_) | Proof::Campaign(_));
        match read_facts(&procfs::sys(), t.pid, want_marker) {
            Read::Gone => out[k] = Some((Verdict::Gone, None)),
            Read::Failed(e) => out[k] = Some((Verdict::Unverified(e), None)),
            Read::Facts(facts) => ready.push((
                k,
                Ready {
                    t,
                    pid,
                    fd: Some(*fd),
                    facts,
                },
            )),
        }
    }
    let (idx, items): (Vec<usize>, Vec<Ready<'_>>) = ready.into_iter().unzip();
    for (k, v) in idx
        .into_iter()
        .zip(judge_and_send_batch_held(items, sig, leaders))
    {
        out[k] = Some(v);
    }
    out.into_iter()
        .map(|v| v.unwrap_or_else(|| (Verdict::Unverified("internal: no verdict".into()), None)))
        .collect()
}

/// Fill the verdicts of the `ready` part of a batch through [`judge_and_send_batch`].
fn merge(
    mut out: Vec<Option<Verdict>>,
    ready: Vec<(usize, Ready<'_>)>,
    sig: i32,
    leaders: Option<&Leaders>,
) -> Vec<Verdict> {
    let (idx, items): (Vec<usize>, Vec<Ready<'_>>) = ready.into_iter().unzip();
    for (k, v) in idx
        .into_iter()
        .zip(judge_and_send_batch(items, sig, leaders))
    {
        out[k] = Some(v);
    }
    out.into_iter()
        .map(|v| v.unwrap_or_else(|| Verdict::Unverified("internal: no verdict".into())))
        .collect()
}

/// Open a pidfd on `pid` for a caller that classifies the process before deciding to signal it (the hunt). Refuses
/// targets that are never signalled (`ValidPid::for_signal`); ESRCH = Gone; no pidfd support = Unverified (the hunt has
/// no weaker path).
pub(crate) fn open_pidfd(pid: u32) -> Result<OwnedFd, Verdict> {
    let p = ValidPid::for_signal(i64::from(pid))
        .map_err(|e| Verdict::NotOurs(Refusal::InvalidPid(e)))?;
    match open_pidfd_checked(p)? {
        Some(fd) => Ok(fd),
        None => Err(Verdict::Unverified(
            "this kernel has no pidfds: the end-of-job proof needs them".into(),
        )),
    }
}

/// Has the process of a pidfd exited? (`Some(true)`: its thread group is dead, it is no survivor; reads made while it
/// was not yet exited described it.) None: the poll failed (inconclusive).
pub(crate) fn pidfd_exited(fd: &OwnedFd) -> Option<bool> {
    sys::pidfd_exited(fd_borrow(fd))
}

/// A target whose facts were read, ready for steps 7-8.
struct Ready<'a> {
    t: &'a ProcessTarget,
    pid: ValidPid,
    fd: Option<&'a OwnedFd>,
    facts: Facts,
}

/// Steps 7-8 for ONE target.
fn judge_one(
    t: &ProcessTarget,
    read: Read,
    pidfd: Option<&OwnedFd>,
    pid: ValidPid,
    sig: i32,
    leaders: Option<&Leaders>,
) -> Verdict {
    let facts = match read {
        Read::Gone => return Verdict::Gone,
        Read::Failed(e) => return Verdict::Unverified(e),
        Read::Facts(f) => f,
    };
    judge_and_send_batch(
        vec![Ready {
            t,
            pid,
            fd: pidfd,
            facts,
        }],
        sig,
        leaders,
    )
    .pop()
    .unwrap_or_else(|| Verdict::Unverified("internal: no verdict".into()))
}

/// How one send went.
enum Sent {
    Ok,
    Gone,
    Failed(String),
}

/// Steps 7 (judgement; the registry lock, when a registry is given, is held from the judgements through the sends) and
/// 8 (the sends) for a batch. procfix5 M1: between them the INTENTS of every target that passed are written with ONE
/// fdatasync; if that fails nothing is sent (`Unverified`). After the sends: completion lines and the delivery log.
fn judge_and_send_batch(
    items: Vec<Ready<'_>>,
    sig: i32,
    leaders: Option<&Leaders>,
) -> Vec<Verdict> {
    judge_and_send_batch_held(items, sig, leaders)
        .into_iter()
        .map(|(v, _)| v)
        .collect()
}

/// [`judge_and_send_batch`] that also returns, for every SIGSTOP delivered through a caller's pidfd, the [`StopHold`]
/// of the process (procfix6, gate5 m1).
fn judge_and_send_batch_held(
    items: Vec<Ready<'_>>,
    sig: i32,
    leaders: Option<&Leaders>,
) -> Vec<(Verdict, Option<StopHold>)> {
    let mut holds: Vec<Option<StopHold>> = (0..items.len()).map(|_| None).collect();
    let mut out: Vec<Option<Verdict>> = (0..items.len()).map(|_| None).collect();
    let guard = leaders.map(Leaders::lock);
    let pinned = |sid: u32| {
        guard
            .as_ref()
            .is_some_and(|g| g.contains(&sid) && leader_unreaped(sid))
    };
    let uid = our_uid();
    let mut pass = Vec::new();
    let mut intents = Vec::new();
    for (k, r) in items.iter().enumerate() {
        match judge(r.t, &r.facts, uid, &pinned) {
            Err(v) => out[k] = Some(v),
            Ok(()) => {
                pass.push(k);
                intents.push(Intent {
                    pid: r.t.pid,
                    start_ticks: Some(r.facts.start_ticks),
                    signal: sig,
                    target: "process",
                    marker: marker_read(&r.facts),
                    proof: proof_word(&r.t.proof),
                    detail: None,
                });
            }
        }
    }
    let ids = match write_intents(&intents) {
        Ok(ids) => ids,
        Err(e) => {
            drop(guard);
            for &k in &pass {
                out[k] = Some(Verdict::Unverified(e.clone()));
            }
            return finish_verdicts(out)
                .into_iter()
                .map(|v| (v, None))
                .collect();
        }
    };
    if !pass.is_empty() {
        site!("signal_after_intent");
    }
    // 8.
    let mut sent = Vec::with_capacity(pass.len());
    for &k in &pass {
        let r = &items[k];
        let res = match r.fd {
            Some(fd) => match sys::pidfd_send_signal(fd_borrow(fd), sig) {
                Ok(()) => Sent::Ok,
                Err(e) if e.raw_os_error() == Some(libc::ESRCH) => Sent::Gone,
                Err(e) => Sent::Failed(format!("send: {e}")),
            },
            // No pidfd: re-read the start time right before kill (residual race of microseconds, documented).
            None => match crate::resources::read_stat(r.t.pid) {
                Ok(st) if st.start_ticks == r.facts.start_ticks => {
                    match sys::kill_pid(r.pid, sig) {
                        Ok(()) => Sent::Ok,
                        Err(e) if e.raw_os_error() == Some(libc::ESRCH) => Sent::Gone,
                        Err(e) => Sent::Failed(format!("send: {e}")),
                    }
                }
                // another start time: the pid names a newer process, the target is dead; or ENOENT / ESRCH
                Ok(_) | Err(ProcError::Gone) => Sent::Gone,
                Err(ProcError::Unreadable(w)) => Sent::Failed(format!("re-check: {w}")),
            },
        };
        sent.push(res);
    }
    drop(guard);
    if !pass.is_empty() {
        site!("signal_after_send");
    }
    let mut done = Vec::with_capacity(pass.len());
    for ((&k, res), id) in pass.iter().zip(sent).zip(ids) {
        let r = &items[k];
        let (v, word) = match res {
            Sent::Ok => {
                log_delivery(
                    r.t.pid,
                    Some(r.facts.start_ticks),
                    sig,
                    "process",
                    marker_read(&r.facts),
                    proof_word(&r.t.proof),
                    None,
                    id.clone(),
                );
                if sig == libc::SIGSTOP && r.fd.is_some() {
                    holds[k] = Some(StopHold {
                        pid: r.t.pid,
                        start_ticks: r.facts.start_ticks,
                        boot_id: r.t.boot_id.clone(),
                        marker: marker_read(&r.facts),
                        proof: proof_word(&r.t.proof),
                    });
                }
                (Verdict::Delivered, "delivered".to_string())
            }
            Sent::Gone => (Verdict::Gone, "gone".to_string()),
            Sent::Failed(e) => (Verdict::Unverified(e.clone()), format!("failed: {e}")),
        };
        out[k] = Some(v);
        done.push((id, word));
    }
    write_done(&done);
    finish_verdicts(out).into_iter().zip(holds).collect()
}

fn finish_verdicts(out: Vec<Option<Verdict>>) -> Vec<Verdict> {
    out.into_iter()
        .map(|v| v.unwrap_or_else(|| Verdict::Unverified("internal: no verdict".into())))
        .collect()
}

/// The marker value the checklist read (None for a pinned-session proof).
fn marker_read(f: &Facts) -> Option<String> {
    match &f.marker {
        Some(Ok(Some(v))) => Some(v.clone()),
        _ => None,
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
    signal_pinned_groups(leaders, &[sid], sig)
        .pop()
        .unwrap_or_else(|| Verdict::Unverified("internal: no verdict".into()))
}

/// [`signal_pinned_group`] for several groups under ONE hold of the registry lock: the pins are checked, the intents
/// of all pinned groups written with one fdatasync (procfix5 M1), then each group is signalled.
pub(crate) fn signal_pinned_groups(leaders: &Leaders, sids: &[u32], sig: i32) -> Vec<Verdict> {
    if !valid_signal(sig) {
        return sids
            .iter()
            .map(|_| Verdict::Unverified(format!("signal number {sig} out of range")))
            .collect();
    }
    let mut out: Vec<Option<Verdict>> = vec![None; sids.len()];
    let guard = leaders.lock();
    let mut pass = Vec::new();
    for (k, &sid) in sids.iter().enumerate() {
        let pgid = match ValidPid::for_signal(i64::from(sid)) {
            Ok(p) => p,
            Err(e) => {
                out[k] = Some(Verdict::NotOurs(Refusal::InvalidPid(e)));
                continue;
            }
        };
        if !guard.contains(&sid) || !leader_unreaped(sid) {
            out[k] = Some(Verdict::NotOurs(Refusal::SessionNotPinned { sid }));
            continue;
        }
        pass.push((k, sid, pgid));
    }
    let intents: Vec<Intent> = pass
        .iter()
        .map(|&(_, sid, _)| Intent {
            pid: sid,
            start_ticks: None,
            signal: sig,
            target: "group",
            marker: None,
            proof: "pinned group",
            detail: None,
        })
        .collect();
    let ids = match write_intents(&intents) {
        Ok(ids) => ids,
        Err(e) => {
            drop(guard);
            for &(k, _, _) in &pass {
                out[k] = Some(Verdict::Unverified(e.clone()));
            }
            return finish_verdicts(out);
        }
    };
    if !pass.is_empty() {
        site!("signal_after_intent");
    }
    let results: Vec<io::Result<()>> = pass
        .iter()
        .map(|&(_, _, pgid)| sys::killpg_pinned(pgid, sig))
        .collect();
    drop(guard);
    if !pass.is_empty() {
        site!("signal_after_send");
    }
    let mut done = Vec::with_capacity(pass.len());
    for ((&(k, sid, _), r), id) in pass.iter().zip(results).zip(ids) {
        let (v, word) = match r {
            Ok(()) => {
                log_delivery(
                    sid,
                    None,
                    sig,
                    "group",
                    None,
                    "pinned group",
                    None,
                    id.clone(),
                );
                (Verdict::Delivered, "delivered".to_string())
            }
            Err(e) if e.raw_os_error() == Some(libc::ESRCH) => (Verdict::Gone, "gone".into()),
            Err(e) => (
                Verdict::Unverified(format!("killpg: {e}")),
                format!("failed: killpg: {e}"),
            ),
        };
        out[k] = Some(v);
        done.push((id, word));
    }
    write_done(&done);
    finish_verdicts(out)
}

/// SIGKILL to every pinned leader group at once (fast exit, audit A M6: "before any fast exit, killpg every unreaped
/// leader group"): one batch, one intent sync. Returns how many groups were signalled.
pub(crate) fn kill_all_pinned(leaders: &Leaders) -> usize {
    let sids: Vec<u32> = leaders.lock().iter().copied().collect();
    signal_pinned_groups(leaders, &sids, libc::SIGKILL)
        .iter()
        .filter(|v| v.delivered())
        .count()
}

/// The pid a pidfd names (`Pid:` of `/proc/self/fdinfo/<fd>`, in our pid namespace): Ok(None) when its process has
/// exited (-1 / 0); Err when the line cannot be read.
fn pidfd_pid(fd: &OwnedFd) -> Result<procfs::PidfdPid, String> {
    use std::os::fd::AsRawFd;
    procfs::sys()
        .self_fdinfo_pid(fd.as_raw_fd())
        .map_err(|e| format!("fdinfo of the pidfd: {e}"))
}

/// procfix6 (gate5 m1): SIGKILL or SIGCONT to processes this guard STOPPED, by the identity held at the stop: the SAME
/// pidfd the stop went through (its fdinfo must name the held pid), the boot, the exact start time read again now, and
/// for SIGKILL our uid as owner of `/proc/<pid>`. NO marker is required: the marker was proven by the checklist that
/// preceded the stop on this very pidfd (the process cannot have been replaced: the pidfd pins it, a recycled number
/// makes the send fail with ESRCH); it may have dropped the marker by an exec between that read and the stop. SIGCONT
/// does not require the uid (it only undoes our own stop). Every send is behind a durable intent (proof "stop hold",
/// the marker read at the stop). Any other signal is refused.
pub(crate) fn signal_held(items: &[(&OwnedFd, &StopHold)], sig: i32) -> Vec<Verdict> {
    if sig != libc::SIGKILL && sig != libc::SIGCONT {
        return items
            .iter()
            .map(|_| {
                Verdict::Unverified(format!(
                    "signal {sig}: a stop hold allows SIGKILL / SIGCONT only"
                ))
            })
            .collect();
    }
    let mut out: Vec<Option<Verdict>> = vec![None; items.len()];
    let uid = our_uid();
    let boot = boot_id();
    let mut pass = Vec::new();
    for (k, (fd, h)) in items.iter().enumerate() {
        let check = || -> Result<(), Verdict> {
            ValidPid::for_signal(i64::from(h.pid))
                .map_err(|e| Verdict::NotOurs(Refusal::InvalidPid(e)))?;
            match &boot {
                Ok(b) if *b == h.boot_id => {}
                Ok(_) => return Err(Verdict::NotOurs(Refusal::OtherBoot)),
                Err(e) => return Err(Verdict::Unverified(format!("boot id unreadable: {e}"))),
            }
            match pidfd_pid(fd) {
                Ok(procfs::PidfdPid::Pid(p)) if p == h.pid => {}
                Ok(procfs::PidfdPid::Exited) => return Err(Verdict::Gone),
                Ok(procfs::PidfdPid::Pid(p)) => {
                    return Err(Verdict::Unverified(format!(
                        "the pidfd names pid {p}, not the held pid {}",
                        h.pid
                    )));
                }
                Err(e) => return Err(Verdict::Unverified(e)),
            }
            let f = match read_facts(&procfs::sys(), h.pid, false) {
                Read::Gone => return Err(Verdict::Gone),
                Read::Failed(e) => return Err(Verdict::Unverified(e)),
                Read::Facts(f) => f,
            };
            if f.zombie_without_threads {
                return Err(Verdict::Gone);
            }
            if f.start_ticks != h.start_ticks {
                return Err(Verdict::NotOurs(Refusal::StartMismatch {
                    recorded: h.start_ticks,
                    current: f.start_ticks,
                }));
            }
            if sig == libc::SIGKILL && f.uid != uid {
                return Err(Verdict::NotOurs(Refusal::OtherUid { uid: f.uid }));
            }
            // The reads above described the pidfd's process only if it had not exited by now.
            if pidfd_exited(fd) == Some(true) {
                return Err(Verdict::Gone);
            }
            Ok(())
        };
        match check() {
            Ok(()) => pass.push(k),
            Err(v) => out[k] = Some(v),
        }
    }
    let intents: Vec<Intent> = pass
        .iter()
        .map(|&k| {
            let h = items[k].1;
            Intent {
                pid: h.pid,
                start_ticks: Some(h.start_ticks),
                signal: sig,
                target: "process",
                marker: h.marker.clone(),
                proof: "stop hold",
                detail: Some(format!(
                    "SIGSTOPped by this process through the same pidfd after the {} check; marker read then",
                    h.proof
                )),
            }
        })
        .collect();
    let ids = match write_intents(&intents) {
        Ok(ids) => ids,
        Err(e) => {
            for &k in &pass {
                out[k] = Some(Verdict::Unverified(e.clone()));
            }
            return finish_verdicts(out);
        }
    };
    if !pass.is_empty() {
        site!("signal_after_intent");
    }
    let sent: Vec<Sent> = pass
        .iter()
        .map(
            |&k| match sys::pidfd_send_signal(fd_borrow(items[k].0), sig) {
                Ok(()) => Sent::Ok,
                Err(e) if e.raw_os_error() == Some(libc::ESRCH) => Sent::Gone,
                Err(e) => Sent::Failed(format!("send: {e}")),
            },
        )
        .collect();
    if !pass.is_empty() {
        site!("signal_after_send");
    }
    let mut done = Vec::with_capacity(pass.len());
    for ((&k, res), id) in pass.iter().zip(sent).zip(ids) {
        let h = items[k].1;
        let (v, word) = match res {
            Sent::Ok => {
                log_delivery(
                    h.pid,
                    Some(h.start_ticks),
                    sig,
                    "process",
                    h.marker.clone(),
                    "stop hold",
                    None,
                    id.clone(),
                );
                (Verdict::Delivered, "delivered".to_string())
            }
            Sent::Gone => (Verdict::Gone, "gone".to_string()),
            Sent::Failed(e) => (Verdict::Unverified(e.clone()), format!("failed: {e}")),
        };
        out[k] = Some(v);
        done.push((id, word));
    }
    write_done(&done);
    finish_verdicts(out)
}

/// [`signal_held`] with SIGKILL (procfix6 m1).
pub(crate) fn kill_held(items: &[(&OwnedFd, &StopHold)]) -> Vec<Verdict> {
    signal_held(items, libc::SIGKILL)
}

/// [`signal_held`] with SIGCONT (procfix6 m1): undo our own stop.
pub(crate) fn continue_held(items: &[(&OwnedFd, &StopHold)]) -> Vec<Verdict> {
    signal_held(items, libc::SIGCONT)
}

/// A cgroup-wide kill (`cgroup.kill` of a job cgroup / job scope WE own, written by `write`): its intent is made durable
/// first, the completion and the delivery are logged after (procfix5 M1). Without a durable intent nothing is written.
pub(crate) fn cgroup_kill_logged(
    dir: &Path,
    proof: &'static str,
    write: impl FnOnce() -> io::Result<()>,
) -> io::Result<()> {
    let detail = Some(dir.display().to_string());
    let ids = write_intents(&[Intent {
        pid: 0,
        start_ticks: None,
        signal: libc::SIGKILL,
        target: "cgroup",
        marker: None,
        proof,
        detail: detail.clone(),
    }])
    .map_err(io::Error::other)?;
    let id = ids.into_iter().next().flatten();
    site!("signal_after_intent");
    let r = write();
    site!("signal_after_send");
    match &r {
        Ok(()) => {
            log_delivery(
                0,
                None,
                libc::SIGKILL,
                "cgroup",
                None,
                proof,
                detail,
                id.clone(),
            );
            write_done(&[(id, "delivered".into())]);
        }
        Err(e) => write_done(&[(id, format!("failed: {e}"))]),
    }
    r
}

/// Reap exactly (pid, start) if it is a ZOMBIE CHILD of ours outside our own session: pidfd first (it pins the process),
/// then a stat read under that pidfd (start time, dead, parent = us, session != `own_session`), then
/// waitid(P_PIDFD, WEXITED | WNOHANG). Never a catch-all wait. The caller (the adopted-children reaper) holds the leader
/// registry lock and has checked that `pid` is not a registered leader: a registered leader is never reaped here
/// (the invariant behind the pinned group signal).
pub(crate) fn reap_adopted(pid: u32, start_ticks: u64, own_session: u32) -> bool {
    let Ok(p) = ValidPid::for_signal(i64::from(pid)) else {
        return false;
    };
    if !pidfd_supported() {
        return false;
    }
    let Ok(fd) = sys::pidfd_open(p) else {
        return false;
    };
    // SAFETY: getpid cannot fail.
    let me = unsafe { libc::getpid() } as u32;
    match crate::resources::read_stat(pid) {
        Ok(st)
            if st.start_ticks == start_ticks
                && st.is_dead()
                && st.ppid == me
                && st.session != own_session =>
        {
            sys::reap_pidfd(&fd)
        }
        // not reaped now (it waits for a later tick); a failed read never reaps
        Ok(_) | Err(_) => false,
    }
}

/// The checklist for several targets inline: per target the pre-check, the pidfd and the reads, then ONE judgement /
/// intent / send batch (procfix5 M1: one intent sync per batch).
fn signal_batch_inline(
    targets: &[ProcessTarget],
    sig: i32,
    leaders: Option<&Leaders>,
) -> Vec<Verdict> {
    let mut out: Vec<Option<Verdict>> = vec![None; targets.len()];
    let mut fds: Vec<(usize, ValidPid, Option<OwnedFd>)> = Vec::new();
    for (k, t) in targets.iter().enumerate() {
        let pid = match precheck(t, sig, leaders) {
            Ok(p) => p,
            Err(v) => {
                out[k] = Some(v);
                continue;
            }
        };
        match open_pidfd_checked(pid) {
            Ok(fd) => fds.push((k, pid, fd)),
            Err(v) => out[k] = Some(v),
        }
    }
    let mut ready = Vec::new();
    for (k, pid, fd) in &fds {
        let t = &targets[*k];
        let want_marker = matches!(t.proof, Proof::Marker(_) | Proof::Campaign(_));
        match read_facts(&procfs::sys(), t.pid, want_marker) {
            Read::Gone => out[*k] = Some(Verdict::Gone),
            Read::Failed(e) => out[*k] = Some(Verdict::Unverified(e)),
            Read::Facts(facts) => ready.push((
                *k,
                Ready {
                    t,
                    pid: *pid,
                    fd: fd.as_ref(),
                    facts,
                },
            )),
        }
    }
    merge(out, ready, sig, leaders)
}

/// Signal several processes through the checklist on ONE helper thread, waiting at most `timeout` for the whole batch
/// (gate re-audit A m-3: N stalled /proc reads no longer block the control loop N x 2 s). Targets not finished within the
/// bound are still handled (each verified at its own send time) in the background; their verdicts are reported as
/// `Unverified("batch bound")`.
pub(crate) fn signal_verified_batch(
    targets: Vec<ProcessTarget>,
    sig: i32,
    leaders: Option<std::sync::Arc<Leaders>>,
    timeout: Duration,
) -> Vec<Verdict> {
    if targets.is_empty() {
        return Vec::new();
    }
    let n = targets.len();
    let ctx = current_context();
    let work = move || {
        let run = || signal_batch_inline(&targets, sig, leaders.as_deref());
        match &ctx {
            Some((by, label)) => with_context(by, label.as_deref(), run),
            None => run(),
        }
    };
    match run_bounded(timeout, work).done() {
        Some(v) => v,
        None => vec![Verdict::Unverified("batch bound reached".into()); n],
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

/// procfix5 (gate4 m2): a supervisor that exceeds `ulimit -f` must not die by SIGXFSZ (gate4: rc 153, a dumped core):
/// with the signal ignored the write fails with EFBIG, which the ledger and the atomic writes report as "disk full"
/// (exit 75). Children get SIG_DFL back before their exec (`real::child_setup`): their own behaviour is unchanged.
pub fn ignore_sigxfsz() -> io::Result<()> {
    // SAFETY: a zeroed sigaction with SIG_IGN and no flags.
    let mut new: libc::sigaction = unsafe { std::mem::zeroed() };
    new.sa_sigaction = libc::SIG_IGN;
    // SAFETY: valid sigaction pointer; the old action is not needed.
    if unsafe { libc::sigaction(libc::SIGXFSZ, &new, std::ptr::null_mut()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
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

#[allow(clippy::let_underscore_must_use)] // test cleanup (outside the reviewed scope, procfix7)
#[cfg(test)]
mod tests {
    use super::*;

    fn facts(start: u64, session: u32, uid: u32, marker: Option<&str>) -> Facts {
        Facts {
            zombie_without_threads: false,
            start_ticks: start,
            session,
            uid,
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

    /// Gate re-audit A m-1: for a pinned-session proof the pin is checked BEFORE the /proc reads it protects. A target
    /// that does not exist, with a session that is not pinned: refused as SessionNotPinned by the pre-check (before the
    /// fix the reads came first and answered Gone). Signal 0, nothing can be delivered either way.
    #[test]
    fn ra_m1_pinned_session_is_checked_before_the_reads() {
        let leaders = Leaders::default();
        let t = ProcessTarget {
            pid: crate::resources::pid_max() - 1,
            boot_id: boot_id().unwrap(),
            start: StartRule::NotBefore(0),
            require_session_leader: false,
            proof: Proof::PinnedSession(4242),
        };
        assert_eq!(
            signal_verified(&t, 0, Some(&leaders), Duration::from_millis(500)),
            Verdict::NotOurs(Refusal::SessionNotPinned { sid: 4242 })
        );
        assert_eq!(
            signal_verified(&t, 0, None, Duration::from_millis(500)),
            Verdict::NotOurs(Refusal::SessionNotPinned { sid: 4242 })
        );
    }

    /// A `sleep 30` child of this test carrying `marker`, after its exec (its environ shows the marker).
    fn marked_sleep(marker: &str) -> std::process::Child {
        let child = std::process::Command::new("sleep")
            .arg("30")
            .env_clear()
            .env(MARKER_VAR, marker)
            .spawn()
            .unwrap();
        let t0 = std::time::Instant::now();
        while std::fs::read(format!("/proc/{}/environ", child.id()))
            .map(|b| marker_in_environ(&b).is_none())
            .unwrap_or(true)
        {
            assert!(t0.elapsed() < Duration::from_secs(10), "child never exec'd");
            std::thread::sleep(Duration::from_millis(5));
        }
        child
    }

    fn tmpdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "rr-guard-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// procfix7 (gate6 m3-a): the campaign directory's intent log is FULL (here /dev/full: every write ENOSPC). Without
    /// a fallback the intent is not durable (nothing may be sent, disk full); with the tmpfs fallback the batch is durable
    /// there, and every later intent and completion line goes there too (the restart merges it by intent id).
    #[test]
    fn m3a_a_full_log_switches_to_the_fallback() {
        let full = tmpdir("m3a-full");
        std::os::unix::fs::symlink("/dev/full", full.join(INTENT_LOG_FILE)).unwrap();
        let fb_dir = tmpdir("m3a-fb");
        let fb_path = fb_dir.join("s.jsonl");
        let mk = |fallback: bool| IntentLog {
            file: std::fs::OpenOptions::new()
                .append(true)
                .open(full.join(INTENT_LOG_FILE))
                .unwrap(),
            reserve: full.join(INTENT_RESERVE_FILE),
            session: "s".into(),
            next: 0,
            disk_full: false,
            fallback: fallback.then(|| {
                (
                    std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&fb_path)
                        .unwrap(),
                    fb_path.clone(),
                )
            }),
            on_fallback: false,
        };
        let it = Intent {
            pid: 4242,
            start_ticks: Some(7),
            signal: libc::SIGKILL,
            target: "process",
            marker: None,
            proof: "unit",
            detail: None,
        };
        let mut l = mk(false);
        let e = write_intents_to(Some(&mut l), std::slice::from_ref(&it), "by".into(), None)
            .unwrap_err();
        assert!(
            e.contains(DISK_FULL_WORD) && l.disk_full && !l.on_fallback,
            "{e}"
        );
        let mut l = mk(true);
        let ids =
            write_intents_to(Some(&mut l), std::slice::from_ref(&it), "by".into(), None).unwrap();
        assert!(l.on_fallback && l.disk_full);
        write_done_to(&mut l, &[(ids[0].clone(), "delivered".into())]);
        let ids2 =
            write_intents_to(Some(&mut l), std::slice::from_ref(&it), "by".into(), None).unwrap();
        let (v, bad) = read_intent_log(&fb_path).unwrap();
        assert_eq!(bad, 0);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].result.as_deref(), Some("delivered"));
        assert_eq!(Some(v[1].line.id.clone()), ids2[0]);
        let _ = std::fs::remove_dir_all(&full);
        let _ = std::fs::remove_dir_all(&fb_dir);
    }

    /// procfix5 (gate4 M1): no durable intent, no signal; with one, the intent precedes the send, the completion
    /// follows, and the delivery names the intent. Runs inside the pid namespace of the crate's test run (rule L10).
    #[test]
    fn m1_a_signal_is_sent_only_after_its_durable_intent() {
        let m = CampaignMarker::new("c1-intent", "j1:a1").unwrap();
        let mut child = marked_sleep(&m.value());
        let pid = child.id();
        let st = crate::resources::read_stat(pid).unwrap();
        let t = ProcessTarget {
            pid,
            boot_id: boot_id().unwrap(),
            start: StartRule::Exact(st.start_ticks),
            require_session_leader: false,
            proof: Proof::Marker(m.clone()),
        };
        // 1. the intent cannot be made durable (the log is /dev/full): NOT sent
        let full = tmpdir("full");
        std::os::unix::fs::symlink("/dev/full", full.join(INTENT_LOG_FILE)).unwrap();
        let fd = open_pidfd(pid).unwrap();
        let v = with_intent_log(&full, "t-full", || {
            signal_with_pidfd(&fd, &t, libc::SIGKILL, None)
        });
        assert!(
            matches!(&v, Verdict::Unverified(e) if e.contains("not durable")),
            "{v:?}"
        );
        std::thread::sleep(Duration::from_millis(50));
        assert!(
            child.try_wait().unwrap().is_none(),
            "nothing may have been sent"
        );
        // 2. a working log: intent, send, completion; the delivery carries the intent id
        let ok = tmpdir("ok");
        let v = with_intent_log(&ok, "t-ok", || {
            with_context("unit test", Some("j1:a1"), || {
                signal_with_pidfd(&fd, &t, libc::SIGKILL, None)
            })
        });
        assert_eq!(v, Verdict::Delivered);
        child.wait().unwrap();
        let (intents, bad) = read_intent_log(&ok.join(INTENT_LOG_FILE)).unwrap();
        assert_eq!(bad, 0);
        assert_eq!(intents.len(), 1);
        let i = &intents[0];
        assert_eq!(i.line.pid, Some(pid));
        assert_eq!(i.line.start_ticks, Some(st.start_ticks));
        assert_eq!(i.line.signal, Some(libc::SIGKILL));
        assert_eq!(i.line.marker.as_deref(), Some(m.value().as_str()));
        assert_eq!(i.line.by.as_deref(), Some("unit test"));
        assert_eq!(i.result.as_deref(), Some("delivered"));
        let (ds, _) = take_deliveries();
        assert!(
            ds.iter()
                .any(|d| d.pid == pid && d.intent.as_deref() == Some(i.line.id.as_str())),
            "the delivery names its intent"
        );
        let _ = std::fs::remove_dir_all(&full);
        let _ = std::fs::remove_dir_all(&ok);
    }

    /// procfix5 M1: the intent log reader skips a torn tail, pairs completions, and `intent_id_in` reads the id back
    /// from a `signal_sent.by` text.
    #[test]
    fn m1_intent_log_reader_and_id_parse() {
        let d = tmpdir("read");
        let p = d.join(INTENT_LOG_FILE);
        let a = r#"{"schema":"rustred.campaign.signal-intent.v1","kind":"intent","id":"s-1","pid":5,"signal":19,"target":"process","proof":"campaign marker","by":"campaign sweep"}"#;
        let b = r#"{"schema":"rustred.campaign.signal-intent.v1","kind":"intent","id":"s-2","pid":5,"signal":9,"target":"process","proof":"campaign marker","by":"campaign sweep"}"#;
        let c = r#"{"schema":"rustred.campaign.signal-intent.v1","kind":"done","id":"s-1","result":"delivered"}"#;
        std::fs::write(&p, format!("{a}\n{b}\n{c}\n{{\"schema\":\"rustred.camp")).unwrap();
        let (v, bad) = read_intent_log(&p).unwrap();
        assert_eq!(bad, 1);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].result.as_deref(), Some("delivered"));
        assert_eq!(v[1].result, None);
        assert_eq!(intent_id_in("campaign sweep [intent s-2]"), Some("s-2"));
        assert_eq!(
            intent_id_in("x (cg) [intent b.1.2-7; recovered at restart: POSSIBLY SENT]"),
            Some("b.1.2-7")
        );
        assert_eq!(intent_id_in("campaign sweep"), None);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// procfix5 M1: the cost of the durable intent on the hot path (printed; run with --nocapture): N single intents
    /// (one fdatasync each) against one batch of N (one fdatasync), in TMPDIR and, if set, in RR_INTENT_BENCH_DIR.
    #[test]
    fn m1_intent_cost() {
        let mut dirs = vec![tmpdir("cost")];
        if let Some(x) = std::env::var_os("RR_INTENT_BENCH_DIR") {
            let d = PathBuf::from(x).join(format!("rr-intent-bench-{}", std::process::id()));
            std::fs::create_dir_all(&d).unwrap();
            dirs.push(d);
        }
        const N: usize = 200;
        let it = Intent {
            pid: 4242,
            start_ticks: Some(1),
            signal: libc::SIGKILL,
            target: "process",
            marker: Some("c:j:a1".into()),
            proof: "attempt marker",
            detail: None,
        };
        for d in &dirs {
            let (single, batch) = with_intent_log(d, "bench", || {
                let t0 = std::time::Instant::now();
                for _ in 0..N {
                    write_intents(std::slice::from_ref(&it)).unwrap();
                }
                let single = t0.elapsed();
                let many = vec![it.clone(); N];
                let t1 = std::time::Instant::now();
                write_intents(&many).unwrap();
                (single, t1.elapsed())
            });
            eprintln!(
                "INTENT COST {}: {N} single intents {:?} ({:?} each); one batch of {N}: {:?}",
                d.display(),
                single,
                single / N as u32,
                batch
            );
            let _ = std::fs::remove_dir_all(d);
        }
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
        let real = crate::resources::read_stat(pid).unwrap();
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
        let v = signal_verified_at(
            &ProcRoot::at(&root),
            &t,
            libc::SIGKILL,
            None,
            Duration::from_millis(300),
        );
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

    // ------------------------------------------------------------------------------------- procfix6 (gate5 m1)

    /// A `sh` child carrying `marker` that execs into an UNMARKED `sleep 30` once it reads a line on stdin.
    fn dropping_child(marker: &str) -> std::process::Child {
        let child = std::process::Command::new("sh")
            .args(["-c", "read x; exec env -u RR_CAMPAIGN_JOB sleep 30"])
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env(MARKER_VAR, marker)
            .stdin(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let t0 = std::time::Instant::now();
        while std::fs::read(format!("/proc/{}/environ", child.id()))
            .map(|b| marker_in_environ(&b).is_none())
            .unwrap_or(true)
        {
            assert!(
                t0.elapsed() < Duration::from_secs(10),
                "child never started"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        child
    }

    /// Test code: SIGSTOP / SIGCONT to our own direct child through the shell's `kill` builtin (the crate's static checks
    /// allow no other `kill` call outside the guard's `sys` module, test modules included).
    fn sh_kill(sig: &str, pid: u32) {
        let st = std::process::Command::new("sh")
            .args(["-c", &format!("kill -{sig} {pid}")])
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .status()
            .unwrap();
        assert!(st.success(), "kill -{sig} {pid}");
    }

    fn state_of(pid: u32) -> Option<char> {
        let s = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        s[s.rfind(')')? + 2..].chars().next()
    }

    fn wait_until(what: &str, f: impl Fn() -> bool) {
        let t0 = std::time::Instant::now();
        while !f() {
            assert!(t0.elapsed() < Duration::from_secs(10), "timed out: {what}");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// The gate5 m1 situation made deterministic: the guard STOPS a marked child (and returns its hold); the child
    /// then drops the marker (here: continued by its parent, it execs into an unmarked image, and is stopped again by
    /// the parent). Returns (child, pidfd, hold, target).
    fn stopped_without_marker(
        tag: &str,
        log: &Path,
    ) -> (std::process::Child, OwnedFd, StopHold, ProcessTarget) {
        use std::io::Write;
        let m = CampaignMarker::new(format!("c1-{tag}"), "j1:a1").unwrap();
        let mut a = dropping_child(&m.value());
        let pid = a.id();
        let st = crate::resources::read_stat(pid).unwrap();
        let t = ProcessTarget {
            pid,
            boot_id: boot_id().unwrap(),
            start: StartRule::Exact(st.start_ticks),
            require_session_leader: false,
            proof: Proof::Marker(m.clone()),
        };
        let fd = open_pidfd(pid).unwrap();
        let (v, hold) = with_intent_log(log, "t-hold", || stop_with_pidfd(&fd, &t, None));
        assert_eq!(v, Verdict::Delivered);
        let hold = hold.expect("a delivered stop gives a hold");
        assert_eq!(hold.pid(), pid);
        assert_eq!(hold.marker.as_deref(), Some(m.value().as_str()));
        wait_until("stopped", || state_of(pid) == Some('T'));
        a.stdin.take().unwrap().write_all(b"go\n").unwrap();
        // test code; our own direct child (not reaped: its pid cannot be reused).
        sh_kill("CONT", pid);
        wait_until("unmarked image", || {
            std::fs::read(format!("/proc/{pid}/environ"))
                .is_ok_and(|b| !b.is_empty() && marker_in_environ(&b).is_none())
        });
        sh_kill("STOP", pid);
        wait_until("stopped again", || state_of(pid) == Some('T'));
        (a, fd, hold, t)
    }

    /// procfix6 (gate5 m1): a member the guard stopped and that dropped its marker is refused by the marker path (the
    /// pre-fix sweep then left it STOPPED) and killed by the identity held at its stop: same pidfd, exact start, our
    /// uid, behind a durable intent with proof "stop hold" and the marker read at the stop. DECOYS: a signal other than
    /// SIGKILL / SIGCONT is refused; the hold with ANOTHER process's pidfd is refused (that process lives); a hold of
    /// another boot is refused; after the exit the hold reports Gone and sends nothing.
    #[test]
    fn m1_stop_hold_kills_a_stopped_member_that_dropped_its_marker() {
        if !crate::launch::in_private_namespace() {
            eprintln!(
                "SKIPPED: spawns and signals processes: only inside a private pid namespace (rule L10)"
            );
            return;
        }
        let log = tmpdir("hold-kill");
        let (mut a, fd, hold, t) = stopped_without_marker("hk", &log);
        let pid = a.id();
        // 1. the marker path refuses it now
        let v = with_intent_log(&log, "t-hold", || {
            signal_with_pidfds(&[(&fd, t.clone())], libc::SIGKILL, None)
        })
        .pop()
        .unwrap();
        assert_eq!(v, Verdict::NotOurs(Refusal::NoMarker));
        // 2. decoys
        assert!(matches!(
            signal_held(&[(&fd, &hold)], libc::SIGTERM).pop().unwrap(),
            Verdict::Unverified(e) if e.contains("SIGKILL / SIGCONT only")
        ));
        let mut b = marked_sleep(&CampaignMarker::new("c1-hk", "j9:a1").unwrap().value());
        let fdb = open_pidfd(b.id()).unwrap();
        let v = with_intent_log(&log, "t-hold", || kill_held(&[(&fdb, &hold)]))
            .pop()
            .unwrap();
        assert!(
            matches!(&v, Verdict::Unverified(e) if e.contains("names pid")),
            "{v:?}"
        );
        let other_boot = StopHold {
            pid: hold.pid,
            start_ticks: hold.start_ticks,
            boot_id: "00000000-0000-0000-0000-000000000000".into(),
            marker: hold.marker.clone(),
            proof: hold.proof,
        };
        let v = with_intent_log(&log, "t-hold", || kill_held(&[(&fd, &other_boot)]))
            .pop()
            .unwrap();
        assert_eq!(v, Verdict::NotOurs(Refusal::OtherBoot));
        let wrong_start = StopHold {
            pid: hold.pid,
            start_ticks: hold.start_ticks + 1,
            boot_id: hold.boot_id.clone(),
            marker: None,
            proof: hold.proof,
        };
        let v = with_intent_log(&log, "t-hold", || kill_held(&[(&fd, &wrong_start)]))
            .pop()
            .unwrap();
        assert!(
            matches!(v, Verdict::NotOurs(Refusal::StartMismatch { .. })),
            "{v:?}"
        );
        std::thread::sleep(Duration::from_millis(50));
        assert!(b.try_wait().unwrap().is_none(), "the decoy lives");
        assert_eq!(
            state_of(pid),
            Some('T'),
            "nothing was sent to the member yet"
        );
        // 3. the held SIGKILL
        let v = with_intent_log(&log, "t-hold", || {
            with_context("unit test", Some("j1:a1"), || kill_held(&[(&fd, &hold)]))
        })
        .pop()
        .unwrap();
        assert_eq!(v, Verdict::Delivered);
        a.wait().unwrap();
        let (intents, _) = read_intent_log(&log.join(INTENT_LOG_FILE)).unwrap();
        let last = intents.last().unwrap();
        assert_eq!(last.line.pid, Some(pid));
        assert_eq!(last.line.signal, Some(libc::SIGKILL));
        assert_eq!(last.line.proof.as_deref(), Some("stop hold"));
        assert_eq!(last.line.marker, hold.marker);
        assert_eq!(last.result.as_deref(), Some("delivered"));
        // 4. reaped: Gone, nothing sent
        let n = intents.len();
        let v = with_intent_log(&log, "t-hold", || kill_held(&[(&fd, &hold)]))
            .pop()
            .unwrap();
        assert_eq!(v, Verdict::Gone);
        assert_eq!(
            read_intent_log(&log.join(INTENT_LOG_FILE)).unwrap().0.len(),
            n
        );
        let _ = b.kill();
        let _ = b.wait();
        let _ = std::fs::remove_dir_all(&log);
    }

    /// procfix6 (gate5 m1): the held SIGCONT undoes our stop (the process runs again, not stopped); without a durable
    /// intent neither the held SIGKILL nor the held SIGCONT is sent (the caller then reports it stranded).
    #[test]
    fn m1_stop_hold_continue_and_no_send_without_intent() {
        if !crate::launch::in_private_namespace() {
            eprintln!(
                "SKIPPED: spawns and signals processes: only inside a private pid namespace (rule L10)"
            );
            return;
        }
        let log = tmpdir("hold-cont");
        let (mut a, fd, hold, _) = stopped_without_marker("hc", &log);
        let pid = a.id();
        let full = tmpdir("hold-full");
        std::os::unix::fs::symlink("/dev/full", full.join(INTENT_LOG_FILE)).unwrap();
        for v in [
            with_intent_log(&full, "t-full", || kill_held(&[(&fd, &hold)])),
            with_intent_log(&full, "t-full", || continue_held(&[(&fd, &hold)])),
        ] {
            assert!(
                matches!(&v[0], Verdict::Unverified(e) if e.contains("not durable")),
                "{v:?}"
            );
        }
        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(state_of(pid), Some('T'), "nothing sent without an intent");
        let v = with_intent_log(&log, "t-hold", || continue_held(&[(&fd, &hold)]));
        assert_eq!(v, vec![Verdict::Delivered]);
        wait_until("continued", || state_of(pid) != Some('T'));
        let _ = a.kill();
        let _ = a.wait();
        let _ = std::fs::remove_dir_all(&log);
        let _ = std::fs::remove_dir_all(&full);
    }
}
