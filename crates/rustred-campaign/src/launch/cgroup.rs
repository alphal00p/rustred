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
        // procfix7 round 5: only a PROVEN present memory.max counts (an unknown answer = not usable: sampling)
        let ok = crate::fsx::presence("cgroup", &probe.join("memory.max")).is_present()
            && OpenOptions::new()
                .write(true)
                .open(probe.join("cgroup.procs"))
                .is_ok();
        #[allow(clippy::let_underscore_must_use)]
        // ignored-ok (procfix7): removal of our own probe directory
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
        let own = crate::resources::own_cgroup_dir()?;
        // SAFETY: getuid has no arguments and cannot fail.
        let uid = unsafe { libc::getuid() };
        let meta = crate::fsx::presence("cgroup", &own)
            .metadata()
            .map_err(|e| format!("{}: {e}", own.display()))?;
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
        match crate::fsx::presence("cgroup", &leaf) {
            crate::fsx::Presence::Present(_) => {}
            crate::fsx::Presence::Absent => {
                fs::create_dir(&leaf).map_err(|e| format!("{}: {e}", leaf.display()))?;
            }
            crate::fsx::Presence::Unknown(e) => {
                return Err(format!("{} cannot be checked: {e}", leaf.display()));
            }
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
            #[allow(clippy::let_underscore_must_use)]
            // ignored-ok (procfix7): an optional knob (absent without swap accounting)
            let _ = write_file(&dir.join("memory.swap.max"), "0"); // absent without swap accounting
            #[allow(clippy::let_underscore_must_use)]
            // ignored-ok (procfix7): an optional knob (absent on older kernels)
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
                #[allow(clippy::let_underscore_must_use)]
                // ignored-ok (procfix7): removal of our own empty job cgroup (an empty cgroup holds no process)
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

    /// `oom_kill` count of memory.events; procfix7 round 5: None when it cannot be read or parsed (UNKNOWN, read
    /// again at the next sample; never 0).
    pub fn oom_kills(&self) -> Option<u64> {
        match crate::fsx::read_to_string("cgroup", &self.dir.join("memory.events")) {
            crate::fsx::Read::Content(t) => t
                .lines()
                .find_map(|l| l.strip_prefix("oom_kill "))
                .and_then(|v| v.trim().parse().ok()),
            crate::fsx::Read::Absent | crate::fsx::Read::Failed(_) => None,
        }
    }

    /// `cgroup.kill` (Linux >= 5.14): SIGKILL to every process of the cgroup, escapees included.
    pub fn kill_all(&self) {
        #[allow(clippy::let_underscore_must_use)]
        // ignored-ok (procfix7): the kill is followed by the proof passes (cgroup.procs belt): survivors make the end unproven
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
        #[allow(clippy::let_underscore_must_use)]
        // ignored-ok (procfix7): removal of our own empty job cgroup (an empty cgroup holds no process)
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
        let own = crate::resources::own_cgroup_dir()?;
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
            match crate::fsx::presence("cgroup", &dir).metadata() {
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
            .all(|f| crate::fsx::presence("cgroup", &probe.join(f)).is_present())
                && OpenOptions::new()
                    .write(true)
                    .open(probe.join("cgroup.procs"))
                    .is_ok();
            #[allow(clippy::let_underscore_must_use)]
            // ignored-ok (procfix7): removal of our own probe directory
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
                #[allow(clippy::let_underscore_must_use)]
                // ignored-ok (procfix7): removal of our own empty job cgroup (an empty cgroup holds no process)
                let _ = fs::remove_dir(&dir);
                Err(format!("open {}/cgroup.procs: {e}", dir.display()))
            }
        }
    }
}

/// procfix7 round 6 (gate10 m10-3): `populated` of `<dir>/cgroup.events`: the kernel's RECURSIVE answer (a process in
/// this cgroup or any child cgroup). A missing line or an unparsable value is an error.
pub(super) fn populated(dir: &Path) -> io::Result<bool> {
    let t = read(&dir.join("cgroup.events"))?;
    match t
        .lines()
        .find_map(|l| l.strip_prefix("populated "))
        .map(str::trim)
    {
        Some("0") => Ok(false),
        Some("1") => Ok(true),
        other => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("cgroup.events: populated {other:?}"),
        )),
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

