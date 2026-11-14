//! Campaign-directory lock with an owner record and stale-lock detection.
//!
//! Two layers:
//! 1. `CDIR/LOCK`: an exclusive `flock` held for the supervisor's lifetime. The descriptor is opened with
//!    `O_CLOEXEC` (Rust's default) and is NEVER made inheritable, so children of the supervisor do not hold it.
//!    This removes the weakness of `rustred-app/src/cli/shards` (which passes its lock fd to every child on
//!    purpose: after a supervisor crash the lock stays held for as long as any child or grandchild lives, and a
//!    restarted supervisor cannot even say who holds it; see interfaces/shards-extraction.md 4.9). Protection
//!    against duplicate work by orphans comes instead from the ledger's write-ahead `launching` record and the
//!    orphan scan by token ([`scan_token_holders`]): every child is started with the environment variable
//!    [`TOKEN_ENV`] = the lock token, so a successor can list (and the process layer can kill, after its
//!    identity checks) the previous supervisor's children.
//! 2. `CDIR/LOCK.json` (`rustred.campaign.lock.v1`): the owner record (pid, pid start time in clock ticks, boot
//!    id, host, command line, UTC start, token), written atomically after the flock is taken.
//!
//! Decision table of [`acquire`] (first match):
//! | flock | owner record | owner liveness | result |
//! |---|---|---|---|
//! | taken | none | - | `Fresh` |
//! | taken | any host name | alive (pid present, same start ticks, same boot id) | refuse `OwnerAliveWithoutLock` |
//! | taken | any host name | provably dead | `TookOver { previous, reason }` (caller appends a ledger record) |
//! | busy  | same host | alive | refuse `HeldByLiveOwner` |
//! | busy  | same host | dead  | refuse `HeldByForeignDescriptor` with the pids holding the file (never happens with this code; an fd leaked by other code) |
//! | busy  | other host / none | - | refuse `Busy` |
//!
//! "Provably dead" = pid absent, or pid present with a different start time, or a different boot id. Host names
//! are informational only (audit A m6); the CDIR must live on a local file system (flock).

use std::fs::{self, File, OpenOptions};
use std::io;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::atomic;
use crate::clock::Clock;

pub const LOCK_SCHEMA: &str = "rustred.campaign.lock.v1";
pub const LOCK_FILE: &str = "LOCK";
pub const OWNER_FILE: &str = "LOCK.json";
/// Environment variable carrying the lock token into every child (orphan identification after a crash).
pub const TOKEN_ENV: &str = "RUSTRED_CAMPAIGN_LOCK_TOKEN";

/// Identity of one process, enough to decide "same process" across pid reuse and reboots.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessIdentity {
    pub pid: u32,
    /// Field 22 of /proc/<pid>/stat (clock ticks after boot).
    pub start_ticks: u64,
    /// /proc/sys/kernel/random/boot_id.
    pub boot_id: String,
    pub host: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnerRecord {
    pub schema: String,
    #[serde(flatten)]
    pub identity: ProcessIdentity,
    pub argv: Vec<String>,
    pub started_utc: String,
    /// Random-ish token (blake3 of identity + time), exported to children as [`TOKEN_ENV`].
    pub token: String,
}

/// Why an owner is judged dead.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeadReason {
    PidAbsent,
    StartTimeDiffers { now: u64 },
    BootIdDiffers { now: String },
    /// The record names this very process (left by an earlier lock of the same process).
    OwnRecord,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Liveness {
    Alive,
    Dead(DeadReason),
    /// Recorded on another host: cannot be judged from here.
    OtherHost,
}

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum LockOutcome {
    Fresh,
    TookOver {
        previous: OwnerRecord,
        reason: DeadReason,
        orphans: Vec<u32>,
    },
}

#[derive(Debug)]
pub enum LockError {
    HeldByLiveOwner {
        owner: OwnerRecord,
    },
    OwnerAliveWithoutLock {
        owner: OwnerRecord,
    },
    HeldByForeignDescriptor {
        owner: OwnerRecord,
        holders: Vec<u32>,
    },
    OtherHost {
        owner: OwnerRecord,
    },
    Busy {
        owner: Option<OwnerRecord>,
    },
    Symlink {
        path: PathBuf,
    },
    BadOwnerRecord {
        path: PathBuf,
        detail: String,
    },
    Io {
        path: PathBuf,
        source: io::Error,
    },
    Atomic(atomic::AtomicError),
}

