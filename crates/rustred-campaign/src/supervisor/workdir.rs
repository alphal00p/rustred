//! procfix7 (gate6 MJ-B): who owns a run directory. A campaign directory copied with `--adopt-moved-campaign` whose
//! spec names an EXTERNAL `dirs.work_dir` (outside the campaign directory) shared that work directory with the
//! original: the copy's launch renamed the original's LIVE run directories aside and its settle read the other
//! campaign's results (gate6 cp_inner.sh: both campaigns rc 10, 7 directories renamed).
//!
//! Rules (never relaxed by `--adopt-moved-campaign`):
//! 1. An external work directory is CLAIMED by the first campaign that starts on it: `WORK/RR_CAMPAIGN_WORK_OWNER`
//!    holds the campaign's marker id, created exclusively. A start whose id differs is REFUSED (exit 1) before anything
//!    in the work directory is touched (no recovery, no rename, no read of a run directory). A work directory inside
//!    the campaign directory is copied with it and needs no claim.
//! 2. Every run directory is created ATOMICALLY WITH ITS IDENTITY (`RUN/sup/CAMPAIGN` = the marker id): built under a
//!    unique temporary name next to it, then renamed into place with RENAME_NOREPLACE. A run directory that already
//!    exists is renamed aside only if it carries THIS campaign's identity (a crash between its creation and the launch
//!    record); one carrying another campaign's identity, or none, is never renamed, removed or read: the run stops with
//!    a typed refusal (exit 1) naming it.
//! 3. The settle (live and at recovery) reads nothing of a run directory whose identity is not this campaign's.

use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

/// The claim file of an external work directory.
pub const WORK_OWNER_FILE: &str = "RR_CAMPAIGN_WORK_OWNER";

/// Is `work_root` outside `cdir` (both canonicalised)? Only such a work directory can be shared by two campaign
/// directories.
pub fn is_external(work_root: &Path, cdir: &Path) -> bool {
    let w = std::fs::canonicalize(work_root).unwrap_or_else(|_| work_root.to_path_buf());
    let c = std::fs::canonicalize(cdir).unwrap_or_else(|_| cdir.to_path_buf());
    !w.starts_with(&c)
}

/// Rule 1: claim an external work directory for the marker id `key` (or confirm the claim). `Err(refusal text)` when it
/// belongs to another campaign or the claim cannot be read; nothing else in the work directory is touched.
pub fn claim_work_dir(work_root: &Path, key: &str) -> Result<ClaimOutcome, String> {
    let p = work_root.join(WORK_OWNER_FILE);
    for _ in 0..3 {
        match std::fs::read_to_string(&p) {
            Ok(s) if s.trim() == key => return Ok(ClaimOutcome::Ours),
            Ok(s) => {
                return Err(format!(
                    "the work directory {} belongs to campaign {:?} ({}); this campaign is {key:?}: refused, nothing in it \
                     was touched. A copied campaign directory must not share its original's dirs.work_dir: give it its \
                     own work_dir (or, if the original campaign is gone for good, check that and replace the owner file \
                     by hand)",
                    work_root.display(),
                    s.trim(),
                    p.display()
                ));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                match crate::atomic::create_new_atomic(&p, format!("{key}\n").as_bytes()) {
                    Ok(()) => return Ok(ClaimOutcome::Claimed),
                    Err(crate::atomic::AtomicError::Exists { .. }) => continue,
                    Err(e) => return Err(format!("claiming {}: {e}", p.display())),
                }
            }
            Err(e) => return Err(format!("{}: {e}", p.display())),
        }
    }
    Err(format!("{}: changed while it was claimed", p.display()))
}

/// The result of a successful claim.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClaimOutcome {
    /// Created now.
    Claimed,
    /// Already this campaign's.
    Ours,
}

/// Whose run directory is this (from `RUN/sup/CAMPAIGN`)?
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunDirOwner {
    Ours,
    /// Another campaign's marker id.
    Other(String),
    /// No identity (absent / unreadable / empty): never ours.
    Unknown(String),
    /// procfix7 (gate7 B1): the identity could not be read for a supervisor resource limit (EMFILE, ENFILE, ENOMEM,
    /// EAGAIN): undecided (the caller backs off / stops typed; never "not ours").
    Unreadable(i32, String),
}

pub fn run_dir_owner(run_dir: &Path, key: &str) -> RunDirOwner {
    let p = run_dir.join("sup").join(super::CAMPAIGN_ID_FILE);
    match std::fs::read_to_string(&p) {
        Ok(s) if s.trim() == key => RunDirOwner::Ours,
        Ok(s) if s.trim().is_empty() => RunDirOwner::Unknown("RUN/sup/CAMPAIGN is empty".into()),
        Ok(s) => RunDirOwner::Other(s.trim().to_string()),
        Err(e)
            if e.kind() != std::io::ErrorKind::NotFound
                && matches!(
                    e.raw_os_error(),
                    Some(libc::EMFILE | libc::ENFILE | libc::ENOMEM | libc::EAGAIN)
                ) =>
        {
            RunDirOwner::Unreadable(
                e.raw_os_error().unwrap_or(libc::EMFILE),
                format!("RUN/sup/CAMPAIGN: {e}"),
            )
        }
        Err(e) => RunDirOwner::Unknown(format!("RUN/sup/CAMPAIGN: {e}")),
    }
}

/// Why a run directory could not be created.
#[derive(Debug)]
pub enum CreateError {
    /// Something already exists at the path (decide with [`run_dir_owner`]).
    Exists,
    Io(std::io::Error),
}

