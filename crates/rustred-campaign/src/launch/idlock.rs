//! HOST-WIDE CAMPAIGN-ID LOCK (gate re-audit B RB-M2, re-audit A C-1).
//!
//! The campaign id is the only proof of ownership: two LIVE supervisors with the same id would treat each other's jobs
//! as their own (re-audit B scenario "two": B's verification and orphan scan killed A's live jobs). At construction the
//! launcher takes an exclusive advisory lock (flock) on `<dir>/<id>.lock`, where `<dir>` is a per-user runtime directory
//! (`$XDG_RUNTIME_DIR/rustred-campaign`, else `/tmp/rustred-campaign-<uid>`; or the configured path), created with mode
//! 0700 and refused when it is not ours or is group/other writable. The lock lives as long as the process (the kernel
//! drops it when the process dies, also on SIGKILL); the descriptor is close-on-exec, so jobs do not inherit it. A second
//! process asking for the same id is REFUSED (and signals nothing). Several launchers in ONE process share the lock (one
//! supervisor process, one registry of leaders).

use std::collections::HashMap;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, Weak};

/// Why the id lock was not obtained.
#[derive(Debug)]
pub enum IdLockError {
    /// Another live process holds the lock of this id.
    InUse { path: PathBuf, holder: Option<u32> },
    /// The lock directory or file is unusable (not ours, permissions, I/O).
    Unusable { path: PathBuf, why: String },
}

impl fmt::Display for IdLockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IdLockError::InUse { path, holder } => write!(
                f,
                "campaign id already in use by a live process{} (lock {})",
                holder.map(|p| format!(" (pid {p})")).unwrap_or_default(),
                path.display()
            ),
            IdLockError::Unusable { path, why } => {
                write!(f, "campaign id lock {} unusable: {why}", path.display())
            }
        }
    }
}

impl std::error::Error for IdLockError {}

/// A held id lock (shared by the launchers of this process).
#[derive(Debug)]
pub(crate) struct IdLock {
    path: PathBuf,
    file: File,
    adhoc: bool,
}

