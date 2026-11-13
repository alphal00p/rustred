//! Atomic single-object files (child-job.md 3.9: `NAME.tmp.<pid>` in the same directory, write_all, fsync, rename,
//! fsync of the directory), ENOSPC / EDQUOT classification and the disk reserve (3.10). std only, no unsafe: a
//! directory is fsynced by opening it read-only and calling `sync_all` (valid on Linux).

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// errno ENOSPC on Linux.
pub const ENOSPC: i32 = 28;
/// errno EDQUOT on Linux.
pub const EDQUOT: i32 = 122;

/// The temporary name used for `path` by this process.
pub fn temp_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    path.with_file_name(format!("{name}.tmp.{}", std::process::id()))
}

/// fsync a directory.
pub fn fsync_dir(dir: &Path) -> io::Result<()> {
    File::open(dir)?.sync_all()
}

/// Write `bytes` to `path` atomically: readers see either the old file or the complete new one, and after `Ok` the
/// new content and the directory entry are on disk. On error the temporary file is removed (best effort).
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let tmp = temp_path(path);
    let r = (|| {
        let mut f = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        fs::rename(&tmp, path)?;
        let dir = match path.parent() {
            Some(d) if !d.as_os_str().is_empty() => d,
            _ => Path::new("."),
        };
        fsync_dir(dir)
    })();
    if r.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    r
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

/// Create the disk reserve: `bytes` real bytes (written, not a sparse file) and fsynced.
pub fn create_reserve(path: &Path, bytes: u64) -> io::Result<()> {
    let mut f = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
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
