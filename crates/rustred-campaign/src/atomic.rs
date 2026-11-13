//! Atomic file installation: write a temporary file in the target's directory, fsync it, rename (or hard-link
//! for no-clobber creation) over the target, fsync the parent directory.
//!
//! Approach copied, with attribution, from `crates/rustred-app/src/application/atomic_file.rs` at b9f06de7
//! (`write_with`: temp `create_new` + `sync_all` + rename / hard_link + directory fsync, temp removed on error).
//! Differences: a typed error ([`AtomicError`]) that keeps "disk full" (ENOSPC, EDQUOT) distinguishable,
//! no Symbolica resource checkpoints, a bounded reader, and a staging abstraction ([`Staging`]) so tests can
//! inject a writer that fails like a full disk or an exhausted quota.
//!
//! Guarantees (on a POSIX file system with atomic rename, ext4/ZFS/XFS):
//! * after any crash, the target holds either its complete old content or its complete new content;
//! * on any error the old target is untouched, and the temporary file is removed (best effort: removal of a
//!   file needs no free space, so only an I/O error of the directory itself can leave one; such leftovers are
//!   named `.<name>.rr-tmp-<pid>-<n>` and [`remove_stale_temporaries`] deletes them at start-up).

use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

/// Typed errors of the atomic layer.
#[derive(Debug)]
pub enum AtomicError {
    /// ENOSPC or EDQUOT while writing, syncing or installing. The old target is intact.
    NoSpace { path: PathBuf, op: &'static str },
    /// No-clobber creation found the target present.
    Exists { path: PathBuf },
    /// A bounded read found more bytes than allowed.
    TooLarge { path: PathBuf, cap: u64 },
    /// Any other I/O error.
    Io {
        path: PathBuf,
        op: &'static str,
        source: io::Error,
    },
}

impl AtomicError {
    pub fn is_no_space(&self) -> bool {
        matches!(self, AtomicError::NoSpace { .. })
    }
    fn from_io(path: &Path, op: &'static str, e: io::Error) -> Self {
        if is_no_space(&e) {
            AtomicError::NoSpace {
                path: path.to_path_buf(),
                op,
            }
        } else if e.kind() == io::ErrorKind::AlreadyExists && op == "install" {
            AtomicError::Exists {
                path: path.to_path_buf(),
            }
        } else {
            AtomicError::Io {
                path: path.to_path_buf(),
                op,
                source: e,
            }
        }
    }
}

impl std::fmt::Display for AtomicError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AtomicError::NoSpace { path, op } => {
                write!(
                    f,
                    "no space left (ENOSPC/EDQUOT) during {op} of {}; the previous file is intact",
                    path.display()
                )
            }
            AtomicError::Exists { path } => {
                write!(f, "{} already exists (no-clobber create)", path.display())
            }
            AtomicError::TooLarge { path, cap } => write!(
                f,
                "{} is larger than the cap of {cap} bytes",
                path.display()
            ),
            AtomicError::Io { path, op, source } => {
                write!(f, "I/O error during {op} of {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for AtomicError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AtomicError::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// True for ENOSPC (28) and EDQUOT (122): both mean "free space first, then resume".
pub fn is_no_space(e: &io::Error) -> bool {
    matches!(e.raw_os_error(), Some(libc::ENOSPC) | Some(libc::EDQUOT))
        || e.kind() == io::ErrorKind::StorageFull
}

/// Replace the target or refuse to clobber it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Replace,
    NoClobber,
}

/// A writable, syncable staging sink.
pub trait StagingWrite: Write {
    fn sync(&mut self) -> io::Result<()>;
}

impl StagingWrite for File {
    fn sync(&mut self) -> io::Result<()> {
        self.sync_all()
    }
}

/// How the temporary file is opened. The real implementation creates `path` with `create_new`; tests wrap it
/// to fail like a full disk.
pub trait Staging {
    fn create(&self, temp_path: &Path) -> io::Result<Box<dyn StagingWrite>>;
}

/// The real staging: `OpenOptions::create_new` (O_EXCL), mode 0644.
#[derive(Clone, Copy, Debug, Default)]
pub struct RealStaging;

impl Staging for RealStaging {
    fn create(&self, temp_path: &Path) -> io::Result<Box<dyn StagingWrite>> {
        Ok(Box::new(
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(temp_path)?,
        ))
    }
}

/// Writes `bytes` atomically to `path` (replacing it).
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), AtomicError> {
    write_atomic_with(path, Mode::Replace, &RealStaging, |w| w.write_all(bytes))
}

/// Creates `path` atomically with `bytes`; refuses (typed `Exists`) if it exists, also when it appears
/// concurrently (hard link is atomic and fails on an existing name).
pub fn create_new_atomic(path: &Path, bytes: &[u8]) -> Result<(), AtomicError> {
    write_atomic_with(path, Mode::NoClobber, &RealStaging, |w| w.write_all(bytes))
}

/// Writes a JSON value (compact, one trailing newline) atomically.
pub fn write_json_atomic<T: serde::Serialize>(path: &Path, value: &T) -> Result<(), AtomicError> {
    let mut bytes = serde_json::to_vec(value).map_err(|e| AtomicError::Io {
        path: path.to_path_buf(),
        op: "serialise",
        source: io::Error::other(e),
    })?;
    bytes.push(b'\n');
    write_atomic(path, &bytes)
}

/// The general form: `write` fills the staging sink; then sync, install, directory sync.
pub fn write_atomic_with(
    path: &Path,
    mode: Mode,
    staging: &dyn Staging,
    write: impl FnOnce(&mut dyn StagingWrite) -> io::Result<()>,
) -> Result<(), AtomicError> {
    let file_name = path.file_name().ok_or_else(|| AtomicError::Io {
        path: path.to_path_buf(),
        op: "name",
        source: io::Error::new(io::ErrorKind::InvalidInput, "path has no file name"),
    })?;
    if mode == Mode::NoClobber && fs::symlink_metadata(path).is_ok() {
        return Err(AtomicError::Exists {
            path: path.to_path_buf(),
        });
    }
    let parent = parent_of(path);
    let mut attempt = 0u64;
    let (temp, mut sink) = loop {
        let candidate = temporary_path(parent, file_name, attempt);
        match staging.create(&candidate) {
            Ok(s) => break (candidate, s),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists && attempt < 1_000_000 => {
                attempt += 1
            }
            Err(e) => return Err(AtomicError::from_io(path, "create temporary", e)),
        }
    };
    let result = (|| {
        write(sink.as_mut()).map_err(|e| AtomicError::from_io(path, "write", e))?;
        sink.flush()
            .map_err(|e| AtomicError::from_io(path, "write", e))?;
        sink.sync()
            .map_err(|e| AtomicError::from_io(path, "sync", e))?;
        drop(sink);
        match mode {
            Mode::Replace => {
                fs::rename(&temp, path).map_err(|e| AtomicError::from_io(path, "install", e))?
            }
            Mode::NoClobber => {
                fs::hard_link(&temp, path).map_err(|e| AtomicError::from_io(path, "install", e))?;
                // The target is installed; a failure to drop the staging name is reported but harmless.
                fs::remove_file(&temp)
                    .map_err(|e| AtomicError::from_io(path, "remove staging link", e))?;
            }
        }
        sync_dir(parent).map_err(|e| AtomicError::from_io(path, "sync directory", e))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

/// Reads a whole file, refusing files larger than `cap` bytes (the check is on the bytes actually read, so a
/// file growing during the read is also caught).
pub fn read_capped(path: &Path, cap: u64) -> Result<Vec<u8>, AtomicError> {
    let file = File::open(path).map_err(|e| AtomicError::from_io(path, "open", e))?;
    let mut buf = Vec::new();
    file.take(cap.saturating_add(1))
        .read_to_end(&mut buf)
        .map_err(|e| AtomicError::from_io(path, "read", e))?;
    if buf.len() as u64 > cap {
        return Err(AtomicError::TooLarge {
            path: path.to_path_buf(),
            cap,
        });
    }
    Ok(buf)
}

/// fsync of a directory (makes renames and creations in it durable).
pub fn sync_dir(dir: &Path) -> io::Result<()> {
    File::open(dir)?.sync_all()
}

/// Deletes leftover temporaries of this module in `dir` (names `.<x>.rr-tmp-<pid>-<n>`). Returns the count.
pub fn remove_stale_temporaries(dir: &Path) -> io::Result<usize> {
    let mut n = 0;
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let s = name.to_string_lossy();
        if s.starts_with('.') && s.contains(".rr-tmp-") && entry.file_type()?.is_file() {
            fs::remove_file(entry.path())?;
            n += 1;
        }
    }
    Ok(n)
}

fn parent_of(path: &Path) -> &Path {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

fn temporary_path(parent: &Path, file_name: &std::ffi::OsStr, attempt: u64) -> PathBuf {
    let mut name = OsString::from(".");
    name.push(file_name);
    name.push(format!(".rr-tmp-{}-{attempt}", std::process::id()));
    parent.join(name)
}

/// Streaming BLAKE3 of a file (64 KiB reads); returns (bytes, lowercase hex).
pub fn blake3_file(path: &Path) -> io::Result<(u64, String)> {
    let mut f = File::open(path)?;
    let mut h = blake3::Hasher::new();
    let mut buf = vec![0u8; 64 * 1024];
    let mut n = 0u64;
    loop {
        let k = f.read(&mut buf)?;
        if k == 0 {
            break;
        }
        h.update(&buf[..k]);
        n += k as u64;
    }
    Ok((n, h.finalize().to_hex().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn scratch(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "rr-atomic-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn replace_and_no_clobber() {
        let d = scratch("basic");
        let p = d.join("a.json");
        write_atomic(&p, b"one").unwrap();
        write_atomic(&p, b"two").unwrap();
        assert_eq!(fs::read(&p).unwrap(), b"two");
        assert!(matches!(
            create_new_atomic(&p, b"three"),
            Err(AtomicError::Exists { .. })
        ));
        assert_eq!(fs::read(&p).unwrap(), b"two");
        create_new_atomic(&d.join("b"), b"x").unwrap();
        assert_eq!(fs::read_dir(&d).unwrap().count(), 2, "no stray temporaries");
        assert!(matches!(
            read_capped(&p, 2),
            Err(AtomicError::TooLarge { .. })
        ));
        assert_eq!(read_capped(&p, 3).unwrap(), b"two");
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn stale_temporaries_are_removed() {
        let d = scratch("stale");
        fs::write(d.join(".x.json.rr-tmp-1-0"), b"junk").unwrap();
        fs::write(d.join("keep.json"), b"k").unwrap();
        assert_eq!(remove_stale_temporaries(&d).unwrap(), 1);
        assert_eq!(fs::read_dir(&d).unwrap().count(), 1);
        fs::remove_dir_all(&d).unwrap();
    }
}