impl IdLock {
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for IdLock {
    fn drop(&mut self) {
        // An ad-hoc id is unique to this process: remove its file while still holding the lock. A campaign's file stays
        // (removing a lock file another process may be opening would split the lock).
        // procfix7 round 5 (positive proof): only while the path still names the file we hold (same device and inode,
        // read back now)
        use std::os::unix::fs::MetadataExt;
        let ours = match (
            self.file.metadata(),
            crate::fsx::presence_lstat("idlock", &self.path),
        ) {
            (Ok(a), crate::fsx::Presence::Present(b)) => a.dev() == b.dev() && a.ino() == b.ino(),
            _ => false,
        };
        if self.adhoc && ours {
            #[allow(clippy::let_underscore_must_use)]
            // ignored-ok (procfix7): the flock is the lock; the file is only removed as a courtesy
            let _ = fs::remove_file(&self.path);
        }
        #[allow(clippy::let_underscore_must_use)]
        // ignored-ok (procfix7): not an error: keeps the locked file alive until here
        let _ = &self.file;
    }
}

/// `/tmp/rustred-campaign-<uid>`: a path derived from the uid ONLY (procfix3, gate-A m-6). It used to follow the
/// starting environment's XDG_RUNTIME_DIR, so two starts with different values locked different files (and
/// `/run/user/<uid>` disappears at a full logout). The directory checks of `prepare_dir` (a real directory, owned by us,
/// not group / other writable; a foreign pre-created one is refused) are unchanged; the sticky /tmp keeps others from
/// replacing our entry. The CDIR flock remains the second, independent guard against two supervisors of one directory.
///
/// procfix7 round 7 (gate11 m11-tmp): INSIDE A PRIVATE PID NAMESPACE (the test suites; the host refuses to start there
/// without the test override) the host's /tmp is shared by every namespace of every lane: the namespaced runs had left
/// ~25,000 lock files in /tmp/rustred-campaign-0. There the directory is `$TMPDIR/rustred-campaign-<uid>` when TMPDIR names
/// a private directory (absolute, not /tmp itself): every supervisor that can be a copy of another in such a run shares
/// that TMPDIR (one test, one namespace). On the host: unchanged.
pub fn default_lock_dir() -> PathBuf {
    let uid = crate::resources::our_uid();
    if crate::launch::namespace_state() == Ok(true)
        && let Some(t) = std::env::var_os("TMPDIR").map(PathBuf::from)
        && t.is_absolute()
        && t.as_os_str() != "/tmp"
        && t.as_os_str() != "/tmp/"
    {
        return t.join(format!("rustred-campaign-{uid}"));
    }
    PathBuf::from(format!("/tmp/rustred-campaign-{uid}"))
}

fn prepare_dir(dir: &Path) -> Result<(), IdLockError> {
    let unusable = |why: String| IdLockError::Unusable {
        path: dir.to_path_buf(),
        why,
    };
    match crate::fsx::presence_lstat("idlock", dir) {
        crate::fsx::Presence::Present(_) => {}
        crate::fsx::Presence::Absent => {
            fs::DirBuilder::new()
                .mode(0o700)
                .create(dir)
                .or_else(|e| {
                    if e.kind() == io::ErrorKind::AlreadyExists {
                        Ok(())
                    } else {
                        Err(e)
                    }
                })
                .map_err(|e| unusable(format!("create: {e}")))?;
        }
        crate::fsx::Presence::Unknown(e) => return Err(unusable(e.to_string())),
    }
    let m = crate::fsx::presence_lstat("idlock", dir)
        .metadata()
        .map_err(|e| unusable(e.to_string()))?;
    if !m.file_type().is_dir() {
        return Err(unusable("not a directory (or a symlink)".into()));
    }
    if m.uid() != crate::resources::our_uid() {
        return Err(unusable(format!("owned by uid {}, not by us", m.uid())));
    }
    if m.permissions().mode() & 0o022 != 0 {
        return Err(unusable(format!(
            "mode {:o} is group/other writable",
            m.permissions().mode() & 0o777
        )));
    }
    Ok(())
}

fn registry() -> &'static Mutex<HashMap<PathBuf, Weak<IdLock>>> {
    static R: OnceLock<Mutex<HashMap<PathBuf, Weak<IdLock>>>> = OnceLock::new();
    R.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Take (or share, within this process) the lock of `id` in `dir` (None: [`default_lock_dir`]). The id must already be
/// validated (`[A-Za-z0-9._-]{1,64}`: a plain file name).
pub(crate) fn acquire(id: &str, dir: Option<&Path>) -> Result<Arc<IdLock>, IdLockError> {
    let dir = dir.map(Path::to_path_buf).unwrap_or_else(default_lock_dir);
    let path = dir.join(format!("{id}.lock"));
    let mut reg = registry().lock().unwrap_or_else(|p| p.into_inner());
    if let Some(l) = reg.get(&path).and_then(Weak::upgrade) {
        return Ok(l);
    }
    prepare_dir(&dir)?;
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&path)
        .map_err(|e| IdLockError::Unusable {
            path: path.clone(),
            why: format!("open: {e}"),
        })?;
    match file.try_lock() {
        Ok(()) => {}
        Err(fs::TryLockError::WouldBlock) => {
            let mut s = String::new();
            #[allow(clippy::let_underscore_must_use)]
            // ignored-ok (procfix7): the holder pid is read for the refusal message only
            let _ = file.read_to_string(&mut s);
            let holder = s.split_whitespace().next().and_then(|w| w.parse().ok());
            return Err(IdLockError::InUse { path, holder });
        }
        Err(fs::TryLockError::Error(e)) => {
            return Err(IdLockError::Unusable {
                path,
                why: format!("flock: {e}"),
            });
        }
    }
    // Informational content: who holds it.
    #[allow(clippy::let_underscore_must_use)]
    // ignored-ok (procfix7): diagnostic content of the lock file (the flock is the lock)
    let _ = file.set_len(0);
    #[allow(clippy::let_underscore_must_use)]
    // ignored-ok (procfix7): diagnostic content of the lock file (the flock is the lock)
    let _ = file.rewind();
    #[allow(clippy::let_underscore_must_use)]
    // ignored-ok (procfix7): diagnostic content of the lock file (the flock is the lock)
    let _ = writeln!(
        file,
        "{} {}",
        std::process::id(),
        // procfix8-ok: diagnostic content of the lock file (the flock is the lock; nothing is decided by it)
        crate::resources::boot_id().unwrap_or_default()
    );
    let l = Arc::new(IdLock {
        path: path.clone(),
        file,
        adhoc: id.starts_with("adhoc-"),
    });
    reg.insert(path, Arc::downgrade(&l));
    Ok(l)
}