impl std::fmt::Display for LockError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let who = |o: &OwnerRecord| {
            format!(
                "pid {} on {} (started {}, command {:?})",
                o.identity.pid, o.identity.host, o.started_utc, o.argv
            )
        };
        match self {
            LockError::HeldByLiveOwner { owner } => {
                write!(f, "campaign is run by a live supervisor: {}", who(owner))
            }
            LockError::OwnerAliveWithoutLock { owner } => {
                write!(
                    f,
                    "recorded owner is alive but does not hold the lock: {}; refusing",
                    who(owner)
                )
            }
            LockError::HeldByForeignDescriptor { owner, holders } => write!(
                f,
                "recorded owner {} is dead but the lock file is held by pids {holders:?} (leaked descriptor)",
                who(owner)
            ),
            LockError::OtherHost { owner } => {
                write!(f, "campaign lock was taken on another host: {}", who(owner))
            }
            LockError::Busy { owner } => write!(f, "campaign lock busy (owner record: {owner:?})"),
            LockError::Symlink { path } => write!(f, "{} is a symlink; refusing", path.display()),
            LockError::BadOwnerRecord { path, detail } => {
                write!(f, "unreadable owner record {}: {detail}", path.display())
            }
            LockError::Io { path, source } => write!(f, "lock I/O on {}: {source}", path.display()),
            LockError::Atomic(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for LockError {}

/// A held campaign lock. Dropping it releases the flock and removes the owner record.
#[derive(Debug)]
pub struct CampaignLock {
    file: Option<File>,
    dir: PathBuf,
    pub owner: OwnerRecord,
}

impl CampaignLock {
    pub fn token(&self) -> &str {
        &self.owner.token
    }
    /// The campaign directory this lock guards (a ledger write handle requires the lock of its own CDIR).
    pub fn dir(&self) -> &Path {
        &self.dir
    }
    /// Releases explicitly (also done on drop): removes LOCK.json first, then unlocks.
    pub fn release(mut self) -> io::Result<()> {
        self.release_inner()
    }
    fn release_inner(&mut self) -> io::Result<()> {
        if let Some(file) = self.file.take() {
            let r = fs::remove_file(self.dir.join(OWNER_FILE));
            drop(file);
            r.or_else(|e| {
                if e.kind() == io::ErrorKind::NotFound {
                    Ok(())
                } else {
                    Err(e)
                }
            })
        } else {
            Ok(())
        }
    }
}

impl Drop for CampaignLock {
    fn drop(&mut self) {
        let _ = self.release_inner();
    }
}

/// Acquires the lock of `dir` (created if missing). See the module table.
#[allow(clippy::result_large_err)]
pub fn acquire(
    dir: &Path,
    clock: &dyn Clock,
    argv: &[String],
) -> Result<(CampaignLock, LockOutcome), LockError> {
    fs::create_dir_all(dir).map_err(|e| LockError::Io {
        path: dir.to_path_buf(),
        source: e,
    })?;
    let lock_path = dir.join(LOCK_FILE);
    if fs::symlink_metadata(&lock_path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(LockError::Symlink { path: lock_path });
    }
    // std opens with O_CLOEXEC: the descriptor is not inherited across exec.
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|e| LockError::Io {
            path: lock_path.clone(),
            source: e,
        })?;
    let previous = read_owner(dir)?;
    let me = current_identity().map_err(|e| LockError::Io {
        path: PathBuf::from("/proc/self"),
        source: e,
    })?;
    let got = try_flock(&file).map_err(|e| LockError::Io {
        path: lock_path.clone(),
        source: e,
    })?;
    if !got {
        // the winner of a race may have written its record after we read ours (audit B exp5)
        let previous = read_owner(dir)?.or(previous);
        return Err(match previous {
            Some(owner) if owner.identity.boot_id == me.boot_id => match liveness(&owner.identity) {
                Liveness::Alive => LockError::HeldByLiveOwner { owner },
                _ => {
                    let holders = holders_of(&lock_path);
                    LockError::HeldByForeignDescriptor { owner, holders }
                }
            },
            other => LockError::Busy { owner: other },
        });
    }
    let outcome = match previous {
        None => LockOutcome::Fresh,
        Some(owner) => {
            // Host names are informational (audit A m6): liveness is judged by boot id + pid + start ticks, so a
            // renamed host does not refuse forever (the CDIR must be on a local file system: flock semantics).
            match liveness(&owner.identity) {
                Liveness::Alive if owner.identity.pid != me.pid => {
                    return Err(LockError::OwnerAliveWithoutLock { owner });
                }
                // Our own pid with a matching identity: a record left by an earlier lock of this very process
                // (e.g. a test that leaked one); treat as a takeover from ourselves.
                Liveness::Alive => LockOutcome::TookOver {
                    orphans: Vec::new(),
                    previous: owner,
                    reason: DeadReason::OwnRecord,
                },
                Liveness::OtherHost => return Err(LockError::OtherHost { owner }),
                Liveness::Dead(reason) => {
                    let orphans = scan_token_holders(&owner.token);
                    LockOutcome::TookOver {
                        previous: owner,
                        reason,
                        orphans,
                    }
                }
            }
        }
    };
    let now = clock.now_utc();
    let token =
        blake3::hash(format!("{}:{}:{}:{}", me.boot_id, me.pid, me.start_ticks, now.0).as_bytes())
            .to_hex()[..32]
            .to_string();
    let owner = OwnerRecord {
        schema: LOCK_SCHEMA.to_string(),
        identity: me,
        argv: argv.to_vec(),
        started_utc: now.to_iso(),
        token,
    };
    atomic::write_json_atomic(&dir.join(OWNER_FILE), &owner).map_err(LockError::Atomic)?;
    Ok((
        CampaignLock {
            file: Some(file),
            dir: dir.to_path_buf(),
            owner,
        },
        outcome,
    ))
}

/// Reads LOCK.json if present.
#[allow(clippy::result_large_err)]
pub fn read_owner(dir: &Path) -> Result<Option<OwnerRecord>, LockError> {
    let path = dir.join(OWNER_FILE);
    match atomic::read_capped(&path, 1 << 20) {
        Ok(bytes) => serde_json::from_slice::<OwnerRecord>(&bytes)
            .map(Some)
            .map_err(|e| LockError::BadOwnerRecord {
                path,
                detail: e.to_string(),
            }),
        Err(atomic::AtomicError::Io { source, .. }) if source.kind() == io::ErrorKind::NotFound => {
            Ok(None)
        }
        Err(e) => Err(LockError::Atomic(e)),
    }
}

fn try_flock(file: &File) -> io::Result<bool> {
    // SAFETY: plain syscall on an owned, open descriptor.
    let r = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if r == 0 {
        return Ok(true);
    }
    let e = io::Error::last_os_error();
    if e.raw_os_error() == Some(libc::EWOULDBLOCK) {
        Ok(false)
    } else {
        Err(e)
    }
}

/// Liveness of a recorded identity, judged on this machine by boot id, pid and start ticks (the host name is
/// informational: a renamed host is the same machine when the boot id is the same, and a different boot id means
/// the process cannot hold anything on this local file system). `OtherHost` is no longer produced.
pub fn liveness(id: &ProcessIdentity) -> Liveness {
    let boot = boot_id().unwrap_or_default();
    if id.boot_id != boot {
        return Liveness::Dead(DeadReason::BootIdDiffers { now: boot });
    }
    match start_ticks(id.pid) {
        None => Liveness::Dead(DeadReason::PidAbsent),
        Some(t) if t != id.start_ticks => Liveness::Dead(DeadReason::StartTimeDiffers { now: t }),
        Some(_) => Liveness::Alive,
    }
}

/// Identity of the calling process.
pub fn current_identity() -> io::Result<ProcessIdentity> {
    let pid = std::process::id();
    Ok(ProcessIdentity {
        pid,
        start_ticks: start_ticks(pid)
            .ok_or_else(|| io::Error::other("cannot read /proc/self/stat"))?,
        boot_id: boot_id()?,
        host: hostname()?,
    })
}

/// Identity of another process on this host, if it exists.
pub fn identity_of(pid: u32) -> Option<ProcessIdentity> {
    Some(ProcessIdentity {
        pid,
        start_ticks: start_ticks(pid)?,
        boot_id: boot_id().ok()?,
        host: hostname().ok()?,
    })
}

/// Field 22 (starttime) of /proc/<pid>/stat; None if the process does not exist (zombies count as existing).
pub fn start_ticks(pid: u32) -> Option<u64> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // comm may contain spaces and parentheses: fields restart after the LAST ')'.
    let rest = &stat[stat.rfind(')')? + 1..];
    // rest starts with " state ppid ...": starttime is field 22 overall, i.e. index 19 of rest's fields.
    rest.split_whitespace().nth(19)?.parse().ok()
}

