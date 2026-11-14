//! Optional cgroup-v2 memory caps in a DELEGATED subtree: detected, never required.
//!
//! A usable root is a cgroup-v2 directory that this user may write, whose `cgroup.subtree_control` already enables the
//! `memory` controller, so that a child directory gets `memory.max`. The usual way to get one is to start the supervisor
//! as `systemd-run --user --scope -p Delegate=yes -- rustred campaign ...` and let the supervisor move itself into a leaf
//! (cgroup v2 forbids processes in a cgroup whose children have controllers): [`CgroupRoot::prepare_in_own_scope`] does
//! that when, and only when, the supervisor is ALONE in its own cgroup and owns it. In a zellij shell the supervisor
//! shares its cgroup with the shell, so this refuses and the launcher enforces caps by sampling (R6: no dependency on
//! systemd units). Per job: `memory.max` = cap, `memory.swap.max` = 0, `memory.oom.group` = 1; `cgroup.kill` also
//! reaches escapees that left the session by setsid, which signals cannot.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::fd::RawFd;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CgroupRoot {
    path: PathBuf,
}

static SEQ: AtomicU64 = AtomicU64::new(0);

fn read(p: &Path) -> io::Result<String> {
    fs::read_to_string(p)
}

fn write_file(p: &Path, s: &str) -> io::Result<()> {
    OpenOptions::new()
        .write(true)
        .open(p)?
        .write_all(s.as_bytes())
}

