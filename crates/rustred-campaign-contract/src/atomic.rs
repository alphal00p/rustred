//! Atomic single-object files (child-job.md 3.9: a unique `NAME.tmp.<pid>.<thread>.<n>` in the same directory,
//! write_all, fsync, rename (or, for OUTCOME, a no-clobber hard link), fsync of the directory), ENOSPC / EDQUOT classification and the disk reserve (3.10). std only, no unsafe: a
//! directory is fsynced by opening it read-only and calling `sync_all` (valid on Linux).

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// errno ENOSPC on Linux.
pub const ENOSPC: i32 = 28;
/// errno EDQUOT on Linux.
pub const EDQUOT: i32 = 122;

static TEMP_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// A fresh temporary name for `path` in the same directory: `NAME.tmp.<pid>.<thread>.<n>`, unique per process,
/// thread and call (audit A m5: two threads never share a temporary inode); it is created with O_EXCL.
pub fn temp_path(path: &Path) -> PathBuf {
    use std::hash::{Hash, Hasher};
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    std::thread::current().id().hash(&mut h);
    let n = TEMP_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    path.with_file_name(format!(
        "{name}.tmp.{}.{:x}.{n}",
        std::process::id(),
        h.finish() & 0xffff_ffff
    ))
}

/// fsync a directory.
pub fn fsync_dir(dir: &Path) -> io::Result<()> {
    File::open(dir)?.sync_all()
}

fn parent_of(path: &Path) -> &Path {
    match path.parent() {
        Some(d) if !d.as_os_str().is_empty() => d,
        _ => Path::new("."),
    }
}

/// Write a fresh temporary file (O_EXCL + O_NOFOLLOW) with `bytes`, fsynced.
fn write_temp(path: &Path, bytes: &[u8]) -> io::Result<PathBuf> {
    use std::os::unix::fs::OpenOptionsExt;
    let tmp = temp_path(path);
    let r = (|| {
        let mut f = OpenOptions::new()
            .write(true)
            .create_new(true)
            .custom_flags(crate::seal::O_NOFOLLOW)
            .open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()
    })();
    match r {
        Ok(()) => Ok(tmp),
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            Err(e)
        }
    }
}

/// Write `bytes` to `path` atomically: readers see either the old file or the complete new one, and after `Ok` the
/// new content and the directory entry are on disk. On error the temporary file is removed (best effort). A symlink
/// at `path` is replaced, never followed (rename replaces the entry).
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let tmp = write_temp(path, bytes)?;
    let r = fs::rename(&tmp, path).and_then(|_| fsync_dir(parent_of(path)));
    if r.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    r
}

/// Like [`write_atomic`], but NEVER replaces an existing entry (audit A C4): the complete temporary file is
/// hard-linked to `path` (atomic, fails with `AlreadyExists` if anything, even a dangling symlink, is there), then
/// the temporary name is removed. Used for OUTCOME.json: a second writer or a second start cannot clobber it.
pub fn write_atomic_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let tmp = write_temp(path, bytes)?;
    let r = fs::hard_link(&tmp, path);
    let _ = fs::remove_file(&tmp);
    r?;
    fsync_dir(parent_of(path))
}

/// [`write_atomic`] of a value as pretty JSON + "\n".
pub fn write_json_atomic<T: serde::Serialize>(path: &Path, value: &T) -> io::Result<()> {
    let mut v = serde_json::to_vec_pretty(value).map_err(io::Error::other)?;
    v.push(b'\n');
    write_atomic(path, &v)
}

/// Whether an I/O error is "no space" (ENOSPC or EDQUOT): exit 50, else an output error is exit 51.
pub fn is_nospace(e: &io::Error) -> bool {
    matches!(e.raw_os_error(), Some(ENOSPC) | Some(EDQUOT))
        || e.kind() == io::ErrorKind::StorageFull
}

/// The exit code for an output I/O error (3.10).
pub fn exit_for_io(e: &io::Error) -> crate::ExitCode {
    if is_nospace(e) {
        crate::ExitCode::NoSpace
    } else {
        crate::ExitCode::Io
    }
}

/// Create the disk reserve (a fresh file): `bytes` real bytes (written, not a sparse file) and fsynced.
pub fn create_reserve(path: &Path, bytes: u64) -> io::Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    // O_EXCL + O_NOFOLLOW: a planted symlink is never followed (audit B F7)
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .custom_flags(crate::seal::O_NOFOLLOW)
        .open(path)?;
    let chunk = vec![0u8; 64 * 1024];
    let mut left = bytes;
    while left > 0 {
        let n = left.min(chunk.len() as u64) as usize;
        f.write_all(&chunk[..n])?;
        left -= n as u64;
    }
    f.sync_all()
}

/// Delete the reserve (on ENOSPC, before writing OUTCOME). Missing is fine.
pub fn release_reserve(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nospace_classes() {
        assert!(is_nospace(&io::Error::from_raw_os_error(ENOSPC)));
        assert!(is_nospace(&io::Error::from_raw_os_error(EDQUOT)));
        assert!(!is_nospace(&io::Error::from_raw_os_error(5)));
        assert_eq!(
            exit_for_io(&io::Error::from_raw_os_error(5)),
            crate::ExitCode::Io
        );
    }
}