pub fn boot_id() -> io::Result<String> {
    Ok(fs::read_to_string("/proc/sys/kernel/random/boot_id")?
        .trim()
        .to_string())
}

pub fn hostname() -> io::Result<String> {
    let mut buf = [0u8; 256];
    // SAFETY: buffer and length are valid; gethostname NUL-terminates when it fits.
    let r = unsafe { libc::gethostname(buf.as_mut_ptr() as *mut libc::c_char, buf.len()) };
    if r != 0 {
        return Err(io::Error::last_os_error());
    }
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    Ok(String::from_utf8_lossy(&buf[..end]).into_owned())
}

fn proc_pids() -> Vec<u32> {
    let Ok(rd) = fs::read_dir("/proc") else {
        return Vec::new();
    };
    rd.filter_map(|e| e.ok()?.file_name().to_str()?.parse().ok())
        .collect()
}

/// Pids (of processes readable by us) whose environment contains `TOKEN_ENV=<token>`.
pub fn scan_token_holders(token: &str) -> Vec<u32> {
    if token.is_empty() {
        return Vec::new();
    }
    let needle = format!("{TOKEN_ENV}={token}");
    let me = std::process::id();
    let mut out: Vec<u32> = proc_pids()
        .into_iter()
        .filter(|&p| p != me)
        .filter(|p| {
            fs::read(format!("/proc/{p}/environ"))
                .map(|env| env.split(|&b| b == 0).any(|kv| kv == needle.as_bytes()))
                .unwrap_or(false)
        })
        .collect();
    out.sort_unstable();
    out
}