impl CgroupRoot {
    /// Check, without changing anything except a probe directory created and removed again, that `path` can hold job
    /// cgroups with memory caps. Err = human-readable reason (the launcher then uses sampling).
    pub fn probe(path: &Path) -> Result<Self, String> {
        if !path.starts_with("/sys/fs/cgroup") {
            return Err(format!("{} is not under /sys/fs/cgroup", path.display()));
        }
        let ctl = read(&path.join("cgroup.subtree_control"))
            .map_err(|e| format!("{}: cgroup.subtree_control unreadable: {e}", path.display()))?;
        if !ctl.split_whitespace().any(|c| c == "memory") {
            return Err(format!(
                "{}: memory controller not enabled for children (subtree_control: {:?})",
                path.display(),
                ctl.trim()
            ));
        }
        let probe = path.join(format!(
            "rr-probe-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&probe)
            .map_err(|e| format!("{}: cannot create a child cgroup: {e}", path.display()))?;
        let ok = probe.join("memory.max").exists()
            && OpenOptions::new()
                .write(true)
                .open(probe.join("cgroup.procs"))
                .is_ok();
        let _ = fs::remove_dir(&probe);
        if !ok {
            return Err(format!(
                "{}: child cgroup lacks a writable memory.max / cgroup.procs",
                path.display()
            ));
        }
        Ok(Self {
            path: path.to_path_buf(),
        })
    }

    /// If this process owns its cgroup and is the only process in it (a `systemd-run --scope -p Delegate=yes`
    /// launch), move it into `<own>/supervisor`, enable `+memory` for children, and return `<own>` as the root.
    /// Refuses (Err with the reason) in every other case; it never moves a process other than this one.
    pub fn prepare_in_own_scope() -> Result<Self, String> {
        let own = crate::resources::own_cgroup_dir().ok_or("no cgroup-v2 membership")?;
        // SAFETY: getuid has no arguments and cannot fail.
        let uid = unsafe { libc::getuid() };
        let meta = fs::metadata(&own).map_err(|e| format!("{}: {e}", own.display()))?;
        if meta.uid() != uid {
            return Err(format!(
                "{} is not owned by this user (not delegated)",
                own.display()
            ));
        }
        let procs =
            read(&own.join("cgroup.procs")).map_err(|e| format!("{}: {e}", own.display()))?;
        let me = std::process::id().to_string();
        if procs.split_whitespace().any(|p| p != me) {
            return Err(format!(
                "{} holds other processes besides the supervisor (e.g. the shell): sampling instead",
                own.display()
            ));
        }
        let leaf = own.join("supervisor");
        if !leaf.exists() {
            fs::create_dir(&leaf).map_err(|e| format!("{}: {e}", leaf.display()))?;
        }
        write_file(&leaf.join("cgroup.procs"), "0")
            .map_err(|e| format!("move into {}: {e}", leaf.display()))?;
        write_file(&own.join("cgroup.subtree_control"), "+memory")
            .map_err(|e| format!("enable memory in {}: {e}", own.display()))?;
        Self::probe(&own)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Create `<root>/job-<label>-<pid>-<n>` with the cap; returns it with `cgroup.procs` open for the child.
    pub(super) fn create_job(&self, label: &str, cap: u64) -> Result<JobCgroup, String> {
        let safe: String = label
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .take(64)
            .collect();
        let dir = self.path.join(format!(
            "job-{safe}-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
        let setup = || -> io::Result<File> {
            write_file(&dir.join("memory.max"), &cap.to_string())?;
            let _ = write_file(&dir.join("memory.swap.max"), "0"); // absent without swap accounting
            let _ = write_file(&dir.join("memory.oom.group"), "1");
            OpenOptions::new()
                .write(true)
                .open(dir.join("cgroup.procs"))
        };
        match setup() {
            Ok(procs) => Ok(JobCgroup {
                dir,
                procs,
                memory: true,
            }),
            Err(e) => {
                let _ = fs::remove_dir(&dir);
                Err(format!("set up {}: {e}", dir.display()))
            }
        }
    }
}

/// One job's cgroup. `remove` after the job's session is empty.
#[derive(Debug)]
pub struct JobCgroup {
    dir: PathBuf,
    procs: File,
    /// The leaf has a memory cap (`create_job`); false for a tracking-only leaf.
    memory: bool,
}

impl JobCgroup {
    pub fn path(&self) -> &Path {
        &self.dir
    }

    /// Does the leaf carry the job's memory cap (false: tracking only)?
    pub fn has_memory_cap(&self) -> bool {
        self.memory
    }

    pub(super) fn procs_fd(&self) -> RawFd {
        super::real::raw(&self.procs)
    }

    /// `oom_kill` count of memory.events (0 if unreadable).
    pub fn oom_kills(&self) -> u64 {
        read(&self.dir.join("memory.events"))
            .ok()
            .and_then(|t| {
                t.lines()
                    .find_map(|l| l.strip_prefix("oom_kill "))
                    .and_then(|v| v.trim().parse().ok())
            })
            .unwrap_or(0)
    }

    /// `cgroup.kill` (Linux >= 5.14): SIGKILL to every process of the cgroup, escapees included.
    pub fn kill_all(&self) {
        let _ = kill_path(&self.dir);
    }

    /// Pids in `cgroup.procs` (strict: a read error is an error).
    pub fn procs(&self) -> io::Result<Vec<u32>> {
        procs_of(&self.dir)
    }

    /// `populated 0` in cgroup.events.
    pub fn is_empty(&self) -> Option<bool> {
        read(&self.dir.join("cgroup.events"))
            .ok()?
            .lines()
            .find_map(|l| l.strip_prefix("populated "))
            .map(|v| v.trim() == "0")
    }

    pub(super) fn remove(self) {
        let _ = fs::remove_dir(&self.dir);
    }
}

/// Is `dir` delegated to us by systemd (`user.delegate` = "1" extended attribute, set on `Delegate=yes` units since
/// systemd 251)?
fn delegated(dir: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt;
    let Ok(c) = std::ffi::CString::new(dir.as_os_str().as_bytes()) else {
        return false;
    };
    for name in [c"user.delegate", c"trusted.delegate"] {
        let mut buf = [0u8; 8];
        // SAFETY: valid C strings and a buffer of the given length.
        let n = unsafe {
            libc::getxattr(
                c.as_ptr(),
                name.as_ptr(),
                buf.as_mut_ptr().cast(),
                buf.len(),
            )
        };
        if n > 0 && buf[..n as usize].starts_with(b"1") {
            return true;
        }
    }
    false
}

impl CgroupRoot {
    /// A TRACKING root for per-job leaf cgroups (gate re-audit B RB-C1 c): this process's own cgroup (or its parent,
    /// when the supervisor moved itself into a `supervisor` leaf) if it is DELEGATED to us (systemd `Delegate=yes`), owned
    /// by our euid, and a probe child accepts `cgroup.procs`, `cgroup.kill` and `cgroup.freeze`. No controller is needed
    /// (the leaf proves membership and kills race-free; memory caps stay with the chosen cap mechanism). Refuses in every
    /// other case, e.g. in a zellij shell (root-owned session scope).
    pub fn probe_tracking() -> Result<Self, String> {
        let own = crate::resources::own_cgroup_dir().ok_or("no cgroup-v2 membership")?;
        let mut candidates = vec![own.clone()];
        if own.file_name().is_some_and(|n| n == "supervisor")
            && let Some(p) = own.parent()
        {
            candidates.push(p.to_path_buf());
        }
        // SAFETY: geteuid has no arguments and cannot fail.
        let euid = unsafe { libc::geteuid() };
        let mut why = Vec::new();
        for dir in candidates {
            if !delegated(&dir) {
                why.push(format!(
                    "{}: not delegated (no user.delegate xattr)",
                    dir.display()
                ));
                continue;
            }
            match fs::metadata(&dir) {
                Ok(m) if m.uid() == euid => {}
                Ok(_) => {
                    why.push(format!("{}: not owned by us", dir.display()));
                    continue;
                }
                Err(e) => {
                    why.push(format!("{}: {e}", dir.display()));
                    continue;
                }
            }
            let probe = dir.join(format!(
                "rr-track-probe-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, Ordering::Relaxed)
            ));
            if let Err(e) = fs::create_dir(&probe) {
                why.push(format!(
                    "{}: cannot create a child cgroup: {e}",
                    dir.display()
                ));
                continue;
            }
            let ok = [
                "cgroup.procs",
                "cgroup.kill",
                "cgroup.freeze",
                "cgroup.events",
            ]
            .iter()
            .all(|f| probe.join(f).exists())
                && OpenOptions::new()
                    .write(true)
                    .open(probe.join("cgroup.procs"))
                    .is_ok();
            let _ = fs::remove_dir(&probe);
            if ok {
                return Ok(Self { path: dir });
            }
            why.push(format!(
                "{}: child cgroup lacks cgroup.procs / kill / freeze",
                dir.display()
            ));
        }
        Err(why.join("; "))
    }

    /// Create a tracking-only leaf `<root>/job-<label>-<pid>-<n>` (no memory settings).
    pub(super) fn create_tracking_leaf(&self, label: &str) -> Result<JobCgroup, String> {
        let safe: String = label
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .take(64)
            .collect();
        let dir = self.path.join(format!(
            "job-{safe}-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
        match OpenOptions::new()
            .write(true)
            .open(dir.join("cgroup.procs"))
        {
            Ok(procs) => Ok(JobCgroup {
                dir,
                procs,
                memory: false,
            }),
            Err(e) => {
                let _ = fs::remove_dir(&dir);
                Err(format!("open {}/cgroup.procs: {e}", dir.display()))
            }
        }
    }
}

/// Pids listed in `<dir>/cgroup.procs` (strict).
pub(super) fn procs_of(dir: &Path) -> io::Result<Vec<u32>> {
    let t = read(&dir.join("cgroup.procs"))?;
    t.split_whitespace()
        .map(|w| {
            w.parse::<u32>()
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "cgroup.procs"))
        })
        .collect()
}

/// `cgroup.kill` of a job cgroup WE created (the only caller passes `JobCgroup` paths). A successful write is a
/// delivered SIGKILL to every process in it: logged like every guarded delivery (gate3 M2).
pub(super) fn kill_path(dir: &Path) -> io::Result<()> {
    // procfix5 M1: durable intent first, then the write (the guard logs the completion and the delivery).
    super::guard::cgroup_kill_logged(dir, "job cgroup", || {
        write_file(&dir.join("cgroup.kill"), "1")
    })
}

/// Freeze (`cgroup.freeze` = 1), wait up to `wait` for `frozen 1`, then `cgroup.kill`. Errors: a write failed.
pub(super) fn freeze_and_kill_path(dir: &Path, wait: std::time::Duration) -> io::Result<()> {
    write_file(&dir.join("cgroup.freeze"), "1")?;
    let t0 = std::time::Instant::now();
    while t0.elapsed() < wait {
        let frozen = read(&dir.join("cgroup.events"))
            .ok()
            .and_then(|t| {
                t.lines()
                    .find_map(|l| l.strip_prefix("frozen "))
                    .map(|v| v.trim() == "1")
            })
            .unwrap_or(false);
        if frozen {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    kill_path(dir)
}

// ------------------------------------------------------------------------------------------------ systemd job scopes

/// The cgroup of a job's transient `systemd-run --user --scope` (procfix3, an ADDITIONAL belt; never a replacement of the
/// identity checks): kernel-maintained membership that also holds descendants which cleared the marker and left the
/// session. Recorded once, while the job's leader is held UNREAPED (its pid cannot be reused), from the leader's own
/// `/proc/<sid>/cgroup`, and only when every check of [`ScopeRef::from_leader_cgroup`] holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ScopeRef {
    pub dir: PathBuf,
    pub dev: u64,
    pub ino: u64,
    pub sid: u32,
}

/// Where systemd puts a user's transient scopes: `/sys/fs/cgroup/user.slice/user-<uid>.slice/user@<uid>.service/`.
fn user_manager_root(uid: u32) -> PathBuf {
    PathBuf::from(format!(
        "/sys/fs/cgroup/user.slice/user-{uid}.slice/user@{uid}.service"
    ))
}

impl ScopeRef {
    /// From the text of the pinned leader's `/proc/<sid>/cgroup`. Every condition must hold, else `None` (the belt is
    /// then simply not used): a cgroup-v2 line `0::<path>`; the path lies under OUR user manager
    /// (`user.slice/user-<uid>.slice/user@<uid>.service/`), has no `..`, and its last component is systemd-run's
    /// transient scope name for THIS leader, `run-p<sid>-i<digits>.scope` (systemd-run registers its own pid, which is
    /// the leader's: it execs the program in place); the directory exists, is owned by our uid; its (dev, inode) is
    /// recorded so that a later use refuses a different directory under the same name.
    pub(crate) fn from_leader_cgroup(
        text: &str,
        sid: u32,
        uid: u32,
        unit: Option<&str>,
    ) -> Option<ScopeRef> {
        let rel = text.lines().find_map(|l| l.strip_prefix("0::"))?.trim();
        let rel = rel.strip_prefix('/')?;
        if rel
            .split('/')
            .any(|c| c.is_empty() || c == "." || c == "..")
        {
            return None;
        }
        let dir = Path::new("/sys/fs/cgroup").join(rel);
        if !dir.starts_with(user_manager_root(uid)) {
            return None;
        }
        let name = dir.file_name()?.to_str()?;
        match unit {
            // gate3 M5: the job's deterministic unit name (campaign id + label), exactly
            Some(u) => {
                if name != format!("{u}.scope") {
                    return None;
                }
            }
            None => {
                let digits = name
                    .strip_prefix(&format!("run-p{sid}-i"))?
                    .strip_suffix(".scope")?;
                if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
                    return None;
                }
            }
        }
        Self::check_dir(&dir, uid).map(|(dev, ino)| ScopeRef { dir, dev, ino, sid })
    }

    fn check_dir(dir: &Path, uid: u32) -> Option<(u64, u64)> {
        let md = fs::symlink_metadata(dir).ok()?;
        (md.file_type().is_dir() && md.uid() == uid).then(|| (md.dev(), md.ino()))
    }

    /// The same directory as recorded (dev, inode, owner)? `Ok(false)`: gone or replaced (no process of ours in it).
    fn still_same(&self, uid: u32) -> bool {
        Self::check_dir(&self.dir, uid) == Some((self.dev, self.ino))
    }

    /// Live processes in the scope other than the job's own (pinned) leader. A scope directory that no longer exists,
    /// or is not the recorded one, holds nothing of ours: empty. A failed read of an existing scope: error.
    pub(crate) fn live_members(&self) -> Result<Vec<u32>, String> {
        let uid = crate::resources::our_uid();
        if !self.still_same(uid) {
            return Ok(Vec::new());
        }
        let pids = match procs_of(&self.dir) {
            Ok(p) => p,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(format!("{}/cgroup.procs: {e}", self.dir.display())),
        };
        let mut out = Vec::new();
        for pid in pids {
            if pid == self.sid {
                continue;
            }
            match crate::resources::read_stat(pid) {
                Ok(Some(st)) if !st.is_dead() => out.push(pid),
                Ok(_) => {}
                Err(e) => return Err(format!("stat of scope member {pid}: {e}")),
            }
        }
        Ok(out)
    }

    /// `cgroup.kill` of the job's scope: kills every process in it. Only after the guarded hunt (never instead of it),
    /// and only while the directory is still the recorded one (dev, inode, owner) under our user manager.
    pub(crate) fn kill_members(&self) -> io::Result<()> {
        let uid = crate::resources::our_uid();
        if !self.dir.starts_with(user_manager_root(uid)) || !self.still_same(uid) {
            return Err(io::Error::other("scope directory is not the recorded one"));
        }
        kill_path(&self.dir)
    }
}

// ------------------------------------------------------------------------------------------------ named job scopes (gate3 M5)

/// The deterministic systemd unit name of a job's transient scope (lane int5, gate3 M5): `rr-<campaign id>-<label>`
/// with every character outside `[A-Za-z0-9_.-]` replaced by `_`, shortened, and ALWAYS suffixed by 16 hex of
/// BLAKE3(campaign id | label) so that two different (campaign, label) pairs never share a name. The campaign id binds
/// the campaign directory (nonce, device, inode): another campaign, or a copy of this directory, never produces it.
/// `systemd-run --unit=NAME --scope` creates `NAME.scope`.
pub fn scope_unit_name(campaign_id: &str, label: &str) -> String {
    let clean = |s: &str| -> String {
        s.chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-') {
                    c
                } else {
                    '_'
                }
            })
            .collect()
    };
    let h = blake3::hash(format!("{campaign_id}|{label}").as_bytes()).to_hex();
    let mut stem = format!("rr-{}-{}", clean(campaign_id), clean(label));
    stem.truncate(160);
    format!("{stem}-{}", &h[..16])
}

/// The cgroup directories where systemd may have put the user scope `<unit>.scope` (exact names only, never a
/// pattern): under our user manager, in `app.slice` (systemd >= 247) or at its root.
fn scope_candidates(unit: &str, uid: u32) -> Vec<PathBuf> {
    let root = user_manager_root(uid);
    vec![
        root.join("app.slice").join(format!("{unit}.scope")),
        root.join(format!("{unit}.scope")),
    ]
}

/// RESTART BACKSTOP (gate3 M5): kill every process left in the job scope `<unit>.scope` whose NAME and leader pid the
/// caller found recorded in THIS campaign's ledger (the supervisor checks both: the launch record names the unit, the
/// `started` record the leader of this boot). Exact directory names only (never a pattern), the directory must be ours
/// (owner uid) and lie under our user manager. `Ok(None)`: no such scope (gone, or systemd never made it); `Ok(Some(n))`:
/// `cgroup.kill` written for a scope with `n` live processes (logged as a delivery, proof "job scope"); `Ok(Some(0))`:
/// the scope exists but is empty (nothing signalled).
pub fn kill_recorded_scope(unit: &str) -> io::Result<Option<usize>> {
    let uid = crate::resources::our_uid();
    for dir in scope_candidates(unit, uid) {
        let Ok(md) = fs::symlink_metadata(&dir) else {
            continue;
        };
        if !md.file_type().is_dir() || md.uid() != uid {
            continue;
        }
        let procs = procs_of(&dir)?;
        if procs.is_empty() {
            return Ok(Some(0));
        }
        super::guard::cgroup_kill_logged(&dir, "job scope", || {
            write_file(&dir.join("cgroup.kill"), "1")
        })?;
        return Ok(Some(procs.len()));
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// procfix3: the scope belt accepts only the leader's own transient scope under our user manager.
    #[test]
    fn scope_ref_accepts_only_this_leaders_transient_scope() {
        let uid = 4242; // no such directory exists for this uid: every valid name still fails the directory check
        let ok = "0::/user.slice/user-4242.slice/user@4242.service/app.slice/run-p77-i123.scope\n";
        assert_eq!(ScopeRef::from_leader_cgroup(ok, 77, uid, None), None);
        for bad in [
            "0::/user.slice/user-4242.slice/user@4242.service/app.slice/run-p78-i123.scope",
            "0::/user.slice/user-4242.slice/user@4242.service/app.slice/run-p77-i.scope",
            "0::/user.slice/user-4242.slice/user@4242.service/app.slice/run-p77-i12x.scope",
            "0::/user.slice/user-1.slice/user@1.service/app.slice/run-p77-i123.scope",
            "0::/system.slice/run-p77-i123.scope",
            "0::/user.slice/user-4242.slice/user@4242.service/../run-p77-i123.scope",
            "1:name=systemd:/run-p77-i123.scope",
        ] {
            assert_eq!(
                ScopeRef::from_leader_cgroup(bad, 77, uid, None),
                None,
                "{bad}"
            );
        }
        // The name rule itself (directory check aside).
        let dir = Path::new(
            "/sys/fs/cgroup/user.slice/user-4242.slice/user@4242.service/app.slice/run-p77-i9.scope",
        );
        assert!(dir.starts_with(user_manager_root(uid)));
    }

    /// gate3 M5: deterministic, unique per (campaign, label), a valid unit name, and the scope belt accepts exactly it.
    #[test]
    fn scope_unit_names_are_deterministic_unique_and_exact() {
        let a = scope_unit_name("camp.0123456789abcdef", "N0.i0001.L0-t0:a1");
        assert_eq!(
            a,
            scope_unit_name("camp.0123456789abcdef", "N0.i0001.L0-t0:a1")
        );
        assert!(
            a.starts_with("rr-camp.0123456789abcdef-N0.i0001.L0-t0_a1-"),
            "{a}"
        );
        assert!(
            a.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
        );
        // labels that sanitise alike still differ (the hash), another campaign id differs
        assert_ne!(
            a,
            scope_unit_name("camp.0123456789abcdef", "N0.i0001.L0-t0_a1")
        );
        assert_ne!(
            a,
            scope_unit_name("camp.fedcba9876543210", "N0.i0001.L0-t0:a1")
        );
        assert!(scope_unit_name("c", &"x".repeat(1000)).len() < 200);
        // the belt: only exactly <unit>.scope under our user manager (the directory check aside)
        let uid = 4242;
        let ok = format!("0::/user.slice/user-4242.slice/user@4242.service/app.slice/{a}.scope\n");
        assert_eq!(
            ScopeRef::from_leader_cgroup(&ok, 77, uid, Some(&a)),
            None,
            "no such directory"
        );
        for bad in [
            format!("0::/user.slice/user-4242.slice/user@4242.service/app.slice/{a}x.scope"),
            "0::/user.slice/user-4242.slice/user@4242.service/app.slice/run-p77-i1.scope"
                .to_string(),
            format!("0::/user.slice/user-1.slice/user@1.service/app.slice/{a}.scope"),
        ] {
            assert_eq!(
                ScopeRef::from_leader_cgroup(&bad, 77, uid, Some(&a)),
                None,
                "{bad}"
            );
        }
        // nothing recorded under a unit name that does not exist: no kill
        assert!(matches!(
            kill_recorded_scope(&scope_unit_name(
                "no-such-campaign.0000000000000000",
                "x:a1"
            )),
            Ok(None)
        ));
    }

    #[test]
    fn probe_refuses_non_cgroup_paths() {
        assert!(CgroupRoot::probe(Path::new("/tmp")).is_err());
        assert!(CgroupRoot::probe(Path::new("/sys/fs/cgroup/does-not-exist-rr")).is_err());
        // prepare_in_own_scope is deliberately not exercised here: it may move the calling process (tests must not
        // change the host's cgroup layout).
    }
}