/// procfix7 round 6 (gate10 m10-3, the listing rule for cgroups): the processes of `dir` AND of every cgroup below it
/// (cgroup.procs lists only a cgroup's OWN members; a process of the job that created and entered a child cgroup is
/// listed there). The listing of child cgroups is complete or an error (fsx), and the caller cross-checks an empty
/// answer with the kernel's RECURSIVE `cgroup.events` populated flag (the independent second source).
pub(super) fn procs_recursive(dir: &Path) -> io::Result<Vec<u32>> {
    let mut out = procs_of(dir)?;
    for e in crate::fsx::list_required("cgroup", dir)? {
        if e.file_type.is_dir() {
            out.extend(procs_recursive(&e.path)?);
        }
    }
    Ok(out)
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
        // the belt is additional to the guarded hunt: a directory that cannot be checked = the belt is not used
        match Self::check_dir(&dir, uid) {
            Ok(Some((dev, ino))) => Some(ScopeRef { dir, dev, ino, sid }),
            Ok(None) | Err(_) => None,
        }
    }

    /// procfix7 round 5: (dev, inode) of our directory; Ok(None) when PROVEN absent, not a directory or not ours; Err
    /// when it cannot be checked (never "gone").
    fn check_dir(dir: &Path, uid: u32) -> io::Result<Option<(u64, u64)>> {
        match crate::fsx::presence_lstat("cgroup", dir) {
            crate::fsx::Presence::Present(md) => {
                Ok((md.file_type().is_dir() && md.uid() == uid).then(|| (md.dev(), md.ino())))
            }
            crate::fsx::Presence::Absent => Ok(None),
            crate::fsx::Presence::Unknown(e) => {
                Err(io::Error::new(e.kind(), format!("{}: {e}", dir.display())))
            }
        }
    }

    /// The same directory as recorded (dev, inode, owner)? `Ok(false)`: gone or replaced (no process of ours in it);
    /// Err: it cannot be checked.
    fn still_same(&self, uid: u32) -> io::Result<bool> {
        Ok(Self::check_dir(&self.dir, uid)? == Some((self.dev, self.ino)))
    }

    /// Live processes in the scope other than the job's own (pinned) leader. A scope directory that no longer exists,
    /// or is not the recorded one, holds nothing of ours: empty. A failed read of an existing scope: error.
    pub(crate) fn live_members(&self) -> Result<Vec<u32>, String> {
        let uid = crate::resources::our_uid();
        if !self
            .still_same(uid)
            .map_err(|e| format!("scope directory cannot be checked: {e}"))?
        {
            return Ok(Vec::new());
        }
        // procfix7 round 6 (gate10 m10-3): the scope's subtree (a process that moved into a child cgroup of the scope is
        // listed there, not in the scope's own cgroup.procs)
        let pids = match procs_recursive(&self.dir) {
            Ok(p) => p,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => {
                return Err(format!(
                    "{} (cgroup.procs of the subtree): {e}",
                    self.dir.display()
                ));
            }
        };
        // ... and the kernel's RECURSIVE answer, cgroup.events `populated`: a scope populated without a listed member is
        // never "empty" (the listing rule's second source)
        if pids.iter().all(|&p| p == self.sid) {
            match populated(&self.dir) {
                Ok(false) => {}
                Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
                Ok(true) if pids.contains(&self.sid) => {}
                Ok(true) => {
                    return Err(format!(
                        "{}: populated (cgroup.events) but no member listed in cgroup.procs (a process in a child cgroup?)",
                        self.dir.display()
                    ));
                }
                Err(e) => return Err(format!("{}/cgroup.events: {e}", self.dir.display())),
            }
        }
        let mut out = Vec::new();
        for pid in pids {
            if pid == self.sid {
                continue;
            }
            match crate::resources::read_stat(pid) {
                Ok(st) if !st.is_dead() => out.push(pid),
                Ok(_) | Err(crate::resources::ProcError::Gone) => {}
                Err(e) => return Err(format!("stat of scope member {pid}: {e}")),
            }
        }
        Ok(out)
    }

    /// `cgroup.kill` of the job's scope: kills every process in it. Only after the guarded hunt (never instead of it),
    /// and only while the directory is still the recorded one (dev, inode, owner) under our user manager.
    pub(crate) fn kill_members(&self) -> io::Result<()> {
        let uid = crate::resources::our_uid();
        if !self.dir.starts_with(user_manager_root(uid)) || !self.still_same(uid)? {
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
/// procfix7 round 7 (gate11 m11-rec-path): where a recorded scope may be: FIRST the cgroup directory verified at the
/// job's start (`job_containment` note), accepted only under `root` (our user manager) with the exact name
/// `<unit>.scope`; then the two places systemd puts a user scope by default.
fn scope_candidates_in(root: &Path, unit: &str, recorded: Option<&Path>) -> Vec<PathBuf> {
    let name = format!("{unit}.scope");
    let mut v = Vec::new();
    if let Some(r) = recorded
        && r.starts_with(root)
        && r.file_name().and_then(|n| n.to_str()) == Some(name.as_str())
        && !r
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        v.push(r.to_path_buf());
    }
    for d in [root.join("app.slice").join(&name), root.join(&name)] {
        if !v.contains(&d) {
            v.push(d);
        }
    }
    v
}

/// RESTART BACKSTOP (gate3 M5): kill every process left in the job scope `<unit>.scope` whose NAME and leader pid the
/// caller found recorded in THIS campaign's ledger (the supervisor checks both: the launch record names the unit, the
/// `started` record the leader of this boot). Exact directory names only (never a pattern), the directory must be ours
/// (owner uid) and lie under our user manager. `Ok(None)`: no such scope (gone, or systemd never made it); `Ok(Some(n))`:
/// `cgroup.kill` written for a scope with `n` live processes (logged as a delivery, proof "job scope"); `Ok(Some(0))`:
/// the scope exists but is empty (nothing signalled).
pub fn kill_recorded_scope(unit: &str) -> io::Result<Option<usize>> {
    kill_recorded_scope_at(unit, None)
}

/// procfix7 round 7 (gate11 m11-rec-path): [`kill_recorded_scope`] that looks FIRST in the scope's recorded (verified)
/// cgroup directory.
pub fn kill_recorded_scope_at(unit: &str, recorded: Option<&Path>) -> io::Result<Option<usize>> {
    let uid = crate::resources::our_uid();
    kill_recorded_scope_in(&user_manager_root(uid), unit, recorded, uid)
}

/// The backstop under a given user-manager root (tests: an emulated cgroup tree).
fn kill_recorded_scope_in(
    root: &Path,
    unit: &str,
    recorded: Option<&Path>,
    uid: u32,
) -> io::Result<Option<usize>> {
    for dir in scope_candidates_in(root, unit, recorded) {
        // procfix7 (gate7 B1, the class): only an ABSENT scope is "no such scope"; another failed read is an error
        let md = match crate::fsx::presence_lstat("cgroup", &dir) {
            crate::fsx::Presence::Present(md) => md,
            crate::fsx::Presence::Absent => continue,
            crate::fsx::Presence::Unknown(e) => {
                return Err(io::Error::new(e.kind(), format!("{}: {e}", dir.display())));
            }
        };
        if !md.file_type().is_dir() || md.uid() != uid {
            continue;
        }
        let procs = procs_recursive(&dir)?;
        // procfix7 round 6 (gate10 m10-3): empty only when the kernel's RECURSIVE populated flag says so (the subtree's
        // cgroup.procs listings are the first source); cgroup.kill is recursive
        if procs.is_empty() && !populated(&dir)? {
            return Ok(Some(0));
        }
        super::guard::cgroup_kill_logged(&dir, "job scope", || {
            write_file(&dir.join("cgroup.kill"), "1")
        })?;
        return Ok(Some(procs.len().max(1)));
    }
    Ok(None)
}

/// procfix7 round 6 (gate10 M10-1 (b)): after a cgroup.kill of the recorded scope `unit`, true once the kernel reports
/// it EMPTY (its recursive `cgroup.events` populated 0), or the scope directory is gone (systemd removes a transient scope
/// once it is empty; it existed a moment ago: we just killed it), within `limit`. Any failed read: false (not proven).
pub fn recorded_scope_emptied(
    unit: &str,
    recorded: Option<&Path>,
    limit: std::time::Duration,
) -> bool {
    let uid = crate::resources::our_uid();
    let root = user_manager_root(uid);
    let t0 = std::time::Instant::now();
    loop {
        let mut present = false;
        for dir in scope_candidates_in(&root, unit, recorded) {
            match crate::fsx::presence_lstat("cgroup", &dir) {
                crate::fsx::Presence::Absent => continue,
                crate::fsx::Presence::Unknown(_) => return false,
                crate::fsx::Presence::Present(md) if md.file_type().is_dir() && md.uid() == uid => {
                    present = true;
                    match populated(&dir) {
                        Ok(false) => return true,
                        Ok(true) => {}
                        Err(_) => return false,
                    }
                }
                crate::fsx::Presence::Present(_) => continue,
            }
        }
        if !present {
            return true;
        }
        if t0.elapsed() >= limit {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

/// procfix7 round 7 (gate11 m11-gone): a scope VERIFIED at its job's start whose recorded cgroup directory is now PROVEN
/// ABSENT (ENOENT; its parent directory present, so the cgroup tree is there and the path is right) was emptied: on
/// cgroup v2 a cgroup with a live member, or with a child cgroup, cannot be removed, and a cgroup cannot be renamed, so
/// every process that stayed in it is gone. (systemd's "unit not found" would add nothing: systemd removes the unit
/// from the same kernel answer.) Any other answer, or a path not under our user manager with the exact unit name: false.
pub fn recorded_scope_removed(unit: &str, recorded: &Path) -> bool {
    let root = user_manager_root(crate::resources::our_uid());
    recorded_scope_removed_in(&root, unit, recorded)
}

fn recorded_scope_removed_in(root: &Path, unit: &str, recorded: &Path) -> bool {
    if scope_candidates_in(root, unit, Some(recorded)).first() != Some(&recorded.to_path_buf()) {
        return false;
    }
    let Some(parent) = recorded.parent() else {
        return false;
    };
    matches!(
        crate::fsx::presence_lstat("cgroup", recorded),
        crate::fsx::Presence::Absent
    ) && matches!(
        crate::fsx::presence_lstat("cgroup", parent),
        crate::fsx::Presence::Present(m) if m.is_dir()
    )
}

#[allow(clippy::let_underscore_must_use)] // test cleanup (outside the reviewed scope, procfix7)
#[allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code
#[cfg(test)]
mod tests {
    use super::*;

    /// procfix7 round 7 (gate11 m11-mut M8): the restart backstop on an EMULATED scope tree: a scope whose cgroup.procs
    /// lists nobody (the processes sit in a child cgroup) but whose recursive `populated` is 1 is NOT empty: cgroup.kill
    /// is written. Truly empty (populated 0): nothing written, Some(0). The recorded path is looked at first (m11-rec-path);
    /// a recorded path that is gone with its parent present is "removed" (m11-gone); a path outside the root never is.
    #[test]
    fn the_recorded_scope_backstop_judges_emptiness_by_the_recursive_flag() {
        let root = std::env::temp_dir().join(format!("pf11-cg-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let unit = "rr-test-0123456789abcdef";
        let deep = root
            .join("app.slice")
            .join("custom.slice")
            .join(format!("{unit}.scope"));
        std::fs::create_dir_all(deep.join("child")).unwrap();
        std::fs::write(deep.join("cgroup.procs"), "").unwrap();
        std::fs::write(deep.join("child").join("cgroup.procs"), "").unwrap();
        std::fs::write(deep.join("cgroup.events"), "populated 1\nfrozen 0\n").unwrap();
        std::fs::write(deep.join("cgroup.kill"), "").unwrap();
        let uid = crate::resources::our_uid();
        // not at a default place: found only through the recorded path
        assert_eq!(
            kill_recorded_scope_in(&root, unit, None, uid).unwrap(),
            None
        );
        let r = kill_recorded_scope_in(&root, unit, Some(&deep), uid).unwrap();
        assert_eq!(r, Some(1), "populated without a listed member is not empty");
        assert_eq!(
            std::fs::read_to_string(deep.join("cgroup.kill")).unwrap(),
            "1"
        );
        // truly empty
        std::fs::write(deep.join("cgroup.events"), "populated 0\nfrozen 0\n").unwrap();
        std::fs::write(deep.join("cgroup.kill"), "").unwrap();
        assert_eq!(
            kill_recorded_scope_in(&root, unit, Some(&deep), uid).unwrap(),
            Some(0)
        );
        assert_eq!(
            std::fs::read_to_string(deep.join("cgroup.kill")).unwrap(),
            ""
        );
        // removed (m11-gone): absent with its parent present; never for a path outside the root or another name
        assert!(!recorded_scope_removed_in(&root, unit, &deep));
        std::fs::remove_dir_all(&deep).unwrap();
        assert!(recorded_scope_removed_in(&root, unit, &deep));
        assert!(!recorded_scope_removed_in(
            &root,
            "rr-other-0123456789abcdef",
            &deep
        ));
        let outside = std::env::temp_dir().join(format!("{unit}.scope"));
        assert!(!recorded_scope_removed_in(&root, unit, &outside));
        let _ = std::fs::remove_dir_all(&root);
    }

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
