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

/// `cgroup.kill` of a job cgroup WE created (the only caller passes `JobCgroup` paths).
pub(super) fn kill_path(dir: &Path) -> io::Result<()> {
    write_file(&dir.join("cgroup.kill"), "1")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_refuses_non_cgroup_paths() {
        assert!(CgroupRoot::probe(Path::new("/tmp")).is_err());
        assert!(CgroupRoot::probe(Path::new("/sys/fs/cgroup/does-not-exist-rr")).is_err());
        // prepare_in_own_scope is deliberately not exercised here: it may move the calling process (tests must not
        // change the host's cgroup layout).
    }
}