/// Pids having `path` open (scan of /proc/*/fd, processes readable by us).
pub fn holders_of(path: &Path) -> Vec<u32> {
    let Ok(target) = fs::canonicalize(path) else {
        return Vec::new();
    };
    let mut out: Vec<u32> = proc_pids()
        .into_iter()
        .filter(|p| {
            fs::read_dir(format!("/proc/{p}/fd"))
                .map(|rd| {
                    rd.filter_map(|e| e.ok())
                        .any(|e| fs::read_link(e.path()).is_ok_and(|l| l == target))
                })
                .unwrap_or(false)
        })
        .collect();
    out.sort_unstable();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn own_identity_is_alive_and_fake_pid_is_dead() {
        let me = current_identity().unwrap();
        assert_eq!(liveness(&me), Liveness::Alive);
        let mut other = me.clone();
        other.start_ticks += 1;
        assert!(matches!(
            liveness(&other),
            Liveness::Dead(DeadReason::StartTimeDiffers { .. })
        ));
        let mut reboot = me.clone();
        reboot.boot_id = "00000000-0000-0000-0000-000000000000".into();
        assert!(matches!(
            liveness(&reboot),
            Liveness::Dead(DeadReason::BootIdDiffers { .. })
        ));
        let mut gone = me.clone();
        gone.pid = u32::MAX - 7;
        assert_eq!(liveness(&gone), Liveness::Dead(DeadReason::PidAbsent));
        let mut far = me;
        far.host = "elsewhere.invalid".into();
        // a renamed host with the same boot id is this machine (audit A m6)
        assert_eq!(liveness(&far), Liveness::Alive);
    }
}