/// Rule 2: create `run_dir` with `RUN/sup/CAMPAIGN` = `key`, atomically (temporary directory + RENAME_NOREPLACE).
/// A crash leaves at most a uniquely named temporary directory carrying our identity (never at the run path).
pub fn create_run_dir(run_dir: &Path, key: &str) -> Result<(), CreateError> {
    let parent = run_dir
        .parent()
        .ok_or_else(|| CreateError::Io(std::io::Error::other("run directory without a parent")))?;
    std::fs::create_dir_all(parent).map_err(CreateError::Io)?;
    if std::fs::symlink_metadata(run_dir).is_ok() {
        return Err(CreateError::Exists);
    }
    let name = run_dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmp: PathBuf = parent.join(format!(".{name}.creating.{}.{nanos}", std::process::id()));
    std::fs::create_dir(&tmp).map_err(CreateError::Io)?;
    let sup = tmp.join("sup");
    let r = (|| -> std::io::Result<()> {
        std::fs::create_dir(&sup)?;
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(sup.join(super::CAMPAIGN_ID_FILE))?;
        f.write_all(format!("{key}\n").as_bytes())?;
        f.sync_all()?;
        std::fs::File::open(&sup)?.sync_all()?;
        std::fs::File::open(&tmp)?.sync_all()
    })();
    // Our own temporary directory (unique name, at most one file): removed again on any failure.
    let discard = || {
        #[allow(clippy::let_underscore_must_use)]
        // ignored-ok (procfix7): removal of our own temporary directory after a failure; the error returned is the real one
        let _ = std::fs::remove_file(sup.join(super::CAMPAIGN_ID_FILE));
        #[allow(clippy::let_underscore_must_use)]
        // ignored-ok (procfix7): removal of our own temporary directory after a failure; the error returned is the real one
        let _ = std::fs::remove_dir(&sup);
        #[allow(clippy::let_underscore_must_use)]
        // ignored-ok (procfix7): removal of our own temporary directory after a failure; the error returned is the real one
        let _ = std::fs::remove_dir(&tmp);
    };
    if let Err(e) = r {
        discard();
        return Err(CreateError::Io(e));
    }
    match rename_noreplace(&tmp, run_dir) {
        Ok(()) => {}
        Err(e) if e.raw_os_error() == Some(libc::EEXIST) => {
            discard();
            return Err(CreateError::Exists);
        }
        Err(e) => {
            discard();
            return Err(CreateError::Io(e));
        }
    }
    std::fs::File::open(parent)
        .and_then(|d| d.sync_all())
        .map_err(CreateError::Io)
}

/// renameat2(RENAME_NOREPLACE): EEXIST when the target exists (also an empty directory, which plain rename replaces).
fn rename_noreplace(from: &Path, to: &Path) -> std::io::Result<()> {
    let a = std::ffi::CString::new(from.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::other("NUL in path"))?;
    let b = std::ffi::CString::new(to.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::other("NUL in path"))?;
    // SAFETY: two NUL-terminated paths that live across the call; integer flags.
    let r = unsafe {
        libc::syscall(
            libc::SYS_renameat2,
            libc::AT_FDCWD,
            a.as_ptr(),
            libc::AT_FDCWD,
            b.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if r == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[allow(clippy::let_underscore_must_use)] // test cleanup (outside the reviewed scope, procfix7)
#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "rr-workdir-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    /// MJ-B rule 1: the first campaign claims; the same id is confirmed; another id is refused, naming the owner.
    #[test]
    fn mj_b_work_dir_claim() {
        let w = tmp("claim");
        assert_eq!(
            claim_work_dir(&w, "orig.0000000000000001"),
            Ok(ClaimOutcome::Claimed)
        );
        assert_eq!(
            claim_work_dir(&w, "orig.0000000000000001"),
            Ok(ClaimOutcome::Ours)
        );
        let e = claim_work_dir(&w, "orig.0000000000000002").unwrap_err();
        assert!(
            e.contains("orig.0000000000000001") && e.contains("refused"),
            "{e}"
        );
        let c = w.join("cdir");
        std::fs::create_dir_all(&c).unwrap();
        assert!(!is_external(&c.join("work"), &c));
        assert!(is_external(&w.join("elsewhere"), &c));
        let _ = std::fs::remove_dir_all(&w);
    }

    /// MJ-B rule 2: a run directory is created with its identity in one step; a second creation finds it (Exists),
    /// also when the path is an EMPTY directory (plain rename would replace it); owners are told apart.
    #[test]
    fn mj_b_run_dir_created_with_its_identity() {
        let w = tmp("rundir");
        let rd = w.join("jobs").join("j1.a1");
        create_run_dir(&rd, "camp.0000000000000001").unwrap();
        assert_eq!(
            run_dir_owner(&rd, "camp.0000000000000001"),
            RunDirOwner::Ours
        );
        assert_eq!(
            run_dir_owner(&rd, "camp.0000000000000002"),
            RunDirOwner::Other("camp.0000000000000001".into())
        );
        assert!(matches!(
            create_run_dir(&rd, "camp.0000000000000002"),
            Err(CreateError::Exists)
        ));
        let empty = w.join("jobs").join("j2.a1");
        std::fs::create_dir_all(&empty).unwrap();
        assert!(matches!(
            create_run_dir(&empty, "camp.0000000000000001"),
            Err(CreateError::Exists)
        ));
        assert!(matches!(
            run_dir_owner(&empty, "camp.0000000000000001"),
            RunDirOwner::Unknown(_)
        ));
        // no temporary directory is left behind
        let left: Vec<_> = std::fs::read_dir(w.join("jobs"))
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().contains(".creating."))
            .collect();
        assert!(left.is_empty());
        let _ = std::fs::remove_dir_all(&w);
    }
}
